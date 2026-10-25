use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
};

fn replace(engine: &OperationEngine, source: std::path::PathBuf, destination: std::path::PathBuf) {
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination,
            },
            &Cancellation::default(),
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
}

#[test]
fn recovery_pages_large_batches_without_hiding_remaining_versions() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    let mut sources = Vec::new();
    for index in 0..257 {
        let name = format!("report-{index:03}.txt");
        fs::write(source.join(&name), b"incoming").unwrap();
        fs::write(destination.join(&name), b"previous").unwrap();
        sources.push(source.join(name));
    }
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources,
                destination,
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: true,
            },
        )
        .unwrap();
    let first = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(first.items.len(), 256);
    let next = engine.recovery_items(first.next, &cancel).unwrap();
    assert_eq!(next.items.len(), 1);
    assert!(next.next.is_none());
    let names: std::collections::HashSet<_> = first
        .items
        .into_iter()
        .chain(next.items)
        .map(|item| item.original.unwrap())
        .collect();
    assert_eq!(names.len(), 257);
}

#[test]
fn recovery_copies_symlinks_verbatim_and_keeps_unavailable_backups_visible() {
    use canopa_files::operations::RecoveryStatus;
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(root.path().join("document"), b"linked document").unwrap();
    fs::write(source.join("link"), b"incoming").unwrap();
    symlink("../document", destination.join("link")).unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    replace(&engine, source.join("link"), destination.clone());
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    let recovered = root.path().join("Recovered link");
    engine.recover_copy(&item, &recovered, &cancel).unwrap();
    assert_eq!(
        fs::read_link(&recovered).unwrap(),
        std::path::Path::new("../document")
    );
    assert_eq!(
        fs::read(root.path().join("document")).unwrap(),
        b"linked document"
    );
    engine.undo(&cancel).unwrap();
    let stored = item.entry.as_ref().unwrap().path.parent().unwrap();
    let absent = stored.with_file_name("temporarily-unavailable-stage");
    fs::rename(stored, &absent).unwrap();
    let unavailable = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert_eq!(unavailable.status, RecoveryStatus::Unavailable);
    assert!(unavailable.entry.is_none());
    assert_eq!(
        unavailable.original.as_deref(),
        Some(destination.join("link").as_path())
    );
    assert!(
        engine
            .recover_copy(&unavailable, &recovered, &cancel)
            .is_err()
    );
    assert!(fs::symlink_metadata(&recovered).is_err());
    fs::rename(&absent, stored).unwrap();
    // A formerly unavailable row needs a fresh observation before copying.
    assert_eq!(
        engine
            .recover_copy(&unavailable, &recovered, &cancel)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
    let refreshed = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    engine
        .recover_copy(&refreshed, &recovered, &cancel)
        .unwrap();
    assert_eq!(
        fs::read_link(&recovered).unwrap(),
        std::path::Path::new("../document")
    );
}

#[test]
fn trashing_the_replacement_keeps_its_backups_original_filename_and_location() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("report.txt"), b"incoming").unwrap();
    fs::write(destination.join("report.txt"), b"previous").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    replace(&engine, source.join("report.txt"), destination.clone());
    let location = canopa_files::trash::TrashLocation::home(root.path().join("data")).unwrap();
    engine
        .trash(&location, &[destination.join("report.txt")], &cancel)
        .unwrap();
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert_eq!(item.name(), std::ffi::OsStr::new("report.txt"));
    assert_eq!(
        item.original.as_deref(),
        Some(destination.join("report.txt").as_path())
    );
    let recovered = root.path().join("Recovered report.txt");
    engine.recover_copy(&item, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"previous");
    engine.undo(&cancel).unwrap();
    engine.undo(&cancel).unwrap();
    engine.undo(&cancel).unwrap();
    assert_eq!(
        fs::read(destination.join("report.txt")).unwrap(),
        b"previous"
    );
}

