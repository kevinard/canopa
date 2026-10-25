use canopa_files::{
    Cancellation,
    operations::{OperationEngine, RecoveryStatus},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::{collections::BTreeSet, fs, path::PathBuf};

struct Fixture {
    _root: tempfile::TempDir,
    history: PathBuf,
    current: PathBuf,
    saved: PathBuf,
}

impl Fixture {
    fn shared_input(archived: &[bool], active: bool, legacy: bool) -> Self {
        Self::build(archived, active, legacy, false)
    }
    fn build(archived: &[bool], active: bool, legacy: bool, alias: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        let transfer_destination = if alias {
            let alias = root.path().join("destination alias");
            std::os::unix::fs::symlink(&destination, &alias).unwrap();
            alias
        } else {
            destination.clone()
        };
        let current = destination.join("note.txt");
        fs::write(source.join("note.txt"), b"incoming work").unwrap();
        fs::write(&current, b"older saved work").unwrap();
        let history = root.path().join("state/history");
        let engine = OperationEngine::open(&history).unwrap();
        let cancel = Cancellation::default();
        let copied = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![source.join("note.txt")],
                    destination: transfer_destination,
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
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].status, RecoveryStatus::UndoBackup);
        let saved = page.items[0]
            .entry
            .as_ref()
            .unwrap()
            .path
            .canonicalize()
            .unwrap();
        drop(engine);
        // Prepare older shared-owner input. Its wire/schema permits several
        // records to retain the same container. No DB output is used to assert
        // cleanup: all observations below cross public Recovery/Undo seams.
        {
            let input = rusqlite::Connection::open(&history).unwrap();
            input.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
            let body: String = input
                .query_row(
                    "SELECT body FROM changes ORDER BY operation,ordinal LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            let change: serde_json::Value = serde_json::from_str(&body).unwrap();
            for protected in archived {
                let mut retained = change["retained"].clone();
                retained["recovery"] = serde_json::json!(protected);
                retained["stage"]["directory"] = serde_json::json!(saved.parent().unwrap());
                let stage = retained["stage"]["identity"].to_string();
                let record = serde_json::json!({
                    "retained": retained,
                    "context": {"original": current, "label": "Expired shared owner"},
                });
                input
                    .execute("INSERT INTO garbage(body) VALUES(?1)", [record.to_string()])
                    .unwrap();
                let entry = input.last_insert_rowid();
                input
                    .execute(
                        "INSERT INTO saved_stage_records VALUES(?1,?2)",
                        rusqlite::params![stage, entry],
                    )
                    .unwrap();
                let paths: BTreeSet<_> = [current.as_path(), saved.parent().unwrap()]
                    .into_iter()
                    .flat_map(|path| path.ancestors().map(|path| path.to_owned()))
                    .collect();
                for path in paths {
                    input
                        .execute(
                            "INSERT INTO garbage_paths VALUES(?1,?2)",
                            rusqlite::params![path.as_os_str().as_encoded_bytes(), entry],
                        )
                        .unwrap();
                }
            }
            if legacy {
                input.execute_batch("DROP TRIGGER recovery_context_deleted; DROP TABLE recovery_context_cleanup; DROP TABLE recovery_context_steps; DROP TABLE namespace_moves; DROP INDEX identity_refs_expected; PRAGMA user_version=15;").unwrap();
            }
            if !active {
                input.execute("DELETE FROM operations", []).unwrap();
            }
        }
        Self {
            _root: root,
            history,
            current,
            saved,
        }
    }
}

#[test]
fn expiry_of_an_archived_owner_does_not_remove_the_same_saved_file_from_active_undo() {
    for legacy in [false, true] {
        let fixture = Fixture::shared_input(&[false], true, legacy);
        let cancel = Cancellation::default();
        let engine = OperationEngine::open(&fixture.history).unwrap();
        assert_eq!(
            fs::read(&fixture.saved).unwrap(),
            b"older saved work",
            "actual shared bytes must survive in both current and older history"
        );
        let page = engine.recovery_items(None, &cancel).unwrap();
        let active = page.items.iter().find(|item| item.held_for_undo).unwrap();
        assert_eq!(
            active.status,
            RecoveryStatus::UndoBackup,
            "an archived owner cannot expire a live Undo backup"
        );
        assert_eq!(fs::read(&fixture.saved).unwrap(), b"older saved work");
        assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming work");
        engine.undo(&cancel).unwrap();
        assert_eq!(fs::read(&fixture.current).unwrap(), b"older saved work");
        assert!(!fixture.saved.exists());
        assert!(
            engine
                .recovery_items(None, &cancel)
                .unwrap()
                .items
                .is_empty()
        );
    }
}

