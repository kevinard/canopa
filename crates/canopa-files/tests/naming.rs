//! New documents and bulk Rename through the public operation engine, in
//! private temporary folders.
use canopa_files::{
    Cancellation,
    operations::{
        OperationEngine, OperationRequest,
        rename_batch::{BatchRename, order},
    },
};
use std::{
    ffi::OsString,
    fs,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

fn engine(root: &Path) -> OperationEngine {
    OperationEngine::open(root.join("state/operations.sqlite3")).unwrap()
}

fn step(folder: &Path, source: &str, destination: &str) -> BatchRename {
    BatchRename {
        source: folder.join(source),
        destination: folder.join(destination),
    }
}

fn no_stages(folder: &Path) -> bool {
    !fs::read_dir(folder).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .as_encoded_bytes()
            .starts_with(b".canopa-files-stage-")
    })
}

#[test]
fn a_new_document_copies_its_template_dated_now_and_undo_removes_it() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    let template = root.path().join("Letter.odt");
    fs::write(&template, b"template").unwrap();
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    fs::File::options()
        .write(true)
        .open(&template)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let engine = engine(root.path());
    let document = folder.join("Letter.odt");
    let summary = engine
        .execute(
            OperationRequest::CreateFile {
                path: document.clone(),
                template: Some(template.clone()),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(summary.paths, std::slice::from_ref(&document));
    assert_eq!(fs::read(&document).unwrap(), b"template");
    assert!(fs::metadata(&document).unwrap().modified().unwrap() > old);
    assert!(no_stages(&folder));
    assert_eq!(
        engine.undo_label().as_deref(),
        Some("Undo create file Letter.odt")
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!document.exists());
    assert_eq!(fs::read(&template).unwrap(), b"template");
}

#[test]
fn a_taken_name_is_refused_and_an_edited_document_is_kept_by_undo() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    let engine = engine(root.path());
    let taken = folder.join("Notes");
    fs::write(&taken, b"mine").unwrap();
    let create = |path: &Path| {
        engine.execute(
            OperationRequest::CreateFile {
                path: path.to_owned(),
                template: None,
            },
            &Cancellation::default(),
        )
    };
    assert!(create(&taken).is_err());
    assert_eq!(fs::read(&taken).unwrap(), b"mine");
    assert!(no_stages(&folder));
    assert!(
        engine.undo_label().is_none(),
        "a refused document has no Undo"
    );

    let empty = folder.join("Empty");
    create(&empty).unwrap();
    assert_eq!(fs::metadata(&empty).unwrap().len(), 0);
    fs::write(&empty, b"written since").unwrap();
    // Undo never removes newer work without asking.
    assert!(engine.undo(&Cancellation::default()).is_err());
    assert_eq!(fs::read(&empty).unwrap(), b"written since");
}

#[test]
fn a_batch_renames_chains_in_order_and_one_undo_returns_every_name() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    for name in ["1", "2", "3"] {
        fs::write(folder.join(name), name).unwrap();
    }
    let raw = folder.join(PathBuf::from(OsString::from_vec(b"caf\xe9".to_vec())));
    fs::write(&raw, b"raw").unwrap();
    let engine = engine(root.path());
    let renames = vec![
        step(&folder, "1", "2"),
        step(&folder, "2", "3"),
        step(&folder, "3", "4"),
        BatchRename {
            source: raw.clone(),
            destination: folder.join("cafe"),
        },
    ];
    let outcome = engine
        .rename_batch(&renames, &Cancellation::default())
        .unwrap();
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert_eq!(outcome.renamed.len(), 4);
    for (name, contents) in [("2", "1"), ("3", "2"), ("4", "3"), ("cafe", "raw")] {
        assert_eq!(fs::read_to_string(folder.join(name)).unwrap(), contents);
    }
    assert!(!folder.join("1").exists() && !raw.exists());
    assert_eq!(engine.undo_label().as_deref(), Some("Undo rename 4 items"));
    engine.undo(&Cancellation::default()).unwrap();
    for name in ["1", "2", "3"] {
        assert_eq!(fs::read_to_string(folder.join(name)).unwrap(), name);
    }
    assert_eq!(fs::read(&raw).unwrap(), b"raw");
    assert!(!folder.join("4").exists() && !folder.join("cafe").exists());
    assert!(engine.undo_label().is_none());
    assert!(no_stages(&folder));
}

#[test]
fn a_taken_destination_fails_alone_and_the_rest_is_one_undo_entry() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    for name in ["a", "b", "c"] {
        fs::write(folder.join(name), name).unwrap();
    }
    // Created after the preview, as another program might.
    fs::write(folder.join("B"), b"other").unwrap();
    let engine = engine(root.path());
    let outcome = engine
        .rename_batch(
            &[
                step(&folder, "a", "A"),
                step(&folder, "b", "B"),
                step(&folder, "c", "C"),
            ],
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(outcome.renamed, [folder.join("A"), folder.join("C")]);
    assert_eq!(outcome.failures.len(), 1);
    assert_eq!(outcome.failures[0].0, folder.join("b"));
    assert_eq!(fs::read(folder.join("B")).unwrap(), b"other");
    assert_eq!(fs::read(folder.join("b")).unwrap(), b"b");
    assert!(outcome.id.is_some() && outcome.interrupted.is_none());
    engine.undo(&Cancellation::default()).unwrap();
    for name in ["a", "b", "c"] {
        assert_eq!(fs::read_to_string(folder.join(name)).unwrap(), name);
    }
    assert_eq!(fs::read(folder.join("B")).unwrap(), b"other");
    assert!(no_stages(&folder));
}

#[test]
fn swaps_and_cancelled_batches_change_nothing() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    for name in ["a", "b"] {
        fs::write(folder.join(name), name).unwrap();
    }
    let engine = engine(root.path());
    let swap = [step(&folder, "a", "b"), step(&folder, "b", "a")];
    assert!(order(&swap).is_err());
    assert!(
        engine
            .rename_batch(&swap, &Cancellation::default())
            .is_err()
    );
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert!(
        engine
            .rename_batch(&[step(&folder, "a", "x")], &cancellation)
            .is_err()
    );
    for name in ["a", "b"] {
        assert_eq!(fs::read_to_string(folder.join(name)).unwrap(), name);
    }
    assert!(engine.undo_label().is_none());
}
