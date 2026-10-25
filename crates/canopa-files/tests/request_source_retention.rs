use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, requests::RequestPhase},
    transfers::{ConflictChoice, TransferEvent, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn replace(engine: &OperationEngine, source: &Path, destination: &Path) {
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.to_owned()],
                destination: destination.to_owned(),
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
}

fn expire_history(engine: &OperationEngine, root: &Path) {
    for index in 0..100 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.join(format!("history-{index}")),
                },
                &Cancellation::default(),
            )
            .unwrap();
    }
}

fn stored_version(root: &Path) -> (OperationEngine, PathBuf) {
    let incoming = root.join("incoming");
    let destination = root.join("destination");
    fs::create_dir(&incoming).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(incoming.join("note.txt"), b"Current destination bytes").unwrap();
    fs::write(destination.join("note.txt"), b"Older retained bytes").unwrap();
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    replace(&engine, &incoming.join("note.txt"), &destination);
    let saved = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| item.entry.map(|entry| entry.path))
        .unwrap();
    (engine, saved)
}

#[test]
fn an_unrelated_rename_cannot_consume_a_retained_source_until_its_owners_release() {
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let clipboard = engine.lease_retained_sources(&captured).unwrap();
    let queued = engine
        .queue_request_with_sources(&"Paste still needs these bytes", &captured)
        .unwrap();
    let renamed = root.path().join("renamed saved bytes");
    let rename = || OperationRequest::Rename {
        source: saved.clone(),
        destination: renamed.clone(),
    };
    let before = engine.latest_undo_id();
    let result = engine.execute(rename(), &Cancellation::default());
    assert!(
        result.is_err(),
        "an unrelated rename consumed an owned retained source"
    );
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    assert!(!renamed.exists());
    assert_eq!(engine.latest_undo_id(), before);
    clipboard.release().unwrap();
    assert!(engine.execute(rename(), &Cancellation::default()).is_err());
    engine.dismiss_request(&queued).unwrap();
    engine.execute(rename(), &Cancellation::default()).unwrap();
    assert!(!saved.exists());
    assert_eq!(fs::read(&renamed).unwrap(), b"Older retained bytes");
    assert_eq!(
        fs::read(root.path().join("destination/note.txt")).unwrap(),
        b"Current destination bytes"
    );
}

#[test]
fn trash_preserves_a_retained_source_until_its_clipboard_lease_releases() {
    use canopa_files::operations::trash::TrashLocation;
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let clipboard = engine.lease_retained_sources(&captured).unwrap();
    let location = TrashLocation::home(root.path().join("private Trash")).unwrap();
    let before = engine.latest_undo_id();
    let refused = engine
        .trash(
            &location,
            std::slice::from_ref(&saved),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        refused.completed, 0,
        "Trash consumed a clipboard-owned retained version"
    );
    assert_eq!(refused.failures.len(), 1);
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    assert_eq!(engine.latest_undo_id(), before);
    assert!(location.list(&Cancellation::default()).unwrap().is_empty());
    clipboard.release().unwrap();
    let trashed = engine
        .trash(
            &location,
            std::slice::from_ref(&saved),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(trashed.completed, 1);
    assert!(trashed.failures.is_empty());
    let items = location.list(&Cancellation::default()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(fs::read(&items[0].path).unwrap(), b"Older retained bytes");
}

#[test]
fn replacement_copy_cannot_change_a_reserved_retained_payload() {
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let clipboard = engine.lease_retained_sources(&captured).unwrap();
    let incoming = root.path().join("unrelated replacement");
    fs::create_dir(&incoming).unwrap();
    let incoming = incoming.join(saved.file_name().unwrap());
    fs::write(&incoming, b"Unrelated replacement bytes").unwrap();
    let request = TransferRequest {
        kind: TransferKind::Copy,
        sources: vec![incoming.clone()],
        destination: saved.parent().unwrap().to_owned(),
    };
    let before = engine.latest_undo_id();
    let outcome = engine
        .transfer(&request, &Cancellation::default(), |event| match event {
            TransferEvent::Conflict { .. } => TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
            _ => TransferReply::Continue,
        })
        .unwrap();
    assert_eq!(
        fs::read(&saved).unwrap(),
        b"Older retained bytes",
        "Replace changed a clipboard-owned retained payload"
    );
    assert_eq!(outcome.completed, 0);
    assert_eq!(outcome.failures.len(), 1);
    assert_eq!(engine.latest_undo_id(), before);
    assert_eq!(fs::read(&incoming).unwrap(), b"Unrelated replacement bytes");
    clipboard.release().unwrap();
    replace(&engine, &incoming, saved.parent().unwrap());
    assert_eq!(fs::read(&saved).unwrap(), b"Unrelated replacement bytes");
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
}

#[test]
fn a_copy_journal_blocks_new_ownership_before_replacing_retained_bytes() {
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let incoming = root.path().join("journal replacement");
    fs::create_dir(&incoming).unwrap();
    let incoming = incoming.join(saved.file_name().unwrap());
    fs::write(&incoming, vec![b'J'; 1024 * 1024]).unwrap();
    let mut acquired = None;
    let outcome = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![incoming],
                destination: saved.parent().unwrap().to_owned(),
            },
            &Cancellation::default(),
            |event| match event {
                TransferEvent::Conflict { .. } => TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
                TransferEvent::Progress { .. } => {
                    if acquired.is_none() {
                        acquired = Some(engine.lease_retained_sources(&captured));
                    }
                    TransferReply::Continue
                }
                _ => TransferReply::Continue,
            },
        )
        .unwrap();
    let refused = match acquired.expect("the actual copy never reported progress") {
        Ok(lease) => {
            lease.release().unwrap();
            false
        }
        Err(error) => {
            assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
            true
        }
    };
    assert!(
        refused,
        "ownership was acquired after replacement journal admission"
    );
    assert_eq!(outcome.completed, 1);
    assert!(outcome.failures.is_empty());
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
}

