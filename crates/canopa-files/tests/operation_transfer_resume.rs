use canopa_files::{
    file_clipboard::Selection,
    local_activity::LocalActivity,
    operation_jobs::{
        JobSnapshot, OperationJobs, Phase, Request, TransferPart, TransferResolution,
    },
    storage::Storage,
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::Path,
    time::{Duration, Instant},
};

fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(result) = probe() {
            return result;
        }
        assert!(Instant::now() < end, "transfer continuation did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn receive<T>(mut receiver: futures_channel::oneshot::Receiver<T>) -> T {
    wait(|| receiver.try_recv().expect("operation owner dropped reply"))
}
fn settled(jobs: &OperationJobs, id: u64) -> JobSnapshot {
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && !job.phase.holds_application())
    })
}
fn question(jobs: &OperationJobs, id: u64) -> canopa_files::operation_jobs::Decision {
    let diagnostic_deadline = Instant::now() + Duration::from_secs(14);
    wait(|| {
        assert!(
            Instant::now() < diagnostic_deadline,
            "Transfer question {id} did not arrive: {:?}",
            jobs.snapshot()
        );
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id)
            .and_then(|job| job.decision)
    })
}
fn choose(jobs: &OperationJobs, id: u64, choice: ConflictChoice) {
    let decision = question(jobs, id);
    jobs.resolve_transfer(
        id,
        decision.generation,
        TransferReply::Resolve {
            choice,
            apply_to_remaining: false,
        },
    )
    .unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .is_some_and(|job| {
                job.decision
                    .as_ref()
                    .is_none_or(|current| current.generation != decision.generation)
            })
            .then_some(())
    });
}
fn submit(
    jobs: &OperationJobs,
    kind: TransferKind,
    sources: Vec<std::path::PathBuf>,
    destination: &Path,
) -> u64 {
    receive(
        jobs.submit(
            Request::Transfer(TransferRequest {
                kind,
                sources,
                destination: destination.to_owned(),
            }),
            None,
        )
        .unwrap(),
    )
    .unwrap()
}
fn start(root: &Path) -> OperationJobs {
    let jobs = OperationJobs::start(root.join("history"), root.join("volumes")).unwrap();
    wait(|| (!jobs.snapshot().initializing).then_some(()));
    assert!(jobs.snapshot().error.is_none());
    jobs
}
fn undo(jobs: &OperationJobs) {
    let id = receive(
        jobs.submit(
            Request::Undo {
                expected: jobs.snapshot().undo.unwrap().id,
            },
            None,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(settled(jobs, id).phase, Phase::Finished);
}

#[test]
fn both_orders_resume_exact_directory_and_remaining_parts_with_independent_undo() {
    use rustix::fs::{CWD, Mode, mkfifoat};
    for kind in [TransferKind::Copy, TransferKind::Move] {
        for first in [TransferPart::Failed, TransferPart::Remaining] {
            let root = tempfile::tempdir().unwrap();
            let output = tempfile::Builder::new()
                .prefix("canopa-resume-")
                .tempdir_in("/dev/shm")
                .unwrap();
            let source = root.path().join("bundle");
            fs::create_dir(&source).unwrap();
            fs::write(source.join("a-done"), b"Confirmed prefix bytes").unwrap();
            mkfifoat(CWD, source.join("b-failed"), Mode::RUSR | Mode::WUSR).unwrap();
            let remaining = root.path().join("z-waiting");
            fs::write(&remaining, b"Remaining selected bytes").unwrap();
            fs::create_dir(output.path().join("bundle")).unwrap();
            fs::write(output.path().join("bundle/marker"), b"Existing folder").unwrap();
            fs::write(output.path().join("z-waiting"), b"Existing file").unwrap();
            let jobs = start(root.path());
            let id = submit(
                &jobs,
                kind,
                vec![source.clone(), remaining.clone()],
                output.path(),
            );
            choose(&jobs, id, ConflictChoice::KeepBoth);
            let decision = question(&jobs, id);
            assert!(output.path().join("bundle (2)/a-done").exists());
            jobs.resolve_transfer(id, decision.generation, TransferReply::Cancel)
                .unwrap();
            let stopped = settled(&jobs, id);
            assert!(
                stopped.transfer_actions.retry
                    && stopped.transfer_actions.remaining
                    && stopped.transfer_actions.abandon
            );
            let report = stopped.transfer.unwrap();
            assert_eq!(report.failures.len(), 1);
            let prefix = output.path().join("bundle (2)/a-done");
            let inode = fs::metadata(&prefix).unwrap().ino();
            fs::write(&prefix, b"Later edit to confirmed prefix").unwrap();
            fs::remove_file(source.join("b-failed")).unwrap();
            fs::write(source.join("b-failed"), b"Repaired child bytes").unwrap();
            let second = if first == TransferPart::Failed {
                TransferPart::Remaining
            } else {
                TransferPart::Failed
            };
            let mut successors = Vec::new();
            for part in [first, second] {
                let next = receive(jobs.resume_transfer(id, part).unwrap()).unwrap();
                successors.push(next);
                assert!(
                    receive(jobs.resume_transfer(id, part).unwrap()).is_err(),
                    "a double handoff was accepted"
                );
                if part == TransferPart::Remaining {
                    choose(&jobs, next, ConflictChoice::KeepBoth);
                }
                let done = settled(&jobs, next);
                assert_eq!(done.phase, Phase::Finished, "{kind:?}/{part:?}: {done:?}");
                assert_eq!(fs::metadata(&prefix).unwrap().ino(), inode);
                assert_eq!(
                    fs::read(&prefix).unwrap(),
                    b"Later edit to confirmed prefix"
                );
            }
            let old = settled(&jobs, id);
            assert_eq!(old.phase, Phase::Finished);
            let historical = old.transfer.unwrap();
            assert_eq!(historical.failures.len(), 1);
            assert_eq!(historical.remaining, report.remaining);
            assert_eq!(historical.completed, report.completed);
            assert_eq!(historical.failed_resolution, TransferResolution::HandedOff);
            assert_eq!(
                historical.remaining_resolution,
                TransferResolution::HandedOff
            );
            assert_eq!(
                fs::read(output.path().join("bundle (2)/b-failed")).unwrap(),
                b"Repaired child bytes"
            );
            assert!(!output.path().join("bundle (3)").exists());
            assert_eq!(
                fs::read(output.path().join("z-waiting (2)")).unwrap(),
                b"Remaining selected bytes"
            );
            assert_eq!(
                fs::read(output.path().join("bundle/marker")).unwrap(),
                b"Existing folder"
            );
            assert_eq!(
                fs::read(output.path().join("z-waiting")).unwrap(),
                b"Existing file"
            );
            // Reverse each successor independently; the edited prefix belongs
            // only to the original entry and must not be touched by either.
            undo(&jobs);
            assert_eq!(
                fs::read(&prefix).unwrap(),
                b"Later edit to confirmed prefix"
            );
            undo(&jobs);
            assert_eq!(
                fs::read(&prefix).unwrap(),
                b"Later edit to confirmed prefix"
            );
            assert!(!output.path().join("bundle (2)/b-failed").exists());
            assert!(!output.path().join("z-waiting (2)").exists());
            assert_eq!(fs::read(&remaining).unwrap(), b"Remaining selected bytes");
            assert_eq!(
                fs::read(source.join("b-failed")).unwrap(),
                b"Repaired child bytes"
            );
            assert!(!jobs.snapshot().holds_application());
            receive(jobs.shutdown().unwrap()).unwrap();
        }
    }
}

#[test]
fn queued_cancellation_preserves_original_source_and_destination_until_explicit_release() {
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        let blocker = root.path().join("blocker");
        fs::write(&blocker, b"Blocker").unwrap();
        fs::write(destination.join("blocker"), b"Existing blocker").unwrap();
        let selected = root.path().join("selected");
        fs::write(&selected, b"Original selected bytes").unwrap();
        let activity = LocalActivity::default();
        let storage = Storage::load(root.path().join("volumes")).unwrap();
        let jobs = OperationJobs::start_with_activity(
            root.path().join("history"),
            storage,
            activity.clone(),
        )
        .unwrap();
        wait(|| (!jobs.snapshot().initializing).then_some(()));
        let blocking = submit(&jobs, TransferKind::Copy, vec![blocker], &destination);
        question(&jobs, blocking);
        let id = submit(&jobs, kind, vec![selected.clone()], &destination);
        jobs.cancel(id).unwrap();
        let stopped = settled(&jobs, id);
        assert_eq!(stopped.transfer.unwrap().remaining, vec![selected.clone()]);
        assert!(stopped.transfer_actions.remaining);
        let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
        jobs.cancel(next).unwrap();
        assert!(settled(&jobs, next).transfer_actions.remaining);
        let parked = root.path().join("original selected");
        fs::rename(&selected, &parked).unwrap();
        fs::write(&selected, b"Unrelated replacement bytes").unwrap();
        let successor =
            receive(jobs.resume_transfer(next, TransferPart::Remaining).unwrap()).unwrap();
        choose(&jobs, blocking, ConflictChoice::Skip);
        assert_eq!(settled(&jobs, blocking).phase, Phase::Finished);
        let refused = settled(&jobs, successor);
        assert!(refused.transfer_actions.retry);
        assert_eq!(refused.transfer.unwrap().completed, 0);
        assert!(!destination.join("selected").exists());
        assert_eq!(fs::read(&selected).unwrap(), b"Unrelated replacement bytes");
        assert_eq!(fs::read(&parked).unwrap(), b"Original selected bytes");
        assert!(activity.holds_application());
        assert!(jobs.snapshot().holds_application());
        assert!(receive(jobs.shutdown().unwrap()).is_err());
        assert!(activity.try_remove().is_err());
        receive(jobs.abandon_transfer(successor).unwrap()).unwrap();
        let released = settled(&jobs, successor);
        assert_eq!(released.phase, Phase::Finished);
        assert_eq!(
            released.transfer.unwrap().failed_resolution,
            TransferResolution::Abandoned
        );
        wait(|| (!activity.holds_application()).then_some(()));
        assert!(!jobs.snapshot().holds_application());
        assert!(
            receive(
                jobs.resume_transfer(successor, TransferPart::Failed)
                    .unwrap()
            )
            .is_err()
        );
        assert!(!destination.join("selected").exists());
        receive(jobs.shutdown().unwrap()).unwrap();
    }
}

#[test]
fn failed_handoff_and_abandonment_transactions_preserve_the_live_owner() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let source = root.path().join("selected");
    fs::write(&source, b"Selected bytes").unwrap();
    fs::write(destination.join("selected"), b"Existing bytes").unwrap();
    let jobs = start(root.path());
    let id = submit(
        &jobs,
        TransferKind::Copy,
        vec![source.clone()],
        &destination,
    );
    let decision = question(&jobs, id);
    jobs.resolve_transfer(id, decision.generation, TransferReply::Cancel)
        .unwrap();
    assert!(settled(&jobs, id).transfer_actions.remaining);
    let database = rusqlite::Connection::open(root.path().join("history")).unwrap();
    database.execute_batch("CREATE TRIGGER private_handoff_failure BEFORE INSERT ON requests BEGIN SELECT RAISE(FAIL, 'private handoff refusal'); END;").unwrap();
    assert!(
        receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap())
            .unwrap_err()
            .contains("private handoff refusal")
    );
    assert!(settled(&jobs, id).transfer_actions.remaining);
    database.execute_batch("DROP TRIGGER private_handoff_failure; CREATE TRIGGER private_abandon_failure BEFORE UPDATE ON requests BEGIN SELECT RAISE(FAIL, 'private abandonment refusal'); END;").unwrap();
    assert!(receive(jobs.abandon_transfer(id).unwrap()).is_err());
    assert!(settled(&jobs, id).transfer_actions.remaining);
    assert!(jobs.snapshot().holds_application());
    database
        .execute_batch("DROP TRIGGER private_abandon_failure;")
        .unwrap();
    let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
    choose(&jobs, next, ConflictChoice::KeepBoth);
    assert_eq!(settled(&jobs, next).phase, Phase::Finished);
    assert_eq!(
        fs::read(destination.join("selected (2)")).unwrap(),
        b"Selected bytes"
    );
    assert_eq!(
        fs::read(destination.join("selected")).unwrap(),
        b"Existing bytes"
    );
    assert_eq!(fs::read(&source).unwrap(), b"Selected bytes");
}

