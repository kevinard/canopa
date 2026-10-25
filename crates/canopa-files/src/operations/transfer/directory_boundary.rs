//! Public worker crashes between directory publication and traversal expansion.
use super::*;
use crate::operation_jobs::{OperationJobs, Phase, Request, TransferPart};
use std::{
    ffi::OsString,
    os::unix::{ffi::OsStringExt, fs::MetadataExt, process::ExitStatusExt},
    time::{Duration, Instant},
};

fn folder() -> OsString {
    OsString::from_vec(b"a-folder-\xfd".to_vec())
}
fn source_path(root: &Path) -> PathBuf {
    root.join("source-parent").join(folder())
}
fn destination_path(output: &Path) -> PathBuf {
    output.join("destination-parent")
}
fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            Instant::now() < end,
            "private directory worker did not settle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn receive<T>(mut reply: futures_channel::oneshot::Receiver<T>) -> T {
    wait(|| {
        reply
            .try_recv()
            .expect("private directory worker dropped reply")
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
pub(in crate::operations) fn boundary(stage: &str, source: &Path, destination: &Path) {
    let Some(root) = std::env::var_os("CANOPA_DIRECTORY_EXPANSION_ROOT").map(PathBuf::from) else {
        return;
    };
    if source != source_path(&root)
        || std::env::var("CANOPA_DIRECTORY_EXPANSION_STAGE").as_deref() != Ok(stage)
    {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-directory-expansion-")
    );
    let output = PathBuf::from(std::env::var_os("CANOPA_DIRECTORY_EXPANSION_OUTPUT").unwrap());
    assert!(destination.starts_with(output));
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let id: u64 = db
        .query_row("SELECT request FROM request_transfer_cursors", [], |r| {
            r.get(0)
        })
        .unwrap();
    fs::write(root.join("marker.json"), serde_json::to_vec(&id).unwrap()).unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private directory worker was not killed");
}
pub(in crate::operations) fn after_promotion(destination: &Path) {
    let Some(root) = std::env::var_os("CANOPA_DIRECTORY_EXPANSION_ROOT").map(PathBuf::from) else {
        return;
    };
    boundary("promoted", &source_path(&root), destination);
}
pub(in crate::operations) fn removal_boundary(point: &str, source: &Path) {
    let Some(output) = std::env::var_os("CANOPA_DIRECTORY_EXPANSION_OUTPUT").map(PathBuf::from)
    else {
        return;
    };
    boundary(
        &format!("cleanup-{point}"),
        source,
        &destination_path(&output).join(folder()),
    );
}

pub(in crate::operations) fn cancel_cleanup_return(source: &Path, cancellation: &Cancellation) {
    let Some(root) = std::env::var_os("CANOPA_DIRECTORY_EXPANSION_ROOT").map(PathBuf::from) else {
        return;
    };
    if source == source_path(&root)
        && matches!(
            std::env::var("CANOPA_DIRECTORY_EXPANSION_STAGE").as_deref(),
            Ok("cleanup-returning" | "cleanup-returned")
        )
    {
        cancellation.cancel();
    }
}

fn fixture(kind: TransferKind, stage: &str, variant: &str, run: impl FnOnce(&Path, &Path, u64)) {
    let root = tempfile::Builder::new()
        .prefix("canopa-directory-expansion-")
        .tempdir()
        .unwrap();
    let output = tempfile::Builder::new()
        .prefix(".canopa-directory-expansion-output-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    assert_ne!(
        fs::metadata(root.path()).unwrap().dev(),
        fs::metadata(output.path()).unwrap().dev(),
        "private cross-filesystem fixture needs distinct devices"
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::directory_boundary::directory_child",
            ])
            .env("CANOPA_DIRECTORY_EXPANSION_ROOT", root.path())
            .env("CANOPA_DIRECTORY_EXPANSION_OUTPUT", output.path())
            .env("CANOPA_DIRECTORY_EXPANSION_STAGE", stage)
            .env("CANOPA_DIRECTORY_EXPANSION_VARIANT", variant)
            .env(
                "CANOPA_DIRECTORY_EXPANSION_KIND",
                if kind == TransferKind::Copy {
                    "Copy"
                } else {
                    "Move"
                },
            )
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let id =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        run(root.path(), output.path(), id);
    }));
    if let Err(error) = result {
        eprintln!(
            "retained directory expansion failure roots: {} and {}",
            root.keep().display(),
            output.keep().display()
        );
        std::panic::resume_unwind(error);
    }
}
#[test]
fn a_published_directory_restores_its_exact_original_child_queue() {
    for kind in [TransferKind::Copy, TransferKind::Move] {
        fixture(kind, "appended", "create", |root, output, id| {
            let target = destination_path(output).join(folder());
            let inode = fs::metadata(&target).unwrap().ino();
            fs::write(
                source_path(root).join("late-unselected"),
                b"Late source data",
            )
            .unwrap();
            fs::write(target.join("foreign"), b"New destination data").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Stopped, "{kind:?}: {row:?}");
                assert!(row.transfer_actions.remaining, "{row:?}");
                assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
                assert_eq!(fs::metadata(&target).unwrap().ino(), inode);
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
            assert_eq!(
                fs::read(target.join("selected")).unwrap(),
                b"Original selected child"
            );
            assert_eq!(
                fs::read_link(target.join(OsString::from_vec(b"link-\xfe".to_vec()))).unwrap(),
                PathBuf::from(OsString::from_vec(b"missing-\xff".to_vec()))
            );
            assert!(!target.join("late-unselected").exists());
            assert_eq!(
                fs::read(source_path(root).join("late-unselected")).unwrap(),
                b"Late source data"
            );
            assert_eq!(
                fs::read(target.join("foreign")).unwrap(),
                b"New destination data"
            );
            assert_eq!(fs::metadata(&target).unwrap().ino(), inode);
            receive(jobs.shutdown().unwrap()).unwrap();
        });
    }
}

