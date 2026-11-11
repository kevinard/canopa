//! Permanent deletion and Empty Trash through the public engine and queue.
use canopa_files::{
    Cancellation,
    operation_jobs::{JobSnapshot, OperationJobs, Phase, Request},
    operations::{OperationEngine, OperationRequest, UndoChoice},
    trash::{
        TrashLocation,
        catalog::{Catalog, Scope},
    },
};
use std::{
    fs,
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{PermissionsExt, symlink},
    },
    path::Path,
    time::{Duration, Instant},
};

fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < deadline, "the worker did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn start(root: &Path) -> OperationJobs {
    let queue =
        OperationJobs::start(root.join("history.sqlite3"), root.join("volumes.json")).unwrap();
    wait_for(|| {
        let state = queue.snapshot();
        assert!(state.error.is_none(), "{:?}", state.error);
        (!state.initializing).then_some(())
    });
    queue
}

fn submit(queue: &OperationJobs, request: Request) -> u64 {
    let mut accepted = queue.submit(request, None).unwrap();
    wait_for(|| accepted.try_recv().unwrap()).unwrap()
}

fn settled(queue: &OperationJobs, id: u64) -> JobSnapshot {
    wait_for(|| {
        queue.snapshot().jobs.into_iter().find(|job| {
            job.id == id
                && matches!(
                    job.phase,
                    Phase::Finished | Phase::Stopped | Phase::Failed | Phase::Interrupted
                )
        })
    })
}

/// Hold the serial queue behind an Undo question so later work stays queued.
fn block_on_review(queue: &OperationJobs, root: &Path) -> (u64, u64) {
    let folder = root.join("review");
    let created = submit(
        queue,
        Request::Mutation(OperationRequest::CreateFolder {
            path: folder.clone(),
        }),
    );
    assert_eq!(settled(queue, created).phase, Phase::Finished);
    fs::write(folder.join("later.txt"), b"later work").unwrap();
    let undo = submit(
        queue,
        Request::Undo {
            expected: queue.snapshot().undo.unwrap().id,
        },
    );
    let generation = wait_for(|| {
        queue
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == undo)
            .and_then(|job| job.decision)
            .map(|decision| decision.generation)
    });
    (undo, generation)
}

fn privileged() -> bool {
    rustix::process::geteuid().is_root()
}

#[test]
fn permanent_deletion_removes_trees_and_links_without_following_them() {
    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("outside.txt");
    fs::write(&outside, b"must survive").unwrap();
    let folder = root.path().join("project");
    fs::create_dir_all(folder.join("nested/deeper")).unwrap();
    let raw = std::ffi::OsString::from_vec(b"raw-\xff\n.txt".to_vec());
    fs::write(folder.join(&raw), b"bytes").unwrap();
    fs::write(folder.join("nested/deeper/leaf"), b"leaf").unwrap();
    symlink(&outside, folder.join("nested/link")).unwrap();
    symlink(root.path(), folder.join("folder-link")).unwrap();
    let link = root.path().join("top-link");
    symlink(&outside, &link).unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let outcome = engine
        .delete_permanently(&[folder.clone(), link.clone()], &Cancellation::default())
        .unwrap();
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(!outcome.cancelled);
    assert_eq!(outcome.completed.len(), 2);
    assert!(fs::symlink_metadata(&folder).is_err());
    assert!(fs::symlink_metadata(&link).is_err());
    assert_eq!(fs::read(&outside).unwrap(), b"must survive");
    // project, nested, deeper, leaf, raw file, two links, and the top link.
    assert_eq!(outcome.entries, 8);
    // Deletion is not undoable and leaves no history entry.
    assert!(engine.undo(&Cancellation::default()).is_err());
}

#[test]
fn cancellation_stops_before_unstarted_items() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first");
    fs::write(&first, b"x").unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        engine
            .delete_permanently(std::slice::from_ref(&first), &cancellation)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    assert!(first.exists());
}

#[test]
fn queued_deletion_runs_with_a_result_and_without_undo() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let queue = start(root);
    let folder = root.join("old builds");
    fs::create_dir(&folder).unwrap();
    for index in 0..20 {
        fs::write(folder.join(format!("artifact-{index}")), vec![0; 1024]).unwrap();
    }
    let undo_before = queue.snapshot().undo.map(|head| head.id);
    let id = submit(
        &queue,
        Request::Delete {
            sources: vec![folder.clone()],
        },
    );
    let job = settled(&queue, id);
    assert_eq!(job.phase, Phase::Finished, "{:?}", job.error);
    assert_eq!(job.title, "Delete 1 item permanently");
    let report = job.delete.unwrap();
    assert!(!report.from_trash);
    assert_eq!(report.completed, vec![folder.clone()]);
    assert_eq!(report.entries, 21);
    assert_eq!(report.bytes, 20 * 1024);
    assert!(!folder.exists());
    assert_eq!(queue.snapshot().undo.map(|head| head.id), undo_before);
    // The result survives reopening and never replays.
    let mut shutdown = queue.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    drop(queue);
    let reopened = start(root);
    let row = settled(&reopened, id);
    assert_eq!(row.phase, Phase::Finished);
    assert_eq!(row.delete.unwrap().completed, vec![folder]);
}

