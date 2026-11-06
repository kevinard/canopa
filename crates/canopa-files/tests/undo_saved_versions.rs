use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, UndoChoice, UndoSource},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    io::{Seek, Write},
};

#[test]
fn an_edited_saved_version_can_be_reviewed_skipped_and_restored_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("report.txt"), b"incoming copy").unwrap();
    let target = destination.join("report.txt");
    fs::write(&target, b"previous destination").unwrap();
    let mut editor = fs::OpenOptions::new().write(true).open(&target).unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&history).unwrap();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.join("report.txt")],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    editor.rewind().unwrap();
    editor.write_all(b"edited saved version").unwrap();
    editor.set_len(20).unwrap();
    editor.sync_all().unwrap();
    let skipped = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedRetained);
            assert_eq!(question.destination, target);
            assert!(question.existing.is_none());
            assert_eq!(question.incoming.entry().unwrap().size, 20);
            assert!(question.can_use_saved_version());
            assert!(!question.can_replace());
            UndoChoice::Skip
        })
        .unwrap();
    assert_eq!(skipped.remaining, 1);
    assert_eq!(skipped.skipped, vec![target.clone()]);
    assert_eq!(fs::read(&target).unwrap(), b"incoming copy");
    assert!(engine.undo(&cancel).is_err());
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let cancelled = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Cancel)
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.remaining, 1);
    let restored = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedRetained);
            UndoChoice::UseSavedVersion
        })
        .unwrap();
    assert_eq!(restored.completed, 1);
    assert_eq!(restored.remaining, 0);
    assert_eq!(fs::read(&target).unwrap(), b"edited saved version");
    assert_eq!(
        fs::read(source.join("report.txt")).unwrap(),
        b"incoming copy"
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
fn a_replaced_saved_version_keeps_older_history_bound_to_the_chosen_file() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    let source = root.path().join("source");
    fs::create_dir(&destination).unwrap();
    fs::create_dir(&source).unwrap();
    let before = destination.join("before.txt");
    let target = destination.join("report.txt");
    fs::write(&before, b"older saved file").unwrap();
    fs::write(source.join("report.txt"), b"incoming copy").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let older = engine
        .execute(
            OperationRequest::Rename {
                source: before.clone(),
                destination: target.clone(),
            },
            &cancel,
        )
        .unwrap();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.join("report.txt")],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    let page = engine.recovery_items(None, &cancel).unwrap();
    let saved = page.items[0].entry.as_ref().unwrap().path.clone();
    let external = root.path().join("saved elsewhere.txt");
    fs::rename(&saved, &external).unwrap();
    fs::write(&saved, b"selected replacement saved file").unwrap();
    let restored = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedRetained);
            UndoChoice::UseSavedVersion
        })
        .unwrap();
    assert_eq!(restored.remaining, 0);
    assert_eq!(
        fs::read(&target).unwrap(),
        b"selected replacement saved file"
    );
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert_eq!(engine.latest_undo_id(), Some(older.id));
    engine.undo(&cancel).unwrap();
    assert_eq!(
        fs::read(&before).unwrap(),
        b"selected replacement saved file"
    );
    assert_eq!(fs::read(&external).unwrap(), b"older saved file");
    assert!(!target.exists());
}

#[test]
fn changes_during_saved_version_review_leave_both_files_and_history_untouched() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::write(&source, b"incoming copy").unwrap();
    fs::create_dir(&destination).unwrap();
    let target = destination.join("report.txt");
    fs::write(&target, b"old file").unwrap();
    let mut editor = fs::OpenOptions::new().write(true).open(&target).unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
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
    assert!(copied.failures.is_empty());
    editor.write_all(b"changed saved file").unwrap();
    editor.sync_all().unwrap();
    let mut saved = None;
    let failure = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedRetained);
            fs::write(
                &question.incoming.entry().unwrap().path,
                b"changed again while waiting",
            )
            .unwrap();
            saved = Some(question.incoming.entry().unwrap().path.clone());
            UndoChoice::UseSavedVersion
        })
        .unwrap_err();
    assert_eq!(failure.kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(
        fs::read(saved.unwrap()).unwrap(),
        b"changed again while waiting"
    );
    assert_eq!(fs::read(&target).unwrap(), b"incoming copy");
    assert_eq!(fs::read(&source).unwrap(), b"incoming copy");
    assert_eq!(engine.latest_undo_id(), copied.id);
    let skipped = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Skip)
        .unwrap();
    assert_eq!(skipped.remaining, 1);
}

