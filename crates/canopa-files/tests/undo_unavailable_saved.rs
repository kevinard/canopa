use canopa_files::{
    Cancellation,
    operations::{OperationEngine, UndoChoice, UndoItem, UndoSource},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::{fs, io, path::PathBuf};

struct Fixture {
    root: tempfile::TempDir,
    engine: OperationEngine,
    history: PathBuf,
    source: PathBuf,
    current: PathBuf,
    saved: PathBuf,
}
impl Fixture {
    fn new(kind: TransferKind) -> Self {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        let source = source.join("report.txt");
        let current = destination.join("report.txt");
        fs::write(&source, b"incoming document").unwrap();
        fs::write(&current, b"saved previous destination").unwrap();
        let history = root.path().join("state/history");
        let engine = OperationEngine::open(&history).unwrap();
        let cancel = Cancellation::default();
        let result = engine
            .transfer(
                &TransferRequest {
                    kind,
                    sources: vec![source.clone()],
                    destination,
                },
                &cancel,
                |_| TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
            )
            .unwrap();
        assert!(result.failures.is_empty());
        let saved = engine.recovery_items(None, &cancel).unwrap().items[0]
            .entry
            .as_ref()
            .unwrap()
            .path
            .clone();
        Self {
            root,
            engine,
            history,
            source,
            current,
            saved,
        }
    }
}

#[test]
fn an_unavailable_saved_version_can_be_skipped_cancelled_and_restored_after_it_reappears() {
    let mut fixture = Fixture::new(TransferKind::Copy);
    let cancel = Cancellation::default();
    let external = fixture.root.path().join("saved version elsewhere");
    fs::rename(&fixture.saved, &external).unwrap();
    assert!(fixture.engine.undo(&cancel).is_err());
    let skipped = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::UnavailableRetained);
            assert!(question.existing.is_some());
            assert_eq!(question.destination, fixture.current);
            assert!(!question.can_use_saved_version());
            assert!(!question.can_replace());
            match question.incoming {
                UndoItem::Unavailable { path, kind, .. } => {
                    assert_eq!(path, fixture.saved);
                    assert_eq!(kind, io::ErrorKind::NotFound);
                }
                _ => panic!("a missing saved version has no invented entry"),
            }
            UndoChoice::Skip
        })
        .unwrap();
    assert_eq!(skipped.completed, 0);
    assert_eq!(skipped.remaining, 1);
    assert_eq!(skipped.skipped, vec![fixture.current.clone()]);
    assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming document");
    drop(fixture.engine);
    fixture.engine = OperationEngine::open(&fixture.history).unwrap();
    let cancelled = fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Cancel)
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.remaining, 1);
    fs::rename(&external, &fixture.saved).unwrap();
    fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            // Moving a saved file out and back changes its recorded version.
            assert_eq!(question.source, UndoSource::ChangedRetained);
            UndoChoice::UseSavedVersion
        })
        .unwrap();
    assert_eq!(
        fs::read(&fixture.current).unwrap(),
        b"saved previous destination"
    );
    assert_eq!(fs::read(&fixture.source).unwrap(), b"incoming document");
    assert!(fixture.engine.latest_undo_id().is_none());
    assert!(
        fixture
            .engine
            .interrupted_operations(&cancel)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn finishing_an_unavailable_saved_version_preserves_its_recovery_record_through_expiry() {
    use canopa_files::operations::OperationRequest;
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let mut fixture = Fixture::new(kind);
        let cancel = Cancellation::default();
        let external = fixture.root.path().join("saved elsewhere");
        fs::rename(&fixture.saved, &external).unwrap();
        let finished = fixture
            .engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.source, UndoSource::UnavailableRetained);
                UndoChoice::FinishStep
            })
            .unwrap();
        assert_eq!(finished.completed, 0);
        assert_eq!(finished.remaining, 0);
        assert!(fixture.engine.latest_undo_id().is_none());
        assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming document");
        assert_eq!(fixture.source.exists(), kind == TransferKind::Copy);
        let page = fixture.engine.recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(!page.items[0].held_for_undo);
        assert!(page.items[0].entry.is_none());
        drop(fixture.engine);
        fs::rename(&external, &fixture.saved).unwrap();
        fixture.engine = OperationEngine::open(&fixture.history).unwrap();
        for index in 0..101 {
            fixture
                .engine
                .execute(
                    OperationRequest::CreateFolder {
                        path: fixture.root.path().join(format!("folder-{index}")),
                    },
                    &cancel,
                )
                .unwrap();
        }
        let page = fixture.engine.recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(!page.items[0].held_for_undo);
        assert!(page.items[0].entry.is_some());
        let recovered = fixture.root.path().join("recovered.txt");
        fixture
            .engine
            .recover_copy(&page.items[0], &recovered, &cancel)
            .unwrap();
        assert_eq!(fs::read(&recovered).unwrap(), b"saved previous destination");
        assert_eq!(
            fs::read(&fixture.saved).unwrap(),
            b"saved previous destination"
        );
        assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming document");
    }
}

