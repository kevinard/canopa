use canopa_files::{
    Cancellation,
    operations::{OperationEngine, UndoChoice},
    transfers::{TransferKind, TransferReply, TransferRequest},
};
use std::{fs, path::Path};

fn copy(engine: &OperationEngine, source: &Path, destination: &Path) {
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.to_owned()],
                destination: destination.to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(result.failures.is_empty());
}

#[test]
fn changed_copies_without_previous_destinations_offer_skip_and_cancel_across_restart() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::write(&source, b"original copy").unwrap();
    fs::create_dir(&destination).unwrap();
    let path = destination.join("report.txt");
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    copy(&engine, &source, &destination);
    fs::write(&path, b"newer work in the copy").unwrap();
    let cancel = Cancellation::default();
    let mut prompted = false;
    let outcome = engine
        .undo_interactive(None, &cancel, |conflict| {
            prompted = true;
            assert_eq!(conflict.incoming.entry().unwrap().path.clone(), path);
            UndoChoice::Skip
        })
        .unwrap();
    assert!(prompted);
    assert_eq!(outcome.completed, 0);
    assert_eq!(outcome.remaining, 1);
    assert_eq!(outcome.skipped, vec![path.clone()]);
    assert_eq!(fs::read(&path).unwrap(), b"newer work in the copy");
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let outcome = engine
        .undo_interactive(None, &cancel, |_| {
            cancel.cancel();
            UndoChoice::Cancel
        })
        .unwrap();
    assert!(outcome.cancelled);
    assert_eq!(outcome.remaining, 1);
    assert!(
        engine.undo(&Cancellation::default()).is_err(),
        "noninteractive Undo keeps refusing changed copies"
    );
    assert_eq!(fs::read(&path).unwrap(), b"newer work in the copy");
}

#[test]
fn keeping_an_edited_copy_finishes_its_history_without_removing_newer_work_or_its_parent() {
    use canopa_files::operations::OperationRequest;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    for name in ["a.txt", "b.txt"] {
        fs::write(source.join(name), b"copied data").unwrap();
    }
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let destination = root.path().join("destination");
    let cancel = Cancellation::default();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: destination.clone(),
            },
            &cancel,
        )
        .unwrap();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.join("a.txt"), source.join("b.txt")],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    fs::write(destination.join("b.txt"), b"edited after copying").unwrap();
    let result = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepItem)
        .unwrap();
    assert_eq!(result.completed, 1);
    assert_eq!(result.kept, vec![destination.join("b.txt")]);
    assert_eq!(result.remaining, 0);
    assert!(result.skipped.is_empty());
    assert!(!destination.join("a.txt").exists());
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert_ne!(engine.latest_undo_id(), copied.id);
    assert!(
        engine.undo(&cancel).is_err(),
        "older folder Undo cannot erase the kept edit"
    );
    assert_eq!(
        fs::read(destination.join("b.txt")).unwrap(),
        b"edited after copying"
    );
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn removing_an_edited_copy_preserves_later_open_handle_writes_through_expiry_and_restart() {
    use canopa_files::operations::OperationRequest;
    use std::io::{Seek, Write};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"copy source").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    copy(&engine, &source, &destination);
    let path = destination.join("report.txt");
    fs::write(&path, b"newer edits").unwrap();
    let mut editor = fs::OpenOptions::new().write(true).open(&path).unwrap();
    let cancel = Cancellation::default();
    let result = engine
        .undo_interactive(None, &cancel, |conflict| {
            assert_eq!(
                conflict.source,
                canopa_files::operations::UndoSource::ChangedCopy
            );
            assert!(conflict.can_move_to_recovery());
            UndoChoice::MoveToRecovery
        })
        .unwrap();
    assert_eq!(result.completed, 1);
    assert_eq!(result.remaining, 0);
    assert!(result.kept.is_empty());
    assert!(!path.exists());
    editor.rewind().unwrap();
    editor
        .write_all(b"later edits through the open handle")
        .unwrap();
    editor.flush().unwrap();
    drop(editor);
    for i in 0..101 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("folder-{i}")),
                },
                &cancel,
            )
            .unwrap();
    }
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].held_for_undo);
    assert_eq!(page.items[0].original.as_deref(), Some(path.as_path()));
    let rescued = root.path().join("rescued.txt");
    engine
        .recover_copy(&page.items[0], &rescued, &cancel)
        .unwrap();
    assert_eq!(
        fs::read(&rescued).unwrap(),
        b"later edits through the open handle"
    );
    assert_eq!(fs::read(&source).unwrap(), b"copy source");
}

