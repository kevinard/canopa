//! Actual public Move Undo death after the original saved entry was returned.
use super::*;
use crate::transfers::{ConflictChoice, TransferReply};

pub(in crate::operations) fn cut(point: &str, change: &Change) {
    if std::env::var("CANOPA_MOVE_RETURN_CUT").as_deref() != Ok(point) {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CANOPA_MOVE_RETURN_ROOT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-move-return-")
    );
    assert!(matches!(
        change,
        Change::Rename {
            retained: Some(_),
            ..
        }
    ));
    assert_eq!(change.forward_path(), root.join("output/item"));
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        b"Incoming Move content"
    );
    if matches!(point, "prepared" | "returning" | "refused") {
        assert!(!root.join("output/item").exists());
        assert_eq!(
            fs::read(change.retained().unwrap().path()).unwrap(),
            b"Previous destination"
        );
    } else {
        assert_eq!(
            fs::read(root.join("output/item")).unwrap(),
            b"Previous destination"
        );
        assert!(!change.retained().unwrap().path().exists());
    }
    fs::write(
        root.join("move-return-change.json"),
        serde_json::to_vec(change).unwrap(),
    )
    .unwrap();
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let pending: String = db
        .query_row("SELECT body FROM pending", [], |r| r.get(0))
        .unwrap();
    fs::write(root.join("move-return-pending.json"), pending).unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private saved Move Undo worker was not killed");
}

#[test]
#[ignore = "private public Move Undo worker killed after the actual saved return"]
fn move_return_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_MOVE_RETURN_ROOT").unwrap());
    fs::create_dir(root.join("source")).unwrap();
    fs::create_dir(root.join("output")).unwrap();
    fs::write(root.join("source/item"), b"Incoming Move content").unwrap();
    fs::write(root.join("output/item"), b"Previous destination").unwrap();
    fs::hard_link(root.join("output/item"), root.join("previous-alias")).unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let cancellation = Cancellation::default();
    if std::env::var("CANOPA_MOVE_RETURN_CUT").as_deref() == Ok("returned") {
        fs::create_dir(root.join("seed")).unwrap();
        fs::write(root.join("seed/item"), b"Incoming Move content").unwrap();
        fs::remove_file(root.join("source/item")).unwrap();
        let copied = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![root.join("seed/item")],
                    destination: root.join("source"),
                },
                &cancellation,
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(copied.failures.is_empty());
    }
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![root.join("source/item")],
                destination: root.join("output"),
            },
            &cancellation,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    assert_eq!(moved.completed, 1);
    if std::env::var("CANOPA_MOVE_RETURN_CUT").as_deref() == Ok("refused") {
        use std::os::unix::fs::PermissionsExt;
        let saved = engine
            .recovery_items(None, &cancellation)
            .unwrap()
            .items
            .into_iter()
            .find_map(|item| item.entry)
            .unwrap()
            .path;
        fs::set_permissions(saved.parent().unwrap(), fs::Permissions::from_mode(0o500)).unwrap();
        if std::env::var_os("CANOPA_MOVE_RETURN_REFUSAL_WRITE").is_some() {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            db.execute_batch("CREATE TRIGGER ignore_refusal BEFORE UPDATE ON pending WHEN json_extract(new.body,'$.returning.phase')='refused' BEGIN SELECT RAISE(IGNORE); END;").unwrap();
        }
    }
    engine.undo(&cancellation).unwrap();
    panic!("Move Undo did not reach the requested death boundary");
}

