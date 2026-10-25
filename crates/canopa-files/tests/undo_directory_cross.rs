use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, UndoChoice},
    transfers::{TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    os::unix::{
        ffi::OsStringExt,
        fs::{MetadataExt, symlink},
    },
    path::PathBuf,
};

#[test]
fn protected_recovery_data_stays_available_during_a_cross_filesystem_folder_return() {
    let source = tempfile::tempdir().unwrap();
    let device = fs::metadata(source.path()).unwrap().dev();
    let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|path| {
            (fs::metadata(&path).ok()?.dev() != device)
                .then(|| {
                    tempfile::Builder::new()
                        .prefix(".canopa-merge-cross-recovery-")
                        .tempdir_in(path)
                        .ok()
                })
                .flatten()
        })
        .unwrap();
    let parent = source.path().join("original");
    fs::create_dir(&parent).unwrap();
    let original = parent.join("project");
    fs::create_dir_all(original.join("nested")).unwrap();
    let seed = source.path().join("protected.txt");
    fs::write(&seed, b"copied version").unwrap();
    let history = source.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed],
                destination: original.join("nested"),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(result.failures.is_empty());
    fs::write(
        original.join("nested/protected.txt"),
        b"protected newer work",
    )
    .unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    let current = source.path().join("moved");
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::remove_dir(&parent).unwrap();
    symlink(destination.path(), &parent).unwrap();
    fs::create_dir(&original).unwrap();
    // Returning this folder must migrate the protected version and its catalog
    // ownership together, without leaving a saved-data container behind.
    let outcome = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert_eq!(outcome.remaining, 0);
    assert!(!current.exists());
    drop(engine);
    let engine = OperationEngine::open(history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    let item = &page.items[0];
    assert!(!item.held_for_undo);
    assert_eq!(
        fs::read(
            &item
                .entry
                .as_ref()
                .expect("protected work must remain inspectable")
                .path
        )
        .unwrap(),
        b"protected newer work"
    );
    let recovered = source.path().join("recovered.txt");
    engine.recover_copy(item, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"protected newer work");
}

#[test]
fn a_killed_cross_filesystem_folder_return_resumes_its_snapshot_without_duplicate_children() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let source = tempfile::Builder::new()
        .prefix("canopa-merge-cross-crash-")
        .tempdir()
        .unwrap();
    let device = fs::metadata(source.path()).unwrap().dev();
    let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|path| {
            (fs::metadata(&path).ok()?.dev() != device)
                .then(|| {
                    tempfile::Builder::new()
                        .prefix(".canopa-merge-cross-crash-")
                        .tempdir_in(path)
                        .ok()
                })
                .flatten()
        })
        .unwrap();
    let parent = source.path().join("original");
    fs::create_dir(&parent).unwrap();
    let original = parent.join("project");
    fs::create_dir_all(original.join("many")).unwrap();
    for index in 0..128 {
        fs::write(
            original.join(format!("many/item-{index:04}.txt")),
            format!("work {index}"),
        )
        .unwrap();
    }
    fs::write(original.join("z.txt"), b"incoming final child").unwrap();
    let history = source.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let current = source.path().join("moved");
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    drop(engine);
    fs::remove_dir(&parent).unwrap();
    symlink(destination.path(), &parent).unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("z.txt"), b"newer final work").unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "crash_cross_directory_destination_helper",
        ])
        .env("CANOPA_CROSS_DIRECTORY_ROOT", source.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(original.join("many").is_dir());
    let unreturned = fs::read_to_string(source.path().join("witness"))
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert!(unreturned > 0);
    let engine = OperationEngine::open(history).unwrap();
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.destination, original.join("z.txt"));
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    for index in 0..128 {
        assert_eq!(
            fs::read(original.join(format!("many/item-{index:04}.txt"))).unwrap(),
            format!("work {index}").as_bytes()
        );
        assert!(
            !original
                .join(format!("many/item-{index:04} (2).txt"))
                .exists()
        );
    }
    assert_eq!(
        fs::read(original.join("z.txt")).unwrap(),
        b"newer final work"
    );
    assert_eq!(
        fs::read(original.join("z (2).txt")).unwrap(),
        b"incoming final child"
    );
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
#[ignore = "child fixture; killed after observing the actual new cross-filesystem folder"]
fn crash_cross_directory_destination_helper() {
    use std::time::Duration;
    let root = PathBuf::from(
        std::env::var_os("CANOPA_CROSS_DIRECTORY_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-merge-cross-crash-")
    );
    let destination = root.join("original/project/many");
    let current = root.join("moved/many");
    let witness = root.join("witness");
    std::thread::spawn(move || {
        while !destination.is_dir() {
            std::thread::sleep(Duration::from_micros(50));
        }
        let unreturned = fs::read_dir(current).unwrap().count();
        fs::write(witness, unreturned.to_string()).unwrap();
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    engine
        .undo_interactive(None, &Cancellation::default(), |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                loop {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        })
        .unwrap();
    panic!("the child did not reach its folder-creation interruption");
}

#[test]
fn a_cross_filesystem_folder_snapshot_is_not_limited_to_one_journal_record() {
    let source = tempfile::tempdir().unwrap();
    let device = fs::metadata(source.path()).unwrap().dev();
    let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|path| {
            (fs::metadata(&path).ok()?.dev() != device)
                .then(|| {
                    tempfile::Builder::new()
                        .prefix(".canopa-merge-cross-many-")
                        .tempdir_in(path)
                        .ok()
                })
                .flatten()
        })
        .unwrap();
    let parent = source.path().join("original");
    fs::create_dir(&parent).unwrap();
    let original = parent.join("project");
    fs::create_dir_all(original.join("many")).unwrap();
    let prefix = "document-".repeat(26);
    for index in 0..256 {
        fs::write(
            original.join(format!("many/{prefix}{index:04}.txt")),
            format!("work {index}"),
        )
        .unwrap();
    }
    let history = source.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let current = source.path().join("moved");
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::remove_dir(&parent).unwrap();
    symlink(destination.path(), &parent).unwrap();
    fs::create_dir(&original).unwrap();
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.destination, original);
            UndoChoice::Merge
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    for index in 0..256 {
        assert_eq!(
            fs::read(original.join(format!("many/{prefix}{index:04}.txt"))).unwrap(),
            format!("work {index}").as_bytes()
        );
    }
    drop(engine);
    let engine = OperationEngine::open(history).unwrap();
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    assert!(engine.latest_undo_id().is_none());
}

