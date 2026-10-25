//! Restarted retained data beneath a created folder with restricted permissions.
use super::*;
use std::os::unix::fs::PermissionsExt;
mod nested_checks;
pub(super) fn read_released(path: &Path) {
    nested_checks::released(path);
}
pub(super) fn read_opened(path: &Path) {
    nested_checks::opened(path);
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o7777
}
fn chmod(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}
pub(super) fn preparing(source: &Path, stage: &Stage, cancellation: &Cancellation) {
    nested_checks::preparing(source, stage, cancellation);
    let Some(root) = std::env::var_os("CANOPA_STAGE_ACCESS_ROOT").map(PathBuf::from) else {
        return;
    };
    if source != root.join("source/b-current") {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-stage-access-")
    );
    if !root.join("resumed").exists() {
        fs::write(root.join("first-partial"), b"ready to cancel").unwrap();
        wait(|| cancellation.is_cancelled().then_some(()));
        return;
    }
    assert_eq!(mode(&root.join("output/source")), 0o700);
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let pending: String = db
        .query_row("SELECT body FROM pending", [], |r| r.get(0))
        .unwrap();
    let pending: Pending = serde_json::from_str(&pending).unwrap();
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
            output: root.join("output/source"),
            binding,
            revision,
            accepted: Sha256::digest(&accepted).to_vec(),
        })
        .unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private restricted-parent worker was not killed");
}
#[test]
#[ignore = "private queued worker killed beneath a restricted created folder"]
fn access_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_STAGE_ACCESS_ROOT").unwrap());
    fs::create_dir(root.join("source")).unwrap();
    fs::create_dir(root.join("output")).unwrap();
    fs::write(root.join("source/a-done"), b"Completed prefix").unwrap();
    fs::write(root.join("source/b-current"), vec![b'x'; 128 * 1024]).unwrap();
    fs::write(root.join("source/z-later"), b"Later leaf").unwrap();
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
            .find(|j| j.id == id && !j.phase.holds_application())
    });
    assert_eq!(stopped.phase, Phase::Stopped, "{stopped:?}");
    assert_eq!(mode(&root.join("output/source")), 0o555);
    fs::write(root.join("source/a-done"), b"Edited completed source").unwrap();
    chmod(&root.join("output/source"), 0o700);
    fs::write(
        root.join("output/source/a-done"),
        b"Edited completed output",
    )
    .unwrap();
    chmod(&root.join("output/source"), 0o400);
    fs::write(root.join("resumed"), b"continue with restricted parent").unwrap();
    receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap()).unwrap();
    std::thread::sleep(Duration::from_secs(25));
    panic!(
        "private restricted-parent boundary was not reached: {:?}",
        jobs.snapshot()
    );
}
fn make_removable(path: &Path) {
    if fs::symlink_metadata(path).unwrap().is_dir() {
        chmod(path, 0o700);
        for entry in fs::read_dir(path).unwrap() {
            make_removable(&entry.unwrap().path());
        }
    }
}
fn fixture(run: impl FnOnce(&Path, Marker, &[u8])) {
    let root = tempfile::Builder::new()
        .prefix("canopa-stage-access-")
        .tempdir()
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::stage_boundary::access_checks::access_child",
            ])
            .env("CANOPA_STAGE_ACCESS_ROOT", root.path())
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let marker: Marker =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        let bytes = b"Unique interrupted edits beneath restricted parent";
        fs::write(work(&marker).payload(), bytes).unwrap();
        run(root.path(), marker, bytes);
    }));
    if let Err(error) = checked {
        eprintln!(
            "retained restricted-parent fixture: {}",
            root.keep().display()
        );
        std::panic::resume_unwind(error);
    }
    make_removable(root.path());
}

