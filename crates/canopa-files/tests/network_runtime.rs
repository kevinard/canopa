//! Connect to Server through the real Files GIO helper, GVFS and an SFTP
//! server, without root and without the developer's session or files:
//!
//! - a private `sshd` on a loopback port serves only this user, with its own
//!   host and client keys;
//! - an `ssh` wrapper first in `PATH` gives GVFS that client key, a private
//!   known-hosts file and a host-key alias, so the server's unknown identity is
//!   asked through the mount operation like a first connection to a real host;
//! - a private D-Bus session and runtime folder start their own `gvfsd` and
//!   `gvfsd-fuse`.
//!
//! Opt in with `CANOPA_FILES_NATIVE_NETWORK=1`; it needs OpenSSH's `sshd` and
//! `sftp-server`, GVFS with its SFTP backend and FUSE, and `dbus-run-session`.
//! The custom harness also hosts the pinned helper entrypoint.
use canopa_files::{
    application::Application,
    local_volumes::{
        self, DeviceAnswer, DeviceOutcome, DevicePhase, DeviceQuestion, DeviceRequestId,
        DeviceRequestSnapshot, DeviceRequests, DeviceStatus, LocalOperations, LocationKind,
    },
    network::ServerAddress,
    operation_jobs::{Phase, Request},
    transfers::{TransferKind, TransferRequest},
};
use std::{
    fs,
    future::Future,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, Stdio},
    task::{Context, Poll},
    thread,
    time::{Duration, Instant},
};

const OPT_IN: &str = "CANOPA_FILES_NATIVE_NETWORK";
const CHILD: &str = "CANOPA_NETWORK_RUNTIME_CHILD";

fn wait<T>(what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn complete<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(futures_util::task::noop_waker_ref());
    wait("shutdown", || match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => Some(value),
        Poll::Pending => None,
    })
}

fn job(requests: &DeviceRequests, id: DeviceRequestId) -> DeviceRequestSnapshot {
    requests
        .snapshot()
        .into_iter()
        .find(|job| job.id == id)
        .unwrap()
}

fn finished(requests: &DeviceRequests, id: DeviceRequestId) -> DeviceRequestSnapshot {
    wait("a finished request", || {
        Some(job(requests, id)).filter(|job| job.phase == DevicePhase::Finished)
    })
}

fn settled(runtime: &LocalOperations, id: u64) -> canopa_files::operation_jobs::JobSnapshot {
    wait("a settled operation", || {
        runtime.jobs().snapshot().jobs.into_iter().find(|job| {
            job.id == id && matches!(job.phase, Phase::Finished | Phase::Stopped | Phase::Failed)
        })
    })
}

fn submit(runtime: &LocalOperations, request: Request) -> u64 {
    let mut accepted = runtime.jobs().submit(request, None).unwrap();
    wait("admission", || accepted.try_recv().unwrap()).unwrap()
}

