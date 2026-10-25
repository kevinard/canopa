use super::*;
use std::os::unix::process::ExitStatusExt;

#[test]
fn cleanup_never_deletes_completed_output_returned_to_its_former_work_name() {
    for recovery_lane in [false, true] {
        for edited in [false, true] {
            let root = tempfile::Builder::new()
                .prefix("canopa-cleanup-returned-")
                .tempdir()
                .unwrap();
            let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "operations::staging::cleanup_boundary::postappend_child",
                    ])
                    .env("CANOPA_POSTAPPEND_ROOT", root.path())
                    .env(
                        "CANOPA_POSTAPPEND_RECOVERY",
                        if recovery_lane { "1" } else { "0" },
                    )
                    .status()
                    .unwrap();
                assert_eq!(status.signal(), Some(9));
                let stage: PathBuf =
                    serde_json::from_slice(&fs::read(root.path().join("stage.json")).unwrap())
                        .unwrap();
                let output = root.path().join("output");
                let identity = Identity::read(&output).unwrap();
                let returned = stage.join("folder");
                fs::rename(&output, &returned).unwrap();
                let bytes = if edited {
                    b"Newer completed work".as_slice()
                } else {
                    b"Original source".as_slice()
                };
                if edited {
                    fs::write(&returned, bytes).unwrap();
                }
                for _ in 0..2 {
                    let engine = OperationEngine::open(root.path().join("history")).unwrap();
                    assert!(
                        identity.matches(&returned),
                        "cleanup consumed already completed output returned to its work name"
                    );
                    assert_eq!(fs::read(&returned).unwrap(), bytes);
                    assert!(
                        !engine
                            .interrupted_operations(&Cancellation::default())
                            .unwrap()
                            .is_empty()
                    );
                    assert_eq!(
                        engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                        1
                    );
                    assert_eq!(
                        fs::read(root.path().join("source")).unwrap(),
                        b"Original source"
                    );
                }
                fs::remove_file(&returned).unwrap();
                let engine = OperationEngine::open(root.path().join("history")).unwrap();
                assert!(
                    engine
                        .interrupted_operations(&Cancellation::default())
                        .unwrap()
                        .is_empty()
                );
                assert!(!stage.exists());
                assert_eq!(
                    engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                    1
                );
            }));
            if let Err(panic) = checked {
                eprintln!(
                    "returned cleanup fixture lane={recovery_lane}/edited={edited} retained at {}",
                    root.keep().display()
                );
                std::panic::resume_unwind(panic);
            }
        }
    }
}

#[test]
fn killed_postappend_cleanup_keeps_its_step_and_newer_output_without_replay() {
    for recovery_lane in [false, true] {
        for replacement in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::staging::cleanup_boundary::postappend_child",
                ])
                .env("CANOPA_POSTAPPEND_ROOT", root.path())
                .env(
                    "CANOPA_POSTAPPEND_RECOVERY",
                    if recovery_lane { "1" } else { "0" },
                )
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let stage: PathBuf =
                serde_json::from_slice(&fs::read(root.path().join("stage.json")).unwrap()).unwrap();
            let output = root.path().join("output");
            let before = fs::metadata(&output).unwrap();
            fs::write(&output, b"Newer completed work").unwrap();
            if replacement {
                fs::write(stage.join("folder"), b"Foreign cleanup occupant").unwrap();
            }
            for _ in 0..2 {
                let engine = OperationEngine::open(root.path().join("history")).unwrap();
                let issues = engine
                    .interrupted_operations(&Cancellation::default())
                    .unwrap();
                assert_eq!(!issues.is_empty(), replacement);
                assert_eq!(engine.latest_undo_id(), Some(1));
                assert_eq!(
                    engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                    1
                );
                assert_eq!(fs::metadata(&output).unwrap().ino(), before.ino());
                assert_eq!(fs::read(&output).unwrap(), b"Newer completed work");
                assert_eq!(
                    fs::read(root.path().join("source")).unwrap(),
                    b"Original source"
                );
                if replacement {
                    assert_eq!(
                        fs::read(stage.join("folder")).unwrap(),
                        b"Foreign cleanup occupant"
                    );
                } else {
                    assert!(!stage.exists());
                }
            }
            if replacement {
                // Resolve only this test's known foreign occupant. The durable
                // Cleanup record then retires its original empty stage.
                fs::remove_file(stage.join("folder")).unwrap();
                let engine = OperationEngine::open(root.path().join("history")).unwrap();
                assert!(
                    engine
                        .interrupted_operations(&Cancellation::default())
                        .unwrap()
                        .is_empty()
                );
                assert!(!stage.exists());
                assert_eq!(engine.latest_undo_id(), Some(1));
                assert_eq!(fs::read(output).unwrap(), b"Newer completed work");
            }
        }
    }
}

