use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, RecoveryStatus, UndoChoice},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    os::unix::fs::{MetadataExt, symlink},
    path::PathBuf,
};

struct Fixture {
    source: tempfile::TempDir,
    destination: tempfile::TempDir,
    original: PathBuf,
    current: PathBuf,
    history: PathBuf,
    engine: Option<OperationEngine>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.source.disable_cleanup(true);
            self.destination.disable_cleanup(true);
            eprintln!(
                "retained saved-merge source: {}",
                self.source.path().display()
            );
            eprintln!(
                "retained saved-merge destination: {}",
                self.destination.path().display()
            );
        }
    }
}
impl Fixture {
    fn new() -> Self {
        let source = tempfile::tempdir().unwrap();
        let device = fs::metadata(source.path()).unwrap().dev();
        let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
            .into_iter()
            .find_map(|path| {
                (fs::metadata(&path).ok()?.dev() != device)
                    .then(|| {
                        tempfile::Builder::new()
                            .prefix(".canopa-saved-stage-return-")
                            .tempdir_in(path)
                            .ok()
                    })
                    .flatten()
            })
            .expect("two writable filesystems");
        let original = source.path().join("original/project");
        fs::create_dir_all(&original).unwrap();
        let current = source.path().join("moved");
        let history = source.path().join("state/history");
        let engine = Some(OperationEngine::open(&history).unwrap());
        Self {
            source,
            destination,
            original,
            current,
            history,
            engine,
        }
    }
    fn engine(&self) -> &OperationEngine {
        self.engine.as_ref().unwrap()
    }
    fn reopen(&mut self) {
        drop(self.engine.take());
        self.engine = Some(OperationEngine::open(&self.history).unwrap());
    }
    fn prepare_return(&self) {
        self.engine()
            .execute(
                OperationRequest::Rename {
                    source: self.original.clone(),
                    destination: self.current.clone(),
                },
                &Cancellation::default(),
            )
            .unwrap();
        let parent = self.original.parent().unwrap();
        fs::remove_dir(parent).unwrap();
        symlink(self.destination.path(), parent).unwrap();
        fs::create_dir(&self.original).unwrap();
    }
    fn replace_copied_file(&self) {
        self.replace_copied_file_of_size(None);
    }
    fn replace_copied_file_of_size(&self, size: Option<u64>) {
        let nested = self.original.join("nested");
        fs::create_dir(&nested).unwrap();
        for (directory, contents) in [
            ("older", b"older saved work".as_slice()),
            ("replacement", b"incoming replacement".as_slice()),
        ] {
            let seed = self.source.path().join(directory);
            fs::create_dir(&seed).unwrap();
            let file = seed.join("note.txt");
            fs::write(&file, contents).unwrap();
            if directory == "older"
                && let Some(size) = size
            {
                fs::OpenOptions::new()
                    .write(true)
                    .open(&file)
                    .unwrap()
                    .set_len(size)
                    .unwrap();
            }
            let outcome = self
                .engine()
                .transfer(
                    &TransferRequest {
                        kind: TransferKind::Copy,
                        sources: vec![file],
                        destination: nested.clone(),
                    },
                    &Cancellation::default(),
                    |_| TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        apply_to_remaining: false,
                    },
                )
                .unwrap();
            assert!(outcome.failures.is_empty());
        }
    }
    fn require_active_backup(&self) {
        let page = self
            .engine()
            .recovery_items(None, &Cancellation::default())
            .unwrap();
        assert_eq!(page.items.len(), 1);
        let backup = &page.items[0];
        assert!(backup.held_for_undo);
        assert_eq!(backup.status, RecoveryStatus::UndoBackup);
        let entry = backup
            .entry
            .as_ref()
            .expect("active backup remains inspectable");
        assert_eq!(fs::read(&entry.path).unwrap(), b"older saved work");
        assert_eq!(
            fs::metadata(&entry.path).unwrap().dev(),
            fs::metadata(self.destination.path()).unwrap().dev()
        );
        assert!(
            entry
                .path
                .canonicalize()
                .unwrap()
                .starts_with(self.destination.path().join("project/nested"))
        );
    }
}

#[test]
fn copying_a_folder_omits_owned_undo_backups_but_keeps_ordinary_private_looking_folders() {
    let fixture = Fixture::new();
    fixture.replace_copied_file();
    let ordinary = ".canopa-files-stage-user-folder";
    fs::create_dir(fixture.original.join("nested").join(ordinary)).unwrap();
    fs::write(
        fixture
            .original
            .join("nested")
            .join(ordinary)
            .join("previous"),
        b"ordinary user data",
    )
    .unwrap();
    let cancel = Cancellation::default();
    let copied = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![fixture.original.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty() && !copied.needs_attention);
    let copied = fixture.destination.path().join("project/nested");
    assert_eq!(
        fs::read(copied.join("note.txt")).unwrap(),
        b"incoming replacement"
    );
    assert_eq!(
        fs::read(copied.join(ordinary).join("previous")).unwrap(),
        b"ordinary user data"
    );
    let children = fs::read_dir(&copied)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(
        children.len(),
        2,
        "Copy must not clone internal Undo data into the new folder: {children:?}"
    );
    let backups = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(backups.items.len(), 1);
    assert_eq!(backups.items[0].status, RecoveryStatus::UndoBackup);
    assert_eq!(
        fs::read(&backups.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"older saved work"
    );
    fixture.engine().undo(&cancel).unwrap();
    assert!(!fixture.destination.path().join("project").exists());
    fixture.engine().undo(&cancel).unwrap();
    assert_eq!(
        fs::read(fixture.original.join("nested/note.txt")).unwrap(),
        b"older saved work"
    );
    fixture.engine().undo(&cancel).unwrap();
    assert!(!fixture.original.join("nested/note.txt").exists());
}

#[test]
fn moving_a_folder_across_filesystems_keeps_its_backup_catalog_and_successive_older_undo() {
    let mut fixture = Fixture::new();
    fixture.replace_copied_file();
    let ordinary = ".canopa-files-stage-user-folder";
    fs::create_dir(fixture.original.join("nested").join(ordinary)).unwrap();
    fs::write(
        fixture
            .original
            .join("nested")
            .join(ordinary)
            .join("previous"),
        b"ordinary user data",
    )
    .unwrap();
    let cancel = Cancellation::default();
    let moved = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty() && !moved.needs_attention);
    assert_eq!(
        moved.completed, 5,
        "internal saved containers must not inflate user item counts"
    );
    assert!(!fixture.original.exists());
    fixture.reopen();
    let target = fixture.destination.path().join("project/nested");
    assert_eq!(
        fs::read(target.join("note.txt")).unwrap(),
        b"incoming replacement"
    );
    assert_eq!(
        fs::read(target.join(ordinary).join("previous")).unwrap(),
        b"ordinary user data"
    );
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    let saved = &page.items[0];
    assert_eq!(
        saved.status,
        RecoveryStatus::UndoBackup,
        "a completed forward Move must leave its saved backup usable"
    );
    assert_eq!(
        saved.original.as_deref(),
        Some(target.join("note.txt").as_path())
    );
    let path = &saved.entry.as_ref().unwrap().path;
    assert_eq!(fs::read(path).unwrap(), b"older saved work");
    assert_eq!(
        fs::metadata(path).unwrap().dev(),
        fs::metadata(&target).unwrap().dev()
    );
    let recovered = fixture.source.path().join("recovered-forward-backup.txt");
    fixture
        .engine()
        .recover_copy(saved, &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"older saved work");
    fixture.engine().undo(&cancel).unwrap();
    assert!(!recovered.exists());
    fixture.engine().undo(&cancel).unwrap();
    assert!(!fixture.destination.path().join("project").exists());
    fixture.reopen();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].status, RecoveryStatus::UndoBackup);
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"older saved work"
    );
    fixture.engine().undo(&cancel).unwrap();
    assert_eq!(
        fs::read(fixture.original.join("nested/note.txt")).unwrap(),
        b"older saved work"
    );
    fixture.engine().undo(&cancel).unwrap();
    assert!(!fixture.original.join("nested/note.txt").exists());
    assert_eq!(
        fs::read(
            fixture
                .original
                .join("nested")
                .join(ordinary)
                .join("previous")
        )
        .unwrap(),
        b"ordinary user data"
    );
}

