use canopa_files::{
    Cancellation,
    operations::{OperationEngine, UndoChoice, UndoSource},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::{fs, path::PathBuf};

#[test]
fn a_killed_source_only_merge_resumes_without_repeating_saved_approval_or_returned_children() {
    use std::{
        os::unix::{ffi::OsStringExt, process::ExitStatusExt},
        process::Command,
    };
    let root = tempfile::Builder::new()
        .prefix("canopa-merge-saved-resume-")
        .tempdir()
        .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "crash_source_only_merge_helper"])
        .env("CANOPA_MERGE_SAVED_RESUME_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let original = root.path().join("item");
    let current = root.path().join("destination/item");
    assert_eq!(fs::read(original.join("file-0000")).unwrap(), b"incoming 0");
    assert!(current.is_dir());
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    let outcome = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::Current);
            assert_eq!(question.destination, original.join("file-0099"));
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(outcome.remaining, 1);
    assert!(!current.exists());
    for index in 0..99 {
        assert_eq!(
            fs::read(original.join(format!("file-{index:04}"))).unwrap(),
            format!("incoming {index}").as_bytes()
        );
        assert!(!original.join(format!("file-{index:04} (2)")).exists());
    }
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].held_for_undo);
    assert!(page.items[0].entry.is_none());
    let held = root.path().join("saved held");
    let saved = PathBuf::from(std::ffi::OsString::from_vec(
        fs::read(root.path().join("saved-location")).unwrap(),
    ));
    assert!(saved.starts_with(root.path()));
    fs::rename(held, saved).unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::UseSavedVersion)
        .unwrap();
    assert_eq!(fs::read(current).unwrap(), b"saved previous destination");
}

