use canopa_files::{
    operation_jobs::{OperationJobs, Phase, Question, Request},
    operations::{OperationRequest, UndoChoice},
    storage::Storage,
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            Instant::now() < until,
            "transfer service did not reach its expected state"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn service(root: &Path) -> OperationJobs {
    let service =
        OperationJobs::start(root.join("state/history"), root.join("state/volumes")).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    assert!(service.snapshot().error.is_none());
    service
}

fn lose_idle_owner(service: OperationJobs, history: &Path) {
    drop(service);
    // Drop requests teardown asynchronously. Observe release of the actual
    // exclusive history lock before starting another owner; do not race it.
    let engine = wait_for(
        || match canopa_files::operations::OperationEngine::open(history) {
            Ok(engine) => Some(engine),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(error) => panic!("owner-loss history probe failed: {error}"),
        },
    );
    drop(engine);
}

fn submit(service: &OperationJobs, request: Request) -> u64 {
    let mut reply = service.submit(request, None).unwrap();
    wait_for(|| reply.try_recv().unwrap()).unwrap()
}

fn finished(service: &OperationJobs, id: u64) {
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Finished)
    });
}

fn question(service: &OperationJobs, id: u64) -> canopa_files::operation_jobs::Decision {
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id)
            .and_then(|job| job.decision)
    })
}

#[test]
fn prepared_clipboard_preserves_selected_identity_until_paste_and_allows_deliberate_reselection() {
    use canopa_files::file_clipboard::Selection;
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("selected.txt");
        let parked = root.path().join("original.txt");
        let destination = root.path().join("destination");
        fs::create_dir(&destination).unwrap();
        fs::write(&source, b"Originally selected bytes").unwrap();
        let storage = Storage::load(root.path().join("state/volumes")).unwrap();
        let jobs =
            OperationJobs::start_with_storage(root.path().join("state/history"), storage.clone())
                .unwrap();
        wait_for(|| (!jobs.snapshot().initializing).then_some(()));
        let mut preparation = jobs
            .prepare_clipboard(
                Selection::new(vec![source.clone()], kind).unwrap(),
                storage.snapshot(),
            )
            .unwrap();
        let prepared = wait_for(|| preparation.try_recv().unwrap()).unwrap();
        fs::rename(&source, &parked).unwrap();
        fs::write(&source, b"Unrelated replacement bytes").unwrap();
        for _ in 0..2 {
            let mut reply = jobs
                .paste_clipboard(prepared.clone(), destination.clone(), None)
                .unwrap();
            let id = wait_for(|| reply.try_recv().unwrap()).unwrap();
            let report = wait_for(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|job| job.id == id && job.phase == Phase::Stopped)
                    .and_then(|job| job.transfer)
            });
            assert_eq!(report.completed, 0);
            assert_eq!(report.failures.len(), 1);
            assert_eq!(fs::read(&source).unwrap(), b"Unrelated replacement bytes");
            assert_eq!(fs::read(&parked).unwrap(), b"Originally selected bytes");
            assert!(!destination.join("selected.txt").exists());
            let state = prepared.snapshot();
            assert!(!state.busy);
            assert_eq!(
                state.selection.unwrap().paths(),
                std::slice::from_ref(&source)
            );
        }
        let mut preparation = jobs
            .prepare_clipboard(
                Selection::new(vec![source.clone()], kind).unwrap(),
                storage.snapshot(),
            )
            .unwrap();
        let fresh = wait_for(|| preparation.try_recv().unwrap()).unwrap();
        let mut reply = jobs
            .paste_clipboard(fresh.clone(), destination.clone(), None)
            .unwrap();
        let id = wait_for(|| reply.try_recv().unwrap()).unwrap();
        finished(&jobs, id);
        assert_eq!(
            fs::read(destination.join("selected.txt")).unwrap(),
            b"Unrelated replacement bytes"
        );
        assert_eq!(fs::read(&parked).unwrap(), b"Originally selected bytes");
        assert_eq!(source.exists(), kind == TransferKind::Copy);
        assert_eq!(
            fresh.snapshot().selection.is_none(),
            kind == TransferKind::Move
        );
        // Settlement of the new intent cannot consume the old failed intent.
        assert_eq!(prepared.snapshot().selection.unwrap().paths(), &[source]);
    }
}

#[test]
fn a_partial_cut_keeps_original_remainders_and_refuses_a_second_unsettled_paste() {
    use canopa_files::file_clipboard::Selection;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    let other = root.path().join("other");
    for path in [&source, &destination, &other] {
        fs::create_dir(path).unwrap();
    }
    let first = source.join("a.txt");
    let second = source.join("b.txt");
    fs::write(&first, b"First selected file").unwrap();
    fs::write(&second, b"Second selected file").unwrap();
    fs::write(destination.join("b.txt"), b"Existing destination").unwrap();
    let storage = Storage::load(root.path().join("state/volumes")).unwrap();
    let jobs =
        OperationJobs::start_with_storage(root.path().join("state/history"), storage.clone())
            .unwrap();
    wait_for(|| (!jobs.snapshot().initializing).then_some(()));
    let mut reply = jobs
        .prepare_clipboard(
            Selection::new(vec![first.clone(), second.clone()], TransferKind::Move).unwrap(),
            storage.snapshot(),
        )
        .unwrap();
    let prepared = wait_for(|| reply.try_recv().unwrap()).unwrap();
    let mut reply = jobs
        .paste_clipboard(prepared.clone(), destination.clone(), None)
        .unwrap();
    let id = wait_for(|| reply.try_recv().unwrap()).unwrap();
    let decision = question(&jobs, id);
    assert!(prepared.snapshot().busy);
    assert_eq!(
        fs::read(destination.join("a.txt")).unwrap(),
        b"First selected file"
    );
    let mut duplicate = jobs
        .paste_clipboard(prepared.clone(), other.clone(), None)
        .unwrap();
    assert!(
        wait_for(|| duplicate.try_recv().unwrap())
            .unwrap_err()
            .contains("already")
    );
    assert!(!other.join("a.txt").exists() && !other.join("b.txt").exists());
    // Preparing another intent remains possible during the live question and
    // the old completion may not alter that independently prepared selection.
    let marker = source.join("marker.txt");
    fs::write(&marker, b"New deliberate intent").unwrap();
    let mut reply = jobs
        .prepare_clipboard(
            Selection::new(vec![marker.clone()], TransferKind::Move).unwrap(),
            storage.snapshot(),
        )
        .unwrap();
    let newer = wait_for(|| reply.try_recv().unwrap()).unwrap();
    jobs.resolve_transfer(
        id,
        decision.generation,
        TransferReply::Resolve {
            choice: ConflictChoice::Skip,
            apply_to_remaining: false,
        },
    )
    .unwrap();
    finished(&jobs, id);
    let state = prepared.snapshot();
    assert!(!state.busy);
    assert_eq!(state.revision, 1);
    assert_eq!(
        state.selection.unwrap().paths(),
        std::slice::from_ref(&second)
    );
    assert_eq!(
        newer.snapshot().selection.unwrap().paths(),
        std::slice::from_ref(&marker)
    );
    assert!(!newer.snapshot().busy);
    let parked = root.path().join("original b.txt");
    fs::rename(&second, &parked).unwrap();
    fs::write(&second, b"Unrelated replacement").unwrap();
    let mut reply = jobs
        .paste_clipboard(prepared.clone(), other.clone(), None)
        .unwrap();
    let id = wait_for(|| reply.try_recv().unwrap()).unwrap();
    let report = wait_for(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Stopped)
            .and_then(|job| job.transfer)
    });
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].source, second);
    assert_eq!(prepared.snapshot().revision, 1);
    assert_eq!(
        prepared.snapshot().selection.unwrap().paths(),
        std::slice::from_ref(&second)
    );
    assert_eq!(fs::read(&second).unwrap(), b"Unrelated replacement");
    assert_eq!(fs::read(&parked).unwrap(), b"Second selected file");
    assert_eq!(
        fs::read(destination.join("b.txt")).unwrap(),
        b"Existing destination"
    );
    assert!(!other.join("a.txt").exists() && !other.join("b.txt").exists());
    let mut reply = jobs
        .paste_clipboard(newer.clone(), other.clone(), None)
        .unwrap();
    let id = wait_for(|| reply.try_recv().unwrap()).unwrap();
    finished(&jobs, id);
    assert!(newer.snapshot().selection.is_none());
    assert_eq!(
        fs::read(other.join("marker.txt")).unwrap(),
        b"New deliberate intent"
    );
    assert_eq!(prepared.snapshot().selection.unwrap().paths(), &[second]);
}

