use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, UndoChoice, UndoSource},
};
use std::{fs, os::unix::fs::symlink};

#[test]
fn an_unreadable_merge_source_or_destination_can_be_reviewed_without_opening_children() {
    use std::os::unix::fs::PermissionsExt;
    struct Reset(std::path::PathBuf);
    impl Drop for Reset {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        }
    }
    for source in [false, true] {
        let fixture = partial_copied_merge();
        let path = if source {
            &fixture.current
        } else {
            &fixture.original
        };
        let reset = Reset(path.clone());
        fs::set_permissions(path, fs::Permissions::from_mode(0o0)).unwrap();
        let cancel = Cancellation::default();
        let result = fixture
            .engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(
                    question.source,
                    if source {
                        UndoSource::MergeSource
                    } else {
                        UndoSource::MergeDestination
                    }
                );
                assert_eq!(question.incoming.path(), path);
                assert!(question.incoming.entry().is_none());
                UndoChoice::Skip
            })
            .unwrap();
        assert_eq!(result.remaining, 2);
        let result = fixture
            .engine
            .undo_interactive(None, &cancel, |_| UndoChoice::FinishStep)
            .unwrap();
        assert_eq!(result.completed, 0);
        assert_eq!(result.remaining, 0);
        drop(reset);
        assert_eq!(
            fs::read(fixture.current.join("z.txt")).unwrap(),
            b"last incoming child"
        );
        assert_eq!(
            fs::read(fixture.original.join("a.txt")).unwrap(),
            b"original copied data"
        );
    }
}

#[test]
fn finishing_a_nested_merge_leaves_its_ancestor_pending_and_continues_a_disjoint_sibling() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    let held = root.path().join("nested destination elsewhere");
    fs::create_dir_all(original.join("nested")).unwrap();
    fs::write(original.join("nested/a.txt"), b"returned nested child").unwrap();
    fs::write(original.join("nested/z.txt"), b"unfinished nested child").unwrap();
    fs::write(original.join("zz.txt"), b"incoming outside sibling").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir_all(original.join("nested")).unwrap();
    fs::write(original.join("nested/z.txt"), b"newer nested z").unwrap();
    fs::write(original.join("zz.txt"), b"newer outside sibling").unwrap();
    let partial = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert_eq!(partial.remaining, 4);
    fs::rename(original.join("nested"), &held).unwrap();
    fs::create_dir(original.join("nested")).unwrap();
    fs::write(original.join("nested/foreign.txt"), b"foreign nested work").unwrap();
    let mut finished = false;
    let outcome = engine
        .undo_interactive(None, &cancel, |question| {
            if question.source == UndoSource::MergeDestination {
                assert_eq!(question.incoming.path(), original.join("nested"));
                finished = true;
                UndoChoice::FinishStep
            } else {
                assert_eq!(question.destination, original.join("zz.txt"));
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert!(finished);
    assert_eq!(outcome.remaining, 1);
    assert_eq!(outcome.completed, 1);
    assert_eq!(
        fs::read(original.join("zz (2).txt")).unwrap(),
        b"incoming outside sibling"
    );
    assert_eq!(
        fs::read(current.join("nested/z.txt")).unwrap(),
        b"unfinished nested child"
    );
    let outcome = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepItem)
        .unwrap();
    assert_eq!(outcome.remaining, 0);
    assert_eq!(
        fs::read(original.join("nested/foreign.txt")).unwrap(),
        b"foreign nested work"
    );
    assert_eq!(
        fs::read(held.join("a.txt")).unwrap(),
        b"returned nested child"
    );
}

#[test]
fn finishing_a_lost_root_protects_its_saved_destination_through_history_expiry() {
    use canopa_files::transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest};
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&original, b"original moved file").unwrap();
    let current = destination.join("project");
    fs::write(&current, b"saved previous destination").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let moved = engine
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
    assert!(moved.failures.is_empty(), "{:?}", moved.failures);
    let external = root.path().join("original moved file elsewhere");
    fs::rename(&current, &external).unwrap();
    fs::create_dir(&current).unwrap();
    fs::write(current.join("a.txt"), b"returned child").unwrap();
    fs::write(current.join("z.txt"), b"unfinished child").unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("z.txt"), b"newer z").unwrap();
    engine
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
    fs::rename(&current, root.path().join("source elsewhere")).unwrap();
    fs::write(&current, b"foreign source occupant").unwrap();
    let result = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::FinishStep)
        .unwrap();
    assert_eq!(result.completed, 0);
    assert_eq!(result.remaining, 0);
    assert_eq!(result.unrestored_previous, vec![current.clone()]);
    for index in 0..101 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("later-{index}")),
                },
                &cancel,
            )
            .unwrap();
    }
    let items = engine.recovery_items(None, &cancel).unwrap().items;
    assert_eq!(items.len(), 1);
    assert!(!items[0].held_for_undo);
    assert_eq!(items[0].original, Some(current.clone()));
    let recovered = root.path().join("recovered saved destination");
    engine.recover_copy(&items[0], &recovered, &cancel).unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"saved previous destination");
    assert_eq!(fs::read(&current).unwrap(), b"foreign source occupant");
    assert_eq!(fs::read(original.join("a.txt")).unwrap(), b"returned child");
    assert_eq!(fs::read(external).unwrap(), b"original moved file");
}