#[test]
fn a_completed_source_directory_removal_restores_only_its_result() {
    for stage in ["cleanup-finished", "cleanup-removed", "cleanup-verified"] {
        fixture(TransferKind::Move, stage, "create", |root, output, id| {
            let target = destination_path(output).join(folder());
            let inode = fs::metadata(&target).unwrap().ino();
            assert!(!source_path(root).exists());
            fs::create_dir(source_path(root)).unwrap();
            fs::write(source_path(root).join("foreign"), b"New source occupant").unwrap();
            fs::write(target.join("selected"), b"Edited completed output").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                if stage != "cleanup-finished" {
                    let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                    let done = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == review && !j.phase.holds_application())
                    });
                    assert_eq!(done.phase, Phase::Finished, "{stage}: {done:?}");
                }
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Finished, "{row:?}");
                assert_eq!(row.transfer.as_ref().unwrap().completed, 3);
                assert!(!row.transfer_actions.remaining && !row.transfer_actions.retry);
                assert_eq!(
                    fs::read(source_path(root).join("foreign")).unwrap(),
                    b"New source occupant"
                );
                assert_eq!(
                    fs::read(target.join("selected")).unwrap(),
                    b"Edited completed output"
                );
                assert_eq!(fs::metadata(&target).unwrap().ino(), inode);
                close(jobs, root);
            }
        });
    }
}

#[test]
fn reviewed_directory_choices_and_preappend_publication_keep_their_expansion() {
    for kind in [TransferKind::Copy, TransferKind::Move] {
        for (stage, variant) in [
            ("appended", "keep-both"),
            ("merged", "merge"),
            ("published", "create"),
            ("preparing", "create"),
        ] {
            fixture(kind, stage, variant, |root, output, id| {
                let target = if variant == "keep-both" {
                    let mut name = folder();
                    name.push(" (2)");
                    destination_path(output).join(name)
                } else {
                    destination_path(output).join(folder())
                };
                fs::write(
                    source_path(root).join("late-unselected"),
                    b"Late source data",
                )
                .unwrap();
                let jobs = start(root);
                if matches!(stage, "published" | "preparing") {
                    let before = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert_eq!(before.phase, Phase::Interrupted, "{before:?}");
                    assert!(!before.transfer_actions.remaining);
                    let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                    let done = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == review && !j.phase.holds_application())
                    });
                    assert_eq!(
                        done.phase,
                        Phase::Finished,
                        "{kind:?}/{stage}/{variant}: {done:?}"
                    );
                }
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(
                    row.phase,
                    Phase::Stopped,
                    "{kind:?}/{stage}/{variant}: {row:?}"
                );
                assert!(row.transfer_actions.remaining, "{row:?}");
                assert_eq!(
                    row.transfer.as_ref().unwrap().completed,
                    usize::from(variant != "merge")
                );
                let inode = fs::metadata(&target).unwrap().ino();
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
                assert_eq!(
                    done.phase,
                    Phase::Finished,
                    "{kind:?}/{stage}/{variant}: {done:?}"
                );
                assert_eq!(
                    fs::read(target.join("selected")).unwrap(),
                    b"Original selected child"
                );
                assert!(!target.join("late-unselected").exists());
                assert_eq!(
                    fs::read(source_path(root).join("late-unselected")).unwrap(),
                    b"Late source data"
                );
                assert_eq!(fs::metadata(&target).unwrap().ino(), inode);
                if variant != "create" {
                    assert_eq!(
                        fs::read(destination_path(output).join(folder()).join("existing")).unwrap(),
                        b"Existing destination data"
                    );
                }
                receive(jobs.shutdown().unwrap()).unwrap();
            });
        }
    }
}

