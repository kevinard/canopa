use canopa_files::{
    action_jobs::{ActionJobs, ActionRequest, Phase as ActionPhase},
    actions::CustomAction,
    local_activity::LocalActivity,
    operation_jobs::{OperationJobs, Phase, Request},
    operations::{OperationRequest, UndoChoice},
    storage::Storage,
};
use futures_channel::oneshot;
use std::{
    fs,
    path::Path,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < deadline, "local work did not settle");
        thread::sleep(Duration::from_millis(5));
    }
}

fn reply<T>(mut receiver: oneshot::Receiver<T>) -> T {
    wait(|| receiver.try_recv().unwrap())
}

fn jobs(root: &Path, activity: &LocalActivity) -> OperationJobs {
    let service = OperationJobs::start_with_activity(
        root.join("history.sqlite3"),
        Storage::load(root.join("volumes.json")).unwrap(),
        activity.clone(),
    )
    .unwrap();
    wait(|| {
        let snapshot = service.snapshot();
        assert!(snapshot.error.is_none(), "{:?}", snapshot.error);
        (!snapshot.initializing).then_some(())
    });
    service
}

fn phase(jobs: &OperationJobs, id: u64, phase: Phase) {
    wait(|| {
        jobs.snapshot()
            .jobs
            .iter()
            .any(|job| job.id == id && job.phase == phase)
            .then_some(())
    });
}

fn create(jobs: &OperationJobs, path: &Path) -> u64 {
    reply(
        jobs.submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: path.to_owned(),
            }),
            None,
        )
        .unwrap(),
    )
    .unwrap()
}

fn action(root: &Path, script: &[u8]) -> ActionRequest {
    let path = root.join("action.sh");
    fs::write(&path, script).unwrap();
    ActionRequest {
        action: CustomAction::new("fixture", "Local command", "/bin/sh %f"),
        paths: Arc::new(vec![path]),
        directory: root.to_owned(),
        resolved_directory: fs::canonicalize(root).unwrap(),
    }
}

fn action_phase(jobs: &ActionJobs, id: u64, phase: ActionPhase) {
    wait(|| {
        jobs.snapshot()
            .jobs
            .iter()
            .any(|job| job.id == id && job.phase == phase)
            .then_some(())
    });
}

#[test]
fn removal_defers_real_file_work_and_commands_and_waiting_commands_can_be_cancelled() {
    let root = tempfile::tempdir().unwrap();
    let activity = LocalActivity::default();
    let files = jobs(root.path(), &activity);
    let commands = ActionJobs::start_with_activity(activity.clone()).unwrap();
    let removal = wait(|| activity.try_remove().ok());
    assert!(activity.holds_application());
    let command = reply(
        commands
            .submit(action(root.path(), b"printf 'command bytes\\n' > result\n"))
            .unwrap(),
    )
    .unwrap();
    action_phase(&commands, command, ActionPhase::WaitingForDevice);
    let target = root.path().join("after removal");
    let mut admission = files
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: target.clone(),
            }),
            None,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_millis(150);
    while Instant::now() < deadline {
        assert!(
            admission.try_recv().unwrap().is_none(),
            "filesystem work was admitted during removal"
        );
        assert!(!target.exists() && !root.path().join("result").exists());
        thread::sleep(Duration::from_millis(5));
    }
    commands.cancel(command).unwrap();
    action_phase(&commands, command, ActionPhase::Finished);
    let snapshot = commands.snapshot();
    assert!(
        snapshot.jobs[0]
            .outcome
            .as_deref()
            .unwrap()
            .as_ref()
            .unwrap()
            .cancelled
    );
    assert!(!root.path().join("result").exists());
    drop(removal);
    let id = reply(admission).unwrap();
    phase(&files, id, Phase::Finished);
    assert!(target.is_dir());
    let next = reply(
        commands
            .submit(action(root.path(), b"printf 'command bytes\\n' > result\n"))
            .unwrap(),
    )
    .unwrap();
    action_phase(&commands, next, ActionPhase::Finished);
    assert_eq!(
        fs::read(root.path().join("result")).unwrap(),
        b"command bytes\n"
    );
    let _removal = wait(|| activity.try_remove().ok());
}