#[test]
fn partial_cut_can_paste_its_remainder_after_a_completed_saved_container_migrates() {
    use canopa_files::{Cancellation, file_clipboard::Selection, operations::OperationEngine};
    use std::{os::unix::fs::MetadataExt, path::PathBuf};
    let root = tempfile::tempdir().unwrap();
    let device = fs::metadata(root.path()).unwrap().dev();
    let other = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|base| {
            tempfile::Builder::new()
                .prefix(".clipboard-partial-migration-")
                .tempdir_in(base)
                .ok()
                .filter(|folder| fs::metadata(folder.path()).unwrap().dev() != device)
        })
        .expect("a writable second filesystem is required");
    let project = root.path().join("project");
    let incoming = root.path().join("incoming");
    let target = other.path().join("target");
    let remaining = other.path().join("remaining");
    for path in [&project, &incoming, &target, &remaining] {
        fs::create_dir(path).unwrap();
    }
    let file = project.join("note.txt");
    fs::write(&file, b"Retained older bytes").unwrap();
    fs::write(incoming.join("note.txt"), b"Current project bytes").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let replacement = engine
        .transfer(
            &TransferRequest {
                sources: vec![incoming.join("note.txt")],
                kind: TransferKind::Copy,
                destination: project.clone(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(
                    event,
                    canopa_files::transfers::TransferEvent::Conflict { .. }
                ) {
                    TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        apply_to_remaining: false,
                    }
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    assert!(replacement.failures.is_empty());
    let saved = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| item.entry.map(|entry| entry.path))
        .unwrap();
    assert_eq!(fs::read(&saved).unwrap(), b"Retained older bytes");
    let saved_relative = saved.strip_prefix(&project).unwrap().to_owned();
    drop(engine);
    let source = root.path().join("remainder.txt");
    fs::write(&source, b"Unmoved remainder bytes").unwrap();
    fs::write(target.join("remainder.txt"), b"Existing target bytes").unwrap();
    let storage = Storage::load(root.path().join("state/volumes")).unwrap();
    let jobs = OperationJobs::start_with_storage(history, storage.clone()).unwrap();
    wait_for(|| (!jobs.snapshot().initializing).then_some(()));
    let mut preparation = jobs
        .prepare_clipboard(
            Selection::new(vec![project.clone(), source.clone()], TransferKind::Move).unwrap(),
            storage.snapshot(),
        )
        .unwrap();
    let prepared = wait_for(|| preparation.try_recv().unwrap()).unwrap();
    let mut reply = jobs
        .paste_clipboard(prepared.clone(), target.clone(), None)
        .unwrap();
    let id = wait_for(|| reply.try_recv().unwrap()).unwrap();
    let decision = wait_for(|| {
        let job = jobs
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id)
            .unwrap();
        assert!(
            matches!(job.phase, Phase::Queued | Phase::Running | Phase::Waiting),
            "Paste stopped before its expected conflict: {job:?}"
        );
        job.decision
    });
    assert!(!project.exists());
    let migrated = target.join("project");
    assert_eq!(
        fs::read(migrated.join("note.txt")).unwrap(),
        b"Current project bytes"
    );
    assert_eq!(
        fs::read(migrated.join(&saved_relative)).unwrap(),
        b"Retained older bytes"
    );
    jobs.resolve_transfer(
        id,
        decision.generation,
        TransferReply::Resolve {
            choice: ConflictChoice::Skip,
            apply_to_remaining: false,
        },
    )
    .unwrap();
    finished(&jobs, id);
    assert_eq!(
        prepared.snapshot().selection.unwrap().paths(),
        std::slice::from_ref(&source)
    );
    let mut reply = jobs
        .paste_clipboard(prepared.clone(), remaining.clone(), None)
        .unwrap();
    let id = wait_for(|| reply.try_recv().unwrap())
        .expect("the completed container must not prevent a Paste of the original remainder");
    finished(&jobs, id);
    assert!(prepared.snapshot().selection.is_none());
    assert!(!source.exists());
    assert_eq!(
        fs::read(remaining.join("remainder.txt")).unwrap(),
        b"Unmoved remainder bytes"
    );
    assert_eq!(
        fs::read(target.join("remainder.txt")).unwrap(),
        b"Existing target bytes"
    );
    assert_eq!(
        fs::read(migrated.join("note.txt")).unwrap(),
        b"Current project bytes"
    );
    assert_eq!(
        fs::read(migrated.join(saved_relative)).unwrap(),
        b"Retained older bytes"
    );
}

#[test]
fn a_prepared_cut_cannot_be_adopted_by_another_history() {
    use canopa_files::file_clipboard::Selection;
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    for path in [&first, &second] {
        fs::create_dir(path).unwrap();
    }
    let source = first.join("selected.txt");
    let foreign_source = second.join("selected.txt");
    fs::write(&source, b"Original first selection").unwrap();
    fs::write(&foreign_source, b"Independent second selection").unwrap();
    let first_storage = Storage::load(first.join("state/volumes")).unwrap();
    let second_storage = Storage::load(second.join("state/volumes")).unwrap();
    let first_jobs =
        OperationJobs::start_with_storage(first.join("state/history"), first_storage.clone())
            .unwrap();
    let second_jobs =
        OperationJobs::start_with_storage(second.join("state/history"), second_storage.clone())
            .unwrap();
    wait_for(|| {
        (!first_jobs.snapshot().initializing && !second_jobs.snapshot().initializing).then_some(())
    });
    let mut preparation = first_jobs
        .prepare_clipboard(
            Selection::new(vec![source.clone()], TransferKind::Move).unwrap(),
            first_storage.snapshot(),
        )
        .unwrap();
    let prepared = wait_for(|| preparation.try_recv().unwrap()).unwrap();
    // Each history has its own first live owner; a private row number alone
    // cannot establish that the supplied handle belongs to the receiving core.
    let mut preparation = second_jobs
        .prepare_clipboard(
            Selection::new(vec![foreign_source.clone()], TransferKind::Move).unwrap(),
            second_storage.snapshot(),
        )
        .unwrap();
    let independent = wait_for(|| preparation.try_recv().unwrap()).unwrap();
    let target = second.join("destination");
    fs::create_dir(&target).unwrap();
    let mut reply = second_jobs
        .paste_clipboard(prepared.clone(), target.clone(), None)
        .unwrap();
    assert!(
        wait_for(|| reply.try_recv().unwrap())
            .unwrap_err()
            .contains("another operation history")
    );
    assert!(!prepared.snapshot().busy && !independent.snapshot().busy);
    assert_eq!(
        independent.snapshot().selection.unwrap().paths(),
        std::slice::from_ref(&foreign_source)
    );
    assert!(second_jobs.snapshot().jobs.is_empty());
    assert!(!target.join("selected.txt").exists());
    assert_eq!(fs::read(&source).unwrap(), b"Original first selection");
    assert_eq!(
        fs::read(&foreign_source).unwrap(),
        b"Independent second selection"
    );
    let target = first.join("destination");
    fs::create_dir(&target).unwrap();
    let mut reply = first_jobs
        .paste_clipboard(prepared.clone(), target.clone(), None)
        .unwrap();
    let id = wait_for(|| reply.try_recv().unwrap()).unwrap();
    finished(&first_jobs, id);
    assert!(prepared.snapshot().selection.is_none());
    assert_eq!(
        fs::read(target.join("selected.txt")).unwrap(),
        b"Original first selection"
    );
    assert!(!source.exists());
    assert_eq!(
        fs::read(&foreign_source).unwrap(),
        b"Independent second selection"
    );
}

#[test]
fn a_queued_transfer_keeps_its_admitted_destination_and_waits_for_the_original_folder() {
    use canopa_files::operation_jobs::TransferQuestion;
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let root = tempfile::tempdir().unwrap();
        let blocker_destination = root.path().join("blocker destination");
        let destination = root.path().join("destination");
        let original = root.path().join("original folder");
        let replacement = root.path().join("replacement folder");
        fs::create_dir(&blocker_destination).unwrap();
        fs::create_dir(&destination).unwrap();
        let blocker = root.path().join("blocker");
        let source = root.path().join("selected");
        fs::write(&blocker, b"Uncopied blocker").unwrap();
        fs::write(blocker_destination.join("blocker"), b"Existing blocker").unwrap();
        fs::write(&source, b"Originally selected bytes").unwrap();
        let jobs = service(root.path());
        let blocking = submit(
            &jobs,
            Request::Transfer(TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![blocker],
                destination: blocker_destination,
            }),
        );
        let conflict = question(&jobs, blocking);
        let queued = submit(
            &jobs,
            Request::Transfer(TransferRequest {
                kind,
                sources: vec![source.clone()],
                destination: destination.clone(),
            }),
        );
        assert_eq!(
            jobs.snapshot()
                .jobs
                .iter()
                .find(|job| job.id == queued)
                .unwrap()
                .phase,
            Phase::Queued
        );
        fs::rename(&destination, &original).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("marker"), b"Unrelated destination bytes").unwrap();
        jobs.resolve_transfer(
            blocking,
            conflict.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::Skip,
                apply_to_remaining: false,
            },
        )
        .unwrap();
        finished(&jobs, blocking);
        let unavailable = question(&jobs, queued);
        assert!(matches!(
            unavailable.question,
            Question::Transfer(TransferQuestion::Unavailable { .. })
        ));
        assert_eq!(fs::read(&source).unwrap(), b"Originally selected bytes");
        assert!(!destination.join("selected").exists());
        assert_eq!(
            fs::read(destination.join("marker")).unwrap(),
            b"Unrelated destination bytes"
        );
        fs::rename(&destination, &replacement).unwrap();
        fs::rename(&original, &destination).unwrap();
        jobs.resolve_transfer(queued, unavailable.generation, TransferReply::Retry)
            .unwrap();
        finished(&jobs, queued);
        assert_eq!(
            fs::read(destination.join("selected")).unwrap(),
            b"Originally selected bytes"
        );
        assert_eq!(source.exists(), kind == TransferKind::Copy);
        let undo = submit(
            &jobs,
            Request::Undo {
                expected: jobs.snapshot().undo.unwrap().id,
            },
        );
        finished(&jobs, undo);
        assert_eq!(fs::read(&source).unwrap(), b"Originally selected bytes");
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
        assert_eq!(
            fs::read(replacement.join("marker")).unwrap(),
            b"Unrelated destination bytes"
        );
    }
}

