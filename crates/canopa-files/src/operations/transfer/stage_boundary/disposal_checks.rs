//! Public worker deaths and reviewed copies from both disposal namespaces.
use super::*;
mod source_checks;
use crate::operations::staging::disposal::{Disposal, Phase as DisposalPhase};

#[derive(Serialize, Deserialize)]
struct Cut {
    disposal: Disposal,
    pending: String,
    binding: String,
}
pub(super) fn point(point: &str, disposal: &Disposal) {
    let Ok(wanted) = std::env::var("CANOPA_STAGE_DISPOSE_POINT") else {
        return;
    };
    let observed = match wanted.as_str() {
        "refuse-consumed" | "ignore-consumed" => "claimed",
        "refuse-retirement" => "roots-gone",
        wanted => wanted,
    };
    if observed != point {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CANOPA_STAGE_DISPOSE_ROOT").unwrap());
    let original: Marker =
        serde_json::from_slice(&fs::read(root.join("marker.json")).unwrap()).unwrap();
    assert_eq!(disposal.original.directory, work(&original).directory);
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let pending: String = db
        .query_row("SELECT body FROM pending", [], |r| r.get(0))
        .unwrap();
    let binding: String = db
        .query_row("SELECT body FROM stage_intents WHERE lane=0", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&pending).unwrap(),
        serde_json::to_value(Pending::Disposing {
            disposal: Box::new(disposal.clone())
        })
        .unwrap()
    );
    fs::write(
        root.join("disposal-phase.json"),
        serde_json::to_vec(&Cut {
            disposal: disposal.clone(),
            pending,
            binding,
        })
        .unwrap(),
    )
    .unwrap();
    match wanted.as_str() {
        "refuse-consumed" => db.execute_batch("CREATE TRIGGER refuse_disposal_write BEFORE UPDATE ON pending WHEN json_extract(NEW.body,'$.disposal.phase')='consumed' BEGIN SELECT RAISE(ABORT,'private disposal phase refusal'); END;").unwrap(),
        "ignore-consumed" => db.execute_batch("CREATE TRIGGER refuse_disposal_write BEFORE UPDATE ON pending WHEN json_extract(NEW.body,'$.disposal.phase')='consumed' BEGIN SELECT RAISE(IGNORE); END;").unwrap(),
        "refuse-retirement" => db.execute_batch("CREATE TRIGGER refuse_disposal_write BEFORE DELETE ON pending BEGIN SELECT RAISE(ABORT,'private disposal retirement refusal'); END;").unwrap(),
        _ => {
            rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL).unwrap();
            panic!("private phase worker was not killed");
        }
    }
}
fn fixture_cut(point: &str, shape: &str, run: impl FnOnce(&Path, Marker, Cut)) {
    fixture_shape(
        if shape == "link" {
            "payload"
        } else {
            "written"
        },
        shape,
        |root, marker| {
            fs::write(marker.output.join("a-done"), b"Changed completed output").unwrap();
            fs::write(root.join("a-done"), b"Changed completed source").unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::transfer::stage_boundary::disposal_child",
                ])
                .env("CANOPA_STAGE_DISPOSE_ROOT", root)
                .env("CANOPA_STAGE_DISPOSE_POINT", point)
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let cut = serde_json::from_slice(&fs::read(root.join("disposal-phase.json")).unwrap())
                .unwrap();
            run(root, marker, cut);
        },
    );
}
fn unchanged(root: &Path, marker: &Marker) {
    assert_eq!(
        fs::read(marker.output.join("a-done")).unwrap(),
        b"Changed completed output"
    );
    assert_eq!(
        fs::read(root.join("a-done")).unwrap(),
        b"Changed completed source"
    );
}
fn continue_work(root: &Path, marker: &Marker, shape: &str) {
    let jobs = start(root);
    let id = receive(
        jobs.resume_transfer(marker.request, TransferPart::Remaining)
            .unwrap(),
    )
    .unwrap();
    let result = wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|j| j.id == id && !j.phase.holds_application())
    });
    assert_eq!(result.phase, Phase::Finished, "{result:?}");
    if shape == "link" {
        use std::os::unix::ffi::OsStringExt;
        assert_eq!(
            fs::read_link(target(marker)).unwrap(),
            PathBuf::from(std::ffi::OsString::from_vec(b"missing-\xfe".to_vec()))
        );
    } else {
        assert_eq!(fs::read(target(marker)).unwrap(), vec![b'x'; 128 * 1024]);
    }
    assert_eq!(
        fs::read(marker.output.join("z-later")).unwrap(),
        b"Later root"
    );
    assert_eq!(
        fs::symlink_metadata(root.join(target(marker).file_name().unwrap())).is_ok(),
        shape != "cross"
    );
    unchanged(root, marker);
    close(jobs, root);
}
fn copy_item(jobs: &OperationJobs, from: &Path, to: &Path, bytes: &[u8]) {
    let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
    let item = page
        .items
        .into_iter()
        .find(|i| i.entry.as_ref().is_some_and(|e| e.path == from))
        .expect("retained disposal data is visible in Recovery");
    let id = receive(
        jobs.submit(
            Request::Recover {
                item: Box::new(item),
                destination: to.to_owned(),
            },
            None,
        )
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
    assert_eq!(fs::read(to).unwrap(), bytes);
    assert_eq!(fs::read(from).unwrap(), bytes);
}
#[test]
fn verified_disposal_cuts_resume_only_unfinished_copy_move_and_raw_link_work() {
    for (shape, point) in [
        ("file", "reserved"),
        ("file", "after-claim"),
        ("file", "claimed"),
        ("file", "after-unlink"),
        ("file", "holding-gone"),
        ("file", "roots-gone"),
        ("link", "claimed"),
        ("link", "after-unlink"),
        ("cross", "after-claim"),
        ("cross", "after-unlink"),
    ] {
        fixture_cut(point, shape, |root, marker, cut| {
            for _ in 0..2 {
                let jobs = start(root);
                let result = reconcile(&jobs);
                assert_eq!(result.phase, Phase::Finished, "{shape}/{point}: {result:?}");
                assert!(!cut.disposal.original.directory.exists());
                assert!(!cut.disposal.holding.directory.exists());
                let original = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == marker.request)
                    .unwrap();
                assert_eq!(original.transfer.unwrap().completed, 1);
                assert!(original.transfer_actions.remaining);
                unchanged(root, &marker);
                close(jobs, root);
            }
            continue_work(root, &marker, shape);
        });
    }
}
#[test]
fn consumed_but_not_unlinked_data_stays_inspectable_and_copyable_until_reviewed() {
    fixture_cut("consumed", "file", |root, marker, cut| {
        assert_eq!(cut.disposal.phase, DisposalPhase::Consumed);
        let payload = cut.disposal.holding.payload();
        let bytes = fs::read(&payload).unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            assert_ne!(reconcile(&jobs).phase, Phase::Finished);
            assert_eq!(fs::read(&payload).unwrap(), bytes);
            unchanged(root, &marker);
            close(jobs, root);
        }
        let jobs = start(root);
        copy_item(&jobs, &payload, &root.join("recovered-partial"), &bytes);
        fs::rename(&payload, root.join("kept-partial")).unwrap();
        assert_eq!(reconcile(&jobs).phase, Phase::Finished);
        assert_eq!(fs::read(root.join("kept-partial")).unwrap(), bytes);
        close(jobs, root);
        continue_work(root, &marker, "file");
    });
}
#[test]
fn consumed_disposal_keeps_new_occupants_in_original_and_holding_namespaces() {
    for namespace in ["original", "holding"] {
        for same_inode in [false, true] {
            fixture("written", |root, marker| {
                fs::write(marker.output.join("a-done"), b"Changed completed output").unwrap();
                fs::write(root.join("a-done"), b"Changed completed source").unwrap();
                let kept = root.join("kept-original-inode");
                fs::hard_link(work(&marker).payload(), &kept).unwrap();
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "operations::transfer::stage_boundary::disposal_child",
                    ])
                    .env("CANOPA_STAGE_DISPOSE_ROOT", root)
                    .env("CANOPA_STAGE_DISPOSE_POINT", "after-unlink")
                    .status()
                    .unwrap();
                assert_eq!(status.signal(), Some(9));
                let cut: Cut =
                    serde_json::from_slice(&fs::read(root.join("disposal-phase.json")).unwrap())
                        .unwrap();
                let payload = if namespace == "original" {
                    cut.disposal.original.payload()
                } else {
                    cut.disposal.holding.payload()
                };
                if same_inode {
                    fs::hard_link(&kept, &payload).unwrap();
                } else {
                    fs::write(&payload, b"New foreign data after unlink").unwrap();
                }
                let bytes = fs::read(&payload).unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                    assert_eq!(fs::read(&payload).unwrap(), bytes);
                    unchanged(root, &marker);
                    close(jobs, root);
                }
                let jobs = start(root);
                copy_item(&jobs, &payload, &root.join("recovered-new-entry"), &bytes);
                fs::rename(&payload, root.join("kept-new-entry")).unwrap();
                assert_eq!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(fs::read(root.join("kept-new-entry")).unwrap(), bytes);
                close(jobs, root);
                continue_work(root, &marker, "file");
            });
        }
    }
}
#[test]
fn refused_or_ignored_phase_writes_and_retirement_keep_the_original_binding_atomic() {
    for point in ["refuse-consumed", "ignore-consumed", "refuse-retirement"] {
        fixture_cut(point, "file", |root, marker, cut| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            for _ in 0..2 {
                let pending: String = db
                    .query_row("SELECT body FROM pending", [], |r| r.get(0))
                    .unwrap();
                let binding: String = db
                    .query_row("SELECT body FROM stage_intents WHERE lane=0", [], |r| {
                        r.get(0)
                    })
                    .unwrap();
                assert_eq!(pending, cut.pending);
                assert_eq!(binding, cut.binding);
                if point == "refuse-retirement" {
                    assert!(!cut.disposal.original.directory.exists());
                    assert!(!cut.disposal.holding.directory.exists());
                } else {
                    assert_eq!(
                        fs::read(cut.disposal.holding.payload()).unwrap(),
                        vec![b'x'; 64 * 1024]
                    );
                }
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                unchanged(root, &marker);
                close(jobs, root);
            }
            db.execute_batch("DROP TRIGGER refuse_disposal_write;")
                .unwrap();
            drop(db);
            let jobs = start(root);
            assert_eq!(reconcile(&jobs).phase, Phase::Finished);
            close(jobs, root);
            continue_work(root, &marker, "file");
        });
    }
}
#[test]
fn disposal_requires_original_parent_and_both_work_roots_before_unlinking() {
    for changed in ["parent", "original", "holding"] {
        fixture_cut("claimed", "file", |root, marker, cut| {
            let replaced = match changed {
                "parent" => marker.output.clone(),
                "original" => cut.disposal.original.directory.clone(),
                _ => cut.disposal.holding.directory.clone(),
            };
            let parked = root.join("parked-original-root");
            fs::rename(&replaced, &parked).unwrap();
            fs::create_dir(&replaced).unwrap();
            let foreign = replaced.join("folder");
            fs::write(&foreign, b"Foreign replacement root").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(fs::read(&foreign).unwrap(), b"Foreign replacement root");
                let actual = if changed == "holding" {
                    parked.join("folder")
                } else if changed == "parent" {
                    parked
                        .join(cut.disposal.holding.directory.file_name().unwrap())
                        .join("folder")
                } else {
                    cut.disposal.holding.payload()
                };
                assert_eq!(fs::read(actual).unwrap(), vec![b'x'; 64 * 1024]);
                close(jobs, root);
            }
            let foreign_parked = root.join("kept-foreign-root");
            fs::rename(&replaced, &foreign_parked).unwrap();
            fs::rename(&parked, &replaced).unwrap();
            let jobs = start(root);
            assert_eq!(reconcile(&jobs).phase, Phase::Finished);
            assert_eq!(
                fs::read(foreign_parked.join("folder")).unwrap(),
                b"Foreign replacement root"
            );
            close(jobs, root);
            continue_work(root, &marker, "file");
        });
    }
}
#[test]
fn changed_disposal_journals_bindings_and_tasks_refuse_before_filesystem_mutation() {
    for fault in [
        "phase",
        "binding-missing",
        "binding-digest",
        "task-progress",
    ] {
        fixture_cut("claimed", "file", |root, marker, cut| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            let progress: Vec<u8> = db
                .query_row(
                    "SELECT progress_sha256 FROM request_transfer_cursors WHERE request=?1",
                    [marker.request],
                    |r| r.get(0),
                )
                .unwrap();
            match fault {
                "phase" => {
                    let mut changed: serde_json::Value =
                        serde_json::from_str(&cut.pending).unwrap();
                    changed["disposal"]["phase"] = "consumed".into();
                    db.execute("UPDATE pending SET body=?1", [changed.to_string()])
                        .unwrap();
                }
                "binding-missing" => {
                    db.execute("DELETE FROM stage_intents WHERE lane=0", [])
                        .unwrap();
                }
                "binding-digest" => {
                    let mut changed: serde_json::Value =
                        serde_json::from_str(&cut.binding).unwrap();
                    changed["digest"] = serde_json::json!(vec![0; 32]);
                    db.execute(
                        "UPDATE stage_intents SET body=?1 WHERE lane=0",
                        [changed.to_string()],
                    )
                    .unwrap();
                }
                _ => {
                    db.execute("UPDATE request_transfer_cursors SET progress_sha256=zeroblob(32) WHERE request=?1", [marker.request]).unwrap();
                }
            }
            for _ in 0..2 {
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(
                    fs::read(cut.disposal.holding.payload()).unwrap(),
                    vec![b'x'; 64 * 1024]
                );
                unchanged(root, &marker);
                close(jobs, root);
            }
            db.execute("UPDATE pending SET body=?1", [&cut.pending])
                .unwrap();
            db.execute(
                "INSERT OR REPLACE INTO stage_intents VALUES(0,?1)",
                [&cut.binding],
            )
            .unwrap();
            db.execute(
                "UPDATE request_transfer_cursors SET progress_sha256=?2 WHERE request=?1",
                rusqlite::params![marker.request, progress],
            )
            .unwrap();
            drop(db);
            let jobs = start(root);
            assert_eq!(reconcile(&jobs).phase, Phase::Finished);
            close(jobs, root);
            continue_work(root, &marker, "file");
        });
    }
}

