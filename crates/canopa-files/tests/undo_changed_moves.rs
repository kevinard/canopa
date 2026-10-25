use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, RecoveryStatus, UndoChoice, UndoSource},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::fs;

#[test]
fn a_replaced_moved_item_can_be_skipped_cancelled_and_returned_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let before = root.path().join("before.txt");
    let original = root.path().join("original.txt");
    let current = root.path().join("current.txt");
    let external = root.path().join("original item elsewhere.txt");
    fs::write(&before, b"original moved contents").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let older = engine
        .execute(
            OperationRequest::Rename {
                source: before.clone(),
                destination: original.clone(),
            },
            &cancel,
        )
        .unwrap();
    let moved = engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::rename(&current, &external).unwrap();
    fs::write(&current, b"replacement at the moved path").unwrap();
    let skipped = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedMove);
            assert_eq!(question.incoming.entry().unwrap().path.clone(), current);
            assert_eq!(question.incoming.entry().unwrap().size, 29);
            assert_eq!(question.destination, original);
            assert!(question.existing.is_none());
            assert!(question.can_return_item());
            assert!(!question.can_replace());
            UndoChoice::Skip
        })
        .unwrap();
    assert_eq!(skipped.skipped, vec![current.clone()]);
    assert_eq!(skipped.remaining, 1);
    assert_eq!(engine.latest_undo_id(), Some(moved.id));
    let cancelled = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Cancel)
        .unwrap();
    assert!(cancelled.cancelled);
    assert!(!original.exists());
    assert_eq!(
        fs::read(&current).unwrap(),
        b"replacement at the moved path"
    );
    assert!(engine.undo(&cancel).is_err());
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let returned = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedMove);
            UndoChoice::ReturnItem
        })
        .unwrap();
    assert_eq!(returned.completed, 1);
    assert_eq!(returned.remaining, 0);
    assert_eq!(
        fs::read(&original).unwrap(),
        b"replacement at the moved path"
    );
    assert!(!current.exists());
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert_eq!(engine.latest_undo_id(), Some(older.id));
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&before).unwrap(), b"replacement at the moved path");
    assert_eq!(fs::read(&external).unwrap(), b"original moved contents");
    assert!(!original.exists());
}

#[test]
fn returning_the_original_physical_item_after_an_adoption_does_not_cycle_older_bindings() {
    let root = tempfile::tempdir().unwrap();
    let before = root.path().join("before.txt");
    let original = root.path().join("original.txt");
    let current = root.path().join("current.txt");
    let outside_a = root.path().join("original elsewhere.txt");
    let outside_b = root.path().join("replacement elsewhere.txt");
    fs::write(&before, b"original contents").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let older = engine
        .execute(
            OperationRequest::Rename {
                source: before.clone(),
                destination: original.clone(),
            },
            &cancel,
        )
        .unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::rename(&current, &outside_a).unwrap();
    fs::write(&current, b"first replacement").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::ReturnItem)
        .unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::rename(&current, &outside_b).unwrap();
    fs::rename(&outside_a, &current).unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::ReturnItem)
        .unwrap();
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert_eq!(engine.latest_undo_id(), Some(older.id));
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&before).unwrap(), b"original contents");
    assert_eq!(fs::read(&outside_b).unwrap(), b"first replacement");
}

#[test]
fn an_old_identity_binding_does_not_override_a_newly_recorded_move_of_that_physical_item() {
    let root = tempfile::tempdir().unwrap();
    let before = root.path().join("before.txt");
    let original = root.path().join("original.txt");
    let current = root.path().join("current.txt");
    let outside = root.path().join("original elsewhere.txt");
    let parked = root.path().join("parked replacement.txt");
    fs::write(&before, b"original contents").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    engine
        .execute(
            OperationRequest::Rename {
                source: before.clone(),
                destination: original.clone(),
            },
            &cancel,
        )
        .unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::rename(&current, &outside).unwrap();
    fs::write(&current, b"adopted replacement").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::ReturnItem)
        .unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: parked.clone(),
            },
            &cancel,
        )
        .unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: outside.clone(),
                destination: original.clone(),
            },
            &cancel,
        )
        .unwrap();
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&outside).unwrap(), b"original contents");
    assert!(!original.exists());
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&original).unwrap(), b"adopted replacement");
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&before).unwrap(), b"adopted replacement");
    assert_eq!(fs::read(&outside).unwrap(), b"original contents");
}

