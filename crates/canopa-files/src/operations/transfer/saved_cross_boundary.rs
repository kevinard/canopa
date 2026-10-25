//! Actual saved-container cross-filesystem tasks through the public worker.
use super::*;
use crate::operation_jobs::{OperationJobs, Phase, Request, TransferPart};
use std::{
    os::unix::{fs::MetadataExt, process::ExitStatusExt},
    time::{Duration, Instant},
};
type MetadataHook = Box<dyn FnOnce(&Path)>;
thread_local! {
    static METADATA: std::cell::RefCell<Option<MetadataHook>> = const { std::cell::RefCell::new(None) };
}
pub(in crate::operations) fn before_metadata(path: &Path) {
    if let Some(hook) = METADATA.with(|slot| slot.borrow_mut().take()) {
        hook(path);
    }
}

#[test]
fn saved_copy_metadata_uses_its_opened_link_and_preserves_a_substituted_payload() {
    use std::{cell::RefCell, ffi::OsString, os::unix::ffi::OsStringExt, rc::Rc};
    for replaced in [false, true] {
        let root = tempfile::Builder::new()
            .prefix("canopa-saved-cross-metadata-")
            .tempdir()
            .unwrap();
        let destination = tempfile::Builder::new()
            .prefix(".canopa-saved-cross-metadata-output-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_ne!(
                fs::metadata(root.path()).unwrap().dev(),
                fs::metadata(destination.path()).unwrap().dev()
            );
            fs::create_dir(root.path().join("seed")).unwrap();
            fs::create_dir(root.path().join("incoming")).unwrap();
            fs::write(root.path().join("seed/item"), b"Selected replacement").unwrap();
            let target = PathBuf::from(OsString::from_vec(b"missing-\xff".to_vec()));
            let original = root.path().join("incoming/item");
            std::os::unix::fs::symlink(&target, &original).unwrap();
            rustix::fs::utimensat(
                rustix::fs::CWD,
                &original,
                &rustix::fs::Timestamps {
                    last_access: rustix::fs::Timespec {
                        tv_sec: 0,
                        tv_nsec: rustix::fs::UTIME_OMIT,
                    },
                    last_modification: rustix::fs::Timespec {
                        tv_sec: 946684800,
                        tv_nsec: 123456789,
                    },
                },
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .unwrap();
            let engine = OperationEngine::open(root.path().join("history")).unwrap();
            let result = engine
                .transfer(
                    &TransferRequest {
                        kind: TransferKind::Copy,
                        sources: vec![root.path().join("seed/item")],
                        destination: root.path().join("incoming"),
                    },
                    &Cancellation::default(),
                    |_| TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        apply_to_remaining: false,
                    },
                )
                .unwrap();
            assert_eq!(result.completed, 1);
            let page = engine
                .recovery_items(None, &Cancellation::default())
                .unwrap();
            let source = page.items[0]
                .entry
                .as_ref()
                .unwrap()
                .path
                .parent()
                .unwrap()
                .to_owned();
            let witness = Rc::new(RefCell::new(None));
            if replaced {
                let witness = witness.clone();
                METADATA.with(|slot| {
                    *slot.borrow_mut() = Some(Box::new(move |path| {
                        fs::rename(path, path.with_extension("held-link")).unwrap();
                        fs::write(path, b"Replacement prepared payload").unwrap();
                        let metadata = fs::symlink_metadata(path).unwrap();
                        *witness.borrow_mut() =
                            Some((path.to_owned(), metadata.mtime(), metadata.mtime_nsec()));
                    }))
                });
            }
            let result = engine
                .transfer(
                    &TransferRequest {
                        kind: TransferKind::Move,
                        sources: vec![source.clone()],
                        destination: destination.path().to_owned(),
                    },
                    &Cancellation::default(),
                    |_| TransferReply::Cancel,
                )
                .unwrap();
            METADATA.with(|slot| {
                slot.borrow_mut().take();
            });
            if replaced {
                assert_eq!(result.completed, 0);
                let (path, seconds, nanos) = witness
                    .borrow()
                    .clone()
                    .expect("actual metadata restoration hook was not reached");
                let metadata = fs::symlink_metadata(&path).unwrap();
                assert_eq!((metadata.mtime(), metadata.mtime_nsec()), (seconds, nanos));
                assert_eq!(fs::read(&path).unwrap(), b"Replacement prepared payload");
                assert_eq!(fs::read_link(source.join("previous")).unwrap(), target);
                assert!(
                    !destination
                        .path()
                        .join(source.file_name().unwrap())
                        .exists()
                );
            } else {
                assert_eq!(result.completed, 1, "{result:?}");
                assert!(result.failures.is_empty());
                let previous = destination
                    .path()
                    .join(source.file_name().unwrap())
                    .join("previous");
                let metadata = fs::symlink_metadata(&previous).unwrap();
                assert_eq!(
                    (metadata.mtime(), metadata.mtime_nsec()),
                    (946684800, 123456789)
                );
                assert_eq!(fs::read_link(previous).unwrap(), target);
            }
        }));
        METADATA.with(|slot| {
            slot.borrow_mut().take();
        });
        if let Err(error) = checked {
            eprintln!(
                "retained saved-cross metadata source: {}",
                root.keep().display()
            );
            eprintln!(
                "retained saved-cross metadata destination: {}",
                destination.keep().display()
            );
            std::panic::resume_unwind(error);
        }
    }
}

fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "private cross-filesystem saved worker did not settle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn receive<T>(mut reply: futures_channel::oneshot::Receiver<T>) -> T {
    wait(|| {
        reply
            .try_recv()
            .expect("private cross-filesystem worker dropped its reply")
    })
}
fn start(root: &Path) -> OperationJobs {
    let jobs = OperationJobs::start(root.join("history"), root.join("volumes")).unwrap();
    wait(|| (!jobs.snapshot().initializing).then_some(()));
    assert!(jobs.snapshot().error.is_none());
    jobs
}
fn close(jobs: OperationJobs, root: &Path) {
    drop(jobs);
    drop(wait(|| match OperationEngine::open(root.join("history")) {
        Ok(engine) => Some(engine),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => None,
        Err(error) => panic!("{error}"),
    }));
}
#[derive(Serialize, Deserialize)]
struct Marker {
    request: u64,
    #[serde(default)]
    migration: Option<Box<saved_stage::Migration>>,
    #[serde(with = "crate::path_serde")]
    source: PathBuf,
    #[serde(with = "crate::path_serde")]
    destination: PathBuf,
}
pub(in crate::operations) fn boundary(point: &str, source: &Path, destination: &Path) {
    write_boundary(point, source, destination, None);
}
pub(in crate::operations) fn migration(point: &str, migration: &saved_stage::Migration) {
    write_boundary(
        point,
        &migration.source.directory,
        migration.destination(),
        Some(migration),
    );
}
fn write_boundary(
    point: &str,
    source: &Path,
    destination: &Path,
    migration: Option<&saved_stage::Migration>,
) {
    let Some(root) = std::env::var_os("CANOPA_SAVED_CROSS_ROOT").map(PathBuf::from) else {
        return;
    };
    if std::env::var("CANOPA_SAVED_CROSS_POINT").as_deref() != Ok(point) {
        return;
    }
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-saved-cross-")
    );
    assert_eq!(source.parent(), Some(root.join("incoming").as_path()));
    let target = PathBuf::from(std::env::var_os("CANOPA_SAVED_CROSS_DESTINATION").unwrap());
    let parent = if std::env::var("CANOPA_SAVED_CROSS_SHAPE").as_deref() == Ok("nested") {
        target.join("incoming")
    } else {
        target.clone()
    };
    assert_eq!(destination.parent(), Some(parent.as_path()));
    assert_ne!(
        fs::metadata(root.join("incoming")).unwrap().dev(),
        fs::metadata(&target).unwrap().dev()
    );
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let request = db
        .query_row("SELECT request FROM request_transfer_cursors", [], |r| {
            r.get(0)
        })
        .unwrap();
    fs::write(
        root.join("marker.json"),
        serde_json::to_vec(&Marker {
            request,
            migration: migration.cloned().map(Box::new),
            source: source.to_owned(),
            destination: destination.to_owned(),
        })
        .unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private cross-filesystem saved worker was not killed");
}
fn fixture(run: impl FnOnce(&Path, Marker)) {
    fixture_at("finished", run);
}
fn fixture_at(point: &str, run: impl FnOnce(&Path, Marker)) {
    fixture_shape(point, "file", run);
}
fn fixture_shape(point: &str, shape: &str, run: impl FnOnce(&Path, Marker)) {
    let root = tempfile::Builder::new()
        .prefix("canopa-saved-cross-")
        .tempdir()
        .unwrap();
    let destination = tempfile::Builder::new()
        .prefix(".canopa-saved-cross-output-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    assert_ne!(
        fs::metadata(root.path()).unwrap().dev(),
        fs::metadata(destination.path()).unwrap().dev()
    );
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::saved_cross_boundary::saved_cross_child",
            ])
            .env("CANOPA_SAVED_CROSS_ROOT", root.path())
            .env("CANOPA_SAVED_CROSS_POINT", point)
            .env("CANOPA_SAVED_CROSS_SHAPE", shape)
            .env("CANOPA_SAVED_CROSS_DESTINATION", destination.path())
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let marker =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        run(root.path(), marker);
    }));
    if let Err(error) = checked {
        let held = destination.path().with_extension("held-original");
        if held.exists() {
            eprintln!("retained saved-cross destination root: {}", held.display());
        }
        eprintln!(
            "retained saved-cross source root: {}",
            root.keep().display()
        );
        eprintln!(
            "retained saved-cross destination root: {}",
            destination.keep().display()
        );
        std::panic::resume_unwind(error);
    }
}
#[test]
fn a_completed_cross_filesystem_saved_move_restores_only_its_result() {
    fixture(|root, marker| {
        let identity = Identity::read(&marker.destination).unwrap();
        fs::create_dir(&marker.source).unwrap();
        fs::write(marker.source.join("foreign"), b"New source occupant").unwrap();
        fs::write(
            marker.destination.join("previous"),
            b"Edited completed saved data",
        )
        .unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            let row = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == marker.request)
                .unwrap();
            assert_eq!(row.phase, Phase::Finished, "{row:?}");
            assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
            assert!(!row.transfer_actions.retry && !row.transfer_actions.remaining);
            assert_eq!(Identity::read(&marker.destination).unwrap(), identity);
            assert_eq!(
                fs::read(marker.destination.join("previous")).unwrap(),
                b"Edited completed saved data"
            );
            assert_eq!(
                fs::read(marker.source.join("foreign")).unwrap(),
                b"New source occupant"
            );
            close(jobs, root);
        }
    });
}

