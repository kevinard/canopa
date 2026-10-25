use canopa_files::storage::{Access, Requirements, Snapshot, Storage};
use std::{
    ffi::OsString,
    fs, io,
    os::unix::{ffi::OsStringExt, fs::symlink},
    path::{Path, PathBuf},
    sync::Arc,
};

fn mount(storage: &Storage, path: &Path, identity: &str) -> u64 {
    let observation = storage.mounted(path.to_owned(), identity.into()).unwrap();
    let epoch = observation.epoch;
    storage.prepare(observation).unwrap();
    epoch
}

fn persisted(root: &Path, snapshot: &Snapshot) -> Requirements {
    let path = root.join("request.json");
    fs::write(&path, serde_json::to_vec(&snapshot.requirements()).unwrap()).unwrap();
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn fresh(path: &Path) -> Arc<Storage> {
    Storage::load(path.to_owned()).unwrap()
}

#[test]
fn restarted_requests_keep_raw_paths_alias_bindings_and_original_uuid_through_retries() {
    let root = tempfile::tempdir().unwrap();
    let volume = root
        .path()
        .join(OsString::from_vec(b"volume-\xff".to_vec()));
    fs::create_dir(&volume).unwrap();
    let original = volume.join("original");
    let foreign = volume.join("foreign");
    fs::create_dir(&original).unwrap();
    fs::create_dir(&foreign).unwrap();
    fs::write(original.join("work"), b"original bytes").unwrap();
    fs::write(foreign.join("work"), b"other bytes").unwrap();
    let alias = root.path().join(OsString::from_vec(b"alias-\xfe".to_vec()));
    symlink(&original, &alias).unwrap();
    let state = root.path().join("state/volumes.json");
    let storage = fresh(&state);
    mount(&storage, &volume, "uuid:original");
    let mut request = storage.snapshot();
    request.bind_folder(alias.clone(), original.clone());
    request.check(&[Access::entry(alias.join("work"))]).unwrap();
    let requirements = persisted(root.path(), &request);
    drop(request);
    drop(storage);

    let storage = fresh(&state);
    mount(&storage, &volume, "uuid:replacement");
    let restored = storage.restore(&requirements);
    assert_eq!(
        restored
            .check(&[Access::entry(alias.join("work"))])
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotConnected
    );
    let mut retry = storage.snapshot();
    retry.retry_from(&restored);
    // Saving a failed Retry must not adopt its currently observed replacement.
    let requirements = persisted(root.path(), &retry);
    drop(retry);
    drop(restored);
    drop(storage);
    let storage = fresh(&state);
    mount(&storage, &volume, "uuid:replacement");
    assert!(
        storage
            .restore(&requirements)
            .check(&[Access::entry(alias.join("work"))])
            .is_err()
    );
    mount(&storage, &volume, "uuid:original");
    let restored = storage.restore(&requirements);
    restored
        .check(&[Access::entry(alias.join("work"))])
        .unwrap();

    fs::remove_file(&alias).unwrap();
    symlink(&foreign, &alias).unwrap();
    assert_eq!(
        restored
            .check(&[Access::entry(alias.join("work"))])
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(fs::read(original.join("work")).unwrap(), b"original bytes");
    assert_eq!(fs::read(foreign.join("work")).unwrap(), b"other bytes");
    fs::remove_file(&alias).unwrap();
    symlink(&original, &alias).unwrap();
    restored
        .check(&[Access::entry(alias.join("work"))])
        .unwrap();
}

#[test]
fn a_no_uuid_request_cannot_gain_authority_from_reused_process_epoch_numbers() {
    let root = tempfile::tempdir().unwrap();
    let volume = root.path().join("volume");
    fs::create_dir(&volume).unwrap();
    let state = root.path().join("state/volumes.json");
    let storage = fresh(&state);
    mount(&storage, &volume, "mount:no-stable-id");
    let first_epoch = mount(&storage, &volume, "mount:no-stable-id");
    let request = storage.snapshot();
    request
        .check(&[Access::entry(volume.join("output"))])
        .unwrap();
    let requirements = persisted(root.path(), &request);
    drop(request);
    drop(storage);

    let storage = fresh(&state);
    let observation = storage
        .mounted(volume.clone(), "mount:no-stable-id".into())
        .unwrap();
    // Loading one saved root and observing it reuses the old process's second
    // counter. Equality must not grant authority over a new process's mount.
    assert_eq!(observation.epoch, first_epoch);
    storage.prepare(observation).unwrap();
    let restored = storage.restore(&requirements);
    assert_eq!(
        restored
            .check(&[Access::entry(volume.join("output"))])
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotConnected
    );
    let mut retry = storage.snapshot();
    retry.retry_from(&restored);
    assert!(
        retry
            .check(&[Access::entry(volume.join("output"))])
            .is_err()
    );
    // A fresh deliberate request remains usable in this process.
    storage
        .snapshot()
        .check(&[Access::entry(volume.join("output"))])
        .unwrap();
}

#[test]
fn runtime_retry_allows_the_same_no_uuid_observation_but_refuses_a_remount() {
    let root = tempfile::tempdir().unwrap();
    let volume = root.path().join("volume");
    fs::create_dir(&volume).unwrap();
    let storage = fresh(&root.path().join("state/volumes.json"));
    let epoch = mount(&storage, &volume, "mount:local");
    let request = storage.snapshot();
    let mut retry = storage.snapshot();
    retry.retry_from(&request);
    retry.check(&[Access::folder(volume.clone())]).unwrap();
    storage.removed(&volume, epoch);
    mount(&storage, &volume, "mount:local");
    let mut retry_again = storage.snapshot();
    retry_again.retry_from(&retry);
    assert!(retry_again.check(&[Access::folder(volume)]).is_err());
}

#[test]
fn restored_source_and_destination_keep_their_independent_capture_times() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    let state = root.path().join("state/volumes.json");
    let storage = fresh(&state);
    mount(&storage, &source, "uuid:source-original");
    let copied = persisted(root.path(), &storage.snapshot());
    mount(&storage, &source, "uuid:source-replacement");
    mount(&storage, &destination, "uuid:destination");
    let pasted = persisted(root.path(), &storage.snapshot());
    drop(storage);
    let storage = fresh(&state);
    mount(&storage, &source, "uuid:source-replacement");
    mount(&storage, &destination, "uuid:destination");
    storage
        .restore(&pasted)
        .check(&[Access::folder(destination)])
        .unwrap();
    assert!(
        storage
            .restore(&copied)
            .check(&[Access::entry(source.join("selected"))])
            .is_err()
    );
    mount(&storage, &source, "uuid:source-original");
    storage
        .restore(&copied)
        .check(&[Access::entry(source.join("selected"))])
        .unwrap();
}

#[test]
fn portable_requirements_reject_incompatible_relative_and_duplicate_bindings() {
    let root = tempfile::tempdir().unwrap();
    let storage = fresh(&root.path().join("state/volumes.json"));
    let mut request = storage.snapshot();
    request.bind_folder(PathBuf::from("/chosen"), PathBuf::from("/original"));
    let value = serde_json::to_value(request.requirements()).unwrap();
    let mut incompatible = value.clone();
    incompatible["version"] = 2.into();
    assert!(serde_json::from_value::<Requirements>(incompatible).is_err());
    let mut relative = value.clone();
    relative["bindings"][0]["resolved"] = "relative".into();
    assert!(serde_json::from_value::<Requirements>(relative).is_err());
    let mut duplicate = value;
    let binding = duplicate["bindings"][0].clone();
    duplicate["bindings"].as_array_mut().unwrap().push(binding);
    assert!(serde_json::from_value::<Requirements>(duplicate).is_err());
}
