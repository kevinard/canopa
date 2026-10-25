use canopa_files::{
    Cancellation,
    operations::OperationEngine,
    transfers::{TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    os::unix::{ffi::OsStringExt, fs::symlink},
};

#[test]
fn a_live_continuation_never_adopts_a_replacement_destination_folder() {
    use canopa_files::transfers::TransferEvent;
    for kind in [TransferKind::Copy, TransferKind::Move] {
        for alias in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let destination = root.path().join("destination");
            let original = root.path().join("original destination");
            let replacement = root.path().join("replacement destination");
            fs::create_dir(&replacement).unwrap();
            if alias {
                fs::create_dir(&original).unwrap();
                symlink(&original, &destination).unwrap();
            } else {
                fs::create_dir(&destination).unwrap();
            }
            let first = root.path().join("a-completed");
            let pending = root.path().join("b-pending");
            fs::write(&first, b"Completed prefix").unwrap();
            fs::write(&pending, b"Originally requested bytes").unwrap();
            let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
            let stopped = engine
                .transfer(
                    &TransferRequest {
                        kind,
                        sources: vec![first.clone(), pending.clone()],
                        destination: destination.clone(),
                    },
                    &Cancellation::default(),
                    |event| {
                        if matches!(event, TransferEvent::Progress { completed: 1, .. }) {
                            TransferReply::Cancel
                        } else {
                            TransferReply::Continue
                        }
                    },
                )
                .unwrap();
            assert!(stopped.cancelled && stopped.completed == 1);
            if alias {
                fs::remove_file(&destination).unwrap();
                symlink(&replacement, &destination).unwrap();
            } else {
                fs::rename(&destination, &original).unwrap();
                fs::rename(&replacement, &destination).unwrap();
            }
            fs::write(destination.join("marker"), b"Unrelated folder bytes").unwrap();
            let refused = engine
                .resume_transfer(
                    stopped.continuation.as_ref().unwrap(),
                    &Cancellation::default(),
                    |event| {
                        if matches!(event, TransferEvent::Unavailable { .. }) {
                            TransferReply::Cancel
                        } else {
                            TransferReply::Continue
                        }
                    },
                )
                .unwrap();
            assert_eq!(
                refused.completed, 0,
                "{kind:?}, alias={alias}: Continue adopted a different destination folder"
            );
            assert_eq!(fs::read(&pending).unwrap(), b"Originally requested bytes");
            assert!(!destination.join("b-pending").exists());
            assert_eq!(
                fs::read(destination.join("marker")).unwrap(),
                b"Unrelated folder bytes"
            );
            assert_eq!(
                fs::read(original.join("a-completed")).unwrap(),
                b"Completed prefix"
            );
            assert_eq!(refused.remaining, vec![pending.clone()]);
            if alias {
                fs::remove_file(&destination).unwrap();
                symlink(&original, &destination).unwrap();
            } else {
                fs::rename(&destination, &replacement).unwrap();
                fs::rename(&original, &destination).unwrap();
            }
            let resumed = engine
                .resume_transfer(
                    refused.continuation.as_ref().unwrap(),
                    &Cancellation::default(),
                    |_| TransferReply::Continue,
                )
                .unwrap();
            assert_eq!(resumed.completed, 1);
            assert!(resumed.failures.is_empty());
            assert_eq!(
                fs::read(destination.join("b-pending")).unwrap(),
                b"Originally requested bytes"
            );
            engine.undo(&Cancellation::default()).unwrap();
            assert!(!destination.join("b-pending").exists());
            assert_eq!(
                fs::read(destination.join("a-completed")).unwrap(),
                b"Completed prefix"
            );
            engine.undo(&Cancellation::default()).unwrap();
            assert_eq!(fs::read(&first).unwrap(), b"Completed prefix");
            assert_eq!(fs::read(&pending).unwrap(), b"Originally requested bytes");
        }
    }
}