#[test]
fn recovery_preserves_active_undo_and_rejects_stale_or_occupied_destinations() {
    use canopa_files::operations::RecoveryStatus;
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    let name = std::ffi::OsString::from_vec(b"report-\xff.txt".to_vec());
    fs::write(source.join(&name), b"incoming").unwrap();
    fs::write(destination.join(&name), b"previous").unwrap();
    let mut document = fs::OpenOptions::new()
        .write(true)
        .open(destination.join(&name))
        .unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    replace(&engine, source.join(&name), destination.clone());
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert_eq!(item.status, RecoveryStatus::UndoBackup);
    assert!(item.held_for_undo);
    assert_eq!(
        item.original.as_deref(),
        Some(destination.join(&name).as_path())
    );
    let recovered = root.path().join("Recovered");
    let inside_retained = item
        .entry
        .as_ref()
        .unwrap()
        .path
        .parent()
        .unwrap()
        .join("Another copy");
    assert_eq!(
        engine
            .recover_copy(&item, &inside_retained, &cancel)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert!(!inside_retained.exists());
    fs::write(&recovered, b"occupied").unwrap();
    assert_eq!(
        engine
            .recover_copy(&item, &recovered, &cancel)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&recovered).unwrap(), b"occupied");
    fs::remove_file(&recovered).unwrap();
    let stopped = Cancellation::default();
    stopped.cancel();
    assert_eq!(
        engine
            .recover_copy(&item, &recovered, &stopped)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    assert!(!recovered.exists());
    engine.recover_copy(&item, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"previous");
    engine.undo(&cancel).unwrap();
    assert_eq!(
        engine.recovery_items(None, &cancel).unwrap().items[0].status,
        RecoveryStatus::UndoBackup
    );
    document.write_all(b"modified").unwrap();
    document.sync_all().unwrap();
    assert_eq!(
        engine
            .recover_copy(&item, &recovered, &cancel)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(!recovered.exists());
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert_eq!(item.status, RecoveryStatus::NeedsRecovery);
    engine.recover_copy(&item, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"modified");
    engine.undo(&cancel).unwrap();
    assert_eq!(
        engine.undo(&cancel).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(fs::read(destination.join(&name)).unwrap(), b"incoming");
    assert_eq!(engine.recovery_items(None, &cancel).unwrap().items.len(), 1);
}

#[test]
fn recovery_follows_retained_versions_through_folder_moves_and_expiry() {
    use canopa_files::operations::RecoveryStatus;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let first = root.path().join("first");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&first).unwrap();
    fs::write(source.join("report.txt"), b"incoming").unwrap();
    fs::write(first.join("report.txt"), b"previous").unwrap();
    let mut document = fs::OpenOptions::new()
        .write(true)
        .open(first.join("report.txt"))
        .unwrap();
    let state = root.path().join("state/history");
    let engine = OperationEngine::open(&state).unwrap();
    let cancel = Cancellation::default();
    replace(&engine, source.join("report.txt"), first.clone());
    let second = root.path().join("second");
    engine
        .execute(
            OperationRequest::Rename {
                source: first.clone(),
                destination: second.clone(),
            },
            &cancel,
        )
        .unwrap();
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert_eq!(item.status, RecoveryStatus::UndoBackup);
    assert_eq!(
        item.original.as_deref(),
        Some(second.join("report.txt").as_path())
    );
    let recovered = root.path().join("Recovered");
    engine.recover_copy(&item, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"previous");
    engine.undo(&cancel).unwrap();
    engine.undo(&cancel).unwrap();
    document.write_all(b"protected").unwrap();
    document.sync_all().unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: first.clone(),
                destination: second.clone(),
            },
            &cancel,
        )
        .unwrap();
    for number in 0..101 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("folder-{number}")),
                },
                &cancel,
            )
            .unwrap();
    }
    let third = root.path().join("third");
    engine
        .execute(
            OperationRequest::Rename {
                source: second.clone(),
                destination: third.clone(),
            },
            &cancel,
        )
        .unwrap();
    drop(engine);
    let engine = OperationEngine::open(&state).unwrap();
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert_eq!(item.status, RecoveryStatus::NeedsRecovery);
    assert_eq!(
        item.original.as_deref(),
        Some(third.join("report.txt").as_path())
    );
    engine.recover_copy(&item, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"protected");
    engine.undo(&cancel).unwrap();
    fs::create_dir(&second).unwrap();
    fs::write(second.join("newer.txt"), b"newer folder contents").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| {
            canopa_files::operations::UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(
        fs::read(second.join("newer.txt")).unwrap(),
        b"newer folder contents"
    );
    let returned = root.path().join("second (2)");
    let item = engine
        .recovery_items(None, &cancel)
        .unwrap()
        .items
        .remove(0);
    assert_eq!(
        item.original.as_deref(),
        Some(returned.join("report.txt").as_path())
    );
    engine.recover_copy(&item, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"protected");
}

#[test]
fn protected_replacement_data_survives_expiry_and_can_be_recovered_without_consuming_it() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    let previous = destination.join("report.txt");
    fs::write(&previous, b"previous version").unwrap();
    let mut open_document = fs::OpenOptions::new().write(true).open(&previous).unwrap();
    fs::write(source.join("report.txt"), b"incoming version").unwrap();
    let state = root.path().join("state/operations.sqlite3");
    let engine = OperationEngine::open(&state).unwrap();
    let cancel = Cancellation::default();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.join("report.txt")],
                destination,
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    open_document.seek(SeekFrom::Start(0)).unwrap();
    open_document
        .write_all(b"new work in the retained document")
        .unwrap();
    open_document.set_len(33).unwrap();
    open_document.sync_all().unwrap();
    drop(open_document);
    for number in 0..101 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.path().join(format!("folder-{number}")),
                },
                &cancel,
            )
            .unwrap();
    }
    drop(engine);
    let engine = OperationEngine::open(&state).unwrap();
    let page = engine.recovery_items(None, &cancel).unwrap();
    assert_eq!(page.items.len(), 1);
    let item = &page.items[0];
    assert_eq!(item.original.as_deref(), Some(previous.as_path()));
    assert!(!item.held_for_undo);
    let recovered = root.path().join("Recovered report.txt");
    engine.recover_copy(item, &recovered, &cancel).unwrap();
    assert_eq!(
        fs::read(&recovered).unwrap(),
        b"new work in the retained document"
    );
    assert_eq!(engine.recovery_items(None, &cancel).unwrap().items.len(), 1);
    engine.undo(&cancel).unwrap();
    assert!(!recovered.exists());
    assert_eq!(engine.recovery_items(None, &cancel).unwrap().items.len(), 1);
}
