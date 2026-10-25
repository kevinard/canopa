use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, UndoChoice},
};
use std::fs;

#[test]
fn an_external_move_does_not_let_older_copy_undo_own_an_independent_restored_destination() {
    use canopa_files::{
        operations::UndoSource,
        transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
    };
    for return_incoming in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let older = root.path().join("older");
        let origin = root.path().join("origin");
        let destination = root.path().join("destination");
        for path in [&older, &origin, &destination] {
            fs::create_dir(path).unwrap();
        }
        let current = destination.join("report.txt");
        let source = origin.join("report.txt");
        let external = root.path().join("incoming elsewhere.txt");
        fs::write(older.join("report.txt"), b"identical document bytes").unwrap();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let copied = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![older.join("report.txt")],
                    destination: destination.clone(),
                },
                &cancel,
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(copied.failures.is_empty());
        let metadata = fs::metadata(&current).unwrap();
        fs::rename(&current, &source).unwrap();
        fs::write(&current, b"identical document bytes").unwrap();
        fs::set_permissions(&current, metadata.permissions()).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&current)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        let moved = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![source.clone()],
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
        if return_incoming {
            engine.undo(&cancel).unwrap();
            fs::rename(&source, &external).unwrap();
        } else {
            fs::rename(&current, &external).unwrap();
            engine
                .undo_interactive(None, &cancel, |_| UndoChoice::RestorePreviousDestination)
                .unwrap();
        }
        let mut asked = false;
        let outcome = engine
            .undo_interactive(None, &cancel, |question| {
                asked = true;
                assert_eq!(question.source, UndoSource::ChangedCopy);
                UndoChoice::Skip
            })
            .unwrap();
        assert!(
            asked,
            "an external move does not authorize adopting its independent previous destination"
        );
        assert_eq!(outcome.completed, 0);
        assert_eq!(outcome.remaining, 1);
        assert_eq!(fs::read(&current).unwrap(), b"identical document bytes");
        assert_eq!(fs::read(&external).unwrap(), b"identical document bytes");
        assert!(!source.exists());
    }
}

#[test]
fn restoring_a_previous_destination_does_not_adopt_it_for_an_older_copy_of_the_missing_incoming_item()
 {
    use canopa_files::{
        operations::UndoSource,
        transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
    };
    for (returned_incoming, keep_replacement, restore_earlier_previous) in [
        (false, false, false),
        (true, false, false),
        (true, true, false),
        (false, false, true),
        (true, false, true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let older = root.path().join("older");
        let origin = root.path().join("origin");
        let destination = root.path().join("destination");
        for path in [&older, &origin, &destination] {
            fs::create_dir(path).unwrap();
        }
        let name = "report.txt";
        let current = destination.join(name);
        let source = origin.join(name);
        let external = root.path().join("incoming elsewhere.txt");
        fs::write(older.join(name), b"identical document bytes").unwrap();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let copied = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![older.join(name)],
                    destination: destination.clone(),
                },
                &cancel,
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(copied.failures.is_empty());
        let metadata = fs::metadata(&current).unwrap();
        if restore_earlier_previous {
            fs::write(&source, b"earlier saved destination").unwrap();
            let moved = engine
                .transfer(
                    &TransferRequest {
                        kind: TransferKind::Move,
                        sources: vec![current.clone()],
                        destination: origin,
                    },
                    &cancel,
                    |_| TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        apply_to_remaining: false,
                    },
                )
                .unwrap();
            assert!(moved.failures.is_empty());
        } else {
            engine
                .execute(
                    OperationRequest::Rename {
                        source: current.clone(),
                        destination: source.clone(),
                    },
                    &cancel,
                )
                .unwrap();
        }
        fs::write(&current, b"identical document bytes").unwrap();
        fs::set_permissions(&current, metadata.permissions()).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&current)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        let moved = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![source.clone()],
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
        if returned_incoming {
            engine.undo(&cancel).unwrap();
            fs::rename(&source, &external).unwrap();
        } else {
            fs::rename(&current, &external).unwrap();
            engine
                .undo_interactive(None, &cancel, |_| UndoChoice::RestorePreviousDestination)
                .unwrap();
        }
        if keep_replacement {
            fs::write(&source, b"replacement kept here").unwrap();
        }
        engine
            .undo_interactive(None, &cancel, |_| {
                if keep_replacement {
                    UndoChoice::KeepItem
                } else if restore_earlier_previous {
                    UndoChoice::RestorePreviousDestination
                } else {
                    UndoChoice::FinishStep
                }
            })
            .unwrap();
        let mut asked = false;
        let outcome = engine
            .undo_interactive(None, &cancel, |question| {
                asked = true;
                assert_eq!(question.source, UndoSource::ChangedCopy);
                UndoChoice::Skip
            })
            .unwrap();
        assert!(
            asked,
            "the restored independent destination still needs its own decision"
        );
        assert_eq!(outcome.completed, 0);
        assert_eq!(outcome.remaining, 1);
        assert_eq!(fs::read(&current).unwrap(), b"identical document bytes");
        assert_eq!(fs::read(&external).unwrap(), b"identical document bytes");
        if keep_replacement {
            assert_eq!(fs::read(&source).unwrap(), b"replacement kept here");
        } else if restore_earlier_previous {
            assert_eq!(fs::read(&source).unwrap(), b"earlier saved destination");
        } else {
            assert!(!source.exists());
        }
    }
}

