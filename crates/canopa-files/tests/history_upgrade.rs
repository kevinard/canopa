use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, UndoChoice, UndoSource},
};

#[test]
fn v4_through_v16_history_upgrade_without_losing_renames_or_later_reviewed_identity_bindings() {
    for version in [4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16] {
        let root = tempfile::tempdir().unwrap();
        let before = root.path().join("before.txt");
        let original = root.path().join("original.txt");
        let current = root.path().join("current.txt");
        let external = root.path().join("original item elsewhere.txt");
        fs::write(&before, b"original contents").unwrap();
        let history = root.path().join("state/history");
        let cancel = Cancellation::default();
        let engine = OperationEngine::open(&history).unwrap();
        let older = engine
            .execute(
                OperationRequest::Rename {
                    source: before.clone(),
                    destination: original.clone(),
                },
                &cancel,
            )
            .unwrap();
        let moved = engine
            .execute(
                OperationRequest::Rename {
                    source: original.clone(),
                    destination: current.clone(),
                },
                &cancel,
            )
            .unwrap();
        drop(engine);
        // Remove v7 ownership context and, for v4/v5, v6 provenance. Ordinary
        // Rename wire records and the derived indexes retain their earlier format.
        // Observe the migration only through public history and actual files.
        {
            let input = rusqlite::Connection::open(&history).unwrap();
            prepare_directory_schema(&input, version);
            if version < 7 {
                input.execute_batch("ALTER TABLE changes DROP COLUMN bindings; DROP TABLE pending_bindings; DROP TABLE identity_refs;").unwrap();
            }
            if version < 6 {
                input.execute_batch("ALTER TABLE changes DROP COLUMN binding_revision; ALTER TABLE bindings DROP COLUMN revision; DELETE FROM metadata WHERE key='binding-revision';").unwrap();
            }
            input.pragma_update(None, "user_version", version).unwrap();
        }
        let engine = OperationEngine::open(&history).unwrap();
        assert_eq!(engine.latest_undo_id(), Some(moved.id));
        fs::rename(&current, &external).unwrap();
        fs::write(&current, b"reviewed replacement").unwrap();
        engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.source, UndoSource::ChangedMove);
                UndoChoice::ReturnItem
            })
            .unwrap();
        drop(engine);
        let engine = OperationEngine::open(&history).unwrap();
        assert_eq!(engine.latest_undo_id(), Some(older.id));
        engine.undo(&cancel).unwrap();
        assert_eq!(fs::read(&before).unwrap(), b"reviewed replacement");
        assert_eq!(fs::read(&external).unwrap(), b"original contents");
    }
}

#[test]
fn unsupported_future_history_is_refused_without_rewriting_its_file() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("history");
    {
        let input = rusqlite::Connection::open(&history).unwrap();
        input
            .execute_batch("PRAGMA user_version=2147483647;")
            .unwrap();
    }
    let original_bytes = fs::read(&history).unwrap();
    let error = OperationEngine::open(&history)
        .err()
        .expect("future history must be refused");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(fs::read(&history).unwrap(), original_bytes);
}

