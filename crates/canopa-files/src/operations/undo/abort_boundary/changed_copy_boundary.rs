//! Public changed-copy preservation workers killed before the metadata handoff.
use super::*;

pub(in crate::operations) fn cut(point: &str, record: &recovery::Record) {
    if std::env::var("CANOPA_CHANGED_COPY_CUT").as_deref() != Ok(point) {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CANOPA_CHANGED_COPY_ROOT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-changed-copy-")
    );
    assert_eq!(
        record.retained.stage.directory.parent(),
        Some(root.join("output").as_path())
    );
    fs::write(
        root.join("preservation.json"),
        serde_json::to_vec(record).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private changed-copy worker was not killed");
}

#[test]
#[ignore = "private public changed-copy worker killed before preservation handoff"]
fn changed_copy_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_CHANGED_COPY_ROOT").unwrap());
    fs::create_dir(root.join("source")).unwrap();
    fs::create_dir(root.join("output")).unwrap();
    fs::write(root.join("source/item"), b"Original copied source").unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![root.join("source/item")],
                destination: root.join("output"),
            },
            &Cancellation::default(),
            |_| crate::transfers::TransferReply::Continue,
        )
        .unwrap();
    drop(engine);
    fs::write(
        root.join("output/item"),
        b"Edited copy selected for preservation",
    )
    .unwrap();
    let jobs = start(&root);
    let expected = jobs.snapshot().undo.unwrap().id;
    let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
    let question = wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Waiting)
            .and_then(|job| job.decision)
    });
    jobs.resolve(id, question.generation, UndoChoice::MoveToRecovery)
        .unwrap();
    panic!(
        "private preservation boundary not reached: {:?}",
        terminal(&jobs, id)
    );
}

fn fixture(point: &str, check: impl FnOnce(&Path, &recovery::Record)) {
    let root = tempfile::Builder::new()
        .prefix("canopa-changed-copy-")
        .tempdir()
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::undo::abort_boundary::changed_copy_boundary::changed_copy_child",
            ])
            .env("CANOPA_CHANGED_COPY_ROOT", root.path())
            .env("CANOPA_CHANGED_COPY_CUT", point)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let record =
            serde_json::from_slice(&fs::read(root.path().join("preservation.json")).unwrap())
                .unwrap();
        check(root.path(), &record);
    }));
    if let Err(error) = result {
        eprintln!(
            "retained changed-copy {point} fixture: {}",
            root.keep().display()
        );
        std::panic::resume_unwind(error);
    }
}

fn reconcile(jobs: &OperationJobs) -> crate::operation_jobs::JobSnapshot {
    let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    terminal(jobs, id)
}

#[test]
fn replacement_history_cannot_take_changed_copy_preservation_or_cleanup() {
    for point in ["registered", "preserved"] {
        fixture(point, |root, record| {
            rusqlite::Connection::open(root.join("history"))
                .unwrap()
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
                .unwrap();
            fs::rename(root.join("history"), root.join("original-history")).unwrap();
            fs::copy(root.join("original-history"), root.join("history")).unwrap();
            let jobs = start(root);
            let done = reconcile(&jobs);
            assert_ne!(
                done.phase,
                Phase::Finished,
                "replacement history must not acquire preservation: {done:?}"
            );
            assert!(
                record.retained.stage.directory.is_dir(),
                "replacement history removed the original work root"
            );
            let path = if point == "registered" {
                root.join("output/item")
            } else {
                record.retained.path()
            };
            assert_eq!(
                fs::read(path).unwrap(),
                b"Edited copy selected for preservation"
            );
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            assert_eq!(
                db.query_row("SELECT count(*) FROM pending", [], |r| r.get::<_, u64>(0))
                    .unwrap(),
                1
            );
            close(jobs, root);
        });
    }
}

fn undo(jobs: &OperationJobs, choice: UndoChoice) {
    let expected = jobs.snapshot().undo.unwrap().id;
    let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
    let decision = wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Waiting)
            .and_then(|job| job.decision)
    });
    jobs.resolve(id, decision.generation, choice).unwrap();
    assert_eq!(terminal(jobs, id).phase, Phase::Finished);
}

#[test]
fn changed_copy_preservation_resumes_or_compensates_without_losing_selected_edits() {
    for point in ["registered", "preserved"] {
        fixture(point, |root, record| {
            for _ in 0..2 {
                let jobs = start(root);
                let done = reconcile(&jobs);
                assert_eq!(done.phase, Phase::Finished, "{point}: {done:?}");
                assert_eq!(
                    fs::read(root.join("source/item")).unwrap(),
                    b"Original copied source"
                );
                if point == "registered" {
                    assert_eq!(
                        fs::read(root.join("output/item")).unwrap(),
                        b"Edited copy selected for preservation"
                    );
                    assert!(!record.retained.stage.directory.exists());
                    assert!(jobs.snapshot().undo.is_some());
                } else {
                    assert!(!root.join("output/item").exists());
                    assert_eq!(
                        fs::read(record.retained.path()).unwrap(),
                        b"Edited copy selected for preservation"
                    );
                    assert!(jobs.snapshot().undo.is_none());
                }
                close(jobs, root);
            }
            if point == "registered" {
                let jobs = start(root);
                undo(&jobs, UndoChoice::MoveToRecovery);
                assert!(!root.join("output/item").exists());
                assert!(
                    partial(&jobs, b"Edited copy selected for preservation")
                        .entry
                        .is_some()
                );
                close(jobs, root);
            }
        });
    }
}

