use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, UndoChoice, UndoSource},
};
use std::{fs, path::PathBuf};

#[test]
fn keeping_new_arrivals_with_reused_names_preserves_older_history_on_returned_children() {
    use canopa_files::transfers::{TransferKind, TransferReply, TransferRequest};
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    let sources = root.path().join("sources");
    fs::create_dir(&sources).unwrap();
    let source = sources.join("a.txt");
    fs::write(&source, b"copied incoming").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&history).unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: original.clone(),
            },
            &cancel,
        )
        .unwrap();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: original.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    fs::write(original.join("z.txt"), b"last incoming").unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("a.txt"), b"newer a").unwrap();
    fs::write(original.join("z.txt"), b"newer z").unwrap();
    engine
        .undo_interactive(None, &cancel, |question| {
            if question.source == UndoSource::MergeRemainder {
                UndoChoice::KeepItem
            } else if question.can_merge() {
                UndoChoice::Merge
            } else {
                if question.destination == original.join("z.txt") {
                    assert_eq!(
                        fs::read(original.join("a (2).txt")).unwrap(),
                        b"copied incoming"
                    );
                    fs::write(current.join("a.txt"), b"new source file using the old name")
                        .unwrap();
                }
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    engine.undo(&cancel).unwrap();
    assert!(!original.join("a (2).txt").exists());
    assert_eq!(fs::read(original.join("a.txt")).unwrap(), b"newer a");
    assert_eq!(
        fs::read(current.join("a.txt")).unwrap(),
        b"new source file using the old name"
    );
    assert_eq!(fs::read(&source).unwrap(), b"copied incoming");
    engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedCreatedFolder);
            UndoChoice::KeepItem
        })
        .unwrap();
    assert!(engine.latest_undo_id().is_none());
    assert_eq!(
        fs::read(original.join("z (2).txt")).unwrap(),
        b"last incoming"
    );
    assert_eq!(fs::read(original.join("z.txt")).unwrap(), b"newer z");
}

#[test]
fn merging_from_trash_removes_only_finished_metadata_and_keeps_new_remainder_listed() {
    use canopa_files::trash::TrashLocation;
    for keep_remainder in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("project");
        fs::create_dir(&original).unwrap();
        fs::write(original.join("item.txt"), b"incoming").unwrap();
        let location = TrashLocation::home(root.path().join("data")).unwrap();
        let cancel = Cancellation::default();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let trashed = engine
            .trash(&location, std::slice::from_ref(&original), &cancel)
            .unwrap();
        assert!(trashed.failures.is_empty());
        let current = location.list(&cancel).unwrap()[0].path.clone();
        fs::create_dir(&original).unwrap();
        fs::write(original.join("item.txt"), b"newer").unwrap();
        let result = engine
            .undo_interactive(None, &cancel, |question| match question.source {
                UndoSource::MergeRemainder => UndoChoice::KeepItem,
                _ if question.can_merge() => UndoChoice::Merge,
                _ => {
                    if keep_remainder {
                        fs::write(current.join("new.txt"), b"new Trash work").unwrap();
                    }
                    UndoChoice::KeepBoth
                }
            })
            .unwrap();
        assert_eq!(result.remaining, 0);
        let items = location.list(&cancel).unwrap();
        if keep_remainder {
            assert_eq!(result.kept, vec![current.clone()]);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].original.as_deref(), Some(original.as_path()));
            assert_eq!(items[0].path, current);
            assert_eq!(
                fs::read(items[0].path.join("new.txt")).unwrap(),
                b"new Trash work"
            );
        } else {
            assert!(items.is_empty());
            assert!(!current.exists());
        }
        assert_eq!(fs::read(original.join("item.txt")).unwrap(), b"newer");
        assert_eq!(
            fs::read(original.join("item (2).txt")).unwrap(),
            b"incoming"
        );
        assert!(engine.latest_undo_id().is_none());
    }
}

