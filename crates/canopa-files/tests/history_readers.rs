use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, UndoChoice, UndoSource},
    transfers::{ConflictChoice, TransferEvent, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    sync::{Arc, mpsc},
    time::Duration,
};

#[test]
fn recovery_can_be_inspected_while_undo_waits_for_a_destination_decision() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::write(&source, b"incoming copy").unwrap();
    fs::create_dir(&destination).unwrap();
    let target = destination.join("report.txt");
    fs::write(&target, b"previous destination").unwrap();
    let engine = Arc::new(OperationEngine::open(root.path().join("state/history")).unwrap());
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination,
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    let expected_label = engine.undo_label();
    fs::write(&target, b"newer work after copying").unwrap();
    let mut observer = None;
    let mut during_question = None;
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.source, UndoSource::PreviousDestination);
            let reader = engine.clone();
            let (send, receive) = mpsc::channel();
            observer = Some(std::thread::spawn(move || {
                let result = (
                    reader.recovery_items(None, &Cancellation::default()),
                    reader.latest_undo_id(),
                    reader.undo_label(),
                    reader.interrupted_operations(&Cancellation::default()),
                );
                let _ = send.send(result);
            }));
            // Always release the writer before asserting the observation deadline,
            // so a failed regression cannot leave a reader thread deadlocked.
            during_question = receive.recv_timeout(Duration::from_secs(2)).ok();
            UndoChoice::Skip
        })
        .unwrap();
    observer.unwrap().join().unwrap();
    assert_eq!(result.remaining, 1);
    assert_eq!(fs::read(&target).unwrap(), b"newer work after copying");
    let (page, undo_id, label, interrupted) = during_question.expect(
        "Recovery and history summaries must finish reading while the Undo question is still open",
    );
    let page = page.unwrap();
    assert_eq!(undo_id, copied.id);
    assert_eq!(label, expected_label);
    assert!(interrupted.unwrap().is_empty());
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].held_for_undo);
    assert_eq!(page.items[0].original.as_deref(), Some(target.as_path()));
    assert_eq!(
        fs::read(&page.items[0].entry.as_ref().unwrap().path).unwrap(),
        b"previous destination"
    );
    assert!(page.interrupted.is_empty());
    assert_eq!(engine.latest_undo_id(), copied.id);
}

#[test]
fn a_running_copy_is_not_reported_as_interrupted_or_ready_for_recovery() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("report.txt");
    let destination = root.path().join("destination");
    fs::write(&source, vec![b'A'; 256 * 1024]).unwrap();
    fs::create_dir(&destination).unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let mut observed = None;
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Progress { bytes, .. } if bytes > 0)
                    && observed.is_none()
                {
                    observed = Some(
                        engine
                            .recovery_items(None, &Cancellation::default())
                            .unwrap(),
                    );
                }
                TransferReply::Continue
            },
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    let page = observed.expect("read Recovery during actual copied bytes");
    assert!(
        page.interrupted.is_empty(),
        "a healthy running copy is not interrupted"
    );
    assert!(
        page.items.is_empty(),
        "an unfinished copy is not a retained recovery version"
    );
    assert_eq!(
        fs::read(destination.join("report.txt")).unwrap(),
        vec![b'A'; 256 * 1024]
    );
}

#[test]
fn readers_refuse_missing_or_replaced_history_without_creating_or_changing_it() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let created = engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("folder"),
            },
            &cancel,
        )
        .unwrap();
    let away = root.path().join("original history");
    fs::rename(&history, &away).unwrap();
    assert_eq!(
        engine.recovery_items(None, &cancel).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert!(
        !history.exists(),
        "a reader must not recreate missing history"
    );
    fs::write(&history, b"unrelated replacement at the old path").unwrap();
    assert_eq!(
        engine.recovery_items(None, &cancel).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(
        engine.interrupted_operations(&cancel).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert!(engine.latest_undo_id().is_none());
    assert_eq!(
        fs::read(&history).unwrap(),
        b"unrelated replacement at the old path"
    );
    fs::remove_file(&history).unwrap();
    fs::rename(&away, &history).unwrap();
    assert_eq!(engine.latest_undo_id(), Some(created.id));
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
    engine.undo(&cancel).unwrap();
    assert!(!root.path().join("folder").exists());
}