#[test]
fn directory_expansions_refuse_replacement_folders_and_parent_generations() {
    for (stage, variant) in [
        ("appended", "create"),
        ("merged", "merge"),
        ("published", "create"),
        ("preparing", "create"),
    ] {
        for fault in ["source", "target", "source-parent", "target-parent"] {
            fixture(TransferKind::Copy, stage, variant, |root, output, id| {
                let source = source_path(root);
                let target = destination_path(output).join(folder());
                let replaced = match fault {
                    "source" => source.clone(),
                    "target" => target.clone(),
                    "source-parent" => source.parent().unwrap().to_owned(),
                    "target-parent" => target.parent().unwrap().to_owned(),
                    _ => unreachable!(),
                };
                if fault == "target" && !target.exists() {
                    fs::create_dir(&target).unwrap();
                }
                let parked = replaced.with_extension("parked-original");
                fs::rename(&replaced, &parked).unwrap();
                fs::create_dir(&replaced).unwrap();
                let foreign = replaced.join("foreign");
                fs::write(&foreign, b"Replacement data must remain").unwrap();
                if fault == "source-parent" {
                    fs::rename(parked.join(folder()), &source).unwrap();
                }
                if fault == "target-parent" && parked.join(folder()).exists() {
                    fs::rename(parked.join(folder()), &target).unwrap();
                }
                let permissions = fs::metadata(&replaced).unwrap().permissions().mode();
                for _ in 0..2 {
                    let jobs = start(root);
                    if matches!(stage, "published" | "preparing") {
                        let review =
                            receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                        let done = wait(|| {
                            jobs.snapshot()
                                .jobs
                                .into_iter()
                                .find(|j| j.id == review && !j.phase.holds_application())
                        });
                        assert_ne!(done.phase, Phase::Finished, "{stage}/{fault}: {done:?}");
                    }
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert!(
                        !row.transfer_actions.remaining && !row.transfer_actions.retry,
                        "{stage}/{fault}: {row:?}"
                    );
                    assert!(
                        receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap())
                            .is_err()
                    );
                    assert_eq!(fs::read(&foreign).unwrap(), b"Replacement data must remain");
                    assert_eq!(
                        fs::metadata(&replaced).unwrap().permissions().mode(),
                        permissions
                    );
                    close(jobs, root);
                }
            });
        }
    }
}
#[test]
fn directory_expansions_refuse_missing_changed_and_borrowed_evidence() {
    for stage in ["appended", "preparing"] {
        for fault in [
            "descriptor",
            "child",
            "position",
            "borrowed",
            "count",
            "step",
        ] {
            if stage == "preparing" && fault == "step" {
                continue;
            }
            fixture(TransferKind::Copy, stage, "create", |root, output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                db.execute_batch(match fault {
                    "descriptor" => "DELETE FROM request_transfer_tasks WHERE role=8",
                    "child" => "DELETE FROM request_transfer_tasks WHERE role=9 AND position=(SELECT min(position) FROM request_transfer_tasks WHERE role=9)",
                    "position" => "UPDATE request_transfer_tasks SET position=x'00000000000000000000000000000000' WHERE role=9 AND position=(SELECT min(position) FROM request_transfer_tasks WHERE role=9)",
                    "borrowed" => "UPDATE request_transfer_tasks SET body=(SELECT body FROM request_transfer_tasks WHERE role=0 LIMIT 1),body_sha256=(SELECT body_sha256 FROM request_transfer_tasks WHERE role=0 LIMIT 1) WHERE role=8",
                    "count" => "UPDATE request_transfer_cursors SET progress=replace(progress,'completed','changed')",
                    "step" => "DELETE FROM request_transfer_cursor_steps",
                    _ => unreachable!(),
                }).unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    if stage == "preparing" {
                        let review =
                            receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                        let done = wait(|| {
                            jobs.snapshot()
                                .jobs
                                .into_iter()
                                .find(|j| j.id == review && !j.phase.holds_application())
                        });
                        assert_ne!(done.phase, Phase::Finished, "{stage}/{fault}: {done:?}");
                    }
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert!(
                        !row.transfer_actions.remaining && !row.transfer_actions.retry,
                        "{stage}/{fault}: {row:?}"
                    );
                    assert_eq!(
                        fs::read(source_path(root).join("selected")).unwrap(),
                        b"Original selected child"
                    );
                    assert!(
                        !destination_path(output)
                            .join(folder())
                            .join("selected")
                            .exists()
                    );
                    close(jobs, root);
                }
            });
        }
    }
}
#[test]
fn empty_directory_expansions_finish_without_recreating_or_double_counting() {
    for kind in [TransferKind::Copy, TransferKind::Move] {
        for (stage, variant) in [("appended", "empty-create"), ("merged", "empty-merge")] {
            fixture(kind, stage, variant, |root, output, id| {
                let target = destination_path(output).join(folder());
                let inode = fs::metadata(&target).unwrap().ino();
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                if kind == TransferKind::Copy {
                    assert_eq!(row.phase, Phase::Finished, "{row:?}");
                    assert_eq!(
                        row.transfer.as_ref().unwrap().completed,
                        usize::from(variant == "empty-create")
                    );
                    assert!(source_path(root).is_dir());
                } else {
                    assert!(row.transfer_actions.remaining, "{row:?}");
                    let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap())
                        .unwrap();
                    let done = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == next && !j.phase.holds_application())
                    });
                    assert_eq!(done.phase, Phase::Finished, "{done:?}");
                    assert_eq!(
                        row.transfer.as_ref().unwrap().completed
                            + done.transfer.as_ref().unwrap().completed,
                        1,
                        "{variant}: {row:?} / {done:?}"
                    );
                    assert!(!source_path(root).exists());
                }
                assert_eq!(fs::metadata(&target).unwrap().ino(), inode);
                if variant == "empty-merge" {
                    assert_eq!(
                        fs::read(target.join("existing")).unwrap(),
                        b"Existing destination data"
                    );
                }
                receive(jobs.shutdown().unwrap()).unwrap();
            });
        }
    }
}

