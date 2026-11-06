use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, UndoChoice, UndoSource},
    transfers::{TransferKind, TransferReply, TransferRequest},
};
use std::fs;

#[test]
fn a_killed_additional_pass_resumes_without_returning_completed_additions_twice() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let root = tempfile::Builder::new()
        .prefix("canopa-additional-resume-")
        .tempdir()
        .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::undo_directory_additions::crash_additional_merge_helper",
        ])
        .env("CANOPA_ADDITIONAL_RESUME_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let original = root.path().join("project");
    assert_eq!(
        fs::read(original.join("file-0000")).unwrap(),
        b"additional 0"
    );
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    let mut questions = 0;
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            questions += 1;
            assert_eq!(question.destination, original.join("file-0099"));
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(questions, 1);
    assert_eq!(result.remaining, 0);
    assert!(!root.path().join("moved").exists());
    for index in 0..99 {
        assert_eq!(
            fs::read(original.join(format!("file-{index:04}"))).unwrap(),
            format!("additional {index}").as_bytes()
        );
        assert!(!original.join(format!("file-{index:04} (2)")).exists());
    }
    assert_eq!(
        fs::read(original.join("file-0099")).unwrap(),
        b"newer final file"
    );
    assert_eq!(
        fs::read(original.join("file-0099 (2)")).unwrap(),
        b"additional 99"
    );
    assert!(engine.latest_undo_id().is_none());
}

#[test]
#[ignore = "child fixture; killed after an actual additional-item return"]
fn crash_additional_merge_helper() {
    use std::{os::unix::fs::MetadataExt, path::PathBuf, time::Duration};
    let root = PathBuf::from(
        std::env::var_os("CANOPA_ADDITIONAL_RESUME_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-additional-resume-")
    );
    let original = root.join("project");
    let current = root.join("moved");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("z.txt"), b"initial child").unwrap();
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
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
    fs::create_dir(&original).unwrap();
    fs::write(original.join("z.txt"), b"newer z").unwrap();
    fs::write(original.join("file-0099"), b"newer final file").unwrap();
    engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else if question.destination == original.join("z.txt") {
                for index in 0..100 {
                    fs::write(
                        current.join(format!("file-{index:04}")),
                        format!("additional {index}"),
                    )
                    .unwrap();
                }
                let first = fs::metadata(current.join("file-0000")).unwrap();
                let (device, inode) = (first.dev(), first.ino());
                let returned = original.join("file-0000");
                std::thread::spawn(move || {
                    while !fs::metadata(&returned)
                        .is_ok_and(|metadata| metadata.dev() == device && metadata.ino() == inode)
                    {
                        std::thread::sleep(Duration::from_micros(50));
                    }
                    rustix::process::kill_process(
                        rustix::process::getpid(),
                        rustix::process::Signal::KILL,
                    )
                    .unwrap();
                });
                UndoChoice::KeepBoth
            } else {
                // Prevent root completion even if the filesystem observer is delayed.
                loop {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        })
        .unwrap();
    panic!("the child was not killed during the additional pass");
}

#[test]
fn repeated_nested_passes_preserve_earlier_returned_recovery() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    let sources = root.path().join("sources");
    fs::create_dir_all(original.join("nested")).unwrap();
    fs::create_dir(&sources).unwrap();
    fs::write(sources.join("a.txt"), b"incoming a").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![sources.join("a.txt")],
                destination: original.join("nested"),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    fs::write(original.join("nested/a.txt"), b"protected original version").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    fs::write(original.join("nested/a.txt"), b"incoming a").unwrap();
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
    fs::write(original.join("nested/a.txt"), b"newer a").unwrap();
    let mut kept_nested = false;
    let first = engine
        .undo_interactive(None, &cancel, |question| {
            if question.source == UndoSource::MergeRemainder {
                if question.destination == original.join("nested") {
                    kept_nested = true;
                    UndoChoice::KeepItem
                } else {
                    UndoChoice::Skip
                }
            } else if question.can_merge() {
                UndoChoice::Merge
            } else {
                fs::write(
                    current.join("nested/a (2).txt"),
                    b"additional file using returned name",
                )
                .unwrap();
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert!(kept_nested);
    assert_eq!(first.remaining, 1);
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert_eq!(
        fs::read(original.join("nested/a (2).txt")).unwrap(),
        b"incoming a"
    );
    assert_eq!(
        fs::read(original.join("nested/a (2) (2).txt")).unwrap(),
        b"additional file using returned name"
    );
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].original,
        Some(original.join("nested/a (2).txt"))
    );
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"protected original version"
    );
}

