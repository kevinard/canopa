//! The operations engine on a real GVFS FUSE location, without root or a
//! server: a private D-Bus session and runtime folder start their own `gvfsd`
//! and `gvfsd-fuse`, and GVFS's `localtest` backend serves a private folder
//! through the same daemon path SMB, SFTP, WebDAV and MTP use. Ignored by
//! default because it needs GVFS with FUSE, `dbus-run-session` and `/dev/fuse`:
//!
//! `cargo test -p canopa-files --test integration cases::network_gvfs:: -- --ignored`
use canopa_files::{
    Cancellation,
    browse::Directory,
    mounts::{Mounts, Placement},
    operations::OperationEngine,
    transfers::{TransferEvent, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const CHILD: &str = "CANOPA_FILES_GVFS_CHILD";

fn available() -> bool {
    Path::new("/dev/fuse").exists()
        && Path::new("/usr/lib/gvfsd-fuse").exists()
        && Path::new("/usr/share/gvfs/mounts/localtest.mount").exists()
        && Command::new("dbus-run-session")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
}

/// Run `name` again inside a private session bus and runtime folder, so the
/// GVFS daemons it starts never touch the developer's session.
fn in_private_session(name: &str) -> bool {
    if std::env::var_os(CHILD).is_some() {
        return true;
    }
    if !available() {
        eprintln!("skipped: GVFS with FUSE, its localtest backend or dbus-run-session is missing");
        return false;
    }
    let runtime = tempfile::Builder::new()
        .prefix("canopa-files-gvfs-runtime-")
        .tempdir()
        .unwrap();
    let status = Command::new("dbus-run-session")
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            name,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, "1")
        .env("XDG_RUNTIME_DIR", runtime.path())
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .status()
        .unwrap();
    // The private gvfsd-fuse exits with its bus; wait for its mount to go.
    let fuse = runtime.path().join("gvfs");
    let deadline = Instant::now() + Duration::from_secs(10);
    while is_mount_point(&fuse) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    if is_mount_point(&fuse) {
        let _ = Command::new("fusermount3").arg("-u").arg(&fuse).status();
    }
    assert!(status.success(), "the private GVFS session failed");
    false
}

fn is_mount_point(path: &Path) -> bool {
    fs::read_to_string("/proc/self/mountinfo").is_ok_and(|mounts| {
        mounts
            .lines()
            .any(|line| line.split(' ').nth(4) == path.to_str())
    })
}

fn gio(arguments: &[&str]) {
    let status = Command::new("gio").args(arguments).status().unwrap();
    assert!(status.success(), "gio {arguments:?} failed");
}

/// Mount `localtest:///` and return the FUSE folder that shows `backing`.
fn mount(backing: &Path) -> PathBuf {
    gio(&["mount", "localtest:///"]);
    let runtime = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
    let remote = runtime
        .join("gvfs/localtest:")
        .join(backing.strip_prefix("/").unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    while !remote.is_dir() {
        assert!(
            Instant::now() < deadline,
            "the GVFS FUSE folder did not appear"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    remote
}

fn copy(engine: &OperationEngine, sources: Vec<PathBuf>, destination: &Path, kind: TransferKind) {
    let outcome = engine
        .transfer(
            &TransferRequest {
                kind,
                sources,
                destination: destination.to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(!outcome.cancelled && !outcome.needs_attention);
}

fn names(path: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
#[ignore = "needs GVFS with FUSE and dbus-run-session; runs in a private session"]
fn copy_move_trash_and_delete_through_gvfs_fuse() {
    if !in_private_session("cases::network_gvfs::copy_move_trash_and_delete_through_gvfs_fuse") {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let backing = root.path().join("server");
    let local = root.path().join("local");
    let data_home = root.path().join("data");
    fs::create_dir_all(backing.join("existing")).unwrap();
    fs::create_dir_all(local.join("album/nested")).unwrap();
    fs::create_dir_all(&data_home).unwrap();
    fs::write(local.join("album/a.txt"), b"first").unwrap();
    fs::write(local.join("album/nested/b.bin"), vec![7u8; 3 << 20]).unwrap();
    fs::write(backing.join("existing/remote.txt"), b"on the server").unwrap();
    let remote = mount(&backing);

    // Browsing and placement: a GVFS folder is remote, so no thumbnails by
    // default, no executable trust and no inotify.
    let mounts = Mounts::read();
    assert!(mounts.remote(&remote));
    assert_eq!(mounts.facts(&remote).placement, Placement::Network);
    assert!(!mounts.facts(&remote).meaningful_executable_bit());
    let listed = Directory::read(&remote).unwrap();
    assert_eq!(listed.entries.len(), 1);

    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    // Copy onto the share: the batched barrier ends a finished Copy, and the
    // output is read back through GVFS before it is published.
    copy(
        &engine,
        vec![local.join("album")],
        &remote,
        TransferKind::Copy,
    );
    assert_eq!(fs::read(backing.join("album/a.txt")).unwrap(), b"first");
    assert_eq!(
        fs::read(backing.join("album/nested/b.bin")).unwrap(),
        vec![7u8; 3 << 20]
    );
    assert_eq!(
        names(&backing),
        ["album", "existing"],
        "no stage folder remains"
    );
    // Undo removes exactly the unchanged copies from the server.
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(names(&backing), ["existing"]);

    // Move from the server to a local folder: the source goes only after the
    // local copy is committed.
    copy(
        &engine,
        vec![remote.join("existing/remote.txt")],
        &local,
        TransferKind::Move,
    );
    assert_eq!(
        fs::read(local.join("remote.txt")).unwrap(),
        b"on the server"
    );
    assert!(!backing.join("existing/remote.txt").exists());

    // No Trash on GVFS locations: the file stays and is reported for the
    // confirmed permanent deletion of ADR0164.
    fs::write(backing.join("existing/old.txt"), b"old").unwrap();
    let old = remote.join("existing/old.txt");
    let trashed = engine
        .trash_files(
            &data_home,
            std::slice::from_ref(&old),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(trashed.unavailable, vec![old.clone()]);
    assert_eq!(trashed.completed, 0);
    assert!(backing.join("existing/old.txt").exists());
    assert!(
        !fs::read_dir(&data_home)
            .unwrap()
            .any(|entry| entry.unwrap().file_name() == "Trash"),
        "nothing went to the home Trash"
    );
    let deleted = engine
        .delete_permanently(&[old], &Cancellation::default())
        .unwrap();
    assert_eq!(deleted.completed.len(), 1, "{:?}", deleted.failures);
    assert!(!backing.join("existing/old.txt").exists());
    gio(&["mount", "-u", "localtest:///"]);
}

#[test]
#[ignore = "needs GVFS with FUSE and dbus-run-session; runs in a private session"]
fn a_disconnected_share_pauses_the_copy_for_attention() {
    if !in_private_session(
        "cases::network_gvfs::a_disconnected_share_pauses_the_copy_for_attention",
    ) {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let backing = root.path().join("server");
    let local = root.path().join("local");
    fs::create_dir_all(&backing).unwrap();
    fs::create_dir_all(&local).unwrap();
    let sources: Vec<_> = (0..24)
        .map(|index| {
            let path = local.join(format!("photo-{index:02}.jpg"));
            fs::write(&path, vec![index as u8; 512 * 1024]).unwrap();
            path
        })
        .collect();
    let remote = mount(&backing);
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let mut disconnected = false;
    let mut asked = None;
    let outcome = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources,
                destination: remote.clone(),
            },
            &Cancellation::default(),
            |event| match event {
                TransferEvent::Progress { completed, .. } if completed >= 3 && !disconnected => {
                    disconnected = true;
                    // The server goes away while the copy runs.
                    gio(&["mount", "-u", "localtest:///"]);
                    TransferReply::Continue
                }
                TransferEvent::Unavailable { message, .. } => {
                    asked = Some(message);
                    TransferReply::Cancel
                }
                _ => TransferReply::Continue,
            },
        )
        .unwrap();
    assert!(disconnected);
    // The interrupted item is not a failure: the copy stops for attention
    // and keeps the rest as its continuation (ADR0165), so Retry resumes it
    // once the share is back.
    assert!(
        outcome.needs_attention,
        "completed {} failures {:?} attention {:?}",
        outcome.completed, outcome.failures, outcome.attention
    );
    let attention = outcome.attention.clone().unwrap_or_default();
    assert!(attention.contains("no longer reachable"), "{attention}");
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(outcome.continuation.is_some());
    assert!(outcome.completed >= 3 && outcome.completed < 24);
    assert!(asked.is_none() || !outcome.cancelled);
    // Everything published on the server is complete; nothing is half written.
    for entry in fs::read_dir(&backing).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("photo-") {
            assert_eq!(entry.metadata().unwrap().len(), 512 * 1024, "{name}");
        }
    }
}