#[test]
fn a_moved_item_can_return_while_its_unavailable_saved_destination_remains_retryable() {
    let mut fixture = Fixture::new(TransferKind::Move);
    let cancel = Cancellation::default();
    let external = fixture.root.path().join("saved elsewhere");
    fs::rename(&fixture.saved, &external).unwrap();
    let returned = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::UnavailableRetained);
            assert!(question.can_return_item());
            UndoChoice::ReturnItem
        })
        .unwrap();
    assert_eq!(returned.completed, 1);
    assert_eq!(returned.remaining, 1);
    assert_eq!(returned.unrestored_previous, vec![fixture.current.clone()]);
    assert_eq!(fs::read(&fixture.source).unwrap(), b"incoming document");
    assert!(!fixture.current.exists());
    let page = fixture.engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].held_for_undo);
    assert!(page.items[0].entry.is_none());
    drop(fixture.engine);
    fixture.engine = OperationEngine::open(&fixture.history).unwrap();
    let skipped = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert!(!question.can_return_item());
            UndoChoice::Skip
        })
        .unwrap();
    assert_eq!(skipped.remaining, 1);
    fs::write(&fixture.source, b"newer returned document edits").unwrap();
    fs::rename(&external, &fixture.saved).unwrap();
    let restored = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedRetained);
            UndoChoice::UseSavedVersion
        })
        .unwrap();
    assert_eq!(restored.completed, 1);
    assert_eq!(restored.remaining, 0);
    assert_eq!(
        fs::read(&fixture.source).unwrap(),
        b"newer returned document edits"
    );
    assert_eq!(
        fs::read(&fixture.current).unwrap(),
        b"saved previous destination"
    );
    assert!(
        fixture
            .engine
            .interrupted_operations(&cancel)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn returning_only_a_moved_item_with_keep_both_keeps_older_history_on_the_returned_file() {
    use canopa_files::operations::OperationRequest;
    let fixture = Fixture::new(TransferKind::Move);
    let cancel = Cancellation::default();
    fixture.engine.undo(&cancel).unwrap();
    let before = fixture.root.path().join("before.txt");
    fixture
        .engine
        .execute(
            OperationRequest::Rename {
                source: fixture.source.clone(),
                destination: before.clone(),
            },
            &cancel,
        )
        .unwrap();
    let older = fixture
        .engine
        .execute(
            OperationRequest::Rename {
                source: before.clone(),
                destination: fixture.source.clone(),
            },
            &cancel,
        )
        .unwrap();
    let moved = fixture
        .engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.source.clone()],
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
    let saved = fixture.engine.recovery_items(None, &cancel).unwrap().items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .clone();
    let external = fixture.root.path().join("saved elsewhere");
    fs::rename(&saved, &external).unwrap();
    fs::write(&fixture.source, b"newer original-location work").unwrap();
    let outcome = fixture
        .engine
        .undo_interactive(None, &cancel, |question| match question.source {
            UndoSource::UnavailableRetained => UndoChoice::ReturnItem,
            UndoSource::Current => UndoChoice::KeepBoth,
            source => panic!("unexpected review {source:?}"),
        })
        .unwrap();
    assert_eq!(outcome.completed, 1);
    assert_eq!(outcome.remaining, 1);
    let returned = fixture.source.with_file_name("report (2).txt");
    assert_eq!(fs::read(&returned).unwrap(), b"incoming document");
    assert_eq!(
        fs::read(&fixture.source).unwrap(),
        b"newer original-location work"
    );
    fs::rename(&external, &saved).unwrap();
    fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::UseSavedVersion)
        .unwrap();
    assert_eq!(fixture.engine.latest_undo_id(), Some(older.id));
    fixture.engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&before).unwrap(), b"incoming document");
    assert_eq!(
        fs::read(&fixture.source).unwrap(),
        b"newer original-location work"
    );
    assert_eq!(
        fs::read(&fixture.current).unwrap(),
        b"saved previous destination"
    );
}

