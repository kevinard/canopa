use canopa_files::{
    Cancellation,
    local_activity::LocalActivity,
    operation_jobs::{
        DropActions, DropTarget, OperationJobs, Phase, PreparedDrag, PreparedDrop, Request,
    },
    storage::Storage,
    transfers::{DropPreference, TransferKind, TransferReply},
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

const BOTH: DropActions = DropActions {
    copy: true,
    move_files: true,
};

fn wait<T>(mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(Instant::now() < deadline, "drop workflow did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn service(root: &Path) -> (OperationJobs, Arc<Storage>, LocalActivity) {
    let storage = Storage::load(root.join("state/volumes")).unwrap();
    let activity = LocalActivity::default();
    let jobs = OperationJobs::start_with_activity(
        root.join("state/history"),
        storage.clone(),
        activity.clone(),
    )
    .unwrap();
    wait(|| (!jobs.snapshot().initializing).then_some(()));
    assert!(jobs.snapshot().error.is_none());
    (jobs, storage, activity)
}

fn drag(jobs: &OperationJobs, paths: Vec<PathBuf>) -> PreparedDrag {
    let mut reply = jobs.prepare_drag(paths, Cancellation::default()).unwrap();
    wait(|| reply.try_recv().unwrap()).unwrap()
}

fn target(jobs: &OperationJobs, drag: &PreparedDrag, path: &Path) -> PreparedDrop {
    let mut reply = jobs
        .prepare_drop(
            drag.clone(),
            DropTarget {
                directory: path.to_owned(),
                binding: None,
            },
            Cancellation::default(),
        )
        .unwrap();
    wait(|| reply.try_recv().unwrap()).unwrap()
}

fn finished(jobs: &OperationJobs, id: u64) -> canopa_files::operation_jobs::JobSnapshot {
    at_phase(jobs, id, Phase::Finished)
}

fn at_phase(
    jobs: &OperationJobs,
    id: u64,
    phase: Phase,
) -> canopa_files::operation_jobs::JobSnapshot {
    wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == phase)
    })
}

fn drop_files(jobs: &OperationJobs, prepared: &PreparedDrop, preference: DropPreference) -> u64 {
    let choice = prepared.choose(preference, BOTH).unwrap();
    let mut reply = jobs.submit_drop(choice).unwrap();
    wait(|| reply.try_recv().unwrap()).unwrap()
}

#[test]
fn negotiated_drop_has_one_admission_and_uses_shared_undo() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("original.txt");
    let destination = root.path().join("destination");
    fs::write(&source, b"Original bytes").unwrap();
    fs::create_dir(&destination).unwrap();
    let (jobs, _, activity) = service(root.path());
    let drag = drag(&jobs, vec![source.clone()]);
    assert!(activity.holds_application() && activity.try_remove().is_err());
    let prepared = target(&jobs, &drag, &destination);
    assert_eq!(
        prepared
            .choose(DropPreference::Automatic, BOTH)
            .unwrap()
            .kind(),
        TransferKind::Move
    );
    let copy_only = DropActions {
        copy: true,
        move_files: false,
    };
    assert_eq!(
        prepared
            .choose(DropPreference::Automatic, copy_only)
            .unwrap()
            .kind(),
        TransferKind::Copy
    );
    assert!(prepared.choose(DropPreference::Move, copy_only).is_none());
    assert!(
        prepared
            .choose(
                DropPreference::Copy,
                DropActions {
                    copy: false,
                    move_files: true
                }
            )
            .is_none()
    );
    let choice = prepared.choose(DropPreference::Copy, BOTH).unwrap();
    let mut accepted = jobs.submit_drop(choice.clone()).unwrap();
    let id = wait(|| accepted.try_recv().unwrap()).unwrap();
    let mut duplicate = jobs.submit_drop(choice).unwrap();
    assert!(wait(|| duplicate.try_recv().unwrap()).is_err());
    assert!(prepared.choose(DropPreference::Move, BOTH).is_none());
    assert!(
        !drag.cancel(),
        "source-side end must not cancel an admitted transfer"
    );
    let report = finished(&jobs, id).transfer.unwrap();
    assert_eq!(report.kind, TransferKind::Copy);
    assert_eq!(report.completed, 1);
    assert_eq!(fs::read(&source).unwrap(), b"Original bytes");
    assert_eq!(
        fs::read(destination.join("original.txt")).unwrap(),
        b"Original bytes"
    );
    let undo = jobs.snapshot().undo.unwrap().id;
    let mut reply = jobs.submit(Request::Undo { expected: undo }, None).unwrap();
    let undo_job = wait(|| reply.try_recv().unwrap()).unwrap();
    finished(&jobs, undo_job);
    assert!(!destination.join("original.txt").exists());
    assert_eq!(fs::read(&source).unwrap(), b"Original bytes");
}