#[test]
fn a_change_during_the_removal_question_keeps_the_newest_file_and_leaves_undo_retryable() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"copy source").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    copy(&engine, &source, &destination);
    let path = destination.join("report.txt");
    fs::write(&path, b"edited before question").unwrap();
    let cancel = Cancellation::default();
    assert!(
        engine
            .undo_interactive(None, &cancel, |_| {
                fs::write(&path, b"newest change while deciding").unwrap();
                UndoChoice::MoveToRecovery
            })
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), b"newest change while deciding");
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
    let outcome = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepItem)
        .unwrap();
    assert_eq!(outcome.remaining, 0);
    assert_eq!(outcome.kept, vec![path]);
}

#[test]
fn a_folder_replacing_a_copied_file_can_be_kept_but_is_not_removed_to_recovery() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("item");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"copy source").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    copy(&engine, &source, &destination);
    let path = destination.join("item");
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("newer"), b"new folder contents").unwrap();
    let cancel = Cancellation::default();
    assert!(
        engine
            .undo_interactive(None, &cancel, |conflict| {
                assert!(!conflict.can_replace());
                UndoChoice::MoveToRecovery
            })
            .is_err()
    );
    let result = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepItem)
        .unwrap();
    assert_eq!(result.kept, vec![path.clone()]);
    assert_eq!(result.remaining, 0);
    assert_eq!(
        fs::read(path.join("newer")).unwrap(),
        b"new folder contents"
    );
}

#[test]
fn undoing_a_folder_copy_keeps_containers_of_kept_edits_and_skips_containers_of_skipped_edits() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("changed.txt"), b"copy source").unwrap();
    fs::write(source.join("unchanged.txt"), b"other source").unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    copy(&engine, &source, &destination);
    let copied = destination.join("source");
    fs::write(copied.join("changed.txt"), b"newer child edits").unwrap();
    let partial = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Skip)
        .unwrap();
    assert_eq!(partial.remaining, 2);
    assert_eq!(partial.completed, 1);
    assert!(!copied.join("unchanged.txt").exists());
    let finished = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepItem)
        .unwrap();
    assert_eq!(finished.remaining, 0);
    assert!(finished.kept.contains(&copied.join("changed.txt")));
    assert!(finished.kept.contains(&copied));
    assert_eq!(
        fs::read(copied.join("changed.txt")).unwrap(),
        b"newer child edits"
    );
    assert!(engine.undo_label().is_none());
}

#[test]
fn keeping_a_copied_child_preserves_its_container_across_cancellation_and_restart() {
    use std::collections::BTreeMap;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    for name in ["a.txt", "b.txt"] {
        fs::write(source.join(name), b"copy source").unwrap();
    }
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    copy(&engine, &source, &destination);
    let copied = destination.join("source");
    let modified: BTreeMap<_, _> = ["a.txt", "b.txt"]
        .into_iter()
        .map(|name| {
            let path = copied.join(name);
            let time = fs::metadata(&path).unwrap().modified().unwrap();
            fs::write(&path, b"newer edits").unwrap();
            (path, time)
        })
        .collect();
    let cancel = Cancellation::default();
    let mut kept = None;
    let mut cancelled = None;
    let outcome = engine
        .undo_interactive(None, &cancel, |conflict| {
            if kept.is_none() {
                kept = Some(conflict.incoming.entry().unwrap().path.clone());
                UndoChoice::KeepItem
            } else {
                cancelled = Some(conflict.incoming.entry().unwrap().path.clone());
                cancel.cancel();
                UndoChoice::Cancel
            }
        })
        .unwrap();
    assert!(outcome.cancelled);
    assert!(
        outcome.kept.contains(&copied),
        "container preservation commits together with keeping its child"
    );
    let cancelled = cancelled.unwrap();
    fs::write(&cancelled, b"copy source").unwrap();
    fs::File::open(&cancelled)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified[&cancelled]))
        .unwrap();
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!cancelled.exists());
    assert_eq!(fs::read(kept.unwrap()).unwrap(), b"newer edits");
    assert!(copied.is_dir());
    assert!(engine.undo_label().is_none());
}

