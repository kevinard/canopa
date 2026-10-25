use canopa_files::{
    Cancellation,
    browse::EntryKind,
    operations::{OperationEngine, OperationRequest, RecoveryStatus},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::os::unix::fs::FileTypeExt;
use std::{
    fs,
    os::unix::{ffi::OsStringExt, fs::symlink},
};

struct Tree {
    engine: OperationEngine,
    saved: std::path::PathBuf,
    current: std::path::PathBuf,
    held: std::path::PathBuf,
    root: tempfile::TempDir,
}
impl Tree {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let seed = root.path().join("seed");
        let target = root.path().join("target");
        fs::create_dir(&seed).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(seed.join("note.txt"), b"incoming work").unwrap();
        let current = target.join("note.txt");
        fs::write(&current, b"older saved work").unwrap();
        let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let copied = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![seed.join("note.txt")],
                    destination: target,
                },
                &cancel,
                |_| TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
            )
            .unwrap();
        assert!(copied.failures.is_empty());
        let saved = engine.recovery_items(None, &cancel).unwrap().items[0]
            .entry
            .as_ref()
            .unwrap()
            .path
            .clone();
        let held = root.path().join("Held saved file.txt");
        fs::rename(&saved, &held).unwrap();
        fs::create_dir(&saved).unwrap();
        fs::write(saved.join("a.txt"), b"first recovered work").unwrap();
        fs::write(saved.join("b.txt"), b"second recovered work").unwrap();
        fs::create_dir(saved.join("nested")).unwrap();
        fs::write(saved.join("nested/c.txt"), b"nested recovered work").unwrap();
        Self {
            engine,
            saved,
            current,
            held,
            root,
        }
    }
    fn item(&self) -> canopa_files::operations::RecoveryItem {
        self.engine
            .recovery_items(None, &Cancellation::default())
            .unwrap()
            .items
            .remove(0)
    }
    fn partial(
        &self,
        destination: &std::path::Path,
    ) -> canopa_files::operations::RecoveryCopyContinuation {
        use canopa_files::transfers::TransferEvent;
        let outcome=self.engine.recover_copy_with_check(&self.item(),destination,&Cancellation::default(),|_,_|Ok(()),|event| {
            if matches!(event,TransferEvent::Progress { ref path,.. } if path==&self.saved.join("a.txt") && destination.join("a.txt").exists()) {TransferReply::Cancel} else {TransferReply::Continue}
        }).unwrap();
        assert!(outcome.cancelled && outcome.failures.is_empty());
        assert!(outcome.id.is_some());
        assert_eq!(outcome.completed, 2);
        assert_eq!(
            fs::read(destination.join("a.txt")).unwrap(),
            b"first recovered work"
        );
        assert!(!destination.join("b.txt").exists());
        outcome.continuation.unwrap()
    }
    fn originals_intact(&self) {
        assert_eq!(fs::read(&self.current).unwrap(), b"incoming work");
        assert_eq!(fs::read(&self.held).unwrap(), b"older saved work");
        assert_eq!(
            fs::read(self.saved.join("a.txt")).unwrap(),
            b"first recovered work"
        );
        assert_eq!(
            fs::read(self.saved.join("b.txt")).unwrap(),
            b"second recovered work"
        );
    }
}

#[test]
fn cancelled_directory_recovery_resumes_exact_remaining_work_in_separate_undo_entries() {
    let mut tree = Tree::new();
    let destination = tree.root.path().join("Recovered tree");
    let stored = serde_json::to_vec(&tree.partial(&destination)).unwrap();
    drop(tree.engine);
    tree.engine = OperationEngine::open(tree.root.path().join("state/history")).unwrap();
    let plan = serde_json::from_slice(&stored).unwrap();
    let cancel = Cancellation::default();
    let resumed = tree
        .engine
        .resume_recovery_copy_with_check(&plan, &cancel, |_, _| Ok(()), |_| TransferReply::Continue)
        .unwrap();
    assert!(!resumed.cancelled && resumed.failures.is_empty());
    assert_eq!(resumed.completed, 3);
    assert!(resumed.continuation.is_none());
    assert_eq!(
        fs::read(destination.join("nested/c.txt")).unwrap(),
        b"nested recovered work"
    );
    tree.engine.undo(&cancel).unwrap();
    assert!(destination.join("a.txt").exists());
    assert!(!destination.join("b.txt").exists());
    assert!(!destination.join("nested").exists());
    tree.engine.undo(&cancel).unwrap();
    assert!(!destination.exists());
    tree.originals_intact();
}

