use canopa_files::{
    Cancellation,
    operations::{
        OperationEngine, OperationRequest,
        requests::{RequestPhase, RequestRecord},
    },
    transfers::{ConflictChoice, TransferEvent, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs, io,
    sync::{Arc, mpsc},
    time::Duration,
};

fn all(engine: &OperationEngine) -> Vec<RequestRecord> {
    let mut result = Vec::new();
    let mut before = None;
    loop {
        let page = engine.request_page(before).unwrap();
        if page.is_empty() {
            return result;
        }
        before = page.last().map(RequestRecord::id);
        result.extend(page);
    }
}

#[test]
fn reopening_keeps_each_phase_without_executing_queued_or_running_requests() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("state/history");
    let output = root.path().join("never-auto-run");
    let engine = OperationEngine::open(&history).unwrap();
    let request = OperationRequest::CreateFolder {
        path: output.clone(),
    };
    let phases = [
        RequestPhase::Queued,
        RequestPhase::Running,
        RequestPhase::Failed,
        RequestPhase::Remaining,
        RequestPhase::FailedWithRemaining,
        RequestPhase::Finished,
    ];
    for phase in phases {
        let record = engine.queue_request(&request).unwrap();
        if phase != RequestPhase::Queued {
            engine.checkpoint_request(&record, phase, &request).unwrap();
        }
    }
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let records = all(&engine);
    assert_eq!(
        records.iter().map(RequestRecord::phase).collect::<Vec<_>>(),
        phases.into_iter().rev().collect::<Vec<_>>()
    );
    for record in records {
        let OperationRequest::CreateFolder { path } = record.payload().unwrap() else {
            panic!("original request changed")
        };
        assert_eq!(path, output);
    }
    assert!(!output.exists());
    assert!(engine.latest_undo_id().is_none());
}

#[test]
fn stale_checkpoints_cannot_replace_newer_plans_or_resurrect_dismissed_work() {
    let root = tempfile::tempdir().unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let first = engine.queue_request(&"original").unwrap();
    let newer = engine
        .checkpoint_request(&first, RequestPhase::Failed, &"exact remaining plan")
        .unwrap();
    assert_eq!(
        engine
            .checkpoint_request(&first, RequestPhase::Finished, &"stale")
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(
        engine.dismiss_request(&first).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(
        all(&engine)[0].payload::<String>().unwrap(),
        "exact remaining plan"
    );
    engine.dismiss_request(&newer).unwrap();
    assert!(
        engine
            .checkpoint_request(&newer, RequestPhase::Failed, &"revived")
            .is_err()
    );
    assert!(all(&engine).is_empty());
    let next = engine.queue_request(&"new selection").unwrap();
    assert!(next.id() > newer.id());
    assert!(
        engine
            .checkpoint_request(&newer, RequestPhase::Finished, &"late result")
            .is_err()
    );
    assert_eq!(
        all(&engine)[0].payload::<String>().unwrap(),
        "new selection"
    );

    let other = OperationEngine::open(root.path().join("other/history")).unwrap();
    other.queue_request(&"earlier other request").unwrap();
    let other_record = other.queue_request(&"another history").unwrap();
    assert_eq!(other_record.id(), next.id());
    assert!(
        engine
            .checkpoint_request(&other_record, RequestPhase::Finished, &"wrong history")
            .is_err()
    );
    assert!(engine.dismiss_request(&other_record).is_err());
    let running = engine
        .checkpoint_request(&next, RequestPhase::Running, &"active")
        .unwrap();
    assert_eq!(
        engine.dismiss_request(&running).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    let finished = engine
        .checkpoint_request(&running, RequestPhase::Finished, &"completed")
        .unwrap();
    assert!(
        engine
            .checkpoint_request(&finished, RequestPhase::Queued, &"repeat")
            .is_err()
    );
}

#[test]
fn keeping_one_hundred_finished_results_never_evicts_older_unfinished_work() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let unfinished: Vec<_> = (0..7)
        .map(|index| {
            engine
                .queue_request(&format!("unfinished {index}"))
                .unwrap()
                .id()
        })
        .collect();
    for index in 0..110 {
        let record = engine.queue_request(&format!("finished {index}")).unwrap();
        engine
            .checkpoint_request(
                &record,
                RequestPhase::Finished,
                &format!("finished {index}"),
            )
            .unwrap();
    }
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let records = all(&engine);
    assert_eq!(records.len(), 107);
    let finished: Vec<_> = records
        .iter()
        .filter(|record| record.phase() == RequestPhase::Finished)
        .collect();
    assert_eq!(finished.len(), 100);
    assert_eq!(finished[0].payload::<String>().unwrap(), "finished 109");
    assert_eq!(finished[99].payload::<String>().unwrap(), "finished 10");
    for id in unfinished {
        assert!(
            records
                .iter()
                .any(|record| record.id() == id && record.phase() == RequestPhase::Queued)
        );
    }
}

#[test]
fn oversized_acceptance_and_checkpoints_leave_previous_requests_intact() {
    let root = tempfile::tempdir().unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let original = engine.queue_request(&"actual plan").unwrap();
    let excessive = "x".repeat(4 * 1024 * 1024);
    assert!(engine.queue_request(&excessive).is_err());
    assert!(
        engine
            .checkpoint_request(&original, RequestPhase::Failed, &excessive)
            .is_err()
    );
    let records = all(&engine);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].payload::<String>().unwrap(), "actual plan");
    engine
        .checkpoint_request(&original, RequestPhase::Finished, &"saved result")
        .unwrap();
}