#[test]
fn restoring_a_saved_destination_restores_its_older_identity_after_another_adoption() {
    use std::os::unix::fs::PermissionsExt;
    for partial in [false, true] {
        if partial && rustix::process::geteuid().is_root() {
            continue;
        }
        let root = tempfile::tempdir().unwrap();
        let before_dir = root.path().join("before");
        let original_dir = root.path().join("original");
        let current_dir = root.path().join("current");
        let outside_dir = root.path().join("outside");
        for path in [&before_dir, &original_dir, &current_dir, &outside_dir] {
            fs::create_dir(path).unwrap();
        }
        let before = before_dir.join("report.txt");
        let original = original_dir.join("report.txt");
        let current = current_dir.join("report.txt");
        let outside = outside_dir.join("report.txt");
        let preserved_original = root.path().join("original physical item preserved.txt");
        fs::write(&before, b"original physical item").unwrap();
        let history = root.path().join("state/history");
        let engine = OperationEngine::open(&history).unwrap();
        let cancel = Cancellation::default();
        engine
            .execute(
                OperationRequest::Rename {
                    source: before.clone(),
                    destination: original.clone(),
                },
                &cancel,
            )
            .unwrap();
        engine
            .execute(
                OperationRequest::Rename {
                    source: original.clone(),
                    destination: current.clone(),
                },
                &cancel,
            )
            .unwrap();
        fs::rename(&current, &outside).unwrap();
        fs::write(&current, b"first adopted replacement").unwrap();
        engine
            .undo_interactive(None, &cancel, |_| UndoChoice::ReturnItem)
            .unwrap();
        let moved = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![outside.clone()],
                    destination: original_dir,
                },
                &cancel,
                |_| TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
            )
            .unwrap();
        assert!(moved.failures.is_empty());
        let saved = engine.recovery_items(None, &cancel).unwrap().items[0]
            .entry
            .as_ref()
            .unwrap()
            .path
            .clone();
        engine
            .execute(
                OperationRequest::Rename {
                    source: original.clone(),
                    destination: current.clone(),
                },
                &cancel,
            )
            .unwrap();
        fs::rename(&current, &preserved_original).unwrap();
        fs::write(&current, b"newest chosen replacement").unwrap();
        engine
            .undo_interactive(None, &cancel, |_| UndoChoice::ReturnItem)
            .unwrap();
        if partial {
            fs::set_permissions(saved.parent().unwrap(), fs::Permissions::from_mode(0o500))
                .unwrap();
            assert_eq!(
                engine.undo(&cancel).unwrap_err().kind(),
                std::io::ErrorKind::PermissionDenied
            );
            fs::set_permissions(saved.parent().unwrap(), fs::Permissions::from_mode(0o700))
                .unwrap();
        }
        drop(engine);
        let engine = OperationEngine::open(&history).unwrap();
        // Reopen completes a proved permission refusal. Without that failed
        // attempt, this Move still needs its first Undo request here.
        if !partial {
            engine.undo(&cancel).unwrap();
        }
        assert_eq!(fs::read(&original).unwrap(), b"first adopted replacement");
        assert_eq!(fs::read(&outside).unwrap(), b"newest chosen replacement");
        drop(engine);
        let engine = OperationEngine::open(&history).unwrap();
        engine.undo(&cancel).unwrap();
        assert_eq!(fs::read(&before).unwrap(), b"first adopted replacement");
        assert_eq!(fs::read(&outside).unwrap(), b"newest chosen replacement");
        assert_eq!(
            fs::read(&preserved_original).unwrap(),
            b"original physical item"
        );
    }
}