fn reconcile(jobs: &OperationJobs) -> crate::operation_jobs::JobSnapshot {
    let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|j| j.id == id && !j.phase.holds_application())
    })
}
fn job(jobs: &OperationJobs, id: u64) -> crate::operation_jobs::JobSnapshot {
    jobs.snapshot()
        .jobs
        .into_iter()
        .find(|j| j.id == id)
        .unwrap()
}
#[test]
fn copied_saved_claims_reconcile_all_publication_and_cleanup_crash_phases() {
    for point in [
        "prepared",
        "normalized",
        "installed",
        "catalogued",
        "root-intent",
        "root-ready",
        "root-claimed",
        "file-ready",
        "file-claimed",
        "unlinked",
        "root-removed",
    ] {
        fixture_at(point, |root, marker| {
            let migration = marker.migration.as_ref().unwrap();
            let claim = saved_stage::forward::claim_path(migration);
            let reused = matches!(
                point,
                "root-ready"
                    | "root-claimed"
                    | "file-ready"
                    | "file-claimed"
                    | "unlinked"
                    | "root-removed"
            );
            if reused {
                fs::create_dir(&marker.source).unwrap();
                fs::write(marker.source.join("foreign"), b"New source occupant").unwrap();
            }
            let jobs = start(root);
            let review = reconcile(&jobs);
            assert_eq!(review.phase, Phase::Finished, "{point}: {review:?}");
            let row = job(&jobs, marker.request);
            assert_eq!(row.phase, Phase::Finished, "{point}: {row:?}");
            assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
            assert_eq!(row.transfer.as_ref().unwrap().bytes, 19);
            assert_eq!(
                Identity::read(&marker.destination).unwrap(),
                migration.stage.identity
            );
            assert_eq!(
                fs::read(marker.destination.join("previous")).unwrap(),
                b"Original saved data"
            );
            assert!(!claim.exists());
            if !reused {
                fs::create_dir(&marker.source).unwrap();
                fs::write(marker.source.join("foreign"), b"New source occupant").unwrap();
            }
            assert_eq!(
                fs::read(marker.source.join("foreign")).unwrap(),
                b"New source occupant"
            );
            close(jobs, root);
            fs::write(
                marker.destination.join("previous"),
                b"Edited completed output",
            )
            .unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                assert_eq!(job(&jobs, marker.request).phase, Phase::Finished);
                assert_eq!(
                    fs::read(marker.destination.join("previous")).unwrap(),
                    b"Edited completed output"
                );
                assert_eq!(
                    fs::read(marker.source.join("foreign")).unwrap(),
                    b"New source occupant"
                );
                close(jobs, root);
            }
        });
    }
}
#[test]
fn a_new_undo_archive_keeps_its_original_claim_through_copied_move_crashes() {
    for point in ["prepared", "catalogued", "file-claimed", "finished"] {
        fixture_shape(point, "archive", |root, marker| {
            let jobs = start(root);
            let review = reconcile(&jobs);
            assert_eq!(review.phase, Phase::Finished, "{point}: {review:?}");
            let result = job(&jobs, marker.request);
            assert_eq!(result.phase, Phase::Finished, "{point}: {result:?}");
            assert_eq!(result.transfer.as_ref().unwrap().completed, 1);
            assert_eq!(result.transfer.as_ref().unwrap().bytes, 19);
            assert_eq!(
                fs::read(marker.destination.join("previous")).unwrap(),
                b"Original saved data"
            );
            close(jobs, root);
            fs::create_dir(&marker.source).unwrap();
            fs::write(marker.source.join("foreign"), b"New archived source work").unwrap();
            fs::write(
                marker.destination.join("previous"),
                b"Edited archive output",
            )
            .unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                assert_eq!(job(&jobs, marker.request).phase, Phase::Finished);
                assert_eq!(
                    fs::read(marker.source.join("foreign")).unwrap(),
                    b"New archived source work"
                );
                assert_eq!(
                    fs::read(marker.destination.join("previous")).unwrap(),
                    b"Edited archive output"
                );
                close(jobs, root);
            }
        });
    }
}
#[test]
fn consumed_saved_claims_finish_without_reopening_changed_output_names() {
    for point in ["unlinked", "root-removed"] {
        for changed in ["edit", "root", "parent"] {
            fixture_at(point, |root, marker| {
                let claim = saved_stage::forward::claim_path(marker.migration.as_ref().unwrap());
                fs::create_dir(&marker.source).unwrap();
                fs::write(marker.source.join("foreign"), b"New source after unlink").unwrap();
                let held = match changed {
                    "root" => {
                        let held = marker.destination.with_extension("held-original");
                        fs::rename(&marker.destination, &held).unwrap();
                        fs::create_dir(&marker.destination).unwrap();
                        Some(held)
                    }
                    "parent" => {
                        let parent = marker.destination.parent().unwrap();
                        let held = parent.with_extension("held-original");
                        fs::rename(parent, &held).unwrap();
                        fs::create_dir(parent).unwrap();
                        fs::create_dir(&marker.destination).unwrap();
                        Some(held)
                    }
                    _ => None,
                };
                fs::write(
                    marker.destination.join("previous"),
                    b"New output after unlink",
                )
                .unwrap();
                for _ in 0..2 {
                    let jobs = start(root);
                    let review = reconcile(&jobs);
                    assert_eq!(
                        review.phase,
                        Phase::Finished,
                        "{point}/{changed}: {review:?}"
                    );
                    let result = job(&jobs, marker.request);
                    assert_eq!(
                        result.phase,
                        Phase::Finished,
                        "{point}/{changed}: {result:?}"
                    );
                    assert_eq!(result.transfer.as_ref().unwrap().completed, 1);
                    assert_eq!(result.transfer.as_ref().unwrap().bytes, 19);
                    assert_eq!(
                        fs::read(marker.destination.join("previous")).unwrap(),
                        b"New output after unlink"
                    );
                    assert_eq!(
                        fs::read(marker.source.join("foreign")).unwrap(),
                        b"New source after unlink"
                    );
                    assert!(!claim.exists());
                    close(jobs, root);
                }
                if let Some(held) = held {
                    let previous = if changed == "parent" {
                        held.join(marker.destination.file_name().unwrap())
                            .join("previous")
                    } else {
                        held.join("previous")
                    };
                    assert_eq!(fs::read(previous).unwrap(), b"Original saved data");
                    fs::remove_dir_all(held).unwrap();
                }
            });
        }
    }
}
#[test]
fn copied_saved_claims_never_adopt_the_same_original_root_in_a_new_source_namespace() {
    for point in ["root-claimed", "file-claimed"] {
        fixture_at(point, |root, marker| {
            let migration = marker.migration.as_ref().unwrap();
            let claim = saved_stage::forward::claim_path(migration);
            fs::rename(&claim, &marker.source).unwrap();
            let data = marker.source.join(if point == "root-claimed" {
                "previous"
            } else {
                "folder"
            });
            fs::write(&data, b"Same inode, new namespace work").unwrap();
            assert_eq!(
                Identity::read(&marker.source).unwrap(),
                migration.source.identity
            );
            for _ in 0..2 {
                let jobs = start(root);
                let review = reconcile(&jobs);
                assert_eq!(review.phase, Phase::Finished, "{point}: {review:?}");
                assert_eq!(job(&jobs, marker.request).phase, Phase::Finished);
                assert_eq!(fs::read(&data).unwrap(), b"Same inode, new namespace work");
                assert_eq!(
                    fs::read(marker.destination.join("previous")).unwrap(),
                    b"Original saved data"
                );
                close(jobs, root);
            }
        });
    }
}
#[test]
fn copied_saved_claims_refuse_missing_receipts_context_and_changed_phases_before_mutation() {
    for point in ["prepared", "catalogued"] {
        for bad in [
            "receipt",
            "receipt-digest",
            "context",
            "phase",
            "phase-missing",
        ] {
            fixture_at(point, |root, marker| {
                let db = rusqlite::Connection::open(root.join("history")).unwrap();
                match bad {
                    "receipt" => {
                        db.execute("DELETE FROM request_transfer_cursor_steps", [])
                            .unwrap();
                    }
                    "receipt-digest" => {
                        db.execute(
                            "UPDATE request_transfer_cursor_steps SET body_sha256=zeroblob(32)",
                            [],
                        )
                        .unwrap();
                    }
                    "context" => {
                        db.execute("DELETE FROM pending_bindings", []).unwrap();
                    }
                    "phase" | "phase-missing" => {
                        let body: String = db
                            .query_row("SELECT body FROM pending", [], |r| r.get(0))
                            .unwrap();
                        let mut body: serde_json::Value = serde_json::from_str(&body).unwrap();
                        let object = body.get_mut("migration").unwrap();
                        if bad == "phase" {
                            object["cleanup"] = serde_json::json!("file-claimed");
                        } else {
                            object.as_object_mut().unwrap().remove("cleanup");
                        }
                        db.execute(
                            "UPDATE pending SET body=?1",
                            [serde_json::to_string(&body).unwrap()],
                        )
                        .unwrap();
                    }
                    _ => unreachable!(),
                }
                drop(db);
                for _ in 0..2 {
                    let jobs = start(root);
                    let review = reconcile(&jobs);
                    assert_ne!(review.phase, Phase::Finished, "{point}/{bad}: {review:?}");
                    assert_eq!(job(&jobs, marker.request).phase, Phase::Interrupted);
                    assert_eq!(
                        fs::read(marker.source.join("previous")).unwrap(),
                        b"Original saved data"
                    );
                    if point == "prepared" {
                        assert!(!marker.destination.exists());
                        assert_eq!(
                            fs::read(marker.migration.as_ref().unwrap().stage.payload()).unwrap(),
                            b"Original saved data"
                        );
                    } else {
                        assert_eq!(
                            fs::read(marker.destination.join("previous")).unwrap(),
                            b"Original saved data"
                        );
                    }
                    close(jobs, root);
                }
            });
        }
    }
}