#[test]
fn a_destination_observed_before_cancellation_is_not_reselected_on_resume() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    let original = root.path().join("original destination");
    fs::create_dir(&destination).unwrap();
    let source = root.path().join("selected");
    fs::write(&source, b"Selected bytes").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let stopped = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &cancelled,
            |_| panic!("initial cancellation must not execute the transfer"),
        )
        .unwrap();
    assert!(stopped.cancelled && stopped.id.is_none());
    fs::rename(&destination, &original).unwrap();
    fs::create_dir(&destination).unwrap();
    let refused = engine
        .resume_transfer(
            stopped.continuation.as_ref().unwrap(),
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(refused.completed, 0);
    assert!(refused.needs_attention);
    assert_eq!(refused.remaining, vec![source.clone()]);
    assert_eq!(fs::read(&source).unwrap(), b"Selected bytes");
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    fs::remove_dir(&destination).unwrap();
    fs::rename(&original, &destination).unwrap();
    let resumed = engine
        .resume_transfer(
            refused.continuation.as_ref().unwrap(),
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(resumed.completed, 1);
    assert_eq!(
        fs::read(destination.join("selected")).unwrap(),
        b"Selected bytes"
    );
    assert!(!source.exists());
}

#[test]
fn continuing_a_transfer_keeps_its_original_selected_sources() {
    use canopa_files::transfers::TransferEvent;
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        let first = root.path().join("a-completed");
        let pending = root.path().join("b-pending");
        let original = root.path().join("parked original");
        fs::write(&first, b"Completed prefix").unwrap();
        fs::write(&pending, b"Originally selected bytes").unwrap();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let stopped = engine
            .transfer(
                &TransferRequest {
                    kind,
                    sources: vec![first.clone(), pending.clone()],
                    destination: destination.clone(),
                },
                &Cancellation::default(),
                |event| {
                    if matches!(event, TransferEvent::Progress { completed: 1, .. }) {
                        TransferReply::Cancel
                    } else {
                        TransferReply::Continue
                    }
                },
            )
            .unwrap();
        assert!(stopped.cancelled);
        assert_eq!(stopped.completed, 1);
        assert_eq!(stopped.remaining, vec![pending.clone()]);
        fs::rename(&pending, &original).unwrap();
        fs::write(&pending, b"Unrelated replacement bytes").unwrap();
        let refused = engine
            .resume_transfer(
                stopped.continuation.as_ref().unwrap(),
                &Cancellation::default(),
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert_eq!(
            refused.completed, 0,
            "{kind:?} Continue must preserve a replacement at the selected pathname"
        );
        assert_eq!(refused.failures.len(), 1);
        assert_eq!(fs::read(&pending).unwrap(), b"Unrelated replacement bytes");
        assert_eq!(fs::read(&original).unwrap(), b"Originally selected bytes");
        assert!(!destination.join("b-pending").exists());
        fs::rename(&pending, root.path().join("parked replacement")).unwrap();
        fs::rename(&original, &pending).unwrap();
        let resumed = engine
            .resume_transfer(
                refused.retry_continuation.as_ref().unwrap(),
                &Cancellation::default(),
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert_eq!(resumed.completed, 1);
        assert!(resumed.failures.is_empty());
        assert_eq!(
            fs::read(destination.join("b-pending")).unwrap(),
            b"Originally selected bytes"
        );
        engine.undo(&Cancellation::default()).unwrap();
        assert!(!destination.join("b-pending").exists());
        assert_eq!(
            fs::read(destination.join("a-completed")).unwrap(),
            b"Completed prefix"
        );
        assert_eq!(fs::read(&pending).unwrap(), b"Originally selected bytes");
        engine.undo(&Cancellation::default()).unwrap();
        assert_eq!(fs::read(&first).unwrap(), b"Completed prefix");
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    }
}

#[test]
fn a_continuation_paused_before_first_access_keeps_raw_symlink_identity() {
    use canopa_files::transfers::TransferAccess;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let source = root
        .path()
        .join(std::ffi::OsString::from_vec(b"selected-\xff".to_vec()));
    let parked = root.path().join("original link");
    symlink("original missing target", &source).unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    cancel.cancel();
    let stopped = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(stopped.cancelled && stopped.completed == 0);
    let still_waiting = engine
        .resume_transfer_with_check(
            stopped.continuation.as_ref().unwrap(),
            &Cancellation::default(),
            |_, access| {
                assert_eq!(access, TransferAccess::DestinationFolder);
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "private destination unavailable",
                ))
            },
            |_| panic!("unavailable placement must precede filesystem work"),
        )
        .unwrap();
    assert!(still_waiting.needs_attention && still_waiting.completed == 0);
    fs::rename(&source, &parked).unwrap();
    symlink("replacement missing target", &source).unwrap();
    let refused = engine
        .resume_transfer(
            still_waiting.continuation.as_ref().unwrap(),
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(refused.completed, 0);
    assert_eq!(refused.failures.len(), 1);
    assert_eq!(
        fs::read_link(&source).unwrap(),
        std::path::Path::new("replacement missing target")
    );
    assert_eq!(
        fs::read_link(&parked).unwrap(),
        std::path::Path::new("original missing target")
    );
    assert_eq!(fs::read_dir(destination).unwrap().count(), 0);
}

#[test]
fn a_live_continuation_cannot_change_kind_or_destination() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    let other = root.path().join("unrequested destination");
    fs::create_dir(&destination).unwrap();
    fs::create_dir(&other).unwrap();
    let source = root.path().join("selected");
    fs::write(&source, b"Original bytes").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    cancel.cancel();
    let stopped = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    let mut changed = stopped.continuation.as_ref().unwrap().clone();
    changed.kind = TransferKind::Move;
    assert!(
        engine
            .resume_transfer(&changed, &Cancellation::default(), |_| {
                panic!("changed intent must be refused before execution")
            })
            .is_err()
    );
    changed.kind = TransferKind::Copy;
    changed.destination = other.clone();
    assert!(
        engine
            .resume_transfer(&changed, &Cancellation::default(), |_| {
                panic!("changed intent must be refused before execution")
            })
            .is_err()
    );
    let resumed = engine
        .resume_transfer(
            stopped.continuation.as_ref().unwrap(),
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(resumed.completed, 1);
    assert_eq!(fs::read(&source).unwrap(), b"Original bytes");
    assert_eq!(
        fs::read(destination.join("selected")).unwrap(),
        b"Original bytes"
    );
    assert_eq!(fs::read_dir(other).unwrap().count(), 0);
}

#[test]
fn an_observed_batch_preserves_changed_folders_links_and_later_created_files() {
    use std::{collections::BTreeSet, path::Path};
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let folder = root.path().join("folder");
    let parked_folder = root.path().join("original folder");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("child"), b"Original folder child").unwrap();
    let link = root.path().join("link");
    let parked_link = root.path().join("original link");
    symlink("original missing target", &link).unwrap();
    let missing = root.path().join("not yet created");
    let unchanged = root.path().join("unchanged");
    fs::write(&unchanged, b"Deliberately selected file").unwrap();
    let dangling = root.path().join("dangling");
    symlink("another missing target", &dangling).unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let observed = TransferRequest {
        kind: TransferKind::Move,
        sources: vec![
            folder.clone(),
            link.clone(),
            missing.clone(),
            unchanged.clone(),
            dangling.clone(),
        ],
        destination: destination.clone(),
    }
    .observe_sources()
    .unwrap();
    fs::rename(&folder, &parked_folder).unwrap();
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("child"), b"Unrelated folder child").unwrap();
    fs::rename(&link, &parked_link).unwrap();
    symlink("replacement missing target", &link).unwrap();
    fs::write(&missing, b"Created after observation").unwrap();
    let report = engine
        .transfer_observed_with_check(
            &observed,
            &Cancellation::default(),
            |_, _| Ok(()),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(report.completed, 2);
    assert_eq!(
        report
            .failures
            .iter()
            .map(|failure| failure.source.clone())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([folder.clone(), link.clone(), missing.clone()])
    );
    assert_eq!(
        fs::read(folder.join("child")).unwrap(),
        b"Unrelated folder child"
    );
    assert_eq!(
        fs::read(parked_folder.join("child")).unwrap(),
        b"Original folder child"
    );
    assert_eq!(
        fs::read_link(&link).unwrap(),
        Path::new("replacement missing target")
    );
    assert_eq!(
        fs::read_link(&parked_link).unwrap(),
        Path::new("original missing target")
    );
    assert_eq!(fs::read(&missing).unwrap(), b"Created after observation");
    for name in ["folder", "link", "not yet created"] {
        assert!(fs::symlink_metadata(destination.join(name)).is_err());
    }
    assert_eq!(
        fs::read(destination.join("unchanged")).unwrap(),
        b"Deliberately selected file"
    );
    assert_eq!(
        fs::read_link(destination.join("dangling")).unwrap(),
        Path::new("another missing target")
    );
    assert!(fs::symlink_metadata(&unchanged).is_err());
    assert!(fs::symlink_metadata(&dangling).is_err());
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(unchanged).unwrap(), b"Deliberately selected file");
    assert_eq!(
        fs::read_link(dangling).unwrap(),
        Path::new("another missing target")
    );
    assert_eq!(
        fs::read(folder.join("child")).unwrap(),
        b"Unrelated folder child"
    );
    assert_eq!(fs::read(missing).unwrap(), b"Created after observation");
}