#[test]
fn a_pending_nested_recovery_version_follows_its_merged_destination_container() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    let source = root.path().join("sources");
    fs::create_dir_all(original.join("nested")).unwrap();
    fs::create_dir(&source).unwrap();
    fs::write(source.join("a.txt"), b"incoming a").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let copy = || {
        let outcome = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![source.join("a.txt")],
                    destination: original.join("nested"),
                },
                &cancel,
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(outcome.failures.is_empty());
    };
    copy();
    fs::write(original.join("nested/a.txt"), b"protected nested version").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    copy();
    let page = engine.recovery_items(None, &cancel).unwrap();
    let stage_name = page.items[0]
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
    fs::create_dir_all(original.join("nested").join(&stage_name)).unwrap();
    fs::write(original.join("nested/a.txt"), b"newer nested a").unwrap();
    let partial = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert!(partial.remaining > 2);
    engine
        .execute(
            OperationRequest::Rename {
                source: original.join("nested"),
                destination: root.path().join("held nested"),
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir(original.join("nested")).unwrap();
    fs::write(original.join("nested/foreign.txt"), b"foreign nested data").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(
        page.items[0].original,
        Some(original.join("nested (2)/a.txt"))
    );
    let result = engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert_eq!(
        fs::read(original.join("nested/foreign.txt")).unwrap(),
        b"foreign nested data"
    );
    assert_eq!(
        fs::read(original.join("nested (2)/a (2).txt")).unwrap(),
        b"incoming a"
    );
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(
        page.items[0].original,
        Some(original.join("nested (2)/a (2).txt"))
    );
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"protected nested version"
    );
}

#[test]
fn pending_additions_and_recovery_follow_the_actual_merged_destination_folder() {
    use canopa_files::transfers::ConflictChoice;
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    let source = root.path().join("sources");
    fs::create_dir(&original).unwrap();
    fs::create_dir(&source).unwrap();
    fs::write(original.join("a.txt"), b"original a").unwrap();
    fs::write(original.join("z.txt"), b"original z").unwrap();
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
    fs::create_dir(&original).unwrap();
    fs::write(original.join("z.txt"), b"newer z").unwrap();
    engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                fs::write(current.join("a.txt"), b"additional a").unwrap();
                UndoChoice::Skip
            }
        })
        .unwrap();
    fs::write(source.join("a.txt"), b"temporary replacement").unwrap();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.join("a.txt")],
                destination: current.clone(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    fs::write(current.join("a.txt"), b"protected additional version").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::Replace)
        .unwrap();
    let partial = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else if question.destination == original.join("z.txt") {
                UndoChoice::KeepBoth
            } else {
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert_eq!(partial.remaining, 2);
    let held = root.path().join("held destination");
    engine
        .execute(
            OperationRequest::Rename {
                source: original.clone(),
                destination: held,
            },
            &cancel,
        )
        .unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("a.txt"), b"foreign new project").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    drop(engine);
    let relocated = root.path().join("project (2)");
    let engine = OperationEngine::open(&history).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].original, Some(relocated.join("a.txt")));
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.destination, relocated.join("a.txt"));
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    assert_eq!(
        fs::read(original.join("a.txt")).unwrap(),
        b"foreign new project"
    );
    assert_eq!(fs::read(relocated.join("a.txt")).unwrap(), b"original a");
    assert_eq!(
        fs::read(relocated.join("a (2).txt")).unwrap(),
        b"additional a"
    );
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items[0].original, Some(relocated.join("a (2).txt")));
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"protected additional version"
    );
}

#[test]
fn a_later_file_return_does_not_redirect_an_additional_items_destination_slot() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    let sources = root.path().join("sources");
    fs::create_dir(&original).unwrap();
    fs::create_dir(&sources).unwrap();
    let source = sources.join("a.txt");
    fs::write(&source, b"original copied data").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let copy = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination: original.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copy.failures.is_empty());
    fs::write(original.join("z.txt"), b"last original child").unwrap();
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
    let mut additional = false;
    let partial = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else if question.destination == original.join("z.txt") {
                fs::write(current.join("a.txt"), b"additional source data").unwrap();
                UndoChoice::KeepBoth
            } else {
                additional = true;
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert!(additional);
    assert_eq!(partial.remaining, 2);
    let held = root.path().join("held original file");
    engine
        .execute(
            OperationRequest::Rename {
                source: original.join("a.txt"),
                destination: held,
            },
            &cancel,
        )
        .unwrap();
    fs::write(original.join("a.txt"), b"newer occupant").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    let finished = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(
                question.destination,
                original.join("a.txt"),
                "an intended destination is not ownership of its previous occupant"
            );
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(finished.remaining, 0);
    assert_eq!(
        fs::read(original.join("a (2).txt")).unwrap(),
        b"original copied data"
    );
    assert_eq!(
        fs::read(original.join("a (3).txt")).unwrap(),
        b"additional source data"
    );
    engine.undo(&cancel).unwrap();
    assert!(!original.join("a (2).txt").exists());
    assert_eq!(fs::read(original.join("a.txt")).unwrap(), b"newer occupant");
    assert_eq!(
        fs::read(original.join("a (3).txt")).unwrap(),
        b"additional source data"
    );
}

