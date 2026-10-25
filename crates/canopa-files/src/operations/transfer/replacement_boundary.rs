//! Reviewed replacement tasks through the real public operation worker.
use super::*;
use crate::operation_jobs::{OperationJobs, Phase, Request, TransferPart};
use std::{
    ffi::OsString,
    os::unix::{ffi::OsStringExt, fs::MetadataExt, process::ExitStatusExt},
    time::{Duration, Instant},
};
fn leaf() -> OsString {
    OsString::from_vec(b"a-replacement-\xfd".to_vec())
}
fn source(root: &Path) -> PathBuf {
    root.join("incoming").join(leaf())
}
fn target(output: &Path) -> PathBuf {
    output.join(leaf())
}
fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            Instant::now() < end,
            "private replacement worker did not settle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn receive<T>(mut reply: futures_channel::oneshot::Receiver<T>) -> T {
    wait(|| {
        reply
            .try_recv()
            .expect("private replacement worker dropped reply")
    })
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
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => None,
        Err(e) => panic!("{e}"),
    }));
}
pub(in crate::operations) fn boundary(point: &str, path: &Path) {
    let Some(root) = std::env::var_os("CANOPA_REPLACEMENT_TASK_ROOT").map(PathBuf::from) else {
        return;
    };
    let output = PathBuf::from(std::env::var_os("CANOPA_REPLACEMENT_TASK_OUTPUT").unwrap());
    let wanted = std::env::var("CANOPA_REPLACEMENT_TASK_POINT").unwrap();
    let expected = if matches!(wanted.as_str(), "finished-link" | "batch-finished") {
        "finished"
    } else {
        &wanted
    };
    if path != target(&output) || point != expected {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-replacement-task-")
    );
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let id: u64 = db
        .query_row("SELECT request FROM request_transfer_cursors", [], |r| {
            r.get(0)
        })
        .unwrap();
    fs::write(root.join("marker.json"), serde_json::to_vec(&id).unwrap()).unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private replacement worker was not killed");
}
fn resume(jobs: &OperationJobs, id: u64, review: bool) -> crate::operation_jobs::JobSnapshot {
    let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
    if review {
        let question = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == next)
                .and_then(|j| j.decision)
        });
        jobs.resolve_transfer(
            next,
            question.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    }
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|j| j.id == next && !j.phase.holds_application())
    })
}
fn fixture(kind: &str, point: &str, run: impl FnOnce(&Path, &Path, u64)) {
    let root = tempfile::Builder::new()
        .prefix("canopa-replacement-task-")
        .tempdir()
        .unwrap();
    let output = if kind == "cross" {
        tempfile::Builder::new()
            .prefix(".canopa-replacement-task-output-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap()
    } else {
        tempfile::Builder::new()
            .prefix("canopa-replacement-task-output-")
            .tempdir()
            .unwrap()
    };
    assert_eq!(
        fs::metadata(root.path()).unwrap().dev() != fs::metadata(output.path()).unwrap().dev(),
        kind == "cross"
    );
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::replacement_boundary::replacement_child",
            ])
            .env("CANOPA_REPLACEMENT_TASK_ROOT", root.path())
            .env("CANOPA_REPLACEMENT_TASK_OUTPUT", output.path())
            .env("CANOPA_REPLACEMENT_TASK_KIND", kind)
            .env("CANOPA_REPLACEMENT_TASK_POINT", point)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9), "{kind}/{point}");
        let id =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        run(root.path(), output.path(), id);
    }));
    if let Err(e) = checked {
        eprintln!(
            "retained replacement task failure roots: {} and {}",
            root.keep().display(),
            output.keep().display()
        );
        std::panic::resume_unwind(e);
    }
}
pub(in crate::operations) fn cancel_return(path: &Path, cancellation: &Cancellation) {
    let Some(output) = std::env::var_os("CANOPA_REPLACEMENT_TASK_OUTPUT").map(PathBuf::from) else {
        return;
    };
    if path == target(&output)
        && matches!(
            std::env::var("CANOPA_REPLACEMENT_TASK_POINT").as_deref(),
            Ok("returning" | "returned")
        )
    {
        cancellation.cancel();
    }
}

