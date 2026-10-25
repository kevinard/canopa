//! Actual public-worker deaths during copy preparation and completed cleanup.
use super::*;
mod access_checks;
mod disposal_checks;
use crate::operation_jobs::{OperationJobs, Phase, Request, TransferPart};
use sha2::{Digest, Sha256};
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
            "private staged-copy worker did not settle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
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
#[derive(Serialize, Deserialize)]
struct Marker {
    request: u64,
    pending: Pending,
    #[serde(with = "crate::path_serde")]
    output: PathBuf,
    binding: Option<String>,
    revision: u64,
    accepted: Vec<u8>,
}
fn leaf() -> std::ffi::OsString {
    use std::os::unix::ffi::OsStringExt;
    if std::env::var("CANOPA_STAGE_TASK_SHAPE").as_deref() == Ok("link") {
        std::ffi::OsString::from_vec(b"b-current-\xfd".to_vec())
    } else {
        "b-current".into()
    }
}
pub(in crate::operations) fn preparing(point: &str, source: &Path, stage: &Stage) {
    let Some(root) = std::env::var_os("CANOPA_STAGE_TASK_ROOT").map(PathBuf::from) else {
        return;
    };
    if source != root.join(leaf()) {
        return;
    }
    write_boundary(point, stage);
}
pub(in crate::operations) fn restricted_preparing(
    source: &Path,
    stage: &Stage,
    cancellation: &Cancellation,
) {
    access_checks::preparing(source, stage, cancellation);
}
pub(in crate::operations) fn nested_read_released(path: &Path) {
    access_checks::read_released(path);
}
pub(in crate::operations) fn nested_read_opened(path: &Path) {
    access_checks::read_opened(path);
}
pub(in crate::operations) fn cleanup(pending: &Pending) {
    let Pending::Cleanup {
        destination, stage, ..
    } = pending
    else {
        return;
    };
    if destination.file_name() != Some(leaf().as_os_str()) {
        return;
    }
    write_boundary("cleanup", stage);
}
pub(in crate::operations) fn disposal_point(point: &str, disposal: &staging::disposal::Disposal) {
    undo::abort_boundary::disposal_boundary::point(point, disposal);
    disposal_checks::point(point, disposal);
}
pub(in crate::operations) fn disposed(stage: &Stage, payload: &fs::File) {
    let Some(root) = std::env::var_os("CANOPA_STAGE_DISPOSE_ROOT").map(PathBuf::from) else {
        return;
    };
    if std::env::var_os("CANOPA_STAGE_DISPOSE_POINT").is_some() {
        return;
    }
    let marker: Marker =
        serde_json::from_slice(&fs::read(root.join("marker.json")).unwrap()).unwrap();
    assert_eq!(stage.directory, work(&marker).directory);
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let pending: String = db
        .query_row("SELECT body FROM pending", [], |r| r.get(0))
        .unwrap();
    let binding: String = db
        .query_row("SELECT body FROM stage_intents WHERE lane=0", [], |r| {
            r.get(0)
        })
        .unwrap();
    fs::write(root.join("disposal-journal.json"), serde_json::to_vec(&serde_json::json!({
        "pending": serde_json::from_str::<serde_json::Value>(&pending).unwrap(),
        "binding": binding,
        "owned_payload_proved": stage.ownership.as_ref().unwrap().require_payload(payload).is_ok(),
    })).unwrap()).unwrap();
    fs::write(root.join("disposal-cut"), b"payload unlinked").unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private disposal worker was not killed");
}
fn write_boundary(point: &str, stage: &Stage) {
    let Some(root) = std::env::var_os("CANOPA_STAGE_TASK_ROOT").map(PathBuf::from) else {
        return;
    };
    let wanted = std::env::var("CANOPA_STAGE_TASK_POINT").unwrap();
    if wanted != point && !(wanted == "refuse-payload" && point == "empty") {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-stage-task-")
    );
    let output = std::env::var_os("CANOPA_STAGE_TASK_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("output"));
    assert_eq!(stage.directory.parent(), Some(output.as_path()));
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let body: String = db
        .query_row("SELECT body FROM pending", [], |r| r.get(0))
        .unwrap();
    let pending: Pending = serde_json::from_str(&body).unwrap();
    assert!(
        matches!(&pending, Pending::Preparing {stage: actual,..} | Pending::Cleanup {stage: actual,..} if actual == stage)
    );
    let request = db
        .query_row("SELECT request FROM request_transfer_cursors", [], |r| {
            r.get(0)
        })
        .unwrap();
    let binding = db
        .query_row("SELECT body FROM stage_intents WHERE lane=0", [], |r| {
            r.get(0)
        })
        .ok();
    let (revision, accepted): (u64, Vec<u8>) = db
        .query_row(
            "SELECT revision,body FROM requests WHERE id=?1",
            [request],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    fs::write(
        root.join("marker.json"),
        serde_json::to_vec(&Marker {
            request,
            pending,
            output,
            binding,
            revision,
            accepted: Sha256::digest(&accepted).to_vec(),
        })
        .unwrap(),
    )
    .unwrap();
    if wanted == "refuse-payload" {
        db.execute_batch("CREATE TRIGGER refuse_stage_payload BEFORE UPDATE ON stage_intents BEGIN SELECT RAISE(ABORT,'private payload save refusal'); END;").unwrap();
        return;
    }
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private staged-copy worker was not killed");
}
fn fixture(point: &str, run: impl FnOnce(&Path, Marker)) {
    fixture_shape(point, "file", run);
}
fn fixture_shape(point: &str, shape: &str, run: impl FnOnce(&Path, Marker)) {
    let root = tempfile::Builder::new()
        .prefix("canopa-stage-task-")
        .tempdir()
        .unwrap();
    let external = (shape == "cross").then(|| {
        tempfile::Builder::new()
            .prefix(".canopa-stage-task-output-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap()
    });
    let output = external
        .as_ref()
        .map_or_else(|| root.path().join("output"), |d| d.path().to_owned());
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::stage_boundary::stage_task_child",
            ])
            .env("CANOPA_STAGE_TASK_ROOT", root.path())
            .env("CANOPA_STAGE_TASK_POINT", point)
            .env("CANOPA_STAGE_TASK_SHAPE", shape)
            .env("CANOPA_STAGE_TASK_OUTPUT", &output)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let marker =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        run(root.path(), marker);
    }));
    if let Err(error) = checked {
        if let Some(external) = external {
            eprintln!(
                "retained staged-task output root: {}",
                external.keep().display()
            );
        }
        eprintln!(
            "retained staged-task source root: {}",
            root.keep().display()
        );
        std::panic::resume_unwind(error);
    }
}
#[test]
fn unproved_preparations_preserve_partial_data_before_request_refusal() {
    fixture("written", |root, marker| {
        let Pending::Preparing { stage, .. } = marker.pending else {
            panic!("preparation");
        };
        let bytes = fs::read(stage.payload()).unwrap();
        assert_eq!(bytes, vec![b'x'; 64 * 1024]);
        fs::write(root.join("output/a-done"), b"Edited completed output").unwrap();
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        db.execute(
            "DELETE FROM request_transfer_cursors WHERE request=?1",
            [marker.request],
        )
        .unwrap();
        drop(db);
        for _ in 0..2 {
            let jobs = start(root);
            assert!(
                fs::read(stage.payload()).ok().as_deref() == Some(bytes.as_slice()),
                "an unproved preparation deleted staged data before request refusal"
            );
            assert_eq!(
                fs::read(root.join("output/a-done")).unwrap(),
                b"Edited completed output"
            );
            assert_eq!(
                fs::read(root.join("b-current")).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            assert_ne!(
                jobs.snapshot()
                    .jobs
                    .iter()
                    .find(|j| j.id == marker.request)
                    .unwrap()
                    .phase,
                Phase::Finished
            );
            close(jobs, root);
        }
    });
}
#[test]
#[ignore = "private public worker killed during actual copy preparation"]
fn stage_task_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_STAGE_TASK_ROOT").unwrap());
    let output = std::env::var_os("CANOPA_STAGE_TASK_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("output"));
    fs::create_dir_all(&output).unwrap();
    fs::write(root.join("a-done"), b"Completed prefix").unwrap();
    let shape = std::env::var("CANOPA_STAGE_TASK_SHAPE").unwrap_or_else(|_| "file".into());
    if shape == "link" {
        use std::os::unix::ffi::OsStringExt;
        std::os::unix::fs::symlink(
            std::ffi::OsString::from_vec(b"missing-\xfe".to_vec()),
            root.join(leaf()),
        )
        .unwrap();
    } else {
        fs::write(root.join(leaf()), vec![b'x'; 128 * 1024]).unwrap();
    }
    fs::write(root.join("z-later"), b"Later root").unwrap();
    if shape == "cross" {
        use std::os::unix::fs::MetadataExt;
        assert_ne!(
            fs::metadata(&root).unwrap().dev(),
            fs::metadata(&output).unwrap().dev()
        );
    }
    let jobs = start(&root);
    let mut reply = jobs
        .submit(
            Request::Transfer(TransferRequest {
                kind: if shape == "cross" {
                    TransferKind::Move
                } else {
                    TransferKind::Copy
                },
                sources: vec![root.join("a-done"), root.join(leaf()), root.join("z-later")],
                destination: output,
            }),
            None,
        )
        .unwrap();
    let id = wait(|| reply.try_recv().unwrap()).unwrap();
    if std::env::var("CANOPA_STAGE_TASK_POINT").as_deref() == Ok("refuse-payload") {
        wait(|| {
            jobs.snapshot()
                .jobs
                .iter()
                .any(|j| {
                    j.id == id && (!j.phase.holds_application() || j.phase == Phase::SaveFailed)
                })
                .then_some(())
        });
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    }
    std::thread::sleep(Duration::from_secs(25));
    panic!("private preparation boundary was not reached");
}

fn work(marker: &Marker) -> &Stage {
    match &marker.pending {
        Pending::Preparing { stage, .. } | Pending::Cleanup { stage, .. } => stage,
        _ => panic!("staged work"),
    }
}

#[test]
fn a_second_cleanup_crash_never_consumes_a_reused_name_for_the_original_inode() {
    fixture("written", |root, marker| {
        let stage = work(&marker);
        let held = root.join("retained-partial-inode");
        fs::hard_link(stage.payload(), &held).unwrap();
        let original = Identity::read(&held).unwrap();
        let bytes = fs::read(&held).unwrap();
        fs::write(marker.output.join("a-done"), b"New completed output").unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::stage_boundary::disposal_child",
            ])
            .env("CANOPA_STAGE_DISPOSE_ROOT", root)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        assert!(root.join("disposal-cut").exists());
        assert!(!stage.payload().exists());
        fs::hard_link(&held, stage.payload()).unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            let _ = reconcile(&jobs);
            assert!(
                original.matches(&stage.payload()),
                "cleanup consumed a new name for an already unlinked original inode"
            );
            assert_eq!(fs::read(stage.payload()).unwrap(), bytes);
            assert_eq!(
                fs::read(marker.output.join("a-done")).unwrap(),
                b"New completed output"
            );
            assert_eq!(
                fs::read(root.join("b-current")).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            close(jobs, root);
        }
    });
}