#[test]
fn answering_a_live_conflict_does_not_authorize_a_replacement_source() {
    use canopa_files::transfers::{ConflictChoice, TransferEvent};
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let source = root.path().join("selected");
    let parked = root.path().join("original selected file");
    fs::write(&source, b"Original selected bytes").unwrap();
    fs::write(destination.join("selected"), b"Original destination bytes").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let mut reviewed = false;
    let report = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Conflict { .. }) {
                    assert!(!reviewed);
                    reviewed = true;
                    fs::rename(&source, &parked).unwrap();
                    fs::write(&source, b"Unrelated replacement bytes").unwrap();
                    TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        apply_to_remaining: false,
                    }
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    assert!(reviewed);
    assert_eq!(report.completed, 0);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].source, source);
    assert_eq!(fs::read(&source).unwrap(), b"Unrelated replacement bytes");
    assert_eq!(fs::read(&parked).unwrap(), b"Original selected bytes");
    assert_eq!(
        fs::read(destination.join("selected")).unwrap(),
        b"Original destination bytes"
    );
    assert!(engine.latest_undo_id().is_none());
}

#[test]
fn destination_loss_during_a_copy_preserves_completed_work_and_exact_continuation() {
    use canopa_files::transfers::{TransferAccess, TransferEvent};
    use std::cell::Cell;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    let c = root.path().join("c");
    fs::write(&a, b"completed first").unwrap();
    fs::write(&b, vec![b'b'; 256 * 1024]).unwrap();
    fs::write(&c, b"still waiting").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let offline = Cell::new(false);
    let check = |_: &std::path::Path, access| {
        if offline.get() && access == TransferAccess::DestinationFolder {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "destination disconnected",
            ))
        } else {
            Ok(())
        }
    };
    let result = engine
        .transfer_with_check(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![a.clone(), b.clone(), c.clone()],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            check,
            |event| {
                if matches!(event, TransferEvent::Progress { ref path, .. } if path == &b) {
                    offline.set(true);
                }
                TransferReply::Continue
            },
        )
        .unwrap();
    assert!(result.needs_attention && !result.cancelled);
    assert_eq!(result.completed, 1);
    assert!(result.failures.is_empty());
    assert_eq!(result.remaining, vec![b.clone(), c.clone()]);
    assert_eq!(fs::read(destination.join("a")).unwrap(), b"completed first");
    assert!(!destination.join("b").exists() && !destination.join("c").exists());
    assert_eq!(fs::read(&b).unwrap(), vec![b'b'; 256 * 1024]);
    assert_eq!(fs::read(&c).unwrap(), b"still waiting");
    assert_eq!(
        engine
            .interrupted_operations(&Cancellation::default())
            .unwrap()
            .len(),
        1
    );
    let partial = fs::read_dir(&destination)
        .unwrap()
        .map(|entry| entry.unwrap().path().join("folder"))
        .find(|path| path.is_file())
        .unwrap();
    let partial_bytes = fs::read(&partial).unwrap();
    let still_offline = engine
        .resume_transfer_with_check(
            result.continuation.as_ref().unwrap(),
            &Cancellation::default(),
            check,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(still_offline.needs_attention && !still_offline.cancelled);
    assert_eq!(
        fs::read(&partial).expect("Continue must not clean unavailable storage"),
        partial_bytes
    );
    offline.set(false);
    let resumed = engine
        .resume_transfer_with_check(
            &result.continuation.unwrap(),
            &Cancellation::default(),
            check,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(resumed.completed, 2);
    assert!(resumed.failures.is_empty() && !resumed.needs_attention);
    assert_eq!(
        fs::read(destination.join("b")).unwrap(),
        vec![b'b'; 256 * 1024]
    );
    assert_eq!(fs::read(destination.join("c")).unwrap(), b"still waiting");
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    assert!(a.exists() && b.exists() && c.exists());
}

#[test]
fn dropping_on_the_same_filesystem_moves_and_control_forces_an_undoable_copy() {
    use canopa_files::transfers::DropPreference;
    let root = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let source = root.path().join("photo.png");
    let destination = root.path().join("sorted");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"image bytes").unwrap();
    let engine = OperationEngine::open(root.path().join("operations.sqlite3")).unwrap();
    let request = TransferRequest::from_drop(
        vec![source.clone()],
        destination.clone(),
        DropPreference::Automatic,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(request.kind, TransferKind::Move);
    let result = engine
        .transfer(&request, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(result.failures.is_empty());
    assert!(!source.exists());
    assert_eq!(
        fs::read(destination.join("photo.png")).unwrap(),
        b"image bytes"
    );
    engine.undo(&Cancellation::default()).unwrap();
    let request = TransferRequest::from_drop(
        vec![source.clone()],
        destination.clone(),
        DropPreference::Copy,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(request.kind, TransferKind::Copy);
    engine
        .transfer(&request, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(source.exists());
    engine.undo(&Cancellation::default()).unwrap();
    assert!(source.exists());
    assert!(!destination.join("photo.png").exists());
}

#[test]
fn cross_filesystem_drops_copy_by_default_and_shift_forces_a_move_without_following_links() {
    use canopa_files::transfers::DropPreference;
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let root_device = fs::metadata(root.path()).unwrap().dev();
    let other = ["/tmp", "/dev/shm"]
        .into_iter()
        .find_map(|base| {
            tempfile::tempdir_in(base).ok().filter(|directory| {
                fs::metadata(directory.path()).is_ok_and(|metadata| metadata.dev() != root_device)
            })
        })
        .expect("a writable second filesystem is required for the drop fixture");
    assert_ne!(
        fs::metadata(root.path()).unwrap().dev(),
        fs::metadata(other.path()).unwrap().dev(),
        "the cross-filesystem fixture must use different filesystems"
    );
    let source = root.path().join("source");
    fs::write(&source, b"keep until copied").unwrap();
    let request = TransferRequest::from_drop(
        vec![source.clone()],
        other.path().into(),
        DropPreference::Automatic,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(request.kind, TransferKind::Copy);
    let engine = OperationEngine::open(root.path().join("state.sqlite3")).unwrap();
    let result = engine
        .transfer(&request, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(result.failures.is_empty());
    assert!(source.exists());
    engine.undo(&Cancellation::default()).unwrap();
    let request = TransferRequest::from_drop(
        vec![source.clone()],
        other.path().into(),
        DropPreference::Move,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(request.kind, TransferKind::Move);
    let result = engine
        .transfer(&request, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(result.failures.is_empty());
    assert!(!source.exists());
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(&source).unwrap(), b"keep until copied");
    let link = other.path().join("link");
    symlink(&source, &link).unwrap();
    let request = TransferRequest::from_drop(
        vec![link],
        other.path().into(),
        DropPreference::Automatic,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        request.kind,
        TransferKind::Move,
        "the link belongs to its own filesystem"
    );
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert_eq!(
        TransferRequest::from_drop(
            vec![source],
            other.path().into(),
            DropPreference::Move,
            &cancelled
        )
        .unwrap_err()
        .kind(),
        std::io::ErrorKind::Interrupted
    );
}

#[test]
fn a_missing_dragged_item_does_not_prevent_the_other_files_from_transferring() {
    use canopa_files::transfers::DropPreference;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let good = root.path().join("good");
    let missing = root.path().join("missing");
    fs::write(&good, b"completed independently").unwrap();
    let request = TransferRequest::from_drop(
        vec![missing.clone(), good.clone()],
        destination.clone(),
        DropPreference::Automatic,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        request.kind,
        TransferKind::Copy,
        "an unknown source filesystem uses a Copy default"
    );
    let engine = OperationEngine::open(root.path().join("operations.sqlite3")).unwrap();
    let outcome = engine
        .transfer(&request, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert_eq!(outcome.failures.len(), 1);
    assert_eq!(outcome.failures[0].source, missing);
    assert_eq!(
        fs::read(destination.join("good")).unwrap(),
        b"completed independently"
    );
    assert!(good.exists());
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!destination.join("good").exists());
}

#[test]
fn resuming_a_kept_both_folder_keeps_its_exact_destination_mapping() {
    use canopa_files::transfers::{ConflictChoice, TransferEvent};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("project");
    let destination = root.path().join("destination");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(destination.join("project")).unwrap();
    fs::write(source.join("a"), b"first").unwrap();
    fs::write(source.join("b"), b"second").unwrap();
    fs::write(destination.join("project/existing"), b"older work").unwrap();
    let history = root.path().join("state/operations.sqlite3");
    let engine = OperationEngine::open(&history).unwrap();
    let outcome = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| match event {
                TransferEvent::Conflict { folders: true, .. } => TransferReply::Resolve {
                    choice: ConflictChoice::KeepBoth,
                    apply_to_remaining: false,
                },
                TransferEvent::Progress {
                    path, completed: 2, ..
                } if path.ends_with("a") => TransferReply::Cancel,
                _ => TransferReply::Continue,
            },
        )
        .unwrap();
    assert!(outcome.cancelled);
    assert_eq!(
        fs::read(destination.join("project (2)/a")).unwrap(),
        b"first"
    );
    assert!(!destination.join("project (2)/b").exists());
    let stored = serde_json::to_vec(&outcome.continuation.unwrap()).unwrap();
    drop(engine);
    let restored = serde_json::from_slice(&stored).unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    let resumed = engine
        .resume_transfer(&restored, &Cancellation::default(), |event| {
            assert!(!matches!(event, TransferEvent::Conflict { .. }));
            TransferReply::Continue
        })
        .unwrap();
    assert!(resumed.failures.is_empty());
    assert_eq!(
        fs::read(destination.join("project (2)/b")).unwrap(),
        b"second"
    );
    assert_eq!(
        fs::read(destination.join("project/existing")).unwrap(),
        b"older work"
    );
    assert!(!destination.join("project (3)").exists());
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!destination.join("project (2)/b").exists());
    assert!(destination.join("project (2)/a").exists());
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!destination.join("project (2)").exists());
}

#[test]
fn copying_a_tree_preserves_raw_names_and_links_and_is_one_persistent_undo_entry() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::create_dir(&destination).unwrap();
    let raw = std::ffi::OsString::from_vec(b"notes-\xff".to_vec());
    fs::write(source.join("nested").join(&raw), b"copied bytes").unwrap();
    symlink("nested", source.join("link")).unwrap();
    let journal = root.path().join("state/operations.json");
    let engine = OperationEngine::open(&journal).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(result.failures.is_empty());
    assert!(!result.cancelled);
    let copied = destination.join("source");
    assert_eq!(
        fs::read(copied.join("nested").join(&raw)).unwrap(),
        b"copied bytes"
    );
    assert_eq!(
        fs::read_link(copied.join("link")).unwrap(),
        std::path::Path::new("nested")
    );
    assert_eq!(
        fs::read(source.join("nested").join(raw)).unwrap(),
        b"copied bytes"
    );
    drop(engine);
    let engine = OperationEngine::open(&journal).unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!copied.exists());
    assert!(source.is_dir());
    assert!(engine.undo_label().is_none());
}

#[test]
fn cancelling_mid_copy_keeps_completed_items_and_removes_the_unfinished_copy() {
    use canopa_files::transfers::TransferEvent;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("a"), b"complete").unwrap();
    fs::write(source.join("b"), vec![7; 512 * 1024]).unwrap();
    fs::write(source.join("c"), b"pending").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let result = engine.transfer(&TransferRequest { kind: TransferKind::Copy, sources: ["a", "b", "c"].map(|name| source.join(name)).to_vec(), destination: destination.clone() }, &Cancellation::default(), |event| {
        if matches!(event, TransferEvent::Progress { ref path, completed: 1, .. } if path.ends_with("b")) { TransferReply::Cancel } else { TransferReply::Continue }
    }).unwrap();
    assert!(result.cancelled);
    assert_eq!(result.completed, 1);
    assert_eq!(fs::read(destination.join("a")).unwrap(), b"complete");
    assert!(!destination.join("b").exists());
    assert!(!destination.join("c").exists());
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
    assert_eq!(fs::read_dir(&source).unwrap().count(), 3);
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
}

#[test]
fn a_cancelled_move_and_an_isolated_failure_have_one_undo_for_actual_completed_work() {
    use canopa_files::transfers::TransferEvent;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    let missing = root.path().join("0-missing");
    fs::write(&a, b"a").unwrap();
    fs::write(&b, b"b").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![missing, a.clone(), b.clone()],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Progress { completed: 1, .. }) {
                    TransferReply::Cancel
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.completed, 1);
    assert!(result.cancelled);
    assert!(!a.exists());
    assert!(b.exists());
    assert!(destination.join("a").exists());
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(a).unwrap(), b"a");
    assert!(b.exists());
    assert!(!destination.join("a").exists());
}

