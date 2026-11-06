use super::*;
use std::os::unix::fs::PermissionsExt;

fn source(root: &Path) -> Source {
    Source::file(root.join("state/lockdown.sqlite3"), false).unwrap()
}

#[test]
fn missing_defaults_noop_and_every_switch_survive_a_fresh_open() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path());
    let mut store = Store::open(source.clone()).unwrap();
    assert_eq!(store.values(), [false; 7]);
    assert!(!source.path.parent().unwrap().exists());
    assert_eq!(store.set(Property::Camera, false).unwrap(), [false; 7]);
    assert!(!source.path.exists());
    for property in Property::ALL {
        store.set(property, true).unwrap();
    }
    assert_eq!(Store::open(source.clone()).unwrap().values(), [true; 7]);
    assert_eq!(fs::metadata(&source.path).unwrap().mode() & 0o777, 0o600);
    let db = Connection::open(&source.path).unwrap();
    let revision = || {
        db.query_row::<i64, _, _>("SELECT revision FROM policy", [], |row| row.get(0))
            .unwrap()
    };
    let before = revision();
    store.set(Property::Camera, true).unwrap();
    assert_eq!(revision(), before);
    store.set(Property::Camera, false).unwrap();
    let reopened = Store::open(source).unwrap().values();
    assert!(!reopened[Property::Camera as usize]);
    assert_eq!(reopened.into_iter().filter(|value| *value).count(), 6);
}

#[test]
fn concurrent_transactions_preserve_other_writers_switches() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path());
    Store::open(source.clone())
        .unwrap()
        .set(Property::Printing, true)
        .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = [Property::Camera, Property::Microphone]
        .into_iter()
        .map(|key| {
            let source = source.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut store = Store::open(source).unwrap();
                barrier.wait();
                store.set(key, true).unwrap();
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let values = Store::open(source).unwrap().values();
    for key in [Property::Printing, Property::Camera, Property::Microphone] {
        assert!(values[key as usize]);
    }
}

#[test]
fn read_only_and_unwritable_sources_refuse_mutation() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path());
    let mut writable = Store::open(source.clone()).unwrap();
    writable.set(Property::Printing, true).unwrap();
    let mut readonly = Store::open(Source {
        read_only: true,
        ..source.clone()
    })
    .unwrap();
    assert_eq!(
        readonly.set(Property::Printing, false),
        Err(Error::ReadOnly)
    );
    assert!(readonly.values()[Property::Printing as usize]);
    if rustix::process::geteuid().as_raw() != 0 {
        fs::set_permissions(&source.path, fs::Permissions::from_mode(0o400)).unwrap();
        assert_eq!(
            writable.set(Property::Printing, false),
            Err(Error::ReadOnly)
        );
        fs::set_permissions(&source.path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(Store::open(source).unwrap().values()[Property::Printing as usize]);
}

#[test]
fn missing_read_only_source_cannot_relax_administrative_policy() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path());
    let readonly = Source {
        read_only: true,
        ..source.clone()
    };
    assert!(matches!(Store::open(readonly.clone()), Err(Error::Source)));
    Store::open(source.clone())
        .unwrap()
        .set(Property::Camera, true)
        .unwrap();
    let mut store = Store::open(readonly).unwrap();
    fs::remove_file(&source.path).unwrap();
    assert_eq!(store.reload(), Err(Error::Source));
    assert!(store.values()[Property::Camera as usize]);
}

#[test]
fn replacement_requires_reload_and_bad_reload_keeps_the_last_valid_values() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path());
    let mut store = Store::open(source.clone()).unwrap();
    store.set(Property::Printing, true).unwrap();
    let replacement = Source::file(root.path().join("replacement.sqlite3"), false).unwrap();
    Store::open(replacement.clone())
        .unwrap()
        .set(Property::Camera, true)
        .unwrap();
    fs::rename(&replacement.path, &source.path).unwrap();
    assert_eq!(store.set(Property::Printing, false), Err(Error::Changed));
    assert!(store.values()[Property::Printing as usize]);
    let camera = store.reload().unwrap();
    assert!(!camera[Property::Printing as usize] && camera[Property::Camera as usize]);
    fs::write(&source.path, b"private malformed policy contents").unwrap();
    assert_eq!(store.reload(), Err(Error::Source));
    assert_eq!(store.values(), camera);
    assert!(!Error::Source.to_string().contains("private"));
}

#[test]
fn foreign_version_invalid_values_and_nonregular_or_oversized_sources_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path());
    let mut store = Store::open(source.clone()).unwrap();
    store.set(Property::Camera, true).unwrap();
    let db = Connection::open(&source.path).unwrap();
    db.execute_batch("PRAGMA user_version=2").unwrap();
    assert!(matches!(Store::open(source.clone()), Err(Error::Source)));
    db.execute_batch(
        "PRAGMA user_version=1; PRAGMA ignore_check_constraints=ON; UPDATE policy SET camera=2;",
    )
    .unwrap();
    assert!(matches!(Store::open(source.clone()), Err(Error::Source)));
    drop(db);
    let link = root.path().join("linked.sqlite3");
    std::os::unix::fs::symlink(&source.path, &link).unwrap();
    assert!(matches!(
        Store::open(Source::file(link, false).unwrap()),
        Err(Error::Source)
    ));
    assert!(matches!(
        Store::open(Source::file(root.path().into(), false).unwrap()),
        Err(Error::Source)
    ));
    OpenOptions::new()
        .write(true)
        .open(&source.path)
        .unwrap()
        .set_len(MAX_BYTES + 1)
        .unwrap();
    assert!(matches!(Store::open(source), Err(Error::Source)));
    let foreign = root.path().join("foreign.sqlite3");
    Connection::open(&foreign)
        .unwrap()
        .execute_batch(
            "CREATE TABLE unrelated(value TEXT); INSERT INTO unrelated VALUES('private');",
        )
        .unwrap();
    assert!(matches!(
        Store::open(Source::file(foreign, false).unwrap()),
        Err(Error::Source)
    ));
}

#[test]
fn a_busy_database_preserves_the_snapshot_and_remains_retryable() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path());
    let mut store = Store::open(source.clone()).unwrap();
    store.set(Property::Camera, true).unwrap();
    let db = Connection::open(&source.path).unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_eq!(store.set(Property::Printing, true), Err(Error::Busy));
    assert!(!store.values()[Property::Printing as usize]);
    db.execute_batch("ROLLBACK").unwrap();
    assert!(store.set(Property::Printing, true).unwrap()[Property::Printing as usize]);
}

#[test]
fn a_matching_header_cannot_disguise_a_view_as_the_policy_table() {
    let root = tempfile::tempdir().unwrap();
    let source = Source::file(root.path().join("view.sqlite3"), false).unwrap();
    Connection::open(&source.path)
        .unwrap()
        .execute_batch(
            "PRAGMA application_id=1129205836; PRAGMA user_version=1;
        CREATE VIEW policy AS SELECT 1 AS id,0 AS revision,0 AS printing,0 AS save_to_disk,
        0 AS application_handlers,0 AS location,0 AS camera,0 AS microphone,0 AS sound_output;",
        )
        .unwrap();
    assert!(matches!(Store::open(source), Err(Error::Source)));
}