#[test]
fn unrelated_moves_preserve_reserved_sources_and_any_confirmed_copy_stays_undoable() {
    use std::os::unix::fs::MetadataExt;
    for cross_filesystem in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (engine, saved) = stored_version(root.path());
        let other = if cross_filesystem {
            let device = fs::metadata(root.path()).unwrap().dev();
            Some(
                [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
                    .into_iter()
                    .find_map(|base| {
                        tempfile::Builder::new()
                            .prefix(".retained-source-consumption-")
                            .tempdir_in(base)
                            .ok()
                            .filter(|folder| fs::metadata(folder.path()).unwrap().dev() != device)
                    })
                    .expect("a writable second filesystem is required"),
            )
        } else {
            None
        };
        let destination = other.as_ref().map_or_else(
            || root.path().join("Move destination"),
            |other| other.path().join("Move destination"),
        );
        fs::create_dir(&destination).unwrap();
        let target = destination.join(saved.file_name().unwrap());
        let captured = engine
            .capture_retained_sources(std::slice::from_ref(&saved))
            .unwrap();
        let clipboard = engine.lease_retained_sources(&captured).unwrap();
        let request = TransferRequest {
            kind: TransferKind::Move,
            sources: vec![saved.clone()],
            destination,
        };
        let before = engine.latest_undo_id();
        let outcome = engine
            .transfer(&request, &Cancellation::default(), |_| {
                TransferReply::Continue
            })
            .unwrap();
        assert!(
            saved.exists(),
            "Move consumed a reserved source (cross-filesystem={cross_filesystem})"
        );
        assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
        assert_eq!(outcome.completed, 0);
        assert_eq!(outcome.failures.len(), 1);
        if target.exists() {
            assert_eq!(fs::read(&target).unwrap(), b"Older retained bytes");
            assert!(outcome.id.is_some(), "a committed copy lost its Undo entry");
            engine.undo(&Cancellation::default()).unwrap();
            assert!(!target.exists());
            assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
        } else {
            assert_eq!(engine.latest_undo_id(), before);
        }
        clipboard.release().unwrap();
        let moved = engine
            .transfer(&request, &Cancellation::default(), |_| {
                TransferReply::Continue
            })
            .unwrap();
        assert_eq!(moved.completed, 1);
        assert!(moved.failures.is_empty());
        assert!(!saved.exists());
        assert_eq!(fs::read(&target).unwrap(), b"Older retained bytes");
        engine.undo(&Cancellation::default()).unwrap();
        assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
        assert!(!target.exists());
    }
}

#[test]
fn a_cross_filesystem_move_cannot_remove_an_empty_folder_inside_a_leased_container() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let source = saved.parent().unwrap().join("empty selected folder");
    fs::create_dir(&source).unwrap();
    let device = fs::metadata(root.path()).unwrap().dev();
    let other = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|base| {
            tempfile::Builder::new()
                .prefix(".retained-empty-folder-")
                .tempdir_in(base)
                .ok()
                .filter(|folder| fs::metadata(folder.path()).unwrap().dev() != device)
        })
        .expect("a writable second filesystem is required");
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&source))
        .unwrap();
    let lease = engine.lease_retained_sources(&captured).unwrap();
    let request = TransferRequest {
        kind: TransferKind::Move,
        sources: vec![source.clone()],
        destination: other.path().to_owned(),
    };
    let target = other.path().join(source.file_name().unwrap());
    let before = engine.latest_undo_id();
    let outcome = engine
        .transfer(&request, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(source.is_dir(), "Move removed a leased empty folder");
    assert_eq!(outcome.failures.len(), 1);
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    if target.exists() {
        assert!(target.is_dir());
        assert!(
            outcome.id.is_some(),
            "a committed folder lost its Undo entry"
        );
        engine.undo(&Cancellation::default()).unwrap();
        assert!(!target.exists());
        assert!(source.is_dir());
    } else {
        assert_eq!(engine.latest_undo_id(), before);
    }
    lease.release().unwrap();
    let outcome = engine
        .transfer(&request, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(outcome.failures.is_empty());
    assert!(!source.exists());
    assert!(target.is_dir());
    engine.undo(&Cancellation::default()).unwrap();
    assert!(source.is_dir());
    assert!(!target.exists());
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
}

#[test]
fn undo_of_an_empty_folder_move_preserves_its_leased_destination() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let device = fs::metadata(root.path()).unwrap().dev();
    let other = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|base| {
            tempfile::Builder::new()
                .prefix(".retained-empty-undo-")
                .tempdir_in(base)
                .ok()
                .filter(|folder| fs::metadata(folder.path()).unwrap().dev() != device)
        })
        .expect("a writable second filesystem is required");
    let source = other.path().join("empty folder");
    fs::create_dir(&source).unwrap();
    let destination = saved.parent().unwrap().join("empty folder");
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: saved.parent().unwrap().to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    assert!(!source.exists());
    assert!(destination.is_dir());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&destination))
        .unwrap();
    let lease = engine.lease_retained_sources(&captured).unwrap();
    let before = engine.latest_undo_id();
    let result = engine.undo(&Cancellation::default());
    assert!(
        destination.is_dir(),
        "Undo removed a leased empty destination"
    );
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(engine.latest_undo_id(), before);
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    lease.release().unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert!(source.is_dir());
    assert!(!destination.exists());
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
}

