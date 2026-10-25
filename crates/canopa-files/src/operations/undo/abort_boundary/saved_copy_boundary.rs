//! Actual saved-replacement Copy Undo death after its payload unlink.
use super::*;
use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

pub(in crate::operations) fn cut(point: &str, change: &Change, stage: &Stage) {
    if std::env::var("CANOPA_SAVED_COPY_UNDO_CUT").as_deref() != Ok(point) {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CANOPA_SAVED_COPY_UNDO_ROOT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-saved-copy-undo-")
    );
    assert_eq!(
        stage.directory.parent(),
        Some(root.join("output").as_path())
    );
    assert!(matches!(
        change,
        Change::Copied {
            retained: Some(_),
            ..
        }
    ));
    if point == "unlinked" {
        assert!(!stage.payload().exists());
        assert_eq!(
            fs::read(root.join("output/item")).unwrap(),
            b"Previous destination"
        );
    }
    fs::write(
        root.join("saved-copy-stage.json"),
        serde_json::to_vec(stage).unwrap(),
    )
    .unwrap();
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let pending: String = db
        .query_row("SELECT body FROM pending", [], |r| r.get(0))
        .unwrap();
    fs::write(root.join("saved-copy-pending.json"), pending).unwrap();
    fs::write(
        root.join("saved-copy-change.json"),
        serde_json::to_vec(change).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private saved Copy Undo worker was not killed");
}

#[test]
#[ignore = "private public saved-replacement Copy Undo worker killed after unlink"]
fn saved_copy_undo_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_SAVED_COPY_UNDO_ROOT").unwrap());
    fs::create_dir(root.join("source")).unwrap();
    fs::create_dir(root.join("output")).unwrap();
    let kind = std::env::var("CANOPA_SAVED_COPY_UNDO_KIND").unwrap_or_else(|_| "regular".into());
    let name = if kind == "raw" {
        OsStr::from_bytes(b"item-\xff")
    } else {
        OsStr::new("item")
    };
    let source = root.join("source").join(name);
    let output = root.join("output").join(name);
    fs::write(&source, vec![b'x'; 128 * 1024]).unwrap();
    if kind == "raw" {
        std::os::unix::fs::symlink(OsStr::from_bytes(b"../missing-\xfe"), &output).unwrap();
    } else {
        fs::write(&output, b"Previous destination").unwrap();
    }
    let engine = OperationEngine::open(root.join("history")).unwrap();
    if kind == "older" {
        fs::create_dir(root.join("older-source")).unwrap();
        fs::write(root.join("older-source/item"), b"Previous destination").unwrap();
        fs::remove_file(&output).unwrap();
        engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![root.join("older-source/item")],
                    destination: root.join("output"),
                },
                &Cancellation::default(),
                |_| crate::transfers::TransferReply::Continue,
            )
            .unwrap();
        let id: u64 = rusqlite::Connection::open(root.join("history"))
            .unwrap()
            .query_row("SELECT max(id) FROM operations", [], |r| r.get(0))
            .unwrap();
        fs::write(root.join("older-undo-id"), id.to_string()).unwrap();
    }
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination: root.join("output"),
            },
            &Cancellation::default(),
            |_| crate::transfers::TransferReply::Resolve {
                choice: crate::transfers::ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    if kind == "reviewed" {
        let saved = engine
            .recovery_items(None, &Cancellation::default())
            .unwrap()
            .items
            .into_iter()
            .next()
            .unwrap()
            .entry
            .unwrap()
            .path;
        fs::rename(&saved, root.join("previous-before-review")).unwrap();
        fs::write(&saved, b"Reviewed replacement").unwrap();
    }
    drop(engine);
    fs::hard_link(&output, root.join("copy-alias")).unwrap();
    if std::env::var("CANOPA_SAVED_COPY_UNDO_MISSING").as_deref() == Ok("1") {
        fs::remove_file(&output).unwrap();
    }
    let jobs = start(&root);
    let expected = jobs.snapshot().undo.unwrap().id;
    let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
    if kind == "reviewed" {
        let question = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|job| job.id == id && job.phase == Phase::Waiting)
                .and_then(|job| job.decision)
        });
        jobs.resolve(id, question.generation, UndoChoice::UseSavedVersion)
            .unwrap();
    }
    panic!(
        "private saved Copy Undo boundary was not reached: {:?}",
        terminal(&jobs, id)
    );
}