#[test]
fn a_windowless_decision_and_queued_work_keep_removal_out_until_their_actual_settlement() {
    let root = tempfile::tempdir().unwrap();
    let activity = LocalActivity::default();
    let service = jobs(root.path(), &activity);
    drop(service.take_notices().unwrap());
    let folder = root.path().join("created");
    let first = create(&service, &folder);
    phase(&service, first, Phase::Finished);
    fs::write(folder.join("later.txt"), b"later work").unwrap();
    let undo = reply(
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
    let question = wait(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == undo)
            .and_then(|job| job.decision)
    });
    assert!(activity.try_remove().is_err());
    let queued_path = root.path().join("queued");
    let queued = create(&service, &queued_path);
    phase(&service, queued, Phase::Queued);
    service.cancel(queued).unwrap();
    wait(|| {
        (!service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == queued && job.phase.holds_application()))
        .then_some(())
    });
    assert!(
        activity.try_remove().is_err(),
        "cancelling queued work released the unanswered operation"
    );
    service
        .resolve(undo, question.generation, UndoChoice::KeepItem)
        .unwrap();
    phase(&service, undo, Phase::Finished);
    let _removal = wait(|| activity.try_remove().ok());
    assert!(!queued_path.exists());
    assert_eq!(fs::read(folder.join("later.txt")).unwrap(), b"later work");
}

#[test]
fn running_custom_actions_exclude_removal_through_process_cancellation() {
    let root = tempfile::tempdir().unwrap();
    let activity = LocalActivity::default();
    let commands = ActionJobs::start_with_activity(activity.clone()).unwrap();
    drop(commands.take_notices().unwrap());
    let id = reply(
        commands
            .submit(action(
                root.path(),
                b": > ready\nwhile :; do /bin/sleep 0.02; done\n",
            ))
            .unwrap(),
    )
    .unwrap();
    wait(|| root.path().join("ready").exists().then_some(()));
    assert!(activity.try_remove().is_err());
    assert!(activity.holds_application());
    commands.cancel(id).unwrap();
    action_phase(&commands, id, ActionPhase::Finished);
    assert!(
        commands.snapshot().jobs[0]
            .outcome
            .as_deref()
            .unwrap()
            .as_ref()
            .unwrap()
            .cancelled
    );
    let _removal = wait(|| activity.try_remove().ok());
}

#[test]
fn failed_admission_and_cancelled_startup_release_their_work_without_a_frontend() {
    let root = tempfile::tempdir().unwrap();
    let activity = LocalActivity::default();
    let service = jobs(root.path(), &activity);
    assert!(
        reply(
            service
                .submit(
                    Request::Mutation(OperationRequest::CreateFolder {
                        path: "relative".into()
                    }),
                    None
                )
                .unwrap()
        )
        .is_err()
    );
    let commands = ActionJobs::start_with_activity(activity.clone()).unwrap();
    let mut invalid = action(root.path(), b"exit 0\n");
    invalid.action.command.clear();
    assert!(reply(commands.submit(invalid).unwrap()).is_err());
    let removal = wait(|| activity.try_remove().ok());
    let history = root.path().join("waiting/history.sqlite3");
    let waiting = OperationJobs::start_with_activity(
        history.clone(),
        Storage::load(root.path().join("other-volumes.json")).unwrap(),
        activity.clone(),
    )
    .unwrap();
    thread::sleep(Duration::from_millis(150));
    assert!(waiting.snapshot().initializing && !history.exists());
    drop(waiting);
    // The removal still owns residency after the cancelled startup drops.
    assert!(activity.holds_application());
    drop(removal);
    wait(|| (!activity.holds_application()).then_some(()));
    assert!(
        !history.exists(),
        "cancelled startup opened history after removal finished"
    );
}

