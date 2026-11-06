use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest},
};

#[test]
fn unavailable_history_storage_prevents_mutation_and_has_one_writer() {
    let root = tempfile::tempdir().unwrap();
    let state = root.path().join("state");
    let path = state.join("operations.json");
    let engine = OperationEngine::open(&path).unwrap();
    assert!(OperationEngine::open(&path).is_err());
    fs::rename(&state, root.path().join("saved-state")).unwrap();
    fs::write(&state, b"not a directory").unwrap();
    let created = root.path().join("new");
    assert!(
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: created.clone()
                },
                &Cancellation::default()
            )
            .is_err()
    );
    assert!(!created.exists());
    assert!(!fs::read_dir(root.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .as_encoded_bytes()
            .starts_with(b".canopa-files-stage-")
    }));
}
use std::{fs, os::unix::ffi::OsStringExt};

#[test]
fn restart_after_a_killed_keep_both_undo_preserves_older_history_and_newer_work() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let root = tempfile::Builder::new()
        .prefix("canopa-undo-crash-")
        .tempdir()
        .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::operations::crash_keep_both_undo_helper",
        ])
        .env("CANOPA_FILES_UNDO_CRASH_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(
        !root.path().join("undo-returned").exists(),
        "the child must be killed before the Undo API completes"
    );
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    assert!(engine.undo_label().unwrap().contains("copy"));
    engine.undo(&Cancellation::default()).unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!root.path().join("original (2)").exists());
    assert_eq!(
        fs::read(root.path().join("original/newer.txt")).unwrap(),
        b"preserve newer work"
    );
    assert_eq!(
        fs::read(root.path().join("source/file-0000")).unwrap(),
        b"keep for Undo"
    );
    assert!(engine.undo_label().is_none());
}

#[test]
#[ignore = "child process fixture; killed after an actual Keep both filesystem reversal"]
fn crash_keep_both_undo_helper() {
    use canopa_files::{
        operations::UndoChoice,
        transfers::{TransferKind, TransferReply, TransferRequest},
    };
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_FILES_UNDO_CRASH_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-undo-crash-")
    );
    let source = root.join("source");
    fs::create_dir(&source).unwrap();
    let original = root.join("original");
    let current = root.join("renamed");
    let engine = OperationEngine::open(root.join("state/operations.sqlite3")).unwrap();
    let cancel = Cancellation::default();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: original.clone(),
            },
            &cancel,
        )
        .unwrap();
    let sources: Vec<_> = (0..4000)
        .map(|number| {
            let path = source.join(format!("file-{number:04}"));
            fs::write(&path, b"keep for Undo").unwrap();
            path
        })
        .collect();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources,
                destination: original.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
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
    fs::write(original.join("newer.txt"), b"preserve newer work").unwrap();
    let restored = root.join("original (2)");
    let armed = std::sync::Arc::new(std::sync::Barrier::new(2));
    let watcher_ready = armed.clone();
    std::thread::spawn(move || {
        watcher_ready.wait();
        while !restored.exists() {
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    armed.wait();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    fs::write(root.join("undo-returned"), b"missed crash window").unwrap();
    panic!("the child was not killed during Undo");
}

#[test]
fn keeping_both_during_undo_restores_a_new_name_and_preserves_older_history() {
    use canopa_files::operations::UndoChoice;
    let root = tempfile::tempdir().unwrap();
    let journal = root.path().join("state/operations.sqlite3");
    let original = root
        .path()
        .join(std::ffi::OsString::from_vec(b"new-\xff".to_vec()));
    let restored = root
        .path()
        .join(std::ffi::OsString::from_vec(b"new-\xff (2)".to_vec()));
    let current = root.path().join("renamed");
    let engine = OperationEngine::open(&journal).unwrap();
    let cancel = Cancellation::default();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: original.clone(),
            },
            &cancel,
        )
        .unwrap();
    let renamed = engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("newer.txt"), b"keep newer work").unwrap();
    let result = engine
        .undo_interactive(Some(renamed.id), &cancel, |conflict| {
            assert_eq!(conflict.incoming.entry().unwrap().path.clone(), current);
            assert_eq!(conflict.existing.as_ref().unwrap().path, original);
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(result.summary.paths, std::slice::from_ref(&restored));
    assert_eq!(result.remaining, 0);
    assert!(!result.cancelled);
    assert!(restored.is_dir());
    assert_eq!(
        fs::read(original.join("newer.txt")).unwrap(),
        b"keep newer work"
    );
    drop(engine);
    let engine = OperationEngine::open(&journal).unwrap();
    engine.undo(&cancel).unwrap();
    assert!(
        !restored.exists(),
        "the older create-folder Undo follows the restored item"
    );
    assert_eq!(
        fs::read(original.join("newer.txt")).unwrap(),
        b"keep newer work"
    );
    assert!(engine.undo_label().is_none());
}

#[test]
fn keep_both_for_a_folder_preserves_the_replacement_backups_inside_it() {
    use canopa_files::{
        operations::UndoChoice,
        transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
    };
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original");
    fs::create_dir(&original).unwrap();
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("report.txt"), b"incoming version").unwrap();
    fs::write(original.join("report.txt"), b"previous version").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let cancel = Cancellation::default();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.join("report.txt")],
                destination: original.clone(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: root.path().join("renamed"),
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("newer.txt"), b"newer work").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    engine.undo(&cancel).unwrap();
    assert_eq!(
        fs::read(root.path().join("original (2)/report.txt")).unwrap(),
        b"previous version"
    );
    assert_eq!(fs::read(original.join("newer.txt")).unwrap(), b"newer work");
    assert_eq!(
        fs::read_dir(root.path().join("original (2)"))
            .unwrap()
            .count(),
        1,
        "the restored backup's stage is released"
    );
    assert!(engine.undo_label().is_none());
}

