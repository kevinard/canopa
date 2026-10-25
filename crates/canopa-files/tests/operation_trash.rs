use canopa_files::{
    Cancellation,
    operation_jobs::{JobSnapshot, OperationJobs, Phase, Request},
    operations::{OperationRequest, UndoChoice},
    trash::TrashLocation,
};
use std::{
    fs,
    os::unix::ffi::OsStringExt,
    path::Path,
    time::{Duration, Instant},
};

fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < deadline, "Trash worker did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn start(root: &Path) -> OperationJobs {
    let queue =
        OperationJobs::start(root.join("history.sqlite3"), root.join("volumes.json")).unwrap();
    wait_for(|| {
        let state = queue.snapshot();
        assert!(state.error.is_none(), "{:?}", state.error);
        (!state.initializing).then_some(())
    });
    queue
}

fn submit(queue: &OperationJobs, request: Request) -> u64 {
    let mut accepted = queue.submit(request, None).unwrap();
    wait_for(|| accepted.try_recv().unwrap()).unwrap()
}

fn settled(queue: &OperationJobs, id: u64) -> JobSnapshot {
    wait_for(|| {
        queue.snapshot().jobs.into_iter().find(|job| {
            job.id == id
                && matches!(
                    job.phase,
                    Phase::Finished | Phase::Stopped | Phase::Failed | Phase::Interrupted
                )
        })
    })
}

fn block_on_review(queue: &OperationJobs, root: &Path) -> (u64, u64) {
    let folder = root.join("review");
    let created = submit(
        queue,
        Request::Mutation(OperationRequest::CreateFolder {
            path: folder.clone(),
        }),
    );
    assert_eq!(settled(queue, created).phase, Phase::Finished);
    fs::write(folder.join("later.txt"), b"later work").unwrap();
    let undo = submit(
        queue,
        Request::Undo {
            expected: queue.snapshot().undo.unwrap().id,
        },
    );
    let generation = wait_for(|| {
        queue
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == undo)
            .and_then(|job| job.decision)
            .map(|decision| decision.generation)
    });
    (undo, generation)
}

#[test]
fn cancelling_queued_trash_records_unstarted_items_without_executing_them_after_reopen() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let queue = start(root);
    let (undo, generation) = block_on_review(&queue, root);
    let source = root.join("keep.txt");
    fs::write(&source, b"keep original").unwrap();
    let id = submit(
        &queue,
        Request::Trash {
            data_home: root.join("data"),
            sources: vec![source.clone()],
        },
    );
    assert_eq!(
        queue
            .snapshot()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .phase,
        Phase::Queued
    );
    queue.cancel(id).unwrap();
    let job = settled(&queue, id);
    assert_eq!(job.phase, Phase::Stopped);
    let report = job.trash.unwrap();
    assert!(report.cancelled);
    assert!(report.completed.is_empty());
    assert!(report.failures.is_empty());
    assert_eq!(report.remaining, vec![source.clone()]);
    assert_eq!(fs::read(&source).unwrap(), b"keep original");
    queue
        .resolve(undo, generation, UndoChoice::KeepItem)
        .unwrap();
    settled(&queue, undo);
    let mut shutdown = queue.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    drop(queue);
    let reopened = start(root);
    let row = settled(&reopened, id);
    assert_eq!(row.phase, Phase::Stopped);
    assert_eq!(row.trash.unwrap().remaining, vec![source.clone()]);
    assert_eq!(fs::read(source).unwrap(), b"keep original");
}

