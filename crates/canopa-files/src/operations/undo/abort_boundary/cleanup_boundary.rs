//! Actual public Undo worker death after reversal commit and before root cleanup.
use super::*;

pub(in crate::operations) fn before_completion(
    connection: &rusqlite::Connection,
    pending: &Pending,
) {
    let Ok(fault) = std::env::var("CANOPA_UNDO_CLEANUP_FAULT") else {
        return;
    };
    let Pending::Undo {
        change: Change::SourceRemoved { .. },
        stage: Some(stage),
        ..
    } = pending
    else {
        return;
    };
    let root = PathBuf::from(std::env::var_os("CANOPA_UNDO_ABORT_ROOT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    fs::write(
        root.join("completed-cleanup-stage.json"),
        serde_json::to_vec(stage).unwrap(),
    )
    .unwrap();
    let binding: String = connection
        .query_row("SELECT body FROM undo_work", [], |r| r.get(0))
        .unwrap();
    fs::write(
        root.join("cleanup-before-completion.json"),
        serde_json::to_vec(&(store::encode(pending).unwrap(), binding)).unwrap(),
    )
    .unwrap();
    let sql = match fault.as_str() {
        "refuse-receipt" => {
            "CREATE TRIGGER private_cleanup BEFORE INSERT ON pending WHEN json_extract(NEW.body,'$.direction')='undo-cleanup' BEGIN SELECT RAISE(ABORT,'private completed Undo receipt refusal'); END;"
        }
        "ignore-receipt" => {
            "CREATE TRIGGER private_cleanup BEFORE INSERT ON pending WHEN json_extract(NEW.body,'$.direction')='undo-cleanup' BEGIN SELECT RAISE(IGNORE); END;"
        }
        "refuse-binding" => {
            "CREATE TRIGGER private_cleanup BEFORE INSERT ON undo_cleanup BEGIN SELECT RAISE(ABORT,'private completed Undo binding refusal'); END;"
        }
        "ignore-binding" => {
            "CREATE TRIGGER private_cleanup BEFORE INSERT ON undo_cleanup BEGIN SELECT RAISE(IGNORE); END;"
        }
        _ => return,
    };
    connection.execute_batch(sql).unwrap();
}
pub(in crate::operations) fn point(stage: &Stage) {
    if std::env::var("CANOPA_UNDO_ABORT_POINT").as_deref() != Ok("completed-cleanup") {
        return;
    }
    if let Ok(fault) = std::env::var("CANOPA_UNDO_CLEANUP_FAULT") {
        let root = PathBuf::from(std::env::var_os("CANOPA_UNDO_ABORT_ROOT").unwrap());
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        let sql = match fault.as_str() {
            "refuse-retirement" => {
                "CREATE TRIGGER private_cleanup BEFORE DELETE ON undo_cleanup BEGIN SELECT RAISE(ABORT,'private completed Undo retirement refusal'); END;"
            }
            "ignore-retirement" => {
                "CREATE TRIGGER private_cleanup BEFORE DELETE ON undo_cleanup BEGIN SELECT RAISE(IGNORE); END;"
            }
            "ignore-pending-retirement" => {
                "CREATE TRIGGER private_cleanup BEFORE DELETE ON pending BEGIN SELECT RAISE(IGNORE); END;"
            }
            _ => return,
        };
        let pending: String = db
            .query_row("SELECT body FROM pending", [], |r| r.get(0))
            .unwrap();
        let binding: String = db
            .query_row("SELECT body FROM undo_cleanup", [], |r| r.get(0))
            .unwrap();
        fs::write(
            root.join("cleanup-before-retirement.json"),
            serde_json::to_vec(&(pending, binding)).unwrap(),
        )
        .unwrap();
        db.execute_batch(sql).unwrap();
        return;
    }
    if std::env::var("CANOPA_UNDO_CLEANUP_CUT").as_deref() != Ok("roots-gone") {
        stop(stage);
    }
}
pub(in crate::operations) fn after_roots(roots: &[Stage]) {
    if std::env::var("CANOPA_UNDO_ABORT_POINT").as_deref() == Ok("completed-cleanup")
        && std::env::var("CANOPA_UNDO_CLEANUP_CUT").as_deref() == Ok("roots-gone")
    {
        stop(&roots[0]);
    }
}
fn stop(stage: &Stage) {
    if std::env::var("CANOPA_UNDO_ABORT_POINT").as_deref() != Ok("completed-cleanup") {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CANOPA_UNDO_ABORT_ROOT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    if stage.directory.parent() != Some(root.join("source").as_path()) {
        return;
    }
    if stage.directory.exists() {
        assert!(stage.identity.matches(&stage.directory));
        assert_eq!(fs::read_dir(&stage.directory).unwrap().count(), 0);
    } else {
        assert_eq!(
            std::env::var("CANOPA_UNDO_CLEANUP_CUT").as_deref(),
            Ok("roots-gone")
        );
    }
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        vec![b'x'; 128 * 1024]
    );
    fs::write(
        root.join("completed-cleanup-stage.json"),
        serde_json::to_vec(stage).unwrap(),
    )
    .unwrap();
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let counts: (u64, u64) = db
        .query_row(
            "SELECT (SELECT count(*) FROM pending), (SELECT count(*) FROM undo_work)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    fs::write(
        root.join("completed-cleanup-owner-counts.json"),
        serde_json::to_vec(&counts).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private completed-reversal cleanup worker was not killed");
}

fn fixture(run: impl FnOnce(&Path, &Path, &Stage)) {
    fixture_at("committed", None, run);
}
fn fixture_at(cut: &str, fault: Option<&str>, run: impl FnOnce(&Path, &Path, &Stage)) {
    let root = tempfile::Builder::new()
        .prefix("canopa-undo-abort-")
        .tempdir()
        .unwrap();
    let output = tempfile::Builder::new()
        .prefix(".canopa-undo-abort-output-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--ignored",
                "--exact",
                "operations::undo::abort_boundary::undo_abort_child",
            ])
            .env("CANOPA_UNDO_ABORT_ROOT", root.path())
            .env("CANOPA_UNDO_ABORT_OUTPUT", output.path())
            .env("CANOPA_UNDO_ABORT_POINT", "completed-cleanup")
            .env("CANOPA_UNDO_CLEANUP_CUT", cut)
            .env_remove("CANOPA_UNDO_CLEANUP_FAULT");
        if let Some(fault) = fault {
            child.env("CANOPA_UNDO_CLEANUP_FAULT", fault);
        }
        let status = child.status().unwrap();
        assert_eq!(status.signal(), Some(9));
        let stage: Stage = serde_json::from_slice(
            &fs::read(root.path().join("completed-cleanup-stage.json")).unwrap(),
        )
        .unwrap();
        run(root.path(), output.path(), &stage);
    }));
    if let Err(error) = checked {
        eprintln!("retained Undo cleanup source: {}", root.keep().display());
        eprintln!("retained Undo cleanup output: {}", output.keep().display());
        std::panic::resume_unwind(error);
    }
}

