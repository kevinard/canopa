use canopa_files::{
    operation_jobs::{OperationJobs, Phase, Request},
    operations::OperationRequest,
    transfers::{TransferKind, TransferRequest},
};
use std::{
    fs,
    os::unix::{fs::MetadataExt, process::ExitStatusExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "request execution fixture timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn start(root: &Path) -> OperationJobs {
    let jobs =
        OperationJobs::start(root.join("state/history"), root.join("state/volumes")).unwrap();
    wait(|| {
        let state = jobs.snapshot();
        assert!(state.error.is_none(), "{:?}", state.error);
        (!state.initializing).then_some(())
    });
    jobs
}

fn killed_mutation(kind: &str) -> (tempfile::TempDir, serde_json::Value) {
    let root = tempfile::Builder::new()
        .prefix("canopa-request-execution-")
        .tempdir()
        .unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::request_execution::request_execution_crash_helper",
        ])
        .env("CANOPA_REQUEST_EXECUTION_ROOT", root.path())
        .env("CANOPA_REQUEST_EXECUTION_KIND", kind)
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let marker =
        serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
    (root, marker)
}

fn transfer_source(root: &Path, kind: &str) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    if kind.ends_with("bytes") {
        root.join(std::ffi::OsString::from_vec(b"source-\xff".to_vec()))
    } else {
        root.join("source")
    }
}

#[test]
fn completed_copy_and_move_restore_exact_results_without_touching_newer_files() {
    for kind in [
        "copy",
        "move",
        "copy-directory",
        "move-directory",
        "copy-batch",
        "move-batch",
        "copy-bytes",
        "move-bytes",
    ] {
        let (root, marker) = killed_mutation(kind);
        let destination = root.path().join("destination");
        let source = transfer_source(root.path(), kind);
        let target = destination.join(source.file_name().unwrap());
        let completed_file = if kind.ends_with("directory") {
            target.join("nested/note")
        } else {
            target.clone()
        };
        fs::write(&completed_file, b"New work after process death").unwrap();
        let identity = fs::metadata(&completed_file).unwrap();
        if source.is_dir() {
            fs::write(source.join("new-source"), b"New source child").unwrap();
        } else {
            fs::write(&source, b"New original occupant").unwrap();
        }
        if kind.ends_with("batch") {
            fs::write(destination.join("second"), b"New second target").unwrap();
            fs::write(root.path().join("second"), b"New second source").unwrap();
        }
        let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
        database
            .execute_batch("DROP TRIGGER refuse_result")
            .unwrap();
        let jobs = start(root.path());
        let row = jobs
            .snapshot()
            .jobs
            .into_iter()
            .find(|row| Some(row.id) == marker["job"].as_u64())
            .unwrap();
        assert_eq!(row.phase, Phase::Finished, "{kind}: {:?}", row.error);
        let report = row.transfer.expect("restored exact transfer report");
        assert_eq!(report.operation, marker["operation"].as_u64());
        assert!(report.completed > 0);
        assert!(
            report.destinations.contains(&target) || report.destinations.contains(&completed_file)
        );
        if kind.ends_with("batch") {
            assert_eq!(report.completed, 2);
            assert_eq!(
                fs::read(destination.join("second")).unwrap(),
                b"New second target"
            );
            assert_eq!(
                fs::read(root.path().join("second")).unwrap(),
                b"New second source"
            );
        }
        assert!(report.failures.is_empty() && report.remaining.is_empty());
        assert_eq!(jobs.snapshot().undo.unwrap().id, report.operation.unwrap());
        assert_eq!(
            fs::read(&completed_file).unwrap(),
            b"New work after process death"
        );
        let after = fs::metadata(&completed_file).unwrap();
        assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
        if source.is_dir() {
            assert_eq!(
                fs::read(source.join("new-source")).unwrap(),
                b"New source child"
            );
        } else {
            assert_eq!(fs::read(&source).unwrap(), b"New original occupant");
        }
        let mut shutdown = jobs.shutdown().unwrap();
        wait(|| shutdown.try_recv().unwrap()).unwrap();
        let jobs = start(root.path());
        assert!(
            jobs.snapshot()
                .jobs
                .iter()
                .any(|row| Some(row.id) == marker["job"].as_u64() && row.phase == Phase::Finished)
        );
        let mut shutdown = jobs.shutdown().unwrap();
        wait(|| shutdown.try_recv().unwrap()).unwrap();
    }
}