#[test]
fn public_keep_preserves_both_disposal_roots_and_resumes_without_external_repair() {
    fixture_cut("consumed", "file", |root, marker, cut| {
        use std::os::unix::ffi::OsStringExt;
        let extra = std::ffi::OsString::from_vec(b"extra-\xfd".to_vec());
        fs::write(cut.disposal.original.payload(), b"New original namespace").unwrap();
        fs::write(
            cut.disposal.holding.directory.join(&extra),
            b"Other retained data",
        )
        .unwrap();
        let jobs = start(root);
        let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
        let item = page
            .items
            .into_iter()
            .find(|i| {
                i.entry
                    .as_ref()
                    .is_some_and(|e| e.path == cut.disposal.holding.payload())
            })
            .unwrap();
        assert!(
            item.can_keep_in_recovery(),
            "disposal data has no public Keep resolution"
        );
        let id = receive(
            jobs.submit(
                Request::KeepRecovery {
                    item: Box::new(item),
                },
                None,
            )
            .unwrap(),
        )
        .unwrap();
        let result = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id && !j.phase.holds_application())
        });
        assert_eq!(result.phase, Phase::Finished, "{result:?}");
        assert_eq!(
            fs::read(cut.disposal.original.payload()).unwrap(),
            b"New original namespace"
        );
        assert_eq!(
            fs::read(cut.disposal.holding.payload()).unwrap(),
            vec![b'x'; 64 * 1024]
        );
        assert_eq!(
            fs::read(cut.disposal.holding.directory.join(&extra)).unwrap(),
            b"Other retained data"
        );
        close(jobs, root);
        for attempt in 0..2 {
            let jobs = start(root);
            assert!(!receive(jobs.needs_recovery_review().unwrap()).unwrap());
            let original = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == marker.request)
                .unwrap();
            assert_eq!(original.phase, Phase::Stopped, "{original:?}");
            assert!(original.transfer_actions.remaining);
            let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
            let item = page
                .items
                .into_iter()
                .find(|i| {
                    i.entry
                        .as_ref()
                        .is_some_and(|e| e.path == cut.disposal.holding.directory)
                })
                .expect("whole retained work folder is exposed by Recovery");
            let destination = root.join(format!("recovered-work-{attempt}"));
            let id = receive(
                jobs.submit(
                    Request::Recover {
                        item: Box::new(item),
                        destination: destination.clone(),
                    },
                    None,
                )
                .unwrap(),
            )
            .unwrap();
            let copied = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id && !j.phase.holds_application())
            });
            assert_eq!(copied.phase, Phase::Finished, "{copied:?}");
            assert_eq!(
                fs::read(destination.join("folder")).unwrap(),
                vec![b'x'; 64 * 1024]
            );
            assert_eq!(
                fs::read(destination.join(&extra)).unwrap(),
                b"Other retained data"
            );
            unchanged(root, &marker);
            close(jobs, root);
        }
        continue_work(root, &marker, "file");
        let engine = OperationEngine::open(root.join("history")).unwrap();
        for index in 0..=HISTORY_LIMIT {
            engine
                .execute(
                    OperationRequest::CreateFolder {
                        path: root.join(format!("expire-{index}")),
                    },
                    &Cancellation::default(),
                )
                .unwrap();
        }
        let page = engine
            .recovery_items(None, &Cancellation::default())
            .unwrap();
        assert!(page.items.iter().any(|item| {
            item.entry
                .as_ref()
                .is_some_and(|entry| entry.path == cut.disposal.holding.directory)
        }));
        assert_eq!(
            fs::read(cut.disposal.holding.directory.join(&extra)).unwrap(),
            b"Other retained data"
        );
    });
}

