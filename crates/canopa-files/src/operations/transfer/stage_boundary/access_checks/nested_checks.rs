//! Created ancestor access must survive nested continuation and Recovery.
use super::*;

pub(super) fn preparing(source: &Path, stage: &Stage, cancellation: &Cancellation) {
    let Some(root) = std::env::var_os("CANOPA_NESTED_ACCESS_ROOT").map(PathBuf::from) else {
        return;
    };
    if source != root.join("source/inner/b-current") {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-nested-access-")
    );
    if !root.join("resumed").exists() {
        fs::write(root.join("first-partial"), b"ready to cancel").unwrap();
        wait(|| cancellation.is_cancelled().then_some(()));
        return;
    }
    assert_eq!(mode(&root.join("output/source")), 0o700);
    assert_eq!(mode(&root.join("output/source/inner")), 0o700);
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let body: String = db
        .query_row("SELECT body FROM pending", [], |row| row.get(0))
        .unwrap();
    let pending: Pending = serde_json::from_str(&body).unwrap();
    assert!(matches!(&pending, Pending::Preparing { stage: actual, .. } if actual == stage));
    let request = db
        .query_row("SELECT request FROM request_transfer_cursors", [], |r| {
            r.get(0)
        })
        .unwrap();
    let binding = db
        .query_row("SELECT body FROM stage_intents WHERE lane=0", [], |r| {
            r.get(0)
        })
        .ok();
    let (revision, accepted): (u64, Vec<u8>) = db
        .query_row(
            "SELECT revision,body FROM requests WHERE id=?1",
            [request],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    fs::write(
        root.join("marker.json"),
        serde_json::to_vec(&Marker {
            request,
            pending,
            output: root.join("output/source/inner"),
            binding,
            revision,
            accepted: Sha256::digest(&accepted).to_vec(),
        })
        .unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private nested access worker was not killed");
}
#[test]
#[ignore = "private Copy worker killed after continuing through nested restricted folders"]
fn nested_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_NESTED_ACCESS_ROOT").unwrap());
    fs::create_dir_all(root.join("source/inner")).unwrap();
    fs::create_dir(root.join("output")).unwrap();
    fs::write(root.join("source/a-done"), b"Completed outer prefix").unwrap();
    fs::write(root.join("source/inner/a-done"), b"Completed inner prefix").unwrap();
    fs::write(root.join("source/inner/b-current"), vec![b'x'; 128 * 1024]).unwrap();
    fs::write(root.join("source/inner/z-later"), b"Later nested leaf").unwrap();
    chmod(&root.join("source/inner"), 0o555);
    chmod(&root.join("source"), 0o555);
    let jobs = start(&root);
    let id = receive(
        jobs.submit(
            Request::Transfer(TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![root.join("source")],
                destination: root.join("output"),
            }),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    wait(|| root.join("first-partial").exists().then_some(()));
    jobs.cancel(id).unwrap();
    let stopped = wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && !job.phase.holds_application())
    });
    assert_eq!(stopped.phase, Phase::Stopped, "{stopped:?}");
    assert_eq!(mode(&root.join("output/source")), 0o555);
    assert_eq!(mode(&root.join("output/source/inner")), 0o555);
    chmod(&root.join("output/source"), 0o700);
    chmod(&root.join("output/source/inner"), 0o700);
    fs::write(root.join("output/source/a-done"), b"Edited completed outer").unwrap();
    fs::write(
        root.join("output/source/inner/a-done"),
        b"Edited completed inner",
    )
    .unwrap();
    chmod(&root.join("output/source/inner"), 0o400);
    let original_inner = fs::File::open(root.join("output/source/inner")).unwrap();
    chmod(&root.join("output/source"), 0o400);
    let replacement = std::env::var_os("CANOPA_NESTED_REPLACE_OUTER").is_some();
    let replacement_inner = if replacement {
        fs::rename(
            root.join("output/source"),
            root.join("output/held-original"),
        )
        .unwrap();
        fs::create_dir_all(root.join("output/source/inner")).unwrap();
        fs::write(
            root.join("output/source/inner/sentinel"),
            b"Replacement data",
        )
        .unwrap();
        let file = fs::File::open(root.join("output/source/inner")).unwrap();
        chmod(&root.join("output/source/inner"), 0o400);
        chmod(&root.join("output/source"), 0o400);
        Some(file)
    } else {
        None
    };
    fs::write(root.join("resumed"), b"continue nested work").unwrap();
    let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
    let stopped = wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == next && !job.phase.holds_application())
    });
    if let Some(replacement_inner) = replacement_inner {
        assert!(
            matches!(stopped.phase, Phase::Stopped | Phase::Failed),
            "{stopped:?}"
        );
        let report = stopped.transfer.as_ref().unwrap();
        assert_eq!(report.completed, 0, "{stopped:?}");
        assert_eq!(report.bytes, 0, "{stopped:?}");
        let refusal = format!("{stopped:?}");
        assert!(
            refusal.contains("original") || refusal.contains("replaced"),
            "{stopped:?}"
        );
        assert_eq!(mode(&root.join("output/source")), 0o400);
        assert_eq!(
            replacement_inner.metadata().unwrap().permissions().mode() & 0o7777,
            0o400
        );
        assert_eq!(
            original_inner.metadata().unwrap().permissions().mode() & 0o7777,
            0o400
        );
        return;
    }
    panic!("nested Continue did not reach its unfinished partial: {stopped:?}");
}
fn nested_fixture(run: impl FnOnce(&Path, Marker, &[u8])) {
    let root = tempfile::Builder::new()
        .prefix("canopa-nested-access-")
        .tempdir()
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::stage_boundary::access_checks::nested_checks::nested_child",
            ])
            .env("CANOPA_NESTED_ACCESS_ROOT", root.path())
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let marker: Marker =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        let bytes = b"Unique interrupted nested edits";
        fs::write(work(&marker).payload(), bytes).unwrap();
        run(root.path(), marker, bytes);
    }));
    if let Err(error) = checked {
        eprintln!("retained nested-access fixture: {}", root.keep().display());
        std::panic::resume_unwind(error);
    }
    make_removable(root.path());
}
pub(super) fn released(path: &Path) {
    let Some(root) = std::env::var_os("CANOPA_NESTED_RELEASE_ROOT").map(PathBuf::from) else {
        return;
    };
    if path != root.join("output/source") {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-nested-access-")
    );
    assert_eq!(mode(path), 0o400);
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let grants: Vec<(Vec<u8>, String)> = db
        .prepare("SELECT path,body FROM directory_read_grants")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(grants.len(), 1, "{grants:?}");
    assert_eq!(
        grants[0].0,
        root.join("output/source/inner")
            .as_os_str()
            .as_encoded_bytes()
    );
    fs::write(
        root.join("release-cut.json"),
        serde_json::to_vec(&grants).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private nested read releaser was not killed");
}
#[test]
#[ignore = "private Recovery releaser killed between ancestor and descendant restoration"]
fn nested_reader_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_NESTED_RELEASE_ROOT").unwrap());
    let jobs = start(&root);
    assert_ne!(reconcile(&jobs).phase, Phase::Finished);
    let item = partial(&jobs, b"Unique interrupted nested edits");
    assert_eq!(mode(&root.join("output/source")), 0o500);
    assert_eq!(mode(&root.join("output/source/inner")), 0o500);
    assert_eq!(grants(&root), 2);
    fs::write(
        root.join("reader-before-release.json"),
        serde_json::to_vec(&item).unwrap(),
    )
    .unwrap();
    drop(item);
    std::thread::sleep(Duration::from_secs(20));
    panic!("private nested release boundary was not reached");
}
#[test]
fn startup_restores_a_nested_read_grant_hidden_by_its_already_restored_ancestor() {
    nested_fixture(|root, marker, bytes| {
        let inner = release_cut(root, &marker);
        let jobs = start(root);
        assert_eq!(mode(&root.join("output/source")), 0o400);
        assert_eq!(
            inner.metadata().unwrap().permissions().mode() & 0o7777,
            0o400,
            "startup must restore the original inner folder despite its non-searchable ancestor"
        );
        assert_eq!(grants(root), 0);
        let item = partial(&jobs, bytes);
        drop(item);
        close(jobs, root);
        assert_eq!(
            inner.metadata().unwrap().permissions().mode() & 0o7777,
            0o400
        );
    });
}
#[test]
fn nested_restricted_created_folders_allow_safe_restart_continuation_and_recovery() {
    continuation_mode(None);
}
#[test]
fn external_mode_edits_with_owner_access_survive_live_recovery_selections() {
    continuation_mode(Some(0o701));
}
#[test]
fn external_mode_edits_without_search_survive_and_allow_fresh_recovery() {
    continuation_mode(Some(0o444));
}
fn continuation_mode(mode: Option<u32>) {
    nested_fixture(|root, marker, bytes| {
        let inner = fs::File::open(&marker.output).unwrap();
        let jobs = start(root);
        assert_ne!(reconcile(&jobs).phase, Phase::Finished);
        assert_eq!(super::mode(&root.join("output/source")), 0o400);
        let item = partial(&jobs, bytes);
        assert_eq!(super::mode(&root.join("output/source")), 0o500);
        assert_eq!(super::mode(&marker.output), 0o500);
        assert!(item.can_keep_in_recovery());
        let copied = receive(
            jobs.submit(
                Request::Recover {
                    item: Box::new(item.clone()),
                    destination: root.join("rescued"),
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
                .find(|job| job.id == copied && !job.phase.holds_application())
        });
        assert_eq!(result.phase, Phase::Finished, "{result:?}");
        assert_eq!(fs::read(root.join("rescued")).unwrap(), bytes);
        let kept = receive(
            jobs.submit(
                Request::KeepRecovery {
                    item: Box::new(item.clone()),
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
                .find(|job| job.id == kept && !job.phase.holds_application())
        });
        assert_eq!(result.phase, Phase::Finished, "{result:?}");
        if let Some(mode) = mode {
            chmod(&marker.output, mode);
            chmod(&root.join("output/source"), mode);
        }
        let resumed = receive(
            jobs.resume_transfer(marker.request, TransferPart::Remaining)
                .unwrap(),
        )
        .unwrap();
        let result = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|job| job.id == resumed && !job.phase.holds_application())
        });
        assert_eq!(result.phase, Phase::Finished, "{result:?}");
        let before_refresh = mode.unwrap_or(0o500);
        assert_eq!(
            super::mode(&root.join("output/source")),
            before_refresh,
            "Continue must preserve an external mode edit while an older Recovery selection lives"
        );
        assert_eq!(
            inner.metadata().unwrap().permissions().mode() & 0o7777,
            before_refresh
        );
        let fresh = partial(&jobs, bytes);
        assert_eq!(
            fs::read(root.join("output/source/a-done")).unwrap(),
            b"Edited completed outer"
        );
        assert_eq!(
            fs::read(marker.output.join("a-done")).unwrap(),
            b"Edited completed inner"
        );
        assert_eq!(
            fs::read(marker.output.join("b-current")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        assert_eq!(
            fs::read(marker.output.join("z-later")).unwrap(),
            b"Later nested leaf"
        );
        drop(fresh);
        drop(item);
        close(jobs, root);
        assert_eq!(
            super::mode(&root.join("output/source")),
            mode.unwrap_or(0o400)
        );
        assert_eq!(
            inner.metadata().unwrap().permissions().mode() & 0o7777,
            mode.unwrap_or(0o400)
        );
        let jobs = start(root);
        let item = partial(&jobs, bytes);
        drop(item);
        close(jobs, root);
        assert_eq!(
            inner.metadata().unwrap().permissions().mode() & 0o7777,
            mode.unwrap_or(0o400)
        );
    });
}
fn release_cut(root: &Path, marker: &Marker) -> fs::File {
    let inner = fs::File::open(&marker.output).unwrap();
    let status=std::process::Command::new(std::env::current_exe().unwrap()).args([
        "--ignored","--exact","operations::transfer::stage_boundary::access_checks::nested_checks::nested_reader_child",
    ]).env("CANOPA_NESTED_RELEASE_ROOT",root).status().unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(root.join("release-cut.json").is_file());
    assert_eq!(mode(&root.join("output/source")), 0o400);
    assert_eq!(
        inner.metadata().unwrap().permissions().mode() & 0o7777,
        0o500
    );
    assert_eq!(grants(root), 1);
    inner
}
pub(super) fn opened(path: &Path) {
    let Some(root) = std::env::var_os("CANOPA_NESTED_STARTUP_ROOT").map(PathBuf::from) else {
        return;
    };
    if path != root.join("output/source") {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-nested-access-")
    );
    assert_eq!(mode(path), 0o500);
    assert_eq!(grants(&root), 2);
    fs::write(
        root.join("startup-cut.json"),
        b"ancestor read grant committed before descendant restoration",
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private startup access worker was not killed");
}
#[test]
#[ignore = "private startup worker killed after committing an ancestor read grant"]
fn nested_startup_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_NESTED_STARTUP_ROOT").unwrap());
    let _jobs = start(&root);
    panic!("private startup access boundary was not reached");
}
#[test]
fn another_startup_death_preserves_both_original_permission_restorations() {
    nested_fixture(|root, marker, bytes| {
        let inner = release_cut(root, &marker);
        let status=std::process::Command::new(std::env::current_exe().unwrap()).args([
            "--ignored","--exact","operations::transfer::stage_boundary::access_checks::nested_checks::nested_startup_child",
        ]).env("CANOPA_NESTED_STARTUP_ROOT",root).status().unwrap();
        assert_eq!(status.signal(), Some(9));
        assert!(root.join("startup-cut.json").is_file());
        assert_eq!(mode(&root.join("output/source")), 0o500);
        assert_eq!(
            inner.metadata().unwrap().permissions().mode() & 0o7777,
            0o500
        );
        assert_eq!(grants(root), 2);
        let jobs = start(root);
        assert_eq!(mode(&root.join("output/source")), 0o400);
        assert_eq!(
            inner.metadata().unwrap().permissions().mode() & 0o7777,
            0o400
        );
        assert_eq!(grants(root), 0);
        let item = partial(&jobs, bytes);
        drop(item);
        close(jobs, root);
    });
}
#[test]
fn startup_never_borrows_a_replacement_ancestor_to_restore_a_nested_grant() {
    nested_fixture(|root, marker, bytes| {
        let inner = release_cut(root, &marker);
        let outer = root.join("output/source");
        let held = root.join("output/held-original");
        fs::rename(&outer, &held).unwrap();
        fs::create_dir_all(outer.join("inner")).unwrap();
        fs::write(outer.join("inner/replacement"), b"Replacement bytes").unwrap();
        let replacement = fs::File::open(outer.join("inner")).unwrap();
        chmod(&outer.join("inner"), 0o400);
        chmod(&outer, 0o400);
        let jobs = start(root);
        assert_eq!(mode(&outer), 0o400);
        assert_eq!(
            replacement.metadata().unwrap().permissions().mode() & 0o7777,
            0o400
        );
        assert_eq!(
            inner.metadata().unwrap().permissions().mode() & 0o7777,
            0o500
        );
        assert_eq!(grants(root), 1);
        let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
        assert!(page.items.iter().all(|item| item.entry.is_none()));
        drop(page);
        close(jobs, root);
        fs::rename(&outer, root.join("output/kept-replacement")).unwrap();
        fs::rename(&held, &outer).unwrap();
        let jobs = start(root);
        assert_eq!(mode(&outer), 0o400);
        assert_eq!(
            inner.metadata().unwrap().permissions().mode() & 0o7777,
            0o400
        );
        assert_eq!(
            replacement.metadata().unwrap().permissions().mode() & 0o7777,
            0o400
        );
        let item = partial(&jobs, bytes);
        drop(item);
        close(jobs, root);
    });
}
#[test]
fn refused_or_ignored_startup_ancestor_grants_preserve_permissions_until_retry() {
    for result in ["ABORT,'private ancestor grant refusal'", "IGNORE"] {
        nested_fixture(|root, marker, bytes| {
            let inner = release_cut(root, &marker);
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            db.execute_batch(&format!("CREATE TRIGGER refuse_ancestor_read BEFORE INSERT ON directory_read_grants BEGIN SELECT RAISE({result}); END;")).unwrap();
            let jobs = start(root);
            assert_eq!(mode(&root.join("output/source")), 0o400);
            assert_eq!(
                inner.metadata().unwrap().permissions().mode() & 0o7777,
                0o500
            );
            assert_eq!(grants(root), 1);
            close(jobs, root);
            db.execute_batch("DROP TRIGGER refuse_ancestor_read")
                .unwrap();
            drop(db);
            let jobs = start(root);
            assert_eq!(mode(&root.join("output/source")), 0o400);
            assert_eq!(
                inner.metadata().unwrap().permissions().mode() & 0o7777,
                0o400
            );
            assert_eq!(grants(root), 0);
            let item = partial(&jobs, bytes);
            drop(item);
            close(jobs, root);
        });
    }
}
#[test]
fn nested_continue_refuses_a_replacement_created_ancestor_before_borrowing_access() {
    let root = tempfile::Builder::new()
        .prefix("canopa-nested-access-")
        .tempdir()
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::stage_boundary::access_checks::nested_checks::nested_child",
            ])
            .env("CANOPA_NESTED_ACCESS_ROOT", root.path())
            .env("CANOPA_NESTED_REPLACE_OUTER", "1")
            .status()
            .unwrap();
        assert!(status.success(), "{status}");
        make_removable(root.path());
        assert_eq!(
            fs::read(root.path().join("output/source/inner/sentinel")).unwrap(),
            b"Replacement data"
        );
        assert!(!root.path().join("output/source/inner/b-current").exists());
        assert_eq!(
            fs::read(root.path().join("output/held-original/a-done")).unwrap(),
            b"Edited completed outer"
        );
        assert_eq!(
            fs::read(root.path().join("output/held-original/inner/a-done")).unwrap(),
            b"Edited completed inner"
        );
    }));
    if let Err(error) = checked {
        eprintln!("retained nested-access fixture: {}", root.keep().display());
        std::panic::resume_unwind(error);
    }
}