#[test]
fn completed_cross_filesystem_move_restores_only_metadata_after_root_replacements() {
    let root = tempfile::Builder::new()
        .prefix("canopa-request-execution-")
        .tempdir()
        .unwrap();
    let source_root = tempfile::Builder::new()
        .prefix(".canopa-transfer-source-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    if fs::metadata(root.path()).unwrap().dev() == fs::metadata(source_root.path()).unwrap().dev() {
        eprintln!(
            "cross-filesystem crash check needs distinct workspace and temporary filesystems"
        );
        return;
    }
    let source = source_root.path().join("source");
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::request_execution::request_execution_crash_helper",
        ])
        .env("CANOPA_REQUEST_EXECUTION_ROOT", root.path())
        .env("CANOPA_REQUEST_EXECUTION_KIND", "move-directory")
        .env("CANOPA_REQUEST_EXECUTION_SOURCE", &source)
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(!source.exists());
    fs::write(&source, b"Replacement source entry").unwrap();
    let destination = root.path().join("destination");
    let archived = root.path().join("completed destination");
    fs::rename(&destination, &archived).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(
        destination.join("sentinel"),
        b"Replacement destination root",
    )
    .unwrap();
    let completed = archived.join("source/nested/note");
    fs::write(&completed, b"Newer completed bytes").unwrap();
    let identity = fs::metadata(&completed).unwrap();
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database
        .execute_batch("DROP TRIGGER refuse_result")
        .unwrap();
    let jobs = start(root.path());
    let row = jobs
        .snapshot()
        .jobs
        .into_iter()
        .find(|row| Some(row.id) == marker["job"].as_u64())
        .unwrap();
    assert_eq!(row.phase, Phase::Finished);
    assert_eq!(
        row.transfer.unwrap().operation,
        marker["operation"].as_u64()
    );
    assert_eq!(fs::read(&source).unwrap(), b"Replacement source entry");
    assert_eq!(fs::read(&completed).unwrap(), b"Newer completed bytes");
    let after = fs::metadata(&completed).unwrap();
    assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
    assert_eq!(
        fs::read(destination.join("sentinel")).unwrap(),
        b"Replacement destination root"
    );
    assert_eq!(fs::read_dir(destination).unwrap().count(), 1);
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn stopped_copy_and_cross_filesystem_move_restore_exact_reports_after_root_replacements() {
    use sha2::{Digest, Sha256};
    for kind in ["partial-copy", "partial-move"] {
        let root = tempfile::Builder::new()
            .prefix("canopa-request-execution-")
            .tempdir()
            .unwrap();
        let source_root = tempfile::Builder::new()
            .prefix(".canopa-transfer-source-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        if kind == "partial-move"
            && fs::metadata(root.path()).unwrap().dev()
                == fs::metadata(source_root.path()).unwrap().dev()
        {
            eprintln!("cross-filesystem stopped Move requires distinct private filesystems");
            continue;
        }
        let source = if kind == "partial-move" {
            source_root.path().join("source")
        } else {
            root.path().join("source")
        };
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--ignored",
                "--exact",
                "cases::request_execution::request_execution_crash_helper",
            ])
            .env("CANOPA_REQUEST_EXECUTION_ROOT", root.path())
            .env("CANOPA_REQUEST_EXECUTION_KIND", kind);
        if kind == "partial-move" {
            child.env("CANOPA_REQUEST_EXECUTION_SOURCE", &source);
        }
        assert_eq!(child.status().unwrap().signal(), Some(9));
        let marker: serde_json::Value =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        let id = marker["job"].as_u64().unwrap();
        let db = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
        let receipt: String = db
            .query_row(
                "SELECT body FROM request_transfer_stops WHERE request=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        let exact: serde_json::Value = serde_json::from_str(&receipt).unwrap();
        assert!(exact["retry"].is_object());
        let destination = root.path().join("destination");
        let archived = root.path().join("original destination");
        fs::rename(&destination, &archived).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("sentinel"), b"Replacement destination").unwrap();
        let target = archived.join("source/a-completed");
        fs::write(&target, b"Newer completed bytes").unwrap();
        let identity = fs::metadata(&target).unwrap();
        let parked_source = source.parent().unwrap().join("original source");
        fs::rename(&source, &parked_source).unwrap();
        fs::create_dir(&source).unwrap();
        fs::write(source.join("sentinel"), b"Replacement source").unwrap();
        db.execute_batch("DROP TRIGGER refuse_result").unwrap();
        for attempt in 0..2 {
            let jobs = start(root.path());
            let row = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|row| row.id == id)
                .unwrap();
            assert_eq!(row.phase, Phase::Stopped, "{kind}: {:?}", row.error);
            let report = row.transfer.unwrap();
            assert_eq!(
                serde_json::to_value(report.as_ref()).unwrap(),
                exact["report"]
            );
            assert!(
                !row.transfer_actions.retry
                    && !row.transfer_actions.remaining
                    && row.transfer_actions.abandon
            );
            assert_eq!(
                jobs.snapshot().undo.unwrap().id,
                marker["operation"].as_u64().unwrap()
            );
            let (revision, body): (u64, Vec<u8>) = db
                .query_row(
                    "SELECT revision,body FROM requests WHERE id=?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            let saved: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(saved["TransferPending"]["outcome"], exact);
            let (duplicate, saved_revision, saved_sha): (Option<String>,u64,Vec<u8>) = db.query_row("SELECT body,saved_revision,saved_body_sha256 FROM request_transfer_stops WHERE request=?1", [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
            assert!(duplicate.is_none());
            assert_eq!(saved_revision, revision);
            assert_eq!(saved_sha, Sha256::digest(body).as_slice());
            assert_eq!(fs::read(&target).unwrap(), b"Newer completed bytes");
            let after = fs::metadata(&target).unwrap();
            assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
            assert_eq!(
                fs::read(source.join("sentinel")).unwrap(),
                b"Replacement source"
            );
            assert_eq!(
                fs::read(destination.join("sentinel")).unwrap(),
                b"Replacement destination"
            );
            assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
            assert_eq!(fs::read_dir(&source).unwrap().count(), 1);
            assert!(!archived.join("source/b-failed").exists());
            if attempt == 0 {
                drop(jobs);
                let engine = wait(|| {
                    match canopa_files::operations::OperationEngine::open(
                        root.path().join("state/history"),
                    ) {
                        Ok(engine) => Some(engine),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
                        Err(error) => panic!("history teardown failed: {error}"),
                    }
                });
                drop(engine);
            } else {
                let mut abandon = jobs.abandon_transfer(id).unwrap();
                wait(|| abandon.try_recv().unwrap()).unwrap();
                let mut shutdown = jobs.shutdown().unwrap();
                wait(|| shutdown.try_recv().unwrap()).unwrap();
            }
        }
    }
}

#[test]
fn schema29_stopped_receipts_survive_the_checkpoint_upgrade_without_replaying_files() {
    use serde_json::value::RawValue;
    use sha2::{Digest, Sha256};
    #[derive(serde::Serialize)]
    struct LegacyStopped {
        report: canopa_files::operation_jobs::TransferReport,
        retry: Option<Box<RawValue>>,
        remaining: Option<Box<RawValue>>,
        interrupted: Option<String>,
        rejected_before_execution: bool,
    }
    fn legacy_plan(value: &serde_json::Value) -> Option<Box<RawValue>> {
        if value.is_null() {
            return None;
        }
        let mut value = value.clone();
        for directory in value["directories"].as_array_mut().unwrap() {
            directory["finish"]
                .as_object_mut()
                .unwrap()
                .remove("original");
        }
        for pending in value["pending"].as_array_mut().unwrap() {
            if let Some(finish) = pending["finish"].as_object_mut() {
                finish.remove("original");
            }
        }
        value["version"] = 2.into();
        let plan: canopa_files::transfers::TransferContinuation =
            serde_json::from_value(value).unwrap();
        let current = serde_json::to_string(&plan).unwrap();
        assert!(current.starts_with("{\"version\":3,"));
        // All other fields retain the old codec's canonical order. Only the
        // new optional traversal facts and format number differ from v2.
        Some(RawValue::from_string(current.replacen("\"version\":3", "\"version\":2", 1)).unwrap())
    }
    let (root, marker) = killed_mutation("partial-copy");
    let id = marker["job"].as_u64().unwrap();
    let db = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    let body: String = db
        .query_row(
            "SELECT body FROM request_transfer_stops WHERE request=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    let actual: serde_json::Value = serde_json::from_str(&body).unwrap();
    let legacy = LegacyStopped {
        report: serde_json::from_value(actual["report"].clone()).unwrap(),
        retry: legacy_plan(&actual["retry"]),
        remaining: legacy_plan(&actual["remaining"]),
        interrupted: None,
        rejected_before_execution: false,
    };
    let body = serde_json::to_string(&legacy).unwrap();
    let original_sha = Sha256::digest(body.as_bytes()).to_vec();
    db.execute("UPDATE request_transfer_stops SET body=?2,body_sha256=?3,report_sha256=?4,retry_sha256=?5,remaining_sha256=?6 WHERE request=?1",
        rusqlite::params![id,body,&original_sha,Sha256::digest(serde_json::to_vec(&legacy.report).unwrap()).to_vec(),
            legacy.retry.as_ref().map(|raw|Sha256::digest(raw.get().as_bytes()).to_vec()),
            legacy.remaining.as_ref().map(|raw|Sha256::digest(raw.get().as_bytes()).to_vec())]).unwrap();
    let accepted: Vec<u8> = db
        .query_row("SELECT body FROM requests WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    let accepted =
        String::from_utf8(accepted)
            .unwrap()
            .replacen("\"version\":3", "\"version\":2", 1);
    db.execute(
        "UPDATE requests SET body=?2 WHERE id=?1",
        rusqlite::params![id, accepted.as_bytes()],
    )
    .unwrap();
    db.execute(
        "UPDATE request_executions SET payload_sha256=?2 WHERE request=?1",
        rusqlite::params![id, Sha256::digest(accepted.as_bytes()).to_vec()],
    )
    .unwrap();
    let intent: String = db
        .query_row(
            "SELECT intent FROM request_transfer_ends WHERE request=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    let mut intent: serde_json::Value = serde_json::from_str(&intent).unwrap();
    intent.as_object_mut().unwrap().remove("history_original");
    let intent = serde_json::to_string(&intent).unwrap();
    db.execute(
        "UPDATE request_transfer_ends SET intent=?2,intent_sha256=?3 WHERE request=?1",
        rusqlite::params![id, intent, Sha256::digest(intent.as_bytes()).to_vec()],
    )
    .unwrap();
    db.execute_batch("PRAGMA user_version=29").unwrap();
    let target = root.path().join("destination/source/a-completed");
    fs::write(&target, b"Newer bytes after legacy process death").unwrap();
    let identity = fs::metadata(&target).unwrap();
    let failed = root.path().join("source/b-failed");
    fs::remove_file(&failed).unwrap();
    fs::write(&failed, b"Repaired source must not replay").unwrap();
    for attempt in 0..2 {
        let jobs = start(root.path());
        if attempt == 0 {
            let initial = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|row| row.id == id)
                .unwrap();
            assert_eq!(initial.phase, Phase::SaveFailed, "{:?}", initial.error);
            assert!(initial.transfer.is_some() && jobs.snapshot().holds_application());
            let retained: String = db
                .query_row(
                    "SELECT body FROM request_transfer_stops WHERE request=?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                retained, body,
                "a refused upgrade must retain the exact old end"
            );
            let old_hash: Vec<u8> = db
                .query_row(
                    "SELECT retry_sha256 FROM request_transfer_stops WHERE request=?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                old_hash,
                Sha256::digest(legacy.retry.as_ref().unwrap().get().as_bytes()).to_vec()
            );
            db.execute_batch("DROP TRIGGER refuse_result").unwrap();
            jobs.retry_save(id).unwrap();
            wait(|| {
                jobs.snapshot()
                    .jobs
                    .iter()
                    .any(|row| row.id == id && row.phase == Phase::Stopped)
                    .then_some(())
            });
        }
        let row = jobs
            .snapshot()
            .jobs
            .into_iter()
            .find(|row| row.id == id)
            .unwrap();
        assert_eq!(row.phase, Phase::Stopped, "{:?}", row.error);
        assert_eq!(
            serde_json::to_value(row.transfer.unwrap().as_ref()).unwrap(),
            serde_json::to_value(&legacy.report).unwrap()
        );
        assert!(!row.transfer_actions.retry && !row.transfer_actions.remaining);
        assert_eq!(
            jobs.snapshot().undo.unwrap().id,
            marker["operation"].as_u64().unwrap()
        );
        assert_eq!(
            fs::read(&target).unwrap(),
            b"Newer bytes after legacy process death"
        );
        let after = fs::metadata(&target).unwrap();
        assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
        assert!(!root.path().join("destination/source/b-failed").exists());
        assert_eq!(
            fs::read(&failed).unwrap(),
            b"Repaired source must not replay"
        );
        let saved: Vec<u8> = db
            .query_row("SELECT body FROM requests WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        let plan = &saved["TransferPending"]["outcome"]["retry"];
        assert_eq!(plan["version"], 3);
        assert!(plan["directories"][0]["finish"].get("original").is_none());
        let typed: canopa_files::transfers::TransferContinuation =
            serde_json::from_value(plan.clone()).unwrap();
        let (original, current): (Vec<u8>, Vec<u8>) = db
            .query_row(
                "SELECT body_sha256,retry_sha256 FROM request_transfer_stops WHERE request=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(original, original_sha);
        assert_eq!(
            current,
            Sha256::digest(serde_json::to_vec(&typed).unwrap()).to_vec()
        );
        let mut shutdown = jobs.shutdown().unwrap();
        wait(|| shutdown.try_recv().unwrap()).unwrap();
    }
}

#[test]
fn stale_changed_unended_and_legacy_stopped_receipts_remain_reviewable() {
    for fault in [
        "missing", "body", "report", "retry", "phase", "intent", "request", "revision", "legacy",
    ] {
        let (root, marker) = killed_mutation("partial-copy");
        let id = marker["job"].as_u64().unwrap();
        let target = root.path().join("destination/source/a-completed");
        fs::write(&target, b"Newer bytes after crash").unwrap();
        let identity = fs::metadata(&target).unwrap();
        let db = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
        db.execute_batch("DROP TRIGGER refuse_result").unwrap();
        db.execute_batch(match fault {
            "missing" => "DELETE FROM request_transfer_stops",
            "body" => "UPDATE request_transfer_stops SET body=body||' '",
            "report" => "UPDATE request_transfer_stops SET report_sha256=zeroblob(32)",
            "retry" => "UPDATE request_transfer_stops SET retry_sha256=zeroblob(32)",
            "phase" => {
                "UPDATE request_transfer_stops SET phase=CASE phase WHEN 2 THEN 3 ELSE 2 END"
            }
            "intent" => "UPDATE request_transfer_ends SET intent=intent||' '",
            "request" => "UPDATE requests SET body=CAST(CAST(body AS TEXT)||' ' AS BLOB)",
            "revision" => "UPDATE requests SET revision=revision+1",
            "legacy" => "DROP TABLE request_transfer_stops; PRAGMA user_version=28",
            _ => unreachable!(),
        })
        .unwrap();
        let before: (u64, Vec<u8>) = db
            .query_row(
                "SELECT revision,body FROM requests WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let jobs = start(root.path());
        let row = jobs
            .snapshot()
            .jobs
            .into_iter()
            .find(|row| row.id == id)
            .unwrap();
        assert_eq!(row.phase, Phase::Interrupted, "{fault}: {row:?}");
        assert!(
            row.transfer.is_none()
                && !row.transfer_actions.retry
                && !row.transfer_actions.remaining
        );
        assert_eq!(
            jobs.snapshot().undo.unwrap().id,
            marker["operation"].as_u64().unwrap()
        );
        assert_eq!(
            before,
            db.query_row(
                "SELECT revision,body FROM requests WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?))
            )
            .unwrap()
        );
        assert_eq!(fs::read(&target).unwrap(), b"Newer bytes after crash");
        let after = fs::metadata(&target).unwrap();
        assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
        let mut shutdown = jobs.shutdown().unwrap();
        wait(|| shutdown.try_recv().unwrap()).unwrap();
    }
}

#[test]
fn unsaved_stopped_transfer_receipt_holds_activity_and_retries_only_metadata() {
    use canopa_files::{local_activity::LocalActivity, storage::Storage};
    let root = tempfile::tempdir().unwrap();
    let activity = LocalActivity::default();
    let jobs = OperationJobs::start_with_activity(
        root.path().join("state/history"),
        Storage::load(root.path().join("state/volumes")).unwrap(),
        activity.clone(),
    )
    .unwrap();
    wait(|| (!jobs.snapshot().initializing).then_some(()));
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("a-completed"), b"Original bytes").unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        source.join("b-failed"),
        rustix::fs::Mode::RUSR,
    )
    .unwrap();
    let db = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    db.execute_batch("CREATE TRIGGER refuse_stop BEFORE INSERT ON request_transfer_stops BEGIN SELECT RAISE(ABORT,'private stopped-end refusal'); END").unwrap();
    let mut admission = jobs
        .submit(
            Request::Transfer(TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: destination.clone(),
            }),
            None,
        )
        .unwrap();
    let id = wait(|| admission.try_recv().unwrap()).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .iter()
            .any(|row| row.id == id && row.phase == Phase::SaveFailed)
            .then_some(())
    });
    let original = jobs
        .snapshot()
        .jobs
        .into_iter()
        .find(|row| row.id == id)
        .unwrap()
        .transfer
        .unwrap();
    assert!(original.completed > 0 && original.failures.len() == 1);
    let target = destination.join("source/a-completed");
    fs::write(&target, b"Newer completed bytes").unwrap();
    let identity = fs::metadata(&target).unwrap();
    fs::remove_file(source.join("b-failed")).unwrap();
    fs::write(source.join("b-failed"), b"Repaired failed item").unwrap();
    assert!(activity.holds_application() && activity.try_remove().is_err());
    let mut shutdown = jobs.shutdown().unwrap();
    assert!(wait(|| shutdown.try_recv().unwrap()).is_err());
    let later = root.path().join("later");
    let mut admission = jobs
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: later.clone(),
            }),
            None,
        )
        .unwrap();
    let next = wait(|| admission.try_recv().unwrap()).unwrap();
    jobs.retry_save(id).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .iter()
            .any(|row| {
                row.id == id
                    && row
                        .error
                        .as_deref()
                        .is_some_and(|e| e.contains("Could not save"))
            })
            .then_some(())
    });
    assert!(!later.exists());
    db.execute_batch("DROP TRIGGER refuse_stop").unwrap();
    jobs.retry_save(id).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .iter()
            .any(|row| row.id == next && row.phase == Phase::Finished)
            .then_some(())
    });
    let row = jobs
        .snapshot()
        .jobs
        .into_iter()
        .find(|row| row.id == id)
        .unwrap();
    assert_eq!(row.phase, Phase::Stopped);
    assert!(row.transfer_actions.retry && row.transfer_actions.abandon);
    assert_eq!(
        serde_json::to_value(row.transfer.unwrap().as_ref()).unwrap(),
        serde_json::to_value(original.as_ref()).unwrap()
    );
    assert_eq!(fs::read(&target).unwrap(), b"Newer completed bytes");
    let after = fs::metadata(&target).unwrap();
    assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
    assert!(!destination.join("source/b-failed").exists());
    assert_eq!(
        fs::read(source.join("b-failed")).unwrap(),
        b"Repaired failed item"
    );
    let mut abandon = jobs.abandon_transfer(id).unwrap();
    wait(|| abandon.try_recv().unwrap()).unwrap();
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
    wait(|| (!activity.holds_application()).then_some(()));
    drop(activity.try_remove().unwrap());
}