#[test]
fn a_failed_source_does_not_stop_other_items_or_authorize_a_queued_replacement() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let queue = start(root);
    let (undo, generation) = block_on_review(&queue, root);
    let good = root.join("good.txt");
    let replaced = root.join("replaced.txt");
    let missing = root.join("missing-parent/absent.txt");
    fs::write(&good, b"move this").unwrap();
    fs::write(&replaced, b"original selected item").unwrap();
    let id = submit(
        &queue,
        Request::Trash {
            data_home: root.join("data"),
            sources: vec![good.clone(), replaced.clone(), missing.clone()],
        },
    );
    fs::rename(&replaced, root.join("original elsewhere.txt")).unwrap();
    fs::write(&replaced, b"preserve replacement").unwrap();
    fs::create_dir(missing.parent().unwrap()).unwrap();
    fs::write(&missing, b"not selected at admission").unwrap();
    queue
        .resolve(undo, generation, UndoChoice::KeepItem)
        .unwrap();
    let job = settled(&queue, id);
    assert_eq!(job.phase, Phase::Stopped, "{:?}", job.error);
    let report = job.trash.unwrap();
    assert_eq!(report.completed, vec![good.clone()]);
    assert_eq!(report.failures.len(), 2);
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.source == replaced)
    );
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.source == missing)
    );
    assert_eq!(fs::read(&replaced).unwrap(), b"preserve replacement");
    assert_eq!(fs::read(&missing).unwrap(), b"not selected at admission");
    assert!(!good.exists());
    let mut shutdown = queue.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    drop(queue);
    let reopened = start(root);
    let report = settled(&reopened, id).trash.unwrap();
    assert_eq!(report.completed, vec![good]);
    assert_eq!(report.failures.len(), 2);
    assert_eq!(fs::read(replaced).unwrap(), b"preserve replacement");
    assert_eq!(fs::read(missing).unwrap(), b"not selected at admission");
}

#[test]
fn restore_uses_the_selected_trash_item_preserves_an_occupied_name_and_can_be_undone() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let data = root.join("data");
    let source = root.join(std::ffi::OsString::from_vec(b"restore-\xff.txt".to_vec()));
    fs::write(&source, b"stored version").unwrap();
    let queue = start(root);
    let trashed = submit(
        &queue,
        Request::Trash {
            data_home: data.clone(),
            sources: vec![source.clone()],
        },
    );
    assert_eq!(settled(&queue, trashed).phase, Phase::Finished);
    let item = TrashLocation::home(&data)
        .unwrap()
        .list(&Cancellation::default())
        .unwrap()
        .pop()
        .unwrap();
    fs::write(&source, b"new occupant").unwrap();
    let conflict = submit(
        &queue,
        Request::Restore {
            item: Box::new(item.clone()),
            destination: None,
        },
    );
    assert_eq!(settled(&queue, conflict).phase, Phase::Failed);
    assert_eq!(fs::read(&source).unwrap(), b"new occupant");
    assert_eq!(fs::read(&item.path).unwrap(), b"stored version");
    let destination = root.join("restored elsewhere.txt");
    let restored = submit(
        &queue,
        Request::Restore {
            item: Box::new(item.clone()),
            destination: Some(destination.clone()),
        },
    );
    let restored = settled(&queue, restored);
    assert_eq!(restored.phase, Phase::Finished, "{:?}", restored.error);
    assert_eq!(fs::read(&destination).unwrap(), b"stored version");
    assert!(!item.path.exists());
    let undo = submit(
        &queue,
        Request::Undo {
            expected: queue.snapshot().undo.unwrap().id,
        },
    );
    assert_eq!(settled(&queue, undo).phase, Phase::Finished);
    assert!(!destination.exists());
    assert_eq!(fs::read(&item.path).unwrap(), b"stored version");
    assert_eq!(fs::read(source).unwrap(), b"new occupant");
    let listed = TrashLocation::home(&data)
        .unwrap()
        .list(&Cancellation::default())
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].original, item.original);
}

#[test]
fn a_refused_restore_into_a_leased_container_does_not_block_later_operations() {
    // Concurrent test-child launches can briefly inherit a close-on-exec flock
    // during this engine handoff. Create and retire the engines after exec in
    // their own child, preserving the actual single-owner rejection semantics.
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args([
        "--ignored",
        "--exact",
        "leased_restore_child",
        "--nocapture",
    ]);
    run_private_child(command);
}

