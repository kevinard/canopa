use canopa_files::{
    operation_jobs::{Decision, JobSnapshot, OperationJobs, Phase, Request, Source},
    operations::{OperationRequest, UndoChoice},
    storage::Storage,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "operation worker did not reach the expected state"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_phase(service: &OperationJobs, id: u64, phase: Phase) -> JobSnapshot {
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == phase)
    })
}

struct FolderReview {
    folder: PathBuf,
    job: u64,
    decision: Decision,
}

fn folder_review(service: &OperationJobs, root: &Path) -> FolderReview {
    let folder = root.join("nonempty");
    let mut admitted = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: folder.clone(),
            }),
            None,
        )
        .unwrap();
    let created = wait_for(|| admitted.try_recv().unwrap()).unwrap();
    wait_for_phase(service, created, Phase::Finished);
    fs::write(folder.join("later.txt"), b"Later work").unwrap();
    let mut admitted = service
        .submit(
            Request::Undo {
                expected: service.snapshot().undo.unwrap().id,
            },
            None,
        )
        .unwrap();
    let job = wait_for(|| admitted.try_recv().unwrap()).unwrap();
    let decision = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|entry| entry.id == job)
            .and_then(|entry| entry.decision)
    });
    FolderReview {
        folder,
        job,
        decision,
    }
}

#[test]
fn an_undo_question_keeps_its_files_safe_and_does_not_block_durable_admission() {
    let temporary = tempfile::tempdir().unwrap();
    let service = OperationJobs::start(
        temporary.path().join("state/history.sqlite3"),
        temporary.path().join("state/volumes.json"),
    )
    .unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let folder = temporary.path().join("created");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: folder.clone(),
            }),
            None,
        )
        .unwrap();
    let first = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, first, Phase::Finished);
    fs::write(folder.join("new-work.txt"), b"Keep this later work").unwrap();
    let expected = service.snapshot().undo.unwrap().id;
    let mut admission = service.submit(Request::Undo { expected }, None).unwrap();
    let undo = wait_for(|| admission.try_recv().unwrap()).unwrap();
    let decision = wait_for(|| {
        service
            .snapshot()
            .jobs
            .iter()
            .find(|job| job.id == undo)
            .and_then(|job| job.decision.clone())
    });
    let next = temporary.path().join("accepted while waiting");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder { path: next.clone() }),
            None,
        )
        .unwrap();
    let later = wait_for(|| admission.try_recv().unwrap()).unwrap();
    assert!(!next.exists(), "mutations stay serial while Undo waits");
    assert_eq!(
        fs::read(folder.join("new-work.txt")).unwrap(),
        b"Keep this later work"
    );
    service
        .resolve(undo, decision.generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for_phase(&service, later, Phase::Finished);
    assert!(next.is_dir());
    assert_eq!(
        fs::read(folder.join("new-work.txt")).unwrap(),
        b"Keep this later work"
    );
    assert!(
        service
            .snapshot()
            .undo
            .unwrap()
            .label
            .contains("accepted while waiting")
    );
}

#[test]
fn an_admitted_folder_creation_changes_the_actual_directory_and_publishes_shared_undo() {
    let temporary = tempfile::tempdir().unwrap();
    let folder = temporary.path().join("files");
    fs::create_dir(&folder).unwrap();
    let service = OperationJobs::start(
        temporary.path().join("state/operations.sqlite3"),
        temporary.path().join("state/volumes.json"),
    )
    .unwrap();
    wait_for(|| {
        let state = service.snapshot();
        assert!(state.error.is_none(), "{:?}", state.error);
        (!state.initializing).then_some(())
    });
    let target = folder.join("A new folder");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: target.clone(),
            }),
            Some(Source {
                directory: folder.clone(),
                resolved: fs::canonicalize(&folder).unwrap(),
            }),
        )
        .unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for(|| {
        let state = service.snapshot();
        state
            .jobs
            .iter()
            .find(|job| job.id == id && job.phase == Phase::Finished)
            .map(|_| ())
    });
    assert!(target.is_dir());
    let undo = service
        .snapshot()
        .undo
        .expect("completed creation must be undoable");
    assert!(undo.label.contains("A new folder"));
}

#[test]
fn queue_changes_wake_the_renderer_without_polling_or_accumulating_snapshots() {
    let temporary = tempfile::tempdir().unwrap();
    let service = OperationJobs::start(
        temporary.path().join("state/history.sqlite3"),
        temporary.path().join("state/volumes.json"),
    )
    .unwrap();
    let mut changes = service
        .take_notices()
        .expect("one renderer owns the stream");
    assert!(service.take_notices().is_none());
    wait_for(|| changes.try_recv().ok());
    assert!(!service.snapshot().initializing);
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: temporary.path().join("new"),
            }),
            None,
        )
        .unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for(|| changes.try_recv().ok());
    wait_for_phase(&service, id, Phase::Finished);
    assert!(service.snapshot().undo.is_some());
    let mut pending = 0;
    while changes.try_recv().is_ok() {
        pending += 1;
    }
    assert!(
        pending <= 1,
        "a paused renderer gets one wake-up for the latest snapshot"
    );
}

#[test]
fn cancelling_a_queued_operation_leaves_its_target_absent_and_running_undo_can_finish() {
    let temporary = tempfile::tempdir().unwrap();
    let service = OperationJobs::start(
        temporary.path().join("state/history.sqlite3"),
        temporary.path().join("state/volumes.json"),
    )
    .unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let FolderReview {
        folder, job: undo, ..
    } = folder_review(&service, temporary.path());
    let target = temporary.path().join("cancelled before starting");
    let mut queued = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: target.clone(),
            }),
            None,
        )
        .unwrap();
    let queued = wait_for(|| queued.try_recv().unwrap()).unwrap();
    service.cancel(queued).unwrap();
    wait_for_phase(&service, queued, Phase::Finished);
    service.cancel(undo).unwrap();
    wait_for_phase(&service, undo, Phase::Finished);
    assert!(!target.exists());
    assert_eq!(fs::read(folder.join("later.txt")).unwrap(), b"Later work");
}

#[test]
fn retrying_a_start_checkpoint_preserves_the_request_until_it_can_execute() {
    let temporary = tempfile::tempdir().unwrap();
    let history = temporary.path().join("state/history.sqlite3");
    let service =
        OperationJobs::start(history.clone(), temporary.path().join("state/volumes.json")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let FolderReview {
        job: undo,
        decision,
        ..
    } = folder_review(&service, temporary.path());
    let target = temporary.path().join("starts after saving");
    let mut admitted = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: target.clone(),
            }),
            None,
        )
        .unwrap();
    let queued = wait_for(|| admitted.try_recv().unwrap()).unwrap();
    // An actual database write failure only at this request's Running
    // checkpoint. Other work can finish saving normally.
    let database = rusqlite::Connection::open(&history).unwrap();
    database.execute_batch(&format!("CREATE TRIGGER refuse_start BEFORE UPDATE ON requests WHEN NEW.id={queued} AND NEW.phase=1 BEGIN SELECT RAISE(FAIL, 'fixture cannot save start'); END;")).unwrap();
    service
        .resolve(undo, decision.generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for_phase(&service, queued, Phase::StartSaveFailed);
    let mut dismissal = service.dismiss_request(queued).unwrap();
    assert!(wait_for(|| dismissal.try_recv().unwrap()).is_err());
    assert!(!target.exists());
    assert!(service.snapshot().holds_application());
    database
        .execute_batch("DROP TRIGGER refuse_start;")
        .unwrap();
    service.retry_save(queued).unwrap();
    wait_for_phase(&service, queued, Phase::Finished);
    assert!(target.is_dir());
}

