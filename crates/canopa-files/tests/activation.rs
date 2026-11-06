use canopa_files::activation::{Claim, Command, Configuration, Instance};
use std::{
    ffi::OsString,
    os::unix::{ffi::OsStringExt, fs::PermissionsExt},
    path::PathBuf,
};

fn isolated_helper(name: &str) {
    let private = tempfile::tempdir().unwrap();
    std::fs::set_permissions(private.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(
        private.path().join("fixture-owned"),
        b"Canopa private activation teardown\n",
    )
    .unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", name, "--test-threads=1"])
        .env("CANOPA_ACTIVATION_TEARDOWN_FIXTURE", private.path())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert!(
        status.is_some_and(|status| status.success()),
        "private {name} must finish without hanging: {status:?}"
    );
}

#[test]
fn a_crashed_idle_owner_leaves_a_reclaimable_endpoint() {
    let private = tempfile::tempdir().unwrap();
    std::fs::set_permissions(private.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(
        private.path().join("fixture-owned"),
        b"Canopa private activation teardown\n",
    )
    .unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::activation::idle_owner_helper",
            "--test-threads=1",
        ])
        .env("CANOPA_ACTIVATION_TEARDOWN_FIXTURE", private.path())
        .spawn()
        .unwrap();
    let ready = private.path().join("ready");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    while !ready.exists()
        && std::time::Instant::now() < deadline
        && child.try_wait().unwrap().is_none()
    {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let started = ready.exists();
    // Always retire the exact owned child, including a readiness failure.
    let _ = child.kill();
    child.wait().unwrap();
    assert!(
        started,
        "private owner must claim its state before interruption"
    );
    let Claim::Primary(owner) = Instance::launch(
        &private.path().join("state"),
        private.path(),
        None,
        Command::Open(vec![]),
        true,
    )
    .unwrap() else {
        panic!("the dead owner's socket must not prevent a new owner")
    };
    let opening = owner.take_openings().pop().unwrap();
    owner.presented(opening);
    assert!(owner.try_exit());
}

#[test]
#[ignore = "private subprocess fixture"]
fn idle_owner_helper() {
    let root = PathBuf::from(
        std::env::var_os("CANOPA_ACTIVATION_TEARDOWN_FIXTURE").expect("private fixture"),
    );
    assert_eq!(
        std::fs::read(root.join("fixture-owned")).unwrap(),
        b"Canopa private activation teardown\n"
    );
    let Claim::Primary(_owner) =
        Instance::launch(&root.join("state"), &root, None, Command::Resident, true).unwrap()
    else {
        panic!("fixture owns state")
    };
    std::fs::write(root.join("ready"), b"Runtime ownership acquired\n").unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn shutdown_does_not_depend_on_the_socket_path_still_existing() {
    isolated_helper("cases::activation::replaced_endpoint_helper");
}

#[test]
#[ignore = "private subprocess fixture"]
fn replaced_endpoint_helper() {
    let root = PathBuf::from(
        std::env::var_os("CANOPA_ACTIVATION_TEARDOWN_FIXTURE").expect("private fixture"),
    );
    assert_eq!(
        std::fs::read(root.join("fixture-owned")).unwrap(),
        b"Canopa private activation teardown\n"
    );
    let Claim::Primary(owner) =
        Instance::launch(&root.join("state"), &root, None, Command::Resident, true).unwrap()
    else {
        panic!("fixture owns state")
    };
    let endpoint = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "sock"))
        .unwrap();
    std::fs::remove_file(&endpoint).unwrap();
    std::fs::write(&endpoint, b"Keep this replacement path\n").unwrap();
    drop(owner);
    assert_eq!(
        std::fs::read(endpoint).unwrap(),
        b"Keep this replacement path\n"
    );
}

#[test]
fn connection_timeout_includes_a_full_socket_accept_backlog() {
    isolated_helper("cases::activation::saturated_endpoint_helper");
}

#[test]
#[ignore = "private subprocess fixture"]
fn saturated_endpoint_helper() {
    use rustix::net::{
        AddressFamily, SocketAddrUnix, SocketFlags, SocketType, connect, socket_with,
    };
    let root = PathBuf::from(
        std::env::var_os("CANOPA_ACTIVATION_TEARDOWN_FIXTURE").expect("private fixture"),
    );
    assert_eq!(
        std::fs::read(root.join("fixture-owned")).unwrap(),
        b"Canopa private activation teardown\n"
    );
    let state = root.join("state");
    let Claim::Primary(owner) =
        Instance::launch(&state, &root, None, Command::Resident, true).unwrap()
    else {
        panic!("fixture owns state")
    };
    let endpoint = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "sock"))
        .unwrap();
    std::fs::remove_file(&endpoint).unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(&endpoint).unwrap();
    rustix::net::listen(&_listener, 1).unwrap();
    let address = SocketAddrUnix::new(&endpoint).unwrap();
    let mut clients = Vec::new();
    loop {
        assert!(clients.len() < 4096, "fixture backlog must be bounded");
        let fd = socket_with(
            AddressFamily::UNIX,
            SocketType::STREAM,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        match connect(&fd, &address) {
            Ok(()) => clients.push(fd),
            Err(rustix::io::Errno::AGAIN) => break,
            Err(error) => panic!("backlog setup: {error}"),
        }
    }
    let started = std::time::Instant::now();
    assert!(Instance::launch(&state, &root, None, Command::Quit, true).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(4));
    // This helper deliberately isolates connection establishment from teardown;
    // process exit releases its exclusively owned sockets and state lock.
    std::mem::forget(owner);
}

