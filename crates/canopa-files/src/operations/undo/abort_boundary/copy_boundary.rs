//! Actual public Copy Undo worker death, including its completed work folder.
use super::*;
use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

pub(in crate::operations) fn cut(point: &str, removal: &undo::copied::Removal) {
    if std::env::var("CANOPA_COPY_UNDO_CUT").as_deref() != Ok(point) {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CANOPA_COPY_UNDO_ROOT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-copy-undo-")
    );
    assert_eq!(
        removal.stage.directory.parent(),
        Some(root.join("output").as_path())
    );
    fs::write(
        root.join("copy-undo-stage.json"),
        serde_json::to_vec(&removal.stage).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("copy-undo-removal.json"),
        serde_json::to_vec(removal).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private Copy Undo phase worker was not killed");
}

fn fixture(point: &str, check: impl FnOnce(&Path, &Stage)) {
    fixture_kind(point, "regular", check);
}
fn fixture_kind(point: &str, kind: &str, check: impl FnOnce(&Path, &Stage)) {
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
            .env("CANOPA_COPY_UNDO_CUT", point)
            .env("CANOPA_COPY_UNDO_KIND", kind)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9), "{point}");
        let stage =
            serde_json::from_slice(&fs::read(root.path().join("copy-undo-stage.json")).unwrap())
                .unwrap();
        check(root.path(), &stage);
    }));
    if let Err(error) = result {
        eprintln!("retained Copy Undo {point} root: {}", root.keep().display());
        std::panic::resume_unwind(error);
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
        .find(|item| {
            item.entry
                .as_ref()
                .is_some_and(|entry| entry.path == stage.directory)
        })
        .unwrap()
}

#[test]
fn original_copy_undo_resumes_or_compensates_every_unconsumed_and_completed_phase() {
    for point in ["prepared", "taking", "detached", "unlinked", "cleared"] {
        fixture(point, |root, stage| {
            fs::write(root.join("source/item"), b"Source edited after crash").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let done = reconcile(&jobs);
                assert_eq!(done.phase, Phase::Finished, "{point}: {done:?}");
                assert_eq!(
                    fs::read(root.join("source/item")).unwrap(),
                    b"Source edited after crash"
                );
                assert!(!stage.directory.exists());
                if point == "taking" {
                    assert_eq!(
                        fs::read(root.join("output/item")).unwrap(),
                        vec![b'x'; 128 * 1024]
                    );
                    assert!(jobs.snapshot().undo.is_some());
                } else {
                    assert!(!root.join("output/item").exists());
                    assert!(jobs.snapshot().undo.is_none());
                }
                close(jobs, root);
            }
            if point == "taking" {
                let jobs = start(root);
                let expected = jobs.snapshot().undo.unwrap().id;
                let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
                assert_eq!(terminal(&jobs, id).phase, Phase::Finished);
                assert!(!root.join("output/item").exists());
                close(jobs, root);
            }
        });
    }
}

#[test]
fn canonical_formatting_remains_compatible_and_changed_history_refuses_copy_undo() {
    for changed in [false, true] {
        fixture("detached", |root, stage| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            let before: String = db
                .query_row("SELECT body FROM changes", [], |r| r.get(0))
                .unwrap();
            let edited = if changed {
                let mut change: Change = serde_json::from_str(&before).unwrap();
                let Change::Copied { path, .. } = &mut change else {
                    unreachable!();
                };
                path.set_file_name("different canonical item");
                serde_json::to_string(&change).unwrap()
            } else {
                serde_json::to_string_pretty(
                    &serde_json::from_str::<serde_json::Value>(&before).unwrap(),
                )
                .unwrap()
            };
            assert_ne!(edited, before);
            db.execute("UPDATE changes SET body=?1", [&edited]).unwrap();
            let jobs = start(root);
            let done = reconcile(&jobs);
            if changed {
                assert_ne!(done.phase, Phase::Finished);
                assert_eq!(fs::read(stage.payload()).unwrap(), vec![b'x'; 128 * 1024]);
                db.execute("UPDATE changes SET body=?1", [&before]).unwrap();
                assert_eq!(reconcile(&jobs).phase, Phase::Finished);
            } else {
                assert_eq!(done.phase, Phase::Finished);
            }
            assert!(!stage.directory.exists());
            assert!(!root.join("output/item").exists());
            assert_eq!(
                fs::read(root.join("source/item")).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            close(jobs, root);
        });
    }
}

