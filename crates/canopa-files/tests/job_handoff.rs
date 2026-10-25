use canopa_files::{
    Cancellation,
    operations::{OperationEngine, requests::RequestPhase},
    transfers::{
        TransferContinuation, TransferEvent, TransferKind, TransferReply, TransferRequest,
    },
};
use serde::{Deserialize, Serialize};
use std::{fs, io, os::unix::fs::MetadataExt};

#[derive(Serialize, Deserialize)]
struct Plans {
    retry: Option<TransferContinuation>,
    remaining: Option<TransferContinuation>,
}

#[test]
fn retry_handoff_survives_reopen_and_keeps_the_other_plan_and_each_completed_prefix() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("project");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("a-completed"), b"completed before pause").unwrap();
    let failed = source.join("b-retry");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &failed,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let interrupted = source.join("c-continue");
    fs::write(&interrupted, vec![b'C'; 256 * 1024]).unwrap();
    fs::write(source.join("d-continue"), b"later remaining work").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let outcome = engine.transfer(&TransferRequest { kind: TransferKind::Copy, sources: vec![source.clone()], destination: destination.clone() }, &Cancellation::default(), |event| {
        if matches!(event, TransferEvent::Progress { path, bytes, .. } if path == interrupted && bytes > 0) { TransferReply::Cancel } else { TransferReply::Continue }
    }).unwrap();
    assert!(outcome.cancelled);
    assert_eq!(outcome.failures.len(), 1);
    let retry = outcome.retry_continuation.unwrap();
    let remaining = outcome.continuation.unwrap();
    let output = destination.join("project");
    assert!(!output.join("c-continue").exists());
    let completed_inode = fs::metadata(output.join("a-completed")).unwrap().ino();
    let accepted = engine.queue_request(&"original request").unwrap();
    let stopped = engine
        .checkpoint_request(
            &accepted,
            RequestPhase::FailedWithRemaining,
            &Plans {
                retry: Some(retry.clone()),
                remaining: Some(remaining.clone()),
            },
        )
        .unwrap();
    let (old, queued) = engine
        .handoff_request(
            &stopped,
            RequestPhase::Remaining,
            &Plans {
                retry: None,
                remaining: Some(remaining),
            },
            &retry,
        )
        .unwrap();
    assert!(queued.id() > old.id());
    assert_eq!(queued.phase(), RequestPhase::Queued);
    assert!(
        engine
            .handoff_request(&stopped, RequestPhase::Finished, &"stale", &retry)
            .is_err()
    );
    drop(engine);
    fs::remove_file(&failed).unwrap();
    fs::write(&failed, b"repaired failed work").unwrap();

    let engine = OperationEngine::open(&history).unwrap();
    let rows = engine.request_page(None).unwrap();
    assert_eq!(rows.len(), 2);
    let queued = rows.iter().find(|row| row.id() == queued.id()).unwrap();
    let old = rows.iter().find(|row| row.id() == old.id()).unwrap();
    let plans: Plans = old.payload().unwrap();
    assert!(plans.retry.is_none());
    let remaining = plans.remaining.unwrap();
    let retry: TransferContinuation = queued.payload().unwrap();
    let running = engine
        .checkpoint_request(queued, RequestPhase::Running, &retry)
        .unwrap();
    let copied = engine
        .resume_transfer(&retry, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(copied.failures.is_empty());
    assert_eq!(
        fs::read(output.join("b-retry")).unwrap(),
        b"repaired failed work"
    );
    assert!(!output.join("d-continue").exists());
    assert_eq!(
        fs::metadata(output.join("a-completed")).unwrap().ino(),
        completed_inode
    );
    engine
        .checkpoint_request(&running, RequestPhase::Finished, &"Retry finished")
        .unwrap();

    let (_, queued) = engine
        .handoff_request(
            old,
            RequestPhase::Finished,
            &Plans {
                retry: None,
                remaining: None,
            },
            &remaining,
        )
        .unwrap();
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let rows = engine.request_page(None).unwrap();
    let queued = rows.iter().find(|row| row.id() == queued.id()).unwrap();
    let remaining: TransferContinuation = queued.payload().unwrap();
    let running = engine
        .checkpoint_request(queued, RequestPhase::Running, &remaining)
        .unwrap();
    let copied = engine
        .resume_transfer(&remaining, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(copied.failures.is_empty());
    engine
        .checkpoint_request(&running, RequestPhase::Finished, &"Continue finished")
        .unwrap();
    assert_eq!(
        fs::read(output.join("c-continue")).unwrap(),
        vec![b'C'; 256 * 1024]
    );
    assert_eq!(
        fs::read(output.join("d-continue")).unwrap(),
        b"later remaining work"
    );
    assert_eq!(
        fs::metadata(output.join("a-completed")).unwrap().ino(),
        completed_inode
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!output.join("c-continue").exists());
    assert!(!output.join("d-continue").exists());
    assert_eq!(
        fs::read(output.join("b-retry")).unwrap(),
        b"repaired failed work"
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!output.join("b-retry").exists());
    assert_eq!(
        fs::read(output.join("a-completed")).unwrap(),
        b"completed before pause"
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!output.exists());
}

#[test]
fn a_rejected_handoff_keeps_the_old_revision_and_both_plans() {
    let root = tempfile::tempdir().unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let accepted = engine.queue_request(&"original").unwrap();
    let stopped = engine
        .checkpoint_request(
            &accepted,
            RequestPhase::FailedWithRemaining,
            &"both exact plans",
        )
        .unwrap();
    for index in 1..1024 {
        engine.queue_request(&index).unwrap();
    }
    let refused = engine
        .handoff_request(
            &stopped,
            RequestPhase::Remaining,
            &"unselected plan",
            &"selected plan",
        )
        .err()
        .unwrap();
    assert_eq!(refused.kind(), io::ErrorKind::StorageFull);
    let mut before = None;
    let mut rows = Vec::new();
    loop {
        let page = engine.request_page(before).unwrap();
        if page.is_empty() {
            break;
        }
        before = page.last().map(|row| row.id());
        rows.extend(page);
    }
    assert_eq!(rows.len(), 1024);
    let original = rows.iter().find(|row| row.id() == stopped.id()).unwrap();
    assert_eq!(original.phase(), RequestPhase::FailedWithRemaining);
    assert_eq!(original.payload::<String>().unwrap(), "both exact plans");
    // The failed transaction has not consumed its revision. Moving all work to
    // the successor fits the same unfinished-row budget and can still succeed.
    let (_, successor) = engine
        .handoff_request(
            &stopped,
            RequestPhase::Finished,
            &"summary",
            &"both exact plans",
        )
        .unwrap();
    assert_eq!(successor.payload::<String>().unwrap(), "both exact plans");
}

#[test]
fn active_finished_oversized_and_foreign_handoffs_cannot_create_new_work() {
    let root = tempfile::tempdir().unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let accepted = engine.queue_request(&"original").unwrap();
    let excessive = "x".repeat(4 * 1024 * 1024);
    assert!(
        engine
            .handoff_request(&accepted, RequestPhase::Finished, &"summary", &excessive)
            .is_err()
    );
    assert!(
        engine
            .handoff_request(&accepted, RequestPhase::Remaining, &excessive, &"new")
            .is_err()
    );
    assert!(
        engine
            .handoff_request(&accepted, RequestPhase::Running, &"active old", &"new")
            .is_err()
    );
    let other = OperationEngine::open(root.path().join("other/history")).unwrap();
    let foreign = other.queue_request(&"foreign").unwrap();
    assert_eq!(accepted.id(), foreign.id());
    assert!(
        engine
            .handoff_request(
                &foreign,
                RequestPhase::Finished,
                &"summary",
                &"wrong history"
            )
            .is_err()
    );
    let active = engine
        .checkpoint_request(&accepted, RequestPhase::Running, &"active")
        .unwrap();
    assert!(
        engine
            .handoff_request(&active, RequestPhase::Finished, &"summary", &"duplicate")
            .is_err()
    );
    let finished = engine
        .checkpoint_request(&active, RequestPhase::Finished, &"finished")
        .unwrap();
    assert!(
        engine
            .handoff_request(&finished, RequestPhase::Finished, &"summary", &"replay")
            .is_err()
    );
    assert_eq!(engine.request_page(None).unwrap().len(), 1);
}