#[test]
fn directory_preparing_promotion_rolls_back_its_forward_and_step_together() {
    for fault in ["forward", "step"] {
        fixture(
            TransferKind::Copy,
            "preparing",
            "create",
            |root, output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                db.execute_batch(if fault == "forward" {
                r#"CREATE TRIGGER refuse_directory_promotion BEFORE INSERT ON pending WHEN instr(NEW.body,'"direction":"forward"')>0 BEGIN SELECT RAISE(ABORT,'private directory promotion refusal'); END"#
            } else {
                "CREATE TRIGGER refuse_directory_promotion BEFORE INSERT ON request_transfer_cursor_steps BEGIN SELECT RAISE(ABORT,'private directory step refusal'); END"
            }).unwrap();
                let before: String = db
                    .query_row("SELECT body FROM pending", [], |r| r.get(0))
                    .unwrap();
                let jobs = start(root);
                for _ in 0..2 {
                    let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                    let done = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == review && !j.phase.holds_application())
                    });
                    assert_ne!(done.phase, Phase::Finished, "{fault}: {done:?}");
                    assert_eq!(
                        db.query_row("SELECT body FROM pending", [], |r| r.get::<_, String>(0))
                            .unwrap(),
                        before
                    );
                    assert_eq!(
                        db.query_row("SELECT count(*) FROM pending_bindings", [], |r| r
                            .get::<_, u64>(0))
                            .unwrap(),
                        0
                    );
                    assert_eq!(
                        db.query_row(
                            "SELECT count(*) FROM request_transfer_cursor_steps",
                            [],
                            |r| r.get::<_, u64>(0)
                        )
                        .unwrap(),
                        0
                    );
                    assert!(!destination_path(output).join(folder()).exists());
                    assert_eq!(
                        fs::read(source_path(root).join("selected")).unwrap(),
                        b"Original selected child"
                    );
                }
                db.execute_batch("DROP TRIGGER refuse_directory_promotion")
                    .unwrap();
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{done:?}");
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert!(row.transfer_actions.remaining, "{row:?}");
                receive(jobs.abandon_transfer(id).unwrap()).unwrap();
                receive(jobs.shutdown().unwrap()).unwrap();
            },
        );
    }
}
#[test]
fn a_second_crash_after_preparing_promotion_keeps_the_same_payload_and_children() {
    fixture(
        TransferKind::Copy,
        "preparing",
        "create",
        |root, output, id| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            let body: String = db
                .query_row("SELECT body FROM pending", [], |r| r.get(0))
                .unwrap();
            let Pending::Preparing { stage, .. } = serde_json::from_str(&body).unwrap() else {
                panic!("expected original Preparing")
            };
            let inode = fs::metadata(stage.payload()).unwrap().ino();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::transfer::directory_boundary::review_directory_child",
                ])
                .env("CANOPA_DIRECTORY_EXPANSION_ROOT", root)
                .env("CANOPA_DIRECTORY_EXPANSION_OUTPUT", output)
                .env("CANOPA_DIRECTORY_EXPANSION_STAGE", "promoted")
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            assert_eq!(
                db.query_row(
                    "SELECT json_extract(body,'$.direction') FROM pending",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
                "forward"
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
            assert_eq!(fs::metadata(stage.payload()).unwrap().ino(), inode);
            assert!(!destination_path(output).join(folder()).exists());
            fs::write(
                source_path(root).join("late-unselected"),
                b"Later source data",
            )
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
            let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == next && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            let target = destination_path(output).join(folder());
            assert_eq!(fs::metadata(&target).unwrap().ino(), inode);
            assert!(!target.join("late-unselected").exists());
            assert_eq!(
                fs::read(source_path(root).join("late-unselected")).unwrap(),
                b"Later source data"
            );
            receive(jobs.shutdown().unwrap()).unwrap();
        },
    );
}
#[test]
fn a_directory_expansion_keeps_large_child_lists_in_bounded_normalized_rows() {
    fixture(
        TransferKind::Copy,
        "appended",
        "large",
        |root, output, id| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            let count: u64 = db
                .query_row(
                    "SELECT count(*) FROM request_transfer_tasks WHERE role=9",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 770);
            assert!(
                db.query_row(
                    "SELECT length(CAST(body AS BLOB)) FROM request_transfer_tasks WHERE role=8",
                    [],
                    |r| r.get::<_, u64>(0)
                )
                .unwrap()
                    < 16384
            );
            assert!(db.query_row("SELECT max(length(CAST(body AS BLOB))) FROM request_transfer_tasks WHERE role=9", [], |r| r.get::<_,u64>(0)).unwrap() < 1024);
            let jobs = start(root);
            let row = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id)
                .unwrap();
            assert!(row.transfer_actions.remaining, "{row:?}");
            assert_eq!(row.transfer.as_ref().unwrap().remaining.len(), 770);
            receive(jobs.abandon_transfer(id).unwrap()).unwrap();
            assert_eq!(
                fs::read(source_path(root).join("selected")).unwrap(),
                b"Original selected child"
            );
            assert!(
                !destination_path(output)
                    .join(folder())
                    .join("selected")
                    .exists()
            );
            receive(jobs.shutdown().unwrap()).unwrap();
        },
    );
}
#[test]
fn original_readonly_folder_modes_survive_preparation_publication_and_continuation() {
    for stage in ["preparing", "published", "appended"] {
        fixture(TransferKind::Copy, stage, "readonly", |root, output, id| {
            let jobs = start(root);
            if stage != "appended" {
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{done:?}");
            }
            let target = destination_path(output).join(folder());
            assert_eq!(
                fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o555
            );
            let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == next && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            assert_eq!(
                fs::read(target.join("selected")).unwrap(),
                b"Original selected child"
            );
            assert_eq!(
                fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o555
            );
            assert_eq!(
                fs::metadata(source_path(root))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o555
            );
            receive(jobs.shutdown().unwrap()).unwrap();
            fs::set_permissions(source_path(root), fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
        });
    }
}

#[test]
fn returned_source_directory_claims_keep_their_exact_postorder_cleanup() {
    for stage in ["cleanup-claimed", "cleanup-returning", "cleanup-returned"] {
        fixture(TransferKind::Move, stage, "create", |root, output, id| {
            let target = destination_path(output).join(folder());
            let inode = fs::metadata(&target).unwrap().ino();
            fs::write(target.join("selected"), b"Edited completed output").unwrap();
            let jobs = start(root);
            let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == review && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{stage}: {done:?}");
            assert!(source_path(root).is_dir());
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
            close(jobs, root);
            let jobs = start(root);
            let row = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id)
                .unwrap();
            assert!(row.transfer_actions.remaining, "{stage}: {row:?}");
            fs::write(
                source_path(root).join("late"),
                b"New unselected source data",
            )
            .unwrap();
            let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == next && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            assert_eq!(
                done.transfer.as_ref().unwrap().source_remainders,
                vec![source_path(root)]
            );
            assert_eq!(
                fs::read(source_path(root).join("late")).unwrap(),
                b"New unselected source data"
            );
            assert_eq!(
                fs::read(target.join("selected")).unwrap(),
                b"Edited completed output"
            );
            assert_eq!(fs::metadata(&target).unwrap().ino(), inode);
            receive(jobs.shutdown().unwrap()).unwrap();
        });
    }
}
#[test]
fn a_claimed_directory_with_an_absent_payload_does_not_adopt_a_new_source_name() {
    fixture(
        TransferKind::Move,
        "cleanup-verified",
        "create",
        |root, output, id| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            let body: String = db
                .query_row("SELECT body FROM pending", [], |r| r.get(0))
                .unwrap();
            let Pending::Forward {
                stage: Some(stage), ..
            } = serde_json::from_str(&body).unwrap()
            else {
                panic!("expected directory claim")
            };
            fs::rename(stage.payload(), source_path(root)).unwrap();
            fs::write(
                source_path(root).join("foreign"),
                b"New namespace entry after claim",
            )
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
            assert_eq!(
                fs::read(source_path(root).join("foreign")).unwrap(),
                b"New namespace entry after claim"
            );
            assert_eq!(
                fs::read(destination_path(output).join(folder()).join("selected")).unwrap(),
                b"Original selected child"
            );
            receive(jobs.shutdown().unwrap()).unwrap();
        },
    );
}
#[test]
fn a_removed_empty_merge_directory_counts_once_without_revisiting_its_namespace() {
    for stage in ["cleanup-finished", "cleanup-removed"] {
        fixture(
            TransferKind::Move,
            stage,
            "empty-merge",
            |root, output, id| {
                fs::create_dir(source_path(root)).unwrap();
                fs::write(
                    source_path(root).join("foreign"),
                    b"New empty-merge source occupant",
                )
                .unwrap();
                let jobs = start(root);
                if stage == "cleanup-removed" {
                    let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                    let done = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == review && !j.phase.holds_application())
                    });
                    assert_eq!(done.phase, Phase::Finished, "{done:?}");
                }
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Finished, "{row:?}");
                assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
                assert_eq!(
                    fs::read(source_path(root).join("foreign")).unwrap(),
                    b"New empty-merge source occupant"
                );
                assert_eq!(
                    fs::read(destination_path(output).join(folder()).join("existing")).unwrap(),
                    b"Existing destination data"
                );
                receive(jobs.shutdown().unwrap()).unwrap();
            },
        );
    }
}