#[test]
fn a_saved_container_cannot_migrate_out_of_an_enclosing_leased_container() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let incoming = root.path().join("nested replacement");
    fs::create_dir(&incoming).unwrap();
    let incoming = incoming.join(saved.file_name().unwrap());
    fs::write(&incoming, b"Enclosing selected bytes").unwrap();
    replace(&engine, &incoming, saved.parent().unwrap());
    let nested = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| {
            item.entry
                .filter(|entry| {
                    entry.path != saved
                        && entry.path.starts_with(saved.parent().unwrap())
                        && fs::read(&entry.path).is_ok_and(|bytes| bytes == b"Older retained bytes")
                })
                .map(|entry| entry.path)
        })
        .expect("replacement did not expose its actual nested saved payload");
    let device = fs::metadata(root.path()).unwrap().dev();
    let other = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|base| {
            tempfile::Builder::new()
                .prefix(".retained-nested-container-")
                .tempdir_in(base)
                .ok()
                .filter(|folder| fs::metadata(folder.path()).unwrap().dev() != device)
        })
        .expect("a writable second filesystem is required");
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let lease = engine.lease_retained_sources(&captured).unwrap();
    let source = nested.parent().unwrap().to_owned();
    let target = other.path().join(source.file_name().unwrap());
    let request = TransferRequest {
        kind: TransferKind::Move,
        sources: vec![source.clone()],
        destination: other.path().to_owned(),
    };
    let before = engine.latest_undo_id();
    let outcome = engine
        .transfer(&request, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(
        nested.exists(),
        "migration removed data from a leased enclosing container"
    );
    assert_eq!(fs::read(&nested).unwrap(), b"Older retained bytes");
    assert_eq!(fs::read(&saved).unwrap(), b"Enclosing selected bytes");
    assert_eq!(outcome.failures.len(), 1);
    assert!(!target.exists());
    assert_eq!(engine.latest_undo_id(), before);
    lease.release().unwrap();
    let outcome = engine
        .transfer(&request, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(!source.exists());
    assert_eq!(
        fs::read(target.join(nested.file_name().unwrap())).unwrap(),
        b"Older retained bytes"
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(&nested).unwrap(), b"Older retained bytes");
    assert_eq!(fs::read(&saved).unwrap(), b"Enclosing selected bytes");
    assert!(!target.exists());
}

#[test]
fn undo_cannot_change_a_leased_payload_through_a_nested_copy_or_rename() {
    for rename in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (engine, saved) = stored_version(root.path());
        let selected = if rename {
            let target = saved.parent().unwrap().join("renamed previous");
            engine
                .execute(
                    OperationRequest::Rename {
                        source: saved.clone(),
                        destination: target.clone(),
                    },
                    &Cancellation::default(),
                )
                .unwrap();
            target
        } else {
            let incoming = root.path().join("new saved selection");
            fs::create_dir(&incoming).unwrap();
            let incoming = incoming.join(saved.file_name().unwrap());
            fs::write(&incoming, b"New saved selection bytes").unwrap();
            replace(&engine, &incoming, saved.parent().unwrap());
            saved.clone()
        };
        let expected = if rename {
            b"Older retained bytes".as_slice()
        } else {
            b"New saved selection bytes".as_slice()
        };
        let captured = engine
            .capture_retained_sources(std::slice::from_ref(&selected))
            .unwrap();
        let lease = engine.lease_retained_sources(&captured).unwrap();
        let before = engine.latest_undo_id();
        let undone = engine.undo(&Cancellation::default());
        assert!(
            undone.is_err(),
            "Undo changed a leased payload (rename={rename})"
        );
        assert_eq!(undone.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
        assert_eq!(fs::read(&selected).unwrap(), expected);
        assert_eq!(engine.latest_undo_id(), before);
        lease.release().unwrap();
        engine.undo(&Cancellation::default()).unwrap();
        assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
        if rename {
            assert!(!selected.exists());
        }
    }
}

#[test]
fn mutation_guards_follow_parent_aliases_but_preserve_final_link_and_ordinary_folder_semantics() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let nested = saved.parent().unwrap().join("nested/deeper");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("nested note"), b"Nested retained bytes").unwrap();
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let lease = engine.lease_retained_sources(&captured).unwrap();
    let escaped = root.path().join("escaped payload");
    for (index, source) in [saved.clone(), nested.join("nested note")]
        .into_iter()
        .enumerate()
    {
        let alias = root.path().join(format!("saved parent alias {index}"));
        symlink(source.parent().unwrap(), &alias).unwrap();
        let result = engine.execute(
            OperationRequest::Rename {
                source: alias.join(source.file_name().unwrap()),
                destination: escaped.clone(),
            },
            &Cancellation::default(),
        );
        assert!(
            result.is_err(),
            "Rename escaped through parent alias {index}"
        );
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
        assert!(!escaped.exists());
    }
    let link = root.path().join("ordinary final link");
    symlink(&saved, &link).unwrap();
    let renamed = root.path().join("renamed link");
    engine
        .execute(
            OperationRequest::Rename {
                source: link.clone(),
                destination: renamed.clone(),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(fs::read_link(&renamed).unwrap(), saved);
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read_link(&link).unwrap(), saved);
    let ordinary = root.path().join(".canopa-files-stage-unowned");
    fs::create_dir(&ordinary).unwrap();
    fs::write(ordinary.join("previous"), b"Ordinary user bytes").unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: ordinary.join("previous"),
                destination: escaped.clone(),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(fs::read(&escaped).unwrap(), b"Ordinary user bytes");
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(
        fs::read(ordinary.join("previous")).unwrap(),
        b"Ordinary user bytes"
    );
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    lease.release().unwrap();
}