fn prepare_directory_schema(input: &rusqlite::Connection, version: u32) {
    if version < 16 {
        input.execute_batch("DROP TRIGGER recovery_context_deleted; DROP TABLE recovery_context_cleanup; DROP TABLE recovery_context_steps; DROP TABLE namespace_moves; DROP INDEX identity_refs_expected;").unwrap();
    }
    if version < 14 {
        input
            .execute_batch("DROP TABLE saved_stage_changes; DROP TABLE saved_stage_records;")
            .unwrap();
    }
    if version < 13 {
        input.execute_batch("DROP TABLE moved_directory_roots;
            CREATE TABLE directory_members_v12 (
                operation INTEGER NOT NULL, ordinal INTEGER NOT NULL, parent INTEGER NOT NULL,
                scope INTEGER REFERENCES directory_scopes(id),
                CHECK(ordinal>parent), PRIMARY KEY(operation,ordinal),
                FOREIGN KEY(operation,ordinal) REFERENCES changes(operation,ordinal) ON DELETE CASCADE,
                FOREIGN KEY(operation,parent) REFERENCES directory_returns(operation,ordinal) ON DELETE CASCADE);
            INSERT INTO directory_members_v12 SELECT operation,ordinal,parent,scope FROM directory_members;
            DROP TABLE directory_members;
            ALTER TABLE directory_members_v12 RENAME TO directory_members;
            CREATE INDEX directory_members_parent ON directory_members(operation,parent);").unwrap();
    }
    if version >= 11 {
        return;
    }
    input
        .execute_batch("DROP TABLE pending_saved_destinations;")
        .unwrap();
    if version >= 10 {
        return;
    }
    if version < 9 {
        input.execute_batch("DROP TABLE directory_archives; DROP TABLE directory_members; DROP TABLE directory_returns;").unwrap();
    } else {
        input.execute_batch("CREATE TABLE directory_archives_v9 (
            operation INTEGER NOT NULL, parent INTEGER NOT NULL,
            entry INTEGER NOT NULL REFERENCES garbage(id) ON DELETE CASCADE,
            PRIMARY KEY(operation,parent,entry),
            FOREIGN KEY(operation,parent) REFERENCES directory_returns(operation,ordinal) ON DELETE CASCADE);
            INSERT INTO directory_archives_v9 SELECT operation,parent,entry FROM directory_archives;
            DROP TABLE directory_archives;
            ALTER TABLE directory_archives_v9 RENAME TO directory_archives;
            ALTER TABLE directory_returns DROP COLUMN scope;
            ALTER TABLE directory_members DROP COLUMN scope;").unwrap();
    }
    input
        .execute_batch("DROP TABLE directory_scopes; DROP TABLE directory_sequences;")
        .unwrap();
}

#[test]
fn pending_v9_flat_and_nested_merges_resume_with_separate_additional_item_ownership() {
    use canopa_files::transfers::{TransferKind, TransferReply, TransferRequest};
    for nested in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("project");
        let current = root.path().join("moved");
        let relative = if nested {
            Path::new("nested")
        } else {
            Path::new("")
        };
        let old_folder = original.join(relative);
        let moved_folder = current.join(relative);
        fs::create_dir_all(&old_folder).unwrap();
        let sources = root.path().join("sources");
        fs::create_dir(&sources).unwrap();
        fs::write(sources.join("a.txt"), b"original copied data").unwrap();
        let history = root.path().join("state/history");
        let engine = OperationEngine::open(&history).unwrap();
        let cancel = Cancellation::default();
        let copy = || {
            let outcome = engine
                .transfer(
                    &TransferRequest {
                        kind: TransferKind::Copy,
                        sources: vec![sources.join("a.txt")],
                        destination: old_folder.clone(),
                    },
                    &cancel,
                    |_| TransferReply::Continue,
                )
                .unwrap();
            assert!(outcome.failures.is_empty());
        };
        copy();
        fs::write(old_folder.join("a.txt"), b"protected original version").unwrap();
        engine
            .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
            .unwrap();
        copy();
        fs::write(old_folder.join("z.txt"), b"original z").unwrap();
        engine
            .execute(
                OperationRequest::Rename {
                    source: original.clone(),
                    destination: current.clone(),
                },
                &cancel,
            )
            .unwrap();
        fs::create_dir_all(&old_folder).unwrap();
        fs::write(old_folder.join("z.txt"), b"newer z").unwrap();
        engine
            .undo_interactive(None, &cancel, |question| {
                if question.can_merge() {
                    UndoChoice::Merge
                } else {
                    fs::write(moved_folder.join("a.txt"), b"additional file").unwrap();
                    UndoChoice::Skip
                }
            })
            .unwrap();
        drop(engine);
        {
            let input = rusqlite::Connection::open(&history).unwrap();
            prepare_directory_schema(&input, 9);
            input.pragma_update(None, "user_version", 9).unwrap();
        }
        let engine = OperationEngine::open(&history).unwrap();
        let result = engine
            .undo_interactive(None, &cancel, |question| {
                if question.can_merge() {
                    UndoChoice::Merge
                } else {
                    UndoChoice::KeepBoth
                }
            })
            .unwrap();
        assert_eq!(result.remaining, 0);
        let page = engine.recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].original, Some(old_folder.join("a.txt")));
        assert_eq!(
            fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
            b"protected original version"
        );
        engine.undo(&cancel).unwrap();
        assert!(!old_folder.join("a.txt").exists());
        assert_eq!(
            fs::read(old_folder.join("a (2).txt")).unwrap(),
            b"additional file"
        );
        assert_eq!(fs::read(old_folder.join("z.txt")).unwrap(), b"newer z");
    }
}
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::{ffi::OsStringExt, fs::MetadataExt},
    path::{Path, PathBuf},
};

