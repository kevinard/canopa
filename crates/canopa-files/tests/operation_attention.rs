use canopa_files::{
    operation_jobs::{AttentionStatus, OperationJobs, Phase, Request},
    operations::{OperationRequest, UndoChoice},
};
use std::time::{Duration, Instant};

fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < deadline, "attention did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn completion_snapshots_never_expose_more_than_one_hundred_finished_rows() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let root = tempfile::tempdir().unwrap();
    let service = OperationJobs::start(
        root.path().join("history.sqlite3"),
        root.path().join("volumes.json"),
    )
    .unwrap();
    let done = Arc::new(AtomicBool::new(false));
    struct StopReader(Arc<AtomicBool>);
    impl Drop for StopReader {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let stop_reader = StopReader(done.clone());
    let observed = service.clone();
    let stopping = done.clone();
    let reader = std::thread::spawn(move || {
        while !stopping.load(Ordering::Acquire) {
            let state = observed.snapshot();
            let finished: Vec<_> = state
                .jobs
                .iter()
                .filter(|job| job.phase == Phase::Finished)
                .map(|job| job.id)
                .collect();
            if finished.len() > 100 {
                return Some(finished);
            }
            std::thread::yield_now();
        }
        None
    });
    for index in 0..104 {
        let mut admission = service
            .submit(
                Request::Mutation(OperationRequest::CreateFolder {
                    path: root.path().join(format!("Folder {index}")),
                }),
                None,
            )
            .unwrap();
        let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
        wait_for(|| {
            service
                .snapshot()
                .jobs
                .iter()
                .any(|job| job.id == id && job.phase == Phase::Finished)
                .then_some(())
        });
    }
    drop(stop_reader);
    let violation = reader.join().unwrap();
    assert!(
        violation.is_none(),
        "Published too many finished rows: {violation:?}"
    );
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    drop(service);
    let reopened = OperationJobs::start(
        root.path().join("history.sqlite3"),
        root.path().join("volumes.json"),
    )
    .unwrap();
    wait_for(|| (!reopened.snapshot().initializing).then_some(()));
    assert_eq!(
        reopened
            .snapshot()
            .jobs
            .iter()
            .filter(|job| job.phase == Phase::Finished)
            .count(),
        100,
        "reopening applies the same finished-row bound"
    );
}

