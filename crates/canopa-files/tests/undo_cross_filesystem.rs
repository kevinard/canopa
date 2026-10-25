use canopa_files::{
    Cancellation,
    operations::{OperationEngine, UndoChoice, UndoSource},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::{fs, os::unix::fs::MetadataExt, path::PathBuf};

struct Fixture {
    source_root: tempfile::TempDir,
    _destination_root: tempfile::TempDir,
    history: PathBuf,
    engine: OperationEngine,
    older: PathBuf,
    original: PathBuf,
    current: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let source_root = tempfile::tempdir().unwrap();
        let device = fs::metadata(source_root.path()).unwrap().dev();
        let destination_root = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
            .into_iter()
            .find_map(|path| {
                (fs::metadata(&path).ok()?.dev() != device)
                    .then(|| {
                        tempfile::Builder::new()
                            .prefix(".canopa-cross-undo-")
                            .tempdir_in(path)
                            .ok()
                    })
                    .flatten()
            })
            .expect("cross-filesystem fixture requires two writable filesystems");
        assert_ne!(device, fs::metadata(destination_root.path()).unwrap().dev());
        let older = source_root.path().join("older/report.txt");
        let original = source_root.path().join("source/report.txt");
        for path in [older.parent().unwrap(), original.parent().unwrap()] {
            fs::create_dir(path).unwrap();
        }
        fs::write(&older, b"incoming document").unwrap();
        let current = destination_root.path().join("report.txt");
        fs::write(&current, b"previous destination").unwrap();
        let history = source_root.path().join("state/history");
        let engine = OperationEngine::open(&history).unwrap();
        let cancel = Cancellation::default();
        for (kind, input, output) in [
            (TransferKind::Copy, &older, original.parent().unwrap()),
            (TransferKind::Move, &original, destination_root.path()),
        ] {
            let result = engine
                .transfer(
                    &TransferRequest {
                        kind,
                        sources: vec![input.to_owned()],
                        destination: output.to_owned(),
                    },
                    &cancel,
                    |_| TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        apply_to_remaining: false,
                    },
                )
                .unwrap();
            assert!(result.failures.is_empty());
        }
        Self {
            source_root,
            _destination_root: destination_root,
            history,
            engine,
            older,
            original,
            current,
        }
    }
}

#[test]
fn cross_filesystem_move_undo_can_keep_both_at_the_original_location_and_preserve_older_copy_history()
 {
    let mut fixture = Fixture::new();
    let cancel = Cancellation::default();
    fs::write(&fixture.original, b"newer original-location work").unwrap();
    let result = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::Current);
            assert_eq!(question.incoming.path(), fixture.current);
            assert_eq!(question.destination, fixture.original);
            assert_eq!(question.incoming.entry().unwrap().size, 17);
            assert_eq!(question.existing.as_ref().unwrap().size, 28);
            assert!(question.can_replace());
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert_eq!(
        fs::read(&fixture.original).unwrap(),
        b"newer original-location work"
    );
    let returned = fixture.original.with_file_name("report (2).txt");
    assert_eq!(fs::read(&returned).unwrap(), b"incoming document");
    assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
    drop(fixture.engine);
    fixture.engine = OperationEngine::open(&fixture.history).unwrap();
    fixture.engine.undo(&cancel).unwrap();
    assert!(!returned.exists());
    assert_eq!(
        fs::read(&fixture.original).unwrap(),
        b"newer original-location work"
    );
    assert_eq!(fs::read(&fixture.older).unwrap(), b"incoming document");
    assert!(
        fixture
            .engine
            .interrupted_operations(&cancel)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn skipping_a_cross_filesystem_return_preserves_its_destination_copy_and_is_retryable() {
    let mut fixture = Fixture::new();
    let cancel = Cancellation::default();
    fs::write(&fixture.original, b"newer original-location work").unwrap();
    let skipped = fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Skip)
        .unwrap();
    assert_eq!(skipped.remaining, 2);
    assert_eq!(skipped.completed, 0);
    assert_eq!(skipped.skipped, vec![fixture.current.clone()]);
    assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming document");
    assert_eq!(
        fs::read(&fixture.original).unwrap(),
        b"newer original-location work"
    );
    let page = fixture.engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].held_for_undo);
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"previous destination"
    );
    drop(fixture.engine);
    fixture.engine = OperationEngine::open(&fixture.history).unwrap();
    let cancelled = fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Cancel)
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.remaining, 2);
    let outcome = fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    assert_eq!(outcome.remaining, 0);
    assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
    assert_eq!(
        fs::read(fixture.original.with_file_name("report (2).txt")).unwrap(),
        b"incoming document"
    );
}