#[test]
fn directory_recovery_never_replaces_new_output_and_undo_keeps_that_work() {
    use canopa_files::operations::UndoChoice;
    use canopa_files::transfers::TransferEvent;
    let tree = Tree::new();
    let destination = tree.root.path().join("Recovered tree");
    let plan = tree.partial(&destination);
    let cancel = Cancellation::default();
    fs::write(destination.join("b.txt"), b"new output work").unwrap();
    let outcome = tree
        .engine
        .resume_recovery_copy_with_check(
            &plan,
            &cancel,
            |_, _| Ok(()),
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
    assert_eq!(outcome.failures.len(), 1);
    assert_eq!(outcome.failures[0].kind, std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read(destination.join("b.txt")).unwrap(),
        b"new output work"
    );
    let retry = tree
        .engine
        .resume_recovery_copy_with_check(
            &outcome.retry_continuation.unwrap(),
            &cancel,
            |_, _| Ok(()),
            |event| {
                if matches!(event, TransferEvent::Conflict { .. }) {
                    TransferReply::Resolve {
                        choice: ConflictChoice::KeepBoth,
                        apply_to_remaining: false,
                    }
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    assert!(retry.failures.is_empty());
    assert_eq!(
        fs::read(destination.join("b (2).txt")).unwrap(),
        b"second recovered work"
    );
    tree.engine.undo(&cancel).unwrap();
    tree.engine.undo(&cancel).unwrap();
    let undone = tree
        .engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepItem)
        .unwrap();
    assert_eq!(undone.remaining, 0);
    assert_eq!(
        fs::read(destination.join("b.txt")).unwrap(),
        b"new output work"
    );
    assert!(!destination.join("a.txt").exists());
    tree.originals_intact();
}

#[test]
fn a_replaced_recovery_output_root_is_refused_until_the_original_folder_returns() {
    let tree = Tree::new();
    let destination = tree.root.path().join("Recovered tree");
    let plan = tree.partial(&destination);
    let cancel = Cancellation::default();
    let held = tree.root.path().join("Held output");
    fs::rename(&destination, &held).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("new.txt"), b"unrelated output root").unwrap();
    let paused = tree
        .engine
        .resume_recovery_copy_with_check(&plan, &cancel, |_, _| Ok(()), |_| TransferReply::Continue)
        .unwrap();
    assert!(paused.needs_attention && paused.completed == 0 && paused.id.is_none());
    assert!(
        paused
            .attention
            .as_ref()
            .unwrap()
            .contains("output folder changed")
    );
    let plan = paused.continuation.unwrap();
    assert!(!destination.join("b.txt").exists());
    let unrelated = tree.root.path().join("Unrelated output");
    fs::rename(&destination, &unrelated).unwrap();
    fs::rename(&held, &destination).unwrap();
    let resumed = tree
        .engine
        .resume_recovery_copy_with_check(&plan, &cancel, |_, _| Ok(()), |_| TransferReply::Continue)
        .unwrap();
    assert!(resumed.failures.is_empty() && !resumed.needs_attention);
    assert_eq!(
        fs::read(unrelated.join("new.txt")).unwrap(),
        b"unrelated output root"
    );
    tree.originals_intact();
}

#[test]
fn recovery_continuation_waits_for_its_original_destination_parent() {
    let tree = Tree::new();
    let parent = tree.root.path().join("Recovery destination");
    fs::create_dir(&parent).unwrap();
    let destination = parent.join("Recovered tree");
    let plan = tree.partial(&destination);
    let cancel = Cancellation::default();
    let held = tree.root.path().join("Held destination");
    fs::rename(&parent, &held).unwrap();
    fs::create_dir(&parent).unwrap();
    fs::write(parent.join("new.txt"), b"unrelated parent work").unwrap();
    let paused = tree
        .engine
        .resume_recovery_copy_with_check(&plan, &cancel, |_, _| Ok(()), |_| TransferReply::Continue)
        .unwrap();
    assert!(paused.needs_attention && paused.completed == 0 && !destination.exists());
    let unrelated = tree.root.path().join("Unrelated parent");
    fs::rename(&parent, &unrelated).unwrap();
    fs::rename(&held, &parent).unwrap();
    let done = tree
        .engine
        .resume_recovery_copy_with_check(
            &paused.continuation.unwrap(),
            &cancel,
            |_, _| Ok(()),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(!done.needs_attention && done.failures.is_empty());
    assert_eq!(
        fs::read(unrelated.join("new.txt")).unwrap(),
        b"unrelated parent work"
    );
    tree.originals_intact();
}

#[test]
fn a_changed_saved_root_requires_refresh_and_preserves_its_partial_output() {
    let tree = Tree::new();
    let destination = tree.root.path().join("Recovered tree");
    let plan = tree.partial(&destination);
    let cancel = Cancellation::default();
    fs::write(tree.saved.join("new.txt"), b"new saved work").unwrap();
    assert_eq!(
        tree.engine
            .resume_recovery_copy_with_check(
                &plan,
                &cancel,
                |_, _| Ok(()),
                |_| TransferReply::Continue
            )
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(
        fs::read(destination.join("a.txt")).unwrap(),
        b"first recovered work"
    );
    assert!(!destination.join("b.txt").exists());
    let fresh = tree.root.path().join("Refreshed recovery");
    tree.engine
        .recover_copy(&tree.item(), &fresh, &cancel)
        .unwrap();
    assert_eq!(fs::read(fresh.join("new.txt")).unwrap(), b"new saved work");
    tree.engine.undo(&cancel).unwrap();
    tree.engine.undo(&cancel).unwrap();
    assert!(!destination.exists() && !fresh.exists());
    tree.originals_intact();
}

#[test]
fn directory_recovery_continues_after_its_identified_saved_container_moves() {
    let tree = Tree::new();
    let destination = tree.root.path().join("Recovered tree");
    let plan = tree.partial(&destination);
    let cancel = Cancellation::default();
    let original_parent = tree.current.parent().unwrap();
    let moved = tree.root.path().join("Moved target");
    tree.engine
        .execute(
            OperationRequest::Rename {
                source: original_parent.to_owned(),
                destination: moved.clone(),
            },
            &cancel,
        )
        .unwrap();
    let resumed = tree
        .engine
        .resume_recovery_copy_with_check(&plan, &cancel, |_, _| Ok(()), |_| TransferReply::Continue)
        .unwrap();
    assert!(resumed.failures.is_empty() && !resumed.needs_attention);
    assert_eq!(
        fs::read(destination.join("nested/c.txt")).unwrap(),
        b"nested recovered work"
    );
    tree.engine.undo(&cancel).unwrap();
    tree.engine.undo(&cancel).unwrap();
    tree.engine.undo(&cancel).unwrap();
    assert!(!destination.exists());
    tree.originals_intact();
}

#[test]
fn unsupported_saved_children_report_partial_copy_and_leave_valid_work_undoable() {
    let tree = Tree::new();
    let destination = tree.root.path().join("Recovered tree");
    let cancel = Cancellation::default();
    let fifo = tree.saved.join("z-pipe");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let outcome = tree
        .engine
        .recover_copy_with_check(
            &tree.item(),
            &destination,
            &cancel,
            |_, _| Ok(()),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(outcome.failures.len(), 1);
    assert!(outcome.id.is_some() && outcome.retry_continuation.is_some());
    assert_eq!(
        fs::read(destination.join("a.txt")).unwrap(),
        b"first recovered work"
    );
    assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
    tree.engine.undo(&cancel).unwrap();
    assert!(!destination.exists());
    tree.originals_intact();
}

#[test]
fn an_unexpected_saved_directory_can_be_copied_out_after_reopen_and_history_expiry() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    let current = destination.join("note.txt");
    fs::write(source.join("note.txt"), b"incoming work").unwrap();
    fs::write(&current, b"older saved file").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.join("note.txt")],
                destination,
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    let page = engine.recovery_items(None, &cancel).unwrap();
    let saved = page.items[0].entry.as_ref().unwrap().path.clone();
    let held = root.path().join("Held original saved file.txt");
    fs::rename(&saved, &held).unwrap();
    fs::create_dir(&saved).unwrap();
    fs::create_dir(saved.join("nested")).unwrap();
    fs::write(saved.join("nested/work.txt"), b"newer saved directory work").unwrap();
    let raw_name = std::ffi::OsString::from_vec(b"notes-\xff.txt".to_vec());
    fs::write(saved.join(&raw_name), b"raw filename work").unwrap();
    let raw_target = std::ffi::OsString::from_vec(b"../missing-\xfe".to_vec());
    symlink(&raw_target, saved.join("raw link")).unwrap();
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    for index in 0..105 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("history-{index}")),
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
    assert_eq!(item.entry.as_ref().unwrap().kind, EntryKind::Directory);
    assert_eq!(
        fs::read(saved.join("nested/work.txt")).unwrap(),
        b"newer saved directory work"
    );
    let recovered = root.path().join("Recovered directory");
    engine.recover_copy(item, &recovered, &cancel).unwrap();
    assert_eq!(
        fs::read(recovered.join("nested/work.txt")).unwrap(),
        b"newer saved directory work"
    );
    assert_eq!(
        fs::read(recovered.join(&raw_name)).unwrap(),
        b"raw filename work"
    );
    assert_eq!(
        fs::read_link(recovered.join("raw link"))
            .unwrap()
            .as_os_str(),
        raw_target
    );
    assert_eq!(fs::read(&current).unwrap(), b"incoming work");
    assert_eq!(fs::read(&held).unwrap(), b"older saved file");
    assert_eq!(
        fs::read(saved.join("nested/work.txt")).unwrap(),
        b"newer saved directory work"
    );
    engine.undo(&cancel).unwrap();
    assert!(!recovered.exists());
    assert!(saved.join("nested/work.txt").exists());
    assert_eq!(
        engine.recovery_items(None, &cancel).unwrap().items[0].status,
        RecoveryStatus::NeedsRecovery
    );
}

#[test]
fn recovery_refuses_existing_and_racing_output_roots_without_adopting_their_contents() {
    use canopa_files::transfers::TransferAccess;
    for racing in [false, true] {
        let tree = Tree::new();
        let destination = tree.root.path().join("Recovered tree");
        let create_foreign = || {
            fs::create_dir(&destination).unwrap();
            fs::write(destination.join("new.txt"), b"unrelated output work").unwrap();
        };
        if !racing {
            create_foreign();
        }
        let previous_undo = tree.engine.latest_undo_id();
        let outcome = tree.engine.recover_copy_with_check(
            &tree.item(),
            &destination,
            &Cancellation::default(),
            |path, access| {
                if racing
                    && access == TransferAccess::SourceEntry
                    && path == tree.saved
                    && !destination.exists()
                {
                    create_foreign();
                }
                Ok(())
            },
            |event| {
                assert!(
                    !matches!(
                        event,
                        canopa_files::transfers::TransferEvent::Conflict { .. }
                    ),
                    "a fresh Recovery root collision must not offer Merge or Replace"
                );
                TransferReply::Continue
            },
        );
        if racing {
            let outcome = outcome.unwrap();
            assert_eq!(outcome.completed, 0);
            assert!(outcome.id.is_none());
            assert_eq!(outcome.failures.len(), 1);
            assert_eq!(outcome.failures[0].kind, std::io::ErrorKind::AlreadyExists);
        } else {
            assert_eq!(
                outcome.unwrap_err().kind(),
                std::io::ErrorKind::AlreadyExists
            );
        }
        assert_eq!(tree.engine.latest_undo_id(), previous_undo);
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
        assert_eq!(
            fs::read(destination.join("new.txt")).unwrap(),
            b"unrelated output work"
        );
        tree.originals_intact();
    }
}

#[test]
fn interrupted_directory_recovery_cleans_only_incomplete_bytes_and_keeps_its_prefix_undoable() {
    use std::{
        os::unix::process::ExitStatusExt,
        process::Command,
        time::{Duration, Instant},
    };
    let Tree {
        engine,
        saved,
        current,
        held,
        root,
    } = Tree::new();
    let large = vec![b'x'; 16 * 1024 * 1024];
    fs::write(saved.join("b.txt"), &large).unwrap();
    drop(engine);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "interrupted_directory_recovery_child",
        ])
        .env("CANOPA_DIRECTORY_RECOVERY_ROOT", root.path())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !root.path().join("copying-bytes").exists() {
        assert!(
            child.try_wait().unwrap().is_none(),
            "copy must reach its public byte-progress boundary"
        );
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("directory Recovery copy never reported actual bytes");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    child.kill().unwrap();
    assert_eq!(child.wait().unwrap().signal(), Some(9));
    let destination = root.path().join("Recovered tree");
    let partial = fs::read_dir(&destination)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("folder"))
        .find(|path| fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0))
        .unwrap();
    let bytes = fs::read(&partial).unwrap();
    assert!(!bytes.is_empty() && bytes.len() < large.len());
    let history = root.path().join("state/history");
    let offline = OperationEngine::open_with_recovery_check(&history, |_| {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "Recovery output device is unavailable",
        ))
    })
    .unwrap();
    assert_eq!(fs::read(&partial).unwrap(), bytes);
    assert_eq!(
        offline
            .interrupted_operations(&Cancellation::default())
            .unwrap()
            .len(),
        1
    );
    drop(offline);
    let engine = OperationEngine::open(&history).unwrap();
    assert!(!partial.exists());
    assert!(!destination.join("b.txt").exists());
    assert!(!destination.join("nested").exists());
    assert_eq!(
        fs::read(destination.join("a.txt")).unwrap(),
        b"first recovered work"
    );
    assert!(
        engine
            .interrupted_operations(&Cancellation::default())
            .unwrap()
            .is_empty()
    );
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].entry.as_ref().unwrap().kind,
        EntryKind::Directory
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!destination.exists());
    assert_eq!(fs::read(saved.join("b.txt")).unwrap(), large);
    assert_eq!(fs::read(&current).unwrap(), b"incoming work");
    assert_eq!(fs::read(&held).unwrap(), b"older saved work");
}