fn exercise(root: &Path) {
    let port: u16 = std::env::var("CANOPA_NETWORK_RUNTIME_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let closed: u16 = std::env::var("CANOPA_NETWORK_RUNTIME_CLOSED")
        .unwrap()
        .parse()
        .unwrap();
    let backing = root.join("server");
    let application =
        Application::start(&root.join("state"), &canopa_config::Request::Defaults).unwrap();
    let runtime = application.operations.as_ref().unwrap().clone();
    let requests = runtime.device_requests();
    drop(runtime.take_notices());
    wait("the device catalog", || {
        (runtime.devices().status == DeviceStatus::Ready).then_some(())
    });

    // 1. A first connection asks to confirm the server's identity.
    let address = ServerAddress::parse(&format!(
        "ssh://127.0.0.1:{port}{}",
        backing.to_str().unwrap()
    ))
    .unwrap();
    assert!(address.uri.starts_with("sftp://"));
    let id = requests.connect(&address).unwrap();
    let prompt = wait("the host identity question", || {
        let job = job(&requests, id);
        assert_ne!(
            job.phase,
            DevicePhase::Finished,
            "finished without asking: {:?}",
            job.outcome
        );
        job.question
    });
    let DeviceQuestion::Choice {
        message, choices, ..
    } = &prompt.question
    else {
        panic!("expected a choice, got {:?}", prompt.question);
    };
    assert!(message.contains("identity"), "{message}");
    let login = choices
        .iter()
        .position(|choice| choice == "Log In Anyway")
        .unwrap_or_else(|| panic!("{choices:?}"));
    requests
        .answer(prompt.token, DeviceAnswer::Choice { choice: login })
        .unwrap();
    let connected = finished(&requests, id);
    let Some(DeviceOutcome::Connected {
        root: mount,
        location,
    }) = connected.outcome
    else {
        panic!("{:?}", connected.outcome);
    };
    assert!(mount.starts_with(root.join("runtime/gvfs")), "{mount:?}");
    assert_eq!(requests.mounted_root(id).unwrap(), location);
    assert_eq!(fs::read(location.join("hello.txt")).unwrap(), b"from sftp");
    let device = runtime
        .devices()
        .devices
        .into_iter()
        .find(|device| device.root.as_deref() == Some(mount.as_path()))
        .expect("the mount is in the catalog");
    assert_eq!(device.kind, LocationKind::Network);
    assert!(
        device
            .uri
            .as_deref()
            .is_some_and(|uri| uri.starts_with("sftp://"))
    );
    assert!(!device.remote_only);
    assert!(
        fs::read_to_string(root.join("known_hosts"))
            .unwrap()
            .contains("canopa-test-server"),
        "SSH remembered the confirmed identity in the private file"
    );

    // 2. The operation queue copies onto the share through its Storage guard,
    // and the share has no Trash.
    let local = root.join("local");
    fs::create_dir_all(&local).unwrap();
    fs::write(local.join("upload.bin"), vec![3u8; 2 << 20]).unwrap();
    let copy = submit(
        &runtime,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![local.join("upload.bin")],
            destination: location.clone(),
        }),
    );
    let copied = settled(&runtime, copy);
    assert_eq!(copied.phase, Phase::Finished, "{:?}", copied.error);
    assert_eq!(
        fs::read(backing.join("upload.bin")).unwrap(),
        vec![3u8; 2 << 20]
    );
    let trash = submit(
        &runtime,
        Request::Trash {
            data_home: root.join("data"),
            sources: vec![location.join("hello.txt")],
        },
    );
    let report = settled(&runtime, trash).trash.expect("a Trash report");
    assert_eq!(report.unavailable, vec![location.join("hello.txt")]);
    assert!(backing.join("hello.txt").exists());

    // 3. Disconnect from the sidebar's Unmount.
    let removal = requests.remove(device.token).unwrap();
    assert!(matches!(
        finished(&requests, removal).outcome,
        Some(DeviceOutcome::Removed)
    ));
    assert!(requests.mounted_root(id).is_err());
    wait("the share to leave the catalog", || {
        (!runtime
            .devices()
            .devices
            .iter()
            .any(|device| device.root.as_deref() == Some(mount.as_path())))
        .then_some(())
    });

    // 4. An offline server and a missing backend explain themselves.
    let offline = requests
        .connect(&ServerAddress::parse(&format!("sftp://127.0.0.1:{closed}/")).unwrap())
        .unwrap();
    let Some(DeviceOutcome::Failed { message }) = finished(&requests, offline).outcome else {
        panic!("an offline server must fail");
    };
    assert!(message.contains("127.0.0.1"), "{message}");
    if !Path::new("/usr/share/gvfs/mounts/nfs.mount").exists() {
        let nfs = requests
            .connect(&ServerAddress::parse("nfs://127.0.0.1/export").unwrap())
            .unwrap();
        let Some(DeviceOutcome::Failed { message }) = finished(&requests, nfs).outcome else {
            panic!("a missing backend must fail");
        };
        assert!(message.contains("gvfs-nfs"), "{message}");
    }

    // 5. Network discovery settles, whether or not anything is announced.
    let listing = requests.enumerate("network:///").unwrap();
    let discovered = wait("network discovery", || {
        requests.listing(listing).and_then(|listing| listing.result)
    });
    println!(
        "network_runtime: network:/// listed {}",
        discovered.map_or_else(
            |error| error,
            |listed| format!("{} entries", listed.entries.len())
        )
    );
    complete(application.shutdown()).unwrap();
    println!(
        "network_runtime: connect, host identity, copy, no Trash, disconnect, offline and missing backend checked"
    );
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn keygen(path: &Path) {
    assert!(
        Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}

fn main() {
    if std::env::args().any(|arg| arg == "--internal-volume-monitor") {
        local_volumes::run_helper().unwrap();
        return;
    }
    if std::env::args().any(|arg| arg == "--list") {
        println!("network_runtime: test");
        return;
    }
    if let Some(root) = std::env::var_os(CHILD) {
        exercise(Path::new(&root));
        return;
    }
    let required = [
        "/usr/bin/sshd",
        "/usr/lib/ssh/sftp-server",
        "/usr/lib/gvfsd-sftp",
        "/usr/lib/gvfsd-fuse",
        "/dev/fuse",
    ];
    if std::env::var_os(OPT_IN).is_none() {
        println!("network_runtime: skipped; set {OPT_IN}=1 to run against a private SFTP server");
        return;
    }
    if let Some(missing) = required.iter().find(|path| !Path::new(path).exists()) {
        println!("network_runtime: skipped; {missing} is missing");
        return;
    }
    // ssh's ControlPath must fit in a socket address, so keep the root short.
    let temporary = tempfile::Builder::new()
        .prefix("cfn-")
        .tempdir_in("/tmp")
        .unwrap();
    let root = temporary.path();
    for name in [
        "runtime", "home", "config", "data", "cache", "state", "bin", "server",
    ] {
        fs::create_dir_all(root.join(name)).unwrap();
    }
    fs::set_permissions(root.join("runtime"), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(root.join("server/hello.txt"), b"from sftp").unwrap();
    keygen(&root.join("host_key"));
    keygen(&root.join("client_key"));
    fs::copy(root.join("client_key.pub"), root.join("authorized_keys")).unwrap();
    let port = free_port();
    fs::write(
        root.join("sshd_config"),
        format!(
            "Port {port}\nListenAddress 127.0.0.1\nHostKey {0}/host_key\nPidFile {0}/sshd.pid\n\
             AuthorizedKeysFile {0}/authorized_keys\nUsePAM no\nStrictModes no\n\
             PasswordAuthentication no\nKbdInteractiveAuthentication no\n\
             Subsystem sftp /usr/lib/ssh/sftp-server\n",
            root.display()
        ),
    )
    .unwrap();
    let wrapper = root.join("bin/ssh");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nexec /usr/bin/ssh -F none -i {0}/client_key -o IdentitiesOnly=yes \
             -o UserKnownHostsFile={0}/known_hosts -o GlobalKnownHostsFile=/dev/null \
             -o HostKeyAlias=canopa-test-server \"$@\"\n",
            root.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let server = Server(
        Command::new("/usr/bin/sshd")
            .arg("-D")
            .arg("-e")
            .arg("-f")
            .arg(root.join("sshd_config"))
            .stdout(Stdio::null())
            .stderr(fs::File::create(root.join("sshd.log")).unwrap())
            .spawn()
            .unwrap(),
    );
    wait("sshd", || {
        std::net::TcpStream::connect(("127.0.0.1", port)).ok()
    });
    let path = format!(
        "{}:{}",
        root.join("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut child = Command::new("dbus-run-session")
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .env(CHILD, root)
        .env("CANOPA_NETWORK_RUNTIME_PORT", port.to_string())
        .env("CANOPA_NETWORK_RUNTIME_CLOSED", free_port().to_string())
        .env("PATH", path)
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_RUNTIME_DIR", root.join("runtime"))
        .env("GIO_USE_VOLUME_MONITOR", "unix")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env_remove("SSH_AUTH_SOCK")
        .env_remove("SSH_ASKPASS")
        .env_remove("SSH_ASKPASS_REQUIRE")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break None;
        }
        thread::sleep(Duration::from_millis(20));
    };
    drop(server);
    // The private gvfsd-fuse leaves with its bus; never leave a mount behind.
    let fuse = root.join("runtime/gvfs");
    let gone = || {
        !fs::read_to_string("/proc/self/mountinfo")
            .unwrap_or_default()
            .contains(fuse.to_str().unwrap())
    };
    let until = Instant::now() + Duration::from_secs(10);
    while !gone() && Instant::now() < until {
        thread::sleep(Duration::from_millis(50));
    }
    if !gone() {
        let _ = Command::new("fusermount3").arg("-u").arg(&fuse).status();
    }
    if status.is_none_or(|status| !status.success()) {
        eprintln!(
            "{}",
            fs::read_to_string(root.join("sshd.log")).unwrap_or_default()
        );
        panic!("the private network runtime failed or timed out");
    }
}