fn identity(path: &Path) -> Value {
    let metadata = fs::symlink_metadata(path).unwrap();
    json!({"device":metadata.dev(), "inode":metadata.ino()})
}
fn stored_path(path: &Path) -> Value {
    json!({"unix_bytes":path.as_os_str().as_encoded_bytes()})
}

#[test]
fn upgrading_old_history_preserves_raw_namespace_bindings_and_archived_recovery() {
    let root = tempfile::tempdir().unwrap();
    let original = root
        .path()
        .join(std::ffi::OsString::from_vec(b"folder-\xff".to_vec()));
    let current = root.path().join("moved");
    fs::create_dir(&original).unwrap();
    let child = original.join("child");
    fs::create_dir(&child).unwrap();
    let previous_identity = identity(&child);
    fs::rename(&child, root.path().join("former-child")).unwrap();
    fs::create_dir(&child).unwrap();
    let replacement_identity = identity(&child);
    assert_ne!(previous_identity, replacement_identity);
    fs::rename(&original, &current).unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("newer"), b"newer work").unwrap();

    // This spelling has the same Path components as original/child. Index keys
    // must ignore repeated separators and '.' without resolving symlinks or '..'.
    let old_child = PathBuf::from(std::ffi::OsString::from_vec(
        [
            root.path().as_os_str().as_encoded_bytes(),
            b"/./folder-\xff//child",
        ]
        .concat(),
    ));
    let sibling = root
        .path()
        .join(std::ffi::OsString::from_vec(b"folder-\xff-extra".to_vec()));
    fs::create_dir(&sibling).unwrap();
    fs::create_dir(sibling.join("child")).unwrap();

    let stage = current.join(".canopa-files-stage-archived");
    fs::create_dir(&stage).unwrap();
    fs::write(stage.join("previous"), b"protected archive").unwrap();
    let archived = json!({
        "retained": {
            "stage": {"directory": stored_path(&stage), "identity": identity(&stage),
                "parent_identity":identity(&current)},
            "identity": identity(&stage.join("previous")), "recovery":true
        },
        "context": {"original":stored_path(&current.join("report.txt")), "label":"older replacement"}
    });
    let history = root.path().join("state/history");
    fs::create_dir(history.parent().unwrap()).unwrap();
    // Construct supported v1 input before opening the engine. Assertions below
    // use public Undo/Recovery and actual files; they never query its database.
    {
        let fixture = rusqlite::Connection::open(&history).unwrap();
        fixture.execute_batch(
            "CREATE TABLE operations (id INTEGER PRIMARY KEY AUTOINCREMENT,label TEXT NOT NULL);
            CREATE TABLE changes (operation INTEGER NOT NULL REFERENCES operations(id) ON DELETE CASCADE,ordinal INTEGER NOT NULL,body TEXT NOT NULL,PRIMARY KEY(operation,ordinal));
            CREATE TABLE pending (singleton INTEGER PRIMARY KEY CHECK(singleton=1),body TEXT NOT NULL);
            CREATE TABLE metadata (key TEXT PRIMARY KEY,value TEXT NOT NULL);
            CREATE TABLE garbage (id INTEGER PRIMARY KEY,body TEXT NOT NULL,error TEXT);
            CREATE TABLE bindings (path BLOB NOT NULL,previous TEXT NOT NULL,replacement TEXT NOT NULL,PRIMARY KEY(path,previous));
            INSERT INTO metadata VALUES('legacy-imported','1');
            INSERT INTO operations VALUES(1,'create children'),(2,'rename folder');
            PRAGMA user_version=1;"
        ).unwrap();
        for (operation, ordinal, change) in [
            (
                1,
                0,
                json!({"kind":"created-folder", "path":stored_path(&old_child), "identity":previous_identity}),
            ),
            (
                1,
                1,
                json!({"kind":"created-folder", "path":stored_path(&sibling.join("child")), "identity":identity(&sibling.join("child"))}),
            ),
            (
                2,
                0,
                json!({"kind":"rename", "original":stored_path(&original), "current":stored_path(&current), "identity":identity(&current)}),
            ),
        ] {
            fixture
                .execute(
                    "INSERT INTO changes VALUES(?1,?2,?3)",
                    rusqlite::params![operation, ordinal, change.to_string()],
                )
                .unwrap();
        }
        fixture
            .execute(
                "INSERT INTO bindings VALUES(?1,?2,?3)",
                rusqlite::params![
                    old_child.as_os_str().as_encoded_bytes(),
                    previous_identity.to_string(),
                    replacement_identity.to_string()
                ],
            )
            .unwrap();
        fixture
            .execute(
                "INSERT INTO garbage(body) VALUES(?1)",
                [archived.to_string()],
            )
            .unwrap();
    }
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let result = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    assert_eq!(result.completed, 1);
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let restored = root
        .path()
        .join(std::ffi::OsString::from_vec(b"folder-\xff (2)".to_vec()));
    let items = engine.recovery_items(None, &cancel).unwrap().items;
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].original.as_deref(),
        Some(restored.join("report.txt").as_path())
    );
    assert!(items[0].entry.is_some());
    let recovered = root.path().join("recovered.txt");
    engine.recover_copy(&items[0], &recovered, &cancel).unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"protected archive");
    engine.undo(&cancel).unwrap();
    engine.undo(&cancel).unwrap();
    assert!(!restored.join("child").exists());
    assert!(!sibling.join("child").exists());
    assert_eq!(fs::read(original.join("newer")).unwrap(), b"newer work");
    assert!(engine.undo_label().is_none());
}