#[test]
#[ignore = "private child isolates history-lock handoff from concurrent process launches"]
fn leased_restore_child() {
    use canopa_files::{
        operations::OperationEngine,
        transfers::{ConflictChoice, TransferEvent, TransferKind, TransferReply, TransferRequest},
    };
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let incoming = root.join("incoming");
    let destination = root.join("destination");
    fs::create_dir(&incoming).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(incoming.join("note.txt"), b"current version").unwrap();
    fs::write(destination.join("note.txt"), b"retained version").unwrap();
    let engine = OperationEngine::open(root.join("history.sqlite3")).unwrap();
    let outcome = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![incoming.join("note.txt")],
                destination,
            },
            &Cancellation::default(),
            |event| match event {
                TransferEvent::Conflict { .. } => TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
                _ => TransferReply::Continue,
            },
        )
        .unwrap();
    assert_eq!(outcome.completed, 1);
    let saved = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| item.entry.map(|entry| entry.path))
        .unwrap();
    drop(engine);
    let queue = start(root);
    let mut leased = queue.lease_sources(vec![saved.clone()]).unwrap();
    let lease = wait_for(|| leased.try_recv().unwrap()).unwrap();
    let source = root.join("restore.txt");
    fs::write(&source, b"restore payload").unwrap();
    let data = root.join("data");
    let trashed = submit(
        &queue,
        Request::Trash {
            data_home: data.clone(),
            sources: vec![source],
        },
    );
    assert_eq!(settled(&queue, trashed).phase, Phase::Finished);
    let item = TrashLocation::home(&data)
        .unwrap()
        .list(&Cancellation::default())
        .unwrap()
        .pop()
        .unwrap();
    let target = saved.parent().unwrap().join("restored.txt");
    let refused = submit(
        &queue,
        Request::Restore {
            item: Box::new(item.clone()),
            destination: Some(target.clone()),
        },
    );
    let row = settled(&queue, refused);
    assert_eq!(row.phase, Phase::Failed, "{:?}", row.error);
    let report = row.restore.unwrap();
    assert!(!report.interrupted);
    assert!(report.restored.is_none());
    assert!(report.error.is_some());
    assert!(!target.exists());
    assert_eq!(fs::read(&item.path).unwrap(), b"restore payload");
    assert_eq!(fs::read(&saved).unwrap(), b"retained version");
    lease.release().unwrap();
    let later = root.join("unrelated operation after refusal");
    let next = submit(
        &queue,
        Request::Mutation(OperationRequest::CreateFolder {
            path: later.clone(),
        }),
    );
    assert_eq!(settled(&queue, next).phase, Phase::Finished);
    assert!(later.is_dir());
    let retry = submit(
        &queue,
        Request::Restore {
            item: Box::new(item),
            destination: Some(target.clone()),
        },
    );
    assert_eq!(settled(&queue, retry).phase, Phase::Finished);
    assert_eq!(fs::read(target).unwrap(), b"restore payload");
    assert_eq!(fs::read(saved).unwrap(), b"retained version");
}

#[test]
fn cancelling_after_a_real_trash_move_keeps_an_exact_completed_prefix_and_undo() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let folder = root.join("selection");
    fs::create_dir(&folder).unwrap();
    let sources: Vec<_> = (0..512)
        .map(|number| folder.join(format!("item-{number:04}.txt")))
        .collect();
    for source in &sources {
        fs::write(source, b"recoverable").unwrap();
    }
    let queue = start(root);
    let id = submit(
        &queue,
        Request::Trash {
            data_home: root.join("data"),
            sources: sources.clone(),
        },
    );
    wait_for(|| (!sources[0].exists()).then_some(()));
    queue.cancel(id).unwrap();
    let job = settled(&queue, id);
    assert_eq!(job.phase, Phase::Stopped, "{:?}", job.error);
    let report = job.trash.unwrap();
    assert!(report.cancelled);
    assert!(!report.completed.is_empty());
    assert!(!report.remaining.is_empty());
    assert!(report.failures.is_empty());
    assert_eq!(
        report.completed.len() + report.remaining.len(),
        sources.len()
    );
    for completed in &report.completed {
        assert!(!completed.exists());
    }
    for remaining in &report.remaining {
        assert_eq!(fs::read(remaining).unwrap(), b"recoverable");
    }
    let undo = submit(
        &queue,
        Request::Undo {
            expected: queue.snapshot().undo.unwrap().id,
        },
    );
    assert_eq!(settled(&queue, undo).phase, Phase::Finished);
    for source in &sources {
        assert_eq!(fs::read(source).unwrap(), b"recoverable");
    }
}

#[test]
fn a_recovered_trash_commit_is_reported_as_completed_and_reopens() {
    run_fault_helper("recover", "trash");
}

#[test]
fn cancellation_after_dispatch_keeps_typed_unstarted_items_and_allows_later_work() {
    run_fault_helper("dispatch", "trash");
}

#[test]
fn a_recovered_restore_commit_is_reported_as_completed_and_reopens() {
    run_fault_helper("recover", "restore");
}

#[test]
fn restore_cancelled_after_dispatch_preserves_the_payload_and_later_work() {
    run_fault_helper("dispatch", "restore");
}