#[test]
fn a_queued_move_preserves_a_replacement_at_its_selected_source_path() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let blocker = root.path().join("blocker.txt");
    fs::write(&blocker, b"Uncopied blocker source").unwrap();
    fs::write(
        destination.join("blocker.txt"),
        b"Existing blocker destination",
    )
    .unwrap();
    let source = root.path().join("selected.txt");
    let parked = root.path().join("original selected file");
    fs::write(&source, b"Original selected bytes").unwrap();
    let jobs = service(root.path());
    let blocking = submit(
        &jobs,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![blocker],
            destination: destination.clone(),
        }),
    );
    let review = question(&jobs, blocking);
    let queued = submit(
        &jobs,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Move,
            sources: vec![source.clone()],
            destination: destination.clone(),
        }),
    );
    assert_eq!(
        jobs.snapshot()
            .jobs
            .iter()
            .find(|job| job.id == queued)
            .unwrap()
            .phase,
        Phase::Queued
    );
    fs::rename(&source, &parked).unwrap();
    fs::write(&source, b"Unrelated replacement bytes").unwrap();
    jobs.resolve_transfer(
        blocking,
        review.generation,
        TransferReply::Resolve {
            choice: ConflictChoice::Skip,
            apply_to_remaining: false,
        },
    )
    .unwrap();
    finished(&jobs, blocking);
    let result = wait_for(|| {
        jobs.snapshot().jobs.into_iter().find(|job| {
            job.id == queued
                && matches!(job.phase, Phase::Stopped | Phase::Finished | Phase::Failed)
        })
    });
    assert!(
        source.exists(),
        "queued Move consumed an unrelated replacement"
    );
    assert_eq!(fs::read(&source).unwrap(), b"Unrelated replacement bytes");
    assert_eq!(fs::read(&parked).unwrap(), b"Original selected bytes");
    assert!(!destination.join("selected.txt").exists());
    let report = result
        .transfer
        .expect("the refused source needs an inspectable transfer result");
    assert_eq!(report.completed, 0);
    assert_eq!(report.failures.len(), 1);
    assert!(jobs.snapshot().undo.is_none());

    let fresh = submit(
        &jobs,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Move,
            sources: vec![source.clone()],
            destination: destination.clone(),
        }),
    );
    finished(&jobs, fresh);
    assert!(!source.exists());
    assert_eq!(
        fs::read(destination.join("selected.txt")).unwrap(),
        b"Unrelated replacement bytes"
    );
    assert_eq!(fs::read(&parked).unwrap(), b"Original selected bytes");
    let undo = submit(
        &jobs,
        Request::Undo {
            expected: jobs.snapshot().undo.unwrap().id,
        },
    );
    finished(&jobs, undo);
    assert_eq!(fs::read(&source).unwrap(), b"Unrelated replacement bytes");
    assert_eq!(fs::read(&parked).unwrap(), b"Original selected bytes");
    assert!(!destination.join("selected.txt").exists());
}

#[test]
fn queued_source_observations_leave_capacity_for_questions_and_later_work() {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "source_observation_budget_helper",
            "--test-threads=1",
        ])
        .status()
        .unwrap();
    assert!(
        status.success(),
        "isolated descriptor-budget acceptance failed"
    );
}

#[test]
#[ignore = "private child process with a reduced descriptor limit"]
fn source_observation_budget_helper() {
    use rustix::process::{Resource, getrlimit, setrlimit};
    let mut limit = getrlimit(Resource::Nofile);
    limit.current = Some(128);
    setrlimit(Resource::Nofile, limit).unwrap();
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let blocker = root.path().join("blocker");
    fs::write(&blocker, b"Blocker source").unwrap();
    fs::write(destination.join("blocker"), b"Blocker destination").unwrap();
    let sources: Vec<_> = (0..64)
        .map(|index| root.path().join(format!("selected-{index}")))
        .collect();
    for source in &sources {
        fs::write(source, b"Selected bytes").unwrap();
    }
    let jobs = service(root.path());
    let blocking = submit(
        &jobs,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![blocker],
            destination: destination.clone(),
        }),
    );
    let review = question(&jobs, blocking);
    let mut oversized = jobs
        .submit(
            Request::Transfer(TransferRequest {
                kind: TransferKind::Copy,
                sources: sources.clone(),
                destination: destination.clone(),
            }),
            None,
        )
        .unwrap();
    assert!(
        wait_for(|| oversized.try_recv().unwrap()).is_err(),
        "an oversized queued selection was admitted without preserving descriptor headroom"
    );
    assert_eq!(jobs.snapshot().jobs.len(), 1);
    let first = submit(
        &jobs,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: sources[..16].to_vec(),
            destination: destination.clone(),
        }),
    );
    let mut over_shared_budget = jobs
        .submit(
            Request::Transfer(TransferRequest {
                kind: TransferKind::Copy,
                sources: sources[16..32].to_vec(),
                destination: destination.clone(),
            }),
            None,
        )
        .unwrap();
    assert!(
        wait_for(|| over_shared_budget.try_recv().unwrap()).is_err(),
        "separate requests exceeded the shared source-observation budget"
    );
    let created = root.path().join("accepted alongside source observations");
    let mutation = submit(
        &jobs,
        Request::Mutation(OperationRequest::CreateFolder {
            path: created.clone(),
        }),
    );
    jobs.resolve_transfer(
        blocking,
        review.generation,
        TransferReply::Resolve {
            choice: ConflictChoice::Skip,
            apply_to_remaining: false,
        },
    )
    .unwrap();
    finished(&jobs, blocking);
    finished(&jobs, first);
    finished(&jobs, mutation);
    assert!(created.is_dir());
    let later = submit(
        &jobs,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: sources[16..32].to_vec(),
            destination: destination.clone(),
        }),
    );
    finished(&jobs, later);
    for source in &sources[..32] {
        assert_eq!(fs::read(source).unwrap(), b"Selected bytes");
        assert_eq!(
            fs::read(destination.join(source.file_name().unwrap())).unwrap(),
            b"Selected bytes"
        );
    }
}

#[test]
fn source_remainders_reach_the_shared_report_and_survive_reopening() {
    use canopa_files::operation_jobs::TransferQuestion;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source/Album");
    let target = root.path().join("destination");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(target.join("Album")).unwrap();
    fs::write(source.join("a.txt"), b"First incoming file").unwrap();
    fs::write(source.join("b.txt"), b"Second incoming file").unwrap();
    fs::write(target.join("Album/b.txt"), b"Previous destination").unwrap();
    let jobs = service(root.path());
    let id = submit(
        &jobs,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Move,
            sources: vec![source.clone()],
            destination: target.clone(),
        }),
    );
    let merge = question(&jobs, id);
    assert!(matches!(
        merge.question,
        Question::Transfer(TransferQuestion::Conflict { folders: true, .. })
    ));
    jobs.resolve_transfer(
        id,
        merge.generation,
        TransferReply::Resolve {
            choice: ConflictChoice::Merge,
            apply_to_remaining: false,
        },
    )
    .unwrap();
    let file = wait_for(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id)
            .and_then(|job| job.decision)
            .filter(|decision| decision.generation > merge.generation)
    });
    assert!(
        matches!(&file.question, Question::Transfer(TransferQuestion::Conflict { source: entry, .. }) if entry.path == source.join("b.txt"))
    );
    assert_eq!(
        fs::read(target.join("Album/a.txt")).unwrap(),
        b"First incoming file"
    );
    fs::write(source.join("later.txt"), b"New source work stays here").unwrap();
    jobs.resolve_transfer(
        id,
        file.generation,
        TransferReply::Resolve {
            choice: ConflictChoice::Replace,
            apply_to_remaining: false,
        },
    )
    .unwrap();
    finished(&jobs, id);
    let report = jobs
        .snapshot()
        .jobs
        .into_iter()
        .find(|job| job.id == id)
        .unwrap()
        .transfer
        .unwrap();
    assert_eq!(
        report.source_remainders.as_slice(),
        std::slice::from_ref(&source)
    );
    assert_eq!(
        fs::read(source.join("later.txt")).unwrap(),
        b"New source work stays here"
    );
    let mut shutdown = jobs.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    drop(jobs);
    let reopened = service(root.path());
    let report = reopened
        .snapshot()
        .jobs
        .into_iter()
        .find(|job| job.id == id)
        .unwrap()
        .transfer
        .unwrap();
    assert_eq!(report.source_remainders, [source]);
    assert_eq!(
        fs::read(target.join("Album/b.txt")).unwrap(),
        b"Second incoming file"
    );
}

#[test]
fn a_stopped_transfer_keeps_its_retained_source_after_other_jobs_expire_history_and_restart() {
    use canopa_files::{Cancellation, operations::OperationEngine, transfers::TransferEvent};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"New destination bytes").unwrap();
    fs::write(destination.join("source.txt"), b"Retained source bytes").unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
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
    let retained = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| item.entry.map(|entry| entry.path))
        .unwrap();
    drop(engine);
    let jobs = service(root.path());
    let id = submit(
        &jobs,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![retained.clone()],
            destination: root.path().join("unavailable destination"),
        }),
    );
    assert!(matches!(
        question(&jobs, id).question,
        Question::Transfer(canopa_files::operation_jobs::TransferQuestion::Unavailable { .. })
    ));
    jobs.cancel(id).unwrap();
    wait_for(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Stopped)
    });
    for index in 0..100 {
        let next = submit(
            &jobs,
            Request::Mutation(OperationRequest::CreateFolder {
                path: root.path().join(format!("later-{index}")),
            }),
        );
        finished(&jobs, next);
    }
    assert_eq!(
        fs::read(&retained).expect("the stopped exact plan still needs its source"),
        b"Retained source bytes"
    );
    let mut shutdown = jobs.shutdown().unwrap();
    // Live executable work refuses orderly shutdown. Dropping this idle owner
    // models owner loss; reopening must preserve its saved report without replay.
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    lose_idle_owner(jobs, &root.path().join("state/history"));
    let jobs = service(root.path());
    assert_eq!(
        jobs.snapshot()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .phase,
        Phase::Stopped
    );
    assert_eq!(fs::read(&retained).unwrap(), b"Retained source bytes");
    let mut shutdown = jobs.shutdown().unwrap();
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    lose_idle_owner(jobs, &history);
    let engine = OperationEngine::open(&history).unwrap();
    let record = engine
        .request_page(Some(id + 1))
        .unwrap()
        .into_iter()
        .find(|record| record.id() == id)
        .unwrap();
    engine.dismiss_request(&record).unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("after dismissal"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!retained.exists());
    assert_eq!(
        fs::read(destination.join("source.txt")).unwrap(),
        b"New destination bytes"
    );
}