#[test]
fn cross_filesystem_undo_replace_preserves_newer_work_in_recovery_without_following_links() {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    for link in [false, true] {
        let fixture = Fixture::new();
        let cancel = Cancellation::default();
        let target = fixture
            .source_root
            .path()
            .join(std::ffi::OsString::from_vec(b"newer-target-\xff".to_vec()));
        fs::write(&target, b"newer original-location work").unwrap();
        if link {
            symlink(&target, &fixture.original).unwrap();
        } else {
            fs::copy(&target, &fixture.original).unwrap();
        }
        let returned = fixture
            .engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.source, UndoSource::Current);
                assert!(question.can_replace());
                UndoChoice::Replace
            })
            .unwrap();
        assert_eq!(returned.remaining, 0);
        assert_eq!(fs::read(&fixture.original).unwrap(), b"incoming document");
        assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
        let page = fixture.engine.recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(!page.items[0].held_for_undo);
        assert_eq!(page.items[0].original.as_ref(), Some(&fixture.original));
        fixture.engine.undo(&cancel).unwrap();
        assert!(!fixture.original.exists());
        let recovered = fixture.source_root.path().join("recovered newer work");
        fixture
            .engine
            .recover_copy(&page.items[0], &recovered, &cancel)
            .unwrap();
        if link {
            assert_eq!(fs::read_link(&recovered).unwrap(), target);
        } else {
            assert_eq!(
                fs::read(&recovered).unwrap(),
                b"newer original-location work"
            );
        }
        assert_eq!(fs::read(&target).unwrap(), b"newer original-location work");
        assert_eq!(fs::read(&fixture.older).unwrap(), b"incoming document");
        assert!(
            fixture
                .engine
                .interrupted_operations(&cancel)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn skipping_one_cross_filesystem_return_still_undoes_other_batch_items() {
    let fixture = Fixture::new();
    let cancel = Cancellation::default();
    fixture.engine.undo(&cancel).unwrap();
    let other = fixture.original.with_file_name("other.txt");
    let other_current = fixture.current.with_file_name("other.txt");
    fs::write(&other, b"other incoming document").unwrap();
    fs::write(&other_current, b"other previous destination").unwrap();
    let result = fixture
        .engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone(), other.clone()],
                destination: fixture.current.parent().unwrap().to_owned(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: true,
            },
        )
        .unwrap();
    assert!(result.failures.is_empty());
    fs::write(&other, b"newer other original-location work").unwrap();
    let skipped = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.destination, other);
            UndoChoice::Skip
        })
        .unwrap();
    assert_eq!(skipped.remaining, 2);
    assert_eq!(skipped.completed, 2);
    assert_eq!(skipped.skipped, vec![other_current.clone()]);
    assert_eq!(
        fs::read(&other_current).unwrap(),
        b"other incoming document"
    );
    assert_eq!(
        fs::read(&other).unwrap(),
        b"newer other original-location work"
    );
    assert_eq!(fs::read(&fixture.original).unwrap(), b"incoming document");
    assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
    fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    assert_eq!(
        fs::read(other.with_file_name("other (2).txt")).unwrap(),
        b"other incoming document"
    );
    assert_eq!(
        fs::read(&other_current).unwrap(),
        b"other previous destination"
    );
}

#[test]
fn changing_the_original_occupant_or_parent_during_cross_filesystem_review_refuses_return() {
    use std::io;
    for replace_parent in [false, true] {
        let fixture = Fixture::new();
        let cancel = Cancellation::default();
        fs::write(&fixture.original, b"newer original-location work").unwrap();
        let parent = fixture.original.parent().unwrap();
        let held = fixture.source_root.path().join("original folder elsewhere");
        let result = fixture.engine.undo_interactive(None, &cancel, |_| {
            if replace_parent {
                fs::rename(parent, &held).unwrap();
                fs::create_dir(parent).unwrap();
            }
            fs::write(&fixture.original, b"changed during the question").unwrap();
            UndoChoice::Replace
        });
        assert_eq!(
            result.unwrap_err().kind(),
            if replace_parent {
                io::ErrorKind::NotFound
            } else {
                io::ErrorKind::WouldBlock
            }
        );
        assert_eq!(
            fs::read(&fixture.original).unwrap(),
            b"changed during the question"
        );
        assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming document");
        assert!(
            fixture
                .engine
                .interrupted_operations(&cancel)
                .unwrap()
                .is_empty()
        );
        let page = fixture.engine.recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page.items[0].held_for_undo);
        if replace_parent {
            fs::rename(
                parent,
                fixture
                    .source_root
                    .path()
                    .join("unrelated folder preserved"),
            )
            .unwrap();
            fs::rename(&held, parent).unwrap();
        }
        fixture
            .engine
            .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
            .unwrap();
        assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
    }
}

#[test]
fn restarting_a_killed_cross_filesystem_return_preserves_conflict_choices_and_older_history() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    for replace in [false, true] {
        let root = tempfile::Builder::new()
            .prefix("canopa-cross-undo-crash-")
            .tempdir()
            .unwrap();
        let device = fs::metadata(root.path()).unwrap().dev();
        let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
            .into_iter()
            .find_map(|path| {
                (fs::metadata(&path).ok()?.dev() != device)
                    .then(|| {
                        tempfile::Builder::new()
                            .prefix(".canopa-cross-undo-")
                            .tempdir_in(path)
                            .ok()
                    })
                    .flatten()
            })
            .expect("cross-filesystem fixture requires two writable filesystems");
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "crash_cross_filesystem_return_helper",
            ])
            .env("CANOPA_FILES_CROSS_ROOT", root.path())
            .env("CANOPA_FILES_CROSS_DESTINATION", destination.path())
            .env(
                "CANOPA_FILES_CROSS_MODE",
                if replace { "replace" } else { "both" },
            )
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        assert!(!root.path().join("undo-returned").exists());
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.source, UndoSource::Current);
                if replace {
                    UndoChoice::Replace
                } else {
                    UndoChoice::KeepBoth
                }
            })
            .unwrap();
        for index in 0..100 {
            let name = format!("file-{index:04}");
            let original = root.path().join("source").join(&name);
            let returned = if replace {
                original.clone()
            } else {
                original.with_file_name(format!("{name} (2)"))
            };
            assert_eq!(fs::read(&returned).unwrap(), b"incoming copied document");
            if !replace {
                assert_eq!(
                    fs::read(&original).unwrap(),
                    b"newer original-location work"
                );
            }
            assert_eq!(
                fs::read(destination.path().join(&name)).unwrap(),
                b"previous destination"
            );
        }
        engine.undo(&cancel).unwrap();
        for index in 0..100 {
            let name = format!("file-{index:04}");
            let original = root.path().join("source").join(&name);
            if replace {
                assert!(!original.exists());
            } else {
                assert_eq!(
                    fs::read(&original).unwrap(),
                    b"newer original-location work"
                );
                assert!(!original.with_file_name(format!("{name} (2)")).exists());
            }
            assert_eq!(
                fs::read(root.path().join("older").join(&name)).unwrap(),
                b"incoming copied document"
            );
        }
        let page = engine.recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), if replace { 100 } else { 0 });
        if replace {
            for item in &page.items {
                assert!(!item.held_for_undo);
                assert_eq!(
                    fs::read(&item.entry.as_ref().unwrap().path).unwrap(),
                    b"newer original-location work"
                );
            }
            let recovered = root.path().join("recovered.txt");
            engine
                .recover_copy(&page.items[0], &recovered, &cancel)
                .unwrap();
            assert_eq!(
                fs::read(recovered).unwrap(),
                b"newer original-location work"
            );
        }
        assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    }
}