#[test]
fn keeping_both_for_a_cross_filesystem_folder_returns_its_children_under_the_chosen_name() {
    let source = tempfile::tempdir().unwrap();
    let device = fs::metadata(source.path()).unwrap().dev();
    let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|path| {
            (fs::metadata(&path).ok()?.dev() != device)
                .then(|| {
                    tempfile::Builder::new()
                        .prefix(".canopa-merge-cross-alias-")
                        .tempdir_in(path)
                        .ok()
                })
                .flatten()
        })
        .unwrap();
    let parent = source.path().join("original");
    fs::create_dir(&parent).unwrap();
    let original = parent.join("project");
    fs::create_dir_all(original.join("nested")).unwrap();
    let seed = source.path().join("copied.txt");
    fs::write(&seed, b"older copied work").unwrap();
    let history = source.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed],
                destination: original.join("nested"),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(result.failures.is_empty());
    let current = source.path().join("moved");
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::remove_dir(&parent).unwrap();
    symlink(destination.path(), &parent).unwrap();
    fs::create_dir_all(original.join("nested")).unwrap();
    fs::write(original.join("nested/newer.txt"), b"newer folder work").unwrap();
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            if question.destination == original {
                UndoChoice::Merge
            } else {
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    let returned = original.join("nested (2)/copied.txt");
    assert_eq!(fs::read(&returned).unwrap(), b"older copied work");
    assert_eq!(
        fs::read(original.join("nested/newer.txt")).unwrap(),
        b"newer folder work"
    );
    drop(engine);
    let engine = OperationEngine::open(history).unwrap();
    engine.undo(&cancel).unwrap();
    assert!(!returned.exists());
    assert_eq!(
        fs::read(original.join("nested/newer.txt")).unwrap(),
        b"newer folder work"
    );
}