fn fixture(point: &str, missing: bool, check: impl FnOnce(&Path, &Stage, &Change)) {
    fixture_kind(point, missing, "regular", check);
}
fn fixture_kind(
    point: &str,
    missing: bool,
    kind: &str,
    check: impl FnOnce(&Path, &Stage, &Change),
) {
    let root = tempfile::Builder::new()
        .prefix("canopa-saved-copy-undo-")
        .tempdir()
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::undo::abort_boundary::saved_copy_boundary::saved_copy_undo_child",
            ])
            .env("CANOPA_SAVED_COPY_UNDO_ROOT", root.path())
            .env("CANOPA_SAVED_COPY_UNDO_CUT", point)
            .env("CANOPA_SAVED_COPY_UNDO_KIND", kind)
            .env(
                "CANOPA_SAVED_COPY_UNDO_MISSING",
                if missing { "1" } else { "0" },
            )
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9), "{point}");
        let stage =
            serde_json::from_slice(&fs::read(root.path().join("saved-copy-stage.json")).unwrap())
                .unwrap();
        let change =
            serde_json::from_slice(&fs::read(root.path().join("saved-copy-change.json")).unwrap())
                .unwrap();
        check(root.path(), &stage, &change);
    }));
    if let Err(e) = result {
        eprintln!(
            "retained saved Copy Undo {point} root: {}",
            root.keep().display()
        );
        std::panic::resume_unwind(e);
    }
}
fn reconcile(jobs: &OperationJobs) -> crate::operation_jobs::JobSnapshot {
    let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    terminal(jobs, id)
}
fn root_item(jobs: &OperationJobs, stage: &Stage) -> RecoveryItem {
    receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
        .unwrap()
        .items
        .into_iter()
        .find(|i| i.entry.as_ref().is_some_and(|e| e.path == stage.directory))
        .unwrap()
}
fn undo(jobs: &OperationJobs) {
    let expected = jobs.snapshot().undo.unwrap().id;
    let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
    let done = terminal(jobs, id);
    assert_eq!(done.phase, Phase::Finished, "{done:?}");
}

#[test]
fn saved_copy_undo_resumes_original_preparations_and_never_reopens_completed_return_targets() {
    for point in [
        "prepared",
        "taking",
        "detached",
        "previous-renamed",
        "restored",
        "cleared",
    ] {
        fixture(point, false, |root, stage, change| {
            if matches!(point, "previous-renamed" | "restored" | "cleared") {
                if point == "restored" {
                    fs::rename(root.join("output/item"), root.join("returned-original")).unwrap();
                }
                fs::write(root.join("output/item"), b"Later work at returned name").unwrap();
            }
            for _ in 0..2 {
                let jobs = start(root);
                let done = reconcile(&jobs);
                assert_eq!(done.phase, Phase::Finished, "{point}: {done:?}");
                assert!(!stage.directory.exists());
                let expected: &[u8] = match point {
                    "taking" => &vec![b'x'; 128 * 1024],
                    "previous-renamed" | "restored" | "cleared" => b"Later work at returned name",
                    _ => b"Previous destination",
                };
                assert_eq!(fs::read(root.join("output/item")).unwrap(), expected);
                if point != "taking" {
                    assert!(!change.retained().unwrap().stage.directory.exists());
                    assert!(jobs.snapshot().undo.is_none());
                }
                close(jobs, root);
            }
            if point == "taking" {
                let jobs = start(root);
                undo(&jobs);
                assert_eq!(
                    fs::read(root.join("output/item")).unwrap(),
                    b"Previous destination"
                );
                close(jobs, root);
            }
        });
    }
}

