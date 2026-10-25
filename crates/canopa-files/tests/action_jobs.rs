use canopa_files::{
    action_jobs::{ActionJobs, ActionRequest, CompletionStatus, Phase},
    actions::CustomAction,
};
use futures_channel::oneshot;
use std::{
    fs,
    path::Path,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

fn wait_for<T>(mut ready: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = ready() {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "custom-action runtime did not settle"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn reply<T>(mut receiver: oneshot::Receiver<T>) -> T {
    wait_for(|| receiver.try_recv().unwrap())
}

fn request(directory: &Path, name: &str, script: &[u8]) -> ActionRequest {
    fs::create_dir_all(directory).unwrap();
    let path = directory.join("action.sh");
    fs::write(&path, script).unwrap();
    ActionRequest {
        action: CustomAction::new("fixture", name, "/bin/sh %f"),
        paths: Arc::new(vec![path]),
        directory: directory.to_owned(),
        resolved_directory: fs::canonicalize(directory).unwrap(),
    }
}

#[test]
fn managed_commands_require_preparation_and_refuse_observer_loss() {
    use canopa_files::{
        local_activity::LocalActivity, operation_jobs::OperationJobs, storage::Storage,
    };
    let root = tempfile::tempdir().unwrap();
    let storage = Storage::load(root.path().join("volumes.json")).unwrap();
    let (prepared, ready) = std::sync::mpsc::sync_channel(1);
    let files =
        OperationJobs::start_after_storage(root.path().join("history.sqlite3"), ready).unwrap();
    let activity = LocalActivity::default();
    let actions = ActionJobs::start_with_storage(activity.clone(), files.storage_source()).unwrap();
    let request = request(
        root.path(),
        "Managed private command",
        b"printf 'prepared storage\\n' > prepared-result\n",
    );
    assert!(actions.submit(request.clone()).is_err());
    assert!(actions.snapshot().jobs.is_empty());
    assert!(!activity.holds_application() && !root.path().join("prepared-result").exists());
    prepared.send(Ok(storage.clone())).unwrap();
    wait_for(|| (!files.snapshot().initializing).then_some(()));
    assert!(files.snapshot().error.is_none());
    let id = reply(actions.submit(request.clone()).unwrap()).unwrap();
    let result = wait_for(|| {
        actions
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Finished)
    });
    assert!(result.outcome.as_deref().unwrap().as_ref().unwrap().success);
    assert_eq!(
        fs::read(root.path().join("prepared-result")).unwrap(),
        b"prepared storage\n"
    );
    assert_eq!(actions.take_completions().len(), 1);
    wait_for(|| (!activity.holds_application()).then_some(()));
    fs::remove_file(root.path().join("prepared-result")).unwrap();
    storage.observer_lost();
    assert!(actions.submit(request).is_err());
    assert!(!root.path().join("prepared-result").exists());
    assert!(!activity.holds_application());
    assert!(reply(actions.shutdown().unwrap()).is_ok());
    assert!(reply(files.shutdown().unwrap()).is_ok());
}

#[test]
fn accepted_commands_advance_without_a_renderer() {
    let root = tempfile::tempdir().unwrap();
    let runtime = ActionJobs::start().unwrap();
    // Observing UI state is optional and must never drive queue advancement.
    drop(runtime.take_notices().unwrap());
    let frontend = runtime.clone();
    let mut blockers = Vec::new();
    for index in 0..4 {
        let directory = root.path().join(format!("block-{index}"));
        let id = reply(frontend.submit(request(&directory, "Blocked action",
            b": > ready\nwhile [ ! -e release ]; do /bin/sleep 0.02; done\nprintf 'block complete\\n'\n"
        )).unwrap()).unwrap();
        wait_for(|| directory.join("ready").exists().then_some(()));
        blockers.push((id, directory));
    }
    let marker = root.path().join("marker");
    let queued = reply(frontend.submit(request(&marker, "Captured queued action",
        b"printf 'captured stdout\\n'\nprintf 'captured stderr\\n' >&2\nprintf 'exact marker bytes\\n' > completed\n"
    )).unwrap()).unwrap();
    drop(frontend);
    wait_for(|| {
        runtime
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == queued && job.phase == Phase::Queued)
            .then_some(())
    });
    assert!(!marker.join("completed").exists());
    assert!(runtime.snapshot().holds_application());
    assert!(reply(runtime.shutdown().unwrap()).is_err());
    fs::write(blockers[0].1.join("release"), b"release one slot").unwrap();
    let completed = wait_for(|| {
        runtime
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == queued && job.phase == Phase::Finished)
    });
    assert_eq!(completed.title, "Captured queued action");
    assert_eq!(
        fs::read(marker.join("completed")).unwrap(),
        b"exact marker bytes\n"
    );
    let outcome = completed.outcome.as_deref().unwrap().as_ref().unwrap();
    assert!(outcome.success);
    assert_eq!(outcome.stdout, b"captured stdout\n");
    assert_eq!(outcome.stderr, b"captured stderr\n");
    for (_, directory) in &blockers {
        fs::write(directory.join("release"), b"release the remaining slots").unwrap();
    }
    wait_for(|| {
        runtime
            .snapshot()
            .jobs
            .iter()
            .all(|job| job.phase == Phase::Finished)
            .then_some(())
    });
    assert_eq!(runtime.take_completions().len(), 5);
    assert!(!runtime.snapshot().holds_application());
    assert!(
        runtime
            .snapshot()
            .jobs
            .iter()
            .all(|job| job.phase == Phase::Finished)
    );
    reply(runtime.shutdown().unwrap()).unwrap();
}

