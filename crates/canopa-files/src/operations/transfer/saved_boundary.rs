//! Saved-container tasks through the public worker, with actual retained data.
use super::*;
use crate::operation_jobs::{OperationJobs, Phase, Request, TransferPart};
use rusqlite::OptionalExtension;
use std::{
    os::unix::{ffi::OsStringExt, fs::MetadataExt, process::ExitStatusExt},
    time::{Duration, Instant},
};

fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(v) = probe() {
            return v;
        }
        assert!(Instant::now() < end, "private saved worker did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn receive<T>(mut reply: futures_channel::oneshot::Receiver<T>) -> T {
    wait(|| {
        reply
            .try_recv()
            .expect("private saved worker dropped reply")
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
        Ok(e) => Some(e),
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => None,
        Err(e) => panic!("{e}"),
    }));
}
#[derive(Serialize, Deserialize)]
struct Marker {
    #[serde(with = "crate::path_serde")]
    source: PathBuf,
    #[serde(with = "crate::path_serde")]
    destination: PathBuf,
    request: u64,
    #[serde(with = "crate::path_serde")]
    payload: PathBuf,
    #[serde(with = "crate::path_serde")]
    work: PathBuf,
}
pub(in crate::operations) fn boundary(point: &str, source: &Path, payload: &Path) {
    let Some(root) = std::env::var_os("CANOPA_SAVED_TASK_ROOT").map(PathBuf::from) else {
        return;
    };
    if std::env::var("CANOPA_SAVED_TASK_POINT").as_deref() != Ok(point) {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-saved-task-")
    );
    let selected: PathBuf =
        serde_json::from_slice(&fs::read(root.join("selected.json")).unwrap()).unwrap();
    if source != selected {
        return;
    }
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let request = db
        .query_row("SELECT request FROM request_transfer_cursors", [], |r| {
            r.get(0)
        })
        .unwrap();
    let body: Option<String> = db
        .query_row("SELECT body FROM request_transfer_cursor_steps", [], |r| {
            r.get(0)
        })
        .optional()
        .unwrap();
    let destination = body
        .map(|body| {
            serde_json::from_str::<Change>(&body)
                .unwrap()
                .forward_path()
                .to_owned()
        })
        .unwrap_or_else(|| root.join("outgoing").join(source.file_name().unwrap()));
    let pending: Option<String> = db
        .query_row("SELECT body FROM pending", [], |r| r.get(0))
        .optional()
        .unwrap();
    let work = pending
        .and_then(
            |body| match serde_json::from_str::<Pending>(&body).unwrap() {
                Pending::Forward {
                    stage: Some(stage), ..
                }
                | Pending::Preparing { stage, .. } => Some(stage.directory),
                _ => None,
            },
        )
        .unwrap_or_default();
    fs::write(
        root.join("marker.json"),
        serde_json::to_vec(&Marker {
            source: source.to_owned(),
            destination,
            payload: payload.to_owned(),
            work,
            request,
        })
        .unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private saved worker was not killed");
}
fn fixture(point: &str, run: impl FnOnce(&Path, Marker)) {
    fixture_shape(point, "plain", run);
}
fn fixture_shape(point: &str, shape: &str, run: impl FnOnce(&Path, Marker)) {
    let root = tempfile::Builder::new()
        .prefix("canopa-saved-task-")
        .tempdir()
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::saved_boundary::saved_child",
            ])
            .env("CANOPA_SAVED_TASK_ROOT", root.path())
            .env("CANOPA_SAVED_TASK_POINT", point)
            .env("CANOPA_SAVED_TASK_SHAPE", shape)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let marker =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        run(root.path(), marker);
    }));
    if let Err(e) = checked {
        eprintln!(
            "retained saved-task failure root: {}",
            root.keep().display()
        );
        std::panic::resume_unwind(e);
    }
}
#[test]
fn a_completed_saved_container_move_restores_only_its_result() {
    fixture("finished", |root, marker| {
        let inode = fs::metadata(&marker.destination).unwrap().ino();
        fs::create_dir(&marker.source).unwrap();
        fs::write(marker.source.join("foreign"), b"New source occupant").unwrap();
        fs::write(
            marker.destination.join("previous"),
            b"Edited completed saved data",
        )
        .unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            let row = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == marker.request)
                .unwrap();
            assert_eq!(row.phase, Phase::Finished, "{row:?}");
            assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
            assert!(!row.transfer_actions.retry && !row.transfer_actions.remaining);
            assert_eq!(fs::metadata(&marker.destination).unwrap().ino(), inode);
            assert_eq!(
                fs::read(marker.destination.join("previous")).unwrap(),
                b"Edited completed saved data"
            );
            assert_eq!(
                fs::read(marker.source.join("foreign")).unwrap(),
                b"New source occupant"
            );
            close(jobs, root);
        }
    });
}

