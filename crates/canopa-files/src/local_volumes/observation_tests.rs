//! The helper pipe is an external protocol seam. Storage uses real private files.
use super::*;
use crate::local_volumes::{DeviceAnswer, DeviceOperation, DeviceOutcome, DevicePhase, Remember};
use std::os::unix::ffi::OsStringExt;

fn frame(root: &std::path::Path, generation: u64) -> Frame {
    Frame {
        version: 3,
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
            kind: LocationKind::Local,
            uri: None,
            remote_only: false,
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
                kind: LocationKind::Local,
                uri: None,
                remote_only: false,
            }),
            2 => invalid.generation = 1,
            _ => invalid.version = 2,
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

/// A parent attached to a fake helper: frames go in through `read_observations`
/// and the commands the parent sends are read back from its private pipe.
struct FakeHelper {
    storage: Arc<Storage>,
    catalog: Arc<Mutex<DeviceSnapshot>>,
    owner: Arc<requests::Requests>,
    commands: std::sync::mpsc::Receiver<Vec<u8>>,
    previous: Observed,
    notice: mpsc::Sender<()>,
    ready: Option<std::sync::mpsc::SyncSender<Ready>>,
    _ready: std::sync::mpsc::Receiver<Ready>,
    _notices: mpsc::Receiver<()>,
}

impl FakeHelper {
    fn new(state: &std::path::Path) -> Self {
        let storage = Storage::load(state.join("volumes.json")).unwrap();
        let catalog = Arc::new(Mutex::new(DeviceSnapshot::default()));
        let (input, commands) = std::sync::mpsc::sync_channel(16);
        let control = Arc::new(Control {
            stopped: AtomicBool::new(false),
            input: Mutex::new(Some(input)),
        });
        let (notice, notices) = mpsc::channel(0);
        let owner = requests::Requests::new(
            control,
            catalog.clone(),
            crate::local_activity::LocalActivity::default(),
            notice.clone(),
        );
        let (ready, receiver) = std::sync::mpsc::sync_channel(1);
        Self {
            storage,
            catalog,
            owner,
            commands,
            previous: Observed::new(),
            notice,
            ready: Some(ready),
            _ready: receiver,
            _notices: notices,
        }
    }

    fn requests(&self) -> DeviceRequests {
        DeviceRequests(self.owner.clone())
    }

    fn send(&mut self, values: Vec<serde_json::Value>) -> io::Result<()> {
        let mut bytes = Vec::new();
        for value in values {
            serde_json::to_writer(&mut bytes, &value).unwrap();
            bytes.push(b'\n');
        }
        read_observations(
            &mut io::Cursor::new(bytes),
            &self.storage,
            &mut self.previous,
            &mut self.ready,
            &self.catalog,
            &mut self.notice,
            Some(&self.owner),
        )
    }

    fn command(&self) -> serde_json::Value {
        let bytes = self.commands.try_recv().expect("a command was sent");
        assert_eq!(bytes.last(), Some(&b'\n'));
        serde_json::from_slice(&bytes).unwrap()
    }
}

fn network_frame(root: &std::path::Path, generation: u64) -> serde_json::Value {
    let mut frame = frame(root, generation);
    frame.mounts[0].revision = generation;
    frame.mounts[0].identity = "location:sftp://me@box/".into();
    frame.devices[0].name = "me on box".into();
    frame.devices[0].kind = LocationKind::Network;
    frame.devices[0].uri = Some("sftp://me@box/".into());
    serde_json::to_value(frame).unwrap()
}

