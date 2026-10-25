//! Public service killed after its actual Copy append and before Move removal.
use super::*;
use crate::{
    local_activity::LocalActivity,
    operation_jobs::{OperationJobs, Phase, Request},
    storage::Storage,
};
use std::{
    os::unix::{ffi::OsStringExt, fs::MetadataExt, process::ExitStatusExt},
    time::{Duration, Instant},
};
fn leaf() -> std::ffi::OsString {
    std::ffi::OsString::from_vec(b"b-current-\xfd".to_vec())
}
pub(super) fn after_copy(source: &Path, destination: &Path) {
    if std::env::var("CANOPA_PUBLISHED_COPY_VARIANT").as_deref() == Ok("keep-both")
        && source.file_name() == Some(std::ffi::OsStr::new("a-done"))
    {
        fs::write(
            destination.parent().unwrap().join(leaf()),
            b"Existing current bytes",
        )
        .unwrap();
    }
    if std::env::var("CANOPA_PUBLISHED_COPY_VARIANT").as_deref() == Ok("terminal-remainder")
        && source.file_name() == Some(std::ffi::OsStr::new("a-remainder"))
    {
        fs::write(
            source.parent().unwrap().join("new-unselected"),
            b"New unselected bytes",
        )
        .unwrap();
    }
    boundary("copied", source);
}
pub(in crate::operations) fn claimed(source: &Path, cancellation: &Cancellation) {
    if std::env::var("CANOPA_PUBLISHED_COPY_STAGE").as_deref() == Ok("returned")
        && std::env::var_os("CANOPA_PUBLISHED_COPY_ROOT")
            .map(PathBuf::from)
            .is_some_and(|root| source.starts_with(root))
        && source.file_name() == Some(leaf().as_os_str())
    {
        cancellation.cancel();
    }
    boundary("ready", source);
}
pub(in crate::operations) fn boundary(stage: &str, source: &Path) {
    if std::env::var("CANOPA_PUBLISHED_COPY_STAGE")
        .as_deref()
        .unwrap_or("copied")
        != stage
    {
        return;
    }

    let Some(root) = std::env::var_os("CANOPA_PUBLISHED_COPY_ROOT").map(PathBuf::from) else {
        return;
    };
    if source.file_name() != Some(leaf().as_os_str()) {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-published-copy-")
    );
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let (request,operation):(u64,u64)=db.query_row("SELECT e.request,e.operation FROM request_executions e JOIN requests r ON r.id=e.request WHERE r.phase=1",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM pending", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        if matches!(
            stage,
            "unlinked" | "published" | "ready" | "returned" | "return-preparing"
        ) {
            1
        } else {
            0
        }
    );
    let marker = serde_json::json!({"request":request,"operation":operation});
    fs::write(
        root.join("marker.json"),
        serde_json::to_vec(&marker).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private worker was not killed");
}
fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(v) = probe() {
            return v;
        }
        assert!(
            Instant::now() < end,
            "published-copy service did not settle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn receive<T>(mut receiver: futures_channel::oneshot::Receiver<T>) -> T {
    wait(|| receiver.try_recv().expect("worker dropped reply"))
}
fn start(root: &Path) -> OperationJobs {
    start_held(root, LocalActivity::default())
}
fn start_held(root: &Path, activity: LocalActivity) -> OperationJobs {
    let jobs = OperationJobs::start_with_activity(
        root.join("history"),
        Storage::load(root.join("volumes")).unwrap(),
        activity,
    )
    .unwrap();
    wait(|| (!jobs.snapshot().initializing).then_some(()));
    assert!(jobs.snapshot().error.is_none(), "{:?}", jobs.snapshot());
    jobs
}
fn choose(jobs: &OperationJobs, id: u64) -> u64 {
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
    question.generation
}