#[test]
fn additional_passes_keep_previous_and_new_recovery_namespaces_separate() {
    use canopa_files::transfers::ConflictChoice;
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    let sources = root.path().join("sources");
    let extra = root.path().join("extra");
    for path in [&original, &sources, &extra] {
        fs::create_dir(path).unwrap();
    }
    let source = sources.join("a.txt");
    fs::write(&source, b"original source").unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let copy = |source: &std::path::Path, destination: &std::path::Path| {
        let result = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![source.to_owned()],
                    destination: destination.to_owned(),
                },
                &cancel,
                |_| TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
            )
            .unwrap();
        assert!(result.failures.is_empty());
    };
    copy(&source, &original);
    fs::write(original.join("a.txt"), b"protected original version").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
        .unwrap();
    copy(&source, &original);
    fs::write(original.join("z.txt"), b"last original child").unwrap();
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
                fs::write(current.join("a.txt"), b"additional file").unwrap();
                UndoChoice::Skip
            }
        })
        .unwrap();
    let extra_source = extra.join("a.txt");
    fs::write(&extra_source, b"temporary new copy").unwrap();
    copy(&extra_source, &current);
    fs::write(current.join("a.txt"), b"protected additional version").unwrap();
    engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::PreviousDestination);
            UndoChoice::Replace
        })
        .unwrap();
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            if question.can_merge() {
                UndoChoice::Merge
            } else {
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 2);
    for (index, item) in page.items.iter().enumerate() {
        let entry = item.entry.as_ref().unwrap();
        let data = fs::read(&entry.path).unwrap();
        let expected = if data == b"protected original version" {
            original.join("a.txt")
        } else {
            assert_eq!(data, b"protected additional version");
            original.join("a (2).txt")
        };
        assert_eq!(item.original.as_ref(), Some(&expected));
        let recovered = root.path().join(format!("recovered-{index}"));
        engine.recover_copy(item, &recovered, &cancel).unwrap();
        assert_eq!(fs::read(&recovered).unwrap(), data);
        engine.undo(&cancel).unwrap();
    }
    engine.undo(&cancel).unwrap();
    assert!(!original.join("a.txt").exists());
    assert_eq!(
        fs::read(original.join("a (2).txt")).unwrap(),
        b"additional file"
    );
}

#[test]
fn merging_additional_items_with_reused_names_preserves_history_of_completed_children() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("project");
    let current = root.path().join("moved");
    let sources = root.path().join("sources");
    fs::create_dir(&sources).unwrap();
    let source = sources.join("a.txt");
    fs::write(&source, b"original copied data").unwrap();
    fs::create_dir(&original).unwrap();
    let cancel = Cancellation::default();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
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
    fs::write(original.join("z.txt"), b"last original child").unwrap();
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
    let mut added = false;
    let mut rescans = 0;
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            if question.source == UndoSource::MergeRemainder {
                rescans += 1;
                assert!(question.can_merge());
                UndoChoice::Merge
            } else if question.can_merge() {
                UndoChoice::Merge
            } else {
                if !added && question.destination == original.join("z.txt") {
                    assert_eq!(
                        fs::read(original.join("a.txt")).unwrap(),
                        b"original copied data"
                    );
                    fs::write(current.join("a.txt"), b"additional source data").unwrap();
                    fs::write(current.join("later.txt"), b"additional disjoint data").unwrap();
                    added = true;
                }
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(rescans, 1);
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    assert_eq!(
        fs::read(original.join("a.txt")).unwrap(),
        b"original copied data"
    );
    assert_eq!(
        fs::read(original.join("a (2).txt")).unwrap(),
        b"additional source data"
    );
    engine.undo(&cancel).unwrap();
    assert!(!original.join("a.txt").exists());
    assert_eq!(
        fs::read(original.join("a (2).txt")).unwrap(),
        b"additional source data"
    );
    assert_eq!(
        fs::read(original.join("z.txt")).unwrap(),
        b"newer destination z"
    );
    assert_eq!(
        fs::read(original.join("later.txt")).unwrap(),
        b"additional disjoint data"
    );
    assert_eq!(fs::read(source).unwrap(), b"original copied data");
}