#[test]
fn a_cross_filesystem_merge_returns_disjoint_nested_and_empty_folders_before_retrying_a_collision()
{
    use std::os::unix::fs::PermissionsExt;
    let source = tempfile::tempdir().unwrap();
    let device = fs::metadata(source.path()).unwrap().dev();
    let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|path| {
            (fs::metadata(&path).ok()?.dev() != device)
                .then(|| {
                    tempfile::Builder::new()
                        .prefix(".canopa-merge-cross-folders-")
                        .tempdir_in(path)
                        .ok()
                })
                .flatten()
        })
        .expect("two writable filesystems are required");
    let parent = source.path().join("original");
    fs::create_dir(&parent).unwrap();
    let original = parent.join("project");
    fs::create_dir(&original).unwrap();
    fs::create_dir(original.join("a-empty")).unwrap();
    fs::create_dir_all(original.join("b-nested/deep")).unwrap();
    fs::set_permissions(original.join("b-nested"), fs::Permissions::from_mode(0o750)).unwrap();
    fs::write(original.join("b-nested/deep/item.txt"), b"nested work").unwrap();
    fs::write(original.join("z.txt"), b"incoming last child").unwrap();
    let current = source.path().join("moved");
    let history = source.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::remove_dir(&parent).unwrap();
    symlink(destination.path(), &parent).unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("z.txt"), b"newer destination work").unwrap();
    let destination_identity = fs::metadata(&original).unwrap().ino();
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert!(result.remaining > 0);
    assert!(original.join("a-empty").is_dir());
    assert_eq!(fs::read_dir(original.join("a-empty")).unwrap().count(), 0);
    assert_eq!(
        fs::metadata(original.join("b-nested"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o750
    );
    assert_eq!(
        fs::read(original.join("b-nested/deep/item.txt")).unwrap(),
        b"nested work"
    );
    assert!(!current.join("b-nested").exists());
    assert!(!current.join("a-empty").exists());
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.destination, original.join("z.txt"));
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    assert_eq!(fs::metadata(&original).unwrap().ino(), destination_identity);
    assert_eq!(
        fs::read(original.join("z.txt")).unwrap(),
        b"newer destination work"
    );
    assert_eq!(
        fs::read(original.join("z (2).txt")).unwrap(),
        b"incoming last child"
    );
}

#[test]
fn a_merge_returns_files_and_raw_links_across_filesystems_and_preserves_older_copy_undo() {
    let source = tempfile::tempdir().unwrap();
    let device = fs::metadata(source.path()).unwrap().dev();
    let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|path| {
            (fs::metadata(&path).ok()?.dev() != device)
                .then(|| {
                    tempfile::Builder::new()
                        .prefix(".canopa-merge-cross-")
                        .tempdir_in(path)
                        .ok()
                })
                .flatten()
        })
        .expect("two writable filesystems are required");
    assert_ne!(device, fs::metadata(destination.path()).unwrap().dev());
    let parent = source.path().join("original");
    fs::create_dir(&parent).unwrap();
    let original = parent.join("project");
    fs::create_dir(&original).unwrap();
    let seed = source.path().join("a.txt");
    fs::write(&seed, b"earlier copied child").unwrap();
    let history = source.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed.clone()],
                destination: original.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    fs::write(original.join("z.txt"), b"incoming last child").unwrap();
    let raw_name = std::ffi::OsString::from_vec(b"raw-link-\xff".to_vec());
    let raw_target = std::ffi::OsString::from_vec(b"target-\xfe".to_vec());
    symlink(&raw_target, original.join(&raw_name)).unwrap();
    let current = source.path().join("moved");
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::remove_dir(&parent).unwrap();
    symlink(destination.path(), &parent).unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("z.txt"), b"newer destination work").unwrap();
    let destination_identity = fs::metadata(&original).unwrap().ino();
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert!(result.remaining > 0);
    assert_eq!(
        fs::read(original.join("a.txt")).unwrap(),
        b"earlier copied child"
    );
    assert_eq!(
        fs::read_link(original.join(&raw_name)).unwrap().as_os_str(),
        raw_target
    );
    assert!(!current.join("a.txt").exists());
    assert!(fs::symlink_metadata(current.join(&raw_name)).is_err());
    assert_eq!(
        fs::read(current.join("z.txt")).unwrap(),
        b"incoming last child"
    );
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    let mut questions = 0;
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            questions += 1;
            assert_eq!(question.destination, original.join("z.txt"));
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(questions, 1);
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    assert_eq!(fs::metadata(&original).unwrap().ino(), destination_identity);
    assert_eq!(
        fs::read(original.join("z.txt")).unwrap(),
        b"newer destination work"
    );
    assert_eq!(
        fs::read(original.join("z (2).txt")).unwrap(),
        b"incoming last child"
    );
    engine.undo(&cancel).unwrap();
    assert!(!original.join("a.txt").exists());
    assert_eq!(fs::read(seed).unwrap(), b"earlier copied child");
    assert_eq!(
        fs::read(original.join("z (2).txt")).unwrap(),
        b"incoming last child"
    );
}