#[test]
fn finishing_a_merge_with_missing_trash_information_keeps_its_remaining_payload_listed() {
    use canopa_files::trash::TrashLocation;
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("a.txt"), b"returned child").unwrap();
    fs::write(original.join("z.txt"), b"unfinished child").unwrap();
    let location = TrashLocation::home(root.path().join("data")).unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    assert!(
        engine
            .trash(&location, std::slice::from_ref(&original), &cancel)
            .unwrap()
            .failures
            .is_empty()
    );
    let current = location.list(&cancel).unwrap()[0].path.clone();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("z.txt"), b"newer z").unwrap();
    engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::Skip
            }
        })
        .unwrap();
    let mut name = current.file_name().unwrap().to_owned();
    name.push(".trashinfo");
    let metadata = current
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("info")
        .join(name);
    fs::remove_file(&metadata).unwrap();
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::MergeSource);
            assert!(question.incoming.entry().is_none());
            UndoChoice::FinishStep
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!metadata.exists());
    let items = location.list(&cancel).unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0].problem.is_some());
    assert_eq!(
        fs::read(items[0].path.join("z.txt")).unwrap(),
        b"unfinished child"
    );
    assert_eq!(fs::read(original.join("a.txt")).unwrap(), b"returned child");
}

struct CopiedFixture {
    root: tempfile::TempDir,
    engine: OperationEngine,
    original: std::path::PathBuf,
    current: std::path::PathBuf,
}

fn partial_copied_merge() -> CopiedFixture {
    use canopa_files::transfers::{TransferKind, TransferReply, TransferRequest};
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    let sources = root.path().join("sources");
    fs::create_dir(&original).unwrap();
    fs::create_dir(&sources).unwrap();
    fs::write(sources.join("a.txt"), b"original copied data").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![sources.join("a.txt")],
                destination: original.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    fs::write(original.join("z.txt"), b"last incoming child").unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("z.txt"), b"newer z").unwrap();
    engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::Skip
            }
        })
        .unwrap();
    CopiedFixture {
        root,
        engine,
        original,
        current,
    }
}

#[test]
fn finishing_a_lost_source_preserves_older_copy_undo_on_a_returned_child() {
    let fixture = partial_copied_merge();
    let held = fixture.root.path().join("source elsewhere");
    fs::rename(&fixture.current, &held).unwrap();
    let cancel = Cancellation::default();
    fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::FinishStep)
        .unwrap();
    fixture.engine.undo(&cancel).unwrap();
    assert!(!fixture.original.join("a.txt").exists());
    assert_eq!(
        fs::read(held.join("z.txt")).unwrap(),
        b"last incoming child"
    );
    assert_eq!(
        fs::read(fixture.original.join("z.txt")).unwrap(),
        b"newer z"
    );
}