fn partial(jobs: &OperationJobs, bytes: &[u8]) -> crate::operations::RecoveryItem {
    receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
        .unwrap()
        .items
        .into_iter()
        .find(|item| {
            item.entry
                .as_ref()
                .is_some_and(|entry| fs::read(&entry.path).is_ok_and(|actual| actual == bytes))
        })
        .expect("unique partial bytes are accessible through public Recovery")
}
fn grants(root: &Path) -> u64 {
    rusqlite::Connection::open(root.join("history"))
        .unwrap()
        .query_row("SELECT count(*) FROM directory_read_grants", [], |row| {
            row.get(0)
        })
        .unwrap()
}
#[test]
#[ignore = "private Recovery selection killed while borrowing read/search access"]
fn access_reader_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_STAGE_ACCESS_ROOT").unwrap());
    let jobs = start(&root);
    assert_ne!(reconcile(&jobs).phase, Phase::Finished);
    let item = partial(&jobs, b"Unique interrupted edits beneath restricted parent");
    assert_eq!(mode(&root.join("output/source")), 0o500);
    assert_eq!(grants(&root), 1);
    fs::write(
        root.join("reader-cut.json"),
        serde_json::to_vec(&item).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private Recovery reader was not killed");
}
#[test]
fn a_killed_recovery_reader_restores_restricted_permissions_on_restart() {
    fixture(|root, marker, bytes| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::stage_boundary::access_checks::access_reader_child",
            ])
            .env("CANOPA_STAGE_ACCESS_ROOT", root)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        assert_eq!(mode(&marker.output), 0o500);
        assert_eq!(grants(root), 1);
        let jobs = start(root);
        assert_eq!(mode(&marker.output), 0o400);
        assert_eq!(grants(root), 0);
        let item = partial(&jobs, bytes);
        assert_eq!(mode(&marker.output), 0o500);
        drop(item);
        close(jobs, root);
        assert_eq!(mode(&marker.output), 0o400);
    });
}
#[test]
fn recovery_read_access_never_changes_a_replacement_folder_or_external_mode_edit() {
    fixture(|root, marker, bytes| {
        let jobs = start(root);
        assert_ne!(reconcile(&jobs).phase, Phase::Finished);
        chmod(&marker.output, 0o500);
        assert_eq!(grants(root), 0);
        assert_ne!(reconcile(&jobs).phase, Phase::Finished);
        assert_eq!(mode(&marker.output), 0o500);
        let external = partial(&jobs, bytes);
        drop(external);
        assert_eq!(grants(root), 0);
        assert_eq!(mode(&marker.output), 0o500);
        chmod(&marker.output, 0o400);
        let item = partial(&jobs, bytes);
        let held = root.join("output/held-original");
        fs::rename(&marker.output, &held).unwrap();
        fs::create_dir(&marker.output).unwrap();
        fs::write(marker.output.join("replacement"), b"Replacement data").unwrap();
        chmod(&marker.output, 0o411);
        drop(item);
        wait(|| (mode(&held) == 0o400).then_some(()));
        assert_eq!(mode(&marker.output), 0o411);
        let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
        assert!(page.items.iter().all(|item| item.entry.is_none()));
        drop(page);
        assert_ne!(reconcile(&jobs).phase, Phase::Finished);
        assert_eq!(mode(&marker.output), 0o411);
        fs::rename(&marker.output, root.join("output/kept-replacement")).unwrap();
        fs::rename(&held, &marker.output).unwrap();
        let item = partial(&jobs, bytes);
        chmod(&marker.output, 0o701);
        drop(item);
        wait(|| (grants(root) == 0).then_some(()));
        assert_eq!(mode(&marker.output), 0o701);
        let item = partial(&jobs, bytes);
        drop(item);
        close(jobs, root);
        assert_eq!(mode(&marker.output), 0o701);
        assert_eq!(mode(&root.join("output/kept-replacement")), 0o411);
    });
}
#[test]
fn refused_or_ignored_recovery_grant_saves_never_change_permissions() {
    for result in ["ABORT,'private grant save refusal'", "IGNORE"] {
        fixture(|root, marker, bytes| {
            let jobs = start(root);
            assert_ne!(reconcile(&jobs).phase, Phase::Finished);
            let cancellation = Cancellation::default();
            cancellation.cancel();
            assert!(receive(jobs.recovery_page(None, cancellation).unwrap()).is_err());
            assert_eq!(mode(&marker.output), 0o400);
            assert_eq!(grants(root), 0);
            let db = rusqlite::Connection::open(root.join("history")).unwrap();
            db.execute_batch(&format!("CREATE TRIGGER refuse_read_access BEFORE INSERT ON directory_read_grants BEGIN SELECT RAISE({result}); END;")).unwrap();
            let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
            assert!(page.items.iter().all(|item| item.entry.is_none()));
            drop(page);
            assert_eq!(mode(&marker.output), 0o400);
            assert_eq!(grants(root), 0);
            db.execute_batch("DROP TRIGGER refuse_read_access").unwrap();
            let item = partial(&jobs, bytes);
            drop(item);
            close(jobs, root);
            assert_eq!(mode(&marker.output), 0o400);
        });
    }
}