#[test]
fn cut_continuation_preserves_untouched_parts_skips_and_a_newer_paste_revision() {
    for newer in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        let other = root.path().join("other");
        fs::create_dir(&destination).unwrap();
        fs::create_dir(&other).unwrap();
        let sources: Vec<_> = ["a-done", "b-skipped", "c-waiting", "d-later"]
            .into_iter()
            .map(|name| root.path().join(name))
            .collect();
        for path in &sources {
            fs::write(path, b"Selected bytes").unwrap();
        }
        for name in ["b-skipped", "c-waiting"] {
            fs::write(destination.join(name), b"Existing bytes").unwrap();
        }
        let storage = Storage::load(root.path().join("volumes")).unwrap();
        let jobs = OperationJobs::start_with_storage(root.path().join("history"), storage.clone())
            .unwrap();
        wait(|| (!jobs.snapshot().initializing).then_some(()));
        let prepared = receive(
            jobs.prepare_clipboard(
                Selection::new(sources.clone(), TransferKind::Move).unwrap(),
                storage.snapshot(),
            )
            .unwrap(),
        )
        .unwrap();
        let id = receive(
            jobs.paste_clipboard(prepared.clone(), destination.clone(), None)
                .unwrap(),
        )
        .unwrap();
        choose(&jobs, id, ConflictChoice::Skip);
        let decision = question(&jobs, id);
        jobs.resolve_transfer(id, decision.generation, TransferReply::Cancel)
            .unwrap();
        assert!(settled(&jobs, id).transfer_actions.remaining);
        assert_eq!(
            prepared.snapshot().selection.unwrap().paths(),
            &sources[1..]
        );
        let mut newer_paths = None;
        if newer {
            // A newer paste of this same opaque selection increments its revision.
            // It is cancelled before doing work, and retains its entire intent.
            fs::write(other.join("b-skipped"), b"Other existing bytes").unwrap();
            let newer_id = receive(
                jobs.paste_clipboard(prepared.clone(), other.clone(), None)
                    .unwrap(),
            )
            .unwrap();
            let decision = question(&jobs, newer_id);
            jobs.resolve_transfer(newer_id, decision.generation, TransferReply::Cancel)
                .unwrap();
            settled(&jobs, newer_id);
            newer_paths = Some(prepared.snapshot().selection.unwrap().paths().to_vec());
        }
        let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
        choose(&jobs, next, ConflictChoice::KeepBoth);
        assert_eq!(settled(&jobs, next).phase, Phase::Finished);
        let expected = newer_paths.unwrap_or_else(|| vec![sources[1].clone()]);
        assert_eq!(prepared.snapshot().selection.unwrap().paths(), expected);
        assert_eq!(fs::read(&sources[1]).unwrap(), b"Selected bytes");
        assert_eq!(
            fs::read(destination.join("c-waiting (2)")).unwrap(),
            b"Selected bytes"
        );
        assert_eq!(
            fs::read(destination.join("d-later")).unwrap(),
            b"Selected bytes"
        );
    }
}