fn fixture(kind: TransferKind, stage: &str, variant: &str, run: impl FnOnce(&Path, &Path, u64)) {
    let root = tempfile::Builder::new()
        .prefix("canopa-published-copy-")
        .tempdir()
        .unwrap();
    let output = tempfile::Builder::new()
        .prefix(".canopa-published-copy-output-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    if kind == TransferKind::Move
        && fs::metadata(root.path()).unwrap().dev() == fs::metadata(output.path()).unwrap().dev()
    {
        return;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::copy_boundary::published_leaf_child",
            ])
            .env("CANOPA_PUBLISHED_COPY_ROOT", root.path())
            .env("CANOPA_PUBLISHED_COPY_OUTPUT", output.path())
            .env(
                "CANOPA_PUBLISHED_COPY_KIND",
                if kind == TransferKind::Copy {
                    "Copy"
                } else {
                    "Move"
                },
            )
            .env("CANOPA_PUBLISHED_COPY_STAGE", stage)
            .env("CANOPA_PUBLISHED_COPY_VARIANT", variant)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let marker: serde_json::Value =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        run(
            root.path(),
            output.path(),
            marker["request"].as_u64().unwrap(),
        );
    }));
    if let Err(error) = result {
        eprintln!(
            "retained published-copy failure roots: {} and {}",
            root.keep().display(),
            output.keep().display()
        );
        std::panic::resume_unwind(error);
    }
}
fn close(jobs: OperationJobs, root: &Path) {
    drop(jobs);
    drop(wait(|| match OperationEngine::open(root.join("history")) {
        Ok(e) => Some(e),
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => None,
        Err(e) => panic!("{e}"),
    }));
}
fn current(output: &Path, variant: &str) -> PathBuf {
    output.join("bundle (2)").join(if variant == "keep-both" {
        let mut name = leaf();
        name.push(" (2)");
        name
    } else {
        leaf()
    })
}
fn check_leaf(path: &Path, variant: &str) {
    if variant == "symlink" {
        assert_eq!(
            fs::read_link(path).unwrap(),
            PathBuf::from(std::ffi::OsString::from_vec(b"missing-\xfe".to_vec()))
        );
    } else {
        assert_eq!(fs::read(path).unwrap(), b"Current selected bytes");
    }
}
#[test]
fn a_killed_published_leaf_resumes_without_recopying_its_committed_output() {
    for kind in [TransferKind::Copy, TransferKind::Move] {
        for variant in ["file", "keep-both", "symlink"] {
            fixture(kind, "copied", variant, |root, output, id| {
                let prefix = output.join("bundle (2)/a-done");
                let before = fs::metadata(&prefix).unwrap();
                fs::write(&prefix, b"Edited completed prefix").unwrap();
                let current = current(output, variant);
                let copied = fs::symlink_metadata(&current).unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert!(
                        row.transfer_actions.remaining,
                        "{kind:?}/{variant}: {row:?}"
                    );
                    check_leaf(&current, variant);
                    close(jobs, root);
                }
                let jobs = start(root);
                let next =
                    receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
                choose(&jobs, next);
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == next && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{kind:?}/{variant}: {done:?}");
                assert_eq!(fs::read(&prefix).unwrap(), b"Edited completed prefix");
                let after = fs::metadata(&prefix).unwrap();
                assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
                let after = fs::symlink_metadata(&current).unwrap();
                assert_eq!(
                    (copied.dev(), copied.ino()),
                    (after.dev(), after.ino()),
                    "committed output was recopied"
                );
                check_leaf(&current, variant);
                if variant == "keep-both" {
                    assert_eq!(
                        fs::read(output.join("bundle (2)").join(leaf())).unwrap(),
                        b"Existing current bytes"
                    );
                }
                assert_eq!(
                    fs::read(output.join("z-later (2)")).unwrap(),
                    b"Later selected bytes"
                );
                assert_eq!(
                    fs::symlink_metadata(root.join("bundle").join(leaf())).is_ok(),
                    kind == TransferKind::Copy
                );
                receive(jobs.shutdown().unwrap()).unwrap();
            });
        }
    }
}
#[test]
fn a_killed_copy_publication_restores_its_proved_cursor_after_explicit_review() {
    for kind in [TransferKind::Copy, TransferKind::Move] {
        fixture(kind, "published", "file", |root, output, id| {
            let copied = current(output, "file");
            let inode = fs::metadata(&copied).unwrap().ino();
            let jobs = start(root);
            assert!(
                !jobs
                    .snapshot()
                    .jobs
                    .iter()
                    .find(|j| j.id == id)
                    .unwrap()
                    .transfer_actions
                    .remaining
            );
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
            close(jobs, root);
            let jobs = start(root);
            let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
            choose(&jobs, next);
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == next && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{kind:?}: {done:?}");
            assert_eq!(fs::metadata(&copied).unwrap().ino(), inode);
            receive(jobs.shutdown().unwrap()).unwrap();
        });
    }
}
#[test]
fn a_killed_source_unlink_is_consumed_without_touching_new_source_or_output_bytes() {
    for stage in ["unlinked", "removed"] {
        fixture(TransferKind::Move, stage, "file", |root, output, id| {
            let source = root.join("bundle").join(leaf());
            fs::write(&source, b"New source occupant").unwrap();
            let current = current(output, "file");
            fs::write(&current, b"Edited completed output").unwrap();
            let original = fs::metadata(&current).unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                if stage == "unlinked" && row.phase == Phase::Interrupted {
                    assert!(!row.transfer_actions.remaining);
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
                assert!(row.transfer_actions.remaining, "{stage}: {row:?}");
                close(jobs, root);
            }
            let jobs = start(root);
            let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
            choose(&jobs, next);
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == next && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{stage}: {done:?}");
            assert_eq!(fs::read(&source).unwrap(), b"New source occupant");
            assert_eq!(fs::read(&current).unwrap(), b"Edited completed output");
            assert_eq!(fs::metadata(current).unwrap().ino(), original.ino());
            receive(jobs.shutdown().unwrap()).unwrap();
        });
    }
}
#[test]
fn a_published_move_refuses_replacement_sources_parents_and_outputs() {
    for fault in ["source", "source-parent", "output", "output-parent"] {
        fixture(TransferKind::Move, "copied", "file", |root, output, id| {
            let source = root.join("bundle").join(leaf());
            let current = current(output, "file");
            match fault {
                "source" => {
                    fs::rename(&source, root.join("preserved-original")).unwrap();
                    fs::write(&source, b"Current selected bytes").unwrap();
                }
                "source-parent" => {
                    fs::rename(root.join("bundle"), root.join("old-bundle")).unwrap();
                    fs::create_dir(root.join("bundle")).unwrap();
                    fs::hard_link(root.join("old-bundle").join(leaf()), &source).unwrap();
                }
                "output" => {
                    fs::rename(&current, output.join("preserved-output")).unwrap();
                    fs::write(&current, b"Current selected bytes").unwrap();
                }
                "output-parent" => {
                    fs::rename(output.join("bundle (2)"), output.join("old-bundle")).unwrap();
                    fs::create_dir(output.join("bundle (2)")).unwrap();
                    fs::hard_link(output.join("old-bundle").join(leaf()), &current).unwrap();
                }
                _ => unreachable!(),
            }
            let selected = fs::metadata(&source).unwrap();
            let copied = fs::metadata(&current).unwrap();
            for _ in 0..2 {
                let jobs = start(root);
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
                assert!(
                    receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).is_err()
                );
                assert_eq!(fs::metadata(&source).unwrap().ino(), selected.ino());
                assert_eq!(fs::metadata(&current).unwrap().ino(), copied.ino());
                assert_eq!(fs::read(&source).unwrap(), b"Current selected bytes");
                assert_eq!(fs::read(&current).unwrap(), b"Current selected bytes");
                close(jobs, root);
            }
        });
    }
}
#[test]
fn a_published_copy_refuses_missing_borrowed_and_changed_step_proofs() {
    for fault in [
        "missing",
        "task",
        "digest",
        "body",
        "ordinal",
        "committed",
        "save",
    ] {
        fixture(TransferKind::Copy, "copied", "file", |root, output, id| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            match fault {
                "missing" => {
                    db.execute("DELETE FROM request_transfer_cursor_steps", [])
                        .unwrap();
                }
                "task" => {
                    db.execute(
                        "UPDATE request_transfer_cursor_steps SET task_sha256=body_sha256",
                        [],
                    )
                    .unwrap();
                }
                "digest" => {
                    db.execute(
                        "UPDATE request_transfer_cursor_steps SET body_sha256=zeroblob(32)",
                        [],
                    )
                    .unwrap();
                }
                "body" => {
                    db.execute("UPDATE request_transfer_cursor_steps SET body=(SELECT body FROM changes ORDER BY ordinal LIMIT 1)",[]).unwrap();
                }
                "ordinal" => {
                    db.execute(
                        "UPDATE request_transfer_cursor_steps SET ordinal=ordinal+1",
                        [],
                    )
                    .unwrap();
                }
                "committed" => {
                    db.execute("UPDATE changes SET body=body||' ' WHERE ordinal=(SELECT MAX(ordinal) FROM changes)",[]).unwrap();
                }
                "save" => {
                    db.execute_batch("CREATE TRIGGER refuse_copy_stop BEFORE INSERT ON request_transfer_stops BEGIN SELECT RAISE(ABORT,'private copied stop refusal'); END").unwrap();
                }
                _ => unreachable!(),
            }
            let current = current(output, "file");
            fs::write(&current, b"Edited completed output").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                let expected = if fault == "save" {
                    Phase::SaveFailed
                } else {
                    Phase::Interrupted
                };
                assert_eq!(row.phase, expected, "{fault}: {row:?}");
                assert!(
                    !row.transfer_actions.remaining
                        && !row.transfer_actions.retry
                        && !row.transfer_actions.abandon,
                    "{fault}: {row:?}"
                );
                assert!(row.error.is_some());
                assert_eq!(
                    db.query_row(
                        "SELECT count(*) FROM request_transfer_cursors WHERE request=?1",
                        [id],
                        |r| r.get::<_, u64>(0)
                    )
                    .unwrap(),
                    1
                );
                assert_eq!(fs::read(&current).unwrap(), b"Edited completed output");
                close(jobs, root);
            }
        });
    }
}
#[test]
fn a_published_move_verifies_original_source_and_output_bytes_before_cleanup() {
    for fault in ["source-bytes", "output-bytes", "source-mode", "output-mode"] {
        fixture(TransferKind::Move, "copied", "file", |root, output, id| {
            let source = root.join("bundle").join(leaf());
            let current = current(output, "file");
            let path = if fault.starts_with("source") {
                &source
            } else {
                &current
            };
            if fault.ends_with("bytes") {
                fs::write(path, b"Newer edited bytes").unwrap();
            } else {
                let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
                fs::set_permissions(path, fs::Permissions::from_mode(mode ^ 0o100)).unwrap();
            }
            let source_before = Fingerprint::read(&source, &Cancellation::default()).unwrap();
            let copy_before = Fingerprint::read(&current, &Cancellation::default()).unwrap();
            let jobs = start(root);
            let row = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id)
                .unwrap();
            assert!(row.transfer_actions.remaining, "{fault}: {row:?}");
            let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
            choose(&jobs, next);
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == next && !j.phase.holds_application())
            });
            assert!(done.transfer_actions.retry, "{fault}: {done:?}");
            assert_eq!(
                Fingerprint::read(&source, &Cancellation::default()).unwrap(),
                source_before
            );
            assert_eq!(
                Fingerprint::read(&current, &Cancellation::default()).unwrap(),
                copy_before
            );
            assert_eq!(done.transfer.as_ref().unwrap().failures.len(), 1);
            receive(jobs.abandon_transfer(next).unwrap()).unwrap();
            receive(jobs.shutdown().unwrap()).unwrap();
        });
    }
}
#[test]
fn a_second_crash_during_resumed_source_cleanup_keeps_the_same_committed_copy() {
    for stage in ["unlinked", "removed"] {
        fixture(TransferKind::Move, "copied", "file", |root, output, id| {
            let copied = current(output, "file");
            let inode = fs::metadata(&copied).unwrap().ino();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::transfer::copy_boundary::resume_leaf_child",
                ])
                .env("CANOPA_PUBLISHED_COPY_ROOT", root)
                .env("CANOPA_PUBLISHED_COPY_OUTPUT", output)
                .env("CANOPA_PUBLISHED_COPY_STAGE", stage)
                .env("CANOPA_PUBLISHED_COPY_REQUEST", id.to_string())
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let marker: serde_json::Value =
                serde_json::from_slice(&fs::read(root.join("marker.json")).unwrap()).unwrap();
            let next = marker["request"].as_u64().unwrap();
            assert_ne!(id, next);
            for _ in 0..2 {
                let jobs = start(root);
                if stage == "unlinked"
                    && jobs
                        .snapshot()
                        .jobs
                        .iter()
                        .any(|j| j.id == next && j.phase == Phase::Interrupted)
                {
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
                    .find(|j| j.id == next)
                    .unwrap();
                assert!(row.transfer_actions.remaining, "{stage}: {row:?}");
                assert_eq!(fs::metadata(&copied).unwrap().ino(), inode);
                close(jobs, root);
            }
            let jobs = start(root);
            let last =
                receive(jobs.resume_transfer(next, TransferPart::Remaining).unwrap()).unwrap();
            choose(&jobs, last);
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == last && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{stage}: {done:?}");
            assert_eq!(fs::metadata(&copied).unwrap().ino(), inode);
            check_leaf(&copied, "file");
            receive(jobs.shutdown().unwrap()).unwrap();
        });
    }
}
#[test]
fn a_killed_final_leaf_restores_completion_without_reopening_completed_paths() {
    for kind in [TransferKind::Copy, TransferKind::Move] {
        for fault in ["edited", "replaced", "parent", "missing-parent"] {
            fixture(
                kind,
                if kind == TransferKind::Copy {
                    "copied"
                } else {
                    "removed"
                },
                "terminal-file",
                |root, output, id| {
                    let source = root.join(leaf());
                    if kind == TransferKind::Copy {
                        fs::rename(&source, root.join("preserved-source")).unwrap();
                    }
                    fs::write(&source, b"New source occupant").unwrap();
                    let target = output.join("target");
                    let copied = target.join(leaf());
                    let original = fs::metadata(&copied).unwrap();
                    let saved = match fault {
                        "edited" => {
                            fs::write(&copied, b"Edited completed output").unwrap();
                            copied.clone()
                        }
                        "replaced" => {
                            fs::rename(&copied, output.join("preserved-output")).unwrap();
                            fs::write(&copied, b"New output occupant").unwrap();
                            output.join("preserved-output")
                        }
                        "parent" | "missing-parent" => {
                            fs::rename(&target, output.join("preserved-parent")).unwrap();
                            if fault == "parent" {
                                fs::create_dir(&target).unwrap();
                                fs::write(&copied, b"New parent occupant").unwrap();
                            }
                            output.join("preserved-parent").join(leaf())
                        }
                        _ => unreachable!(),
                    };
                    for _ in 0..2 {
                        let jobs = start(root);
                        let row = jobs
                            .snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == id)
                            .unwrap();
                        assert_eq!(row.phase, Phase::Finished, "{kind:?}/{fault}: {row:?}");
                        let report = row.transfer.as_ref().unwrap();
                        assert_eq!(report.completed, 1);
                        assert_eq!(report.bytes, 22);
                        assert_eq!(report.destinations, vec![copied.clone()]);
                        assert!(
                            !row.transfer_actions.retry
                                && !row.transfer_actions.remaining
                                && !row.transfer_actions.abandon
                        );
                        assert_eq!(fs::read(&source).unwrap(), b"New source occupant");
                        assert_eq!(fs::metadata(&saved).unwrap().ino(), original.ino());
                        assert_eq!(
                            fs::read(&saved).unwrap(),
                            if fault == "edited" {
                                b"Edited completed output".as_slice()
                            } else {
                                b"Current selected bytes".as_slice()
                            }
                        );
                        if fault == "replaced" {
                            assert_eq!(fs::read(&copied).unwrap(), b"New output occupant");
                        }
                        if fault == "parent" {
                            assert_eq!(fs::read(&copied).unwrap(), b"New parent occupant");
                        }
                        if fault == "missing-parent" {
                            assert!(!target.exists());
                        }
                        close(jobs, root);
                    }
                },
            );
        }
    }
}
#[test]
fn a_final_leaf_keeps_earlier_source_folder_remainders_in_its_completed_report() {
    fixture(
        TransferKind::Move,
        "removed",
        "terminal-remainder",
        |root, output, id| {
            for _ in 0..2 {
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Finished, "{row:?}");
                assert_eq!(
                    row.transfer.as_ref().unwrap().source_remainders,
                    vec![root.join("a-folder")]
                );
                assert_eq!(
                    fs::read(root.join("a-folder/new-unselected")).unwrap(),
                    b"New unselected bytes"
                );
                assert_eq!(
                    fs::read(output.join("target/a-folder/a-remainder")).unwrap(),
                    b"Earlier source bytes"
                );
                close(jobs, root);
            }
        },
    );
}
#[test]
fn a_final_leaf_journal_review_restores_completion_before_and_after_reopening() {
    for (kind, stage) in [
        (TransferKind::Copy, "published"),
        (TransferKind::Move, "unlinked"),
    ] {
        fixture(kind, stage, "terminal-file", |root, output, id| {
            let jobs = start(root);
            let row = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id)
                .unwrap();
            assert_eq!(row.phase, Phase::Interrupted);
            assert!(!row.transfer_actions.remaining);
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
            assert_eq!(row.phase, Phase::Finished, "{kind:?}: {row:?}");
            assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
            fs::write(output.join("target").join(leaf()), b"Edited after review").unwrap();
            close(jobs, root);
            for _ in 0..2 {
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Finished, "{row:?}");
                assert_eq!(
                    fs::read(output.join("target").join(leaf())).unwrap(),
                    b"Edited after review"
                );
                close(jobs, root);
            }
        });
    }
}
#[test]
fn final_completion_and_cursor_retirement_roll_back_together_on_save_failure() {
    for (kind, stage) in [
        (TransferKind::Copy, "copied"),
        (TransferKind::Move, "unlinked"),
    ] {
        for fault in ["end", "retire"] {
            fixture(kind, stage, "terminal-file", |root, output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                db.execute_batch(if fault=="end" {"CREATE TRIGGER refuse_terminal_end BEFORE UPDATE OF result ON request_transfer_ends WHEN NEW.result IS NOT NULL BEGIN SELECT RAISE(ABORT,'private terminal end refusal'); END"} else {"CREATE TRIGGER refuse_terminal_end BEFORE DELETE ON request_transfer_cursors BEGIN SELECT RAISE(ABORT,'private terminal retirement refusal'); END"}).unwrap();
                let body: Vec<u8> = db
                    .query_row("SELECT body FROM requests WHERE id=?1", [id], |r| r.get(0))
                    .unwrap();
                let copied = output.join("target").join(leaf());
                if stage == "copied" {
                    fs::write(&copied, b"Edited completed output").unwrap();
                }
                for attempt in 0..2 {
                    let activity = LocalActivity::default();
                    let jobs = start_held(root, activity.clone());
                    if stage == "unlinked" && attempt == 0 {
                        let review =
                            receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
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
                    assert_eq!(row.phase, Phase::SaveFailed, "{fault}: {row:?}");
                    assert!(row.error.is_some());
                    assert!(activity.holds_application());
                    fs::write(&copied, b"Edited completed output").unwrap();
                    assert_eq!(
                        db.query_row("SELECT count(*) FROM request_transfer_cursors", [], |r| r
                            .get::<_, u64>(
                            0
                        ))
                        .unwrap(),
                        1
                    );
                    assert!(
                        db.query_row(
                            "SELECT result IS NULL FROM request_transfer_ends",
                            [],
                            |r| r.get::<_, bool>(0)
                        )
                        .unwrap()
                    );
                    assert_eq!(
                        db.query_row("SELECT body FROM requests WHERE id=?1", [id], |r| r
                            .get::<_, Vec<u8>>(0))
                            .unwrap(),
                        body
                    );
                    assert_eq!(fs::read(&copied).unwrap(), b"Edited completed output");
                    if attempt == 1 {
                        jobs.retry_save(id).unwrap();
                        let failed = wait(|| {
                            jobs.snapshot()
                                .jobs
                                .into_iter()
                                .find(|j| j.id == id && j.error != row.error)
                        });
                        assert_eq!(failed.phase, Phase::SaveFailed, "{failed:?}");
                        assert!(activity.holds_application());
                        db.execute_batch("DROP TRIGGER refuse_terminal_end")
                            .unwrap();
                        jobs.retry_save(id).unwrap();
                        let row = wait(|| {
                            jobs.snapshot()
                                .jobs
                                .into_iter()
                                .find(|job| job.id == id && job.phase == Phase::Finished)
                        });
                        assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
                        wait(|| (!activity.holds_application()).then_some(()));
                        assert_eq!(fs::read(&copied).unwrap(), b"Edited completed output");
                    }
                    close(jobs, root);
                }
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert_eq!(row.phase, Phase::Finished, "{fault}: {row:?}");
                assert_eq!(fs::read(&copied).unwrap(), b"Edited completed output");
                receive(jobs.shutdown().unwrap()).unwrap();
            });
        }
    }
}