#[test]
fn cancellation_invalidates_every_hover_and_releases_the_gesture_hold() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("selected.txt");
    fs::write(&source, b"Never moved").unwrap();
    let (jobs, _, activity) = service(root.path());
    let drag = drag(&jobs, vec![source.clone()]);
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    let a = target(&jobs, &drag, &first);
    let b = target(&jobs, &drag, &second);
    let old = a.choose(DropPreference::Automatic, BOTH).unwrap();
    assert!(drag.cancel());
    assert!(!drag.cancel());
    assert!(b.choose(DropPreference::Copy, BOTH).is_none());
    wait(|| (!activity.holds_application()).then_some(()));
    let removal = activity.try_remove().unwrap();
    let mut reply = jobs.submit_drop(old).unwrap();
    assert!(wait(|| reply.try_recv().unwrap()).is_err());
    assert!(jobs.snapshot().jobs.is_empty());
    assert_eq!(fs::read(&source).unwrap(), b"Never moved");
    assert_eq!(fs::read_dir(&first).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&second).unwrap().count(), 0);
    drop(removal);
}

#[test]
fn source_capture_survives_hover_and_refuses_a_replacement_before_drop() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("selected.txt");
    let parked = root.path().join("parked.txt");
    let destination = root.path().join("destination");
    fs::write(&source, b"Selected bytes").unwrap();
    fs::create_dir(&destination).unwrap();
    let (jobs, _, _) = service(root.path());
    let drag = drag(&jobs, vec![source.clone()]);
    fs::rename(&source, &parked).unwrap();
    fs::write(&source, b"Unrelated replacement").unwrap();
    let prepared = target(&jobs, &drag, &destination);
    let id = drop_files(&jobs, &prepared, DropPreference::Automatic);
    let report = at_phase(&jobs, id, Phase::Stopped).transfer.unwrap();
    assert_eq!(report.completed, 0);
    assert_eq!(report.failures.len(), 1);
    assert!(report.failures[0].message.contains("changed"));
    assert_eq!(fs::read(&source).unwrap(), b"Unrelated replacement");
    assert_eq!(fs::read(&parked).unwrap(), b"Selected bytes");
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
}

#[test]
fn missing_source_stays_a_failure_after_creation_while_raw_named_peer_copies() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let name = std::ffi::OsString::from_vec(b"original-\xff.txt".to_vec());
    let source = root.path().join(&name);
    let missing = root.path().join("missing.txt");
    let destination = root.path().join("destination");
    fs::write(&source, b"Original raw name").unwrap();
    fs::create_dir(&destination).unwrap();
    let (jobs, _, _) = service(root.path());
    let drag = drag(&jobs, vec![source.clone(), missing.clone()]);
    let prepared = target(&jobs, &drag, &destination);
    assert_eq!(prepared.request().kind, TransferKind::Copy);
    fs::write(&missing, b"Created after capture").unwrap();
    let id = drop_files(&jobs, &prepared, DropPreference::Automatic);
    let report = at_phase(&jobs, id, Phase::Stopped).transfer.unwrap();
    assert_eq!(report.completed, 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].source, missing);
    assert_eq!(
        fs::read(destination.join(name)).unwrap(),
        b"Original raw name"
    );
    assert_eq!(fs::read(&missing).unwrap(), b"Created after capture");
    assert!(!destination.join("missing.txt").exists());
}

fn unavailable(jobs: &OperationJobs, id: u64) {
    let job = at_phase(jobs, id, Phase::Stopped);
    assert!(job.decision.is_none());
    let report = job.transfer.unwrap();
    assert!(report.needs_attention);
    assert_eq!(report.completed, 0);
    assert_eq!(report.remaining.len(), 1);
}