struct Fixture(Option<tempfile::TempDir>);
impl Fixture {
    fn new() -> Self {
        Self(Some(
            tempfile::Builder::new()
                .prefix("canopa-move-return-")
                .tempdir()
                .unwrap(),
        ))
    }
    fn path(&self) -> &Path {
        self.0.as_ref().unwrap().path()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking()
            && let Some(root) = self.0.take()
        {
            eprintln!("retained Move return fixture: {}", root.keep().display());
        }
    }
}
fn completed_return(reintroduce: bool) {
    let fixture = Fixture::new();
    let root = fixture.path();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "operations::undo::abort_boundary::move_return_boundary::move_return_child",
        ])
        .env("CANOPA_MOVE_RETURN_CUT", "previous-renamed")
        .env("CANOPA_MOVE_RETURN_ROOT", root)
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let change: Change =
        serde_json::from_slice(&fs::read(root.join("move-return-change.json")).unwrap()).unwrap();
    let saved = change.retained().unwrap();
    if reintroduce {
        fs::hard_link(root.join("previous-alias"), saved.path()).unwrap();
    }
    fs::write(
        root.join("output/item"),
        b"Later edits to the completed saved return",
    )
    .unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let cancellation = Cancellation::default();
    let _reconciled = engine.retry_recovery(&cancellation);
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        b"Incoming Move content"
    );
    assert_eq!(
        fs::read(root.join("output/item")).unwrap(),
        b"Later edits to the completed saved return"
    );
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let remaining: u64 = db
        .query_row("SELECT count(*) FROM changes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        remaining, 0,
        "the completed return must retire without granting the reused saved name another return"
    );
    assert!(engine.latest_undo_id().is_none());
    if reintroduce {
        assert_eq!(
            fs::read(saved.path()).unwrap(),
            b"Later edits to the completed saved return"
        );
        let item = engine
            .recovery_items(None, &cancellation)
            .unwrap()
            .items
            .into_iter()
            .find(|item| {
                item.entry
                    .as_ref()
                    .is_some_and(|e| e.path == saved.stage.directory)
            })
            .expect("empty-only cleanup exposes the original nonempty root for fresh Keep");
        engine.keep_in_recovery(&item, &cancellation).unwrap();
        drop(engine);
        let engine = OperationEngine::open(root.join("history")).unwrap();
        assert!(
            engine
                .recovery_items(None, &cancellation)
                .unwrap()
                .interrupted
                .is_empty()
        );
        assert_eq!(
            fs::read(saved.path()).unwrap(),
            b"Later edits to the completed saved return"
        );
    }
}
#[test]
fn a_completed_saved_return_keeps_later_edits_without_reintroducing_its_old_name() {
    completed_return(false);
}
#[test]
fn a_completed_saved_return_retires_once_when_its_old_name_is_reintroduced() {
    completed_return(true);
}

fn interrupted(point: &str) -> (Fixture, Change) {
    let fixture = Fixture::new();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "operations::undo::abort_boundary::move_return_boundary::move_return_child",
        ])
        .env("CANOPA_MOVE_RETURN_CUT", point)
        .env("CANOPA_MOVE_RETURN_ROOT", fixture.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let change =
        serde_json::from_slice(&fs::read(fixture.path().join("move-return-change.json")).unwrap())
            .unwrap();
    (fixture, change)
}
#[test]
fn a_consumed_return_without_completion_never_replays_its_name_and_fresh_keep_preserves_edits() {
    let (fixture, change) = interrupted("returning");
    let root = fixture.path();
    let saved = change.retained().unwrap();
    fs::write(saved.path(), b"Later edit in consumed saved name").unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let cancel = Cancellation::default();
    assert!(engine.retry_recovery(&cancel).is_err());
    assert!(!root.join("output/item").exists());
    let read = || {
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .into_iter()
            .find(|i| {
                i.entry
                    .as_ref()
                    .is_some_and(|e| e.path == saved.stage.directory)
            })
            .unwrap()
    };
    let stale = read();
    fs::write(
        saved.stage.directory.join("new-child"),
        b"Later unexpected work",
    )
    .unwrap();
    assert!(engine.keep_in_recovery(&stale, &cancel).is_err());
    let fresh = read();
    assert!(fresh.can_keep_in_recovery());
    let replay: RecoveryItem =
        serde_json::from_slice(&serde_json::to_vec(&fresh).unwrap()).unwrap();
    assert!(
        !replay.can_keep_in_recovery(),
        "a serialized Recovery item cannot recreate a live Keep review"
    );
    assert!(engine.keep_in_recovery(&replay, &cancel).is_err());
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert!(engine.keep_in_recovery(&fresh, &cancelled).is_err());
    engine.keep_in_recovery(&fresh, &cancel).unwrap();
    drop(engine);
    let engine = OperationEngine::open(root.join("history")).unwrap();
    assert!(engine.latest_undo_id().is_none());
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .interrupted
            .is_empty()
    );
    assert_eq!(
        fs::read(saved.path()).unwrap(),
        b"Later edit in consumed saved name"
    );
    assert_eq!(
        fs::read(saved.stage.directory.join("new-child")).unwrap(),
        b"Later unexpected work"
    );
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        b"Incoming Move content"
    );
    assert!(!root.join("output/item").exists());
}
#[test]
fn a_replacement_saved_root_is_never_adopted_by_retry_or_keep() {
    let (fixture, change) = interrupted("returning");
    let root = fixture.path();
    let saved = change.retained().unwrap();
    let parked = root.join("original-saved-root");
    fs::rename(&saved.stage.directory, &parked).unwrap();
    fs::create_dir(&saved.stage.directory).unwrap();
    fs::write(saved.path(), b"Replacement root work").unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let cancel = Cancellation::default();
    assert!(engine.retry_recovery(&cancel).is_err());
    for item in engine.recovery_items(None, &cancel).unwrap().items {
        assert!(engine.keep_in_recovery(&item, &cancel).is_err());
    }
    assert_eq!(fs::read(saved.path()).unwrap(), b"Replacement root work");
    assert!(!root.join("output/item").exists());
    let replacement = root.join("replacement-saved-root");
    fs::rename(&saved.stage.directory, &replacement).unwrap();
    fs::rename(&parked, &saved.stage.directory).unwrap();
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .into_iter()
        .find(|i| {
            i.entry
                .as_ref()
                .is_some_and(|e| e.path == saved.stage.directory)
        })
        .unwrap();
    engine.keep_in_recovery(&item, &cancel).unwrap();
    assert_eq!(
        fs::read(replacement.join("previous")).unwrap(),
        b"Replacement root work"
    );
    assert_eq!(fs::read(saved.path()).unwrap(), b"Previous destination");
}
#[test]
fn an_original_prepared_return_refuses_cloned_history_then_resumes_when_the_original_returns() {
    let (fixture, _change) = interrupted("prepared");
    let root = fixture.path();
    let history = root.join("history");
    let db = rusqlite::Connection::open(&history).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    drop(db);
    let original = root.join("original-history");
    fs::rename(&history, &original).unwrap();
    fs::copy(&original, &history).unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    assert!(engine.retry_recovery(&Cancellation::default()).is_err());
    assert!(!root.join("output/item").exists());
    drop(engine);
    fs::rename(&history, root.join("cloned-history")).unwrap();
    fs::rename(&original, &history).unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    engine.retry_recovery(&Cancellation::default()).unwrap();
    assert!(engine.latest_undo_id().is_none());
    assert_eq!(
        fs::read(root.join("output/item")).unwrap(),
        b"Previous destination"
    );
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        b"Incoming Move content"
    );
}