#[test]
fn unsaved_transfer_end_holds_activity_and_retries_only_metadata() {
    use canopa_files::{local_activity::LocalActivity, storage::Storage};
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let root = tempfile::tempdir().unwrap();
        let activity = LocalActivity::default();
        let jobs = OperationJobs::start_with_activity(
            root.path().join("state/history"),
            Storage::load(root.path().join("state/volumes")).unwrap(),
            activity.clone(),
        )
        .unwrap();
        wait(|| (!jobs.snapshot().initializing).then_some(()));
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"Original transfer").unwrap();
        fs::create_dir(&destination).unwrap();
        let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
        database.execute_batch("CREATE TRIGGER refuse_transfer_end BEFORE UPDATE OF result ON request_transfer_ends BEGIN SELECT RAISE(ABORT,'private transfer end-save refusal'); END").unwrap();
        let mut admission = jobs
            .submit(
                Request::Transfer(TransferRequest {
                    kind,
                    sources: vec![source.clone()],
                    destination: destination.clone(),
                }),
                None,
            )
            .unwrap();
        let id = wait(|| admission.try_recv().unwrap()).unwrap();
        wait(|| {
            jobs.snapshot()
                .jobs
                .iter()
                .any(|row| row.id == id && row.phase == Phase::SaveFailed)
                .then_some(())
        });
        let target = destination.join("source");
        fs::write(&target, b"Newer completed bytes").unwrap();
        let identity = fs::metadata(&target).unwrap();
        fs::write(&source, b"New source occupant").unwrap();
        assert!(activity.holds_application() && activity.try_remove().is_err());
        let mut shutdown = jobs.shutdown().unwrap();
        assert!(wait(|| shutdown.try_recv().unwrap()).is_err());
        let later = root.path().join("later");
        let mut admission = jobs
            .submit(
                Request::Mutation(OperationRequest::CreateFolder {
                    path: later.clone(),
                }),
                None,
            )
            .unwrap();
        let next = wait(|| admission.try_recv().unwrap()).unwrap();
        jobs.retry_save(id).unwrap();
        wait(|| {
            jobs.snapshot()
                .jobs
                .iter()
                .find(|row| row.id == id)
                .filter(|row| {
                    row.error
                        .as_deref()
                        .is_some_and(|error| error.contains("Could not save"))
                })
                .map(|_| ())
        });
        assert!(!later.exists());
        database
            .execute_batch("DROP TRIGGER refuse_transfer_end")
            .unwrap();
        jobs.retry_save(id).unwrap();
        wait(|| {
            jobs.snapshot()
                .jobs
                .iter()
                .any(|row| row.id == next && row.phase == Phase::Finished)
                .then_some(())
        });
        assert_eq!(fs::read(&target).unwrap(), b"Newer completed bytes");
        assert_eq!(fs::read(&source).unwrap(), b"New source occupant");
        let after = fs::metadata(&target).unwrap();
        assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
        assert!(
            jobs.snapshot()
                .jobs
                .iter()
                .any(|row| row.id == id && row.phase == Phase::Finished)
        );
        let mut shutdown = jobs.shutdown().unwrap();
        wait(|| shutdown.try_recv().unwrap()).unwrap();
        wait(|| (!activity.holds_application()).then_some(()));
        drop(activity.try_remove().unwrap());
    }
}