#[test]
fn destination_alias_is_bound_at_hover_and_cannot_retarget_on_release() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let old = root.path().join("old");
    let newer = root.path().join("new");
    let alias = root.path().join("alias");
    fs::write(&source, b"Keep original").unwrap();
    fs::create_dir(&old).unwrap();
    fs::create_dir(&newer).unwrap();
    symlink(&old, &alias).unwrap();
    let (jobs, _, _) = service(root.path());
    let drag = drag(&jobs, vec![source.clone()]);
    let prepared = target(&jobs, &drag, &alias);
    fs::remove_file(&alias).unwrap();
    symlink(&newer, &alias).unwrap();
    let mut stale_browser = jobs
        .prepare_drop(
            drag.clone(),
            DropTarget {
                directory: alias.clone(),
                binding: Some(canopa_files::operation_jobs::Source {
                    directory: alias.clone(),
                    resolved: old.clone(),
                }),
            },
            Cancellation::default(),
        )
        .unwrap();
    assert!(wait(|| stale_browser.try_recv().unwrap()).is_err());
    let id = drop_files(&jobs, &prepared, DropPreference::Move);
    unavailable(&jobs, id);
    assert_eq!(fs::read(&source).unwrap(), b"Keep original");
    assert_eq!(fs::read_dir(&old).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&newer).unwrap().count(), 0);
}

#[test]
fn a_replacement_device_at_the_same_directory_cannot_inherit_hover_authority() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let destination = root.path().join("device");
    fs::write(&source, b"Original").unwrap();
    fs::create_dir(&destination).unwrap();
    let (jobs, storage, _) = service(root.path());
    let observed = storage
        .mounted(destination.clone(), "uuid:first".into())
        .unwrap();
    let epoch = observed.epoch;
    storage.prepare(observed).unwrap();
    let drag = drag(&jobs, vec![source.clone()]);
    let prepared = target(&jobs, &drag, &destination);
    storage.removed(&destination, epoch);
    let observed = storage
        .mounted(destination.clone(), "uuid:replacement".into())
        .unwrap();
    storage.prepare(observed).unwrap();
    let mut refreshed = jobs
        .prepare_drop(
            drag.clone(),
            DropTarget {
                directory: destination.clone(),
                binding: None,
            },
            Cancellation::default(),
        )
        .unwrap();
    assert!(wait(|| refreshed.try_recv().unwrap()).is_err());
    let id = drop_files(&jobs, &prepared, DropPreference::Copy);
    unavailable(&jobs, id);
    assert_eq!(fs::read(&source).unwrap(), b"Original");
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
}

#[test]
fn foreign_runtime_cannot_claim_a_drag_or_consume_the_original_intent() {
    let root = tempfile::tempdir().unwrap();
    let foreign = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let destination = root.path().join("destination");
    fs::write(&source, b"Original").unwrap();
    fs::create_dir(&destination).unwrap();
    let (jobs, storage, _) = service(root.path());
    let (other, _, _) = service(foreign.path());
    let drag = drag(&jobs, vec![source.clone()]);
    let prepared = target(&jobs, &drag, &destination);
    let choice = prepared.choose(DropPreference::Copy, BOTH).unwrap();
    let mut reply = other.submit_drop(choice.clone()).unwrap();
    assert!(
        wait(|| reply.try_recv().unwrap())
            .unwrap_err()
            .contains("another runtime")
    );
    let mut reply = jobs.submit_drop(choice).unwrap();
    let id = wait(|| reply.try_recv().unwrap()).unwrap();
    assert_eq!(finished(&jobs, id).transfer.unwrap().completed, 1);
    assert!(other.snapshot().jobs.is_empty());
    let selection =
        canopa_files::file_clipboard::Selection::new(vec![source], TransferKind::Copy).unwrap();
    let mut reply = jobs
        .prepare_clipboard(selection, storage.snapshot())
        .unwrap();
    let clipboard = wait(|| reply.try_recv().unwrap()).unwrap();
    let mut reply = other.paste_clipboard(clipboard, destination, None).unwrap();
    assert!(
        wait(|| reply.try_recv().unwrap())
            .unwrap_err()
            .contains("another runtime")
    );
}