#[test]
fn result_and_start_save_failures_keep_the_exact_plan_and_never_replay_the_prefix() {
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        let first = root.path().join("a-completed");
        let pending = root.path().join("z-waiting");
        fs::write(&first, b"Completed bytes").unwrap();
        fs::write(&pending, b"Pending bytes").unwrap();
        fs::write(destination.join("z-waiting"), b"Existing bytes").unwrap();
        let jobs = start(root.path());
        let id = submit(
            &jobs,
            kind,
            vec![first.clone(), pending.clone()],
            &destination,
        );
        let decision = question(&jobs, id);
        let prefix = destination.join("a-completed");
        let inode = fs::metadata(&prefix).unwrap().ino();
        let database = rusqlite::Connection::open(root.path().join("history")).unwrap();
        database.execute_batch(&format!("CREATE TRIGGER private_result_failure BEFORE UPDATE ON requests WHEN NEW.id={id} AND NEW.phase<>1 BEGIN SELECT RAISE(FAIL, 'private result refusal'); END;")).unwrap();
        jobs.resolve_transfer(id, decision.generation, TransferReply::Cancel)
            .unwrap();
        let unsaved = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|job| job.id == id && job.phase == Phase::SaveFailed)
        });
        assert_eq!(unsaved.transfer.unwrap().completed, 1);
        assert!(!unsaved.transfer_actions.remaining && !unsaved.transfer_actions.abandon);
        assert!(receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).is_err());
        assert!(jobs.snapshot().holds_application());
        let marker = root.path().join("queued marker");
        let later = receive(
            jobs.submit(
                Request::Mutation(canopa_files::operations::OperationRequest::CreateFolder {
                    path: marker.clone(),
                }),
                None,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(
            !marker.exists(),
            "unsaved outcome allowed conflicting dispatch"
        );
        fs::write(&prefix, b"Later prefix edit").unwrap();
        database
            .execute_batch("DROP TRIGGER private_result_failure;")
            .unwrap();
        jobs.retry_save(id).unwrap();
        assert!(settled(&jobs, id).transfer_actions.remaining);
        assert_eq!(settled(&jobs, later).phase, Phase::Finished);
        database.execute_batch("CREATE TRIGGER private_start_failure BEFORE UPDATE ON requests WHEN OLD.phase=0 AND NEW.phase=1 BEGIN SELECT RAISE(FAIL, 'private start refusal'); END;").unwrap();
        let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
        wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|job| job.id == next && job.phase == Phase::StartSaveFailed)
        });
        assert_eq!(fs::read(&pending).unwrap(), b"Pending bytes");
        assert!(jobs.snapshot().holds_application());
        database
            .execute_batch("DROP TRIGGER private_start_failure;")
            .unwrap();
        jobs.retry_save(next).unwrap();
        choose(&jobs, next, ConflictChoice::KeepBoth);
        assert_eq!(settled(&jobs, next).phase, Phase::Finished);
        assert_eq!(fs::metadata(&prefix).unwrap().ino(), inode);
        assert_eq!(fs::read(&prefix).unwrap(), b"Later prefix edit");
        assert_eq!(
            fs::read(destination.join("z-waiting (2)")).unwrap(),
            b"Pending bytes"
        );
        assert_eq!(
            fs::read(destination.join("z-waiting")).unwrap(),
            b"Existing bytes"
        );
        assert_eq!(pending.exists(), kind == TransferKind::Copy);
    }
}

#[test]
fn resumed_application_work_refuses_retargeted_roots_and_accepts_the_actual_original() {
    use canopa_files::operation_jobs::{Question, TransferQuestion};
    use std::os::unix::fs::symlink;
    for kind in [TransferKind::Copy, TransferKind::Move] {
        for alias in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let original = root.path().join("original");
            let destination = root.path().join("destination");
            let replacement = root.path().join("replacement");
            fs::create_dir(&replacement).unwrap();
            if alias {
                fs::create_dir(&original).unwrap();
                symlink(&original, &destination).unwrap();
            } else {
                fs::create_dir(&destination).unwrap();
            }
            let source = root.path().join("selected");
            fs::write(&source, b"Selected bytes").unwrap();
            fs::write(destination.join("selected"), b"Existing bytes").unwrap();
            let jobs = start(root.path());
            let id = submit(&jobs, kind, vec![source.clone()], &destination);
            let decision = question(&jobs, id);
            jobs.resolve_transfer(id, decision.generation, TransferReply::Cancel)
                .unwrap();
            assert!(settled(&jobs, id).transfer_actions.remaining);
            if alias {
                fs::remove_file(&destination).unwrap();
                symlink(&replacement, &destination).unwrap();
            } else {
                fs::rename(&destination, &original).unwrap();
                fs::rename(&replacement, &destination).unwrap();
            }
            fs::write(destination.join("marker"), b"Unrelated folder").unwrap();
            let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
            let decision = question(&jobs, next);
            assert!(matches!(
                decision.question,
                Question::Transfer(TransferQuestion::Unavailable { .. })
            ));
            assert!(!destination.join("selected").exists());
            assert_eq!(
                fs::read(destination.join("marker")).unwrap(),
                b"Unrelated folder"
            );
            if alias {
                fs::remove_file(&destination).unwrap();
                symlink(&original, &destination).unwrap();
            } else {
                fs::rename(&destination, &replacement).unwrap();
                fs::rename(&original, &destination).unwrap();
            }
            jobs.resolve_transfer(next, decision.generation, TransferReply::Retry)
                .unwrap();
            let conflict = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|job| job.id == next)
                    .and_then(|job| job.decision)
                    .filter(|current| current.generation != decision.generation)
            });
            jobs.resolve_transfer(
                next,
                conflict.generation,
                TransferReply::Resolve {
                    choice: ConflictChoice::KeepBoth,
                    apply_to_remaining: false,
                },
            )
            .unwrap();
            assert_eq!(settled(&jobs, next).phase, Phase::Finished);
            assert_eq!(
                fs::read(destination.join("selected (2)")).unwrap(),
                b"Selected bytes"
            );
            assert_eq!(
                fs::read(destination.join("selected")).unwrap(),
                b"Existing bytes"
            );
            assert_eq!(
                fs::read(replacement.join("marker")).unwrap(),
                b"Unrelated folder"
            );
        }
    }
}

#[test]
fn cut_parts_sharing_a_folder_release_it_only_after_both_parts_finish() {
    use rustix::fs::{CWD, Mode, mkfifoat};
    for first in [TransferPart::Failed, TransferPart::Remaining] {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::Builder::new()
            .prefix("canopa-cut-parts-")
            .tempdir_in("/dev/shm")
            .unwrap();
        let source = root.path().join("bundle");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("a-done"), b"Prefix").unwrap();
        mkfifoat(CWD, source.join("b-failed"), Mode::RUSR | Mode::WUSR).unwrap();
        fs::write(source.join("c-waiting"), b"Pending bytes").unwrap();
        fs::create_dir(output.path().join("bundle")).unwrap();
        fs::write(output.path().join("bundle/c-waiting"), b"Existing bytes").unwrap();
        let storage = Storage::load(root.path().join("volumes")).unwrap();
        let jobs = OperationJobs::start_with_storage(root.path().join("history"), storage.clone())
            .unwrap();
        wait(|| (!jobs.snapshot().initializing).then_some(()));
        let prepared = receive(
            jobs.prepare_clipboard(
                Selection::new(vec![source.clone()], TransferKind::Move).unwrap(),
                storage.snapshot(),
            )
            .unwrap(),
        )
        .unwrap();
        let id = receive(
            jobs.paste_clipboard(prepared.clone(), output.path().to_owned(), None)
                .unwrap(),
        )
        .unwrap();
        choose(&jobs, id, ConflictChoice::Merge);
        let decision = question(&jobs, id);
        jobs.resolve_transfer(id, decision.generation, TransferReply::Cancel)
            .unwrap();
        let stopped = settled(&jobs, id);
        assert!(stopped.transfer_actions.retry && stopped.transfer_actions.remaining);
        fs::remove_file(source.join("b-failed")).unwrap();
        fs::write(source.join("b-failed"), b"Repaired child").unwrap();
        let second = if first == TransferPart::Failed {
            TransferPart::Remaining
        } else {
            TransferPart::Failed
        };
        for (index, part) in [first, second].into_iter().enumerate() {
            let next = receive(jobs.resume_transfer(id, part).unwrap()).unwrap();
            if part == TransferPart::Remaining {
                choose(&jobs, next, ConflictChoice::KeepBoth);
            }
            assert_eq!(settled(&jobs, next).phase, Phase::Finished);
            if index == 0 {
                assert_eq!(
                    prepared.snapshot().selection.unwrap().paths(),
                    std::slice::from_ref(&source)
                );
            }
        }
        assert!(!source.exists());
        assert!(
            prepared.snapshot().selection.is_none(),
            "completed sibling work left a transient source remainder in Cut"
        );
        assert_eq!(
            fs::read(output.path().join("bundle/c-waiting")).unwrap(),
            b"Existing bytes"
        );
        assert_eq!(
            fs::read(output.path().join("bundle/c-waiting (2)")).unwrap(),
            b"Pending bytes"
        );
        assert_eq!(
            fs::read(output.path().join("bundle/b-failed")).unwrap(),
            b"Repaired child"
        );
    }
}