#[test]
fn renaming_a_whole_saved_container_keeps_its_lease_and_catalog_cleanup_together() {
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let lease = engine.lease_retained_sources(&captured).unwrap();
    let parent = root.path().join("relocated saved data");
    fs::create_dir(&parent).unwrap();
    let moved = parent.join(saved.parent().unwrap().file_name().unwrap());
    engine
        .execute(
            OperationRequest::Rename {
                source: saved.parent().unwrap().to_owned(),
                destination: moved.clone(),
            },
            &Cancellation::default(),
        )
        .unwrap();
    let relocated = moved.join(saved.file_name().unwrap());
    assert_eq!(fs::read(&relocated).unwrap(), b"Older retained bytes");
    expire_history(&engine, root.path());
    assert_eq!(fs::read(&relocated).unwrap(), b"Older retained bytes");
    lease.release().unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("relocated lease released"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!relocated.exists());
    assert_eq!(
        fs::read(root.path().join("destination/note.txt")).unwrap(),
        b"Current destination bytes"
    );
}

#[test]
fn an_invalid_saved_container_rename_is_refused_before_mutation_or_history_changes() {
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let before = engine.latest_undo_id();
    let destination = root.path().join("outside the saved-container namespace");
    let result = engine.execute(
        OperationRequest::Rename {
            source: saved.parent().unwrap().to_owned(),
            destination: destination.clone(),
        },
        &Cancellation::default(),
    );
    assert!(
        result.is_err(),
        "an invalid saved-container movement was committed"
    );
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    assert!(!destination.exists());
    assert_eq!(engine.latest_undo_id(), before);
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("history remains usable"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(engine.latest_undo_id(), before);
}

#[test]
fn a_process_owned_source_lease_protects_expired_bytes_until_its_last_clone_releases() {
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let lease = engine.lease_retained_sources(&captured).unwrap();
    let reader = lease.clone();
    assert!(engine.request_page(None).unwrap().is_empty());
    expire_history(&engine, root.path());
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    lease.release().unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("reader still owns the selection"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    reader.release().unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("last reader released"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!saved.exists());
    assert!(engine.request_page(None).unwrap().is_empty());
    assert_eq!(
        fs::read(root.path().join("destination/note.txt")).unwrap(),
        b"Current destination bytes"
    );
}

#[test]
fn accepted_requests_keep_their_sources_after_independent_clipboard_leases_release() {
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let clipboard = engine.lease_retained_sources(&captured).unwrap();
    let independent = engine.lease_retained_sources(&captured).unwrap();
    let accepted = engine
        .queue_request_with_sources(&"Accepted Paste", &captured)
        .unwrap();
    assert_eq!(engine.request_page(None).unwrap().len(), 1);
    expire_history(&engine, root.path());
    clipboard.release().unwrap();
    independent.release().unwrap();
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    let id = accepted.id();
    drop(engine);
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let records = engine.request_page(None).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id(), id);
    assert_eq!(records[0].payload::<String>().unwrap(), "Accepted Paste");
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    engine.dismiss_request(&records[0]).unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("accepted Paste dismissed"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!saved.exists());
}

#[test]
fn dropping_a_source_lease_does_not_wait_for_the_database_writer() {
    use std::{sync::mpsc, time::Duration};
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let lease = engine.lease_retained_sources(&captured).unwrap();
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database.execute_batch("BEGIN IMMEDIATE").unwrap();
    let (send, receive) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        drop(lease);
        send.send(()).unwrap();
    });
    let dropped = receive.recv_timeout(Duration::from_secs(1));
    database.execute_batch("ROLLBACK").unwrap();
    worker.join().unwrap();
    dropped.expect("dropping a clipboard lease waited for filesystem metadata I/O");
    // New acquisition acknowledges earlier queued releases before accepting.
    let next = engine.lease_retained_sources(&captured).unwrap();
    next.release().unwrap();
    expire_history(&engine, root.path());
    assert!(!saved.exists());
}

#[test]
fn a_failed_source_release_preserves_bytes_and_is_retried_before_new_acquisition() {
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let lease = engine.lease_retained_sources(&captured).unwrap();
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database.execute_batch("CREATE TRIGGER refuse_lease_release BEFORE DELETE ON requests BEGIN SELECT RAISE(FAIL, 'fixture refuses lease release'); END;").unwrap();
    assert!(lease.release().is_err());
    expire_history(&engine, root.path());
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    let ordinary = engine
        .capture_retained_sources(&[root.path().join("incoming/note.txt")])
        .unwrap();
    assert!(engine.lease_retained_sources(&ordinary).is_err());
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    database
        .execute_batch("DROP TRIGGER refuse_lease_release")
        .unwrap();
    let resumed = engine.lease_retained_sources(&ordinary).unwrap();
    resumed.release().unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("release was acknowledged"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!saved.exists());
}

#[test]
fn concurrent_explicit_releases_report_the_final_deletion_failure() {
    use std::sync::{Arc, Barrier};
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let lease = engine.lease_retained_sources(&captured).unwrap();
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database.execute_batch("CREATE TRIGGER refuse_concurrent_release BEFORE DELETE ON requests BEGIN SELECT RAISE(FAIL, 'fixture refuses final release'); END;").unwrap();
    let barrier = Arc::new(Barrier::new(16));
    let mut workers = Vec::new();
    for lease in std::iter::repeat_n(lease.clone(), 15).chain(std::iter::once(lease)) {
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            lease.release()
        }));
    }
    let failures = workers
        .into_iter()
        .filter_map(|worker| worker.join().unwrap().err())
        .count();
    assert_eq!(
        failures, 1,
        "one final owner must acknowledge failed deletion"
    );
    expire_history(&engine, root.path());
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    database
        .execute_batch("DROP TRIGGER refuse_concurrent_release")
        .unwrap();
    let ordinary = engine
        .capture_retained_sources(&[root.path().join("incoming/note.txt")])
        .unwrap();
    engine
        .lease_retained_sources(&ordinary)
        .unwrap()
        .release()
        .unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("concurrent release acknowledged"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!saved.exists());
}

