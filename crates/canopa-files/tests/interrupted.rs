use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest},
    transfers::{TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

struct PreservedFixture(Option<tempfile::TempDir>);
impl PreservedFixture {
    fn new(prefix: &str) -> Self {
        Self(Some(
            tempfile::Builder::new().prefix(prefix).tempdir().unwrap(),
        ))
    }
    fn path(&self) -> &std::path::Path {
        self.0.as_ref().unwrap().path()
    }
}
impl Drop for PreservedFixture {
    fn drop(&mut self) {
        if std::thread::panicking()
            && let Some(root) = self.0.take()
        {
            eprintln!(
                "retained expanded interrupted fixture: {}",
                root.keep().display()
            );
        }
    }
}

#[test]
fn interrupted_undo_keeps_history_inspectable_and_allows_journaled_recovery_copies() {
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    let root = PreservedFixture::new("canopa-interrupted-undo-");
    let quarantine = interrupted_undo(root.path());
    let work = quarantine.parent().unwrap().to_owned();
    assert!(!root.path().join("undo-returned").exists());
    // A post-crash metadata edit must remain recoverable in every observed phase.
    let mode = fs::metadata(&quarantine).unwrap().permissions().mode();
    fs::set_permissions(&quarantine, fs::Permissions::from_mode(mode ^ 0o040)).unwrap();
    let original = root.path().join("destination/report.txt");
    fs::write(&original, b"newer occupant").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).expect("recovery leaves history inspectable");
    let cancel = Cancellation::default();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.interrupted.len(), 1);
    assert!(page.interrupted[0].paths.iter().any(|path| path == &work));
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].entry.as_ref().unwrap().path, work);
    assert!(page.items[0].can_keep_in_recovery());
    assert_eq!(
        fs::read(&quarantine).unwrap(),
        fs::read(root.path().join("source/report.txt")).unwrap()
    );
    let forbidden = root.path().join("blocked-folder");
    assert!(
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: forbidden.clone()
                },
                &cancel
            )
            .is_err()
    );
    assert!(!forbidden.exists());
    assert!(engine.retry_recovery(&cancel).is_err());
    assert!(
        engine
            .recover_copy(&page.items[0], &work.join("unsafe-copy"), &cancel)
            .is_err()
    );
    let alias = root.path().join("quarantine-alias");
    std::os::unix::fs::symlink(&work, &alias).unwrap();
    let aliased = alias.join("unsafe-copy");
    assert!(
        engine
            .recover_copy(&page.items[0], &aliased, &cancel)
            .is_err()
    );
    assert!(!aliased.exists());
    let salvage = root.path().join("salvage");
    fs::create_dir(&salvage).unwrap();
    drop(engine);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "interrupted_recovery_copy_child"])
        .env("CANOPA_INTERRUPTED_ROOT", root.path())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let unfinished = loop {
        if let Some(path) = recovery_partial(&salvage, 0) {
            child.kill().unwrap();
            assert_eq!(child.wait().unwrap().signal(), Some(9));
            break path;
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "copy must stop after actual bytes were written"
        );
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("did not observe recovery-copy data");
        }
        std::thread::sleep(Duration::from_micros(50));
    };
    assert!(!root.path().join("recovery-copy-returned").exists());
    let unfinished_bytes = fs::read(&unfinished).unwrap();
    assert!(!unfinished_bytes.is_empty());
    let engine = OperationEngine::open_with_recovery_check(&history, |paths| {
        assert!(paths.iter().any(|path| path.starts_with(&salvage)));
        Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "The original recovery device is unavailable",
        ))
    })
    .unwrap();
    assert_eq!(fs::read(&unfinished).unwrap(), unfinished_bytes);
    let issues = engine.interrupted_operations(&cancel).unwrap();
    assert_eq!(issues.len(), 2);
    assert!(
        issues
            .iter()
            .all(|issue| issue.message.contains("device is unavailable"))
    );
    assert_eq!(fs::read(&original).unwrap(), b"newer occupant");
    assert!(quarantine.exists());
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert!(
        !unfinished.exists(),
        "only the reproducible unfinished leaf is disposable"
    );
    assert!(quarantine.exists());
    assert_eq!(engine.interrupted_operations(&cancel).unwrap().len(), 1);
    let rescued = root.path().join("rescued work folder");
    engine
        .recover_copy(&page.items[0], &rescued, &cancel)
        .unwrap();
    assert_eq!(
        fs::read(rescued.join("folder")).unwrap(),
        fs::read(root.path().join("source/report.txt")).unwrap()
    );
    assert_eq!(fs::read(&original).unwrap(), b"newer occupant");
    assert!(
        engine.undo(&cancel).is_err(),
        "export must not retire the original journal"
    );
    let fresh = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.entry.as_ref().is_some_and(|entry| entry.path == work))
        .unwrap();
    assert!(fresh.can_keep_in_recovery());
    let before_keep = engine.latest_undo_id();
    engine.keep_in_recovery(&fresh, &cancel).unwrap();
    assert_eq!(engine.latest_undo_id(), before_keep);
    engine.retry_recovery(&cancel).unwrap();
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    let mut undone = 0;
    while engine.undo_label().is_some() {
        assert!(undone < 4, "only Recovery exports should remain undoable");
        engine.undo(&cancel).unwrap();
        undone += 1;
    }
    assert!(!rescued.exists());
    assert_eq!(fs::read(&original).unwrap(), b"newer occupant");
    assert_eq!(
        fs::read(&quarantine).unwrap(),
        fs::read(root.path().join("source/report.txt")).unwrap()
    );
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert!(engine.undo_label().is_none());
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .iter()
            .any(|item| item.entry.as_ref().is_some_and(|entry| entry.path == work))
    );
    assert_eq!(fs::read(&original).unwrap(), b"newer occupant");
}