#[test]
#[ignore = "child process fixture; killed after an actual cross-filesystem conflict return"]
fn crash_cross_filesystem_return_helper() {
    let root =
        PathBuf::from(std::env::var_os("CANOPA_FILES_CROSS_ROOT").expect("parent fixture root"));
    let destination = PathBuf::from(
        std::env::var_os("CANOPA_FILES_CROSS_DESTINATION").expect("parent fixture destination"),
    );
    let mode = std::env::var("CANOPA_FILES_CROSS_MODE").unwrap();
    assert!(matches!(
        mode.as_str(),
        "both" | "replace" | "reviewed" | "saved"
    ));
    let replace = mode == "replace";
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-cross-undo-crash-")
    );
    assert!(
        destination
            .file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b".canopa-cross-undo-")
    );
    assert_ne!(
        fs::metadata(&root).unwrap().dev(),
        fs::metadata(&destination).unwrap().dev()
    );
    let older = root.join("older");
    let source = root.join("source");
    fs::create_dir(&older).unwrap();
    fs::create_dir(&source).unwrap();
    for index in 0..100 {
        fs::write(
            older.join(format!("file-{index:04}")),
            b"incoming copied document",
        )
        .unwrap();
        fs::write(
            destination.join(format!("file-{index:04}")),
            b"previous destination",
        )
        .unwrap();
    }
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let cancel = Cancellation::default();
    for (kind, input, output) in [
        (TransferKind::Copy, &older, &source),
        (TransferKind::Move, &source, &destination),
    ] {
        let result = engine
            .transfer(
                &TransferRequest {
                    kind,
                    sources: (0..100)
                        .map(|index| input.join(format!("file-{index:04}")))
                        .collect(),
                    destination: output.clone(),
                },
                &cancel,
                |_| TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: true,
                },
            )
            .unwrap();
        assert!(result.failures.is_empty());
    }
    for index in 0..100 {
        fs::write(
            source.join(format!("file-{index:04}")),
            b"newer original-location work",
        )
        .unwrap();
    }
    if mode == "reviewed" {
        for index in 0..100 {
            fs::write(
                destination.join(format!("file-{index:04}")),
                b"reviewed destination edits",
            )
            .unwrap();
        }
    } else if mode == "saved" {
        for index in 0..100 {
            fs::rename(
                destination.join(format!("file-{index:04}")),
                destination.join(format!("elsewhere-{index:04}")),
            )
            .unwrap();
        }
    }
    let returned = if mode == "saved" {
        destination.join("file-0099")
    } else {
        source.join(if replace {
            "file-0099"
        } else {
            "file-0099 (2)"
        })
    };
    let expected: &[u8] = match mode.as_str() {
        "reviewed" => b"reviewed destination edits",
        "saved" => b"previous destination",
        _ => b"incoming copied document",
    };
    let ready = std::sync::Arc::new(std::sync::Barrier::new(2));
    let watcher_ready = ready.clone();
    std::thread::spawn(move || {
        watcher_ready.wait();
        while !fs::read(&returned).is_ok_and(|bytes| bytes == expected) {
            std::thread::sleep(std::time::Duration::from_micros(50));
        }
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    ready.wait();
    engine
        .undo_interactive(None, &cancel, |question| {
            if mode == "saved" {
                UndoChoice::RestorePreviousDestination
            } else if question.source == UndoSource::ChangedCrossMove {
                UndoChoice::ReturnItem
            } else if replace {
                UndoChoice::Replace
            } else {
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    fs::write(root.join("undo-returned"), b"missed crash window").unwrap();
    panic!("the child was not killed during cross-filesystem return");
}

#[test]
fn cross_filesystem_conflict_return_preserves_raw_link_targets_and_their_contents() {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    let fixture = Fixture::new();
    let cancel = Cancellation::default();
    fixture.engine.undo(&cancel).unwrap();
    fs::remove_file(&fixture.original).unwrap();
    let raw_target = PathBuf::from(std::ffi::OsString::from_vec(b"target-\xff".to_vec()));
    let target = fixture.original.parent().unwrap().join(&raw_target);
    fs::write(&target, b"link target contents").unwrap();
    symlink(&raw_target, &fixture.original).unwrap();
    let moved = fixture
        .engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: fixture.current.parent().unwrap().to_owned(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    fs::write(&fixture.original, b"newer original-location work").unwrap();
    fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(
                question.incoming.entry().unwrap().kind,
                canopa_files::browse::EntryKind::Symlink
            );
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(
        fs::read_link(fixture.original.with_file_name("report (2).txt")).unwrap(),
        raw_target
    );
    assert_eq!(fs::read(&target).unwrap(), b"link target contents");
    assert_eq!(
        fs::read(&fixture.original).unwrap(),
        b"newer original-location work"
    );
    assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
}

#[test]
fn a_skipped_cross_filesystem_return_blocks_an_older_same_filesystem_frame_at_that_path() {
    let fixture = Fixture::new();
    let cancel = Cancellation::default();
    fixture.engine.undo(&cancel).unwrap();
    let local = fixture.current.parent().unwrap().join("local");
    fs::create_dir(&local).unwrap();
    let local_source = local.join("report.txt");
    fs::write(&local_source, b"earlier same-filesystem source").unwrap();
    let result = fixture
        .engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![local_source.clone(), fixture.original.clone()],
                destination: fixture.current.parent().unwrap().to_owned(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: true,
            },
        )
        .unwrap();
    assert!(result.failures.is_empty());
    assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming document");
    assert!(!local_source.exists());
    fs::write(&fixture.original, b"newer original-location work").unwrap();
    let mut questions = 0;
    let skipped = fixture
        .engine
        .undo_interactive(None, &cancel, |_| {
            questions += 1;
            UndoChoice::Skip
        })
        .unwrap();
    assert_eq!(
        questions, 1,
        "an older frame cannot act on an unresolved newer version"
    );
    assert_eq!(skipped.remaining, 3);
    assert_eq!(skipped.completed, 0);
    assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming document");
    assert_eq!(
        fs::read(&fixture.original).unwrap(),
        b"newer original-location work"
    );
    assert!(!local_source.exists());
    let preserved = fixture.source_root.path().join("newer work preserved.txt");
    fs::rename(&fixture.original, &preserved).unwrap();
    fixture.engine.undo(&cancel).unwrap();
    assert_eq!(
        fs::read(&local_source).unwrap(),
        b"earlier same-filesystem source"
    );
    assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
    assert_eq!(fs::read(&fixture.original).unwrap(), b"incoming document");
    assert_eq!(
        fs::read(preserved).unwrap(),
        b"newer original-location work"
    );
}

#[test]
fn a_skipped_cross_filesystem_child_keeps_an_older_moved_parent_at_its_current_location() {
    use canopa_files::transfers::TransferEvent;
    let fixture = Fixture::new();
    let cancel = Cancellation::default();
    fixture.engine.undo(&cancel).unwrap();
    let local = fixture.current.parent().unwrap().join("local/project");
    let incoming = fixture.source_root.path().join("incoming/project");
    fs::create_dir_all(&local).unwrap();
    fs::create_dir_all(&incoming).unwrap();
    fs::write(local.join("child.txt"), b"earlier directory contents").unwrap();
    fs::write(incoming.join("child.txt"), b"incoming child contents").unwrap();
    // An unsupported special node stays at the source, so this successful leaf
    // move has no removed-source-directory step to reverse before its conflict.
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        incoming.join("unsupported"),
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let result = fixture
        .engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![local.clone(), incoming.clone()],
                destination: fixture.current.parent().unwrap().to_owned(),
            },
            &cancel,
            |event| TransferReply::Resolve {
                choice: if matches!(event, TransferEvent::Conflict { folders: true, .. }) {
                    ConflictChoice::Merge
                } else {
                    ConflictChoice::Replace
                },
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(!result.failures.is_empty());
    let current = fixture.current.parent().unwrap().join("project");
    assert_eq!(
        fs::read(current.join("child.txt")).unwrap(),
        b"incoming child contents"
    );
    assert!(!local.exists());
    fs::write(incoming.join("child.txt"), b"newer original child").unwrap();
    let skipped = fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Skip)
        .unwrap();
    assert_eq!(skipped.remaining, 3);
    assert_eq!(skipped.completed, 0);
    assert_eq!(
        fs::read(current.join("child.txt")).unwrap(),
        b"incoming child contents"
    );
    assert!(
        !local.exists(),
        "an older directory return must not move the skipped child's copy away"
    );
    let page = fixture.engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].held_for_undo);
    let preserved = fixture.source_root.path().join("newer child preserved.txt");
    fs::rename(incoming.join("child.txt"), &preserved).unwrap();
    fixture.engine.undo(&cancel).unwrap();
    assert_eq!(
        fs::read(local.join("child.txt")).unwrap(),
        b"earlier directory contents"
    );
    assert_eq!(
        fs::read(incoming.join("child.txt")).unwrap(),
        b"incoming child contents"
    );
    assert_eq!(fs::read(preserved).unwrap(), b"newer original child");
    assert!(!current.exists());
}

#[test]
fn a_changed_cross_filesystem_destination_can_be_skipped_cancelled_and_returned_for_safe_cleanup() {
    for replace_node in [false, true] {
        let mut fixture = Fixture::new();
        let cancel = Cancellation::default();
        let external = fixture
            .current
            .with_file_name("original destination copy elsewhere");
        if replace_node {
            fs::rename(&fixture.current, &external).unwrap();
        }
        fs::write(&fixture.current, b"selected destination edits").unwrap();
        assert!(fixture.engine.undo(&cancel).is_err());
        let skipped = fixture
            .engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.incoming.path(), fixture.current);
                assert_eq!(question.destination, fixture.original);
                assert!(question.can_return_item());
                UndoChoice::Skip
            })
            .unwrap();
        assert_eq!(skipped.completed, 0);
        assert_eq!(skipped.remaining, 2);
        assert_eq!(
            fs::read(&fixture.current).unwrap(),
            b"selected destination edits"
        );
        assert!(!fixture.original.exists());
        drop(fixture.engine);
        fixture.engine = OperationEngine::open(&fixture.history).unwrap();
        let cancelled = fixture
            .engine
            .undo_interactive(None, &cancel, |_| UndoChoice::Cancel)
            .unwrap();
        assert!(cancelled.cancelled);
        assert_eq!(cancelled.remaining, 2);
        let returned = fixture
            .engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.incoming.path(), fixture.current);
                assert!(question.can_return_item());
                UndoChoice::ReturnItem
            })
            .unwrap();
        assert_eq!(returned.remaining, 0);
        assert_eq!(returned.completed, 2);
        assert_eq!(
            fs::read(&fixture.original).unwrap(),
            b"selected destination edits"
        );
        assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
        if replace_node {
            assert_eq!(fs::read(&external).unwrap(), b"incoming document");
        }
        let mut asked = false;
        let older = fixture
            .engine
            .undo_interactive(None, &cancel, |question| {
                asked = true;
                assert_eq!(question.source, UndoSource::ChangedCopy);
                UndoChoice::Skip
            })
            .unwrap();
        assert!(
            asked,
            "returning edits does not authorize an older Copy Undo to delete them"
        );
        assert_eq!(older.remaining, 1);
        assert_eq!(
            fs::read(&fixture.original).unwrap(),
            b"selected destination edits"
        );
        assert_eq!(fs::read(&fixture.older).unwrap(), b"incoming document");
    }
}