#[test]
#[ignore = "child fixture; killed after an actual source-only Merge child return"]
fn crash_source_only_merge_helper() {
    use std::{os::unix::fs::MetadataExt, time::Duration};
    let root = PathBuf::from(
        std::env::var_os("CANOPA_MERGE_SAVED_RESUME_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-merge-saved-resume-")
    );
    let original = root.join("item");
    let destination = root.join("destination");
    fs::create_dir(&destination).unwrap();
    let current = destination.join("item");
    fs::write(&original, b"original moved file").unwrap();
    fs::write(&current, b"saved previous destination").unwrap();
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![original.clone()],
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
    fs::write(
        root.join("saved-location"),
        saved.as_os_str().as_encoded_bytes(),
    )
    .unwrap();
    fs::rename(saved, root.join("saved held")).unwrap();
    fs::rename(&current, root.join("original moved file elsewhere")).unwrap();
    fs::create_dir(&current).unwrap();
    fs::create_dir(&original).unwrap();
    for index in 0..100 {
        fs::write(
            current.join(format!("file-{index:04}")),
            format!("incoming {index}"),
        )
        .unwrap();
    }
    fs::write(original.join("file-0099"), b"newer final child").unwrap();
    let metadata = fs::metadata(current.join("file-0000")).unwrap();
    let (device, inode) = (metadata.dev(), metadata.ino());
    let returned = original.join("file-0000");
    std::thread::spawn(move || {
        while !fs::metadata(&returned)
            .is_ok_and(|metadata| metadata.dev() == device && metadata.ino() == inode)
        {
            std::thread::sleep(Duration::from_micros(50));
        }
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    engine
        .undo_interactive(None, &cancel, |question| {
            if matches!(
                question.source,
                UndoSource::ChangedMove | UndoSource::UnavailableRetained
            ) {
                UndoChoice::ReturnItem
            } else if question.can_merge() {
                UndoChoice::Merge
            } else {
                loop {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        })
        .unwrap();
    panic!("the child was not killed during its Merge");
}

struct Fixture {
    root: tempfile::TempDir,
    engine: OperationEngine,
    history: PathBuf,
    original: PathBuf,
    current: PathBuf,
    saved: PathBuf,
}

impl Fixture {
    fn leave_saved_review_pending(&self, held: &std::path::Path) {
        let result = self
            .engine
            .undo_interactive(None, &Cancellation::default(), |question| {
                if question.source == UndoSource::ChangedMove {
                    UndoChoice::ReturnItem
                } else if question.can_merge() {
                    UndoChoice::Merge
                } else if question.source == UndoSource::UnavailableMergeRetained {
                    UndoChoice::Skip
                } else {
                    fs::rename(&self.saved, held).unwrap();
                    UndoChoice::KeepBoth
                }
            })
            .unwrap();
        assert_eq!(result.remaining, 1);
        assert!(self.current.is_dir());
        assert_eq!(fs::read_dir(&self.current).unwrap().count(), 0);
    }
    fn return_contents_only(&self, held: &std::path::Path) {
        fs::rename(&self.saved, held).unwrap();
        let result = self
            .engine
            .undo_interactive(None, &Cancellation::default(), |question| {
                if matches!(
                    question.source,
                    UndoSource::ChangedMove | UndoSource::UnavailableRetained
                ) {
                    UndoChoice::ReturnItem
                } else if question.can_merge() {
                    UndoChoice::Merge
                } else {
                    UndoChoice::KeepBoth
                }
            })
            .unwrap();
        assert_eq!(result.remaining, 1);
        assert!(!self.current.exists());
    }
    fn new() -> Self {
        Self::with_saved_copy(false)
    }
    fn with_saved_copy(copied: bool) -> Self {
        Self::prepare(copied, true)
    }
    fn prepare(copied: bool, directory: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("item");
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        let current = destination.join("item");
        fs::write(&original, b"original moved file").unwrap();
        let history = root.path().join("state/history");
        let engine = OperationEngine::open(&history).unwrap();
        let cancel = Cancellation::default();
        if copied {
            let seed = root.path().join("seed");
            fs::create_dir(&seed).unwrap();
            fs::write(seed.join("item"), b"saved previous destination").unwrap();
            let result = engine
                .transfer(
                    &TransferRequest {
                        kind: TransferKind::Copy,
                        sources: vec![seed.join("item")],
                        destination: destination.clone(),
                    },
                    &cancel,
                    |_| TransferReply::Continue,
                )
                .unwrap();
            assert!(result.failures.is_empty());
        } else {
            fs::write(&current, b"saved previous destination").unwrap();
        }
        let result = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![original.clone()],
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
        if directory {
            fs::rename(&current, root.path().join("original moved file elsewhere")).unwrap();
            fs::create_dir(&current).unwrap();
            fs::write(current.join("a.txt"), b"first returned child").unwrap();
            fs::write(current.join("z.txt"), b"last returned child").unwrap();
            fs::create_dir(&original).unwrap();
            fs::write(original.join("z.txt"), b"newer destination z").unwrap();
        }
        Self {
            root,
            engine,
            history,
            original,
            current,
            saved,
        }
    }
}

#[test]
fn unavailable_saved_review_during_merge_can_skip_cancel_and_reject_stale_decisions() {
    let fixture = Fixture::new();
    let held = fixture.root.path().join("saved elsewhere");
    fixture.leave_saved_review_pending(&held);
    let cancel = Cancellation::default();
    assert!(fixture.engine.undo(&cancel).is_err());
    assert!(
        fixture
            .engine
            .interrupted_operations(&cancel)
            .unwrap()
            .is_empty()
    );
    for choice in [UndoChoice::ReturnItem, UndoChoice::FinishStep] {
        for folder in [
            &fixture.current,
            &fixture.original,
            fixture.saved.parent().unwrap(),
        ] {
            let work = folder.join("work added during saved review");
            let error = fixture
                .engine
                .undo_interactive(None, &cancel, |question| {
                    assert_eq!(question.source, UndoSource::UnavailableMergeRetained);
                    fs::write(&work, b"newer work").unwrap();
                    choice
                })
                .unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
            assert_eq!(fs::read(&work).unwrap(), b"newer work");
            assert!(fixture.current.is_dir());
            fs::remove_file(work).unwrap();
        }
    }
    let cancelled = fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Cancel)
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.remaining, 1);
    let cancelled = fixture
        .engine
        .undo_interactive(None, &cancel, |_| {
            cancel.cancel();
            UndoChoice::ReturnItem
        })
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.remaining, 1);
    let result = fixture
        .engine
        .undo_interactive(None, &Cancellation::default(), |question| {
            assert_eq!(question.source, UndoSource::UnavailableMergeRetained);
            UndoChoice::ReturnItem
        })
        .unwrap();
    assert_eq!(result.remaining, 1);
    assert!(!fixture.current.exists());
}

#[test]
fn finishing_an_unavailable_merge_saved_version_preserves_it_and_the_empty_source() {
    use canopa_files::operations::OperationRequest;
    let fixture = Fixture::new();
    let held = fixture.root.path().join("saved elsewhere");
    fixture.leave_saved_review_pending(&held);
    let cancel = Cancellation::default();
    let result = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::UnavailableMergeRetained);
            UndoChoice::FinishStep
        })
        .unwrap();
    assert_eq!(result.completed, 0);
    assert_eq!(result.remaining, 0);
    assert_eq!(result.dismissed, vec![fixture.current.clone()]);
    assert_eq!(result.unrestored_previous, vec![fixture.current.clone()]);
    assert!(fixture.current.is_dir());
    for index in 0..101 {
        fixture
            .engine
            .execute(
                OperationRequest::CreateFolder {
                    path: fixture.root.path().join(format!("later-{index}")),
                },
                &cancel,
            )
            .unwrap();
    }
    fs::rename(held, &fixture.saved).unwrap();
    let page = fixture.engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].held_for_undo);
    let recovered = fixture.root.path().join("recovered saved version");
    fixture
        .engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"saved previous destination");
    assert_eq!(
        fs::read(fixture.original.join("a.txt")).unwrap(),
        b"first returned child"
    );
}