#[test]
fn keeping_remaining_contents_protects_the_unrestored_previous_destination_through_expiry() {
    use canopa_files::transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest};
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("item");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let current = destination.join("item");
    fs::write(&original, b"original incoming file").unwrap();
    fs::write(&current, b"previous destination").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![original.clone()],
                destination,
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    let elsewhere = root.path().join("original elsewhere");
    fs::rename(&current, &elsewhere).unwrap();
    fs::create_dir(&current).unwrap();
    fs::write(current.join("work.txt"), b"selected directory contents").unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("work.txt"), b"newer destination work").unwrap();
    let result = engine
        .undo_interactive(None, &cancel, |question| match question.source {
            UndoSource::ChangedMove => UndoChoice::ReturnItem,
            UndoSource::MergeRemainder => UndoChoice::KeepItem,
            _ if question.can_merge() => UndoChoice::Merge,
            _ => {
                fs::write(current.join("new.txt"), b"new source contents").unwrap();
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(result.completed, 1);
    assert_eq!(result.remaining, 0);
    assert_eq!(result.kept, vec![current.clone()]);
    assert_eq!(result.unrestored_previous, vec![current.clone()]);
    for index in 0..101 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("later-{index}")),
                },
                &cancel,
            )
            .unwrap();
    }
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].held_for_undo);
    let recovered = root.path().join("recovered.txt");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"previous destination");
    assert_eq!(
        fs::read(current.join("new.txt")).unwrap(),
        b"new source contents"
    );
    assert_eq!(
        fs::read(original.join("work (2).txt")).unwrap(),
        b"selected directory contents"
    );
    assert_eq!(
        fs::read(original.join("work.txt")).unwrap(),
        b"newer destination work"
    );
    assert_eq!(fs::read(elsewhere).unwrap(), b"original incoming file");
}

#[test]
fn remaining_items_review_is_retryable_and_refuses_stale_or_cancelled_keep() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("item.txt"), b"incoming").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&history).unwrap();
    let operation = engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("item.txt"), b"newer").unwrap();
    let partial = engine
        .undo_interactive(None, &cancel, |question| match question.source {
            UndoSource::MergeRemainder => {
                assert!(question.can_merge());
                assert!(!question.can_replace());
                UndoChoice::Skip
            }
            _ if question.can_merge() => UndoChoice::Merge,
            _ => {
                fs::write(current.join("new.txt"), b"new work").unwrap();
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(partial.remaining, 1);
    assert_eq!(
        engine.undo(&cancel).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(engine.latest_undo_id(), Some(operation.id));
    let cancelled = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::MergeRemainder);
            UndoChoice::Cancel
        })
        .unwrap();
    assert!(cancelled.cancelled);
    for (folder, name, choice) in [
        (&current, "later-source.txt", UndoChoice::KeepItem),
        (&original, "later-destination.txt", UndoChoice::KeepItem),
        (&current, "later-merge-source.txt", UndoChoice::Merge),
        (&original, "later-merge-destination.txt", UndoChoice::Merge),
    ] {
        let error = engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.source, UndoSource::MergeRemainder);
                fs::write(folder.join(name), b"later work").unwrap();
                choice
            })
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert_eq!(engine.latest_undo_id(), Some(operation.id));
    }
    let cancelled = engine
        .undo_interactive(None, &cancel, |_| {
            cancel.cancel();
            UndoChoice::KeepItem
        })
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.remaining, 1);
    let merge_cancel = Cancellation::default();
    let cancelled = engine
        .undo_interactive(None, &merge_cancel, |_| {
            merge_cancel.cancel();
            UndoChoice::Merge
        })
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.remaining, 1);
    let finished = engine
        .undo_interactive(None, &Cancellation::default(), |_| UndoChoice::KeepItem)
        .unwrap();
    assert_eq!(finished.remaining, 0);
    assert_eq!(finished.completed, 0);
    assert_eq!(finished.kept, vec![current.clone()]);
    assert_eq!(fs::read(current.join("new.txt")).unwrap(), b"new work");
    assert_eq!(
        fs::read(current.join("later-source.txt")).unwrap(),
        b"later work"
    );
    assert_eq!(
        fs::read(original.join("later-destination.txt")).unwrap(),
        b"later work"
    );
    assert_eq!(fs::read(original.join("item.txt")).unwrap(), b"newer");
    assert_eq!(
        fs::read(original.join("item (2).txt")).unwrap(),
        b"incoming"
    );
}

