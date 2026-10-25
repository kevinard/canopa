#![cfg(feature = "background-actions")]

use canopa_files::{
    action_jobs::{ActionRequest, Phase},
    actions::CustomAction,
    local_actions::LocalActions,
};
use std::{
    fs,
    sync::Arc,
    time::{Duration, Instant},
};

fn wait_for<T>(mut ready: impl FnMut() -> Option<T>) -> T {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = ready() {
            return value;
        }
        assert!(Instant::now() < until, "background action did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn commands_and_failed_attention_settle_without_a_frontend_or_observer() {
    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("action.sh");
    fs::write(&script, b": > ready\nwhile [ ! -e release ]; do /bin/sleep 0.02; done\nprintf 'background result\\n' > completed\n").unwrap();
    // A real platform-startup failure must remain observable without making
    // accepted commands or receipt handoff depend on a frontend's event loop.
    let runtime = LocalActions::start(Err("Notification startup refused".into())).unwrap();
    drop(runtime.take_notices().unwrap());
    let frontend = runtime.clone();
    let mut admission = frontend
        .submit(ActionRequest {
            action: CustomAction::new("background", "Background action", "/bin/sh %f"),
            paths: Arc::new(vec![script]),
            directory: root.path().to_owned(),
            resolved_directory: fs::canonicalize(root.path()).unwrap(),
        })
        .unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for(|| root.path().join("ready").exists().then_some(()));
    drop(frontend);
    assert!(runtime.snapshot().holds_application());
    let mut refused = runtime.shutdown().unwrap();
    assert!(wait_for(|| refused.try_recv().unwrap()).is_err());
    fs::write(root.path().join("release"), b"release").unwrap();
    let snapshot = wait_for(|| {
        let snapshot = runtime.snapshot();
        (snapshot
            .queue
            .jobs
            .iter()
            .any(|job| job.id == id && job.phase == Phase::Finished)
            && snapshot.notification_errors.contains_key(&id)
            && !snapshot.holds_application())
        .then_some(snapshot)
    });
    assert_eq!(
        fs::read(root.path().join("completed")).unwrap(),
        b"background result\n"
    );
    assert_eq!(
        snapshot.notification_errors[&id],
        "Notification startup refused"
    );
    assert_eq!(snapshot.queue.pending_completions, 0);
    let mut stopped = runtime.shutdown().unwrap();
    wait_for(|| stopped.try_recv().unwrap()).unwrap();
}

#[test]
#[ignore = "requires the maintained private notification bus fixture"]
fn notification_delivery_is_bounded_and_holds_shutdown_without_gui_initialization() {
    use canopa_files::desktop_notifications::Worker;
    use std::path::PathBuf;
    assert!(std::env::var_os("DISPLAY").is_none());
    assert!(std::env::var_os("WAYLAND_DISPLAY").is_none());
    let root = PathBuf::from(std::env::var_os("CANOPA_ACTION_HOST_FIXTURE").unwrap());
    assert_eq!(
        fs::read(root.join("fixture-owned")).unwrap(),
        b"Canopa private action host\n"
    );
    let runtime = LocalActions::start(Ok(Worker::new().unwrap())).unwrap();
    drop(runtime.take_notices().unwrap());
    let frontend = runtime.clone();
    let mut accepted = Vec::new();
    for index in 0..20 {
        let directory = root.join(format!("command-{index}"));
        fs::create_dir(&directory).unwrap();
        let script = directory.join("action.sh");
        fs::write(
            &script,
            format!("printf 'headless result {index}\\n' > completed\n"),
        )
        .unwrap();
        let mut admitted = frontend
            .submit(ActionRequest {
                action: CustomAction::new(
                    "headless",
                    format!("Headless action {index}"),
                    "/bin/sh %f",
                ),
                paths: Arc::new(vec![script]),
                directory: directory.clone(),
                resolved_directory: fs::canonicalize(&directory).unwrap(),
            })
            .unwrap();
        accepted.push((wait_for(|| admitted.try_recv().unwrap()).unwrap(), index));
    }
    drop(frontend);
    wait_for(|| {
        let state = runtime.snapshot();
        (state.queue.jobs.len() == 20
            && state.queue.pending_completions == 0
            && state
                .queue
                .jobs
                .iter()
                .all(|job| job.phase == Phase::Finished))
        .then_some(())
    });
    wait_for(|| root.join("notifications.held").exists().then_some(()));
    let mut refused = runtime.shutdown().unwrap();
    assert!(wait_for(|| refused.try_recv().unwrap()).is_err());
    // The serialized refusal also follows completion of the handoff batch.
    let held = runtime.snapshot();
    assert!(held.holds_application());
    assert!((1..=17).contains(&held.pending_notifications));
    assert!(held.notification_errors.len() >= 3);
    assert_eq!(
        held.pending_notifications + held.notification_errors.len(),
        20
    );
    assert!(
        held.notification_errors
            .values()
            .all(|error| error.contains("busy"))
    );
    for (_, index) in &accepted {
        assert_eq!(
            fs::read(root.join(format!("command-{index}/completed"))).unwrap(),
            format!("headless result {index}\n").as_bytes()
        );
    }
    let expected: Vec<_> = accepted
        .iter()
        .filter(|(id, _)| !held.notification_errors.contains_key(id))
        .map(|(_, index)| format!("Headless action {index}"))
        .collect();
    fs::write(
        root.join("result.json"),
        serde_json::to_vec(&serde_json::json!({
            "accepted":20, "busy":held.notification_errors.len(), "expected_titles":expected,
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(root.join("release-notifications"), b"release").unwrap();
    let final_state = wait_for(|| {
        let state = runtime.snapshot();
        (!state.holds_application()).then_some(state)
    });
    assert_eq!(final_state.notification_errors, held.notification_errors);
    assert!(final_state.error.is_none(), "{:?}", final_state.error);
    let mut stopped = runtime.shutdown().unwrap();
    wait_for(|| stopped.try_recv().unwrap()).unwrap();
}