#[test]
fn copied_files_with_newer_contents_are_preserved_when_undoing_a_batch() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let file = root.path().join("file");
    fs::write(&file, b"initial").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![file],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    fs::write(destination.join("file"), b"newer work").unwrap();
    assert!(engine.undo(&Cancellation::default()).is_err());
    assert_eq!(fs::read(destination.join("file")).unwrap(), b"newer work");
    assert!(engine.undo_label().is_some());
}

#[test]
fn forced_cross_filesystem_moves_commit_before_removing_the_source_and_undo_back() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let destination = tempfile::Builder::new()
        .prefix(".canopa-cross-filesystem-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    if fs::metadata(root.path()).unwrap().dev() == fs::metadata(destination.path()).unwrap().dev() {
        eprintln!("cross-filesystem fixture unavailable on this runner");
        return;
    }
    let source = root.path().join("photo");
    fs::write(&source, vec![8; 128 * 1024]).unwrap();
    let journal = root.path().join("state/operations.sqlite3");
    let engine = OperationEngine::open(&journal).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.path().to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(result.failures.is_empty(), "{:?}", result.failures);
    assert!(!source.exists());
    assert_eq!(
        fs::read(destination.path().join("photo")).unwrap(),
        vec![8; 128 * 1024]
    );
    drop(engine);
    let engine = OperationEngine::open(&journal).unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(source).unwrap(), vec![8; 128 * 1024]);
    assert_eq!(fs::read_dir(destination.path()).unwrap().count(), 0);
}