#[test]
fn retrying_a_result_save_never_replays_the_original_request() {
    let temporary = tempfile::tempdir().unwrap();
    let history = temporary.path().join("state/history.sqlite3");
    let service =
        OperationJobs::start(history.clone(), temporary.path().join("state/volumes.json")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let FolderReview {
        folder, job: undo, ..
    } = folder_review(&service, temporary.path());
    let target = temporary.path().join("must never run");
    let mut queued = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: target.clone(),
            }),
            None,
        )
        .unwrap();
    let queued = wait_for(|| queued.try_recv().unwrap()).unwrap();
    let moved = history.with_extension("temporarily-moved");
    fs::rename(&history, &moved).unwrap();
    service.cancel(queued).unwrap();
    wait_for_phase(&service, queued, Phase::SaveFailed);
    let mut dismissal = service.dismiss_request(queued).unwrap();
    assert!(wait_for(|| dismissal.try_recv().unwrap()).is_err());
    assert!(service.snapshot().holds_application());
    let mut refused = service.shutdown().unwrap();
    assert!(wait_for(|| refused.try_recv().unwrap()).is_err());
    fs::rename(&moved, &history).unwrap();
    service.retry_save(queued).unwrap();
    wait_for_phase(&service, queued, Phase::Finished);
    service.cancel(undo).unwrap();
    wait_for_phase(&service, undo, Phase::Finished);
    assert!(!target.exists());
    assert_eq!(fs::read(folder.join("later.txt")).unwrap(), b"Later work");
}

#[test]
fn reopening_preserves_raw_rename_results_and_undo_without_running_them_again() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let temporary = tempfile::tempdir().unwrap();
    let history = temporary.path().join("state/history.sqlite3");
    let volumes = temporary.path().join("state/volumes.json");
    let source = temporary
        .path()
        .join(OsString::from_vec(b"old-\xff.txt".to_vec()));
    let destination = temporary
        .path()
        .join(OsString::from_vec(b"renamed-\xfe.txt".to_vec()));
    fs::write(&source, b"Original contents").unwrap();
    let service = OperationJobs::start(history.clone(), volumes.clone()).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let mut accepted = service
        .submit(
            Request::Mutation(OperationRequest::Rename {
                source: source.clone(),
                destination: destination.clone(),
            }),
            None,
        )
        .unwrap();
    let id = wait_for(|| accepted.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, id, Phase::Finished);
    let expected_undo = service.snapshot().undo.unwrap().id;
    let mut stopped = service.shutdown().unwrap();
    wait_for(|| stopped.try_recv().unwrap()).unwrap();
    fs::write(&destination, b"Newer user changes").unwrap();
    let reopened = OperationJobs::start(history, volumes).unwrap();
    wait_for(|| {
        let state = reopened.snapshot();
        assert!(state.error.is_none(), "{:?}", state.error);
        (!state.initializing).then_some(())
    });
    assert_eq!(reopened.snapshot().undo.unwrap().id, expected_undo);
    let state = reopened.snapshot();
    let completed = state.jobs.iter().find(|job| job.id == id).unwrap();
    assert_eq!(completed.phase, Phase::Finished);
    assert!(completed.paths.contains(&destination));
    assert!(!state.holds_application());
    assert_eq!(fs::read(&destination).unwrap(), b"Newer user changes");
    assert!(!source.exists());
}

#[test]
fn a_killed_worker_reopens_queued_and_running_requests_for_review_without_replaying_them() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let temporary = tempfile::Builder::new()
        .prefix("canopa-operation-worker-crash-")
        .tempdir()
        .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "operation_worker_crash_helper"])
        .env("CANOPA_OPERATION_WORKER_CRASH_ROOT", temporary.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    let service = OperationJobs::start(
        temporary.path().join("state/history.sqlite3"),
        temporary.path().join("state/volumes.json"),
    )
    .unwrap();
    wait_for(|| {
        let state = service.snapshot();
        assert!(state.error.is_none(), "{:?}", state.error);
        (!state.initializing).then_some(())
    });
    let state = service.snapshot();
    assert!(state.jobs.iter().any(|job| job.title == "Undo"
        && job.phase == Phase::Interrupted
        && job.decision.is_none()));
    assert!(
        state
            .jobs
            .iter()
            .any(|job| job.title.contains("must not start") && job.phase == Phase::AwaitingReview)
    );
    assert!(state.undo.is_some());
    assert!(!temporary.path().join("must not start").exists());
    let queued = state
        .jobs
        .iter()
        .find(|job| job.phase == Phase::AwaitingReview)
        .unwrap();
    assert!(queued.dismissible);
    let mut dismissal = service.dismiss_request(queued.id).unwrap();
    wait_for(|| dismissal.try_recv().unwrap()).unwrap();
    assert!(!temporary.path().join("must not start").exists());
    assert_eq!(
        fs::read(temporary.path().join("nonempty/later.txt")).unwrap(),
        b"Later work"
    );
}

#[test]
#[ignore = "private child process, killed with an admitted request behind an Undo question"]
fn operation_worker_crash_helper() {
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_OPERATION_WORKER_CRASH_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-operation-worker-crash-")
    );
    let service = OperationJobs::start(
        root.join("state/history.sqlite3"),
        root.join("state/volumes.json"),
    )
    .unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let _review = folder_review(&service, &root);
    let mut queued = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: root.join("must not start"),
            }),
            None,
        )
        .unwrap();
    wait_for(|| queued.try_recv().unwrap()).unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    panic!("child was not killed");
}

