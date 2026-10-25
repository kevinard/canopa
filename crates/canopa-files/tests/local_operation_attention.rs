#![cfg(feature = "background-operations")]

use canopa_files::{
    operation_attention::OperationAttention,
    operation_jobs::{OperationJobs, Phase, Request},
    operations::{OperationRequest, UndoChoice},
};
use std::{
    fs,
    time::{Duration, Instant},
};

fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "operation attention did not settle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn operation_and_decision_attention_settle_without_a_frontend_or_observer() {
    let root = tempfile::tempdir().unwrap();
    let service = OperationJobs::start(
        root.path().join("state/history.sqlite3"),
        root.path().join("state/volumes.json"),
    )
    .unwrap();
    let owner =
        OperationAttention::start(service.clone(), Err("Notification startup refused".into()))
            .unwrap();
    let frontend = owner.clone();
    drop(frontend.take_notices().unwrap());
    drop(frontend);
    let folder = root.path().join("Keep later work");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: folder.clone(),
            }),
            None,
        )
        .unwrap();
    let created = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for(|| {
        let state = owner.snapshot();
        (state
            .queue
            .jobs
            .iter()
            .any(|job| job.id == created && job.phase == Phase::Finished)
            && state
                .notification_errors
                .get(&created)
                .is_some_and(|error| error == "Notification startup refused")
            && !state.queue.holds_application())
        .then_some(())
    });
    assert!(folder.is_dir());
    fs::write(folder.join("later.txt"), b"Later work remains safe").unwrap();
    let mut admission = service
        .submit(
            Request::Undo {
                expected: service.snapshot().undo.unwrap().id,
            },
            None,
        )
        .unwrap();
    let undo = wait_for(|| admission.try_recv().unwrap()).unwrap();
    let generation = wait_for(|| {
        let state = owner.snapshot();
        (state.notification_errors.contains_key(&undo) && state.queue.pending_attention == 0)
            .then(|| {
                state
                    .queue
                    .jobs
                    .iter()
                    .find(|job| job.id == undo)
                    .and_then(|job| job.decision.as_ref())
                    .map(|decision| decision.generation)
            })
            .flatten()
    });
    assert!(
        service.snapshot().holds_application(),
        "the unanswered decision remains active after its delivery refusal"
    );
    let mut shutdown = service.shutdown().unwrap();
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    service
        .resolve(undo, generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for(|| (!owner.snapshot().queue.holds_application()).then_some(()));
    assert_eq!(
        fs::read(folder.join("later.txt")).unwrap(),
        b"Later work remains safe"
    );
    assert_eq!(
        owner.snapshot().notification_errors[&undo],
        "Notification startup refused"
    );
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
#[ignore = "requires the maintained private notification bus fixture"]
fn operation_delivery_and_decisions_hold_shutdown_without_gui_initialization() {
    use canopa_files::desktop_notifications::Worker;
    use std::path::PathBuf;
    assert!(std::env::var_os("DISPLAY").is_none());
    assert!(std::env::var_os("WAYLAND_DISPLAY").is_none());
    let root = PathBuf::from(std::env::var_os("CANOPA_OPERATION_ATTENTION_FIXTURE").unwrap());
    assert_eq!(
        fs::read(root.join("fixture-owned")).unwrap(),
        b"Canopa private operation attention\n"
    );
    let service = OperationJobs::start(
        root.join("state/history.sqlite3"),
        root.join("state/volumes.json"),
    )
    .unwrap();
    let owner = OperationAttention::start(service.clone(), Ok(Worker::new().unwrap())).unwrap();
    let frontend = owner.clone();
    drop(frontend.take_notices().unwrap());
    drop(frontend);
    let folder = root.join("Headless operation");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: folder.clone(),
            }),
            None,
        )
        .unwrap();
    let created = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == created && job.phase == Phase::Finished)
            .then_some(())
    });
    wait_for(|| root.join("notifications.held").exists().then_some(()));
    assert!(folder.is_dir());
    assert_eq!(owner.snapshot().queue.pending_attention, 1);
    assert!(owner.snapshot().queue.holds_application());
    let mut shutdown = service.shutdown().unwrap();
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    fs::write(root.join("release-notifications"), b"release").unwrap();
    wait_for(|| (!owner.snapshot().queue.holds_application()).then_some(()));
    assert!(owner.snapshot().notification_errors.is_empty());
    fs::write(folder.join("later.txt"), b"Headless later work survives\n").unwrap();
    let mut admission = service
        .submit(
            Request::Undo {
                expected: service.snapshot().undo.unwrap().id,
            },
            None,
        )
        .unwrap();
    let undo = wait_for(|| admission.try_recv().unwrap()).unwrap();
    let generation = wait_for(|| {
        service
            .snapshot()
            .jobs
            .iter()
            .find(|job| job.id == undo)
            .and_then(|job| job.decision.as_ref())
            .map(|decision| decision.generation)
    });
    wait_for(|| {
        let events: Vec<serde_json::Value> =
            serde_json::from_slice(&fs::read(root.join("notifications.json")).unwrap()).unwrap();
        events
            .iter()
            .any(|event| {
                event["title"] == "Operation needs a decision"
                    && event["body"] == "Undo · Open Operations to review."
            })
            .then_some(())
    });
    let next = root.join("Queued without a browser");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder { path: next.clone() }),
            None,
        )
        .unwrap();
    let queued = wait_for(|| admission.try_recv().unwrap()).unwrap();
    assert!(!next.exists());
    assert!(owner.snapshot().queue.holds_application());
    let mut shutdown = service.shutdown().unwrap();
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    service
        .resolve(undo, generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for(|| {
        let snapshot = owner.snapshot();
        (snapshot
            .queue
            .jobs
            .iter()
            .any(|job| job.id == queued && job.phase == Phase::Finished)
            && !snapshot.queue.holds_application())
        .then_some(())
    });
    assert!(next.is_dir());
    assert_eq!(
        fs::read(folder.join("later.txt")).unwrap(),
        b"Headless later work survives\n"
    );
    assert!(owner.snapshot().notification_errors.is_empty());
    assert!(owner.snapshot().error.is_none());
    fs::write(root.join("result.json"), serde_json::to_vec(&serde_json::json!({"completed":3,"decision_generation":generation,"delivery_hold_refused_shutdown":true})).unwrap()).unwrap();
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}