#[test]
fn a_finished_directory_leaves_only_later_batch_work_to_continue() {
    fixture(
        TransferKind::Move,
        "cleanup-finished",
        "batch",
        |root, output, id| {
            let target = destination_path(output).join(folder());
            fs::create_dir(source_path(root)).unwrap();
            fs::write(source_path(root).join("foreign"), b"New source folder").unwrap();
            fs::rename(&target, target.with_extension("completed")).unwrap();
            fs::create_dir(&target).unwrap();
            fs::write(target.join("foreign"), b"New destination folder").unwrap();
            let jobs = start(root);
            let row = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id)
                .unwrap();
            assert_eq!(row.phase, Phase::Stopped, "{row:?}");
            assert_eq!(row.transfer.as_ref().unwrap().completed, 3);
            assert!(row.transfer_actions.remaining, "{row:?}");
            close(jobs, root);
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
                fs::read(destination_path(output).join("z-tail")).unwrap(),
                b"Later selected root"
            );
            assert!(!root.join("source-parent/z-tail").exists());
            assert_eq!(
                fs::read(source_path(root).join("foreign")).unwrap(),
                b"New source folder"
            );
            assert_eq!(
                fs::read(target.join("foreign")).unwrap(),
                b"New destination folder"
            );
            assert_eq!(
                fs::read(target.with_extension("completed").join("selected")).unwrap(),
                b"Original selected child"
            );
            receive(jobs.shutdown().unwrap()).unwrap();
        },
    );
}