#[test]
fn source_leases_share_request_budgets_without_becoming_visible_jobs() {
    let root = tempfile::tempdir().unwrap();
    let (engine, _) = stored_version(root.path());
    let ordinary = engine
        .capture_retained_sources(&[root.path().join("incoming/note.txt")])
        .unwrap();
    for index in 0..1023 {
        engine.queue_request(&index).unwrap();
    }
    let lease = engine.lease_retained_sources(&ordinary).unwrap();
    assert_eq!(engine.request_page(None).unwrap().len(), 100);
    assert_eq!(
        engine.lease_retained_sources(&ordinary).unwrap_err().kind(),
        std::io::ErrorKind::StorageFull
    );
    lease.release().unwrap();
    engine.queue_request(&"lease budget returned").unwrap();
    assert_eq!(
        engine.lease_retained_sources(&ordinary).unwrap_err().kind(),
        std::io::ErrorKind::StorageFull
    );
    assert_eq!(
        engine.request_page(None).unwrap()[0]
            .payload::<String>()
            .unwrap(),
        "lease budget returned"
    );
}

#[test]
fn a_source_lease_follows_physical_migration_and_keeps_history_locked_after_engine_drop() {
    use std::{
        os::unix::fs::MetadataExt,
        time::{Duration, Instant},
    };
    let root = tempfile::tempdir().unwrap();
    let device = fs::metadata(root.path()).unwrap().dev();
    let other = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|base| {
            tempfile::Builder::new()
                .prefix(".source-lease-migration-")
                .tempdir_in(base)
                .ok()
                .filter(|folder| fs::metadata(folder.path()).unwrap().dev() != device)
        })
        .expect("a writable second filesystem is required");
    let (engine, saved) = stored_version(root.path());
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let lease = engine.lease_retained_sources(&captured).unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![root.path().join("destination")],
                destination: other.path().to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty(), "{:?}", moved.failures);
    let saved = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| item.entry.map(|entry| entry.path))
        .unwrap();
    assert_ne!(fs::metadata(&saved).unwrap().dev(), device);
    expire_history(&engine, root.path());
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    let history = root.path().join("state/history");
    drop(engine);
    assert_eq!(
        OperationEngine::open(&history).err().unwrap().kind(),
        std::io::ErrorKind::WouldBlock
    );
    lease.release().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let engine = loop {
        match OperationEngine::open(&history) {
            Ok(engine) => break engine,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "released lease kept history locked"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("reopen after source release: {error}"),
        }
    };
    assert!(!saved.exists());
    assert!(engine.request_page(None).unwrap().is_empty());
    assert_eq!(
        fs::read(other.path().join("destination/note.txt")).unwrap(),
        b"Current destination bytes"
    );
}

#[test]
fn explicit_final_lease_release_after_engine_drop_allows_immediate_history_reopen() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("state/history");
    let source = root.path().join("ordinary source");
    fs::write(&source, b"Keep these bytes").unwrap();
    for iteration in 0..64 {
        let engine = OperationEngine::open(&history).unwrap();
        let sources = engine
            .capture_retained_sources(std::slice::from_ref(&source))
            .unwrap();
        let lease = engine.lease_retained_sources(&sources).unwrap();
        drop(engine);
        assert_eq!(
            OperationEngine::open(&history).err().unwrap().kind(),
            std::io::ErrorKind::WouldBlock
        );
        lease.release().unwrap();
        let reopened = OperationEngine::open(&history)
            .unwrap_or_else(|error| panic!("explicit release left the old history owner alive at iteration {iteration}: {error}"));
        assert!(reopened.request_page(None).unwrap().is_empty());
        assert_eq!(fs::read(&source).unwrap(), b"Keep these bytes");
    }
}

#[test]
fn restart_releases_dead_process_source_leases_but_preserves_accepted_requests() {
    use std::{
        os::unix::process::ExitStatusExt,
        process::Command,
        time::{Duration, Instant},
    };
    for accepted in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (engine, saved) = stored_version(root.path());
        drop(engine);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "crash_source_lease_helper"])
            .env("CANOPA_SOURCE_LEASE_ROOT", root.path())
            .env(
                "CANOPA_SOURCE_LEASE_ACCEPTED",
                if accepted { "yes" } else { "no" },
            )
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let marker = root.path().join("lease-held");
        while !marker.exists() && Instant::now() < deadline && child.try_wait().unwrap().is_none() {
            std::thread::sleep(Duration::from_millis(5));
        }
        let held = marker.exists();
        let protected = fs::read(&saved).ok();
        let _ = child.kill();
        let status = child.wait().unwrap();
        assert!(held, "child did not establish an expired clipboard lease");
        assert_eq!(
            protected.as_deref(),
            Some(b"Older retained bytes".as_slice())
        );
        assert_eq!(status.signal(), Some(9));
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let records = engine.request_page(None).unwrap();
        if accepted {
            assert_eq!(records.len(), 1);
            assert_eq!(
                records[0].payload::<String>().unwrap(),
                "Accepted before crash"
            );
            assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
            engine.dismiss_request(&records[0]).unwrap();
            engine
                .execute(
                    OperationRequest::CreateFolder {
                        path: root.path().join("durable owner finished"),
                    },
                    &Cancellation::default(),
                )
                .unwrap();
        } else {
            assert!(records.is_empty());
        }
        assert!(!saved.exists());
        assert_eq!(
            fs::read(root.path().join("destination/note.txt")).unwrap(),
            b"Current destination bytes"
        );
    }
}