#[test]
fn consumed_copy_or_previous_names_require_explicit_keep_before_a_new_saved_return() {
    for point in ["copy-renamed", "restoring"] {
        fixture(point, false, |root, stage, change| {
            let saved = change.retained().unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(fs::read(stage.payload()).unwrap(), vec![b'x'; 128 * 1024]);
                assert_eq!(fs::read(saved.path()).unwrap(), b"Previous destination");
                assert!(!root.join("output/item").exists());
                assert!(root_item(&jobs, stage).can_keep_in_recovery());
                close(jobs, root);
            }
            let jobs = start(root);
            let id = receive(
                jobs.submit(
                    Request::Recover {
                        item: Box::new(root_item(&jobs, stage)),
                        destination: root.join("exported"),
                    },
                    None,
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(terminal(&jobs, id).phase, Phase::Finished);
            assert_eq!(keep(&jobs, root_item(&jobs, stage)).phase, Phase::Finished);
            undo(&jobs);
            assert!(!root.join("exported").exists());
            undo(&jobs);
            assert_eq!(
                fs::read(root.join("output/item")).unwrap(),
                b"Previous destination"
            );
            assert_eq!(fs::read(stage.payload()).unwrap(), vec![b'x'; 128 * 1024]);
            assert!(!saved.stage.directory.exists());
            close(jobs, root);
        });
    }
}

#[test]
fn absent_copies_restore_the_original_saved_destination_without_new_unlink_rights() {
    for point in [
        "prepared",
        "restoring",
        "previous-renamed",
        "restored",
        "cleared",
    ] {
        fixture(point, true, |root, stage, change| {
            let jobs = start(root);
            let done = reconcile(&jobs);
            if point == "restoring" {
                assert_ne!(done.phase, Phase::Finished);
                assert_eq!(keep(&jobs, root_item(&jobs, stage)).phase, Phase::Finished);
                undo(&jobs);
                assert!(stage.directory.exists());
            } else {
                assert_eq!(done.phase, Phase::Finished, "{point}: {done:?}");
                assert!(!stage.directory.exists());
            }
            assert_eq!(
                fs::read(root.join("output/item")).unwrap(),
                b"Previous destination"
            );
            assert_eq!(
                fs::read(root.join("copy-alias")).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            assert!(!change.retained().unwrap().stage.directory.exists());
            close(jobs, root);
        });
    }
}

#[test]
fn saved_copy_undo_refuses_replaced_saved_roots_entries_and_copy_work_roots() {
    for which in ["saved-root", "previous", "copy-root"] {
        fixture("prepared", false, |root, stage, change| {
            let saved = change.retained().unwrap();
            match which {
                "saved-root" => {
                    fs::rename(&saved.stage.directory, root.join("held-saved-root")).unwrap();
                    fs::create_dir(&saved.stage.directory).unwrap();
                    fs::write(saved.path(), b"Replacement saved root").unwrap();
                }
                "previous" => {
                    fs::rename(saved.path(), root.join("held-previous")).unwrap();
                    fs::write(saved.path(), b"Replacement saved item").unwrap();
                }
                "copy-root" => {
                    fs::rename(&stage.directory, root.join("held-copy-root")).unwrap();
                    fs::create_dir(&stage.directory).unwrap();
                    fs::write(stage.payload(), b"Replacement copy root").unwrap();
                }
                _ => unreachable!(),
            }
            for _ in 0..2 {
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(
                    fs::read(root.join("output/item")).unwrap(),
                    vec![b'x'; 128 * 1024]
                );
                if which == "copy-root" {
                    assert_eq!(fs::read(stage.payload()).unwrap(), b"Replacement copy root");
                } else {
                    assert!(fs::read(saved.path()).unwrap().starts_with(b"Replacement"));
                }
                close(jobs, root);
            }
        });
    }
}

#[test]
fn keeping_copy_work_after_a_recorded_saved_return_finishes_that_undo_without_replaying_the_target()
{
    fixture("restored", false, |root, stage, _| {
        fs::write(root.join("output/item"), b"Later returned-file edits").unwrap();
        let jobs = start(root);
        assert_eq!(keep(&jobs, root_item(&jobs, stage)).phase, Phase::Finished);
        assert!(
            jobs.snapshot().undo.is_none(),
            "Keep must commit the already recorded saved return instead of leaving its old Copy Undo owner active"
        );
        assert_eq!(reconcile(&jobs).phase, Phase::Finished);
        assert_eq!(
            fs::read(root.join("output/item")).unwrap(),
            b"Later returned-file edits"
        );
        assert_eq!(fs::read(stage.payload()).unwrap(), vec![b'x'; 128 * 1024]);
        close(jobs, root);
    });
}

fn rows(root: &Path) -> (String, String) {
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    (
        db.query_row("SELECT body FROM pending", [], |r| r.get(0))
            .unwrap(),
        db.query_row("SELECT body FROM copy_undo", [], |r| r.get(0))
            .unwrap(),
    )
}
#[test]
fn saved_copy_undo_failed_phase_and_completion_writes_leave_the_exact_previous_owner() {
    for (point, sql) in [
        ("prepared", "BEFORE UPDATE ON pending"),
        ("prepared", "BEFORE UPDATE ON copy_undo"),
        ("previous-renamed", "BEFORE UPDATE ON pending"),
        ("previous-renamed", "BEFORE UPDATE ON copy_undo"),
        ("cleared", "BEFORE DELETE ON changes"),
        ("cleared", "BEFORE DELETE ON copy_undo"),
        ("cleared", "BEFORE INSERT ON undo_cleanup"),
    ] {
        for fail in ["IGNORE", "ABORT,'private saved Copy Undo refusal'"] {
            fixture(point, false, |root, stage, change| {
                let before = rows(root);
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                db.execute_batch(&format!(
                    "CREATE TRIGGER saved_copy_fault {sql} BEGIN SELECT RAISE({fail}); END;"
                ))
                .unwrap();
                if point != "prepared" {
                    fs::write(
                        root.join("output/item"),
                        b"Later restored destination edits",
                    )
                    .unwrap();
                }
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(rows(root), before);
                assert!(stage.identity.matches(&stage.directory));
                assert!(
                    change
                        .retained()
                        .unwrap()
                        .stage
                        .identity
                        .matches(&change.retained().unwrap().stage.directory)
                );
                close(jobs, root);
                db.execute_batch("DROP TRIGGER saved_copy_fault;").unwrap();
                let jobs = start(root);
                assert_eq!(reconcile(&jobs).phase, Phase::Finished);
                assert!(!stage.directory.exists());
                assert!(!change.retained().unwrap().stage.directory.exists());
                let expected: &[u8] = if point == "prepared" {
                    b"Previous destination"
                } else {
                    b"Later restored destination edits"
                };
                assert_eq!(fs::read(root.join("output/item")).unwrap(), expected);
                close(jobs, root);
            });
        }
    }
}

#[test]
fn saved_raw_link_returns_preserve_filename_and_target_bytes_and_later_target_replacements() {
    for point in ["detached", "previous-renamed", "restored"] {
        fixture_kind(point, false, "raw", |root, stage, change| {
            let output = root.join("output").join(OsStr::from_bytes(b"item-\xff"));
            let expected = if point == "restored" {
                b"../later-\xfd".as_slice()
            } else {
                b"../missing-\xfe".as_slice()
            };
            if point == "restored" {
                fs::remove_file(&output).unwrap();
                std::os::unix::fs::symlink(OsStr::from_bytes(expected), &output).unwrap();
            }
            for _ in 0..2 {
                let jobs = start(root);
                let done = reconcile(&jobs);
                assert_eq!(done.phase, Phase::Finished, "{point}: {done:?}");
                assert_eq!(
                    fs::read_link(&output).unwrap().as_os_str().as_bytes(),
                    expected
                );
                assert!(!stage.directory.exists());
                assert!(!change.retained().unwrap().stage.directory.exists());
                close(jobs, root);
            }
        });
    }
}

#[test]
fn kept_recorded_returns_commit_atomically_and_leave_older_copy_undo_on_the_returned_file() {
    fixture_kind("restored", false, "older", |root, stage, change| {
        let jobs = start(root);
        let before = rows(root);
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        for sql in [
            "BEFORE INSERT ON garbage",
            "BEFORE DELETE ON changes",
            "BEFORE DELETE ON copy_undo",
            "BEFORE INSERT ON undo_cleanup",
        ] {
            for fail in ["IGNORE", "ABORT,'private kept saved return refusal'"] {
                db.execute_batch(&format!(
                    "CREATE TRIGGER kept_return_fault {sql} BEGIN SELECT RAISE({fail}); END;"
                ))
                .unwrap();
                assert_ne!(keep(&jobs, root_item(&jobs, stage)).phase, Phase::Finished);
                assert_eq!(rows(root), before);
                assert_eq!(
                    db.query_row("SELECT count(*) FROM garbage", [], |r| r.get::<_, u64>(0))
                        .unwrap(),
                    0
                );
                db.execute_batch("DROP TRIGGER kept_return_fault;").unwrap();
            }
        }
        assert_eq!(keep(&jobs, root_item(&jobs, stage)).phase, Phase::Finished);
        let older: u64 = fs::read_to_string(root.join("older-undo-id"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(jobs.snapshot().undo.unwrap().id, older);
        assert!(!change.retained().unwrap().stage.directory.exists());
        undo(&jobs);
        assert!(!root.join("output/item").exists());
        assert_eq!(fs::read(stage.payload()).unwrap(), vec![b'x'; 128 * 1024]);
        assert_eq!(
            fs::read(root.join("older-source/item")).unwrap(),
            b"Previous destination"
        );
        close(jobs, root);
    });
}

#[test]
fn explicitly_reviewed_new_saved_objects_keep_their_selected_generation_across_restart() {
    for point in ["prepared", "previous-renamed", "restored"] {
        fixture_kind(point, false, "reviewed", |root, stage, _| {
            for _ in 0..2 {
                let jobs = start(root);
                let done = reconcile(&jobs);
                assert_eq!(done.phase, Phase::Finished, "{point}: {done:?}");
                assert_eq!(
                    fs::read(root.join("output/item")).unwrap(),
                    b"Reviewed replacement"
                );
                assert_eq!(
                    fs::read(root.join("previous-before-review")).unwrap(),
                    b"Previous destination"
                );
                assert!(!stage.directory.exists());
                close(jobs, root);
            }
        });
    }
}

#[test]
fn selected_saved_sources_remain_reserved_before_copy_claim_and_before_saved_return() {
    for point in ["prepared", "detached"] {
        fixture(point, false, |root, stage, change| {
            let engine = OperationEngine::open_with_recovery_check(root.join("history"), |_| {
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "private source-lease observation",
                ))
            })
            .unwrap();
            let captured = engine
                .capture_retained_sources(&[change.retained().unwrap().path()])
                .unwrap();
            let before = rows(root);
            let rejected = engine.lease_retained_sources(&captured).unwrap_err();
            assert_eq!(rejected.kind(), io::ErrorKind::WouldBlock);
            assert_eq!(rows(root), before);
            engine.retry_recovery(&Cancellation::default()).unwrap();
            assert_eq!(
                fs::read(root.join("output/item")).unwrap(),
                b"Previous destination"
            );
            assert!(!stage.directory.exists());
            drop(engine);
        });
    }
}

#[test]
fn a_saved_source_lease_blocks_undo_before_any_copy_or_previous_name_is_consumed() {
    let root = tempfile::Builder::new()
        .prefix("canopa-saved-copy-undo-")
        .tempdir()
        .unwrap();
    fs::create_dir(root.path().join("source")).unwrap();
    fs::create_dir(root.path().join("output")).unwrap();
    fs::write(root.path().join("source/item"), vec![b'x'; 128 * 1024]).unwrap();
    fs::write(root.path().join("output/item"), b"Previous destination").unwrap();
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![root.path().join("source/item")],
                destination: root.path().join("output"),
            },
            &Cancellation::default(),
            |_| crate::transfers::TransferReply::Resolve {
                choice: crate::transfers::ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    let saved = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .next()
        .unwrap()
        .entry
        .unwrap()
        .path;
    let capture = engine
        .capture_retained_sources(std::slice::from_ref(&saved))
        .unwrap();
    let lease = engine.lease_retained_sources(&capture).unwrap();
    assert_eq!(
        engine.undo(&Cancellation::default()).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(
        fs::read(root.path().join("output/item")).unwrap(),
        vec![b'x'; 128 * 1024]
    );
    assert_eq!(fs::read(&saved).unwrap(), b"Previous destination");
    let db = rusqlite::Connection::open(root.path().join("history")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM pending", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        0
    );
    lease.release().unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(
        fs::read(root.path().join("output/item")).unwrap(),
        b"Previous destination"
    );
    assert!(!saved.parent().unwrap().exists());
}

#[test]
fn schema55_bound_copy_undo_keeps_its_wire_binding_when_opened_by_schema56() {
    let root = tempfile::Builder::new()
        .prefix("canopa-copy-undo-")
        .tempdir()
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::undo::abort_boundary::copy_boundary::copy_undo_child",
            ])
            .env("CANOPA_COPY_UNDO_ROOT", root.path())
            .env("CANOPA_COPY_UNDO_CUT", "prepared")
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let db = rusqlite::Connection::open(root.path().join("history")).unwrap();
        let body: String = db
            .query_row("SELECT body FROM pending", [], |r| r.get(0))
            .unwrap();
        assert!(!body.contains("previous_returned") && !body.contains("\"previous\":"));
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(value["removal"]["selected"].is_object());
        db.pragma_update(None, "user_version", 55).unwrap();
        drop(db);
        for _ in 0..2 {
            let jobs = start(root.path());
            assert_eq!(reconcile(&jobs).phase, Phase::Finished);
            assert!(!root.path().join("output/item").exists());
            assert!(jobs.snapshot().undo.is_none());
            close(jobs, root.path());
        }
        assert_eq!(
            fs::read(root.path().join("source/item")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
    }));
    if let Err(e) = result {
        eprintln!(
            "retained schema55 Copy Undo root: {}",
            root.keep().display()
        );
        std::panic::resume_unwind(e);
    }
}

