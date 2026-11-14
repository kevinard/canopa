use canopa_files::{
    Cancellation,
    operation_jobs::{JobSnapshot, OperationJobs, Phase, Request},
    operations::OperationEngine,
    transfers::{ConflictChoice, TransferEvent, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

fn wait_for<T>(mut ready: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = ready() {
            return value;
        }
        assert!(Instant::now() < end, "Recovery workflow did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn receive<T>(mut receiver: futures_channel::oneshot::Receiver<T>) -> T {
    wait_for(|| receiver.try_recv().expect("operation reply dropped"))
}
fn settled(service: &OperationJobs, id: u64) -> JobSnapshot {
    wait_for(|| {
        service.snapshot().jobs.into_iter().find(|job| {
            job.id == id && !matches!(job.phase, Phase::Queued | Phase::Running | Phase::Waiting)
        })
    })
}
fn saved_file(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let incoming = root.join("incoming");
    let destination = root.join("destination");
    fs::create_dir(&incoming).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(incoming.join("report.txt"), b"incoming bytes\n").unwrap();
    fs::write(destination.join("report.txt"), b"previous bytes\n").unwrap();
    let history = root.join("history");
    let engine = OperationEngine::open(&history).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![incoming.join("report.txt")],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    (history, destination.join("report.txt"))
}

#[test]
fn runtime_catalog_copy_and_undo_preserve_the_saved_version_and_survive_reopening() {
    use std::os::unix::ffi::OsStringExt;
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, current) = saved_file(root);
    let service = OperationJobs::start(history.clone(), root.join("volumes")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    assert!(!receive(service.needs_recovery_review().unwrap()).unwrap());
    let page = receive(
        service
            .recovery_page(None, Cancellation::default())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(page.items.len(), 1);
    let item = page.items.into_iter().next().unwrap();
    assert_eq!(item.original.as_ref(), Some(&current));
    let retained = item.entry.as_ref().unwrap().path.clone();
    let target = root.join(std::ffi::OsString::from_vec(b"recovered-\xff.txt".to_vec()));
    let id = receive(
        service
            .submit(
                Request::Recover {
                    item: Box::new(item),
                    destination: target.clone(),
                },
                None,
            )
            .unwrap(),
    )
    .unwrap();
    let result = settled(&service, id);
    assert_eq!(result.phase, Phase::Finished, "{result:?}");
    assert_eq!(fs::read(&target).unwrap(), b"previous bytes\n");
    assert_eq!(fs::read(&retained).unwrap(), b"previous bytes\n");
    assert_eq!(fs::read(&current).unwrap(), b"incoming bytes\n");
    let report = result.recovery.unwrap();
    assert_eq!(report.transfer.completed, 1);
    assert_eq!(report.transfer.destinations, vec![target.clone()]);
    let undo = service.snapshot().undo.unwrap();
    let id = receive(
        service
            .submit(Request::Undo { expected: undo.id }, None)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(settled(&service, id).phase, Phase::Finished);
    assert!(!target.exists());
    assert_eq!(fs::read(&retained).unwrap(), b"previous bytes\n");
    receive(service.shutdown().unwrap()).unwrap();
    let reopened = OperationJobs::start(history, root.join("volumes")).unwrap();
    wait_for(|| (!reopened.snapshot().initializing).then_some(()));
    let result = reopened
        .snapshot()
        .jobs
        .into_iter()
        .find(|job| job.recovery.is_some())
        .unwrap();
    assert_eq!(result.phase, Phase::Finished);
    assert_eq!(result.recovery.unwrap().transfer.completed, 1);
    assert!(
        !target.exists(),
        "reopening replayed the completed Recovery copy"
    );
    receive(reopened.shutdown().unwrap()).unwrap();
}

#[test]
fn a_cancelled_directory_copy_keeps_exact_runtime_continuation_through_an_output_refusal() {
    run_paused_copy("output-root");
}

#[test]
fn a_missing_output_parent_does_not_discard_the_exact_recovery_continuation() {
    run_paused_copy("missing-parent");
}

#[test]
fn cancelling_a_queued_recovery_continuation_keeps_its_exact_remaining_work() {
    run_paused_copy("queued-cancel");
}

#[test]
fn saving_a_partial_recovery_result_preserves_its_report_and_only_then_enables_continuation() {
    run_paused_copy("save-failure");
}

#[test]
fn queued_reconciliation_cannot_adopt_a_different_journal_after_running_work_finishes() {
    run_paused_copy("stale-review");
}

#[test]
fn cancellation_at_recovery_engine_entry_keeps_the_unstarted_source_in_its_outcome() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, current) = saved_file(root);
    let engine = OperationEngine::open(history).unwrap();
    let item = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .remove(0);
    let stored = item.entry.as_ref().unwrap().path.clone();
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let target = root.join("cancelled-before-engine-entry.txt");
    let outcome = engine
        .recover_copy_with_check(
            &item,
            &target,
            &cancelled,
            |_, _| panic!("cancelled work must not check or mutate filesystem placement"),
            |_| panic!("cancelled work must not ask a question"),
        )
        .unwrap();
    assert!(outcome.cancelled);
    assert_eq!(outcome.remaining, vec![stored.clone()]);
    assert_eq!(outcome.completed, 0);
    assert!(
        outcome.id.is_none()
            && outcome.continuation.is_none()
            && outcome.retry_continuation.is_none()
    );
    assert!(!target.exists());
    assert_eq!(std::fs::read(stored).unwrap(), b"previous bytes\n");
    assert_eq!(std::fs::read(current).unwrap(), b"incoming bytes\n");
    assert!(
        engine
            .interrupted_operations(&Cancellation::default())
            .unwrap()
            .is_empty()
    );
}

fn run_paused_copy(case: &str) {
    use std::process::{Command, Stdio};
    let root = tempfile::Builder::new()
        .prefix("canopa-files-recovery-runtime-")
        .tempdir_in("/tmp")
        .unwrap();
    let library = root.path().join("pause.so");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recovery_pause.c");
    let output = Command::new("cc")
        .args(["-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(source)
        .args(["-ldl", "-o"])
        .arg(&library)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::operation_recovery::recovery_copy_pause_child",
            "--nocapture",
        ])
        .env("LD_PRELOAD", library)
        .env("CANOPA_FILES_RECOVERY_PAUSE_ROOT", root.path())
        .env("CANOPA_FILES_RECOVERY_PAUSE_CASE", case)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let end = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= end {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("Recovery copy child timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if case != "output-root" {
        assert!(
            root.path().join(format!("{case}-tested")).exists(),
            "child did not exercise its {case} branch"
        );
    }
}

#[test]
#[ignore = "private child pauses its actual filesystem worker; parent owns timeout and cleanup"]
fn recovery_copy_pause_child() {
    use canopa_files::operation_jobs::RecoveryPart;
    let root =
        std::path::PathBuf::from(std::env::var_os("CANOPA_FILES_RECOVERY_PAUSE_ROOT").unwrap());
    assert!(
        root.starts_with("/tmp")
            && root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("canopa-files-recovery-runtime-")
    );
    let (history, current, stored, held) = saved_tree(&root);
    let service = OperationJobs::start(history.clone(), root.join("volumes")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let item = receive(
        service
            .recovery_page(None, Cancellation::default())
            .unwrap(),
    )
    .unwrap()
    .items
    .remove(0);
    fs::create_dir(root.join("output")).unwrap();
    let target = root.join("output/recovered");
    let id = receive(
        service
            .submit(
                Request::Recover {
                    item: Box::new(item),
                    destination: target.clone(),
                },
                None,
            )
            .unwrap(),
    )
    .unwrap();
    wait_for(|| root.join("copy-paused").exists().then_some(()));
    assert_eq!(
        fs::read(target.join("a.txt")).unwrap(),
        b"first recovered bytes\n"
    );
    service.cancel(id).unwrap();
    // FIFO control admission: this reply proves Cancel was processed while the
    // actual mutation worker was paused, and inspection remained available.
    assert_eq!(
        receive(
            service
                .recovery_page(None, Cancellation::default())
                .unwrap()
        )
        .unwrap()
        .items
        .len(),
        1
    );
    let case = std::env::var("CANOPA_FILES_RECOVERY_PAUSE_CASE").unwrap();
    let database = (case == "save-failure").then(|| {
        let database = rusqlite::Connection::open(&history).unwrap();
        database.execute_batch(&format!("CREATE TRIGGER private_recovery_result_failure BEFORE UPDATE ON requests WHEN NEW.id={id} AND NEW.phase<>1 BEGIN SELECT RAISE(FAIL, 'private Recovery result save failure'); END;")).unwrap();
        database
    });
    let review = (case == "stale-review")
        .then(|| receive(service.submit(Request::Reconcile, None).unwrap()).unwrap());
    fs::write(root.join("copy-release"), b"resume filesystem worker").unwrap();
    if let Some(database) = database {
        let unsaved = settled(&service, id);
        assert_eq!(unsaved.phase, Phase::SaveFailed, "{unsaved:?}");
        // The output folder and its first installed child are both changes.
        assert_eq!(unsaved.recovery.as_ref().unwrap().transfer.completed, 2);
        assert!(!unsaved.recovery_actions.remaining && !unsaved.recovery_actions.retry);
        assert!(
            receive(
                service
                    .resume_recovery(id, RecoveryPart::Remaining)
                    .unwrap()
            )
            .is_err()
        );
        assert!(service.snapshot().holds_application());
        database
            .execute_batch("DROP TRIGGER private_recovery_result_failure;")
            .unwrap();
        service.retry_save(id).unwrap();
        wait_for(|| {
            service
                .snapshot()
                .jobs
                .iter()
                .any(|job| job.id == id && job.phase == Phase::Stopped)
                .then_some(())
        });
        fs::write(
            root.join("save-failure-tested"),
            b"failed save and exact retry tested",
        )
        .unwrap();
    }
    let partial = settled(&service, id);
    if let Some(review) = review {
        let refused = settled(&service, review);
        assert_eq!(refused.phase, Phase::Failed, "{refused:?}");
        assert!(refused.error.unwrap().contains("changed after this review"));
        fs::write(root.join("stale-review-tested"), b"changed journal refused").unwrap();
    }
    assert_eq!(partial.phase, Phase::Stopped, "{partial:?}");
    assert!(partial.recovery_actions.remaining);
    assert!(partial.recovery.as_ref().unwrap().transfer.cancelled);
    assert!(!target.join("b.txt").exists());
    let mut id = id;
    if case == "queued-cancel" {
        fs::write(root.join("queued-cancel-tested"), b"branch exercised").unwrap();
        let folder = root.join("wait-folder");
        let created = receive(
            service
                .submit(
                    Request::Mutation(canopa_files::operations::OperationRequest::CreateFolder {
                        path: folder.clone(),
                    }),
                    None,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(settled(&service, created).phase, Phase::Finished);
        fs::write(folder.join("later.txt"), b"later work stays").unwrap();
        let undo = receive(
            service
                .submit(
                    Request::Undo {
                        expected: service.snapshot().undo.unwrap().id,
                    },
                    None,
                )
                .unwrap(),
        )
        .unwrap();
        let question = wait_for(|| {
            service
                .snapshot()
                .jobs
                .into_iter()
                .find(|job| job.id == undo)
                .and_then(|job| job.decision)
        });
        let queued = receive(
            service
                .resume_recovery(id, RecoveryPart::Remaining)
                .unwrap(),
        )
        .unwrap();
        service.cancel(queued).unwrap();
        receive(
            service
                .recovery_page(None, Cancellation::default())
                .unwrap(),
        )
        .unwrap();
        let stopped = settled(&service, queued);
        assert_eq!(stopped.phase, Phase::Stopped, "{stopped:?}");
        assert!(
            stopped.recovery_actions.remaining,
            "queued Cancel discarded the exact Recovery plan"
        );
        service
            .resolve(
                undo,
                question.generation,
                canopa_files::operations::UndoChoice::KeepItem,
            )
            .unwrap();
        assert_eq!(settled(&service, undo).phase, Phase::Finished);
        assert_eq!(
            fs::read(folder.join("later.txt")).unwrap(),
            b"later work stays"
        );
        id = queued;
    }

    if case == "missing-parent" {
        fs::write(root.join("missing-parent-tested"), b"branch exercised").unwrap();
        let held_parent = root.join("held-output-parent");
        fs::rename(root.join("output"), &held_parent).unwrap();
        let waiting = receive(
            service
                .resume_recovery(id, RecoveryPart::Remaining)
                .unwrap(),
        )
        .unwrap();
        let stopped = settled(&service, waiting);
        assert_eq!(stopped.phase, Phase::Stopped, "{stopped:?}");
        assert!(
            stopped.recovery_actions.remaining,
            "missing output parent discarded its exact plan"
        );
        assert!(!target.exists());
        fs::rename(held_parent, root.join("output")).unwrap();
        id = waiting;
    }
    let actual = root.join("original-output");
    fs::rename(&target, &actual).unwrap();
    fs::create_dir(&target).unwrap();
    fs::write(target.join("newer.txt"), b"newer output stays\n").unwrap();
    let next = receive(
        service
            .resume_recovery(id, RecoveryPart::Remaining)
            .unwrap(),
    )
    .unwrap();
    let refused = settled(&service, next);
    assert_eq!(refused.phase, Phase::Stopped, "{refused:?}");
    assert!(refused.recovery_actions.remaining);
    assert_eq!(
        fs::read(target.join("newer.txt")).unwrap(),
        b"newer output stays\n"
    );
    assert!(
        receive(
            service
                .resume_recovery(id, RecoveryPart::Remaining)
                .unwrap()
        )
        .is_err(),
        "one continuation was admitted twice"
    );
    fs::remove_file(target.join("newer.txt")).unwrap();
    fs::remove_dir(&target).unwrap();
    fs::rename(&actual, &target).unwrap();
    let final_id = receive(
        service
            .resume_recovery(next, RecoveryPart::Remaining)
            .unwrap(),
    )
    .unwrap();
    let finished = settled(&service, final_id);
    assert_eq!(finished.phase, Phase::Finished, "{finished:?}");
    assert_eq!(
        fs::read(target.join("a.txt")).unwrap(),
        b"first recovered bytes\n"
    );
    assert_eq!(
        fs::read(target.join("b.txt")).unwrap(),
        b"remaining recovered bytes\n"
    );
    for _ in 0..2 {
        let undo = service.snapshot().undo.unwrap();
        let id = receive(
            service
                .submit(Request::Undo { expected: undo.id }, None)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(settled(&service, id).phase, Phase::Finished);
    }
    assert!(!target.exists());
    assert_eq!(fs::read(&current).unwrap(), b"incoming bytes\n");
    assert_eq!(fs::read(held).unwrap(), b"previous bytes\n");
    assert_eq!(
        fs::read(stored.join("a.txt")).unwrap(),
        b"first recovered bytes\n"
    );
    assert_eq!(
        fs::read(stored.join("b.txt")).unwrap(),
        b"remaining recovered bytes\n"
    );
    receive(service.shutdown().unwrap()).unwrap();
}

fn saved_tree(
    root: &Path,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let (history, current) = saved_file(root);
    let engine = OperationEngine::open(&history).unwrap();
    let stored = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .clone();
    let held = root.join("held-saved-file");
    fs::rename(&stored, &held).unwrap();
    fs::create_dir(&stored).unwrap();
    fs::write(stored.join("a.txt"), b"first recovered bytes\n").unwrap();
    fs::write(stored.join("b.txt"), b"remaining recovered bytes\n").unwrap();
    drop(engine);
    (history, current, stored, held)
}

#[test]
fn retry_failed_recovery_preserves_newer_output_and_rejects_replace_and_stale_answers() {
    use canopa_files::operation_jobs::{Question, RecoveryPart, TransferQuestion};
    use std::os::unix::fs::PermissionsExt;
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, current, stored, held) = saved_tree(root);
    let blocked = stored.join("b.txt");
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o0)).unwrap();
    assert_eq!(
        fs::File::open(&blocked).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied,
        "fixture requires a real unreadable saved child"
    );
    let service = OperationJobs::start(history, root.join("volumes")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let item = receive(
        service
            .recovery_page(None, Cancellation::default())
            .unwrap(),
    )
    .unwrap()
    .items
    .remove(0);
    let target = root.join("recovered");
    let id = receive(
        service
            .submit(
                Request::Recover {
                    item: Box::new(item),
                    destination: target.clone(),
                },
                None,
            )
            .unwrap(),
    )
    .unwrap();
    let partial = settled(&service, id);
    assert_eq!(partial.phase, Phase::Failed, "{partial:?}");
    assert!(partial.recovery_actions.retry);
    assert_eq!(
        partial.recovery.as_ref().unwrap().transfer.failures.len(),
        1
    );
    assert_eq!(
        fs::read(target.join("a.txt")).unwrap(),
        b"first recovered bytes\n"
    );
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(target.join("b.txt"), b"newer destination work\n").unwrap();
    let next = receive(service.resume_recovery(id, RecoveryPart::Failed).unwrap()).unwrap();
    let question = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == next)
            .and_then(|job| job.decision)
    });
    assert!(matches!(
        question.question,
        Question::Recovery(TransferQuestion::Conflict { .. })
    ));
    service
        .resolve_transfer(
            next,
            question.generation + 1,
            TransferReply::Resolve {
                choice: ConflictChoice::KeepBoth,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    service
        .resolve_transfer(
            next,
            question.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    receive(
        service
            .recovery_page(None, Cancellation::default())
            .unwrap(),
    )
    .unwrap();
    let waiting = service
        .snapshot()
        .jobs
        .into_iter()
        .find(|job| job.id == next)
        .unwrap();
    assert_eq!(waiting.phase, Phase::Waiting);
    assert_eq!(waiting.decision.unwrap().generation, question.generation);
    assert_eq!(
        fs::read(target.join("b.txt")).unwrap(),
        b"newer destination work\n"
    );
    service
        .resolve_transfer(
            next,
            question.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::KeepBoth,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    let finished = settled(&service, next);
    assert_eq!(finished.phase, Phase::Finished, "{finished:?}");
    let report = finished.recovery.unwrap();
    assert_eq!(report.transfer.completed, 1);
    assert_eq!(report.transfer.destination, target);
    let copy = target.join("b (2).txt");
    assert_eq!(fs::read(&copy).unwrap(), b"remaining recovered bytes\n");
    assert_eq!(
        fs::read(target.join("b.txt")).unwrap(),
        b"newer destination work\n"
    );
    let undo = receive(
        service
            .submit(
                Request::Undo {
                    expected: service.snapshot().undo.unwrap().id,
                },
                None,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(settled(&service, undo).phase, Phase::Finished);
    assert!(!copy.exists());
    assert_eq!(
        fs::read(target.join("b.txt")).unwrap(),
        b"newer destination work\n"
    );
    fs::remove_file(target.join("b.txt")).unwrap();
    let undo = receive(
        service
            .submit(
                Request::Undo {
                    expected: service.snapshot().undo.unwrap().id,
                },
                None,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(settled(&service, undo).phase, Phase::Finished);
    assert!(!target.exists());
    assert_eq!(fs::read(current).unwrap(), b"incoming bytes\n");
    assert_eq!(fs::read(held).unwrap(), b"previous bytes\n");
    assert_eq!(fs::read(blocked).unwrap(), b"remaining recovered bytes\n");
    receive(service.shutdown().unwrap()).unwrap();
}

#[test]
fn runtime_salvages_interrupted_copy_undo_through_whole_folder_copy_keep_and_export_undo() {
    interrupted_undo_salvage(false);
}

#[test]
fn runtime_discovers_legacy_interruption_without_saved_request_rows() {
    interrupted_undo_salvage(true);
}

fn interrupted_undo_salvage(legacy: bool) {
    use std::process::{Command, Stdio};
    struct Fixture(Option<tempfile::TempDir>);
    impl Fixture {
        fn path(&self) -> &Path {
            self.0.as_ref().unwrap().path()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if std::thread::panicking() {
                eprintln!(
                    "retained interrupted Recovery root: {}",
                    self.0.take().unwrap().keep().display()
                );
            }
        }
    }
    // The child is killed when its quarantine appears, but it may already have
    // saved the next (Detached) phase by then, which reconciles differently.
    // Only a kill in the Taking phase exercises this case; repeat until one does.
    let mut attempts = 0;
    let (root, quarantine) = loop {
        attempts += 1;
        assert!(
            attempts <= 20,
            "the child never stopped in the Taking phase"
        );
        let root = Fixture(Some(
            tempfile::Builder::new()
                .prefix("canopa-files-recovery-interrupted-")
                .tempdir_in("/tmp")
                .unwrap(),
        ));
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "cases::operation_recovery::recovery_interrupted_undo_child",
                "--nocapture",
            ])
            .env("CANOPA_FILES_RECOVERY_INTERRUPTED_ROOT", root.path())
            .env(
                "CANOPA_FILES_RECOVERY_INTERRUPTED_LEGACY",
                if legacy { "1" } else { "0" },
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let end = Instant::now() + Duration::from_secs(20);
        let quarantine = loop {
            let found = fs::read_dir(root.path().join("destination"))
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .file_name()
                        .as_encoded_bytes()
                        .starts_with(b".canopa-files-stage-")
                })
                .map(|entry| entry.path().join("folder"))
                .find(|path| path.is_file());
            if root.path().join("undo-ready").exists()
                && let Some(found) = found
            {
                child.kill().unwrap();
                child.wait().unwrap();
                break found;
            }
            if child.try_wait().unwrap().is_some() || Instant::now() >= end {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "did not interrupt a real Undo quarantine: stdout={} stderr={}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            std::thread::sleep(Duration::from_micros(50));
        };
        let phase = {
            let history = rusqlite::Connection::open(root.path().join("history")).unwrap();
            let body: String = history
                .query_row("SELECT body FROM pending WHERE singleton=1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            serde_json::from_str::<serde_json::Value>(&body).unwrap()["removal"]["phase"]
                .as_str()
                .map(str::to_owned)
        };
        if phase.as_deref() == Some("taking") {
            break (root, quarantine);
        }
    };
    let original = root.path().join("destination/report.txt");
    fs::write(&original, b"newer occupant stays\n").unwrap();
    let service =
        OperationJobs::start(root.path().join("history"), root.path().join("volumes")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    assert!(
        service.snapshot().error.is_none(),
        "{:?}",
        service.snapshot().error
    );
    if legacy {
        assert!(service.snapshot().jobs.is_empty());
    }
    assert!(receive(service.needs_recovery_review().unwrap()).unwrap());
    let page = receive(
        service
            .recovery_page(None, Cancellation::default())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(page.interrupted.len(), 1);
    let item = page
        .items
        .into_iter()
        .find(|item| {
            item.entry
                .as_ref()
                .is_some_and(|entry| Some(entry.path.as_path()) == quarantine.parent())
        })
        .unwrap();
    let recovered = root.path().join("rescued.txt");
    let copy = receive(
        service
            .submit(
                Request::Recover {
                    item: Box::new(item),
                    destination: recovered.clone(),
                },
                None,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(settled(&service, copy).phase, Phase::Finished);
    assert_eq!(
        fs::read(recovered.join("folder")).unwrap(),
        fs::read(root.path().join("source/report.txt")).unwrap()
    );
    assert_eq!(fs::read(&original).unwrap(), b"newer occupant stays\n");
    assert!(quarantine.exists());
    assert!(receive(service.needs_recovery_review().unwrap()).unwrap());
    assert_eq!(
        receive(
            service
                .recovery_page(None, Cancellation::default())
                .unwrap()
        )
        .unwrap()
        .interrupted
        .len(),
        1,
        "copy-out replayed the original journal"
    );
    let refused = receive(service.submit(Request::Reconcile, None).unwrap()).unwrap();
    assert_eq!(settled(&service, refused).phase, Phase::Failed);
    assert_eq!(fs::read(&original).unwrap(), b"newer occupant stays\n");
    assert!(quarantine.exists());
    assert!(receive(service.needs_recovery_review().unwrap()).unwrap());
    let fresh = receive(
        service
            .recovery_page(None, Cancellation::default())
            .unwrap(),
    )
    .unwrap()
    .items
    .into_iter()
    .find(|item| {
        item.entry
            .as_ref()
            .is_some_and(|entry| Some(entry.path.as_path()) == quarantine.parent())
    })
    .unwrap();
    assert!(fresh.can_keep_in_recovery());
    let kept = receive(
        service
            .submit(
                Request::KeepRecovery {
                    item: Box::new(fresh),
                },
                None,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(settled(&service, kept).phase, Phase::Finished);
    assert_eq!(fs::read(&original).unwrap(), b"newer occupant stays\n");
    let held = root.path().join("held-newer.txt");
    fs::rename(&original, &held).unwrap();
    let retry = receive(service.submit(Request::Reconcile, None).unwrap()).unwrap();
    assert_eq!(settled(&service, retry).phase, Phase::Finished);
    assert!(
        receive(
            service
                .recovery_page(None, Cancellation::default())
                .unwrap()
        )
        .unwrap()
        .interrupted
        .is_empty()
    );
    assert!(
        quarantine.exists(),
        "explicit Keep preserves the original work folder"
    );
    assert!(!receive(service.needs_recovery_review().unwrap()).unwrap());
    let folder = root.path().join("normal-work");
    let id = receive(
        service
            .submit(
                Request::Mutation(canopa_files::operations::OperationRequest::CreateFolder {
                    path: folder.clone(),
                }),
                None,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(settled(&service, id).phase, Phase::Finished);
    for _ in 0..2 {
        let undo = receive(
            service
                .submit(
                    Request::Undo {
                        expected: service.snapshot().undo.unwrap().id,
                    },
                    None,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(settled(&service, undo).phase, Phase::Finished);
    }
    assert!(!folder.exists() && !recovered.exists() && !original.exists());
    assert_eq!(fs::read(held).unwrap(), b"newer occupant stays\n");
    assert_eq!(
        fs::read(quarantine).unwrap(),
        fs::read(root.path().join("source/report.txt")).unwrap()
    );
    receive(service.shutdown().unwrap()).unwrap();
}

#[test]
#[ignore = "private child killed after actual runtime Undo quarantine; parent owns cleanup"]
fn recovery_interrupted_undo_child() {
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_FILES_RECOVERY_INTERRUPTED_ROOT").unwrap(),
    );
    assert!(
        root.starts_with("/tmp")
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-files-recovery-interrupted-")
    );
    fs::create_dir(root.join("source")).unwrap();
    fs::create_dir(root.join("destination")).unwrap();
    fs::write(root.join("source/report.txt"), vec![b'r'; 16 * 1024 * 1024]).unwrap();
    if std::env::var("CANOPA_FILES_RECOVERY_INTERRUPTED_LEGACY").as_deref() == Ok("1") {
        let engine = OperationEngine::open(root.join("history")).unwrap();
        engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![root.join("source/report.txt")],
                    destination: root.join("destination"),
                },
                &Cancellation::default(),
                |event| {
                    assert!(
                        matches!(
                            event,
                            TransferEvent::Verifying { .. } | TransferEvent::Progress { .. }
                        ),
                        "{event:?}"
                    );
                    TransferReply::Continue
                },
            )
            .unwrap();
        fs::write(root.join("undo-ready"), b"parent can observe quarantine").unwrap();
        let result = engine.undo(&Cancellation::default());
        panic!("parent must kill the direct engine during actual Undo: {result:?}");
    }
    let service = OperationJobs::start(root.join("history"), root.join("volumes")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let copy = receive(
        service
            .submit(
                Request::Transfer(TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![root.join("source/report.txt")],
                    destination: root.join("destination"),
                }),
                None,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(settled(&service, copy).phase, Phase::Finished);
    fs::write(root.join("undo-ready"), b"parent can observe quarantine").unwrap();
    let undo = receive(
        service
            .submit(
                Request::Undo {
                    expected: service.snapshot().undo.unwrap().id,
                },
                None,
            )
            .unwrap(),
    )
    .unwrap();
    let result = settled(&service, undo);
    panic!("parent must kill the child during actual Undo: {result:?}");
}

#[test]
fn recovery_finalization_keeps_confirmed_results_until_explicit_reconciliation_succeeds() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, current) = saved_file(root);
    let service = OperationJobs::start(history.clone(), root.join("volumes")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    for index in 0..99 {
        let id = receive(
            service
                .submit(
                    Request::Mutation(canopa_files::operations::OperationRequest::CreateFolder {
                        path: root.join(format!("folder-{index}")),
                    }),
                    None,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(settled(&service, id).phase, Phase::Finished);
    }
    let page = receive(
        service
            .recovery_page(None, Cancellation::default())
            .unwrap(),
    )
    .unwrap();
    let item = page
        .items
        .into_iter()
        .find(|item| item.original.as_ref() == Some(&current))
        .unwrap();
    let retained = item.entry.as_ref().unwrap().path.clone();
    let database = rusqlite::Connection::open(&history).unwrap();
    database.execute_batch("CREATE TRIGGER private_recovery_trim_failure BEFORE DELETE ON operations BEGIN SELECT RAISE(FAIL, 'private Recovery finalization failure'); END;").unwrap();
    let output = root.join("confirmed-recovery.txt");
    let id = receive(
        service
            .submit(
                Request::Recover {
                    item: Box::new(item),
                    destination: output.clone(),
                },
                None,
            )
            .unwrap(),
    )
    .unwrap();
    let failed = settled(&service, id);
    assert_eq!(failed.phase, Phase::Interrupted, "{failed:?}");
    assert_eq!(failed.recovery.as_ref().unwrap().transfer.completed, 1);
    assert!(failed.recovery.as_ref().unwrap().interrupted);
    assert!(receive(service.needs_recovery_review().unwrap()).unwrap());
    assert_eq!(fs::read(&output).unwrap(), b"previous bytes\n");
    assert_eq!(fs::read(&retained).unwrap(), b"previous bytes\n");
    let retry = receive(service.submit(Request::Reconcile, None).unwrap()).unwrap();
    assert_eq!(settled(&service, retry).phase, Phase::Failed);
    assert_eq!(
        settled(&service, id).phase,
        Phase::Interrupted,
        "a failed reconciliation acknowledged the old report"
    );
    database
        .execute_batch("DROP TRIGGER private_recovery_trim_failure;")
        .unwrap();
    let retry = receive(service.submit(Request::Reconcile, None).unwrap()).unwrap();
    assert_eq!(settled(&service, retry).phase, Phase::Finished);
    let reviewed = settled(&service, id);
    assert_eq!(reviewed.phase, Phase::Stopped, "{reviewed:?}");
    assert!(
        reviewed.recovery.as_ref().unwrap().interrupted,
        "keep the historical outcome"
    );
    assert_eq!(reviewed.recovery.as_ref().unwrap().transfer.completed, 1);
    assert!(!reviewed.recovery_actions.retry && !reviewed.recovery_actions.remaining);
    assert!(!receive(service.needs_recovery_review().unwrap()).unwrap());
    let folder = root.join("work-after-review");
    let next = receive(
        service
            .submit(
                Request::Mutation(canopa_files::operations::OperationRequest::CreateFolder {
                    path: folder.clone(),
                }),
                None,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(settled(&service, next).phase, Phase::Finished);
    receive(service.shutdown().unwrap()).unwrap();
    drop(service);
    drop(database);
    let reopened = OperationJobs::start(history, root.join("volumes")).unwrap();
    wait_for(|| (!reopened.snapshot().initializing).then_some(()));
    assert_eq!(settled(&reopened, id).phase, Phase::Stopped);
    assert_eq!(
        settled(&reopened, id).recovery.unwrap().transfer.completed,
        1
    );
    assert_eq!(fs::read(output).unwrap(), b"previous bytes\n");
    assert_eq!(fs::read(current).unwrap(), b"incoming bytes\n");
    receive(reopened.shutdown().unwrap()).unwrap();
}

#[test]
fn occupied_output_and_changed_saved_source_refusals_do_not_block_unrelated_work() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, current) = saved_file(root);
    let service = OperationJobs::start(history, root.join("volumes")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let item = receive(
        service
            .recovery_page(None, Cancellation::default())
            .unwrap(),
    )
    .unwrap()
    .items
    .remove(0);
    let output = root.join("occupied.txt");
    fs::write(&output, b"newer destination stays\n").unwrap();
    let id = receive(
        service
            .submit(
                Request::Recover {
                    item: Box::new(item.clone()),
                    destination: output.clone(),
                },
                None,
            )
            .unwrap(),
    )
    .unwrap();
    let refused = settled(&service, id);
    assert_eq!(refused.phase, Phase::Failed, "{refused:?}");
    assert!(!refused.recovery.unwrap().interrupted);
    fs::write(
        &item.entry.as_ref().unwrap().path,
        b"externally changed saved version\n",
    )
    .unwrap();
    let stale_output = root.join("stale-copy.txt");
    // Admission may refuse the stale catalog item before creating a request.
    if let Ok(id) = receive(
        service
            .submit(
                Request::Recover {
                    item: Box::new(item),
                    destination: stale_output.clone(),
                },
                None,
            )
            .unwrap(),
    ) {
        let refused = settled(&service, id);
        assert_eq!(refused.phase, Phase::Failed, "{refused:?}");
        assert!(!refused.recovery.unwrap().interrupted);
    }
    assert!(!stale_output.exists());
    assert_eq!(fs::read(output).unwrap(), b"newer destination stays\n");
    assert_eq!(fs::read(current).unwrap(), b"incoming bytes\n");
    let folder = root.join("unrelated-work");
    let next = receive(
        service
            .submit(
                Request::Mutation(canopa_files::operations::OperationRequest::CreateFolder {
                    path: folder.clone(),
                }),
                None,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(settled(&service, next).phase, Phase::Finished);
    assert!(folder.is_dir());
    receive(service.shutdown().unwrap()).unwrap();
}