#[test]
fn replacement_returns_refuse_missing_or_changed_original_task_receipts() {
    for kind in ["copy", "move", "cross"] {
        for fault in ["missing", "changed"] {
            fixture(kind, "backup", |root, output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                let pending: String = db
                    .query_row("SELECT body FROM pending", [], |r| r.get(0))
                    .unwrap();
                let Pending::Forward {
                    stage: Some(stage), ..
                } = serde_json::from_str(&pending).unwrap()
                else {
                    panic!("expected replacement claim")
                };
                db.execute_batch(if fault == "missing" {
                    "DELETE FROM request_transfer_cursor_steps"
                } else {
                    "UPDATE request_transfer_cursor_steps SET body_sha256=zeroblob(32)"
                })
                .unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                    wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == review && !j.phase.holds_application())
                    });
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert_eq!(row.phase, Phase::Interrupted, "{kind}/{fault}: {row:?}");
                    assert!(!row.transfer_actions.remaining && !row.transfer_actions.retry);
                    assert_eq!(fs::read(source(root)).unwrap(), b"Selected incoming bytes");
                    assert_eq!(
                        fs::read(stage.directory.join("previous")).unwrap(),
                        b"Reviewed old destination"
                    );
                    assert!(!target(output).exists());
                    close(jobs, root);
                }
            });
        }
    }
}