#[test]
fn a_changed_saved_version_is_reviewed_after_an_active_merge_returns_its_children() {
    let fixture = Fixture::new();
    let cancel = Cancellation::default();
    let mut reviewed = false;
    let outcome = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            if question.source == UndoSource::ChangedMove {
                UndoChoice::ReturnItem
            } else if question.can_merge() {
                UndoChoice::Merge
            } else if question.source == UndoSource::ChangedRetained {
                reviewed = true;
                UndoChoice::UseSavedVersion
            } else {
                fs::write(&fixture.saved, b"edited saved version").unwrap();
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert!(reviewed);
    assert_eq!(outcome.remaining, 0);
    assert_eq!(fs::read(&fixture.current).unwrap(), b"edited saved version");
    assert_eq!(
        fs::read(fixture.original.join("z (2).txt")).unwrap(),
        b"last returned child"
    );
}

#[test]
fn a_deferred_saved_destination_keeps_its_slot_when_a_later_file_returns_under_a_new_name() {
    use canopa_files::operations::OperationRequest;
    let fixture = Fixture::new();
    let held_saved = fixture.root.path().join("saved elsewhere");
    fixture.return_contents_only(&held_saved);
    fs::write(&fixture.current, b"later occupant").unwrap();
    let cancel = Cancellation::default();
    fixture
        .engine
        .execute(
            OperationRequest::Rename {
                source: fixture.current.clone(),
                destination: fixture.root.path().join("later file held"),
            },
            &cancel,
        )
        .unwrap();
    fs::write(&fixture.current, b"newest occupant").unwrap();
    fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    fs::rename(held_saved, &fixture.saved).unwrap();
    let result = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(
                question.destination, fixture.current,
                "saved restoration owns a destination slot, not its later occupant"
            );
            if question.source == UndoSource::ChangedRetained {
                UndoChoice::UseSavedVersion
            } else {
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert_eq!(fs::read(&fixture.current).unwrap(), b"newest occupant");
    assert_eq!(
        fs::read(fixture.current.with_file_name("item (2)")).unwrap(),
        b"later occupant"
    );
    assert_eq!(
        fs::read(fixture.current.with_file_name("item (3)")).unwrap(),
        b"saved previous destination"
    );
}

#[test]
fn deferred_saved_restoration_preserves_older_undo_after_an_unrelated_file_alias() {
    use canopa_files::operations::OperationRequest;
    for directory in [false, true] {
        let fixture = Fixture::prepare(true, directory);
        let held = fixture.root.path().join("saved elsewhere");
        fixture.return_contents_only(&held);
        fs::write(&fixture.current, b"later occupant").unwrap();
        let cancel = Cancellation::default();
        fixture
            .engine
            .execute(
                OperationRequest::Rename {
                    source: fixture.current.clone(),
                    destination: fixture.root.path().join("held occupant"),
                },
                &cancel,
            )
            .unwrap();
        fs::write(&fixture.current, b"newest occupant").unwrap();
        fixture
            .engine
            .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
            .unwrap();
        fs::rename(held, &fixture.saved).unwrap();
        fixture
            .engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.destination, fixture.current);
                if question.source == UndoSource::ChangedRetained {
                    UndoChoice::UseSavedVersion
                } else {
                    UndoChoice::KeepBoth
                }
            })
            .unwrap();
        assert_eq!(
            fs::read(fixture.current.with_file_name("item (3)")).unwrap(),
            b"saved previous destination"
        );
        fixture.engine.undo(&cancel).unwrap();
        assert!(!fixture.current.with_file_name("item (3)").exists());
        assert_eq!(fs::read(&fixture.current).unwrap(), b"newest occupant");
        assert_eq!(
            fs::read(fixture.current.with_file_name("item (2)")).unwrap(),
            b"later occupant"
        );
    }
}