#[test]
fn saved_version_review_is_followed_by_the_normal_destination_conflict_choices() {
    for choice in [UndoChoice::KeepBoth, UndoChoice::Replace] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("report.txt");
        let destination = root.path().join("destination");
        fs::write(&source, b"incoming copy").unwrap();
        fs::create_dir(&destination).unwrap();
        let target = destination.join("report.txt");
        fs::write(&target, b"previous destination").unwrap();
        let mut editor = fs::OpenOptions::new().write(true).open(&target).unwrap();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let copied = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
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
        assert!(copied.failures.is_empty());
        editor.write_all(b"edited saved version").unwrap();
        editor.sync_all().unwrap();
        fs::write(&target, b"newer destination work").unwrap();
        let mut questions = Vec::new();
        let outcome = engine
            .undo_interactive(None, &cancel, |question| {
                questions.push(question.source);
                if question.source == UndoSource::ChangedRetained {
                    assert!(question.existing.is_none());
                    UndoChoice::UseSavedVersion
                } else {
                    assert_eq!(question.source, UndoSource::PreviousDestination);
                    assert_eq!(question.existing.as_ref().unwrap().path, target);
                    choice
                }
            })
            .unwrap();
        assert_eq!(
            questions,
            vec![UndoSource::ChangedRetained, UndoSource::PreviousDestination]
        );
        assert_eq!(outcome.remaining, 0);
        if choice == UndoChoice::KeepBoth {
            assert_eq!(
                fs::read(destination.join("report (2).txt")).unwrap(),
                b"edited saved version"
            );
            assert_eq!(fs::read(&target).unwrap(), b"newer destination work");
        } else {
            assert_eq!(fs::read(&target).unwrap(), b"edited saved version");
            let page = engine.recovery_items(None, &cancel).unwrap();
            assert_eq!(page.items.len(), 1);
            assert_eq!(
                fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
                b"newer destination work"
            );
        }
        assert_eq!(fs::read(&source).unwrap(), b"incoming copy");
    }
}

#[test]
fn undoing_a_move_can_restore_a_replaced_saved_link_without_following_its_target() {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"incoming moved file").unwrap();
    let target = destination.join("report.txt");
    let old_link_target = root.path().join("old target");
    fs::write(&old_link_target, b"original link target stays intact").unwrap();
    symlink(&old_link_target, &target).unwrap();
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
    assert!(!source.exists());
    let saved = engine.recovery_items(None, &cancel).unwrap().items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .clone();
    let external = root.path().join("original saved link");
    fs::rename(&saved, &external).unwrap();
    let raw_target = root
        .path()
        .join(std::ffi::OsString::from_vec(b"new target-\xff".to_vec()));
    fs::write(&raw_target, b"replacement link target stays intact").unwrap();
    symlink(&raw_target, &saved).unwrap();
    let restored = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedRetained);
            assert_eq!(
                question.incoming.entry().unwrap().kind,
                canopa_files::browse::EntryKind::Symlink
            );
            assert!(question.can_use_saved_version());
            UndoChoice::UseSavedVersion
        })
        .unwrap();
    assert_eq!(restored.remaining, 0);
    assert_eq!(fs::read(&source).unwrap(), b"incoming moved file");
    assert_eq!(fs::read_link(&target).unwrap(), raw_target);
    assert_eq!(fs::read_link(&external).unwrap(), old_link_target);
    assert_eq!(
        fs::read(&raw_target).unwrap(),
        b"replacement link target stays intact"
    );
    assert_eq!(
        fs::read(&old_link_target).unwrap(),
        b"original link target stays intact"
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
fn replacing_the_saved_versions_folder_during_review_does_not_authorize_its_replacement() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::write(&source, b"incoming copy").unwrap();
    fs::create_dir(&destination).unwrap();
    let target = destination.join("report.txt");
    fs::write(&target, b"old file").unwrap();
    let mut editor = fs::OpenOptions::new().write(true).open(&target).unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
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
    assert!(copied.failures.is_empty());
    editor.write_all(b"edited saved version").unwrap();
    editor.sync_all().unwrap();
    let quarantine = root.path().join("original saved folder");
    let mut saved = None;
    let error = engine
        .undo_interactive(None, &cancel, |question| {
            let path = question.incoming.entry().unwrap().path.clone();
            let folder = path.parent().unwrap();
            fs::rename(folder, &quarantine).unwrap();
            fs::create_dir(folder).unwrap();
            fs::write(&path, b"unexpected replacement").unwrap();
            saved = Some(path);
            UndoChoice::UseSavedVersion
        })
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    let saved = saved.unwrap();
    assert_eq!(fs::read(&saved).unwrap(), b"unexpected replacement");
    assert_eq!(fs::read(&target).unwrap(), b"incoming copy");
    assert_eq!(engine.latest_undo_id(), copied.id);
    fs::remove_file(&saved).unwrap();
    fs::remove_dir(saved.parent().unwrap()).unwrap();
    fs::rename(&quarantine, saved.parent().unwrap()).unwrap();
    let restored = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::UseSavedVersion)
        .unwrap();
    assert_eq!(restored.remaining, 0);
    assert_eq!(fs::read(&target).unwrap(), b"edited saved version");
    assert_eq!(fs::read(&source).unwrap(), b"incoming copy");
}

