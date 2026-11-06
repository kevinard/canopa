use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, RecoveryStatus, UndoChoice},
};
use std::{fs, os::unix::ffi::OsStringExt};

#[test]
fn restarting_after_a_killed_replace_undo_preserves_restored_and_displaced_work() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let root = tempfile::Builder::new()
        .prefix("canopa-undo-replace-crash-")
        .tempdir()
        .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::undo_replace::crash_replace_undo_helper",
        ])
        .env("CANOPA_FILES_REPLACE_CRASH_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(!root.path().join("undo-returned").exists());
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        fs::read(root.path().join("source/file-0499")).unwrap(),
        b"moved original contents"
    );
    let recovered = root.path().join("Recovered newer work");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(
        fs::read(&recovered).unwrap(),
        b"newer work at the source name"
    );
    engine.undo(&cancel).unwrap();
    engine.undo(&cancel).unwrap();
    for index in 0..500 {
        assert_eq!(
            fs::read(root.path().join(format!("source/file-{index:04}"))).unwrap(),
            b"moved original contents"
        );
    }
    assert_eq!(
        fs::read_dir(root.path().join("destination"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(engine.recovery_items(None, &cancel).unwrap().items.len(), 1);
}

#[test]
#[ignore = "child process fixture; killed after an actual Replace Undo filesystem reversal"]
fn crash_replace_undo_helper() {
    use canopa_files::transfers::{TransferKind, TransferReply, TransferRequest};
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_FILES_REPLACE_CRASH_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-undo-replace-crash-")
    );
    let source = root.join("source");
    let destination = root.join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    let sources: Vec<_> = (0..500)
        .map(|index| {
            let file = source.join(format!("file-{index:04}"));
            fs::write(&file, b"moved original contents").unwrap();
            file
        })
        .collect();
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let cancel = Cancellation::default();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources,
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    fs::write(source.join("file-0499"), b"newer work at the source name").unwrap();
    let current = destination.join("file-0499");
    let ready = std::sync::Arc::new(std::sync::Barrier::new(2));
    let watcher_ready = ready.clone();
    std::thread::spawn(move || {
        watcher_ready.wait();
        while current.exists() {
            std::thread::sleep(std::time::Duration::from_micros(50));
        }
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    ready.wait();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Replace)
        .unwrap();
    fs::write(root.join("undo-returned"), b"missed crash window").unwrap();
    panic!("the child was not killed during Undo");
}

#[test]
fn changes_during_an_undo_replace_question_preserve_both_files_and_leave_the_engine_usable() {
    for change_incoming in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("report.txt");
        let current = root.path().join("renamed.txt");
        fs::write(&original, b"original contents").unwrap();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        engine
            .execute(
                OperationRequest::Rename {
                    source: original.clone(),
                    destination: current.clone(),
                },
                &cancel,
            )
            .unwrap();
        fs::write(&original, b"newer work").unwrap();
        let error = engine
            .undo_interactive(None, &cancel, |_| {
                if change_incoming {
                    fs::rename(&current, root.path().join("saved original")).unwrap();
                    fs::write(&current, b"new occupant at the renamed path").unwrap();
                } else {
                    fs::write(&original, b"edited while the question was open").unwrap();
                }
                UndoChoice::Replace
            })
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert_eq!(
            fs::read(&original).unwrap(),
            if change_incoming {
                &b"newer work"[..]
            } else {
                &b"edited while the question was open"[..]
            }
        );
        assert_eq!(
            fs::read(&current).unwrap(),
            if change_incoming {
                &b"new occupant at the renamed path"[..]
            } else {
                &b"original contents"[..]
            }
        );
        assert!(
            engine
                .recovery_items(None, &cancel)
                .unwrap()
                .items
                .is_empty()
        );
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join("after failed Undo"),
                },
                &cancel,
            )
            .unwrap();
    }
}

#[test]
fn undo_trash_replace_restores_a_symlink_and_retains_the_newer_file() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original link");
    symlink("missing-relative-target", &original).unwrap();
    let location = canopa_files::trash::TrashLocation::home(root.path().join("data")).unwrap();
    let state = root.path().join("state/history");
    let engine = OperationEngine::open(&state).unwrap();
    let cancel = Cancellation::default();
    engine
        .trash(&location, std::slice::from_ref(&original), &cancel)
        .unwrap();
    fs::write(&original, b"newer file after Trash").unwrap();
    engine
        .undo_interactive(None, &cancel, |conflict| {
            assert_eq!(conflict.source, canopa_files::operations::UndoSource::Trash);
            assert!(conflict.can_replace());
            UndoChoice::Replace
        })
        .unwrap();
    assert_eq!(
        fs::read_link(&original).unwrap(),
        std::path::Path::new("missing-relative-target")
    );
    assert!(location.list(&cancel).unwrap().is_empty());
    drop(engine);
    let engine = OperationEngine::open(&state).unwrap();
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert_eq!(item.original.as_deref(), Some(original.as_path()));
    let recovered = root.path().join("Recovered newer file");
    engine.recover_copy(&item, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"newer file after Trash");
}