#[test]
fn failed_cleanup_registration_rolls_back_the_step_and_preserves_forward_intent() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("history");
    let source = root.path().join("source");
    let output = root.path().join("output");
    fs::write(&source, b"Original source").unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    let mut state = engine.state.lock().unwrap();
    let id = state.begin("cleanup rollback").unwrap();
    let prepared = fingerprint::prepare_copy(
        &source,
        root.path(),
        &Cancellation::default(),
        |created| state.register_preparing(id, &output, created),
        |_| {},
        || Ok(()),
    )
    .unwrap();
    let change = Change::Copied {
        source: None,
        publication: None,
        access_parent: None,
        path: output.clone(),
        identity: prepared.identity,
        fingerprint: prepared.fingerprint,
        retained: None,
    };
    state
        .intent(&Pending::Forward {
            id,
            ordinal: 0,
            change: change.clone(),
            stage: Some(prepared.stage.clone()),
        })
        .unwrap();
    prepared.opened.publish(&output).unwrap();
    let database = rusqlite::Connection::open(&history).unwrap();
    database.execute_batch("CREATE TRIGGER reject_cleanup BEFORE INSERT ON pending WHEN json_extract(NEW.body,'$.direction')='cleanup' BEGIN SELECT RAISE(FAIL,'cleanup save refused'); END;").unwrap();
    assert!(
        state
            .append_with_cleanup(id, 0, &change, &prepared.stage)
            .is_err()
    );
    assert!(state.last_change(id).unwrap().is_none());
    assert!(matches!(
        state.pending().unwrap(),
        Some(Pending::Forward { .. })
    ));
    assert_eq!(fs::read(&output).unwrap(), b"Original source");
    database
        .execute_batch("DROP TRIGGER reject_cleanup")
        .unwrap();
    state
        .append_with_cleanup(id, 0, &change, &prepared.stage)
        .unwrap();
    assert!(matches!(
        state.pending().unwrap(),
        Some(Pending::Cleanup { .. })
    ));
    drop(state);
    drop(engine);
    let reopened = OperationEngine::open(&history).unwrap();
    assert!(
        reopened
            .interrupted_operations(&Cancellation::default())
            .unwrap()
            .is_empty()
    );
    assert_eq!(reopened.latest_undo_id(), Some(id));
    assert_eq!(fs::read(output).unwrap(), b"Original source");
}

#[test]
#[ignore = "parent kills a real process after the append/cleanup transaction"]
fn postappend_child() {
    let Some(root) = std::env::var_os("CANOPA_POSTAPPEND_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let source = root.join("source");
    let output = root.join("output");
    fs::write(&source, b"Original source").unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let mut state = engine.state.lock().unwrap();
    let run = |state: &mut Store| -> io::Result<()> {
        let id = state.begin("copy before cleanup death")?;
        let prepared = fingerprint::prepare_copy(
            &source,
            &root,
            &Cancellation::default(),
            |created| state.register_preparing(id, &output, created),
            |_| {},
            || Ok(()),
        )?;
        let change = Change::Copied {
            source: None,
            publication: None,
            access_parent: None,
            path: output.clone(),
            identity: prepared.identity,
            fingerprint: prepared.fingerprint,
            retained: None,
        };
        state.intent(&Pending::Forward {
            id,
            ordinal: 0,
            change: change.clone(),
            stage: Some(prepared.stage.clone()),
        })?;
        prepared.opened.publish(&output)?;
        state.append_with_cleanup(id, 0, &change, &prepared.stage)?;
        fs::write(
            root.join("stage.json"),
            serde_json::to_vec(&prepared.stage.directory).unwrap(),
        )?;
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)?;
        unreachable!();
    };
    if std::env::var("CANOPA_POSTAPPEND_RECOVERY").unwrap() == "1" {
        state.in_recovery_copy(run).unwrap();
    } else {
        run(&mut state).unwrap();
    }
    panic!("postappend child was not killed");
}