#[test]
fn a_partial_folder_move_keeps_recovery_context_on_the_unmoved_file_then_resumes_and_undoes_each_part()
 {
    use canopa_files::transfers::TransferEvent;
    let mut fixture = Fixture::new();
    fixture.replace_copied_file();
    let cancel = Cancellation::default();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    let source_box = page.items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .parent()
        .unwrap()
        .to_owned();
    let source_file = fixture.original.join("nested/note.txt");
    let target_file = fixture.destination.path().join("project/nested/note.txt");
    let moved = fixture.engine().transfer(&TransferRequest {
        kind: TransferKind::Move,
        sources: vec![fixture.original.clone()],
        destination: fixture.destination.path().to_owned(),
    }, &cancel, |event| {
        if matches!(event, TransferEvent::Progress { ref path, .. } if path == &source_box && !path.exists()) {
            TransferReply::Cancel
        } else { TransferReply::Continue }
    }).unwrap();
    assert!(moved.cancelled && moved.failures.is_empty());
    assert!(source_file.exists() && !target_file.exists());
    let first = moved.id.unwrap();
    let stored = serde_json::to_vec(&moved.continuation.unwrap()).unwrap();
    fixture.reopen();
    let continuation = serde_json::from_slice(&stored).unwrap();
    let cancel = Cancellation::default();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].status, RecoveryStatus::UndoBackup);
    assert_eq!(
        page.items[0].original.as_deref(),
        Some(source_file.as_path()),
        "Recovery must describe the file still at the source, even though its backup has moved"
    );
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"older saved work"
    );
    let resumed = fixture
        .engine()
        .resume_transfer(&continuation, &cancel, |_| TransferReply::Continue)
        .unwrap();
    assert!(resumed.failures.is_empty() && resumed.remaining.is_empty());
    assert_ne!(resumed.id, Some(first));
    assert!(!fixture.original.exists());
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(
        page.items[0].original.as_deref(),
        Some(target_file.as_path())
    );
    fixture.engine().undo(&cancel).unwrap();
    assert!(source_file.exists());
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items[0].status, RecoveryStatus::UndoBackup);
    assert_eq!(
        page.items[0].original.as_deref(),
        Some(source_file.as_path())
    );
    fixture.reopen();
    assert_eq!(fixture.engine().latest_undo_id(), Some(first));
    fixture.engine().undo(&cancel).unwrap();
    assert!(!fixture.destination.path().join("project").exists());
    fixture.engine().undo(&cancel).unwrap();
    assert_eq!(fs::read(&source_file).unwrap(), b"older saved work");
    fixture.engine().undo(&cancel).unwrap();
    assert!(!source_file.exists());
}

#[test]
fn folder_merge_keeps_saved_context_with_skipped_and_renamed_files_on_both_filesystems() {
    use canopa_files::transfers::TransferEvent;
    for same_filesystem in [false, true] {
        for skip_first in [false, true] {
            let mut fixture = Fixture::new();
            fixture.replace_copied_file();
            let cancel = Cancellation::default();
            let target_parent = if same_filesystem {
                let path = fixture.source.path().join("same-fs-target");
                fs::create_dir(&path).unwrap();
                path
            } else {
                fixture.destination.path().to_owned()
            };
            let target = target_parent.join("project/nested");
            fs::create_dir_all(&target).unwrap();
            fs::write(target.join("note.txt"), b"unrelated destination note").unwrap();
            let page = fixture.engine().recovery_items(None, &cancel).unwrap();
            let name = page.items[0]
                .entry
                .as_ref()
                .unwrap()
                .path
                .parent()
                .unwrap()
                .file_name()
                .unwrap();
            let occupied = target.join(name);
            fs::create_dir(&occupied).unwrap();
            fs::write(occupied.join("previous"), b"unrelated saved-looking folder").unwrap();
            let request = TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: target_parent,
            };
            let moved = fixture
                .engine()
                .transfer(&request, &cancel, |event| match event {
                    TransferEvent::Conflict {
                        folders, source, ..
                    } => {
                        assert!(
                            source.path.file_name().unwrap() != name,
                            "internal metadata collision should choose its own private name"
                        );
                        TransferReply::Resolve {
                            choice: if folders {
                                ConflictChoice::Merge
                            } else if skip_first {
                                ConflictChoice::Skip
                            } else {
                                ConflictChoice::KeepBoth
                            },
                            apply_to_remaining: false,
                        }
                    }
                    _ => TransferReply::Continue,
                })
                .unwrap();
            assert!(moved.failures.is_empty() && !moved.needs_attention);
            let first = moved.id.unwrap();
            fixture.reopen();
            let source_file = fixture.original.join("nested/note.txt");
            let page = fixture.engine().recovery_items(None, &cancel).unwrap();
            assert_eq!(page.items.len(), 1);
            assert_eq!(page.items[0].status, RecoveryStatus::UndoBackup);
            assert_eq!(
                fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
                b"older saved work"
            );
            assert_ne!(
                page.items[0].entry.as_ref().unwrap().path.parent().unwrap(),
                occupied
            );
            if skip_first {
                assert!(source_file.exists());
                assert_eq!(
                    page.items[0].original.as_deref(),
                    Some(source_file.as_path())
                );
                let resumed = fixture
                    .engine()
                    .transfer(&request, &cancel, |event| match event {
                        TransferEvent::Conflict { folders, .. } => TransferReply::Resolve {
                            choice: if folders {
                                ConflictChoice::Merge
                            } else {
                                ConflictChoice::KeepBoth
                            },
                            apply_to_remaining: false,
                        },
                        _ => TransferReply::Continue,
                    })
                    .unwrap();
                assert!(resumed.failures.is_empty() && resumed.skipped.is_empty());
            }
            assert!(!fixture.original.exists());
            assert_eq!(
                fs::read(target.join("note (2).txt")).unwrap(),
                b"incoming replacement"
            );
            let page = fixture.engine().recovery_items(None, &cancel).unwrap();
            assert_eq!(
                page.items[0].original.as_deref(),
                Some(target.join("note (2).txt").as_path())
            );
            fixture.engine().undo(&cancel).unwrap();
            assert_eq!(fs::read(&source_file).unwrap(), b"incoming replacement");
            if skip_first {
                fixture.reopen();
                assert_eq!(fixture.engine().latest_undo_id(), Some(first));
                fixture.engine().undo(&cancel).unwrap();
            }
            assert_eq!(
                fs::read(target.join("note.txt")).unwrap(),
                b"unrelated destination note"
            );
            assert_eq!(
                fs::read(occupied.join("previous")).unwrap(),
                b"unrelated saved-looking folder"
            );
            fixture.engine().undo(&cancel).unwrap();
            assert_eq!(fs::read(&source_file).unwrap(), b"older saved work");
            fixture.engine().undo(&cancel).unwrap();
            assert!(!source_file.exists());
        }
    }
}

#[test]
fn a_forward_folder_move_keeps_active_and_archived_versions_copyable_after_return_and_expiry() {
    let mut fixture = Fixture::new();
    fixture.replace_copied_file();
    let cancel = Cancellation::default();
    let seed = fixture.source.path().join("protected.txt");
    fs::write(&seed, b"protected seed").unwrap();
    fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed],
                destination: fixture.original.join("nested"),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    fs::write(
        fixture.original.join("nested/protected.txt"),
        b"protected newer work",
    )
    .unwrap();
    fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    let moved = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty() && !fixture.original.exists());
    fixture.reopen();
    let target = fixture.destination.path().join("project/nested");
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 2);
    for item in &page.items {
        let name = if item.held_for_undo {
            "note.txt"
        } else {
            "protected.txt"
        };
        assert_eq!(item.original.as_deref(), Some(target.join(name).as_path()));
        assert_eq!(
            item.status,
            if item.held_for_undo {
                RecoveryStatus::UndoBackup
            } else {
                RecoveryStatus::NeedsRecovery
            }
        );
        assert!(item.entry.as_ref().unwrap().path.starts_with(&target));
        let copied = fixture.source.path().join(format!("recovered-{name}"));
        fixture
            .engine()
            .recover_copy(item, &copied, &cancel)
            .unwrap();
        assert_eq!(
            fs::read(&copied).unwrap(),
            if item.held_for_undo {
                b"older saved work".as_slice()
            } else {
                b"protected newer work".as_slice()
            }
        );
        fixture.engine().undo(&cancel).unwrap();
    }
    fixture.engine().undo(&cancel).unwrap();
    assert!(!fixture.destination.path().join("project").exists());
    fixture.reopen();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 2);
    for item in &page.items {
        let name = if item.held_for_undo {
            "note.txt"
        } else {
            "protected.txt"
        };
        assert_eq!(
            item.original.as_deref(),
            Some(fixture.original.join("nested").join(name).as_path())
        );
    }
    fixture.engine().undo(&cancel).unwrap();
    fixture.engine().undo(&cancel).unwrap();
    for index in 0..101 {
        fixture
            .engine()
            .execute(
                OperationRequest::CreateFolder {
                    path: fixture.source.path().join(format!("expiry-{index}")),
                },
                &cancel,
            )
            .unwrap();
    }
    fixture.reopen();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].held_for_undo);
    assert_eq!(page.items[0].status, RecoveryStatus::NeedsRecovery);
    let recovered = fixture.source.path().join("protected-after-expiry.txt");
    fixture
        .engine()
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"protected newer work");
}

