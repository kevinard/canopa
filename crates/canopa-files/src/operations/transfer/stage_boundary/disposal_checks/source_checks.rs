//! A disposable partial must remain reproducible from its original source.
use super::*;

fn retained_partial(jobs: &OperationJobs, bytes: &[u8]) -> crate::operations::RecoveryItem {
    receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
        .unwrap()
        .items
        .into_iter()
        .find(|item| {
            item.entry
                .as_ref()
                .is_some_and(|entry| fs::read(&entry.path).is_ok_and(|actual| actual == bytes))
        })
        .expect("partial bytes remain available through public Recovery")
}
fn preserve_partial(missing: bool, shape: &str) {
    fixture_shape("written", shape, |root, marker| {
        let stage = work(&marker);
        let bytes = if missing {
            fs::read(stage.payload()).unwrap()
        } else {
            let bytes = b"Unique edits in interrupted work".to_vec();
            fs::write(stage.payload(), &bytes).unwrap();
            bytes
        };
        if missing {
            fs::remove_file(root.join("b-current")).unwrap();
        }
        fs::write(marker.output.join("a-done"), b"Changed completed output").unwrap();
        fs::write(root.join("a-done"), b"Changed completed source").unwrap();
        let jobs = start(root);
        let result = reconcile(&jobs);
        let _item = retained_partial(&jobs, &bytes);
        assert_ne!(result.phase, Phase::Finished, "{result:?}");
        unchanged(root, &marker);
        close(jobs, root);
        for _ in 0..2 {
            let jobs = start(root);
            assert!(receive(jobs.needs_recovery_review().unwrap()).unwrap());
            let _item = retained_partial(&jobs, &bytes);
            unchanged(root, &marker);
            close(jobs, root);
        }
        let jobs = start(root);
        let item = retained_partial(&jobs, &bytes);
        let path = item.entry.as_ref().unwrap().path.clone();
        copy_item(&jobs, &path, &root.join("rescued-partial"), &bytes);
        let fresh = retained_partial(&jobs, &bytes);
        assert!(fresh.can_keep_in_recovery());
        assert_eq!(keep_item(&jobs, fresh).phase, Phase::Finished);
        if missing {
            let original = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|job| job.id == marker.request)
                .unwrap();
            assert_eq!(original.phase, Phase::Stopped, "{original:?}");
            assert!(original.transfer_actions.abandon, "{original:?}");
            receive(jobs.abandon_transfer(marker.request).unwrap()).unwrap();
        }
        close(jobs, root);
        if !missing {
            continue_work(root, &marker, shape);
        }
        assert!(fs::read(&path).is_ok_and(|actual| actual == bytes));
        assert!(fs::read(root.join("rescued-partial")).is_ok_and(|actual| actual == bytes));
    });
}
#[test]
fn missing_original_sources_leave_partial_data_recoverable_after_restart() {
    preserve_partial(true, "file");
}
#[test]
fn edited_partials_leave_unique_data_recoverable_after_restart() {
    preserve_partial(false, "file");
}

#[test]
fn returning_the_exact_original_source_allows_verified_disposal_and_continuation() {
    fixture_shape("written", "file", |root, marker| {
        fs::write(marker.output.join("a-done"), b"Changed completed output").unwrap();
        fs::write(root.join("a-done"), b"Changed completed source").unwrap();
        fs::rename(root.join("b-current"), root.join("original-source")).unwrap();
        let jobs = start(root);
        assert_ne!(reconcile(&jobs).phase, Phase::Finished);
        let _item = retained_partial(&jobs, &vec![b'x'; 64 * 1024]);
        close(jobs, root);
        fs::write(root.join("b-current"), vec![b'x'; 128 * 1024]).unwrap();
        let jobs = start(root);
        assert_ne!(
            reconcile(&jobs).phase,
            Phase::Finished,
            "replacement bytes do not prove the original generation"
        );
        let _item = retained_partial(&jobs, &vec![b'x'; 64 * 1024]);
        close(jobs, root);
        fs::rename(root.join("b-current"), root.join("replacement-source")).unwrap();
        fs::rename(root.join("original-source"), root.join("b-current")).unwrap();
        let jobs = start(root);
        assert_eq!(reconcile(&jobs).phase, Phase::Finished);
        close(jobs, root);
        continue_work(root, &marker, "file");
        assert_eq!(
            fs::read(root.join("replacement-source")).unwrap().len(),
            128 * 1024
        );
    });
}