#[test]
fn startup_transfer_result_save_failure_retains_the_exact_result_for_retry() {
    for kind in ["copy", "move"] {
        let (root, marker) = killed_mutation(kind);
        // Leave the private result-save refusal installed across startup.
        let jobs = start(root.path());
        let id = marker["job"].as_u64().unwrap();
        let row = jobs
            .snapshot()
            .jobs
            .into_iter()
            .find(|row| row.id == id)
            .unwrap();
        assert_eq!(row.phase, Phase::SaveFailed);
        assert!(row.transfer.is_some());
        let mut shutdown = jobs.shutdown().unwrap();
        assert!(wait(|| shutdown.try_recv().unwrap()).is_err());
        let target = root.path().join("destination/source");
        fs::write(&target, b"Newer target after another start").unwrap();
        fs::write(root.path().join("source"), b"New original occupant").unwrap();
        let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
        database
            .execute_batch("DROP TRIGGER refuse_result")
            .unwrap();
        jobs.retry_save(id).unwrap();
        wait(|| {
            jobs.snapshot()
                .jobs
                .iter()
                .any(|row| row.id == id && row.phase == Phase::Finished)
                .then_some(())
        });
        assert_eq!(
            fs::read(&target).unwrap(),
            b"Newer target after another start"
        );
        assert_eq!(
            fs::read(root.path().join("source")).unwrap(),
            b"New original occupant"
        );
        assert_eq!(
            jobs.snapshot().undo.unwrap().id,
            marker["operation"].as_u64().unwrap()
        );
        let mut shutdown = jobs.shutdown().unwrap();
        wait(|| shutdown.try_recv().unwrap()).unwrap();
    }
}