#[test]
fn clipboard_source_leasing_stays_available_during_undo_review_and_hands_off_to_paste() {
    use canopa_files::{Cancellation, operations::OperationEngine, transfers::TransferEvent};
    let root = tempfile::tempdir().unwrap();
    let incoming = root.path().join("note.txt");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&incoming, b"Current destination bytes").unwrap();
    fs::write(destination.join("note.txt"), b"Clipboard retained bytes").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![incoming],
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
    let storage = Storage::load(root.path().join("state/volumes")).unwrap();
    let jobs =
        OperationJobs::start_with_storage(root.path().join("state/history"), storage.clone())
            .unwrap();
    wait_for(|| (!jobs.snapshot().initializing).then_some(()));
    let nonempty = root.path().join("review folder");
    let create = submit(
        &jobs,
        Request::Mutation(OperationRequest::CreateFolder {
            path: nonempty.clone(),
        }),
    );
    finished(&jobs, create);
    fs::write(nonempty.join("later.txt"), b"Later work").unwrap();
    let undo = submit(
        &jobs,
        Request::Undo {
            expected: jobs.snapshot().undo.unwrap().id,
        },
    );
    let review = question(&jobs, undo);
    let count = jobs.snapshot().jobs.len();
    let mut prepared = jobs.lease_sources(vec![saved.clone()]).unwrap();
    // Always answer the live question before asserting the deadline, so a
    // regression cannot leave the test waiting behind the mutation worker.
    let deadline = Instant::now() + Duration::from_secs(3);
    let lease = loop {
        if let Some(result) = prepared.try_recv().unwrap() {
            break Some(result);
        }
        if Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    jobs.resolve(undo, review.generation, UndoChoice::KeepItem)
        .unwrap();
    let lease = lease
        .expect("clipboard preparation blocked on Undo review")
        .unwrap();
    finished(&jobs, undo);
    assert_eq!(
        jobs.snapshot().jobs.len(),
        count,
        "a clipboard lease is not an operation"
    );
    for index in 0..100 {
        let next = submit(
            &jobs,
            Request::Mutation(OperationRequest::CreateFolder {
                path: root.path().join(format!("clipboard-history-{index}")),
            }),
        );
        finished(&jobs, next);
    }
    assert_eq!(fs::read(&saved).unwrap(), b"Clipboard retained bytes");
    let mut accepted = jobs
        .submit_transfer_with_lease(
            TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![saved.clone()],
                destination: root.path().join("Paste destination not ready"),
            },
            storage.snapshot(),
            None,
            lease,
        )
        .unwrap();
    let paste = wait_for(|| accepted.try_recv().unwrap()).unwrap();
    question(&jobs, paste);
    jobs.cancel(paste).unwrap();
    wait_for(|| {
        jobs.snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == paste && job.phase == Phase::Stopped)
    });
    // Clipboard ownership has gone. The durable stopped plan still owns its
    // source independently, including across process-local worker shutdown.
    let mut shutdown = jobs.shutdown().unwrap();
    // Live executable work refuses orderly shutdown. Dropping this idle owner
    // models owner loss; reopening must preserve its saved report without replay.
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    lose_idle_owner(jobs, &root.path().join("state/history"));
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    assert_eq!(fs::read(&saved).unwrap(), b"Clipboard retained bytes");
    let record = engine
        .request_page(Some(paste + 1))
        .unwrap()
        .into_iter()
        .find(|row| row.id() == paste)
        .unwrap();
    engine.dismiss_request(&record).unwrap();
    engine
        .execute(
            OperationRequest::CreateFolder {
                path: root.path().join("Paste dismissed"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!saved.exists());
    assert_eq!(
        fs::read(destination.join("note.txt")).unwrap(),
        b"Current destination bytes"
    );
}

#[test]
fn cancellation_at_engine_entry_preserves_exact_remaining_intent_without_uncertain_changes() {
    use canopa_files::{
        Cancellation, operations::OperationEngine, transfers::TransferContinuation,
    };
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let source = root.path().join("source.txt");
    fs::write(&source, b"Not started yet").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    cancel.cancel();
    let stopped = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &cancel,
            |_| panic!("cancelled work must not ask a question"),
        )
        .unwrap();
    assert!(stopped.cancelled);
    assert_eq!(stopped.completed, 0);
    assert_eq!(stopped.id, None);
    assert_eq!(stopped.remaining, vec![source.clone()]);
    let plan = serde_json::to_vec(&stopped.continuation.unwrap()).unwrap();
    let restored: TransferContinuation = serde_json::from_slice(&plan).unwrap();
    let stopped = engine
        .resume_transfer(&restored, &cancel, |_| {
            panic!("cancelled continuation must not run")
        })
        .unwrap();
    assert!(stopped.cancelled);
    assert_eq!(stopped.completed, 0);
    assert_eq!(
        serde_json::to_vec(&stopped.continuation.unwrap()).unwrap(),
        plan
    );
    assert!(engine.latest_undo_id().is_none());
    assert!(!destination.join("source.txt").exists());
    let resumed = engine
        .resume_transfer(&restored, &Cancellation::default(), |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert_eq!(resumed.completed, 1);
    assert_eq!(
        fs::read(destination.join("source.txt")).unwrap(),
        b"Not started yet"
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!destination.join("source.txt").exists());
    assert_eq!(fs::read(source).unwrap(), b"Not started yet");
}

#[test]
fn cancelling_just_dispatched_transfers_keeps_the_serial_queue_usable() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.bin");
    fs::File::create(&source)
        .unwrap()
        .set_len(512 * 1024)
        .unwrap();
    let service = service(root.path());
    for index in 0..16 {
        let destination = root.path().join(format!("destination-{index}"));
        fs::create_dir(&destination).unwrap();
        let mut reply = service
            .submit(
                Request::Transfer(TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![source.clone()],
                    destination,
                }),
                None,
            )
            .unwrap();
        // Observe admission promptly to exercise Cancel racing dispatch, rather
        // than deliberately waiting for a copy chunk or a conflict question.
        let deadline = Instant::now() + Duration::from_secs(15);
        let id = loop {
            if let Some(reply) = reply.try_recv().unwrap() {
                break reply.unwrap();
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        service.cancel(id).unwrap();
        let stopped = wait_for(|| {
            service
                .snapshot()
                .jobs
                .into_iter()
                .find(|job| job.id == id && !job.phase.holds_application())
        });
        assert!(
            matches!(stopped.phase, Phase::Finished | Phase::Stopped),
            "cancellation became uncertain: {:?}: {:?}",
            stopped.phase,
            stopped.error
        );
        let marker = root.path().join(format!("still usable-{index}"));
        let next = submit(
            &service,
            Request::Mutation(OperationRequest::CreateFolder {
                path: marker.clone(),
            }),
        );
        finished(&service, next);
        assert!(marker.is_dir());
    }
    assert_eq!(fs::metadata(source).unwrap().len(), 512 * 1024);
}

#[test]
fn a_folder_only_confirmed_prefix_keeps_its_operation_id_through_a_fatal_journal_error_and_restart()
{
    interrupted_folder_prefix(false);
}

#[test]
fn an_interrupted_report_save_failure_keeps_the_accepted_task_and_retries_without_file_work() {
    interrupted_folder_prefix(true);
}

fn interrupted_folder_prefix(refuse_report: bool) {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let folder = root.path().join("a-empty folder");
    fs::create_dir(&folder).unwrap();
    let source = root.path().join("z-conflict.txt");
    fs::write(&source, b"Incoming file").unwrap();
    fs::write(destination.join("z-conflict.txt"), b"Previous file").unwrap();
    let service = service(root.path());
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![folder, source.clone()],
            destination: destination.clone(),
        }),
    );
    let decision = question(&service, id);
    let created = destination.join("a-empty folder");
    let inode = fs::metadata(&created).unwrap().ino();
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    let accepted: (u64, Vec<u8>) = database
        .query_row(
            "SELECT revision,body FROM requests WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    database.execute_batch("CREATE TRIGGER refuse_journal_clear BEFORE DELETE ON pending BEGIN SELECT RAISE(FAIL, 'fixture journal cannot settle'); END;").unwrap();
    if refuse_report {
        database.execute_batch("CREATE TRIGGER refuse_interruption BEFORE INSERT ON request_transfer_interruptions BEGIN SELECT RAISE(ABORT, 'fixture interruption report cannot save'); END;").unwrap();
    }
    service
        .resolve_transfer(
            id,
            decision.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    let stopped = || {
        service.snapshot().jobs.into_iter().find(|job| {
            job.id == id && (!job.phase.holds_application() || job.phase == Phase::SaveFailed)
        })
    };
    let mut interrupted = wait_for(stopped);
    if refuse_report {
        assert_eq!(interrupted.phase, Phase::SaveFailed);
        assert!(
            interrupted
                .error
                .as_ref()
                .unwrap()
                .contains("fixture interruption report cannot save")
        );
        assert_eq!(
            database
                .query_row(
                    "SELECT count(*) FROM request_transfer_interruptions",
                    [],
                    |r| r.get::<_, u64>(0)
                )
                .unwrap(),
            0
        );
        let before_retry = fs::read(destination.join("z-conflict.txt")).unwrap();
        database
            .execute_batch("DROP TRIGGER refuse_interruption;")
            .unwrap();
        service.retry_save(id).unwrap();
        interrupted = wait_for(|| stopped().filter(|j| j.phase != Phase::SaveFailed));
        assert_eq!(
            fs::read(destination.join("z-conflict.txt")).unwrap(),
            before_retry
        );
    }
    assert_eq!(interrupted.phase, Phase::Interrupted);
    let current: (u64, Vec<u8>) = database
        .query_row(
            "SELECT revision,body FROM requests WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        current, accepted,
        "descriptive result replaced the accepted task"
    );
    let report = interrupted.transfer.unwrap();
    assert_eq!(report.completed, 1);
    assert!(
        report.operation.is_some(),
        "confirmed folder lost its shared history association"
    );
    assert!(
        interrupted
            .error
            .unwrap()
            .contains("fixture journal cannot settle")
    );
    let uncertain_destination = fs::read(destination.join("z-conflict.txt")).unwrap();
    database
        .execute_batch("DROP TRIGGER refuse_journal_clear;")
        .unwrap();
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
    drop(service);
    for _ in 0..2 {
        let reopened = self::service(root.path());
        let interrupted = reopened
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id)
            .unwrap();
        assert_eq!(interrupted.phase, Phase::Interrupted);
        let restored_report = interrupted.transfer.unwrap();
        assert_eq!(restored_report.operation, report.operation);
        assert_eq!(restored_report.completed, 1);
        assert!(
            !interrupted.transfer_actions.retry
                && !interrupted.transfer_actions.remaining
                && !interrupted.transfer_actions.abandon
        );
        assert_eq!(fs::metadata(&created).unwrap().ino(), inode);
        assert_eq!(
            fs::read(destination.join("z-conflict.txt")).unwrap(),
            uncertain_destination
        );
        assert_eq!(fs::read(&source).unwrap(), b"Incoming file");
        let current: (u64, Vec<u8>) = database
            .query_row(
                "SELECT revision,body FROM requests WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(current, accepted);
        let mut shutdown = reopened.shutdown().unwrap();
        wait_for(|| shutdown.try_recv().unwrap()).unwrap();
        drop(reopened);
    }
    let reopened = self::service(root.path());
    let review = submit(&reopened, Request::Reconcile);
    finished(&reopened, review);
    let result = wait_for(|| {
        reopened
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase != Phase::Interrupted)
    });
    assert_eq!(result.phase, Phase::Finished, "{result:?}");
    assert_eq!(result.transfer.unwrap().completed, 2);
    assert_eq!(fs::metadata(&created).unwrap().ino(), inode);
    assert_eq!(
        fs::read(destination.join("z-conflict.txt")).unwrap(),
        uncertain_destination
    );
    assert_eq!(fs::read(&source).unwrap(), b"Incoming file");
    assert_eq!(
        database
            .query_row(
                "SELECT count(*) FROM request_transfer_interruptions WHERE request=?1",
                [id],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn corrupt_transfer_records_are_rejected_without_changing_their_saved_bytes_or_files() {
    use canopa_files::operations::{OperationEngine, requests::RequestPhase};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"Original source").unwrap();
    fs::write(destination.join("source.txt"), b"Original destination").unwrap();
    let service = service(root.path());
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![source.clone()],
            destination: destination.clone(),
        }),
    );
    question(&service, id);
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    let body: Vec<u8> = database
        .query_row("SELECT body FROM requests WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    let plan: serde_json::Value = serde_json::from_slice(&body).unwrap();
    service.cancel(id).unwrap();
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Stopped)
    });
    let mut shutdown = service.shutdown().unwrap();
    // Live executable work refuses orderly shutdown. Dropping this idle owner
    // models owner loss; reopening must preserve its saved report without replay.
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    lose_idle_owner(service, &root.path().join("state/history"));
    drop(database);
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let stopped: serde_json::Value = engine.request_page(None).unwrap()[0].payload().unwrap();
    drop(engine);
    let mut missing_sources = plan.clone();
    missing_sources["Plan"]
        .as_object_mut()
        .unwrap()
        .remove("source_requirements");
    let mut null_sources = plan.clone();
    null_sources["Plan"]["source_requirements"] = serde_json::Value::Null;
    let mut no_sources = plan.clone();
    no_sources["Plan"]["request"]["Transfer"]["sources"] = serde_json::json!([]);
    let mut no_plans = stopped.clone();
    no_plans["TransferPending"]["outcome"]["remaining"] = serde_json::Value::Null;
    let mut missing_remaining = stopped.clone();
    missing_remaining["TransferPending"]["outcome"]["report"]["remaining"] = serde_json::json!([]);
    let mut wrong_kind = stopped.clone();
    wrong_kind["TransferPending"]["outcome"]["report"]["kind"] = serde_json::json!("Move");
    let mut relative_report = stopped.clone();
    relative_report["TransferPending"]["outcome"]["report"]["destination"] =
        serde_json::json!([114, 101, 108, 97, 116, 105, 118, 101]);
    let mut invented_retry = stopped.clone();
    invented_retry["TransferPending"]["outcome"]["retry"] =
        stopped["TransferPending"]["outcome"]["remaining"].clone();
    let mut unproven_failure = plan.clone();
    unproven_failure["Plan"]["error"] = serde_json::json!("ambiguous original failure");
    let mut false_rejection = plan.clone();
    false_rejection["Plan"]["rejected_before_execution"] = serde_json::json!(true);
    for (name, payload, phase) in [
        (
            "missing source requirements",
            missing_sources,
            RequestPhase::Queued,
        ),
        (
            "null source requirements",
            null_sources,
            RequestPhase::Running,
        ),
        ("empty selection", no_sources, RequestPhase::Queued),
        ("stopped without plans", no_plans, RequestPhase::Finished),
        (
            "missing reported remainder",
            missing_remaining,
            RequestPhase::Remaining,
        ),
        (
            "mismatched transfer kind",
            wrong_kind,
            RequestPhase::Remaining,
        ),
        (
            "relative report destination",
            relative_report,
            RequestPhase::Remaining,
        ),
        (
            "retry without failures",
            invented_retry,
            RequestPhase::FailedWithRemaining,
        ),
        (
            "unproven failed original",
            unproven_failure,
            RequestPhase::Failed,
        ),
        (
            "rejection without an error",
            false_rejection,
            RequestPhase::Queued,
        ),
    ] {
        let fixture = root.path().join(name);
        let history = fixture.join("state/history");
        let engine = OperationEngine::open(history.clone()).unwrap();
        let record = engine.queue_request(&payload).unwrap();
        let record = engine.checkpoint_request(&record, phase, &payload).unwrap();
        let expected = serde_json::to_vec(&payload).unwrap();
        drop(engine);
        let service = OperationJobs::start(history.clone(), fixture.join("state/volumes")).unwrap();
        let state = wait_for(|| {
            let state = service.snapshot();
            (!state.initializing).then_some(state)
        });
        assert!(state.error.is_some(), "accepted corrupt record: {name}");
        assert!(!state.holds_application());
        drop(service);
        let database = rusqlite::Connection::open(history).unwrap();
        let (actual_phase, actual): (u8, Vec<u8>) = database
            .query_row(
                "SELECT phase,body FROM requests WHERE id=?1",
                [record.id()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(actual_phase, phase as u8, "rewrote corrupt phase: {name}");
        assert_eq!(actual, expected, "rewrote corrupt payload: {name}");
    }
    assert_eq!(fs::read(source).unwrap(), b"Original source");
    assert_eq!(
        fs::read(destination.join("source.txt")).unwrap(),
        b"Original destination"
    );
}

#[test]
fn an_empty_transfer_selection_is_refused_before_durable_admission() {
    let root = tempfile::tempdir().unwrap();
    let service = service(root.path());
    let mut reply = service
        .submit(
            Request::Transfer(TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![],
                destination: root.path().to_owned(),
            }),
            None,
        )
        .unwrap();
    assert!(wait_for(|| reply.try_recv().unwrap()).is_err());
    assert!(service.snapshot().jobs.is_empty());
    assert!(service.snapshot().undo.is_none());
}