#[test]
fn cancelled_or_replaced_selections_are_preserved() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let queue = start(root);
    let (undo, generation) = block_on_review(&queue, root);
    let kept = root.join("kept.txt");
    let replaced = root.join("replaced.txt");
    let good = root.join("good.txt");
    for path in [&kept, &replaced, &good] {
        fs::write(path, b"original").unwrap();
    }
    let cancelled = submit(
        &queue,
        Request::Delete {
            sources: vec![kept.clone()],
        },
    );
    let id = submit(
        &queue,
        Request::Delete {
            sources: vec![replaced.clone(), good.clone()],
        },
    );
    queue.cancel(cancelled).unwrap();
    let job = settled(&queue, cancelled);
    assert_eq!(job.phase, Phase::Stopped);
    let report = job.delete.unwrap();
    assert!(report.cancelled);
    assert_eq!(report.remaining, vec![kept.clone()]);
    fs::rename(&replaced, root.join("moved away.txt")).unwrap();
    fs::write(&replaced, b"a different file").unwrap();
    queue
        .resolve(undo, generation, UndoChoice::KeepItem)
        .unwrap();
    let job = settled(&queue, id);
    assert_eq!(job.phase, Phase::Stopped, "{:?}", job.error);
    let report = job.delete.unwrap();
    assert_eq!(report.completed, vec![good.clone()]);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].source, replaced);
    assert_eq!(fs::read(&replaced).unwrap(), b"a different file");
    assert_eq!(fs::read(&kept).unwrap(), b"original");
    assert!(!good.exists());
}

#[test]
fn deleting_from_trash_removes_payload_information_and_size_cache_entry() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    let folder = root.path().join("photos");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("a.jpg"), b"jpeg").unwrap();
    let file = root.path().join("note.txt");
    fs::write(&file, b"note").unwrap();
    let location = TrashLocation::home(&data).unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    engine
        .trash(
            &location,
            &[folder.clone(), file.clone()],
            &Cancellation::default(),
        )
        .unwrap();
    let items = location.list(&Cancellation::default()).unwrap();
    assert_eq!(items.len(), 2);
    let stored = items
        .iter()
        .find(|item| item.original.as_deref() == Some(folder.as_path()))
        .unwrap()
        .clone();
    let stored_name = stored.path.file_name().unwrap().as_bytes().to_vec();
    let cache = location.directory().join("directorysizes");
    fs::write(
        &cache,
        [
            b"4 1700000000 ".as_slice(),
            &stored_name,
            b"\n9 1700000000 unrelated%20folder\n",
        ]
        .concat(),
    )
    .unwrap();
    let outcome = engine
        .delete_from_trash(std::slice::from_ref(&stored), &Cancellation::default())
        .unwrap();
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert_eq!(outcome.completed, vec![stored.path.clone()]);
    assert!(fs::symlink_metadata(&stored.path).is_err());
    let remaining = location.list(&Cancellation::default()).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].original.as_deref(), Some(file.as_path()));
    assert_eq!(
        fs::read_dir(location.directory().join("info"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        fs::read(&cache).unwrap(),
        b"9 1700000000 unrelated%20folder\n"
    );
    // Removal from Trash adds nothing to Undo; the older Trash entry no
    // longer has its stored folder and is refused rather than invented.
    assert!(!folder.exists());
}

#[test]
fn per_volume_trash_and_orphaned_payloads_are_emptied() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let volume = root.path().join("volume");
    fs::create_dir_all(volume.join("folder")).unwrap();
    let source = volume.join("folder/report.pdf");
    fs::write(&source, b"pdf").unwrap();
    let location = TrashLocation::volume(&volume).unwrap();
    let uid = fs::metadata(root.path()).unwrap().uid();
    assert_eq!(location.directory(), volume.join(format!(".Trash-{uid}")));
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    engine
        .trash(
            &location,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    // Another program left a payload with unreadable information.
    let orphan = location.directory().join("files/orphan");
    fs::write(&orphan, b"orphan").unwrap();
    fs::write(
        location.directory().join("info/orphan.trashinfo"),
        b"not a trash info file",
    )
    .unwrap();
    let items = location.list(&Cancellation::default()).unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|item| item.problem.is_some()));
    let outcome = engine
        .delete_from_trash(&items, &Cancellation::default())
        .unwrap();
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert_eq!(outcome.completed.len(), 2);
    assert!(location.list(&Cancellation::default()).unwrap().is_empty());
    for store in ["files", "info"] {
        assert_eq!(
            fs::read_dir(location.directory().join(store))
                .unwrap()
                .count(),
            0,
            "{store} is empty"
        );
    }
}