#[test]
fn changed_unavailable_saved_observations_refuse_finish_without_consuming_history() {
    use std::os::unix::fs::PermissionsExt;
    for case in 0..3 {
        if case == 2 && rustix::process::geteuid().is_root() {
            continue;
        }
        let fixture = Fixture::new(TransferKind::Copy);
        let cancel = Cancellation::default();
        let external = fixture.root.path().join("saved elsewhere");
        let stage = fixture.saved.parent().unwrap().to_owned();
        let held_stage = fixture.root.path().join("holding folder elsewhere");
        if case == 2 {
            fs::set_permissions(&stage, fs::Permissions::from_mode(0o000)).unwrap();
        } else {
            fs::rename(&fixture.saved, &external).unwrap();
        }
        let result = fixture.engine.undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::UnavailableRetained);
            match case {
                0 => fs::rename(&external, &fixture.saved).unwrap(),
                1 => {
                    fs::rename(&stage, &held_stage).unwrap();
                    fs::create_dir(&stage).unwrap();
                    fs::write(&fixture.saved, b"unrelated replacement folder contents").unwrap();
                }
                _ => fs::set_permissions(&stage, fs::Permissions::from_mode(0o700)).unwrap(),
            }
            UndoChoice::FinishStep
        });
        if case == 2 {
            fs::set_permissions(&stage, fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
        assert!(fixture.engine.latest_undo_id().is_some());
        assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming document");
        let page = fixture.engine.recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page.items[0].held_for_undo);
        if case == 1 {
            assert_eq!(
                fs::read(&fixture.saved).unwrap(),
                b"unrelated replacement folder contents"
            );
            assert!(page.items[0].entry.is_none());
            fs::rename(&stage, fixture.root.path().join("foreign folder preserved")).unwrap();
            fs::rename(&held_stage, &stage).unwrap();
            fs::rename(&external, &fixture.saved).unwrap();
        }
        fixture
            .engine
            .undo_interactive(None, &cancel, |_| UndoChoice::UseSavedVersion)
            .unwrap();
        assert_eq!(
            fs::read(&fixture.current).unwrap(),
            b"saved previous destination"
        );
    }
}

#[test]
fn finishing_with_a_replaced_saved_folder_preserves_both_its_record_and_the_foreign_folder() {
    let fixture = Fixture::new(TransferKind::Copy);
    let cancel = Cancellation::default();
    let stage = fixture.saved.parent().unwrap().to_owned();
    let original_stage = fixture.root.path().join("original holding folder");
    fs::rename(&stage, &original_stage).unwrap();
    fs::create_dir(&stage).unwrap();
    fs::write(&fixture.saved, b"foreign folder contents").unwrap();
    let finished = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::UnavailableRetained);
            assert!(question.incoming.entry().is_none());
            assert!(!question.can_return_item());
            UndoChoice::FinishStep
        })
        .unwrap();
    assert_eq!(finished.completed, 0);
    assert_eq!(finished.unrestored_previous, vec![fixture.current.clone()]);
    assert_eq!(
        fs::read(&fixture.saved).unwrap(),
        b"foreign folder contents"
    );
    let page = fixture.engine.recovery_items(None, &cancel).unwrap();
    assert!(!page.items[0].held_for_undo);
    assert!(page.items[0].entry.is_none());
    let foreign = fixture.root.path().join("foreign folder preserved");
    fs::rename(&stage, &foreign).unwrap();
    fs::rename(&original_stage, &stage).unwrap();
    let page = fixture.engine.recovery_items(None, &cancel).unwrap();
    let recovered = fixture.root.path().join("recovered.txt");
    fixture
        .engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"saved previous destination");
    assert_eq!(
        fs::read(foreign.join("previous")).unwrap(),
        b"foreign folder contents"
    );
    assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming document");
}