#[test]
fn completing_a_recorded_return_never_adopts_a_new_occupant_for_older_copy_undo() {
    let (fixture, _change) = interrupted("returned");
    let root = fixture.path();
    let metadata = fs::metadata(root.join("source/item")).unwrap();
    fs::rename(
        root.join("source/item"),
        root.join("returned-incoming-elsewhere"),
    )
    .unwrap();
    fs::write(root.join("source/item"), b"Incoming Move content").unwrap();
    fs::File::open(root.join("source/item"))
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
        .unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine.retry_recovery(&Cancellation::default()).unwrap();
    assert!(
        engine.undo(&Cancellation::default()).is_err(),
        "recorded completion must retain the returned original's identity instead of lending older Copy Undo ownership of a replacement"
    );
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        b"Incoming Move content"
    );
    assert_eq!(
        fs::read(root.join("returned-incoming-elsewhere")).unwrap(),
        b"Incoming Move content"
    );
    assert_eq!(
        fs::read(root.join("output/item")).unwrap(),
        b"Previous destination"
    );
}

#[test]
fn saved_return_accepts_canonical_formatting_changes_but_refuses_changed_history() {
    for tamper in [false, true] {
        let (fixture, _) = interrupted("prepared");
        let root = fixture.path();
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        let (id, ordinal, body): (u64, u64, String) = db
            .query_row(
                "SELECT operation,ordinal,body FROM changes ORDER BY operation DESC,ordinal DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        let mut change: serde_json::Value = serde_json::from_str(&body).unwrap();
        if tamper {
            change["original"] = serde_json::to_value(root.join("different-source")).unwrap();
        }
        db.execute(
            "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
            rusqlite::params![id, ordinal, serde_json::to_string_pretty(&change).unwrap()],
        )
        .unwrap();
        let engine = OperationEngine::open(root.join("history")).unwrap();
        let result = engine.retry_recovery(&Cancellation::default());
        assert_eq!(result.is_err(), tamper);
        assert_eq!(
            fs::read(root.join("source/item")).unwrap(),
            b"Incoming Move content"
        );
        if tamper {
            assert!(!root.join("output/item").exists());
            db.execute(
                "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                rusqlite::params![id, ordinal, body],
            )
            .unwrap();
            engine.retry_recovery(&Cancellation::default()).unwrap();
        }
        assert_eq!(
            fs::read(root.join("output/item")).unwrap(),
            b"Previous destination"
        );
        assert!(engine.latest_undo_id().is_none());
    }
}

#[test]
fn a_proved_saved_return_refusal_resumes_but_an_ignored_refusal_write_does_not_rearm_the_name() {
    use std::os::unix::fs::PermissionsExt;
    if rustix::process::geteuid().is_root() {
        return;
    }
    for ignored in [false, true] {
        let fixture = Fixture::new();
        let root = fixture.path();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "operations::undo::abort_boundary::move_return_boundary::move_return_child",
            ])
            .env("CANOPA_MOVE_RETURN_CUT", "refused")
            .env("CANOPA_MOVE_RETURN_ROOT", root);
        if ignored {
            command.env("CANOPA_MOVE_RETURN_REFUSAL_WRITE", "1");
        }
        let status = command.status().unwrap();
        assert_eq!(status.signal(), Some(9));
        let pending: Pending =
            serde_json::from_slice(&fs::read(root.join("move-return-pending.json")).unwrap())
                .unwrap();
        let Pending::UndoSavedReturn { returning } = pending else {
            panic!("saved return");
        };
        assert_eq!(
            returning.phase,
            if ignored {
                saved_return::Phase::Returning
            } else {
                saved_return::Phase::Refused
            }
        );
        fs::set_permissions(
            &returning.stage.directory,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let engine = OperationEngine::open(root.join("history")).unwrap();
        let cancel = Cancellation::default();
        if ignored {
            assert!(engine.retry_recovery(&cancel).is_err());
            assert!(!root.join("output/item").exists());
            let item = engine
                .recovery_items(None, &cancel)
                .unwrap()
                .items
                .into_iter()
                .find(|item| {
                    item.entry
                        .as_ref()
                        .is_some_and(|e| e.path == returning.stage.directory)
                })
                .unwrap();
            engine.keep_in_recovery(&item, &cancel).unwrap();
            assert_eq!(
                fs::read(returning.change().retained().unwrap().path()).unwrap(),
                b"Previous destination"
            );
        } else {
            engine.retry_recovery(&cancel).unwrap();
            assert_eq!(
                fs::read(root.join("output/item")).unwrap(),
                b"Previous destination"
            );
        }
        assert_eq!(
            fs::read(root.join("source/item")).unwrap(),
            b"Incoming Move content"
        );
        assert!(engine.latest_undo_id().is_none());
    }
}
#[test]
fn a_proved_refusal_cannot_resume_from_a_replacement_saved_entry() {
    use std::os::unix::fs::PermissionsExt;
    if rustix::process::geteuid().is_root() {
        return;
    }
    let (fixture, change) = interrupted("refused");
    let root = fixture.path();
    let retained = change.retained().unwrap();
    fs::set_permissions(&retained.stage.directory, fs::Permissions::from_mode(0o700)).unwrap();
    fs::rename(
        retained.path(),
        retained.stage.directory.join("selected-original"),
    )
    .unwrap();
    fs::write(retained.path(), b"Replacement saved work").unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let cancel = Cancellation::default();
    assert!(engine.retry_recovery(&cancel).is_err());
    assert!(!root.join("output/item").exists());
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .into_iter()
        .find(|i| {
            i.entry
                .as_ref()
                .is_some_and(|e| e.path == retained.stage.directory)
        })
        .unwrap();
    engine.keep_in_recovery(&item, &cancel).unwrap();
    assert_eq!(
        fs::read(retained.path()).unwrap(),
        b"Replacement saved work"
    );
    assert_eq!(
        fs::read(retained.stage.directory.join("selected-original")).unwrap(),
        b"Previous destination"
    );
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        b"Incoming Move content"
    );
}

#[test]
fn a_retained_source_lease_defers_proved_saved_refusal_continuation_until_release() {
    use std::os::unix::fs::PermissionsExt;
    if rustix::process::geteuid().is_root() {
        return;
    }
    let (fixture, change) = interrupted("refused");
    let root = fixture.path();
    let retained = change.retained().unwrap();
    // Open while permission still refuses; the actual saved return remains pending.
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let cancel = Cancellation::default();
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&retained.stage.directory))
        .unwrap();
    let lease = engine.lease_retained_sources(&captured).unwrap();
    fs::set_permissions(&retained.stage.directory, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(engine.retry_recovery(&cancel).is_err());
    assert!(!root.join("output/item").exists());
    assert_eq!(fs::read(retained.path()).unwrap(), b"Previous destination");
    lease.release().unwrap();
    engine.retry_recovery(&cancel).unwrap();
    assert_eq!(
        fs::read(root.join("output/item")).unwrap(),
        b"Previous destination"
    );
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        b"Incoming Move content"
    );
    assert!(engine.latest_undo_id().is_none());
}