#[test]
fn a_queued_rename_refuses_a_retargeted_browsing_alias() {
    use std::os::unix::fs::symlink;
    let temporary = tempfile::tempdir().unwrap();
    let original = temporary.path().join("original");
    let replacement = temporary.path().join("replacement");
    for directory in [&original, &replacement] {
        fs::create_dir(directory).unwrap();
    }
    fs::write(original.join("source.txt"), b"Original source").unwrap();
    fs::write(replacement.join("source.txt"), b"Replacement source").unwrap();
    let alias = temporary.path().join("browsing-alias");
    symlink(&original, &alias).unwrap();
    let service = OperationJobs::start(
        temporary.path().join("state/history.sqlite3"),
        temporary.path().join("state/volumes.json"),
    )
    .unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let FolderReview {
        job: undo,
        decision,
        ..
    } = folder_review(&service, temporary.path());
    let mut queued = service
        .submit(
            Request::Mutation(OperationRequest::Rename {
                source: alias.join("source.txt"),
                destination: alias.join("renamed.txt"),
            }),
            Some(Source {
                directory: alias.clone(),
                resolved: fs::canonicalize(&original).unwrap(),
            }),
        )
        .unwrap();
    let queued = wait_for(|| queued.try_recv().unwrap()).unwrap();
    fs::remove_file(&alias).unwrap();
    symlink(&replacement, &alias).unwrap();
    service
        .resolve(undo, decision.generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for_phase(&service, queued, Phase::Failed);
    assert_eq!(
        fs::read(original.join("source.txt")).unwrap(),
        b"Original source"
    );
    assert_eq!(
        fs::read(replacement.join("source.txt")).unwrap(),
        b"Replacement source"
    );
    assert!(!original.join("renamed.txt").exists());
    assert!(!replacement.join("renamed.txt").exists());
}

#[test]
fn queued_rename_preserves_replacement_files_folders_and_final_symlinks() {
    use std::os::unix::fs::{MetadataExt, symlink};
    for kind in ["file", "folder", "symlink", "missing"] {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("selected");
        let parked = temporary.path().join("original");
        let destination = temporary.path().join("renamed");
        match kind {
            "file" => fs::write(&source, b"Original file").unwrap(),
            "folder" => {
                fs::create_dir(&source).unwrap();
                fs::write(source.join("child"), b"Original child").unwrap();
            }
            "symlink" => symlink("original referent", &source).unwrap(),
            "missing" => {}
            _ => unreachable!(),
        }
        let service = OperationJobs::start(
            temporary.path().join("state/history.sqlite3"),
            temporary.path().join("state/volumes.json"),
        )
        .unwrap();
        wait_for(|| (!service.snapshot().initializing).then_some(()));
        let review = folder_review(&service, temporary.path());
        let mut admitted = service
            .submit(
                Request::Mutation(OperationRequest::Rename {
                    source: source.clone(),
                    destination: destination.clone(),
                }),
                None,
            )
            .unwrap();
        let rename = wait_for(|| admitted.try_recv().unwrap()).unwrap();
        assert_eq!(
            wait_for_phase(&service, rename, Phase::Queued).phase,
            Phase::Queued
        );
        if kind != "missing" {
            fs::rename(&source, &parked).unwrap();
        }
        fs::write(&source, b"Unrelated replacement").unwrap();
        let replacement = fs::symlink_metadata(&source).unwrap();
        service
            .resolve(review.job, review.decision.generation, UndoChoice::KeepItem)
            .unwrap();
        let result = wait_for(|| {
            service.snapshot().jobs.into_iter().find(|job| {
                job.id == rename && matches!(job.phase, Phase::Failed | Phase::Finished)
            })
        });
        assert_eq!(result.phase, Phase::Failed, "{kind}: {result:?}");
        assert!(result.error.is_some());
        assert!(!destination.try_exists().unwrap());
        let after = fs::symlink_metadata(&source).unwrap();
        assert_eq!(
            (after.dev(), after.ino()),
            (replacement.dev(), replacement.ino())
        );
        assert_eq!(fs::read(&source).unwrap(), b"Unrelated replacement");
        match kind {
            "file" => assert_eq!(fs::read(&parked).unwrap(), b"Original file"),
            "folder" => assert_eq!(fs::read(parked.join("child")).unwrap(), b"Original child"),
            "symlink" => assert_eq!(
                fs::read_link(&parked).unwrap(),
                Path::new("original referent")
            ),
            "missing" => assert!(!parked.exists()),
            _ => unreachable!(),
        }
        // A deliberate later request may select the replacement. The rejected
        // request must neither mutate it nor make it unusable as new work.
        let mut fresh = service
            .submit(
                Request::Mutation(OperationRequest::Rename {
                    source: source.clone(),
                    destination: destination.clone(),
                }),
                None,
            )
            .unwrap();
        let fresh = wait_for(|| fresh.try_recv().unwrap()).unwrap();
        wait_for_phase(&service, fresh, Phase::Finished);
        assert_eq!(fs::read(&destination).unwrap(), b"Unrelated replacement");
        let mut shutdown = service.shutdown().unwrap();
        wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    }
}

#[test]
fn queued_rename_accepts_returned_originals_and_preserves_final_symlink_semantics() {
    use std::{
        ffi::OsString,
        os::unix::{
            ffi::OsStringExt,
            fs::{MetadataExt, symlink},
        },
    };
    for kind in ["file", "folder", "symlink"] {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary
            .path()
            .join(OsString::from_vec(b"selected-\xff".to_vec()));
        let destination = temporary
            .path()
            .join(OsString::from_vec(b"renamed-\xfe".to_vec()));
        let parked = temporary.path().join("parked");
        let referent = temporary.path().join("referent");
        fs::write(&referent, b"Earlier referent").unwrap();
        match kind {
            "file" => fs::write(&source, b"Earlier file").unwrap(),
            "folder" => {
                fs::create_dir(&source).unwrap();
                fs::write(source.join("child"), b"Earlier child").unwrap();
            }
            "symlink" => symlink(&referent, &source).unwrap(),
            _ => unreachable!(),
        }
        let identity = fs::symlink_metadata(&source).unwrap();
        let service = OperationJobs::start(
            temporary.path().join("state/history.sqlite3"),
            temporary.path().join("state/volumes.json"),
        )
        .unwrap();
        wait_for(|| (!service.snapshot().initializing).then_some(()));
        let review = folder_review(&service, temporary.path());
        let mut admitted = service
            .submit(
                Request::Mutation(OperationRequest::Rename {
                    source: source.clone(),
                    destination: destination.clone(),
                }),
                None,
            )
            .unwrap();
        let rename = wait_for(|| admitted.try_recv().unwrap()).unwrap();
        wait_for_phase(&service, rename, Phase::Queued);
        fs::rename(&source, &parked).unwrap();
        fs::write(&source, b"Temporary replacement").unwrap();
        match kind {
            "file" => fs::write(&parked, b"Edited original file").unwrap(),
            "folder" => fs::write(parked.join("child"), b"Edited original child").unwrap(),
            "symlink" => {
                fs::remove_file(&referent).unwrap();
                fs::write(&referent, b"Newer referent").unwrap();
            }
            _ => unreachable!(),
        }
        fs::remove_file(&source).unwrap();
        fs::rename(&parked, &source).unwrap();
        service
            .resolve(review.job, review.decision.generation, UndoChoice::KeepItem)
            .unwrap();
        wait_for_phase(&service, rename, Phase::Finished);
        let after = fs::symlink_metadata(&destination).unwrap();
        assert_eq!((after.dev(), after.ino()), (identity.dev(), identity.ino()));
        match kind {
            "file" => assert_eq!(fs::read(&destination).unwrap(), b"Edited original file"),
            "folder" => assert_eq!(
                fs::read(destination.join("child")).unwrap(),
                b"Edited original child"
            ),
            "symlink" => {
                assert!(after.is_symlink());
                assert_eq!(fs::read_link(&destination).unwrap(), referent);
                assert_eq!(fs::read(&referent).unwrap(), b"Newer referent");
            }
            _ => unreachable!(),
        }
        assert!(!source.exists());
        let mut undo = service
            .submit(
                Request::Undo {
                    expected: service.snapshot().undo.unwrap().id,
                },
                None,
            )
            .unwrap();
        let undo = wait_for(|| undo.try_recv().unwrap()).unwrap();
        wait_for_phase(&service, undo, Phase::Finished);
        let after = fs::symlink_metadata(&source).unwrap();
        assert_eq!((after.dev(), after.ino()), (identity.dev(), identity.ino()));
        assert!(fs::symlink_metadata(&destination).is_err());
        let mut shutdown = service.shutdown().unwrap();
        wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    }
}

#[test]
fn queued_rename_refuses_replaced_parent_folders_even_if_the_leaf_survives() {
    for replace_source in [true, false] {
        let temporary = tempfile::tempdir().unwrap();
        let source_parent = temporary.path().join("source-parent");
        let destination_parent = temporary.path().join("destination-parent");
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&destination_parent).unwrap();
        let source = source_parent.join("selected");
        let destination = destination_parent.join("renamed");
        fs::write(&source, b"Original leaf").unwrap();
        let service = OperationJobs::start(
            temporary.path().join("state/history.sqlite3"),
            temporary.path().join("state/volumes.json"),
        )
        .unwrap();
        wait_for(|| (!service.snapshot().initializing).then_some(()));
        let review = folder_review(&service, temporary.path());
        let mut admitted = service
            .submit(
                Request::Mutation(OperationRequest::Rename {
                    source: source.clone(),
                    destination: destination.clone(),
                }),
                Some(Source {
                    directory: source_parent.clone(),
                    resolved: source_parent.clone(),
                }),
            )
            .unwrap();
        let rename = wait_for(|| admitted.try_recv().unwrap()).unwrap();
        wait_for_phase(&service, rename, Phase::Queued);
        let parent = if replace_source {
            &source_parent
        } else {
            &destination_parent
        };
        let parked = temporary.path().join("original-parent");
        fs::rename(parent, &parked).unwrap();
        fs::create_dir(parent).unwrap();
        if replace_source {
            fs::hard_link(parked.join("selected"), &source).unwrap();
        }
        fs::write(parent.join("unrelated"), b"Replacement folder work").unwrap();
        service
            .resolve(review.job, review.decision.generation, UndoChoice::KeepItem)
            .unwrap();
        let result = wait_for(|| {
            service.snapshot().jobs.into_iter().find(|job| {
                job.id == rename && matches!(job.phase, Phase::Failed | Phase::Finished)
            })
        });
        assert_eq!(
            result.phase,
            Phase::Failed,
            "replace_source={replace_source}: {result:?}"
        );
        assert_eq!(fs::read(&source).unwrap(), b"Original leaf");
        assert!(!destination.exists());
        assert_eq!(
            fs::read(parent.join("unrelated")).unwrap(),
            b"Replacement folder work"
        );
        if replace_source {
            assert_eq!(fs::read(parked.join("selected")).unwrap(), b"Original leaf");
        }
        let mut shutdown = service.shutdown().unwrap();
        wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    }
}