#[test]
fn recovered_stops_retry_their_end_save_before_gaining_continuation_owners() {
    for (kind, stage) in [
        (TransferKind::Copy, "copied"),
        (TransferKind::Copy, "published"),
        (TransferKind::Move, "copied"),
        (TransferKind::Move, "unlinked"),
    ] {
        for fault in ["stop", "retire"] {
            fixture(kind, stage, "file", |root, output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                db.execute_batch(if fault == "stop" {
                    "CREATE TRIGGER refuse_recovered_stop BEFORE INSERT ON request_transfer_stops BEGIN SELECT RAISE(ABORT,'private recovered stop refusal'); END"
                } else {
                    "CREATE TRIGGER refuse_recovered_stop BEFORE DELETE ON request_transfer_cursors BEGIN SELECT RAISE(ABORT,'private recovered cursor refusal'); END"
                }).unwrap();
                let body: Vec<u8> = db
                    .query_row("SELECT body FROM requests WHERE id=?1", [id], |r| r.get(0))
                    .unwrap();
                let copied = current(output, "file");
                let inode = fs::metadata(&copied).unwrap().ino();
                let source = root.join("bundle").join(leaf());
                let source_existed = source.exists();
                let activity = LocalActivity::default();
                let jobs = start_held(root, activity.clone());
                if stage != "copied" {
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
                assert_eq!(
                    row.phase,
                    Phase::SaveFailed,
                    "{kind:?}/{stage}/{fault}: {row:?}"
                );
                assert!(activity.holds_application());
                assert!(
                    !row.transfer_actions.retry
                        && !row.transfer_actions.remaining
                        && !row.transfer_actions.abandon
                );
                assert!(
                    receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).is_err()
                );
                assert!(receive(jobs.abandon_transfer(id).unwrap()).is_err());
                assert_eq!(
                    db.query_row("SELECT count(*) FROM request_transfer_cursors", [], |r| r
                        .get::<_, u64>(
                        0
                    ))
                    .unwrap(),
                    1
                );
                assert_eq!(
                    db.query_row("SELECT count(*) FROM request_transfer_stops", [], |r| r
                        .get::<_, u64>(0))
                        .unwrap(),
                    0
                );
                assert_eq!(
                    db.query_row("SELECT body FROM requests WHERE id=?1", [id], |r| r
                        .get::<_, Vec<u8>>(0))
                        .unwrap(),
                    body
                );
                fs::write(
                    output.join("bundle (2)/a-done"),
                    b"Edited completed prefix before save retry",
                )
                .unwrap();
                jobs.retry_save(id).unwrap();
                let failed = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id && j.error != row.error)
                });
                assert_eq!(failed.phase, Phase::SaveFailed, "{failed:?}");
                assert!(activity.holds_application());
                db.execute_batch("DROP TRIGGER refuse_recovered_stop")
                    .unwrap();
                jobs.retry_save(id).unwrap();
                let stopped = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id && j.phase != Phase::SaveFailed)
                });
                assert_eq!(stopped.phase, Phase::Stopped, "{stopped:?}");
                assert!(stopped.transfer_actions.remaining && stopped.transfer_actions.abandon);
                assert!(activity.holds_application());
                assert_eq!(
                    db.query_row("SELECT count(*) FROM request_transfer_cursors", [], |r| r
                        .get::<_, u64>(
                        0
                    ))
                    .unwrap(),
                    0
                );
                assert_eq!(fs::metadata(&copied).unwrap().ino(), inode);
                assert_eq!(source.exists(), source_existed);
                assert_eq!(
                    fs::read(output.join("bundle (2)/a-done")).unwrap(),
                    b"Edited completed prefix before save retry"
                );
                assert_eq!(
                    fs::read(output.join("z-later")).unwrap(),
                    b"Existing later bytes"
                );
                receive(jobs.abandon_transfer(id).unwrap()).unwrap();
                wait(|| (!activity.holds_application()).then_some(()));
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
fn recovered_end_save_witnesses_refuse_changed_requests_and_reservations() {
    for terminal in [false, true] {
        for fault in ["body", "revision", "intent"] {
            fixture(
                TransferKind::Copy,
                "copied",
                if terminal { "terminal-file" } else { "file" },
                |root, output, id| {
                    let db = rusqlite::Connection::open(root.join("history")).unwrap();
                    db.execute_batch(if terminal {
                    "CREATE TRIGGER refuse_recovered_end BEFORE UPDATE OF result ON request_transfer_ends WHEN NEW.result IS NOT NULL BEGIN SELECT RAISE(ABORT,'private end refusal'); END"
                } else {
                    "CREATE TRIGGER refuse_recovered_end BEFORE INSERT ON request_transfer_stops BEGIN SELECT RAISE(ABORT,'private end refusal'); END"
                }).unwrap();
                    let activity = LocalActivity::default();
                    let jobs = start_held(root, activity.clone());
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert_eq!(row.phase, Phase::SaveFailed, "{row:?}");
                    let body: Vec<u8> = db
                        .query_row("SELECT body FROM requests WHERE id=?1", [id], |r| r.get(0))
                        .unwrap();
                    let intent: String = db
                        .query_row(
                            "SELECT intent FROM request_transfer_ends WHERE request=?1",
                            [id],
                            |r| r.get(0),
                        )
                        .unwrap();
                    db.execute_batch("DROP TRIGGER refuse_recovered_end")
                        .unwrap();
                    db.execute_batch(match fault {
                        "body" => "UPDATE requests SET body=CAST(CAST(body AS TEXT)||' ' AS BLOB)",
                        "revision" => "UPDATE requests SET revision=revision+1",
                        "intent" => "UPDATE request_transfer_ends SET intent=intent||' '",
                        _ => unreachable!(),
                    })
                    .unwrap();
                    let copied = if terminal {
                        output.join("target").join(leaf())
                    } else {
                        current(output, "file")
                    };
                    fs::write(&copied, b"Completed output edited before refused retry").unwrap();
                    jobs.retry_save(id).unwrap();
                    let failed = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == id && j.error != row.error)
                    });
                    assert_eq!(failed.phase, Phase::SaveFailed, "{fault}: {failed:?}");
                    assert!(
                        !failed.transfer_actions.retry
                            && !failed.transfer_actions.remaining
                            && !failed.transfer_actions.abandon
                    );
                    assert!(activity.holds_application());
                    assert_eq!(
                        db.query_row("SELECT count(*) FROM request_transfer_cursors", [], |r| r
                            .get::<_, u64>(
                            0
                        ))
                        .unwrap(),
                        1
                    );
                    assert_eq!(
                        db.query_row("SELECT count(*) FROM request_transfer_stops", [], |r| r
                            .get::<_, u64>(0))
                            .unwrap(),
                        0
                    );
                    assert!(
                        db.query_row(
                            "SELECT result IS NULL FROM request_transfer_ends",
                            [],
                            |r| r.get::<_, bool>(0)
                        )
                        .unwrap()
                    );
                    assert_eq!(
                        fs::read(&copied).unwrap(),
                        b"Completed output edited before refused retry"
                    );
                    match fault {
                        "body" => {
                            db.execute(
                                "UPDATE requests SET body=?2 WHERE id=?1",
                                rusqlite::params![id, body],
                            )
                            .unwrap();
                        }
                        "revision" => {
                            db.execute("UPDATE requests SET revision=revision-1 WHERE id=?1", [id])
                                .unwrap();
                        }
                        "intent" => {
                            db.execute(
                                "UPDATE request_transfer_ends SET intent=?2 WHERE request=?1",
                                rusqlite::params![id, intent],
                            )
                            .unwrap();
                        }
                        _ => unreachable!(),
                    }
                    jobs.retry_save(id).unwrap();
                    let done = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == id && j.phase != Phase::SaveFailed)
                    });
                    if terminal {
                        assert_eq!(done.phase, Phase::Finished, "{done:?}");
                    } else {
                        assert_eq!(done.phase, Phase::Stopped, "{done:?}");
                        assert!(done.transfer_actions.remaining);
                        receive(jobs.abandon_transfer(id).unwrap()).unwrap();
                    }
                    wait(|| (!activity.holds_application()).then_some(()));
                    assert_eq!(
                        fs::read(&copied).unwrap(),
                        b"Completed output edited before refused retry"
                    );
                    receive(jobs.shutdown().unwrap()).unwrap();
                },
            );
        }
    }
}