#[test]
fn copied_saved_claims_refuse_original_parent_work_and_payload_substitutions() {
    for part in ["source-parent", "destination-parent", "work", "payload"] {
        fixture_at("prepared", |root, marker| {
            let migration = marker.migration.as_ref().unwrap();
            let (selected, held) = match part {
                "source-parent" => (root.join("incoming"), root.join("held-incoming")),
                "destination-parent" => {
                    let parent = marker.destination.parent().unwrap();
                    (parent.to_owned(), parent.with_extension("held-original"))
                }
                "work" => (
                    migration.stage.directory.clone(),
                    marker.destination.parent().unwrap().join("held-work"),
                ),
                "payload" => (
                    migration.stage.payload(),
                    marker.destination.parent().unwrap().join("held-payload"),
                ),
                _ => unreachable!(),
            };
            fs::rename(&selected, &held).unwrap();
            if part == "payload" {
                fs::write(&selected, b"Foreign prepared payload").unwrap();
            } else {
                fs::create_dir(&selected).unwrap();
                fs::write(selected.join("foreign"), b"Foreign folder occupant").unwrap();
            }
            for _ in 0..2 {
                let jobs = start(root);
                let review = reconcile(&jobs);
                assert_ne!(review.phase, Phase::Finished, "{part}: {review:?}");
                assert_eq!(job(&jobs, marker.request).phase, Phase::Interrupted);
                if part == "payload" {
                    assert_eq!(fs::read(&selected).unwrap(), b"Foreign prepared payload");
                } else {
                    assert_eq!(
                        fs::read(selected.join("foreign")).unwrap(),
                        b"Foreign folder occupant"
                    );
                }
                close(jobs, root);
            }
            if part == "payload" {
                fs::remove_file(&selected).unwrap();
            } else {
                fs::remove_file(selected.join("foreign")).unwrap();
                fs::remove_dir(&selected).unwrap();
            }
            fs::rename(&held, &selected).unwrap();
            let jobs = start(root);
            let review = reconcile(&jobs);
            assert_eq!(review.phase, Phase::Finished, "{part}: {review:?}");
            assert_eq!(job(&jobs, marker.request).phase, Phase::Finished);
            assert_eq!(
                fs::read(marker.destination.join("previous")).unwrap(),
                b"Original saved data"
            );
            close(jobs, root);
        });
    }
}
#[test]
fn copied_saved_claims_preserve_new_private_payloads_after_a_verified_unlink() {
    for name in ["folder", "previous", "other"] {
        fixture_at("unlinked", |root, marker| {
            let migration = marker.migration.as_ref().unwrap();
            let claim = saved_stage::forward::claim_path(migration);
            let foreign = claim.join(name);
            fs::write(&foreign, b"New private work").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let review = reconcile(&jobs);
                assert_ne!(review.phase, Phase::Finished, "{name}: {review:?}");
                assert_eq!(job(&jobs, marker.request).phase, Phase::Interrupted);
                assert_eq!(fs::read(&foreign).unwrap(), b"New private work");
                assert_eq!(
                    fs::read(marker.destination.join("previous")).unwrap(),
                    b"Original saved data"
                );
                close(jobs, root);
            }
            fs::remove_file(&foreign).unwrap();
            let jobs = start(root);
            let review = reconcile(&jobs);
            assert_eq!(review.phase, Phase::Finished, "{name}: {review:?}");
            assert_eq!(job(&jobs, marker.request).phase, Phase::Finished);
            close(jobs, root);
        });
    }
}
#[test]
fn copied_saved_claim_cutover_rolls_back_catalog_receipt_and_phase_together() {
    fixture_at("installed", |root, marker| {
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        let original: String = db
            .query_row(
                "SELECT body FROM changes WHERE operation=1 AND ordinal=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        db.execute_batch("CREATE TRIGGER refuse_saved_cutover BEFORE INSERT ON changes WHEN NEW.operation=2 BEGIN SELECT RAISE(ABORT,'private cutover refusal'); END;").unwrap();
        drop(db);
        let jobs = start(root);
        let review = reconcile(&jobs);
        assert_ne!(review.phase, Phase::Finished, "{review:?}");
        assert_eq!(job(&jobs, marker.request).phase, Phase::Interrupted);
        assert_eq!(
            fs::read(marker.source.join("previous")).unwrap(),
            b"Original saved data"
        );
        close(jobs, root);
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        let after: String = db
            .query_row(
                "SELECT body FROM changes WHERE operation=1 AND ordinal=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after, original);
        assert_eq!(
            db.query_row("SELECT count(*) FROM changes WHERE operation=2", [], |r| {
                r.get::<_, u64>(0)
            })
            .unwrap(),
            0
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM request_transfer_cursor_steps",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
            1
        );
        db.execute_batch("DROP TRIGGER refuse_saved_cutover;")
            .unwrap();
        drop(db);
        let jobs = start(root);
        let review = reconcile(&jobs);
        assert_eq!(review.phase, Phase::Finished, "{review:?}");
        assert_eq!(job(&jobs, marker.request).phase, Phase::Finished);
        close(jobs, root);
    });
}

fn link_target() -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    std::ffi::OsString::from_vec(b"missing-\xff".to_vec()).into()
}
#[test]
fn copied_saved_links_reconcile_publication_and_claims_with_zero_copied_bytes() {
    for point in ["installed", "file-claimed"] {
        fixture_shape(point, "link", |root, marker| {
            let jobs = start(root);
            let review = reconcile(&jobs);
            assert_eq!(review.phase, Phase::Finished, "{point}: {review:?}");
            let row = job(&jobs, marker.request);
            assert_eq!(row.phase, Phase::Finished, "{row:?}");
            assert_eq!(row.transfer.as_ref().unwrap().completed, 1);
            assert_eq!(row.transfer.as_ref().unwrap().bytes, 0);
            assert_eq!(
                fs::read_link(marker.destination.join("previous")).unwrap(),
                link_target()
            );
            close(jobs, root);
        });
    }
}
#[test]
fn copied_saved_keep_both_and_nested_steps_resume_only_the_later_batch_work() {
    for shape in ["batch", "nested"] {
        fixture_shape("finished", shape, |root, marker| {
            let completed = marker
                .destination
                .parent()
                .unwrap()
                .join("completed-original");
            fs::rename(&marker.destination, &completed).unwrap();
            fs::create_dir(&marker.destination).unwrap();
            fs::write(marker.destination.join("foreign"), b"New destination work").unwrap();
            fs::create_dir(&marker.source).unwrap();
            fs::write(marker.source.join("foreign"), b"New source work").unwrap();
            for _ in 0..2 {
                let jobs = start(root);
                let row = job(&jobs, marker.request);
                assert_eq!(row.phase, Phase::Stopped, "{shape}: {row:?}");
                assert!(row.transfer_actions.remaining);
                assert_eq!(
                    row.transfer.as_ref().unwrap().completed,
                    if shape == "batch" { 1 } else { 0 }
                );
                assert_eq!(row.transfer.as_ref().unwrap().bytes, 19);
                close(jobs, root);
            }
            let jobs = start(root);
            let id = receive(
                jobs.resume_transfer(marker.request, TransferPart::Remaining)
                    .unwrap(),
            )
            .unwrap();
            let done = wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|j| j.id == id && !j.phase.holds_application())
            });
            assert_eq!(done.phase, Phase::Finished, "{shape}: {done:?}");
            assert_eq!(
                fs::read(completed.join("previous")).unwrap(),
                b"Original saved data"
            );
            assert_eq!(
                fs::read(marker.destination.join("foreign")).unwrap(),
                b"New destination work"
            );
            assert_eq!(
                fs::read(marker.source.join("foreign")).unwrap(),
                b"New source work"
            );
            if shape == "batch" {
                assert_eq!(
                    fs::read(marker.destination.parent().unwrap().join("z-tail")).unwrap(),
                    b"Later selected root"
                );
                assert_eq!(
                    fs::read(
                        marker
                            .destination
                            .parent()
                            .unwrap()
                            .join(marker.source.file_name().unwrap())
                            .join("foreign")
                    )
                    .unwrap(),
                    b"Existing conflict"
                );
            } else {
                assert_eq!(
                    fs::read(marker.destination.parent().unwrap().join("item")).unwrap(),
                    b"Selected replacement"
                );
            }
            close(jobs, root);
        });
    }
}