#[test]
fn queued_rename_follows_parent_aliases_and_accepts_the_original_folders_returning() {
    use std::os::unix::fs::symlink;
    let temporary = tempfile::tempdir().unwrap();
    let original = temporary.path().join("original");
    let alias = temporary.path().join("alias");
    let parked = temporary.path().join("parked");
    fs::create_dir(&original).unwrap();
    symlink(&original, &alias).unwrap();
    let source = alias.join("selected");
    let destination = alias.join("renamed");
    fs::write(&source, b"Original leaf").unwrap();
    let service = OperationJobs::start(
        temporary.path().join("state/history.sqlite3"),
        temporary.path().join("state/volumes.json"),
    )
    .unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let review = folder_review(&service, temporary.path());
    let mut admitted = service
        .submit(
            Request::Mutation(OperationRequest::Rename {
                source: source.clone(),
                destination: destination.clone(),
            }),
            Some(Source {
                directory: alias.clone(),
                resolved: original.clone(),
            }),
        )
        .unwrap();
    let rename = wait_for(|| admitted.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, rename, Phase::Queued);
    fs::rename(&original, &parked).unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("unrelated"), b"Replacement folder").unwrap();
    let replacement = temporary.path().join("replacement");
    fs::rename(&original, &replacement).unwrap();
    fs::rename(&parked, &original).unwrap();
    service
        .resolve(review.job, review.decision.generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for_phase(&service, rename, Phase::Finished);
    assert_eq!(fs::read(&destination).unwrap(), b"Original leaf");
    assert_eq!(
        fs::read(replacement.join("unrelated")).unwrap(),
        b"Replacement folder"
    );
    assert!(!replacement.join("renamed").exists());
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

fn retained_rename_fixture(root: &Path) -> (PathBuf, PathBuf) {
    use canopa_files::{
        Cancellation,
        operations::OperationEngine,
        transfers::{ConflictChoice, TransferEvent, TransferKind, TransferReply, TransferRequest},
    };
    let history = root.join("state/history.sqlite3");
    let incoming = root.join("incoming");
    let destination = root.join("destination");
    fs::create_dir(&incoming).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(incoming.join("note.txt"), b"Current destination bytes").unwrap();
    fs::write(destination.join("note.txt"), b"Older retained bytes").unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![incoming.join("note.txt")],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |event| match event {
                TransferEvent::Conflict { .. } => TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
                _ => TransferReply::Continue,
            },
        )
        .unwrap();
    let saved = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| item.entry.map(|entry| entry.path))
        .unwrap();
    drop(engine);
    (history, saved)
}