#[test]
fn accepted_openings_hold_the_same_owner_through_presentation() {
    let private = tempfile::tempdir().unwrap();
    let runtime = private.path().join("runtime");
    std::fs::create_dir(&runtime).unwrap();
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    let state = private.path().join("state");
    let Claim::Primary(owner) =
        Instance::launch(&state, &runtime, None, Command::Open(vec![]), true).unwrap()
    else {
        panic!("first launch must own the runtime")
    };
    let initial = owner.take_openings().pop().unwrap();
    assert!(!owner.try_exit(), "handoff retains the accepted opening");
    let raw = PathBuf::from(OsString::from_vec(b"/private/raw-\xff".to_vec()));
    assert!(matches!(
        Instance::launch(
            &state,
            &runtime,
            None,
            Command::Open(vec![raw.clone()]),
            true
        )
        .unwrap(),
        Claim::Forwarded
    ));
    owner.presented(initial);
    assert!(!owner.try_exit(), "second launch belongs to this owner");
    let second = owner.take_openings().pop().unwrap();
    assert_eq!(
        second.presentation,
        canopa_files::presentation::Presentation::open(vec![raw])
    );
    assert!(!owner.try_exit(), "taking the request cannot drop its hold");
    owner.presented(second);
    assert!(owner.try_exit());
    drop(owner);
    assert!(matches!(
        Instance::launch(&state, &runtime, None, Command::Quit, true).unwrap(),
        Claim::Absent
    ));
}

#[test]
fn resident_ownership_bounds_openings_and_keeps_its_configuration() {
    let private = tempfile::tempdir().unwrap();
    std::fs::set_permissions(private.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let state = private.path().join("state");
    let Claim::Primary(owner) = Instance::launch(
        &state,
        private.path(),
        Some(Configuration::Defaults),
        Command::Resident,
        true,
    )
    .unwrap() else {
        panic!("first resident owns its state")
    };
    assert!(owner.take_openings().is_empty());
    assert!(!owner.try_exit());
    let refused = Instance::launch(
        &state,
        private.path(),
        Some(Configuration::File(PathBuf::from("/private/other.kdl"))),
        Command::Open(vec![]),
        true,
    );
    assert!(refused.is_err());
    assert!(
        owner.take_openings().is_empty(),
        "a conflicting configuration never opens a window"
    );
    for _ in 0..64 {
        assert!(matches!(
            Instance::launch(&state, private.path(), None, Command::Open(vec![]), true).unwrap(),
            Claim::Forwarded
        ));
    }
    assert!(Instance::launch(&state, private.path(), None, Command::Open(vec![]), true).is_err());
    let mut openings = owner.take_openings();
    assert_eq!(openings.len(), 64);
    assert!(matches!(
        Instance::launch(&state, private.path(), None, Command::Quit, true).unwrap(),
        Claim::Forwarded
    ));
    let last = openings.pop().unwrap();
    for opening in openings {
        owner.presented(opening);
    }
    assert!(!owner.try_exit(), "Quit cannot lose a handed-off opening");
    owner.presented(last);
    assert!(owner.try_exit());
}

#[test]
fn a_headless_owner_refuses_browsers_and_stays_available_for_orderly_quit() {
    let private = tempfile::tempdir().unwrap();
    std::fs::set_permissions(private.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let state = private.path().join("state");
    let Claim::Primary(owner) =
        Instance::launch(&state, private.path(), None, Command::Resident, false).unwrap()
    else {
        panic!("headless resident owns its state")
    };
    assert!(Instance::launch(&state, private.path(), None, Command::Open(vec![]), true).is_err());
    assert!(owner.take_openings().is_empty());
    assert!(
        !owner.try_exit(),
        "refused presentation does not kill the headless owner"
    );
    assert!(matches!(
        Instance::launch(&state, private.path(), None, Command::Quit, false).unwrap(),
        Claim::Forwarded
    ));
    assert!(owner.try_exit());
}

#[test]
fn quick_look_forwards_exact_local_path_to_the_existing_owner() {
    let root = tempfile::tempdir().unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let state = root.path().join("state");
    let Claim::Primary(owner) =
        Instance::launch(&state, root.path(), None, Command::Resident, true).unwrap()
    else {
        panic!()
    };
    let claim = Instance::launch(
        &state,
        root.path(),
        None,
        Command::QuickLook("file:///tmp/raw-%FF.txt".into()),
        true,
    )
    .unwrap();
    assert!(matches!(claim, Claim::Forwarded));
    let opening = owner.take_openings().pop().unwrap();
    assert_eq!(
        opening.presentation.view,
        canopa_files::presentation::View::QuickLook(PathBuf::from(OsString::from_vec(
            b"/tmp/raw-\xff.txt".to_vec()
        )))
    );
    owner.presented(opening);
    assert!(owner.take_openings().is_empty());
}
