//! The helper pipe is an external protocol seam. Storage uses real private files.
use super::*;
use std::os::unix::ffi::OsStringExt;

fn frame(root: &std::path::Path, generation: u64) -> Frame {
    Frame {
        version: 2,
        generation,
        mounts: vec![Mount {
            instance: 11,
            revision: 1,
            path: root.to_owned(),
            identity: "uuid:private-device".into(),
        }],
        devices: vec![catalog::WireDevice {
            instance: 22,
            name: "Private drive".into(),
            mount: Some(11),
            can_mount: false,
            removal: Some(Removal::Unmount),
        }],
    }
}

fn read(
    frames: Vec<Frame>,
    storage: &Arc<Storage>,
    catalog: &Mutex<DeviceSnapshot>,
) -> io::Result<()> {
    let mut bytes = Vec::new();
    for frame in frames {
        serde_json::to_writer(&mut bytes, &frame).unwrap();
        bytes.push(b'\n');
    }
    let (mut notice, _unconsumed) = mpsc::channel(0);
    let (ready, _receiver) = std::sync::mpsc::sync_channel(1);
    read_observations(
        &mut io::Cursor::new(bytes),
        storage,
        &mut Observed::new(),
        &mut Some(ready),
        catalog,
        &mut notice,
        None,
    )
}

#[test]
fn catalog_uses_prepared_storage_and_preserves_raw_root_bytes() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary
        .path()
        .join(std::ffi::OsString::from_vec(b"drive-\xff".to_vec()));
    std::fs::create_dir(&root).unwrap();
    let storage = Storage::load(temporary.path().join("volumes.json")).unwrap();
    let catalog = Mutex::new(DeviceSnapshot::default());
    read(vec![frame(&root, 1)], &storage, &catalog).unwrap();
    let state = catalog.lock().unwrap().clone();
    assert_eq!(state.status, DeviceStatus::Ready);
    assert_eq!(state.devices[0].root.as_deref(), Some(root.as_path()));
    assert_eq!(state.devices[0].removal, Some(Removal::Unmount));
    storage
        .snapshot()
        .check(&[crate::storage::Access::folder(&root)])
        .unwrap();
}

#[test]
fn unmounted_catalog_changes_without_consuming_notices_or_losing_storage_guard() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("drive");
    std::fs::create_dir(&root).unwrap();
    let storage = Storage::load(temporary.path().join("volumes.json")).unwrap();
    let mut unmounted = frame(&root, 2);
    unmounted.mounts.clear();
    unmounted.devices[0].mount = None;
    unmounted.devices[0].can_mount = true;
    unmounted.devices[0].removal = None;
    let catalog = Mutex::new(DeviceSnapshot::default());
    read(vec![frame(&root, 1), unmounted], &storage, &catalog).unwrap();
    let state = catalog.lock().unwrap().clone();
    assert_eq!(state.generation, 2);
    assert_eq!(state.devices[0].root, None);
    assert!(state.devices[0].can_mount);
    assert_eq!(state.devices[0].token.revision, 2);
    assert!(
        root.is_dir(),
        "the old mount point deliberately still exists"
    );
    assert!(
        storage
            .snapshot()
            .check(&[crate::storage::Access::folder(root)])
            .is_err()
    );
}

#[test]
fn invalid_catalog_or_replayed_generation_cannot_replace_prepared_observation() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("drive");
    std::fs::create_dir(&root).unwrap();
    for case in 0..4 {
        let storage = Storage::load(temporary.path().join(format!("state-{case}.json"))).unwrap();
        let mut invalid = frame(&root, 2);
        match case {
            0 => invalid.devices[0].mount = Some(999),
            1 => invalid.devices.push(catalog::WireDevice {
                instance: 22,
                name: "Duplicate ID".into(),
                mount: None,
                can_mount: true,
                removal: None,
            }),
            2 => invalid.generation = 1,
            _ => invalid.version = 1,
        }
        let catalog = Mutex::new(DeviceSnapshot::default());
        assert!(read(vec![frame(&root, 1), invalid], &storage, &catalog).is_err());
        let state = catalog.lock().unwrap().clone();
        assert_eq!(state.generation, 1);
        assert_eq!(state.devices[0].root.as_deref(), Some(root.as_path()));
    }
}