#[test]
fn restarting_a_killed_saved_only_restoration_preserves_missing_items_and_older_copy_undo() {
    use std::os::unix::process::ExitStatusExt;
    let root = tempfile::Builder::new()
        .prefix("canopa-unavailable-undo-crash-")
        .tempdir()
        .unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "crash_unavailable_restore_helper",
            "--ignored",
            "--nocapture",
        ])
        .env("CANOPA_FILES_UNAVAILABLE_CRASH_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(!root.path().join("undo-returned").exists());
    let moved_id: u64 = fs::read_to_string(root.path().join("moved-id"))
        .unwrap()
        .parse()
        .unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    if engine.latest_undo_id() == Some(moved_id) {
        let outcome = engine
            .undo_interactive(Some(moved_id), &cancel, |_| {
                UndoChoice::RestorePreviousDestination
            })
            .unwrap();
        assert_eq!(outcome.remaining, 0);
    }
    for index in 0..500 {
        let name = format!("file-{index:04}");
        assert!(!root.path().join("source").join(&name).exists());
        assert_eq!(
            fs::read(root.path().join("destination").join(&name)).unwrap(),
            b"previous copied destination"
        );
        assert_eq!(
            fs::read(root.path().join("external").join(&name)).unwrap(),
            b"incoming moved elsewhere"
        );
    }
    assert_eq!(
        fs::read_dir(root.path().join("destination"))
            .unwrap()
            .count(),
        500
    );
    engine.undo(&cancel).unwrap();
    assert_eq!(
        fs::read_dir(root.path().join("destination"))
            .unwrap()
            .count(),
        0
    );
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
    for index in 0..500 {
        assert_eq!(
            fs::read(root.path().join(format!("older/file-{index:04}"))).unwrap(),
            b"previous copied destination"
        );
        assert_eq!(
            fs::read(root.path().join(format!("external/file-{index:04}"))).unwrap(),
            b"incoming moved elsewhere"
        );
    }
}