#[test]
fn consumed_copy_undo_names_are_never_repeated_and_uncertain_payloads_survive_recovery() {
    for point in ["renamed", "consumed", "unlinked"] {
        for original in [false, true] {
            if point == "unlinked" && original {
                continue;
            }
            fixture(point, |root, stage| {
                let payload = stage.payload();
                if point == "renamed" {
                    if original {
                        fs::hard_link(&payload, root.join("output/item")).unwrap();
                    } else {
                        fs::write(root.join("output/item"), b"New source name").unwrap();
                    }
                } else {
                    if original {
                        let identity = Identity::read(&payload).unwrap();
                        fs::rename(&payload, root.join("held-output")).unwrap();
                        fs::write(&payload, b"Temporary occupant").unwrap();
                        fs::remove_file(&payload).unwrap();
                        fs::rename(root.join("held-output"), &payload).unwrap();
                        assert!(identity.matches(&payload));
                    } else {
                        fs::write(&payload, b"New payload name").unwrap();
                    }
                    fs::write(root.join("output/item"), b"New source name").unwrap();
                }
                let bytes = fs::read(&payload).unwrap();
                let output = fs::read(root.join("output/item")).unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                    assert_eq!(fs::read(&payload).unwrap(), bytes);
                    assert_eq!(fs::read(root.join("output/item")).unwrap(), output);
                    assert!(root_item(&jobs, stage).can_keep_in_recovery());
                    close(jobs, root);
                }
                let jobs = start(root);
                assert_eq!(keep(&jobs, root_item(&jobs, stage)).phase, Phase::Finished);
                assert_eq!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(fs::read(&payload).unwrap(), bytes);
                assert_eq!(fs::read(root.join("output/item")).unwrap(), output);
                close(jobs, root);
            });
        }
    }
}

#[test]
fn copy_undo_refuses_replacement_roots_and_parents_and_preserves_replacement_items() {
    for replaced in ["root", "parent", "selected", "payload"] {
        fixture(
            if replaced == "payload" {
                "detached"
            } else {
                "prepared"
            },
            |root, stage| {
                let held = root.join("original-copy");
                match replaced {
                    "root" => {
                        fs::rename(&stage.directory, root.join("original-stage")).unwrap();
                        fs::create_dir(&stage.directory).unwrap();
                        fs::write(stage.payload(), b"Replacement root work").unwrap();
                    }
                    "parent" => {
                        fs::rename(root.join("output"), root.join("original-parent")).unwrap();
                        fs::create_dir(root.join("output")).unwrap();
                        fs::create_dir(&stage.directory).unwrap();
                        fs::write(stage.payload(), b"Replacement parent work").unwrap();
                        fs::write(root.join("output/item"), b"Replacement output").unwrap();
                    }
                    "selected" => {
                        fs::rename(root.join("output/item"), &held).unwrap();
                        fs::write(root.join("output/item"), b"Replacement output").unwrap();
                    }
                    "payload" => {
                        fs::rename(stage.payload(), &held).unwrap();
                        fs::write(stage.payload(), b"Replacement payload").unwrap();
                    }
                    _ => unreachable!(),
                }
                for _ in 0..2 {
                    let jobs = start(root);
                    let done = reconcile(&jobs);
                    if replaced == "selected" {
                        assert_eq!(done.phase, Phase::Finished, "{done:?}");
                        assert!(!stage.directory.exists());
                        assert_eq!(
                            fs::read(root.join("output/item")).unwrap(),
                            b"Replacement output"
                        );
                        assert!(jobs.snapshot().undo.is_some());
                    } else {
                        assert_ne!(done.phase, Phase::Finished);
                        assert!(stage.directory.exists());
                        assert!(
                            fs::read(stage.payload())
                                .unwrap()
                                .starts_with(b"Replacement")
                        );
                    }
                    if held.exists() {
                        assert_eq!(fs::read(&held).unwrap(), vec![b'x'; 128 * 1024]);
                    }
                    assert_eq!(
                        fs::read(root.join("source/item")).unwrap(),
                        vec![b'x'; 128 * 1024]
                    );
                    close(jobs, root);
                }
            },
        );
    }
}

