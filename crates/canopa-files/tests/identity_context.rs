use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, UndoChoice, trash::TrashLocation},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::{fs, path::Path};

fn move_one(engine: &OperationEngine, source: &Path, destination: &Path, cancel: &Cancellation) {
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.to_owned()],
                destination: destination.to_owned(),
            },
            cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(result.failures.is_empty());
}

#[test]
fn a_temporary_move_into_an_empty_path_restores_its_prior_adoption_on_undo() {
    let root = tempfile::tempdir().unwrap();
    let before = root.path().join("before");
    let original = root.path().join("original");
    let current = root.path().join("current");
    let outside = root.path().join("original physical item elsewhere");
    let parked = root.path().join("prior adopted version elsewhere");
    let preserved = root.path().join("original physical item preserved");
    fs::write(&before, b"original physical item").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let rename = |from: &Path, to: &Path| {
        engine
            .execute(
                OperationRequest::Rename {
                    source: from.to_owned(),
                    destination: to.to_owned(),
                },
                &cancel,
            )
            .unwrap()
    };
    rename(&before, &original);
    rename(&original, &current);
    fs::rename(&current, &outside).unwrap();
    fs::write(&current, b"prior adopted version").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::ReturnItem)
        .unwrap();
    fs::rename(&original, &parked).unwrap();
    rename(&outside, &original);
    rename(&original, &current);
    fs::rename(&current, &preserved).unwrap();
    fs::write(&current, b"newest selected version").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::ReturnItem)
        .unwrap();
    engine.undo(&cancel).unwrap();
    assert!(!original.exists());
    fs::rename(&parked, &original).unwrap();
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&before).unwrap(), b"prior adopted version");
    assert_eq!(fs::read(&outside).unwrap(), b"newest selected version");
    assert_eq!(fs::read(&preserved).unwrap(), b"original physical item");
}

#[test]
fn trash_recovery_recognizes_a_previously_adopted_identity_after_a_permission_failure() {
    use std::os::unix::fs::PermissionsExt;
    if rustix::process::geteuid().is_root() {
        return;
    }
    for undo_restore in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let older = root.path().join("older");
        let source = root.path().join("source");
        let next = root.path().join("next");
        for folder in [&older, &source, &next] {
            fs::create_dir(folder).unwrap();
        }
        let original = source.join("report.txt");
        fs::write(older.join("report.txt"), b"original copied document").unwrap();
        let history = root.path().join("state/history");
        let engine = OperationEngine::open(&history).unwrap();
        let cancel = Cancellation::default();
        let copied = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![older.join("report.txt")],
                    destination: source.clone(),
                },
                &cancel,
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(copied.failures.is_empty());
        let location = TrashLocation::home(root.path().join("data")).unwrap();
        engine
            .trash(&location, std::slice::from_ref(&original), &cancel)
            .unwrap();
        let item = location.list(&cancel).unwrap().remove(0);
        let restored_id = if undo_restore {
            Some(engine.restore(&item, None, &cancel).unwrap().id)
        } else {
            None
        };
        let current = if undo_restore {
            original.clone()
        } else {
            item.path.clone()
        };
        let metadata = fs::metadata(&current).unwrap();
        move_one(&engine, &current, &next, &cancel);
        let moved = next.join(current.file_name().unwrap());
        let external = root.path().join("original physical item elsewhere");
        fs::rename(&moved, &external).unwrap();
        fs::copy(&external, &moved).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&moved)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        engine
            .undo_interactive(None, &cancel, |_| UndoChoice::ReturnItem)
            .unwrap();
        let blocked = if undo_restore {
            source.clone()
        } else {
            location.directory().join("info")
        };
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o500)).unwrap();
        let failure = engine.undo(&cancel);
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(failure.is_err());
        drop(engine);
        let engine = OperationEngine::open(&history).unwrap();
        if undo_restore {
            assert_eq!(engine.latest_undo_id(), restored_id);
            engine.undo(&cancel).unwrap();
            assert!(!original.exists());
            engine.undo(&cancel).unwrap();
        }
        assert_eq!(engine.latest_undo_id(), copied.id);
        assert_eq!(fs::read(&original).unwrap(), b"original copied document");
        engine.undo(&cancel).unwrap();
        assert!(!original.exists());
        assert_eq!(fs::read(&external).unwrap(), b"original copied document");
        assert_eq!(
            fs::read(older.join("report.txt")).unwrap(),
            b"original copied document"
        );
        assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    }
}
