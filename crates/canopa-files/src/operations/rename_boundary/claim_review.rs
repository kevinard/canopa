//! Review and retained ownership of ordinary Rename claims.
use super::*;

fn blocked_claim(root: &Path, kind: u8) -> (OperationEngine, RecoveryItem) {
    use std::os::unix::fs::PermissionsExt;
    let source = root.join("source");
    let target = root.join("renamed");
    match kind {
        0 => fs::write(&source, b"Claimed bytes").unwrap(),
        1 => {
            fs::create_dir(&source).unwrap();
            fs::write(source.join("child"), b"Claimed bytes").unwrap();
            fs::set_permissions(&source, fs::Permissions::from_mode(0o555)).unwrap();
        }
        _ => {
            fs::write(root.join("referent"), b"Claimed bytes").unwrap();
            std::os::unix::fs::symlink("referent", &source).unwrap();
        }
    }
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let source_at_gate = source.clone();
    let target_at_gate = target.clone();
    let _reset = after_claim(move |_, _| {
        fs::write(source_at_gate, b"Newer source").unwrap();
        fs::write(target_at_gate, b"Newer destination").unwrap();
    });
    assert!(
        engine
            .execute(
                OperationRequest::Rename {
                    source,
                    destination: target
                },
                &Cancellation::default()
            )
            .is_err()
    );
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert_eq!(page.interrupted.len(), 1);
    assert_eq!(page.items.len(), 1);
    (engine, page.items.into_iter().next().unwrap())
}

