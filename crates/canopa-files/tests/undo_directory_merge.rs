use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, UndoChoice, UndoSource},
    transfers::{TransferKind, TransferReply, TransferRequest},
};
use std::fs;

#[test]
fn undo_can_merge_a_renamed_directory_without_replacing_the_newer_destination_root() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("incoming.txt"), b"incoming work").unwrap();
    fs::create_dir(original.join("nested")).unwrap();
    fs::write(original.join("nested/raw.txt"), b"nested incoming work").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
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
    fs::write(original.join("newer.txt"), b"newer destination work").unwrap();
    let destination_inode = fs::metadata(&original).unwrap().ino();
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.destination, original);
            assert!(question.can_merge());
            UndoChoice::Merge
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    assert_eq!(fs::metadata(&original).unwrap().ino(), destination_inode);
    assert_eq!(
        fs::read(original.join("newer.txt")).unwrap(),
        b"newer destination work"
    );
    assert_eq!(
        fs::read(original.join("incoming.txt")).unwrap(),
        b"incoming work"
    );
    assert_eq!(
        fs::read(original.join("nested/raw.txt")).unwrap(),
        b"nested incoming work"
    );
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    assert!(engine.latest_undo_id().is_none());
}

#[test]
fn a_partial_directory_merge_reopens_and_retries_only_the_skipped_child() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("a.txt"), b"first incoming").unwrap();
    fs::write(original.join("b.txt"), b"second incoming").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: current.clone(),
            },
            &cancel,
        )
        .unwrap();
    let operation = engine.latest_undo_id().unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("b.txt"), b"newer second").unwrap();
    let partial = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                assert_eq!(question.destination, original.join("b.txt"));
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert_eq!(partial.completed, 1);
    assert_eq!(engine.latest_undo_id(), Some(operation));
    assert_eq!(fs::read(original.join("a.txt")).unwrap(), b"first incoming");
    assert!(!current.join("a.txt").exists());
    assert_eq!(fs::read(current.join("b.txt")).unwrap(), b"second incoming");
    assert_eq!(fs::read(original.join("b.txt")).unwrap(), b"newer second");
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    let mut questions = 0;
    let finished = engine
        .undo_interactive(None, &cancel, |question| {
            questions += 1;
            assert_eq!(question.destination, original.join("b.txt"));
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(questions, 1);
    assert_eq!(finished.remaining, 0);
    assert!(!current.exists());
    assert_eq!(fs::read(original.join("a.txt")).unwrap(), b"first incoming");
    assert_eq!(fs::read(original.join("b.txt")).unwrap(), b"newer second");
    assert_eq!(
        fs::read(original.join("b (2).txt")).unwrap(),
        b"second incoming"
    );
    assert!(engine.latest_undo_id().is_none());
}

#[test]
fn merge_preserves_active_and_archived_recovery_and_does_not_adopt_the_newer_root() {
    use canopa_files::transfers::ConflictChoice;
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    let sources = root.path().join("sources");
    fs::create_dir(&sources).unwrap();
    let source = sources.join("report.txt");
    fs::write(&source, b"copied report").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: original.clone(),
            },
            &cancel,
        )
        .unwrap();
    protect_copy(&engine, &source, &original, b"incoming protected report");
    fs::write(original.join("report.txt"), b"previous report").unwrap();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: original.clone(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    let copy_id = engine.latest_undo_id();
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
    protect_copy(&engine, &source, &original, b"destination protected report");
    fs::write(original.join("report.txt"), b"newer visible report").unwrap();
    let merged = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(merged.remaining, 0);
    assert!(!current.exists());
    assert_eq!(engine.latest_undo_id(), copy_id);
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 3);
    let mut found = std::collections::BTreeSet::new();
    for (index, item) in page.items.iter().enumerate() {
        let entry = item
            .entry
            .as_ref()
            .expect("all incoming and newer-destination backups stay available");
        let contents = fs::read(&entry.path).unwrap();
        if contents == b"incoming protected report" {
            assert_eq!(
                item.original.as_deref(),
                Some(original.join("report (2).txt").as_path())
            );
        } else if contents == b"destination protected report" {
            assert_eq!(
                item.original.as_deref(),
                Some(original.join("report.txt").as_path())
            );
        } else {
            assert_eq!(contents, b"previous report");
            assert!(item.held_for_undo);
        }
        let recovered = root.path().join(format!("recovered-{index}"));
        engine.recover_copy(item, &recovered, &cancel).unwrap();
        assert_eq!(fs::read(&recovered).unwrap(), contents);
        engine.undo(&cancel).unwrap();
        assert!(!recovered.exists());
        found.insert(contents);
    }
    assert_eq!(found.len(), 3);
    engine.undo(&cancel).unwrap();
    assert_eq!(
        fs::read(original.join("report (2).txt")).unwrap(),
        b"previous report"
    );
    assert_eq!(
        fs::read(original.join("report.txt")).unwrap(),
        b"newer visible report"
    );
    engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedCreatedFolder);
            UndoChoice::KeepItem
        })
        .unwrap();
    assert!(engine.latest_undo_id().is_none());
    assert_eq!(
        fs::read(original.join("report.txt")).unwrap(),
        b"newer visible report"
    );
}