#[test]
fn keeping_a_changed_cross_filesystem_destination_finishes_both_steps_and_protects_its_backup() {
    use canopa_files::operations::OperationRequest;
    let mut fixture = Fixture::new();
    let cancel = Cancellation::default();
    fs::write(&fixture.current, b"destination edits kept here").unwrap();
    let kept = fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepItem)
        .unwrap();
    assert_eq!(kept.completed, 0);
    assert_eq!(kept.remaining, 0);
    assert_eq!(kept.kept, vec![fixture.current.clone()]);
    assert!(!fixture.original.exists());
    assert_eq!(
        fs::read(&fixture.current).unwrap(),
        b"destination edits kept here"
    );
    let page = fixture.engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].held_for_undo);
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"previous destination"
    );
    drop(fixture.engine);
    fixture.engine = OperationEngine::open(&fixture.history).unwrap();
    for index in 0..101 {
        fixture
            .engine
            .execute(
                OperationRequest::CreateFolder {
                    path: fixture.source_root.path().join(format!("later-{index}")),
                },
                &cancel,
            )
            .unwrap();
    }
    let page = fixture.engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    let recovered = fixture
        .source_root
        .path()
        .join("recovered previous destination.txt");
    fixture
        .engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"previous destination");
    assert_eq!(
        fs::read(&fixture.current).unwrap(),
        b"destination edits kept here"
    );
}