#[test]
fn a_replaced_trash_payload_is_not_deleted() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("draft.txt");
    fs::write(&source, b"draft").unwrap();
    let location = TrashLocation::home(root.path().join("data")).unwrap();
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    engine
        .trash(
            &location,
            std::slice::from_ref(&source),
            &Cancellation::default(),
        )
        .unwrap();
    let item = location.list(&Cancellation::default()).unwrap().remove(0);
    fs::remove_file(&item.path).unwrap();
    fs::write(&item.path, b"another program's file").unwrap();
    let outcome = engine
        .delete_from_trash(std::slice::from_ref(&item), &Cancellation::default())
        .unwrap();
    assert!(outcome.completed.is_empty());
    assert_eq!(outcome.failures.len(), 1);
    assert_eq!(fs::read(&item.path).unwrap(), b"another program's file");
}

#[test]
fn empty_trash_is_a_queued_request_over_the_listed_items() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let data = root.join("data");
    let queue = start(root);
    let sources: Vec<_> = ["one.txt", "two.txt"]
        .into_iter()
        .map(|name| root.join(name))
        .collect();
    for source in &sources {
        fs::write(source, b"trash me").unwrap();
    }
    let trashed = submit(
        &queue,
        Request::Trash {
            data_home: data.clone(),
            sources: sources.clone(),
        },
    );
    assert_eq!(settled(&queue, trashed).phase, Phase::Finished);
    let catalog = Catalog::read(&data, Scope::Main, &Cancellation::default()).unwrap();
    assert_eq!(catalog.items.len(), 2);
    // An item trashed after the confirmation is not part of the request.
    let late = root.join("late.txt");
    fs::write(&late, b"late").unwrap();
    let late_trash = submit(
        &queue,
        Request::Trash {
            data_home: data.clone(),
            sources: vec![late.clone()],
        },
    );
    assert_eq!(settled(&queue, late_trash).phase, Phase::Finished);
    let id = submit(
        &queue,
        Request::DeleteTrash {
            items: catalog.items.clone(),
            empty: true,
        },
    );
    let job = settled(&queue, id);
    assert_eq!(job.phase, Phase::Finished, "{:?}", job.error);
    assert_eq!(job.title, "Empty Trash (2 items)");
    let report = job.delete.unwrap();
    assert!(report.from_trash);
    assert_eq!(report.completed.len(), 2);
    let remaining = Catalog::read(&data, Scope::Main, &Cancellation::default()).unwrap();
    assert_eq!(remaining.items.len(), 1);
    assert_eq!(remaining.items[0].original.as_deref(), Some(late.as_path()));
}

#[test]
fn a_volume_without_usable_trash_reports_its_sources_for_confirmed_deletion() {
    if privileged() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let data = root.join("read-only data");
    fs::create_dir(&data).unwrap();
    let source = root.join("cannot trash.txt");
    fs::write(&source, b"still here").unwrap();
    fs::set_permissions(&data, fs::Permissions::from_mode(0o555)).unwrap();
    let queue = start(root);
    let id = submit(
        &queue,
        Request::Trash {
            data_home: data.clone(),
            sources: vec![source.clone()],
        },
    );
    let job = settled(&queue, id);
    let report = job.trash.unwrap();
    assert!(report.completed.is_empty());
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.unavailable, vec![source.clone()]);
    assert_eq!(fs::read(&source).unwrap(), b"still here");
    let deleted = submit(
        &queue,
        Request::Delete {
            sources: report.unavailable.clone(),
        },
    );
    assert_eq!(settled(&queue, deleted).phase, Phase::Finished);
    assert!(!source.exists());
    fs::set_permissions(&data, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_read_only_volume_root_cannot_hold_trash_but_ordinary_failures_are_not_offered() {
    use std::os::unix::fs::MetadataExt;
    if privileged() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let volume = root.path().join("volume");
    fs::create_dir_all(volume.join("mine")).unwrap();
    let location = TrashLocation::volume(&volume).unwrap();
    let uid = fs::metadata(root.path()).unwrap().uid();
    fs::remove_dir_all(volume.join(format!(".Trash-{uid}"))).unwrap();
    fs::set_permissions(&volume, fs::Permissions::from_mode(0o555)).unwrap();
    let source = volume.join("mine/file");
    fs::write(&source, b"x").unwrap();
    let missing = volume.join("mine/missing");
    let engine = OperationEngine::open(root.path().join("state/operations.sqlite3")).unwrap();
    let outcome = engine
        .trash(
            &location,
            &[source.clone(), missing.clone()],
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(outcome.failures.len(), 2);
    // Storage is unusable for both; a vanished source still is not "deletable".
    assert_eq!(outcome.unavailable, vec![source.clone()]);
    assert!(!outcome.unavailable.contains(&missing));
    assert!(source.exists());
    fs::set_permissions(&volume, fs::Permissions::from_mode(0o755)).unwrap();
}