#[test]
fn a_restored_final_result_retries_its_application_save_without_filesystem_work() {
    for (kind, stage) in [
        (TransferKind::Copy, "copied"),
        (TransferKind::Move, "unlinked"),
    ] {
        fixture(kind, stage, "terminal-file", |root, output, id| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            db.execute_batch(&format!("CREATE TRIGGER refuse_terminal_result BEFORE UPDATE OF phase ON requests WHEN OLD.id={id} AND OLD.phase=1 AND NEW.phase=5 BEGIN SELECT RAISE(ABORT,'private terminal result refusal'); END")).unwrap();
            let activity = LocalActivity::default();
            let jobs = start_held(root, activity.clone());
            if stage == "unlinked" {
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
            assert_eq!(row.phase, Phase::SaveFailed, "{kind:?}: {row:?}");
            assert!(activity.holds_application());
            assert_eq!(
                db.query_row("SELECT count(*) FROM request_transfer_cursors", [], |r| r
                    .get::<_, u64>(
                    0
                ))
                .unwrap(),
                0
            );
            fs::write(
                output.join("target").join(leaf()),
                b"Edited before metadata retry",
            )
            .unwrap();
            db.execute_batch("DROP TRIGGER refuse_terminal_result")
                .unwrap();
            jobs.retry_save(id).unwrap();
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id && j.phase != Phase::SaveFailed)
            });
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            wait(|| (!activity.holds_application()).then_some(()));
            assert_eq!(
                fs::read(output.join("target").join(leaf())).unwrap(),
                b"Edited before metadata retry"
            );
            receive(jobs.shutdown().unwrap()).unwrap();
        });
    }
}
#[test]
fn a_second_crash_after_final_resumed_cleanup_restores_only_its_completed_result() {
    for stage in ["unlinked", "removed"] {
        fixture(
            TransferKind::Move,
            "copied",
            "terminal-file",
            |root, output, id| {
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "operations::transfer::copy_boundary::resume_leaf_child",
                    ])
                    .env("CANOPA_PUBLISHED_COPY_ROOT", root)
                    .env("CANOPA_PUBLISHED_COPY_OUTPUT", output)
                    .env("CANOPA_PUBLISHED_COPY_STAGE", stage)
                    .env("CANOPA_PUBLISHED_COPY_REQUEST", id.to_string())
                    .status()
                    .unwrap();
                assert_eq!(status.signal(), Some(9));
                let marker: serde_json::Value =
                    serde_json::from_slice(&fs::read(root.join("marker.json")).unwrap()).unwrap();
                let next = marker["request"].as_u64().unwrap();
                assert_ne!(id, next);
                fs::write(root.join(leaf()), b"New source after final cleanup").unwrap();
                fs::write(
                    output.join("target").join(leaf()),
                    b"Edited after final cleanup",
                )
                .unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    if stage == "unlinked"
                        && jobs
                            .snapshot()
                            .jobs
                            .iter()
                            .any(|j| j.id == next && j.phase == Phase::Interrupted)
                    {
                        let review =
                            receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
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
                        .find(|j| j.id == next)
                        .unwrap();
                    assert_eq!(row.phase, Phase::Finished, "{stage}: {row:?}");
                    assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
                    assert_eq!(row.transfer.as_ref().unwrap().bytes, 0);
                    assert!(!row.transfer_actions.remaining && !row.transfer_actions.retry);
                    assert_eq!(
                        fs::read(root.join(leaf())).unwrap(),
                        b"New source after final cleanup"
                    );
                    assert_eq!(
                        fs::read(output.join("target").join(leaf())).unwrap(),
                        b"Edited after final cleanup"
                    );
                    close(jobs, root);
                }
            },
        );
    }
}
#[test]
fn a_final_dangling_link_has_exact_completion_without_following_new_targets() {
    for kind in [TransferKind::Copy, TransferKind::Move] {
        fixture(
            kind,
            if kind == TransferKind::Copy {
                "copied"
            } else {
                "removed"
            },
            "terminal-link",
            |root, output, id| {
                let source = root.join(leaf());
                if kind == TransferKind::Copy {
                    fs::remove_file(&source).unwrap();
                }
                let target = std::ffi::OsString::from_vec(b"new-missing-\xfc".to_vec());
                std::os::unix::fs::symlink(&target, &source).unwrap();
                let copied = output.join("target").join(leaf());
                fs::remove_file(&copied).unwrap();
                std::os::unix::fs::symlink(&target, &copied).unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert_eq!(row.phase, Phase::Finished, "{kind:?}: {row:?}");
                    assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
                    assert_eq!(row.transfer.as_ref().unwrap().bytes, 0);
                    assert_eq!(fs::read_link(&source).unwrap(), PathBuf::from(&target));
                    assert_eq!(fs::read_link(&copied).unwrap(), PathBuf::from(&target));
                    close(jobs, root);
                }
            },
        );
    }
}
#[test]
fn a_final_leaf_refuses_damaged_step_and_remainder_evidence_without_recording_an_end() {
    for fault in [
        "step-missing",
        "step-digest",
        "task-missing",
        "remainder-missing",
        "remainder-borrowed",
    ] {
        let remainder = fault.starts_with("remainder");
        fixture(
            if remainder {
                TransferKind::Move
            } else {
                TransferKind::Copy
            },
            if remainder { "removed" } else { "copied" },
            if remainder {
                "terminal-remainder"
            } else {
                "terminal-file"
            },
            |root, output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                match fault {
                    "step-missing" => {
                        db.execute("DELETE FROM request_transfer_cursor_steps", [])
                            .unwrap();
                    }
                    "step-digest" => {
                        db.execute(
                            "UPDATE request_transfer_cursor_steps SET body_sha256=zeroblob(32)",
                            [],
                        )
                        .unwrap();
                    }
                    "task-missing" => {
                        db.execute("DELETE FROM request_transfer_tasks WHERE role=0", [])
                            .unwrap();
                    }
                    "remainder-missing" => {
                        assert_eq!(
                            db.execute("DELETE FROM request_transfer_tasks WHERE role=7", [])
                                .unwrap(),
                            1
                        );
                    }
                    "remainder-borrowed" => {
                        assert_eq!(db.execute("UPDATE request_transfer_tasks SET body=(SELECT body FROM request_transfer_tasks WHERE role=4 LIMIT 1),body_sha256=(SELECT body_sha256 FROM request_transfer_tasks WHERE role=4 LIMIT 1) WHERE role=7",[]).unwrap(),1);
                    }
                    _ => unreachable!(),
                }
                let copied = output.join("target").join(leaf());
                fs::write(&copied, b"Edited final output").unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert_eq!(row.phase, Phase::Interrupted, "{fault}: {row:?}");
                    assert!(
                        !row.transfer_actions.remaining
                            && !row.transfer_actions.retry
                            && !row.transfer_actions.abandon
                    );
                    assert!(
                        db.query_row(
                            "SELECT result IS NULL FROM request_transfer_ends",
                            [],
                            |r| r.get::<_, bool>(0)
                        )
                        .unwrap()
                    );
                    assert_eq!(
                        db.query_row("SELECT count(*) FROM request_transfer_cursors", [], |r| r
                            .get::<_, u64>(
                            0
                        ))
                        .unwrap(),
                        1
                    );
                    assert_eq!(fs::read(&copied).unwrap(), b"Edited final output");
                    if remainder {
                        assert_eq!(
                            fs::read(root.join("a-folder/new-unselected")).unwrap(),
                            b"New unselected bytes"
                        );
                    }
                    close(jobs, root);
                }
            },
        );
    }
}
#[test]
fn an_original_inode_reintroduced_after_unlink_is_a_new_name_to_preserve() {
    for resumed in [false, true] {
        fixture(
            TransferKind::Move,
            if resumed { "copied" } else { "unlinked" },
            "terminal-alias",
            |root, output, mut id| {
                if resumed {
                    let status = std::process::Command::new(std::env::current_exe().unwrap())
                        .args([
                            "--ignored",
                            "--exact",
                            "operations::transfer::copy_boundary::resume_leaf_child",
                        ])
                        .env("CANOPA_PUBLISHED_COPY_ROOT", root)
                        .env("CANOPA_PUBLISHED_COPY_OUTPUT", output)
                        .env("CANOPA_PUBLISHED_COPY_STAGE", "unlinked")
                        .env("CANOPA_PUBLISHED_COPY_REQUEST", id.to_string())
                        .status()
                        .unwrap();
                    assert_eq!(status.signal(), Some(9));
                    let marker: serde_json::Value =
                        serde_json::from_slice(&fs::read(root.join("marker.json")).unwrap())
                            .unwrap();
                    id = marker["request"].as_u64().unwrap();
                }
                let alias = root.join("preserved-alias");
                let source = root.join(leaf());
                fs::hard_link(&alias, &source).unwrap();
                let inode = fs::metadata(&source).unwrap().ino();
                let jobs = start(root);
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
                assert_eq!(row.phase, Phase::Finished, "{resumed}: {row:?}");
                assert!(!row.transfer_actions.remaining && !row.transfer_actions.retry);
                close(jobs, root);
                for _ in 0..2 {
                    let jobs = start(root);
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert_eq!(row.phase, Phase::Finished, "{row:?}");
                    assert_eq!(fs::metadata(&source).unwrap().ino(), inode);
                    assert_eq!(fs::metadata(&alias).unwrap().ino(), inode);
                    assert_eq!(fs::read(&source).unwrap(), b"Current selected bytes");
                    assert_eq!(
                        fs::read(output.join("target").join(leaf())).unwrap(),
                        b"Current selected bytes"
                    );
                    close(jobs, root);
                }
            },
        );
    }
}