#[test]
fn retrying_a_file_read_failure_uses_the_same_selected_object_after_permission_repair() {
    use std::os::unix::fs::PermissionsExt;
    if rustix::process::geteuid().is_root() {
        eprintln!("permission refusal requires an ordinary user");
        return;
    }
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::Builder::new()
            .prefix("canopa-file-retry-")
            .tempdir_in("/dev/shm")
            .unwrap();
        let source = root.path().join("selected");
        fs::write(&source, b"Original selected file bytes").unwrap();
        let inode = fs::metadata(&source).unwrap().ino();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o000)).unwrap();
        let jobs = start(root.path());
        let id = submit(&jobs, kind, vec![source.clone()], output.path());
        let stopped = settled(&jobs, id);
        assert!(stopped.transfer_actions.retry && !stopped.transfer_actions.remaining);
        assert_eq!(stopped.transfer.unwrap().failures.len(), 1);
        assert!(!output.path().join("selected").exists());
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(fs::metadata(&source).unwrap().ino(), inode);
        let next = receive(jobs.resume_transfer(id, TransferPart::Failed).unwrap()).unwrap();
        assert_eq!(settled(&jobs, next).phase, Phase::Finished);
        assert_eq!(
            fs::read(output.path().join("selected")).unwrap(),
            b"Original selected file bytes"
        );
        undo(&jobs);
        assert!(!output.path().join("selected").exists());
        assert_eq!(fs::read(&source).unwrap(), b"Original selected file bytes");
    }
}

#[test]
fn queued_handoff_waits_for_review_while_unselected_stopped_parts_regain_owners() {
    use rustix::fs::{CWD, Mode, mkfifoat};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("bundle");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::create_dir(destination.join("bundle")).unwrap();
    fs::write(source.join("a-done"), b"Completed prefix").unwrap();
    mkfifoat(CWD, source.join("b-failed"), Mode::RUSR | Mode::WUSR).unwrap();
    let later = root.path().join("z-later");
    fs::write(&later, b"Unstarted later bytes").unwrap();
    fs::write(destination.join("z-later"), b"Existing later bytes").unwrap();
    let blocker = root.path().join("blocker");
    fs::write(&blocker, b"Blocker source").unwrap();
    fs::write(destination.join("blocker"), b"Existing blocker").unwrap();
    let jobs = start(root.path());
    let id = submit(
        &jobs,
        TransferKind::Copy,
        vec![source.clone(), later],
        &destination,
    );
    choose(&jobs, id, ConflictChoice::KeepBoth);
    let decision = question(&jobs, id);
    jobs.resolve_transfer(id, decision.generation, TransferReply::Cancel)
        .unwrap();
    assert!(settled(&jobs, id).transfer_actions.retry);
    let database = rusqlite::Connection::open(root.path().join("history")).unwrap();
    let payload = |id| -> serde_json::Value {
        let body: Vec<u8> = database
            .query_row("SELECT body FROM requests WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    };
    let exact = payload(id)["TransferPending"]["outcome"]["retry"].clone();
    assert!(exact.is_object());
    let blocking = submit(&jobs, TransferKind::Copy, vec![blocker], &destination);
    question(&jobs, blocking);
    let successor = receive(jobs.resume_transfer(id, TransferPart::Failed).unwrap()).unwrap();
    assert_eq!(
        payload(successor)["Plan"].get("transfer_checkpoint"),
        Some(&exact),
        "the handoff discarded its exact Keep both/traversal/cleanup checkpoint"
    );
    assert!(!destination.join("bundle (2)/b-failed").exists());
    drop(database);
    drop(jobs);
    // Deliberate in-process owner loss, with actual lock retirement. This is
    // descriptive restoration evidence, not filesystem-step crash acceptance.
    let engine = wait(|| {
        match canopa_files::operations::OperationEngine::open(root.path().join("history")) {
            Ok(engine) => Some(engine),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(error) => panic!("history teardown failed: {error}"),
        }
    });
    drop(engine);
    let reopened = start(root.path());
    let queued = reopened
        .snapshot()
        .jobs
        .into_iter()
        .find(|job| job.id == successor)
        .unwrap();
    assert_eq!(queued.phase, Phase::AwaitingReview);
    assert!(!queued.transfer_actions.retry && !queued.transfer_actions.remaining);
    assert!(
        receive(
            reopened
                .resume_transfer(successor, TransferPart::Remaining)
                .unwrap()
        )
        .is_err()
    );
    assert_eq!(
        fs::read(destination.join("bundle (2)/a-done")).unwrap(),
        b"Completed prefix"
    );
    assert!(!destination.join("bundle (2)/b-failed").exists());
    assert_eq!(
        fs::read(destination.join("z-later")).unwrap(),
        b"Existing later bytes"
    );
    // The unselected, independently sealed part now regains its own owner.
    assert!(settled(&reopened, id).transfer_actions.remaining);
    for row in reopened.snapshot().jobs {
        if row.transfer_actions.abandon {
            receive(reopened.abandon_transfer(row.id).unwrap()).unwrap();
        }
    }
    receive(reopened.shutdown().unwrap()).unwrap();
}

#[test]
fn a_killed_stopped_batch_restores_both_exact_parts_and_keeps_completed_work() {
    use std::os::unix::process::ExitStatusExt;
    for kind in [TransferKind::Copy, TransferKind::Move] {
        for first in [TransferPart::Failed, TransferPart::Remaining] {
            let root = tempfile::Builder::new()
                .prefix("canopa-transfer-restart-")
                .tempdir()
                .unwrap();
            let output = tempfile::Builder::new()
                .prefix(".canopa-transfer-restart-output-")
                .tempdir_in(std::env::current_dir().unwrap())
                .unwrap();
            if kind == TransferKind::Move
                && fs::metadata(root.path()).unwrap().dev()
                    == fs::metadata(output.path()).unwrap().dev()
            {
                continue; // The real cross-filesystem Move must copy the FIFO and fail it.
            }
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "cases::operation_transfer_resume::restart_paused_transfer_helper",
                ])
                .env("CANOPA_TRANSFER_RESTART_ROOT", root.path())
                .env("CANOPA_TRANSFER_RESTART_OUTPUT", output.path())
                .env(
                    "CANOPA_TRANSFER_RESTART_KIND",
                    if kind == TransferKind::Copy {
                        "Copy"
                    } else {
                        "Move"
                    },
                )
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let marker: serde_json::Value =
                serde_json::from_slice(&fs::read(root.path().join("restart-marker.json")).unwrap())
                    .unwrap();
            let id = marker["request"].as_u64().unwrap();
            let prefix = output.path().join("bundle (2)/a-done");
            let before = fs::metadata(&prefix).unwrap();
            fs::write(&prefix, b"Newer completed work after death").unwrap();
            let failed = root.path().join("bundle/b-failed");
            fs::remove_file(&failed).unwrap();
            fs::write(&failed, b"Repaired unselected child").unwrap();
            let jobs = start(root.path());
            let restored = settled(&jobs, id);
            if !(restored.transfer_actions.retry && restored.transfer_actions.remaining) {
                let retained_root = root.keep();
                let retained_output = output.keep();
                panic!(
                    "restart lost exact Retry/Continue: {restored:?}; retained private data: {} and {}",
                    retained_root.display(),
                    retained_output.display()
                );
            }
            assert_eq!(
                restored.transfer.as_ref().unwrap().operation,
                marker["operation"].as_u64()
            );
            let second = if first == TransferPart::Failed {
                TransferPart::Remaining
            } else {
                TransferPart::Failed
            };
            for part in [first, second] {
                let next = receive(jobs.resume_transfer(id, part).unwrap()).unwrap();
                assert!(receive(jobs.resume_transfer(id, part).unwrap()).is_err());
                if part == TransferPart::Remaining {
                    choose(&jobs, next, ConflictChoice::KeepBoth);
                }
                let done = settled(&jobs, next);
                assert_eq!(done.phase, Phase::Finished, "{kind:?}/{part:?}: {done:?}");
                let after = fs::metadata(&prefix).unwrap();
                assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
                assert_eq!(
                    fs::read(&prefix).unwrap(),
                    b"Newer completed work after death"
                );
            }
            assert_eq!(settled(&jobs, id).phase, Phase::Finished);
            assert_eq!(
                fs::read(output.path().join("bundle (2)/b-failed")).unwrap(),
                b"Repaired unselected child"
            );
            assert!(!output.path().join("bundle (3)").exists());
            assert_eq!(
                fs::read(output.path().join("z-waiting (2)")).unwrap(),
                b"Remaining selected bytes"
            );
            undo(&jobs);
            undo(&jobs);
            assert_eq!(
                fs::read(&prefix).unwrap(),
                b"Newer completed work after death"
            );
            assert!(!output.path().join("bundle (2)/b-failed").exists());
            assert!(!output.path().join("z-waiting (2)").exists());
            receive(jobs.shutdown().unwrap()).unwrap();
        }
    }
}