#[test]
#[ignore = "child process fixture; killed after restoring a saved destination without returning its missing incoming item"]
fn crash_unavailable_restore_helper() {
    use canopa_files::transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest};
    use std::os::unix::fs::MetadataExt;
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_FILES_UNAVAILABLE_CRASH_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-unavailable-undo-crash-")
    );
    let older = root.join("older");
    let source = root.join("source");
    let destination = root.join("destination");
    let external = root.join("external");
    for path in [&older, &source, &destination, &external] {
        fs::create_dir(path).unwrap();
    }
    for index in 0..500 {
        fs::write(
            older.join(format!("file-{index:04}")),
            b"previous copied destination",
        )
        .unwrap();
        fs::write(
            source.join(format!("file-{index:04}")),
            b"incoming moved elsewhere",
        )
        .unwrap();
    }
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let cancel = Cancellation::default();
    for (kind, input) in [(TransferKind::Copy, &older), (TransferKind::Move, &source)] {
        let outcome = engine
            .transfer(
                &TransferRequest {
                    kind,
                    sources: (0..500)
                        .map(|index| input.join(format!("file-{index:04}")))
                        .collect(),
                    destination: destination.clone(),
                },
                &cancel,
                |_| TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: true,
                },
            )
            .unwrap();
        assert!(outcome.failures.is_empty());
        if kind == TransferKind::Move {
            fs::write(root.join("moved-id"), outcome.id.unwrap().to_string()).unwrap();
        }
    }
    let mut next = None;
    let mut selected = None;
    loop {
        let page = engine.recovery_items(next, &cancel).unwrap();
        for item in page.items {
            if item.original.as_deref() == Some(destination.join("file-0499").as_path()) {
                let metadata = fs::symlink_metadata(item.entry.unwrap().path).unwrap();
                selected = Some((metadata.dev(), metadata.ino()));
            }
        }
        next = page.next;
        if next.is_none() {
            break;
        }
    }
    let (device, inode) = selected.unwrap();
    for index in 0..500 {
        let name = format!("file-{index:04}");
        fs::rename(destination.join(&name), external.join(&name)).unwrap();
    }
    let returned = destination.join("file-0499");
    let ready = std::sync::Arc::new(std::sync::Barrier::new(2));
    let watcher_ready = ready.clone();
    std::thread::spawn(move || {
        watcher_ready.wait();
        while !fs::symlink_metadata(&returned)
            .is_ok_and(|metadata| metadata.dev() == device && metadata.ino() == inode)
        {
            std::thread::sleep(std::time::Duration::from_micros(50));
        }
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    ready.wait();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::RestorePreviousDestination)
        .unwrap();
    fs::write(root.join("undo-returned"), b"missed crash window").unwrap();
    panic!("the child was not killed during saved-only restoration");
}

#[test]
fn skipping_an_unavailable_move_continues_the_other_items_and_preserves_its_own_history() {
    use canopa_files::transfers::{TransferEvent, TransferKind, TransferReply, TransferRequest};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("a.txt"), b"available item").unwrap();
    fs::write(source.join("b.txt"), b"unavailable item").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.join("a.txt"), source.join("b.txt")],
                destination: destination.clone(),
            },
            &cancel,
            |event| {
                assert!(!matches!(event, TransferEvent::Conflict { .. }));
                TransferReply::Continue
            },
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    let elsewhere = root.path().join("b elsewhere.txt");
    fs::rename(destination.join("b.txt"), &elsewhere).unwrap();
    let partly = engine
        .undo_interactive(moved.id, &cancel, |_| UndoChoice::Skip)
        .unwrap();
    assert_eq!(partly.completed, 1);
    assert_eq!(partly.skipped, [destination.join("b.txt")]);
    assert_eq!(partly.remaining, 1);
    assert!(partly.dismissed.is_empty());
    assert_eq!(fs::read(source.join("a.txt")).unwrap(), b"available item");
    assert!(!destination.join("a.txt").exists());
    assert_eq!(engine.latest_undo_id(), moved.id);
    let finished = engine
        .undo_interactive(moved.id, &cancel, |_| UndoChoice::FinishStep)
        .unwrap();
    assert_eq!(finished.completed, 0);
    assert_eq!(finished.remaining, 0);
    assert_eq!(fs::read(&elsewhere).unwrap(), b"unavailable item");
    assert!(!source.join("b.txt").exists());
    assert!(engine.latest_undo_id().is_none());
}