fn cross_fixture() -> Option<(tempfile::TempDir, tempfile::TempDir)> {
    use std::os::unix::fs::MetadataExt;
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::Builder::new()
        .prefix(".canopa-cross-filesystem-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    if fs::metadata(source.path()).unwrap().dev() == fs::metadata(destination.path()).unwrap().dev()
    {
        eprintln!("cross-filesystem fixture unavailable on this runner");
        None
    } else {
        Some((source, destination))
    }
}

#[test]
fn losing_a_move_source_after_quarantine_preserves_its_payload_for_recovery() {
    use canopa_files::transfers::TransferAccess;
    use std::cell::Cell;
    let Some((root, destination)) = cross_fixture() else {
        return;
    };
    let source = root.path().join("photo");
    fs::write(&source, vec![b'p'; 128 * 1024]).unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let lost = Cell::new(false);
    let result = engine
        .transfer_with_check(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.path().to_owned(),
            },
            &Cancellation::default(),
            |path, access| {
                if access == TransferAccess::SourceEntry && path == source && !source.exists() {
                    lost.set(true);
                }
                if lost.get() && path.starts_with(root.path()) {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotConnected,
                        "source device disappeared",
                    ))
                } else {
                    Ok(())
                }
            },
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(
        lost.get(),
        "source availability must be checked after quarantine and before deletion"
    );
    assert!(result.needs_attention && !result.cancelled);
    let payload = fs::read_dir(root.path())
        .unwrap()
        .map(|entry| entry.unwrap().path().join("folder"))
        .find(|path| path.is_file())
        .expect("interrupted source payload stays owned by its journal");
    assert_eq!(fs::read(&payload).unwrap(), vec![b'p'; 128 * 1024]);
    assert_eq!(
        fs::read(destination.path().join("photo")).unwrap(),
        vec![b'p'; 128 * 1024]
    );
    assert_eq!(
        engine
            .interrupted_operations(&Cancellation::default())
            .unwrap()
            .len(),
        1
    );
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert_eq!(fs::read(&source).unwrap(), vec![b'p'; 128 * 1024]);
    assert!(!payload.exists());
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(&source).unwrap(), vec![b'p'; 128 * 1024]);
    assert!(!destination.path().join("photo").exists());
}