#[test]
fn keeping_both_after_another_adoption_preserves_the_newer_return_binding() {
    let root = tempfile::tempdir().unwrap();
    let before = root.path().join("before.txt");
    let original = root.path().join("original.txt");
    let current = root.path().join("current.txt");
    let outside_a = root.path().join("original elsewhere.txt");
    let outside_b = root.path().join("first replacement elsewhere.txt");
    fs::write(&before, b"original contents").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    engine
        .execute(
            OperationRequest::Rename {
                source: before.clone(),
                destination: original.clone(),
            },
            &cancel,
        )
        .unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::rename(&current, &outside_a).unwrap();
    fs::write(&current, b"first adopted replacement").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::ReturnItem)
        .unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::rename(&current, &outside_b).unwrap();
    fs::write(&current, b"second adopted replacement").unwrap();
    fs::write(&original, b"newer work at original location").unwrap();
    let mut questions = Vec::new();
    engine
        .undo_interactive(None, &cancel, |question| {
            questions.push(question.source);
            if question.source == UndoSource::ChangedMove {
                UndoChoice::ReturnItem
            } else {
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(questions, [UndoSource::ChangedMove, UndoSource::Current]);
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&before).unwrap(), b"second adopted replacement");
    assert_eq!(
        fs::read(&original).unwrap(),
        b"newer work at original location"
    );
    assert_eq!(fs::read(&outside_a).unwrap(), b"original contents");
    assert_eq!(fs::read(&outside_b).unwrap(), b"first adopted replacement");
}

#[test]
fn keeping_a_replacement_preserves_its_saved_previous_destination_through_expiry() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"original moved contents").unwrap();
    let current = destination.join("report.txt");
    fs::write(&current, b"previous destination before moving").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    let external = root.path().join("moved elsewhere.txt");
    fs::rename(&current, &external).unwrap();
    fs::write(&current, b"replacement to keep here").unwrap();
    let kept = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedMove);
            UndoChoice::KeepItem
        })
        .unwrap();
    assert_eq!(kept.kept, vec![current.clone()]);
    assert_eq!(kept.completed, 0);
    assert_eq!(kept.remaining, 0);
    assert_eq!(fs::read(&current).unwrap(), b"replacement to keep here");
    assert_eq!(fs::read(&external).unwrap(), b"original moved contents");
    assert!(!source.exists());
    for index in 0..101 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("folder-{index}")),
                },
                &cancel,
            )
            .unwrap();
    }
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    let item = &page.items[0];
    assert!(!item.held_for_undo);
    assert_eq!(item.status, RecoveryStatus::NeedsRecovery);
    assert_eq!(item.original.as_deref(), Some(current.as_path()));
    let recovered = root.path().join("Recovered previous destination.txt");
    engine.recover_copy(item, &recovered, &cancel).unwrap();
    assert_eq!(
        fs::read(&recovered).unwrap(),
        b"previous destination before moving"
    );
    assert_eq!(
        fs::read(item.entry.as_ref().unwrap().path.clone()).unwrap(),
        b"previous destination before moving"
    );
    engine.undo(&cancel).unwrap();
    assert!(!recovered.exists());
    assert_eq!(fs::read(&current).unwrap(), b"replacement to keep here");
}

#[test]
fn returning_a_replacement_asks_separately_about_newer_work_at_its_original_location() {
    for choice in [UndoChoice::KeepBoth, UndoChoice::Replace] {
        let root = tempfile::tempdir().unwrap();
        let before = root.path().join("before.txt");
        let source = root.path().join("report.txt");
        let destination = root.path().join("destination");
        let external = root.path().join("original moved item elsewhere.txt");
        fs::create_dir(&destination).unwrap();
        fs::write(&before, b"original incoming item").unwrap();
        let current = destination.join("report.txt");
        fs::write(&current, b"previous destination").unwrap();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let older = engine
            .execute(
                OperationRequest::Rename {
                    source: before.clone(),
                    destination: source.clone(),
                },
                &cancel,
            )
            .unwrap();
        let moved = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![source.clone()],
                    destination: destination.clone(),
                },
                &cancel,
                |_| TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
            )
            .unwrap();
        assert!(moved.failures.is_empty());
        fs::rename(&current, &external).unwrap();
        fs::write(&current, b"replacement selected for return").unwrap();
        fs::write(&source, b"newer work at the original location").unwrap();
        let mut questions = Vec::new();
        let outcome = engine
            .undo_interactive(None, &cancel, |question| {
                questions.push(question.source);
                if question.source == UndoSource::ChangedMove {
                    UndoChoice::ReturnItem
                } else {
                    assert_eq!(question.source, UndoSource::Current);
                    assert_eq!(question.incoming.entry().unwrap().path.clone(), current);
                    assert_eq!(question.existing.as_ref().unwrap().path, source);
                    choice
                }
            })
            .unwrap();
        assert_eq!(
            questions,
            vec![UndoSource::ChangedMove, UndoSource::Current]
        );
        assert_eq!(outcome.remaining, 0);
        let returned = if choice == UndoChoice::KeepBoth {
            root.path().join("report (2).txt")
        } else {
            source.clone()
        };
        assert_eq!(
            fs::read(&returned).unwrap(),
            b"replacement selected for return"
        );
        assert_eq!(fs::read(&current).unwrap(), b"previous destination");
        assert_eq!(fs::read(&external).unwrap(), b"original incoming item");
        assert_eq!(engine.latest_undo_id(), Some(older.id));
        engine.undo(&cancel).unwrap();
        assert_eq!(
            fs::read(&before).unwrap(),
            b"replacement selected for return"
        );
        if choice == UndoChoice::KeepBoth {
            assert_eq!(
                fs::read(&source).unwrap(),
                b"newer work at the original location"
            );
        } else {
            let page = engine.recovery_items(None, &cancel).unwrap();
            assert_eq!(page.items.len(), 1);
            assert_eq!(
                fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
                b"newer work at the original location"
            );
        }
    }
}