#[test]
fn changing_the_shown_saved_destination_refuses_restore_and_keeps_it_retryable_after_restart() {
    use canopa_files::transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    let elsewhere = root.path().join("incoming elsewhere.txt");
    let history = root.path().join("state/history");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"incoming contents").unwrap();
    let current = destination.join("report.txt");
    fs::write(&current, b"saved previous destination").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&history).unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
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
    let saved = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0)
        .entry
        .unwrap()
        .path;
    fs::rename(&current, &elsewhere).unwrap();
    let error = engine
        .undo_interactive(None, &cancel, |_| {
            fs::write(&saved, b"new saved edits while reviewing").unwrap();
            UndoChoice::RestorePreviousDestination
        })
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(engine.latest_undo_id(), moved.id);
    assert!(!source.exists() && !current.exists());
    assert_eq!(fs::read(&elsewhere).unwrap(), b"incoming contents");
    assert_eq!(
        fs::read(&saved).unwrap(),
        b"new saved edits while reviewing"
    );
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::RestorePreviousDestination)
        .unwrap();
    assert_eq!(
        fs::read(&current).unwrap(),
        b"new saved edits while reviewing"
    );
    assert_eq!(fs::read(&elsewhere).unwrap(), b"incoming contents");
    assert!(!source.exists());
}

#[test]
fn finishing_an_unavailable_move_preserves_its_previous_destination_through_expiry() {
    use canopa_files::{
        operations::RecoveryStatus,
        transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
    };
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    let external = root.path().join("incoming moved elsewhere.txt");
    let history = root.path().join("state/history");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"incoming moved contents").unwrap();
    let current = destination.join("report.txt");
    fs::write(&current, b"previous destination").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&history).unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
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
    fs::rename(&current, &external).unwrap();
    let finished = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::FinishStep)
        .unwrap();
    assert_eq!(finished.completed, 0);
    assert_eq!(
        finished.dismissed.as_slice(),
        std::slice::from_ref(&current)
    );
    assert!(finished.kept.is_empty());
    assert_eq!(finished.remaining, 0);
    assert!(engine.latest_undo_id().is_none());
    assert!(!source.exists() && !current.exists());
    assert_eq!(fs::read(&external).unwrap(), b"incoming moved contents");
    for index in 0..101 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("folder-{index}")),
                },
                &cancel,
            )
            .unwrap();
    }
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    let saved = &page.items[0];
    assert!(!saved.held_for_undo);
    assert_eq!(saved.status, RecoveryStatus::NeedsRecovery);
    let recovered = root.path().join("recovered previous destination.txt");
    engine.recover_copy(saved, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"previous destination");
    engine.undo(&cancel).unwrap();
    assert!(!recovered.exists());
    assert_eq!(fs::read(&external).unwrap(), b"incoming moved contents");
    assert_eq!(engine.recovery_items(None, &cancel).unwrap().items.len(), 1);
}

#[test]
fn an_unavailable_moved_item_can_be_skipped_cancelled_and_returned_after_it_reappears() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original.txt");
    let current = root.path().join("current.txt");
    let elsewhere = root.path().join("moved elsewhere.txt");
    let history = root.path().join("state/history");
    fs::write(&original, b"original moved contents").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&history).unwrap();
    let moved = engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::rename(&current, &elsewhere).unwrap();
    let mut asked = false;
    let outcome = engine
        .undo_interactive(None, &cancel, |question| {
            asked = true;
            assert_eq!(question.destination, original);
            assert_eq!(question.incoming.path(), current);
            assert!(question.incoming.entry().is_none());
            assert!(!question.can_return_item() && !question.can_replace());
            UndoChoice::Skip
        })
        .unwrap();
    assert!(asked);
    assert_eq!(outcome.completed, 0);
    assert_eq!(outcome.skipped.as_slice(), std::slice::from_ref(&current));
    assert_eq!(outcome.remaining, 1);
    assert_eq!(engine.latest_undo_id(), Some(moved.id));
    assert_eq!(fs::read(&elsewhere).unwrap(), b"original moved contents");
    assert!(!original.exists() && !current.exists());
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let cancelled = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Cancel)
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(engine.latest_undo_id(), Some(moved.id));
    fs::rename(&elsewhere, &current).unwrap();
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&original).unwrap(), b"original moved contents");
    assert!(!current.exists());
    assert!(engine.latest_undo_id().is_none());
}