#[test]
fn an_unsaved_cancelled_result_keeps_removal_out_until_its_checkpoint_succeeds() {
    let root = tempfile::tempdir().unwrap();
    let activity = LocalActivity::default();
    let service = jobs(root.path(), &activity);
    let folder = root.path().join("created");
    let first = create(&service, &folder);
    phase(&service, first, Phase::Finished);
    fs::write(folder.join("later.txt"), b"keep later work").unwrap();
    let undo = reply(
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
    let question = wait(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == undo)
            .and_then(|job| job.decision)
    });
    let target = root.path().join("cancelled folder");
    let queued = create(&service, &target);
    let history = root.path().join("history.sqlite3");
    let moved = root.path().join("moved-history.sqlite3");
    fs::rename(&history, &moved).unwrap();
    service.cancel(queued).unwrap();
    phase(&service, queued, Phase::SaveFailed);
    fs::rename(&moved, &history).unwrap();
    service
        .resolve(undo, question.generation, UndoChoice::KeepItem)
        .unwrap();
    phase(&service, undo, Phase::Finished);
    assert!(
        activity.try_remove().is_err(),
        "removal ignored the unsaved stopped result"
    );
    service.retry_save(queued).unwrap();
    phase(&service, queued, Phase::Finished);
    let _removal = wait(|| activity.try_remove().ok());
    assert!(!target.exists());
    assert_eq!(
        fs::read(folder.join("later.txt")).unwrap(),
        b"keep later work"
    );
}

#[test]
fn cancelled_recovery_inspection_retires_even_behind_a_deferred_mutation() {
    let root = tempfile::tempdir().unwrap();
    let activity = LocalActivity::default();
    let service = jobs(root.path(), &activity);
    let removal = wait(|| activity.try_remove().ok());
    let target = root.path().join("waiting mutation");
    let mut mutation = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: target.clone(),
            }),
            None,
        )
        .unwrap();
    let cancellation = canopa_files::Cancellation::default();
    let inspection = service.recovery_page(None, cancellation.clone()).unwrap();
    cancellation.cancel();
    assert!(reply(inspection).is_err());
    assert!(mutation.try_recv().unwrap().is_none());
    assert!(!target.exists());
    assert!(
        reply(service.shutdown().unwrap()).is_err(),
        "shutdown discarded deferred admission"
    );
    drop(removal);
    let id = reply(mutation).unwrap();
    phase(&service, id, Phase::Finished);
    let _removal = wait(|| activity.try_remove().ok());
    assert!(target.is_dir());
}

#[test]
fn the_host_is_woken_after_actual_removal_retirement() {
    use futures_util::{FutureExt, StreamExt};
    let activity = LocalActivity::default();
    let mut notices = activity.take_notices().unwrap();
    assert!(activity.take_notices().is_none());
    let removal = activity.try_remove().unwrap();
    assert!(activity.holds_application());
    assert_eq!(notices.next().now_or_never(), Some(Some(())));
    drop(removal);
    assert!(!activity.holds_application());
    assert_eq!(notices.next().now_or_never(), Some(Some(())));
}

#[test]
fn saturated_removal_admission_still_retires_cancelled_reads_and_refuses_shutdown() {
    let root = tempfile::tempdir().unwrap();
    let activity = LocalActivity::default();
    let service = jobs(root.path(), &activity);
    let removal = wait(|| activity.try_remove().ok());
    // The existing bounded deferred backlog is full. These are real read-only
    // history requests; no fabricated coordinator state or filesystem hook.
    let mut pending = Vec::new();
    for _ in 0..1024 {
        pending.push(wait(|| service.needs_recovery_review().ok()));
    }
    let cancellation = canopa_files::Cancellation::default();
    let inspection = wait(|| service.recovery_page(None, cancellation.clone()).ok());
    cancellation.cancel();
    assert!(reply(inspection).is_err());
    let target = root.path().join("refused excess admission");
    let excess = wait(|| {
        service
            .submit(
                Request::Mutation(OperationRequest::CreateFolder {
                    path: target.clone(),
                }),
                None,
            )
            .ok()
    });
    assert!(reply(excess).is_err(), "deferred work exceeded its bound");
    let shutdown = wait(|| service.shutdown().ok());
    assert!(reply(shutdown).is_err());
    assert!(!target.exists());
    drop(removal);
    for receipt in pending {
        assert!(!reply(receipt).unwrap());
    }
    let _removal = wait(|| activity.try_remove().ok());
    assert!(!target.exists(), "refused work ran after removal finished");
}