#[test]
fn deferred_restoration_follows_the_parent_and_unproved_legacy_work_stays_exportable() {
    use canopa_files::operations::OperationRequest;
    for legacy in [false, true] {
        let mut fixture = Fixture::prepare(true, !legacy);
        let held_saved = fixture.root.path().join("saved elsewhere");
        fixture.return_contents_only(&held_saved);
        if legacy {
            drop(fixture.engine);
            let input = rusqlite::Connection::open(&fixture.history).unwrap();
            input.execute_batch("DROP TABLE pending_saved_destinations; UPDATE changes SET body=json_remove(body,'$.retained.stage.parent_identity'),bindings=NULL WHERE json_extract(body,'$.kind')='retained-only'; PRAGMA user_version=10;").unwrap();
            drop(input);
            fixture.engine = OperationEngine::open(&fixture.history).unwrap();
        }
        let parent = fixture.current.parent().unwrap();
        let cancel = Cancellation::default();
        fixture
            .engine
            .execute(
                OperationRequest::Rename {
                    source: parent.to_owned(),
                    destination: fixture.root.path().join("parent held"),
                },
                &cancel,
            )
            .unwrap();
        fs::create_dir(parent).unwrap();
        fs::write(parent.join("foreign.txt"), b"foreign parent work").unwrap();
        fixture
            .engine
            .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
            .unwrap();
        let relocated = fixture.root.path().join("destination (2)");
        let stage_name = fixture.saved.parent().unwrap().file_name().unwrap();
        fs::rename(
            held_saved,
            relocated
                .join(stage_name)
                .join(fixture.saved.file_name().unwrap()),
        )
        .unwrap();
        let result = fixture.engine.undo_interactive(None, &cancel, |question| {
            assert_eq!(question.destination, relocated.join("item"));
            UndoChoice::UseSavedVersion
        });
        if legacy {
            let error = result.unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
            assert!(error.to_string().contains("no original history-file proof"));
            assert!(!relocated.join("item").exists());
            let protected = relocated
                .join(stage_name)
                .join(fixture.saved.file_name().unwrap());
            assert_eq!(fs::read(&protected).unwrap(), b"saved previous destination");
            let item = fixture
                .engine
                .recovery_items(None, &cancel)
                .unwrap()
                .items
                .into_iter()
                .find(|item| item.entry.as_ref().is_some_and(|e| e.path == protected))
                .unwrap();
            let export = fixture.root.path().join("rescued legacy saved version");
            fixture
                .engine
                .recover_copy(&item, &export, &cancel)
                .unwrap();
            assert_eq!(fs::read(&export).unwrap(), b"saved previous destination");
            fixture.engine.undo(&cancel).unwrap();
            assert!(!export.exists());
            assert_eq!(fs::read(&protected).unwrap(), b"saved previous destination");
            drop(fixture.engine);
            fixture.engine = OperationEngine::open(&fixture.history).unwrap();
            assert!(
                fixture
                    .engine
                    .undo_interactive(None, &cancel, |_| UndoChoice::UseSavedVersion)
                    .is_err()
            );
            assert_eq!(
                fs::read(parent.join("foreign.txt")).unwrap(),
                b"foreign parent work"
            );
            assert_eq!(fs::read(&protected).unwrap(), b"saved previous destination");
            assert!(!relocated.join("item").exists());
            continue;
        }
        let result = result.unwrap();
        assert_eq!(result.remaining, 0);
        assert_eq!(
            fs::read(relocated.join("item")).unwrap(),
            b"saved previous destination"
        );
        fixture.engine.undo(&cancel).unwrap();
        assert!(!relocated.join("item").exists());
        assert_eq!(
            fs::read(parent.join("foreign.txt")).unwrap(),
            b"foreign parent work"
        );
    }
}