#[test]
fn cross_filesystem_directory_moves_restore_nested_files_and_links_with_one_undo() {
    let Some((root, destination)) = cross_fixture() else {
        return;
    };
    let source = root.path().join("Photos");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::write(source.join("nested/photo"), b"photo").unwrap();
    symlink("nested", source.join("link")).unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.path().to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(result.failures.is_empty(), "{:?}", result.failures);
    assert!(!source.exists());
    assert_eq!(
        fs::read(destination.path().join("Photos/nested/photo")).unwrap(),
        b"photo"
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(source.join("nested/photo")).unwrap(), b"photo");
    assert_eq!(
        fs::read_link(source.join("link")).unwrap(),
        std::path::Path::new("nested")
    );
    assert_eq!(fs::read_dir(destination.path()).unwrap().count(), 0);
}

#[test]
fn cancelling_cross_filesystem_move_before_source_removal_rolls_back_its_unfinished_copy() {
    use canopa_files::transfers::TransferEvent;
    let Some((root, destination)) = cross_fixture() else {
        return;
    };
    let source = root.path().join("photo");
    fs::write(&source, b"incoming").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.path().to_owned(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Verifying { .. }) {
                    TransferReply::Cancel
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    assert!(result.cancelled);
    assert!(result.failures.is_empty());
    assert_eq!(fs::read(source).unwrap(), b"incoming");
    assert_eq!(fs::read_dir(destination.path()).unwrap().count(), 0);
    assert!(engine.undo_label().is_none());
}

#[test]
fn source_changes_after_cross_filesystem_copy_are_retained_at_source() {
    use canopa_files::transfers::TransferEvent;
    let Some((root, destination)) = cross_fixture() else {
        return;
    };
    let source = root.path().join("photo");
    fs::write(&source, b"incoming").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.path().to_owned(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Verifying { .. }) {
                    fs::write(&source, b"newer source").unwrap();
                }
                TransferReply::Continue
            },
        )
        .unwrap();
    assert_eq!(result.failures.len(), 1);
    assert_eq!(fs::read(&source).unwrap(), b"newer source");
    assert_eq!(
        fs::read(destination.path().join("photo")).unwrap(),
        b"incoming"
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(source).unwrap(), b"newer source");
    assert_eq!(fs::read_dir(destination.path()).unwrap().count(), 0);
}

#[test]
fn older_copy_and_rename_history_survive_a_cross_filesystem_round_trip() {
    use canopa_files::operations::OperationRequest;
    let Some((root, destination)) = cross_fixture() else {
        return;
    };
    let seed = root.path().join("photo");
    fs::write(&seed, b"photo").unwrap();
    let folder = root.path().join("source");
    fs::create_dir(&folder).unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed.clone()],
                destination: folder.clone(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    let renamed = folder.join("organized");
    engine
        .execute(
            OperationRequest::Rename {
                source: folder.join("photo"),
                destination: renamed.clone(),
            },
            &Cancellation::default(),
        )
        .unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![renamed.clone()],
                destination: destination.path().to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(&renamed).unwrap(), b"photo");
    // Keeping both during a later folder reversal must also relocate the inode
    // bindings created by the cross-filesystem round trip.
    engine
        .execute(
            OperationRequest::Rename {
                source: folder.clone(),
                destination: root.path().join("moved-folder"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("newer"), b"newer work").unwrap();
    engine
        .undo_interactive(None, &Cancellation::default(), |_| {
            canopa_files::operations::UndoChoice::KeepBoth
        })
        .unwrap();
    let restored_folder = root.path().join("source (2)");
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(restored_folder.join("photo")).unwrap(), b"photo");
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read_dir(restored_folder).unwrap().count(), 0);
    assert_eq!(fs::read(folder.join("newer")).unwrap(), b"newer work");
    assert_eq!(fs::read(seed).unwrap(), b"photo");
    assert!(engine.undo_label().is_none());
}

