#![cfg(feature = "local-runtime")]

use canopa_files::{
    operation_jobs::{JobSnapshot, OperationJobs, Phase, Request},
    recovery_catalog::Reader,
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

fn wait<T>(mut ready: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = ready() {
            return value;
        }
        assert!(Instant::now() < deadline, "Recovery catalog did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn receive<T>(mut receiver: futures_channel::oneshot::Receiver<T>) -> T {
    wait(|| receiver.try_recv().expect("backend reply dropped"))
}
fn start(root: &Path) -> OperationJobs {
    let service = OperationJobs::start(root.join("history"), root.join("volumes")).unwrap();
    wait(|| (!service.snapshot().initializing).then_some(()));
    assert!(service.snapshot().error.is_none());
    service
}
fn submit(service: &OperationJobs, request: Request) -> u64 {
    receive(service.submit(request, None).unwrap()).unwrap()
}
fn settled(service: &OperationJobs, id: u64) -> JobSnapshot {
    wait(|| {
        service.snapshot().jobs.into_iter().find(|job| {
            job.id == id && !matches!(job.phase, Phase::Queued | Phase::Running | Phase::Waiting)
        })
    })
}
fn answer_replace(service: &OperationJobs, id: u64) {
    let question = wait(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == id)
            .and_then(|job| {
                assert!(
                    matches!(job.phase, Phase::Queued | Phase::Running | Phase::Waiting),
                    "Transfer stopped before its replacement question: {job:?}"
                );
                job.decision
            })
    });
    service
        .resolve_transfer(
            id,
            question.generation,
            TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: true,
            },
        )
        .unwrap();
}

#[test]
fn returned_ancestors_restore_observation_of_retained_payloads() {
    use canopa_files::{Cancellation, operations::OperationEngine};
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::create_dir(root.join("incoming")).unwrap();
    fs::create_dir(root.join("destination")).unwrap();
    let source = root.join("incoming/report.txt");
    fs::write(&source, b"incoming").unwrap();
    fs::write(root.join("destination/report.txt"), b"saved version").unwrap();
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination: root.join("destination"),
            },
            &Cancellation::default(),
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: true,
            },
        )
        .unwrap();
    let item = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items
        .remove(0);
    let identity = item.identity();
    let path = item.entry.unwrap().path;
    assert!(path.starts_with(root.join("destination")));
    drop(engine);
    fs::rename(root.join("destination"), root.join("parked")).unwrap();
    let service = start(root);
    let reader = Reader::start(service.clone()).unwrap();
    wait(|| {
        let s = reader.snapshot();
        (!s.loading && s.catalog.items.len() == 1 && s.catalog.items[0].entry.is_none())
            .then_some(())
    });
    fs::rename(root.join("parked"), root.join("destination")).unwrap();
    let returned = wait(|| {
        let s = reader.snapshot();
        (!s.loading)
            .then(|| {
                s.catalog
                    .items
                    .iter()
                    .find(|i| i.identity() == identity && i.entry.is_some())
                    .cloned()
            })
            .flatten()
    });
    let version = returned.version();
    fs::write(path, b"externally edited after return").unwrap();
    wait(|| {
        let s = reader.snapshot();
        (!s.loading
            && s.catalog
                .items
                .iter()
                .any(|i| i.identity() == identity && i.version() != version))
        .then_some(())
    });
    drop(reader);
    receive(service.shutdown().unwrap()).unwrap();
}