#[test]
fn a_finalization_failure_retains_the_confirmed_prefix_and_never_saves_an_executable_original_batch()
 {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let a = root.path().join("a-completed");
    let z = root.path().join("z-conflict");
    fs::write(&a, b"Completed prefix").unwrap();
    fs::write(&z, b"Uncopied source").unwrap();
    fs::write(destination.join("z-conflict"), b"Keep destination").unwrap();
    let service = service(root.path());
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![a.clone(), z.clone()],
            destination: destination.clone(),
        }),
    );
    question(&service, id);
    let inode = fs::metadata(destination.join("a-completed")).unwrap().ino();
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    let accepted: (u64, Vec<u8>) = database
        .query_row(
            "SELECT revision,body FROM requests WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    // Empty operation rows are pruned only at final history trimming, after the
    // copied prefix and the cancelled continuation have already been recorded.
    database.execute_batch("INSERT INTO operations(label) VALUES('fixture trim barrier');
        CREATE TRIGGER refuse_transfer_trim BEFORE DELETE ON operations
        WHEN OLD.label='fixture trim barrier' BEGIN SELECT RAISE(FAIL, 'fixture final trim failed'); END;").unwrap();
    service.cancel(id).unwrap();
    let stopped = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && !job.phase.holds_application())
    });
    assert_eq!(
        stopped.phase,
        Phase::Interrupted,
        "an uncertain transfer must require review"
    );
    assert!(
        stopped
            .error
            .as_deref()
            .unwrap()
            .contains("fixture final trim failed")
    );
    let report = stopped.transfer.unwrap();
    assert_eq!(report.completed, 1);
    assert_eq!(report.remaining, vec![z.clone()]);
    assert_eq!(report.destinations, vec![destination.join("a-completed")]);
    assert_eq!(
        report.operation,
        service.snapshot().undo.map(|head| head.id)
    );
    database.execute_batch("DROP TRIGGER refuse_transfer_trim; DELETE FROM operations WHERE label='fixture trim barrier';").unwrap();
    let refused = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![z.clone()],
            destination: destination.clone(),
        }),
    );
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == refused && job.phase == Phase::Failed)
    });
    let marker = root.path().join("must wait for reconciliation");
    let later = submit(
        &service,
        Request::Mutation(OperationRequest::CreateFolder {
            path: marker.clone(),
        }),
    );
    wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == later && job.phase == Phase::Failed)
    });
    assert!(!marker.exists());
    let mut shutdown = service.shutdown().unwrap();
    // Live executable work refuses orderly shutdown. Dropping this idle owner
    // models owner loss; reopening must preserve its saved report without replay.
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    lose_idle_owner(service, &root.path().join("state/history"));
    let engine =
        canopa_files::operations::OperationEngine::open(root.path().join("state/history")).unwrap();
    let saved = engine
        .request_page(None)
        .unwrap()
        .into_iter()
        .find(|record| record.id() == id)
        .unwrap();
    assert_eq!(
        saved.phase(),
        canopa_files::operations::requests::RequestPhase::Running
    );
    let payload: serde_json::Value = saved.payload().unwrap();
    assert!(
        payload.get("Plan").is_some(),
        "the accepted task must remain intact"
    );
    let current: (u64, Vec<u8>) = database
        .query_row(
            "SELECT revision,body FROM requests WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(current, accepted);
    let description: String = database
        .query_row(
            "SELECT body FROM request_transfer_interruptions WHERE request=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    let description: serde_json::Value = serde_json::from_str(&description).unwrap();
    assert!(description["interrupted"].is_string());
    assert!(
        description["remaining"].is_object(),
        "known exact continuation was discarded"
    );
    drop(engine);
    fs::write(&a, b"Newer source edit").unwrap();
    let reopened = self::service(root.path());
    let stopped = reopened
        .snapshot()
        .jobs
        .into_iter()
        .find(|job| job.id == id)
        .unwrap();
    assert_eq!(stopped.phase, Phase::Interrupted);
    assert!(
        !stopped.transfer_actions.retry
            && !stopped.transfer_actions.remaining
            && !stopped.transfer_actions.abandon
    );
    assert_eq!(
        reopened
            .snapshot()
            .jobs
            .iter()
            .find(|job| job.id == refused)
            .unwrap()
            .phase,
        Phase::Failed
    );
    assert_eq!(stopped.transfer.unwrap().completed, 1);
    assert_eq!(
        fs::metadata(destination.join("a-completed")).unwrap().ino(),
        inode
    );
    assert_eq!(
        fs::read(destination.join("a-completed")).unwrap(),
        b"Completed prefix"
    );
    assert_eq!(
        fs::read(destination.join("z-conflict")).unwrap(),
        b"Keep destination"
    );
    assert_eq!(fs::read(&a).unwrap(), b"Newer source edit");
    let review = submit(&reopened, Request::Reconcile);
    finished(&reopened, review);
    let reviewed = reopened
        .snapshot()
        .jobs
        .into_iter()
        .find(|j| j.id == id)
        .unwrap();
    assert_eq!(reviewed.phase, Phase::Stopped, "{reviewed:?}");
    assert!(reviewed.transfer_actions.remaining);
    assert!(!reviewed.transfer_actions.retry);
    assert_eq!(reviewed.transfer.unwrap().completed, 1);
    assert_eq!(fs::read(a).unwrap(), b"Newer source edit");
    assert_eq!(
        fs::metadata(destination.join("a-completed")).unwrap().ino(),
        inode
    );
    assert_eq!(
        fs::read(destination.join("z-conflict")).unwrap(),
        b"Keep destination"
    );
}

#[test]
fn a_forced_cross_filesystem_move_and_shared_undo_preserve_the_actual_contents() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::Builder::new()
        .prefix(".transfer-worker-cross-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let device = fs::metadata(root.path()).unwrap().dev();
    let destination = ["/tmp", "/dev/shm"]
        .into_iter()
        .find_map(|base| {
            tempfile::Builder::new()
                .prefix("transfer-worker-cross-")
                .tempdir_in(base)
                .ok()
                .filter(|folder| {
                    fs::metadata(folder.path()).is_ok_and(|metadata| metadata.dev() != device)
                })
        })
        .expect("a writable second filesystem is required for the transfer-worker fixture");
    let other_device = fs::metadata(destination.path()).unwrap().dev();
    assert_ne!(device, other_device);
    eprintln!("cross-filesystem worker test used devices {device} and {other_device}");
    let source = root.path().join("photo.bin");
    fs::write(&source, b"Move me safely").unwrap();
    let service = service(root.path());
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Move,
            sources: vec![source.clone()],
            destination: destination.path().to_owned(),
        }),
    );
    finished(&service, id);
    assert!(!source.exists());
    assert_eq!(
        fs::read(destination.path().join("photo.bin")).unwrap(),
        b"Move me safely"
    );
    let undo = submit(
        &service,
        Request::Undo {
            expected: service.snapshot().undo.unwrap().id,
        },
    );
    finished(&service, undo);
    assert_eq!(fs::read(source).unwrap(), b"Move me safely");
    assert!(!destination.path().join("photo.bin").exists());
    let mut shutdown = service.shutdown().unwrap();
    wait_for(|| shutdown.try_recv().unwrap()).unwrap();
}