#[test]
fn merging_a_move_keeps_both_conflicting_files_and_restores_the_original_folder_on_undo() {
    use canopa_files::transfers::{ConflictChoice, TransferEvent};
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("Photos");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir_all(destination.join("Photos")).unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o750)).unwrap();
    for name in ["a.jpg", "b.jpg"] {
        fs::write(source.join(name), b"incoming").unwrap();
        fs::write(destination.join("Photos").join(name), b"existing").unwrap();
    }
    let path = root.path().join("state/operations.sqlite3");
    let engine = OperationEngine::open(&path).unwrap();
    let mut file_prompts = 0;
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| {
                if let TransferEvent::Conflict { folders, .. } = event {
                    if !folders {
                        file_prompts += 1;
                    }
                    TransferReply::Resolve {
                        choice: if folders {
                            ConflictChoice::Merge
                        } else {
                            ConflictChoice::KeepBoth
                        },
                        apply_to_remaining: true,
                    }
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    assert!(result.failures.is_empty(), "{:?}", result.failures);
    assert_eq!(file_prompts, 1);
    assert!(!source.exists());
    assert_eq!(
        fs::read(destination.join("Photos/a (2).jpg")).unwrap(),
        b"incoming"
    );
    drop(engine);
    let engine = OperationEngine::open(&path).unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(
        fs::metadata(&source).unwrap().permissions().mode() & 0o777,
        0o750
    );
    for name in ["a.jpg", "b.jpg"] {
        assert_eq!(fs::read(source.join(name)).unwrap(), b"incoming");
        assert_eq!(
            fs::read(destination.join("Photos").join(name)).unwrap(),
            b"existing"
        );
    }
    assert_eq!(fs::read_dir(destination.join("Photos")).unwrap().count(), 2);
}

#[test]
fn replacing_files_keeps_the_previous_destination_until_persistent_undo() {
    use canopa_files::transfers::{ConflictChoice, TransferEvent};
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        let source = root.path().join("photo");
        fs::write(&source, b"incoming photo").unwrap();
        fs::write(destination.join("photo"), b"previous destination").unwrap();
        let path = root.path().join("state/operations.sqlite3");
        let engine = OperationEngine::open(&path).unwrap();
        let result = engine
            .transfer(
                &TransferRequest {
                    kind,
                    sources: vec![source.clone()],
                    destination: destination.clone(),
                },
                &Cancellation::default(),
                |event| {
                    if matches!(event, TransferEvent::Conflict { .. }) {
                        TransferReply::Resolve {
                            choice: ConflictChoice::Replace,
                            apply_to_remaining: true,
                        }
                    } else {
                        TransferReply::Continue
                    }
                },
            )
            .unwrap();
        assert!(result.failures.is_empty(), "{:?}", result.failures);
        assert_eq!(
            fs::read(destination.join("photo")).unwrap(),
            b"incoming photo"
        );
        assert_eq!(source.exists(), kind == TransferKind::Copy);
        drop(engine);
        let engine = OperationEngine::open(&path).unwrap();
        engine.undo(&Cancellation::default()).unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"incoming photo");
        assert_eq!(
            fs::read(destination.join("photo")).unwrap(),
            b"previous destination"
        );
        assert_eq!(fs::read_dir(destination).unwrap().count(), 1);
    }
}

#[test]
fn replacing_a_destination_that_changed_during_the_prompt_does_not_overwrite_it() {
    use canopa_files::transfers::{ConflictChoice, TransferEvent};
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let source = root.path().join("file");
    fs::write(&source, b"incoming").unwrap();
    fs::write(destination.join("file"), b"previous").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Conflict { .. }) {
                    fs::rename(destination.join("file"), destination.join("saved-previous"))
                        .unwrap();
                    fs::write(destination.join("file"), b"newer work").unwrap();
                    TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        apply_to_remaining: false,
                    }
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.completed, 0);
    assert_eq!(fs::read(source).unwrap(), b"incoming");
    assert_eq!(fs::read(destination.join("file")).unwrap(), b"newer work");
    assert_eq!(
        fs::read(destination.join("saved-previous")).unwrap(),
        b"previous"
    );
    assert_eq!(fs::read_dir(destination).unwrap().count(), 2);
}

#[test]
fn replacing_a_destination_changed_during_copy_cleans_only_its_own_staged_work() {
    use canopa_files::transfers::{ConflictChoice, TransferEvent};
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let source = root.path().join("file");
    let incoming = vec![42; 128 * 1024];
    fs::write(&source, &incoming).unwrap();
    fs::write(destination.join("file"), b"previous").unwrap();
    let history = root.path().join("history");
    let engine = OperationEngine::open(&history).unwrap();
    let mut changed = false;
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| match event {
                TransferEvent::Conflict { .. } => TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
                TransferEvent::Progress { bytes, .. } if bytes > 0 && !changed => {
                    fs::rename(destination.join("file"), destination.join("saved-previous"))
                        .unwrap();
                    fs::write(destination.join("file"), b"newer work").unwrap();
                    changed = true;
                    TransferReply::Continue
                }
                _ => TransferReply::Continue,
            },
        )
        .unwrap();
    assert!(changed);
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.completed, 0);
    drop(engine);
    for _ in 0..2 {
        let engine = OperationEngine::open(&history).unwrap();
        assert!(
            engine
                .interrupted_operations(&Cancellation::default())
                .unwrap()
                .is_empty()
        );
        assert_eq!(fs::read(&source).unwrap(), incoming);
        assert_eq!(fs::read(destination.join("file")).unwrap(), b"newer work");
        assert_eq!(
            fs::read(destination.join("saved-previous")).unwrap(),
            b"previous"
        );
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 2);
    }
}

#[test]
fn in_place_edits_while_the_conflict_prompt_is_open_are_not_silently_replaced() {
    use canopa_files::transfers::{ConflictChoice, TransferEvent};
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let source = root.path().join("file");
    fs::write(&source, b"incoming").unwrap();
    fs::write(destination.join("file"), b"previous").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Conflict { .. }) {
                    fs::write(destination.join("file"), b"newer saved version").unwrap();
                    TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        apply_to_remaining: false,
                    }
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.completed, 0);
    assert_eq!(
        fs::read(destination.join("file")).unwrap(),
        b"newer saved version"
    );
    assert_eq!(fs::read_dir(destination).unwrap().count(), 1);
}