#[test]
fn changed_copy_work_refuses_replacement_roots_and_keeps_uncertain_entries_with_fresh_review() {
    for replaced in ["root", "entry"] {
        fixture("preserved", |root, record| {
            let stage = &record.retained.stage;
            let moved = root.join("held-original");
            if replaced == "root" {
                fs::rename(&stage.directory, &moved).unwrap();
                fs::create_dir(&stage.directory).unwrap();
                fs::write(stage.directory.join("unrelated"), b"Replacement root data").unwrap();
            } else {
                fs::rename(record.retained.path(), &moved).unwrap();
                fs::write(record.retained.path(), b"Replacement saved entry").unwrap();
            }
            let jobs = start(root);
            assert_ne!(reconcile(&jobs).phase, Phase::Finished);
            if replaced == "root" {
                assert_eq!(
                    fs::read(stage.directory.join("unrelated")).unwrap(),
                    b"Replacement root data"
                );
                close(jobs, root);
                fs::rename(&stage.directory, root.join("foreign-root")).unwrap();
                fs::rename(&moved, &stage.directory).unwrap();
                let jobs = start(root);
                assert_eq!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(
                    fs::read(record.retained.path()).unwrap(),
                    b"Edited copy selected for preservation"
                );
                close(jobs, root);
            } else {
                let item = receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
                    .unwrap()
                    .items
                    .into_iter()
                    .find(|i| i.entry.as_ref().is_some_and(|e| e.path == stage.directory))
                    .unwrap();
                assert!(item.can_keep_in_recovery());
                let before = rows(root);
                fs::write(
                    record.retained.path(),
                    b"Replacement saved entry edited after review",
                )
                .unwrap();
                assert_ne!(keep(&jobs, item).phase, Phase::Finished);
                assert_eq!(rows(root), before);
                let item = receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
                    .unwrap()
                    .items
                    .into_iter()
                    .find(|i| i.entry.as_ref().is_some_and(|e| e.path == stage.directory))
                    .unwrap();
                assert_eq!(keep(&jobs, item).phase, Phase::Finished);
                assert_eq!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(
                    fs::read(record.retained.path()).unwrap(),
                    b"Replacement saved entry edited after review"
                );
                assert_eq!(
                    fs::read(&moved).unwrap(),
                    b"Edited copy selected for preservation"
                );
                fs::write(root.join("output/item"), b"Newer current work").unwrap();
                undo(&jobs, UndoChoice::KeepItem);
                assert_eq!(
                    fs::read(root.join("output/item")).unwrap(),
                    b"Newer current work"
                );
                close(jobs, root);
            }
        });
    }
}

fn rows(root: &Path) -> Vec<(String, Vec<String>)> {
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    ["pending", "undo_work", "changes", "garbage"]
        .into_iter()
        .map(|t| {
            let mut query = db
                .prepare(&format!("SELECT body FROM {t} ORDER BY body"))
                .unwrap();
            (
                t.to_owned(),
                query
                    .query_map([], |r| r.get::<_, String>(0))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap(),
            )
        })
        .collect()
}

#[test]
fn refused_or_ignored_changed_copy_handoffs_preserve_the_entire_original_owner() {
    fixture("preserved", |root, record| {
        let engine = OperationEngine::open_with_recovery_check(root.join("history"), |_| {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "private changed-copy write check",
            ))
        })
        .unwrap();
        let before = rows(root);
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        for sql in [
            "BEFORE INSERT ON garbage",
            "BEFORE DELETE ON changes",
            "BEFORE DELETE ON pending",
            "BEFORE DELETE ON undo_work",
        ] {
            for failure in ["IGNORE", "ABORT,'private preservation refusal'"] {
                db.execute_batch(&format!(
                    "CREATE TRIGGER preserve_fault {sql} BEGIN SELECT RAISE({failure}); END;"
                ))
                .unwrap();
                assert!(
                    engine.retry_recovery(&Cancellation::default()).is_err(),
                    "{sql}/{failure}"
                );
                assert_eq!(rows(root), before, "{sql}/{failure}");
                assert_eq!(
                    fs::read(record.retained.path()).unwrap(),
                    b"Edited copy selected for preservation"
                );
                db.execute_batch("DROP TRIGGER preserve_fault;").unwrap();
            }
        }
        engine.retry_recovery(&Cancellation::default()).unwrap();
        assert!(engine.latest_undo_id().is_none());
    });
}