#[test]
fn skipping_an_undo_conflict_keeps_it_retryable_and_undoes_other_items() {
    use canopa_files::{
        operations::UndoChoice,
        transfers::{TransferKind, TransferReply, TransferRequest},
    };
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("a.txt"), b"first").unwrap();
    fs::write(source.join("b.txt"), b"second").unwrap();
    let journal = root.path().join("state/operations.sqlite3");
    let engine = OperationEngine::open(&journal).unwrap();
    let cancel = Cancellation::default();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.join("a.txt"), source.join("b.txt")],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    fs::write(source.join("b.txt"), b"newer second").unwrap();
    let partial = engine
        .undo_interactive(moved.id, &cancel, |_| UndoChoice::Skip)
        .unwrap();
    assert_eq!(partial.completed, 1);
    assert_eq!(partial.remaining, 1);
    assert_eq!(fs::read(source.join("a.txt")).unwrap(), b"first");
    assert_eq!(fs::read(source.join("b.txt")).unwrap(), b"newer second");
    assert_eq!(fs::read(destination.join("b.txt")).unwrap(), b"second");
    assert!(!destination.join("a.txt").exists());
    drop(engine);
    let engine = OperationEngine::open(&journal).unwrap();
    let finished = engine
        .undo_interactive(moved.id, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    assert_eq!(finished.remaining, 0);
    assert_eq!(fs::read(source.join("b (2).txt")).unwrap(), b"second");
    assert_eq!(fs::read(source.join("b.txt")).unwrap(), b"newer second");
    assert!(engine.undo_label().is_none());
}

#[test]
fn keep_both_retargets_history_through_a_restored_directory_symlink() {
    use canopa_files::operations::UndoChoice;
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    fs::create_dir(&target).unwrap();
    let link = root.path().join("link");
    std::os::unix::fs::symlink("target", &link).unwrap();
    let current = root.path().join("renamed-link");
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let cancel = Cancellation::default();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: link.join("created"),
            },
            &cancel,
        )
        .unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: link.clone(),
                destination: current,
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir(&link).unwrap();
    fs::create_dir(link.join("created")).unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    engine.undo(&cancel).unwrap();
    assert!(!target.join("created").exists());
    assert!(link.join("created").is_dir());
    assert_eq!(
        fs::read_link(root.path().join("link (2)")).unwrap(),
        std::path::Path::new("target")
    );
}

#[test]
fn undo_trash_can_keep_both_and_older_undo_follows_the_restored_file() {
    use canopa_files::operations::UndoChoice;
    let root = tempfile::tempdir().unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let original = root.path().join("start.txt");
    let renamed = root.path().join("work.txt");
    fs::write(&original, b"before Trash").unwrap();
    let cancel = Cancellation::default();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: renamed.clone(),
            },
            &cancel,
        )
        .unwrap();
    engine
        .trash_files(
            &root.path().join("data"),
            std::slice::from_ref(&renamed),
            &cancel,
        )
        .unwrap();
    fs::write(&renamed, b"newer work").unwrap();
    let outcome = engine
        .undo_interactive(None, &cancel, |conflict| {
            assert_eq!(conflict.existing.as_ref().unwrap().path, renamed);
            assert_eq!(conflict.source, canopa_files::operations::UndoSource::Trash);
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(outcome.completed, 1);
    assert_eq!(
        fs::read(root.path().join("work (2).txt")).unwrap(),
        b"before Trash"
    );
    assert_eq!(fs::read(&renamed).unwrap(), b"newer work");
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&original).unwrap(), b"before Trash");
    assert_eq!(fs::read(&renamed).unwrap(), b"newer work");
    assert!(!root.path().join("work (2).txt").exists());
    assert!(engine.undo_label().is_none());
}