#[test]
fn completed_directory_removals_refuse_missing_or_changed_task_proofs() {
    for fault in ["receipt", "body", "position", "task", "head"] {
        fixture(
            TransferKind::Move,
            "cleanup-finished",
            "create",
            |root, output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                db.execute_batch(match fault {
                "receipt" => "DELETE FROM request_transfer_cursor_steps",
                "body" => "UPDATE request_transfer_cursor_steps SET body=(SELECT body FROM changes WHERE ordinal=0)",
                "position" => "UPDATE request_transfer_cursor_steps SET position=x'ffffffffffffffff'",
                "task" => "UPDATE request_transfer_cursor_steps SET task_sha256=zeroblob(32)",
                "head" => "DELETE FROM request_transfer_tasks WHERE role=0",
                _ => unreachable!(),
            }).unwrap();
                fs::create_dir(source_path(root)).unwrap();
                fs::write(source_path(root).join("foreign"), b"New source folder").unwrap();
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
                        receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap())
                            .is_err()
                    );
                    assert_eq!(
                        fs::read(source_path(root).join("foreign")).unwrap(),
                        b"New source folder"
                    );
                    assert_eq!(
                        fs::read(destination_path(output).join(folder()).join("selected")).unwrap(),
                        b"Original selected child"
                    );
                    close(jobs, root);
                }
            },
        );
    }
}