fn owner_rows(root: &Path) -> (String, String) {
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    (
        db.query_row("SELECT body FROM pending", [], |r| r.get(0))
            .unwrap(),
        db.query_row("SELECT body FROM copy_undo", [], |r| r.get(0))
            .unwrap(),
    )
}
#[test]
fn copy_undo_phase_and_completion_writes_roll_back_to_the_exact_original_owner() {
    for (point, sql) in [
        ("prepared", "BEFORE UPDATE ON pending"),
        ("prepared", "BEFORE UPDATE ON copy_undo"),
        ("cleared", "BEFORE DELETE ON changes"),
        ("cleared", "BEFORE DELETE ON copy_undo"),
        ("cleared", "BEFORE INSERT ON undo_cleanup"),
    ] {
        for fail in ["IGNORE", "ABORT,'private Copy Undo write refusal'"] {
            fixture(point, |root, stage| {
                let before = owner_rows(root);
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                db.execute_batch(&format!(
                    "CREATE TRIGGER copy_undo_fault {sql} BEGIN SELECT RAISE({fail}); END;"
                ))
                .unwrap();
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(owner_rows(root), before);
                assert!(stage.identity.matches(&stage.directory));
                assert_eq!(fs::read_dir(&stage.directory).unwrap().count(), 0);
                if point == "prepared" {
                    assert_eq!(
                        fs::read(root.join("output/item")).unwrap(),
                        vec![b'x'; 128 * 1024]
                    );
                }
                close(jobs, root);
                db.execute_batch("DROP TRIGGER copy_undo_fault;").unwrap();
                let jobs = start(root);
                assert_eq!(reconcile(&jobs).phase, Phase::Finished);
                assert!(!stage.directory.exists());
                assert!(!root.join("output/item").exists());
                close(jobs, root);
            });
        }
    }
}

#[test]
fn copy_undo_refuses_missing_or_edited_independent_task_and_history_proofs() {
    for tamper in ["missing", "binding", "pending", "history"] {
        fixture("prepared", |root, stage| {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            match tamper {
                "missing" => {
                    db.execute("DELETE FROM copy_undo", []).unwrap();
                }
                "binding" => {
                    let body: String = db
                        .query_row("SELECT body FROM copy_undo", [], |r| r.get(0))
                        .unwrap();
                    let mut value: serde_json::Value = serde_json::from_str(&body).unwrap();
                    value["pending"][0] =
                        serde_json::json!(255 - value["pending"][0].as_u64().unwrap());
                    db.execute("UPDATE copy_undo SET body=?1", [value.to_string()])
                        .unwrap();
                }
                "pending" => {
                    let body: String = db
                        .query_row("SELECT body FROM pending", [], |r| r.get(0))
                        .unwrap();
                    let mut value: Pending = serde_json::from_str(&body).unwrap();
                    let Pending::UndoCopied { removal } = &mut value else {
                        panic!()
                    };
                    removal.phase = undo::copied::Phase::Preserved;
                    db.execute(
                        "UPDATE pending SET body=?1",
                        [store::encode(&value).unwrap()],
                    )
                    .unwrap();
                }
                "history" => {
                    let body: String = db
                        .query_row("SELECT body FROM changes", [], |r| r.get(0))
                        .unwrap();
                    let mut value: Change = serde_json::from_str(&body).unwrap();
                    let Change::Copied { path, .. } = &mut value else {
                        panic!()
                    };
                    *path = root.join("unrelated");
                    db.execute(
                        "UPDATE changes SET body=?1",
                        [store::encode(&value).unwrap()],
                    )
                    .unwrap();
                }
                _ => unreachable!(),
            }
            for _ in 0..2 {
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                assert!(stage.identity.matches(&stage.directory));
                assert_eq!(fs::read_dir(&stage.directory).unwrap().count(), 0);
                assert_eq!(
                    fs::read(root.join("output/item")).unwrap(),
                    vec![b'x'; 128 * 1024]
                );
                assert!(!root_item(&jobs, stage).can_keep_in_recovery());
                close(jobs, root);
            }
        });
    }
}