#[test]
fn an_unavailable_cross_filesystem_destination_can_be_skipped_cancelled_and_returned_after_reappearance()
 {
    let mut fixture = Fixture::new();
    let cancel = Cancellation::default();
    let external = fixture.current.with_file_name("destination copy elsewhere");
    fs::rename(&fixture.current, &external).unwrap();
    let skipped = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert!(question.incoming.entry().is_none());
            assert_eq!(question.incoming.path(), fixture.current);
            assert!(!question.can_return_item());
            UndoChoice::Skip
        })
        .unwrap();
    assert_eq!(skipped.completed, 0);
    assert_eq!(skipped.remaining, 2);
    assert!(!fixture.original.exists());
    assert_eq!(fs::read(&external).unwrap(), b"incoming document");
    drop(fixture.engine);
    fixture.engine = OperationEngine::open(&fixture.history).unwrap();
    let cancelled = fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Cancel)
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.remaining, 2);
    fs::rename(&external, &fixture.current).unwrap();
    fixture.engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&fixture.original).unwrap(), b"incoming document");
    assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
    fixture.engine.undo(&cancel).unwrap();
    assert!(!fixture.original.exists());
    assert_eq!(fs::read(&fixture.older).unwrap(), b"incoming document");
}

#[test]
fn finishing_an_unavailable_cross_filesystem_destination_protects_its_saved_version() {
    use std::os::unix::fs::PermissionsExt;
    for unreadable in [false, true] {
        let mut fixture = Fixture::new();
        let cancel = Cancellation::default();
        let external = fixture
            .current
            .with_file_name("unavailable destination elsewhere");
        if unreadable {
            fs::set_permissions(&fixture.current, fs::Permissions::from_mode(0o0)).unwrap();
        } else {
            fs::rename(&fixture.current, &external).unwrap();
        }
        let outcome = fixture
            .engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.source, UndoSource::UnavailableCrossMove);
                assert!(!question.can_return_item());
                UndoChoice::FinishStep
            })
            .unwrap();
        assert_eq!(outcome.completed, 0);
        assert_eq!(outcome.remaining, 0);
        assert_eq!(outcome.dismissed, vec![fixture.current.clone()]);
        assert_eq!(outcome.unrestored_previous, vec![fixture.current.clone()]);
        assert!(!fixture.original.exists());
        if unreadable {
            fs::set_permissions(&fixture.current, fs::Permissions::from_mode(0o600)).unwrap();
            assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming document");
        } else {
            assert!(!fixture.current.exists());
            assert_eq!(fs::read(&external).unwrap(), b"incoming document");
        }
        drop(fixture.engine);
        fixture.engine = OperationEngine::open(&fixture.history).unwrap();
        let page = fixture.engine.recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(!page.items[0].held_for_undo);
        let recovered = fixture
            .source_root
            .path()
            .join("saved previous recovered.txt");
        fixture
            .engine
            .recover_copy(&page.items[0], &recovered, &cancel)
            .unwrap();
        assert_eq!(fs::read(recovered).unwrap(), b"previous destination");
    }
}