#[test]
fn an_unavailable_moves_previous_destination_can_be_restored_without_returning_the_missing_item() {
    use canopa_files::transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    let elsewhere = root.path().join("incoming moved elsewhere.txt");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"incoming moved contents").unwrap();
    let current = destination.join("report.txt");
    fs::write(&current, b"saved previous destination").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&history).unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
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
    fs::rename(&current, &elsewhere).unwrap();
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let restored = engine
        .undo_interactive(None, &cancel, |question| {
            assert!(question.incoming.entry().is_none());
            assert!(question.can_restore_previous_destination());
            assert_eq!(question.saved_previous.as_ref().unwrap().size, 26);
            UndoChoice::RestorePreviousDestination
        })
        .unwrap();
    assert_eq!(restored.completed, 1);
    assert_eq!(
        restored.dismissed.as_slice(),
        std::slice::from_ref(&current)
    );
    assert_eq!(restored.remaining, 0);
    assert!(!source.exists());
    assert_eq!(fs::read(&elsewhere).unwrap(), b"incoming moved contents");
    assert_eq!(fs::read(&current).unwrap(), b"saved previous destination");
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert!(engine.latest_undo_id().is_none());
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn an_item_that_reappears_during_review_cannot_be_dismissed_or_replaced_by_its_saved_destination() {
    use canopa_files::transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest};
    for choice in [
        UndoChoice::FinishStep,
        UndoChoice::RestorePreviousDestination,
    ] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("report.txt");
        let destination = root.path().join("destination");
        let elsewhere = root.path().join("incoming moved elsewhere.txt");
        fs::create_dir(&destination).unwrap();
        fs::write(&source, b"incoming moved contents").unwrap();
        let current = destination.join("report.txt");
        fs::write(&current, b"saved previous destination").unwrap();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let moved = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![source.clone()],
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
        fs::rename(&current, &elsewhere).unwrap();
        let error = engine
            .undo_interactive(None, &cancel, |_| {
                fs::write(&current, b"new arrival while reviewing").unwrap();
                choice
            })
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert_eq!(engine.latest_undo_id(), moved.id);
        assert_eq!(fs::read(&current).unwrap(), b"new arrival while reviewing");
        assert_eq!(fs::read(&elsewhere).unwrap(), b"incoming moved contents");
        assert!(!source.exists());
        let saved = engine.recovery_items(None, &cancel).unwrap();
        assert_eq!(saved.items.len(), 1);
        assert!(saved.items[0].held_for_undo);
        assert_eq!(
            fs::read(&saved.items[0].entry.as_ref().unwrap().path).unwrap(),
            b"saved previous destination"
        );
        assert!(saved.interrupted.is_empty());
    }
}

