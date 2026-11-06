use super::*;
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncReader};

const INPUT_SERVICE: &str = "org.canopa.Niri.RemoteInput1";
struct Bus {
    child: Child,
    address: String,
}
impl Bus {
    fn new() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        Self {
            child,
            address: address.trim().into(),
        }
    }
    async fn connect(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
struct InputService {
    pointer: Arc<std::sync::atomic::AtomicU32>,
    mapped: u32,
}
#[zbus::interface(name = "org.canopa.Niri.RemoteInput1")]
impl InputService {
    #[zbus(property)]
    fn version(&self) -> u32 {
        1
    }
    #[zbus(property)]
    fn pointer_version(&self) -> fdo::Result<u32> {
        match self.pointer.load(std::sync::atomic::Ordering::SeqCst) {
            0 => Err(fdo::Error::UnknownProperty("PointerVersion".into())),
            99 => Err(fdo::Error::Failed("pointer probe failed".into())),
            value => Ok(value),
        }
    }
    #[zbus(property)]
    fn mapped_pointer_version(&self) -> fdo::Result<u32> {
        if self.mapped == 0 {
            Err(fdo::Error::UnknownProperty("MappedPointerVersion".into()))
        } else {
            Ok(self.mapped)
        }
    }
}
struct Fixture {
    root: tempfile::TempDir,
    _bus: Bus,
    server: Connection,
    backend: Connection,
    caller: Connection,
    producer: Production,
    ipc: tokio::task::JoinHandle<()>,
    pointer_version: Arc<std::sync::atomic::AtomicU32>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.ipc.abort();
    }
}
impl Fixture {
    async fn new(mode: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let bus = Bus::new();
        let server = bus.connect().await;
        let pointer_version = Arc::new(std::sync::atomic::AtomicU32::new(match mode {
            "pointer" | "combined" | "pointer-pending" | "legacy-mapped" | "mapped" => 1,
            "broken-pointer" => 99,
            _ => 0,
        }));
        server
            .object_server()
            .at(
                "/org/canopa/Niri/RemoteInput1",
                InputService {
                    pointer: pointer_version.clone(),
                    mapped: match mode {
                        "mapped" => 2,
                        "legacy-mapped" => 1,
                        _ => 0,
                    },
                },
            )
            .await
            .unwrap();
        server.request_name(INPUT_SERVICE).await.unwrap();
        let backend = bus.connect().await;
        let caller = bus.connect().await;
        let socket = root.path().join("ipc.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let ipc = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut input = AsyncReader::new(stream);
                    let mut line = String::new();
                    if input.read_line(&mut line).await.unwrap() == 0 {
                        return;
                    }
                    assert_eq!(line, "\"Version\"\n");
                    input
                        .get_mut()
                        .write_all(b"{\"Ok\":{\"Version\":\"niri private fixture\"}}\n")
                        .await
                        .unwrap();
                });
            }
        });
        let script = r#"
import json,os,sys,time
from pathlib import Path
root=Path(sys.argv[1]);mode=sys.argv[2];kind=sys.argv[-1]
if kind=='--remote-input-worker':
    (root/'unexpected-input-worker').write_text('input started before connection')
    raise SystemExit(73)
if kind=='--remote-desktop-dialog':
    value=json.load(sys.stdin);(root/'dialog.pid').write_text(str(os.getpid()));(root/'dialog.json').write_text(json.dumps(value))
    if mode=='pointer-pending':
        while not (root/'accept').exists():time.sleep(.01)
        print('{"sources":[],"devices":2}',flush=True)
    elif mode.startswith('pending'):time.sleep(30)
    elif mode=='pointer':print('{"sources":[],"devices":2}',flush=True)
    elif mode=='combined':print('{"sources":[],"devices":3}',flush=True)
    elif mode=='cancel':print('null',flush=True)
    elif mode=='invalid':print('{"sources":[],"devices":2}',flush=True)
    elif mode=='empty':print('{"sources":[],"devices":0}',flush=True)
    else:print('{"sources":[],"devices":1}',flush=True)