#[test]
fn keeping_both_for_the_returned_root_preserves_forward_saved_ownership_and_older_undo() {
    let mut fixture = Fixture::new();
    fixture.replace_copied_file();
    let cancel = Cancellation::default();
    let moved = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    fs::create_dir(&fixture.original).unwrap();
    fs::write(
        fixture.original.join("newer.txt"),
        b"unrelated newer original folder work",
    )
    .unwrap();
    let owned = fs::metadata(&fixture.original).unwrap().ino();
    let returned = fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    assert_eq!(returned.remaining, 0);
    assert!(!fixture.destination.path().join("project").exists());
    let alias = fixture.original.with_file_name("project (2)");
    assert_eq!(
        fs::read(alias.join("nested/note.txt")).unwrap(),
        b"incoming replacement"
    );
    assert_eq!(fs::metadata(&fixture.original).unwrap().ino(), owned);
    fixture.reopen();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].status, RecoveryStatus::UndoBackup);
    assert_eq!(
        page.items[0].original.as_deref(),
        Some(alias.join("nested/note.txt").as_path())
    );
    assert!(
        page.items[0]
            .entry
            .as_ref()
            .unwrap()
            .path
            .starts_with(&alias)
    );
    fixture.engine().undo(&cancel).unwrap();
    assert_eq!(
        fs::read(alias.join("nested/note.txt")).unwrap(),
        b"older saved work"
    );
    fixture.engine().undo(&cancel).unwrap();
    assert!(!alias.join("nested/note.txt").exists());
    assert_eq!(
        fs::read(fixture.original.join("newer.txt")).unwrap(),
        b"unrelated newer original folder work"
    );
    assert_eq!(fs::metadata(&fixture.original).unwrap().ino(), owned);
}

#[test]
fn resuming_after_destination_reconnect_recognizes_the_saved_handoff_completed_by_reconciliation() {
    use canopa_files::transfers::TransferAccess;
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    };
    let mut fixture = Fixture::new();
    fixture.replace_copied_file_of_size(Some(8 * 1024 * 1024));
    let cancel = Cancellation::default();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    let source = page.items[0].entry.as_ref().unwrap().path.clone();
    let target_parent = fixture.destination.path().join("project/nested");
    let target = target_parent
        .join(source.parent().unwrap().file_name().unwrap())
        .join("previous");
    let held = target_parent.with_file_name("held-nested");
    let unavailable = AtomicBool::new(false);
    let finished = AtomicBool::new(false);
    let request = TransferRequest {
        kind: TransferKind::Move,
        sources: vec![fixture.original.clone()],
        destination: fixture.destination.path().to_owned(),
    };
    let moved = std::thread::scope(|scope| {
        let engine = fixture.engine();
        let watcher = scope.spawn(|| {
            let deadline = Instant::now() + Duration::from_secs(30);
            while Instant::now() < deadline && !finished.load(Ordering::Acquire) {
                if target.exists() && source.exists() {
                    let page = engine
                        .recovery_items(None, &Cancellation::default())
                        .unwrap();
                    if page.items.iter().any(|item| {
                        item.status == RecoveryStatus::UndoBackup
                            && item
                                .entry
                                .as_ref()
                                .is_some_and(|entry| entry.path == source)
                    }) {
                        fs::rename(&target_parent, &held).unwrap();
                        fs::create_dir(&target_parent).unwrap();
                        unavailable.store(true, Ordering::Release);
                        return;
                    }
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            panic!("the actual installed, source-owned saved handoff was not observed");
        });
        let result = engine.transfer_with_check(
            &request,
            &cancel,
            |_, access| {
                if access == TransferAccess::DestinationFolder
                    && unavailable.load(Ordering::Acquire)
                {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotConnected,
                        "private destination temporarily unavailable",
                    ))
                } else {
                    Ok(())
                }
            },
            |_| TransferReply::Continue,
        );
        finished.store(true, Ordering::Release);
        watcher.join().unwrap();
        result.unwrap()
    });
    assert!(moved.needs_attention && moved.failures.is_empty());
    assert!(unavailable.load(Ordering::Acquire));
    assert_eq!(
        fixture
            .engine()
            .interrupted_operations(&cancel)
            .unwrap()
            .len(),
        1,
        "the fixture must interrupt a real journaled handoff, not merely pause the next item"
    );
    let prefix = moved.id.unwrap();
    let continuation = moved.continuation.unwrap();
    assert!(target_parent.read_dir().unwrap().next().is_none());
    fs::remove_dir(&target_parent).unwrap();
    fs::rename(&held, &target_parent).unwrap();
    unavailable.store(false, Ordering::Release);
    let resumed = fixture
        .engine()
        .resume_transfer(&continuation, &cancel, |_| TransferReply::Continue)
        .unwrap();
    assert!(
        resumed.failures.is_empty(),
        "reconciliation must not turn its already completed metadata task into a missing-source failure: {:?}",
        resumed.failures
    );
    assert!(!resumed.needs_attention && resumed.remaining.is_empty() && !fixture.original.exists());
    assert_ne!(resumed.id, Some(prefix));
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].status, RecoveryStatus::UndoBackup);
    let saved = fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap();
    assert!(saved.starts_with(b"older saved work"));
    fixture.engine().undo(&cancel).unwrap();
    fixture.reopen();
    assert_eq!(fixture.engine().latest_undo_id(), Some(prefix));
    fixture.engine().undo(&cancel).unwrap();
    assert!(!fixture.destination.path().join("project").exists());
    fixture.engine().undo(&cancel).unwrap();
    assert_eq!(
        fs::read(fixture.original.join("nested/note.txt")).unwrap(),
        saved
    );
    fixture.engine().undo(&cancel).unwrap();
}

#[test]
fn undo_of_a_later_unrelated_file_move_does_not_move_an_archived_context_back_to_an_old_source() {
    let mut fixture = Fixture::new();
    let cancel = Cancellation::default();
    let nested = fixture.original.join("nested");
    fs::create_dir(&nested).unwrap();
    let seed = fixture.source.path().join("protected.txt");
    fs::write(&seed, b"original copied contents").unwrap();
    fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed],
                destination: nested.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    fs::write(
        nested.join("protected.txt"),
        b"archived work from the original folder",
    )
    .unwrap();
    fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    let moved = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty() && !fixture.original.exists());
    fixture.reopen();
    let destination = fixture.destination.path().join("project/nested");
    let expected_origin = destination.join("protected.txt");
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].held_for_undo);
    assert_eq!(
        page.items[0].original.as_deref(),
        Some(expected_origin.as_path())
    );
    let saved = page.items[0].entry.as_ref().unwrap().path.clone();
    assert_eq!(
        fs::read(&saved).unwrap(),
        b"archived work from the original folder"
    );
    fs::create_dir_all(&nested).unwrap();
    let unrelated = nested.join("protected.txt");
    fs::write(&unrelated, b"unrelated file created after the folder move").unwrap();
    let later = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![unrelated.clone()],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(later.failures.is_empty());
    assert!(!unrelated.exists());
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(
        page.items[0].original.as_deref(),
        Some(expected_origin.as_path())
    );
    fixture.engine().undo(&cancel).unwrap();
    fixture.reopen();
    assert_eq!(
        fs::read(&unrelated).unwrap(),
        b"unrelated file created after the folder move"
    );
    assert!(!expected_origin.exists());
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].status, RecoveryStatus::NeedsRecovery);
    assert_eq!(page.items[0].entry.as_ref().unwrap().path, saved);
    assert_eq!(
        fs::read(&saved).unwrap(),
        b"archived work from the original folder"
    );
    assert_eq!(
        page.items[0].original.as_deref(),
        Some(expected_origin.as_path()),
        "an unrelated later move does not own this archived version's context"
    );
}

