//! Whole-entry Moves through the public worker, killed before its task checkpoint.
use super::*;
use crate::operation_jobs::{OperationJobs, Phase, Request, TransferPart};
use std::{
    ffi::OsString,
    os::unix::{ffi::OsStringExt, fs::MetadataExt, process::ExitStatusExt},
    time::{Duration, Instant},
};

fn leaf() -> OsString {
    OsString::from_vec(b"a-item-\xfd".to_vec())
}
fn source(root: &Path) -> PathBuf {
    root.join("incoming").join(leaf())
}
fn target(root: &Path) -> PathBuf {
    root.join("outgoing").join(leaf())
}
fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            Instant::now() < end,
            "private whole-entry worker did not settle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn receive<T>(mut reply: futures_channel::oneshot::Receiver<T>) -> T {
    wait(|| {
        reply
            .try_recv()
            .expect("private whole-entry worker dropped reply")
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
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => None,
        Err(error) => panic!("{error}"),
    }));
}
pub(in crate::operations) fn boundary(point: &str, path: &Path) {
    let Some(root) = std::env::var_os("CANOPA_WHOLE_MOVE_ROOT").map(PathBuf::from) else {
        return;
    };
    if path != source(&root) || std::env::var("CANOPA_WHOLE_MOVE_POINT").as_deref() != Ok(point) {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-whole-move-")
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
    panic!("private whole-entry worker was not killed");
}
fn fixture(point: &str, shape: &str, run: impl FnOnce(&Path, u64)) {
    let root = tempfile::Builder::new()
        .prefix("canopa-whole-move-")
        .tempdir()
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::move_boundary::move_child",
            ])
            .env("CANOPA_WHOLE_MOVE_ROOT", root.path())
            .env("CANOPA_WHOLE_MOVE_POINT", point)
            .env("CANOPA_WHOLE_MOVE_SHAPE", shape)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9), "{point}/{shape}");
        let id =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        run(root.path(), id);
    }));
    if let Err(error) = checked {
        eprintln!(
            "retained whole-entry Move failure root: {}",
            root.keep().display()
        );
        std::panic::resume_unwind(error);
    }
}
pub(in crate::operations) fn cancel_return(path: &Path, cancellation: &Cancellation) {
    let Some(root) = std::env::var_os("CANOPA_WHOLE_MOVE_ROOT").map(PathBuf::from) else {
        return;
    };
    if path == source(&root)
        && matches!(
            std::env::var("CANOPA_WHOLE_MOVE_POINT").as_deref(),
            Ok("returning" | "returned")
        )
    {
        cancellation.cancel();
    }
}