#[test]
fn a_second_saved_cleanup_crash_preserves_new_source_names_and_unknown_work() {
    fixture_at("catalogued", |root, _marker| {
        let initial: Marker =
            serde_json::from_slice(&fs::read(root.join("marker.json")).unwrap()).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::saved_cross_boundary::saved_reconcile_child",
            ])
            .env("CANOPA_SAVED_CROSS_ROOT", root)
            .env(
                "CANOPA_SAVED_CROSS_DESTINATION",
                initial.destination.parent().unwrap(),
            )
            .env("CANOPA_SAVED_CROSS_POINT", "file-claimed")
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let marker: Marker =
            serde_json::from_slice(&fs::read(root.join("marker.json")).unwrap()).unwrap();
        let claim = saved_stage::forward::claim_path(marker.migration.as_ref().unwrap());
        fs::create_dir(&marker.source).unwrap();
        fs::write(
            marker.source.join("foreign"),
            b"New source after second crash",
        )
        .unwrap();
        let foreign = claim.join("other");
        fs::write(&foreign, b"New private work after second crash").unwrap();
        let jobs = start(root);
        let review = reconcile(&jobs);
        assert_ne!(review.phase, Phase::Finished, "{review:?}");
        assert_eq!(
            fs::read(&foreign).unwrap(),
            b"New private work after second crash"
        );
        assert_eq!(
            fs::read(marker.source.join("foreign")).unwrap(),
            b"New source after second crash"
        );
        close(jobs, root);
        fs::remove_file(&foreign).unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            let review = reconcile(&jobs);
            assert_eq!(review.phase, Phase::Finished, "{review:?}");
            assert_eq!(job(&jobs, marker.request).phase, Phase::Finished);
            assert_eq!(
                fs::read(marker.source.join("foreign")).unwrap(),
                b"New source after second crash"
            );
            assert_eq!(
                fs::read(marker.destination.join("previous")).unwrap(),
                b"Original saved data"
            );
            close(jobs, root);
        }
    });
}
#[test]
fn a_refused_saved_cleanup_phase_keeps_its_independent_binding_unchanged() {
    fixture_at("root-ready", |root, marker| {
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        let before: String = db
            .query_row("SELECT body FROM pending", [], |r| r.get(0))
            .unwrap();
        let binding: String = db
            .query_row("SELECT body FROM pending_bindings", [], |r| r.get(0))
            .unwrap();
        db.execute_batch("CREATE TRIGGER refuse_saved_phase BEFORE UPDATE ON pending BEGIN SELECT RAISE(ABORT,'private saved phase refusal'); END;").unwrap();
        drop(db);
        let jobs = start(root);
        let review = reconcile(&jobs);
        assert_ne!(review.phase, Phase::Finished, "{review:?}");
        close(jobs, root);
        let db = rusqlite::Connection::open(root.join("history")).unwrap();
        assert_eq!(
            db.query_row("SELECT body FROM pending", [], |r| r.get::<_, String>(0))
                .unwrap(),
            before
        );
        assert_eq!(
            db.query_row("SELECT body FROM pending_bindings", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            binding
        );
        assert_eq!(
            fs::read(
                saved_stage::forward::claim_path(marker.migration.as_ref().unwrap())
                    .join("previous")
            )
            .unwrap(),
            b"Original saved data"
        );
        db.execute_batch("DROP TRIGGER refuse_saved_phase;")
            .unwrap();
        drop(db);
        let jobs = start(root);
        let review = reconcile(&jobs);
        assert_eq!(review.phase, Phase::Finished, "{review:?}");
        assert_eq!(job(&jobs, marker.request).phase, Phase::Finished);
        close(jobs, root);
    });
}
#[test]
#[ignore = "private public reconciliation worker killed during an existing saved claim"]
fn saved_reconcile_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_SAVED_CROSS_ROOT").unwrap());
    let jobs = start(&root);
    let _review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    std::thread::sleep(Duration::from_secs(25));
    panic!("private saved reconciliation boundary was not reached");
}
#[test]
#[ignore = "private public worker killed after actual saved-container migration"]
fn saved_cross_child() {
    let root = PathBuf::from(std::env::var_os("CANOPA_SAVED_CROSS_ROOT").unwrap());
    let destination = PathBuf::from(std::env::var_os("CANOPA_SAVED_CROSS_DESTINATION").unwrap());
    fs::create_dir(root.join("seed")).unwrap();
    fs::create_dir(root.join("incoming")).unwrap();
    fs::write(root.join("seed/item"), b"Selected replacement").unwrap();
    let shape = std::env::var("CANOPA_SAVED_CROSS_SHAPE").unwrap_or_else(|_| "file".into());
    if shape == "link" {
        std::os::unix::fs::symlink(link_target(), root.join("incoming/item")).unwrap();
    } else {
        fs::write(root.join("incoming/item"), b"Original saved data").unwrap();
    }
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![root.join("seed/item")],
                destination: root.join("incoming"),
            },
            &Cancellation::default(),
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert_eq!(result.completed, 1);
    if shape == "archive" {
        fs::write(root.join("seed/archive"), b"Copied archive seed").unwrap();
        let copied = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![root.join("seed/archive")],
                    destination: root.join("incoming"),
                },
                &Cancellation::default(),
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert_eq!(copied.completed, 1);
        fs::write(root.join("incoming/archive"), b"Original saved data").unwrap();
        engine
            .undo_interactive(None, &Cancellation::default(), |_| {
                undo::UndoChoice::MoveToRecovery
            })
            .unwrap();
    }
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert_eq!(page.items.len(), if shape == "archive" { 2 } else { 1 });
    let item = page
        .items
        .iter()
        .find(|item| item.held_for_undo != (shape == "archive"))
        .unwrap();
    let source = item
        .entry
        .as_ref()
        .unwrap()
        .path
        .parent()
        .unwrap()
        .to_owned();
    drop(engine);
    let mut sources = vec![source.clone()];
    if shape == "batch" {
        let conflict = destination.join(source.file_name().unwrap());
        fs::create_dir(&conflict).unwrap();
        fs::write(conflict.join("foreign"), b"Existing conflict").unwrap();
        let tail = root.join("seed/z-tail");
        fs::write(&tail, b"Later selected root").unwrap();
        sources.push(tail);
    } else if shape == "nested" {
        fs::create_dir(destination.join("incoming")).unwrap();
        sources = vec![root.join("incoming")];
    }
    let jobs = start(&root);
    let id = receive(
        jobs.submit(
            Request::Transfer(TransferRequest {
                kind: TransferKind::Move,
                sources,
                destination,
            }),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    if shape == "batch" || shape == "nested" {
        let question = wait(|| job(&jobs, id).decision);
        jobs.resolve_transfer(
            id,
            question.generation,
            TransferReply::Resolve {
                choice: if shape == "batch" {
                    ConflictChoice::KeepBoth
                } else {
                    ConflictChoice::Merge
                },
                apply_to_remaining: false,
            },
        )
        .unwrap();
    }
    std::thread::sleep(Duration::from_secs(25));
    panic!("private cross-filesystem saved boundary was not reached");
}
