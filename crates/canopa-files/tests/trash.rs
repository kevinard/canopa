use canopa_files::{Cancellation, operations::OperationEngine, trash::TrashLocation};
use std::{
    fs,
    os::unix::{ffi::OsStringExt, fs::symlink},
};

#[test]
fn trash_preserves_raw_names_links_and_contents_and_undo_survives_restart() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    let raw = std::ffi::OsString::from_vec(b"note-\xff\n%.txt".to_vec());
    fs::write(source.join(&raw), b"recoverable bytes").unwrap();
    symlink("missing-target", source.join("link")).unwrap();
    let location = TrashLocation::home(&data).unwrap();
    let history = root.path().join("state/operations.sqlite3");
    let engine = OperationEngine::open(&history).unwrap();
    let outcome = engine
        .trash(
            &location,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(outcome.failures.is_empty());
    assert_eq!(outcome.completed, 1);
    assert!(!source.exists());
    let items = location.list(&Cancellation::default()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].original.as_deref(), Some(source.as_path()));
    assert_eq!(
        fs::read(items[0].path.join(&raw)).unwrap(),
        b"recoverable bytes"
    );
    assert_eq!(
        fs::read_link(items[0].path.join("link")).unwrap(),
        std::path::Path::new("missing-target")
    );
    assert!(
        items[0]
            .deletion_date
            .as_ref()
            .is_some_and(|date| date.len() == 19)
    );
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(source.join(raw)).unwrap(), b"recoverable bytes");
    assert!(location.list(&Cancellation::default()).unwrap().is_empty());
    assert_eq!(fs::read_dir(data.join("Trash/info")).unwrap().count(), 0);
}

#[test]
fn restore_preserves_newer_work_and_can_be_undone_back_into_trash() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    fs::write(&source, b"trashed report").unwrap();
    let location = TrashLocation::home(root.path().join("data")).unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    engine
        .trash(
            &location,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    let item = location.list(&Cancellation::default()).unwrap().remove(0);
    fs::write(&source, b"newer report").unwrap();
    assert_eq!(
        engine
            .restore(&item, None, &Cancellation::default())
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&source).unwrap(), b"newer report");
    assert_eq!(fs::read(&item.path).unwrap(), b"trashed report");
    let alternative = root.path().join("recovered report.txt");
    engine
        .restore(&item, Some(&alternative), &Cancellation::default())
        .unwrap();
    assert_eq!(fs::read(&alternative).unwrap(), b"trashed report");
    assert!(location.list(&Cancellation::default()).unwrap().is_empty());
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!alternative.exists());
    let restored_item = location.list(&Cancellation::default()).unwrap().remove(0);
    assert_eq!(restored_item.original.as_deref(), Some(source.as_path()));
    assert_eq!(fs::read(restored_item.path).unwrap(), b"trashed report");
    assert_eq!(fs::read(&source).unwrap(), b"newer report");
    fs::remove_file(&source).unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(&source).unwrap(), b"trashed report");
}