#[test]
#[ignore = "child fixture; parent kills the actual lease owner after history expiry"]
fn crash_source_lease_helper() {
    let root = PathBuf::from(std::env::var_os("CANOPA_SOURCE_LEASE_ROOT").unwrap());
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let saved = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| item.entry.map(|entry| entry.path))
        .unwrap();
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let _lease = engine.lease_retained_sources(&captured).unwrap();
    if std::env::var("CANOPA_SOURCE_LEASE_ACCEPTED").unwrap() == "yes" {
        engine
            .queue_request_with_sources(&"Accepted before crash", &captured)
            .unwrap();
    }
    expire_history(&engine, &root);
    assert_eq!(fs::read(saved).unwrap(), b"Older retained bytes");
    fs::write(root.join("lease-held"), b"protected").unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn handing_off_a_plan_preserves_each_unfinished_owners_source_until_its_last_release() {
    for remaining_phase in [RequestPhase::Remaining, RequestPhase::Finished] {
        let root = tempfile::tempdir().unwrap();
        let (engine, saved) = stored_version(root.path());
        let sources = engine
            .capture_retained_sources(std::slice::from_ref(&saved))
            .unwrap();
        let accepted = engine
            .queue_request_with_sources(&"Original plans", &sources)
            .unwrap();
        let stopped = engine
            .checkpoint_request(&accepted, RequestPhase::FailedWithRemaining, &"Exact plans")
            .unwrap();
        let excessive = "x".repeat(4 * 1024 * 1024);
        assert!(
            engine
                .handoff_request(&stopped, remaining_phase, &"Unselected plan", &excessive)
                .is_err()
        );
        assert_eq!(
            engine.request_page(None).unwrap()[0]
                .payload::<String>()
                .unwrap(),
            "Exact plans"
        );
        let (original, successor) = engine
            .handoff_request(
                &stopped,
                remaining_phase,
                &"Unselected plan",
                &"Chosen exact plan",
            )
            .unwrap();
        assert!(
            engine
                .checkpoint_request(&stopped, RequestPhase::Finished, &"Stale completion")
                .is_err()
        );
        assert!(engine.dismiss_request(&stopped).is_err());
        expire_history(&engine, root.path());
        assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
        let last_owner = if remaining_phase == RequestPhase::Remaining {
            engine.dismiss_request(&successor).unwrap();
            original
        } else {
            engine.dismiss_request(&original).unwrap();
            successor
        };
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join("one owner remains"),
                },
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
        drop(engine);
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let restored = engine
            .request_page(None)
            .unwrap()
            .into_iter()
            .find(|record| record.id() == last_owner.id())
            .unwrap();
        engine
            .checkpoint_request(&restored, RequestPhase::Finished, &"Completed chosen work")
            .unwrap();
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join("last owner finished"),
                },
                &Cancellation::default(),
            )
            .unwrap();
        assert!(
            !saved.exists(),
            "finishing the last owner releases expired retained bytes"
        );
        assert_eq!(
            fs::read(root.path().join("destination/note.txt")).unwrap(),
            b"Current destination bytes"
        );
    }
}

#[test]
fn admission_during_undo_review_protects_the_source_before_undo_can_consume_it() {
    use canopa_files::operations::{UndoChoice, UndoSource};
    use std::{
        sync::{Arc, mpsc},
        time::Duration,
    };
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let engine = Arc::new(engine);
    fs::write(&saved, b"Saved bytes edited before review").unwrap();
    let mut worker = None;
    let mut accepted = None;
    let result = engine.undo_interactive(None, &Cancellation::default(), |question| {
        assert_eq!(question.source, UndoSource::ChangedRetained);
        let writer = engine.clone();
        let selected = saved.clone();
        let (send, receive) = mpsc::channel();
        worker = Some(std::thread::spawn(move || {
            let result = writer
                .capture_retained_sources(&[selected])
                .and_then(|sources| {
                    writer.queue_request_with_sources(&"Copy the reviewed source later", &sources)
                });
            let _ = send.send(result);
        }));
        accepted = receive.recv_timeout(Duration::from_secs(3)).ok();
        UndoChoice::UseSavedVersion
    });
    // Finish the review before asserting its deadline, so a regression cannot
    // strand a worker behind Undo's mutation lock.
    worker.unwrap().join().unwrap();
    let accepted = accepted
        .expect("source admission blocked on Undo review")
        .unwrap();
    assert_eq!(result.err().unwrap().kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(
        fs::read(&saved).unwrap(),
        b"Saved bytes edited before review"
    );
    assert_eq!(
        fs::read(root.path().join("destination/note.txt")).unwrap(),
        b"Current destination bytes"
    );
    engine.dismiss_request(&accepted).unwrap();
    let undone = engine
        .undo_interactive(None, &Cancellation::default(), |_| {
            UndoChoice::UseSavedVersion
        })
        .unwrap();
    assert_eq!(undone.remaining, 0);
    assert_eq!(
        fs::read(root.path().join("destination/note.txt")).unwrap(),
        b"Saved bytes edited before review"
    );
    assert!(!saved.exists());
}

#[test]
fn an_over_budget_admission_rolls_back_its_source_ownership_and_leaves_undo_available() {
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let sources = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    // The JSON string alone fits the existing 4 MiB limit exactly. Its source
    // reservation must also fit; refusal must roll back both pieces of admission.
    let at_payload_limit = "q".repeat(4 * 1024 * 1024 - 2);
    assert_eq!(
        engine
            .queue_request_with_sources(&at_payload_limit, &sources)
            .err()
            .unwrap()
            .kind(),
        std::io::ErrorKind::StorageFull
    );
    assert!(engine.request_page(None).unwrap().is_empty());
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!saved.exists());
    assert_eq!(
        fs::read(root.path().join("destination/note.txt")).unwrap(),
        b"Older retained bytes"
    );
}

