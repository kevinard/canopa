#![cfg(feature = "wayland-clipboard")]
use canopa_files::{
    file_clipboard::{GNOME, KDE, Selection, URIS, wayland::Clipboard},
    transfers::TransferKind,
};
use futures_channel::oneshot;
use std::{
    ffi::OsString,
    io::{Read, Write},
    os::unix::{ffi::OsStringExt, net::UnixListener},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

fn wait<T>(mut check: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(Instant::now() < end, "private clipboard did not reply");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn receive<T>(mut reply: oneshot::Receiver<Result<T, String>>) -> T {
    wait(|| reply.try_recv().unwrap()).unwrap_or_else(|error| panic!("{error}"))
}

fn private_home() -> PathBuf {
    let runtime = PathBuf::from(
        std::env::var_os("CANOPA_PRIVATE_CLIPBOARD_RUNTIME").expect("private fixture required"),
    );
    assert_eq!(
        std::env::var_os("XDG_RUNTIME_DIR"),
        Some(runtime.clone().into_os_string())
    );
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    assert!(home.starts_with(&runtime));
    home
}
fn ready() -> Clipboard {
    private_home();
    let clipboard = Clipboard::start().unwrap();
    wait(|| {
        let state = clipboard.snapshot();
        assert!(state.error.is_none(), "{:?}", state.error);
        state.ready.then_some(())
    });
    clipboard
}
struct Peer(Child);
impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn external(mime: &str, bytes: &[u8]) -> Peer {
    let child = Command::new("wl-copy")
        .args(["--foreground", "--type", mime])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut peer = Peer(child);
    peer.0.stdin.take().unwrap().write_all(bytes).unwrap();
    peer
}
fn peer_bytes(mime: &str) -> Vec<u8> {
    let result = Command::new("wl-paste")
        .args(["--no-newline", "--type", mime])
        .output()
        .unwrap();
    assert!(result.status.success(), "{:?}", result.stderr);
    result.stdout
}

#[test]
#[ignore = "requires the repository's private Niri clipboard fixture"]
fn overlapping_cut_remainders_cannot_reintroduce_consumed_files() {
    let home = private_home();
    let clipboard = ready();
    let a = home.join("overlap-a.txt");
    let b = home.join("overlap-b.txt");
    let c = home.join("overlap-c.txt");
    for (path, bytes) in [(&a, b"A"), (&b, b"B"), (&c, b"C")] {
        std::fs::write(path, bytes).unwrap();
    }
    let token = receive(
        clipboard
            .publish(
                Selection::new(vec![a.clone(), b.clone(), c.clone()], TransferKind::Move).unwrap(),
            )
            .unwrap(),
    );
    assert!(receive(
        clipboard
            .replace_cut(
                token,
                Some(Selection::new(vec![b.clone(), c.clone()], TransferKind::Move).unwrap())
            )
            .unwrap()
    ));
    // Another accepted Move used the earlier a/b/c selection. Its later result
    // keeps a/c, but a was already consumed by the first confirmed result.
    assert!(receive(
        clipboard
            .replace_cut(
                token,
                Some(Selection::new(vec![a.clone(), c.clone()], TransferKind::Move).unwrap())
            )
            .unwrap()
    ));
    let received = receive(clipboard.read().unwrap());
    assert_eq!(received.selection.paths(), std::slice::from_ref(&c));
    assert_eq!(received.owned, Some(token));
    assert_eq!(
        peer_bytes(GNOME),
        format!("cut\nfile://{}\n", c.to_str().unwrap()).as_bytes()
    );
    assert!(receive(
        clipboard
            .replace_cut(
                token,
                Some(Selection::new(vec![a.clone()], TransferKind::Move).unwrap())
            )
            .unwrap()
    ));
    assert!(clipboard.snapshot().owned.is_none());
    assert_eq!(std::fs::read(a).unwrap(), b"A");
    assert_eq!(std::fs::read(b).unwrap(), b"B");
    assert_eq!(std::fs::read(c).unwrap(), b"C");
}

#[test]
#[ignore = "requires the repository's private clipboard runtime"]
fn silent_display_startup_has_a_deadline_and_observes_shutdown() {
    let home = private_home();
    for mode in ["deadline", "drop"] {
        let socket = home.join(format!("silent-display-{mode}.socket"));
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let log = home.join(format!("silent-display-{mode}.log"));
        let output = std::fs::File::create(&log).unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "clipboard_startup_helper",
                "--test-threads=1",
            ])
            .env("WAYLAND_DISPLAY", &socket)
            .env("WAYLAND_SOCKET", "reserved-for-iced")
            .env("CANOPA_PRIVATE_CLIPBOARD_STARTUP", mode)
            .stdout(Stdio::from(output.try_clone().unwrap()))
            .stderr(Stdio::from(output))
            .spawn()
            .unwrap();
        let mut peer = Peer(child);
        let (mut connection, _) = wait(|| match listener.accept() {
            Ok(connection) => Some(connection),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(error) => panic!("{error}"),
        });
        connection.set_nonblocking(true).unwrap();
        std::fs::write(
            home.join(format!("clipboard-connected-{mode}")),
            b"connected",
        )
        .unwrap();
        if mode == "deadline" {
            let status = wait(|| peer.0.try_wait().unwrap());
            assert!(
                status.success(),
                "{}",
                std::fs::read_to_string(log).unwrap()
            );
        } else {
            wait(|| home.join("clipboard-dropped").exists().then_some(()));
            wait(|| {
                assert!(peer.0.try_wait().unwrap().is_none());
                let mut bytes = [0; 4096];
                match connection.read(&mut bytes) {
                    Ok(0) => Some(()),
                    Ok(_) => None,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
                    Err(error) => panic!("{error}"),
                }
            });
            std::fs::write(home.join("clipboard-shutdown-observed"), b"done").unwrap();
            assert!(wait(|| peer.0.try_wait().unwrap()).success());
        }
    }
}