#[test]
fn keeping_a_reviewed_claim_preserves_its_role_across_restart_and_unblocks_work() {
    use std::os::unix::fs::PermissionsExt;
    for kind in 0..3 {
        let root = tempfile::tempdir().unwrap();
        let (engine, item) = blocked_claim(root.path(), kind);
        let payload = item.entry.as_ref().unwrap().path.clone();
        let identity = item.identity();
        let physical = Identity::read(&payload).unwrap();
        assert!(item.can_keep_in_recovery());
        engine
            .keep_in_recovery(&item, &Cancellation::default())
            .unwrap();
        assert!(
            engine
                .keep_in_recovery(&item, &Cancellation::default())
                .is_err()
        );
        assert_eq!(engine.latest_undo_id(), None);
        assert!(physical.matches(&payload));
        assert_eq!(
            fs::read(root.path().join("source")).unwrap(),
            b"Newer source"
        );
        assert_eq!(
            fs::read(root.path().join("renamed")).unwrap(),
            b"Newer destination"
        );
        drop(engine);
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let page = engine
            .recovery_items(None, &Cancellation::default())
            .unwrap();
        assert!(page.interrupted.is_empty());
        assert_eq!(page.items.len(), 1);
        let kept = &page.items[0];
        assert_eq!(kept.identity(), identity);
        assert!(!kept.can_keep_in_recovery());
        assert!(!kept.held_for_undo);
        assert_eq!(kept.entry.as_ref().unwrap().path, payload);
        let output = root.path().join("recovered");
        engine
            .recover_copy(kept, &output, &Cancellation::default())
            .unwrap();
        if kind == 1 {
            assert_eq!(fs::read(output.join("child")).unwrap(), b"Claimed bytes");
            assert_eq!(
                fs::metadata(&payload).unwrap().permissions().mode() & 0o777,
                0o555
            );
            assert!(
                engine
                    .recover_copy(kept, &payload.join("recursive"), &Cancellation::default())
                    .is_err()
            );
        } else if kind == 2 {
            assert_eq!(fs::read_link(&output).unwrap(), Path::new("referent"));
        } else {
            assert_eq!(fs::read(&output).unwrap(), b"Claimed bytes");
        }
        // Force normal retention/expiry through more than a full history window.
        for index in 0..=HISTORY_LIMIT {
            engine
                .execute(
                    OperationRequest::CreateFolder {
                        path: root.path().join(format!("later-{index}")),
                    },
                    &Cancellation::default(),
                )
                .unwrap();
        }
        assert!(physical.matches(&payload));
        assert_eq!(
            engine
                .recovery_items(None, &Cancellation::default())
                .unwrap()
                .items[0]
                .identity(),
            identity
        );
        if kind == 1 {
            fs::set_permissions(&payload, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
}

#[test]
fn claim_review_refuses_changed_or_serialized_selections_and_rolls_back_failed_owner_transfer() {
    let root = tempfile::tempdir().unwrap();
    let (engine, item) = blocked_claim(root.path(), 0);
    let payload = item.entry.as_ref().unwrap().path.clone();
    let restored: RecoveryItem =
        serde_json::from_slice(&serde_json::to_vec(&item).unwrap()).unwrap();
    assert!(!restored.can_keep_in_recovery());
    assert!(
        engine
            .keep_in_recovery(&restored, &Cancellation::default())
            .is_err()
    );
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert!(engine.keep_in_recovery(&item, &cancellation).is_err());
    fs::write(&payload, b"Edited since review").unwrap();
    assert!(
        engine
            .keep_in_recovery(&item, &Cancellation::default())
            .is_err()
    );
    let current = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .remove(0);
    let connection = rusqlite::Connection::open(root.path().join("history")).unwrap();
    let journal: String = connection
        .query_row("SELECT body FROM pending", [], |row| row.get(0))
        .unwrap();
    connection.execute_batch("CREATE TRIGGER refuse_claim_transfer BEFORE DELETE ON pending BEGIN SELECT RAISE(ABORT,'private claim transfer failure'); END;").unwrap();
    assert!(
        engine
            .keep_in_recovery(&current, &Cancellation::default())
            .unwrap_err()
            .to_string()
            .contains("private claim transfer failure")
    );
    assert_eq!(
        connection
            .query_row("SELECT body FROM pending", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        journal
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM garbage", [], |row| row
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
    assert_eq!(fs::read(&payload).unwrap(), b"Edited since review");
    // Same entry and version do not authorize a different journal owner.
    connection
        .execute_batch("DROP TRIGGER refuse_claim_transfer;")
        .unwrap();
    let changed = format!("{journal} ");
    connection
        .execute("UPDATE pending SET body=?1", [&changed])
        .unwrap();
    assert!(
        engine
            .keep_in_recovery(&current, &Cancellation::default())
            .is_err()
    );
    let refreshed = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .remove(0);
    engine
        .keep_in_recovery(&refreshed, &Cancellation::default())
        .unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM garbage", [], |row| row
                .get::<_, u64>(0))
            .unwrap(),
        1
    );
    assert_eq!(fs::read(&payload).unwrap(), b"Edited since review");
}

#[test]
fn kept_sibling_directory_sources_have_independent_retention_owners() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let (engine, item) = blocked_claim(root.path(), 1);
    engine
        .keep_in_recovery(&item, &Cancellation::default())
        .unwrap();
    let payload = item.entry.as_ref().unwrap().path.clone();
    fs::set_permissions(&payload, fs::Permissions::from_mode(0o755)).unwrap();
    let child = payload.join("child");
    let captured = engine
        .capture_retained_sources(std::slice::from_ref(&child))
        .unwrap();
    assert_eq!(captured.stages.len(), 1);
    let first = engine
        .queue_request_with_sources(&"first independent owner", &captured)
        .unwrap();
    let second = engine
        .queue_request_with_sources(&"second independent owner", &captured)
        .unwrap();
    let target = payload.join("changed");
    let rename = || {
        engine.execute(
            OperationRequest::Rename {
                source: child.clone(),
                destination: target.clone(),
            },
            &Cancellation::default(),
        )
    };
    assert!(rename().is_err());
    engine.dismiss_request(&first).unwrap();
    assert!(rename().is_err());
    assert_eq!(fs::read(&child).unwrap(), b"Claimed bytes");
    engine.dismiss_request(&second).unwrap();
    rename().unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"Claimed bytes");
}

#[test]
fn an_unexpected_sibling_symlink_is_retained_as_the_entry_not_its_referent() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("renamed");
    let parked = root.path().join("original-directory");
    let referent = root.path().join("referent");
    fs::create_dir(&source).unwrap();
    fs::write(&referent, b"Separate referent bytes").unwrap();
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let _reset = install(move |actual, _| {
        fs::rename(actual, parked).unwrap();
        std::os::unix::fs::symlink("referent", actual).unwrap();
        BEFORE_RENAME.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(|_, original| {
                fs::write(original, b"Newer occupant blocks return").unwrap();
            }));
        });
    });
    assert!(
        engine
            .execute(
                OperationRequest::Rename {
                    source: source.clone(),
                    destination: target
                },
                &Cancellation::default()
            )
            .is_err()
    );
    let item = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .remove(0);
    let payload = item.entry.as_ref().unwrap().path.clone();
    assert_eq!(fs::read_link(&payload).unwrap(), Path::new("referent"));
    engine
        .keep_in_recovery(&item, &Cancellation::default())
        .unwrap();
    let selected = engine
        .capture_retained_sources(std::slice::from_ref(&payload))
        .unwrap();
    if selected.stages.len() != 1 {
        panic!(
            "kept sibling link lost its retained source owner; fixture preserved at {}",
            root.keep().display()
        );
    }
    assert!(
        engine
            .capture_retained_sources(std::slice::from_ref(&referent))
            .unwrap()
            .stages
            .is_empty()
    );
    let owner = engine
        .queue_request_with_sources(&"selected link", &selected)
        .unwrap();
    let rename = || {
        engine.execute(
            OperationRequest::Rename {
                source: payload.clone(),
                destination: root.path().join("changed-link"),
            },
            &Cancellation::default(),
        )
    };
    let result = rename();
    if result.is_ok() {
        panic!(
            "independent work moved the selected sibling link; fixture preserved at {}",
            root.keep().display()
        );
    }
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
    fs::remove_file(&referent).unwrap();
    assert_eq!(
        engine
            .capture_retained_sources(std::slice::from_ref(&payload))
            .unwrap()
            .stages
            .len(),
        1
    );
    assert_eq!(rename().unwrap_err().kind(), io::ErrorKind::WouldBlock);
    fs::write(&referent, b"Separate referent bytes").unwrap();
    assert_eq!(fs::read_link(&payload).unwrap(), Path::new("referent"));
    assert_eq!(fs::read(&referent).unwrap(), b"Separate referent bytes");
    assert_eq!(fs::read(&source).unwrap(), b"Newer occupant blocks return");
    engine.dismiss_request(&owner).unwrap();
}