#[test]
fn verified_replacement_refresh_is_atomic_with_history_and_cleanup() {
    for kind in ["copy", "move"] {
        for table in ["steps", "changes"] {
            fixture(kind, "published", |root, output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                let before: String = db
                    .query_row("SELECT body FROM pending", [], |r| r.get(0))
                    .unwrap();
                let receipt: String = db
                    .query_row("SELECT body FROM request_transfer_cursor_steps", [], |r| {
                        r.get(0)
                    })
                    .unwrap();
                db.execute_batch(if table == "steps" {
                    "CREATE TRIGGER refuse_replacement_refresh BEFORE UPDATE ON request_transfer_cursor_steps BEGIN SELECT RAISE(ABORT,'private replacement refresh refusal'); END"
                } else { "CREATE TRIGGER refuse_replacement_refresh BEFORE INSERT ON changes BEGIN SELECT RAISE(ABORT,'private replacement history refusal'); END" }).unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                    let done = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == review && !j.phase.holds_application())
                    });
                    assert_ne!(done.phase, Phase::Finished, "{kind}/{table}: {done:?}");
                    assert_eq!(
                        db.query_row("SELECT body FROM pending", [], |r| r.get::<_, String>(0))
                            .unwrap(),
                        before
                    );
                    assert_eq!(
                        db.query_row("SELECT body FROM request_transfer_cursor_steps", [], |r| {
                            r.get::<_, String>(0)
                        })
                        .unwrap(),
                        receipt
                    );
                    assert_eq!(
                        db.query_row("SELECT count(*) FROM changes", [], |r| r.get::<_, u64>(0))
                            .unwrap(),
                        0
                    );
                    assert_eq!(
                        fs::read(target(output)).unwrap(),
                        b"Selected incoming bytes"
                    );
                    close(jobs, root);
                }
                db.execute_batch("DROP TRIGGER refuse_replacement_refresh")
                    .unwrap();
                let jobs = start(root);
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{done:?}");
                assert_eq!(
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap()
                        .phase,
                    Phase::Finished
                );
                receive(jobs.shutdown().unwrap()).unwrap();
            });
        }
    }
}
#[test]
fn completed_reviewed_replacements_restore_their_actual_results() {
    for kind in ["copy", "move"] {
        for link in [false, true] {
            fixture(
                kind,
                if link { "finished-link" } else { "finished" },
                |root, output, id| {
                    let inode = fs::symlink_metadata(target(output)).unwrap().ino();
                    if !link {
                        fs::write(target(output), b"Edited completed output").unwrap();
                    }
                    if link && fs::symlink_metadata(source(root)).is_ok() {
                        fs::remove_file(source(root)).unwrap();
                    }
                    fs::write(source(root), b"New source contents").unwrap();
                    let db = rusqlite::Connection::open(root.join("history")).unwrap();
                    let body: String = db
                        .query_row("SELECT body FROM changes WHERE ordinal=0", [], |r| r.get(0))
                        .unwrap();
                    let change: Change = serde_json::from_str(&body).unwrap();
                    let previous = change.retained().unwrap().stage.directory.join("previous");
                    for _ in 0..2 {
                        let jobs = start(root);
                        let row = jobs
                            .snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == id)
                            .unwrap();
                        assert_eq!(row.phase, Phase::Finished, "{kind}/{link}: {row:?}");
                        assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
                        assert!(!row.transfer_actions.remaining && !row.transfer_actions.retry);
                        assert_eq!(fs::symlink_metadata(target(output)).unwrap().ino(), inode);
                        if link {
                            assert_eq!(
                                fs::read_link(target(output)).unwrap(),
                                PathBuf::from(OsString::from_vec(b"new-\xfe".to_vec()))
                            );
                            assert_eq!(
                                fs::read_link(&previous).unwrap(),
                                PathBuf::from(OsString::from_vec(b"old-\xff".to_vec()))
                            );
                        } else {
                            assert_eq!(
                                fs::read(target(output)).unwrap(),
                                b"Edited completed output"
                            );
                            assert_eq!(fs::read(&previous).unwrap(), b"Reviewed old destination");
                        }
                        assert_eq!(fs::read(source(root)).unwrap(), b"New source contents");
                        close(jobs, root);
                    }
                },
            );
        }
    }
}
#[test]
fn replacement_publication_restores_only_the_proved_step_and_original_cleanup() {
    for kind in ["copy", "move", "cross"] {
        for point in ["publishing", "published"] {
            fixture(kind, point, |root, output, id| {
                let jobs = start(root);
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{kind}/{point}: {done:?}");
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                let inode = fs::metadata(target(output)).unwrap().ino();
                if kind == "cross" {
                    assert_eq!(row.phase, Phase::Stopped, "{row:?}");
                    assert!(row.transfer_actions.remaining && !row.transfer_actions.retry);
                    assert_eq!(row.transfer.as_ref().unwrap().completed, 0);
                    assert_eq!(fs::read(source(root)).unwrap(), b"Selected incoming bytes");
                    close(jobs, root);
                    let jobs = start(root);
                    let done = resume(&jobs, id, false);
                    assert_eq!(done.phase, Phase::Finished, "{done:?}");
                    assert_eq!(done.transfer.as_ref().unwrap().completed, 1);
                    assert_eq!(fs::metadata(target(output)).unwrap().ino(), inode);
                    assert!(!source(root).exists());
                    receive(jobs.shutdown().unwrap()).unwrap();
                } else {
                    assert_eq!(row.phase, Phase::Finished, "{row:?}");
                    assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
                    receive(jobs.shutdown().unwrap()).unwrap();
                }
                assert_eq!(
                    fs::read(target(output)).unwrap(),
                    b"Selected incoming bytes"
                );
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                let body: String = db
                    .query_row(
                        "SELECT body FROM changes WHERE operation=1 AND ordinal=0",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap();
                let change: Change = serde_json::from_str(&body).unwrap();
                assert_eq!(
                    fs::read(change.retained().unwrap().stage.directory.join("previous")).unwrap(),
                    b"Reviewed old destination"
                );
            });
        }
    }
}

#[test]
fn returned_replacements_resume_only_after_a_fresh_conflict_answer() {
    for kind in ["copy", "move", "cross"] {
        for point in ["unclaimed", "ready", "backup", "returning", "returned"] {
            fixture(kind, point, |root, output, id| {
                let source_inode = fs::metadata(source(root)).unwrap().ino();
                let jobs = start(root);
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{kind}/{point}: {done:?}");
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Stopped, "{kind}/{point}: {row:?}");
                assert_eq!(row.transfer.as_ref().unwrap().completed, 0);
                assert_eq!(fs::metadata(source(root)).unwrap().ino(), source_inode);
                assert_eq!(fs::read(source(root)).unwrap(), b"Selected incoming bytes");
                assert_eq!(
                    fs::read(target(output)).unwrap(),
                    b"Reviewed old destination"
                );
                close(jobs, root);
                let jobs = start(root);
                let done = resume(&jobs, id, true);
                assert_eq!(done.phase, Phase::Finished, "{kind}/{point}: {done:?}");
                assert_eq!(done.transfer.as_ref().unwrap().completed, 1);
                assert_eq!(
                    fs::read(target(output)).unwrap(),
                    b"Selected incoming bytes"
                );
                receive(jobs.shutdown().unwrap()).unwrap();
            });
        }
    }
}