#[test]
#[ignore = "parent kills the copy after real child bytes are written"]
fn interrupted_directory_recovery_child() {
    use canopa_files::transfers::TransferEvent;
    let root =
        std::path::PathBuf::from(std::env::var_os("CANOPA_DIRECTORY_RECOVERY_ROOT").unwrap());
    assert!(root.starts_with(std::env::temp_dir()) && root.join("Held saved file.txt").is_file());
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    let source = item.entry.as_ref().unwrap().path.join("b.txt");
    engine.recover_copy_with_check(&item, &root.join("Recovered tree"), &cancel, |_, _| Ok(()), |event| {
        if matches!(event, TransferEvent::Progress { ref path, bytes, .. } if path == &source && bytes > 64) {
            fs::write(root.join("copying-bytes"), b"ready").unwrap();
            loop { std::thread::sleep(std::time::Duration::from_secs(1)); }
        }
        TransferReply::Continue
    }).unwrap();
    panic!("parent must kill the partial directory copy");
}

#[test]
fn recovery_continuation_survives_expiry_triggered_by_its_own_completed_prefix() {
    let tree = Tree::new();
    let cancel = Cancellation::default();
    for ordinal in 0..99 {
        tree.engine
            .execute(
                OperationRequest::CreateFolder {
                    path: tree.root.path().join(format!("Other work {ordinal}")),
                },
                &cancel,
            )
            .unwrap();
    }
    assert!(tree.item().held_for_undo);
    let destination = tree.root.path().join("Recovered tree");
    let plan = tree.partial(&destination);
    assert!(!tree.item().held_for_undo);
    let outcome = tree
        .engine
        .resume_recovery_copy_with_check(&plan, &cancel, |_, _| Ok(()), |_| TransferReply::Continue)
        .unwrap();
    assert!(outcome.failures.is_empty() && !outcome.needs_attention);
    assert_eq!(
        fs::read(destination.join("nested/c.txt")).unwrap(),
        b"nested recovered work"
    );
    tree.engine.undo(&cancel).unwrap();
    assert!(destination.join("a.txt").exists() && !destination.join("b.txt").exists());
    tree.engine.undo(&cancel).unwrap();
    assert!(!destination.exists());
    tree.originals_intact();
}