#[test]
fn volume_trash_uses_private_fallback_and_relative_paths_for_long_raw_filenames() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let volume = root.path().join("volume");
    let elsewhere = root.path().join("elsewhere");
    fs::create_dir_all(volume.join("folder")).unwrap();
    fs::create_dir(&elsewhere).unwrap();
    symlink(&elsewhere, volume.join(".Trash")).unwrap();
    let mut bytes = vec![b'x'; 240];
    bytes.extend_from_slice(b"-\xff\n%.txt");
    let name = std::ffi::OsString::from_vec(bytes);
    let source = volume.join("folder").join(name);
    fs::write(&source, b"on this volume").unwrap();
    let location = TrashLocation::volume(&volume).unwrap();
    assert_eq!(
        location.directory(),
        volume.join(format!(
            ".Trash-{}",
            fs::metadata(root.path()).unwrap().uid()
        ))
    );
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    engine
        .trash(
            &location,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
    let information = fs::read_dir(location.directory().join("info"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = fs::read_to_string(information).unwrap();
    assert!(text.contains("\nPath=folder/xxx"), "{text}");
    assert!(text.contains("-%FF%0A%25.txt\n"), "{text}");
    let item = location.list(&Cancellation::default()).unwrap().remove(0);
    assert_eq!(item.original.as_deref(), Some(source.as_path()));
    engine
        .restore(&item, None, &Cancellation::default())
        .unwrap();
    assert_eq!(fs::read(source).unwrap(), b"on this volume");
}

#[test]
fn automatic_trash_keeps_the_original_symlink_parent_location_and_one_batch_undo() {
    let root = tempfile::tempdir().unwrap();
    let real = root.path().join("real");
    let alias = root.path().join("alias");
    fs::create_dir(&real).unwrap();
    symlink(&real, &alias).unwrap();
    let through_alias = alias.join("report.txt");
    fs::write(&through_alias, b"through the alias").unwrap();
    let other = root.path().join("other.txt");
    fs::write(&other, b"another file").unwrap();
    let data = root.path().join("data");
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let outcome = engine
        .trash_files(
            &data,
            &[through_alias.clone(), other.clone()],
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(outcome.completed, 2);
    assert!(outcome.failures.is_empty());
    let items = TrashLocation::home(data)
        .unwrap()
        .list(&Cancellation::default())
        .unwrap();
    assert!(
        items
            .iter()
            .any(|item| item.original.as_deref() == Some(through_alias.as_path()))
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(through_alias).unwrap(), b"through the alias");
    assert_eq!(fs::read(other).unwrap(), b"another file");
    assert!(engine.undo_label().is_none());
}

#[test]
fn a_failed_item_does_not_erase_other_work_and_cancelled_trash_is_a_no_op() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    fs::write(&source, b"keep recoverable").unwrap();
    let location = TrashLocation::home(root.path().join("data")).unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let result = engine
        .trash(
            &location,
            &[root.path().join("missing"), source.clone()],
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(result.completed, 1);
    assert_eq!(result.failures.len(), 1);
    engine.undo(&Cancellation::default()).unwrap();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        engine
            .trash(&location, std::slice::from_ref(&source), &cancellation)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    assert_eq!(fs::read(source).unwrap(), b"keep recoverable");
    assert!(location.list(&Cancellation::default()).unwrap().is_empty());
    assert_eq!(
        fs::read_dir(location.directory().join("info"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn trash_contents_remain_restorable_after_their_undo_entry_expires() {
    use canopa_files::operations::OperationRequest;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("keep in Trash.txt");
    fs::write(&source, b"independent of Undo lifetime").unwrap();
    let location = TrashLocation::home(root.path().join("data")).unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    engine
        .trash(
            &location,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    for index in 0..100 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("folder-{index}")),
                },
                &Cancellation::default(),
            )
            .unwrap();
    }
    for _ in 0..100 {
        engine.undo(&Cancellation::default()).unwrap();
    }
    assert!(engine.undo_label().is_none());
    let item = location.list(&Cancellation::default()).unwrap().remove(0);
    engine
        .restore(&item, None, &Cancellation::default())
        .unwrap();
    assert_eq!(fs::read(source).unwrap(), b"independent of Undo lifetime");
}

#[test]
fn changing_a_volume_root_does_not_redirect_a_selected_restore() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let volume = root.path().join("volume");
    fs::create_dir(&volume).unwrap();
    let source = volume.join("report.txt");
    fs::write(&source, b"selected on the original volume").unwrap();
    let location = TrashLocation::volume(&volume).unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    engine
        .trash(
            &location,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    let item = location.list(&Cancellation::default()).unwrap().remove(0);
    let relative_store = location.directory().strip_prefix(&volume).unwrap();
    let relative_item = item.path.strip_prefix(&volume).unwrap();
    let moved = root.path().join("moved-volume");
    fs::rename(&volume, &moved).unwrap();
    fs::create_dir_all(location.directory().join("files")).unwrap();
    fs::create_dir(location.directory().join("info")).unwrap();
    for directory in [
        location.directory().to_owned(),
        location.directory().join("files"),
        location.directory().join("info"),
    ] {
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::hard_link(moved.join(relative_item), &item.path).unwrap();
    let information = fs::read_dir(moved.join(relative_store).join("info"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    fs::copy(
        information.path(),
        location
            .directory()
            .join("info")
            .join(information.file_name()),
    )
    .unwrap();
    assert_eq!(
        engine
            .restore(&item, None, &Cancellation::default())
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound
    );
    assert!(!source.exists());
    assert_eq!(
        fs::read(item.path).unwrap(),
        b"selected on the original volume"
    );
}

#[test]
fn concurrent_restore_requests_record_only_the_move_that_actually_happened() {
    use canopa_files::transfers::{TransferEvent, TransferKind, TransferReply, TransferRequest};
    use std::sync::{Arc, mpsc};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    fs::write(&source, b"one actual restore").unwrap();
    let location = TrashLocation::home(root.path().join("data")).unwrap();
    let engine =
        Arc::new(OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap());
    engine
        .trash(
            &location,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    let item = location.list(&Cancellation::default()).unwrap().remove(0);
    let gate_source = root.path().join("gate");
    let gate_destination = root.path().join("destination");
    fs::create_dir(&gate_destination).unwrap();
    fs::write(&gate_source, b"incoming").unwrap();
    fs::write(gate_destination.join("gate"), b"existing").unwrap();
    let (locked, ready) = mpsc::channel();
    let (release, choice) = mpsc::channel();
    struct Release(Option<mpsc::Sender<TransferReply>>);
    impl Drop for Release {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(TransferReply::Cancel);
            }
        }
    }
    let release = Release(Some(release));
    let blocker_engine = engine.clone();
    let blocker = std::thread::spawn(move || {
        blocker_engine.transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![gate_source],
                destination: gate_destination,
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Conflict { .. }) {
                    let _ = locked.send(());
                    choice.recv().unwrap_or(TransferReply::Cancel)
                } else {
                    TransferReply::Continue
                }
            },
        )
    });
    ready
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let (started, waiting) = mpsc::channel();
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let engine = engine.clone();
            let item = item.clone();
            let started = started.clone();
            std::thread::spawn(move || {
                started.send(()).unwrap();
                engine.restore(&item, None, &Cancellation::default())
            })
        })
        .collect();
    for _ in 0..4 {
        waiting
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    }
    // The public conflict interaction keeps mutations queued while the requests
    // arrive. A selected item must be validated when its request can execute.
    std::thread::sleep(std::time::Duration::from_millis(100));
    drop(release);
    blocker.join().unwrap().unwrap();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!source.exists());
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(source).unwrap(), b"one actual restore");
    assert!(engine.undo_label().is_none());
}

