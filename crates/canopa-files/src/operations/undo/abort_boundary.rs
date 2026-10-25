//! Actual public Undo worker death while preparing a cross-filesystem return.
use super::*;
pub(in crate::operations) mod changed_copy_boundary;
pub(in crate::operations) mod cleanup_boundary;
pub(in crate::operations) mod copy_boundary;
pub(in crate::operations) mod direct_displacement_boundary;
pub(in crate::operations) mod displaced_boundary;
pub(in crate::operations) mod disposal_boundary;
pub(in crate::operations) mod move_return_boundary;
pub(in crate::operations) mod saved_copy_boundary;
use crate::{
    operation_jobs::{OperationJobs, Phase, Request},
    transfers::{TransferKind, TransferRequest},
};
use std::{
    os::unix::process::ExitStatusExt,
    time::{Duration, Instant},
};

fn wait<T>(mut read: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = read() {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "private Undo abort worker did not settle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn receive<T>(mut reply: futures_channel::oneshot::Receiver<T>) -> T {
    wait(|| reply.try_recv().unwrap())
}
fn start(root: &Path) -> OperationJobs {
    let jobs = OperationJobs::start(root.join("history"), root.join("volumes")).unwrap();
    wait(|| (!jobs.snapshot().initializing).then_some(()));
    assert!(jobs.snapshot().error.is_none());
    jobs
}
fn close(jobs: OperationJobs, root: &Path) {
    drop(jobs);
    drop(wait(|| match OperationEngine::open(root.join("history")) {
        Ok(engine) => Some(engine),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => None,
        Err(error) => panic!("{error}"),
    }));
}
fn terminal(jobs: &OperationJobs, id: u64) -> crate::operation_jobs::JobSnapshot {
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && !job.phase.holds_application())
    })
}
fn partial(jobs: &OperationJobs, bytes: &[u8]) -> RecoveryItem {
    receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
        .unwrap()
        .items
        .into_iter()
        .find(|item| {
            item.entry
                .as_ref()
                .is_some_and(|entry| fs::read(&entry.path).is_ok_and(|actual| actual == bytes))
        })
        .expect("Undo partial remains available through public Recovery")
}
fn keep(jobs: &OperationJobs, item: RecoveryItem) -> crate::operation_jobs::JobSnapshot {
    let id = receive(
        jobs.submit(
            Request::KeepRecovery {
                item: Box::new(item),
            },
            None,
        )
        .unwrap(),
    )
    .unwrap();
    terminal(jobs, id)
}
pub(in crate::operations) fn preparing(source: &Path, stage: &Stage) {
    if std::env::var("CANOPA_UNDO_ABORT_POINT").is_ok_and(|point| point != "copy") {
        return;
    }
    let Some(root) = std::env::var_os("CANOPA_UNDO_ABORT_ROOT").map(PathBuf::from) else {
        return;
    };
    let output = PathBuf::from(std::env::var_os("CANOPA_UNDO_ABORT_OUTPUT").unwrap());
    if source != output.join("item") {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-undo-abort-")
    );
    assert_eq!(
        stage.directory.parent(),
        Some(root.join("source").as_path())
    );
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let pending: String = db
        .query_row("SELECT body FROM pending", [], |row| row.get(0))
        .unwrap();
    let pending: Pending = serde_json::from_str(&pending).unwrap();
    assert!(matches!(&pending,Pending::Undo {stage:Some(actual),..} if actual==stage));
    fs::write(
        root.join("marker.json"),
        serde_json::to_vec(&pending).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private Undo preparation worker was not killed");
}
#[test]
#[ignore = "private public Undo worker killed during a cross-filesystem return copy"]
fn undo_abort_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_UNDO_ABORT_ROOT").unwrap());
    let output = PathBuf::from(std::env::var_os("CANOPA_UNDO_ABORT_OUTPUT").unwrap());
    fs::create_dir(root.join("source")).unwrap();
    fs::write(root.join("source/item"), vec![b'x'; 128 * 1024]).unwrap();
    assert_ne!(
        fs::metadata(&root).unwrap().dev(),
        fs::metadata(&output).unwrap().dev()
    );
    let jobs = start(&root);
    let id = receive(
        jobs.submit(
            Request::Transfer(TransferRequest {
                kind: TransferKind::Move,
                sources: vec![root.join("source/item")],
                destination: output.clone(),
            }),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    let done = wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && !job.phase.holds_application())
    });
    assert_eq!(done.phase, Phase::Finished, "{done:?}");
    assert!(!root.join("source/item").exists());
    assert_eq!(fs::read(output.join("item")).unwrap().len(), 128 * 1024);
    let operation = done.transfer.as_ref().unwrap().operation.unwrap();
    let relocate = std::env::var("CANOPA_UNDO_ABORT_RELOCATE").as_deref() == Ok("1");
    let replace = std::env::var("CANOPA_UNDO_ABORT_REPLACE").as_deref() == Ok("1");
    if relocate || replace {
        fs::write(root.join("source/item"), b"Occupied source name").unwrap();
    }
    let id = receive(
        jobs.submit(
            Request::Undo {
                expected: operation,
            },
            None,
        )
        .unwrap(),
    )
    .unwrap();
    if relocate || replace {
        let decision = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|job| job.id == id && job.phase == Phase::Waiting)
                .and_then(|job| job.decision)
        });
        jobs.resolve(
            id,
            decision.generation,
            if replace {
                UndoChoice::Replace
            } else {
                UndoChoice::KeepBoth
            },
        )
        .unwrap();
    }
    let done = wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && !job.phase.holds_application())
    });
    if std::env::var("CANOPA_UNDO_CLEANUP_FAULT").is_ok() {
        fs::write(
            root.join("completed-cleanup-result.txt"),
            format!("{done:?}"),
        )
        .unwrap();
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    }
    panic!("private Undo preparation boundary was not reached: {done:?}");
}
fn fixture(run: impl FnOnce(&Path, &Path, &Stage)) {
    fixture_choice(false, run);
}
fn fixture_choice(relocate: bool, run: impl FnOnce(&Path, &Path, &Stage)) {
    fixture_decision(relocate, false, run);
}
fn fixture_decision(relocate: bool, replace: bool, run: impl FnOnce(&Path, &Path, &Stage)) {
    fixture_decision_at(relocate, replace, "copy", run);
}
fn fixture_decision_at(
    relocate: bool,
    replace: bool,
    point: &str,
    run: impl FnOnce(&Path, &Path, &Stage),
) {
    let root = tempfile::Builder::new()
        .prefix("canopa-undo-abort-")
        .tempdir()
        .unwrap();
    let output = tempfile::Builder::new()
        .prefix(".canopa-undo-abort-output-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::undo::abort_boundary::undo_abort_child",
            ])
            .env("CANOPA_UNDO_ABORT_ROOT", root.path())
            .env("CANOPA_UNDO_ABORT_OUTPUT", output.path())
            .env(
                "CANOPA_UNDO_ABORT_RELOCATE",
                if relocate { "1" } else { "0" },
            )
            .env("CANOPA_UNDO_ABORT_REPLACE", if replace { "1" } else { "0" })
            .env("CANOPA_UNDO_ABORT_POINT", point)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let pending: Pending =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        let stage = match pending {
            Pending::Undo {
                stage: Some(stage), ..
            } => stage,
            Pending::Undo {
                stage: None,
                displaced: Some(record),
                ..
            } => record.retained.stage,
            _ => panic!("Undo stage"),
        };
        run(root.path(), output.path(), &stage);
    }));
    if let Err(error) = checked {
        eprintln!("retained Undo abort source: {}", root.keep().display());
        eprintln!("retained Undo abort output: {}", output.keep().display());
        std::panic::resume_unwind(error);
    }
}
#[test]
fn interrupted_undo_replace_keeps_all_return_work_and_retries_without_losing_the_newer_item() {
    fixture_decision(false, true, |root, output, stage| {
        let bytes = b"Edited Undo Replace return partial";
        fs::write(stage.payload(), bytes).unwrap();
        let jobs = start(root);
        let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
        assert_ne!(terminal(&jobs, id).phase, Phase::Finished);
        let item = partial(&jobs, bytes);
        assert!(
            item.can_keep_in_recovery(),
            "a bound interrupted Undo Replace must offer a safe Keep disposition"
        );
        assert_eq!(keep(&jobs, item).phase, Phase::Finished);
        assert_eq!(
            fs::read(root.join("source/item")).unwrap(),
            b"Occupied source name"
        );
        assert_eq!(
            fs::read(output.join("item")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        let expected = jobs.snapshot().undo.unwrap().id;
        let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
        let decision = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id && j.phase == Phase::Waiting)
                .and_then(|j| j.decision)
        });
        jobs.resolve(id, decision.generation, UndoChoice::Replace)
            .unwrap();
        let done = terminal(&jobs, id);
        assert_eq!(done.phase, Phase::Finished, "{done:?}");
        assert_eq!(
            fs::read(root.join("source/item")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        assert!(!output.join("item").exists());
        drop(partial(&jobs, bytes));
        drop(partial(&jobs, b"Occupied source name"));
        close(jobs, root);
    });
}
#[test]
fn unchanged_undo_partials_reconcile_and_allow_the_original_undo_to_retry() {
    fixture(|root, output, stage| {
        assert_eq!(fs::read(stage.payload()).unwrap(), vec![b'x'; 64 * 1024]);
        let jobs = start(root);
        let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
        let done = terminal(&jobs, id);
        fs::write(root.join("reconciled.txt"), format!("{done:?}")).unwrap();
        assert_eq!(
            done.phase,
            Phase::Finished,
            "an unchanged partial must reconcile from its original source without manual retention: {done:?}"
        );
        assert!(!stage.directory.exists());
        assert!(!root.join("source/item").exists());
        assert_eq!(
            fs::read(output.join("item")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        close(jobs, root);
        let jobs = start(root);
        let expected = jobs.snapshot().undo.unwrap().id;
        let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
        let done = terminal(&jobs, id);
        assert_eq!(done.phase, Phase::Finished, "{done:?}");
        assert_eq!(
            fs::read(root.join("source/item")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        assert!(!output.join("item").exists());
        assert!(
            receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
                .unwrap()
                .items
                .is_empty()
        );
        close(jobs, root);
    });
}
#[test]
fn edited_interrupted_undo_partials_remain_publicly_recoverable() {
    fixture(|root, output, stage| {
        let bytes = b"Unique edits in interrupted Undo return";
        fs::write(stage.payload(), bytes).unwrap();
        fs::write(
            stage.directory.join("extra-work"),
            b"Extra interrupted work",
        )
        .unwrap();
        let Pending::Undo { id: expected, .. } =
            serde_json::from_slice::<Pending>(&fs::read(root.join("marker.json")).unwrap())
                .unwrap()
        else {
            unreachable!();
        };
        let jobs = start(root);
        let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
        let done = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|job| job.id == id && !job.phase.holds_application())
        });
        fs::write(root.join("reconciled.txt"), format!("{done:?}")).unwrap();
        let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
        fs::write(
            root.join("recovery-page.json"),
            serde_json::to_vec_pretty(&page.items).unwrap(),
        )
        .unwrap();
        assert!(
            page.items.iter().any(|item| item
                .entry
                .as_ref()
                .is_some_and(|entry| fs::read(&entry.path).is_ok_and(|actual| actual == bytes))),
            "edited Undo partial bytes must stay available through public Recovery"
        );
        assert_eq!(
            fs::read(output.join("item")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        assert!(!root.join("source/item").exists());
        assert_ne!(done.phase, Phase::Finished, "{done:?}");
        close(jobs, root);
        for _ in 0..2 {
            let jobs = start(root);
            let item = partial(&jobs, bytes);
            assert!(item.can_keep_in_recovery());
            drop(item);
            close(jobs, root);
        }
        let jobs = start(root);
        let item = partial(&jobs, bytes);
        let id = receive(
            jobs.submit(
                Request::Recover {
                    item: Box::new(item.clone()),
                    destination: root.join("rescued"),
                },
                None,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(terminal(&jobs, id).phase, Phase::Finished);
        assert_eq!(fs::read(root.join("rescued")).unwrap(), bytes);
        assert_eq!(keep(&jobs, item).phase, Phase::Finished);
        let exported = jobs.snapshot().undo.unwrap().id;
        let id = receive(
            jobs.submit(Request::Undo { expected: exported }, None)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(terminal(&jobs, id).phase, Phase::Finished);
        assert!(!root.join("rescued").exists());
        let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
        let done = terminal(&jobs, id);
        assert_eq!(done.phase, Phase::Finished, "{done:?}");
        assert_eq!(
            fs::read(root.join("source/item")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        assert!(!output.join("item").exists());
        let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
        let root_item = page
            .items
            .into_iter()
            .find(|item| {
                item.entry
                    .as_ref()
                    .is_some_and(|entry| entry.path == stage.directory)
            })
            .expect("whole interrupted Undo folder remains catalogued");
        let copy = receive(
            jobs.submit(
                Request::Recover {
                    item: Box::new(root_item),
                    destination: root.join("rescued-work"),
                },
                None,
            )
            .unwrap(),
        )
        .unwrap();
        let done = terminal(&jobs, copy);
        assert_eq!(done.phase, Phase::Finished, "{done:?}");
        assert_eq!(fs::read(root.join("rescued-work/folder")).unwrap(), bytes);
        assert_eq!(
            fs::read(root.join("rescued-work/extra-work")).unwrap(),
            b"Extra interrupted work"
        );
        drop(partial(&jobs, bytes));
        close(jobs, root);
    });
}
#[test]
fn legacy_unbound_undo_partials_are_copyable_without_adopting_cleanup_rights() {
    fixture(|root, _output, stage| {
        let bytes = b"Legacy interrupted Undo data";
        fs::write(stage.payload(), bytes).unwrap();
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        db.execute_batch("DROP TABLE undo_work; PRAGMA user_version=49;")
            .unwrap();
        drop(db);
        let jobs = start(root);
        let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
        assert_ne!(terminal(&jobs, id).phase, Phase::Finished);
        let item = partial(&jobs, bytes);
        assert!(!item.can_keep_in_recovery());
        let id = receive(
            jobs.submit(
                Request::Recover {
                    item: Box::new(item),
                    destination: root.join("legacy-rescued"),
                },
                None,
            )
            .unwrap(),
        )
        .unwrap();
        let done = terminal(&jobs, id);
        assert_eq!(done.phase, Phase::Finished, "{done:?}");
        assert_eq!(fs::read(root.join("legacy-rescued")).unwrap(), bytes);
        assert_eq!(fs::read(stage.payload()).unwrap(), bytes);
        close(jobs, root);
    });
}
#[test]
fn keep_both_undo_return_work_preserves_its_original_history_owner() {
    fixture_choice(true, |root, output, stage| {
        let bytes = b"Relocated interrupted Undo work";
        fs::write(stage.payload(), bytes).unwrap();
        let jobs = start(root);
        let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
        assert_ne!(terminal(&jobs, id).phase, Phase::Finished);
        let item = partial(&jobs, bytes);
        assert!(item.can_keep_in_recovery());
        assert_eq!(keep(&jobs, item).phase, Phase::Finished);
        let expected = jobs.snapshot().undo.unwrap().id;
        let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
        let decision = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|job| job.id == id && job.phase == Phase::Waiting)
                .and_then(|job| job.decision)
        });
        jobs.resolve(id, decision.generation, UndoChoice::KeepBoth)
            .unwrap();
        let done = terminal(&jobs, id);
        assert_eq!(done.phase, Phase::Finished, "{done:?}");
        assert_eq!(
            fs::read(root.join("source/item")).unwrap(),
            b"Occupied source name"
        );
        assert!(!output.join("item").exists());
        assert!(fs::read_dir(root.join("source")).unwrap().any(|entry| {
            fs::read(entry.unwrap().path()).is_ok_and(|actual| actual == vec![b'x'; 128 * 1024])
        }));
        drop(partial(&jobs, bytes));
        close(jobs, root);
    });
}
#[test]
fn incomplete_undo_work_returned_to_the_source_never_completes_the_step() {
    fixture(|root, output, stage| {
        let bytes = fs::read(stage.payload()).unwrap();
        assert_eq!(bytes.len(), 64 * 1024);
        fs::rename(stage.payload(), root.join("source/item")).unwrap();
        let jobs = start(root);
        let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
        let done = terminal(&jobs, id);
        assert_ne!(done.phase, Phase::Finished, "{done:?}");
        assert_eq!(fs::read(root.join("source/item")).unwrap(), bytes);
        assert_eq!(fs::read(output.join("item")).unwrap().len(), 128 * 1024);
        let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
        let item = page
            .items
            .into_iter()
            .find(|item| {
                item.entry
                    .as_ref()
                    .is_some_and(|entry| entry.path == stage.directory)
            })
            .expect("whole Undo work folder is available for resolution");
        assert!(item.can_keep_in_recovery());
        assert_eq!(keep(&jobs, item).phase, Phase::Finished);
        assert!(jobs.snapshot().undo.is_some());
        close(jobs, root);
    });
}
fn bodies(root: &Path) -> (String, String, u64) {
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    (
        db.query_row("SELECT body FROM pending", [], |r| r.get(0))
            .unwrap(),
        db.query_row("SELECT body FROM undo_work", [], |r| r.get(0))
            .unwrap(),
        db.query_row("SELECT count(*) FROM garbage", [], |r| r.get(0))
            .unwrap(),
    )
}
#[test]
fn changed_undo_journals_cannot_borrow_another_work_binding() {
    fixture(|root, output, stage| {
        let bytes = b"Preserved bound Undo work";
        fs::write(stage.payload(), bytes).unwrap();
        let before = bodies(root);
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        let mut changed: serde_json::Value = serde_json::from_str(&before.0).unwrap();
        changed["ordinal"] = serde_json::json!(999);
        db.execute("UPDATE pending SET body=?1", [changed.to_string()])
            .unwrap();
        let jobs = start(root);
        let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
        assert_ne!(terminal(&jobs, id).phase, Phase::Finished);
        let item = partial(&jobs, bytes);
        assert!(!item.can_keep_in_recovery());
        drop(item);
        assert_eq!(fs::read(output.join("item")).unwrap().len(), 128 * 1024);
        close(jobs, root);
        db.execute("UPDATE pending SET body=?1", [&before.0])
            .unwrap();
        let jobs = start(root);
        let item = partial(&jobs, bytes);
        assert!(item.can_keep_in_recovery());
        assert_eq!(keep(&jobs, item).phase, Phase::Finished);
        close(jobs, root);
    });
}
#[test]
fn failed_undo_retention_transactions_preserve_the_exact_journal_and_all_bytes() {
    for trigger in [
        "CREATE TRIGGER refuse_keep BEFORE INSERT ON garbage BEGIN SELECT RAISE(IGNORE); END;",
        "CREATE TRIGGER refuse_keep BEFORE DELETE ON pending BEGIN SELECT RAISE(IGNORE); END;",
        "CREATE TRIGGER refuse_keep BEFORE DELETE ON undo_work BEGIN SELECT RAISE(IGNORE); END;",
    ] {
        fixture(|root, _output, stage| {
            let bytes = b"Unique Undo retention save failure";
            fs::write(stage.payload(), bytes).unwrap();
            let jobs = start(root);
            let item = partial(&jobs, bytes);
            assert!(item.can_keep_in_recovery());
            let before = bodies(root);
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            db.execute_batch(trigger).unwrap();
            assert_ne!(keep(&jobs, item).phase, Phase::Finished);
            assert_eq!(bodies(root), before);
            assert_eq!(fs::read(stage.payload()).unwrap(), bytes);
            db.execute_batch("DROP TRIGGER refuse_keep").unwrap();
            let item = partial(&jobs, bytes);
            assert_eq!(keep(&jobs, item).phase, Phase::Finished);
            close(jobs, root);
        });
    }
}
#[test]
fn replacing_an_undo_work_folder_refuses_keep_and_preserves_both_versions() {
    fixture(|root, _output, stage| {
        let bytes = b"Original interrupted Undo work";
        fs::write(stage.payload(), bytes).unwrap();
        let jobs = start(root);
        let item = partial(&jobs, bytes);
        assert!(item.can_keep_in_recovery());
        let before = bodies(root);
        let held = root.join("source/held-work");
        fs::rename(&stage.directory, &held).unwrap();
        fs::create_dir(&stage.directory).unwrap();
        fs::write(stage.payload(), b"Foreign work").unwrap();
        assert_ne!(keep(&jobs, item).phase, Phase::Finished);
        assert_eq!(bodies(root), before);
        assert_eq!(fs::read(stage.payload()).unwrap(), b"Foreign work");
        fs::rename(&stage.directory, root.join("source/kept-foreign-work")).unwrap();
        fs::rename(&held, &stage.directory).unwrap();
        let item = partial(&jobs, bytes);
        assert_eq!(keep(&jobs, item).phase, Phase::Finished);
        close(jobs, root);
    });
}