#[test]
fn a_capture_cannot_be_used_by_another_history_or_after_undo_consumes_its_source() {
    let root = tempfile::tempdir().unwrap();
    let (engine, saved) = stored_version(root.path());
    let sources = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let other = OperationEngine::open(root.path().join("other/history")).unwrap();
    assert!(other.lease_retained_sources(&sources).is_err());
    assert!(
        other
            .queue_request_with_sources(&"Foreign selection", &sources)
            .is_err()
    );
    assert!(other.request_page(None).unwrap().is_empty());
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!saved.exists());
    assert_eq!(
        engine.lease_retained_sources(&sources).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(
        engine
            .queue_request_with_sources(&"Stale selection", &sources)
            .err()
            .unwrap()
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(engine.request_page(None).unwrap().is_empty());
    assert_eq!(
        fs::read(root.path().join("destination/note.txt")).unwrap(),
        b"Older retained bytes"
    );
}

#[test]
fn directory_aliases_include_owned_sources_but_final_links_and_unowned_lookalikes_do_not() {
    use std::os::unix::fs::symlink;
    for selection in ["directory alias", "final link", "unowned lookalike"] {
        let root = tempfile::tempdir().unwrap();
        let (engine, saved) = stored_version(root.path());
        let selected = root.path().join(selection);
        match selection {
            "directory alias" => symlink(root.path(), &selected).unwrap(),
            "final link" => symlink(&saved, &selected).unwrap(),
            _ => {
                fs::create_dir(&selected).unwrap();
                let lookalike = selected.join(saved.parent().unwrap().file_name().unwrap());
                fs::create_dir(&lookalike).unwrap();
                fs::write(lookalike.join("previous"), b"Unowned bytes").unwrap();
            }
        }
        // A final directory link is copied as a link. Select its child directory
        // through the alias to exercise resolution of an ancestor link instead.
        let selected = if selection == "directory alias" {
            selected.join("destination")
        } else {
            selected
        };
        let sources = engine
            .capture_retained_sources(&[selected.clone(), selected])
            .unwrap();
        let record = engine
            .queue_request_with_sources(&"Selected source", &sources)
            .unwrap();
        expire_history(&engine, root.path());
        assert_eq!(
            saved.exists(),
            selection == "directory alias",
            "{selection}"
        );
        if selection == "directory alias" {
            assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
        } else if selection == "final link" {
            assert_eq!(fs::read_link(root.path().join(selection)).unwrap(), saved);
        } else {
            assert_eq!(
                fs::read(
                    root.path()
                        .join(selection)
                        .join(saved.parent().unwrap().file_name().unwrap())
                        .join("previous")
                )
                .unwrap(),
                b"Unowned bytes"
            );
        }
        engine.dismiss_request(&record).unwrap();
    }
}

#[test]
fn an_unfinished_request_keeps_its_retained_source_through_expiry_and_restart_until_dismissed() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original");
    let incoming = root.path().join("incoming");
    let destination = root.path().join("destination");
    for path in [&original, &incoming, &destination] {
        fs::create_dir(path).unwrap();
    }
    fs::write(original.join("note.txt"), b"Older retained bytes").unwrap();
    fs::write(incoming.join("note.txt"), b"Current destination bytes").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    replace(&engine, &original.join("note.txt"), &destination);
    replace(&engine, &incoming.join("note.txt"), &destination);
    let saved = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| item.entry.map(|entry| entry.path))
        .unwrap();
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    let sources = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let request = engine
        .queue_request_with_sources(&"Copy this retained version later", &sources)
        .unwrap();
    expire_history(&engine, root.path());
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    let request = engine
        .request_page(None)
        .unwrap()
        .into_iter()
        .find(|row| row.id() == request.id())
        .unwrap();
    assert_eq!(
        request.payload::<String>().unwrap(),
        "Copy this retained version later"
    );
    engine.dismiss_request(&request).unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("cleanup after dismissal"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(
        !saved.exists(),
        "released unchanged retained bytes should expire normally"
    );
    assert_eq!(
        fs::read(original.join("note.txt")).unwrap(),
        b"Older retained bytes"
    );
    assert_eq!(
        fs::read(destination.join("note.txt")).unwrap(),
        b"Current destination bytes"
    );
}

#[test]
fn retained_source_ownership_follows_a_saved_container_across_filesystems() {
    use std::{os::unix::fs::MetadataExt, path::PathBuf};
    let root = tempfile::tempdir().unwrap();
    let device = fs::metadata(root.path()).unwrap().dev();
    let other = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|base| {
            tempfile::Builder::new()
                .prefix(".retained-source-migration-")
                .tempdir_in(base)
                .ok()
                .filter(|folder| fs::metadata(folder.path()).unwrap().dev() != device)
        })
        .expect("a writable second filesystem is required");
    let original = root.path().join("original");
    let incoming = root.path().join("incoming");
    let project = root.path().join("project");
    let destination = project.join("nested");
    for path in [&original, &incoming, &destination] {
        fs::create_dir_all(path).unwrap();
    }
    fs::write(original.join("note.txt"), b"Older retained bytes").unwrap();
    fs::write(incoming.join("note.txt"), b"Current destination bytes").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    replace(&engine, &original.join("note.txt"), &destination);
    replace(&engine, &incoming.join("note.txt"), &destination);
    let saved = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| item.entry.map(|entry| entry.path))
        .unwrap();
    let sources = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let request = engine
        .queue_request_with_sources(&"Copy this retained version later", &sources)
        .unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![project],
                destination: other.path().to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty(), "{:?}", moved.failures);
    let saved = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| {
            item.entry
                .filter(|entry| {
                    fs::read(&entry.path).is_ok_and(|bytes| bytes == b"Older retained bytes")
                })
                .map(|entry| entry.path)
        })
        .unwrap();
    assert_ne!(fs::metadata(saved.parent().unwrap()).unwrap().dev(), device);
    expire_history(&engine, root.path());
    assert_eq!(
        fs::read(&saved).expect("the migrated reservation must protect its actual new container"),
        b"Older retained bytes"
    );
    drop(engine);
    let engine = OperationEngine::open(history).unwrap();
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    let request = engine
        .request_page(None)
        .unwrap()
        .into_iter()
        .find(|record| record.id() == request.id())
        .unwrap();
    engine.dismiss_request(&request).unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("released after migration"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!saved.exists());
    assert_eq!(
        fs::read(other.path().join("project/nested/note.txt")).unwrap(),
        b"Current destination bytes"
    );
}