#[test]
fn restarted_partial_beneath_a_nonsearchable_created_parent_is_recoverable() {
    fixture(|root, marker, bytes| {
        let jobs = start(root);
        fs::write(
            root.join("startup-jobs.txt"),
            format!("{:?}", jobs.snapshot()),
        )
        .unwrap();
        let reconciled = reconcile(&jobs);
        fs::write(root.join("reconciled.txt"), format!("{reconciled:?}")).unwrap();
        assert_ne!(reconciled.phase, Phase::Finished);
        assert_eq!(mode(&marker.output), 0o400);
        let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
        fs::write(
            root.join("recovery-page.json"),
            serde_json::to_vec_pretty(&page.items).unwrap(),
        )
        .unwrap();
        let item = page
            .items
            .into_iter()
            .find(|item| {
                item.entry
                    .as_ref()
                    .is_some_and(|entry| fs::read(&entry.path).is_ok_and(|actual| actual == bytes))
            })
            .expect("partial bytes are publicly recoverable beneath the restricted created parent");
        assert!(item.can_keep_in_recovery());
        assert_eq!(mode(&marker.output), 0o500);
        drop(item);
        wait(|| (mode(&marker.output) == 0o400).then_some(()));
        let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
        let item = page
            .items
            .into_iter()
            .find(|item| {
                item.entry
                    .as_ref()
                    .is_some_and(|entry| fs::read(&entry.path).is_ok_and(|actual| actual == bytes))
            })
            .unwrap();
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
                .find(|j| j.id == copied && !j.phase.holds_application())
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
                .find(|j| j.id == kept && !j.phase.holds_application())
        });
        assert_eq!(result.phase, Phase::Finished, "{result:?}");
        let original = jobs
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == marker.request)
            .unwrap();
        assert!(original.transfer_actions.remaining, "{original:?}");
        let resumed = receive(
            jobs.resume_transfer(marker.request, TransferPart::Remaining)
                .unwrap(),
        )
        .unwrap();
        let result = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == resumed && !j.phase.holds_application())
        });
        assert_eq!(result.phase, Phase::Finished, "{result:?}");
        assert_eq!(
            fs::read(marker.output.join("a-done")).unwrap(),
            b"Edited completed output"
        );
        assert_eq!(
            fs::read(marker.output.join("b-current")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        assert_eq!(
            fs::read(marker.output.join("z-later")).unwrap(),
            b"Later leaf"
        );
        drop(item);
        close(jobs, root);
        assert_eq!(mode(&marker.output), 0o400);
        let jobs = start(root);
        let page = receive(jobs.recovery_page(None, Cancellation::default()).unwrap()).unwrap();
        assert!(page.items.iter().any(|item| {
            item.entry
                .as_ref()
                .is_some_and(|entry| fs::read(&entry.path).is_ok_and(|actual| actual == bytes))
        }));
        drop(page);
        close(jobs, root);
        assert_eq!(mode(&marker.output), 0o400);
    });
}
