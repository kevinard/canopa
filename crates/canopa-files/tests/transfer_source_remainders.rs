use canopa_files::{
    Cancellation,
    operations::OperationEngine,
    transfers::{ConflictChoice, TransferEvent, TransferKind, TransferReply, TransferRequest},
};
use std::{fs, os::unix::fs::MetadataExt};

#[test]
fn a_move_reports_the_original_source_folder_when_new_work_arrives_during_merge() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source/Album");
    let target = root.path().join("destination");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(target.join("Album")).unwrap();
    let original = source.join("one.txt");
    let later = source.join("later.txt");
    fs::write(&original, b"Moved original bytes").unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let mut arrived = false;
    let outcome = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: target.clone(),
            },
            &Cancellation::default(),
            |event| match event {
                TransferEvent::Conflict { folders: true, .. } => TransferReply::Resolve {
                    choice: ConflictChoice::Merge,
                    apply_to_remaining: false,
                },
                TransferEvent::Progress { path, .. }
                    if path == original && !original.exists() && !arrived =>
                {
                    fs::write(&later, b"New work stays at the source").unwrap();
                    arrived = true;
                    TransferReply::Continue
                }
                _ => TransferReply::Continue,
            },
        )
        .unwrap();
    assert!(arrived);
    assert!(outcome.failures.is_empty());
    assert!(outcome.remaining.is_empty());
    assert_eq!(
        fs::read(target.join("Album/one.txt")).unwrap(),
        b"Moved original bytes"
    );
    assert_eq!(fs::read(later).unwrap(), b"New work stays at the source");
    assert_eq!(outcome.source_remainders, [source]);
}

#[test]
fn a_new_folder_at_the_completed_sources_name_is_not_a_source_remainder() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source/Album");
    let target = root.path().join("destination");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(target.join("Album")).unwrap();
    fs::write(source.join("one.txt"), b"Moved original bytes").unwrap();
    // Keep the observed original inode alive, so this prepares a provably
    // different directory rather than relying on the allocator's reuse policy.
    let original = fs::File::open(&source).unwrap();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let mut recreated = false;
    let outcome = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: target.clone(),
            },
            &Cancellation::default(),
            |event| match event {
                TransferEvent::Conflict { folders: true, .. } => TransferReply::Resolve {
                    choice: ConflictChoice::Merge,
                    apply_to_remaining: false,
                },
                TransferEvent::Progress { path, .. }
                    if path == source && !source.exists() && !recreated =>
                {
                    fs::create_dir(&source).unwrap();
                    fs::write(source.join("unrelated.txt"), b"An unrelated new folder").unwrap();
                    recreated = true;
                    TransferReply::Continue
                }
                _ => TransferReply::Continue,
            },
        )
        .unwrap();
    assert!(recreated);
    assert_ne!(
        original.metadata().unwrap().ino(),
        fs::metadata(&source).unwrap().ino()
    );
    assert!(outcome.source_remainders.is_empty());
    assert_eq!(
        fs::read(target.join("Album/one.txt")).unwrap(),
        b"Moved original bytes"
    );
    assert_eq!(
        fs::read(source.join("unrelated.txt")).unwrap(),
        b"An unrelated new folder"
    );
}
