#![cfg(feature = "local-runtime")]
use canopa_files::{
    activation::{Claim, Command, Instance},
    file_manager::{Host, NAME, PATH},
    presentation::View,
};
use std::{
    fs,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::PathBuf,
    time::{Duration, Instant},
};

#[test]
fn standard_requests_use_the_owned_headless_runtime_and_respect_other_owners() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("fixture-owned"),
        b"private FileManager1 test\n",
    )
    .unwrap();
    let configuration = root.path().join("bus.conf");
    fs::write(&configuration, format!("<busconfig><type>session</type><listen>unix:tmpdir={}</listen><policy context=\"default\"><allow send_destination=\"*\"/><allow receive_sender=\"*\"/><allow own=\"*\"/></policy></busconfig>", root.path().display())).unwrap();
    let mut child = std::process::Command::new("dbus-run-session")
        .arg("--config-file")
        .arg(configuration)
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "file_manager_bus_helper",
            "--test-threads=1",
        ])
        .env("CANOPA_FILE_MANAGER_FIXTURE", root.path())
        .process_group(0)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            let _ = rustix::process::kill_process_group(
                rustix::process::Pid::from_child(&child),
                rustix::process::Signal::KILL,
            );
            child.wait().unwrap();
            panic!("private FileManager1 session timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "private session-bus subprocess"]
fn file_manager_bus_helper() {
    let root =
        PathBuf::from(std::env::var_os("CANOPA_FILE_MANAGER_FIXTURE").expect("private fixture"));
    assert_eq!(
        fs::read(root.join("fixture-owned")).unwrap(),
        b"private FileManager1 test\n"
    );
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let Claim::Primary(owner) = Instance::launch(
        &root.join("state"),
        &root,
        None,
        Command::Open(vec![]),
        true,
    )
    .unwrap() else {
        panic!()
    };
    let initial = owner.take_openings().pop().unwrap();
    owner.presented(initial);
    let host = Host::start(owner.clone())
        .unwrap()
        .expect("unowned private name");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let client = zbus::Connection::session().await.unwrap();
        let call = async |method: &str| {
            client
                .call_method(
                    Some(NAME),
                    PATH,
                    Some(NAME),
                    method,
                    &(
                        vec![
                            "file:///tmp/private/raw-%FF.txt",
                            "file:///tmp/private/.hidden",
                        ],
                        "startup_TIME1",
                    ),
                )
                .await
        };
        call("ShowItems").await.unwrap();
        let request = owner.take_openings().pop().unwrap();
        assert!(
            !owner.try_exit(),
            "accepted D-Bus intent retains its handoff hold"
        );
        let View::Browsers(targets) = &request.presentation.view else {
            panic!()
        };
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].selected.len(), 2);
        owner.presented(request);
        call("ShowFolders").await.unwrap();
        let request = owner.take_openings().pop().unwrap();
        let View::Browsers(targets) = &request.presentation.view else {
            panic!()
        };
        assert_eq!(targets.len(), 2);
        assert!(targets.iter().all(|t| t.selected.is_empty()));
        owner.presented(request);
        call("ShowItemProperties").await.unwrap();
        let request = owner.take_openings().pop().unwrap();
        assert!(matches!(&request.presentation.view, View::Properties(paths) if paths.len()==2));
        owner.presented(request);
        assert!(
            client
                .call_method(
                    Some(NAME),
                    PATH,
                    Some(NAME),
                    "ShowItems",
                    &(vec!["file:///tmp/good", "https://example.test/remote"], "")
                )
                .await
                .is_err()
        );
        assert!(owner.take_openings().is_empty());
        assert!(owner.try_exit());
        assert!(
            call("ShowItems").await.is_err(),
            "shutdown admission is shared with D-Bus"
        );
        host.shutdown().unwrap();
        let other = zbus::Connection::session().await.unwrap();
        other
            .request_name_with_flags(NAME, zbus::fdo::RequestNameFlags::DoNotQueue.into())
            .await
            .unwrap();
        let Claim::Primary(other_runtime) = Instance::launch(
            &root.join("other-state"),
            &root,
            None,
            Command::Resident,
            true,
        )
        .unwrap() else {
            panic!()
        };
        assert!(
            Host::start(other_runtime).unwrap().is_none(),
            "an existing name is never replaced or queued behind"
        );
        let proxy = zbus::fdo::DBusProxy::new(&client).await.unwrap();
        assert_eq!(
            proxy
                .get_name_owner(NAME.try_into().unwrap())
                .await
                .unwrap()
                .as_str(),
            other.unique_name().unwrap().as_str()
        );
        other.release_name(NAME).await.unwrap();
        let Claim::Primary(headless) = Instance::launch(
            &root.join("headless-state"),
            &root,
            None,
            Command::Resident,
            false,
        )
        .unwrap() else {
            panic!()
        };
        let headless_host = Host::start(headless.clone()).unwrap().unwrap();
        for method in ["ShowFolders", "ShowItems", "ShowItemProperties"] {
            let error = call(method).await.unwrap_err();
            assert!(
                error.to_string().contains("without a display"),
                "{method}: {error}"
            );
        }
        assert!(headless.take_openings().is_empty());
        headless_host.shutdown().unwrap();
    });
}