elif kind=='--screencast-status':
    value=json.loads(sys.stdin.readline());(root/'controls.json').write_text(json.dumps(value))
    (root/'controls.pid').write_text(str(os.getpid()))
    print('{"ready":true}',flush=True)
    sys.stdin.read()
else:raise SystemExit(74)
"#;
        let process = Arc::new(DialogProcess {
            executable: "python3".into(),
            arguments: vec![
                "-c".into(),
                script.into(),
                root.path().as_os_str().to_owned(),
                mode.into(),
            ],
        });
        let capture = Arc::new(CaptureProduction::with_socket(
            process.clone(),
            Some(socket),
        ));
        let producer = Production::new(process, capture);
        Self {
            root,
            _bus: bus,
            server,
            backend,
            caller,
            producer,
            ipc,
            pointer_version,
        }
    }
    fn request(&self) -> StartRequest {
        StartRequest {
            app_id: "org.canopa.RemoteFixture".into(),
            caller: self.caller.unique_name().unwrap().clone(),
            parent_window: "".into(),
            modal: true,
            activation_token: None,
            devices: DeviceOptions { types: 1 },
            sources: None,
            clipboard: false,
        }
    }
    async fn file(&self, name: &str) -> PathBuf {
        let path = self.root.path().join(name);
        tokio::time::timeout(Duration::from_secs(3), async {
            while !path.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        path
    }
    async fn retired(&self, name: &str) {
        let pid = std::fs::read_to_string(self.file(name).await).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while PathBuf::from(format!("/proc/{pid}")).exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
}
#[tokio::test]
async fn availability_notifications_follow_the_live_owner_and_stop_with_registration() {
    use futures_util::StreamExt;
    let fixture = Fixture::new("mapped").await;
    let producer = Arc::new(Production::new(
        fixture.producer.process.clone(),
        fixture.producer.capture.clone(),
    ));
    let sessions = ScreenCast::with_presenters(
        fixture.producer.capture.clone(),
        producer,
        crate::requests::Requests::default(),
    );
    let remote = RemoteDesktop::new(sessions.clone());
    fixture
        .backend
        .object_server()
        .at(crate::OBJECT_PATH, remote.clone())
        .await
        .unwrap();
    let observer = fixture._bus.connect().await;
    let properties = zbus::fdo::PropertiesProxy::builder(&observer)
        .destination(fixture.backend.unique_name().unwrap().clone())
        .unwrap()
        .path(crate::OBJECT_PATH)
        .unwrap()
        .build()
        .await
        .unwrap();
    let mut changes = properties.receive_properties_changed().await.unwrap();
    let watch = remote
        .observe_available_devices(&fixture.backend)
        .await
        .unwrap();
    async fn mask(changes: &mut zbus::fdo::PropertiesChangedStream, expected: u32) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let event = changes.next().await.unwrap();
                let args = event.args().unwrap();
                if let Some(value) = args.changed_properties().get("AvailableDeviceTypes")
                    && u32::try_from(value).unwrap() == expected
                {
                    break;
                }
            }
        })
        .await
        .unwrap();
    }
    mask(&mut changes, 3).await;
    fixture
        .pointer_version
        .store(2, std::sync::atomic::Ordering::SeqCst);
    let emitter =
        zbus::object_server::SignalEmitter::new(&fixture.server, "/org/canopa/Niri/RemoteInput1")
            .unwrap();
    zbus::fdo::Properties::properties_changed(
        &emitter,
        INPUT_SERVICE.try_into().unwrap(),
        HashMap::new(),
        vec!["PointerVersion"].into(),
    )
    .await
    .unwrap();
    mask(&mut changes, 1).await;
    let endpoint = InputEndpoint::resolve(
        &fixture.backend,
        fixture.producer.capture.socket().unwrap().clone(),
    )
    .await
    .unwrap();
    fixture.server.release_name(INPUT_SERVICE).await.unwrap();
    mask(&mut changes, 0).await;
    assert!(endpoint.devices(&fixture.backend, false).await.is_err());
    watch.stop().await;
    fixture
        .pointer_version
        .store(1, std::sync::atomic::Ordering::SeqCst);
    fixture.server.request_name(INPUT_SERVICE).await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(80), changes.next())
            .await
            .is_err()
    );
    sessions.shutdown().await.unwrap();
}
#[tokio::test]
async fn pointer_negotiation_requires_live_scale_descriptors_only_for_captured_input() {
    for (mode, relative, captured) in [
        ("accept", 1, 1),
        ("pointer", 3, 1),
        ("legacy-mapped", 3, 1),
        ("mapped", 3, 3),
    ] {
        let fixture = Fixture::new(mode).await;
        let endpoint = InputEndpoint::resolve(
            &fixture.backend,
            fixture.producer.capture.socket().unwrap().clone(),
        )
        .await
        .unwrap();
        assert_eq!(
            endpoint.devices(&fixture.backend, false).await.unwrap(),
            relative
        );
        assert_eq!(
            endpoint.devices(&fixture.backend, true).await.unwrap(),
            captured
        );
        assert_eq!(
            fixture.producer.available_devices(&fixture.backend).await,
            relative
        );
        assert!(!fixture.root.path().join("dialog.json").exists());
        assert!(!fixture.root.path().join("unexpected-input-worker").exists());
    }
    let fixture = Fixture::new("broken-pointer").await;
    assert_eq!(
        fixture.producer.available_devices(&fixture.backend).await,
        0
    );
}
#[tokio::test]
async fn input_only_pointer_approval_and_zero_steps_do_not_open_a_worker() {
    for (mode, devices) in [("pointer", 2), ("combined", 3)] {
        let fixture = Fixture::new(mode).await;
        let (_cancel, cancelled) = watch::channel(false);
        let mut request = fixture.request();
        request.devices.types = 3;
        let mut active = fixture
            .producer
            .present(fixture.backend.clone(), request, cancelled.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(active.devices(), devices);
        active
            .pointer(
                PointerInput::Discrete { axis: 0, steps: 0 },
                cancelled.clone(),
            )
            .await
            .unwrap();
        active
            .pointer(
                PointerInput::Axis {
                    x: 0.,
                    y: 0.,
                    finish: false,
                },
                cancelled,
            )
            .await
            .unwrap();
        assert!(!fixture.root.path().join("unexpected-input-worker").exists());
        active.stop().await.unwrap();
        fixture.retired("controls.pid").await;
    }
}
#[tokio::test]
async fn loss_of_pointer_capability_during_consent_cannot_publish_approval() {
    let fixture = Fixture::new("pointer-pending").await;
    let (_cancel, cancelled) = watch::channel(false);
    let mut request = fixture.request();
    request.devices.types = 2;
    let opening = fixture
        .producer
        .present(fixture.backend.clone(), request, cancelled);
    tokio::pin!(opening);
    tokio::select! { _=fixture.file("dialog.json") => {}, _=&mut opening => panic!("consent returned early") }
    fixture
        .pointer_version
        .store(0, std::sync::atomic::Ordering::SeqCst);
    std::fs::write(fixture.root.path().join("accept"), "").unwrap();
    assert!(opening.await.is_err());
    fixture.retired("dialog.pid").await;
    assert!(!fixture.root.path().join("controls.json").exists());
    assert!(!fixture.root.path().join("unexpected-input-worker").exists());
}
#[tokio::test]
async fn cancellation_or_invalid_approval_never_opens_input_or_controls() {
    for mode in [
        "cancel",
        "invalid",
        "empty",
        "pending",
        "pending-compositor",
    ] {
        let fixture = Fixture::new(mode).await;
        let (cancel, cancelled) = watch::channel(false);
        let opening =
            fixture
                .producer
                .present(fixture.backend.clone(), fixture.request(), cancelled);
        tokio::pin!(opening);
        let result = if mode.starts_with("pending") {
            tokio::select! {_=fixture.file("dialog.json")=>{},_= &mut opening=>panic!("pending dialog returned")}
            if mode == "pending" {
                cancel.send_replace(true);
            } else {
                fixture.server.clone().close().await.unwrap();
            }
            opening.await
        } else {
            opening.await
        };
        if matches!(mode, "cancel" | "pending") {
            assert!(result.unwrap().is_none());
        } else {
            assert!(result.is_err());
        }
        fixture.retired("dialog.pid").await;
        assert!(!fixture.root.path().join("controls.json").exists());
        assert!(!fixture.root.path().join("unexpected-input-worker").exists());
    }
}
#[tokio::test]
async fn approved_input_only_controls_retire_on_loss_before_any_eis_connection() {
    for loss in ["compositor", "caller", "backend"] {
        let fixture = Fixture::new("accept").await;
        let (_cancel, cancelled) = watch::channel(false);
        let control = fixture
            .producer
            .present(fixture.backend.clone(), fixture.request(), cancelled)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(control.devices(), 1);
        assert!(control.streams().is_empty());
        assert!(!*control.closed().borrow());
        let details: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixture.file("controls.json").await).unwrap())
                .unwrap();
        assert_eq!(details["devices"], 1);
        assert_eq!(details["sources"], serde_json::json!([]));
        assert!(!fixture.root.path().join("unexpected-input-worker").exists());
        match loss {
            "compositor" => fixture.server.clone().close().await.unwrap(),
            "caller" => fixture.caller.clone().close().await.unwrap(),
            "backend" => fixture.backend.clone().close().await.unwrap(),
            _ => unreachable!(),
        }
        let mut closed = control.closed();
        tokio::time::timeout(Duration::from_secs(3), stopped(&mut closed))
            .await
            .unwrap();
        control.stop().await.unwrap();
        fixture.retired("controls.pid").await;
        assert!(!fixture.root.path().join("unexpected-input-worker").exists());
    }
}