#[test]
fn decision_receipts_keep_their_generation_and_cannot_release_a_later_outcome() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("state/history.sqlite3");
    let volumes = root.path().join("state/volumes.json");
    let service = OperationJobs::start(history.clone(), volumes.clone()).unwrap();
    let mut attention = service.observe_attention().unwrap();
    drop(attention.take_notices());
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
        service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == created && job.phase == Phase::Finished)
            .then_some(())
    });
    for event in attention.take_pending() {
        attention.acknowledge(event);
    }
    std::fs::write(folder.join("later.txt"), b"Keep these later bytes").unwrap();
    let mut admission = service
        .submit(
            Request::Undo {
                expected: service.snapshot().undo.unwrap().id,
            },
            None,
        )
        .unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    let generation = wait_for(|| {
        service
            .snapshot()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .and_then(|job| job.decision.as_ref())
            .map(|decision| decision.generation)
    });
    let events = attention.take_pending();
    assert_eq!(events.len(), 1);
    let question = events.into_iter().next().unwrap();
    assert_eq!(question.id, id);
    assert!(
        matches!(question.status, AttentionStatus::Decision { generation: current } if current == generation)
    );
    service
        .resolve(id, generation + 1, UndoChoice::KeepItem)
        .unwrap();
    let mut shutdown = service.shutdown().unwrap();
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    let snapshot = service.snapshot();
    let job = snapshot.jobs.iter().find(|job| job.id == id).unwrap();
    assert_eq!(job.phase, Phase::Waiting);
    assert_eq!(job.decision.as_ref().unwrap().generation, generation);
    service
        .resolve(id, generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == id && job.phase == Phase::Finished)
            .then_some(())
    });
    assert_eq!(
        std::fs::read(folder.join("later.txt")).unwrap(),
        b"Keep these later bytes"
    );
    assert_eq!(service.snapshot().pending_attention, 2);
    attention.acknowledge(question);
    assert_eq!(service.snapshot().pending_attention, 1);
    assert!(service.snapshot().holds_application());
    let events = attention.take_pending();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, id);
    assert!(matches!(
        events[0].status,
        AttentionStatus::Outcome {
            phase: Phase::Finished,
            ..
        }
    ));
    attention.acknowledge(events.into_iter().next().unwrap());
    assert!(!service.snapshot().holds_application());
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    let reopened = OperationJobs::start(history, volumes).unwrap();
    let _observer = reopened.observe_attention().unwrap();
    wait_for(|| (!reopened.snapshot().initializing).then_some(()));
    assert_eq!(
        reopened.snapshot().pending_attention,
        0,
        "historical results must not be announced again"
    );
    assert_eq!(
        std::fs::read(folder.join("later.txt")).unwrap(),
        b"Keep these later bytes"
    );
    let mut shutdown = reopened.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn attention_survives_row_trimming_and_bounds_admission_and_concurrent_delivery() {
    let root = tempfile::tempdir().unwrap();
    let service = OperationJobs::start(
        root.path().join("state/history.sqlite3"),
        root.path().join("state/volumes.json"),
    )
    .unwrap();
    let mut attention = service.observe_attention().unwrap();
    drop(attention.take_notices());
    let mut expected = std::collections::BTreeSet::new();
    let mut last = 0;
    for index in 0..1024 {
        let mut admission = service
            .submit(
                Request::Mutation(OperationRequest::CreateFolder {
                    path: root.path().join(format!("Folder {index}")),
                }),
                None,
            )
            .unwrap();
        last = wait_for(|| admission.try_recv().unwrap()).unwrap();
        assert!(expected.insert(last));
        // Fill attention with completed results, rather than making a thousand
        // serial filesystem operations share one short completion deadline.
        wait_for(|| {
            service
                .snapshot()
                .jobs
                .iter()
                .any(|job| job.id == last && job.phase == Phase::Finished)
                .then_some(())
        });
    }
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == last && job.phase == Phase::Finished)
            .then_some(())
    });
    let snapshot = service.snapshot();
    assert_eq!(snapshot.pending_attention, 1024);
    assert_eq!(snapshot.jobs.len(), 100);
    assert!(root.path().join("Folder 0").is_dir());
    assert!(root.path().join("Folder 1023").is_dir());
    let refused = root.path().join("Refused while attention is full");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: refused.clone(),
            }),
            None,
        )
        .unwrap();
    assert!(
        wait_for(|| admission.try_recv().unwrap())
            .unwrap_err()
            .contains("attention queue is full")
    );
    assert!(!refused.exists());
    let mut first_batch = attention.take_pending();
    assert_eq!(first_batch.len(), 17);
    assert!(attention.take_pending().is_empty());
    assert_eq!(service.snapshot().pending_attention, 1024);
    let first = first_batch.remove(0);
    assert_eq!(first.title, "create folder Folder 0");
    let mut delivered = std::collections::BTreeSet::from([first.id]);
    attention.acknowledge(first);
    let admitted = root.path().join("Admitted after acknowledgement");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: admitted.clone(),
            }),
            None,
        )
        .unwrap();
    last = wait_for(|| admission.try_recv().unwrap()).unwrap();
    expected.insert(last);
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == last && job.phase == Phase::Finished)
            .then_some(())
    });
    assert!(admitted.is_dir());
    for event in first_batch {
        assert!(delivered.insert(event.id));
        attention.acknowledge(event);
    }
    while service.snapshot().pending_attention > 0 {
        let batch = attention.take_pending();
        assert!(!batch.is_empty() && batch.len() <= 17);
        for event in batch {
            assert!(delivered.insert(event.id));
            attention.acknowledge(event);
        }
    }
    assert_eq!(delivered, expected);
    assert!(!service.snapshot().holds_application());
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn completion_attention_protects_shutdown_through_the_delivery_handoff() {
    let root = tempfile::tempdir().unwrap();
    let service = OperationJobs::start(
        root.path().join("state/history.sqlite3"),
        root.path().join("state/volumes.json"),
    )
    .unwrap();
    let mut attention = service.observe_attention().unwrap();
    assert!(service.observe_attention().is_err());
    assert!(service.take_notices().is_none());
    drop(attention.take_notices());
    let folder = root.path().join("Created without a frontend");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: folder.clone(),
            }),
            None,
        )
        .unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == id && job.phase == Phase::Finished)
            .then_some(())
    });
    assert!(folder.is_dir());
    assert!(service.snapshot().undo.is_some());
    assert_eq!(service.snapshot().pending_attention, 1);
    assert!(service.snapshot().holds_application());
    let mut shutdown = service.shutdown().unwrap();
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    let events = attention.take_pending();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, id);
    assert_eq!(events[0].title, "create folder Created without a frontend");
    assert!(matches!(
        events[0].status,
        AttentionStatus::Outcome {
            phase: Phase::Finished,
            ..
        }
    ));
    assert!(attention.take_pending().is_empty());
    assert!(
        service.snapshot().holds_application(),
        "taking a notice must retain its hold until delivery is acknowledged"
    );
    let mut shutdown = service.shutdown().unwrap();
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    attention.acknowledge(events.into_iter().next().unwrap());
    assert!(!service.snapshot().holds_application());
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn acknowledging_save_failure_attention_does_not_release_the_unsaved_results() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("state/history.sqlite3");
    let service =
        OperationJobs::start(history.clone(), root.path().join("state/volumes.json")).unwrap();
    let mut attention = service.observe_attention().unwrap();
    drop(attention.take_notices());
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
        service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == created && job.phase == Phase::Finished)
            .then_some(())
    });
    for event in attention.take_pending() {
        attention.acknowledge(event);
    }
    std::fs::write(
        folder.join("later.txt"),
        b"Unsaved result still protects these bytes",
    )
    .unwrap();
    let mut admission = service
        .submit(
            Request::Undo {
                expected: service.snapshot().undo.unwrap().id,
            },
            None,
        )
        .unwrap();
    let undo = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == undo && job.phase == Phase::Waiting)
            .then_some(())
    });
    for event in attention.take_pending() {
        attention.acknowledge(event);
    }
    let target = root.path().join("Cancelled before execution");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: target.clone(),
            }),
            None,
        )
        .unwrap();
    let queued = wait_for(|| admission.try_recv().unwrap()).unwrap();
    // Make the actual private history path unavailable; assertions stay on the
    // public queue, Undo outcome and files, without reading database internals.
    let moved = history.with_extension("temporarily-moved");
    std::fs::rename(&history, &moved).unwrap();
    service.cancel(queued).unwrap();
    service.cancel(undo).unwrap();
    wait_for(|| {
        let snapshot = service.snapshot();
        [queued, undo]
            .iter()
            .all(|id| {
                snapshot
                    .jobs
                    .iter()
                    .any(|job| job.id == *id && job.phase == Phase::SaveFailed)
            })
            .then_some(())
    });
    let events = attention.take_pending();
    assert_eq!(events.len(), 2);
    for event in events {
        assert!(
            matches!(&event.status, AttentionStatus::Outcome { phase: Phase::SaveFailed, detail } if detail.contains("Result not saved"))
        );
        attention.acknowledge(event);
    }
    let snapshot = service.snapshot();
    assert_eq!(snapshot.pending_attention, 0);
    assert!(
        !snapshot
            .jobs
            .iter()
            .any(|job| matches!(job.phase, Phase::Queued | Phase::Running | Phase::Waiting))
    );
    assert!(
        snapshot.holds_application(),
        "delivered attention is not a saved filesystem result"
    );
    let mut shutdown = service.shutdown().unwrap();
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    std::fs::rename(moved, history).unwrap();
    service.retry_save(queued).unwrap();
    service.retry_save(undo).unwrap();
    wait_for(|| {
        let unsaved = service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.phase.needs_save());
        (!unsaved).then_some(())
    });
    for event in attention.take_pending() {
        attention.acknowledge(event);
    }
    assert!(!service.snapshot().holds_application());
    assert!(
        !target.exists(),
        "saving the stopped result must not execute the cancelled request"
    );
    assert_eq!(
        std::fs::read(folder.join("later.txt")).unwrap(),
        b"Unsaved result still protects these bytes"
    );
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}