#[test]
#[ignore = "child process fixture; parent accepts but never answers Wayland requests"]
fn clipboard_startup_helper() {
    let Ok(mode) = std::env::var("CANOPA_PRIVATE_CLIPBOARD_STARTUP") else {
        return;
    };
    let home = private_home();
    let clipboard = Clipboard::start().unwrap();
    if mode == "deadline" {
        let start = Instant::now();
        let mut request = clipboard.read().unwrap();
        let response = wait(|| {
            request
                .try_recv()
                .expect("startup request was silently abandoned")
        });
        assert!(response.unwrap_err().contains("starting"));
        let failure = wait(|| {
            assert!(
                start.elapsed() < Duration::from_secs(8),
                "startup had no deadline"
            );
            clipboard.snapshot().error
        });
        assert!(failure.contains("did not respond"), "{failure}");
        assert!(!clipboard.snapshot().ready);
    } else {
        wait(|| home.join("clipboard-connected-drop").exists().then_some(()));
        drop(clipboard);
        std::fs::write(home.join("clipboard-dropped"), b"dropped").unwrap();
        wait(|| {
            home.join("clipboard-shutdown-observed")
                .exists()
                .then_some(())
        });
    }
    assert_eq!(
        std::env::var("WAYLAND_SOCKET").unwrap(),
        "reserved-for-iced"
    );
}