#[test]
fn changing_a_merge_root_during_child_review_refuses_replace_before_creating_a_stage() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    let held = root.path().join("held destination");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("report.txt"), b"incoming").unwrap();
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
    fs::write(original.join("report.txt"), b"newer").unwrap();
    let error = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                fs::rename(&original, &held).unwrap();
                fs::create_dir(&original).unwrap();
                fs::write(original.join("sentinel.txt"), b"unapproved root").unwrap();
                UndoChoice::Replace
            }
        })
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(fs::read_dir(&original).unwrap().count(), 1);
    assert_eq!(
        fs::read(original.join("sentinel.txt")).unwrap(),
        b"unapproved root"
    );
    assert_eq!(fs::read(held.join("report.txt")).unwrap(), b"newer");
    assert_eq!(fs::read(current.join("report.txt")).unwrap(), b"incoming");
    let unapproved = root.path().join("unapproved");
    fs::rename(&original, &unapproved).unwrap();
    fs::rename(&held, &original).unwrap();
    let result = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert_eq!(
        fs::read(original.join("report (2).txt")).unwrap(),
        b"incoming"
    );
    assert_eq!(fs::read(original.join("report.txt")).unwrap(), b"newer");
}

#[test]
fn nested_merge_can_cancel_reopen_and_replace_a_child_while_protecting_newer_work() {
    use std::os::unix::{ffi::OsStrExt, fs::symlink};
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    fs::create_dir_all(original.join("nested")).unwrap();
    symlink(
        std::ffi::OsStr::from_bytes(b"raw-\xff"),
        original.join("nested/a-link"),
    )
    .unwrap();
    fs::write(original.join("nested/b.txt"), b"incoming child").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
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
    fs::write(original.join("nested/b.txt"), b"newer child").unwrap();
    let partial = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::Cancel
            }
        })
        .unwrap();
    assert!(partial.cancelled);
    assert_eq!(
        fs::read_link(original.join("nested/a-link"))
            .unwrap()
            .as_os_str()
            .as_bytes(),
        b"raw-\xff"
    );
    assert!(!current.join("nested/a-link").is_symlink());
    assert_eq!(
        fs::read(current.join("nested/b.txt")).unwrap(),
        b"incoming child"
    );
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    let finished = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.destination, original.join("nested/b.txt"));
            assert!(question.can_replace());
            UndoChoice::Replace
        })
        .unwrap();
    assert_eq!(finished.remaining, 0);
    assert!(!current.exists());
    assert_eq!(
        fs::read(original.join("nested/b.txt")).unwrap(),
        b"incoming child"
    );
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    let recovered = root.path().join("newer recovered.txt");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"newer child");
}

#[test]
fn recovery_stage_collisions_keep_both_containers_instead_of_merging_their_identities() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    fs::create_dir(&original).unwrap();
    let source = root.path().join("report.txt");
    fs::write(&source, b"source").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&history).unwrap();
    protect_copy(&engine, &source, &original, b"protected incoming version");
    let before = engine.recovery_items(None, &cancel).unwrap();
    let stage_name = before.items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_owned();
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
    let newer_container = original.join(&stage_name);
    fs::create_dir(&newer_container).unwrap();
    fs::write(newer_container.join("newer.txt"), b"unrelated newer work").unwrap();
    let mut questions = 0;
    engine
        .undo_interactive(None, &cancel, |question| {
            questions += 1;
            if questions == 1 {
                assert!(question.can_merge());
                UndoChoice::Merge
            } else {
                assert!(
                    !question.can_merge(),
                    "Recovery containers must retain separate physical identities"
                );
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(questions, 2);
    assert!(!current.exists());
    assert_eq!(
        fs::read(newer_container.join("newer.txt")).unwrap(),
        b"unrelated newer work"
    );
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"protected incoming version"
    );
    let recovered = root.path().join("recovered.txt");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"protected incoming version");
}