#[test]
fn cancelled_capture_retires_while_device_removal_still_holds_admission() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    fs::write(&source, b"Original").unwrap();
    let (jobs, _, activity) = service(root.path());
    wait(|| (!activity.holds_application()).then_some(()));
    let removal = activity.try_remove().unwrap();
    let cancellation = Cancellation::default();
    let mut reply = jobs
        .prepare_drag(vec![source.clone()], cancellation.clone())
        .unwrap();
    cancellation.cancel();
    assert!(wait(|| reply.try_recv().unwrap()).is_err());
    assert_eq!(fs::read(&source).unwrap(), b"Original");
    drop(removal);
    wait(|| (!activity.holds_application()).then_some(()));
    assert!(jobs.snapshot().jobs.is_empty());
}

#[test]
fn cancelled_hover_refuses_its_old_choice_without_consuming_another_target() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("selected.txt");
    fs::write(&source, b"Selected bytes").unwrap();
    let (jobs, _, _) = service(root.path());
    let drag = drag(&jobs, vec![source.clone()]);
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    let cancellation = Cancellation::default();
    let mut preparation = jobs
        .prepare_drop(
            drag.clone(),
            DropTarget {
                directory: first.clone(),
                binding: None,
            },
            cancellation.clone(),
        )
        .unwrap();
    let old = wait(|| preparation.try_recv().unwrap()).unwrap();
    let choice = old.choose(DropPreference::Automatic, BOTH).unwrap();
    cancellation.cancel();
    assert!(old.choose(DropPreference::Copy, BOTH).is_none());
    let mut reply = jobs.submit_drop(choice).unwrap();
    assert!(wait(|| reply.try_recv().unwrap()).is_err());
    let next = target(&jobs, &drag, &second);
    let id = drop_files(&jobs, &next, DropPreference::Automatic);
    let report = finished(&jobs, id).transfer.unwrap();
    assert_eq!(report.kind, TransferKind::Move);
    assert_eq!(report.completed, 1);
    assert!(!source.exists());
    assert_eq!(fs::read_dir(&first).unwrap().count(), 0);
    assert_eq!(
        fs::read(second.join("selected.txt")).unwrap(),
        b"Selected bytes"
    );
    let undo = jobs.snapshot().undo.unwrap().id;
    let mut reply = jobs.submit(Request::Undo { expected: undo }, None).unwrap();
    let id = wait(|| reply.try_recv().unwrap()).unwrap();
    finished(&jobs, id);
    assert_eq!(fs::read(&source).unwrap(), b"Selected bytes");
    assert!(!second.join("selected.txt").exists());
}

#[test]
fn real_cross_filesystem_drop_defaults_to_copy_and_explicit_move_is_undoable() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let external = tempfile::tempdir().unwrap();
    assert_ne!(
        root.path().metadata().unwrap().dev(),
        external.path().metadata().unwrap().dev(),
        "this acceptance case requires the repository and temporary directory on distinct real filesystems"
    );
    let source = root.path().join("source.txt");
    fs::write(&source, b"Cross-filesystem bytes").unwrap();
    let (jobs, _, _) = service(root.path());
    let first = drag(&jobs, vec![source.clone()]);
    let prepared = target(&jobs, &first, external.path());
    assert_eq!(prepared.request().kind, TransferKind::Copy);
    let id = drop_files(&jobs, &prepared, DropPreference::Automatic);
    assert_eq!(finished(&jobs, id).transfer.unwrap().completed, 1);
    assert_eq!(fs::read(&source).unwrap(), b"Cross-filesystem bytes");
    let destination = external.path().join("move");
    fs::create_dir(&destination).unwrap();
    let second = drag(&jobs, vec![source.clone()]);
    let prepared = target(&jobs, &second, &destination);
    let id = drop_files(&jobs, &prepared, DropPreference::Move);
    let report = finished(&jobs, id).transfer.unwrap();
    assert_eq!(report.kind, TransferKind::Move);
    assert_eq!(report.completed, 1);
    assert!(!source.exists());
    assert_eq!(
        fs::read(destination.join("source.txt")).unwrap(),
        b"Cross-filesystem bytes"
    );
    let undo = jobs.snapshot().undo.unwrap().id;
    let mut reply = jobs.submit(Request::Undo { expected: undo }, None).unwrap();
    let id = wait(|| reply.try_recv().unwrap()).unwrap();
    finished(&jobs, id);
    assert_eq!(fs::read(&source).unwrap(), b"Cross-filesystem bytes");
    assert!(!destination.join("source.txt").exists());
    assert_eq!(
        fs::read(external.path().join("source.txt")).unwrap(),
        b"Cross-filesystem bytes"
    );
}