#[test]
fn recovery_continuation_follows_an_expired_saved_record_after_container_rename_and_reopen() {
    let tree = Tree::new();
    let destination = tree.root.path().join("Recovered tree");
    let plan = tree.partial(&destination);
    let cancel = Cancellation::default();
    for ordinal in 0..105 {
        tree.engine
            .execute(
                OperationRequest::CreateFolder {
                    path: tree.root.path().join(format!("Other work {ordinal}")),
                },
                &cancel,
            )
            .unwrap();
    }
    assert!(!tree.item().held_for_undo);
    let moved = tree.root.path().join("Target moved");
    let target = tree.current.parent().unwrap();
    tree.engine
        .execute(
            OperationRequest::Rename {
                source: target.to_owned(),
                destination: moved.clone(),
            },
            &cancel,
        )
        .unwrap();
    let saved = moved.join(tree.saved.strip_prefix(target).unwrap());
    let current = moved.join(tree.current.file_name().unwrap());
    let Tree {
        engine, held, root, ..
    } = tree;
    drop(engine);
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert!(!item.held_for_undo);
    assert_eq!(item.entry.as_ref().unwrap().path, saved);
    let outcome = engine
        .resume_recovery_copy_with_check(&plan, &cancel, |_, _| Ok(()), |_| TransferReply::Continue)
        .unwrap();
    assert!(outcome.failures.is_empty() && !outcome.needs_attention);
    assert_eq!(
        fs::read(destination.join("nested/c.txt")).unwrap(),
        b"nested recovered work"
    );
    engine.undo(&cancel).unwrap();
    assert!(destination.join("a.txt").exists());
    assert!(!destination.join("b.txt").exists());
    assert_eq!(
        fs::read(saved.join("b.txt")).unwrap(),
        b"second recovered work"
    );
    assert_eq!(fs::read(&current).unwrap(), b"incoming work");
    assert_eq!(fs::read(&held).unwrap(), b"older saved work");
}