#[test]
#[ignore = "private second worker killed during staging disposal"]
fn disposal_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_STAGE_DISPOSE_ROOT").unwrap());
    let jobs = start(&root);
    let reviewed = reconcile(&jobs);
    if std::env::var("CANOPA_STAGE_DISPOSE_POINT")
        .is_ok_and(|p| p.starts_with("refuse-") || p.starts_with("ignore-"))
    {
        assert_eq!(reviewed.phase, Phase::Failed, "{reviewed:?}");
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    }
    panic!("private disposal boundary was not reached: {reviewed:?}");
}
fn target(marker: &Marker) -> &Path {
    match &marker.pending {
        Pending::Preparing { destination, .. } | Pending::Cleanup { destination, .. } => {
            destination
        }
        _ => panic!("staged work"),
    }
}
fn receive<T>(mut receiver: futures_channel::oneshot::Receiver<T>) -> T {
    wait(|| receiver.try_recv().unwrap())
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
#[test]
fn proved_preparations_resume_only_unfinished_files_and_raw_links_after_two_opens() {
    for (shape, point) in [
        ("file", "empty"),
        ("file", "payload"),
        ("file", "written"),
        ("cross", "written"),
        ("link", "payload"),
    ] {
        fixture_shape(point, shape, |root, marker| {
            let current = root.join(target(&marker).file_name().unwrap());
            fs::write(marker.output.join("a-done"), b"Edited completed output").unwrap();
            fs::write(root.join("a-done"), b"New completed source namespace").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let reviewed = reconcile(&jobs);
                assert_eq!(
                    reviewed.phase,
                    Phase::Finished,
                    "{shape}/{point}: {reviewed:?}"
                );
                assert!(!work(&marker).directory.exists());
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == marker.request)
                    .unwrap();
                assert_eq!(
                    row.transfer.as_ref().unwrap().completed,
                    1,
                    "{shape}/{point}: {row:?}"
                );
                assert!(row.transfer_actions.remaining, "{shape}/{point}: {row:?}");
                assert_eq!(
                    fs::read(marker.output.join("a-done")).unwrap(),
                    b"Edited completed output"
                );
                assert_eq!(
                    fs::read(root.join("a-done")).unwrap(),
                    b"New completed source namespace"
                );
                close(jobs, root);
            }
            let jobs = start(root);
            let id = receive(
                jobs.resume_transfer(marker.request, TransferPart::Remaining)
                    .unwrap(),
            )
            .unwrap();
            let row = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id && !j.phase.holds_application())
            });
            assert_eq!(row.phase, Phase::Finished, "{shape}/{point}: {row:?}");
            if shape == "link" {
                use std::os::unix::ffi::OsStringExt;
                assert_eq!(
                    fs::read_link(target(&marker)).unwrap(),
                    PathBuf::from(std::ffi::OsString::from_vec(b"missing-\xfe".to_vec()))
                );
            } else {
                assert_eq!(fs::read(target(&marker)).unwrap(), vec![b'x'; 128 * 1024]);
            }
            assert_eq!(
                fs::read(marker.output.join("z-later")).unwrap(),
                b"Later root"
            );
            assert_eq!(
                fs::read(marker.output.join("a-done")).unwrap(),
                b"Edited completed output"
            );
            assert_eq!(
                fs::read(root.join("a-done")).unwrap(),
                b"New completed source namespace"
            );
            assert_eq!(fs::symlink_metadata(&current).is_ok(), shape != "cross");
            close(jobs, root);
        });
    }
}
#[test]
fn preparation_bindings_reject_changed_journals_requests_and_heads_before_mutation() {
    for fault in [
        "binding",
        "binding-digest",
        "pending",
        "execution",
        "receipt",
        "head",
        "phase",
    ] {
        fixture("written", |root, marker| {
            let stage = work(&marker);
            let bytes = fs::read(stage.payload()).unwrap();
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            match fault {
                "binding" => {
                    db.execute("DELETE FROM stage_intents WHERE lane=0", [])
                        .unwrap();
                }
                "binding-digest" => {
                    let mut body: serde_json::Value =
                        serde_json::from_str(marker.binding.as_ref().unwrap()).unwrap();
                    body["digest"] = serde_json::to_value([0u8; 32]).unwrap();
                    db.execute(
                        "UPDATE stage_intents SET body=?1 WHERE lane=0",
                        [body.to_string()],
                    )
                    .unwrap();
                }
                "pending" => {
                    let mut body: serde_json::Value =
                        serde_json::to_value(&marker.pending).unwrap();
                    body["destination"] =
                        serde_json::to_value(marker.output.join("different")).unwrap();
                    db.execute("UPDATE pending SET body=?1", [body.to_string()])
                        .unwrap();
                }
                "execution" => {
                    db.execute(
                        "DELETE FROM request_executions WHERE request=?1",
                        [marker.request],
                    )
                    .unwrap();
                }
                "receipt" => {
                    db.execute("UPDATE request_transfer_ends SET intent_sha256=zeroblob(32) WHERE request=?1",[marker.request]).unwrap();
                }
                "head" => {
                    db.execute(
                        "DELETE FROM request_transfer_tasks WHERE request=?1 AND role=0",
                        [marker.request],
                    )
                    .unwrap();
                }
                "phase" => {
                    db.execute("UPDATE requests SET phase=0 WHERE id=?1", [marker.request])
                        .unwrap();
                }
                _ => unreachable!(),
            }
            drop(db);
            for _ in 0..2 {
                let jobs = start(root);
                assert!(
                    fs::read(stage.payload()).ok().as_deref() == Some(bytes.as_slice()),
                    "{fault}: staged data changed"
                );
                assert_eq!(
                    fs::read(root.join("b-current")).unwrap(),
                    vec![b'x'; 128 * 1024]
                );
                assert_ne!(reconcile(&jobs).phase, Phase::Finished, "{fault}");
                close(jobs, root);
            }
        });
    }
}
#[test]
fn completed_cleanup_restores_metadata_without_reopening_changed_copy_names() {
    for replace in [false, true] {
        fixture("cleanup", |root, marker| {
            let stage = work(&marker);
            assert!(stage.directory.is_dir());
            assert!(!stage.payload().exists());
            let held = root.join("held-completed");
            if replace {
                fs::rename(target(&marker), &held).unwrap();
            }
            fs::write(target(&marker), b"New completed destination bytes").unwrap();
            fs::write(root.join("b-current"), b"New completed source bytes").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                assert_eq!(reconcile(&jobs).phase, Phase::Finished);
                assert!(!stage.directory.exists());
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == marker.request)
                    .unwrap();
                assert_eq!(row.transfer.as_ref().unwrap().completed, 2, "{row:?}");
                assert!(row.transfer_actions.remaining);
                assert_eq!(
                    fs::read(target(&marker)).unwrap(),
                    b"New completed destination bytes"
                );
                assert_eq!(
                    fs::read(root.join("b-current")).unwrap(),
                    b"New completed source bytes"
                );
                close(jobs, root);
            }
            let jobs = start(root);
            let id = receive(
                jobs.resume_transfer(marker.request, TransferPart::Remaining)
                    .unwrap(),
            )
            .unwrap();
            let row = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id && !j.phase.holds_application())
            });
            assert_eq!(row.phase, Phase::Finished, "{row:?}");
            assert_eq!(
                fs::read(target(&marker)).unwrap(),
                b"New completed destination bytes"
            );
            assert_eq!(
                fs::read(root.join("b-current")).unwrap(),
                b"New completed source bytes"
            );
            assert_eq!(
                fs::read(marker.output.join("z-later")).unwrap(),
                b"Later root"
            );
            if replace {
                assert_eq!(fs::read(held).unwrap(), vec![b'x'; 128 * 1024]);
            }
            close(jobs, root);
        });
    }
}
#[test]
fn cleanup_refuses_missing_bindings_and_step_receipts_then_uses_the_original_restored_records() {
    for fault in ["binding", "step"] {
        fixture("cleanup", |root, marker| {
            let stage = work(&marker);
            let Pending::Cleanup { ordinal, .. } = &marker.pending else {
                panic!("cleanup")
            };
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            let step:(u64,u64,Vec<u8>,Vec<u8>,String,Vec<u8>)=db.query_row("SELECT request,ordinal,position,task_sha256,body,body_sha256 FROM request_transfer_cursor_steps WHERE request=?1 AND ordinal=?2",rusqlite::params![marker.request,ordinal],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap();
            if fault == "binding" {
                db.execute("DELETE FROM stage_intents WHERE lane=0", [])
                    .unwrap();
            } else {
                db.execute(
                    "DELETE FROM request_transfer_cursor_steps WHERE request=?1 AND ordinal=?2",
                    rusqlite::params![marker.request, ordinal],
                )
                .unwrap();
            }
            drop(db);
            for _ in 0..2 {
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished, "{fault}");
                assert!(stage.directory.is_dir());
                assert_eq!(fs::read(target(&marker)).unwrap(), vec![b'x'; 128 * 1024]);
                close(jobs, root);
            }
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            if fault == "binding" {
                db.execute(
                    "INSERT INTO stage_intents VALUES(0,?1)",
                    [marker.binding.as_ref().unwrap()],
                )
                .unwrap();
            } else {
                db.execute(
                    "INSERT INTO request_transfer_cursor_steps VALUES(?1,?2,?3,?4,?5,?6)",
                    rusqlite::params![step.0, step.1, step.2, step.3, step.4, step.5],
                )
                .unwrap();
            }
            drop(db);
            let jobs = start(root);
            assert_eq!(reconcile(&jobs).phase, Phase::Finished, "{fault}");
            assert!(!stage.directory.exists());
            close(jobs, root);
        });
    }
}
#[test]
fn refused_payload_registration_rolls_back_binding_and_journal_and_never_adopts_the_unknown_leaf() {
    fixture("refuse-payload", |root, marker| {
        let stage = work(&marker);
        assert!(stage.payload_identity.is_none());
        let unknown = stage.payload();
        assert_eq!(fs::metadata(&unknown).unwrap().len(), 0);
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        let (revision, accepted): (u64, Vec<u8>) = db
            .query_row(
                "SELECT revision,body FROM requests WHERE id=?1",
                [marker.request],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            revision, marker.revision,
            "a descriptive save replaced the accepted revision"
        );
        assert_eq!(Sha256::digest(&accepted).as_slice(), marker.accepted);
        assert_eq!(
            db.query_row("SELECT body FROM stage_intents WHERE lane=0", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            *marker.binding.as_ref().unwrap()
        );
        assert_eq!(
            db.query_row("SELECT body FROM pending", [], |r| r.get::<_, String>(0))
                .unwrap(),
            serde_json::to_string(&marker.pending).unwrap()
        );
        db.execute_batch("DROP TRIGGER refuse_stage_payload;")
            .unwrap();
        drop(db);
        for _ in 0..2 {
            let jobs = start(root);
            assert_ne!(reconcile(&jobs).phase, Phase::Finished);
            assert!(unknown.exists());
            assert_eq!(
                fs::read(root.join("b-current")).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            close(jobs, root);
        }
        let kept = root.join("kept-unregistered");
        fs::rename(&unknown, &kept).unwrap();
        let jobs = start(root);
        assert_eq!(reconcile(&jobs).phase, Phase::Finished);
        assert!(kept.exists());
        assert!(!stage.directory.exists());
        close(jobs, root);
    });
}

#[test]
fn original_preparation_parents_work_and_payloads_must_return_before_cleanup_can_continue() {
    for changed in ["parent", "work", "payload"] {
        fixture("written", |root, marker| {
            let stage = work(&marker);
            let before = fs::read(stage.payload()).unwrap();
            let held = root.join("held-original");
            match changed {
                "parent" => {
                    fs::rename(&marker.output, &held).unwrap();
                    fs::create_dir(&marker.output).unwrap();
                    fs::create_dir(&stage.directory).unwrap();
                }
                "work" => {
                    fs::rename(&stage.directory, &held).unwrap();
                    fs::create_dir(&stage.directory).unwrap();
                }
                "payload" => {
                    fs::rename(stage.payload(), &held).unwrap();
                }
                _ => unreachable!(),
            }
            fs::write(stage.payload(), b"New staged namespace").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished, "{changed}");
                assert_eq!(fs::read(stage.payload()).unwrap(), b"New staged namespace");
                close(jobs, root);
            }
            let foreign = root.join("kept-foreign");
            match changed {
                "parent" => {
                    fs::rename(&marker.output, &foreign).unwrap();
                    fs::rename(&held, &marker.output).unwrap();
                }
                "work" => {
                    fs::rename(&stage.directory, &foreign).unwrap();
                    fs::rename(&held, &stage.directory).unwrap();
                }
                _ => {
                    fs::rename(stage.payload(), &foreign).unwrap();
                    fs::rename(&held, stage.payload()).unwrap();
                }
            }
            assert_eq!(fs::read(stage.payload()).unwrap(), before);
            let jobs = start(root);
            let reviewed = reconcile(&jobs);
            assert_eq!(reviewed.phase, Phase::Finished, "{changed}: {reviewed:?}");
            assert!(!stage.directory.exists());
            let kept = match changed {
                "parent" => foreign
                    .join(stage.directory.file_name().unwrap())
                    .join("folder"),
                "work" => foreign.join("folder"),
                _ => foreign,
            };
            assert_eq!(fs::read(kept).unwrap(), b"New staged namespace");
            close(jobs, root);
        });
    }
}
#[test]
fn refused_cleanup_retirement_keeps_journal_and_binding_atomic_after_the_empty_root_is_gone() {
    fixture("cleanup", |root, marker| {
        let stage = work(&marker);
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        db.execute_batch("CREATE TRIGGER refuse_stage_retirement BEFORE DELETE ON pending BEGIN SELECT RAISE(ABORT,'private retirement refusal'); END;").unwrap();
        drop(db);
        let jobs = start(root);
        assert_ne!(reconcile(&jobs).phase, Phase::Finished);
        close(jobs, root);
        assert!(!stage.directory.exists());
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        assert_eq!(
            db.query_row("SELECT body FROM stage_intents WHERE lane=0", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            *marker.binding.as_ref().unwrap()
        );
        assert_eq!(
            db.query_row("SELECT body FROM pending", [], |r| r.get::<_, String>(0))
                .unwrap(),
            serde_json::to_string(&marker.pending).unwrap()
        );
        db.execute_batch("DROP TRIGGER refuse_stage_retirement;")
            .unwrap();
        drop(db);
        fs::write(target(&marker), b"Changed after cleanup syscall").unwrap();
        let jobs = start(root);
        let reviewed = reconcile(&jobs);
        assert_eq!(reviewed.phase, Phase::Finished, "{reviewed:?}");
        assert_eq!(
            fs::read(target(&marker)).unwrap(),
            b"Changed after cleanup syscall"
        );
        close(jobs, root);
    });
}