#[test]
fn drop_conflict_waits_for_explicit_keep_both_and_undo_preserves_existing_bytes() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("selected.txt");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"Incoming bytes").unwrap();
    fs::write(destination.join("selected.txt"), b"Existing bytes").unwrap();
    let (jobs, _, _) = service(root.path());
    let drag = drag(&jobs, vec![source.clone()]);
    let prepared = target(&jobs, &drag, &destination);
    let id = drop_files(&jobs, &prepared, DropPreference::Automatic);
    let decision = wait(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id)
            .and_then(|job| job.decision)
    });
    assert_eq!(fs::read(&source).unwrap(), b"Incoming bytes");
    assert_eq!(
        fs::read(destination.join("selected.txt")).unwrap(),
        b"Existing bytes"
    );
    jobs.resolve_transfer(
        id,
        decision.generation,
        TransferReply::Resolve {
            choice: canopa_files::transfers::ConflictChoice::KeepBoth,
            apply_to_remaining: false,
        },
    )
    .unwrap();
    let report = finished(&jobs, id).transfer.unwrap();
    assert_eq!(report.completed, 1);
    let incoming = &report.destinations[0];
    assert_ne!(incoming, &destination.join("selected.txt"));
    assert_eq!(fs::read(incoming).unwrap(), b"Incoming bytes");
    assert!(!source.exists());
    let undo = jobs.snapshot().undo.unwrap().id;
    let mut reply = jobs.submit(Request::Undo { expected: undo }, None).unwrap();
    let id = wait(|| reply.try_recv().unwrap()).unwrap();
    finished(&jobs, id);
    assert_eq!(fs::read(&source).unwrap(), b"Incoming bytes");
    assert_eq!(
        fs::read(destination.join("selected.txt")).unwrap(),
        b"Existing bytes"
    );
    assert!(!incoming.exists());
}

#[test]
fn replacing_the_local_destination_directory_after_hover_preserves_both_folders() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("selected.txt");
    let destination = root.path().join("destination");
    let original = root.path().join("original-destination");
    fs::write(&source, b"Original selection").unwrap();
    fs::create_dir(&destination).unwrap();
    let (jobs, _, _) = service(root.path());
    let drag = drag(&jobs, vec![source.clone()]);
    let prepared = target(&jobs, &drag, &destination);
    fs::rename(&destination, &original).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("newer.txt"), b"New destination bytes").unwrap();
    let id = drop_files(&jobs, &prepared, DropPreference::Move);
    let job = wait(|| {
        jobs.snapshot().jobs.into_iter().find(|job| {
            job.id == id
                && matches!(
                    job.phase,
                    Phase::Stopped | Phase::Finished | Phase::Failed | Phase::Interrupted
                )
        })
    });
    assert_eq!(job.phase, Phase::Stopped, "{job:?}");
    let report = job.transfer.unwrap();
    assert!(report.needs_attention);
    assert_eq!(report.completed, 0);
    assert_eq!(fs::read(&source).unwrap(), b"Original selection");
    assert_eq!(fs::read_dir(&original).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
    assert_eq!(
        fs::read(destination.join("newer.txt")).unwrap(),
        b"New destination bytes"
    );
}