#[test]
fn returned_directory_task_disposition_is_atomic_with_its_empty_intent() {
    for table in ["pending", "request_transfer_cursor_steps"] {
        fixture(
            TransferKind::Move,
            "cleanup-returned",
            "create",
            |root, _output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                let before: String = db
                    .query_row("SELECT body FROM pending", [], |r| r.get(0))
                    .unwrap();
                db.execute_batch(if table == "pending" {
                r#"CREATE TRIGGER refuse_directory_return BEFORE INSERT ON pending WHEN instr(NEW.body,'"direction":"preparing"')>0 BEGIN SELECT RAISE(ABORT,'private directory return refusal'); END"#
            } else {
                "CREATE TRIGGER refuse_directory_return BEFORE DELETE ON request_transfer_cursor_steps BEGIN SELECT RAISE(ABORT,'private directory receipt refusal'); END"
            }).unwrap();
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
                    assert!(source_path(root).is_dir());
                    close(jobs, root);
                }
                db.execute_batch("DROP TRIGGER refuse_directory_return")
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
                    db.query_row(
                        "SELECT count(*) FROM request_transfer_cursor_steps",
                        [],
                        |r| r.get::<_, u64>(0)
                    )
                    .unwrap(),
                    0
                );
                assert!(
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap()
                        .transfer_actions
                        .remaining
                );
                receive(jobs.abandon_transfer(id).unwrap()).unwrap();
                receive(jobs.shutdown().unwrap()).unwrap();
            },
        );
    }
}

#[test]
fn a_second_crash_after_directory_return_has_only_empty_stage_cleanup_rights() {
    fixture(
        TransferKind::Move,
        "cleanup-returned",
        "create",
        |root, output, id| {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::transfer::directory_boundary::review_directory_child",
                ])
                .env("CANOPA_DIRECTORY_EXPANSION_ROOT", root)
                .env("CANOPA_DIRECTORY_EXPANSION_OUTPUT", output)
                .env(
                    "CANOPA_DIRECTORY_EXPANSION_STAGE",
                    "cleanup-return-preparing",
                )
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            let body: String = db
                .query_row("SELECT body FROM pending", [], |r| r.get(0))
                .unwrap();
            let Pending::Preparing { stage, .. } = serde_json::from_str(&body).unwrap() else {
                panic!("expected returned empty intent")
            };
            assert!(
                stage.directory_claim.is_none()
                    && stage.payload_identity.is_none()
                    && !stage.sibling_payload
            );
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM request_transfer_cursor_steps",
                    [],
                    |r| r.get::<_, u64>(0)
                )
                .unwrap(),
                0
            );
            fs::write(source_path(root).join("foreign"), b"New source content").unwrap();
            fs::write(stage.payload(), b"Foreign stage content").unwrap();
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
                assert_eq!(fs::read(stage.payload()).unwrap(), b"Foreign stage content");
                assert_eq!(
                    fs::read(source_path(root).join("foreign")).unwrap(),
                    b"New source content"
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
            assert!(
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap()
                    .transfer_actions
                    .remaining
            );
            receive(jobs.abandon_transfer(id).unwrap()).unwrap();
            receive(jobs.shutdown().unwrap()).unwrap();
        },
    );
}

