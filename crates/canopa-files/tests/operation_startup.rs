use canopa_files::{
    operation_jobs::{OperationJobs, Phase, Request},
    operations::OperationRequest,
    storage::Storage,
};
use std::{
    fs,
    sync::mpsc,
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
            "operation startup did not finish"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn storage_preparation_gates_history_and_work_without_a_frontend() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path();
    let volume = directory.join("private volume");
    fs::create_dir(&volume).unwrap();
    let storage = Storage::load(directory.join("volumes.json")).unwrap();
    let (prepared, ready) = mpsc::sync_channel(1);
    let history = directory.join("state/operations.sqlite3");
    let owner = OperationJobs::start_after_storage(history.clone(), ready).unwrap();
    let frontend = owner.clone();
    drop(frontend.take_notices());
    let target = volume.join("created without a frontend");
    let mut admitted = frontend
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: target.clone(),
            }),
            None,
        )
        .unwrap();
    drop(frontend);
    std::thread::sleep(Duration::from_millis(50));
    assert!(owner.snapshot().initializing);
    assert!(owner.storage().is_none());
    assert!(admitted.try_recv().unwrap().is_none());
    assert!(!history.exists());
    assert!(!target.exists());

    let observation = storage
        .mounted(volume.clone(), "uuid:private-startup-device".into())
        .unwrap();
    let epoch = observation.epoch;
    storage.prepare(observation).unwrap();
    assert!(prepared.send(Ok(storage.clone())).is_ok());
    let id = wait_for(|| admitted.try_recv().unwrap()).unwrap();
    wait_for(|| {
        owner
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Finished)
    });
    assert!(target.is_dir());
    assert!(history.is_file());
    assert!(owner.snapshot().undo.is_some());

    let observed = owner.storage().expect("the original prepared storage");
    assert!(std::sync::Arc::ptr_eq(&observed, &storage));
    storage.removed(&volume, epoch);
    let refused = volume.join("must remain absent after removal");
    let mut admitted = owner
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: refused.clone(),
            }),
            None,
        )
        .unwrap();
    let id = wait_for(|| admitted.try_recv().unwrap()).unwrap();
    let failure = wait_for(|| {
        owner
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Failed)
    });
    assert!(failure.error.is_some());
    assert!(!refused.exists());
    assert!(target.is_dir());
}

#[test]
fn failed_or_disconnected_preparation_never_opens_history() {
    let temporary = tempfile::tempdir().unwrap();
    for failure in [Some("The private volume observer failed."), None] {
        let history = temporary.path().join(if failure.is_some() {
            "explicit-failure.sqlite3"
        } else {
            "disconnected.sqlite3"
        });
        let (prepared, ready) = mpsc::sync_channel(1);
        let owner = OperationJobs::start_after_storage(history.clone(), ready).unwrap();
        if let Some(error) = failure {
            assert!(prepared.send(Err(error.into())).is_ok());
        }
        drop(prepared);
        let state = wait_for(|| {
            let state = owner.snapshot();
            (!state.initializing).then_some(state)
        });
        let error = state.error.expect("startup failure must remain observable");
        if let Some(expected) = failure {
            assert_eq!(error, expected);
        } else {
            assert!(error.contains("before its first observation"), "{error}");
        }
        assert!(owner.storage().is_none());
        assert!(state.jobs.is_empty());
        assert!(!history.exists());
    }
}

#[test]
fn final_owner_drop_releases_startup_even_if_the_platform_has_not_replied() {
    let temporary = tempfile::tempdir().unwrap();
    let history = temporary.path().join("unopened.sqlite3");
    let (prepared, ready) = mpsc::sync_channel(1);
    let owner = OperationJobs::start_after_storage(history.clone(), ready).unwrap();
    let mut notices = owner.take_notices().unwrap();
    drop(owner);
    wait_for(|| match notices.try_recv() {
        Err(futures_channel::mpsc::TryRecvError::Closed) => Some(()),
        _ => None,
    });
    assert!(!history.exists());
    assert!(
        prepared
            .send(Err("A deliberately late reply".into()))
            .is_err()
    );
}

#[test]
fn successful_readiness_after_final_drop_does_not_initialize_history() {
    let temporary = tempfile::tempdir().unwrap();
    let storage = Storage::load(temporary.path().join("volumes.json")).unwrap();
    for attempt in 0..16 {
        let history = temporary.path().join(format!("late-{attempt}.sqlite3"));
        let (prepared, ready) = mpsc::sync_channel(1);
        let owner = OperationJobs::start_after_storage(history.clone(), ready).unwrap();
        let mut notices = owner.take_notices().unwrap();
        // Let the startup worker wait on the actual platform handoff. The
        // successful observation is deliberately sent only after final Drop.
        std::thread::sleep(Duration::from_millis(10));
        assert!(owner.snapshot().initializing);
        drop(owner);
        let _ = prepared.send(Ok(storage.clone()));
        wait_for(|| match notices.try_recv() {
            Err(futures_channel::mpsc::TryRecvError::Closed) => Some(()),
            _ => None,
        });
        assert!(
            !history.exists(),
            "late readiness initialized history after final-owner shutdown"
        );
    }
}