#[test]
fn repreparing_a_replaced_destination_keeps_its_original_observation() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("selected.txt");
    let destination = root.path().join("destination");
    let original = root.path().join("original-destination");
    fs::write(&source, b"Original selection").unwrap();
    fs::create_dir(&destination).unwrap();
    let (jobs, _, _) = service(root.path());
    let gesture = drag(&jobs, vec![source.clone()]);
    let cancellation = Cancellation::default();
    let mut reply = jobs
        .prepare_drop(
            gesture.clone(),
            DropTarget {
                directory: destination.clone(),
                binding: None,
            },
            cancellation.clone(),
        )
        .unwrap();
    let old = wait(|| reply.try_recv().unwrap()).unwrap();
    fs::rename(&destination, &original).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("newer.txt"), b"New destination bytes").unwrap();
    cancellation.cancel();
    assert!(old.choose(DropPreference::Move, BOTH).is_none());
    let mut reply = jobs
        .prepare_drop(
            gesture,
            DropTarget {
                directory: destination.clone(),
                binding: None,
            },
            Cancellation::default(),
        )
        .unwrap();
    assert!(wait(|| reply.try_recv().unwrap()).is_err());
    assert_eq!(fs::read(&source).unwrap(), b"Original selection");
    assert_eq!(fs::read_dir(&original).unwrap().count(), 0);
    assert!(!destination.join("selected.txt").exists());

    let fresh = drag(&jobs, vec![source.clone()]);
    let reviewed = target(&jobs, &fresh, &destination);
    let id = drop_files(&jobs, &reviewed, DropPreference::Move);
    assert_eq!(finished(&jobs, id).transfer.unwrap().completed, 1);
    let undo = jobs.snapshot().undo.unwrap().id;
    let mut reply = jobs.submit(Request::Undo { expected: undo }, None).unwrap();
    finished(&jobs, wait(|| reply.try_recv().unwrap()).unwrap());
    assert_eq!(fs::read(&source).unwrap(), b"Original selection");
    assert!(!destination.join("selected.txt").exists());
    assert_eq!(
        fs::read(destination.join("newer.txt")).unwrap(),
        b"New destination bytes"
    );
}

#[test]
fn repreparing_after_content_changes_still_moves_and_undoes() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("selected.txt");
    let destination = root.path().join("destination");
    fs::write(&source, b"Original selection").unwrap();
    fs::create_dir(&destination).unwrap();
    let (jobs, _, _) = service(root.path());
    let gesture = drag(&jobs, vec![source.clone()]);
    let _first = target(&jobs, &gesture, &destination);
    fs::write(destination.join("arrival.txt"), b"Unrelated arrival").unwrap();
    let refreshed = target(&jobs, &gesture, &destination);
    let id = drop_files(&jobs, &refreshed, DropPreference::Move);
    assert_eq!(finished(&jobs, id).transfer.unwrap().completed, 1);
    assert!(!source.exists());
    assert_eq!(
        fs::read(destination.join("selected.txt")).unwrap(),
        b"Original selection"
    );
    let undo = jobs.snapshot().undo.unwrap().id;
    let mut reply = jobs.submit(Request::Undo { expected: undo }, None).unwrap();
    finished(&jobs, wait(|| reply.try_recv().unwrap()).unwrap());
    assert_eq!(fs::read(&source).unwrap(), b"Original selection");
    assert_eq!(
        fs::read(destination.join("arrival.txt")).unwrap(),
        b"Unrelated arrival"
    );
}

#[test]
fn a_drop_into_the_items_own_folder_or_onto_itself_is_refused_before_release() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    let inner = folder.join("inner");
    fs::create_dir_all(&inner).unwrap();
    let file = folder.join("note.txt");
    fs::write(&file, b"Note").unwrap();
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(&folder, &alias).unwrap();
    let (jobs, _, _) = service(root.path());
    let gesture = drag(&jobs, vec![file.clone(), inner.clone()]);
    let refused = |directory: &Path| {
        let mut reply = jobs
            .prepare_drop(
                gesture.clone(),
                DropTarget {
                    directory: directory.to_owned(),
                    binding: None,
                },
                Cancellation::default(),
            )
            .unwrap();
        wait(|| reply.try_recv().unwrap()).is_err()
    };
    // Its own folder, the same folder through an alias, and a dragged folder itself.
    assert!(refused(&folder));
    assert!(refused(&alias));
    assert!(refused(&inner));
    // Elsewhere stays an ordinary destination.
    let elsewhere = root.path().join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    assert!(!refused(&elsewhere));
    assert_eq!(fs::read(&file).unwrap(), b"Note");
}
