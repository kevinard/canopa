//! Public same-filesystem Undo Replace deaths around original displacement.
use super::*;
use crate::transfers::{ConflictChoice, TransferReply};

pub(in crate::operations) fn cut(point: &str) {
    if std::env::var("CANOPA_DIRECT_UNDO_CUT").as_deref() != Ok(point) {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CANOPA_DIRECT_UNDO_ROOT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-direct-undo-")
    );
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let body: String = db
        .query_row("SELECT body FROM pending", [], |r| r.get(0))
        .unwrap();
    let pending: Pending = serde_json::from_str(&body).unwrap();
    assert!(matches!(
        &pending,
        Pending::Undo {
            change: Change::Rename { .. },
            stage: None,
            displaced: Some(_),
            ..
        }
    ));
    fs::write(root.join("pending.json"), body).unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private direct Undo worker was not killed");
}

#[test]
#[ignore = "private public same-filesystem Undo Replace worker killed at a durable boundary"]
fn direct_displacement_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_DIRECT_UNDO_ROOT").unwrap());
    for name in ["seed", "source", "output"] {
        fs::create_dir(root.join(name)).unwrap();
    }
    fs::write(root.join("seed/item"), b"Incoming Move").unwrap();
    fs::write(root.join("output/item"), b"Previous destination").unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let cancel = Cancellation::default();
    for (kind, from, to) in [
        (TransferKind::Copy, "seed/item", "source"),
        (TransferKind::Move, "source/item", "output"),
    ] {
        let outcome = engine
            .transfer(
                &TransferRequest {
                    kind,
                    sources: vec![root.join(from)],
                    destination: root.join(to),
                },
                &cancel,
                |_| TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
            )
            .unwrap();
        assert!(outcome.failures.is_empty());
        assert_eq!(outcome.completed, 1);
    }
    fs::write(root.join("source/item"), b"Newer source occupant").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Replace)
        .unwrap();
    panic!("direct displacement boundary was not reached");
}

struct Fixture(Option<tempfile::TempDir>);
impl Fixture {
    fn new(point: &str) -> Self {
        let fixture = Self(Some(
            tempfile::Builder::new()
                .prefix("canopa-direct-undo-")
                .tempdir()
                .unwrap(),
        ));
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "operations::undo::abort_boundary::direct_displacement_boundary::direct_displacement_child"])
            .env("CANOPA_DIRECT_UNDO_ROOT", fixture.path()).env("CANOPA_DIRECT_UNDO_CUT", point).status().unwrap();
        assert_eq!(status.signal(), Some(9));
        fixture
    }
    fn path(&self) -> &Path {
        self.0.as_ref().unwrap().path()
    }
    fn record(&self) -> recovery::Record {
        let pending: Pending =
            serde_json::from_slice(&fs::read(self.path().join("pending.json")).unwrap()).unwrap();
        let Pending::Undo {
            displaced: Some(record),
            ..
        } = pending
        else {
            panic!("direct displaced Undo");
        };
        *record
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking()
            && let Some(root) = self.0.take()
        {
            eprintln!("retained direct Undo fixture: {}", root.keep().display());
        }
    }
}
fn assert_bytes(root: &Path, path: &str, bytes: &[u8]) {
    assert_eq!(fs::read(root.join(path)).unwrap(), bytes);
}

#[test]
fn direct_displacement_before_and_after_rename_compensates_once_and_requires_another_replace_answer()
 {
    for point in ["registered", "displaced"] {
        let fixture = Fixture::new(point);
        let root = fixture.path();
        let record = fixture.record();
        for _ in 0..2 {
            let engine = OperationEngine::open(root.join("history")).unwrap();
            engine.retry_recovery(&Cancellation::default()).unwrap();
            assert_bytes(root, "source/item", b"Newer source occupant");
            assert_bytes(root, "output/item", b"Incoming Move");
            assert!(!record.retained.stage.directory.exists());
            assert!(engine.latest_undo_id().is_some());
        }
        let engine = OperationEngine::open(root.join("history")).unwrap();
        let mut questions = 0;
        engine
            .undo_interactive(None, &Cancellation::default(), |conflict| {
                questions += 1;
                assert!(conflict.can_replace());
                UndoChoice::Replace
            })
            .unwrap();
        assert_eq!(questions, 1);
        assert_bytes(root, "source/item", b"Incoming Move");
        assert_bytes(root, "output/item", b"Previous destination");
        assert!(
            engine
                .recovery_items(None, &Cancellation::default())
                .unwrap()
                .items
                .into_iter()
                .any(|item| {
                    item.entry.is_some_and(|e| {
                        fs::read(e.path).is_ok_and(|b| b == b"Newer source occupant")
                    })
                })
        );
    }
}

#[test]
fn direct_displacement_rejects_a_replacement_work_root_then_compensates_the_original() {
    let fixture = Fixture::new("displaced");
    let root = fixture.path();
    let record = fixture.record();
    let stage = &record.retained.stage.directory;
    fs::rename(stage, root.join("original-work")).unwrap();
    fs::create_dir(stage).unwrap();
    fs::write(record.retained.path(), b"Replacement root work").unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let cancel = Cancellation::default();
    assert!(engine.retry_recovery(&cancel).is_err());
    for item in engine.recovery_items(None, &cancel).unwrap().items {
        assert!(engine.keep_in_recovery(&item, &cancel).is_err());
    }
    assert_eq!(
        fs::read(record.retained.path()).unwrap(),
        b"Replacement root work"
    );
    assert!(!root.join("source/item").exists());
    fs::rename(stage, root.join("replacement-work")).unwrap();
    fs::rename(root.join("original-work"), stage).unwrap();
    engine.retry_recovery(&cancel).unwrap();
    assert_bytes(root, "source/item", b"Newer source occupant");
    assert_bytes(root, "output/item", b"Incoming Move");
    assert_bytes(root, "replacement-work/previous", b"Replacement root work");
}

