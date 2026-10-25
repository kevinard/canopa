use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, UndoChoice},
};
use std::fs;

#[test]
fn undo_of_a_created_folder_with_new_contents_can_be_skipped_cancelled_and_kept() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("state/history");
    let folder = root.path().join("New folder");
    let added = folder.join("new work.txt");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
    let created = engine
        .execute(
            OperationRequest::CreateFolder {
                path: folder.clone(),
            },
            &cancel,
        )
        .unwrap();
    fs::write(&added, b"work added after folder creation").unwrap();
    assert!(engine.undo(&cancel).is_err());
    let mut asked = false;
    let skipped = engine
        .undo_interactive(Some(created.id), &cancel, |question| {
            asked = true;
            assert_eq!(question.incoming.path(), folder);
            assert!(!question.can_replace());
            assert!(!question.can_return_item());
            UndoChoice::Skip
        })
        .unwrap();
    assert!(asked);
    assert_eq!(skipped.completed, 0);
    assert_eq!(skipped.remaining, 1);
    assert_eq!(skipped.skipped, vec![folder.clone()]);
    assert_eq!(
        fs::read(&added).unwrap(),
        b"work added after folder creation"
    );
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    let cancelled = engine
        .undo_interactive(Some(created.id), &cancel, |_| UndoChoice::Cancel)
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.remaining, 1);
    let kept = engine
        .undo_interactive(Some(created.id), &cancel, |_| UndoChoice::KeepItem)
        .unwrap();
    assert_eq!(kept.completed, 0);
    assert_eq!(kept.remaining, 0);
    assert_eq!(kept.kept, vec![folder.clone()]);
    assert_eq!(
        fs::read(&added).unwrap(),
        b"work added after folder creation"
    );
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    assert!(engine.latest_undo_id().is_none());
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
}

#[test]
fn a_new_nested_child_can_keep_its_created_containers_after_other_copy_steps_are_undone() {
    use canopa_files::transfers::{TransferKind, TransferReply, TransferRequest};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("sources/project");
    let destination = root.path().join("destination");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("nested/copied.txt"), b"copied source").unwrap();
    let other = root.path().join("sources/other.txt");
    fs::write(&other, b"other source").unwrap();
    let history = root.path().join("state/history");
    let mut engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone(), other.clone()],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    let project = destination.join("project");
    let nested = project.join("nested");
    let added = nested.join("new work.txt");
    fs::write(&added, b"new nested work").unwrap();
    let skipped = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.incoming.path(), nested);
            UndoChoice::Skip
        })
        .unwrap();
    assert_eq!(skipped.completed, 2);
    assert_eq!(skipped.remaining, 2);
    assert!(!destination.join("other.txt").exists());
    assert!(!nested.join("copied.txt").exists());
    assert_eq!(fs::read(&added).unwrap(), b"new nested work");
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    let kept = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepItem)
        .unwrap();
    assert_eq!(kept.completed, 0);
    assert_eq!(kept.remaining, 0);
    assert_eq!(kept.kept, vec![nested.clone(), project]);
    assert_eq!(fs::read(&added).unwrap(), b"new nested work");
    assert_eq!(
        fs::read(source.join("nested/copied.txt")).unwrap(),
        b"copied source"
    );
    assert_eq!(fs::read(&other).unwrap(), b"other source");
}

#[test]
fn keeping_a_nonempty_created_folder_preserves_protected_recovery_data_inside_it() {
    use canopa_files::{
        operations::UndoSource,
        transfers::{TransferKind, TransferReply, TransferRequest},
    };
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("file.txt"), b"original source").unwrap();
    let history = root.path().join("state/history");
    let mut engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
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
    assert!(copied.failures.is_empty());
    let folder = destination.join("source");
    let file = folder.join("file.txt");
    fs::write(&file, b"edited copied data").unwrap();
    let mut questions = Vec::new();
    let outcome = engine
        .undo_interactive(None, &cancel, |question| {
            questions.push(question.source);
            if question.source == UndoSource::ChangedCopy {
                UndoChoice::MoveToRecovery
            } else {
                UndoChoice::KeepItem
            }
        })
        .unwrap();
    assert_eq!(
        questions,
        vec![UndoSource::ChangedCopy, UndoSource::NonemptyFolder]
    );
    assert_eq!(outcome.completed, 1);
    assert_eq!(outcome.remaining, 0);
    assert_eq!(outcome.kept, vec![folder.clone()]);
    assert!(!file.exists());
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
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].held_for_undo);
    let recovered = root.path().join("recovered.txt");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"edited copied data");
    assert_eq!(
        fs::read(source.join("file.txt")).unwrap(),
        b"original source"
    );
    assert!(folder.is_dir());
}