#[test]
fn queued_rename_keeps_its_retained_source_through_undo_history_expiry() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, saved) = retained_rename_fixture(root);
    let service = OperationJobs::start(history, root.join("state/volumes.json")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let review = folder_review(&service, root);
    // Admit Rename while the original version exists, but let older queued
    // operations expire its Undo owner before Rename reaches dispatch.
    for index in 0..100 {
        let mut admission = service
            .submit(
                Request::Mutation(OperationRequest::CreateFolder {
                    path: root.join(format!("expire-history-{index}")),
                }),
                None,
            )
            .unwrap();
        wait_for(|| admission.try_recv().unwrap()).unwrap();
    }
    let renamed = root.join("renamed retained bytes");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::Rename {
                source: saved.clone(),
                destination: renamed.clone(),
            }),
            None,
        )
        .unwrap();
    let rename = wait_for(|| admission.try_recv().unwrap()).unwrap();
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    service
        .resolve(review.job, review.decision.generation, UndoChoice::KeepItem)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    let result = loop {
        let job = service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == rename)
            .unwrap();
        if matches!(
            job.phase,
            Phase::Finished | Phase::Failed | Phase::SaveFailed
        ) {
            break job;
        }
        assert!(Instant::now() < deadline, "queued expiry did not finish");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(
        result.phase,
        Phase::Finished,
        "{result:?}; retained source still exists: {}",
        saved.exists()
    );
    assert!(!saved.exists());
    assert_eq!(fs::read(&renamed).unwrap(), b"Older retained bytes");
    assert_eq!(
        fs::read(root.join("destination/note.txt")).unwrap(),
        b"Current destination bytes"
    );
    let rename_operation = service.snapshot().undo.unwrap().id;
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: root.join("later operation triggers retained cleanup"),
            }),
            None,
        )
        .unwrap();
    let later = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, later, Phase::Finished);
    let mut admission = service
        .submit(
            Request::Undo {
                expected: service.snapshot().undo.unwrap().id,
            },
            None,
        )
        .unwrap();
    let undo_later = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, undo_later, Phase::Finished);
    let mut admission = service
        .submit(
            Request::Undo {
                expected: rename_operation,
            },
            None,
        )
        .unwrap();
    let undo = wait_for(|| admission.try_recv().unwrap()).unwrap();
    let result = wait_for(|| {
        service.snapshot().jobs.into_iter().find(|row| {
            row.id == undo && matches!(row.phase, Phase::Finished | Phase::Failed | Phase::Waiting)
        })
    });
    assert_eq!(
        result.phase,
        Phase::Finished,
        "{result:?}; original retained parent exists: {}",
        saved.parent().unwrap().exists()
    );
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    assert!(!renamed.exists());
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn refused_admission_and_queued_cancellation_release_retained_rename_ownership() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, saved) = retained_rename_fixture(root);
    let service = OperationJobs::start(history.clone(), root.join("state/volumes.json")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let review = folder_review(&service, root);
    let renamed = root.join("renamed saved bytes");
    let rename = || {
        Request::Mutation(OperationRequest::Rename {
            source: saved.clone(),
            destination: renamed.clone(),
        })
    };
    let database = rusqlite::Connection::open(&history).unwrap();
    let rows: u64 = database
        .query_row("SELECT count(*) FROM requests", [], |row| row.get(0))
        .unwrap();
    database.execute_batch("CREATE TRIGGER refuse_reservation BEFORE INSERT ON request_sources BEGIN SELECT RAISE(FAIL,'private admission refusal'); END;").unwrap();
    let mut refused = service.submit(rename(), None).unwrap();
    assert!(wait_for(|| refused.try_recv().unwrap()).is_err());
    assert_eq!(
        database
            .query_row("SELECT count(*) FROM requests", [], |row| row
                .get::<_, u64>(0))
            .unwrap(),
        rows
    );
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    database
        .execute_batch("DROP TRIGGER refuse_reservation")
        .unwrap();
    let mut admission = service.submit(rename(), None).unwrap();
    let cancelled = wait_for(|| admission.try_recv().unwrap()).unwrap();
    service.cancel(cancelled).unwrap();
    wait_for_phase(&service, cancelled, Phase::Finished);
    // A fresh accepted request is independent of the cancelled reservation.
    let mut admission = service.submit(rename(), None).unwrap();
    let fresh = wait_for(|| admission.try_recv().unwrap()).unwrap();
    service
        .resolve(review.job, review.decision.generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for_phase(&service, fresh, Phase::Finished);
    assert!(!saved.exists());
    assert_eq!(fs::read(&renamed).unwrap(), b"Older retained bytes");
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn failed_retained_rename_survives_restart_and_dismissal_releases_only_its_request() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, saved) = retained_rename_fixture(root);
    let start = || OperationJobs::start(history.clone(), root.join("state/volumes.json")).unwrap();
    let service = start();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let mut prepared = service.lease_sources(vec![saved.clone()]).unwrap();
    let lease = wait_for(|| prepared.try_recv().unwrap()).unwrap();
    let renamed = root.join("renamed saved bytes");
    let rename = || {
        Request::Mutation(OperationRequest::Rename {
            source: saved.clone(),
            destination: renamed.clone(),
        })
    };
    let mut admission = service.submit(rename(), None).unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    assert!(wait_for_phase(&service, id, Phase::Failed).dismissible);
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    lease.release().unwrap();
    drop(service);
    let service = start();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    assert!(
        service.snapshot().error.is_none(),
        "{:?}",
        service.snapshot()
    );
    assert!(wait_for_phase(&service, id, Phase::Failed).dismissible);
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    assert!(
        !renamed.exists(),
        "restoring the row executed its old request"
    );
    let database = rusqlite::Connection::open(&history).unwrap();
    database.execute_batch("CREATE TRIGGER refuse_dismissal BEFORE DELETE ON request_sources BEGIN SELECT RAISE(ABORT,'private dismissal refusal'); END;").unwrap();
    let mut dismissal = service.dismiss_request(id).unwrap();
    assert!(wait_for(|| dismissal.try_recv().unwrap()).is_err());
    assert!(wait_for_phase(&service, id, Phase::Failed).dismissible);
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    database
        .execute_batch("DROP TRIGGER refuse_dismissal")
        .unwrap();
    // Another independent live owner remains protected after dismissal.
    let mut prepared = service.lease_sources(vec![saved.clone()]).unwrap();
    let lease = wait_for(|| prepared.try_recv().unwrap()).unwrap();
    let mut dismissal = service.dismiss_request(id).unwrap();
    wait_for(|| dismissal.try_recv().unwrap()).unwrap();
    let dismissed = wait_for_phase(&service, id, Phase::Finished);
    assert!(!dismissed.dismissible);
    assert!(dismissed.detail.unwrap().contains("dismissed after review"));
    let mut admission = service.submit(rename(), None).unwrap();
    let second = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, second, Phase::Failed);
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    assert!(!renamed.exists());
    let mut dismissal = service.dismiss_request(second).unwrap();
    wait_for(|| dismissal.try_recv().unwrap()).unwrap();
    lease.release().unwrap();
    let mut admission = service.submit(rename(), None).unwrap();
    let fresh = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, fresh, Phase::Finished);
    assert_eq!(fs::read(&renamed).unwrap(), b"Older retained bytes");
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn dismissal_refuses_active_queued_work_and_a_real_unresolved_journal() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, saved) = retained_rename_fixture(root);
    let service = OperationJobs::start(history.clone(), root.join("state/volumes.json")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let review = folder_review(&service, root);
    let renamed = root.join("renamed saved bytes");
    let mut admission = service
        .submit(
            Request::Mutation(OperationRequest::Rename {
                source: saved.clone(),
                destination: renamed.clone(),
            }),
            None,
        )
        .unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    for refused in [review.job, id] {
        let mut dismissal = service.dismiss_request(refused).unwrap();
        assert!(wait_for(|| dismissal.try_recv().unwrap()).is_err());
    }
    let database = rusqlite::Connection::open(&history).unwrap();
    // Refuse recording the real rename after its filesystem syscall. Its
    // durable Forward journal must keep protecting the uncertain step.
    database.execute_batch("CREATE TRIGGER refuse_change BEFORE INSERT ON changes BEGIN SELECT RAISE(ABORT,'private confirmed-step refusal'); END;").unwrap();
    service
        .resolve(review.job, review.decision.generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for_phase(&service, id, Phase::Failed);
    assert_eq!(fs::read(&renamed).unwrap(), b"Older retained bytes");
    assert!(!saved.exists());
    let mut dismissal = service.dismiss_request(id).unwrap();
    assert!(
        wait_for(|| dismissal.try_recv().unwrap())
            .unwrap_err()
            .contains("Recovery")
    );
    assert_eq!(fs::read(&renamed).unwrap(), b"Older retained bytes");
    database
        .execute_batch("DROP TRIGGER refuse_change")
        .unwrap();
    let mut admission = service.submit(Request::Reconcile, None).unwrap();
    let reconciliation = wait_for(|| admission.try_recv().unwrap()).unwrap();
    let result = wait_for(|| {
        service.snapshot().jobs.into_iter().find(|row| {
            row.id == reconciliation
                && matches!(
                    row.phase,
                    Phase::Finished | Phase::Failed | Phase::SaveFailed
                )
        })
    });
    assert_eq!(result.phase, Phase::Finished, "{result:?}");
    let mut dismissal = service.dismiss_request(id).unwrap();
    wait_for(|| dismissal.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, id, Phase::Finished);
    assert_eq!(fs::read(&renamed).unwrap(), b"Older retained bytes");
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn queued_rename_observations_are_bounded_and_release_after_cancellation_and_completion() {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "rename_observation_budget_helper",
            "--nocapture",
        ])
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
#[ignore = "private child process with a reduced descriptor limit"]
fn rename_observation_budget_helper() {
    use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};
    let limits = getrlimit(Resource::Nofile);
    setrlimit(
        Resource::Nofile,
        Rlimit {
            current: Some(128),
            maximum: limits.maximum,
        },
    )
    .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("selected");
    let destination = temporary.path().join("renamed");
    fs::write(&source, b"Original leaf").unwrap();
    let service = OperationJobs::start(
        temporary.path().join("state/history.sqlite3"),
        temporary.path().join("state/volumes.json"),
    )
    .unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let review = folder_review(&service, temporary.path());
    let request = || {
        Request::Mutation(OperationRequest::Rename {
            source: source.clone(),
            destination: destination.clone(),
        })
    };
    let mut queued = Vec::new();
    loop {
        let mut admitted = service.submit(request(), None).unwrap();
        match wait_for(|| admitted.try_recv().unwrap()) {
            Ok(id) => queued.push(id),
            Err(error) => {
                assert!(error.contains("Too many selected sources"), "{error}");
                break;
            }
        }
        assert!(
            queued.len() <= 16,
            "Rename admission bypassed its pin budget"
        );
    }
    assert_eq!(
        queued.len(),
        16,
        "same-parent Rename needs two ordinary pins"
    );
    for id in queued {
        service.cancel(id).unwrap();
        wait_for_phase(&service, id, Phase::Finished);
    }
    // Cancellation returned every reservation; the waiting question remains usable.
    let mut admitted = service.submit(request(), None).unwrap();
    let rename = wait_for(|| admitted.try_recv().unwrap()).unwrap();
    service
        .resolve(review.job, review.decision.generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for_phase(&service, rename, Phase::Finished);
    assert_eq!(fs::read(&destination).unwrap(), b"Original leaf");
    // Finished rows must not retain pins. Repeat more than the live allowance.
    for _ in 0..20 {
        fs::rename(&destination, &source).unwrap();
        let mut admitted = service.submit(request(), None).unwrap();
        let rename = wait_for(|| admitted.try_recv().unwrap()).unwrap();
        wait_for_phase(&service, rename, Phase::Finished);
    }
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn the_platform_observer_can_invalidate_an_admitted_requests_original_volume() {
    let temporary = tempfile::tempdir().unwrap();
    let volume = temporary.path().join("volume");
    fs::create_dir(&volume).unwrap();
    let storage = Storage::load(temporary.path().join("state/volumes.json")).unwrap();
    let original = storage
        .mounted(volume.clone(), "uuid:original".into())
        .unwrap();
    let epoch = original.epoch;
    storage.prepare(original).unwrap();
    let service = OperationJobs::start_with_storage(
        temporary.path().join("state/history.sqlite3"),
        storage.clone(),
    )
    .unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let review = folder_review(&service, temporary.path());
    let target = volume.join("must stay absent");
    let mut admitted = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: target.clone(),
            }),
            None,
        )
        .unwrap();
    let job = wait_for(|| admitted.try_recv().unwrap()).unwrap();
    storage.removed(&volume, epoch);
    let replacement = storage.mounted(volume, "uuid:replacement".into()).unwrap();
    storage.prepare(replacement).unwrap();
    service
        .resolve(review.job, review.decision.generation, UndoChoice::KeepItem)
        .unwrap();
    let result = wait_for_phase(&service, job, Phase::Failed);
    assert!(result.error.is_some());
    assert!(!target.exists());
}