#[test]
fn restoring_only_the_previous_destination_does_not_relocate_archives_as_if_the_missing_link_returned()
 {
    use canopa_files::transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest};
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("real folder");
    let original = root.path().join("alias");
    let destination = root.path().join("destination");
    let copy_source = root.path().join("report.txt");
    fs::create_dir(&folder).unwrap();
    fs::create_dir(&destination).unwrap();
    symlink(&folder, &original).unwrap();
    fs::write(folder.join("report.txt"), b"previous child").unwrap();
    fs::write(&copy_source, b"copied child").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![copy_source],
                destination: original.clone(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    fs::write(
        original.join("report.txt"),
        b"newer child edits for Recovery",
    )
    .unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Replace)
        .unwrap();
    let archived = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert!(!archived.held_for_undo);
    let archive_suffix = archived
        .entry
        .as_ref()
        .unwrap()
        .path
        .strip_prefix(&original)
        .unwrap()
        .to_owned();
    let current = destination.join("alias");
    fs::write(&current, b"previous link destination").unwrap();
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
    let elsewhere = root.path().join("missing link elsewhere");
    fs::rename(&current, &elsewhere).unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::RestorePreviousDestination)
        .unwrap();
    let archived = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert_eq!(
        archived.original.as_deref(),
        Some(current.join("report.txt").as_path())
    );
    assert_eq!(
        fs::read(folder.join(archive_suffix)).unwrap(),
        b"newer child edits for Recovery"
    );
    let held_previous = root.path().join("restored previous destination.txt");
    fs::rename(&current, &held_previous).unwrap();
    fs::rename(&elsewhere, &current).unwrap();
    let archived = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    let recovered = root.path().join("recovered child edits.txt");
    engine.recover_copy(&archived, &recovered, &cancel).unwrap();
    assert_eq!(
        fs::read(&recovered).unwrap(),
        b"newer child edits for Recovery"
    );
    assert_eq!(
        fs::read(&held_previous).unwrap(),
        b"previous link destination"
    );
    assert!(!original.exists());
}

#[test]
fn an_unreadable_move_can_be_finished_without_touching_the_item_or_disposing_its_saved_destination()
{
    use canopa_files::{
        operations::UndoItem,
        transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
    };
    use std::os::unix::fs::PermissionsExt;
    if rustix::process::geteuid().is_root() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"incoming contents").unwrap();
    let current = destination.join("report.txt");
    fs::write(&current, b"previous destination").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    fs::set_permissions(&destination, fs::Permissions::from_mode(0o000)).unwrap();
    let mut observed = Vec::new();
    let skipped = engine.undo_interactive(None, &cancel, |question| {
        observed.push((
            match question.incoming {
                UndoItem::Unavailable { kind, .. } => Some(kind),
                _ => None,
            },
            question.can_restore_previous_destination(),
        ));
        UndoChoice::Skip
    });
    let finished = engine.undo_interactive(None, &cancel, |_| UndoChoice::FinishStep);
    fs::set_permissions(&destination, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        observed,
        [(Some(std::io::ErrorKind::PermissionDenied), false)]
    );
    assert_eq!(skipped.unwrap().remaining, 1);
    let finished = finished.unwrap();
    assert_eq!(finished.completed, 0);
    assert_eq!(
        finished.dismissed.as_slice(),
        std::slice::from_ref(&current)
    );
    assert_eq!(finished.remaining, 0);
    assert_eq!(fs::read(&current).unwrap(), b"incoming contents");
    assert!(!source.exists());
    let saved = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert!(!saved.held_for_undo);
    let recovered = root.path().join("recovered previous.txt");
    engine.recover_copy(&saved, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"previous destination");
}

#[test]
fn restoring_the_shown_previous_destination_approves_its_current_version_in_one_review() {
    use canopa_files::{
        operations::UndoSource,
        transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
    };
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    let elsewhere = root.path().join("incoming elsewhere.txt");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"incoming contents").unwrap();
    let current = destination.join("report.txt");
    fs::write(&current, b"previous destination").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
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
    let saved = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0)
        .entry
        .unwrap()
        .path;
    fs::write(&saved, b"edited saved destination").unwrap();
    fs::rename(&current, &elsewhere).unwrap();
    let mut questions = Vec::new();
    let restored = engine
        .undo_interactive(None, &cancel, |question| {
            questions.push(question.source);
            if question.source == UndoSource::UnavailableMove {
                assert_eq!(question.saved_previous.as_ref().unwrap().size, 24);
                UndoChoice::RestorePreviousDestination
            } else {
                UndoChoice::Cancel
            }
        })
        .unwrap();
    assert_eq!(questions, [UndoSource::UnavailableMove]);
    assert_eq!(restored.completed, 1);
    assert!(!restored.cancelled);
    assert_eq!(fs::read(&current).unwrap(), b"edited saved destination");
    assert_eq!(fs::read(&elsewhere).unwrap(), b"incoming contents");
    assert!(!source.exists());
}