#[test]
fn progress_is_observable_while_copying_and_cancellation_cleans_the_uncommitted_file() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("large.bin");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let size = 256 * 1024 * 1024;
    fs::File::create(&source).unwrap().set_len(size).unwrap();
    let service = service(root.path());
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![source.clone()],
            destination: destination.clone(),
        }),
    );
    let progress = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Running)
            .and_then(|job| job.progress)
            .filter(|progress| progress.bytes > 0 && progress.bytes < size)
    });
    assert_eq!(progress.path, source);
    assert_eq!(progress.completed, 0);
    service.cancel(id).unwrap();
    let stopped = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && !job.phase.holds_application())
    });
    assert_eq!(stopped.phase, Phase::Stopped, "{stopped:?}");
    assert!(stopped.progress.is_none());
    let report = stopped.transfer.unwrap();
    assert!(report.cancelled);
    assert_eq!(report.completed, 0);
    assert_eq!(report.remaining, vec![source.clone()]);
    assert_eq!(fs::metadata(source).unwrap().len(), size);
    assert!(
        fs::read_dir(destination).unwrap().next().is_none(),
        "unfinished stage or destination survived cancellation"
    );
}

#[test]
fn an_unavailable_folder_waits_for_its_typed_retry_and_then_copies_to_the_reviewed_location() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("missing folder");
    let source = root.path().join("item.txt");
    fs::write(&source, b"Wait for the destination").unwrap();
    let service = service(root.path());
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![source.clone()],
            destination: destination.clone(),
        }),
    );
    let decision = question(&service, id);
    assert!(matches!(
        decision.question,
        Question::Transfer(canopa_files::operation_jobs::TransferQuestion::Unavailable { .. })
    ));
    service
        .resolve_transfer(
            id,
            decision.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::KeepBoth,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    let marker = root.path().join("queued while unavailable");
    let queued = submit(
        &service,
        Request::Mutation(OperationRequest::CreateFolder {
            path: marker.clone(),
        }),
    );
    assert!(
        service
            .snapshot()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .decision
            .is_some()
    );
    assert!(!marker.exists());
    fs::create_dir(&destination).unwrap();
    service
        .resolve_transfer(id, decision.generation, TransferReply::Retry)
        .unwrap();
    finished(&service, id);
    finished(&service, queued);
    assert_eq!(
        fs::read(destination.join("item.txt")).unwrap(),
        b"Wait for the destination"
    );
    assert_eq!(fs::read(source).unwrap(), b"Wait for the destination");
}

#[test]
fn retry_preserves_observed_destination_identity_when_its_path_is_reused() {
    use canopa_files::operation_jobs::TransferQuestion;
    use std::os::unix::fs::MetadataExt;
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        let parked = root.path().join("original destination");
        fs::create_dir(&destination).unwrap();
        let original = fs::metadata(&destination).unwrap().ino();
        let a = root.path().join("a");
        let b = root.path().join("b");
        let c = root.path().join("c");
        fs::write(&a, b"confirmed prefix").unwrap();
        fs::write(&b, b"skipped source").unwrap();
        fs::write(&c, b"unstarted source").unwrap();
        fs::write(destination.join("b"), b"existing destination").unwrap();
        let jobs = service(root.path());
        let id = submit(
            &jobs,
            Request::Transfer(TransferRequest {
                kind,
                sources: vec![a.clone(), b.clone(), c.clone()],
                destination: destination.clone(),
            }),
        );
        let conflict = question(&jobs, id);
        assert!(matches!(
            &conflict.question,
            Question::Transfer(TransferQuestion::Conflict { source, .. }) if source.path == b
        ));
        let prefix = fs::metadata(destination.join("a")).unwrap().ino();
        fs::rename(&destination, &parked).unwrap();
        jobs.resolve_transfer(
            id,
            conflict.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::Skip,
                apply_to_remaining: false,
            },
        )
        .unwrap();
        let next_unavailable = |after| {
            wait_for(|| {
                let job = jobs
                    .snapshot()
                    .jobs
                    .into_iter()
                    .find(|job| job.id == id)
                    .unwrap();
                assert_ne!(
                    job.phase,
                    Phase::Finished,
                    "Retry completed before the observed destination returned"
                );
                job.decision
                    .filter(|decision| decision.generation > after)
                    .inspect(|decision| {
                        assert!(matches!(
                            &decision.question,
                            Question::Transfer(TransferQuestion::Unavailable { .. })
                        ));
                    })
            })
        };
        let missing = next_unavailable(conflict.generation);
        jobs.resolve_transfer(id, missing.generation, TransferReply::Retry)
            .unwrap();
        let still_missing = next_unavailable(missing.generation);
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("unrelated.txt"), b"unrelated folder").unwrap();
        assert_ne!(fs::metadata(&destination).unwrap().ino(), original);
        jobs.resolve_transfer(id, still_missing.generation, TransferReply::Retry)
            .unwrap();
        let replacement = next_unavailable(still_missing.generation);
        assert!(!destination.join("c").exists());
        assert_eq!(
            fs::read(destination.join("unrelated.txt")).unwrap(),
            b"unrelated folder"
        );
        assert_eq!(fs::metadata(parked.join("a")).unwrap().ino(), prefix);
        assert_eq!(fs::read(&c).unwrap(), b"unstarted source");
        fs::remove_file(destination.join("unrelated.txt")).unwrap();
        fs::remove_dir(&destination).unwrap();
        fs::rename(&parked, &destination).unwrap();
        jobs.resolve_transfer(id, replacement.generation, TransferReply::Retry)
            .unwrap();
        finished(&jobs, id);
        let report = jobs
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id)
            .unwrap()
            .transfer
            .unwrap();
        assert_eq!(report.completed, 2);
        assert_eq!(report.skipped, std::slice::from_ref(&b));
        assert!(report.failures.is_empty() && report.remaining.is_empty());
        assert_eq!(fs::metadata(&destination).unwrap().ino(), original);
        assert_eq!(fs::metadata(destination.join("a")).unwrap().ino(), prefix);
        assert_eq!(
            fs::read(destination.join("c")).unwrap(),
            b"unstarted source"
        );
        assert_eq!(a.exists(), kind == TransferKind::Copy);
        assert_eq!(c.exists(), kind == TransferKind::Copy);
        let undo = submit(
            &jobs,
            Request::Undo {
                expected: jobs.snapshot().undo.unwrap().id,
            },
        );
        finished(&jobs, undo);
        assert!(!destination.join("a").exists() && !destination.join("c").exists());
        assert_eq!(
            fs::read(destination.join("b")).unwrap(),
            b"existing destination"
        );
        assert_eq!(fs::metadata(&destination).unwrap().ino(), original);
        assert_eq!(fs::read(&a).unwrap(), b"confirmed prefix");
        assert_eq!(fs::read(&b).unwrap(), b"skipped source");
        assert_eq!(fs::read(&c).unwrap(), b"unstarted source");
    }
}