#[test]
fn the_total_payload_budget_rolls_back_acceptance_and_allows_a_smaller_checkpoint() {
    let root = tempfile::tempdir().unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    // A JSON string adds exactly two quote bytes to this ASCII payload.
    let payload = "x".repeat(4 * 1024 * 1024 - 2);
    let first = engine.queue_request(&payload).unwrap();
    for _ in 1..16 {
        engine.queue_request(&payload).unwrap();
    }
    assert_eq!(
        engine
            .queue_request(&"over total budget")
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::StorageFull
    );
    assert_eq!(all(&engine).len(), 16);
    engine
        .checkpoint_request(&first, RequestPhase::Failed, &"smaller exact plan")
        .unwrap();
    engine
        .queue_request(&"new work after freeing space")
        .unwrap();
    assert_eq!(all(&engine).len(), 17);
}

#[test]
fn the_unfinished_budget_rejects_new_work_and_recovers_after_explicit_dismissal() {
    let root = tempfile::tempdir().unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    for index in 0..1024 {
        engine.queue_request(&index).unwrap();
    }
    assert_eq!(
        engine.queue_request(&"too many").err().unwrap().kind(),
        io::ErrorKind::StorageFull
    );
    let records = all(&engine);
    assert_eq!(records.len(), 1024);
    engine.dismiss_request(&records[0]).unwrap();
    let accepted = engine.queue_request(&"reviewed new request").unwrap();
    assert_eq!(
        accepted.payload::<String>().unwrap(),
        "reviewed new request"
    );
    assert_eq!(all(&engine).len(), 1024);
}

#[test]
fn unreadable_payloads_stay_visible_and_preserved_after_reopening() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let damaged = engine
        .queue_request(&"will become unreadable input")
        .unwrap()
        .id();
    let intact = engine.queue_request(&"other unfinished work").unwrap().id();
    drop(engine);
    // Corrupt input is prepared through SQLite; all assertions use the public
    // registry. Opening must not drop or rewrite an unrecognized caller payload.
    {
        let input = rusqlite::Connection::open(&history).unwrap();
        input
            .execute(
                "UPDATE requests SET body=?2 WHERE id=?1",
                rusqlite::params![damaged, b"{broken JSON".as_slice()],
            )
            .unwrap();
    }
    for _ in 0..2 {
        let engine = OperationEngine::open(&history).unwrap();
        let records = all(&engine);
        assert_eq!(records.len(), 2);
        assert!(
            records
                .iter()
                .find(|record| record.id() == damaged)
                .unwrap()
                .payload::<serde_json::Value>()
                .is_err()
        );
        assert_eq!(
            records
                .iter()
                .find(|record| record.id() == intact)
                .unwrap()
                .payload::<String>()
                .unwrap(),
            "other unfinished work"
        );
    }
}