#[test]
fn incomplete_stale_corrupt_and_legacy_transfer_ends_never_grant_replay() {
    for fault in [
        "unended", "result", "intent", "payload", "revision", "legacy",
    ] {
        let (root, marker) = killed_mutation("move");
        let id = marker["job"].as_u64().unwrap();
        let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
        database
            .execute_batch("DROP TRIGGER refuse_result")
            .unwrap();
        database
            .execute_batch(match fault {
                "unended" => "UPDATE request_transfer_ends SET result=NULL,result_sha256=NULL",
                "result" => "UPDATE request_transfer_ends SET result=result||' '",
                "intent" => "UPDATE request_transfer_ends SET intent=intent||' '",
                "payload" => "UPDATE requests SET body=CAST(CAST(body AS TEXT)||' ' AS BLOB)",
                "revision" => "UPDATE requests SET revision=revision+1",
                "legacy" => "DROP TABLE request_transfer_ends; PRAGMA user_version=26",
                _ => unreachable!(),
            })
            .unwrap();
        let target = root.path().join("destination/source");
        fs::write(&target, b"Newer completed bytes").unwrap();
        fs::write(root.path().join("source"), b"New source occupant").unwrap();
        let before: (u64, Vec<u8>) = database
            .query_row(
                "SELECT revision,body FROM requests WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let jobs = start(root.path());
        let row = jobs
            .snapshot()
            .jobs
            .into_iter()
            .find(|row| row.id == id)
            .unwrap();
        assert_eq!(row.phase, Phase::Interrupted, "{fault}: {row:?}");
        assert!(
            !row.transfer_actions.retry
                && !row.transfer_actions.remaining
                && !row.transfer_actions.abandon
        );
        if matches!(fault, "result" | "intent") {
            assert!(row.error.unwrap().contains("could not be verified"));
        }
        assert_eq!(fs::read(&target).unwrap(), b"Newer completed bytes");
        assert_eq!(
            fs::read(root.path().join("source")).unwrap(),
            b"New source occupant"
        );
        assert_eq!(
            before,
            database
                .query_row(
                    "SELECT revision,body FROM requests WHERE id=?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?))
                )
                .unwrap()
        );
        let mut shutdown = jobs.shutdown().unwrap();
        wait(|| shutdown.try_recv().unwrap()).unwrap();
    }
}

#[test]
fn ended_transfer_survives_history_expiry_without_recreating_undo_or_files() {
    use canopa_files::{Cancellation, operations::OperationEngine};
    let (root, marker) = killed_mutation("copy");
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database
        .execute_batch("DROP TRIGGER refuse_result")
        .unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    for index in 0..100 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("later-{index}")),
                },
                &Cancellation::default(),
            )
            .unwrap();
    }
    let latest = engine.latest_undo_id().unwrap();
    assert_ne!(Some(latest), marker["operation"].as_u64());
    drop(engine);
    fs::remove_file(root.path().join("destination/source")).unwrap();
    let jobs = start(root.path());
    let row = jobs
        .snapshot()
        .jobs
        .into_iter()
        .find(|row| Some(row.id) == marker["job"].as_u64())
        .unwrap();
    assert_eq!(row.phase, Phase::Finished);
    assert_eq!(
        row.transfer.unwrap().operation,
        marker["operation"].as_u64()
    );
    assert!(!root.path().join("destination/source").exists());
    assert_eq!(jobs.snapshot().undo.unwrap().id, latest);
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn completed_folder_restores_its_result_and_preserves_post_crash_children() {
    let (root, marker) = killed_mutation("create-folder");
    let folder = root.path().join("created");
    let identity = fs::metadata(&folder).unwrap();
    fs::write(folder.join("newer.txt"), b"New child after process death").unwrap();
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database
        .execute_batch("DROP TRIGGER refuse_result")
        .unwrap();
    let jobs = start(root.path());
    let row = jobs
        .snapshot()
        .jobs
        .into_iter()
        .find(|row| Some(row.id) == marker["job"].as_u64())
        .unwrap();
    assert_eq!(row.phase, Phase::Finished);
    assert_eq!(
        jobs.snapshot().undo.unwrap().id,
        marker["operation"].as_u64().unwrap()
    );
    let after = fs::metadata(&folder).unwrap();
    assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
    assert_eq!(
        fs::read(folder.join("newer.txt")).unwrap(),
        b"New child after process death"
    );
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn an_unsaved_live_end_keeps_activity_and_blocks_dispatch_until_metadata_only_retry() {
    use canopa_files::{local_activity::LocalActivity, storage::Storage};
    let root = tempfile::tempdir().unwrap();
    let activity = LocalActivity::default();
    let storage = Storage::load(root.path().join("state/volumes")).unwrap();
    let jobs = OperationJobs::start_with_activity(
        root.path().join("state/history"),
        storage,
        activity.clone(),
    )
    .unwrap();
    wait(|| (!jobs.snapshot().initializing).then_some(()));
    assert!(jobs.snapshot().error.is_none());
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database.execute_batch("CREATE TRIGGER refuse_end BEFORE UPDATE OF ended ON request_mutation_ends WHEN NEW.ended=1 BEGIN SELECT RAISE(ABORT,'private end-save refusal'); END").unwrap();
    let folder = root.path().join("created");
    let mut admission = jobs
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: folder.clone(),
            }),
            None,
        )
        .unwrap();
    let id = wait(|| admission.try_recv().unwrap()).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .iter()
            .any(|row| row.id == id && row.phase == Phase::SaveFailed)
            .then_some(())
    });
    let identity = fs::metadata(&folder).unwrap();
    fs::write(folder.join("newer"), b"Do not replay this Create Folder").unwrap();
    assert_eq!(
        database
            .query_row(
                "SELECT ended FROM request_mutation_ends WHERE request=?1",
                [id],
                |row| row.get::<_, u8>(0)
            )
            .unwrap(),
        0
    );
    assert!(activity.holds_application() && activity.try_remove().is_err());
    let mut shutdown = jobs.shutdown().unwrap();
    assert!(wait(|| shutdown.try_recv().unwrap()).is_err());
    let later = root.path().join("later");
    let mut admission = jobs
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: later.clone(),
            }),
            None,
        )
        .unwrap();
    let next = wait(|| admission.try_recv().unwrap()).unwrap();
    assert!(!later.exists());
    jobs.retry_save(id).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .iter()
            .find(|row| row.id == id)
            .filter(|row| {
                row.error
                    .as_deref()
                    .is_some_and(|error| error.contains("Could not save"))
            })
            .map(|_| ())
    });
    assert!(!later.exists());
    database.execute_batch("DROP TRIGGER refuse_end").unwrap();
    jobs.retry_save(id).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .iter()
            .any(|row| row.id == next && row.phase == Phase::Finished)
            .then_some(())
    });
    let after = fs::metadata(&folder).unwrap();
    assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
    assert_eq!(
        fs::read(folder.join("newer")).unwrap(),
        b"Do not replay this Create Folder"
    );
    assert_eq!(
        database
            .query_row(
                "SELECT ended FROM request_mutation_ends WHERE request=?1",
                [id],
                |row| row.get::<_, u8>(0)
            )
            .unwrap(),
        1
    );
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
    wait(|| (!activity.holds_application()).then_some(()));
    drop(activity.try_remove().unwrap());
}