#[test]
fn losing_the_platform_observer_refuses_queued_work_even_outside_known_volumes() {
    let temporary = tempfile::tempdir().unwrap();
    let storage = Storage::load(temporary.path().join("state/volumes.json")).unwrap();
    let service = OperationJobs::start_with_storage(
        temporary.path().join("state/history.sqlite3"),
        storage.clone(),
    )
    .unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let review = folder_review(&service, temporary.path());
    let target = temporary.path().join("must stay absent");
    let mut admitted = service
        .submit(
            Request::Mutation(OperationRequest::CreateFolder {
                path: target.clone(),
            }),
            None,
        )
        .unwrap();
    let job = wait_for(|| admitted.try_recv().unwrap()).unwrap();
    storage.observer_lost();
    service
        .resolve(review.job, review.decision.generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for_phase(&service, job, Phase::Failed);
    assert!(!target.exists());
}

#[test]
fn undo_rechecks_placement_after_a_question_before_returning_or_replacing_files() {
    let temporary = tempfile::tempdir().unwrap();
    let storage = Storage::load(temporary.path().join("state/volumes.json")).unwrap();
    let service = OperationJobs::start_with_storage(
        temporary.path().join("state/history.sqlite3"),
        storage.clone(),
    )
    .unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let source = temporary.path().join("original.txt");
    let destination = temporary.path().join("renamed.txt");
    fs::write(&source, b"Original contents").unwrap();
    let mut admitted = service
        .submit(
            Request::Mutation(OperationRequest::Rename {
                source: source.clone(),
                destination: destination.clone(),
            }),
            None,
        )
        .unwrap();
    let rename = wait_for(|| admitted.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, rename, Phase::Finished);
    fs::write(&source, b"Newer work at original name").unwrap();
    let mut admitted = service
        .submit(
            Request::Undo {
                expected: service.snapshot().undo.unwrap().id,
            },
            None,
        )
        .unwrap();
    let undo = wait_for(|| admitted.try_recv().unwrap()).unwrap();
    let decision = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == undo)
            .and_then(|job| job.decision)
    });
    storage.observer_lost();
    service
        .resolve(undo, decision.generation, UndoChoice::Replace)
        .unwrap();
    wait_for_phase(&service, undo, Phase::Finished);
    assert_eq!(fs::read(&source).unwrap(), b"Newer work at original name");
    assert_eq!(fs::read(&destination).unwrap(), b"Original contents");
}