fn recovery_partial(root: &Path, depth: usize) -> Option<std::path::PathBuf> {
    if depth > 6 {
        return None;
    }
    for entry in fs::read_dir(root).ok()?.filter_map(Result::ok) {
        let path = entry.path();
        let kind = entry.file_type().ok()?;
        if kind.is_file()
            && entry.file_name() == "folder"
            && root
                .file_name()
                .is_some_and(|name| name.as_encoded_bytes().starts_with(b".canopa-files-stage-"))
            && entry.metadata().is_ok_and(|metadata| metadata.len() > 0)
        {
            return Some(path);
        }
        if kind.is_dir()
            && let Some(path) = recovery_partial(&path, depth + 1)
        {
            return Some(path);
        }
    }
    None
}

#[test]
#[ignore = "child process fixture; parent kills it during an actual copied-file Undo"]
fn interrupted_undo_child() {
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_INTERRUPTED_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-interrupted-undo-")
    );
    prepare_copy(&root);
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    fs::write(root.join("undo-ready"), b"ready").unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    fs::write(root.join("undo-returned"), b"missed crash").unwrap();
    panic!("parent must kill the child during Undo");
}

#[test]
#[ignore = "child process fixture; parent kills it after recovery-copy bytes are written"]
fn interrupted_recovery_copy_child() {
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_INTERRUPTED_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-interrupted-undo-")
    );
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let page = engine.recovery_items(None, &cancel).unwrap();
    engine
        .recover_copy(&page.items[0], &root.join("salvage/recovered"), &cancel)
        .unwrap();
    fs::write(root.join("recovery-copy-returned"), b"missed crash").unwrap();
    panic!("parent must kill the child during recovery copying");
}

fn prepare_copy(root: &Path) {
    let source = root.join("source");
    let destination = root.join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("report.txt"), vec![b'a'; 16 * 1024 * 1024]).unwrap();
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.join("report.txt")],
                destination,
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(result.failures.is_empty());
}

fn interrupted_undo(root: &Path) -> std::path::PathBuf {
    use std::os::unix::process::ExitStatusExt;
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "interrupted_undo_child"])
        .env("CANOPA_INTERRUPTED_ROOT", root)
        .spawn()
        .unwrap();
    let destination = root.join("destination");
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let payload = fs::read_dir(&destination)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b".canopa-files-stage-")
            })
            .map(|entry| entry.path().join("folder"))
            .find(|path| path.exists());
        if root.join("undo-ready").exists()
            && let Some(payload) = payload
        {
            child.kill().unwrap();
            assert_eq!(child.wait().unwrap().signal(), Some(9));
            return payload;
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "child must stop during an actual Undo"
        );
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("did not observe the Undo quarantine");
        }
        std::thread::sleep(Duration::from_micros(50));
    }
}

#[test]
fn an_interrupted_original_directory_can_be_salvaged_without_replaying_its_journal() {
    let root = PreservedFixture::new("canopa-interrupted-undo-");
    let quarantine = interrupted_undo(root.path());
    let work = quarantine.parent().unwrap().to_owned();
    let held = root.path().join("Held original bytes");
    fs::rename(&quarantine, &held).unwrap();
    fs::create_dir(&quarantine).unwrap();
    fs::write(quarantine.join("a.txt"), b"saved directory work").unwrap();
    fs::create_dir(quarantine.join("nested")).unwrap();
    fs::write(quarantine.join("nested/b.txt"), b"nested saved work").unwrap();
    let original = root.path().join("destination/report.txt");
    fs::write(&original, b"newer occupant").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.interrupted.len(), 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].entry.as_ref().unwrap().kind,
        canopa_files::browse::EntryKind::Directory
    );
    let destination = root.path().join("Salvaged directory");
    let outcome = engine
        .recover_copy_with_check(
            &page.items[0],
            &destination,
            &cancel,
            |path, _| {
                assert!(
                    path.starts_with(&destination)
                        || path == work
                        || path.starts_with(&work)
                        || path == root.path(),
                    "salvage must not attempt placement or replay of the original journal: {}",
                    path.display()
                );
                Ok(())
            },
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(outcome.completed, 5);
    assert!(outcome.failures.is_empty() && !outcome.needs_attention);
    assert_eq!(
        fs::read(destination.join("folder/nested/b.txt")).unwrap(),
        b"nested saved work"
    );
    assert_eq!(fs::read(&original).unwrap(), b"newer occupant");
    assert!(quarantine.is_dir() && held.is_file());
    assert_eq!(engine.interrupted_operations(&cancel).unwrap().len(), 1);
    assert!(engine.retry_recovery(&cancel).is_err());
    assert!(engine.undo(&cancel).is_err());
    assert_eq!(
        fs::read(destination.join("folder/a.txt")).unwrap(),
        b"saved directory work"
    );
}