fn review_item(jobs: &OperationJobs, path: &Path) -> crate::operations::RecoveryItem {
    receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.entry.as_ref().is_some_and(|entry| entry.path == path))
        .expect("interrupted entry is visible")
}
fn keep_item(
    jobs: &OperationJobs,
    item: crate::operations::RecoveryItem,
) -> crate::operation_jobs::JobSnapshot {
    let id = receive(
        jobs.submit(
            Request::KeepRecovery {
                item: Box::new(item),
            },
            None,
        )
        .unwrap(),
    )
    .unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && !job.phase.holds_application())
    })
}
fn journal_matches(root: &Path, cut: &Cut) {
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let pending: String = db
        .query_row("SELECT body FROM pending", [], |row| row.get(0))
        .unwrap();
    let binding: String = db
        .query_row("SELECT body FROM stage_intents WHERE lane=0", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(pending, cut.pending);
    assert_eq!(binding, cut.binding);
    let count: u64 = db
        .query_row("SELECT count(*) FROM garbage", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0, "failed Keep must roll back every retained owner");
}

#[test]
fn public_keep_requires_fresh_review_of_both_roots_and_never_deserializes_authority() {
    fixture_cut("consumed", "file", |root, marker, cut| {
        let jobs = start(root);
        let item = review_item(&jobs, &cut.disposal.holding.payload());
        let restored = serde_json::from_slice(&serde_json::to_vec(&item).unwrap()).unwrap();
        assert_ne!(keep_item(&jobs, restored).phase, Phase::Finished);
        journal_matches(root, &cut);
        fs::write(cut.disposal.original.payload(), b"New work after review").unwrap();
        assert_ne!(keep_item(&jobs, item).phase, Phase::Finished);
        journal_matches(root, &cut);
        let fresh = review_item(&jobs, &cut.disposal.holding.payload());
        assert_eq!(keep_item(&jobs, fresh).phase, Phase::Finished);
        assert_eq!(
            fs::read(cut.disposal.original.payload()).unwrap(),
            b"New work after review"
        );
        close(jobs, root);
        continue_work(root, &marker, "file");
    });
}

#[test]
fn public_keep_refuses_replacement_work_roots_and_preserves_both_generations() {
    fixture_cut("consumed", "file", |root, marker, cut| {
        let jobs = start(root);
        let item = review_item(&jobs, &cut.disposal.holding.payload());
        let original = root.join("original-holding");
        fs::rename(&cut.disposal.holding.directory, &original).unwrap();
        fs::create_dir(&cut.disposal.holding.directory).unwrap();
        fs::write(cut.disposal.holding.payload(), b"Replacement folder").unwrap();
        assert_ne!(keep_item(&jobs, item).phase, Phase::Finished);
        journal_matches(root, &cut);
        assert!(
            receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
                .unwrap()
                .items
                .iter()
                .all(|item| !item.can_keep_in_recovery())
        );
        assert_eq!(
            fs::read(original.join("folder")).unwrap(),
            vec![b'x'; 64 * 1024]
        );
        assert_eq!(
            fs::read(cut.disposal.holding.payload()).unwrap(),
            b"Replacement folder"
        );
        fs::rename(
            &cut.disposal.holding.directory,
            root.join("replacement-holding"),
        )
        .unwrap();
        fs::rename(&original, &cut.disposal.holding.directory).unwrap();
        let fresh = review_item(&jobs, &cut.disposal.holding.payload());
        assert_eq!(keep_item(&jobs, fresh).phase, Phase::Finished);
        close(jobs, root);
        continue_work(root, &marker, "file");
        assert_eq!(
            fs::read(root.join("replacement-holding/folder")).unwrap(),
            b"Replacement folder"
        );
    });
}

#[test]
fn public_keep_rolls_back_retention_when_writes_are_aborted_or_ignored() {
    for statement in [
        "CREATE TRIGGER refuse_keep BEFORE DELETE ON pending BEGIN SELECT RAISE(IGNORE); END;",
        "CREATE TRIGGER refuse_keep BEFORE DELETE ON pending BEGIN SELECT RAISE(ABORT,'private Keep refusal'); END;",
        "CREATE TRIGGER refuse_keep BEFORE INSERT ON garbage BEGIN SELECT RAISE(IGNORE); END;",
        "CREATE TRIGGER refuse_keep BEFORE INSERT ON garbage_paths BEGIN SELECT RAISE(IGNORE); END;",
        "CREATE TRIGGER refuse_keep BEFORE INSERT ON saved_stage_records BEGIN SELECT RAISE(IGNORE); END;",
        "CREATE TRIGGER refuse_keep BEFORE DELETE ON stage_intents BEGIN SELECT RAISE(IGNORE); END;",
    ] {
        fixture_cut("consumed", "file", |root, marker, cut| {
            let jobs = start(root);
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            db.execute_batch(statement).unwrap();
            let item = review_item(&jobs, &cut.disposal.holding.payload());
            let result = keep_item(&jobs, item);
            assert_ne!(result.phase, Phase::Finished, "{statement}: {result:?}");
            journal_matches(root, &cut);
            assert_eq!(
                fs::read(cut.disposal.holding.payload()).unwrap(),
                vec![b'x'; 64 * 1024]
            );
            db.execute_batch("DROP TRIGGER refuse_keep").unwrap();
            let fresh = review_item(&jobs, &cut.disposal.holding.payload());
            assert_eq!(keep_item(&jobs, fresh).phase, Phase::Finished);
            close(jobs, root);
            continue_work(root, &marker, "file");
        });
    }
}

#[test]
fn public_keep_preserves_earlier_disposal_phases_cross_moves_and_raw_links() {
    for (point, shape) in [
        ("reserved", "file"),
        ("after-claim", "file"),
        ("claimed", "cross"),
        ("consumed", "link"),
    ] {
        fixture_cut(point, shape, |root, marker, cut| {
            let retained = if fs::symlink_metadata(cut.disposal.original.payload()).is_ok() {
                cut.disposal.original.payload()
            } else {
                cut.disposal.holding.payload()
            };
            let version = fingerprint::Version::read(&retained).unwrap();
            let jobs = start(root);
            let item = review_item(&jobs, &cut.disposal.original.directory);
            assert!(item.can_keep_in_recovery());
            assert_eq!(keep_item(&jobs, item).phase, Phase::Finished);
            assert!(version.matches(&retained));
            close(jobs, root);
            continue_work(root, &marker, shape);
            assert!(version.matches(&retained));
        });
    }
}