fn legacy_record(root: &Path, fresh: &Stage, change: &Change) {
    let saved = change.retained().unwrap();
    fs::remove_dir(&fresh.directory).unwrap();
    fs::hard_link(root.join("copy-alias"), saved.stage.payload()).unwrap();
    fs::write(root.join("output/item"), b"Later restored-file edits").unwrap();
    let mut db = rusqlite::Connection::open(root.join("history")).unwrap();
    let (id, ordinal): (u64, u64) = db
        .query_row("SELECT operation,ordinal FROM changes", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    let legacy = Pending::Undo {
        id,
        ordinal,
        change: change.clone(),
        stage: Some(saved.stage.clone()),
        relocation: None,
        displaced: None,
    };
    let transaction = db.transaction().unwrap();
    transaction
        .execute(
            "UPDATE pending SET body=?1",
            [serde_json::to_string(&legacy).unwrap()],
        )
        .unwrap();
    transaction.execute("DELETE FROM copy_undo", []).unwrap();
    transaction.pragma_update(None, "user_version", 55).unwrap();
    transaction.commit().unwrap();
    drop(db);
}

#[test]
fn legacy_saved_completion_refuses_replaced_history_files() {
    fixture("unlinked", false, |root, fresh, change| {
        legacy_record(root, fresh, change);
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        drop(db);
        fs::rename(root.join("history"), root.join("original-history")).unwrap();
        fs::copy(root.join("original-history"), root.join("history")).unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            assert_ne!(reconcile(&jobs).phase, Phase::Finished);
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            assert_eq!(
                db.query_row("SELECT count(*) FROM changes", [], |r| r.get::<_, u64>(0))
                    .unwrap(),
                1,
                "a replacement history cannot acquire original saved-root cleanup ownership"
            );
            assert_eq!(
                fs::read(change.retained().unwrap().stage.payload()).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            close(jobs, root);
        }
    });
}

#[test]
fn unbound_legacy_saved_copy_undo_cannot_remove_reintroduced_payloads_on_restart() {
    fixture("unlinked", false, |root, fresh, change| {
        legacy_record(root, fresh, change);
        let saved = change.retained().unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            let done = reconcile(&jobs);
            assert!(
                saved.stage.payload().exists(),
                "an unbound legacy Copy Undo journal cannot authorize a restarted unlink"
            );
            assert_ne!(done.phase, Phase::Finished);
            assert_eq!(
                fs::read(saved.stage.payload()).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            assert_eq!(
                fs::read(root.join("output/item")).unwrap(),
                b"Later restored-file edits"
            );
            assert!(root_item(&jobs, &saved.stage).entry.is_some());
            close(jobs, root);
        }
    });
}

#[test]
fn consumed_saved_copy_undo_payload_name_is_never_unlinked_again_after_restart() {
    let root = tempfile::Builder::new()
        .prefix("canopa-saved-copy-undo-")
        .tempdir()
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::undo::abort_boundary::saved_copy_boundary::saved_copy_undo_child",
            ])
            .env("CANOPA_SAVED_COPY_UNDO_ROOT", root.path())
            .env("CANOPA_SAVED_COPY_UNDO_CUT", "unlinked")
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let stage: Stage =
            serde_json::from_slice(&fs::read(root.path().join("saved-copy-stage.json")).unwrap())
                .unwrap();
        fs::hard_link(root.path().join("copy-alias"), stage.payload()).unwrap();
        fs::write(
            root.path().join("output/item"),
            b"Later edits to restored previous destination",
        )
        .unwrap();
        let identity = Identity::read(&stage.payload()).unwrap();
        for _ in 0..2 {
            let jobs = start(root.path());
            let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
            let done = terminal(&jobs, id);
            fs::write(
                root.path().join("saved-copy-reconcile.txt"),
                format!("{done:?}"),
            )
            .unwrap();
            assert!(
                identity.matches(&stage.payload()),
                "restart must not repeat a consumed unlink on a reintroduced Copy Undo payload name"
            );
            assert_eq!(fs::read(stage.payload()).unwrap(), vec![b'x'; 128 * 1024]);
            assert_eq!(
                fs::read(root.path().join("output/item")).unwrap(),
                b"Later edits to restored previous destination"
            );
            close(jobs, root.path());
        }
    }));
    if let Err(error) = result {
        eprintln!("retained saved Copy Undo root: {}", root.keep().display());
        std::panic::resume_unwind(error);
    }
}