#[test]
#[ignore = "requires the repository's private Niri clipboard fixture"]
fn partial_cut_updates_preserve_newer_external_and_foreign_selections() {
    let home = private_home();
    let first = ready();
    let a = home.join("cut-a.txt");
    let b = home.join("cut-b.txt");
    let c = home.join("newer-copy.txt");
    for (path, bytes) in [(&a, b"A"), (&b, b"B"), (&c, b"C")] {
        std::fs::write(path, bytes).unwrap();
    }
    let token = receive(
        first
            .publish(Selection::new(vec![a.clone(), b.clone()], TransferKind::Move).unwrap())
            .unwrap(),
    );
    assert!(receive(
        first
            .replace_cut(
                token,
                Some(Selection::new(vec![b.clone()], TransferKind::Move).unwrap())
            )
            .unwrap()
    ));
    assert_eq!(
        peer_bytes(GNOME),
        format!("cut\nfile://{}\n", b.to_str().unwrap()).into_bytes()
    );
    let received = receive(first.read().unwrap());
    assert_eq!(received.selection.paths(), std::slice::from_ref(&b));
    assert_eq!(received.owned, Some(token));
    let foreign = ready();
    let newer = receive(
        foreign
            .publish(Selection::new(vec![c.clone()], TransferKind::Move).unwrap())
            .unwrap(),
    );
    assert_ne!(newer, token);
    assert!(!receive(foreign.replace_cut(token, None).unwrap()));
    assert_eq!(foreign.snapshot().owned, Some(newer));
    let list = format!("file://{}\r\n", c.to_str().unwrap());
    let _peer = external(URIS, list.as_bytes());
    wait(|| foreign.snapshot().owned.is_none().then_some(()));
    assert!(!receive(foreign.replace_cut(newer, None).unwrap()));
    assert!(!receive(first.replace_cut(token, None).unwrap()));
    let received = receive(first.read().unwrap());
    assert_eq!(received.selection.paths(), std::slice::from_ref(&c));
    assert_eq!(received.selection.kind(), TransferKind::Copy);
    assert_eq!(received.owned, None);
    assert_eq!(peer_bytes(URIS), list.as_bytes());
    assert_eq!(std::fs::read(a).unwrap(), b"A");
    assert_eq!(std::fs::read(b).unwrap(), b"B");
    assert_eq!(std::fs::read(c).unwrap(), b"C");
}

