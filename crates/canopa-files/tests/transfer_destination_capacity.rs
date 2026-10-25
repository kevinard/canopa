use canopa_files::{
    Cancellation,
    operations::OperationEngine,
    transfers::{TransferKind, TransferReply, TransferRequest},
};
use std::fs;

#[test]
fn destination_observations_are_bounded_shared_and_released_with_their_last_owner() {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "destination_capacity_helper",
            "--test-threads=1",
        ])
        .status()
        .unwrap();
    assert!(
        status.success(),
        "isolated destination capacity acceptance failed"
    );
}

#[test]
#[ignore = "private child with a reduced descriptor limit"]
fn destination_capacity_helper() {
    use rustix::process::{Resource, getrlimit, setrlimit};
    let mut limit = getrlimit(Resource::Nofile);
    limit.current = Some(128);
    setrlimit(Resource::Nofile, limit).unwrap();
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::write(&source, b"Original source bytes").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let mut plans = Vec::new();
    let mut clones = Vec::new();
    for index in 0..8 {
        let destination = root.path().join(format!("destination-{index}"));
        fs::create_dir(&destination).unwrap();
        let outcome = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![source.clone()],
                    destination: destination.clone(),
                },
                &cancelled,
                |_| panic!("cancelled admission must not execute"),
            )
            .unwrap();
        let plan = outcome.continuation.unwrap();
        if index == 0 {
            clones.extend((0..20).map(|_| plan.clone()));
        }
        plans.push(plan);
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    }
    let destination = root.path().join("next destination");
    fs::create_dir(&destination).unwrap();
    let next = TransferRequest {
        kind: TransferKind::Copy,
        sources: vec![source.clone()],
        destination: destination.clone(),
    };
    let error = next.observe_sources().unwrap_err();
    assert!(error.to_string().contains("destinations"), "{error}");
    drop(plans.remove(0));
    assert!(
        next.observe_sources().is_err(),
        "clones must retain their original pin"
    );
    drop(clones);
    // The released slot supports repeated new work: neither a failed capture
    // nor a completed transfer leaks its reservation or changes the old plans.
    for _ in 0..3 {
        let outcome = engine
            .transfer(&next, &Cancellation::default(), |_| TransferReply::Continue)
            .unwrap();
        assert_eq!(outcome.completed, 1);
        assert_eq!(
            fs::read(destination.join("source")).unwrap(),
            b"Original source bytes"
        );
        engine.undo(&Cancellation::default()).unwrap();
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    }
    assert_eq!(fs::read(&source).unwrap(), b"Original source bytes");
    assert_eq!(plans.len(), 7);
}