#[test]
fn undo_replace_of_a_move_restores_both_original_versions_and_keeps_open_newer_work() {
    use canopa_files::transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest};
    use std::io::{Seek, SeekFrom, Write};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&target).unwrap();
    let original = source.join("report.txt");
    let current = target.join("report.txt");
    fs::write(&original, b"incoming document").unwrap();
    fs::write(&current, b"previous destination document").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![original.clone()],
                destination: target,
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    fs::write(&original, b"newer document at source").unwrap();
    let mut open_newer = fs::OpenOptions::new().write(true).open(&original).unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Replace)
        .unwrap();
    assert_eq!(fs::read(&original).unwrap(), b"incoming document");
    assert_eq!(
        fs::read(&current).unwrap(),
        b"previous destination document"
    );
    open_newer.seek(SeekFrom::Start(0)).unwrap();
    open_newer.write_all(b"work saved after the Undo").unwrap();
    open_newer.set_len(25).unwrap();
    open_newer.sync_all().unwrap();
    let items = engine.recovery_items(None, &cancel).unwrap().items;
    assert_eq!(items.len(), 1);
    let recovered = root.path().join("Recovered newer document");
    engine.recover_copy(&items[0], &recovered, &cancel).unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"work saved after the Undo");
    assert_eq!(fs::read(original).unwrap(), b"incoming document");
}

#[test]
fn undo_replace_does_not_replace_folders_or_folder_file_collisions() {
    for (incoming_folder, existing_folder) in [(true, true), (true, false), (false, true)] {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original");
        let current = root.path().join("renamed");
        if incoming_folder {
            fs::create_dir(&original).unwrap();
            fs::write(original.join("contents"), b"incoming").unwrap();
        } else {
            fs::write(&original, b"incoming").unwrap();
        }
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        engine
            .execute(
                OperationRequest::Rename {
                    source: original.clone(),
                    destination: current.clone(),
                },
                &cancel,
            )
            .unwrap();
        if existing_folder {
            fs::create_dir(&original).unwrap();
            fs::write(original.join("contents"), b"newer").unwrap();
        } else {
            fs::write(&original, b"newer").unwrap();
        }
        assert_eq!(
            engine
                .undo_interactive(None, &cancel, |conflict| {
                    assert!(!conflict.can_replace());
                    UndoChoice::Replace
                })
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert_eq!(
            fs::read(if existing_folder {
                original.join("contents")
            } else {
                original.clone()
            })
            .unwrap(),
            b"newer"
        );
        assert_eq!(
            fs::read(if incoming_folder {
                current.join("contents")
            } else {
                current.clone()
            })
            .unwrap(),
            b"incoming"
        );
        assert!(
            engine
                .recovery_items(None, &cancel)
                .unwrap()
                .items
                .is_empty()
        );
        engine
            .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
            .unwrap();
        let returned = root.path().join("original (2)");
        assert_eq!(
            fs::read(if incoming_folder {
                returned.join("contents")
            } else {
                returned
            })
            .unwrap(),
            b"incoming"
        );
    }
}

#[test]
fn undo_replace_retains_newer_work_for_recovery_after_history_expiry_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let original = root
        .path()
        .join(std::ffi::OsString::from_vec(b"report-\xff.txt".to_vec()));
    let current = root.path().join("renamed.txt");
    let state = root.path().join("state/history");
    fs::write(&original, b"original contents").unwrap();
    let engine = OperationEngine::open(&state).unwrap();
    let cancel = Cancellation::default();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::write(&original, b"newer work at the original name").unwrap();
    let outcome = engine
        .undo_interactive(None, &cancel, |conflict| {
            assert_eq!(conflict.incoming.entry().unwrap().path.clone(), current);
            assert_eq!(conflict.existing.as_ref().unwrap().path, original);
            UndoChoice::Replace
        })
        .unwrap();
    assert_eq!(outcome.completed, 1);
    assert_eq!(outcome.remaining, 0);
    assert_eq!(fs::read(&original).unwrap(), b"original contents");
    assert!(!current.exists());
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
    let engine = OperationEngine::open(&state).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    let item = &page.items[0];
    assert_eq!(item.original.as_deref(), Some(original.as_path()));
    assert_eq!(item.status, RecoveryStatus::NeedsRecovery);
    assert!(!item.held_for_undo);
    let recovered = root.path().join("Recovered newer report.txt");
    engine.recover_copy(item, &recovered, &cancel).unwrap();
    assert_eq!(
        fs::read(&recovered).unwrap(),
        b"newer work at the original name"
    );
    engine.undo(&cancel).unwrap();
    assert!(!recovered.exists());
    assert_eq!(engine.recovery_items(None, &cancel).unwrap().items.len(), 1);
    assert_eq!(fs::read(original).unwrap(), b"original contents");
}