#[test]
fn a_recreated_source_folder_cannot_claim_archived_context_from_a_partially_moved_older_folder() {
    use canopa_files::transfers::TransferEvent;
    let mut fixture = Fixture::new();
    let cancel = Cancellation::default();
    let nested = fixture.original.join("nested");
    fs::create_dir(&nested).unwrap();
    let seed = fixture.source.path().join("protected.txt");
    fs::write(&seed, b"copied seed").unwrap();
    fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed],
                destination: nested.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    fs::write(
        nested.join("protected.txt"),
        b"protected work from the older folder",
    )
    .unwrap();
    fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    fs::write(nested.join("remaining.txt"), b"unmoved older-folder file").unwrap();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    let saved_box = page.items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .parent()
        .unwrap()
        .to_owned();
    let partial=fixture.engine().transfer(&TransferRequest {kind:TransferKind::Move,sources:vec![fixture.original.clone()],destination:fixture.destination.path().to_owned()},&cancel,|event| {
        if matches!(event,TransferEvent::Progress {ref path,..} if path==&saved_box && !path.exists()) {TransferReply::Cancel} else {TransferReply::Continue}
    }).unwrap();
    assert!(partial.cancelled && partial.failures.is_empty());
    let held = fixture.source.path().join("held-older-folder");
    fs::rename(&fixture.original, &held).unwrap();
    fs::create_dir_all(&nested).unwrap();
    fs::write(
        nested.join("protected.txt"),
        b"unrelated replacement folder contents",
    )
    .unwrap();
    let cancel = Cancellation::default();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    let expected = nested.join("protected.txt");
    assert_eq!(page.items[0].original.as_deref(), Some(expected.as_path()));
    let saved = page.items[0].entry.as_ref().unwrap().path.clone();
    let replacement = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Merge,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(replacement.failures.is_empty());
    fixture.reopen();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items[0].entry.as_ref().unwrap().path, saved);
    assert_eq!(
        fs::read(&saved).unwrap(),
        b"protected work from the older folder"
    );
    assert_eq!(
        fs::read(held.join("nested/remaining.txt")).unwrap(),
        b"unmoved older-folder file"
    );
    assert_eq!(
        page.items[0].original.as_deref(),
        Some(expected.as_path()),
        "matching parent names do not prove that the replacement folder owns this context"
    );
}

#[test]
fn v15_archived_path_maps_remain_recorded_hints_and_their_data_stays_recoverable() {
    let mut fixture = Fixture::new();
    let cancel = Cancellation::default();
    let nested = fixture.original.join("nested");
    fs::create_dir(&nested).unwrap();
    let seed = fixture.source.path().join("protected.txt");
    fs::write(&seed, b"copied seed").unwrap();
    fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed],
                destination: nested.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    fs::write(
        nested.join("protected.txt"),
        b"protected work from legacy history",
    )
    .unwrap();
    fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    let moved = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    drop(fixture.engine.take());
    // Prepare actual v15 wire/schema input. Its path maps had no step or parent
    // provenance. Observe migration only through Recovery and real files.
    {
        let input = rusqlite::Connection::open(&fixture.history).unwrap();
        let rows: Vec<(u64, String)> = input
            .prepare("SELECT id,body FROM garbage")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for (id, body) in rows {
            let mut body: serde_json::Value = serde_json::from_str(&body).unwrap();
            for movement in body["context_moves"].as_array_mut().unwrap() {
                let fields = movement.as_object_mut().unwrap();
                fields.remove("container_step");
                fields.remove("owner_step");
                fields.remove("parent_identity");
            }
            input
                .execute(
                    "UPDATE garbage SET body=?2 WHERE id=?1",
                    rusqlite::params![id, body.to_string()],
                )
                .unwrap();
        }
        input.execute_batch("DROP TRIGGER recovery_context_deleted; DROP TABLE recovery_context_cleanup; DROP TABLE recovery_context_steps; DROP TABLE namespace_moves; DROP INDEX identity_refs_expected; PRAGMA user_version=15;").unwrap();
    }
    fixture.reopen();
    let target = fixture.destination.path().join("project/nested");
    let recorded = target.join("protected.txt");
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].original_is_recorded());
    assert_eq!(page.items[0].original.as_deref(), Some(recorded.as_path()));
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("protected.txt"), b"new unrelated source").unwrap();
    let unrelated = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![nested.join("protected.txt")],
                destination: target,
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(unrelated.failures.is_empty());
    fixture.engine().undo(&cancel).unwrap();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert!(page.items[0].original_is_recorded());
    assert_eq!(page.items[0].original.as_deref(), Some(recorded.as_path()));
    fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    fixture.reopen();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items[0].status, RecoveryStatus::NeedsRecovery);
    assert!(page.items[0].original_is_recorded());
    assert_eq!(page.items[0].original.as_deref(), Some(recorded.as_path()));
    let copy = fixture.source.path().join("legacy-recovered.txt");
    fixture
        .engine()
        .recover_copy(&page.items[0], &copy, &cancel)
        .unwrap();
    assert_eq!(
        fs::read(copy).unwrap(),
        b"protected work from legacy history"
    );
    assert_eq!(
        fs::read(nested.join("protected.txt")).unwrap(),
        b"new unrelated source"
    );
}

#[test]
fn repeated_folder_moves_and_expired_steps_do_not_lend_archived_context_to_new_file_moves() {
    let mut fixture = Fixture::new();
    let cancel = Cancellation::default();
    let nested = fixture.original.join("nested");
    fs::create_dir(&nested).unwrap();
    let seed = fixture.source.path().join("protected.txt");
    fs::write(&seed, b"seed").unwrap();
    fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed],
                destination: nested,
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    fs::write(
        fixture.original.join("nested/protected.txt"),
        b"protected through many folder moves",
    )
    .unwrap();
    fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    let first = fixture.original.clone();
    let second = fixture.destination.path().join("project");
    let mut current = first.clone();
    for index in 0..112 {
        let destination = if current == first {
            second.parent().unwrap()
        } else {
            first.parent().unwrap()
        };
        let moved = fixture
            .engine()
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![current.clone()],
                    destination: destination.to_owned(),
                },
                &cancel,
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(
            moved.failures.is_empty(),
            "move {index}: {:?}",
            moved.failures
        );
        current = destination.join("project");
    }
    fixture.reopen();
    let expected = current.join("nested/protected.txt");
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].original_is_recorded());
    assert_eq!(page.items[0].original.as_deref(), Some(expected.as_path()));
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"protected through many folder moves"
    );
    let other = if current == first { second } else { first };
    fs::create_dir_all(other.join("nested")).unwrap();
    let source = other.join("nested/protected.txt");
    fs::write(&source, b"new unrelated work after expiry").unwrap();
    let moved = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: current.join("nested"),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    fixture.engine().undo(&cancel).unwrap();
    fixture.reopen();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items[0].original.as_deref(), Some(expected.as_path()));
    assert_eq!(
        fs::read(&source).unwrap(),
        b"new unrelated work after expiry"
    );
    let recovered = fixture.source.path().join("after-context-expiry.txt");
    fixture
        .engine()
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(
        fs::read(recovered).unwrap(),
        b"protected through many folder moves"
    );
}

#[test]
fn reusing_a_context_path_pair_does_not_consume_an_older_saved_container_movement() {
    let mut fixture = Fixture::new();
    let cancel = Cancellation::default();
    let nested = fixture.original.join("nested");
    fs::create_dir(&nested).unwrap();
    let seed = fixture.source.path().join("protected.txt");
    fs::write(&seed, b"copied seed").unwrap();
    fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed],
                destination: nested.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    fs::write(
        nested.join("protected.txt"),
        b"protected across repeated context paths",
    )
    .unwrap();
    fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    let moved = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    let target = fixture.destination.path().join("project/nested");
    let expected = target.join("protected.txt");
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    let saved_box = page.items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .parent()
        .unwrap()
        .to_owned();
    fs::create_dir_all(&nested).unwrap();
    let explicit = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![saved_box],
                destination: nested.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(explicit.failures.is_empty());
    // The container is moved explicitly, then inside another folder whose path
    // pair repeats the first move. Its context is already at the destination,
    // so that second containing-folder move must not acquire the older marker.
    let repeated = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Merge,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(repeated.failures.is_empty());
    fixture.engine().undo(&cancel).unwrap();
    fixture.reopen();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].original.as_deref(), Some(expected.as_path()));
    assert!(
        page.items[0]
            .entry
            .as_ref()
            .unwrap()
            .path
            .starts_with(&nested)
    );
    fixture.engine().undo(&cancel).unwrap();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items[0].original.as_deref(), Some(expected.as_path()));
    assert!(
        page.items[0]
            .entry
            .as_ref()
            .unwrap()
            .path
            .starts_with(&target)
    );
    fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    fixture.reopen();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    let returned = page.items[0].original.as_ref().unwrap();
    assert!(returned.starts_with(fixture.source.path()));
    assert!(!returned.starts_with(&fixture.original));
    assert_eq!(returned.file_name().unwrap(), "protected.txt");
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"protected across repeated context paths"
    );
}