fn reconcile(jobs: &OperationJobs) -> crate::operation_jobs::JobSnapshot {
    let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|j| j.id == id && !j.phase.holds_application())
    })
}
fn job(jobs: &OperationJobs, id: u64) -> crate::operation_jobs::JobSnapshot {
    jobs.snapshot()
        .jobs
        .into_iter()
        .find(|j| j.id == id)
        .unwrap()
}
pub(in crate::operations) fn cancel_return(source: &Path, cancellation: &Cancellation) {
    if let Some(root) = std::env::var_os("CANOPA_SAVED_TASK_ROOT").map(PathBuf::from)
        && matches!(
            std::env::var("CANOPA_SAVED_TASK_POINT").as_deref(),
            Ok("returning" | "returned" | "return-preparing")
        )
    {
        let selected: PathBuf =
            serde_json::from_slice(&fs::read(root.join("selected.json")).unwrap()).unwrap();
        if source == selected {
            cancellation.cancel();
        }
    }
}
#[test]
fn returned_saved_container_cleanup_preserves_an_unexpected_private_payload() {
    fixture("return-preparing", |root, marker| {
        let foreign = marker.work.join("folder");
        fs::write(&foreign, b"Foreign private payload").unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            let review = reconcile(&jobs);
            assert_ne!(review.phase, Phase::Finished, "{review:?}");
            assert_eq!(job(&jobs, marker.request).phase, Phase::Interrupted);
            assert_eq!(fs::read(&foreign).unwrap(), b"Foreign private payload");
            assert_eq!(
                fs::read(marker.source.join("previous")).unwrap(),
                b"Original saved data"
            );
            close(jobs, root);
        }
        fs::remove_file(&foreign).unwrap();
        let jobs = start(root);
        let review = reconcile(&jobs);
        assert_eq!(review.phase, Phase::Finished, "{review:?}");
        let row = job(&jobs, marker.request);
        assert_eq!(row.phase, Phase::Stopped, "{row:?}");
        assert!(row.transfer_actions.remaining);
        close(jobs, root);
    });
}
#[test]
fn saved_container_catalog_and_task_completion_roll_back_together() {
    fixture("published", |root, marker| {
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        let original: String = db
            .query_row(
                "SELECT body FROM changes WHERE operation=1 AND ordinal=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        db.execute_batch("CREATE TRIGGER refuse_saved_completion BEFORE INSERT ON changes WHEN NEW.operation=2 BEGIN SELECT RAISE(ABORT,'private saved completion refusal'); END;").unwrap();
        drop(db);
        let jobs = start(root);
        let review = reconcile(&jobs);
        assert_ne!(review.phase, Phase::Finished, "{review:?}");
        assert_eq!(job(&jobs, marker.request).phase, Phase::Interrupted);
        assert_eq!(
            fs::read(marker.destination.join("previous")).unwrap(),
            b"Original saved data"
        );
        close(jobs, root);
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        let after: String = db
            .query_row(
                "SELECT body FROM changes WHERE operation=1 AND ordinal=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after, original);
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM request_transfer_cursor_steps",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
            1
        );
        db.execute_batch("DROP TRIGGER refuse_saved_completion;")
            .unwrap();
        drop(db);
        let jobs = start(root);
        let review = reconcile(&jobs);
        assert_eq!(review.phase, Phase::Finished, "{review:?}");
        assert_eq!(job(&jobs, marker.request).phase, Phase::Finished);
        close(jobs, root);
        let engine = OperationEngine::open(root.join("history")).unwrap();
        let page = engine
            .recovery_items(None, &Cancellation::default())
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.items[0].entry.as_ref().unwrap().path,
            marker.destination.join("previous")
        );
    });
}
#[test]
fn saved_container_publication_preserves_raw_previous_links() {
    fixture_shape("publishing", "link", |root, marker| {
        let jobs = start(root);
        let review = reconcile(&jobs);
        assert_eq!(review.phase, Phase::Finished, "{review:?}");
        assert_eq!(job(&jobs, marker.request).phase, Phase::Finished);
        assert_eq!(
            fs::read_link(marker.destination.join("previous")).unwrap(),
            PathBuf::from(std::ffi::OsString::from_vec(b"missing-\xff".to_vec()))
        );
        close(jobs, root);
    });
}
#[test]
fn completed_saved_keep_both_and_nested_moves_preserve_the_remaining_tasks() {
    for shape in ["batch", "nested"] {
        fixture_shape("finished", shape, |root, marker| {
            let completed = root.join("completed-saved-container");
            fs::rename(&marker.destination, &completed).unwrap();
            fs::create_dir(&marker.destination).unwrap();
            fs::write(
                marker.destination.join("foreign"),
                b"New destination occupant",
            )
            .unwrap();
            fs::create_dir(&marker.source).unwrap();
            fs::write(marker.source.join("foreign"), b"New source occupant").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let row = job(&jobs, marker.request);
                assert_eq!(row.phase, Phase::Stopped, "{shape}: {row:?}");
                assert!(row.transfer_actions.remaining);
                assert_eq!(
                    row.transfer.as_ref().unwrap().completed,
                    if shape == "batch" { 1 } else { 0 }
                );
                assert_eq!(
                    row.transfer.as_ref().unwrap().destinations,
                    if shape == "batch" {
                        vec![marker.destination.clone()]
                    } else {
                        vec![]
                    }
                );
                close(jobs, root);
            }
            let jobs = start(root);
            let id = receive(
                jobs.resume_transfer(marker.request, TransferPart::Remaining)
                    .unwrap(),
            )
            .unwrap();
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{shape}: {done:?}");
            assert_eq!(
                fs::read(completed.join("previous")).unwrap(),
                b"Original saved data"
            );
            assert_eq!(
                fs::read(marker.destination.join("foreign")).unwrap(),
                b"New destination occupant"
            );
            assert_eq!(
                fs::read(marker.source.join("foreign")).unwrap(),
                b"New source occupant"
            );
            if shape == "batch" {
                assert_eq!(
                    fs::read(root.join("outgoing/z-tail")).unwrap(),
                    b"Later selected root"
                );
            } else {
                assert_eq!(
                    fs::read(root.join("outgoing/incoming/item")).unwrap(),
                    b"Selected replacement"
                );
            }
            close(jobs, root);
        });
    }
}
#[test]
fn saved_container_claims_restore_their_original_task_before_publication() {
    for point in ["intent", "ready", "returning", "returned"] {
        fixture(point, |root, marker| {
            let jobs = start(root);
            let review = reconcile(&jobs);
            assert_eq!(review.phase, Phase::Finished, "{point}: {review:?}");
            let row = job(&jobs, marker.request);
            assert_eq!(row.phase, Phase::Stopped, "{point}: {row:?}");
            assert_eq!(row.transfer.as_ref().unwrap().completed, 0);
            assert!(row.transfer_actions.remaining && !row.transfer_actions.retry);
            assert!(!marker.destination.exists());
            assert_eq!(
                fs::read(marker.source.join("previous")).unwrap(),
                b"Original saved data"
            );
            let inode = fs::metadata(&marker.source).unwrap().ino();
            close(jobs, root);
            let jobs = start(root);
            let id = receive(
                jobs.resume_transfer(marker.request, TransferPart::Remaining)
                    .unwrap(),
            )
            .unwrap();
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{point}: {done:?}");
            assert_eq!(done.transfer.as_ref().unwrap().completed, 1);
            assert_eq!(fs::metadata(&marker.destination).unwrap().ino(), inode);
            assert!(!marker.source.exists());
            close(jobs, root);
            let engine = OperationEngine::open(root.join("history")).unwrap();
            let page = engine
                .recovery_items(None, &Cancellation::default())
                .unwrap();
            assert_eq!(page.items.len(), 1);
            assert_eq!(
                page.items[0].entry.as_ref().unwrap().path,
                marker.destination.join("previous")
            );
            engine.undo(&Cancellation::default()).unwrap();
            assert_eq!(
                fs::read(marker.source.join("previous")).unwrap(),
                b"Original saved data"
            );
        });
    }
}
#[test]
fn saved_container_publication_finishes_without_using_a_new_source_occupant() {
    for point in ["publishing", "published"] {
        fixture(point, |root, marker| {
            fs::create_dir(&marker.source).unwrap();
            fs::write(marker.source.join("foreign"), b"New source occupant").unwrap();
            let inode = fs::metadata(&marker.payload).unwrap().ino();
            let jobs = start(root);
            let review = reconcile(&jobs);
            assert_eq!(review.phase, Phase::Finished, "{point}: {review:?}");
            let row = job(&jobs, marker.request);
            assert_eq!(row.phase, Phase::Finished, "{point}: {row:?}");
            assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
            assert_eq!(row.transfer.as_ref().unwrap().bytes, 0);
            assert_eq!(fs::metadata(&marker.destination).unwrap().ino(), inode);
            assert_eq!(
                fs::read(marker.destination.join("previous")).unwrap(),
                b"Original saved data"
            );
            assert_eq!(
                fs::read(marker.source.join("foreign")).unwrap(),
                b"New source occupant"
            );
            close(jobs, root);
            let jobs = start(root);
            assert_eq!(job(&jobs, marker.request).phase, Phase::Finished);
            close(jobs, root);
        });
    }
}
#[test]
fn saved_container_restart_refuses_a_replaced_previous_entry_or_parent() {
    for part in ["previous", "source-parent", "destination-parent", "work"] {
        fixture("publishing", |root, marker| {
            let (selected, held) = match part {
                "previous" => (marker.payload.join("previous"), root.join("held-previous")),
                "source-parent" => (root.join("incoming"), root.join("held-incoming")),
                "destination-parent" => (root.join("outgoing"), root.join("held-outgoing")),
                "work" => (marker.work.clone(), root.join("held-work")),
                _ => unreachable!(),
            };
            fs::rename(&selected, &held).unwrap();
            if part == "previous" {
                fs::write(&selected, b"Replacement previous entry").unwrap();
            } else {
                fs::create_dir(&selected).unwrap();
                fs::write(selected.join("foreign"), b"Replacement folder occupant").unwrap();
            }
            for _ in 0..2 {
                let jobs = start(root);
                let review = reconcile(&jobs);
                assert_ne!(review.phase, Phase::Finished, "{part}: {review:?}");
                assert_eq!(job(&jobs, marker.request).phase, Phase::Interrupted);
                if part == "previous" {
                    assert_eq!(fs::read(&selected).unwrap(), b"Replacement previous entry");
                } else {
                    assert_eq!(
                        fs::read(selected.join("foreign")).unwrap(),
                        b"Replacement folder occupant"
                    );
                }
                close(jobs, root);
            }
            if part == "previous" {
                fs::remove_file(&selected).unwrap();
            } else {
                fs::remove_file(selected.join("foreign")).unwrap();
                fs::remove_dir(&selected).unwrap();
            }
            fs::rename(&held, &selected).unwrap();
            let jobs = start(root);
            let review = reconcile(&jobs);
            assert_eq!(review.phase, Phase::Finished, "{part}: {review:?}");
            let row = job(&jobs, marker.request);
            if part == "previous" {
                // Replacing and returning the child changes its container's
                // version. Return that original container before a fresh part.
                assert_eq!(row.phase, Phase::Stopped, "{row:?}");
                assert!(row.transfer_actions.remaining);
                let id = receive(
                    jobs.resume_transfer(marker.request, TransferPart::Remaining)
                        .unwrap(),
                )
                .unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{done:?}");
            } else {
                assert_eq!(row.phase, Phase::Finished, "{row:?}");
            }
            assert_eq!(
                fs::read(marker.destination.join("previous")).unwrap(),
                b"Original saved data"
            );
            close(jobs, root);
        });
    }
}
#[test]
fn saved_container_restart_does_not_recreate_a_missing_or_changed_step_receipt() {
    for corrupt in [false, true] {
        fixture("publishing", |root, marker| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            if corrupt {
                db.execute(
                    "UPDATE request_transfer_cursor_steps SET body_sha256=zeroblob(32)",
                    [],
                )
                .unwrap();
            } else {
                db.execute("DELETE FROM request_transfer_cursor_steps", [])
                    .unwrap();
            }
            drop(db);
            for _ in 0..2 {
                let jobs = start(root);
                let review = reconcile(&jobs);
                assert_ne!(review.phase, Phase::Finished, "{corrupt}: {review:?}");
                assert_eq!(job(&jobs, marker.request).phase, Phase::Interrupted);
                assert_eq!(
                    fs::read(marker.payload.join("previous")).unwrap(),
                    b"Original saved data"
                );
                assert!(!marker.destination.exists());
                close(jobs, root);
            }
        });
    }
}
#[test]
#[ignore = "private public saved-container worker killed after its actual movement"]
fn saved_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_SAVED_TASK_ROOT").unwrap());
    fs::create_dir(root.join("seed")).unwrap();
    fs::create_dir(root.join("incoming")).unwrap();
    fs::create_dir(root.join("outgoing")).unwrap();
    let seed = root.join("seed/item");
    fs::write(&seed, b"Selected replacement").unwrap();
    let shape = std::env::var("CANOPA_SAVED_TASK_SHAPE").unwrap();
    if shape == "link" {
        std::os::unix::fs::symlink(
            std::ffi::OsString::from_vec(b"missing-\xff".to_vec()),
            root.join("incoming/item"),
        )
        .unwrap();
    } else {
        fs::write(root.join("incoming/item"), b"Original saved data").unwrap();
    }
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed],
                destination: root.join("incoming"),
            },
            &Cancellation::default(),
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert_eq!(result.completed, 1);
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert_eq!(page.items.len(), 1);
    let source = page.items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .parent()
        .unwrap()
        .to_owned();
    drop(engine);
    fs::write(
        root.join("selected.json"),
        serde_json::to_vec(&source).unwrap(),
    )
    .unwrap();
    let mut sources = vec![source.clone()];
    if shape == "batch" {
        fs::create_dir(root.join("outgoing").join(source.file_name().unwrap())).unwrap();
        fs::write(
            root.join("outgoing")
                .join(source.file_name().unwrap())
                .join("foreign"),
            b"Existing conflict",
        )
        .unwrap();
        let tail = root.join("seed/z-tail");
        fs::write(&tail, b"Later selected root").unwrap();
        sources.push(tail);
    } else if shape == "nested" {
        fs::create_dir(root.join("outgoing/incoming")).unwrap();
        sources = vec![root.join("incoming")];
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
    if shape == "batch" || shape == "nested" {
        let question = wait(|| job(&jobs, id).decision);
        jobs.resolve_transfer(
            id,
            question.generation,
            TransferReply::Resolve {
                choice: if shape == "batch" {
                    ConflictChoice::KeepBoth
                } else {
                    ConflictChoice::Merge
                },
                apply_to_remaining: false,
            },
        )
        .unwrap();
    }
    std::thread::sleep(Duration::from_secs(25));
    panic!("private saved boundary was not reached");
}