#[test]
fn a_saved_version_reappearing_during_a_return_location_conflict_refuses_partial_return() {
    let fixture = Fixture::new(TransferKind::Move);
    let cancel = Cancellation::default();
    let external = fixture.root.path().join("saved elsewhere");
    fs::rename(&fixture.saved, &external).unwrap();
    fs::write(&fixture.source, b"newer original-location work").unwrap();
    let result = fixture
        .engine
        .undo_interactive(None, &cancel, |question| match question.source {
            UndoSource::UnavailableRetained => UndoChoice::ReturnItem,
            UndoSource::Current => {
                fs::rename(&external, &fixture.saved).unwrap();
                UndoChoice::KeepBoth
            }
            source => panic!("unexpected question {source:?}"),
        });
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
    assert_eq!(fs::read(&fixture.current).unwrap(), b"incoming document");
    assert_eq!(
        fs::read(&fixture.source).unwrap(),
        b"newer original-location work"
    );
    assert!(!fixture.source.with_file_name("report (2).txt").exists());
    assert!(fixture.engine.latest_undo_id().is_some());
    assert!(
        fixture
            .engine
            .interrupted_operations(&cancel)
            .unwrap()
            .is_empty()
    );
    let outcome = fixture
        .engine
        .undo_interactive(None, &cancel, |question| match question.source {
            UndoSource::ChangedRetained => UndoChoice::UseSavedVersion,
            UndoSource::Current => UndoChoice::KeepBoth,
            source => panic!("unexpected question {source:?}"),
        })
        .unwrap();
    assert_eq!(outcome.remaining, 0);
    assert_eq!(
        fs::read(fixture.source.with_file_name("report (2).txt")).unwrap(),
        b"incoming document"
    );
    assert_eq!(
        fs::read(&fixture.current).unwrap(),
        b"saved previous destination"
    );
}

#[test]
fn restarting_a_killed_source_only_return_keeps_saved_restoration_and_older_copy_undo() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let root = tempfile::Builder::new()
        .prefix("canopa-saved-return-crash-")
        .tempdir()
        .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::undo_unavailable_saved::crash_source_only_return_helper",
        ])
        .env("CANOPA_FILES_SAVED_RETURN_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(!root.path().join("undo-returned").exists());
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let returned = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::UnavailableRetained);
            if question.can_return_item() {
                UndoChoice::ReturnItem
            } else {
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert_eq!(returned.remaining, 100);
    let locations: Vec<(PathBuf, PathBuf)> =
        serde_json::from_slice(&fs::read(root.path().join("saved-locations.json")).unwrap())
            .unwrap();
    assert_eq!(locations.len(), 100);
    for (saved, external) in locations {
        fs::rename(external, saved).unwrap();
    }
    engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedRetained);
            UndoChoice::UseSavedVersion
        })
        .unwrap();
    for index in 0..100 {
        let name = format!("file-{index:04}");
        assert_eq!(
            fs::read(root.path().join("source").join(&name)).unwrap(),
            b"incoming copied document"
        );
        assert_eq!(
            fs::read(root.path().join("destination").join(&name)).unwrap(),
            b"saved destination"
        );
    }
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read_dir(root.path().join("source")).unwrap().count(), 0);
    for index in 0..100 {
        assert_eq!(
            fs::read(root.path().join("older").join(format!("file-{index:04}"))).unwrap(),
            b"incoming copied document"
        );
    }
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
}