#[test]
fn unended_changed_and_corrupt_receipts_remain_reviewable_without_touching_files() {
    for fault in ["unended", "intent", "payload", "revision"] {
        let (root, marker) = killed_mutation("rename");
        let id = marker["job"].as_u64().unwrap();
        let target = root.path().join("renamed");
        fs::write(&target, b"Newer target contents").unwrap();
        fs::write(root.path().join("source"), b"New original occupant").unwrap();
        let identity = fs::metadata(&target).unwrap();
        let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
        database
            .execute_batch("DROP TRIGGER refuse_result")
            .unwrap();
        database
            .execute_batch(match fault {
                "unended" => "UPDATE request_mutation_ends SET ended=0",
                "intent" => "UPDATE request_mutation_ends SET intent=intent||' '",
                "payload" => "UPDATE requests SET body=CAST(CAST(body AS TEXT)||' ' AS BLOB)",
                "revision" => "UPDATE requests SET revision=revision+1",
                _ => unreachable!(),
            })
            .unwrap();
        let before: (u64, Vec<u8>) = database
            .query_row(
                "SELECT revision,body FROM requests WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let jobs = start(root.path());
        let row = jobs
            .snapshot()
            .jobs
            .into_iter()
            .find(|row| row.id == id)
            .unwrap();
        assert_eq!(row.phase, Phase::Interrupted, "{fault}: {row:?}");
        assert!(row.dismissible);
        if fault == "intent" {
            assert!(row.error.unwrap().contains("could not be verified"));
        }
        assert_eq!(fs::read(&target).unwrap(), b"Newer target contents");
        assert_eq!(
            fs::read(root.path().join("source")).unwrap(),
            b"New original occupant"
        );
        let after = fs::metadata(&target).unwrap();
        assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
        assert_eq!(
            before,
            database
                .query_row(
                    "SELECT revision,body FROM requests WHERE id=?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?))
                )
                .unwrap()
        );
        assert_eq!(
            database
                .query_row(
                    "SELECT count(*) FROM request_mutation_ends WHERE request=?1",
                    [id],
                    |row| row.get::<_, u64>(0)
                )
                .unwrap(),
            1
        );
        let mut shutdown = jobs.shutdown().unwrap();
        wait(|| shutdown.try_recv().unwrap()).unwrap();
    }
}

#[test]
fn ended_mutation_survives_normal_undo_history_expiry() {
    use canopa_files::{Cancellation, operations::OperationEngine};
    let (root, marker) = killed_mutation("rename");
    let operation = marker["operation"].as_u64().unwrap();
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database
        .execute_batch("DROP TRIGGER refuse_result")
        .unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    for index in 0..100 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("later-{index}")),
                },
                &Cancellation::default(),
            )
            .unwrap();
    }
    drop(engine);
    assert_eq!(
        database
            .query_row(
                "SELECT count(*) FROM operations WHERE id=?1",
                [operation],
                |row| row.get::<_, u64>(0)
            )
            .unwrap(),
        0
    );
    let jobs = start(root.path());
    let row = jobs
        .snapshot()
        .jobs
        .into_iter()
        .find(|row| Some(row.id) == marker["job"].as_u64())
        .unwrap();
    assert_eq!(row.phase, Phase::Finished);
    assert_eq!(
        fs::read(root.path().join("renamed")).unwrap(),
        b"Original source"
    );
    assert!(!root.path().join("source").exists());
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn failed_confirmed_step_save_does_not_create_a_completion_receipt() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::write(&source, b"Original").unwrap();
    let jobs = start(root.path());
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database.execute_batch("CREATE TRIGGER refuse_step BEFORE INSERT ON changes BEGIN SELECT RAISE(ABORT,'private confirmed-step refusal'); END").unwrap();
    let mut admission = jobs
        .submit(
            Request::Mutation(OperationRequest::Rename {
                source,
                destination: root.path().join("renamed"),
            }),
            None,
        )
        .unwrap();
    let id = wait(|| admission.try_recv().unwrap()).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .iter()
            .any(|row| row.id == id && row.phase == Phase::Failed)
            .then_some(())
    });
    assert_eq!(
        database
            .query_row(
                "SELECT ended FROM request_mutation_ends WHERE request=?1",
                [id],
                |row| row.get::<_, u8>(0)
            )
            .unwrap(),
        0
    );
    database.execute_batch("DROP TRIGGER refuse_step").unwrap();
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
    let jobs = start(root.path());
    assert_eq!(
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|row| row.id == id)
            .unwrap()
            .phase,
        Phase::Failed
    );
    assert_eq!(
        database
            .query_row(
                "SELECT ended FROM request_mutation_ends WHERE request=?1",
                [id],
                |row| row.get::<_, u8>(0)
            )
            .unwrap(),
        0
    );
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn a_confirmed_rename_reopens_as_finished_without_replaying_or_overwriting_new_work() {
    let root = tempfile::Builder::new()
        .prefix("canopa-request-execution-")
        .tempdir()
        .unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::request_execution::request_execution_crash_helper",
        ])
        .env("CANOPA_REQUEST_EXECUTION_ROOT", root.path())
        .env("CANOPA_REQUEST_EXECUTION_KIND", "rename")
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
    let id = marker["job"].as_u64().unwrap();
    let operation = marker["operation"].as_u64().unwrap();
    let target = root.path().join("renamed");
    let source = root.path().join("source");
    let original = fs::metadata(&target).unwrap();
    fs::write(&target, b"Edited after completed Rename and process death").unwrap();
    fs::write(&source, b"Newer original occupant").unwrap();
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database
        .execute_batch("DROP TRIGGER refuse_result")
        .unwrap();
    drop(database);
    let jobs = start(root.path());
    let row = jobs
        .snapshot()
        .jobs
        .into_iter()
        .find(|row| row.id == id)
        .unwrap();
    let after = fs::metadata(&target).unwrap();
    assert_eq!((original.dev(), original.ino()), (after.dev(), after.ino()));
    assert_eq!(
        fs::read(&target).unwrap(),
        b"Edited after completed Rename and process death"
    );
    assert_eq!(fs::read(&source).unwrap(), b"Newer original occupant");
    assert_eq!(jobs.snapshot().undo.as_ref().unwrap().id, operation);
    let observation = serde_json::json!({"job":id,"operation":operation,"row":format!("{row:?}"),"target_identity":[after.dev(),after.ino()],"target_bytes":fs::read(&target).unwrap(),"source_bytes":fs::read(&source).unwrap()});
    fs::write(
        root.path().join("reopened-result.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
    if row.phase != Phase::Finished {
        panic!(
            "completed Rename restored as {:?}; fixture preserved at {}",
            row.phase,
            root.keep().display()
        );
    }
    assert!(!row.dismissible);
    assert!(!row.transfer_actions.retry && !row.transfer_actions.remaining);
}

#[test]
fn killed_result_save_reopens_the_exact_filesystem_execution_without_replaying_it() {
    for kind in ["rename", "partial-copy"] {
        let root = tempfile::Builder::new()
            .prefix("canopa-request-execution-")
            .tempdir()
            .unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "cases::request_execution::request_execution_crash_helper",
            ])
            .env("CANOPA_REQUEST_EXECUTION_ROOT", root.path())
            .env("CANOPA_REQUEST_EXECUTION_KIND", kind)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let marker: serde_json::Value =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        let job = marker["job"].as_u64().unwrap();
        let operation = marker["operation"].as_u64().unwrap();
        let target = if kind == "rename" {
            root.path().join("renamed")
        } else {
            root.path().join("destination/source/a-completed")
        };
        let identity = fs::metadata(&target).unwrap();
        fs::write(&target, b"Edited after the process died").unwrap();
        let jobs = start(root.path());
        let state = jobs.snapshot();
        let row = state.jobs.iter().find(|row| row.id == job).unwrap();
        assert_eq!(row.phase, Phase::SaveFailed);
        assert!(!row.transfer_actions.retry && !row.transfer_actions.remaining);
        if kind == "rename" {
            assert!(
                row.detail
                    .as_deref()
                    .unwrap_or("")
                    .contains(&format!("Filesystem operation #{operation}"))
            );
        } else {
            let report = row
                .transfer
                .as_ref()
                .expect("exact stopped result survived");
            assert_eq!(report.operation, Some(operation));
            assert!(report.completed > 0);
            assert_eq!(report.failures.len(), 1);
            assert!(
                report
                    .destinations
                    .iter()
                    .any(|path| path == target.parent().unwrap())
            );
        }
        assert_eq!(state.undo.as_ref().unwrap().id, operation);
        assert_eq!(fs::read(&target).unwrap(), b"Edited after the process died");
        let after = fs::metadata(&target).unwrap();
        assert_eq!((after.dev(), after.ino()), (identity.dev(), identity.ino()));
        if kind == "rename" {
            assert!(!root.path().join("source").exists());
        } else {
            assert_eq!(
                fs::read(root.path().join("source/a-completed")).unwrap(),
                b"Original source"
            );
        }
        {
            assert!(state.holds_application());
            let mut shutdown = jobs.shutdown().unwrap();
            assert!(wait(|| shutdown.try_recv().unwrap()).is_err());
            let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
            database
                .execute_batch("DROP TRIGGER refuse_result")
                .unwrap();
            jobs.retry_save(job).unwrap();
            wait(|| {
                jobs.snapshot()
                    .jobs
                    .iter()
                    .any(|row| {
                        row.id == job
                            && row.phase
                                == if kind == "rename" {
                                    Phase::Finished
                                } else {
                                    Phase::Stopped
                                }
                    })
                    .then_some(())
            });
            assert_eq!(fs::read(&target).unwrap(), b"Edited after the process died");
        }
        if kind == "partial-copy" {
            let restored = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|row| row.id == job)
                .unwrap();
            assert!(restored.transfer_actions.retry && restored.transfer_actions.abandon);
            let mut abandon = jobs.abandon_transfer(job).unwrap();
            wait(|| abandon.try_recv().unwrap()).unwrap();
        }
        let mut shutdown = jobs.shutdown().unwrap();
        wait(|| shutdown.try_recv().unwrap()).unwrap();
        let engine =
            canopa_files::operations::OperationEngine::open(root.path().join("state/history"))
                .unwrap();
        let record = engine
            .request_page(None)
            .unwrap()
            .into_iter()
            .find(|record| record.id() == job)
            .unwrap();
        let effects = engine.request_effects(&record).unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].revision, 2);
        assert_eq!(effects[0].operation, operation);
        assert!(effects[0].confirmed_steps > 0 && effects[0].history_available);
        assert!(!effects[0].journal_pending);
        assert!(effects[0].paths.contains(&target));
        assert_eq!(
            effects[0].payload_sha256.as_slice(),
            serde_json::from_value::<Vec<u8>>(marker["payload_sha256"].clone()).unwrap()
        );
    }
}