#[test]
fn removing_a_link_that_replaced_a_copy_preserves_raw_names_and_the_link_target() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let name = std::ffi::OsString::from_vec(b"copy-\xff".to_vec());
    let source = root.path().join(&name);
    let destination = root.path().join("destination");
    fs::write(&source, b"original copied data").unwrap();
    fs::create_dir(&destination).unwrap();
    let target = root
        .path()
        .join(std::ffi::OsString::from_vec(b"target-\xff".to_vec()));
    fs::create_dir(&target).unwrap();
    fs::write(target.join("sentinel"), b"directory target stays intact").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    copy(&engine, &source, &destination);
    let path = destination.join(&name);
    fs::remove_file(&path).unwrap();
    let raw_target = std::ffi::OsString::from_vec(b"../target-\xff".to_vec());
    std::os::unix::fs::symlink(&raw_target, &path).unwrap();
    let cancel = Cancellation::default();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    assert!(fs::symlink_metadata(&path).is_err());
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].original.as_deref(), Some(path.as_path()));
    let recovered = destination.join("recovered-link");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read_link(&recovered).unwrap().as_os_str(), raw_target);
    assert_eq!(
        fs::read(target.join("sentinel")).unwrap(),
        b"directory target stays intact"
    );
}

#[test]
fn restarting_a_killed_changed_copy_removal_preserves_all_newer_files_in_recovery() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let root = tempfile::Builder::new()
        .prefix("canopa-changed-copy-crash-")
        .tempdir()
        .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::undo_changed_copy::changed_copy_removal_child",
        ])
        .env("CANOPA_CHANGED_COPY_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(!root.path().join("undo-returned").exists());
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    if engine.undo_label().is_some() {
        engine
            .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
            .unwrap();
    }
    assert!(engine.undo_label().is_none());
    let mut originals = std::collections::BTreeSet::new();
    let mut cursor = None;
    loop {
        let page = engine.recovery_items(cursor, &cancel).unwrap();
        for item in &page.items {
            assert_eq!(
                fs::read(&item.entry.as_ref().unwrap().path).unwrap(),
                b"newer copy edits"
            );
            originals.insert(item.original.clone().unwrap());
        }
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(originals.len(), 500);
    for index in 0..500 {
        assert!(
            !root
                .path()
                .join("destination")
                .join(format!("file-{index:04}"))
                .exists()
        );
        assert_eq!(
            fs::read(root.path().join("source").join(format!("file-{index:04}"))).unwrap(),
            b"original copied data"
        );
    }
}

#[test]
#[ignore = "child process fixture; killed after an actual changed-copy removal"]
fn changed_copy_removal_child() {
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_CHANGED_COPY_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-changed-copy-crash-")
    );
    let source = root.join("source");
    let destination = root.join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    let sources = (0..500)
        .map(|index| {
            let path = source.join(format!("file-{index:04}"));
            fs::write(&path, b"original copied data").unwrap();
            path
        })
        .collect();
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let cancel = Cancellation::default();
    assert!(
        engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources,
                    destination: destination.clone()
                },
                &cancel,
                |_| TransferReply::Continue
            )
            .unwrap()
            .failures
            .is_empty()
    );
    for index in 0..500 {
        fs::write(
            destination.join(format!("file-{index:04}")),
            b"newer copy edits",
        )
        .unwrap();
    }
    let watch = destination.join("file-0499");
    let ready = std::sync::Arc::new(std::sync::Barrier::new(2));
    let watcher_ready = ready.clone();
    std::thread::spawn(move || {
        watcher_ready.wait();
        while watch.exists() {
            std::thread::sleep(std::time::Duration::from_micros(50));
        }
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    ready.wait();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    fs::write(root.join("undo-returned"), b"missed crash").unwrap();
    panic!("parent must observe an actual interrupted removal");
}