#[tokio::test]
#[ignore = "requires the private RemoteDesktop compositor/Iced/libei fixture"]
async fn native_backend_fixture() {
    use std::io::{Read, Write};
    assert_eq!(
        std::env::var("CANOPA_REMOTE_DESKTOP_NATIVE").as_deref(),
        Ok("1")
    );
    let executable = std::env::var_os("CANOPA_PORTAL_BINARY").expect("native Portal executable");
    let arguments = match std::env::var_os("CANOPA_PORTAL_CONFIG") {
        Some(path) => vec!["--canopa-config".into(), path],
        None => vec!["--defaults".into()],
    };
    let process = Arc::new(DialogProcess {
        executable: executable.into(),
        arguments,
    });
    let capture = Arc::new(CaptureProduction::new(process.clone()));
    let remote = Arc::new(Production::new(process, capture.clone()));
    let sessions = ScreenCast::with_presenters(capture, remote, Requests::default());
    let remote = RemoteDesktop::new(sessions.clone());
    let server = zbus::connection::Builder::session()
        .unwrap()
        .name(crate::BUS_NAME)
        .unwrap()
        .serve_at(crate::OBJECT_PATH, sessions.clone())
        .unwrap()
        .serve_at(crate::OBJECT_PATH, remote.clone())
        .unwrap()
        .serve_at(
            crate::OBJECT_PATH,
            crate::clipboard::Clipboard::new(sessions.clone()),
        )
        .unwrap()
        .serve_at(
            crate::OBJECT_PATH,
            crate::Settings::new(canopa_theme::Appearance::default()),
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let availability = remote.observe_available_devices(&server).await.unwrap();
    println!(
        "remote-backend-ready {}",
        serde_json::json!({"pid":std::process::id(),"owner":server.unique_name().unwrap().as_str()})
    );
    std::io::stdout().flush().unwrap();
    tokio::task::spawn_blocking(|| {
        let mut byte = [0_u8];
        let _ = std::io::stdin().lock().read(&mut byte);
    })
    .await
    .unwrap();
    sessions.shutdown().await.unwrap();
    availability.stop().await;
    server.close().await.unwrap();
}