#[test]
fn a_large_recovery_page_stays_responsive_with_unrelated_later_copy_steps() {
    use canopa_files::{Cancellation, operations::OperationEngine};
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::create_dir(root.join("incoming")).unwrap();
    fs::create_dir(root.join("destination")).unwrap();
    let mut sources = Vec::new();
    for index in 0..270 {
        let name = format!("file-{index:03}.txt");
        let source = root.join("incoming").join(&name);
        fs::write(&source, b"incoming").unwrap();
        fs::write(root.join("destination").join(name), b"saved version").unwrap();
        sources.push(source);
    }
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources,
                destination: root.join("destination"),
            },
            &Cancellation::default(),
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: true,
            },
        )
        .unwrap();
    drop(engine);
    let service = start(root);
    let began = Instant::now();
    let page = receive(
        service
            .recovery_page(None, Cancellation::default())
            .unwrap(),
    )
    .unwrap();
    let elapsed = began.elapsed();
    assert_eq!(page.items.len(), 256);
    assert!(page.next.is_some());
    // This private metadata-only page must leave room for interactive commands.
    // The bound has substantial margin over ordinary filesystem/stat overhead;
    // Decoding all later copy steps for every row misses this budget.
    assert!(
        elapsed < Duration::from_secs(2),
        "Recovery page held the shared queue for {elapsed:?}"
    );
    receive(service.shutdown().unwrap()).unwrap();
}

#[test]
fn headless_reader_observes_new_versions_external_edits_and_unavailable_payloads() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let service = start(root);
    let reader = Reader::start(service.clone()).unwrap();
    wait(|| (!reader.snapshot().loading).then_some(()));
    assert!(reader.snapshot().catalog.items.is_empty());
    assert!(reader.snapshot().error.is_none());
    fs::create_dir(root.join("incoming")).unwrap();
    fs::create_dir(root.join("destination")).unwrap();
    let name = OsString::from_vec(b"report-\xff.txt".to_vec());
    let incoming = root.join("incoming").join(&name);
    let destination = root.join("destination").join(&name);
    fs::write(&incoming, b"incoming bytes\n").unwrap();
    fs::write(&destination, b"retained bytes\n").unwrap();
    let copy = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![incoming.clone()],
            destination: root.join("destination"),
        }),
    );
    wait(|| {
        service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == copy && job.phase == Phase::Waiting)
            .then_some(())
    });
    for _ in 0..250 {
        reader.refresh();
    }
    let requested = reader.snapshot().revision;
    wait(|| {
        let s = reader.snapshot();
        (s.revision >= requested && !s.loading).then_some(())
    });
    assert_eq!(
        service
            .snapshot()
            .jobs
            .iter()
            .find(|job| job.id == copy)
            .unwrap()
            .phase,
        Phase::Waiting
    );
    answer_replace(&service, copy);
    assert_eq!(settled(&service, copy).phase, Phase::Finished);
    let item = wait(|| {
        let s = reader.snapshot();
        (!s.loading && s.catalog.items.len() == 1).then(|| s.catalog.items[0].clone())
    });
    assert_eq!(item.name(), name);
    assert_eq!(item.original.as_ref(), Some(&destination));
    let identity = item.identity();
    let version = item.version();
    let path = item.entry.as_ref().unwrap().path.clone();
    fs::write(&path, b"externally edited saved bytes\n").unwrap();
    wait(|| {
        let s = reader.snapshot();
        s.catalog
            .items
            .iter()
            .any(|item| item.identity() == identity && item.version() != version)
            .then_some(())
    });
    let held = root.join("held-payload");
    fs::rename(&path, &held).unwrap();
    wait(|| {
        let s = reader.snapshot();
        s.catalog
            .items
            .iter()
            .any(|item| item.identity() == identity && item.entry.is_none())
            .then_some(())
    });
    fs::rename(&held, &path).unwrap();
    let returned = wait(|| {
        let s = reader.snapshot();
        s.catalog
            .items
            .iter()
            .find(|item| item.identity() == identity && item.entry.is_some())
            .cloned()
    });
    let output = root.join("recovered.txt");
    let recovered = submit(
        &service,
        Request::Recover {
            item: Box::new(returned),
            destination: output.clone(),
        },
    );
    assert_eq!(settled(&service, recovered).phase, Phase::Finished);
    assert_eq!(
        fs::read(output).unwrap(),
        b"externally edited saved bytes\n"
    );
    assert_eq!(fs::read(&path).unwrap(), b"externally edited saved bytes\n");
    assert_eq!(fs::read(destination).unwrap(), b"incoming bytes\n");
    let waiting = submit(
        &service,
        Request::Transfer(TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![incoming],
            destination: root.join("destination"),
        }),
    );
    let decision = wait(|| {
        service
            .snapshot()
            .jobs
            .into_iter()
            .find(|job| job.id == waiting)
            .and_then(|job| job.decision)
    });
    drop(reader);
    assert_eq!(
        service
            .snapshot()
            .jobs
            .iter()
            .find(|job| job.id == waiting)
            .unwrap()
            .phase,
        Phase::Waiting
    );
    service
        .resolve_transfer(waiting, decision.generation, TransferReply::Cancel)
        .unwrap();
    assert_eq!(settled(&service, waiting).phase, Phase::Stopped);
    // Cancelling execution preserves its exact remaining plan independently
    // of the dropped catalog reader. Explicit abandonment permits shutdown.
    assert!(receive(service.shutdown().unwrap()).is_err());
    receive(service.abandon_transfer(waiting).unwrap()).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"externally edited saved bytes\n");
    receive(service.shutdown().unwrap()).unwrap();
}

