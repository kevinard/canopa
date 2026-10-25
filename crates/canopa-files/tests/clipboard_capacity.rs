use canopa_files::{
    file_clipboard::Selection,
    operation_jobs::{OperationJobs, Phase, PreparedSelection, Request},
    storage::Storage,
    transfers::{TransferKind, TransferRequest},
};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < until, "clipboard runtime did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn prepare(
    jobs: &OperationJobs,
    storage: &std::sync::Arc<Storage>,
    paths: &[PathBuf],
    previous: Option<&PreparedSelection>,
) -> Result<PreparedSelection, String> {
    let selection = Selection::new(paths.to_vec(), TransferKind::Copy).unwrap();
    let mut reply = match previous {
        Some(previous) => jobs.replace_clipboard(previous.clone(), selection, storage.snapshot()),
        None => jobs.prepare_clipboard(selection, storage.snapshot()),
    }
    .unwrap();
    wait_for(|| reply.try_recv().unwrap())
}

#[test]
fn replacing_a_full_owned_selection_preserves_both_intents_and_restores_the_normal_budget() {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "clipboard_replacement_budget_helper",
            "--test-threads=1",
        ])
        .status()
        .unwrap();
    assert!(
        status.success(),
        "isolated clipboard replacement acceptance failed"
    );
}

#[test]
#[ignore = "private child with a reduced descriptor limit"]
fn clipboard_replacement_budget_helper() {
    use rustix::process::{Resource, getrlimit, setrlimit};
    let mut limit = getrlimit(Resource::Nofile);
    limit.current = Some(128);
    setrlimit(Resource::Nofile, limit).unwrap();
    let root = tempfile::tempdir().unwrap();
    let first: Vec<_> = (0..32)
        .map(|index| root.path().join(format!("original-{index}")))
        .collect();
    let second: Vec<_> = (0..33)
        .map(|index| root.path().join(format!("replacement-{index}")))
        .collect();
    for path in &first {
        fs::write(path, b"Original selected bytes").unwrap();
    }
    for path in &second {
        fs::write(path, b"Replacement selected bytes").unwrap();
    }
    let marker = root.path().join("marker");
    fs::write(&marker, b"Deliberate marker bytes").unwrap();
    let storage = Storage::load(root.path().join("state/volumes")).unwrap();
    let jobs =
        OperationJobs::start_with_storage(root.path().join("state/history"), storage.clone())
            .unwrap();
    wait_for(|| (!jobs.snapshot().initializing).then_some(()));
    assert!(jobs.snapshot().error.is_none());
    let original = prepare(&jobs, &storage, &first, None).unwrap();
    let old_clone = original.clone();
    assert!(prepare(&jobs, &storage, std::slice::from_ref(&marker), None).is_err());
    let replacement = prepare(&jobs, &storage, &second[..32], Some(&original)).unwrap();
    assert_eq!(original.snapshot().selection.unwrap().paths(), first);
    assert_eq!(
        replacement.snapshot().selection.unwrap().paths(),
        &second[..32]
    );
    // Independent callers do not inherit temporary replacement capacity.
    assert!(
        TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![marker.clone()],
            destination: root.path().to_owned(),
        }
        .observe_sources()
        .is_err()
    );

    // The old opaque intent still protects the actual original while both
    // full selections are alive. Paste must refuse its replaced pathname.
    let parked = root.path().join("parked-original");
    fs::rename(&first[0], &parked).unwrap();
    fs::write(&first[0], b"Unrelated later file").unwrap();
    let destination = root.path().join("old-paste");
    fs::create_dir(&destination).unwrap();
    let mut reply = jobs
        .paste_clipboard(old_clone.clone(), destination.clone(), None)
        .unwrap();
    let id = wait_for(|| reply.try_recv().unwrap()).unwrap();
    let report = wait_for(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Stopped)
            .and_then(|job| job.transfer)
    });
    assert_eq!(report.completed, 31);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].source, first[0]);
    assert!(!destination.join("original-0").exists());
    assert_eq!(fs::read(&parked).unwrap(), b"Original selected bytes");
    assert_eq!(fs::read(&first[0]).unwrap(), b"Unrelated later file");
    drop(original);
    drop(old_clone);

    // The stopped transfer now owns its exact executable Retry independently
    // of clipboard transport. Only explicit abandonment may release these pins.
    assert!(prepare(&jobs, &storage, std::slice::from_ref(&marker), None).is_err());
    let mut release = jobs.abandon_transfer(id).unwrap();
    wait_for(|| release.try_recv().unwrap()).unwrap();

    // Refusal releases every partial reservation and leaves the previous
    // intent intact, allowing an immediate deliberate smaller replacement.
    assert!(prepare(&jobs, &storage, &second, Some(&replacement)).is_err());
    assert_eq!(
        replacement.snapshot().selection.unwrap().paths(),
        &second[..32]
    );
    let latest = prepare(
        &jobs,
        &storage,
        std::slice::from_ref(&marker),
        Some(&replacement),
    )
    .unwrap();
    assert!(prepare(&jobs, &storage, std::slice::from_ref(&marker), None).is_err());
    drop(replacement);
    let ordinary = prepare(&jobs, &storage, &second[..31], None).unwrap();
    assert!(prepare(&jobs, &storage, &second[31..32], None).is_err());
    drop(ordinary);

    // Resource credit belongs to the originating runtime, even though both
    // instances share the process-wide descriptor budget.
    let other =
        OperationJobs::start_with_storage(root.path().join("other/history"), storage.clone())
            .unwrap();
    wait_for(|| (!other.snapshot().initializing).then_some(()));
    let error = prepare(&other, &storage, &second[..1], Some(&latest)).unwrap_err();
    assert!(error.contains("another runtime"), "{error}");

    let destination = root.path().join("latest-paste");
    fs::create_dir(&destination).unwrap();
    let mut reply = jobs
        .paste_clipboard(latest.clone(), destination.clone(), None)
        .unwrap();
    let id = wait_for(|| reply.try_recv().unwrap()).unwrap();
    wait_for(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Finished)
    });
    assert_eq!(
        fs::read(destination.join("marker")).unwrap(),
        b"Deliberate marker bytes"
    );
    let mut reply = jobs
        .submit(
            Request::Undo {
                expected: jobs.snapshot().undo.unwrap().id,
            },
            None,
        )
        .unwrap();
    let id = wait_for(|| reply.try_recv().unwrap()).unwrap();
    wait_for(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Finished)
    });
    assert!(!destination.join("marker").exists());
    assert_eq!(fs::read(&marker).unwrap(), b"Deliberate marker bytes");
    for path in &second {
        assert_eq!(fs::read(path).unwrap(), b"Replacement selected bytes");
    }
    drop(latest);
    let normal = prepare(&jobs, &storage, &first, None).unwrap();
    assert!(prepare(&jobs, &storage, std::slice::from_ref(&marker), None).is_err());
    drop(normal);
}