#[test]
fn completed_reversal_cleanup_survives_restart_without_repeating_the_return() {
    fixture(|root, output, stage| {
        assert!(stage.identity.matches(&stage.directory));
        assert_eq!(fs::read_dir(&stage.directory).unwrap().count(), 0);
        assert_eq!(
            fs::read(output.join("item")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        let edited = b"Later edits to the returned source";
        fs::write(root.join("source/item"), edited).unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
            let done = terminal(&jobs, id);
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            assert_eq!(fs::read(root.join("source/item")).unwrap(), edited);
            assert_eq!(
                fs::read(output.join("item")).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            assert!(
                !stage.directory.exists(),
                "a committed reversal must retain durable cleanup until its original empty work folder is gone"
            );
            close(jobs, root);
        }
        let jobs = start(root);
        let expected = jobs.snapshot().undo.unwrap().id;
        let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
        let done = terminal(&jobs, id);
        assert_eq!(done.phase, Phase::Finished, "{done:?}");
        assert_eq!(fs::read(root.join("source/item")).unwrap(), edited);
        assert!(!output.join("item").exists());
        close(jobs, root);
    });
}

fn reconcile(jobs: &OperationJobs) -> crate::operation_jobs::JobSnapshot {
    let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    terminal(jobs, id)
}
fn row(root: &Path, table: &str) -> String {
    rusqlite::Connection::open(root.join("history"))
        .unwrap()
        .query_row(&format!("SELECT body FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
fn root_item(jobs: &OperationJobs, stage: &Stage) -> RecoveryItem {
    receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
        .unwrap()
        .items
        .into_iter()
        .find(|item| {
            item.entry
                .as_ref()
                .is_some_and(|entry| entry.path == stage.directory)
        })
        .unwrap()
}
#[test]
fn completed_cleanup_preserves_reintroduced_output_and_extra_children_through_copy_keep_and_undo() {
    for returned_output in [false, true] {
        fixture(|root, output, stage| {
            if returned_output {
                fs::rename(root.join("source/item"), stage.payload()).unwrap();
                fs::write(root.join("source/item"), b"Later target occupant").unwrap();
            } else {
                fs::write(stage.payload(), b"Foreign work folder entry").unwrap();
            }
            fs::create_dir(stage.directory.join("extra")).unwrap();
            fs::write(stage.directory.join("extra/item"), b"Additional work").unwrap();
            let bytes = fs::read(stage.payload()).unwrap();
            let target = fs::read(root.join("source/item")).unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(fs::read(stage.payload()).unwrap(), bytes);
                assert_eq!(fs::read(root.join("source/item")).unwrap(), target);
                assert!(root_item(&jobs, stage).can_keep_in_recovery());
                close(jobs, root);
            }
            let jobs = start(root);
            let item = root_item(&jobs, stage);
            let id = receive(
                jobs.submit(
                    Request::Recover {
                        item: Box::new(item),
                        destination: root.join("exported"),
                    },
                    None,
                )
                .unwrap(),
            )
            .unwrap();
            let done = terminal(&jobs, id);
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            assert_eq!(fs::read(root.join("exported/folder")).unwrap(), bytes);
            assert_eq!(
                fs::read(root.join("exported/extra/item")).unwrap(),
                b"Additional work"
            );
            assert_eq!(keep(&jobs, root_item(&jobs, stage)).phase, Phase::Finished);
            let expected = jobs.snapshot().undo.unwrap().id;
            let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
            assert_eq!(terminal(&jobs, id).phase, Phase::Finished);
            assert!(!root.join("exported").exists());
            let expected = jobs.snapshot().undo.unwrap().id;
            let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
            assert_eq!(terminal(&jobs, id).phase, Phase::Finished);
            assert!(!output.join("item").exists());
            assert_eq!(fs::read(stage.payload()).unwrap(), bytes);
            assert_eq!(fs::read(root.join("source/item")).unwrap(), target);
            close(jobs, root);
        });
    }
}
#[test]
fn completed_cleanup_refuses_replacement_roots_and_recovers_when_the_original_returns() {
    fixture(|root, output, stage| {
        let held = root.join("original-empty-root");
        fs::rename(&stage.directory, &held).unwrap();
        fs::create_dir(&stage.directory).unwrap();
        fs::write(stage.payload(), b"Replacement root data").unwrap();
        fs::write(root.join("source/item"), b"Edited completed return").unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            assert_ne!(reconcile(&jobs).phase, Phase::Finished);
            assert_eq!(fs::read(stage.payload()).unwrap(), b"Replacement root data");
            assert_eq!(
                fs::read(root.join("source/item")).unwrap(),
                b"Edited completed return"
            );
            close(jobs, root);
        }
        fs::rename(&stage.directory, root.join("preserved-replacement-root")).unwrap();
        fs::rename(&held, &stage.directory).unwrap();
        let jobs = start(root);
        assert_eq!(reconcile(&jobs).phase, Phase::Finished);
        assert!(!stage.directory.exists());
        assert_eq!(
            fs::read(output.join("item")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        assert_eq!(
            fs::read(root.join("preserved-replacement-root/folder")).unwrap(),
            b"Replacement root data"
        );
        close(jobs, root);
    });
}
#[test]
fn a_second_death_after_empty_root_removal_preserves_later_occupants_and_retires_only_metadata() {
    fixture_at("roots-gone", None, |root, _, stage| {
        assert!(!stage.directory.exists());
        fs::create_dir(&stage.directory).unwrap();
        fs::write(stage.payload(), b"New root after completed removal").unwrap();
        fs::write(root.join("source/item"), b"Later return edits").unwrap();
        let jobs = start(root);
        assert_ne!(reconcile(&jobs).phase, Phase::Finished);
        assert_eq!(
            fs::read(stage.payload()).unwrap(),
            b"New root after completed removal"
        );
        close(jobs, root);
        fs::rename(&stage.directory, root.join("kept-new-root")).unwrap();
        let jobs = start(root);
        assert_eq!(reconcile(&jobs).phase, Phase::Finished);
        assert_eq!(
            fs::read(root.join("source/item")).unwrap(),
            b"Later return edits"
        );
        assert_eq!(
            fs::read(root.join("kept-new-root/folder")).unwrap(),
            b"New root after completed removal"
        );
        close(jobs, root);
    });
}
#[test]
fn failed_completed_cleanup_saves_preserve_the_exact_previous_owner() {
    for fault in [
        "refuse-receipt",
        "ignore-receipt",
        "refuse-binding",
        "ignore-binding",
        "refuse-retirement",
        "ignore-retirement",
        "ignore-pending-retirement",
    ] {
        fixture_at("committed", Some(fault), |root, _, stage| {
            assert!(
                !fs::read_to_string(root.join("completed-cleanup-result.txt"))
                    .unwrap()
                    .contains("phase: Finished")
            );
            let retirement = fault.contains("retirement");
            let before: (String, String) = serde_json::from_slice(
                &fs::read(root.join(if retirement {
                    "cleanup-before-retirement.json"
                } else {
                    "cleanup-before-completion.json"
                }))
                .unwrap(),
            )
            .unwrap();
            assert_eq!(row(root, "pending"), before.0);
            assert_eq!(
                row(
                    root,
                    if retirement {
                        "undo_cleanup"
                    } else {
                        "undo_work"
                    }
                ),
                before.1
            );
            assert_eq!(stage.directory.exists(), !retirement);
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            db.execute_batch("DROP TRIGGER IF EXISTS private_cleanup")
                .unwrap();
            drop(db);
            let jobs = start(root);
            assert_eq!(reconcile(&jobs).phase, Phase::Finished);
            assert!(!stage.directory.exists());
            assert_eq!(
                fs::read(root.join("source/item")).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            close(jobs, root);
        });
    }
}
#[test]
fn edited_completed_cleanup_receipts_never_adopt_current_work() {
    for field in ["id", "root", "binding"] {
        fixture(|root, _, stage| {
            let original = row(root, "pending");
            let binding = row(root, "undo_cleanup");
            let mut edited: serde_json::Value = serde_json::from_str(&original).unwrap();
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            match field {
                "id" => edited["cleanup"]["id"] = 999.into(),
                "root" => edited["cleanup"]["roots"][0]["identity"]["inode"] = 999.into(),
                "binding" => {
                    db.execute("DELETE FROM undo_cleanup", []).unwrap();
                }
                _ => unreachable!(),
            }
            db.execute("UPDATE pending SET body=?1", [edited.to_string()])
                .unwrap();
            drop(db);
            let jobs = start(root);
            assert_ne!(reconcile(&jobs).phase, Phase::Finished);
            assert!(stage.identity.matches(&stage.directory));
            close(jobs, root);
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            db.execute("UPDATE pending SET body=?1", [&original])
                .unwrap();
            db.execute("INSERT INTO undo_cleanup VALUES(0,?1) ON CONFLICT(lane) DO UPDATE SET body=excluded.body", [&binding]).unwrap();
            drop(db);
            let jobs = start(root);
            assert_eq!(reconcile(&jobs).phase, Phase::Finished);
            assert!(!stage.directory.exists());
            close(jobs, root);
        });
    }
}