#[test]
fn a_continued_directory_removal_recovers_its_new_operation_range() {
    fixture(
        TransferKind::Move,
        "appended",
        "create",
        |root, output, original_id| {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::transfer::directory_boundary::review_directory_child",
                ])
                .env("CANOPA_DIRECTORY_EXPANSION_ROOT", root)
                .env("CANOPA_DIRECTORY_EXPANSION_OUTPUT", output)
                .env("CANOPA_DIRECTORY_EXPANSION_STAGE", "cleanup-finished")
                .env("CANOPA_DIRECTORY_EXPANSION_RESUME", "yes")
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let id: u64 =
                serde_json::from_slice(&fs::read(root.join("marker.json")).unwrap()).unwrap();
            assert_ne!(id, original_id);
            fs::create_dir(source_path(root)).unwrap();
            fs::write(
                source_path(root).join("foreign"),
                b"New source after continued removal",
            )
            .unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Finished, "{row:?}");
                assert_eq!(row.transfer.as_ref().unwrap().completed, 2);
                assert!(!row.transfer_actions.remaining && !row.transfer_actions.retry);
                assert_eq!(
                    fs::read(source_path(root).join("foreign")).unwrap(),
                    b"New source after continued removal"
                );
                assert_eq!(
                    fs::read(destination_path(output).join(folder()).join("selected")).unwrap(),
                    b"Original selected child"
                );
                close(jobs, root);
            }
        },
    );
}

#[test]
#[ignore = "private review worker killed after exact Preparing promotion"]
fn review_directory_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_DIRECTORY_EXPANSION_ROOT").unwrap());
    let jobs = start(&root);
    if std::env::var("CANOPA_DIRECTORY_EXPANSION_RESUME").as_deref() == Ok("yes") {
        let row = jobs
            .snapshot()
            .jobs
            .into_iter()
            .find(|j| j.transfer_actions.remaining)
            .unwrap();
        receive(
            jobs.resume_transfer(row.id, TransferPart::Remaining)
                .unwrap(),
        )
        .unwrap();
    } else {
        receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    }
    std::thread::sleep(Duration::from_secs(25));
    panic!("private directory promotion boundary was not reached");
}

#[test]
#[ignore = "private worker killed after actual directory publication"]
fn directory_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_DIRECTORY_EXPANSION_ROOT").unwrap());
    let output = PathBuf::from(std::env::var_os("CANOPA_DIRECTORY_EXPANSION_OUTPUT").unwrap());
    assert_eq!(
        output.parent(),
        Some(std::env::current_dir().unwrap().as_path())
    );
    assert!(
        output
            .file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b".canopa-directory-expansion-output-")
    );
    let source = source_path(&root);
    fs::create_dir(source.parent().unwrap()).unwrap();
    fs::create_dir(&source).unwrap();
    fs::create_dir(destination_path(&output)).unwrap();
    let variant = std::env::var("CANOPA_DIRECTORY_EXPANSION_VARIANT").unwrap();
    if !variant.starts_with("empty") {
        fs::write(source.join("selected"), b"Original selected child").unwrap();
        std::os::unix::fs::symlink(
            OsString::from_vec(b"missing-\xff".to_vec()),
            source.join(OsString::from_vec(b"link-\xfe".to_vec())),
        )
        .unwrap();
    }
    if variant == "large" {
        for index in 0..768 {
            fs::write(
                source.join(format!("{index:05}-{}", "x".repeat(230))),
                b"Large queue child",
            )
            .unwrap();
        }
    }
    if variant.ends_with("merge") || variant == "keep-both" {
        fs::create_dir(destination_path(&output).join(folder())).unwrap();
        fs::write(
            destination_path(&output).join(folder()).join("existing"),
            b"Existing destination data",
        )
        .unwrap();
    }
    if variant == "readonly" {
        fs::set_permissions(&source, fs::Permissions::from_mode(0o555)).unwrap();
    }
    let mut sources = vec![source];
    if variant == "batch" {
        let tail = root.join("source-parent/z-tail");
        fs::write(&tail, b"Later selected root").unwrap();
        sources.push(tail);
    }
    let jobs = start(&root);
    let id = receive(
        jobs.submit(
            Request::Transfer(TransferRequest {
                kind: if std::env::var("CANOPA_DIRECTORY_EXPANSION_KIND").as_deref() == Ok("Copy") {
                    TransferKind::Copy
                } else {
                    TransferKind::Move
                },
                sources,
                destination: destination_path(&output),
            }),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    if variant.ends_with("merge") || variant == "keep-both" {
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
                choice: if variant.ends_with("merge") {
                    ConflictChoice::Merge
                } else {
                    ConflictChoice::KeepBoth
                },
                apply_to_remaining: false,
            },
        )
        .unwrap();
    }
    std::thread::sleep(Duration::from_secs(25));
    panic!("private directory boundary was not reached");
}
