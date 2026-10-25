//! Kill real Undo reconciliation between durable disposal phases.
use super::*;
mod rollback_checks;
pub(in crate::operations) fn after_raw_return() {
    if std::env::var("CANOPA_UNDO_RAW_RETURN_CUT").as_deref() != Ok("1") {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CANOPA_UNDO_DISPOSE_ROOT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    let (pending, binding) = rows(&root);
    fs::write(
        root.join("raw-return-cut.json"),
        serde_json::to_vec(&(pending, binding)).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private post-return worker was not killed");
}
pub(in crate::operations) fn before_disposal_open(stage: &Stage) {
    if std::env::var("CANOPA_UNDO_MOVE_COPY_BEFORE_OPEN").as_deref() != Ok("1") {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CANOPA_UNDO_DISPOSE_ROOT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert_eq!(
        stage.directory.parent(),
        Some(root.join("source").as_path())
    );
    let kept = root.join("kept-return-copy");
    if !kept.exists() {
        fs::rename(stage.payload(), kept).unwrap();
    }
}
use crate::operations::staging::disposal::Disposal;

#[derive(serde::Serialize, serde::Deserialize)]
struct Cut {
    disposal: Disposal,
    pending: String,
    binding: String,
}
fn rows(root: &Path) -> (String, String) {
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    (
        db.query_row("SELECT body FROM pending", [], |r| r.get(0))
            .unwrap(),
        db.query_row("SELECT body FROM undo_work", [], |r| r.get(0))
            .unwrap(),
    )
}
pub(in crate::operations) fn point(point: &str, disposal: &Disposal) {
    let Ok(wanted) = std::env::var("CANOPA_UNDO_DISPOSE_POINT") else {
        return;
    };
    let observed = if matches!(
        wanted.as_str(),
        "return-refuse-consumed" | "return-ignore-consumed" | "return-ignore-binding"
    ) {
        "return-claimed"
    } else if matches!(
        wanted.as_str(),
        "return-refuse-receipt" | "return-ignore-receipt"
    ) {
        "return-after-rename"
    } else if matches!(
        wanted.as_str(),
        "return-refuse-retirement" | "return-ignore-retirement"
    ) {
        "return-roots-gone"
    } else if matches!(
        wanted.as_str(),
        "refuse-consumed" | "ignore-consumed" | "ignore-binding-phase"
    ) {
        "claimed"
    } else if wanted.starts_with("edit-") {
        "consumed"
    } else if wanted.contains("retirement") {
        "roots-gone"
    } else {
        &wanted
    };
    if observed != point {
        return;
    }
    assert!(disposal.undo.is_some());
    let root = PathBuf::from(std::env::var_os("CANOPA_UNDO_DISPOSE_ROOT").unwrap());
    let marker = root.join("undo-disposal-cut.json");
    if marker.exists() {
        return;
    }
    let (pending, binding) = rows(&root);
    assert_eq!(
        serde_json::from_str::<Pending>(&pending)
            .map(|p| store::encode(&p).unwrap())
            .unwrap(),
        store::encode(&Pending::Disposing {
            disposal: Box::new(disposal.clone())
        })
        .unwrap()
    );
    fs::write(
        &marker,
        serde_json::to_vec(&Cut {
            disposal: disposal.clone(),
            pending,
            binding,
        })
        .unwrap(),
    )
    .unwrap();
    if wanted == "edit-payload-consumed" {
        fs::write(
            disposal.holding.payload(),
            b"New partial edits after consumption",
        )
        .unwrap();
        return;
    }
    if wanted == "edit-source-consumed" {
        let Some(Pending::Undo {
            change: Change::SourceRemoved { current, .. },
            ..
        }) = disposal.undo.as_deref()
        else {
            unreachable!();
        };
        fs::write(current, b"New moved destination edits after consumption").unwrap();
        return;
    }
    let trigger = match wanted.as_str() {
        "return-refuse-consumed" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE UPDATE ON pending WHEN json_extract(NEW.body,'$.disposal.returned.phase')='consumed' BEGIN SELECT RAISE(ABORT,'private Undo return refusal'); END;",
        ),
        "return-ignore-consumed" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE UPDATE ON pending WHEN json_extract(NEW.body,'$.disposal.returned.phase')='consumed' BEGIN SELECT RAISE(IGNORE); END;",
        ),
        "return-ignore-binding" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE UPDATE ON undo_work BEGIN SELECT RAISE(IGNORE); END;",
        ),
        "return-refuse-receipt" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE UPDATE ON pending WHEN json_extract(NEW.body,'$.disposal.returned.phase')='returned' BEGIN SELECT RAISE(ABORT,'private Undo return receipt refusal'); END;",
        ),
        "return-ignore-receipt" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE UPDATE ON pending WHEN json_extract(NEW.body,'$.disposal.returned.phase')='returned' BEGIN SELECT RAISE(IGNORE); END;",
        ),
        "return-refuse-retirement" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE DELETE ON pending BEGIN SELECT RAISE(ABORT,'private Undo return retirement refusal'); END;",
        ),
        "return-ignore-retirement" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE DELETE ON pending BEGIN SELECT RAISE(IGNORE); END;",
        ),
        "refuse-consumed" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE UPDATE ON pending WHEN json_extract(NEW.body,'$.disposal.phase')='consumed' BEGIN SELECT RAISE(ABORT,'private Undo disposal refusal'); END;",
        ),
        "ignore-consumed" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE UPDATE ON pending WHEN json_extract(NEW.body,'$.disposal.phase')='consumed' BEGIN SELECT RAISE(IGNORE); END;",
        ),
        "ignore-binding-phase" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE UPDATE ON undo_work BEGIN SELECT RAISE(IGNORE); END;",
        ),
        "refuse-retirement" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE DELETE ON pending BEGIN SELECT RAISE(ABORT,'private Undo retirement refusal'); END;",
        ),
        "ignore-retirement" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE DELETE ON pending BEGIN SELECT RAISE(IGNORE); END;",
        ),
        "ignore-binding-retirement" => Some(
            "CREATE TRIGGER private_undo_disposal BEFORE DELETE ON undo_work BEGIN SELECT RAISE(IGNORE); END;",
        ),
        _ => None,
    };
    if let Some(sql) = trigger {
        rusqlite::Connection::open(root.join("history"))
            .unwrap()
            .execute_batch(sql)
            .unwrap();
    } else {
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
        panic!("private Undo disposal worker was not killed");
    }
}
fn reconcile(jobs: &OperationJobs) -> crate::operation_jobs::JobSnapshot {
    let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    terminal(jobs, id)
}
#[test]
#[ignore = "private public Undo reconciliation worker killed or refused at a disposal boundary"]
fn undo_disposal_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_UNDO_DISPOSE_ROOT").unwrap());
    let jobs = start(&root);
    let done = reconcile(&jobs);
    if std::env::var("CANOPA_UNDO_DISPOSE_EXPECT_FINISHED").as_deref() == Ok("1") {
        assert_eq!(done.phase, Phase::Finished, "{done:?}");
    } else {
        assert_ne!(done.phase, Phase::Finished, "{done:?}");
    }
    close(jobs, &root);
}
fn cut(root: &Path, point: &str) -> Cut {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "operations::undo::abort_boundary::disposal_boundary::undo_disposal_child",
        ])
        .env("CANOPA_UNDO_DISPOSE_ROOT", root)
        .env("CANOPA_UNDO_DISPOSE_POINT", point)
        .status()
        .unwrap();
    if point.contains("refuse-") || point.contains("ignore-") || point.starts_with("edit-") {
        assert!(status.success(), "{point}: {status}");
    } else {
        assert_eq!(status.signal(), Some(9), "{point}: {status}");
    }
    serde_json::from_slice(&fs::read(root.join("undo-disposal-cut.json")).unwrap()).unwrap()
}
fn original(root: &Path, output: &Path) {
    assert!(!root.join("source/item").exists());
    assert_eq!(
        fs::read(output.join("item")).unwrap(),
        vec![b'x'; 128 * 1024]
    );
}
fn retry(root: &Path, output: &Path) {
    let jobs = start(root);
    let expected = jobs.snapshot().undo.unwrap().id;
    let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
    let done = terminal(&jobs, id);
    assert_eq!(done.phase, Phase::Finished, "{done:?}");
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        vec![b'x'; 128 * 1024]
    );
    assert!(!output.join("item").exists());
    close(jobs, root);
}
#[test]
fn undo_disposal_deaths_retire_only_unfinished_work_and_keep_the_original_undo() {
    for point in [
        "reserved",
        "after-claim",
        "claimed",
        "after-unlink",
        "holding-gone",
        "roots-gone",
    ] {
        fixture(|root, output, _stage| {
            let cut = cut(root, point);
            for _ in 0..2 {
                let jobs = start(root);
                let done = reconcile(&jobs);
                assert_eq!(done.phase, Phase::Finished, "{point}: {done:?}");
                assert!(!cut.disposal.original.directory.exists());
                assert!(!cut.disposal.holding.directory.exists());
                original(root, output);
                close(jobs, root);
            }
            retry(root, output);
        });
    }
}
#[test]
fn consumed_undo_disposal_preserves_unlinked_or_reintroduced_entries_until_keep() {
    for point in ["consumed", "after-unlink"] {
        for same_inode in [false, true] {
            fixture(|root, output, stage| {
                let held = root.join("held-partial");
                fs::hard_link(stage.payload(), &held).unwrap();
                let cut = cut(root, point);
                let payload = cut.disposal.holding.payload();
                if point == "after-unlink" {
                    if same_inode {
                        fs::hard_link(&held, &payload).unwrap();
                    } else {
                        fs::write(&payload, b"New contents after the original unlink").unwrap();
                    }
                }
                let bytes = fs::read(&payload).unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                    assert_eq!(fs::read(&payload).unwrap(), bytes);
                    original(root, output);
                    close(jobs, root);
                }
                let jobs = start(root);
                let item = partial(&jobs, &bytes);
                assert!(item.can_keep_in_recovery());
                assert_eq!(keep(&jobs, item).phase, Phase::Finished);
                assert_eq!(fs::read(&payload).unwrap(), bytes);
                close(jobs, root);
                retry(root, output);
                let jobs = start(root);
                drop(partial(&jobs, &bytes));
                close(jobs, root);
            });
        }
    }
}
#[test]
fn undo_disposal_refuses_identical_byte_source_replacements_and_missing_sources() {
    for replacement in ["missing", "entry", "parent"] {
        fixture(|root, output, stage| {
            let held = if replacement == "parent" {
                output.with_file_name(format!(
                    "{}-held",
                    output.file_name().unwrap().to_string_lossy()
                ))
            } else {
                output.join("held-original")
            };
            if replacement == "parent" {
                fs::write(
                    root.join("held-source-parent.txt"),
                    held.as_os_str().as_encoded_bytes(),
                )
                .unwrap();
                eprintln!(
                    "private Undo disposal held source parent: {}",
                    held.display()
                );
                fs::rename(output, &held).unwrap();
                fs::create_dir(output).unwrap();
            } else {
                fs::rename(output.join("item"), &held).unwrap();
            }
            if replacement != "missing" {
                fs::write(output.join("item"), vec![b'x'; 128 * 1024]).unwrap();
            }
            for _ in 0..2 {
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(fs::read(stage.payload()).unwrap(), vec![b'x'; 64 * 1024]);
                assert!(partial(&jobs, &vec![b'x'; 64 * 1024]).can_keep_in_recovery());
                close(jobs, root);
            }
            if replacement == "parent" {
                fs::remove_file(output.join("item")).unwrap();
                fs::remove_dir(output).unwrap();
                fs::rename(&held, output).unwrap();
            } else {
                if replacement != "missing" {
                    fs::remove_file(output.join("item")).unwrap();
                }
                fs::rename(&held, output.join("item")).unwrap();
            }
            let jobs = start(root);
            assert_eq!(reconcile(&jobs).phase, Phase::Finished);
            close(jobs, root);
            retry(root, output);
        });
    }
}
#[test]
fn late_undo_disposal_edits_never_gain_a_new_unlink_after_consumption() {
    for point in ["edit-payload-consumed", "edit-source-consumed"] {
        fixture(|root, output, _stage| {
            let cut = cut(root, point);
            let payload = cut.disposal.holding.payload();
            let partial_bytes = fs::read(&payload).unwrap();
            let source_bytes = fs::read(output.join("item")).unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(fs::read(&payload).unwrap(), partial_bytes);
                assert_eq!(fs::read(output.join("item")).unwrap(), source_bytes);
                close(jobs, root);
            }
            let jobs = start(root);
            let item = partial(&jobs, &partial_bytes);
            assert_eq!(keep(&jobs, item).phase, Phase::Finished);
            let expected = jobs.snapshot().undo.unwrap().id;
            let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
            if point == "edit-source-consumed" {
                let decision = wait(|| {
                    jobs.snapshot()
                        .jobs
                        .into_iter()
                        .find(|j| j.id == id && j.phase == Phase::Waiting)
                        .and_then(|j| j.decision)
                });
                jobs.resolve(id, decision.generation, UndoChoice::ReturnItem)
                    .unwrap();
            }
            let done = terminal(&jobs, id);
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            assert_eq!(fs::read(root.join("source/item")).unwrap(), source_bytes);
            assert!(!output.join("item").exists());
            assert_eq!(fs::read(&payload).unwrap(), partial_bytes);
            close(jobs, root);
        });
    }
}
#[test]
fn failed_undo_disposal_saves_preserve_the_exact_phase_and_binding() {
    for point in [
        "refuse-consumed",
        "ignore-consumed",
        "ignore-binding-phase",
        "refuse-retirement",
        "ignore-retirement",
        "ignore-binding-retirement",
    ] {
        fixture(|root, output, _stage| {
            let cut = cut(root, point);
            for _ in 0..2 {
                assert_eq!(
                    rows(root),
                    (cut.pending.clone(), cut.binding.clone()),
                    "{point}"
                );
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished, "{point}");
                original(root, output);
                close(jobs, root);
            }
            rusqlite::Connection::open(root.join("history"))
                .unwrap()
                .execute_batch("DROP TRIGGER private_undo_disposal;")
                .unwrap();
            let jobs = start(root);
            assert_eq!(reconcile(&jobs).phase, Phase::Finished);
            close(jobs, root);
            retry(root, output);
        });
    }
}