#[test]
fn whole_saved_container_unavailability_and_reappearance_are_guarded_before_merge() {
    for reappear in [false, true] {
        let fixture = Fixture::new();
        let stage = fixture.saved.parent().unwrap();
        let held = fixture.root.path().join("saved container elsewhere");
        fs::rename(stage, &held).unwrap();
        let cancel = Cancellation::default();
        let result = fixture.engine.undo_interactive(None, &cancel, |question| {
            if matches!(
                question.source,
                UndoSource::ChangedMove | UndoSource::UnavailableRetained
            ) {
                UndoChoice::ReturnItem
            } else if question.can_merge() {
                if reappear {
                    fs::rename(&held, stage).unwrap();
                }
                UndoChoice::Merge
            } else {
                UndoChoice::KeepBoth
            }
        });
        if reappear {
            assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
            assert_eq!(
                fs::read(fixture.current.join("a.txt")).unwrap(),
                b"first returned child"
            );
            assert!(!fixture.original.join("a.txt").exists());
            assert!(
                fixture
                    .engine
                    .interrupted_operations(&cancel)
                    .unwrap()
                    .is_empty()
            );
            fixture
                .engine
                .undo_interactive(None, &cancel, |question| {
                    if question.source == UndoSource::ChangedMove {
                        UndoChoice::ReturnItem
                    } else if question.can_merge() {
                        UndoChoice::Merge
                    } else {
                        UndoChoice::KeepBoth
                    }
                })
                .unwrap();
        } else {
            assert_eq!(result.unwrap().remaining, 1);
            assert!(!fixture.current.exists());
            fs::rename(held, stage).unwrap();
            fixture
                .engine
                .undo_interactive(None, &cancel, |_| UndoChoice::UseSavedVersion)
                .unwrap();
        }
        assert_eq!(
            fs::read(&fixture.current).unwrap(),
            b"saved previous destination"
        );
        assert_eq!(
            fs::read(fixture.original.join("a.txt")).unwrap(),
            b"first returned child"
        );
    }
}

#[test]
fn a_live_merge_protects_saved_history_from_unrelated_source_location_occupants() {
    use canopa_files::operations::OperationRequest;
    let fixture = Fixture::with_saved_copy(true);
    let cancel = Cancellation::default();
    fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            if question.source == UndoSource::ChangedMove {
                UndoChoice::ReturnItem
            } else if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::Skip
            }
        })
        .unwrap();
    let held_source = fixture.root.path().join("incoming elsewhere");
    fs::rename(&fixture.current, &held_source).unwrap();
    fs::write(&fixture.current, b"later occupant").unwrap();
    fixture
        .engine
        .execute(
            OperationRequest::Rename {
                source: fixture.current.clone(),
                destination: fixture.root.path().join("held occupant"),
            },
            &cancel,
        )
        .unwrap();
    fs::write(&fixture.current, b"newest occupant").unwrap();
    fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    fs::rename(
        &fixture.current,
        fixture.root.path().join("newest occupant elsewhere"),
    )
    .unwrap();
    fs::rename(held_source, &fixture.current).unwrap();
    fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    assert_eq!(
        fs::read(&fixture.current).unwrap(),
        b"saved previous destination"
    );
    fixture.engine.undo(&cancel).unwrap();
    assert!(!fixture.current.exists());
    assert_eq!(
        fs::read(fixture.current.with_file_name("item (2)")).unwrap(),
        b"later occupant"
    );
    assert_eq!(
        fs::read(fixture.root.path().join("newest occupant elsewhere")).unwrap(),
        b"newest occupant"
    );
}

#[test]
fn a_saved_version_lost_during_merge_remains_retryable_after_the_contents_return() {
    let mut fixture = Fixture::new();
    let held = fixture.root.path().join("saved version elsewhere");
    let cancel = Cancellation::default();
    let mut deferred = 0;
    let result = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            if question.source == UndoSource::ChangedMove {
                UndoChoice::ReturnItem
            } else if question.can_merge() {
                UndoChoice::Merge
            } else if question.incoming.entry().is_none() {
                assert_eq!(question.incoming.path(), fixture.saved);
                assert!(question.can_return_item());
                deferred += 1;
                UndoChoice::ReturnItem
            } else {
                assert_eq!(question.destination, fixture.original.join("z.txt"));
                fs::rename(&fixture.saved, &held).unwrap();
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(deferred, 1);
    assert_eq!(result.remaining, 1);
    assert_eq!(result.unrestored_previous, vec![fixture.current.clone()]);
    assert!(!fixture.current.exists());
    assert_eq!(
        fs::read(fixture.original.join("a.txt")).unwrap(),
        b"first returned child"
    );
    assert_eq!(
        fs::read(fixture.original.join("z (2).txt")).unwrap(),
        b"last returned child"
    );
    drop(fixture.engine);
    fixture.engine = OperationEngine::open(&fixture.history).unwrap();
    fs::rename(held, &fixture.saved).unwrap();
    let restored = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedRetained);
            UndoChoice::UseSavedVersion
        })
        .unwrap();
    assert_eq!(restored.remaining, 0);
    assert_eq!(
        fs::read(&fixture.current).unwrap(),
        b"saved previous destination"
    );
    assert_eq!(
        fs::read(fixture.original.join("z (2).txt")).unwrap(),
        b"last returned child"
    );
}