fn killed_stopped_fixture(
    kind: TransferKind,
) -> Option<(tempfile::TempDir, tempfile::TempDir, u64)> {
    use std::os::unix::process::ExitStatusExt;
    let root = tempfile::Builder::new()
        .prefix("canopa-transfer-restart-")
        .tempdir()
        .unwrap();
    let output = tempfile::Builder::new()
        .prefix(".canopa-transfer-restart-output-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    if kind == TransferKind::Move
        && fs::metadata(root.path()).unwrap().dev() == fs::metadata(output.path()).unwrap().dev()
    {
        return None;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::operation_transfer_resume::restart_paused_transfer_helper",
        ])
        .env("CANOPA_TRANSFER_RESTART_ROOT", root.path())
        .env("CANOPA_TRANSFER_RESTART_OUTPUT", output.path())
        .env(
            "CANOPA_TRANSFER_RESTART_KIND",
            if kind == TransferKind::Copy {
                "Copy"
            } else {
                "Move"
            },
        )
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("restart-marker.json")).unwrap())
            .unwrap();
    Some((root, output, marker["request"].as_u64().unwrap()))
}

#[test]
fn restored_parts_survive_a_second_crash_without_replaying_the_first_handoff() {
    use std::os::unix::process::ExitStatusExt;
    for kind in [TransferKind::Copy, TransferKind::Move] {
        for first in [TransferPart::Failed, TransferPart::Remaining] {
            let Some((root, output, id)) = killed_stopped_fixture(kind) else {
                continue;
            };
            if kind == TransferKind::Move
                && fs::metadata(root.path()).unwrap().dev()
                    == fs::metadata(output.path()).unwrap().dev()
            {
                continue;
            }
            let failed = root.path().join("bundle/b-failed");
            fs::remove_file(&failed).unwrap();
            fs::write(&failed, b"Repaired child").unwrap();
            let prefix = output.path().join("bundle (2)/a-done");
            fs::write(&prefix, b"Edited completed prefix").unwrap();
            let before = fs::metadata(&prefix).unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "cases::operation_transfer_resume::restart_one_part_helper",
                ])
                .env("CANOPA_TRANSFER_RESTART_ROOT", root.path())
                .env(
                    "CANOPA_TRANSFER_RESTART_FIRST",
                    if first == TransferPart::Failed {
                        "Failed"
                    } else {
                        "Remaining"
                    },
                )
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let jobs = start(root.path());
            let restored = settled(&jobs, id);
            assert_eq!(
                restored.transfer_actions.retry,
                first != TransferPart::Failed,
                "{restored:?}"
            );
            assert_eq!(
                restored.transfer_actions.remaining,
                first != TransferPart::Remaining,
                "{restored:?}"
            );
            assert!(receive(jobs.resume_transfer(id, first).unwrap()).is_err());
            let second = if first == TransferPart::Failed {
                TransferPart::Remaining
            } else {
                TransferPart::Failed
            };
            let next = receive(jobs.resume_transfer(id, second).unwrap()).unwrap();
            if second == TransferPart::Remaining {
                choose(&jobs, next, ConflictChoice::KeepBoth);
            }
            assert_eq!(settled(&jobs, next).phase, Phase::Finished);
            assert_eq!(settled(&jobs, id).phase, Phase::Finished);
            let after = fs::metadata(&prefix).unwrap();
            assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
            assert_eq!(fs::read(&prefix).unwrap(), b"Edited completed prefix");
            assert_eq!(
                fs::read(output.path().join("bundle (2)/b-failed")).unwrap(),
                b"Repaired child"
            );
            undo(&jobs);
            undo(&jobs);
            assert_eq!(fs::read(&prefix).unwrap(), b"Edited completed prefix");
            assert!(!output.path().join("bundle (2)/b-failed").exists());
            assert!(!output.path().join("z-waiting (2)").exists());
            receive(jobs.shutdown().unwrap()).unwrap();
        }
    }
}

#[test]
#[ignore = "private process killed after consuming one restored part"]
fn restart_one_part_helper() {
    let root = std::path::PathBuf::from(std::env::var_os("CANOPA_TRANSFER_RESTART_ROOT").unwrap());
    assert_eq!(root.parent().unwrap(), std::env::temp_dir());
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-transfer-restart-")
    );
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("restart-marker.json")).unwrap()).unwrap();
    let id = marker["request"].as_u64().unwrap();
    let part = match std::env::var("CANOPA_TRANSFER_RESTART_FIRST")
        .unwrap()
        .as_str()
    {
        "Failed" => TransferPart::Failed,
        "Remaining" => TransferPart::Remaining,
        _ => panic!("invalid part"),
    };
    let jobs = start(&root);
    let next = receive(jobs.resume_transfer(id, part).unwrap()).unwrap();
    if part == TransferPart::Remaining {
        choose(&jobs, next, ConflictChoice::KeepBoth);
    }
    assert_eq!(settled(&jobs, next).phase, Phase::Finished);
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private child was not killed");
}

#[test]
fn restored_stopped_parts_hold_activity_until_explicit_abandonment() {
    let (root, _output, id) = killed_stopped_fixture(TransferKind::Copy).unwrap();
    let activity = LocalActivity::default();
    let jobs = OperationJobs::start_with_activity(
        root.path().join("history"),
        Storage::load(root.path().join("volumes")).unwrap(),
        activity.clone(),
    )
    .unwrap();
    wait(|| (!jobs.snapshot().initializing).then_some(()));
    assert!(settled(&jobs, id).transfer_actions.abandon);
    // Shutdown is unguarded. Its refusal proves the worker started and the
    // temporary startup hold retired, without adding another activity guard.
    assert!(receive(jobs.shutdown().unwrap()).is_err());
    assert!(activity.holds_application() && activity.try_remove().is_err());
    receive(jobs.abandon_transfer(id).unwrap()).unwrap();
    receive(jobs.shutdown().unwrap()).unwrap();
    assert!(!activity.holds_application());
    drop(activity.try_remove().unwrap());
}