#[test]
fn returned_whole_entry_claims_restore_their_original_unfinished_task() {
    for point in ["intent", "ready", "returning", "returned"] {
        for shape in ["file", "link", "folder"] {
            fixture(point, shape, |root, id| {
                let jobs = start(root);
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{point}/{shape}: {done:?}");
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Stopped, "{point}/{shape}: {row:?}");
                assert_eq!(row.transfer.as_ref().unwrap().completed, 0);
                assert!(row.transfer_actions.remaining && !row.transfer_actions.retry);
                assert!(fs::symlink_metadata(target(root)).is_err());
                let inode = fs::symlink_metadata(source(root)).unwrap().ino();
                close(jobs, root);
                let jobs = start(root);
                let next =
                    receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == next && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{point}/{shape}: {done:?}");
                assert_eq!(done.transfer.as_ref().unwrap().completed, 1);
                assert_eq!(fs::symlink_metadata(target(root)).unwrap().ino(), inode);
                assert!(fs::symlink_metadata(source(root)).is_err());
                receive(jobs.shutdown().unwrap()).unwrap();
            });
        }
    }
}
#[test]
fn a_completed_whole_entry_move_restores_its_result_without_reopening_its_entries() {
    for shape in ["file", "link", "folder"] {
        fixture("finished", shape, |root, id| {
            let inode = fs::symlink_metadata(target(root)).unwrap().ino();
            fs::write(source(root), b"New source occupant").unwrap();
            if shape == "folder" {
                fs::write(target(root).join("child"), b"Edited completed output").unwrap();
            } else if shape == "file" {
                fs::write(target(root), b"Edited completed output").unwrap();
            }
            for _ in 0..2 {
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Finished, "{shape}: {row:?}");
                assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
                assert_eq!(row.transfer.as_ref().unwrap().bytes, 0);
                assert!(!row.transfer_actions.remaining && !row.transfer_actions.retry);
                assert_eq!(fs::read(source(root)).unwrap(), b"New source occupant");
                assert_eq!(fs::symlink_metadata(target(root)).unwrap().ino(), inode);
                if shape == "link" {
                    assert_eq!(
                        fs::read_link(target(root)).unwrap(),
                        PathBuf::from(OsString::from_vec(b"missing-\xff".to_vec()))
                    );
                } else {
                    assert_eq!(
                        fs::read(if shape == "folder" {
                            target(root).join("child")
                        } else {
                            target(root)
                        })
                        .unwrap(),
                        b"Edited completed output"
                    );
                }
                close(jobs, root);
            }
        });
    }
}
#[test]
fn returned_whole_entry_claims_refuse_missing_or_changed_task_receipts() {
    for fault in ["missing", "changed"] {
        fixture("ready", "file", |root, id| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
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
                assert!(
                    !row.transfer_actions.remaining && !row.transfer_actions.retry,
                    "{fault}: {row:?}"
                );
                assert_eq!(row.phase, Phase::Interrupted, "{fault}: {row:?}");
                let pending: String = db
                    .query_row("SELECT body FROM pending", [], |r| r.get(0))
                    .unwrap();
                let Pending::Forward {
                    stage: Some(stage), ..
                } = serde_json::from_str(&pending).unwrap()
                else {
                    panic!("expected preserved Move intent")
                };
                let preserved = if source(root).exists() {
                    source(root)
                } else {
                    stage.payload()
                };
                assert_eq!(fs::read(preserved).unwrap(), b"Original whole-entry bytes");
                assert!(!target(root).exists());
                close(jobs, root);
            }
        });
    }
}

#[test]
fn a_second_crash_after_whole_entry_return_retires_only_its_uncommitted_step() {
    fixture("ready", "file", |root, id| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::move_boundary::review_move_child",
            ])
            .env("CANOPA_WHOLE_MOVE_ROOT", root)
            .env("CANOPA_WHOLE_MOVE_POINT", "return-preparing")
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
            stage.entry_move.is_none()
                && stage.payload_identity.is_none()
                && !stage.sibling_payload
        );
        fs::write(stage.payload(), b"Foreign private payload").unwrap();
        fs::write(root.join("incoming/unselected"), b"New unselected data").unwrap();
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
            assert_eq!(
                fs::read(source(root)).unwrap(),
                b"Original whole-entry bytes"
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
        let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
        let done = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == next && !j.phase.holds_application())
        });
        assert_eq!(done.phase, Phase::Finished, "{done:?}");
        assert_eq!(
            fs::read(target(root)).unwrap(),
            b"Original whole-entry bytes"
        );
        assert_eq!(
            fs::read(root.join("incoming/unselected")).unwrap(),
            b"New unselected data"
        );
        receive(jobs.shutdown().unwrap()).unwrap();
    });
}