fn held_payload(root: &Path) -> PathBuf {
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let body: String = db
        .query_row("SELECT body FROM pending", [], |row| row.get(0))
        .unwrap();
    let Pending::Disposing { disposal } = serde_json::from_str(&body).unwrap() else {
        panic!("disposal")
    };
    disposal.holding.payload()
}
#[test]
fn cancellation_and_changes_during_reproduction_never_consume_partial_data() {
    for mode in [
        "cancel",
        "edit-during-read",
        "edit-after-save",
        "replace-source-after-save",
    ] {
        fixture_shape("written", "file", |root, _marker| {
            let engine = OperationEngine::open_with_recovery_check(root.join("history"), |_| {
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "private explicit review gate",
                ))
            })
            .unwrap();
            let cancellation = Cancellation::default();
            let cancel_at = cancellation.clone();
            let root_at = root.to_owned();
            let point = if mode == "cancel" || mode == "edit-during-read" {
                "chunk"
            } else {
                "consumed"
            };
            let _reset = fingerprint::source::at_reproduction(point, move || match mode {
                "cancel" => cancel_at.cancel(),
                "edit-during-read" | "edit-after-save" => {
                    fs::write(held_payload(&root_at), b"Late partial edits").unwrap()
                }
                "replace-source-after-save" => {
                    fs::rename(root_at.join("b-current"), root_at.join("original-source")).unwrap();
                    fs::write(root_at.join("b-current"), vec![b'x'; 128 * 1024]).unwrap();
                }
                _ => unreachable!(),
            });
            let error = engine.retry_recovery(&cancellation).unwrap_err();
            if mode == "cancel" {
                assert_eq!(error.kind(), io::ErrorKind::Interrupted);
            }
            let payload = held_payload(root);
            let expected = if mode.starts_with("edit-") {
                b"Late partial edits".to_vec()
            } else {
                vec![b'x'; 64 * 1024]
            };
            assert!(
                fs::read(&payload).is_ok_and(|actual| actual == expected),
                "{mode}"
            );
            let page = engine
                .recovery_items(None, &Cancellation::default())
                .unwrap();
            assert!(!page.interrupted.is_empty());
            if mode == "cancel" {
                engine.retry_recovery(&Cancellation::default()).unwrap();
                assert!(!payload.exists());
            } else {
                let item = page
                    .items
                    .into_iter()
                    .find(|item| {
                        item.entry
                            .as_ref()
                            .is_some_and(|entry| entry.path == payload)
                    })
                    .unwrap();
                assert!(item.can_keep_in_recovery());
                engine
                    .keep_in_recovery(&item, &Cancellation::default())
                    .unwrap();
                assert!(fs::read(payload).is_ok_and(|actual| actual == expected));
            }
        });
    }
}

#[test]
fn interrupted_cross_filesystem_moves_preserve_missing_and_edited_partial_data() {
    preserve_partial(true, "cross");
    preserve_partial(false, "cross");
}

#[test]
fn legacy_preparations_without_source_proof_require_keep_before_continuation() {
    fixture_shape("written", "file", |root, marker| {
        fs::write(marker.output.join("a-done"), b"Changed completed output").unwrap();
        fs::write(root.join("a-done"), b"Changed completed source").unwrap();
        crate::operations::store::legacy_reproduction_fixture(&root.join("history"));
        let _reset = fingerprint::source::at_reproduction("chunk", || {
            panic!("legacy data adopted a current source")
        });
        let engine = OperationEngine::open(root.join("history")).unwrap();
        assert!(engine.retry_recovery(&Cancellation::default()).is_err());
        let bytes = vec![b'x'; 64 * 1024];
        let page = engine
            .recovery_items(None, &Cancellation::default())
            .unwrap();
        let item = page
            .items
            .into_iter()
            .find(|item| {
                item.entry
                    .as_ref()
                    .is_some_and(|entry| fs::read(&entry.path).is_ok_and(|actual| actual == bytes))
            })
            .unwrap();
        let path = item.entry.as_ref().unwrap().path.clone();
        assert!(item.can_keep_in_recovery());
        engine
            .keep_in_recovery(&item, &Cancellation::default())
            .unwrap();
        drop(engine);
        let jobs = start(root);
        assert_eq!(reconcile(&jobs).phase, Phase::Finished);
        close(jobs, root);
        continue_work(root, &marker, "file");
        assert!(fs::read(path).is_ok_and(|actual| actual == bytes));
    });
}

#[test]
fn a_live_source_failure_keeps_partial_data_and_advances_the_original_request_cursor() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("large");
    let later = root.path().join("later");
    let output = root.path().join("output");
    fs::create_dir(&output).unwrap();
    let size = 256 * 1024 * 1024;
    fs::File::create(&source).unwrap().set_len(size).unwrap();
    fs::write(&later, b"Unrelated item").unwrap();
    let jobs = start(root.path());
    let id = receive(
        jobs.submit(
            Request::Transfer(TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone(), later],
                destination: output.clone(),
            }),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Running)
            .and_then(|job| job.progress)
            .filter(|progress| {
                progress.path == source && progress.bytes > 0 && progress.bytes < size
            })
    });
    fs::write(&source, b"New original source").unwrap();
    let failed = wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && !job.phase.holds_application())
    });
    assert_eq!(failed.phase, Phase::Stopped, "{failed:?}");
    assert!(failed.transfer_actions.retry && failed.transfer_actions.abandon);
    let report = failed.transfer.unwrap();
    assert_eq!(report.completed, 1);
    assert_eq!(report.failures.len(), 1);
    assert!(report.failures[0].message.contains("Recovery"));
    assert!(!output.join("large").exists());
    assert_eq!(fs::read(output.join("later")).unwrap(), b"Unrelated item");
    assert_eq!(fs::read(&source).unwrap(), b"New original source");
    assert!(!receive(jobs.needs_recovery_review().unwrap()).unwrap());
    let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
    assert!(page.interrupted.is_empty());
    let item = page
        .items
        .into_iter()
        .find(|item| {
            item.entry.as_ref().is_some_and(|entry| {
                fs::metadata(&entry.path).is_ok_and(|metadata| metadata.is_file())
            })
        })
        .unwrap();
    let partial = item.entry.as_ref().unwrap().path.clone();
    let bytes = fs::read(&partial).unwrap();
    assert!(!bytes.is_empty() && bytes.iter().all(|byte| *byte == 0));
    copy_item(
        &jobs,
        &partial,
        &root.path().join("rescued-live-partial"),
        &bytes,
    );
    close(jobs, root.path());
    let jobs = start(root.path());
    assert!(!receive(jobs.needs_recovery_review().unwrap()).unwrap());
    assert!(fs::read(partial).is_ok_and(|actual| actual == bytes));
    assert_eq!(fs::read(output.join("later")).unwrap(), b"Unrelated item");
    close(jobs, root.path());
}
