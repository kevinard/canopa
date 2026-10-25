use canopa_files::{
    Cancellation,
    operations::{OperationEngine, OperationRequest, RecoveryItem},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
    trash::{TrashItem, TrashLocation},
};
use std::{
    ffi::OsString,
    fs,
    os::unix::ffi::OsStringExt,
    time::{Duration, SystemTime},
};

fn stored<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T) -> T {
    serde_json::from_slice(&serde_json::to_vec(value).unwrap()).unwrap()
}

fn old_file(path: &std::path::Path, bytes: &[u8]) -> SystemTime {
    fs::write(path, bytes).unwrap();
    let modified = SystemTime::UNIX_EPOCH - Duration::new(12_345, 123_456_789);
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), modified);
    modified
}

#[test]
fn decoded_create_rename_and_transfer_requests_keep_original_unix_paths() {
    let root = tempfile::tempdir().unwrap();
    let folder = root
        .path()
        .join(OsString::from_vec(b"folder-\xff".to_vec()));
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    engine
        .execute(
            stored(&OperationRequest::CreateFolder {
                path: folder.clone(),
            }),
            &cancel,
        )
        .unwrap();
    let before = folder.join(OsString::from_vec(b"before-\xfe".to_vec()));
    let after = folder.join(OsString::from_vec(b"after-\xfd".to_vec()));
    fs::write(&before, b"request bytes").unwrap();
    let rename = stored(&OperationRequest::Rename {
        source: before.clone(),
        destination: after.clone(),
    });
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    engine.execute(rename, &cancel).unwrap();
    assert!(!before.exists());
    assert_eq!(fs::read(&after).unwrap(), b"request bytes");
    let target = root
        .path()
        .join(OsString::from_vec(b"target-\xfc".to_vec()));
    fs::create_dir(&target).unwrap();
    let request = stored(&TransferRequest {
        kind: TransferKind::Copy,
        sources: vec![after.clone()],
        destination: target.clone(),
    });
    let result = engine
        .transfer(&request, &cancel, |_| TransferReply::Continue)
        .unwrap();
    assert!(result.failures.is_empty());
    assert_eq!(
        fs::read(target.join(after.file_name().unwrap())).unwrap(),
        b"request bytes"
    );
    assert_eq!(fs::read(after).unwrap(), b"request bytes");
}

#[test]
fn a_stored_volume_trash_item_restores_raw_names_and_a_pre_epoch_timestamp() {
    let root = tempfile::tempdir().unwrap();
    let volume = root
        .path()
        .join(OsString::from_vec(b"volume-\xff".to_vec()));
    fs::create_dir(&volume).unwrap();
    let source = volume.join(OsString::from_vec(b"report-\xfe\n%.txt".to_vec()));
    let modified = old_file(&source, b"older recoverable work");
    let location = TrashLocation::volume(&volume).unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    assert!(
        engine
            .trash(&location, std::slice::from_ref(&source), &cancel)
            .unwrap()
            .failures
            .is_empty()
    );
    let item: TrashItem = stored(&location.list(&cancel).unwrap().remove(0));
    assert_eq!(item.entry.modified, Some(modified));
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    engine.restore(&item, None, &cancel).unwrap();
    assert_eq!(fs::read(&source).unwrap(), b"older recoverable work");
    assert_eq!(fs::metadata(&source).unwrap().modified().unwrap(), modified);
    assert!(location.list(&cancel).unwrap().is_empty());
    engine.undo(&cancel).unwrap();
    assert!(!source.exists());
    let returned = location.list(&cancel).unwrap().remove(0);
    assert_eq!(fs::read(returned.path).unwrap(), b"older recoverable work");
}

#[test]
fn a_stored_recovery_selection_keeps_raw_names_observed_version_and_saved_bytes() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    let name = OsString::from_vec(b"saved-\xff.txt".to_vec());
    fs::write(source.join(&name), b"incoming work").unwrap();
    let modified = old_file(&destination.join(&name), b"retained older work");
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.join(&name)],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    let item: RecoveryItem = stored(
        &engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .remove(0),
    );
    assert_eq!(item.entry.as_ref().unwrap().modified, Some(modified));
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let recovered = root
        .path()
        .join(OsString::from_vec(b"recovered-\xfe.txt".to_vec()));
    engine.recover_copy(&item, &recovered, &cancel).unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"retained older work");
    assert_eq!(fs::read(destination.join(name)).unwrap(), b"incoming work");
    engine.undo(&cancel).unwrap();
    assert!(!recovered.exists());
    assert_eq!(
        fs::read(item.entry.unwrap().path).unwrap(),
        b"retained older work"
    );
}