#[test]
fn direct_displacement_requires_the_original_history_file_before_compensation() {
    let fixture = Fixture::new("displaced");
    let root = fixture.path();
    let history = root.join("history");
    let db = rusqlite::Connection::open(&history).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    drop(db);
    fs::rename(&history, root.join("original-history")).unwrap();
    fs::copy(root.join("original-history"), &history).unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    assert!(engine.retry_recovery(&Cancellation::default()).is_err());
    assert!(!root.join("source/item").exists());
    assert_eq!(
        fs::read(fixture.record().retained.path()).unwrap(),
        b"Newer source occupant"
    );
    drop(engine);
    fs::rename(&history, root.join("cloned-history")).unwrap();
    fs::rename(root.join("original-history"), &history).unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    engine.retry_recovery(&Cancellation::default()).unwrap();
    assert_bytes(root, "source/item", b"Newer source occupant");
    assert_bytes(root, "output/item", b"Incoming Move");
}

#[test]
fn direct_displacement_after_incoming_return_finishes_saved_work_without_adopting_a_new_source_occupant()
 {
    let fixture = Fixture::new("incoming-returned");
    let root = fixture.path();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine.retry_recovery(&cancel).unwrap();
    drop(engine);
    let metadata = fs::metadata(root.join("source/item")).unwrap();
    fs::rename(root.join("source/item"), root.join("original-returned")).unwrap();
    fs::write(root.join("source/item"), b"Incoming Move").unwrap();
    fs::File::open(root.join("source/item"))
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
        .unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let cancel = Cancellation::default();
    engine.undo(&cancel).unwrap(); // Only the saved previous destination remains.
    assert!(
        engine.undo(&cancel).is_err(),
        "older Copy must refuse a new occupant after the recorded Move return"
    );
    assert_bytes(root, "source/item", b"Incoming Move");
    assert_bytes(root, "original-returned", b"Incoming Move");
    assert_bytes(root, "output/item", b"Previous destination");
    assert_eq!(
        fs::read(fixture.record().retained.path()).unwrap(),
        b"Newer source occupant"
    );
}

#[test]
fn an_ignored_direct_binding_retirement_keeps_the_original_task_until_retry() {
    let fixture = Fixture::new("incoming-returned");
    let root = fixture.path();
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    db.execute_batch("CREATE TRIGGER refuse_retirement BEFORE DELETE ON undo_work BEGIN SELECT RAISE(IGNORE); END;").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    for _ in 0..2 {
        assert!(engine.retry_recovery(&cancel).is_err());
        let counts: (u64, u64) = db
            .query_row(
                "SELECT (SELECT count(*) FROM pending),(SELECT count(*) FROM undo_work)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 1));
        assert_bytes(root, "source/item", b"Incoming Move");
        assert!(!root.join("output/item").exists());
        assert_eq!(
            fs::read(fixture.record().retained.path()).unwrap(),
            b"Newer source occupant"
        );
    }
    db.execute_batch("DROP TRIGGER refuse_retirement;").unwrap();
    engine.retry_recovery(&cancel).unwrap();
    engine.undo(&cancel).unwrap();
    assert_bytes(root, "source/item", b"Incoming Move");
    assert_bytes(root, "output/item", b"Previous destination");
    assert_eq!(
        fs::read(fixture.record().retained.path()).unwrap(),
        b"Newer source occupant"
    );
}

#[test]
fn changed_direct_displaced_work_can_be_copied_and_freshly_kept_before_another_undo() {
    let fixture = Fixture::new("displaced");
    let root = fixture.path();
    let record = fixture.record();
    let stage = &record.retained.stage.directory;
    fs::write(stage.join("extra"), b"Unexpected work").unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let cancel = Cancellation::default();
    assert!(engine.retry_recovery(&cancel).is_err());
    let read = || {
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.entry.as_ref().is_some_and(|e| e.path == *stage))
            .unwrap()
    };
    let stale = read();
    engine
        .recover_copy(&stale, &root.join("export"), &cancel)
        .unwrap();
    assert_bytes(root, "export/previous", b"Newer source occupant");
    assert_bytes(root, "export/extra", b"Unexpected work");
    fs::write(stage.join("extra-2"), b"Later work").unwrap();
    assert!(engine.keep_in_recovery(&stale, &cancel).is_err());
    let fresh = read();
    assert!(fresh.can_keep_in_recovery());
    let serialized: RecoveryItem =
        serde_json::from_slice(&serde_json::to_vec(&fresh).unwrap()).unwrap();
    assert!(engine.keep_in_recovery(&serialized, &cancel).is_err());
    engine.keep_in_recovery(&fresh, &cancel).unwrap();
    drop(engine);
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine.undo(&cancel).unwrap(); // Export copy.
    engine.undo(&cancel).unwrap(); // Fresh incoming and saved returns.
    assert_bytes(root, "source/item", b"Incoming Move");
    assert_bytes(root, "output/item", b"Previous destination");
    assert_eq!(
        fs::read(record.retained.path()).unwrap(),
        b"Newer source occupant"
    );
    assert_eq!(fs::read(stage.join("extra-2")).unwrap(), b"Later work");
}