#[test]
fn merge_preserves_an_unavailable_saved_remainder_and_can_finish_once_it_returns() {
    use canopa_files::transfers::ConflictChoice;
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("item");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let current = destination.join("item");
    fs::write(&original, b"original incoming file").unwrap();
    fs::write(&current, b"previous destination").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&history).unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![original.clone()],
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
    let elsewhere = root.path().join("incoming elsewhere");
    fs::rename(&current, &elsewhere).unwrap();
    fs::create_dir(&current).unwrap();
    fs::write(current.join("returned.txt"), b"selected directory contents").unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("newer.txt"), b"newer directory contents").unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    let previous = page.items[0].entry.as_ref().unwrap().path.clone();
    let saved = root.path().join("previous elsewhere");
    fs::rename(&previous, &saved).unwrap();
    let partial = engine
        .undo_interactive(None, &cancel, |question| match question.source {
            UndoSource::ChangedMove | UndoSource::UnavailableRetained => UndoChoice::ReturnItem,
            UndoSource::Current => UndoChoice::Merge,
            _ => panic!("unexpected question: {:?}", question.source),
        })
        .unwrap();
    assert_eq!(partial.remaining, 1);
    assert_eq!(partial.unrestored_previous, vec![current.clone()]);
    assert!(!current.exists());
    assert_eq!(
        fs::read(original.join("returned.txt")).unwrap(),
        b"selected directory contents"
    );
    assert_eq!(
        fs::read(original.join("newer.txt")).unwrap(),
        b"newer directory contents"
    );
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].held_for_undo);
    fs::rename(&saved, &previous).unwrap();
    let finished = engine
        .undo_interactive(None, &cancel, |question| match question.source {
            UndoSource::ChangedMove => UndoChoice::ReturnItem,
            UndoSource::ChangedRetained => UndoChoice::UseSavedVersion,
            UndoSource::Current => UndoChoice::Merge,
            _ => panic!("unexpected question: {:?}", question.source),
        })
        .unwrap();
    assert_eq!(finished.remaining, 0);
    assert_eq!(fs::read(&current).unwrap(), b"previous destination");
    assert_eq!(
        fs::read(original.join("returned.txt")).unwrap(),
        b"selected directory contents"
    );
    assert_eq!(
        fs::read(original.join("newer.txt")).unwrap(),
        b"newer directory contents"
    );
    assert_eq!(fs::read(elsewhere).unwrap(), b"original incoming file");
}

#[test]
fn keeping_both_directory_versions_preserves_recovery_data_inside_the_newer_destination() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("incoming.txt"), b"original incoming work").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
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
    fs::write(original.join("keep.txt"), b"newer directory work").unwrap();
    let source = root.path().join("report.txt");
    fs::write(&source, b"copy source").unwrap();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: original.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    fs::write(
        original.join("report.txt"),
        b"newer work protected in Recovery",
    )
    .unwrap();
    engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedCopy);
            UndoChoice::MoveToRecovery
        })
        .unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].held_for_undo);
    let protected = page.items[0].entry.as_ref().unwrap().path.clone();
    engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.incoming.path(), current);
            assert_eq!(question.destination, original);
            UndoChoice::KeepBoth
        })
        .unwrap();
    let returned = root.path().join("project (2)");
    assert_eq!(
        fs::read(returned.join("incoming.txt")).unwrap(),
        b"original incoming work"
    );
    assert_eq!(
        fs::read(original.join("keep.txt")).unwrap(),
        b"newer directory work"
    );
    assert_eq!(
        fs::read(&protected).unwrap(),
        b"newer work protected in Recovery"
    );
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(
        page.items[0].entry.is_some(),
        "newer destination recovery data did not move with the incoming directory"
    );
    let recovered = root.path().join("recovered.txt");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(
        fs::read(recovered).unwrap(),
        b"newer work protected in Recovery"
    );
    assert_eq!(fs::read(&source).unwrap(), b"copy source");
}

fn protect_copy(
    engine: &OperationEngine,
    source: &std::path::Path,
    destination: &std::path::Path,
    contents: &[u8],
) {
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.to_owned()],
                destination: destination.to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    fs::write(destination.join(source.file_name().unwrap()), contents).unwrap();
    engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedCopy);
            UndoChoice::MoveToRecovery
        })
        .unwrap();
}