#[test]
fn a_skipped_nonempty_folder_can_be_removed_after_its_new_contents_are_moved_out() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("created");
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: folder.clone(),
            },
            &cancel,
        )
        .unwrap();
    let added = folder.join("new.txt");
    fs::write(&added, b"new work").unwrap();
    let skipped = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Skip)
        .unwrap();
    assert_eq!(skipped.remaining, 1);
    let external = root.path().join("new work elsewhere.txt");
    fs::rename(added, &external).unwrap();
    let done = engine
        .undo_interactive(None, &cancel, |_| {
            panic!("an owned empty folder needs no conflict choice")
        })
        .unwrap();
    assert_eq!(done.completed, 1);
    assert_eq!(done.remaining, 0);
    assert!(!folder.exists());
    assert_eq!(fs::read(external).unwrap(), b"new work");
}

#[test]
fn a_changed_created_folder_review_does_not_finish_another_folder_or_ignore_cancellation() {
    for case in ["child-added", "replaced", "cancelled"] {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("created");
        let held = root.path().join("held original folder");
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let created = engine
            .execute(
                OperationRequest::CreateFolder {
                    path: folder.clone(),
                },
                &cancel,
            )
            .unwrap();
        fs::write(folder.join("new.txt"), b"original new work").unwrap();
        let outcome = engine.undo_interactive(None, &cancel, |_| {
            match case {
                "child-added" => fs::write(folder.join("another.txt"), b"later work").unwrap(),
                "replaced" => {
                    fs::rename(&folder, &held).unwrap();
                    fs::create_dir(&folder).unwrap();
                }
                _ => cancel.cancel(),
            }
            UndoChoice::KeepItem
        });
        if case == "cancelled" {
            assert!(outcome.unwrap().cancelled);
        } else {
            assert_eq!(outcome.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
        }
        assert_eq!(engine.latest_undo_id(), Some(created.id));
        if case == "replaced" {
            fs::remove_dir(&folder).unwrap();
            fs::rename(&held, &folder).unwrap();
        }
        assert_eq!(
            fs::read(folder.join("new.txt")).unwrap(),
            b"original new work"
        );
        let kept = engine
            .undo_interactive(None, &Cancellation::default(), |_| UndoChoice::KeepItem)
            .unwrap();
        assert_eq!(kept.remaining, 0);
        assert_eq!(kept.kept, vec![folder]);
    }
}

#[test]
fn another_item_at_a_created_folder_location_can_be_skipped_cancelled_and_kept() {
    use std::os::unix::{
        ffi::OsStringExt,
        fs::{MetadataExt, symlink},
    };
    for kind in ["file", "folder", "link", "fifo"] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("created");
        let held = root.path().join("original elsewhere");
        let history = root.path().join("state/history");
        let cancel = Cancellation::default();
        let mut engine = OperationEngine::open(&history).unwrap();
        let created = engine
            .execute(
                OperationRequest::CreateFolder { path: path.clone() },
                &cancel,
            )
            .unwrap();
        fs::write(path.join("new.txt"), b"original folder work").unwrap();
        fs::rename(&path, &held).unwrap();
        let raw_target =
            std::path::PathBuf::from(std::ffi::OsString::from_vec(b"target-\xff".to_vec()));
        fs::write(root.path().join(&raw_target), b"link target work").unwrap();
        match kind {
            "file" => fs::write(&path, b"replacement file work").unwrap(),
            "folder" => fs::create_dir(&path).unwrap(),
            "link" => symlink(&raw_target, &path).unwrap(),
            _ => rustix::fs::mkfifoat(
                rustix::fs::CWD,
                &path,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            )
            .unwrap(),
        }
        let metadata = fs::symlink_metadata(&path).unwrap();
        let expected_node = (metadata.dev(), metadata.ino());
        assert!(engine.undo(&cancel).is_err());
        let skipped = engine
            .undo_interactive(Some(created.id), &cancel, |question| {
                assert_eq!(question.incoming.path(), path);
                assert!(question.incoming.entry().is_some());
                assert!(!question.can_replace());
                assert!(!question.can_return_item());
                assert!(!question.can_move_to_recovery());
                UndoChoice::Skip
            })
            .unwrap();
        assert_eq!(skipped.completed, 0);
        assert_eq!(skipped.remaining, 1);
        assert_eq!(skipped.skipped, vec![path.clone()]);
        drop(engine);
        engine = OperationEngine::open(&history).unwrap();
        let cancelled = engine
            .undo_interactive(Some(created.id), &cancel, |_| UndoChoice::Cancel)
            .unwrap();
        assert!(cancelled.cancelled);
        assert_eq!(cancelled.remaining, 1);
        let kept = engine
            .undo_interactive(Some(created.id), &cancel, |_| UndoChoice::KeepItem)
            .unwrap();
        assert_eq!(kept.completed, 0);
        assert_eq!(kept.remaining, 0);
        assert_eq!(kept.kept, vec![path.clone()]);
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert_eq!((metadata.dev(), metadata.ino()), expected_node);
        assert_eq!(
            fs::read(held.join("new.txt")).unwrap(),
            b"original folder work"
        );
        assert_eq!(
            fs::read(root.path().join(&raw_target)).unwrap(),
            b"link target work"
        );
        if kind == "file" {
            assert_eq!(fs::read(&path).unwrap(), b"replacement file work");
        }
        if kind == "link" {
            assert_eq!(fs::read_link(&path).unwrap(), raw_target);
        }
    }
}