#[test]
fn new_arrivals_can_be_kept_in_the_incoming_folder_after_recorded_children_return() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("a.txt"), b"first incoming").unwrap();
    fs::write(original.join("b.txt"), b"second incoming").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&history).unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("b.txt"), b"newer second").unwrap();
    let mut questions = 0;
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            questions += 1;
            match questions {
                1 => UndoChoice::Merge,
                2 => {
                    fs::write(current.join("new.txt"), b"new source work").unwrap();
                    UndoChoice::KeepBoth
                }
                3 => {
                    assert_eq!(question.incoming.path(), current);
                    UndoChoice::KeepItem
                }
                _ => panic!("unexpected question"),
            }
        })
        .expect("new arrivals need an explicit Keep/Skip/Cancel review");
    assert_eq!(questions, 3);
    assert_eq!(result.completed, 2);
    assert_eq!(result.kept, vec![current.clone()]);
    assert_eq!(result.remaining, 0);
    assert_eq!(
        fs::read(current.join("new.txt")).unwrap(),
        b"new source work"
    );
    assert_eq!(fs::read(original.join("a.txt")).unwrap(), b"first incoming");
    assert_eq!(fs::read(original.join("b.txt")).unwrap(), b"newer second");
    assert_eq!(
        fs::read(original.join("b (2).txt")).unwrap(),
        b"second incoming"
    );
    assert!(engine.latest_undo_id().is_none());
}

#[test]
fn ordinary_history_expiry_preserves_an_unfinished_directory_merge() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("a.txt"), b"first incoming").unwrap();
    fs::write(original.join("b.txt"), b"second incoming").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
    let operation = engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("b.txt"), b"newer second").unwrap();
    let partial = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert_eq!(partial.completed, 1);
    for index in 0..101 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("later-{index}")),
                },
                &cancel,
            )
            .unwrap();
    }
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    for _ in 0..100 {
        engine.undo(&cancel).unwrap();
    }
    assert_eq!(engine.latest_undo_id(), Some(operation.id));
    let finished = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.destination, original.join("b.txt"));
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(finished.remaining, 0);
    assert!(!current.exists());
    assert_eq!(fs::read(original.join("a.txt")).unwrap(), b"first incoming");
    assert_eq!(fs::read(original.join("b.txt")).unwrap(), b"newer second");
    assert_eq!(
        fs::read(original.join("b (2).txt")).unwrap(),
        b"second incoming"
    );
    assert!(engine.latest_undo_id().is_none());
}

#[test]
fn a_killed_directory_merge_resumes_without_returning_completed_children_twice() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let root = tempfile::Builder::new()
        .prefix("canopa-directory-resume-")
        .tempdir()
        .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "crash_directory_merge_helper"])
        .env("CANOPA_DIRECTORY_RESUME_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let original = root.path().join("project");
    let current = root.path().join("moved");
    assert_eq!(fs::read(original.join("file-0000")).unwrap(), b"incoming 0");
    assert!(current.is_dir());
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    let mut questions = 0;
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            questions += 1;
            assert_eq!(question.destination, original.join("file-0099"));
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(questions, 1);
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    for index in 0..99 {
        assert_eq!(
            fs::read(original.join(format!("file-{index:04}"))).unwrap(),
            format!("incoming {index}").as_bytes()
        );
    }
    assert_eq!(
        fs::read(original.join("file-0099")).unwrap(),
        b"newer destination"
    );
    assert_eq!(
        fs::read(original.join("file-0099 (2)")).unwrap(),
        b"incoming 99"
    );
    assert!(engine.latest_undo_id().is_none());
}

#[test]
#[ignore = "child process fixture; killed after a real directory child return"]
fn crash_directory_merge_helper() {
    use std::{
        os::unix::fs::MetadataExt,
        sync::{Arc, Barrier},
        time::Duration,
    };
    let root = PathBuf::from(
        std::env::var_os("CANOPA_DIRECTORY_RESUME_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-directory-resume-")
    );
    let original = root.join("project");
    let current = root.join("moved");
    fs::create_dir(&original).unwrap();
    for index in 0..100 {
        fs::write(
            original.join(format!("file-{index:04}")),
            format!("incoming {index}"),
        )
        .unwrap();
    }
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let cancel = Cancellation::default();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current,
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("file-0099"), b"newer destination").unwrap();
    let metadata = fs::metadata(root.join("moved/file-0000")).unwrap();
    let (device, inode) = (metadata.dev(), metadata.ino());
    let returned = original.join("file-0000");
    let ready = Arc::new(Barrier::new(2));
    let watcher_ready = ready.clone();
    std::thread::spawn(move || {
        watcher_ready.wait();
        while !fs::metadata(&returned)
            .is_ok_and(|metadata| metadata.dev() == device && metadata.ino() == inode)
        {
            std::thread::sleep(Duration::from_micros(50));
        }
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    ready.wait();
    engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                // Keep the last collision pending even if the watcher is delayed.
                loop {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        })
        .unwrap();
    panic!("the child was not killed during the return");
}