#[test]
fn finishing_a_replaced_destination_does_not_adopt_it_for_older_copy_undo() {
    let fixture = partial_copied_merge();
    let held = fixture.root.path().join("destination elsewhere");
    fs::rename(&fixture.original, &held).unwrap();
    fs::create_dir(&fixture.original).unwrap();
    fs::write(
        fixture.original.join("a.txt"),
        b"foreign copied-file occupant",
    )
    .unwrap();
    let cancel = Cancellation::default();
    fixture
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::FinishStep)
        .unwrap();
    let mut asked = false;
    let older = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            asked = true;
            assert_eq!(question.source, UndoSource::ChangedCopy);
            UndoChoice::Skip
        })
        .unwrap();
    assert!(asked);
    assert_eq!(older.remaining, 1);
    assert_eq!(
        fs::read(fixture.original.join("a.txt")).unwrap(),
        b"foreign copied-file occupant"
    );
    assert_eq!(
        fs::read(held.join("a.txt")).unwrap(),
        b"original copied data"
    );
    assert_eq!(
        fs::read(fixture.current.join("z.txt")).unwrap(),
        b"last incoming child"
    );
}

#[test]
fn finishing_a_merge_refuses_changes_to_either_root_and_respects_cancellation() {
    let fixture = partial_copied_merge();
    let operation = fixture.engine.latest_undo_id();
    fs::rename(
        &fixture.original,
        fixture.root.path().join("destination elsewhere"),
    )
    .unwrap();
    fs::create_dir(&fixture.original).unwrap();
    let cancel = Cancellation::default();
    assert_eq!(
        fixture.engine.undo(&cancel).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(
        fixture
            .engine
            .interrupted_operations(&cancel)
            .unwrap()
            .is_empty()
    );
    for path in [&fixture.current, &fixture.original] {
        let result = fixture.engine.undo_interactive(None, &cancel, |_| {
            fs::write(path.join("arrived during review"), b"new work").unwrap();
            UndoChoice::FinishStep
        });
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
        assert_eq!(fixture.engine.latest_undo_id(), operation);
    }
    let cancelled = fixture
        .engine
        .undo_interactive(None, &cancel, |_| {
            cancel.cancel();
            UndoChoice::FinishStep
        })
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.remaining, 2);
    let outcome = fixture
        .engine
        .undo_interactive(None, &Cancellation::default(), |_| UndoChoice::FinishStep)
        .unwrap();
    assert_eq!(outcome.remaining, 0);
    assert_eq!(
        fs::read(fixture.current.join("arrived during review")).unwrap(),
        b"new work"
    );
    assert_eq!(
        fs::read(fixture.original.join("arrived during review")).unwrap(),
        b"new work"
    );
}

#[test]
fn finishing_a_merge_with_a_lost_root_leaves_both_namespaces_untouched() {
    for source_lost in [false, true] {
        for replacement in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let original = root.path().join("project");
            let current = root.path().join("moved");
            let held = root.path().join("original root elsewhere");
            fs::create_dir(&original).unwrap();
            fs::write(original.join("a.txt"), b"first incoming child").unwrap();
            fs::write(original.join("z.txt"), b"last incoming child").unwrap();
            let history = root.path().join("state/history");
            let cancel = Cancellation::default();
            let engine = OperationEngine::open(&history).unwrap();
            engine
                .execute(
                    OperationRequest::Rename {
                        source: original.clone(),
                        destination: current.clone(),
                    },
                    &cancel,
                )
                .unwrap();
            fs::create_dir(&original).unwrap();
            fs::write(original.join("z.txt"), b"newer z").unwrap();
            engine
                .undo_interactive(None, &cancel, |question| {
                    if question.can_merge() {
                        UndoChoice::Merge
                    } else {
                        UndoChoice::Skip
                    }
                })
                .unwrap();
            let lost = if source_lost { &current } else { &original };
            fs::rename(lost, &held).unwrap();
            if replacement {
                fs::create_dir(lost).unwrap();
                fs::write(lost.join("foreign.txt"), b"foreign replacement work").unwrap();
            }
            let outcome = engine
                .undo_interactive(None, &cancel, |question| {
                    assert_eq!(
                        question.source,
                        if source_lost {
                            UndoSource::MergeSource
                        } else {
                            UndoSource::MergeDestination
                        }
                    );
                    assert_eq!(question.incoming.path(), lost);
                    UndoChoice::FinishStep
                })
                .unwrap();
            assert_eq!(outcome.remaining, 0);
            assert_eq!(outcome.completed, 0);
            assert_eq!(outcome.dismissed, vec![current.clone()]);
            assert!(engine.latest_undo_id().is_none());
            assert_eq!(
                fs::read(held.join("z.txt")).unwrap(),
                if source_lost {
                    b"last incoming child".as_slice()
                } else {
                    b"newer z".as_slice()
                }
            );
            assert!(!held.join("z (2).txt").exists());
            if replacement {
                assert_eq!(
                    fs::read(lost.join("foreign.txt")).unwrap(),
                    b"foreign replacement work"
                );
            } else {
                assert!(!lost.exists());
            }
            drop(engine);
            assert!(
                OperationEngine::open(&history)
                    .unwrap()
                    .latest_undo_id()
                    .is_none()
            );
        }
    }
}