#[test]
fn restarted_owners_refuse_replaced_objects_and_unsealed_or_changed_parts() {
    for changed in [
        "source",
        "source-parent",
        "destination",
        "traversed-destination",
        "plan",
        "no-seal",
        "revision",
        "history",
    ] {
        let (root, output, id) = killed_stopped_fixture(TransferKind::Copy).unwrap();
        let mut foreign = None;
        let mut parked_namespaces = Vec::new();
        match changed {
            "source" => {
                let path = root.path().join("z-waiting");
                fs::rename(&path, root.path().join("parked")).unwrap();
                fs::write(&path, b"Replacement selected bytes").unwrap();
                foreign = Some(path);
            }
            "source-parent" => {
                // Keep each exact entry but replace its containing directory.
                let owner = tempfile::tempdir().unwrap();
                let parked = owner.path().join("original");
                fs::rename(root.path(), &parked).unwrap();
                fs::create_dir(root.path()).unwrap();
                for entry in fs::read_dir(&parked).unwrap() {
                    let entry = entry.unwrap();
                    fs::rename(entry.path(), root.path().join(entry.file_name())).unwrap();
                }
                parked_namespaces.push(owner);
            }
            "destination" | "traversed-destination" => {
                let path = if changed == "destination" {
                    output.path().to_owned()
                } else {
                    output.path().join("bundle (2)")
                };
                let owner = tempfile::Builder::new()
                    .prefix(".canopa-transfer-restart-output-")
                    .tempdir_in(std::env::current_dir().unwrap())
                    .unwrap();
                fs::rename(&path, owner.path().join("original")).unwrap();
                fs::create_dir(&path).unwrap();
                fs::write(path.join("sentinel"), b"Replacement selected bytes").unwrap();
                foreign = Some(path.join("sentinel"));
                parked_namespaces.push(owner);
            }
            "history" => {
                let history = root.path().join("history");
                let copied = root.path().join("copied-history");
                let db = rusqlite::Connection::open(&history).unwrap();
                db.execute("VACUUM INTO ?1", [copied.to_str().unwrap()])
                    .unwrap();
                drop(db);
                fs::rename(&history, root.path().join("original-history")).unwrap();
                fs::rename(copied, &history).unwrap();
            }
            "plan" | "no-seal" | "revision" => {
                let db = rusqlite::Connection::open(root.path().join("history")).unwrap();
                if changed == "no-seal" {
                    db.execute("DELETE FROM request_transfer_stops WHERE request=?1", [id])
                        .unwrap();
                } else if changed == "revision" {
                    db.execute("UPDATE requests SET revision=revision+1 WHERE id=?1", [id])
                        .unwrap();
                } else {
                    let bytes: Vec<u8> = db
                        .query_row("SELECT body FROM requests WHERE id=?1", [id], |row| {
                            row.get(0)
                        })
                        .unwrap();
                    let mut body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    body["TransferPending"]["outcome"]["retry"]["pending"][0]["destination"] =
                        serde_json::json!(output.path().join("unapproved"));
                    db.execute(
                        "UPDATE requests SET body=?2 WHERE id=?1",
                        rusqlite::params![id, serde_json::to_vec(&body).unwrap()],
                    )
                    .unwrap();
                }
            }
            _ => unreachable!(),
        }
        let jobs = start(root.path());
        let row = settled(&jobs, id);
        if matches!(changed, "plan" | "no-seal" | "revision" | "history") {
            assert!(
                !row.transfer_actions.retry
                    && !row.transfer_actions.remaining
                    && !row.transfer_actions.abandon,
                "{changed}: {row:?}"
            );
        } else {
            assert!(row.transfer_actions.abandon, "{changed}: {row:?}");
            assert_eq!(
                row.transfer_actions.retry,
                changed == "source",
                "{changed}: {row:?}"
            );
            assert_eq!(
                row.transfer_actions.remaining,
                changed == "traversed-destination",
                "{changed}: {row:?}"
            );
            receive(jobs.abandon_transfer(id).unwrap()).unwrap();
        }
        if let Some(path) = foreign {
            assert_eq!(fs::read(path).unwrap(), b"Replacement selected bytes");
        }
        assert!(!output.path().join("unapproved").exists());
        receive(jobs.shutdown().unwrap()).unwrap();
    }
}

#[test]
#[ignore = "private process killed after an exact failed/remaining result is saved"]
fn restart_paused_transfer_helper() {
    let root = std::path::PathBuf::from(std::env::var_os("CANOPA_TRANSFER_RESTART_ROOT").unwrap());
    let output =
        std::path::PathBuf::from(std::env::var_os("CANOPA_TRANSFER_RESTART_OUTPUT").unwrap());
    assert_eq!(root.parent().unwrap(), std::env::temp_dir());
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-transfer-restart-")
    );
    assert_eq!(output.parent().unwrap(), std::env::current_dir().unwrap());
    assert!(
        output
            .file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b".canopa-transfer-restart-output-")
    );
    let kind = match std::env::var("CANOPA_TRANSFER_RESTART_KIND")
        .unwrap()
        .as_str()
    {
        "Copy" => TransferKind::Copy,
        "Move" => TransferKind::Move,
        _ => panic!("invalid fixture kind"),
    };
    let source = root.join("bundle");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("a-done"), b"Confirmed prefix bytes").unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        source.join("b-failed"),
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let remaining = root.join("z-waiting");
    fs::write(&remaining, b"Remaining selected bytes").unwrap();
    fs::create_dir(output.join("bundle")).unwrap();
    fs::write(output.join("bundle/marker"), b"Existing folder").unwrap();
    fs::write(output.join("z-waiting"), b"Existing file").unwrap();
    let jobs = start(&root);
    let id = submit(&jobs, kind, vec![source, remaining], &output);
    choose(&jobs, id, ConflictChoice::KeepBoth);
    let decision = question(&jobs, id);
    assert!(output.join("bundle (2)/a-done").exists());
    jobs.resolve_transfer(id, decision.generation, TransferReply::Cancel)
        .unwrap();
    let row = settled(&jobs, id);
    assert!(row.transfer_actions.retry && row.transfer_actions.remaining);
    let operation = row.transfer.as_ref().unwrap().operation.unwrap();
    fs::write(
        root.join("restart-marker.json"),
        serde_json::to_vec(&serde_json::json!({"request":id,"operation":operation})).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private child was not killed");
}

