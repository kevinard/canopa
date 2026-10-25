use canopa_files::{
    Cancellation,
    operations::OperationEngine,
    trash::{
        TrashLocation,
        catalog::{Catalog, Scope},
    },
};
use std::{fs, os::unix::ffi::OsStringExt};

#[test]
fn main_catalog_preserves_raw_names_and_orphans_without_creating_storage() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let data = root.join("data");
    let empty = Catalog::read(&data, Scope::Main, &Cancellation::default()).unwrap();
    assert!(empty.items.is_empty());
    assert!(empty.warnings.is_empty());
    assert!(!data.exists(), "reading Trash created storage");
    let source = root.join(std::ffi::OsString::from_vec(b"picture-\xff.png".to_vec()));
    fs::write(&source, b"stored bytes").unwrap();
    let location = TrashLocation::home(&data).unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine
        .trash(
            &location,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    let snapshot = Catalog::read(&data, Scope::Main, &Cancellation::default()).unwrap();
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].original.as_ref(), Some(&source));
    assert_eq!(snapshot.items[0].name(), source.file_name().unwrap());
    assert_eq!(fs::read(&snapshot.items[0].path).unwrap(), b"stored bytes");
    assert_eq!(snapshot.roots, vec![location.directory().to_owned()]);
    let replacement = root.join("replacement");
    fs::write(&replacement, b"newer bytes!").unwrap();
    fs::File::options()
        .write(true)
        .open(&replacement)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(snapshot.items[0].entry.modified.unwrap()))
        .unwrap();
    fs::rename(&replacement, &snapshot.items[0].path).unwrap();
    let replaced = Catalog::read(&data, Scope::Main, &Cancellation::default()).unwrap();
    assert_eq!(replaced.items[0].entry.size, snapshot.items[0].entry.size);
    assert_eq!(
        replaced.items[0].entry.modified,
        snapshot.items[0].entry.modified
    );
    assert_ne!(
        replaced.items[0].version(),
        snapshot.items[0].version(),
        "a replacement with preserved size/date must invalidate prepared content"
    );
    let mut info = snapshot.items[0].path.file_name().unwrap().to_owned();
    info.push(".trashinfo");
    fs::remove_file(location.directory().join("info").join(info)).unwrap();
    let orphan = Catalog::read(&data, Scope::Main, &Cancellation::default()).unwrap();
    assert_eq!(orphan.items.len(), 1);
    assert!(orphan.items[0].original.is_none());
    assert!(orphan.items[0].problem.is_some());
    assert_eq!(fs::read(&orphan.items[0].path).unwrap(), b"newer bytes!");
}

#[cfg(feature = "local-runtime")]
#[test]
fn headless_reader_observes_storage_creation_metadata_changes_and_restore() {
    use canopa_files::trash::catalog::Reader;
    use std::time::{Duration, Instant};
    fn wait(mut ready: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready() {
            assert!(
                Instant::now() < deadline,
                "Trash catalog did not publish filesystem changes"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let data = root.join("data");
    let reader = Reader::start(data.clone(), Scope::Main).unwrap();
    wait(|| !reader.snapshot().loading);
    assert!(!data.exists());
    let source = root.join("readable.txt");
    fs::write(&source, b"preview and restore").unwrap();
    let location = TrashLocation::home(&data).unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine
        .trash(
            &location,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    wait(|| reader.snapshot().catalog.items.len() == 1);
    let item = reader.snapshot().catalog.items[0].clone();
    let mut name = item.path.file_name().unwrap().to_owned();
    name.push(".trashinfo");
    let info = location.directory().join("info").join(name);
    let metadata = fs::read(&info).unwrap();
    fs::remove_file(&info).unwrap();
    wait(|| {
        reader
            .snapshot()
            .catalog
            .items
            .first()
            .is_some_and(|item| item.problem.is_some())
    });
    assert_eq!(fs::read(&item.path).unwrap(), b"preview and restore");
    fs::write(&info, metadata).unwrap();
    wait(|| {
        reader
            .snapshot()
            .catalog
            .items
            .first()
            .is_some_and(|item| item.problem.is_none())
    });
    engine
        .restore(&item, None, &Cancellation::default())
        .unwrap();
    wait(|| !reader.snapshot().loading && reader.snapshot().catalog.items.is_empty());
    assert_eq!(fs::read(source).unwrap(), b"preview and restore");
    reader.refresh();
    wait(|| !reader.snapshot().loading);
    assert!(reader.snapshot().error.is_none());
}

#[cfg(feature = "local-runtime")]
#[test]
fn a_new_volume_store_is_observed_without_manual_refresh() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let temporary = tempfile::Builder::new()
        .prefix("canopa-files-trash-catalog-")
        .tempdir_in("/tmp")
        .unwrap();
    let root = temporary.path();
    fs::create_dir(root.join("volume")).unwrap();
    fs::write(
        root.join("mountinfo"),
        format!(
            "1 0 0:1 / {} rw - tmpfs fixture rw\n",
            root.join("volume").display()
        ),
    )
    .unwrap();
    let library = root.join("mounts.so");
    let compiled = Command::new("cc")
        .args(["-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/catalog_mounts.c"),
        )
        .arg("-ldl")
        .arg("-o")
        .arg(&library)
        .status()
        .unwrap();
    assert!(compiled.success());
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "catalog_mount_helper",
            "--ignored",
            "--nocapture",
        ])
        .env("CANOPA_FILES_CATALOG_FIXTURE", root)
        .env("LD_PRELOAD", library)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let output = child.wait_with_output().unwrap();
            assert!(
                status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("private catalog child timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        root.join("mountinfo-read").is_file(),
        "the private inventory was not used"
    );
}

#[cfg(feature = "local-runtime")]
#[test]
#[ignore = "private child with a synthetic mount inventory; parent owns timeout and cleanup"]
fn catalog_mount_helper() {
    use canopa_files::trash::catalog::Reader;
    use std::time::{Duration, Instant};
    let root = std::path::PathBuf::from(std::env::var_os("CANOPA_FILES_CATALOG_FIXTURE").unwrap());
    assert_eq!(root.parent(), Some(std::path::Path::new("/tmp")));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-files-trash-catalog-")
    );
    let data = root.join("data");
    fs::create_dir(&data).unwrap();
    let volume = root.join("volume");
    let source = volume.join("source.txt");
    fs::write(&source, b"new volume Trash\n").unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let reader = Reader::start(data, Scope::All).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while reader.snapshot().loading {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(reader.snapshot().catalog.items.is_empty());
    let location = TrashLocation::volume(&volume).unwrap();
    engine
        .trash(
            &location,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    while reader.snapshot().catalog.items.is_empty() {
        assert!(
            Instant::now() < deadline,
            "new store was not discovered without manual Refresh"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let snapshot = reader.snapshot();
    assert_eq!(snapshot.catalog.items.len(), 1);
    assert_eq!(snapshot.catalog.items[0].original.as_ref(), Some(&source));
    assert_eq!(
        fs::read(&snapshot.catalog.items[0].path).unwrap(),
        b"new volume Trash\n"
    );
}