#[test]
fn an_active_backup_keeps_the_actual_file_alias_after_a_renamed_folder_moves_with_keep_both() {
    let mut fixture = Fixture::new();
    fixture.replace_copied_file();
    let cancel = Cancellation::default();
    fixture
        .engine()
        .execute(
            OperationRequest::Rename {
                source: fixture.original.clone(),
                destination: fixture.current.clone(),
            },
            &cancel,
        )
        .unwrap();
    let target = fixture.destination.path().join("moved/nested");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("note.txt"), b"unrelated destination work").unwrap();
    let moved = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.current.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |event| {
                use canopa_files::transfers::TransferEvent;
                let choice = if matches!(event, TransferEvent::Conflict { folders: true, .. }) {
                    ConflictChoice::Merge
                } else {
                    ConflictChoice::KeepBoth
                };
                TransferReply::Resolve {
                    choice,
                    apply_to_remaining: false,
                }
            },
        )
        .unwrap();
    assert!(moved.failures.is_empty() && !fixture.current.exists());
    fixture.reopen();
    let alias = fs::read_dir(&target)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.is_file() && fs::read(path).unwrap() == b"incoming replacement")
        .unwrap();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].original.as_deref(), Some(alias.as_path()));
    assert_eq!(page.items[0].status, RecoveryStatus::UndoBackup);
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"older saved work"
    );
    fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::Merge)
        .unwrap();
    assert_eq!(
        fs::read(fixture.current.join("nested/note.txt")).unwrap(),
        b"incoming replacement"
    );
    fixture.engine().undo(&cancel).unwrap();
    assert_eq!(
        fs::read(fixture.original.join("nested/note.txt")).unwrap(),
        b"incoming replacement"
    );
    fixture.engine().undo(&cancel).unwrap();
    assert_eq!(
        fs::read(fixture.original.join("nested/note.txt")).unwrap(),
        b"older saved work"
    );
    assert_eq!(
        fs::read(target.join("note.txt")).unwrap(),
        b"unrelated destination work"
    );
    fixture.engine().undo(&cancel).unwrap();
    assert!(!fixture.original.join("nested/note.txt").exists());
}

#[test]
fn active_backups_stay_visible_during_partial_returns_and_bind_older_undo_to_the_returned_version()
{
    let mut fixture = Fixture::new();
    fixture.replace_copied_file();
    fs::write(fixture.original.join("z.txt"), b"incoming late work").unwrap();
    fixture.prepare_return();
    fs::write(fixture.original.join("z.txt"), b"newer destination work").unwrap();
    let cancel = Cancellation::default();
    let mut inspected_while_waiting = false;
    let partial = fixture
        .engine()
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                return UndoChoice::Merge;
            }
            assert_eq!(question.destination, fixture.original.join("z.txt"));
            fixture.require_active_backup();
            inspected_while_waiting = true;
            UndoChoice::Skip
        })
        .unwrap();
    assert!(inspected_while_waiting && partial.remaining > 0);
    fixture.require_active_backup();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    let copy = fixture.source.path().join("recovered-copy.txt");
    fixture
        .engine()
        .recover_copy(&page.items[0], &copy, &cancel)
        .unwrap();
    assert_eq!(fs::read(&copy).unwrap(), b"older saved work");
    fixture.engine().undo(&cancel).unwrap();
    assert!(!copy.exists());
    fixture.reopen();
    fixture.require_active_backup();
    let completed = fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    assert_eq!(completed.remaining, 0);
    assert!(!fixture.current.exists());
    fixture.require_active_backup();
    fixture.reopen();
    fixture.engine().undo(&cancel).unwrap();
    assert_eq!(
        fs::read(fixture.original.join("nested/note.txt")).unwrap(),
        b"older saved work"
    );
    assert!(
        fixture
            .engine()
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
    fixture.engine().undo(&cancel).unwrap();
    assert!(!fixture.original.join("nested/note.txt").exists());
    assert_eq!(
        fs::read(fixture.original.join("z.txt")).unwrap(),
        b"newer destination work"
    );
    assert_eq!(
        fs::read(fixture.original.join("z (2).txt")).unwrap(),
        b"incoming late work"
    );
    assert!(
        fixture
            .engine()
            .interrupted_operations(&cancel)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn an_unowned_directory_with_a_saved_container_name_is_returned_as_ordinary_user_data() {
    let fixture = Fixture::new();
    let name = ".canopa-files-stage-user-folder";
    fs::create_dir(fixture.original.join(name)).unwrap();
    fs::write(
        fixture.original.join(name).join("previous"),
        b"ordinary user work",
    )
    .unwrap();
    fixture.prepare_return();
    let cancel = Cancellation::default();
    let returned = fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::Merge)
        .unwrap();
    assert_eq!(returned.remaining, 0);
    assert!(!fixture.current.exists());
    assert_eq!(
        fs::read(fixture.original.join(name).join("previous")).unwrap(),
        b"ordinary user work"
    );
    assert!(
        fixture
            .engine()
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn v13_through_v15_ownership_survives_upgrade_for_active_and_archived_versions_before_return() {
    for version in [13, 14, 15] {
        let mut fixture = Fixture::new();
        fixture.replace_copied_file();
        let seed = fixture.source.path().join("protected.txt");
        fs::write(&seed, b"copied protected seed").unwrap();
        let cancel = Cancellation::default();
        let copy = fixture
            .engine()
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![seed],
                    destination: fixture.original.join("nested"),
                },
                &cancel,
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(copy.failures.is_empty());
        fs::write(
            fixture.original.join("nested/protected.txt"),
            b"protected newer work",
        )
        .unwrap();
        fixture
            .engine()
            .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
            .unwrap();
        fixture.prepare_return();
        drop(fixture.engine.take());
        // These are the only new schema14 tables. No migration has occurred, so
        // optional recreated identities are absent from this authentic v13 input.
        {
            let input = rusqlite::Connection::open(&fixture.history).unwrap();
            input.execute_batch("DROP TRIGGER recovery_context_deleted; DROP TABLE recovery_context_cleanup; DROP TABLE recovery_context_steps; DROP TABLE namespace_moves; DROP INDEX identity_refs_expected;").unwrap();
            if version < 14 {
                input
                    .execute_batch(
                        "DROP TABLE saved_stage_changes; DROP TABLE saved_stage_records;",
                    )
                    .unwrap();
            }
            input.pragma_update(None, "user_version", version).unwrap();
        }
        fixture.reopen();
        let returned = fixture
            .engine()
            .undo_interactive(None, &cancel, |_| UndoChoice::Merge)
            .unwrap();
        assert_eq!(returned.remaining, 0);
        assert!(!fixture.current.exists());
        fixture.reopen();
        let page = fixture.engine().recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(
            page.items.iter().filter(|item| item.held_for_undo).count(),
            1
        );
        for item in &page.items {
            let entry = item
                .entry
                .as_ref()
                .expect("both prior-schema saved versions migrate");
            assert_eq!(
                fs::symlink_metadata(&entry.path).unwrap().dev(),
                fs::metadata(fixture.destination.path()).unwrap().dev()
            );
            assert_eq!(
                fs::read(&entry.path).unwrap(),
                if item.held_for_undo {
                    b"older saved work".as_slice()
                } else {
                    b"protected newer work".as_slice()
                }
            );
        }
        fixture.engine().undo(&cancel).unwrap();
        fixture.engine().undo(&cancel).unwrap();
        let page = fixture.engine().recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(!page.items[0].held_for_undo);
        let copy = fixture.source.path().join("protected-copy.txt");
        fixture
            .engine()
            .recover_copy(&page.items[0], &copy, &cancel)
            .unwrap();
        assert_eq!(fs::read(copy).unwrap(), b"protected newer work");
    }
}

#[test]
fn recreating_a_saved_version_edited_through_an_open_handle_does_not_authorize_its_edits() {
    use std::io::Write;
    let mut fixture = Fixture::new();
    fixture.replace_copied_file();
    let cancel = Cancellation::default();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&page.items[0].entry.as_ref().unwrap().path)
        .unwrap();
    writer
        .write_all(b"edited saved work through open handle")
        .unwrap();
    writer.sync_all().unwrap();
    let moved = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    let forward = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(forward.items[0].status, RecoveryStatus::NeedsRecovery);
    fixture.engine().undo(&cancel).unwrap();
    fixture.reopen();
    fixture.prepare_return();
    let returned = fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::Merge)
        .unwrap();
    assert_eq!(returned.remaining, 0);
    fixture.reopen();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].held_for_undo);
    assert_eq!(page.items[0].status, RecoveryStatus::NeedsRecovery);
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"edited saved work through open handle"
    );
    let mut reviews = 0;
    let skipped = fixture
        .engine()
        .undo_interactive(None, &cancel, |question| {
            assert!(question.can_use_saved_version());
            reviews += 1;
            UndoChoice::Skip
        })
        .unwrap();
    assert_eq!(reviews, 1);
    assert!(skipped.remaining > 0);
    assert_eq!(
        fs::read(fixture.original.join("nested/note.txt")).unwrap(),
        b"incoming replacement"
    );
    fixture
        .engine()
        .undo_interactive(None, &cancel, |question| {
            assert!(question.can_use_saved_version());
            UndoChoice::UseSavedVersion
        })
        .unwrap();
    let kept = fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepItem)
        .unwrap();
    assert_eq!(kept.remaining, 0);
    assert_eq!(kept.kept, vec![fixture.original.join("nested/note.txt")]);
    assert_eq!(
        fs::read(fixture.original.join("nested/note.txt")).unwrap(),
        b"edited saved work through open handle"
    );
    drop(writer);
}