#[test]
fn directory_keep_both_moves_incoming_recovery_records_and_leaves_newer_records_in_place() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("incoming.txt"), b"incoming work").unwrap();
    let source = root.path().join("report.txt");
    fs::write(&source, b"copy source").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
    protect_copy(&engine, &source, &original, b"incoming protected work");
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
    fs::write(original.join("newer.txt"), b"newer visible work").unwrap();
    protect_copy(&engine, &source, &original, b"newer protected work");
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    let returned = root.path().join("project (2)");
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
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
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 2);
    let mut found = std::collections::BTreeSet::new();
    for (index, item) in page.items.iter().enumerate() {
        assert!(!item.held_for_undo);
        let entry = item
            .entry
            .as_ref()
            .expect("both protected versions remain available");
        let contents = fs::read(&entry.path).unwrap();
        if contents == b"incoming protected work" {
            assert!(entry.path.starts_with(&returned));
        } else {
            assert_eq!(contents, b"newer protected work");
            assert!(entry.path.starts_with(&original));
        }
        let recovered = root.path().join(format!("recovered-{index}.txt"));
        engine.recover_copy(item, &recovered, &cancel).unwrap();
        assert_eq!(fs::read(recovered).unwrap(), contents);
        found.insert(contents);
    }
    assert_eq!(
        found,
        [
            b"incoming protected work".to_vec(),
            b"newer protected work".to_vec()
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        fs::read(returned.join("incoming.txt")).unwrap(),
        b"incoming work"
    );
    assert_eq!(
        fs::read(original.join("newer.txt")).unwrap(),
        b"newer visible work"
    );
}

#[test]
fn a_partial_directory_return_preserves_both_recovery_namespaces_and_its_saved_remainder() {
    use canopa_files::transfers::ConflictChoice;
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("item");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let current = destination.join("item");
    fs::write(&original, b"original moved file").unwrap();
    fs::write(&current, b"previous destination").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![original.clone()],
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
    let old_incoming = root.path().join("original incoming elsewhere.txt");
    fs::rename(&current, &old_incoming).unwrap();
    fs::create_dir(&current).unwrap();
    fs::write(current.join("incoming.txt"), b"selected directory work").unwrap();
    let source = root.path().join("report.txt");
    fs::write(&source, b"copy source").unwrap();
    protect_copy(&engine, &source, &current, b"incoming protected work");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("newer.txt"), b"newer directory work").unwrap();
    protect_copy(&engine, &source, &original, b"newer protected work");
    let page = engine.recovery_items(None, &cancel).unwrap();
    let previous = page
        .items
        .iter()
        .find(|item| item.held_for_undo)
        .unwrap()
        .entry
        .as_ref()
        .unwrap()
        .path
        .clone();
    let held_previous = root.path().join("saved previous elsewhere.txt");
    fs::rename(&previous, &held_previous).unwrap();
    let mut questions = Vec::new();
    let partial = engine
        .undo_interactive(None, &cancel, |question| {
            questions.push(question.source);
            match question.source {
                UndoSource::ChangedMove | UndoSource::UnavailableRetained => UndoChoice::ReturnItem,
                UndoSource::Current => UndoChoice::KeepBoth,
                _ => panic!("unexpected return question: {:?}", question.source),
            }
        })
        .unwrap();
    assert_eq!(
        questions,
        vec![
            UndoSource::ChangedMove,
            UndoSource::UnavailableRetained,
            UndoSource::Current
        ]
    );
    assert_eq!(partial.completed, 1);
    assert_eq!(partial.remaining, 1);
    assert_eq!(partial.unrestored_previous, vec![current.clone()]);
    let returned = root.path().join("item (2)");
    assert_eq!(
        fs::read(returned.join("incoming.txt")).unwrap(),
        b"selected directory work"
    );
    assert_eq!(
        fs::read(original.join("newer.txt")).unwrap(),
        b"newer directory work"
    );
    assert_eq!(fs::read(&old_incoming).unwrap(), b"original moved file");
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 3);
    let mut protected = std::collections::BTreeSet::new();
    for item in page.items.iter().filter(|item| !item.held_for_undo) {
        let entry = item
            .entry
            .as_ref()
            .expect("both sides remain recoverable during a saved-only remainder");
        let contents = fs::read(&entry.path).unwrap();
        if contents == b"incoming protected work" {
            assert!(entry.path.starts_with(&returned));
        } else {
            assert_eq!(contents, b"newer protected work");
            assert!(entry.path.starts_with(&original));
        }
        protected.insert(contents);
    }
    assert_eq!(protected.len(), 2);
    fs::rename(&held_previous, &previous).unwrap();
    let finished = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedRetained);
            UndoChoice::UseSavedVersion
        })
        .unwrap();
    assert_eq!(finished.remaining, 0);
    assert_eq!(fs::read(&current).unwrap(), b"previous destination");
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 2);
    for (index, item) in page.items.iter().enumerate() {
        let recovered = root.path().join(format!("recovered-partial-{index}.txt"));
        let expected = fs::read(&item.entry.as_ref().unwrap().path).unwrap();
        engine.recover_copy(item, &recovered, &cancel).unwrap();
        assert_eq!(fs::read(recovered).unwrap(), expected);
    }
}