#[test]
fn changing_the_reviewed_item_in_either_question_refuses_return_without_consuming_history() {
    for collision in [None, Some(UndoChoice::KeepBoth), Some(UndoChoice::Replace)] {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original.txt");
        let current = root.path().join("current.txt");
        let external = root.path().join("original elsewhere.txt");
        fs::write(&original, b"original contents").unwrap();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let moved = engine
            .execute(
                OperationRequest::Rename {
                    source: original.clone(),
                    destination: current.clone(),
                },
                &cancel,
            )
            .unwrap();
        fs::rename(&current, &external).unwrap();
        fs::write(&current, b"replacement being reviewed").unwrap();
        if collision.is_some() {
            fs::write(&original, b"newer original-location work").unwrap();
        }
        let error = engine
            .undo_interactive(None, &cancel, |question| {
                if question.source == UndoSource::ChangedMove {
                    if collision.is_none() {
                        fs::write(&current, b"changed while review was open").unwrap();
                    }
                    UndoChoice::ReturnItem
                } else {
                    assert_eq!(question.source, UndoSource::Current);
                    fs::write(&current, b"changed while review was open").unwrap();
                    collision.unwrap()
                }
            })
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert_eq!(
            fs::read(&current).unwrap(),
            b"changed while review was open"
        );
        assert_eq!(fs::read(&external).unwrap(), b"original contents");
        if collision.is_some() {
            assert_eq!(
                fs::read(&original).unwrap(),
                b"newer original-location work"
            );
        } else {
            assert!(!original.exists());
        }
        assert_eq!(engine.latest_undo_id(), Some(moved.id));
        assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
        assert!(
            engine
                .recovery_items(None, &cancel)
                .unwrap()
                .items
                .is_empty()
        );
        assert!(!root.path().join("original (2).txt").exists());
        assert!(!fs::read_dir(root.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .as_encoded_bytes()
                .starts_with(b".canopa-files-stage-")
        }));
    }
}

#[test]
fn returning_replacement_folders_and_raw_target_links_preserves_all_contents() {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    for directory in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let original = root
            .path()
            .join(std::ffi::OsString::from_vec(b"original-\xfe".to_vec()));
        let current = root
            .path()
            .join(std::ffi::OsString::from_vec(b"current-\xff".to_vec()));
        let external = root.path().join("original elsewhere");
        fs::write(&original, b"original moved file").unwrap();
        let history = root.path().join("state/history");
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
        fs::rename(&current, &external).unwrap();
        let target = root
            .path()
            .join(std::ffi::OsString::from_vec(b"link-target-\xfd".to_vec()));
        fs::write(&target, b"link target stays intact").unwrap();
        if directory {
            fs::create_dir(&current).unwrap();
            fs::write(current.join("child.txt"), b"replacement folder contents").unwrap();
        } else {
            symlink(&target, &current).unwrap();
        }
        drop(engine);
        let engine = OperationEngine::open(&history).unwrap();
        let outcome = engine
            .undo_interactive(None, &cancel, |question| {
                assert_eq!(question.source, UndoSource::ChangedMove);
                assert!(question.can_return_item());
                UndoChoice::ReturnItem
            })
            .unwrap();
        assert_eq!(outcome.remaining, 0);
        assert!(!current.exists());
        if directory {
            assert_eq!(
                fs::read(original.join("child.txt")).unwrap(),
                b"replacement folder contents"
            );
        } else {
            assert_eq!(fs::read_link(&original).unwrap(), target);
        }
        assert_eq!(fs::read(&target).unwrap(), b"link target stays intact");
        assert_eq!(fs::read(&external).unwrap(), b"original moved file");
    }
}