#[test]
fn published_whole_entry_moves_finish_only_their_original_publication() {
    for point in ["publishing", "published"] {
        for shape in ["file", "link", "folder"] {
            fixture(point, shape, |root, id| {
                fs::write(source(root), b"New source occupant").unwrap();
                if point == "published" && shape != "link" {
                    fs::write(
                        if shape == "folder" {
                            target(root).join("child")
                        } else {
                            target(root)
                        },
                        b"Edited published output",
                    )
                    .unwrap();
                }
                let jobs = start(root);
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{point}/{shape}: {done:?}");
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Finished, "{point}/{shape}: {row:?}");
                assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
                assert_eq!(
                    row.transfer.as_ref().unwrap().destinations,
                    vec![target(root)]
                );
                assert_eq!(fs::read(source(root)).unwrap(), b"New source occupant");
                if shape == "link" {
                    assert_eq!(
                        fs::read_link(target(root)).unwrap(),
                        PathBuf::from(OsString::from_vec(b"missing-\xff".to_vec()))
                    );
                } else {
                    assert_eq!(
                        fs::read(if shape == "folder" {
                            target(root).join("child")
                        } else {
                            target(root)
                        })
                        .unwrap(),
                        if point == "published" {
                            b"Edited published output".as_slice()
                        } else if shape == "folder" {
                            b"Original folder child".as_slice()
                        } else {
                            b"Original whole-entry bytes".as_slice()
                        }
                    );
                }
                close(jobs, root);
                let jobs = start(root);
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
fn a_missing_published_move_never_infers_a_return_from_its_new_source_name() {
    for replacement in [false, true] {
        fixture("published", "file", |root, id| {
            fs::rename(target(root), source(root)).unwrap();
            fs::write(source(root), b"New namespace entry on original inode").unwrap();
            if replacement {
                fs::write(target(root), b"Replacement destination").unwrap();
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
                assert_ne!(done.phase, Phase::Finished, "{done:?}");
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Interrupted, "{row:?}");
                assert!(!row.transfer_actions.remaining && !row.transfer_actions.retry);
                assert_eq!(
                    fs::read(source(root)).unwrap(),
                    b"New namespace entry on original inode"
                );
                if replacement {
                    assert_eq!(fs::read(target(root)).unwrap(), b"Replacement destination");
                } else {
                    assert!(!target(root).exists());
                }
                close(jobs, root);
            }
        });
    }
}

#[test]
fn a_completed_keep_both_move_leaves_only_later_batch_work_to_continue() {
    fixture("finished", "batch-keep-both", |root, id| {
        let mut name = leaf();
        name.push(" (2)");
        let chosen = root.join("outgoing").join(name);
        fs::rename(&chosen, root.join("completed-original")).unwrap();
        fs::write(&chosen, b"New chosen-name occupant").unwrap();
        fs::write(source(root), b"New source occupant").unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            let row = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id)
                .unwrap();
            assert_eq!(row.phase, Phase::Stopped, "{row:?}");
            assert!(row.transfer_actions.remaining);
            assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
            assert_eq!(
                row.transfer.as_ref().unwrap().destinations,
                vec![chosen.clone()]
            );
            close(jobs, root);
        }
        let jobs = start(root);
        let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
        let done = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == next && !j.phase.holds_application())
        });
        assert_eq!(done.phase, Phase::Finished, "{done:?}");
        assert_eq!(done.transfer.as_ref().unwrap().completed, 1);
        assert_eq!(
            fs::read(root.join("outgoing/z-tail")).unwrap(),
            b"Later selected root"
        );
        assert_eq!(fs::read(target(root)).unwrap(), b"Existing conflict");
        assert_eq!(fs::read(source(root)).unwrap(), b"New source occupant");
        assert_eq!(fs::read(chosen).unwrap(), b"New chosen-name occupant");
        assert_eq!(
            fs::read(root.join("completed-original")).unwrap(),
            b"Original whole-entry bytes"
        );
        receive(jobs.shutdown().unwrap()).unwrap();
    });
}

#[test]
fn completed_whole_entry_moves_refuse_missing_or_changed_task_proofs() {
    for fault in ["receipt", "body", "position", "task", "head"] {
        fixture("finished", "file", |root, id| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            db.execute_batch(match fault {
                "receipt" => "DELETE FROM request_transfer_cursor_steps",
                "body" => "UPDATE request_transfer_cursor_steps SET body='{}'",
                "position" => {
                    "UPDATE request_transfer_cursor_steps SET position=x'ffffffffffffffff'"
                }
                "task" => "UPDATE request_transfer_cursor_steps SET task_sha256=zeroblob(32)",
                "head" => "DELETE FROM request_transfer_tasks WHERE role=0",
                _ => unreachable!(),
            })
            .unwrap();
            fs::write(source(root), b"New source occupant").unwrap();
            fs::write(target(root), b"Edited completed output").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Interrupted, "{fault}: {row:?}");
                assert!(!row.transfer_actions.remaining && !row.transfer_actions.retry);
                assert!(
                    receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).is_err()
                );
                assert_eq!(fs::read(source(root)).unwrap(), b"New source occupant");
                assert_eq!(fs::read(target(root)).unwrap(), b"Edited completed output");
                close(jobs, root);
            }
        });
    }
}