#[test]
fn missing_or_unreadable_recovery_output_pauses_without_consuming_its_exact_plan() {
    use std::os::unix::fs::PermissionsExt;
    for missing in [true, false] {
        let tree = Tree::new();
        let parent = tree.root.path().join("Output parent");
        fs::create_dir(&parent).unwrap();
        let destination = parent.join("Recovered tree");
        let plan = tree.partial(&destination);
        let previous = tree.engine.latest_undo_id();
        let held = tree.root.path().join("Held output");
        if missing {
            fs::rename(&destination, &held).unwrap();
        } else {
            fs::set_permissions(&parent, fs::Permissions::from_mode(0o0)).unwrap();
        }
        let result = tree.engine.resume_recovery_copy_with_check(
            &plan,
            &Cancellation::default(),
            |_, _| Ok(()),
            |_| TransferReply::Continue,
        );
        if !missing {
            fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let paused = result.unwrap();
        assert!(paused.needs_attention && paused.completed == 0 && paused.id.is_none());
        assert_eq!(tree.engine.latest_undo_id(), previous);
        if missing {
            assert!(!destination.exists());
            fs::rename(&held, &destination).unwrap();
        }
        let outcome = tree
            .engine
            .resume_recovery_copy_with_check(
                &paused.continuation.unwrap(),
                &Cancellation::default(),
                |_, _| Ok(()),
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(!outcome.needs_attention && outcome.failures.is_empty());
        assert_eq!(
            fs::read(destination.join("b.txt")).unwrap(),
            b"second recovered work"
        );
        tree.originals_intact();
    }
}

#[test]
fn queued_recovery_selection_survives_expiry_but_still_refuses_a_changed_source() {
    use std::os::unix::fs::PermissionsExt;
    for changed in [false, true] {
        let tree = Tree::new();
        let item = tree.item();
        let cancel = Cancellation::default();
        for ordinal in 0..105 {
            tree.engine
                .execute(
                    OperationRequest::CreateFolder {
                        path: tree.root.path().join(format!("Other work {ordinal}")),
                    },
                    &cancel,
                )
                .unwrap();
        }
        let archived = tree.item();
        assert!(!archived.held_for_undo);
        assert_eq!(archived.entry.as_ref().unwrap().path, tree.saved);
        let destination = tree.root.path().join("Recovered tree");
        if changed {
            fs::set_permissions(&tree.saved, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let result = tree.engine.recover_copy(&item, &destination, &cancel);
        if changed {
            let error = result.unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
            assert!(error.to_string().contains("refresh Recovery"));
            assert!(!destination.exists());
        } else {
            result.unwrap();
            assert_eq!(
                fs::read(destination.join("nested/c.txt")).unwrap(),
                b"nested recovered work"
            );
            tree.engine.undo(&cancel).unwrap();
            assert!(!destination.exists());
        }
        tree.originals_intact();
    }
}