#[test]
fn changed_sources_fail_without_committing_a_partial_copy_and_other_items_continue() {
    use canopa_files::transfers::TransferEvent;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    fs::write(&a, vec![3; 128 * 1024]).unwrap();
    fs::write(&b, b"b").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let mut changed = false;
    let result = engine.transfer(
        &TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![a.clone(), b],
            destination: destination.clone(),
        },
        &Cancellation::default(),
        |event| {
            if !changed && matches!(event, TransferEvent::Progress { ref path, .. } if path == &a) {
                fs::write(&a, b"newer source").unwrap();
                changed = true;
            }
            TransferReply::Continue
        },
    );
    let result = result.unwrap();
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.completed, 1);
    assert!(!destination.join("a").exists());
    assert_eq!(fs::read(a).unwrap(), b"newer source");
    assert_eq!(fs::read(destination.join("b")).unwrap(), b"b");
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert!(
        page.interrupted.is_empty(),
        "failed partial retention must retire its exact journal"
    );
    let partial = page
        .items
        .iter()
        .find(|item| {
            item.entry.as_ref().is_some_and(|entry| {
                fs::metadata(&entry.path).is_ok_and(|metadata| metadata.is_file())
            })
        })
        .expect("old partial bytes are protected in Recovery");
    let bytes = fs::read(&partial.entry.as_ref().unwrap().path).unwrap();
    assert!(!bytes.is_empty() && bytes.iter().all(|byte| *byte == 3));
    let roots: std::collections::HashSet<_> = page
        .items
        .iter()
        .filter_map(|item| item.entry.as_ref())
        .filter(|entry| fs::metadata(&entry.path).is_ok_and(|metadata| metadata.is_dir()))
        .map(|entry| entry.path.clone())
        .collect();
    assert_eq!(roots.len(), 2);
    assert!(fs::read_dir(&destination).unwrap().all(|entry| {
        let path = entry.unwrap().path();
        path == destination.join("b") || roots.contains(&path)
    }));
    engine
        .recover_copy(
            partial,
            &root.path().join("rescued-partial"),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(fs::read(root.path().join("rescued-partial")).is_ok_and(|actual| actual == bytes));
}

#[test]
fn a_replaced_destination_folder_pauses_instead_of_copying_into_the_new_folder() {
    use canopa_files::transfers::TransferEvent;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    fs::write(&a, b"a").unwrap();
    fs::write(&b, b"b").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let mut replaced = false;
    let mut attention = false;
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![a, b],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| {
                if !replaced && matches!(event, TransferEvent::Progress { completed: 1, .. }) {
                    fs::rename(&destination, root.path().join("old-destination")).unwrap();
                    fs::create_dir(&destination).unwrap();
                    replaced = true;
                }
                if matches!(event, TransferEvent::Unavailable { .. }) {
                    attention = true;
                }
                TransferReply::Continue
            },
        )
        .unwrap();
    assert!(attention && result.needs_attention);
    assert_eq!(result.completed, 1);
    assert_eq!(result.remaining.len(), 1);
    assert_eq!(fs::read_dir(destination).unwrap().count(), 0);
    assert_eq!(
        fs::read(root.path().join("old-destination/a")).unwrap(),
        b"a"
    );
}

#[test]
fn expired_history_releases_retained_data_without_deleting_current_files() {
    use canopa_files::{
        operations::OperationRequest,
        transfers::{ConflictChoice, TransferEvent},
    };
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let source = root.path().join("file");
    fs::write(&source, b"incoming").unwrap();
    fs::write(destination.join("file"), b"previous").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Conflict { .. }) {
                    TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        apply_to_remaining: false,
                    }
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 2);
    fs::write(destination.join("file"), b"newer work").unwrap();
    for number in 0..100 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("new-{number}")),
                },
                &Cancellation::default(),
            )
            .unwrap();
    }
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
    assert_eq!(fs::read(destination.join("file")).unwrap(), b"newer work");
}

#[test]
fn newer_work_saved_through_an_open_handle_to_a_retained_file_does_not_expire() {
    use canopa_files::{
        operations::OperationRequest,
        transfers::{ConflictChoice, TransferEvent},
    };
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let source = root.path().join("file");
    fs::write(&source, b"incoming").unwrap();
    fs::write(destination.join("file"), b"previous").unwrap();
    let mut editor = fs::OpenOptions::new()
        .write(true)
        .open(destination.join("file"))
        .unwrap();
    let path = root.path().join("state/operations.sqlite3");
    let engine = OperationEngine::open(&path).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Conflict { .. }) {
                    TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        apply_to_remaining: false,
                    }
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    editor.write_all(b"newer work").unwrap();
    editor.sync_all().unwrap();
    for number in 0..100 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("new-{number}")),
                },
                &Cancellation::default(),
            )
            .unwrap();
    }
    drop(engine);
    let _engine = OperationEngine::open(&path).unwrap();
    let stage = fs::read_dir(&destination)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.is_dir())
        .expect("retained recovery work");
    assert_eq!(fs::read(stage.join("previous")).unwrap(), b"newer work");
    assert_eq!(fs::read(destination.join("file")).unwrap(), b"incoming");
}

#[test]
fn restarting_after_a_killed_copy_cleans_the_partial_file_and_keeps_completed_undo() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let root = tempfile::Builder::new()
        .prefix("canopa-transfer-crash-")
        .tempdir()
        .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "crash_copy_helper"])
        .env("CANOPA_FILES_CRASH_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let destination = root.path().join("destination");
    assert!(fs::read_dir(&destination).unwrap().count() >= 2);
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
    assert_eq!(fs::read(destination.join("a")).unwrap(), b"complete");
    assert_eq!(
        fs::metadata(root.path().join("source/b")).unwrap().len(),
        512 * 1024
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read_dir(destination).unwrap().count(), 0);
}

#[test]
#[ignore = "child process fixture; the parent kills it during an actual copy"]
fn crash_copy_helper() {
    use canopa_files::transfers::TransferEvent;
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_FILES_CRASH_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-transfer-crash-")
    );
    let source = root.join("source");
    let destination = root.join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("a"), b"complete").unwrap();
    fs::write(source.join("b"), vec![4; 512 * 1024]).unwrap();
    let engine = OperationEngine::open(root.join("state/operations.sqlite3")).unwrap();
    engine.transfer(&TransferRequest { kind: TransferKind::Copy, sources: vec![source.join("a"), source.join("b")], destination }, &Cancellation::default(), |event| {
        if matches!(event, TransferEvent::Progress { ref path, completed: 1, .. } if path.ends_with("b")) { rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL).unwrap(); }
        TransferReply::Continue
    }).unwrap();
    panic!("copy fixture did not reach the crash point");
}