fn run_fault_helper(mode: &str, action: &str) {
    let root = tempfile::Builder::new()
        .prefix("canopa-files-trash-fault-")
        .tempdir_in("/tmp")
        .unwrap();
    let library = root.path().join("faults.so");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/trash_faults.c");
    let compiler = std::process::Command::new("cc")
        .args(["-shared", "-fPIC", "-O2"])
        .arg(source)
        .args(["-ldl", "-o"])
        .arg(&library)
        .output()
        .unwrap();
    assert!(
        compiler.status.success(),
        "{}",
        String::from_utf8_lossy(&compiler.stderr)
    );
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--ignored", "--exact", "trash_fault_helper", "--nocapture"])
        .env("LD_PRELOAD", library)
        .env("CANOPA_FILES_TRASH_FAULT_ROOT", root.path())
        .env("CANOPA_FILES_TRASH_FAULT_MODE", mode)
        .env("CANOPA_FILES_TRASH_FAULT_ACTION", action);
    run_private_child(command);
}

fn run_private_child(mut command: std::process::Command) {
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("private Trash test helper timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_final_history_trim_error_keeps_the_committed_trash_report_without_a_replay_plan() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let queue = start(root);
    for index in 0..100 {
        let id = submit(
            &queue,
            Request::Mutation(OperationRequest::CreateFolder {
                path: root.join(format!("Folder {index}")),
            }),
        );
        assert_eq!(settled(&queue, id).phase, Phase::Finished);
    }
    let history = root.join("history.sqlite3");
    let connection = rusqlite::Connection::open(&history).unwrap();
    connection.execute_batch("CREATE TRIGGER private_trim_failure BEFORE DELETE ON operations WHEN OLD.id=1 BEGIN SELECT RAISE(FAIL, 'private trim failure'); END;").unwrap();
    let source = root.join("completed.txt");
    fs::write(&source, b"completed before finalization failed").unwrap();
    let id = submit(
        &queue,
        Request::Trash {
            data_home: root.join("data"),
            sources: vec![source.clone()],
        },
    );
    let job = settled(&queue, id);
    assert!(!source.exists(), "Trash committed before the failure");
    assert_eq!(job.phase, Phase::Interrupted, "{:?}", job.error);
    let report = job
        .trash
        .expect("known completed work must survive an outer error");
    assert_eq!(report.completed, vec![source.clone()]);
    assert!(report.remaining.is_empty());
    assert!(job.error.unwrap().contains("private trim failure"));
    connection
        .execute_batch("DROP TRIGGER private_trim_failure;")
        .unwrap();
    drop(connection);
    let blocked = root.join("needs explicit reconciliation");
    let next = submit(
        &queue,
        Request::Mutation(OperationRequest::CreateFolder {
            path: blocked.clone(),
        }),
    );
    assert_eq!(settled(&queue, next).phase, Phase::Failed);
    assert!(
        !blocked.exists(),
        "an interrupted report grants no new mutation authority"
    );
    let mut shutdown = queue.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    drop(queue);
    let reopened = start(root);
    assert_eq!(
        settled(&reopened, id).trash.unwrap().completed,
        vec![source.clone()]
    );
    assert!(
        !source.exists(),
        "reopening does not execute the original request"
    );
    let items = TrashLocation::home(root.join("data"))
        .unwrap()
        .list(&Cancellation::default())
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        fs::read(&items[0].path).unwrap(),
        b"completed before finalization failed"
    );
}