#[test]
fn pages_remain_loaded_and_saved_identity_survives_active_undo_expiry() {
    use canopa_files::operations::OperationRequest;
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::create_dir(root.join("incoming")).unwrap();
    fs::create_dir(root.join("destination")).unwrap();
    let mut sources = Vec::new();
    for index in 0..270 {
        let name = format!("report-{index:03}.txt");
        let source = root.join("incoming").join(&name);
        fs::write(&source, b"incoming bytes\n").unwrap();
        fs::write(
            root.join("destination").join(name),
            b"original retained version\n",
        )
        .unwrap();
        sources.push(source);
    }
    // Seed the paged catalog through the public engine. A 270-source queued
    // plan can exceed its independent durable-record budget before any conflict;
    // this regression exercises catalog paging and Undo expiry, not plan admission.
    let engine = canopa_files::operations::OperationEngine::open(root.join("history")).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources,
                destination: root.join("destination"),
            },
            &canopa_files::Cancellation::default(),
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: true,
            },
        )
        .unwrap();
    drop(engine);
    let service = start(root);
    let reader = Reader::start(service.clone()).unwrap();
    wait(|| {
        let s = reader.snapshot();
        (!s.loading && s.catalog.items.len() == 256).then_some(())
    });
    assert!(reader.snapshot().catalog.has_more);
    reader.load_more();
    wait(|| {
        let s = reader.snapshot();
        (!s.loading && s.catalog.items.len() == 270).then_some(())
    });
    assert!(!reader.snapshot().catalog.has_more);
    let identities = reader
        .snapshot()
        .catalog
        .items
        .iter()
        .map(|item| item.identity())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        identities.len(),
        270,
        "duplicate saved items across page cursors"
    );
    let item = reader.snapshot().catalog.items[0].clone();
    assert!(item.held_for_undo);
    let identity = item.identity();
    let path = item.entry.as_ref().unwrap().path.clone();
    let version = item.version();
    fs::write(&path, b"protected newer saved bytes\n").unwrap();
    wait(|| {
        reader
            .snapshot()
            .catalog
            .items
            .iter()
            .any(|item| item.identity() == identity && item.version() != version)
            .then_some(())
    });
    reader.refresh();
    wait(|| {
        let s = reader.snapshot();
        (!s.loading && s.catalog.items.len() == 270).then_some(())
    });
    assert!(
        !reader.snapshot().catalog.has_more,
        "Refresh discarded loaded pages"
    );
    for index in 0..100 {
        let created = submit(
            &service,
            Request::Mutation(OperationRequest::CreateFolder {
                path: root.join(format!("folder-{index}")),
            }),
        );
        assert_eq!(settled(&service, created).phase, Phase::Finished);
    }
    let archived = wait(|| {
        let s = reader.snapshot();
        (!s.loading && s.catalog.items.len() == 1 && !s.catalog.has_more)
            .then(|| s.catalog.items[0].clone())
    });
    assert_eq!(archived.identity(), identity);
    assert!(!archived.held_for_undo);
    assert_eq!(
        fs::read(&archived.entry.unwrap().path).unwrap(),
        b"protected newer saved bytes\n"
    );
    drop(reader);
    receive(service.shutdown().unwrap()).unwrap();
}