#[test]
fn existing_json_history_is_migrated_once_and_remains_undoable() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let root = tempfile::tempdir().unwrap();
    let created = root.path().join("created");
    fs::create_dir(&created).unwrap();
    let state = root.path().join("state");
    fs::create_dir(&state).unwrap();
    let metadata = fs::symlink_metadata(&created).unwrap();
    fs::write(state.join("operations.json"), serde_json::to_vec(&serde_json::json!({"version":1, "next_id":9, "entries":[{"id":7,"label":"create folder created","change":{"kind":"created-folder","path":created,"identity":{"device":metadata.dev(),"inode":metadata.ino()}}}],"pending":null})).unwrap()).unwrap();
    let path = state.join("operations.sqlite3");
    let engine = OperationEngine::open(&path).unwrap();
    assert!(engine.undo_label().unwrap().contains("create folder"));
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!created.exists());
    let summary = engine
        .execute(
            OperationRequest::CreateFolder {
                path: created.clone(),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(summary.id >= 9);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    drop(engine);
    let engine = OperationEngine::open(&path).unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert!(engine.undo_label().is_none());
    assert!(!created.exists());
}

#[test]
fn creating_and_renaming_a_folder_remain_undoable_after_reopening_history() {
    let root = tempfile::tempdir().unwrap();
    let journal = root.path().join("state/operations.json");
    let created = root
        .path()
        .join(std::ffi::OsString::from_vec(b"new-\xff".to_vec()));
    let renamed = root.path().join("Organized");
    let engine = OperationEngine::open(&journal).unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: created.clone(),
            },
            &Cancellation::default(),
        )
        .unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: created.clone(),
                destination: renamed.clone(),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!created.exists());
    assert!(renamed.is_dir());
    drop(engine);

    let engine = OperationEngine::open(&journal).unwrap();
    assert!(engine.undo_label().unwrap().contains("rename"));
    engine.undo(&Cancellation::default()).unwrap();
    assert!(created.is_dir());
    assert!(!renamed.exists());
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!created.exists());
    assert!(engine.undo_label().is_none());
}

#[test]
fn rename_and_undo_preserve_files_that_now_occupy_the_destination() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original");
    let renamed = root.path().join("renamed");
    fs::write(&original, b"original contents").unwrap();
    fs::write(&renamed, b"existing destination").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.json")).unwrap();
    assert_eq!(
        engine
            .execute(
                OperationRequest::Rename {
                    source: original.clone(),
                    destination: renamed.clone()
                },
                &Cancellation::default()
            )
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&renamed).unwrap(), b"existing destination");
    fs::remove_file(&renamed).unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: renamed.clone(),
            },
            &Cancellation::default(),
        )
        .unwrap();
    fs::write(&original, b"newer work").unwrap();
    assert_eq!(
        engine.undo(&Cancellation::default()).unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&original).unwrap(), b"newer work");
    assert_eq!(fs::read(&renamed).unwrap(), b"original contents");
    assert!(engine.undo_label().is_some());
}

#[test]
fn undoing_folder_creation_keeps_new_contents_and_cancelled_requests_do_nothing() {
    let root = tempfile::tempdir().unwrap();
    let created = root.path().join("new");
    let engine = OperationEngine::open(root.path().join("state/operations.json")).unwrap();
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: created.clone()
                },
                &cancel
            )
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    assert!(!created.exists());
    assert!(engine.undo_label().is_none());
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: created.clone(),
            },
            &Cancellation::default(),
        )
        .unwrap();
    fs::write(created.join("keep.txt"), b"newer work").unwrap();
    assert!(engine.undo(&Cancellation::default()).is_err());
    assert_eq!(fs::read(created.join("keep.txt")).unwrap(), b"newer work");
    assert!(engine.undo_label().is_some());
}
#[test]
fn an_undo_action_for_an_older_operation_never_reverses_newer_work() {
    let root = tempfile::tempdir().unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let cancellation = Cancellation::default();
    let first = engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("first"),
            },
            &cancellation,
        )
        .unwrap();
    let second = engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("second"),
            },
            &cancellation,
        )
        .unwrap();
    assert_eq!(
        engine
            .undo_operation(first.id, &cancellation)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(root.path().join("first").is_dir());
    assert!(root.path().join("second").is_dir());
    engine.undo_operation(second.id, &cancellation).unwrap();
    assert!(root.path().join("first").is_dir());
    assert!(!root.path().join("second").exists());
}