#[test]
fn queued_transfer_persists_original_facts_before_dispatch_without_adopting_replacements() {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("output");
    fs::create_dir(&output).unwrap();
    let blocker = root.path().join("blocker");
    fs::write(&blocker, b"Blocker").unwrap();
    fs::write(output.join("blocker"), b"Existing blocker").unwrap();
    let jobs = start(root.path());
    let active = submit(&jobs, TransferKind::Copy, vec![blocker], &output);
    let decision = question(&jobs, active);
    let selected = root.path().join("selected");
    fs::write(&selected, b"Original selected bytes").unwrap();
    let alpha = root.path().join("alpha");
    fs::write(&alpha, b"Another originally selected file").unwrap();
    let id = submit(
        &jobs,
        TransferKind::Copy,
        vec![selected.clone(), alpha],
        &output,
    );
    let database = rusqlite::Connection::open(root.path().join("history")).unwrap();
    let saved = || -> serde_json::Value {
        let body: Vec<u8> = database
            .query_row("SELECT body FROM requests WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    };
    let before = saved();
    let checkpoint = &before["Plan"]["transfer_checkpoint"];
    assert_eq!(checkpoint["version"], 3);
    assert_eq!(checkpoint["restart"]["version"], 1);
    assert_eq!(
        checkpoint["restart"]["sources"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        checkpoint["restart"]["destination"],
        before["Plan"]["request"]["Transfer"]["destination"]
    );
    assert!(matches!(
        checkpoint["restart"]["sources"][0]["captured"]["original"]["state"].as_str(),
        Some("available" | "unavailable")
    ));
    fs::rename(&selected, root.path().join("parked")).unwrap();
    fs::write(&selected, b"Replacement selected bytes").unwrap();
    assert_eq!(saved(), before);
    jobs.resolve_transfer(active, decision.generation, TransferReply::Cancel)
        .unwrap();
    let stopped = settled(&jobs, id);
    assert_ne!(stopped.phase, Phase::Finished);
    assert!(!output.join("selected").exists());
    assert_eq!(fs::read(selected).unwrap(), b"Replacement selected bytes");
    receive(jobs.abandon_transfer(active).unwrap()).unwrap();
    receive(jobs.abandon_transfer(id).unwrap()).unwrap();
    receive(jobs.shutdown().unwrap()).unwrap();
}

#[test]
fn a_killed_active_batch_restores_both_exact_parts_and_keeps_completed_work() {
    use std::os::unix::process::ExitStatusExt;
    for kind in [TransferKind::Copy, TransferKind::Move] {
        for first in [TransferPart::Failed, TransferPart::Remaining] {
            let root = tempfile::Builder::new()
                .prefix("canopa-active-transfer-restart-")
                .tempdir()
                .unwrap();
            let output = tempfile::Builder::new()
                .prefix(".canopa-active-transfer-restart-output-")
                .tempdir_in(std::env::current_dir().unwrap())
                .unwrap();
            if kind == TransferKind::Move
                && fs::metadata(root.path()).unwrap().dev()
                    == fs::metadata(output.path()).unwrap().dev()
            {
                continue; // The real cross-filesystem Move must copy the FIFO and fail it.
            }
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "cases::operation_transfer_resume::restart_active_transfer_helper",
                ])
                .env("CANOPA_TRANSFER_RESTART_ROOT", root.path())
                .env("CANOPA_TRANSFER_RESTART_OUTPUT", output.path())
                .env(
                    "CANOPA_TRANSFER_RESTART_KIND",
                    if kind == TransferKind::Copy {
                        "Copy"
                    } else {
                        "Move"
                    },
                )
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let marker: serde_json::Value =
                serde_json::from_slice(&fs::read(root.path().join("restart-marker.json")).unwrap())
                    .unwrap();
            let id = marker["request"].as_u64().unwrap();
            let prefix = output.path().join("bundle (2)/a-done");
            let before = fs::metadata(&prefix).unwrap();
            fs::write(&prefix, b"Newer completed work after death").unwrap();
            let failed = root.path().join("bundle/b-failed");
            fs::remove_file(&failed).unwrap();
            fs::write(&failed, b"Repaired unselected child").unwrap();
            let jobs = start(root.path());
            let restored = settled(&jobs, id);
            if !(restored.transfer_actions.retry && restored.transfer_actions.remaining) {
                let retained_root = root.keep();
                let retained_output = output.keep();
                panic!(
                    "restart lost exact Retry/Continue: {restored:?}; retained private data: {} and {}",
                    retained_root.display(),
                    retained_output.display()
                );
            }
            assert_eq!(
                restored.transfer.as_ref().unwrap().operation,
                marker["operation"].as_u64()
            );
            let second = if first == TransferPart::Failed {
                TransferPart::Remaining
            } else {
                TransferPart::Failed
            };
            for part in [first, second] {
                let next = receive(jobs.resume_transfer(id, part).unwrap()).unwrap();
                assert!(receive(jobs.resume_transfer(id, part).unwrap()).is_err());
                if part == TransferPart::Remaining {
                    choose(&jobs, next, ConflictChoice::KeepBoth);
                }
                let done = settled(&jobs, next);
                assert_eq!(done.phase, Phase::Finished, "{kind:?}/{part:?}: {done:?}");
                let after = fs::metadata(&prefix).unwrap();
                assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
                assert_eq!(
                    fs::read(&prefix).unwrap(),
                    b"Newer completed work after death"
                );
            }
            assert_eq!(settled(&jobs, id).phase, Phase::Finished);
            assert_eq!(
                fs::read(output.path().join("bundle (2)/b-failed")).unwrap(),
                b"Repaired unselected child"
            );
            assert!(!output.path().join("bundle (3)").exists());
            assert_eq!(
                fs::read(output.path().join("z-waiting (2)")).unwrap(),
                b"Remaining selected bytes"
            );
            undo(&jobs);
            undo(&jobs);
            assert_eq!(
                fs::read(&prefix).unwrap(),
                b"Newer completed work after death"
            );
            assert!(!output.path().join("bundle (2)/b-failed").exists());
            assert!(!output.path().join("z-waiting (2)").exists());
            receive(jobs.shutdown().unwrap()).unwrap();
        }
    }
}

fn killed_active_fixture() -> (tempfile::TempDir, tempfile::TempDir, u64) {
    use std::os::unix::process::ExitStatusExt;
    let root = tempfile::Builder::new()
        .prefix("canopa-active-transfer-restart-")
        .tempdir()
        .unwrap();
    let output = tempfile::Builder::new()
        .prefix(".canopa-active-transfer-restart-output-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::operation_transfer_resume::restart_active_transfer_helper",
        ])
        .env("CANOPA_TRANSFER_RESTART_ROOT", root.path())
        .env("CANOPA_TRANSFER_RESTART_OUTPUT", output.path())
        .env("CANOPA_TRANSFER_RESTART_KIND", "Copy")
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("restart-marker.json")).unwrap())
            .unwrap();
    (root, output, marker["request"].as_u64().unwrap())
}

#[test]
fn recovered_active_parts_regain_owners_after_retrying_the_result_save() {
    let (root, output, id) = killed_active_fixture();
    let prefix = output.path().join("bundle (2)/a-done");
    fs::write(&prefix, b"Edited completed prefix before result retry").unwrap();
    let db = rusqlite::Connection::open(root.path().join("history")).unwrap();
    db.execute_batch("CREATE TRIGGER refuse_cursor_result BEFORE UPDATE ON requests WHEN NEW.phase<>1 BEGIN SELECT RAISE(ABORT,'private cursor result-save refusal'); END").unwrap();
    let activity = LocalActivity::default();
    let jobs = OperationJobs::start_with_activity(
        root.path().join("history"),
        Storage::load(root.path().join("volumes")).unwrap(),
        activity.clone(),
    )
    .unwrap();
    wait(|| (!jobs.snapshot().initializing).then_some(()));
    let unsaved = jobs
        .snapshot()
        .jobs
        .into_iter()
        .find(|row| row.id == id)
        .unwrap();
    assert_eq!(unsaved.phase, Phase::SaveFailed, "{unsaved:?}");
    assert!(!unsaved.transfer_actions.retry && !unsaved.transfer_actions.remaining);
    assert!(activity.holds_application());
    db.execute_batch("DROP TRIGGER refuse_cursor_result")
        .unwrap();
    jobs.retry_save(id).unwrap();
    let saved = settled(&jobs, id);
    assert!(
        saved.transfer_actions.retry
            && saved.transfer_actions.remaining
            && saved.transfer_actions.abandon,
        "{saved:?}"
    );
    assert!(activity.holds_application() && activity.try_remove().is_err());
    receive(jobs.abandon_transfer(id).unwrap()).unwrap();
    receive(jobs.shutdown().unwrap()).unwrap();
    assert!(!activity.holds_application());
    assert_eq!(
        fs::read(prefix).unwrap(),
        b"Edited completed prefix before result retry"
    );
}

