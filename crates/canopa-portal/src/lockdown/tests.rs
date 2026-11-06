use super::*;
use futures_util::FutureExt;

#[tokio::test]
async fn consumers_remember_a_denial_even_when_reenable_coalesces_the_watch_update() {
    let root = tempfile::tempdir().unwrap();
    let policy = Lockdown::open(Source::file(root.path().join("policy.sqlite3"), false).unwrap())
        .await
        .unwrap();
    let mut changes = policy.0.changes.lock().unwrap().take().unwrap();
    let observed = policy.observe();
    let initial = *observed.borrow();
    for value in [true, false] {
        let update = policy.request(Operation::Set(Property::Printing, value));
        tokio::pin!(update);
        assert!(update.as_mut().now_or_never().is_none());
        changes
            .recv()
            .await
            .unwrap()
            .acknowledged
            .send(Ok(()))
            .unwrap();
        update.await.unwrap();
    }
    let latest = *observed.borrow();
    assert!(!latest.disabled(Property::Printing));
    assert!(!latest.permits(&initial, Property::Printing));
    assert!(latest.permits(&initial, Property::SaveToDisk));
    assert!(latest.permits(&latest, Property::Printing));
    policy.shutdown().await.unwrap();
    assert!(observed.has_changed().is_err());
}

#[test]
fn revision_exhaustion_stays_closed_until_a_new_policy_lifetime() {
    let mut before = Snapshot::initial([false; 7]);
    before.revisions[Property::Printing as usize] = Some(u64::MAX);
    let mut disabled = [false; 7];
    disabled[Property::Printing as usize] = true;
    let after = before.changed(disabled).changed([false; 7]);
    assert!(after.disabled(Property::Printing));
    assert!(!after.permits(&after, Property::Printing));
    assert!(!after.disabled(Property::SaveToDisk));
}

#[tokio::test]
async fn publication_and_durable_mutation_survive_a_cancelled_request() {
    let root = tempfile::tempdir().unwrap();
    let source = Source::file(root.path().join("policy.sqlite3"), false).unwrap();
    let policy = Lockdown::open(source.clone()).await.unwrap();
    let mut changes = policy.0.changes.lock().unwrap().take().unwrap();
    let client = policy.clone();
    let task =
        tokio::spawn(async move { client.request(Operation::Set(Property::Camera, true)).await });
    let change = changes.recv().await.unwrap();
    assert!(!task.is_finished(), "reply must wait for publication");
    assert!(!change.before[Property::Camera as usize] && change.after[Property::Camera as usize]);
    task.abort();
    let _ = task.await;
    change.acknowledged.send(Ok(())).unwrap();
    assert!(store::Store::open(source).unwrap().values()[Property::Camera as usize]);
    policy.reload().await.unwrap();
    assert!(changes.try_recv().is_err(), "no-op reload must not publish");
    policy.shutdown().await.unwrap();
}

#[tokio::test]
async fn bounded_admission_and_shutdown_keep_accepted_work_ordered() {
    let root = tempfile::tempdir().unwrap();
    let policy = Lockdown::open(Source::file(root.path().join("policy.sqlite3"), false).unwrap())
        .await
        .unwrap();
    let mut changes = policy.0.changes.lock().unwrap().take().unwrap();
    let mut first = Box::pin(policy.request(Operation::Set(Property::Printing, true)));
    assert!(first.as_mut().now_or_never().is_none());
    let change = changes.recv().await.unwrap();
    let mut queued = Vec::new();
    for _ in 0..CAPACITY {
        let mut request = Box::pin(policy.reload());
        assert!(request.as_mut().now_or_never().is_none());
        queued.push(request);
    }
    assert_eq!(policy.reload().await, Err(Error::Capacity));
    let mut shutdown = Box::pin(policy.shutdown());
    assert!(shutdown.as_mut().now_or_never().is_none());
    assert_eq!(
        policy.request(Operation::Set(Property::Camera, true)).await,
        Err(Error::Stopped)
    );
    change.acknowledged.send(Ok(())).unwrap();
    first.await.unwrap();
    for request in queued {
        request.await.unwrap();
    }
    shutdown.await.unwrap();
    assert!(policy.values()[Property::Printing as usize]);
    assert!(!policy.values()[Property::Camera as usize]);
}