#[test]
#[ignore = "private child process with one exact filesystem fault"]
fn trash_fault_helper() {
    let root = std::path::PathBuf::from(std::env::var_os("CANOPA_FILES_TRASH_FAULT_ROOT").unwrap());
    assert!(
        root.as_os_str()
            .as_encoded_bytes()
            .starts_with(b"/tmp/canopa-files-trash-fault-")
    );
    let folder = root.join("source");
    fs::create_dir(&folder).unwrap();
    let source = folder.join("item.txt");
    fs::write(&source, b"preserve the committed move").unwrap();
    let queue = start(&root);
    let restore = std::env::var("CANOPA_FILES_TRASH_FAULT_ACTION").unwrap() == "restore";
    let request = if restore {
        let id = submit(
            &queue,
            Request::Trash {
                data_home: root.join("data"),
                sources: vec![source.clone()],
            },
        );
        assert_eq!(settled(&queue, id).phase, Phase::Finished);
        let item = TrashLocation::home(root.join("data"))
            .unwrap()
            .list(&Cancellation::default())
            .unwrap()
            .pop()
            .unwrap();
        Request::Restore {
            item: Box::new(item),
            destination: None,
        }
    } else {
        Request::Trash {
            data_home: root.join("data"),
            sources: vec![source.clone()],
        }
    };
    if std::env::var("CANOPA_FILES_TRASH_FAULT_MODE").unwrap() == "dispatch" {
        fs::write(root.join("dispatch-arm"), b"armed").unwrap();
        let id = submit(&queue, request);
        wait_for(|| root.join("dispatch-started").exists().then_some(()));
        assert_eq!(
            queue
                .snapshot()
                .jobs
                .iter()
                .find(|job| job.id == id)
                .unwrap()
                .phase,
            Phase::Running
        );
        queue.cancel(id).unwrap();
        // Durable admission proves the serialized coordinator processed the
        // preceding Cancel before the paused mutation worker is released.
        let later = root.join("after cancellation");
        let next = submit(
            &queue,
            Request::Mutation(OperationRequest::CreateFolder {
                path: later.clone(),
            }),
        );
        fs::write(root.join("dispatch-release"), b"released").unwrap();
        let job = settled(&queue, id);
        assert_eq!(job.phase, Phase::Stopped, "{:?}", job.error);
        assert!(job.error.is_none());
        if restore {
            let report = job.restore.unwrap();
            assert!(report.cancelled);
            assert!(!report.interrupted);
            assert!(report.restored.is_none());
            assert!(report.operation.is_none());
            assert!(!source.exists());
            let items = TrashLocation::home(root.join("data"))
                .unwrap()
                .list(&Cancellation::default())
                .unwrap();
            assert_eq!(items.len(), 1);
            assert_eq!(report.source, items[0].path);
            assert_eq!(
                fs::read(&items[0].path).unwrap(),
                b"preserve the committed move"
            );
        } else {
            let report = job.trash.unwrap();
            assert!(
                report.cancelled,
                "the acknowledged cancellation was not retained"
            );
            assert_eq!(report.remaining, vec![source.clone()]);
            assert!(report.completed.is_empty());
            assert!(report.failures.is_empty());
            assert_eq!(fs::read(&source).unwrap(), b"preserve the committed move");
        }
        assert_eq!(settled(&queue, next).phase, Phase::Finished);
        assert!(later.is_dir());
        return;
    }
    fs::write(root.join("fsync-arm"), b"armed").unwrap();
    let id = submit(&queue, request);
    let job = settled(&queue, id);
    assert!(
        root.join("fsync-fault-fired").is_file(),
        "actual fsync failure was not injected"
    );
    assert_eq!(
        source.exists(),
        restore,
        "the payload rename actually committed"
    );
    assert_eq!(job.phase, Phase::Finished, "{:?}", job.error);
    if restore {
        let report = job.restore.as_ref().unwrap();
        assert_eq!(report.restored.as_ref(), Some(&source));
        assert!(report.operation.is_some());
        assert!(report.error.is_none());
        assert_eq!(job.paths.as_ref(), std::slice::from_ref(&source));
        assert_eq!(fs::read(&source).unwrap(), b"preserve the committed move");
    } else {
        let report = job.trash.unwrap();
        assert_eq!(report.completed, vec![source.clone()]);
        assert!(report.failures.is_empty());
    }
    let mut shutdown = queue.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    drop(queue);
    let reopened = start(&root);
    let row = settled(&reopened, id);
    assert_eq!(row.phase, Phase::Finished);
    assert_eq!(row.paths.as_ref(), std::slice::from_ref(&source));
    let undo = submit(
        &reopened,
        Request::Undo {
            expected: reopened.snapshot().undo.unwrap().id,
        },
    );
    assert_eq!(settled(&reopened, undo).phase, Phase::Finished);
    if restore {
        assert!(!source.exists());
        let items = TrashLocation::home(root.join("data"))
            .unwrap()
            .list(&Cancellation::default())
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            fs::read(&items[0].path).unwrap(),
            b"preserve the committed move"
        );
    } else {
        assert_eq!(fs::read(source).unwrap(), b"preserve the committed move");
    }
}