#[cfg(feature = "local-runtime")]
#[test]
fn application_keep_claim_runs_through_the_shared_queue_and_preserves_stopped_requests() {
    use crate::operation_jobs::{OperationJobs, Phase, Request};
    use std::time::{Duration, Instant};
    fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(value) = probe() {
                return value;
            }
            assert!(
                Instant::now() < deadline,
                "private claim queue did not settle"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let root = tempfile::tempdir().unwrap();
    let (engine, _) = blocked_claim(root.path(), 0);
    drop(engine);
    let jobs = OperationJobs::start(
        root.path().join("history"),
        root.path().join("volumes.json"),
    )
    .unwrap();
    wait(|| {
        let snapshot = jobs.snapshot();
        assert!(snapshot.error.is_none(), "{:?}", snapshot.error);
        (!snapshot.initializing).then_some(())
    });
    // A legitimate application request fails while the claim blocks ordinary
    // work. Keeping the claim must not consume this independent stopped row.
    let mut admit = jobs
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: root.path().join("must-wait"),
            }),
            None,
        )
        .unwrap();
    let stopped = wait(|| admit.try_recv().unwrap()).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|row| row.id == stopped && row.phase == Phase::Failed)
    });
    let connection = rusqlite::Connection::open(root.path().join("history")).unwrap();
    let previous: Vec<u8> = connection
        .query_row("SELECT body FROM requests WHERE id=?1", [stopped], |row| {
            row.get(0)
        })
        .unwrap();
    let mut page = jobs.recovery_page(None, Cancellation::default()).unwrap();
    let item = wait(|| page.try_recv().unwrap()).unwrap().items.remove(0);
    let payload = item.entry.as_ref().unwrap().path.clone();
    let serialized: RecoveryItem =
        serde_json::from_slice(&serde_json::to_vec(&item).unwrap()).unwrap();
    let mut admit = jobs
        .submit(
            Request::KeepRecovery {
                item: Box::new(serialized),
            },
            None,
        )
        .unwrap();
    let refused = wait(|| admit.try_recv().unwrap()).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|row| row.id == refused && row.phase == Phase::Failed && row.dismissible)
    });
    let mut dismissal = jobs.dismiss_request(refused).unwrap();
    assert!(wait(|| dismissal.try_recv().unwrap()).is_err());
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM pending", [], |row| row
                .get::<_, u64>(0))
            .unwrap(),
        1
    );
    connection.execute_batch("CREATE TRIGGER refuse_claim_result BEFORE UPDATE ON requests WHEN NEW.phase=5 BEGIN SELECT RAISE(ABORT,'private keep result-save failure'); END;").unwrap();
    let mut admit = jobs
        .submit(
            Request::KeepRecovery {
                item: Box::new(item),
            },
            None,
        )
        .unwrap();
    let id = wait(|| admit.try_recv().unwrap()).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|row| row.id == id && row.phase == Phase::SaveFailed)
    });
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM pending", [], |row| row
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM garbage", [], |row| row
                .get::<_, u64>(0))
            .unwrap(),
        1
    );
    connection
        .execute_batch("DROP TRIGGER refuse_claim_result")
        .unwrap();
    jobs.retry_save(id).unwrap();

    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|row| row.id == id && row.phase == Phase::Finished)
    });
    let mut dismissal = jobs.dismiss_request(refused).unwrap();
    wait(|| dismissal.try_recv().unwrap()).unwrap();
    let later = root.path().join("ordinary-work");
    let mut admit = jobs
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: later.clone(),
            }),
            None,
        )
        .unwrap();
    let id = wait(|| admit.try_recv().unwrap()).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|row| row.id == id && row.phase == Phase::Finished)
    });
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
    drop(jobs);
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    assert!(later.is_dir());
    assert_eq!(fs::read(&payload).unwrap(), b"Claimed bytes");
    let connection = rusqlite::Connection::open(root.path().join("history")).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT body FROM requests WHERE id=?1", [stopped], |row| {
                row.get::<_, Vec<u8>>(0)
            })
            .unwrap(),
        previous
    );
    assert_eq!(
        engine
            .recovery_items(None, &Cancellation::default())
            .unwrap()
            .items
            .len(),
        1
    );
}

#[test]
fn kept_directory_claim_receipt_cannot_move_away_from_its_sibling_payload() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let (engine, item) = blocked_claim(root.path(), 1);
    let payload = item.entry.as_ref().unwrap().path.clone();
    let receipt = match engine.state.lock().unwrap().pending().unwrap().unwrap() {
        Pending::Forward {
            stage: Some(stage), ..
        } => stage.directory,
        _ => panic!("expected private claim"),
    };
    engine
        .keep_in_recovery(&item, &Cancellation::default())
        .unwrap();
    let moved = root.path().join(".canopa-files-stage-moved-receipt");
    if engine
        .execute(
            OperationRequest::Rename {
                source: receipt.clone(),
                destination: moved.clone(),
            },
            &Cancellation::default(),
        )
        .is_ok()
    {
        panic!(
            "claim receipt moved without its readonly sibling; fixture preserved at {}",
            root.keep().display()
        );
    }
    assert!(receipt.is_dir());
    assert!(!moved.exists());
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert_eq!(page.items[0].entry.as_ref().unwrap().path, payload);
    assert_eq!(fs::read(payload.join("child")).unwrap(), b"Claimed bytes");
    fs::set_permissions(&payload, fs::Permissions::from_mode(0o755)).unwrap();
}