#[test]
fn active_cursor_refuses_corrupt_borrowed_or_advanced_work_and_keeps_its_data() {
    for fault in [
        "body",
        "missing",
        "borrowed",
        "position",
        "spec",
        "watermark",
        "advanced",
        "save",
    ] {
        let (root, output, id) = killed_active_fixture();
        let prefix = output.path().join("bundle (2)/a-done");
        fs::write(&prefix, b"Edited completed prefix after active crash").unwrap();
        let before = fs::metadata(&prefix).unwrap();
        let db = rusqlite::Connection::open(root.path().join("history")).unwrap();
        match fault {
            "body" => {
                db.execute("UPDATE request_transfer_tasks SET body=replace(body,'z-waiting','unapproved') WHERE request=?1 AND role=0", [id]).unwrap();
            }
            "missing" => {
                db.execute(
                    "DELETE FROM request_transfer_tasks WHERE request=?1 AND role=0",
                    [id],
                )
                .unwrap();
            }
            "borrowed" => {
                db.execute("UPDATE request_transfer_tasks SET body=(SELECT body FROM request_transfer_tasks WHERE request=?1 AND role=1 LIMIT 1),body_sha256=(SELECT body_sha256 FROM request_transfer_tasks WHERE request=?1 AND role=1 LIMIT 1) WHERE request=?1 AND role=0", [id]).unwrap();
            }
            "position" => {
                db.execute("UPDATE request_transfer_tasks SET position=x'8000000000000009' WHERE request=?1 AND role=0", [id]).unwrap();
            }
            "spec" => {
                db.execute("UPDATE request_transfer_cursors SET spec=replace(spec,'Copy','Move') WHERE request=?1", [id]).unwrap();
            }
            "watermark" => {
                db.execute(
                    "UPDATE request_transfer_cursors SET ordinal=ordinal+1 WHERE request=?1",
                    [id],
                )
                .unwrap();
            }
            "advanced" => {
                db.execute("INSERT INTO changes(operation,ordinal,body,binding_revision,bindings) SELECT operation,(SELECT MAX(ordinal)+1 FROM changes WHERE operation=c.operation),body,binding_revision,bindings FROM changes c WHERE operation=(SELECT operation FROM request_executions WHERE request=?1) ORDER BY ordinal DESC LIMIT 1", [id]).unwrap();
            }
            "save" => {
                db.execute_batch("CREATE TRIGGER refuse_recovered_stop BEFORE INSERT ON request_transfer_stops BEGIN SELECT RAISE(ABORT,'private recovered stop refusal'); END").unwrap();
            }
            _ => unreachable!(),
        }
        let rows: u64 = db
            .query_row(
                "SELECT COUNT(*) FROM request_transfer_tasks WHERE request=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        for _ in 0..2 {
            let jobs = start(root.path());
            let row = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id)
                .unwrap();
            let expected = if fault == "save" {
                Phase::SaveFailed
            } else {
                Phase::Interrupted
            };
            assert_eq!(row.phase, expected, "{fault}: {row:?}");
            assert!(
                !row.transfer_actions.retry
                    && !row.transfer_actions.remaining
                    && !row.transfer_actions.abandon,
                "{fault}: {row:?}"
            );
            assert!(
                row.error.is_some(),
                "{fault}: the refused cursor needs its reason"
            );
            assert_eq!(
                db.query_row(
                    "SELECT COUNT(*) FROM request_transfer_cursors WHERE request=?1",
                    [id],
                    |row| row.get::<_, u64>(0)
                )
                .unwrap(),
                1
            );
            assert_eq!(
                db.query_row(
                    "SELECT COUNT(*) FROM request_transfer_tasks WHERE request=?1",
                    [id],
                    |row| row.get::<_, u64>(0)
                )
                .unwrap(),
                rows
            );
            assert_eq!(
                fs::read(&prefix).unwrap(),
                b"Edited completed prefix after active crash"
            );
            let after = fs::metadata(&prefix).unwrap();
            assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
            assert!(!output.path().join("bundle (2)/b-failed").exists());
            assert!(!output.path().join("z-waiting (2)").exists());
            if fault == "save" {
                assert!(receive(jobs.shutdown().unwrap()).is_err());
                drop(jobs);
                drop(wait(
                    || match canopa_files::operations::OperationEngine::open(
                        root.path().join("history"),
                    ) {
                        Ok(engine) => Some(engine),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
                        Err(error) => panic!("{error}"),
                    },
                ));
            } else {
                receive(jobs.shutdown().unwrap()).unwrap();
            }
        }
        if fault == "save" {
            db.execute_batch("DROP TRIGGER refuse_recovered_stop")
                .unwrap();
            let jobs = start(root.path());
            let row = settled(&jobs, id);
            assert!(
                row.transfer_actions.retry && row.transfer_actions.remaining,
                "{row:?}"
            );
            receive(jobs.abandon_transfer(id).unwrap()).unwrap();
            receive(jobs.shutdown().unwrap()).unwrap();
        }
    }
}

#[test]
#[ignore = "private process killed while an active transfer waits on its next conflict"]
fn restart_active_transfer_helper() {
    let root = std::path::PathBuf::from(std::env::var_os("CANOPA_TRANSFER_RESTART_ROOT").unwrap());
    let output =
        std::path::PathBuf::from(std::env::var_os("CANOPA_TRANSFER_RESTART_OUTPUT").unwrap());
    assert_eq!(root.parent().unwrap(), std::env::temp_dir());
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-active-transfer-restart-")
    );
    assert_eq!(output.parent().unwrap(), std::env::current_dir().unwrap());
    assert!(
        output
            .file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b".canopa-active-transfer-restart-output-")
    );
    let kind = match std::env::var("CANOPA_TRANSFER_RESTART_KIND")
        .unwrap()
        .as_str()
    {
        "Copy" => TransferKind::Copy,
        "Move" => TransferKind::Move,
        _ => panic!("invalid fixture kind"),
    };
    let source = root.join("bundle");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("a-done"), b"Confirmed prefix bytes").unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        source.join("b-failed"),
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let remaining = root.join("z-waiting");
    fs::write(&remaining, b"Remaining selected bytes").unwrap();
    fs::create_dir(output.join("bundle")).unwrap();
    fs::write(output.join("bundle/marker"), b"Existing folder").unwrap();
    fs::write(output.join("z-waiting"), b"Existing file").unwrap();
    let jobs = start(&root);
    let ledger = rusqlite::Connection::open(root.join("history")).unwrap();
    ledger.execute_batch("CREATE TRIGGER reject_queue_tail_rewrites BEFORE UPDATE ON request_transfer_tasks WHEN OLD.role=0 BEGIN SELECT RAISE(ABORT,'unchanged queued tasks must not be rewritten'); END").unwrap();
    let id = submit(&jobs, kind, vec![source, remaining], &output);
    choose(&jobs, id, ConflictChoice::KeepBoth);
    let _decision = question(&jobs, id);
    assert!(output.join("bundle (2)/a-done").exists());
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let operation: u64 = db
        .query_row(
            "SELECT operation FROM request_executions WHERE request=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    let pending: u64 = db
        .query_row("SELECT COUNT(*) FROM pending", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        pending, 0,
        "this gate kills between completed filesystem steps"
    );
    let (recorded, actual): (u64, u64) = db.query_row("SELECT bytes,length(CAST(spec AS BLOB))+length(CAST(progress AS BLOB))+256+(SELECT coalesce(sum(length(CAST(body AS BLOB))+length(position)+128),0) FROM request_transfer_tasks WHERE request=?1) FROM request_transfer_cursors WHERE request=?1", [id], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
    assert_eq!(
        recorded, actual,
        "incremental cursor byte accounting drifted"
    );
    ledger
        .execute_batch("DROP TRIGGER reject_queue_tail_rewrites")
        .unwrap();
    fs::write(
        root.join("restart-marker.json"),
        serde_json::to_vec(&serde_json::json!({"request":id,"operation":operation})).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private child was not killed");
}