#[test]
fn a_second_crash_after_replacement_return_keeps_only_empty_cleanup_rights() {
    for kind in ["copy", "move"] {
        fixture(kind, "returned", |root, output, id| {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::transfer::replacement_boundary::review_replacement_child",
                ])
                .env("CANOPA_REPLACEMENT_TASK_ROOT", root)
                .env("CANOPA_REPLACEMENT_TASK_OUTPUT", output)
                .env("CANOPA_REPLACEMENT_TASK_POINT", "return-preparing")
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM request_transfer_cursor_steps",
                    [],
                    |r| r.get::<_, u64>(0)
                )
                .unwrap(),
                0
            );
            let body: String = db
                .query_row("SELECT body FROM pending", [], |r| r.get(0))
                .unwrap();
            let Pending::Preparing { stage, .. } = serde_json::from_str(&body).unwrap() else {
                panic!("expected empty returned stage")
            };
            assert!(
                stage.replacement.is_none()
                    && stage.entry_move.is_none()
                    && stage.payload_identity.is_none()
            );
            fs::write(stage.payload(), b"Foreign private payload").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                assert_ne!(done.phase, Phase::Finished, "{done:?}");
                assert_eq!(
                    fs::read(stage.payload()).unwrap(),
                    b"Foreign private payload"
                );
                assert_eq!(fs::read(source(root)).unwrap(), b"Selected incoming bytes");
                assert_eq!(
                    fs::read(target(output)).unwrap(),
                    b"Reviewed old destination"
                );
                close(jobs, root);
            }
            fs::remove_file(stage.payload()).unwrap();
            let jobs = start(root);
            let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == review && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            let done = resume(&jobs, id, true);
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            receive(jobs.shutdown().unwrap()).unwrap();
        });
    }
}

#[test]
fn completed_replacement_batches_preserve_new_source_output_and_saved_data() {
    for kind in ["copy", "move"] {
        fixture(kind, "batch-finished", |root, output, id| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            let body: String = db
                .query_row("SELECT body FROM changes WHERE ordinal=0", [], |r| r.get(0))
                .unwrap();
            let change: Change = serde_json::from_str(&body).unwrap();
            let previous = change.retained().unwrap().stage.directory.join("previous");
            fs::rename(target(output), root.join("completed-original")).unwrap();
            fs::write(target(output), b"New output occupant").unwrap();
            fs::write(source(root), b"New source contents").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Stopped, "{row:?}");
                assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
                assert!(row.transfer_actions.remaining);
                close(jobs, root);
            }
            let jobs = start(root);
            let done = resume(&jobs, id, false);
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            assert_eq!(done.transfer.as_ref().unwrap().completed, 1);
            assert_eq!(
                fs::read(output.join("z-tail")).unwrap(),
                b"Later selected root"
            );
            assert_eq!(fs::read(source(root)).unwrap(), b"New source contents");
            assert_eq!(fs::read(target(output)).unwrap(), b"New output occupant");
            assert_eq!(
                fs::read(root.join("completed-original")).unwrap(),
                b"Selected incoming bytes"
            );
            assert_eq!(fs::read(previous).unwrap(), b"Reviewed old destination");
            receive(jobs.shutdown().unwrap()).unwrap();
        });
    }
}

#[test]
fn completed_replacement_tasks_refuse_lost_or_changed_canonical_receipts() {
    for kind in ["copy", "move"] {
        for fault in ["receipt", "body", "task", "head"] {
            fixture(kind, "finished", |root, output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                db.execute_batch(match fault {
                    "receipt" => "DELETE FROM request_transfer_cursor_steps",
                    "body" => "UPDATE request_transfer_cursor_steps SET body='{}'",
                    "task" => "UPDATE request_transfer_cursor_steps SET task_sha256=zeroblob(32)",
                    "head" => "DELETE FROM request_transfer_tasks WHERE role=0",
                    _ => unreachable!(),
                })
                .unwrap();
                fs::write(target(output), b"Edited completed output").unwrap();
                fs::write(source(root), b"New source contents").unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert_eq!(row.phase, Phase::Interrupted, "{kind}/{fault}: {row:?}");
                    assert!(!row.transfer_actions.remaining && !row.transfer_actions.retry);
                    assert_eq!(fs::read(source(root)).unwrap(), b"New source contents");
                    assert_eq!(
                        fs::read(target(output)).unwrap(),
                        b"Edited completed output"
                    );
                    close(jobs, root);
                }
            });
        }
    }
}