#[test]
fn a_missing_cross_filesystem_destination_can_restore_only_its_saved_previous_version() {
    let mut fixture = Fixture::new();
    let cancel = Cancellation::default();
    let external = fixture.current.with_file_name("incoming elsewhere");
    fs::rename(&fixture.current, &external).unwrap();
    fs::write(&fixture.original, b"new original location work").unwrap();
    let outcome = fixture.engine.undo_interactive(None, &cancel, |question| {
        assert_eq!(question.source, UndoSource::UnavailableCrossMove);
        assert!(question.can_restore_previous_destination());
        assert_eq!(question.saved_previous.as_ref().unwrap().size, 20);
        UndoChoice::RestorePreviousDestination
    });
    if outcome.is_err() {
        fixture.source_root.disable_cleanup(true);
        fixture._destination_root.disable_cleanup(true);
        eprintln!(
            "retained saved-only source: {}",
            fixture.source_root.path().display()
        );
        eprintln!(
            "retained saved-only destination: {}",
            fixture._destination_root.path().display()
        );
    }
    let outcome = outcome.unwrap();
    assert_eq!(outcome.completed, 1);
    assert_eq!(outcome.remaining, 0);
    assert_eq!(outcome.dismissed, vec![fixture.current.clone()]);
    assert!(outcome.unrestored_previous.is_empty());
    assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
    assert_eq!(
        fs::read(&fixture.original).unwrap(),
        b"new original location work"
    );
    assert_eq!(fs::read(&external).unwrap(), b"incoming document");
    drop(fixture.engine);
    fixture.engine = OperationEngine::open(&fixture.history).unwrap();
    assert!(
        fixture
            .engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
    let mut asked = false;
    let older = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            asked = true;
            assert_eq!(question.source, UndoSource::ChangedCopy);
            UndoChoice::Skip
        })
        .unwrap();
    assert!(
        asked,
        "a saved-only restoration grants no ownership of newer original-location work"
    );
    assert_eq!(older.remaining, 1);
    assert_eq!(
        fs::read(&fixture.original).unwrap(),
        b"new original location work"
    );
}

#[test]
fn cross_filesystem_destination_choices_reject_changes_during_review() {
    for case in [
        "return",
        "keep",
        "later-conflict",
        "finish",
        "restore",
        "saved-changed",
    ] {
        let fixture = Fixture::new();
        let cancel = Cancellation::default();
        let external = fixture.current.with_file_name("held incoming");
        let unavailable = matches!(case, "finish" | "restore" | "saved-changed");
        if unavailable {
            fs::rename(&fixture.current, &external).unwrap();
        } else {
            fs::write(&fixture.current, b"first reviewed version").unwrap();
        }
        let saved = fixture.engine.recovery_items(None, &cancel).unwrap().items[0]
            .entry
            .as_ref()
            .unwrap()
            .path
            .clone();
        if case == "later-conflict" {
            fs::write(&fixture.original, b"newer original occupant").unwrap();
        }
        let result = fixture.engine.undo_interactive(None, &cancel, |question| {
            if case == "later-conflict" && question.source == UndoSource::ChangedCrossMove {
                return UndoChoice::ReturnItem;
            }
            if case == "saved-changed" {
                fs::write(&saved, b"saved changed during review").unwrap();
            } else {
                fs::write(&fixture.current, b"destination changed during review").unwrap();
            }
            match case {
                "keep" => UndoChoice::KeepItem,
                "finish" => UndoChoice::FinishStep,
                "restore" | "saved-changed" => UndoChoice::RestorePreviousDestination,
                "later-conflict" => UndoChoice::Replace,
                _ => UndoChoice::ReturnItem,
            }
        });
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "{case}"
        );
        if case == "saved-changed" {
            assert!(!fixture.current.exists());
        } else {
            assert_eq!(
                fs::read(&fixture.current).unwrap(),
                b"destination changed during review",
                "{case}"
            );
        }
        if case == "later-conflict" {
            assert_eq!(
                fs::read(&fixture.original).unwrap(),
                b"newer original occupant"
            );
        } else {
            assert!(!fixture.original.exists(), "{case}");
        }
        if unavailable {
            assert_eq!(fs::read(&external).unwrap(), b"incoming document");
        }
        assert!(
            fixture.engine.recovery_items(None, &cancel).unwrap().items[0].held_for_undo,
            "{case}"
        );
        let retry = fixture
            .engine
            .undo_interactive(None, &cancel, |_| UndoChoice::Skip)
            .unwrap();
        assert_eq!(retry.remaining, 2, "{case}");
    }
}