#[test]
fn a_forked_child_cannot_keep_history_locked_after_the_last_real_owner_releases_it() {
    use std::{
        ffi::{c_int, c_void},
        io::{Read, Write},
        os::fd::{AsRawFd, FromRawFd},
    };
    unsafe extern "C" {
        #[link_name = "fork"]
        fn child_fork() -> c_int;
        #[link_name = "pipe2"]
        fn child_pipe(fds: *mut c_int, flags: c_int) -> c_int;
        #[link_name = "close"]
        fn child_close(fd: c_int) -> c_int;
        #[link_name = "read"]
        fn child_read(fd: c_int, buffer: *mut c_void, count: usize) -> isize;
        #[link_name = "write"]
        fn child_write(fd: c_int, buffer: *const c_void, count: usize) -> isize;
        #[link_name = "_exit"]
        fn child_exit(status: c_int) -> !;
        #[link_name = "waitpid"]
        fn child_wait(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
    }
    fn pipe() -> (fs::File, fs::File) {
        let mut fds = [-1; 2];
        // The successful call creates two independently owned CLOEXEC files.
        assert_eq!(
            unsafe {
                child_pipe(
                    fds.as_mut_ptr(),
                    rustix::fs::OFlags::CLOEXEC.bits() as c_int,
                )
            },
            0,
            "anonymous control pipe: {}",
            std::io::Error::last_os_error()
        );
        unsafe { (fs::File::from_raw_fd(fds[0]), fs::File::from_raw_fd(fds[1])) }
    }
    struct Child {
        pid: c_int,
        release: fs::File,
    }
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.release.write_all(&[1]);
            let mut status = 0;
            loop {
                // Only the parent runs Rust destruction and waits for its own child.
                if unsafe { child_wait(self.pid, &mut status, 0) } >= 0
                    || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
                {
                    break;
                }
            }
        }
    }
    for with_lease in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let history = root.path().join("state/history");
        let source = root.path().join("ordinary source");
        fs::write(&source, b"Preserve original source").unwrap();
        let engine = OperationEngine::open(&history).unwrap();
        let lease = with_lease.then(|| {
            let captured = engine
                .capture_retained_sources(std::slice::from_ref(&source))
                .unwrap();
            engine.lease_retained_sources(&captured).unwrap()
        });
        let (mut ready_read, ready_write) = pipe();
        let (release_read, release_write) = pipe();
        let ready_read_fd = ready_read.as_raw_fd();
        let ready_write_fd = ready_write.as_raw_fd();
        let release_read_fd = release_read.as_raw_fd();
        let release_write_fd = release_write.as_raw_fd();
        // Reproduce the inherited-descriptor window before a concurrent Command
        // exec closes CLOEXEC files. The child performs only async-signal-safe
        // C calls and _exit, never Rust/SQLite locks or destruction after fork.
        let pid = unsafe { child_fork() };
        assert!(
            pid >= 0,
            "fork fixture: {}",
            std::io::Error::last_os_error()
        );
        if pid == 0 {
            unsafe {
                child_close(ready_read_fd);
                child_close(release_write_fd);
                let ready = [1_u8];
                if child_write(ready_write_fd, ready.as_ptr().cast(), 1) != 1 {
                    child_exit(2);
                }
                let mut release = [0_u8];
                if child_read(release_read_fd, release.as_mut_ptr().cast(), 1) < 0 {
                    child_exit(3);
                }
                child_exit(0);
            }
        }
        drop(ready_write);
        drop(release_read);
        let child = Child {
            pid,
            release: release_write,
        };
        let mut poll = [rustix::event::PollFd::new(
            &ready_read,
            rustix::event::PollFlags::IN,
        )];
        assert_eq!(
            rustix::event::poll(
                &mut poll,
                Some(&rustix::event::Timespec {
                    tv_sec: 5,
                    tv_nsec: 0
                })
            )
            .unwrap(),
            1,
            "controlled child did not acknowledge its inherited descriptors"
        );
        ready_read.read_exact(&mut [0]).unwrap();
        assert_eq!(
            OperationEngine::open(&history).err().unwrap().kind(),
            std::io::ErrorKind::WouldBlock
        );
        drop(engine);
        if let Some(lease) = lease {
            assert_eq!(
                OperationEngine::open(&history).err().unwrap().kind(),
                std::io::ErrorKind::WouldBlock
            );
            lease.release().unwrap();
        }
        let reopened=OperationEngine::open(&history).unwrap_or_else(|error|
            panic!("a forked nonowner retained history after real ownership ended (lease={with_lease}): {error}"));
        assert!(reopened.request_page(None).unwrap().is_empty());
        assert_eq!(fs::read(source).unwrap(), b"Preserve original source");
        drop(reopened);
        drop(child);
    }
}