#[test]
fn an_unchanged_recreated_backup_expires_normally_without_leaving_an_orphaned_container() {
    let fixture = Fixture::new();
    fixture.replace_copied_file();
    fixture.prepare_return();
    let cancel = Cancellation::default();
    let returned = fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::Merge)
        .unwrap();
    assert_eq!(returned.remaining, 0);
    fixture.require_active_backup();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    let saved = page.items[0].entry.as_ref().unwrap().path.clone();
    let container = saved.parent().unwrap().to_owned();
    for index in 0..101 {
        fixture
            .engine()
            .execute(
                OperationRequest::CreateFolder {
                    path: fixture.source.path().join(format!("later-{index:03}")),
                },
                &cancel,
            )
            .unwrap();
    }
    assert!(
        fixture
            .engine()
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
    assert!(!saved.exists() && !container.exists());
    assert_eq!(
        fs::read(fixture.original.join("nested/note.txt")).unwrap(),
        b"incoming replacement"
    );
}

#[test]
fn a_saved_raw_symlink_keeps_its_target_and_older_copy_undo_after_returning_across_filesystems() {
    use std::os::unix::ffi::OsStringExt;
    let fixture = Fixture::new();
    let nested = fixture.original.join("nested");
    fs::create_dir(&nested).unwrap();
    let target = std::ffi::OsString::from_vec(vec![b'n', 0xfe, b'x']);
    let seed = fixture.source.path().join("seed");
    fs::create_dir(&seed).unwrap();
    let link = seed.join("note.txt");
    symlink(&target, &link).unwrap();
    let cancel = Cancellation::default();
    let first = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![link],
                destination: nested.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(first.failures.is_empty());
    let original_link = fs::symlink_metadata(fixture.original.join("nested/note.txt")).unwrap();
    let incoming = fixture.source.path().join("incoming");
    fs::create_dir(&incoming).unwrap();
    fs::write(incoming.join("note.txt"), b"replacement for raw link").unwrap();
    let second = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![incoming.join("note.txt")],
                destination: nested,
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(second.failures.is_empty());
    let moved = fixture
        .engine()
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![fixture.original.clone()],
                destination: fixture.destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    let forward = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(forward.items[0].status, RecoveryStatus::UndoBackup);
    assert_eq!(
        fs::read_link(&forward.items[0].entry.as_ref().unwrap().path).unwrap(),
        PathBuf::from(&target)
    );
    fixture.engine().undo(&cancel).unwrap();
    fixture.prepare_return();
    let returned = fixture
        .engine()
        .undo_interactive(None, &cancel, |_| UndoChoice::Merge)
        .unwrap();
    assert_eq!(returned.remaining, 0);
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_eq!(
        fs::read_link(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        PathBuf::from(&target)
    );
    fixture.engine().undo(&cancel).unwrap();
    assert_eq!(
        fs::read_link(fixture.original.join("nested/note.txt")).unwrap(),
        PathBuf::from(&target)
    );
    let restored = fs::symlink_metadata(fixture.original.join("nested/note.txt")).unwrap();
    assert_eq!(restored.mode(), original_link.mode());
    assert_eq!(
        (restored.mtime(), restored.mtime_nsec()),
        (original_link.mtime(), original_link.mtime_nsec()),
        "returning a saved link must preserve its original modification time"
    );
    let removed = fixture
        .engine()
        .undo_interactive(None, &cancel, |_| {
            panic!("returning our own saved link must not invent a changed-copy conflict")
        })
        .unwrap();
    assert_eq!(removed.remaining, 0);
    assert!(fs::symlink_metadata(fixture.original.join("nested/note.txt")).is_err());
}

#[test]
fn a_killed_saved_stage_handoff_reopens_before_and_after_catalog_cutover_without_losing_older_undo()
{
    use std::{os::unix::process::ExitStatusExt, process::Command};
    const LENGTH: u64 = 8 * 1024 * 1024;
    for boundary in ["installed", "catalogued"] {
        let mut fixture = Fixture::new();
        fixture.replace_copied_file_of_size(Some(LENGTH));
        fixture.prepare_return();
        drop(fixture.engine.take());
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "cases::undo_saved_stage_move::crash_saved_stage_handoff_helper",
            ])
            .env("CANOPA_SAVED_STAGE_ROOT", fixture.source.path())
            .env("CANOPA_SAVED_STAGE_BOUNDARY", boundary)
            .status()
            .unwrap();
        assert_eq!(
            status.signal(),
            Some(9),
            "the helper must observe the requested public boundary"
        );
        let witness: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture.source.path().join("witness.json")).unwrap())
                .unwrap();
        assert_eq!(witness["boundary"], boundary);
        assert_eq!(witness["destination_installed"], true);
        assert_eq!(witness["source_still_present"], true);
        // Undo migration records omit all schema15 forward-only wire fields.
        // This is an authentic schema14 interrupted input for startup upgrade.
        {
            let input = rusqlite::Connection::open(&fixture.history).unwrap();
            input.execute_batch("DROP TRIGGER recovery_context_deleted; DROP TABLE recovery_context_cleanup; DROP TABLE recovery_context_steps; DROP TABLE namespace_moves; DROP INDEX identity_refs_expected;").unwrap();
            input.pragma_update(None, "user_version", 14).unwrap();
        }
        let mut observed_pending = false;
        fixture.engine = Some(
            OperationEngine::open_with_recovery_check(&fixture.history, |paths| {
                observed_pending = !paths.is_empty();
                Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "inspect the interrupted handoff before retrying",
                ))
            })
            .unwrap(),
        );
        assert!(
            observed_pending,
            "the kill must leave actual journaled work to reconcile"
        );
        let cancel = Cancellation::default();
        let before_retry = fixture.engine().recovery_items(None, &cancel).unwrap();
        let active = before_retry
            .items
            .iter()
            .find(|item| item.status == RecoveryStatus::UndoBackup)
            .unwrap();
        assert_eq!(
            active.entry.as_ref().unwrap().path,
            PathBuf::from(witness["public_catalog_path"].as_str().unwrap())
        );
        fixture.engine().retry_recovery(&cancel).unwrap();
        assert!(
            fixture
                .engine()
                .interrupted_operations(&cancel)
                .unwrap()
                .is_empty()
        );
        let page = fixture.engine().recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page.items[0].held_for_undo);
        assert_eq!(page.items[0].status, RecoveryStatus::UndoBackup);
        let saved = &page.items[0].entry.as_ref().unwrap().path;
        assert_eq!(fs::metadata(saved).unwrap().len(), LENGTH);
        assert_eq!(
            fs::metadata(saved).unwrap().dev(),
            fs::metadata(fixture.destination.path()).unwrap().dev()
        );
        let bytes = fs::read(saved).unwrap();
        assert!(bytes.starts_with(b"older saved work"));
        assert!(
            bytes[b"older saved work".len()..]
                .iter()
                .all(|byte| *byte == 0)
        );
        let result = fixture
            .engine()
            .undo_interactive(None, &cancel, |_| UndoChoice::Merge)
            .unwrap();
        assert_eq!(result.remaining, 0);
        assert!(!fixture.current.exists());
        fixture.engine().undo(&cancel).unwrap();
        assert_eq!(
            fs::read(fixture.original.join("nested/note.txt")).unwrap(),
            bytes
        );
        fixture.engine().undo(&cancel).unwrap();
        assert!(!fixture.original.join("nested/note.txt").exists());
        assert!(
            fixture
                .engine()
                .recovery_items(None, &cancel)
                .unwrap()
                .items
                .is_empty()
        );
        drop(fixture.engine.take());
    }
}