fn stopped_peer(clipboard: &Clipboard, name: &str) -> Peer {
    let home = private_home();
    let marker = home.join(name);
    let before = clipboard.snapshot().revision;
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "clipboard_stalled_owner_helper",
            "--test-threads=1",
        ])
        .env("CANOPA_PRIVATE_CLIPBOARD_PEER_READY", &marker)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut peer = Peer(child);
    wait(|| {
        assert!(
            peer.0.try_wait().unwrap().is_none(),
            "private owner exited before setup"
        );
        (marker.exists() && clipboard.snapshot().revision > before).then_some(())
    });
    assert!(
        Command::new("kill")
            .args(["-STOP", &peer.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    peer
}

#[test]
#[ignore = "requires the repository's private Niri clipboard fixture"]
fn an_unresponsive_owner_has_a_deadline_and_does_not_block_clipboard_dispatch() {
    let home = private_home();
    let clipboard = ready();
    let peer = stopped_peer(&clipboard, "stalled-first.ready");
    let start = Instant::now();
    let mut read = clipboard.read().unwrap();
    let result = wait(|| read.try_recv().unwrap());
    assert!(result.unwrap_err().contains("did not respond"));
    assert!(start.elapsed() < Duration::from_secs(8));
    drop(peer);
    let replacement =
        Selection::new(vec![home.join("responsive.txt")], TransferKind::Copy).unwrap();
    receive(clipboard.publish(replacement.clone()).unwrap());
    let _peer = stopped_peer(&clipboard, "stalled-second.ready");
    let mut read = clipboard.read().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert!(read.try_recv().unwrap().is_none());
    let start = Instant::now();
    let token = receive(clipboard.publish(replacement).unwrap());
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "a peer read blocked new Copy"
    );
    assert_eq!(clipboard.snapshot().owned, Some(token));
    assert!(
        wait(|| read.try_recv().unwrap())
            .unwrap_err()
            .contains("clipboard changed")
    );
}

#[test]
#[ignore = "child process fixture; parent pauses this actual clipboard owner"]
fn clipboard_stalled_owner_helper() {
    let Some(marker) = std::env::var_os("CANOPA_PRIVATE_CLIPBOARD_PEER_READY") else {
        return;
    };
    let home = private_home();
    let clipboard = ready();
    receive(
        clipboard
            .publish(
                Selection::new(vec![home.join("paused-source.txt")], TransferKind::Copy).unwrap(),
            )
            .unwrap(),
    );
    std::fs::write(marker, b"ready").unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
#[ignore = "requires the repository's private Niri clipboard fixture"]
fn oversized_external_data_is_refused_without_poisoning_later_reads() {
    let home = private_home();
    let clipboard = ready();
    let before = clipboard.snapshot().revision;
    let oversized = vec![b'x'; canopa_files::file_clipboard::LIMIT + 1];
    let peer = external(URIS, &oversized);
    wait(|| (clipboard.snapshot().revision > before).then_some(()));
    let mut read = clipboard.read().unwrap();
    assert!(
        wait(|| read.try_recv().unwrap())
            .unwrap_err()
            .contains("too large")
    );
    drop(peer);
    let uri = format!(
        "copy\nfile://{}\n",
        home.join("after-limit.txt").to_str().unwrap()
    );
    let before = clipboard.snapshot().revision;
    let _peer = external(GNOME, uri.as_bytes());
    wait(|| {
        (clipboard.snapshot().revision > before && clipboard.snapshot().has_files).then_some(())
    });
    let received = receive(clipboard.read().unwrap());
    assert_eq!(received.selection.paths(), &[home.join("after-limit.txt")]);
    assert_eq!(received.selection.kind(), TransferKind::Copy);
}

#[test]
#[ignore = "requires the repository's private Niri clipboard fixture"]
fn native_file_selection_is_available_to_independent_peers_and_keeps_raw_paths() {
    let runtime = PathBuf::from(
        std::env::var_os("CANOPA_PRIVATE_CLIPBOARD_RUNTIME").expect("private fixture required"),
    );
    assert_eq!(
        std::env::var_os("XDG_RUNTIME_DIR"),
        Some(runtime.clone().into_os_string())
    );
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    assert!(home.starts_with(&runtime));
    let clipboard = Clipboard::start().unwrap();
    wait(|| {
        let state = clipboard.snapshot();
        assert!(state.error.is_none(), "{:?}", state.error);
        state.ready.then_some(())
    });
    let path = home.join(OsString::from_vec(b"caf\xc3\xa9-\xff.txt".to_vec()));
    std::fs::write(&path, b"Private clipboard file bytes").unwrap();
    let uri = format!("file://{}/caf%C3%A9-%FF.txt", home.to_str().unwrap());
    let selection = Selection::new(vec![path.clone()], TransferKind::Move).unwrap();
    let token = receive(clipboard.publish(selection).unwrap());
    assert_eq!(clipboard.snapshot().owned, Some(token));
    let types = Command::new("wl-paste")
        .arg("--list-types")
        .output()
        .unwrap();
    assert!(types.status.success());
    let types = String::from_utf8(types.stdout).unwrap();
    for mime in [GNOME, URIS, KDE] {
        assert!(types.lines().any(|line| line == mime), "{types}");
    }
    for (mime, expected) in [
        (GNOME, format!("cut\n{uri}\n").into_bytes()),
        (URIS, format!("{uri}\r\n").into_bytes()),
        (KDE, b"1".to_vec()),
    ] {
        let peer = Command::new("wl-paste")
            .args(["--no-newline", "--type", mime])
            .output()
            .unwrap();
        assert!(peer.status.success(), "{:?}", peer.stderr);
        assert_eq!(peer.stdout, expected);
    }
    let received = receive(clipboard.read().unwrap());
    assert_eq!(
        std::fs::read(&received.selection.paths()[0]).unwrap(),
        b"Private clipboard file bytes"
    );
    assert_eq!(received.selection.paths(), &[path]);
    assert_eq!(received.selection.kind(), TransferKind::Move);
    assert_eq!(received.owned, Some(token));
    assert!(receive(clipboard.replace_cut(token, None).unwrap()));
    wait(|| clipboard.snapshot().owned.is_none().then_some(()));
}