#[test]
fn unavailable_created_folders_can_be_skipped_cancelled_and_finished_without_removing_files() {
    use std::{os::unix::fs::PermissionsExt, path::PathBuf};
    struct ResetPermissions(PathBuf);
    impl Drop for ResetPermissions {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        }
    }
    for case in ["missing", "unreadable", "parent-unreadable"] {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("created");
        let held = root.path().join("original elsewhere");
        let history = root.path().join("state/history");
        let cancel = Cancellation::default();
        let mut engine = OperationEngine::open(&history).unwrap();
        let created = engine
            .execute(
                OperationRequest::CreateFolder { path: path.clone() },
                &cancel,
            )
            .unwrap();
        fs::write(path.join("work.txt"), b"work in the unavailable folder").unwrap();
        let permissions = match case {
            "missing" => {
                fs::rename(&path, &held).unwrap();
                None
            }
            "unreadable" => Some(path.clone()),
            _ => Some(parent.clone()),
        };
        let _reset = permissions.clone().map(ResetPermissions);
        if let Some(path) = &permissions {
            fs::set_permissions(path, fs::Permissions::from_mode(0o0)).unwrap();
        }
        let skipped = engine
            .undo_interactive(Some(created.id), &cancel, |question| {
                assert_eq!(question.incoming.path(), path);
                assert!(question.incoming.entry().is_none());
                assert!(!question.can_replace());
                assert!(!question.can_return_item());
                UndoChoice::Skip
            })
            .unwrap();
        assert_eq!(skipped.completed, 0);
        assert_eq!(skipped.remaining, 1);
        assert_eq!(skipped.skipped, vec![path.clone()]);
        drop(engine);
        engine = OperationEngine::open(&history).unwrap();
        let cancelled = engine
            .undo_interactive(Some(created.id), &cancel, |_| UndoChoice::Cancel)
            .unwrap();
        assert!(cancelled.cancelled);
        assert_eq!(cancelled.remaining, 1);
        let finished = engine
            .undo_interactive(Some(created.id), &cancel, |_| UndoChoice::FinishStep)
            .unwrap();
        assert_eq!(finished.completed, 0);
        assert_eq!(finished.remaining, 0);
        assert!(finished.kept.is_empty());
        assert!(finished.dismissed.is_empty());
        assert_eq!(finished.unremoved, vec![path.clone()]);
        if let Some(path) = &permissions {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let location = if case == "missing" { held } else { path };
        assert_eq!(
            fs::read(location.join("work.txt")).unwrap(),
            b"work in the unavailable folder"
        );
        drop(engine);
        engine = OperationEngine::open(&history).unwrap();
        assert!(engine.latest_undo_id().is_none());
    }
}