#[test]
fn a_missing_or_replaced_merge_destination_can_be_skipped_cancelled_and_retried() {
    for replacement in 0..4 {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("project");
        let current = root.path().join("moved");
        let held = root.path().join("merged folder elsewhere");
        let foreign = root.path().join("foreign folder");
        fs::create_dir(&original).unwrap();
        fs::write(original.join("a.txt"), b"first incoming child").unwrap();
        fs::write(original.join("z.txt"), b"last incoming child").unwrap();
        let cancel = Cancellation::default();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        engine
            .execute(
                OperationRequest::Rename {
                    source: original.clone(),
                    destination: current.clone(),
                },
                &cancel,
            )
            .unwrap();
        fs::create_dir(&original).unwrap();
        fs::write(original.join("z.txt"), b"newer destination z").unwrap();
        let partial = engine
            .undo_interactive(None, &cancel, |question| {
                if question.can_merge() {
                    UndoChoice::Merge
                } else {
                    UndoChoice::Skip
                }
            })
            .unwrap();
        assert_eq!(partial.remaining, 2);
        fs::rename(&original, &held).unwrap();
        match replacement {
            0 => {}
            1 => fs::write(&original, b"foreign file").unwrap(),
            2 => fs::create_dir(&original).unwrap(),
            3 => {
                fs::create_dir(&foreign).unwrap();
                fs::write(foreign.join("z.txt"), b"foreign linked data").unwrap();
                symlink(&foreign, &original).unwrap();
            }
            _ => unreachable!(),
        }
        let mut questions = 0;
        let skipped = engine
            .undo_interactive(None, &cancel, |question| {
                questions += 1;
                assert_eq!(question.incoming.path(), original);
                assert_eq!(question.destination, current);
                assert!(!question.can_merge());
                assert!(!question.can_replace());
                assert!(!question.can_return_item());
                UndoChoice::Skip
            })
            .unwrap();
        assert_eq!(
            questions, 1,
            "review the affected Merge once, before its child"
        );
        assert_eq!(skipped.remaining, 2);
        assert_eq!(skipped.completed, 0);
        let cancelled = engine
            .undo_interactive(None, &cancel, |_| UndoChoice::Cancel)
            .unwrap();
        assert!(cancelled.cancelled);
        assert_eq!(cancelled.remaining, 2);
        assert_eq!(
            fs::read(current.join("z.txt")).unwrap(),
            b"last incoming child"
        );
        if replacement == 2 {
            fs::remove_dir(&original).unwrap();
        } else if replacement != 0 {
            fs::remove_file(&original).unwrap();
        }
        fs::rename(&held, &original).unwrap();
        let finished = engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.destination, original.join("z.txt"));
                UndoChoice::KeepBoth
            })
            .unwrap();
        assert_eq!(finished.remaining, 0);
        assert_eq!(
            fs::read(original.join("a.txt")).unwrap(),
            b"first incoming child"
        );
        assert!(!original.join("a (2).txt").exists());
        assert_eq!(
            fs::read(original.join("z (2).txt")).unwrap(),
            b"last incoming child"
        );
        if replacement == 3 {
            assert_eq!(
                fs::read(foreign.join("z.txt")).unwrap(),
                b"foreign linked data"
            );
        }
    }
}