#[test]
fn a_killed_forward_saved_handoff_reconciles_then_a_retried_folder_move_preserves_both_undo_parts()
{
    use std::{os::unix::process::ExitStatusExt, process::Command};
    const LENGTH: u64 = 8 * 1024 * 1024;
    for boundary in ["installed", "catalogued"] {
        let mut fixture = Fixture::new();
        fixture.replace_copied_file_of_size(Some(LENGTH));
        drop(fixture.engine.take());
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "cases::undo_saved_stage_move::crash_saved_stage_handoff_helper",
            ])
            .env("CANOPA_SAVED_STAGE_ROOT", fixture.source.path())
            .env("CANOPA_SAVED_STAGE_BOUNDARY", boundary)
            .env(
                "CANOPA_SAVED_STAGE_FORWARD_DESTINATION",
                fixture.destination.path(),
            )
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let witness: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture.source.path().join("witness.json")).unwrap())
                .unwrap();
        assert_eq!(witness["boundary"], boundary);
        assert_eq!(witness["destination_installed"], true);
        assert_eq!(witness["source_still_present"], true);
        let mut pending = false;
        fixture.engine = Some(
            OperationEngine::open_with_recovery_check(&fixture.history, |paths| {
                pending = !paths.is_empty();
                Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "inspect the interrupted forward handoff",
                ))
            })
            .unwrap(),
        );
        assert!(pending);
        let cancel = Cancellation::default();
        let page = fixture.engine().recovery_items(None, &cancel).unwrap();
        let active = page
            .items
            .iter()
            .find(|item| item.status == RecoveryStatus::UndoBackup)
            .unwrap();
        assert_eq!(
            active.entry.as_ref().unwrap().path,
            PathBuf::from(witness["public_catalog_path"].as_str().unwrap())
        );
        fixture.engine().retry_recovery(&cancel).unwrap();
        assert!(
            fixture
                .engine()
                .interrupted_operations(&cancel)
                .unwrap()
                .is_empty()
        );
        let partial = fixture.engine().latest_undo_id().unwrap();
        let source_file = fixture.original.join("nested/note.txt");
        assert_eq!(fs::read(&source_file).unwrap(), b"incoming replacement");
        let page = fixture.engine().recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].status, RecoveryStatus::UndoBackup);
        assert_eq!(
            page.items[0].original.as_deref(),
            Some(source_file.as_path())
        );
        let saved = &page.items[0].entry.as_ref().unwrap().path;
        assert_eq!(fs::metadata(saved).unwrap().len(), LENGTH);
        let bytes = fs::read(saved).unwrap();
        assert!(
            bytes.starts_with(b"older saved work")
                && bytes[b"older saved work".len()..]
                    .iter()
                    .all(|byte| *byte == 0)
        );
        // This is a fresh, deliberate retry of the remaining source folder,
        // not a claim that unfinished jobs are already persisted across restart.
        let resumed = fixture
            .engine()
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![fixture.original.clone()],
                    destination: fixture.destination.path().to_owned(),
                },
                &cancel,
                |_| TransferReply::Resolve {
                    choice: ConflictChoice::Merge,
                    apply_to_remaining: false,
                },
            )
            .unwrap();
        assert!(resumed.failures.is_empty() && !fixture.original.exists());
        assert_ne!(resumed.id, Some(partial));
        fixture.engine().undo(&cancel).unwrap();
        assert_eq!(fs::read(&source_file).unwrap(), b"incoming replacement");
        fixture.reopen();
        assert_eq!(fixture.engine().latest_undo_id(), Some(partial));
        fixture.engine().undo(&cancel).unwrap();
        assert!(!fixture.destination.path().join("project").exists());
        fixture.engine().undo(&cancel).unwrap();
        assert_eq!(fs::read(&source_file).unwrap(), bytes);
        fixture.engine().undo(&cancel).unwrap();
        assert!(!source_file.exists());
    }
}