#[test]
fn a_noninteractive_undo_error_does_not_consume_an_unavailable_created_folder_step() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("created");
    let held = root.path().join("elsewhere");
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let created = engine
        .execute(
            OperationRequest::CreateFolder { path: path.clone() },
            &cancel,
        )
        .unwrap();
    fs::write(path.join("work.txt"), b"new work").unwrap();
    fs::rename(&path, &held).unwrap();
    assert!(engine.undo(&cancel).is_err());
    assert_eq!(
        engine.latest_undo_id(),
        Some(created.id),
        "an error must not silently finish the unavailable folder step"
    );
    let skipped = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Skip)
        .unwrap();
    assert_eq!(skipped.remaining, 1);
    fs::rename(&held, &path).unwrap();
    let work = root.path().join("work preserved.txt");
    fs::rename(path.join("work.txt"), &work).unwrap();
    engine.undo(&cancel).unwrap();
    assert!(!path.exists());
    assert_eq!(fs::read(work).unwrap(), b"new work");
}

#[test]
fn keeping_a_replacement_does_not_give_older_created_folder_history_ownership_of_it() {
    use canopa_files::operations::UndoSource;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("created");
    let cancel = Cancellation::default();
    let history = root.path().join("state/history");
    let mut engine = OperationEngine::open(&history).unwrap();
    let older = engine
        .execute(
            OperationRequest::CreateFolder { path: path.clone() },
            &cancel,
        )
        .unwrap();
    let first = root.path().join("first original elsewhere");
    fs::write(path.join("work.txt"), b"first work").unwrap();
    fs::rename(&path, &first).unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder { path: path.clone() },
            &cancel,
        )
        .unwrap();
    let second = root.path().join("second original elsewhere");
    fs::write(path.join("work.txt"), b"second work").unwrap();
    fs::rename(&path, &second).unwrap();
    fs::create_dir(&path).unwrap();
    let kept = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::ChangedCreatedFolder);
            UndoChoice::KeepItem
        })
        .unwrap();
    assert_eq!(kept.remaining, 0);
    assert_eq!(engine.latest_undo_id(), Some(older.id));
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    let mut asked = false;
    let older = engine
        .undo_interactive(None, &cancel, |question| {
            asked = true;
            assert_eq!(question.source, UndoSource::ChangedCreatedFolder);
            UndoChoice::Skip
        })
        .unwrap();
    assert!(asked);
    assert_eq!(older.remaining, 1);
    assert!(path.is_dir());
    assert_eq!(fs::read(first.join("work.txt")).unwrap(), b"first work");
    assert_eq!(fs::read(second.join("work.txt")).unwrap(), b"second work");
}

#[test]
fn changed_created_folder_choices_reject_reappearance_new_metadata_and_changed_failures() {
    use std::{os::unix::fs::PermissionsExt, path::PathBuf};
    struct Reset(Vec<PathBuf>);
    impl Drop for Reset {
        fn drop(&mut self) {
            for path in &self.0 {
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
            }
        }
    }
    for case in [
        "replacement-changed",
        "missing-reappears",
        "missing-error-changes",
        "unreadable-replaced",
        "unreadable-metadata-changes",
        "cancelled-missing",
    ] {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("created");
        let held = root.path().join("held original");
        let cancel = Cancellation::default();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let created = engine
            .execute(
                OperationRequest::CreateFolder { path: path.clone() },
                &cancel,
            )
            .unwrap();
        fs::write(path.join("work.txt"), b"original work").unwrap();
        let _reset = Reset(vec![parent.clone(), path.clone(), held.clone()]);
        if case.starts_with("unreadable") {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
        } else {
            fs::rename(&path, &held).unwrap();
            if case == "replacement-changed" {
                fs::write(&path, b"first replacement").unwrap();
            }
        }
        let outcome = engine.undo_interactive(None, &cancel, |_| {
            match case {
                "replacement-changed" => fs::write(&path, b"later replacement work").unwrap(),
                "missing-reappears" => fs::rename(&held, &path).unwrap(),
                "missing-error-changes" => {
                    fs::set_permissions(&parent, fs::Permissions::from_mode(0o0)).unwrap()
                }
                "unreadable-replaced" => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                    fs::rename(&path, &held).unwrap();
                    fs::create_dir(&path).unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
                }
                "unreadable-metadata-changes" => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o100)).unwrap()
                }
                _ => cancel.cancel(),
            }
            if case == "replacement-changed" {
                UndoChoice::KeepItem
            } else {
                UndoChoice::FinishStep
            }
        });
        if case == "cancelled-missing" {
            let result = outcome.unwrap();
            assert!(result.cancelled);
            assert_eq!(result.remaining, 1);
            assert!(result.unremoved.is_empty());
        } else {
            assert_eq!(
                outcome.unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock,
                "{case}"
            );
        }
        assert_eq!(engine.latest_undo_id(), Some(created.id), "{case}");
        for path in [&parent, &path, &held] {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
        }
        let original = if held.is_dir() { &held } else { &path };
        assert_eq!(
            fs::read(original.join("work.txt")).unwrap(),
            b"original work",
            "{case}"
        );
        if case == "replacement-changed" {
            assert_eq!(fs::read(&path).unwrap(), b"later replacement work");
        }
    }
}