#[test]
fn upgrading_v6_copy_aliases_requires_review_before_removing_another_physical_file() {
    use canopa_files::transfers::{TransferKind, TransferReply, TransferRequest};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("report"), b"same document").unwrap();
    let current = destination.join("report");
    let external = root.path().join("original elsewhere");
    let history = root.path().join("history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.join("report")],
                destination,
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    let metadata = fs::metadata(&current).unwrap();
    let logical = identity(&current);
    fs::rename(&current, &external).unwrap();
    fs::copy(&external, &current).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&current)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
        .unwrap();
    let replacement = identity(&current);
    assert_ne!(logical, replacement);
    drop(engine);
    // Prepare v6 input containing an ambiguous path-wide alias. Such aliases
    // recorded no proof that this Copy entry adopted the replacement.
    {
        let input = rusqlite::Connection::open(&history).unwrap();
        prepare_directory_schema(&input, 6);
        input.execute_batch("ALTER TABLE changes DROP COLUMN bindings; DROP TABLE pending_bindings; DROP TABLE identity_refs; PRAGMA user_version=6;").unwrap();
        input
            .execute(
                "INSERT INTO bindings(path,previous,replacement,revision) VALUES(?1,?2,?3,1)",
                rusqlite::params![
                    current.as_os_str().as_encoded_bytes(),
                    logical.to_string(),
                    replacement.to_string()
                ],
            )
            .unwrap();
    }
    let engine = OperationEngine::open(&history).unwrap();
    let mut asked = false;
    let outcome = engine
        .undo_interactive(None, &cancel, |question| {
            asked = true;
            assert_eq!(question.source, UndoSource::ChangedCopy);
            UndoChoice::Skip
        })
        .unwrap();
    assert!(asked);
    assert_eq!(outcome.completed, 0);
    assert_eq!(outcome.remaining, 1);
    for path in [&current, &external, &source.join("report")] {
        assert_eq!(fs::read(path).unwrap(), b"same document");
    }
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepItem)
        .unwrap();
    assert!(engine.latest_undo_id().is_none());
    assert_eq!(fs::read(&current).unwrap(), b"same document");
}