#[test]
fn returned_source_claims_resume_their_original_cleanup_without_recopying() {
    for stage in ["ready", "returned"] {
        fixture(
            TransferKind::Move,
            stage,
            "terminal-file",
            |root, output, id| {
                let copied = output.join("target").join(leaf());
                let before = fs::metadata(&copied).unwrap();
                let jobs = start(root);
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
                assert!(row.transfer_actions.remaining, "{stage}: {row:?}");
                close(jobs, root);
                for _ in 0..2 {
                    let jobs = start(root);
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert!(row.transfer_actions.remaining, "{stage}: {row:?}");
                    assert_eq!(
                        fs::read(root.join(leaf())).unwrap(),
                        b"Current selected bytes"
                    );
                    assert_eq!(fs::metadata(&copied).unwrap().ino(), before.ino());
                    close(jobs, root);
                }
                let jobs = start(root);
                let next =
                    receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == next && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{stage}: {done:?}");
                assert_eq!(fs::metadata(&copied).unwrap().ino(), before.ino());
                assert_eq!(fs::read(&copied).unwrap(), b"Current selected bytes");
                assert!(!root.join(leaf()).exists());
                receive(jobs.shutdown().unwrap()).unwrap();
            },
        );
    }
}
#[test]
fn source_return_disposition_rolls_back_and_refuses_changed_task_proofs() {
    for fault in [
        "delete",
        "prepare",
        "step-body",
        "step-hash",
        "head-hash",
        "context",
    ] {
        fixture(
            TransferKind::Move,
            "ready",
            "terminal-file",
            |root, output, id| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                let before: Pending = serde_json::from_str(
                    &db.query_row("SELECT body FROM pending", [], |r| r.get::<_, String>(0))
                        .unwrap(),
                )
                .unwrap();
                let Pending::Forward {
                    stage: Some(stage), ..
                } = before
                else {
                    panic!("missing source claim")
                };
                let payload = stage.payload();
                let damaged = match fault {
                    "delete" => {
                        db.execute_batch("CREATE TRIGGER refuse_source_return BEFORE DELETE ON request_transfer_cursor_steps WHEN OLD.ordinal=1 BEGIN SELECT RAISE(ABORT,'private source return refusal'); END").unwrap();
                        false
                    }
                    "prepare" => {
                        db.execute_batch(r#"CREATE TRIGGER refuse_source_return BEFORE INSERT ON pending WHEN instr(NEW.body,'"direction":"preparing"')>0 BEGIN SELECT RAISE(ABORT,'private source return refusal'); END"#).unwrap();
                        false
                    }
                    "step-body" => {
                        db.execute("UPDATE request_transfer_cursor_steps SET body=body||' ' WHERE ordinal=1",[]).unwrap();
                        true
                    }
                    "step-hash" => {
                        db.execute("UPDATE request_transfer_cursor_steps SET body_sha256=zeroblob(32) WHERE ordinal=1",[]).unwrap();
                        true
                    }
                    "head-hash" => {
                        db.execute("UPDATE request_transfer_tasks SET body_sha256=zeroblob(32) WHERE role=0",[]).unwrap();
                        true
                    }
                    "context" => {
                        let text: String = db
                            .query_row("SELECT body FROM pending_bindings WHERE lane=0", [], |r| {
                                r.get(0)
                            })
                            .unwrap();
                        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
                        let byte = value["forward"]["digest"][0].as_u64().unwrap();
                        value["forward"]["digest"][0] = serde_json::json!(byte ^ 1);
                        db.execute(
                            "UPDATE pending_bindings SET body=?1 WHERE lane=0",
                            [serde_json::to_string(&value).unwrap()],
                        )
                        .unwrap();
                        true
                    }
                    _ => unreachable!(),
                };
                let bound: String = db
                    .query_row(
                        "SELECT body FROM request_transfer_cursor_steps WHERE ordinal=1",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                    let done = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == review && !j.phase.holds_application())
                    });
                    assert_ne!(done.phase, Phase::Finished, "{fault}: {done:?}");
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
                    assert_eq!(
                        db.query_row(
                            "SELECT body FROM request_transfer_cursor_steps WHERE ordinal=1",
                            [],
                            |r| r.get::<_, String>(0)
                        )
                        .unwrap(),
                        bound
                    );
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
                        fs::read(output.join("target").join(leaf())).unwrap(),
                        b"Current selected bytes"
                    );
                    let retained = if root.join(leaf()).exists() {
                        root.join(leaf())
                    } else {
                        payload.clone()
                    };
                    assert_eq!(fs::read(retained).unwrap(), b"Current selected bytes");
                    close(jobs, root);
                }
                if !damaged {
                    db.execute_batch("DROP TRIGGER refuse_source_return")
                        .unwrap();
                    let jobs = start(root);
                    let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                    let done = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == review && !j.phase.holds_application())
                    });
                    assert_eq!(done.phase, Phase::Finished, "{fault}: {done:?}");
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert!(row.transfer_actions.remaining, "{row:?}");
                    receive(jobs.abandon_transfer(id).unwrap()).unwrap();
                    receive(jobs.shutdown().unwrap()).unwrap();
                }
            },
        );
    }
}
#[test]
fn a_second_crash_during_source_return_retains_only_empty_cleanup_rights() {
    for foreign in [false, true] {
        fixture(
            TransferKind::Move,
            "ready",
            "terminal-file",
            |root, output, id| {
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "operations::transfer::copy_boundary::review_return_child",
                    ])
                    .env("CANOPA_PUBLISHED_COPY_ROOT", root)
                    .env("CANOPA_PUBLISHED_COPY_STAGE", "return-preparing")
                    .status()
                    .unwrap();
                assert_eq!(status.signal(), Some(9));
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                let pending: Pending = serde_json::from_str(
                    &db.query_row("SELECT body FROM pending", [], |r| r.get::<_, String>(0))
                        .unwrap(),
                )
                .unwrap();
                let Pending::Preparing { stage, .. } = pending else {
                    panic!("return preparation not saved")
                };
                assert_eq!(
                    db.query_row(
                        "SELECT count(*) FROM request_transfer_cursor_steps WHERE ordinal=1",
                        [],
                        |r| r.get::<_, u64>(0)
                    )
                    .unwrap(),
                    0
                );
                assert_eq!(
                    db.query_row(
                        "SELECT count(*) FROM request_transfer_cursor_steps WHERE ordinal=0",
                        [],
                        |r| r.get::<_, u64>(0)
                    )
                    .unwrap(),
                    1
                );
                if foreign {
                    fs::write(stage.payload(), b"Unexpected private contents").unwrap();
                }
                let copied = output.join("target").join(leaf());
                let inode = fs::metadata(&copied).unwrap().ino();
                let jobs = start(root);
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                if foreign {
                    assert_ne!(done.phase, Phase::Finished, "{done:?}");
                    assert_eq!(
                        fs::read(stage.payload()).unwrap(),
                        b"Unexpected private contents"
                    );
                    assert_eq!(
                        fs::read(root.join(leaf())).unwrap(),
                        b"Current selected bytes"
                    );
                    fs::remove_file(stage.payload()).unwrap();
                    let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                    let done = wait(|| {
                        jobs.snapshot()
                            .jobs
                            .into_iter()
                            .find(|j| j.id == review && !j.phase.holds_application())
                    });
                    assert_eq!(done.phase, Phase::Finished, "{done:?}");
                } else {
                    assert_eq!(done.phase, Phase::Finished, "{done:?}");
                }
                close(jobs, root);
                let jobs = start(root);
                let row = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id)
                    .unwrap();
                assert!(row.transfer_actions.remaining, "{row:?}");
                let next =
                    receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == next && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{done:?}");
                assert_eq!(fs::metadata(&copied).unwrap().ino(), inode);
                receive(jobs.shutdown().unwrap()).unwrap();
            },
        );
    }
}
#[test]
fn returned_source_cleanup_refuses_replacement_inputs_and_outputs() {
    for replacement in ["source", "output"] {
        fixture(
            TransferKind::Move,
            "ready",
            "terminal-file",
            |root, output, id| {
                let jobs = start(root);
                let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                let done = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == review && !j.phase.holds_application())
                });
                assert_eq!(done.phase, Phase::Finished, "{done:?}");
                close(jobs, root);
                let source = root.join(leaf());
                let copied = output.join("target").join(leaf());
                let path = if replacement == "source" {
                    &source
                } else {
                    &copied
                };
                fs::rename(path, root.join("saved-original"))
                    .or_else(|_| fs::rename(path, output.join("saved-original")))
                    .unwrap();
                fs::write(path, b"Current selected bytes").unwrap();
                let inode = fs::metadata(path).unwrap().ino();
                for _ in 0..2 {
                    let jobs = start(root);
                    let row = jobs
                        .snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id)
                        .unwrap();
                    assert!(
                        !row.transfer_actions.remaining && !row.transfer_actions.retry,
                        "{replacement}: {row:?}"
                    );
                    assert_eq!(fs::read(&source).unwrap(), b"Current selected bytes");
                    assert_eq!(fs::read(&copied).unwrap(), b"Current selected bytes");
                    assert_eq!(fs::metadata(path).unwrap().ino(), inode);
                    close(jobs, root);
                }
            },
        );
    }
}
#[test]
#[ignore = "private review process killed after the exact returned-stage transaction"]
fn review_return_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_PUBLISHED_COPY_ROOT").unwrap());
    let jobs = start(&root);
    receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    std::thread::sleep(Duration::from_secs(25));
    panic!("returned-stage boundary was not reached");
}
#[test]
#[ignore = "private restarted service killed during original source cleanup"]
fn resume_leaf_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_PUBLISHED_COPY_ROOT").unwrap());
    let id = std::env::var("CANOPA_PUBLISHED_COPY_REQUEST")
        .unwrap()
        .parse()
        .unwrap();
    let jobs = start(&root);
    receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
    std::thread::sleep(Duration::from_secs(25));
    panic!("resumed cleanup boundary was not reached");
}
#[test]
#[ignore = "private service process killed after its actual Copy append"]
fn published_leaf_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_PUBLISHED_COPY_ROOT").unwrap());
    let output = PathBuf::from(std::env::var_os("CANOPA_PUBLISHED_COPY_OUTPUT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-published-copy-")
    );
    assert_eq!(
        output.parent(),
        Some(std::env::current_dir().unwrap().as_path())
    );
    assert!(
        output
            .file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b".canopa-published-copy-output-")
    );
    let kind = match std::env::var("CANOPA_PUBLISHED_COPY_KIND")
        .unwrap()
        .as_str()
    {
        "Copy" => TransferKind::Copy,
        "Move" => TransferKind::Move,
        _ => panic!("kind"),
    };
    let variant = std::env::var("CANOPA_PUBLISHED_COPY_VARIANT").unwrap_or_else(|_| "file".into());
    if variant.starts_with("terminal-") {
        let source = root.join(leaf());
        if variant == "terminal-link" {
            std::os::unix::fs::symlink(
                std::ffi::OsString::from_vec(b"missing-\xfe".to_vec()),
                &source,
            )
            .unwrap();
        } else {
            fs::write(&source, b"Current selected bytes").unwrap();
        }

        if variant == "terminal-alias" {
            fs::hard_link(&source, root.join("preserved-alias")).unwrap();
        }
        let destination = output.join("target");
        fs::create_dir(&destination).unwrap();
        let mut sources = Vec::new();
        if variant == "terminal-remainder" {
            fs::create_dir(root.join("a-folder")).unwrap();
            fs::write(root.join("a-folder/a-remainder"), b"Earlier source bytes").unwrap();
            sources.push(root.join("a-folder"));
        }
        sources.push(source);
        let jobs = start(&root);
        receive(
            jobs.submit(
                Request::Transfer(TransferRequest {
                    kind,
                    sources,
                    destination,
                }),
                None,
            )
            .unwrap(),
        )
        .unwrap();
        std::thread::sleep(Duration::from_secs(25));
        panic!("terminal boundary was not reached");
    }
    fs::create_dir(root.join("bundle")).unwrap();
    fs::write(root.join("bundle/a-done"), b"Completed prefix").unwrap();
    if variant == "symlink" {
        std::os::unix::fs::symlink(
            std::ffi::OsString::from_vec(b"missing-\xfe".to_vec()),
            root.join("bundle").join(leaf()),
        )
        .unwrap();
    } else {
        fs::write(root.join("bundle").join(leaf()), b"Current selected bytes").unwrap();
    }
    fs::write(root.join("z-later"), b"Later selected bytes").unwrap();
    fs::create_dir(output.join("bundle")).unwrap();
    fs::write(output.join("z-later"), b"Existing later bytes").unwrap();
    let jobs = start(&root);
    let id = receive(
        jobs.submit(
            Request::Transfer(TransferRequest {
                kind,
                sources: vec![root.join("bundle"), root.join("z-later")],
                destination: output,
            }),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    let first = choose(&jobs, id);
    if variant == "keep-both" {
        wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id)
                .and_then(|j| j.decision)
                .filter(|q| q.generation != first)
        });
        choose(&jobs, id);
    }
    std::thread::sleep(Duration::from_secs(25));
    panic!("actual copy boundary was not reached");
}