#[test]
fn cross_filesystem_review_does_not_authorize_cleanup_of_later_destination_edits() {
    let fixture = Fixture::new();
    let cancel = Cancellation::default();
    fs::write(&fixture.current, b"selected return version").unwrap();
    let saved = fixture.engine.recovery_items(None, &cancel).unwrap().items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .clone();
    fs::write(&saved, b"edited saved predecessor").unwrap();
    let mut questions = Vec::new();
    let outcome = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            questions.push(question.source);
            match question.source {
                UndoSource::ChangedCrossMove => UndoChoice::ReturnItem,
                UndoSource::ChangedRetained => {
                    assert_eq!(
                        fs::read(&fixture.original).unwrap(),
                        b"selected return version"
                    );
                    fs::write(&fixture.current, b"edits after the return").unwrap();
                    UndoChoice::UseSavedVersion
                }
                UndoSource::PreviousDestination => UndoChoice::Skip,
                _ => panic!("unexpected review"),
            }
        })
        .unwrap();
    assert_eq!(
        questions,
        vec![
            UndoSource::ChangedCrossMove,
            UndoSource::ChangedRetained,
            UndoSource::PreviousDestination
        ]
    );
    assert_eq!(outcome.completed, 1);
    assert_eq!(outcome.remaining, 1);
    assert_eq!(
        fs::read(&fixture.original).unwrap(),
        b"selected return version"
    );
    assert_eq!(
        fs::read(&fixture.current).unwrap(),
        b"edits after the return"
    );
    let outcome = fixture
        .engine
        .undo_interactive(None, &cancel, |question| match question.source {
            UndoSource::ChangedRetained => UndoChoice::UseSavedVersion,
            UndoSource::PreviousDestination => UndoChoice::Replace,
            _ => panic!("a completed source return must not be repeated"),
        })
        .unwrap();
    assert_eq!(outcome.remaining, 0);
    assert_eq!(
        fs::read(&fixture.current).unwrap(),
        b"edited saved predecessor"
    );
    let page = fixture.engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"edits after the return"
    );
}

#[test]
fn reviewed_cross_filesystem_returns_and_saved_only_restorations_survive_process_kill() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    for mode in ["reviewed", "saved"] {
        let root = tempfile::Builder::new()
            .prefix("canopa-cross-undo-crash-")
            .tempdir()
            .unwrap();
        let device = fs::metadata(root.path()).unwrap().dev();
        let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
            .into_iter()
            .find_map(|path| {
                (fs::metadata(&path).ok()?.dev() != device)
                    .then(|| {
                        tempfile::Builder::new()
                            .prefix(".canopa-cross-undo-")
                            .tempdir_in(path)
                            .ok()
                    })
                    .flatten()
            })
            .expect("two actual writable filesystems");
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "crash_cross_filesystem_return_helper",
            ])
            .env("CANOPA_FILES_CROSS_ROOT", root.path())
            .env("CANOPA_FILES_CROSS_DESTINATION", destination.path())
            .env("CANOPA_FILES_CROSS_MODE", mode)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        assert!(!root.path().join("undo-returned").exists());
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let finished = engine
            .undo_interactive(None, &cancel, |question| match question.source {
                UndoSource::ChangedCrossMove => UndoChoice::ReturnItem,
                UndoSource::Current => UndoChoice::KeepBoth,
                UndoSource::UnavailableCrossMove => UndoChoice::RestorePreviousDestination,
                _ => panic!("unexpected replay review: {:?}", question.source),
            })
            .unwrap();
        assert_eq!(finished.remaining, 0);
        for index in 0..100 {
            let name = format!("file-{index:04}");
            let original = root.path().join("source").join(&name);
            assert_eq!(
                fs::read(&original).unwrap(),
                b"newer original-location work"
            );
            assert_eq!(
                fs::read(destination.path().join(&name)).unwrap(),
                b"previous destination"
            );
            let returned = original.with_file_name(format!("{name} (2)"));
            if mode == "reviewed" {
                assert_eq!(fs::read(returned).unwrap(), b"reviewed destination edits");
            } else {
                assert!(!returned.exists());
                assert_eq!(
                    fs::read(destination.path().join(format!("elsewhere-{index:04}"))).unwrap(),
                    b"incoming copied document"
                );
            }
        }
        let mut questions = 0;
        let older = engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.source, UndoSource::ChangedCopy);
                questions += 1;
                UndoChoice::Skip
            })
            .unwrap();
        assert_eq!(
            questions, 100,
            "older Copy Undo must review all edited or unrelated originals after {mode}"
        );
        assert_eq!(older.remaining, 100);
        assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    }
}