#[test]
fn restore_finalization_retains_the_completed_destination_without_replay_authority() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let queue = start(root);
    for index in 0..99 {
        let id = submit(
            &queue,
            Request::Mutation(OperationRequest::CreateFolder {
                path: root.join(format!("Folder {index}")),
            }),
        );
        assert_eq!(settled(&queue, id).phase, Phase::Finished);
    }
    let source = root.join("restore.txt");
    fs::write(&source, b"confirmed restore").unwrap();
    let trashed = submit(
        &queue,
        Request::Trash {
            data_home: root.join("data"),
            sources: vec![source.clone()],
        },
    );
    assert_eq!(settled(&queue, trashed).phase, Phase::Finished);
    let item = TrashLocation::home(root.join("data"))
        .unwrap()
        .list(&Cancellation::default())
        .unwrap()
        .pop()
        .unwrap();
    let connection = rusqlite::Connection::open(root.join("history.sqlite3")).unwrap();
    connection.execute_batch("CREATE TRIGGER private_trim_failure BEFORE DELETE ON operations WHEN OLD.id=1 BEGIN SELECT RAISE(FAIL, 'private restore trim failure'); END;").unwrap();
    let restored = submit(
        &queue,
        Request::Restore {
            item: Box::new(item.clone()),
            destination: None,
        },
    );
    let row = settled(&queue, restored);
    assert_eq!(fs::read(&source).unwrap(), b"confirmed restore");
    assert!(!item.path.exists());
    assert_eq!(row.phase, Phase::Interrupted, "{:?}", row.error);
    assert_eq!(row.paths.as_ref(), std::slice::from_ref(&source));
    let report = row.restore.as_ref().unwrap();
    assert!(report.interrupted);
    assert_eq!(report.restored.as_ref(), Some(&source));
    assert_eq!(report.source, item.path);
    assert!(row.error.unwrap().contains("private restore trim failure"));
    connection
        .execute_batch("DROP TRIGGER private_trim_failure;")
        .unwrap();
    drop(connection);
    let blocked = root.join("restore needs explicit reconciliation");
    let id = submit(
        &queue,
        Request::Mutation(OperationRequest::CreateFolder {
            path: blocked.clone(),
        }),
    );
    assert_eq!(settled(&queue, id).phase, Phase::Failed);
    assert!(!blocked.exists());
    let mut shutdown = queue.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    drop(queue);
    let reopened = start(root);
    let row = settled(&reopened, restored);
    assert_eq!(row.phase, Phase::Interrupted);
    assert_eq!(row.paths.as_ref(), std::slice::from_ref(&source));
    assert_eq!(fs::read(source).unwrap(), b"confirmed restore");
    assert!(!item.path.exists());
}

#[test]
fn queued_trash_preserves_raw_names_and_symlinks_and_shares_actual_undo() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let data = root.join("data");
    let raw = root.join(std::ffi::OsString::from_vec(b"private-\xff.txt".to_vec()));
    let target = root.join("target.txt");
    let link = root.join("link.txt");
    fs::write(&raw, b"raw contents").unwrap();
    fs::write(&target, b"keep target").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let queue = start(root);
    let id = submit(
        &queue,
        Request::Trash {
            data_home: data.clone(),
            sources: vec![raw.clone(), link.clone()],
        },
    );
    let job = settled(&queue, id);
    assert_eq!(job.phase, Phase::Finished, "{:?}", job.error);
    let report = job.trash.unwrap();
    assert_eq!(report.completed.len(), 2);
    assert!(report.failures.is_empty());
    assert!(report.remaining.is_empty());
    assert!(!raw.exists());
    assert!(fs::symlink_metadata(&link).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"keep target");
    let items = TrashLocation::home(&data)
        .unwrap()
        .list(&Cancellation::default())
        .unwrap();
    assert_eq!(items.len(), 2);
    assert!(
        items
            .iter()
            .any(|item| item.original.as_ref() == Some(&raw))
    );
    let undo = queue.snapshot().undo.unwrap();
    let undone = submit(&queue, Request::Undo { expected: undo.id });
    assert_eq!(settled(&queue, undone).phase, Phase::Finished);
    assert_eq!(fs::read(&raw).unwrap(), b"raw contents");
    assert_eq!(fs::read_link(&link).unwrap(), target);
    assert!(
        TrashLocation::home(&data)
            .unwrap()
            .list(&Cancellation::default())
            .unwrap()
            .is_empty()
    );
}