#[test]
#[ignore = "child process fixture; killed after an actual source-only return"]
fn crash_source_only_return_helper() {
    use std::os::unix::fs::MetadataExt;
    let root = PathBuf::from(
        std::env::var_os("CANOPA_FILES_SAVED_RETURN_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-saved-return-crash-")
    );
    let older = root.join("older");
    let source = root.join("source");
    let destination = root.join("destination");
    let external = root.join("external");
    for path in [&older, &source, &destination, &external] {
        fs::create_dir(path).unwrap();
    }
    for index in 0..100 {
        fs::write(
            older.join(format!("file-{index:04}")),
            b"incoming copied document",
        )
        .unwrap();
        fs::write(
            destination.join(format!("file-{index:04}")),
            b"saved destination",
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
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 100);
    let mut locations = Vec::new();
    for (index, item) in page.items.into_iter().enumerate() {
        let saved = item.entry.unwrap().path;
        let elsewhere = external.join(format!("saved-{index}"));
        fs::rename(&saved, &elsewhere).unwrap();
        locations.push((saved, elsewhere));
    }
    // Known fixture locations obtained from public Recovery, used only to make
    // these same saved files available again after the child is killed.
    fs::write(
        root.join("saved-locations.json"),
        serde_json::to_vec(&locations).unwrap(),
    )
    .unwrap();
    let metadata = fs::metadata(destination.join("file-0099")).unwrap();
    let (device, inode) = (metadata.dev(), metadata.ino());
    let returned = source.join("file-0099");
    let ready = std::sync::Arc::new(std::sync::Barrier::new(2));
    let watcher_ready = ready.clone();
    std::thread::spawn(move || {
        watcher_ready.wait();
        while !fs::metadata(&returned)
            .is_ok_and(|metadata| metadata.dev() == device && metadata.ino() == inode)
        {
            std::thread::sleep(std::time::Duration::from_micros(50));
        }
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    ready.wait();
    engine
        .undo_interactive(None, &cancel, |question| {
            assert!(question.can_return_item());
            UndoChoice::ReturnItem
        })
        .unwrap();
    fs::write(root.join("undo-returned"), b"missed crash window").unwrap();
    panic!("the child was not killed during source-only reversal");
}

#[test]
fn source_only_replace_preserves_newer_original_work_while_saved_restoration_remains_pending() {
    let fixture = Fixture::new(TransferKind::Move);
    let cancel = Cancellation::default();
    let external = fixture.root.path().join("saved elsewhere");
    fs::rename(&fixture.saved, &external).unwrap();
    fs::write(&fixture.source, b"newer original-location work").unwrap();
    let outcome = fixture
        .engine
        .undo_interactive(None, &cancel, |question| match question.source {
            UndoSource::UnavailableRetained => UndoChoice::ReturnItem,
            UndoSource::Current => UndoChoice::Replace,
            source => panic!("unexpected question {source:?}"),
        })
        .unwrap();
    assert_eq!(outcome.completed, 1);
    assert_eq!(outcome.remaining, 1);
    assert_eq!(fs::read(&fixture.source).unwrap(), b"incoming document");
    assert!(!fixture.current.exists());
    let page = fixture.engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 2);
    let preserved = page.items.iter().find(|item| !item.held_for_undo).unwrap();
    assert_eq!(preserved.original.as_ref(), Some(&fixture.source));
    assert_eq!(
        fs::read(&preserved.entry.as_ref().unwrap().path).unwrap(),
        b"newer original-location work"
    );
    fs::rename(&external, &fixture.saved).unwrap();
    fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::UseSavedVersion)
        .unwrap();
    let page = fixture.engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    let recovered = fixture.root.path().join("recovered newer work.txt");
    fixture
        .engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(
        fs::read(&recovered).unwrap(),
        b"newer original-location work"
    );
    assert_eq!(fs::read(&fixture.source).unwrap(), b"incoming document");
    assert_eq!(
        fs::read(&fixture.current).unwrap(),
        b"saved previous destination"
    );
}