#[test]
fn an_unprotected_archive_cannot_expire_a_protected_archives_shared_bytes() {
    let fixture = Fixture::shared_input(&[false, true], false, true);
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&fixture.history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].status, RecoveryStatus::NeedsRecovery);
    assert_eq!(fs::read(&fixture.saved).unwrap(), b"older saved work");
    let recovered = fixture.current.with_file_name("Recovered note.txt");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"older saved work");
    assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming work");
    engine.undo(&cancel).unwrap();
    assert!(!recovered.exists());
    assert_eq!(fs::read(&fixture.saved).unwrap(), b"older saved work");
}

#[test]
fn unchanged_unprotected_archived_owners_expire_once_without_unavailable_catalog_remnants() {
    let fixture = Fixture::shared_input(&[false, false], false, true);
    let engine = OperationEngine::open(&fixture.history).unwrap();
    assert!(
        engine
            .recovery_items(None, &Cancellation::default())
            .unwrap()
            .items
            .is_empty()
    );
    assert!(!fixture.saved.parent().unwrap().exists());
    assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming work");
}

#[test]
fn shared_archives_with_newer_saved_edits_keep_their_records_and_remain_copyable() {
    let fixture = Fixture::shared_input(&[false, false], false, false);
    fs::write(
        &fixture.saved,
        b"newer shared edits requiring explicit recovery",
    )
    .unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&fixture.history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 2);
    assert!(
        page.items
            .iter()
            .all(|item| item.status == RecoveryStatus::NeedsRecovery)
    );
    let recovered = fixture.current.with_file_name("Recovered edits.txt");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(
        fs::read(&recovered).unwrap(),
        b"newer shared edits requiring explicit recovery"
    );
    assert_eq!(
        fs::read(&fixture.saved).unwrap(),
        b"newer shared edits requiring explicit recovery"
    );
    drop(engine);
    let engine = OperationEngine::open(&fixture.history).unwrap();
    assert_eq!(engine.recovery_items(None, &cancel).unwrap().items.len(), 2);
    assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming work");
}

#[test]
fn additional_work_in_a_saved_folder_preserves_its_previous_file_and_recovery_record() {
    let fixture = Fixture::shared_input(&[false], false, true);
    let additional = fixture.saved.with_file_name("additional work.txt");
    fs::write(&additional, b"additional user work").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&fixture.history).unwrap();
    assert_eq!(fs::read(&fixture.saved).unwrap(), b"older saved work");
    assert_eq!(fs::read(&additional).unwrap(), b"additional user work");
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].status, RecoveryStatus::NeedsRecovery);
    let recovered = fixture.current.with_file_name("Recovered previous.txt");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"older saved work");
    assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming work");
}

#[test]
fn an_active_owner_through_a_parent_alias_still_protects_the_actual_saved_container() {
    let fixture = Fixture::build(&[false], true, true, true);
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&fixture.history).unwrap();
    assert_eq!(fs::read(&fixture.saved).unwrap(), b"older saved work");
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].status, RecoveryStatus::UndoBackup);
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&fixture.current).unwrap(), b"older saved work");
}

#[test]
fn an_unavailable_owner_alias_keeps_the_other_recovery_record_and_its_saved_bytes() {
    let fixture = Fixture::build(&[false], true, false, true);
    let alias = fixture._root.path().join("destination alias");
    fs::remove_file(&alias).unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&fixture.history).unwrap();
    assert_eq!(fs::read(&fixture.saved).unwrap(), b"older saved work");
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 2);
    assert!(
        page.items
            .iter()
            .any(|item| item.held_for_undo && item.status == RecoveryStatus::Unavailable)
    );
    let available = page.items.iter().find(|item| !item.held_for_undo).unwrap();
    assert_eq!(available.status, RecoveryStatus::NeedsRecovery);
    let recovered = fixture
        .current
        .with_file_name("Recovered while alias missing.txt");
    engine.recover_copy(available, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"older saved work");
    drop(engine);
    std::os::unix::fs::symlink(fixture.current.parent().unwrap(), &alias).unwrap();
    let engine = OperationEngine::open(&fixture.history).unwrap();
    engine.undo(&cancel).unwrap();
    assert!(!recovered.exists());
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&fixture.current).unwrap(), b"older saved work");
}
