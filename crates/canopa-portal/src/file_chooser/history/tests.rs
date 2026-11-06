use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn accepted_folders_persist_per_app_as_private_byte_paths() {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("state/history.db");
    let a = root
        .path()
        .join(std::ffi::OsString::from_vec(b"raw-\xff".to_vec()));
    let b = root.path().join("another");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let history = History::file(database.clone()).unwrap();
    assert!(history.load("org.example.A".into()).await.is_none());
    assert!(
        !database.exists(),
        "Reading must not create preference state"
    );
    history.save("org.example.A".into(), a.clone()).await;
    history.save("org.example.B".into(), b.clone()).await;
    drop(history);
    let reopened = History::file(database.clone()).unwrap();
    assert_eq!(reopened.load("org.example.A".into()).await, Some(a));
    assert_eq!(reopened.load("org.example.B".into()).await, Some(b.clone()));
    assert!(reopened.load("org.example.Other".into()).await.is_none());
    assert_eq!(
        fs::metadata(&database).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::remove_dir(b).unwrap();
    assert!(reopened.load("org.example.B".into()).await.is_none());
}

#[tokio::test]
async fn anonymous_invalid_and_disabled_history_create_nothing() {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("missing/history.db");
    let history = History::file(database.clone()).unwrap();
    for app in ["", "../other", "id\0injection", "invalid id"] {
        history.save(app.into(), root.path().into()).await;
        assert!(history.load(app.into()).await.is_none());
    }
    history
        .save("org.example.Valid".into(), "relative".into())
        .await;
    assert!(!database.exists() && !database.parent().unwrap().exists());
    let disabled = History::disabled();
    disabled
        .save("org.example.Valid".into(), root.path().into())
        .await;
    assert!(disabled.load("org.example.Valid".into()).await.is_none());
}

#[tokio::test]
async fn state_retains_only_the_last_128_applications() {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("state/history.db");
    for history in [History::memory(), History::file(database.clone()).unwrap()] {
        for n in 0..=MAX_APPS {
            history
                .write(&format!("org.example.App{n}"), root.path().into())
                .unwrap();
        }
        assert!(history.read("org.example.App0").unwrap().is_none());
        assert!(history.read("org.example.App1").unwrap().is_some());
        history
            .write("org.example.App1", root.path().into())
            .unwrap();
        history
            .write("org.example.Last", root.path().into())
            .unwrap();
        assert!(history.read("org.example.App1").unwrap().is_some());
        assert!(history.read("org.example.App2").unwrap().is_none());
    }
    assert!(fs::metadata(database).unwrap().len() <= MAX_FILE);
}

#[tokio::test]
async fn unsafe_foreign_and_future_state_is_not_rewritten() {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("history.db");
    let history = History::file(database.clone()).unwrap();
    let unrelated = root.path().join("unrelated");
    fs::write(&unrelated, b"keep my data").unwrap();
    std::os::unix::fs::symlink(&unrelated, &database).unwrap();
    history
        .save("org.example.A".into(), root.path().into())
        .await;
    assert!(history.load("org.example.A".into()).await.is_none());
    assert_eq!(fs::read(&unrelated).unwrap(), b"keep my data");
    fs::remove_file(&database).unwrap();
    let db = Connection::open(&database).unwrap();
    db.execute_batch("CREATE TABLE unrelated(value TEXT); INSERT INTO unrelated VALUES('keep');")
        .unwrap();
    drop(db);
    fs::set_permissions(&database, fs::Permissions::from_mode(0o600)).unwrap();
    let before = fs::read(&database).unwrap();
    history
        .save("org.example.A".into(), root.path().into())
        .await;
    assert!(history.load("org.example.A".into()).await.is_none());
    assert_eq!(fs::read(&database).unwrap(), before);
    fs::remove_file(&database).unwrap();
    history
        .save("org.example.A".into(), root.path().into())
        .await;
    let db = Connection::open(&database).unwrap();
    db.pragma_update(None, "user_version", 2).unwrap();
    drop(db);
    let before = fs::read(&database).unwrap();
    history
        .save("org.example.B".into(), root.path().into())
        .await;
    assert!(history.load("org.example.A".into()).await.is_none());
    assert_eq!(fs::read(&database).unwrap(), before);
    fs::set_permissions(&database, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(history.load("org.example.A".into()).await.is_none());
}

#[tokio::test]
async fn optional_history_never_queues_behind_an_active_operation() {
    let root = tempfile::tempdir().unwrap();
    let history = History::memory();
    history
        .save("org.example.A".into(), root.path().into())
        .await;
    let held = history.0.operations.clone().acquire_owned().await.unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_millis(50),
            history.load("org.example.A".into())
        )
        .await
        .unwrap()
        .is_none()
    );
    tokio::time::timeout(
        Duration::from_millis(50),
        history.save("org.example.B".into(), root.path().into()),
    )
    .await
    .unwrap();
    drop(held);
    assert_eq!(
        history.load("org.example.A".into()).await,
        Some(root.path().into())
    );
    assert!(history.load("org.example.B".into()).await.is_none());
}

#[test]
fn folder_hint_uses_only_the_selected_folder_or_a_common_file_parent() {
    use crate::file_chooser::{ChooserRequest, Mode, Selection};
    let mut request = ChooserRequest::parse(
        Mode::Open,
        "org.example.A".into(),
        String::new(),
        "Choose".into(),
        Default::default(),
    )
    .unwrap();
    let mut selection = Selection {
        paths: vec![b"/a/one".to_vec(), b"/a/two".to_vec()],
        choices: vec![],
        current_filter: None,
        writable: true,
    };
    assert_eq!(selection.folder(&request), Some("/a".into()));
    selection.paths[1] = b"/b/other".to_vec();
    assert!(selection.folder(&request).is_none());
    selection.paths.truncate(1);
    request.directory = true;
    assert_eq!(selection.folder(&request), Some("/a/one".into()));
    request.mode = Mode::SaveMany;
    assert_eq!(selection.folder(&request), Some("/a".into()));
}