#[test]
fn replacement_publication_refuses_substituted_saved_entries_and_parents() {
    for kind in ["copy", "move"] {
        for fault in ["previous", "parent"] {
            fixture(kind, "publishing", |root, output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                let body: String = db
                    .query_row("SELECT body FROM pending", [], |r| r.get(0))
                    .unwrap();
                let Pending::Forward {
                    stage: Some(stage), ..
                } = serde_json::from_str(&body).unwrap()
                else {
                    panic!("expected replacement intent")
                };
                let parked = root.join("parked-original");
                if fault == "previous" {
                    fs::rename(stage.directory.join("previous"), &parked).unwrap();
                    fs::write(
                        stage.directory.join("previous"),
                        b"Reviewed old destination",
                    )
                    .unwrap();
                } else {
                    fs::rename(output, &parked).unwrap();
                    fs::create_dir(output).unwrap();
                    fs::write(output.join("foreign"), b"Foreign parent data").unwrap();
                }
                for _ in 0..2 {
                    let jobs = start(root);
                    let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                    let done = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == review && !j.phase.holds_application())
                    });
                    assert_ne!(done.phase, Phase::Finished, "{kind}/{fault}: {done:?}");
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert!(!row.transfer_actions.remaining && !row.transfer_actions.retry);
                    if fault == "previous" {
                        assert_eq!(
                            fs::read(stage.directory.join("previous")).unwrap(),
                            b"Reviewed old destination"
                        );
                        assert_eq!(fs::read(&parked).unwrap(), b"Reviewed old destination");
                    } else {
                        assert_eq!(
                            fs::read(output.join("foreign")).unwrap(),
                            b"Foreign parent data"
                        );
                    }
                    close(jobs, root);
                }
                if fault == "previous" {
                    fs::remove_file(stage.directory.join("previous")).unwrap();
                    fs::rename(&parked, stage.directory.join("previous")).unwrap();
                } else {
                    fs::remove_file(output.join("foreign")).unwrap();
                    fs::remove_dir(output).unwrap();
                    fs::rename(&parked, output).unwrap();
                }
                let jobs = start(root);
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{done:?}");
                assert_eq!(
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap()
                        .phase,
                    Phase::Finished
                );
                receive(jobs.shutdown().unwrap()).unwrap();
            });
        }
    }
}

#[test]
#[ignore = "private replacement review worker killed after return disposition"]
fn review_replacement_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_REPLACEMENT_TASK_ROOT").unwrap());
    let jobs = start(&root);
    receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    std::thread::sleep(Duration::from_secs(25));
    panic!("private replacement review boundary was not reached");
}

#[test]
#[ignore = "private replacement worker killed at the real claim/publication boundary"]
fn replacement_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_REPLACEMENT_TASK_ROOT").unwrap());
    let output = PathBuf::from(std::env::var_os("CANOPA_REPLACEMENT_TASK_OUTPUT").unwrap());
    fs::create_dir(root.join("incoming")).unwrap();
    let point = std::env::var("CANOPA_REPLACEMENT_TASK_POINT").unwrap();
    if point == "finished-link" {
        std::os::unix::fs::symlink(OsString::from_vec(b"new-\xfe".to_vec()), source(&root))
            .unwrap();
        std::os::unix::fs::symlink(OsString::from_vec(b"old-\xff".to_vec()), target(&output))
            .unwrap();
    } else {
        fs::write(source(&root), b"Selected incoming bytes").unwrap();
        fs::write(target(&output), b"Reviewed old destination").unwrap();
    }
    let mut sources = vec![source(&root)];
    if point == "batch-finished" {
        let tail = root.join("incoming/z-tail");
        fs::write(&tail, b"Later selected root").unwrap();
        sources.push(tail);
    }
    let jobs = start(&root);
    let id = receive(
        jobs.submit(
            Request::Transfer(TransferRequest {
                kind: if std::env::var("CANOPA_REPLACEMENT_TASK_KIND").as_deref() == Ok("copy") {
                    TransferKind::Copy
                } else {
                    TransferKind::Move
                },
                sources,
                destination: output,
            }),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    let question = wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|j| j.id == id)
            .and_then(|j| j.decision)
    });
    jobs.resolve_transfer(
        id,
        question.generation,
        TransferReply::Resolve {
            choice: ConflictChoice::Replace,
            apply_to_remaining: false,
        },
    )
    .unwrap();
    std::thread::sleep(Duration::from_secs(25));
    panic!("private replacement boundary was not reached");
}