#[test]
fn changed_cross_filesystem_links_return_raw_targets_and_special_nodes_offer_keep() {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    for kind in ["link", "folder", "fifo"] {
        let fixture = Fixture::new();
        let cancel = Cancellation::default();
        let external = fixture.current.with_file_name("incoming copy elsewhere");
        fs::rename(&fixture.current, &external).unwrap();
        let raw_target = PathBuf::from(std::ffi::OsString::from_vec(
            b"unavailable-target-\xff".to_vec(),
        ));
        match kind {
            "link" => symlink(&raw_target, &fixture.current).unwrap(),
            "folder" => {
                fs::create_dir(&fixture.current).unwrap();
                fs::write(fixture.current.join("child"), b"replacement child").unwrap();
            }
            _ => rustix::fs::mkfifoat(
                rustix::fs::CWD,
                &fixture.current,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            )
            .unwrap(),
        }
        let outcome = fixture
            .engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.source, UndoSource::ChangedCrossMove);
                assert_eq!(question.can_return_item(), kind == "link");
                if kind == "link" {
                    UndoChoice::ReturnItem
                } else {
                    UndoChoice::KeepItem
                }
            })
            .unwrap();
        assert_eq!(outcome.remaining, 0);
        assert_eq!(fs::read(&external).unwrap(), b"incoming document");
        if kind == "link" {
            assert_eq!(fs::read_link(&fixture.original).unwrap(), raw_target);
            assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
        } else {
            assert!(!fixture.original.exists());
            assert_eq!(outcome.kept, vec![fixture.current.clone()]);
            assert!(!fixture.engine.recovery_items(None, &cancel).unwrap().items[0].held_for_undo);
            if kind == "folder" {
                assert_eq!(
                    fs::read(fixture.current.join("child")).unwrap(),
                    b"replacement child"
                );
            }
        }
    }
}

#[test]
fn v7_cross_filesystem_history_upgrades_and_reviews_edits_without_losing_older_copy_protection() {
    let mut fixture = Fixture::new();
    drop(fixture.engine);
    // Forward SourceRemoved records omit the new optional review field. This
    // constructs the old schema input, then observes only public Undo and files.
    {
        let input = rusqlite::Connection::open(&fixture.history).unwrap();
        input.execute_batch("DROP TABLE pending_saved_destinations; DROP TABLE directory_archives; DROP TABLE directory_members; DROP TABLE directory_returns; DROP TABLE directory_scopes; DROP TABLE directory_sequences;").unwrap();
        input.pragma_update(None, "user_version", 7).unwrap();
    }
    fixture.engine = OperationEngine::open(&fixture.history).unwrap();
    fs::write(&fixture.current, b"edits reviewed after upgrade").unwrap();
    let cancel = Cancellation::default();
    fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedCrossMove);
            UndoChoice::ReturnItem
        })
        .unwrap();
    assert_eq!(
        fs::read(&fixture.original).unwrap(),
        b"edits reviewed after upgrade"
    );
    assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
    drop(fixture.engine);
    fixture.engine = OperationEngine::open(&fixture.history).unwrap();
    let older = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedCopy);
            UndoChoice::Skip
        })
        .unwrap();
    assert_eq!(older.remaining, 1);
    assert_eq!(
        fs::read(&fixture.original).unwrap(),
        b"edits reviewed after upgrade"
    );
}

#[test]
fn a_saved_only_return_binds_its_paired_copy_and_accepts_only_formatting_changes() {
    for tamper in [false, true] {
        let mut fixture = Fixture::new();
        let cancel = Cancellation::default();
        fixture.source_root.disable_cleanup(true);
        fixture._destination_root.disable_cleanup(true);
        eprintln!(
            "retained paired source: {}",
            fixture.source_root.path().display()
        );
        eprintln!(
            "retained paired destination: {}",
            fixture._destination_root.path().display()
        );
        let external = fixture.current.with_file_name("incoming held");
        fs::rename(&fixture.current, &external).unwrap();
        fs::write(&fixture.original, b"New original work").unwrap();
        let db = rusqlite::Connection::open(&fixture.history).unwrap();
        db.execute_batch("CREATE TRIGGER stop_saved_phase BEFORE UPDATE ON pending WHEN json_extract(new.body,'$.returning.phase')='returning' BEGIN SELECT RAISE(ABORT,'private saved-only phase stop'); END;").unwrap();
        assert!(
            fixture
                .engine
                .undo_interactive(None, &cancel, |_| UndoChoice::RestorePreviousDestination)
                .is_err()
        );
        drop(fixture.engine);
        let (id,ordinal,body):(u64,u64,String)=db.query_row("SELECT operation,ordinal,body FROM changes WHERE json_extract(body,'$.kind')='copied' AND json_extract(body,'$.retained') IS NOT NULL ORDER BY operation DESC,ordinal DESC LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        let mut copy: serde_json::Value = serde_json::from_str(&body).unwrap();
        if tamper {
            let size = copy["fingerprint"]["size"].as_u64().unwrap();
            copy["fingerprint"]["size"] = (size + 1).into();
        }
        db.execute(
            "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
            rusqlite::params![id, ordinal, serde_json::to_string_pretty(&copy).unwrap()],
        )
        .unwrap();
        db.execute_batch("DROP TRIGGER stop_saved_phase;").unwrap();
        fixture.engine = OperationEngine::open(&fixture.history).unwrap();
        let result = fixture.engine.retry_recovery(&cancel);
        assert_eq!(result.is_err(), tamper);
        if tamper {
            assert!(!fixture.current.exists());
            db.execute(
                "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                rusqlite::params![id, ordinal, body],
            )
            .unwrap();
            fixture.engine.retry_recovery(&cancel).unwrap();
        }
        assert_eq!(fs::read(&fixture.current).unwrap(), b"previous destination");
        assert_eq!(fs::read(&fixture.original).unwrap(), b"New original work");
        assert_eq!(fs::read(external).unwrap(), b"incoming document");
        fixture.source_root.disable_cleanup(false);
        fixture._destination_root.disable_cleanup(false);
    }
}