#[test]
#[ignore = "child fixture; killed after observing installed saved data or the public catalog cutover"]
fn crash_saved_stage_handoff_helper() {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };
    let root = PathBuf::from(std::env::var_os("CANOPA_SAVED_STAGE_ROOT").unwrap());
    let boundary = std::env::var("CANOPA_SAVED_STAGE_BOUNDARY").unwrap();
    let forward = std::env::var_os("CANOPA_SAVED_STAGE_FORWARD_DESTINATION").map(|destination| {
        TransferRequest {
            kind: TransferKind::Move,
            sources: vec![root.join("original/project")],
            destination: PathBuf::from(destination),
        }
    });
    let engine = Arc::new(OperationEngine::open(root.join("state/history")).unwrap());
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert_eq!(page.items.len(), 1);
    let source = page.items[0].entry.as_ref().unwrap().path.clone();
    let stage_name = source.parent().unwrap().file_name().unwrap();
    let target = forward
        .as_ref()
        .map_or_else(
            || root.join("original/project/nested"),
            |request| request.destination.join("project/nested"),
        )
        .join(stage_name)
        .join("previous");
    let finished = Arc::new(AtomicBool::new(false));
    let reader = engine.clone();
    let done = finished.clone();
    let watcher = std::thread::spawn(move || -> () {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut last_observation = None;
        while Instant::now() < deadline && !done.load(Ordering::Acquire) {
            let source_present = fs::symlink_metadata(&source).is_ok()
                || fs::symlink_metadata(source.with_file_name("folder")).is_ok();
            if fs::symlink_metadata(&target).is_ok() && source_present {
                let page = reader
                    .recovery_items(None, &Cancellation::default())
                    .unwrap();
                if let Some(item) = page.items.first()
                    && let Some(entry) = &item.entry
                {
                    last_observation = Some((entry.path.clone(), item.held_for_undo, item.status));
                    let selected = if boundary == "installed" {
                        &source
                    } else {
                        &target
                    };
                    if &entry.path == selected
                        && item.held_for_undo
                        && item.status == RecoveryStatus::UndoBackup
                    {
                        fs::write(root.join("witness.json"), serde_json::to_vec(&serde_json::json!({
                            "boundary":boundary,"destination_installed":true,"source_still_present":true,
                            "public_catalog_path":entry.path,
                        })).unwrap()).unwrap();
                        rustix::process::kill_process(
                            rustix::process::getpid(),
                            rustix::process::Signal::KILL,
                        )
                        .unwrap();
                        unreachable!("SIGKILL must terminate this helper");
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!(
            "requested {boundary} handoff boundary was not observed; source={source:?}, target={target:?}, last={last_observation:?}"
        );
    });
    let outcome = if let Some(request) = forward {
        format!(
            "{:?}",
            engine.transfer(&request, &Cancellation::default(), |_| {
                TransferReply::Continue
            })
        )
    } else {
        format!(
            "{:?}",
            engine.undo_interactive(None, &Cancellation::default(), |_| UndoChoice::Merge)
        )
    };
    finished.store(true, Ordering::Release);
    watcher.join().unwrap();
    panic!(
        "the helper completed without observing its requested boundary: {:?}",
        outcome
    );
}

#[test]
fn an_occupied_saved_stage_destination_allows_skip_then_keep_both_without_merging_private_data() {
    let mut fixture = Fixture::new();
    fixture.replace_copied_file();
    let cancel = Cancellation::default();
    let backup = fixture.engine().recovery_items(None, &cancel).unwrap();
    let stage_name = backup.items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .parent()
        .unwrap()
        .file_name()
        .unwrap();
    let occupied = fixture.original.join("nested").join(stage_name);
    fixture.prepare_return();
    fs::create_dir_all(&occupied).unwrap();
    fs::write(occupied.join("previous"), b"unrelated destination work").unwrap();
    let mut asked = false;
    let partial = fixture
        .engine()
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                return UndoChoice::Merge;
            }
            assert_eq!(question.destination, occupied);
            assert!(!question.can_replace());
            asked = true;
            UndoChoice::Skip
        })
        .unwrap();
    assert!(asked && partial.remaining > 0);
    assert_eq!(
        fs::read(occupied.join("previous")).unwrap(),
        b"unrelated destination work"
    );
    assert_eq!(
        fs::read(
            fixture
                .engine()
                .recovery_items(None, &cancel)
                .unwrap()
                .items[0]
                .entry
                .as_ref()
                .unwrap()
                .path
                .clone()
        )
        .unwrap(),
        b"older saved work"
    );
    fixture.reopen();
    let complete = fixture
        .engine()
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.destination, occupied);
            assert!(!question.can_merge());
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(complete.remaining, 0);
    assert!(!fixture.current.exists());
    fixture.require_active_backup();
    let page = fixture.engine().recovery_items(None, &cancel).unwrap();
    assert_ne!(
        page.items[0].entry.as_ref().unwrap().path.parent().unwrap(),
        occupied
    );
    fixture.reopen();
    fixture.engine().undo(&cancel).unwrap();
    assert_eq!(
        fs::read(fixture.original.join("nested/note.txt")).unwrap(),
        b"older saved work"
    );
    fixture.engine().undo(&cancel).unwrap();
    assert!(!fixture.original.join("nested/note.txt").exists());
    assert_eq!(
        fs::read(occupied.join("previous")).unwrap(),
        b"unrelated destination work"
    );
    assert!(
        fixture
            .engine()
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn a_malformed_migration_stays_inspectable_and_allows_copying_its_prepared_data() {
    use serde_json::json;
    use xxhash_rust::xxh3::xxh3_128;
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("state/history");
    let target = root.path().join("created-folder");
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(&history).unwrap();
    let operation = engine
        .execute(
            OperationRequest::CreateFolder {
                path: target.clone(),
            },
            &cancel,
        )
        .unwrap();
    drop(engine);
    let source = root.path().join(".canopa-files-stage-source");
    let prepared = root.path().join(".canopa-files-stage-prepared");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&prepared).unwrap();
    let bytes = b"saved data remains recoverable";
    fs::write(source.join("previous"), bytes).unwrap();
    fs::write(prepared.join("folder"), bytes).unwrap();
    let identity =
        |metadata: &fs::Metadata| json!({"device":metadata.dev(),"inode":metadata.ino()});
    let stage = |directory: &std::path::Path, payload: Option<&std::path::Path>| {
        json!({
            "directory":directory,
            "identity":identity(&fs::symlink_metadata(directory).unwrap()),
            "parent_identity":identity(&fs::metadata(directory.parent().unwrap()).unwrap()),
            "payload_identity":payload.map(|path| identity(&fs::symlink_metadata(path).unwrap())),
        })
    };
    let metadata = fs::symlink_metadata(source.join("previous")).unwrap();
    let fingerprint = json!({"size":metadata.len(),"mode":metadata.mode() & 0o777,
        "modified":metadata.mtime(),"modified_nanos":metadata.mtime_nsec(),
        "link":false,"xxh3_128":xxh3_128(bytes).to_be_bytes().to_vec()});
    // This is a malformed journal input, not a private-store output assertion:
    // CreatedFolder has no return path and cannot represent a saved-stage move.
    let journal = json!({"direction":"stage-migration","migration":{
        "id":operation.id,"ordinal":0,
        "change":{"kind":"created-folder","path":target,
            "identity":identity(&fs::metadata(&target).unwrap())},
        "source":stage(&source,None),
        "source_version":{"identity":identity(&metadata),"size":metadata.len(),
            "mode":metadata.mode(),"modified":metadata.mtime(),"modified_nanos":metadata.mtime_nsec(),
            "changed":metadata.ctime(),"changed_nanos":metadata.ctime_nsec()},
        "source_fingerprint":fingerprint,
        "stage":stage(&prepared,Some(&prepared.join("folder"))),
        "copy_identity":identity(&fs::metadata(prepared.join("folder")).unwrap()),
        "fingerprint":fingerprint,"context":null,"relocation":null,"catalogued":false,
    }});
    {
        let input = rusqlite::Connection::open(&history).unwrap();
        input
            .execute("INSERT INTO pending VALUES(1,?1)", [journal.to_string()])
            .unwrap();
    }
    let engine = OperationEngine::open(&history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.interrupted.len(), 1);
    assert!(
        page.interrupted[0]
            .message
            .contains("invalid saved-stage return change")
    );
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].status, RecoveryStatus::NeedsRecovery);
    assert_eq!(
        page.items[0].entry.as_ref().unwrap().path,
        prepared.join("folder")
    );
    let recovered = root.path().join("recovered.txt");
    engine
        .recover_copy(&page.items[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), bytes);
    assert_eq!(fs::read(source.join("previous")).unwrap(), bytes);
    assert_eq!(fs::read(prepared.join("folder")).unwrap(), bytes);
    assert!(target.is_dir());
    assert_eq!(engine.interrupted_operations(&cancel).unwrap().len(), 1);
}

#[test]
fn a_migrated_saved_container_keeps_original_proof_for_later_moves_and_older_undo() {
    use canopa_files::transfers::TransferEvent;
    let root = tempfile::Builder::new()
        .prefix("canopa-saved-cross-owner-")
        .tempdir()
        .unwrap();
    let destination = tempfile::Builder::new()
        .prefix(".canopa-saved-cross-owner-output-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_ne!(
            fs::metadata(root.path()).unwrap().dev(),
            fs::metadata(destination.path()).unwrap().dev()
        );
        fs::create_dir(root.path().join("seed")).unwrap();
        fs::create_dir(root.path().join("incoming")).unwrap();
        fs::write(root.path().join("seed/item"), b"Selected replacement").unwrap();
        fs::write(root.path().join("incoming/item"), b"Original saved data").unwrap();
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let result = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![root.path().join("seed/item")],
                    destination: root.path().join("incoming"),
                },
                &Cancellation::default(),
                |_| TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
            )
            .unwrap();
        assert_eq!(result.completed, 1);
        let page = engine
            .recovery_items(None, &Cancellation::default())
            .unwrap();
        let source = page.items[0]
            .entry
            .as_ref()
            .unwrap()
            .path
            .parent()
            .unwrap()
            .to_owned();
        let moved = destination.path().join(source.file_name().unwrap());
        let result = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![source.clone()],
                    destination: destination.path().to_owned(),
                },
                &Cancellation::default(),
                |event| match event {
                    TransferEvent::Progress { .. } | TransferEvent::Verifying { .. } => {
                        TransferReply::Continue
                    }
                    _ => TransferReply::Cancel,
                },
            )
            .unwrap();
        assert_eq!(
            result.completed, 1,
            "cancelled={}, attention={:?}, failures={:?}",
            result.cancelled, result.attention, result.failures
        );
        assert!(result.failures.is_empty());
        let inode = fs::metadata(&moved).unwrap().ino();
        let next = destination.path().join("next");
        fs::create_dir(&next).unwrap();
        let result = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![moved.clone()],
                    destination: next.clone(),
                },
                &Cancellation::default(),
                |event| match event {
                    TransferEvent::Progress { .. } | TransferEvent::Verifying { .. } => {
                        TransferReply::Continue
                    }
                    _ => TransferReply::Cancel,
                },
            )
            .unwrap();
        assert_eq!(
            result.completed, 1,
            "cancelled={}, attention={:?}, failures={:?}",
            result.cancelled, result.attention, result.failures
        );
        assert!(result.failures.is_empty());
        let final_root = next.join(source.file_name().unwrap());
        assert_eq!(fs::metadata(&final_root).unwrap().ino(), inode);
        assert_eq!(
            fs::read(final_root.join("previous")).unwrap(),
            b"Original saved data"
        );
        engine.undo(&Cancellation::default()).unwrap();
        assert_eq!(fs::metadata(&moved).unwrap().ino(), inode);
        engine.undo(&Cancellation::default()).unwrap();
        assert_eq!(
            fs::read(source.join("previous")).unwrap(),
            b"Original saved data"
        );
        engine.undo(&Cancellation::default()).unwrap();
        assert_eq!(
            fs::read(root.path().join("incoming/item")).unwrap(),
            b"Original saved data"
        );
    }));
    if let Err(error) = checked {
        eprintln!(
            "retained saved-cross owner source: {}",
            root.keep().display()
        );
        eprintln!(
            "retained saved-cross owner destination: {}",
            destination.keep().display()
        );
        std::panic::resume_unwind(error);
    }
}