#[test]
fn connect_sends_only_the_normalized_address_and_opens_its_folder() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("gvfs/sftp:host=box,user=me");
    let location = root.join("srv/data");
    std::fs::create_dir_all(&location).unwrap();
    let mut helper = FakeHelper::new(temporary.path());
    let mut empty = frame(&root, 1);
    empty.mounts.clear();
    empty.devices.clear();
    helper
        .send(vec![serde_json::to_value(empty).unwrap()])
        .unwrap();
    let requests = helper.requests();
    let address = crate::network::ServerAddress::parse("ssh://me@box/srv/data").unwrap();
    let id = requests.connect(&address).unwrap();
    let command = helper.command();
    assert_eq!(command["command"]["kind"], "connect");
    assert_eq!(command["command"]["uri"], "sftp://me@box/srv/data");
    assert!(
        requests.connect(&address).is_err(),
        "one device request at a time"
    );
    let job = requests
        .snapshot()
        .into_iter()
        .find(|job| job.id == id)
        .unwrap();
    assert_eq!(job.operation, DeviceOperation::Connect);
    assert_eq!(job.device, None);
    assert_eq!(job.name, "srv on me@box");

    // The helper publishes the new mount before the outcome.
    helper
        .send(vec![
            network_frame(&root, 2),
            serde_json::json!({
                "version": 1, "id": 1, "event": "finished",
                "outcome": {"kind": "connected",
                    "root": serde_json::to_value(&root).unwrap(),
                    "location": serde_json::to_value(&location).unwrap()}
            }),
        ])
        .unwrap();
    assert_eq!(requests.mounted_root(id).unwrap(), location);
    let device = helper.catalog.lock().unwrap().devices[0].clone();
    assert_eq!(device.kind, LocationKind::Network);
    assert_eq!(device.uri.as_deref(), Some("sftp://me@box/"));
    helper
        .storage
        .snapshot()
        .check(&[crate::storage::Access::folder(&location)])
        .unwrap();

    // Disconnecting the mount retires the proof, like a removed drive.
    let mut gone = frame(&root, 3);
    gone.mounts.clear();
    gone.devices.clear();
    helper
        .send(vec![serde_json::to_value(gone).unwrap()])
        .unwrap();
    assert!(requests.mounted_root(id).is_err());
    assert!(
        helper
            .storage
            .snapshot()
            .check(&[crate::storage::Access::folder(&location)])
            .is_err(),
        "work on a disconnected share waits instead of writing to the old path"
    );
}

#[test]
fn server_logins_offer_remembering_only_when_the_backend_can_keep_it() {
    let temporary = tempfile::tempdir().unwrap();
    let mut helper = FakeHelper::new(temporary.path());
    let mut empty = frame(temporary.path(), 1);
    empty.mounts.clear();
    empty.devices.clear();
    helper
        .send(vec![serde_json::to_value(empty).unwrap()])
        .unwrap();
    let requests = helper.requests();
    let address = crate::network::ServerAddress::parse("smb://nas/share").unwrap();
    let id = requests.connect(&address).unwrap();
    helper.command();
    let password = |flags: gio::AskPasswordFlags, question: u64| {
        serde_json::json!({
            "version": 1, "id": 1, "event": "question", "question": question,
            "prompt": {"kind": "password", "message": "Password required for share on nas",
                "username": "me", "domain": "WORKGROUP", "flags": flags.bits()}
        })
    };
    let answer = |save| DeviceAnswer::Password {
        username: "me".into(),
        domain: "WORKGROUP".into(),
        password: "private-secret".into(),
        anonymous: false,
        pim: 0,
        hidden_volume: false,
        system_volume: false,
        save,
    };
    let current = |requests: &DeviceRequests| {
        requests
            .snapshot()
            .into_iter()
            .find(|job| job.id == id)
            .unwrap()
            .question
            .unwrap()
    };
    helper
        .send(vec![password(
            gio::AskPasswordFlags::NEED_PASSWORD | gio::AskPasswordFlags::NEED_DOMAIN,
            1,
        )])
        .unwrap();
    let prompt = current(&requests);
    assert!(!prompt.question.password_fields().unwrap().save);
    assert!(
        requests
            .answer(prompt.token, answer(Remember::Permanently))
            .is_err(),
        "no remember choice without SAVING_SUPPORTED"
    );
    assert!(helper.commands.try_recv().is_err(), "nothing was sent");

    // Wrong credentials: the backend asks again, now able to keep them.
    helper
        .send(vec![password(
            gio::AskPasswordFlags::NEED_PASSWORD
                | gio::AskPasswordFlags::NEED_DOMAIN
                | gio::AskPasswordFlags::SAVING_SUPPORTED,
            2,
        )])
        .unwrap();
    let prompt = current(&requests);
    assert!(prompt.question.password_fields().unwrap().save);
    requests
        .answer(prompt.token, answer(Remember::Permanently))
        .unwrap();
    let command = helper.command();
    assert_eq!(command["command"]["kind"], "answer");
    assert_eq!(command["command"]["answer"]["save"], "permanently");

    // An SSH host-key question is an ordinary choice bound to its serial.
    helper
        .send(vec![serde_json::json!({
            "version": 1, "id": 1, "event": "question", "question": 3,
            "prompt": {"kind": "choice", "message": "The identity of the remote computer (box) is unknown.",
                "choices": ["Log In Anyway", "Cancel Login"], "blocked": false}
        })])
        .unwrap();
    let prompt = current(&requests);
    requests
        .answer(prompt.token, DeviceAnswer::Choice { choice: 0 })
        .unwrap();
    assert_eq!(helper.command()["command"]["answer"]["choice"], 0);

    helper
        .send(vec![serde_json::json!({
            "version": 1, "id": 1, "event": "finished",
            "outcome": {"kind": "failed", "message": "nas refused the login."}
        })])
        .unwrap();
    let job = requests
        .snapshot()
        .into_iter()
        .find(|job| job.id == id)
        .unwrap();
    assert_eq!(job.phase, DevicePhase::Finished);
    assert!(
        matches!(job.outcome, Some(DeviceOutcome::Failed { ref message }) if message.contains("refused"))
    );
    assert!(requests.mounted_root(id).is_err());
    assert!(
        requests.connect(&address).is_ok(),
        "a failed connection can be retried"
    );
}