#[test]
fn returning_a_reviewed_fifo_renames_the_node_without_opening_its_stream() {
    use std::os::unix::fs::FileTypeExt;
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original");
    let current = root.path().join("current");
    let external = root.path().join("original elsewhere");
    fs::write(&original, b"original contents").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
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
    fs::rename(&current, &external).unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &current,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let start = std::time::Instant::now();
    engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedMove);
            assert!(question.can_return_item());
            UndoChoice::ReturnItem
        })
        .unwrap();
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
    assert!(
        fs::symlink_metadata(&original)
            .unwrap()
            .file_type()
            .is_fifo()
    );
    assert!(!current.exists());
    assert_eq!(fs::read(&external).unwrap(), b"original contents");
}

#[test]
fn edits_to_the_same_moved_item_return_with_it_without_a_replacement_question() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original.txt");
    let current = root.path().join("current.txt");
    fs::write(&original, b"original contents").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
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
    fs::write(&current, b"edited since moving").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| {
            panic!("returning the same edited item does not overwrite newer work")
        })
        .unwrap();
    assert_eq!(fs::read(&original).unwrap(), b"edited since moving");
    assert!(!current.exists());
}

#[test]
fn a_partial_return_keeps_the_selected_identity_bound_when_restoring_the_backup_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    if rustix::process::geteuid().is_root() {
        eprintln!("requires an unprivileged user to refuse the saved-file rename");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let before = root.path().join("before.txt");
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    let external = root.path().join("original incoming item elsewhere.txt");
    fs::create_dir(&destination).unwrap();
    fs::write(&before, b"original incoming item").unwrap();
    let current = destination.join("report.txt");
    fs::write(&current, b"previous destination").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let older = engine
        .execute(
            OperationRequest::Rename {
                source: before.clone(),
                destination: source.clone(),
            },
            &cancel,
        )
        .unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    let saved = engine.recovery_items(None, &cancel).unwrap().items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .clone();
    fs::rename(&current, &external).unwrap();
    fs::write(&current, b"selected replacement").unwrap();
    let error = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedMove);
            fs::set_permissions(saved.parent().unwrap(), fs::Permissions::from_mode(0o500))
                .unwrap();
            UndoChoice::ReturnItem
        })
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    fs::set_permissions(saved.parent().unwrap(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(fs::read(&source).unwrap(), b"selected replacement");
    assert!(!current.exists());
    assert_eq!(fs::read(&saved).unwrap(), b"previous destination");
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    // The saved return resumes on reopen; its completed incoming return stays
    // bound to the selected item for the older Undo below.
    assert_eq!(engine.latest_undo_id(), Some(older.id));
    assert_eq!(fs::read(&current).unwrap(), b"previous destination");
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert_eq!(engine.latest_undo_id(), Some(older.id));
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&before).unwrap(), b"selected replacement");
    assert_eq!(fs::read(&external).unwrap(), b"original incoming item");
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn returning_a_reviewed_hard_link_can_preserve_the_existing_link_to_the_same_file() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original.txt");
    let current = root.path().join("current.txt");
    let external = root.path().join("original elsewhere.txt");
    fs::write(&original, b"original contents").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
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
    fs::rename(&current, &external).unwrap();
    fs::write(&current, b"replacement shared with its original location").unwrap();
    fs::hard_link(&current, &original).unwrap();
    let outcome = engine
        .undo_interactive(None, &cancel, |question| {
            if question.source == UndoSource::ChangedMove {
                UndoChoice::ReturnItem
            } else {
                assert_eq!(question.source, UndoSource::Current);
                UndoChoice::Replace
            }
        })
        .unwrap();
    assert_eq!(outcome.remaining, 0);
    assert!(!current.exists());
    assert_eq!(
        fs::read(&original).unwrap(),
        b"replacement shared with its original location"
    );
    assert_eq!(fs::read(&external).unwrap(), b"original contents");
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"replacement shared with its original location"
    );
}