#[test]
fn retry_save_persists_the_actual_partial_transfer_instead_of_replaying_its_original_request() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let a = root.path().join("a-completed");
    let z = root.path().join("z-conflict");
    fs::write(&a, b"Completed prefix").unwrap();
    fs::write(&z, b"Uncopied source").unwrap();
    fs::write(destination.join("z-conflict"), b"Keep destination").unwrap();
    let service = service(root.path());
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![a, z.clone()],
            destination: destination.clone(),
        }),
    );
    question(&service, id);
    let inode = fs::metadata(destination.join("a-completed")).unwrap().ino();
    let database = rusqlite::Connection::open(root.path().join("state/history")).unwrap();
    database.execute_batch(&format!("CREATE TRIGGER refuse_transfer_result BEFORE UPDATE ON requests WHEN NEW.id={id} AND NEW.phase<>1 BEGIN SELECT RAISE(FAIL, 'fixture cannot save stopped transfer'); END;")).unwrap();
    service.cancel(id).unwrap();
    let unsaved = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::SaveFailed)
    });
    assert_eq!(unsaved.transfer.unwrap().completed, 1);
    assert!(service.snapshot().holds_application());
    database
        .execute_batch("DROP TRIGGER refuse_transfer_result;")
        .unwrap();
    service.retry_save(id).unwrap();
    let saved = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Stopped)
    });
    assert_eq!(saved.transfer.unwrap().remaining, vec![z]);
    assert_eq!(
        fs::metadata(destination.join("a-completed")).unwrap().ino(),
        inode
    );
    assert_eq!(
        fs::read(destination.join("z-conflict")).unwrap(),
        b"Keep destination"
    );
    let mut shutdown = service.shutdown().unwrap();
    // Live executable work refuses orderly shutdown. Dropping this idle owner
    // models owner loss; reopening must preserve its saved report without replay.
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    lose_idle_owner(service, &root.path().join("state/history"));
    let service = self::service(root.path());
    assert_eq!(
        service
            .snapshot()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .phase,
        Phase::Stopped
    );
    assert_eq!(
        fs::metadata(destination.join("a-completed")).unwrap().ino(),
        inode
    );
    assert_eq!(
        fs::read(destination.join("z-conflict")).unwrap(),
        b"Keep destination"
    );
}

#[test]
fn folder_merge_and_file_conflicts_keep_distinct_questions_and_restore_previous_contents_on_undo() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("Photos");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir_all(destination.join("Photos")).unwrap();
    fs::write(source.join("a.jpg"), b"Incoming image").unwrap();
    fs::write(source.join("b.jpg"), b"Added image").unwrap();
    fs::write(destination.join("Photos/a.jpg"), b"Existing image").unwrap();
    fs::write(destination.join("Photos/old.jpg"), b"Unrelated image").unwrap();
    let service = service(root.path());
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![source.clone()],
            destination: destination.clone(),
        }),
    );
    let folder = question(&service, id);
    let Question::Transfer(ref capabilities) = folder.question else {
        panic!("wrong question kind")
    };
    assert!(capabilities.can_merge());
    assert!(!capabilities.can_replace());
    service
        .resolve_transfer(
            id,
            folder.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::Merge,
                apply_to_remaining: true,
            },
        )
        .unwrap();
    let file = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id)
            .and_then(|job| job.decision)
            .filter(|decision| decision.generation > folder.generation)
    });
    let Question::Transfer(ref capabilities) = file.question else {
        panic!("wrong question kind")
    };
    assert!(!capabilities.can_merge());
    assert!(capabilities.can_replace());
    service
        .resolve_transfer(id, folder.generation, TransferReply::Cancel)
        .unwrap();
    let marker = root.path().join("after Merge");
    let queued = submit(
        &service,
        Request::Mutation(OperationRequest::CreateFolder {
            path: marker.clone(),
        }),
    );
    assert!(!marker.exists());
    assert_eq!(
        service
            .snapshot()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .decision
            .as_ref()
            .unwrap()
            .generation,
        file.generation
    );
    service
        .resolve_transfer(
            id,
            file.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::KeepBoth,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    finished(&service, id);
    finished(&service, queued);
    assert_eq!(
        fs::read(destination.join("Photos/a.jpg")).unwrap(),
        b"Existing image"
    );
    assert_eq!(
        fs::read(destination.join("Photos/a (2).jpg")).unwrap(),
        b"Incoming image"
    );
    assert_eq!(
        fs::read(destination.join("Photos/b.jpg")).unwrap(),
        b"Added image"
    );
    let undo = submit(
        &service,
        Request::Undo {
            expected: service.snapshot().undo.unwrap().id,
        },
    );
    finished(&service, undo);
    let undo = submit(
        &service,
        Request::Undo {
            expected: service.snapshot().undo.unwrap().id,
        },
    );
    finished(&service, undo);
    assert!(!destination.join("Photos/a (2).jpg").exists());
    assert!(!destination.join("Photos/b.jpg").exists());
    assert_eq!(
        fs::read(destination.join("Photos/a.jpg")).unwrap(),
        b"Existing image"
    );
    assert_eq!(
        fs::read(destination.join("Photos/old.jpg")).unwrap(),
        b"Unrelated image"
    );
    assert_eq!(fs::read(source.join("a.jpg")).unwrap(), b"Incoming image");
}

#[test]
fn batch_file_choices_do_not_answer_mixed_item_conflicts_and_both_groups_remain_undoable() {
    use canopa_files::operation_jobs::TransferQuestion;
    use std::os::unix::fs::MetadataExt;
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::create_dir_all(source.join("b-folder")).unwrap();
        fs::create_dir_all(destination.join("c-file")).unwrap();
        for name in ["a", "c-file", "d"] {
            fs::write(source.join(name), format!("Incoming {name}")).unwrap();
        }
        fs::write(source.join("b-folder/child"), b"Incoming folder child").unwrap();
        for name in ["a", "b-folder", "d"] {
            fs::write(destination.join(name), format!("Previous {name}")).unwrap();
        }
        fs::write(destination.join("c-file/kept"), b"Existing folder child").unwrap();
        let source_ids: Vec<_> = ["a", "b-folder", "b-folder/child", "c-file", "d"]
            .into_iter()
            .map(|name| (name, fs::metadata(source.join(name)).unwrap().ino()))
            .collect();
        let destination_ids: Vec<_> = ["a", "b-folder", "c-file", "c-file/kept", "d"]
            .into_iter()
            .map(|name| (name, fs::metadata(destination.join(name)).unwrap().ino()))
            .collect();
        let jobs = service(root.path());
        let id = submit(
            &jobs,
            Request::Transfer(TransferRequest {
                kind,
                sources: ["a", "b-folder", "c-file", "d"]
                    .into_iter()
                    .map(|name| source.join(name))
                    .collect(),
                destination: destination.clone(),
            }),
        );
        let first = question(&jobs, id);
        let Question::Transfer(ref ordinary) = first.question else {
            panic!("wrong question kind")
        };
        assert!(ordinary.can_replace() && !ordinary.can_merge());
        jobs.resolve_transfer(
            id,
            first.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: true,
            },
        )
        .unwrap();
        let mixed = wait_for(|| {
            let job = jobs
                .snapshot()
                .jobs
                .into_iter()
                .find(|job| job.id == id)
                .unwrap();
            assert!(
                !matches!(job.phase, Phase::Finished | Phase::Failed | Phase::Stopped),
                "mixed collision finished without review: {:?}",
                job.detail
            );
            job.decision
                .filter(|decision| decision.generation > first.generation)
        });
        let Question::Transfer(ref capabilities) = mixed.question else {
            panic!("wrong question kind")
        };
        assert!(!capabilities.can_replace() && !capabilities.can_merge());
        assert!(
            matches!(capabilities, TransferQuestion::Conflict { source: entry, .. }
            if entry.path == source.join("b-folder"))
        );
        assert_eq!(fs::read(destination.join("a")).unwrap(), b"Incoming a");
        assert_eq!(
            fs::read(destination.join("b-folder")).unwrap(),
            b"Previous b-folder"
        );
        assert_eq!(fs::read(source.join("c-file")).unwrap(), b"Incoming c-file");
        jobs.resolve_transfer(
            id,
            mixed.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::KeepBoth,
                apply_to_remaining: true,
            },
        )
        .unwrap();
        finished(&jobs, id);
        let report = jobs
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id)
            .unwrap()
            .transfer
            .unwrap();
        assert!(report.failures.is_empty() && report.remaining.is_empty());
        assert!(report.skipped.is_empty());
        for name in ["a", "d"] {
            assert_eq!(
                fs::read(destination.join(name)).unwrap(),
                format!("Incoming {name}").as_bytes()
            );
        }
        assert_eq!(
            fs::read(destination.join("b-folder (2)/child")).unwrap(),
            b"Incoming folder child"
        );
        assert_eq!(
            fs::read(destination.join("c-file (2)")).unwrap(),
            b"Incoming c-file"
        );
        for (name, identity) in &source_ids {
            if kind == TransferKind::Copy {
                assert_eq!(fs::metadata(source.join(name)).unwrap().ino(), *identity);
            } else {
                assert!(!source.join(name).exists());
                let moved = match *name {
                    "b-folder" => "b-folder (2)",
                    "b-folder/child" => "b-folder (2)/child",
                    "c-file" => "c-file (2)",
                    name => name,
                };
                assert_eq!(
                    fs::metadata(destination.join(moved)).unwrap().ino(),
                    *identity
                );
            }
        }
        let undo = submit(
            &jobs,
            Request::Undo {
                expected: jobs.snapshot().undo.unwrap().id,
            },
        );
        finished(&jobs, undo);
        assert!(!destination.join("b-folder (2)").exists());
        assert!(!destination.join("c-file (2)").exists());
        for (name, identity) in destination_ids {
            assert_eq!(
                fs::metadata(destination.join(name)).unwrap().ino(),
                identity
            );
        }
        for name in ["a", "b-folder", "d"] {
            assert_eq!(
                fs::read(destination.join(name)).unwrap(),
                format!("Previous {name}").as_bytes()
            );
        }
        assert_eq!(
            fs::read(destination.join("c-file/kept")).unwrap(),
            b"Existing folder child"
        );
        for name in ["a", "c-file", "d"] {
            assert_eq!(
                fs::read(source.join(name)).unwrap(),
                format!("Incoming {name}").as_bytes()
            );
        }
        assert_eq!(
            fs::read(source.join("b-folder/child")).unwrap(),
            b"Incoming folder child"
        );
        if kind == TransferKind::Move {
            for (name, identity) in source_ids {
                assert_eq!(fs::metadata(source.join(name)).unwrap().ino(), identity);
            }
        }
    }
}

