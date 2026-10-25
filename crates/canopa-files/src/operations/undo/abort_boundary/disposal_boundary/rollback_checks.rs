//! Real return-claim deaths exercise both journal ownership and renamed entries.
use super::*;
fn fixture_return(point: &str, run: impl FnOnce(&Path, &Path, Cut)) {
    fixture_decision_at(false, true, "after-displace", |root, output, _| {
        run(root, output, cut(root, point))
    });
}
fn completed(jobs: &OperationJobs, root: &Path, output: &Path) {
    let done = reconcile(jobs);
    assert_eq!(done.phase, Phase::Finished, "{done:?}");
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        b"Occupied source name"
    );
    assert_eq!(
        fs::read(output.join("item")).unwrap(),
        vec![b'x'; 128 * 1024]
    );
}
fn retry_original(jobs: &OperationJobs, root: &Path, output: &Path, replace: bool) {
    let expected = jobs.snapshot().undo.unwrap().id;
    let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
    if replace {
        let decision = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id && j.phase == Phase::Waiting)
                .and_then(|j| j.decision)
        });
        jobs.resolve(id, decision.generation, UndoChoice::Replace)
            .unwrap();
    }
    let done = terminal(jobs, id);
    assert_eq!(done.phase, Phase::Finished, "{done:?}");
    assert_eq!(
        fs::read(root.join("source/item")).unwrap(),
        vec![b'x'; 128 * 1024]
    );
    assert!(!output.join("item").exists());
}
#[test]
fn empty_and_disappearing_copy_preparations_preserve_displaced_work_and_allow_retry() {
    for shape in ["empty", "missing", "during-open"] {
        fixture_decision_at(
            false,
            true,
            if shape == "empty" {
                "empty-copy"
            } else {
                "after-displace"
            },
            |root, output, stage| {
                if shape == "missing" {
                    fs::rename(stage.payload(), root.join("kept-return-copy")).unwrap();
                }
                if shape == "during-open" {
                    let status=std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--ignored","--exact","operations::undo::abort_boundary::disposal_boundary::undo_disposal_child"])
                    .env("CANOPA_UNDO_DISPOSE_ROOT",root).env("CANOPA_UNDO_MOVE_COPY_BEFORE_OPEN","1").env("CANOPA_UNDO_DISPOSE_EXPECT_FINISHED","1").status().unwrap();
                    assert!(status.success());
                }
                for _ in 0..2 {
                    let jobs = start(root);
                    completed(&jobs, root, output);
                    assert!(!stage.directory.exists());
                    if shape != "empty" {
                        assert_eq!(
                            fs::read(root.join("kept-return-copy")).unwrap(),
                            vec![b'x'; 128 * 1024]
                        );
                    }
                    close(jobs, root);
                }
                let jobs = start(root);
                retry_original(&jobs, root, output, true);
                close(jobs, root);
            },
        );
    }
}
#[test]
fn a_missing_return_payload_never_replays_a_consumed_previous_name() {
    fixture_decision_at(false, true, "after-displace", |root, output, stage| {
        let pending: Pending =
            serde_json::from_slice(&fs::read(root.join("marker.json")).unwrap()).unwrap();
        let Pending::Undo {
            displaced: Some(record),
            ..
        } = pending
        else {
            unreachable!();
        };
        fs::remove_file(stage.payload()).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::undo::abort_boundary::disposal_boundary::undo_disposal_child",
            ])
            .env("CANOPA_UNDO_DISPOSE_ROOT", root)
            .env("CANOPA_UNDO_RAW_RETURN_CUT", "1")
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        fs::rename(root.join("source/item"), root.join("held-returned-item")).unwrap();
        fs::hard_link(root.join("held-returned-item"), record.retained.path()).unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            let done = reconcile(&jobs);
            fs::write(root.join("reintroduced-return.txt"), format!("{done:?}")).unwrap();
            assert!(
                record.retained.path().exists(),
                "a consumed previous-name entry must remain in Recovery"
            );
            assert_ne!(done.phase, Phase::Finished);
            assert!(!root.join("source/item").exists());
            assert_eq!(
                fs::read(record.retained.path()).unwrap(),
                b"Occupied source name"
            );
            assert_eq!(
                fs::read(output.join("item")).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            close(jobs, root);
        }
        let jobs = start(root);
        assert_eq!(
            keep(&jobs, partial(&jobs, b"Occupied source name")).phase,
            Phase::Finished
        );
        retry_original(&jobs, root, output, false);
        assert_eq!(
            fs::read(record.retained.path()).unwrap(),
            b"Occupied source name"
        );
        assert_eq!(
            fs::read(root.join("held-returned-item")).unwrap(),
            b"Occupied source name"
        );
        close(jobs, root);
    });
}
#[test]
fn killed_undo_returns_resume_only_unfinished_claim_and_cleanup_work() {
    for point in [
        "return-reserved",
        "return-after-claim",
        "return-claimed",
        "return-after-rename",
        "return-returned",
        "return-roots-gone",
    ] {
        fixture_return(point, |root, output, cut| {
            for _ in 0..2 {
                let jobs = start(root);
                completed(&jobs, root, output);
                assert!(!cut.disposal.original.directory.exists());
                assert!(!cut.disposal.holding.directory.exists());
                assert!(
                    !cut.disposal
                        .returned
                        .as_ref()
                        .unwrap()
                        .holding
                        .as_ref()
                        .unwrap()
                        .directory
                        .exists()
                );
                close(jobs, root);
            }
            let jobs = start(root);
            retry_original(&jobs, root, output, true);
            close(jobs, root);
        });
    }
}
#[test]
fn consumed_undo_return_names_are_never_renamed_again_even_for_the_same_inode() {
    for point in ["return-consumed", "return-after-rename"] {
        for same_inode in [false, true] {
            fixture_decision_at(false, true, "after-displace", |root, output, _| {
                let pending: Pending =
                    serde_json::from_slice(&fs::read(root.join("marker.json")).unwrap()).unwrap();
                let Pending::Undo {
                    displaced: Some(record),
                    ..
                } = pending
                else {
                    unreachable!();
                };
                let held = root.join("held-newer-inode");
                fs::hard_link(record.retained.path(), &held).unwrap();
                let cut = cut(root, point);
                let stage = cut
                    .disposal
                    .returned
                    .as_ref()
                    .unwrap()
                    .holding
                    .as_ref()
                    .unwrap();
                if point == "return-after-rename" {
                    if same_inode {
                        fs::hard_link(&held, stage.payload()).unwrap();
                    } else {
                        fs::write(stage.payload(), b"New contents in consumed return name")
                            .unwrap();
                    }
                }
                let bytes = fs::read(stage.payload()).unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                    assert_eq!(fs::read(stage.payload()).unwrap(), bytes);
                    assert_eq!(
                        fs::read(output.join("item")).unwrap(),
                        vec![b'x'; 128 * 1024]
                    );
                    close(jobs, root);
                }
                let jobs = start(root);
                let item = partial(&jobs, &bytes);
                assert!(item.can_keep_in_recovery());
                let id = receive(
                    jobs.submit(
                        Request::Recover {
                            item: Box::new(item.clone()),
                            destination: root.join("exported-return"),
                        },
                        None,
                    )
                    .unwrap(),
                )
                .unwrap();
                assert_eq!(terminal(&jobs, id).phase, Phase::Finished);
                assert_eq!(fs::read(root.join("exported-return")).unwrap(), bytes);
                let exported = jobs.snapshot().undo.unwrap().id;
                assert_eq!(keep(&jobs, item).phase, Phase::Finished);
                let id = receive(
                    jobs.submit(Request::Undo { expected: exported }, None)
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(terminal(&jobs, id).phase, Phase::Finished);
                retry_original(&jobs, root, output, point == "return-after-rename");
                assert_eq!(fs::read(stage.payload()).unwrap(), bytes);
                close(jobs, root);
            });
        }
    }
}
#[test]
fn failed_undo_return_saves_leave_the_exact_prior_owner_and_preserve_completed_returns() {
    for point in [
        "return-refuse-consumed",
        "return-ignore-consumed",
        "return-ignore-binding",
        "return-refuse-receipt",
        "return-ignore-receipt",
        "return-refuse-retirement",
        "return-ignore-retirement",
    ] {
        fixture_return(point, |root, output, cut| {
            for _ in 0..2 {
                assert_eq!(
                    rows(root),
                    (cut.pending.clone(), cut.binding.clone()),
                    "{point}"
                );
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished, "{point}");
                assert_eq!(
                    fs::read(output.join("item")).unwrap(),
                    vec![b'x'; 128 * 1024]
                );
                if point.contains("receipt") || point.contains("retirement") {
                    assert_eq!(
                        fs::read(root.join("source/item")).unwrap(),
                        b"Occupied source name"
                    );
                } else {
                    assert!(!root.join("source/item").exists());
                }
                close(jobs, root);
            }
            rusqlite::Connection::open(root.join("history"))
                .unwrap()
                .execute_batch("DROP TRIGGER private_undo_disposal;")
                .unwrap();
            let jobs = start(root);
            completed(&jobs, root, output);
            retry_original(&jobs, root, output, true);
            close(jobs, root);
        });
    }
}
#[test]
fn a_saved_return_receipt_never_touches_a_later_original_location_occupant() {
    fixture_return("return-returned", |root, output, _| {
        fs::rename(root.join("source/item"), root.join("returned-newer-file")).unwrap();
        fs::write(
            root.join("source/item"),
            b"Later original-location occupant",
        )
        .unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            assert_eq!(reconcile(&jobs).phase, Phase::Finished);
            assert_eq!(
                fs::read(root.join("source/item")).unwrap(),
                b"Later original-location occupant"
            );
            assert_eq!(
                fs::read(root.join("returned-newer-file")).unwrap(),
                b"Occupied source name"
            );
            close(jobs, root);
        }
        let jobs = start(root);
        retry_original(&jobs, root, output, true);
        assert_eq!(
            fs::read(root.join("returned-newer-file")).unwrap(),
            b"Occupied source name"
        );
        drop(partial(&jobs, b"Later original-location occupant"));
        close(jobs, root);
    });
}
#[test]
fn claimed_previous_names_preserve_new_entries_and_allow_whole_folder_recovery() {
    for same_inode in [false, true] {
        fixture_return("return-claimed", |root, output, cut| {
            let Pending::Undo {
                displaced: Some(record),
                ..
            } = cut.disposal.undo.as_deref().unwrap()
            else {
                unreachable!();
            };
            let returned = cut
                .disposal
                .returned
                .as_ref()
                .unwrap()
                .holding
                .as_ref()
                .unwrap();
            if same_inode {
                fs::hard_link(returned.payload(), record.retained.path()).unwrap();
            } else {
                fs::write(
                    record.retained.path(),
                    b"New previous-name entry after its claim",
                )
                .unwrap();
            }
            let bytes = fs::read(record.retained.path()).unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                assert_ne!(reconcile(&jobs).phase, Phase::Finished);
                assert_eq!(
                    fs::read(root.join("source/item")).unwrap(),
                    b"Occupied source name"
                );
                assert_eq!(fs::read(record.retained.path()).unwrap(), bytes);
                close(jobs, root);
            }
            let jobs = start(root);
            let item = receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
                .unwrap()
                .items
                .into_iter()
                .find(|item| {
                    item.entry
                        .as_ref()
                        .is_some_and(|entry| entry.path == record.retained.stage.directory)
                })
                .unwrap();
            assert!(item.can_keep_in_recovery());
            assert_eq!(keep(&jobs, item).phase, Phase::Finished);
            retry_original(&jobs, root, output, true);
            let item = receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
                .unwrap()
                .items
                .into_iter()
                .find(|item| {
                    item.entry
                        .as_ref()
                        .is_some_and(|entry| entry.path == record.retained.stage.directory)
                })
                .unwrap();
            let id = receive(
                jobs.submit(
                    Request::Recover {
                        item: Box::new(item),
                        destination: root.join("recovered-claimed-root"),
                    },
                    None,
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(terminal(&jobs, id).phase, Phase::Finished);
            assert_eq!(
                fs::read(root.join("recovered-claimed-root/previous")).unwrap(),
                bytes
            );
            close(jobs, root);
        });
    }
}
#[test]
fn target_collisions_and_replacement_return_roots_preserve_all_newer_work() {
    for replace_root in [false, true] {
        fixture_return("return-claimed", |root, output, cut| {
            let stage = cut
                .disposal
                .returned
                .as_ref()
                .unwrap()
                .holding
                .as_ref()
                .unwrap();
            let held = root.join("held-return-root");
            if replace_root {
                fs::rename(&stage.directory, &held).unwrap();
                fs::create_dir(&stage.directory).unwrap();
                fs::write(stage.payload(), b"Foreign return root data").unwrap();
            } else {
                fs::write(root.join("source/item"), b"New target after claim").unwrap();
            }
            let jobs = start(root);
            assert_ne!(reconcile(&jobs).phase, Phase::Finished);
            assert_eq!(
                fs::read(output.join("item")).unwrap(),
                vec![b'x'; 128 * 1024]
            );
            if replace_root {
                assert_eq!(
                    fs::read(stage.payload()).unwrap(),
                    b"Foreign return root data"
                );
                assert!(
                    receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
                        .unwrap()
                        .items
                        .iter()
                        .all(|item| !item.can_keep_in_recovery())
                );
                fs::rename(&stage.directory, root.join("preserved-foreign-return-root")).unwrap();
                fs::rename(&held, &stage.directory).unwrap();
                assert!(partial(&jobs, b"Occupied source name").can_keep_in_recovery());
                completed(&jobs, root, output);
            } else {
                assert_eq!(
                    fs::read(root.join("source/item")).unwrap(),
                    b"New target after claim"
                );
                assert_eq!(
                    keep(&jobs, partial(&jobs, b"Occupied source name")).phase,
                    Phase::Finished
                );
                assert_eq!(fs::read(stage.payload()).unwrap(), b"Occupied source name");
            }
            retry_original(&jobs, root, output, true);
            close(jobs, root);
        });
    }
}