#[test]
fn missing_or_traversing_trash_information_remains_visible_without_redirecting_restore() {
    let root = tempfile::tempdir().unwrap();
    let location = TrashLocation::home(root.path().join("data")).unwrap();
    fs::create_dir_all(location.directory().join("files")).unwrap();
    fs::create_dir(location.directory().join("info")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    for path in [
        location.directory().to_owned(),
        location.directory().join("files"),
        location.directory().join("info"),
    ] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::write(
        location.directory().join("files/orphan"),
        b"orphan contents",
    )
    .unwrap();
    fs::write(
        location.directory().join("files/hostile"),
        b"malformed information",
    )
    .unwrap();
    fs::write(
        location.directory().join("info/hostile.trashinfo"),
        b"[Trash Info]\nPath=../../elsewhere\nDeletionDate=2026-10-16T12:00:00\n",
    )
    .unwrap();
    let items = location.list(&Cancellation::default()).unwrap();
    assert_eq!(items.len(), 2);
    assert!(
        items
            .iter()
            .all(|item| item.original.is_none() && item.problem.is_some())
    );
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    for item in &items {
        assert!(
            engine
                .restore(item, None, &Cancellation::default())
                .is_err()
        );
    }
    assert!(!root.path().join("elsewhere").exists());
    assert_eq!(
        fs::read(location.directory().join("files/orphan")).unwrap(),
        b"orphan contents"
    );
    assert_eq!(
        fs::read(location.directory().join("files/hostile")).unwrap(),
        b"malformed information"
    );
}

#[test]
fn a_killed_trash_batch_retains_every_file_and_recovers_undo_for_its_completed_prefix() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let root = tempfile::tempdir().unwrap();
    let sources = root.path().join("source");
    fs::create_dir(&sources).unwrap();
    for index in 0..1000 {
        fs::write(sources.join(format!("{index:04}")), b"survives SIGKILL").unwrap();
    }
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Child(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "cases::trash::crash_trash_helper", "--ignored"])
            .env("CANOPA_FILES_TRASH_CRASH_ROOT", root.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while sources.join("0000").exists() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "child finished before the crash point"
        );
        assert!(Instant::now() < deadline, "child did not start trashing");
        std::thread::sleep(Duration::from_millis(1));
    }
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let location = TrashLocation::home(root.path().join("data")).unwrap();
    let items = location.list(&Cancellation::default()).unwrap();
    assert!(!items.is_empty());
    assert_eq!(fs::read_dir(&sources).unwrap().count() + items.len(), 1000);
    for item in &items {
        assert_eq!(fs::read(&item.path).unwrap(), b"survives SIGKILL");
    }
    engine.undo(&Cancellation::default()).unwrap();
    assert!(location.list(&Cancellation::default()).unwrap().is_empty());
    for index in 0..1000 {
        assert_eq!(
            fs::read(sources.join(format!("{index:04}"))).unwrap(),
            b"survives SIGKILL"
        );
    }
}

#[test]
#[ignore = "child process fixture; its parent kills it after an actual Trash rename"]
fn crash_trash_helper() {
    let Some(root) =
        std::env::var_os("CANOPA_FILES_TRASH_CRASH_ROOT").map(std::path::PathBuf::from)
    else {
        return;
    };
    let sources = fs::read_dir(root.join("source"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    let engine = OperationEngine::open(root.join("state/operations.sqlite3")).unwrap();
    engine
        .trash(
            &TrashLocation::home(root.join("data")).unwrap(),
            &sources,
            &Cancellation::default(),
        )
        .unwrap();
}

#[test]
fn trash_on_the_data_home_filesystem_records_no_other_trash_top() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    let source = root.path().join("note.txt");
    fs::write(&source, b"note").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let outcome = engine
        .trash_files(
            &data,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(outcome.completed, 1, "{outcome:?}");
    assert_eq!(fs::read_dir(data.join("Trash/files")).unwrap().count(), 1);
    // Only subvolume tops below a mount are recorded (ADR 0152).
    assert!(!data.join("canopa-files").exists());
}