#[test]
fn prepared_rename_refuses_replacements_and_keeps_original_authority_after_refusal() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let directory = root.join("selected parent");
    fs::create_dir(&directory).unwrap();
    let source = directory.join("selected");
    let parked = directory.join("parked");
    fs::write(&source, b"Original").unwrap();
    let service = OperationJobs::start(root.join("history.db"), root.join("volumes.json")).unwrap();
    let mut preparing = service.prepare_rename(source.clone(), None).unwrap();
    let prepared = wait_for(|| preparing.try_recv().unwrap()).unwrap();
    fs::rename(&source, &parked).unwrap();
    fs::write(&source, b"Unselected replacement").unwrap();
    let mut admission = service
        .submit_rename(prepared.clone(), "renamed".into())
        .unwrap();
    assert!(
        wait_for(|| admission.try_recv().unwrap())
            .unwrap_err()
            .contains("changed")
    );
    assert_eq!(fs::read(&source).unwrap(), b"Unselected replacement");
    assert!(!directory.join("renamed").exists());
    fs::remove_file(&source).unwrap();
    fs::rename(&parked, &source).unwrap();
    let other =
        OperationJobs::start(root.join("other.db"), root.join("other-volumes.json")).unwrap();
    let mut foreign = other
        .submit_rename(prepared.clone(), "renamed".into())
        .unwrap();
    assert!(
        wait_for(|| foreign.try_recv().unwrap())
            .unwrap_err()
            .contains("another runtime")
    );
    for name in ["", ".", "..", "nested/name", "name/"] {
        let mut invalid = service
            .submit_rename(prepared.clone(), name.into())
            .unwrap();
        assert!(wait_for(|| invalid.try_recv().unwrap()).is_err());
    }
    // Parent aliases and final entries retain separate original observations.
    let parked_parent = root.join("parked parent");
    fs::rename(&directory, &parked_parent).unwrap();
    fs::create_dir(&directory).unwrap();
    fs::write(&source, b"Replacement folder contents").unwrap();
    let mut admission = service
        .submit_rename(prepared.clone(), "renamed".into())
        .unwrap();
    assert!(
        wait_for(|| admission.try_recv().unwrap())
            .unwrap_err()
            .contains("folder has changed")
    );
    fs::remove_file(&source).unwrap();
    fs::remove_dir(&directory).unwrap();
    fs::rename(&parked_parent, &directory).unwrap();
    let mut admission = service
        .submit_rename(prepared.clone(), "renamed".into())
        .unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, id, Phase::Finished);
    assert_eq!(fs::read(directory.join("renamed")).unwrap(), b"Original");
    let mut duplicate = service.submit_rename(prepared, "again".into()).unwrap();
    assert!(
        wait_for(|| duplicate.try_recv().unwrap())
            .unwrap_err()
            .contains("already been accepted")
    );
    for service in [service, other] {
        let mut shutdown = service.shutdown().unwrap();
        wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    }
}