#[test]
fn restarting_a_killed_saved_version_restore_preserves_the_choice_and_older_history() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let root = tempfile::Builder::new()
        .prefix("canopa-saved-undo-crash-")
        .tempdir()
        .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::undo_saved_versions::crash_saved_version_restore_helper",
        ])
        .env("CANOPA_FILES_SAVED_CRASH_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(!root.path().join("undo-returned").exists());
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let restored = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedRetained);
            UndoChoice::UseSavedVersion
        })
        .unwrap();
    assert_eq!(restored.remaining, 0);
    for index in 0..500 {
        assert_eq!(
            fs::read(root.path().join(format!("destination/file-{index:04}"))).unwrap(),
            b"previous destination"
        );
    }
    engine.undo(&cancel).unwrap();
    for index in 0..500 {
        assert_eq!(
            fs::read(root.path().join(format!("older/file-{index:04}"))).unwrap(),
            b"previous destination"
        );
        assert_eq!(
            fs::read(root.path().join(format!("incoming/file-{index:04}"))).unwrap(),
            b"incoming replacement"
        );
        assert_eq!(
            fs::read(root.path().join(format!("external/file-{index:04}"))).unwrap(),
            b"previous destination"
        );
    }
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
}

#[test]
fn a_failed_reviewed_saved_return_can_be_copied_kept_and_preserved_through_expiry() {
    use std::os::unix::fs::PermissionsExt;
    // This failure depends on ordinary directory permissions. Linux root
    // bypasses them; the native application and normal CI run as a user.
    if rustix::process::geteuid().is_root() {
        eprintln!("requires an unprivileged user to refuse the saved-file rename");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::write(&source, b"incoming moved file").unwrap();
    fs::create_dir(&destination).unwrap();
    let target = destination.join("report.txt");
    fs::write(&target, b"old destination").unwrap();
    let mut editor = fs::OpenOptions::new().write(true).open(&target).unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
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
    editor.write_all(b"edited saved destination").unwrap();
    editor.sync_all().unwrap();
    let mut saved = None;
    let error = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedRetained);
            let path = question.incoming.entry().unwrap().path.clone();
            fs::set_permissions(path.parent().unwrap(), fs::Permissions::from_mode(0o500)).unwrap();
            saved = Some(path);
            UndoChoice::UseSavedVersion
        })
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    let saved = saved.unwrap();
    fs::set_permissions(saved.parent().unwrap(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(fs::read(&source).unwrap(), b"incoming moved file");
    assert!(!target.exists());
    assert_eq!(fs::read(&saved).unwrap(), b"edited saved destination");
    assert_eq!(engine.latest_undo_id(), moved.id);
    // The proved permission refusal remains available for Copy/Keep until a
    // new Retry or restart reopens its exact selected entry and parents.
    let work = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .into_iter()
        .find(|item| {
            item.entry
                .as_ref()
                .is_some_and(|entry| entry.path == saved.parent().unwrap())
        })
        .expect("the consumed return exposes its whole original work folder");
    assert!(work.can_keep_in_recovery());
    let before_keep = root.path().join("Recovery before Keep");
    let exported = engine.recover_copy(&work, &before_keep, &cancel).unwrap();
    assert_eq!(
        fs::read(before_keep.join("previous")).unwrap(),
        b"edited saved destination"
    );
    assert_eq!(fs::read(&saved).unwrap(), b"edited saved destination");
    let fresh = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .into_iter()
        .find(|item| {
            item.entry
                .as_ref()
                .is_some_and(|entry| entry.path == saved.parent().unwrap())
        })
        .unwrap();
    engine.keep_in_recovery(&fresh, &cancel).unwrap();
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .interrupted
            .is_empty()
    );
    assert_eq!(
        engine.latest_undo_id(),
        Some(exported.id),
        "Keep retires the interrupted return and preserves Undo of its recovery export"
    );
    assert!(!target.exists());
    drop(editor);
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert_eq!(engine.latest_undo_id(), Some(exported.id));
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
    assert_eq!(
        page.items.len(),
        1,
        "an approved saved version must not be discarded when its unfinished restoration expires"
    );
    let item = &page.items[0];
    assert!(!item.held_for_undo);
    assert_eq!(
        item.status,
        canopa_files::operations::RecoveryStatus::NeedsRecovery
    );
    let recovered = root.path().join("Recovered edited destination.txt");
    engine.recover_copy(item, &recovered, &cancel).unwrap();
    assert_eq!(
        fs::read(recovered.join("previous")).unwrap(),
        b"edited saved destination"
    );
    assert_eq!(fs::read(&saved).unwrap(), b"edited saved destination");
    assert_eq!(fs::read(&source).unwrap(), b"incoming moved file");
    assert!(!target.exists());
}