#[test]
fn reviewing_and_dismissing_a_lost_rename_result_preserves_files_and_execution_evidence() {
    let root = tempfile::Builder::new()
        .prefix("canopa-request-execution-")
        .tempdir()
        .unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::request_execution::request_execution_crash_helper",
        ])
        .env("CANOPA_REQUEST_EXECUTION_ROOT", root.path())
        .env("CANOPA_REQUEST_EXECUTION_KIND", "rename")
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
    let id = marker["job"].as_u64().unwrap();
    let operation = marker["operation"].as_u64().unwrap();
    let target = root.path().join("renamed");
    fs::write(&target, b"Newer work after death").unwrap();
    let identity = fs::metadata(&target).unwrap();
    // A schema25 history has start/step evidence, but no typed end receipt.
    // Upgrading it must preserve review/dismissal rather than invent success.
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database
        .execute_batch("DROP TABLE request_mutation_ends; PRAGMA user_version=25")
        .unwrap();
    drop(database);
    let jobs = start(root.path());
    let row = jobs
        .snapshot()
        .jobs
        .into_iter()
        .find(|row| row.id == id)
        .unwrap();
    assert_eq!(row.phase, Phase::Interrupted);
    assert!(row.dismissible);
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    let original: Vec<u8> = database
        .query_row("SELECT body FROM requests WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    // The original save refusal remains installed after process death. Review
    // must fail atomically and remain available until storage permits saving.
    let mut dismissal = jobs.dismiss_request(id).unwrap();
    assert!(wait(|| dismissal.try_recv().unwrap()).is_err());
    assert_eq!(
        jobs.snapshot()
            .jobs
            .iter()
            .find(|row| row.id == id)
            .unwrap()
            .phase,
        Phase::Interrupted
    );
    database
        .execute_batch("DROP TRIGGER refuse_result")
        .unwrap();
    let mut dismissal = jobs.dismiss_request(id).unwrap();
    wait(|| dismissal.try_recv().unwrap()).unwrap();
    assert_eq!(jobs.snapshot().undo.unwrap().id, operation);
    assert_eq!(fs::read(&target).unwrap(), b"Newer work after death");
    let after = fs::metadata(&target).unwrap();
    assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
    assert!(!root.path().join("source").exists());
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
    let jobs = start(root.path());
    let row = jobs
        .snapshot()
        .jobs
        .into_iter()
        .find(|row| row.id == id)
        .unwrap();
    assert_eq!(row.phase, Phase::Finished);
    assert!(!row.dismissible);
    assert!(row.detail.unwrap().contains("dismissed after review"));
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
    let engine =
        canopa_files::operations::OperationEngine::open(root.path().join("state/history")).unwrap();
    let record = engine
        .request_page(None)
        .unwrap()
        .into_iter()
        .find(|record| record.id() == id)
        .unwrap();
    let effects = engine.request_effects(&record).unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(
        (
            effects[0].revision,
            effects[0].operation,
            effects[0].confirmed_steps
        ),
        (2, operation, 1)
    );
    use sha2::{Digest, Sha256};
    assert_eq!(
        effects[0].payload_sha256.as_slice(),
        Sha256::digest(original).as_slice()
    );
}