#[test]
fn listings_and_remote_only_mounts_are_bounded_read_only_results() {
    let temporary = tempfile::tempdir().unwrap();
    let mut helper = FakeHelper::new(temporary.path());
    let remote = |generation| {
        let mut remote = frame(temporary.path(), generation);
        remote.mounts.clear();
        remote.devices[0].mount = None;
        remote.devices[0].kind = LocationKind::Phone;
        remote.devices[0].uri = Some("mtp://Pixel_7/".into());
        remote.devices[0].remote_only = true;
        remote
    };
    helper
        .send(vec![serde_json::to_value(remote(1)).unwrap()])
        .unwrap();
    let device = helper.catalog.lock().unwrap().devices[0].clone();
    assert!(device.remote_only && device.root.is_none());
    let requests = helper.requests();
    let listing = requests.enumerate("mtp://Pixel_7/").unwrap();
    assert_eq!(helper.command()["command"]["kind"], "enumerate");
    assert!(requests.listing(listing).unwrap().result.is_none());
    helper
        .send(vec![serde_json::json!({
            "version": 1, "id": 1, "event": "enumerated", "truncated": false, "error": null,
            "entries": [{"name": "DCIM", "uri": "mtp://Pixel_7/DCIM", "directory": true, "size": null}]
        })])
        .unwrap();
    let listed = requests.listing(listing).unwrap().result.unwrap().unwrap();
    assert_eq!(listed.entries[0].name, "DCIM");

    let failed = requests.enumerate("network:///").unwrap();
    helper.command();
    helper
        .send(vec![serde_json::json!({
            "version": 1, "id": 2, "event": "enumerated", "truncated": false,
            "error": "Files cannot open browsing the local network: install the gvfs package.",
            "entries": []
        })])
        .unwrap();
    assert!(
        requests
            .listing(failed)
            .unwrap()
            .result
            .unwrap()
            .unwrap_err()
            .contains("install")
    );

    // A remote-only device cannot also claim a local root.
    let mut invalid = remote(2);
    invalid.mounts = frame(temporary.path(), 2).mounts;
    invalid.devices[0].mount = Some(11);
    assert!(
        helper
            .send(vec![serde_json::to_value(invalid).unwrap()])
            .is_err()
    );
}