#[test]
#[ignore = "child process fixture; killed after an actual selected saved-version restoration"]
fn crash_saved_version_restore_helper() {
    use std::os::unix::fs::MetadataExt;
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_FILES_SAVED_CRASH_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-saved-undo-crash-")
    );
    let older = root.join("older");
    let incoming = root.join("incoming");
    let destination = root.join("destination");
    let external = root.join("external");
    for directory in [&older, &incoming, &destination, &external] {
        fs::create_dir(directory).unwrap();
    }
    for index in 0..500 {
        fs::write(
            older.join(format!("file-{index:04}")),
            b"previous destination",
        )
        .unwrap();
        fs::write(
            incoming.join(format!("file-{index:04}")),
            b"incoming replacement",
        )
        .unwrap();
    }
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let cancel = Cancellation::default();
    for source in [older, incoming] {
        let copied = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: (0..500)
                        .map(|index| source.join(format!("file-{index:04}")))
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
        assert!(copied.failures.is_empty());
    }
    let mut cursor = None;
    let mut selected = None;
    let mut saved_count = 0;
    loop {
        let page = engine.recovery_items(cursor, &cancel).unwrap();
        for item in page.items {
            let saved = item.entry.unwrap().path;
            let original = item.original.unwrap();
            let metadata = fs::metadata(&saved).unwrap();
            let elsewhere = external.join(original.file_name().unwrap());
            fs::rename(&saved, &elsewhere).unwrap();
            fs::copy(&elsewhere, &saved).unwrap();
            fs::OpenOptions::new()
                .write(true)
                .open(&saved)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
                .unwrap();
            if original.file_name().unwrap() == "file-0499" {
                let metadata = fs::metadata(&saved).unwrap();
                selected = Some((original, metadata.dev(), metadata.ino()));
            }
            saved_count += 1;
        }
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(saved_count, 500);
    let (restored, device, inode) = selected.unwrap();
    let ready = std::sync::Arc::new(std::sync::Barrier::new(2));
    let watcher_ready = ready.clone();
    std::thread::spawn(move || {
        watcher_ready.wait();
        while !fs::metadata(&restored)
            .is_ok_and(|metadata| metadata.dev() == device && metadata.ino() == inode)
        {
            std::thread::sleep(std::time::Duration::from_micros(50));
        }
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    ready.wait();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::UseSavedVersion)
        .unwrap();
    fs::write(root.join("undo-returned"), b"missed crash window").unwrap();
    panic!("the child was not killed during selected saved-version restoration");
}