#[test]
fn restarting_a_killed_reviewed_move_preserves_selected_files_and_older_copy_undo() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    for adopted in [false, true] {
        let root = tempfile::Builder::new()
            .prefix("canopa-moved-undo-crash-")
            .tempdir()
            .unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "crash_reviewed_move_helper"])
            .env("CANOPA_FILES_MOVED_CRASH_ROOT", root.path())
            .env(
                "CANOPA_FILES_MOVED_CRASH_ADOPTED",
                if adopted { "yes" } else { "no" },
            )
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        assert!(!root.path().join("undo-returned").exists());
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let outcome = engine
            .undo_interactive(None, &cancel, |question| {
                assert!(!adopted, "an already adopted item needs no second review");
                assert_eq!(question.source, UndoSource::ChangedMove);
                UndoChoice::ReturnItem
            })
            .unwrap();
        assert_eq!(outcome.remaining, 0);
        for index in 0..500 {
            assert_eq!(
                fs::read(root.path().join(format!("source/file-{index:04}"))).unwrap(),
                b"incoming copied data"
            );
            assert_eq!(
                fs::read(root.path().join(format!("destination/file-{index:04}"))).unwrap(),
                b"previous destination"
            );
        }
        engine.undo(&cancel).unwrap();
        assert_eq!(fs::read_dir(root.path().join("source")).unwrap().count(), 0);
        assert_eq!(
            fs::read_dir(root.path().join("destination"))
                .unwrap()
                .count(),
            500
        );
        for index in 0..500 {
            assert_eq!(
                fs::read(root.path().join(format!("older/file-{index:04}"))).unwrap(),
                b"incoming copied data"
            );
            assert_eq!(
                fs::read(root.path().join(format!("external/file-{index:04}"))).unwrap(),
                b"incoming copied data"
            );
        }
        assert!(
            engine
                .recovery_items(None, &cancel)
                .unwrap()
                .items
                .is_empty()
        );
    }
}

#[test]
#[ignore = "child process fixture; killed after an actual reviewed moved-item return"]
fn crash_reviewed_move_helper() {
    use std::os::unix::fs::MetadataExt;
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_FILES_MOVED_CRASH_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-moved-undo-crash-")
    );
    let older = root.join("older");
    let source = root.join("source");
    let destination = root.join("destination");
    let external = root.join("external");
    for directory in [&older, &source, &destination, &external] {
        fs::create_dir(directory).unwrap();
    }
    for index in 0..500 {
        fs::write(
            older.join(format!("file-{index:04}")),
            b"incoming copied data",
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
        let outcome = engine
            .transfer(
                &TransferRequest {
                    kind,
                    sources: (0..500)
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
        assert!(outcome.failures.is_empty());
    }
    let adopted = std::env::var("CANOPA_FILES_MOVED_CRASH_ADOPTED").unwrap() == "yes";
    let current_directory = if adopted {
        let next = root.join("next");
        fs::create_dir(&next).unwrap();
        let result = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: (0..500)
                        .map(|index| destination.join(format!("file-{index:04}")))
                        .collect(),
                    destination: next.clone(),
                },
                &cancel,
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(result.failures.is_empty());
        next
    } else {
        destination.clone()
    };
    let mut selected = None;
    for index in 0..500 {
        let name = format!("file-{index:04}");
        let current = current_directory.join(&name);
        let elsewhere = external.join(&name);
        let metadata = fs::metadata(&current).unwrap();
        fs::rename(&current, &elsewhere).unwrap();
        fs::copy(&elsewhere, &current).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&current)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        if index == 499 {
            let metadata = fs::metadata(&current).unwrap();
            selected = Some((source.join(&name), metadata.dev(), metadata.ino()));
        }
    }
    if adopted {
        engine
            .undo_interactive(None, &cancel, |_| UndoChoice::ReturnItem)
            .unwrap();
    }
    let (returned, device, inode) = selected.unwrap();
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
        .undo_interactive(None, &cancel, |_| UndoChoice::ReturnItem)
        .unwrap();
    fs::write(root.join("undo-returned"), b"missed crash window").unwrap();
    panic!("the child was not killed during reviewed move reversal");
}