#[test]
fn finishing_an_unavailable_child_keeps_created_ancestor_steps_retryable() {
    use canopa_files::{
        operations::UndoSource,
        transfers::{TransferKind, TransferReply, TransferRequest},
    };
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("project");
    let destination = root.path().join("destination");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::create_dir(&destination).unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
    let copied = engine
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
    assert!(copied.failures.is_empty());
    let parent = destination.join("project");
    let child = parent.join("nested");
    fs::write(child.join("work.txt"), b"new nested work").unwrap();
    let held = root.path().join("nested elsewhere");
    fs::rename(&child, &held).unwrap();
    let mut questions = 0;
    let finished = engine
        .undo_interactive(None, &cancel, |question| {
            questions += 1;
            assert_eq!(question.source, UndoSource::UnavailableCreatedFolder);
            assert_eq!(question.incoming.path(), child);
            UndoChoice::FinishStep
        })
        .unwrap();
    assert_eq!(questions, 1);
    assert_eq!(finished.completed, 0);
    assert_eq!(finished.remaining, 1);
    assert_eq!(finished.unremoved, vec![child]);
    assert!(finished.kept.is_empty());
    assert!(parent.is_dir());
    drop(engine);
    engine = OperationEngine::open(&history).unwrap();
    let done = engine
        .undo_interactive(None, &cancel, |_| {
            panic!("the owned empty ancestor can be removed normally")
        })
        .unwrap();
    assert_eq!(done.completed, 1);
    assert_eq!(done.remaining, 0);
    assert!(!parent.exists());
    assert_eq!(fs::read(held.join("work.txt")).unwrap(), b"new nested work");
    assert!(source.join("nested").is_dir());
}

#[test]
fn finishing_an_unreadable_created_folder_preserves_protected_recovery_inside_it() {
    use canopa_files::{
        operations::UndoSource,
        transfers::{TransferKind, TransferReply, TransferRequest},
    };
    use std::{os::unix::fs::PermissionsExt, path::PathBuf};
    struct Reset(PathBuf);
    impl Drop for Reset {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        }
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("file.txt"), b"original source").unwrap();
    let history = root.path().join("state/history");
    let cancel = Cancellation::default();
    let mut engine = OperationEngine::open(&history).unwrap();
    let copied = engine
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
    assert!(copied.failures.is_empty());
    let folder = destination.join("source");
    let _reset = Reset(folder.clone());
    fs::write(folder.join("file.txt"), b"protected copied edits").unwrap();
    let partial = engine
        .undo_interactive(None, &cancel, |question| match question.source {
            UndoSource::ChangedCopy => UndoChoice::MoveToRecovery,
            UndoSource::NonemptyFolder => {
                fs::set_permissions(&folder, fs::Permissions::from_mode(0o0)).unwrap();
                UndoChoice::Skip
            }
            _ => panic!("unexpected review"),
        })
        .unwrap();
    assert_eq!(partial.completed, 1);
    assert_eq!(partial.remaining, 1);
    let finished = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::UnavailableCreatedFolder);
            UndoChoice::FinishStep
        })
        .unwrap();
    assert_eq!(finished.completed, 0);
    assert_eq!(finished.remaining, 0);
    assert_eq!(finished.unremoved, vec![folder.clone()]);
    let unavailable = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(unavailable.items.len(), 1);
    assert!(unavailable.items[0].entry.is_none());
    assert!(!unavailable.items[0].held_for_undo);
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o700)).unwrap();
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
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].held_for_undo);
    let recovered = root.path().join("recovered.txt");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"protected copied edits");
    assert_eq!(
        fs::read(source.join("file.txt")).unwrap(),
        b"original source"
    );
    assert!(folder.is_dir());
}