fn fresh(check: impl FnOnce(&Path, &OperationEngine)) {
    let root = tempfile::Builder::new()
        .prefix("canopa-changed-copy-")
        .tempdir()
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fs::create_dir(root.path().join("source")).unwrap();
        fs::create_dir(root.path().join("output")).unwrap();
        fs::write(root.path().join("source/item"), b"Copied source").unwrap();
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![root.path().join("source/item")],
                    destination: root.path().join("output"),
                },
                &Cancellation::default(),
                |_| crate::transfers::TransferReply::Continue,
            )
            .unwrap();
        fs::write(root.path().join("output/item"), b"Edited copy").unwrap();
        check(root.path(), &engine);
    }));
    if let Err(error) = result {
        eprintln!(
            "retained changed-copy admission fixture: {}",
            root.keep().display()
        );
        std::panic::resume_unwind(error);
    }
}

#[test]
fn changed_copy_binding_admission_rolls_back_every_refused_or_ignored_write() {
    for table in ["pending", "undo_work"] {
        for failure in ["IGNORE", "ABORT,'private preparation refusal'"] {
            fresh(|root, engine| {
                let before = rows(root);
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                db.execute_batch(&format!("CREATE TRIGGER admission_fault BEFORE INSERT ON {table} BEGIN SELECT RAISE({failure}); END;")).unwrap();
                assert!(
                    engine
                        .undo_interactive(None, &Cancellation::default(), |_| {
                            UndoChoice::MoveToRecovery
                        })
                        .is_err()
                );
                assert_eq!(rows(root), before);
                assert_eq!(fs::read(root.join("output/item")).unwrap(), b"Edited copy");
                assert_eq!(fs::read_dir(root.join("output")).unwrap().count(), 1);
                db.execute_batch("DROP TRIGGER admission_fault;").unwrap();
                engine
                    .undo_interactive(None, &Cancellation::default(), |_| {
                        UndoChoice::MoveToRecovery
                    })
                    .unwrap();
                assert!(!root.join("output/item").exists());
            });
        }
    }
}

#[test]
fn a_changed_copy_review_cannot_adopt_a_replacement_parent_or_entry() {
    for replaced in ["parent", "entry"] {
        fresh(|root, engine| {
            let before = rows(root);
            assert!(
                engine
                    .undo_interactive(None, &Cancellation::default(), |_| {
                        if replaced == "parent" {
                            fs::rename(root.join("output"), root.join("original-output")).unwrap();
                            fs::create_dir(root.join("output")).unwrap();
                            fs::hard_link(
                                root.join("original-output/item"),
                                root.join("output/item"),
                            )
                            .unwrap();
                        } else {
                            fs::rename(root.join("output/item"), root.join("original-item"))
                                .unwrap();
                            fs::write(root.join("output/item"), b"Replacement occupant").unwrap();
                        }
                        UndoChoice::MoveToRecovery
                    })
                    .is_err()
            );
            assert_eq!(rows(root), before);
            assert_eq!(fs::read_dir(root.join("output")).unwrap().count(), 1);
            assert_eq!(
                fs::read(root.join("output/item")).unwrap(),
                if replaced == "parent" {
                    b"Edited copy".as_slice()
                } else {
                    b"Replacement occupant".as_slice()
                }
            );
        });
    }
}

#[test]
fn ignored_preservation_inserts_cannot_reuse_an_older_recovery_archive_row() {
    fresh(|root, engine| {
        let cancel = Cancellation::default();
        let mut sources = Vec::new();
        for index in 0..3 {
            let path = root.join("source").join(format!("older-{index}"));
            fs::write(&path, b"Earlier copy").unwrap();
            sources.push(path);
        }
        engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources,
                    destination: root.join("output"),
                },
                &cancel,
                |_| crate::transfers::TransferReply::Continue,
            )
            .unwrap();
        for index in 0..3 {
            fs::write(
                root.join("output").join(format!("older-{index}")),
                b"Earlier protected edit",
            )
            .unwrap();
        }
        engine
            .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
            .unwrap();
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM garbage", [], |r| r.get::<_, u64>(0))
                .unwrap(),
            3
        );
        db.execute_batch("CREATE TRIGGER archive_refusal BEFORE INSERT ON garbage BEGIN SELECT RAISE(IGNORE); END;").unwrap();
        assert!(
            engine
                .undo_interactive(None, &cancel, |_| UndoChoice::MoveToRecovery)
                .is_err(),
            "ignored archive insert must not retire an uncatalogued selected edit"
        );
        for table in ["pending", "undo_work", "changes"] {
            assert_eq!(
                db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, u64>(0))
                    .unwrap(),
                1,
                "{table}"
            );
        }
        assert_eq!(
            db.query_row("SELECT count(*) FROM garbage", [], |r| r.get::<_, u64>(0))
                .unwrap(),
            3
        );
        db.execute_batch("DROP TRIGGER archive_refusal;").unwrap();
        engine.retry_recovery(&cancel).unwrap();
        let page = engine.recovery_items(None, &cancel).unwrap();
        assert_eq!(page.items.len(), 4);
        let main = page
            .items
            .iter()
            .find(|item| item.original.as_ref() == Some(&root.join("output/item")))
            .unwrap();
        assert_eq!(
            fs::read(&main.entry.as_ref().unwrap().path).unwrap(),
            b"Edited copy"
        );
        assert!(engine.latest_undo_id().is_none());
    });
}