#[test]
fn undelivered_completions_survive_finished_row_expiry() {
    let root = tempfile::tempdir().unwrap();
    let runtime = ActionJobs::start().unwrap();
    drop(runtime.take_notices().unwrap());
    let mut action = CustomAction::new("quick", "Quick action", "/bin/true");
    action.conditions.minimum_selection = 0;
    let captured = ActionRequest {
        action,
        paths: Arc::new(vec![]),
        directory: root.path().to_owned(),
        resolved_directory: fs::canonicalize(root.path()).unwrap(),
    };
    for _ in 0..1024 {
        reply(runtime.submit(captured.clone()).unwrap()).unwrap();
    }
    wait_for(|| {
        let snapshot = runtime.snapshot();
        (snapshot.jobs.len() == 100 && snapshot.jobs.iter().all(|job| job.phase == Phase::Finished))
            .then_some(())
    });
    assert!(!runtime.snapshot().jobs.iter().any(|job| job.id == 1));
    assert_eq!(runtime.snapshot().pending_completions, 1024);
    assert!(
        reply(runtime.submit(captured.clone()).unwrap()).is_err(),
        "unhanded receipts exceeded their admission capacity"
    );
    assert!(
        reply(runtime.shutdown().unwrap()).is_err(),
        "the runtime discarded unhanded completion notices when result rows expired"
    );
    let receipts = runtime.take_completions();
    assert_eq!(receipts.len(), 1024);
    let ids: std::collections::BTreeSet<_> = receipts.iter().map(|receipt| receipt.id).collect();
    assert_eq!(ids, (1..=1024).collect());
    assert!(
        receipts
            .iter()
            .all(|receipt| receipt.title == "Quick action"
                && receipt.status == CompletionStatus::Success)
    );
    assert!(runtime.take_completions().is_empty());
    assert!(!runtime.snapshot().holds_application());
    let fresh = reply(runtime.submit(captured).unwrap()).unwrap();
    wait_for(|| {
        runtime
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == fresh && job.phase == Phase::Finished)
            .then_some(())
    });
    let receipts = runtime.take_completions();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].id, fresh);
    assert_eq!(receipts[0].status, CompletionStatus::Success);
    reply(runtime.shutdown().unwrap()).unwrap();
}

#[test]
fn cancellation_releases_admission_capacity_and_never_runs_cancelled_queued_commands() {
    let root = tempfile::tempdir().unwrap();
    let runtime = ActionJobs::start().unwrap();
    let blocked = root.path().join("blocked");
    let captured = request(
        &blocked,
        "Gated command",
        b"printf 'started\\n' >> starts\nwhile :; do /bin/sleep 0.02; done\n",
    );
    let ids: Vec<_> = (0..1024)
        .map(|_| reply(runtime.submit(captured.clone()).unwrap()).unwrap())
        .collect();
    wait_for(|| {
        fs::read_to_string(blocked.join("starts"))
            .ok()
            .filter(|text| text.lines().count() == 4)
    });
    assert!(reply(runtime.submit(captured).unwrap()).is_err());
    for id in &ids[4..] {
        runtime.cancel(*id).unwrap();
    }
    wait_for(|| {
        let snapshot = runtime.snapshot();
        (snapshot
            .jobs
            .iter()
            .filter(|job| job.phase == Phase::Finished)
            .count()
            == 100
            && snapshot
                .jobs
                .iter()
                .filter(|job| job.phase == Phase::Running)
                .count()
                == 4)
            .then_some(())
    });
    let marker = root.path().join("after cancellation");
    let later = reply(
        runtime
            .submit(request(
                &marker,
                "Later command",
                b"printf 'accepted after refusal\\n' > completed\n",
            ))
            .unwrap(),
    )
    .unwrap();
    for id in &ids[..4] {
        runtime.cancel(*id).unwrap();
    }
    wait_for(|| {
        let snapshot = runtime.snapshot();
        snapshot
            .jobs
            .iter()
            .any(|job| job.id == later && job.phase == Phase::Finished)
            .then_some(())
    });
    wait_for(|| {
        runtime
            .snapshot()
            .jobs
            .iter()
            .all(|job| job.phase == Phase::Finished)
            .then_some(())
    });
    assert_eq!(
        fs::read_to_string(blocked.join("starts"))
            .unwrap()
            .lines()
            .count(),
        4
    );
    assert_eq!(
        fs::read(marker.join("completed")).unwrap(),
        b"accepted after refusal\n"
    );
    let receipts = runtime.take_completions();
    assert_eq!(
        receipts.len(),
        5,
        "queued cancellations must not fabricate completion notices"
    );
    for id in &ids[..4] {
        assert!(
            receipts
                .iter()
                .any(|r| r.id == *id && r.status == CompletionStatus::Cancelled)
        );
    }
    assert!(
        receipts
            .iter()
            .any(|r| r.id == later && r.status == CompletionStatus::Success)
    );
    assert_eq!(runtime.snapshot().jobs.len(), 100);
    reply(runtime.shutdown().unwrap()).unwrap();
}

#[test]
fn a_captured_action_refuses_a_retargeted_working_folder() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir(&second).unwrap();
    let mut captured = request(
        &first,
        "Bound command",
        b"printf 'must never run\\n' > unexpected\n",
    );
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(&first, &alias).unwrap();
    captured.directory = alias.clone();
    fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&second, &alias).unwrap();
    let runtime = ActionJobs::start().unwrap();
    let id = reply(runtime.submit(captured).unwrap()).unwrap();
    let finished = wait_for(|| {
        runtime
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Finished)
    });
    assert!(
        matches!(finished.completion_status(), Some(CompletionStatus::Unavailable(error)) if error.contains("original folder changed"))
    );
    assert!(!first.join("unexpected").exists());
    assert!(!second.join("unexpected").exists());
    assert_eq!(runtime.take_completions().len(), 1);
    reply(runtime.shutdown().unwrap()).unwrap();
}
