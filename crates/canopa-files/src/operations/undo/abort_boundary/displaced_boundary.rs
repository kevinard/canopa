//! Public Undo Replace deaths before copying and after actual displacement.
use super::*;
fn stop(pending: &Pending) {
    let root = PathBuf::from(std::env::var_os("CANOPA_UNDO_ABORT_ROOT").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-undo-abort-")
    );
    fs::write(
        root.join("marker.json"),
        serde_json::to_vec(pending).unwrap(),
    )
    .unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("private Undo Replace worker was not killed");
}
pub(in crate::operations) fn registered(pending: &Pending) {
    if std::env::var("CANOPA_UNDO_ABORT_POINT").as_deref() == Ok("registered") {
        stop(pending);
    }
}
pub(in crate::operations) fn empty_copy(pending: &Pending) {
    if std::env::var("CANOPA_UNDO_ABORT_POINT").as_deref() == Ok("empty-copy")
        && matches!(pending, Pending::Undo { stage: Some(stage), .. } if stage.payload_identity.is_none())
    {
        stop(pending);
    }
}
pub(in crate::operations) fn before_rename(source: &Path, destination: &Path) {
    if std::env::var("CANOPA_UNDO_ABORT_POINT").as_deref() != Ok("after-displace") {
        return;
    }
    let root = PathBuf::from(std::env::var_os("CANOPA_UNDO_ABORT_ROOT").unwrap());
    if destination != root.join("source/item") {
        return;
    }
    let db = rusqlite::Connection::open(root.join("history")).unwrap();
    let body: String = db
        .query_row("SELECT body FROM pending", [], |r| r.get(0))
        .unwrap();
    let pending: Pending = serde_json::from_str(&body).unwrap();
    let Pending::Undo {
        stage: Some(stage),
        displaced: Some(record),
        ..
    } = &pending
    else {
        return;
    };
    if source != stage.payload() {
        return;
    }
    assert!(!destination.exists());
    assert_eq!(
        fs::read(record.retained.path()).unwrap(),
        b"Occupied source name"
    );
    assert_eq!(fs::read(source).unwrap(), vec![b'x'; 128 * 1024]);
    stop(&pending);
}
fn displaced(root: &Path) -> recovery::Record {
    let pending: Pending =
        serde_json::from_slice(&fs::read(root.join("marker.json")).unwrap()).unwrap();
    let Pending::Undo {
        displaced: Some(record),
        ..
    } = pending
    else {
        panic!("displaced Undo");
    };
    *record
}
fn reconcile(jobs: &OperationJobs) -> crate::operation_jobs::JobSnapshot {
    let id = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
    terminal(jobs, id)
}
fn retry(jobs: &OperationJobs, root: &Path, output: &Path, replace: bool) {
    let expected = jobs.snapshot().undo.unwrap().id;
    let id = receive(jobs.submit(Request::Undo { expected }, None).unwrap()).unwrap();
    if replace {
        let question = wait(|| {
            jobs.snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id && j.phase == Phase::Waiting)
                .and_then(|j| j.decision)
        });
        jobs.resolve(id, question.generation, UndoChoice::Replace)
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
fn unchanged_undo_replace_reconciles_both_before_and_after_displacement() {
    for point in ["copy", "after-displace"] {
        fixture_decision_at(false, true, point, |root, output, stage| {
            let record = displaced(root);
            for _ in 0..2 {
                let jobs = start(root);
                let done = reconcile(&jobs);
                fs::write(
                    root.join("reconciled-unchanged-replace.txt"),
                    format!("{point}: {done:?}"),
                )
                .unwrap();
                assert_eq!(
                    done.phase,
                    Phase::Finished,
                    "unchanged Undo Replace must reconcile without manual Keep: {point}: {done:?}"
                );
                assert_eq!(
                    fs::read(root.join("source/item")).unwrap(),
                    b"Occupied source name"
                );
                assert_eq!(
                    fs::read(output.join("item")).unwrap(),
                    vec![b'x'; 128 * 1024]
                );
                assert!(!stage.directory.exists());
                assert!(!record.retained.stage.directory.exists());
                close(jobs, root);
            }
            let jobs = start(root);
            retry(&jobs, root, output, true);
            drop(partial(&jobs, b"Occupied source name"));
            close(jobs, root);
        });
    }
}
#[test]
fn a_registered_displaced_preparation_reconciles_empty_work_without_reusing_its_replace_answer() {
    fixture_decision_at(false, true, "registered", |root, output, stage| {
        assert!(!stage.payload().exists());
        let held = stage
            .directory
            .with_file_name("held-original-displaced-root");
        fs::rename(&stage.directory, &held).unwrap();
        fs::create_dir(&stage.directory).unwrap();
        fs::write(
            stage.directory.join("foreign"),
            b"Foreign work root contents",
        )
        .unwrap();
        let jobs = start(root);
        assert_ne!(reconcile(&jobs).phase, Phase::Finished);
        assert_eq!(
            fs::read(stage.directory.join("foreign")).unwrap(),
            b"Foreign work root contents"
        );
        assert_eq!(
            fs::read(root.join("source/item")).unwrap(),
            b"Occupied source name"
        );
        assert_eq!(
            fs::read(output.join("item")).unwrap(),
            vec![b'x'; 128 * 1024]
        );
        close(jobs, root);
        fs::rename(&stage.directory, root.join("preserved-foreign-root")).unwrap();
        fs::rename(&held, &stage.directory).unwrap();
        let jobs = start(root);
        assert_eq!(reconcile(&jobs).phase, Phase::Finished);
        assert!(!stage.directory.exists());
        assert_eq!(
            fs::read(root.join("source/item")).unwrap(),
            b"Occupied source name"
        );
        retry(&jobs, root, output, true);
        drop(partial(&jobs, b"Occupied source name"));
        close(jobs, root);
    });
}
#[test]
fn killed_displaced_returns_keep_both_roots_and_preserve_the_newer_item_after_retry() {
    fixture_decision_at(false, true, "after-displace", |root, output, stage| {
        let record = displaced(root);
        let bytes = b"Edited full return after the original location was displaced";
        fs::write(stage.payload(), bytes).unwrap();
        fs::write(
            record.retained.stage.directory.join("extra"),
            b"Extra displaced-root work",
        )
        .unwrap();
        for _ in 0..2 {
            let jobs = start(root);
            assert_ne!(reconcile(&jobs).phase, Phase::Finished);
            assert!(partial(&jobs, bytes).can_keep_in_recovery());
            assert_eq!(
                fs::read(record.retained.path()).unwrap(),
                b"Occupied source name"
            );
            assert!(!root.join("source/item").exists());
            close(jobs, root);
        }
        let jobs = start(root);
        assert_eq!(keep(&jobs, partial(&jobs, bytes)).phase, Phase::Finished);
        retry(&jobs, root, output, false);
        drop(partial(&jobs, bytes));
        drop(partial(&jobs, b"Occupied source name"));
        let root_item = receive(jobs.recovery_page(None, Cancellation::default()).unwrap())
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
                    item: Box::new(root_item),
                    destination: root.join("rescued-displaced-work"),
                },
                None,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(terminal(&jobs, id).phase, Phase::Finished);
        assert_eq!(
            fs::read(root.join("rescued-displaced-work/previous")).unwrap(),
            b"Occupied source name"
        );
        assert_eq!(
            fs::read(root.join("rescued-displaced-work/extra")).unwrap(),
            b"Extra displaced-root work"
        );
        close(jobs, root);
    });
}
#[test]
fn stale_displaced_reviews_refuse_replaced_previous_entries_and_work_roots() {
    for replace_root in [false, true] {
        fixture_decision_at(false, true, "after-displace", |root, output, stage| {
            let record = displaced(root);
            let bytes = b"Return work to retain with displaced data";
            fs::write(stage.payload(), bytes).unwrap();
            let jobs = start(root);
            let item = partial(&jobs, bytes);
            assert!(item.can_keep_in_recovery());
            let path = if replace_root {
                record.retained.stage.directory.clone()
            } else {
                record.retained.path()
            };
            let held = root.join("held-displaced-original");
            fs::rename(&path, &held).unwrap();
            if replace_root {
                fs::create_dir(&path).unwrap();
                fs::write(path.join("previous"), b"Foreign displaced contents").unwrap();
            } else {
                fs::write(&path, b"Foreign displaced contents").unwrap();
            }
            assert_ne!(keep(&jobs, item).phase, Phase::Finished);
            assert_eq!(
                fs::read(record.retained.path()).unwrap(),
                b"Foreign displaced contents"
            );
            assert_eq!(fs::read(stage.payload()).unwrap(), bytes);
            assert!(!partial(&jobs, bytes).can_keep_in_recovery());
            fs::rename(&path, root.join("preserved-foreign-displaced")).unwrap();
            fs::rename(&held, &path).unwrap();
            assert_eq!(keep(&jobs, partial(&jobs, bytes)).phase, Phase::Finished);
            retry(&jobs, root, output, false);
            drop(partial(&jobs, b"Occupied source name"));
            close(jobs, root);
        });
    }
}