#[test]
fn copy_undo_keep_requires_a_fresh_private_review_and_preserves_export_and_original_undo() {
    fixture("prepared", |root, stage| {
        fs::write(stage.payload(), b"Unexpected work").unwrap();
        fs::create_dir(stage.directory.join("extra")).unwrap();
        fs::write(stage.directory.join("extra/item"), b"Extra work").unwrap();
        let jobs = start(root);
        assert_ne!(reconcile(&jobs).phase, Phase::Finished);
        let item = root_item(&jobs, stage);
        assert!(item.can_keep_in_recovery());
        let decoded: RecoveryItem =
            serde_json::from_slice(&serde_json::to_vec(&item).unwrap()).unwrap();
        assert!(!decoded.can_keep_in_recovery());
        fs::write(stage.directory.join("added"), b"Later work").unwrap();
        assert_ne!(keep(&jobs, item).phase, Phase::Finished);
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
        assert_eq!(
            fs::read(root.join("exported/folder")).unwrap(),
            b"Unexpected work"
        );
        assert_eq!(
            fs::read(root.join("exported/extra/item")).unwrap(),
            b"Extra work"
        );
        assert_eq!(
            fs::read(root.join("exported/added")).unwrap(),
            b"Later work"
        );
        let original_owner = owner_rows(root);
        for (sql, fail) in [
            ("BEFORE INSERT ON garbage", "IGNORE"),
            ("BEFORE INSERT ON garbage", "ABORT,'private Keep refusal'"),
            ("BEFORE DELETE ON copy_undo", "IGNORE"),
            ("BEFORE DELETE ON pending", "IGNORE"),
        ] {
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            db.execute_batch(&format!(
                "CREATE TRIGGER copy_keep_fault {sql} BEGIN SELECT RAISE({fail}); END;"
            ))
            .unwrap();
            assert_ne!(keep(&jobs, root_item(&jobs, stage)).phase, Phase::Finished);
            assert_eq!(owner_rows(root), original_owner);
            db.execute_batch("DROP TRIGGER copy_keep_fault;").unwrap();
        }
        assert_eq!(keep(&jobs, root_item(&jobs, stage)).phase, Phase::Finished);
        assert!(!root_item(&jobs, stage).can_keep_in_recovery());
        for _ in 0..2 {
            let expected = jobs.snapshot().undo.unwrap().id;
            let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
            assert_eq!(terminal(&jobs, id).phase, Phase::Finished);
        }
        assert!(!root.join("exported").exists());
        assert!(!root.join("output/item").exists());
        assert_eq!(fs::read(stage.payload()).unwrap(), b"Unexpected work");
        assert_eq!(
            fs::read(stage.directory.join("added")).unwrap(),
            b"Later work"
        );
        assert_eq!(
            fs::read(root.join("source/item")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        close(jobs, root);
    });
}

#[test]
fn copy_undo_restart_preserves_raw_symlink_names_and_targets() {
    let name = OsStr::from_bytes(b"item-\xff");
    let target = Path::new(OsStr::from_bytes(b"../missing-\xfe"));
    for point in ["prepared", "detached", "consumed"] {
        fixture_kind(point, "symlink", |root, stage| {
            for _ in 0..2 {
                let jobs = start(root);
                let done = reconcile(&jobs);
                if point == "consumed" {
                    assert_ne!(done.phase, Phase::Finished);
                    assert_eq!(fs::read_link(stage.payload()).unwrap(), target);
                } else {
                    assert_eq!(done.phase, Phase::Finished, "{point}: {done:?}");
                    assert!(!stage.directory.exists());
                }
                assert_eq!(
                    fs::read_link(root.join("source").join(name)).unwrap(),
                    target
                );
                assert!(fs::symlink_metadata(root.join("output").join(name)).is_err());
                close(jobs, root);
            }
            if point == "consumed" {
                let jobs = start(root);
                assert_eq!(keep(&jobs, root_item(&jobs, stage)).phase, Phase::Finished);
                assert_eq!(fs::read_link(stage.payload()).unwrap(), target);
                close(jobs, root);
            }
        });
    }
}

#[test]
fn refused_initial_copy_undo_registration_keeps_the_copy_and_cleans_only_its_empty_created_root() {
    for table in ["pending", "copy_undo"] {
        for failure in ["IGNORE", "ABORT,'private registration refusal'"] {
            let root = tempfile::Builder::new()
                .prefix("canopa-copy-undo-")
                .tempdir()
                .unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::undo::abort_boundary::copy_boundary::copy_undo_child",
                ])
                .env("CANOPA_COPY_UNDO_ROOT", root.path())
                .env("CANOPA_COPY_UNDO_SAVE_TABLE", table)
                .env("CANOPA_COPY_UNDO_SAVE_FAILURE", failure)
                .status()
                .unwrap();
            if !status.success() {
                panic!(
                    "retained Copy Undo registration root: {}",
                    root.keep().display()
                );
            }
            assert_eq!(
                fs::read(root.path().join("output/item")).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            assert_eq!(fs::read_dir(root.path().join("output")).unwrap().count(), 1);
        }
    }
}

pub(in crate::operations) fn completed(stage: &Stage) {
    if std::env::var("CANOPA_COPY_UNDO_CUT").as_deref() != Ok("completed") {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CANOPA_COPY_UNDO_ROOT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-copy-undo-")
    );
    assert_eq!(
        stage.directory.parent(),
        Some(root.join("output").as_path())
    );
    assert!(stage.identity.matches(&stage.directory));
    assert_eq!(fs::read_dir(&stage.directory).unwrap().count(), 0);
    assert!(!root.join("output/item").exists());
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        vec![b'x'; 128 * 1024]
    );
    fs::write(
        root.join("copy-undo-stage.json"),
        serde_json::to_vec(stage).unwrap(),
    )
    .unwrap();
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let counts:(u64,u64,u64,u64)=db.query_row("SELECT (SELECT count(*) FROM pending),(SELECT count(*) FROM undo_work),(SELECT count(*) FROM undo_cleanup),(SELECT count(*) FROM changes)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    fs::write(
        root.join("copy-undo-owner-counts.json"),
        serde_json::to_vec(&counts).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private Copy Undo completion worker was not killed");
}

#[test]
#[ignore = "private public Copy Undo worker killed before completed-root cleanup"]
fn copy_undo_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_COPY_UNDO_ROOT").unwrap());
    fs::create_dir(root.join("source")).unwrap();
    fs::create_dir(root.join("output")).unwrap();
    let symlink = std::env::var("CANOPA_COPY_UNDO_KIND").as_deref() == Ok("symlink");
    let name = if symlink {
        OsStr::from_bytes(b"item-\xff")
    } else {
        OsStr::new("item")
    };
    let source = root.join("source").join(name);
    let output = root.join("output").join(name);
    if symlink {
        std::os::unix::fs::symlink(OsStr::from_bytes(b"../missing-\xfe"), &source).unwrap();
    } else {
        fs::write(&source, vec![b'x'; 128 * 1024]).unwrap();
    }
    let jobs = start(&root);
    let id = receive(
        jobs.submit(
            Request::Transfer(TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: root.join("output"),
            }),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    let done = terminal(&jobs, id);
    assert_eq!(done.phase, Phase::Finished, "{done:?}");
    if symlink {
        assert_eq!(
            fs::read_link(&output).unwrap(),
            fs::read_link(&source).unwrap()
        );
    } else {
        assert_eq!(fs::read(&output).unwrap(), vec![b'x'; 128 * 1024]);
    }
    if let Ok(table) = std::env::var("CANOPA_COPY_UNDO_SAVE_TABLE") {
        assert!(matches!(table.as_str(), "pending" | "copy_undo"));
        let failure = std::env::var("CANOPA_COPY_UNDO_SAVE_FAILURE").unwrap();
        assert!(matches!(
            failure.as_str(),
            "IGNORE" | "ABORT,'private registration refusal'"
        ));
        rusqlite::Connection::open(root.join("history")).unwrap().execute_batch(
            &format!("CREATE TRIGGER copy_undo_initial_fault BEFORE INSERT ON {table} BEGIN SELECT RAISE({failure}); END;")
        ).unwrap();
    }
    let expected = jobs.snapshot().undo.unwrap().id;
    let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
    let done = terminal(&jobs, id);
    if std::env::var_os("CANOPA_COPY_UNDO_SAVE_TABLE").is_some() {
        assert_ne!(done.phase, Phase::Finished);
        assert_eq!(fs::read(&output).unwrap(), vec![b'x'; 128 * 1024]);
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        let counts: (u64,u64,u64) = db.query_row("SELECT (SELECT count(*) FROM pending),(SELECT count(*) FROM copy_undo),(SELECT count(*) FROM changes)", [], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(counts, (0, 0, 1));
        close(jobs, &root);
        return;
    }
    panic!("private Copy Undo completion boundary was not reached: {done:?}");
}

#[test]
fn completed_copy_undo_cleans_its_original_empty_root_after_restart_without_touching_new_work() {
    let root = tempfile::Builder::new()
        .prefix("canopa-copy-undo-")
        .tempdir()
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::undo::abort_boundary::copy_boundary::copy_undo_child",
            ])
            .env("CANOPA_COPY_UNDO_ROOT", root.path())
            .env("CANOPA_COPY_UNDO_CUT", "completed")
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let stage: Stage =
            serde_json::from_slice(&fs::read(root.path().join("copy-undo-stage.json")).unwrap())
                .unwrap();
        assert!(stage.identity.matches(&stage.directory));
        assert_eq!(fs::read_dir(&stage.directory).unwrap().count(), 0);
        fs::write(root.path().join("source/item"), b"Later source edits").unwrap();
        fs::write(
            root.path().join("output/item"),
            b"New output after completed Undo",
        )
        .unwrap();
        for _ in 0..2 {
            let jobs = start(root.path());
            let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
            let done = terminal(&jobs, id);
            assert_eq!(done.phase, Phase::Finished, "{done:?}");
            assert_eq!(
                fs::read(root.path().join("source/item")).unwrap(),
                b"Later source edits"
            );
            assert_eq!(
                fs::read(root.path().join("output/item")).unwrap(),
                b"New output after completed Undo"
            );
            assert!(jobs.snapshot().undo.is_none());
            assert!(
                !stage.directory.exists(),
                "completed Copy Undo must retain cleanup ownership until its original empty work folder is gone"
            );
            close(jobs, root.path());
        }
    }));
    if let Err(error) = checked {
        eprintln!("retained Copy Undo root: {}", root.keep().display());
        std::panic::resume_unwind(error);
    }
}