#[test]
fn replacing_a_destination_volume_during_a_conflict_preserves_both_versions_and_explains_the_pause()
{
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination volume");
    fs::create_dir(&destination).unwrap();
    let source = root.path().join("item.txt");
    fs::write(&source, b"Incoming source").unwrap();
    fs::write(destination.join("item.txt"), b"Original destination").unwrap();
    let storage = Storage::load(root.path().join("state/volumes")).unwrap();
    let original = storage
        .mounted(destination.clone(), "uuid:original".into())
        .unwrap();
    let epoch = original.epoch;
    storage.prepare(original).unwrap();
    let service =
        OperationJobs::start_with_storage(root.path().join("state/history"), storage.clone())
            .unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Move,
            sources: vec![source.clone()],
            destination: destination.clone(),
        }),
    );
    let decision = question(&service, id);
    storage.removed(&destination, epoch);
    let held = root.path().join("held destination");
    fs::rename(&destination, &held).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("item.txt"), b"Replacement destination").unwrap();
    let replacement = storage
        .mounted(destination.clone(), "uuid:replacement".into())
        .unwrap();
    storage.prepare(replacement).unwrap();
    service
        .resolve_transfer(
            id,
            decision.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    let stopped = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Stopped)
    });
    assert!(
        stopped.error.is_some(),
        "the stopped row lost its placement failure explanation"
    );
    let report = stopped.transfer.unwrap();
    assert!(report.needs_attention);
    assert_eq!(report.completed, 0);
    assert_eq!(report.remaining, vec![source.clone()]);
    assert_eq!(fs::read(source).unwrap(), b"Incoming source");
    assert_eq!(
        fs::read(destination.join("item.txt")).unwrap(),
        b"Replacement destination"
    );
    assert_eq!(
        fs::read(held.join("item.txt")).unwrap(),
        b"Original destination"
    );
}

#[test]
fn captured_source_placement_refuses_replacement_media_without_blocking_other_sources_or_a_later_destination()
 {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source volume");
    let destination = root.path().join("later destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("media.txt"), b"Original volume").unwrap();
    let local = root.path().join("local.txt");
    fs::write(&local, b"Independent source").unwrap();
    let storage = Storage::load(root.path().join("state/volumes")).unwrap();
    let original = storage
        .mounted(source.clone(), "uuid:original".into())
        .unwrap();
    let epoch = original.epoch;
    storage.prepare(original).unwrap();
    let captured = storage.snapshot();
    storage.removed(&source, epoch);
    let held = root.path().join("held original");
    fs::rename(&source, &held).unwrap();
    fs::create_dir(&source).unwrap();
    fs::write(source.join("media.txt"), b"Replacement volume").unwrap();
    let replacement = storage
        .mounted(source.clone(), "uuid:replacement".into())
        .unwrap();
    storage.prepare(replacement).unwrap();
    let later = storage
        .mounted(destination.clone(), "uuid:chosen later".into())
        .unwrap();
    storage.prepare(later).unwrap();
    let service =
        OperationJobs::start_with_storage(root.path().join("state/history"), storage).unwrap();
    wait_for(|| (!service.snapshot().initializing).then_some(()));
    let mut reply = service
        .submit_transfer(
            TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.join("media.txt"), local.clone()],
                destination: destination.clone(),
            },
            captured,
            None,
        )
        .unwrap();
    let id = wait_for(|| reply.try_recv().unwrap()).unwrap();
    let stopped = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Stopped)
    });
    let report = stopped.transfer.unwrap();
    assert_eq!(report.completed, 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].source, source.join("media.txt"));
    assert!(!destination.join("media.txt").exists());
    assert_eq!(
        fs::read(destination.join("local.txt")).unwrap(),
        b"Independent source"
    );
    assert!(!local.exists());
    assert_eq!(
        fs::read(source.join("media.txt")).unwrap(),
        b"Replacement volume"
    );
    assert_eq!(
        fs::read(held.join("media.txt")).unwrap(),
        b"Original volume"
    );
}

#[test]
fn a_transfer_question_keeps_admission_live_and_ignores_wrong_kind_and_stale_answers() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("item.txt");
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(&source, b"Incoming").unwrap();
    fs::write(destination.join("item.txt"), b"Previous destination").unwrap();
    let service = service(root.path());
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![source.clone()],
            destination: destination.clone(),
        }),
    );
    let decision = question(&service, id);
    assert!(matches!(decision.question, Question::Transfer(_)));
    service
        .resolve(id, decision.generation, UndoChoice::Replace)
        .unwrap();
    service
        .resolve_transfer(id, decision.generation + 1, TransferReply::Cancel)
        .unwrap();
    service
        .resolve_transfer(id, decision.generation, TransferReply::Continue)
        .unwrap();
    let queued = root.path().join("created after question");
    let next = submit(
        &service,
        Request::Mutation(OperationRequest::CreateFolder {
            path: queued.clone(),
        }),
    );
    assert!(
        service
            .snapshot()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .decision
            .is_some(),
        "an answer absent from the displayed question released it"
    );
    assert!(!queued.exists());
    assert_eq!(question(&service, id).generation, decision.generation);
    assert_eq!(
        fs::read(destination.join("item.txt")).unwrap(),
        b"Previous destination"
    );
    service
        .resolve_transfer(
            id,
            decision.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    finished(&service, id);
    finished(&service, next);
    assert_eq!(fs::read(destination.join("item.txt")).unwrap(), b"Incoming");
    let undo = submit(
        &service,
        Request::Undo {
            expected: service.snapshot().undo.unwrap().id,
        },
    );
    finished(&service, undo);
    let undo = submit(
        &service,
        Request::Undo {
            expected: service.snapshot().undo.unwrap().id,
        },
    );
    finished(&service, undo);
    assert_eq!(
        fs::read(destination.join("item.txt")).unwrap(),
        b"Previous destination"
    );
    assert_eq!(fs::read(source).unwrap(), b"Incoming");
}

#[test]
fn a_cancelled_transfer_reopens_its_exact_stopped_report_without_replaying_the_batch() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let a = root.path().join("a-completed");
    let z = root.path().join("z-conflict");
    fs::write(&a, b"Completed prefix").unwrap();
    fs::write(&z, b"Uncopied source").unwrap();
    fs::write(destination.join("z-conflict"), b"Previous destination").unwrap();
    let service = service(root.path());
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![a.clone(), z.clone()],
            destination: destination.clone(),
        }),
    );
    question(&service, id);
    let waiting = service
        .snapshot()
        .jobs
        .into_iter()
        .find(|job| job.id == id)
        .unwrap();
    let progress = waiting
        .progress
        .expect("the question must retain the latest progress");
    assert_eq!(
        progress.completed, 1,
        "a throttled update hid the completed prefix while waiting"
    );
    assert_eq!(progress.bytes, b"Completed prefix".len() as u64);
    service.cancel(id).unwrap();
    let stopped = wait_for(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id && job.phase == Phase::Stopped)
    });
    let report = stopped.transfer.unwrap();
    assert!(report.cancelled);
    assert_eq!(report.completed, 1);
    assert_eq!(report.remaining, vec![z.clone()]);
    let inode = fs::metadata(destination.join("a-completed")).unwrap().ino();
    let mut shutdown = service.shutdown().unwrap();
    // Live executable work refuses orderly shutdown. Dropping this idle owner
    // models owner loss; reopening must preserve its saved report without replay.
    assert!(wait_for(|| shutdown.try_recv().unwrap()).is_err());
    lose_idle_owner(service, &root.path().join("state/history"));
    fs::write(destination.join("z-conflict"), b"Newer destination").unwrap();
    let service = self::service(root.path());
    let stopped = service
        .snapshot()
        .jobs
        .into_iter()
        .find(|job| job.id == id)
        .unwrap();
    assert_eq!(stopped.phase, Phase::Stopped);
    assert_eq!(stopped.transfer.unwrap().remaining, vec![z]);
    assert_eq!(
        fs::read(destination.join("z-conflict")).unwrap(),
        b"Newer destination"
    );
    assert_eq!(
        fs::metadata(destination.join("a-completed")).unwrap().ino(),
        inode
    );
    let undo = submit(
        &service,
        Request::Undo {
            expected: service.snapshot().undo.unwrap().id,
        },
    );
    finished(&service, undo);
    assert!(!destination.join("a-completed").exists());
    assert_eq!(fs::read(a).unwrap(), b"Completed prefix");
    assert_eq!(
        fs::read(destination.join("z-conflict")).unwrap(),
        b"Newer destination"
    );
}

#[test]
fn a_transfer_uses_the_shared_service_and_undo_reverses_its_actual_files() {
    use std::{
        ffi::OsString,
        os::unix::{ffi::OsStringExt, fs::symlink},
    };
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    let raw = source.join(OsString::from_vec(b"notes-\xff.txt".to_vec()));
    fs::write(&raw, b"Original bytes").unwrap();
    symlink(raw.file_name().unwrap(), source.join("link")).unwrap();
    let service = service(root.path());
    let id = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![source.clone()],
            destination: destination.clone(),
        }),
    );
    finished(&service, id);
    let copied = destination.join("source");
    assert_eq!(
        fs::read(copied.join(raw.file_name().unwrap())).unwrap(),
        b"Original bytes"
    );
    assert_eq!(
        fs::read_link(copied.join("link")).unwrap(),
        raw.file_name().unwrap()
    );
    assert_eq!(fs::read(&raw).unwrap(), b"Original bytes");
    let undo = service.snapshot().undo.unwrap();
    let id = submit(&service, Request::Undo { expected: undo.id });
    finished(&service, id);
    assert!(!copied.exists());
    assert_eq!(fs::read(raw).unwrap(), b"Original bytes");
}