#[test]
fn returned_whole_entry_disposition_rolls_back_both_receipt_and_empty_intent() {
    for table in ["pending", "steps"] {
        fixture("returned", "file", |root, id| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            let before: String = db
                .query_row("SELECT body FROM pending", [], |r| r.get(0))
                .unwrap();
            db.execute_batch(if table == "pending" {
                r#"CREATE TRIGGER refuse_move_return BEFORE INSERT ON pending WHEN instr(NEW.body,'"direction":"preparing"')>0 BEGIN SELECT RAISE(ABORT,'private Move return refusal'); END"#
            } else { "CREATE TRIGGER refuse_move_return BEFORE DELETE ON request_transfer_cursor_steps BEGIN SELECT RAISE(ABORT,'private Move receipt refusal'); END" }).unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                assert_ne!(done.phase, Phase::Finished, "{table}: {done:?}");
                assert_eq!(
                    db.query_row("SELECT body FROM pending", [], |r| r.get::<_, String>(0))
                        .unwrap(),
                    before
                );
                assert_eq!(
                    db.query_row(
                        "SELECT count(*) FROM request_transfer_cursor_steps",
                        [],
                        |r| r.get::<_, u64>(0)
                    )
                    .unwrap(),
                    1
                );
                assert_eq!(
                    fs::read(source(root)).unwrap(),
                    b"Original whole-entry bytes"
                );
                close(jobs, root);
            }
            db.execute_batch("DROP TRIGGER refuse_move_return").unwrap();
            let jobs = start(root);
            let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == review && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == next && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            receive(jobs.shutdown().unwrap()).unwrap();
        });
    }
}

#[test]
#[ignore = "private review worker killed after whole-entry return disposition"]
fn review_move_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_WHOLE_MOVE_ROOT").unwrap());
    let jobs = start(&root);
    receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    std::thread::sleep(Duration::from_secs(25));
    panic!("private whole-entry review boundary was not reached");
}

#[test]
#[ignore = "private public-worker fixture killed at the actual whole-entry boundary"]
fn move_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_WHOLE_MOVE_ROOT").unwrap());
    fs::create_dir(root.join("incoming")).unwrap();
    fs::create_dir(root.join("outgoing")).unwrap();
    match std::env::var("CANOPA_WHOLE_MOVE_SHAPE").unwrap().as_str() {
        "file" | "batch-keep-both" => {
            fs::write(source(&root), b"Original whole-entry bytes").unwrap()
        }
        "link" => {
            std::os::unix::fs::symlink(OsString::from_vec(b"missing-\xff".to_vec()), source(&root))
                .unwrap()
        }
        "folder" => {
            fs::create_dir(source(&root)).unwrap();
            fs::write(source(&root).join("child"), b"Original folder child").unwrap();
        }
        _ => panic!("unknown private Move shape"),
    }
    let mut sources = vec![source(&root)];
    let batch = std::env::var("CANOPA_WHOLE_MOVE_SHAPE").as_deref() == Ok("batch-keep-both");
    if batch {
        fs::write(target(&root), b"Existing conflict").unwrap();
        let tail = root.join("incoming/z-tail");
        fs::write(&tail, b"Later selected root").unwrap();
        sources.push(tail);
    }
    let jobs = start(&root);
    let id = receive(
        jobs.submit(
            Request::Transfer(TransferRequest {
                kind: TransferKind::Move,
                sources,
                destination: root.join("outgoing"),
            }),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    if batch {
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
                choice: ConflictChoice::KeepBoth,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    }
    std::thread::sleep(Duration::from_secs(25));
    panic!("private whole-entry boundary was not reached");
}