#[test]
fn failed_storage_preparation_publishes_neither_catalog_nor_readiness() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("file-instead-of-root");
    std::fs::write(&root, b"not a device directory").unwrap();
    let storage = Storage::load(temporary.path().join("volumes.json")).unwrap();
    let catalog = Mutex::new(DeviceSnapshot::default());
    let (mut notice, mut notices) = mpsc::channel(0);
    let (ready, receiver) = std::sync::mpsc::sync_channel(1);
    let mut ready = Some(ready);
    let mut bytes = serde_json::to_vec(&frame(&root, 1)).unwrap();
    bytes.push(b'\n');
    assert!(
        read_observations(
            &mut io::Cursor::new(bytes),
            &storage,
            &mut Observed::new(),
            &mut ready,
            &catalog,
            &mut notice,
            None,
        )
        .is_err()
    );
    assert_eq!(catalog.lock().unwrap().status, DeviceStatus::Starting);
    assert!(catalog.lock().unwrap().devices.is_empty());
    assert!(matches!(
        receiver.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    assert!(matches!(notices.try_recv(), Err(mpsc::TryRecvError::Empty)));
}

#[test]
fn completed_mount_lookup_survives_unrelated_revisions_but_not_remount_or_failed_preparation() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("drive");
    std::fs::create_dir(&root).unwrap();
    let storage = Storage::load(temporary.path().join("volumes.json")).unwrap();
    let catalog = Arc::new(Mutex::new(DeviceSnapshot::default()));
    let (input, _reader) = std::sync::mpsc::sync_channel(16);
    let control = Arc::new(Control {
        stopped: AtomicBool::new(false),
        input: Mutex::new(Some(input)),
    });
    let (mut notice, _notices) = mpsc::channel(0);
    let owner = requests::Requests::new(
        control,
        catalog.clone(),
        crate::local_activity::LocalActivity::default(),
        notice.clone(),
    );
    let requests = DeviceRequests(owner.clone());
    let (ready, _receiver) = std::sync::mpsc::sync_channel(1);
    let mut ready = Some(ready);
    let mut previous = Observed::new();
    let mut consume = |values: Vec<serde_json::Value>| {
        let mut bytes = Vec::new();
        for value in values {
            serde_json::to_writer(&mut bytes, &value).unwrap();
            bytes.push(b'\n');
        }
        read_observations(
            &mut io::Cursor::new(bytes),
            &storage,
            &mut previous,
            &mut ready,
            &catalog,
            &mut notice,
            Some(&owner),
        )
    };
    let mut initial = frame(&root, 1);
    initial.mounts.clear();
    initial.devices[0].mount = None;
    initial.devices[0].can_mount = true;
    consume(vec![serde_json::to_value(initial).unwrap()]).unwrap();
    let token = catalog.lock().unwrap().devices[0].token;
    let id = requests.mount(token).unwrap();
    let mut mounted = frame(&root, 2);
    mounted.mounts[0].revision = 2;
    let mut unrelated = frame(&root, 3);
    unrelated.mounts[0].revision = 2;
    consume(vec![serde_json::to_value(mounted).unwrap(), serde_json::json!({
        "version":1,"id":1,"event":"finished","outcome":{"kind":"mounted","root":serde_json::to_value(&root).unwrap()}
    }), serde_json::to_value(unrelated).unwrap()]).unwrap();
    assert_eq!(requests.mounted_root(id).unwrap(), root);
    assert!(requests.root(token).is_err());
    let current = catalog.lock().unwrap().devices[0].token;
    assert_eq!(requests.root(current).unwrap(), root);

    let mut removed = frame(&root, 4);
    removed.mounts.clear();
    removed.devices[0].mount = None;
    let mut remounted = frame(&root, 5);
    remounted.mounts[0].revision = 5;
    consume(vec![
        serde_json::to_value(removed).unwrap(),
        serde_json::to_value(remounted).unwrap(),
    ])
    .unwrap();
    assert!(root.is_dir());
    assert!(requests.mounted_root(id).is_err());
    let token = catalog.lock().unwrap().devices[0].token;
    assert_eq!(requests.root(token).unwrap(), root);

    let unavailable = temporary.path().join("file-instead-of-root");
    std::fs::write(&unavailable, b"not a directory").unwrap();
    let mut failed = frame(&unavailable, 6);
    failed.mounts[0].revision = 6;
    assert!(consume(vec![serde_json::to_value(failed).unwrap()]).is_err());
    assert_eq!(catalog.lock().unwrap().status, DeviceStatus::Starting);
    assert!(requests.root(token).is_err());
}