#[test]
fn prepared_rename_atomically_transfers_retention_and_rollback_preserves_its_lease() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, saved) = retained_rename_fixture(root);
    let target = saved.parent().unwrap().join("prepared renamed");
    let service = OperationJobs::start(history.clone(), root.join("state/volumes.json")).unwrap();
    let mut preparing = service.prepare_rename(saved.clone(), None).unwrap();
    let prepared = wait_for(|| preparing.try_recv().unwrap()).unwrap();
    let database = rusqlite::Connection::open(&history).unwrap();
    let sources = || {
        database
            .query_row("SELECT count(*) FROM request_sources", [], |row| {
                row.get::<_, u64>(0)
            })
            .unwrap()
    };
    let original_sources = sources();
    assert!(original_sources > 0);
    database.execute_batch("CREATE TRIGGER refuse_handoff BEFORE DELETE ON request_sources WHEN EXISTS(SELECT 1 FROM source_leases WHERE request=OLD.request) BEGIN SELECT RAISE(ABORT,'private atomic handoff refusal'); END;").unwrap();
    let mut refused = service
        .submit_rename(prepared.clone(), "prepared renamed".into())
        .unwrap();
    assert!(
        wait_for(|| refused.try_recv().unwrap())
            .unwrap_err()
            .contains("private atomic handoff refusal")
    );
    assert_eq!(sources(), original_sources);
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    database
        .execute_batch("DROP TRIGGER refuse_handoff")
        .unwrap();
    let mut admission = service
        .submit_rename(prepared, "prepared renamed".into())
        .unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, id, Phase::Finished);
    assert_eq!(fs::read(&target).unwrap(), b"Older retained bytes");
    assert!(!saved.exists());
    let mut undo = service
        .submit(
            Request::Undo {
                expected: service.snapshot().undo.unwrap().id,
            },
            None,
        )
        .unwrap();
    let id = wait_for(|| undo.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, id, Phase::Finished);
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn queued_prepared_rename_keeps_original_observations_through_dispatch() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let source = root.join("selected");
    let parked = root.join("parked");
    fs::write(&source, b"Original selected bytes").unwrap();
    let service = OperationJobs::start(root.join("history.db"), root.join("volumes.json")).unwrap();
    let review = folder_review(&service, root);
    let mut preparing = service.prepare_rename(source.clone(), None).unwrap();
    let prepared = wait_for(|| preparing.try_recv().unwrap()).unwrap();
    let mut admission = service.submit_rename(prepared, "renamed".into()).unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    fs::rename(&source, &parked).unwrap();
    fs::write(&source, b"Replacement").unwrap();
    service
        .resolve(review.job, review.decision.generation, UndoChoice::KeepItem)
        .unwrap();
    wait_for_phase(&service, id, Phase::Failed);
    assert_eq!(fs::read(&source).unwrap(), b"Replacement");
    assert_eq!(fs::read(&parked).unwrap(), b"Original selected bytes");
    assert!(!root.join("renamed").exists());
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn prepared_rename_retains_saved_bytes_before_admission_and_releases_abandoned_drafts() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, saved) = retained_rename_fixture(root);
    let service = OperationJobs::start(history.clone(), root.join("state/volumes.json")).unwrap();
    let mut preparing = service.prepare_rename(saved.clone(), None).unwrap();
    let prepared = wait_for(|| preparing.try_recv().unwrap()).unwrap();
    let database = rusqlite::Connection::open(&history).unwrap();
    let held = || {
        database.query_row("SELECT count(*) FROM request_sources s JOIN source_leases l ON s.request=l.request", [], |row| row.get::<_, u64>(0)).unwrap()
    };
    assert!(held() > 0);
    let mut last = 0;
    for index in 0..100 {
        let mut admission = service
            .submit(
                Request::Mutation(OperationRequest::CreateFolder {
                    path: root.join(format!("expiry-{index}")),
                }),
                None,
            )
            .unwrap();
        last = wait_for(|| admission.try_recv().unwrap()).unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == last && job.phase == Phase::Finished)
        {
            break;
        }
        assert!(
            Instant::now() < deadline
                && !service.snapshot().jobs.iter().any(|job| matches!(
                    job.phase,
                    Phase::Failed | Phase::SaveFailed | Phase::StartSaveFailed
                )),
            "queued history expiry did not finish; last={last}; snapshot={:?}; phases={:?}",
            service
                .snapshot()
                .jobs
                .iter()
                .filter(|job| job.phase != Phase::Finished && job.phase != Phase::Queued)
                .map(|job| (job.id, job.phase, &job.error))
                .collect::<Vec<_>>(),
            database
                .prepare("SELECT phase,count(*) FROM requests GROUP BY phase")
                .unwrap()
                .query_map([], |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u64>(1)?)))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    // A shared clone is still the same prepared owner. Closing one draft handle
    // must not release the retained bytes while another handle can submit it.
    let clone = prepared.clone();
    drop(prepared);
    assert!(held() > 0);
    let target = saved.parent().unwrap().join("after expiry");
    let mut admission = service.submit_rename(clone, "after expiry".into()).unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, id, Phase::Finished);
    assert_eq!(fs::read(&target).unwrap(), b"Older retained bytes");
    wait_for(|| (held() == 0).then_some(()));
    let mut preparing = service.prepare_rename(target.clone(), None).unwrap();
    let abandoned = wait_for(|| preparing.try_recv().unwrap()).unwrap();
    assert!(held() > 0);
    drop(abandoned);
    wait_for(|| (held() == 0).then_some(()));
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn concurrent_queue_admission_and_filesystem_journaling_complete_without_lock_upgrade_failures() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let service = OperationJobs::start(root.join("history.db"), root.join("volumes.json")).unwrap();
    let mut last = 0;
    for index in 0..100 {
        let mut admission = service
            .submit(
                Request::Mutation(OperationRequest::CreateFolder {
                    path: root.join(format!("burst-{index}")),
                }),
                None,
            )
            .unwrap();
        last = wait_for(|| admission.try_recv().unwrap()).unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let snapshot = service.snapshot();
        let failures = snapshot
            .jobs
            .iter()
            .filter(|job| {
                matches!(
                    job.phase,
                    Phase::Failed | Phase::SaveFailed | Phase::StartSaveFailed
                )
            })
            .map(|job| (job.id, job.phase, &job.error))
            .collect::<Vec<_>>();
        assert!(
            failures.is_empty(),
            "overlapping durable admission/journal writes failed: {failures:?}"
        );
        if snapshot
            .jobs
            .iter()
            .any(|job| job.id == last && job.phase == Phase::Finished)
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "queue burst stalled: {snapshot:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    for index in 0..100 {
        assert!(root.join(format!("burst-{index}")).is_dir());
    }
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn independent_prepared_rename_owners_keep_each_others_retained_sources_protected() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (history, saved) = retained_rename_fixture(root);
    let service = OperationJobs::start(history, root.join("state/volumes.json")).unwrap();
    let mut preparing = service.prepare_rename(saved.clone(), None).unwrap();
    let first = wait_for(|| preparing.try_recv().unwrap()).unwrap();
    let mut preparing = service.prepare_rename(saved.clone(), None).unwrap();
    let other = wait_for(|| preparing.try_recv().unwrap()).unwrap();
    let mut admission = service
        .submit_rename(first, "independently protected".into())
        .unwrap();
    let failed = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, failed, Phase::Failed);
    assert_eq!(fs::read(&saved).unwrap(), b"Older retained bytes");
    assert!(
        !saved
            .parent()
            .unwrap()
            .join("independently protected")
            .exists()
    );
    let mut dismissal = service.dismiss_request(failed).unwrap();
    wait_for(|| dismissal.try_recv().unwrap()).unwrap();
    let mut admission = service
        .submit_rename(other, "independently protected".into())
        .unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, id, Phase::Finished);
    assert_eq!(
        fs::read(saved.parent().unwrap().join("independently protected")).unwrap(),
        b"Older retained bytes"
    );
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn prepared_rename_does_not_adopt_replacement_storage_while_its_name_is_drafted() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let volume = root.join("volume");
    fs::create_dir(&volume).unwrap();
    let source = volume.join("selected");
    fs::write(&source, b"Originally selected bytes").unwrap();
    let storage = Storage::load(root.join("state/volumes.json")).unwrap();
    let original = storage
        .mounted(volume.clone(), "uuid:original".into())
        .unwrap();
    let epoch = original.epoch;
    storage.prepare(original).unwrap();
    let service =
        OperationJobs::start_with_storage(root.join("state/history.sqlite3"), storage.clone())
            .unwrap();
    let mut preparing = service.prepare_rename(source.clone(), None).unwrap();
    let prepared = wait_for(|| preparing.try_recv().unwrap()).unwrap();
    // The private provider changes authority while the actual directory, entry
    // and their open pins are unchanged. No host mount is altered.
    storage.removed(&volume, epoch);
    let replacement = storage
        .mounted(volume.clone(), "uuid:replacement".into())
        .unwrap();
    storage.prepare(replacement).unwrap();
    let mut admission = service.submit_rename(prepared, "renamed".into()).unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    let result = wait_for(|| {
        service.snapshot().jobs.into_iter().find(|job| {
            job.id == id
                && matches!(
                    job.phase,
                    Phase::Finished | Phase::Failed | Phase::SaveFailed
                )
        })
    });
    assert_eq!(
        result.phase,
        Phase::Failed,
        "a drafted Rename adopted replacement storage: {result:?}"
    );
    assert_eq!(fs::read(&source).unwrap(), b"Originally selected bytes");
    assert!(!volume.join("renamed").exists());
    let mut preparing = service.prepare_rename(source.clone(), None).unwrap();
    let reviewed = wait_for(|| preparing.try_recv().unwrap()).unwrap();
    let mut admission = service
        .submit_rename(reviewed, "reviewed replacement".into())
        .unwrap();
    let id = wait_for(|| admission.try_recv().unwrap()).unwrap();
    wait_for_phase(&service, id, Phase::Finished);
    assert_eq!(
        fs::read(volume.join("reviewed replacement")).unwrap(),
        b"Originally selected bytes"
    );
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}