#[test]
fn committed_requests_survive_exit_without_running_destructors() {
    let root = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "request_exit_helper", "--nocapture"])
        .env("CANOPA_FILES_REQUEST_EXIT_FIXTURE", root.path())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(73));
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let records = all(&engine);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].phase(), RequestPhase::Running);
    assert_eq!(
        records[0].payload::<String>().unwrap(),
        "committed running checkpoint"
    );
    assert!(engine.latest_undo_id().is_none());
}

#[test]
fn request_exit_helper() {
    let Some(root) = std::env::var_os("CANOPA_FILES_REQUEST_EXIT_FIXTURE") else {
        return;
    };
    let engine =
        OperationEngine::open(std::path::PathBuf::from(root).join("state/history")).unwrap();
    let queued = engine.queue_request(&"accepted").unwrap();
    engine
        .checkpoint_request(
            &queued,
            RequestPhase::Running,
            &"committed running checkpoint",
        )
        .unwrap();
    std::process::exit(73);
}

#[test]
fn acceptance_and_checkpoints_finish_while_an_actual_transfer_waits_for_a_question() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report");
    let destination = root.path().join("destination");
    fs::write(&source, b"incoming").unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("report"), b"previous").unwrap();
    let engine = Arc::new(OperationEngine::open(root.path().join("state/history")).unwrap());
    let mut observer = None;
    let mut during_question = None;
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Conflict { .. }) {
                    let writer = engine.clone();
                    let (send, receive) = mpsc::channel();
                    observer = Some(std::thread::spawn(move || {
                        let result =
                            writer
                                .queue_request(&"queued during question")
                                .and_then(|record| {
                                    writer.checkpoint_request(
                                        &record,
                                        RequestPhase::Failed,
                                        &"checkpoint during question",
                                    )
                                });
                        let _ = send.send(result);
                    }));
                    during_question = receive.recv_timeout(Duration::from_secs(3)).ok();
                    TransferReply::Resolve {
                        choice: ConflictChoice::Skip,
                        apply_to_remaining: false,
                    }
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    // Release the question before asserting a deadline, so failures cannot strand
    // the mutation mutex or a test worker waiting behind it.
    observer.unwrap().join().unwrap();
    let record = during_question
        .expect("request persistence blocked on a UI question")
        .unwrap();
    assert_eq!(record.phase(), RequestPhase::Failed);
    assert_eq!(
        all(&engine)[0].payload::<String>().unwrap(),
        "checkpoint during question"
    );
    assert!(copied.failures.is_empty());
    assert_eq!(fs::read(destination.join("report")).unwrap(), b"previous");
}

#[test]
fn a_replaced_history_is_refused_without_touching_the_replacement() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let record = engine.queue_request(&"original").unwrap();
    let away = root.path().join("held history");
    fs::rename(&history, &away).unwrap();
    fs::write(&history, b"unrelated replacement").unwrap();
    assert!(engine.queue_request(&"new").is_err());
    assert!(
        engine
            .checkpoint_request(&record, RequestPhase::Finished, &"late")
            .is_err()
    );
    assert!(engine.dismiss_request(&record).is_err());
    assert!(engine.request_page(None).is_err());
    assert_eq!(fs::read(&history).unwrap(), b"unrelated replacement");
    fs::remove_file(&history).unwrap();
    fs::rename(away, &history).unwrap();
    assert_eq!(all(&engine)[0].payload::<String>().unwrap(), "original");
}