#[test]
fn upgrading_schema22_preserves_old_requests_without_inventing_execution_evidence() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("history");
    let engine = canopa_files::operations::OperationEngine::open(&history).unwrap();
    let accepted = engine.queue_request(&"legacy queued request").unwrap();
    drop(engine);
    let database = rusqlite::Connection::open(&history).unwrap();
    database
        .execute_batch("DROP TABLE request_executions; PRAGMA user_version=22;")
        .unwrap();
    drop(database);
    let engine = canopa_files::operations::OperationEngine::open(&history).unwrap();
    let record = engine.request_page(None).unwrap().remove(0);
    assert_eq!(record.id(), accepted.id());
    assert_eq!(record.payload::<String>().unwrap(), "legacy queued request");
    assert!(engine.request_effects(&record).unwrap().is_empty());
    let database = rusqlite::Connection::open(&history).unwrap();
    assert_eq!(
        database
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap(),
        61
    );
}

#[test]
#[ignore = "private child killed after filesystem commit and a refused result checkpoint"]
fn request_execution_crash_helper() {
    let root =
        PathBuf::from(std::env::var_os("CANOPA_REQUEST_EXECUTION_ROOT").expect("parent fixture"));
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-request-execution-")
    );
    let kind = std::env::var("CANOPA_REQUEST_EXECUTION_KIND").unwrap();
    let source = match std::env::var_os("CANOPA_REQUEST_EXECUTION_SOURCE") {
        None => transfer_source(&root, &kind),
        Some(path) => {
            let path = PathBuf::from(path);
            let parent = path.parent().unwrap();
            assert_eq!(parent.parent().unwrap(), std::env::current_dir().unwrap());
            assert!(
                parent
                    .file_name()
                    .unwrap()
                    .as_encoded_bytes()
                    .starts_with(b".canopa-transfer-source-")
            );
            assert_eq!(path.file_name().unwrap(), "source");
            path
        }
    };
    let request = if kind == "create-folder" {
        Request::Mutation(OperationRequest::CreateFolder {
            path: root.join("created"),
        })
    } else if kind == "rename" {
        fs::write(&source, b"Original source").unwrap();
        Request::Mutation(OperationRequest::Rename {
            source,
            destination: root.join("renamed"),
        })
    } else if matches!(
        kind.as_str(),
        "copy"
            | "move"
            | "copy-directory"
            | "move-directory"
            | "copy-batch"
            | "move-batch"
            | "copy-bytes"
            | "move-bytes"
    ) {
        if kind.ends_with("directory") {
            fs::create_dir_all(source.join("nested")).unwrap();
            fs::write(source.join("nested/note"), b"Original source").unwrap();
        } else {
            fs::write(&source, b"Original source").unwrap();
        }
        let destination = root.join("destination");
        fs::create_dir(&destination).unwrap();
        let mut sources = vec![source];
        if kind.ends_with("batch") {
            let second = root.join("second");
            fs::write(&second, b"Second source").unwrap();
            sources.push(second);
        }
        Request::Transfer(TransferRequest {
            kind: if kind.starts_with("copy") {
                TransferKind::Copy
            } else {
                TransferKind::Move
            },
            sources,
            destination,
        })
    } else {
        assert!(matches!(kind.as_str(), "partial-copy" | "partial-move"));
        fs::create_dir(&source).unwrap();
        fs::write(source.join("a-completed"), b"Original source").unwrap();
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            source.join("b-failed"),
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .unwrap();
        let destination = root.join("destination");
        fs::create_dir(&destination).unwrap();
        Request::Transfer(TransferRequest {
            kind: if kind == "partial-copy" {
                TransferKind::Copy
            } else {
                TransferKind::Move
            },
            sources: vec![source],
            destination,
        })
    };
    let jobs = start(&root);
    let database = rusqlite::Connection::open(root.join("state/history")).unwrap();
    database.execute_batch("CREATE TRIGGER refuse_result BEFORE UPDATE OF phase ON requests WHEN OLD.phase=1 AND NEW.phase<>1 BEGIN SELECT RAISE(ABORT,'private result-save refusal'); END;").unwrap();
    let mut admission = jobs.submit(request, None).unwrap();
    let job = wait(|| admission.try_recv().unwrap()).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .iter()
            .any(|row| row.id == job && row.phase == Phase::SaveFailed)
            .then_some(())
    });
    let operation = jobs.snapshot().undo.unwrap().id;
    let payload_sha256: Vec<u8> = database
        .query_row(
            "SELECT payload_sha256 FROM request_executions WHERE request=?1",
            [job],
            |row| row.get(0),
        )
        .unwrap();
    fs::write(
        root.join("marker.json"),
        serde_json::to_vec(
            &serde_json::json!({"job":job,"operation":operation,"payload_sha256":payload_sha256}),
        )
        .unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("child was not killed");
}
