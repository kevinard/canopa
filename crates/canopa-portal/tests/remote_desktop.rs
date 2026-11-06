use canopa_portal::{
    BUS_NAME, OBJECT_PATH, remote_desktop as remote,
    requests::{Options, Requests},
    screencast::{self, ScreenCast, Stream},
    screencast_capture::{Geometry, Source},
};
use futures_util::future::BoxFuture;
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::{fd::OwnedFd, unix::net::UnixStream},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
use zbus::{
    Connection,
    zvariant::{OwnedFd as BusFd, OwnedObjectPath},
};
const CAPTURE: &str = "org.freedesktop.impl.portal.ScreenCast";
const REMOTE: &str = "org.freedesktop.impl.portal.RemoteDesktop";
const APP: &str = "org.canopa.RemoteTest";
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
struct Interaction {
    request: remote::StartRequest,
    cancelled: watch::Receiver<bool>,
    reply: oneshot::Sender<anyhow::Result<Option<Box<dyn remote::Control>>>>,
}
struct Probe(mpsc::UnboundedSender<Interaction>);
impl remote::Presenter for Probe {
    fn present<'a>(
        &'a self,
        _: Connection,
        request: remote::StartRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn remote::Control>>>> {
        Box::pin(async move {
            let (reply, result) = oneshot::channel();
            self.0
                .send(Interaction {
                    request,
                    cancelled,
                    reply,
                })
                .unwrap();
            result.await?
        })
    }
}
struct Live {
    devices: u32,
    streams: Vec<Stream>,
    closed: watch::Sender<bool>,
    stops: Arc<AtomicUsize>,
    peer: Option<UnixStream>,
    pending: bool,
    entered: watch::Sender<bool>,
    ended: bool,
    keyboard: Arc<Mutex<Vec<remote::KeyboardInput>>>,
    keyboard_ready: Option<watch::Receiver<bool>>,
}
impl Drop for Live {
    fn drop(&mut self) {
        if !self.ended {
            self.stops.fetch_add(1, Ordering::SeqCst);
        }
    }
}
impl remote::Control for Live {
    fn devices(&self) -> u32 {
        self.devices
    }
    fn streams(&self) -> &[Stream] {
        &self.streams
    }
    fn closed(&self) -> watch::Receiver<bool> {
        self.closed.subscribe()
    }
    fn connect<'a>(
        &'a mut self,
        mut cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<OwnedFd>> {
        Box::pin(async move {
            self.entered.send_replace(true);
            if self.pending {
                while !*cancelled.borrow() {
                    cancelled.changed().await?;
                }
                anyhow::bail!("Session ended");
            }
            let (client, mut peer) = UnixStream::pair()?;
            peer.write_all(b"approved")?;
            self.peer = Some(peer);
            Ok(client.into())
        })
    }
    fn keyboard<'a>(
        &'a mut self,
        input: remote::KeyboardInput,
        mut cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, zbus::fdo::Result<()>> {
        Box::pin(async move {
            self.keyboard.lock().unwrap().push(input);
            self.entered.send_replace(true);
            if self.pending {
                while !*cancelled.borrow() {
                    cancelled
                        .changed()
                        .await
                        .map_err(|_| zbus::fdo::Error::Failed("Session ended".into()))?;
                }
                return Err(zbus::fdo::Error::Failed("Session ended".into()));
            }
            if let Some(ready) = self.keyboard_ready.as_mut() {
                while !*ready.borrow() {
                    ready
                        .changed()
                        .await
                        .map_err(|_| zbus::fdo::Error::Failed("Acknowledgement ended".into()))?;
                }
            }
            if matches!(
                input,
                remote::KeyboardInput::Keysym {
                    symbol: 0x0101_f600,
                    ..
                }
            ) {
                return Err(zbus::fdo::Error::NotSupported("Unavailable symbol".into()));
            }
            Ok(())
        })
    }
    fn stop(mut self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>> {
        self.ended = true;
        self.stops.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
}
fn grant(
    devices: u32,
    streams: Vec<Stream>,
    pending: bool,
) -> (
    Box<dyn remote::Control>,
    watch::Sender<bool>,
    Arc<AtomicUsize>,
    watch::Receiver<bool>,
) {
    let (control, closed, stops, entered, _) = observed_grant(devices, streams, pending);
    (control, closed, stops, entered)
}
type ObservedGrant = (
    Box<dyn remote::Control>,
    watch::Sender<bool>,
    Arc<AtomicUsize>,
    watch::Receiver<bool>,
    Arc<Mutex<Vec<remote::KeyboardInput>>>,
);
fn observed_grant(devices: u32, streams: Vec<Stream>, pending: bool) -> ObservedGrant {
    observed_grant_with_ready(devices, streams, pending, None)
}
fn observed_grant_with_ready(
    devices: u32,
    streams: Vec<Stream>,
    pending: bool,
    keyboard_ready: Option<watch::Receiver<bool>>,
) -> ObservedGrant {
    let (closed, _) = watch::channel(false);
    let stops = Arc::new(AtomicUsize::new(0));
    let (entered, entering) = watch::channel(false);
    let keyboard = Arc::new(Mutex::new(Vec::new()));
    (
        Box::new(Live {
            devices,
            streams,
            closed: closed.clone(),
            stops: stops.clone(),
            peer: None,
            pending,
            entered,
            ended: false,
            keyboard: keyboard.clone(),
            keyboard_ready,
        }),
        closed,
        stops,
        entering,
        keyboard,
    )
}
fn stream() -> Stream {
    Stream {
        node: 41,
        serial: 700,
        mapping: None,
        source: Source::Monitor("DP-1".into()),
        geometry: Some(Geometry {
            position: (-640, 0),
            size: (640, 480),
        }),
    }
}
fn path(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/session/remote/{name}")
        .try_into()
        .unwrap()
}
fn request(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/remote/{name}")
        .try_into()
        .unwrap()
}
fn types(value: u32) -> Options {
    Options::from([("types".into(), value.into())])
}
struct Fixture {
    bus: Bus,
    _server: Connection,
    client: Connection,
    portal: ScreenCast,
    presentations: mpsc::UnboundedReceiver<Interaction>,
}
impl Fixture {
    async fn new() -> Self {
        let bus = Bus::new();
        let (present, presentations) = mpsc::unbounded_channel();
        let portal = ScreenCast::with_presenters(
            Arc::new(screencast::Unavailable),
            Arc::new(Probe(present)),
            Requests::default(),
        );
        let server = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(BUS_NAME)
            .unwrap()
            .serve_at(OBJECT_PATH, portal.clone())
            .unwrap()
            .serve_at(OBJECT_PATH, remote::RemoteDesktop::new(portal.clone()))
            .unwrap()
            .serve_at(
                OBJECT_PATH,
                canopa_portal::clipboard::Clipboard::new(portal.clone()),
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        let client = bus.connect().await;
        Self {
            bus,
            _server: server,
            client,
            portal,
            presentations,
        }
    }
    async fn create(&self, name: &str, interface: &str) -> zbus::Result<(u32, Options)> {
        call(
            &self.client,
            interface,
            "CreateSession",
            &(
                request(&format!("create{name}")),
                path(name),
                APP,
                Options::new(),
            ),
        )
        .await
    }
    async fn select(
        &self,
        name: &str,
        interface: &str,
        method: &str,
        options: Options,
    ) -> zbus::Result<(u32, Options)> {
        call(
            &self.client,
            interface,
            method,
            &(
                request(&format!("select{method}{name}")),
                path(name),
                APP,
                options,
            ),
        )
        .await
    }
    fn start(
        &self,
        name: &str,
        interface: &str,
    ) -> tokio::task::JoinHandle<zbus::Result<(u32, Options)>> {
        let client = self.client.clone();
        let name = name.to_owned();
        let interface = interface.to_owned();
        tokio::spawn(async move {
            call(
                &client,
                &interface,
                "Start",
                &(
                    request(&format!("start{name}")),
                    path(&name),
                    APP,
                    "wayland:parent",
                    Options::new(),
                ),
            )
            .await
        })
    }
    async fn presentation(&mut self) -> Interaction {
        tokio::time::timeout(Duration::from_secs(3), self.presentations.recv())
            .await
            .unwrap()
            .unwrap()
    }
}
async fn call<B: serde::Serialize + zbus::zvariant::DynamicType>(
    connection: &Connection,
    interface: &str,
    method: &str,
    body: &B,
) -> zbus::Result<(u32, Options)> {
    tokio::time::timeout(Duration::from_secs(3), async {
        zbus::Proxy::new(connection, BUS_NAME, OBJECT_PATH, interface)
            .await?
            .call(method, body)
            .await
    })
    .await
    .unwrap()
}
async fn connect(connection: &Connection, name: &str, app: &str) -> zbus::Result<BusFd> {
    zbus::Proxy::new(connection, BUS_NAME, OBJECT_PATH, REMOTE)
        .await?
        .call("ConnectToEIS", &(path(name), app, Options::new()))
        .await
}
async fn keyboard(
    connection: &Connection,
    name: &str,
    method: &str,
    value: i32,
    state: u32,
) -> zbus::Result<()> {
    zbus::Proxy::new(connection, BUS_NAME, OBJECT_PATH, REMOTE)
        .await?
        .call(method, &(path(name), Options::new(), value, state))
        .await
}
fn error(result: zbus::Result<()>, expected: &str) {
    match result {
        Err(zbus::Error::MethodError(name, _, _)) => assert_eq!(name.as_str(), expected),
        other => panic!("Expected {expected}, received {other:?}"),
    }
}
async fn close(connection: &Connection, key: OwnedObjectPath, interface: &str) -> zbus::Result<()> {
    zbus::Proxy::new(connection, BUS_NAME, key, interface.to_owned())
        .await?
        .call("Close", &())
        .await
}
async fn retired(connection: &Connection, key: OwnedObjectPath) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let message = connection
                .call_method(
                    Some(BUS_NAME),
                    key.clone(),
                    Some("org.freedesktop.DBus.Introspectable"),
                    "Introspect",
                    &(),
                )
                .await;
            if message.is_err()
                || !message
                    .unwrap()
                    .body()
                    .deserialize::<String>()
                    .unwrap()
                    .contains("interface name=\"org.freedesktop.impl.portal.Session\"")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn keyboard_notify_checks_owner_grant_state_and_the_one_way_eis_gate() {
    let mut f = Fixture::new().await;
    f.create("keys", REMOTE).await.unwrap();
    error(
        keyboard(&f.client, "keys", "NotifyKeyboardKeycode", 42, 1).await,
        "org.freedesktop.DBus.Error.InvalidArgs",
    );
    f.select("keys", REMOTE, "SelectDevices", types(1))
        .await
        .unwrap();
    let started = f.start("keys", REMOTE);
    let request = f.presentation().await;
    error(
        keyboard(&f.client, "keys", "NotifyKeyboardKeysym", 0x41, 1).await,
        "org.freedesktop.DBus.Error.InvalidArgs",
    );
    let (control, _closed, stops, _, inputs) = observed_grant(1, vec![], false);
    request.reply.send(Ok(Some(control))).ok().unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 0);
    let foreign = f.bus.connect().await;
    error(
        keyboard(&foreign, "keys", "NotifyKeyboardKeysym", 0x41, 1).await,
        "org.freedesktop.DBus.Error.AccessDenied",
    );
    for (method, value, state) in [
        ("NotifyKeyboardKeycode", -1, 1),
        ("NotifyKeyboardKeycode", 0x300, 1),
        ("NotifyKeyboardKeycode", 42, 2),
        ("NotifyKeyboardKeysym", 0, 1),
        ("NotifyKeyboardKeysym", i32::MAX, 0),
    ] {
        error(
            keyboard(&f.client, "keys", method, value, state).await,
            "org.freedesktop.DBus.Error.InvalidArgs",
        );
    }
    assert!(inputs.lock().unwrap().is_empty());
    keyboard(&f.client, "keys", "NotifyKeyboardKeysym", 0x41, 1)
        .await
        .unwrap();
    keyboard(&f.client, "keys", "NotifyKeyboardKeysym", 0x41, 0)
        .await
        .unwrap();
    error(
        keyboard(&f.client, "keys", "NotifyKeyboardKeysym", 0x0101_f600, 1).await,
        "org.freedesktop.DBus.Error.NotSupported",
    );
    keyboard(&f.client, "keys", "NotifyKeyboardKeycode", 42, 1)
        .await
        .unwrap();
    assert_eq!(inputs.lock().unwrap().len(), 4);
    assert_eq!(
        inputs.lock().unwrap()[0],
        remote::KeyboardInput::Keysym {
            symbol: 0x41,
            pressed: true
        }
    );
    let fd = connect(&f.client, "keys", APP).await.unwrap();
    error(
        keyboard(&f.client, "keys", "NotifyKeyboardKeycode", 42, 0).await,
        "org.freedesktop.DBus.Error.AccessDenied",
    );
    assert_eq!(inputs.lock().unwrap().len(), 4);
    assert_eq!(stops.load(Ordering::SeqCst), 0);
    drop(fd);
    close(
        &f.client,
        path("keys"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    retired(&f.client, path("keys")).await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);

    f.create("viewonly", REMOTE).await.unwrap();
    f.select("viewonly", CAPTURE, "SelectSources", types(1))
        .await
        .unwrap();
    f.select("viewonly", REMOTE, "SelectDevices", types(1))
        .await
        .unwrap();
    let started = f.start("viewonly", REMOTE);
    let request = f.presentation().await;
    let (control, _closed, _, _, inputs) = observed_grant(0, vec![stream()], false);
    request.reply.send(Ok(Some(control))).ok().unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 0);
    error(
        keyboard(&f.client, "viewonly", "NotifyKeyboardKeycode", 42, 1).await,
        "org.freedesktop.DBus.Error.AccessDenied",
    );
    assert!(inputs.lock().unwrap().is_empty());
    f.create("capture", CAPTURE).await.unwrap();
    error(
        keyboard(&f.client, "capture", "NotifyKeyboardKeycode", 42, 1).await,
        "org.freedesktop.DBus.Error.AccessDenied",
    );
    f.portal.shutdown().await.unwrap();
}
#[tokio::test]
async fn pending_keyboard_notify_retires_on_session_caller_or_source_loss() {
    for reason in ["session", "caller", "source", "backend"] {
        let mut f = Fixture::new().await;
        let observer = f.bus.connect().await;
        f.create("pendingkeys", REMOTE).await.unwrap();
        f.select("pendingkeys", REMOTE, "SelectDevices", types(1))
            .await
            .unwrap();
        let started = f.start("pendingkeys", REMOTE);
        let request = f.presentation().await;
        let (control, source, stops, mut entered, inputs) = observed_grant(1, vec![], true);
        request.reply.send(Ok(Some(control))).ok().unwrap();
        assert_eq!(started.await.unwrap().unwrap().0, 0);
        let caller = f.client.clone();
        let pending = tokio::spawn(async move {
            keyboard(&caller, "pendingkeys", "NotifyKeyboardKeysym", 0x41, 1).await
        });
        tokio::time::timeout(Duration::from_secs(3), async {
            while !*entered.borrow() {
                entered.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        match reason {
            "session" => close(
                &f.client,
                path("pendingkeys"),
                "org.freedesktop.impl.portal.Session",
            )
            .await
            .unwrap(),
            "caller" => f.client.clone().close().await.unwrap(),
            "backend" => f._server.clone().close().await.unwrap(),
            _ => {
                source.send_replace(true);
            }
        }
        assert!(
            tokio::time::timeout(Duration::from_secs(3), pending)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        retired(&observer, path("pendingkeys")).await;
        tokio::time::timeout(Duration::from_secs(3), async {
            while stops.load(Ordering::SeqCst) != 1 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(inputs.lock().unwrap().len(), 1);
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn keyboard_shortcut_burst_waits_for_acknowledgements_without_retiring() {
    let mut f = Fixture::new().await;
    f.create("shortcut", REMOTE).await.unwrap();
    f.select("shortcut", REMOTE, "SelectDevices", types(1))
        .await
        .unwrap();
    let started = f.start("shortcut", REMOTE);
    let request = f.presentation().await;
    let (ready, waiting) = watch::channel(false);
    let (control, _source, stops, mut entered, inputs) =
        observed_grant_with_ready(1, vec![], false, Some(waiting));
    request.reply.send(Ok(Some(control))).ok().unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 0);
    let sequence = [
        (29_i32, 1_u32),
        (56, 1),
        (54, 1),
        (183, 1),
        (183, 0),
        (54, 0),
        (56, 0),
        (29, 0),
    ];
    let caller = f.client.clone();
    let first =
        tokio::spawn(
            async move { keyboard(&caller, "shortcut", "NotifyKeyboardKeycode", 29, 1).await },
        );
    tokio::time::timeout(Duration::from_secs(3), async {
        while !*entered.borrow() {
            entered.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    let caller = f.client.clone();
    let burst = tokio::spawn(async move {
        let proxy = zbus::Proxy::new(&caller, BUS_NAME, OBJECT_PATH, REMOTE)
            .await
            .unwrap();
        futures_util::future::join_all(sequence.into_iter().skip(1).map(|(code, state)| {
            let proxy = proxy.clone();
            async move {
                proxy
                    .call::<_, _, ()>(
                        "NotifyKeyboardKeycode",
                        &(path("shortcut"), Options::new(), code, state),
                    )
                    .await
            }
        }))
        .await
    });
    // The receiver delays acknowledgements while the client sends the whole
    // ordinary shortcut, just as the standard frontend forwards Notify calls.
    tokio::time::sleep(Duration::from_millis(50)).await;
    ready.send_replace(true);
    let mut replies = tokio::time::timeout(Duration::from_secs(3), burst)
        .await
        .unwrap()
        .unwrap();
    replies.insert(0, first.await.unwrap());
    assert!(replies.iter().all(Result::is_ok), "{replies:?}");
    assert_eq!(stops.load(Ordering::SeqCst), 0);
    assert_eq!(
        *inputs.lock().unwrap(),
        sequence.map(|(code, state)| remote::KeyboardInput::Keycode {
            code: code as u32,
            pressed: state == 1,
        })
    );
    close(
        &f.client,
        path("shortcut"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    retired(&f.client, path("shortcut")).await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn blocked_keyboard_input_cannot_grow_the_shared_command_queue() {
    let mut f = Fixture::new().await;
    f.create("boundedkeys", REMOTE).await.unwrap();
    f.select("boundedkeys", REMOTE, "SelectDevices", types(1))
        .await
        .unwrap();
    let started = f.start("boundedkeys", REMOTE);
    let request = f.presentation().await;
    let (control, _source, stops, mut entered, inputs) = observed_grant(1, vec![], true);
    request.reply.send(Ok(Some(control))).ok().unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 0);
    let mut tasks = tokio::task::JoinSet::new();
    let caller = f.client.clone();
    tasks.spawn(
        async move { keyboard(&caller, "boundedkeys", "NotifyKeyboardKeycode", 42, 1).await },
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        while !*entered.borrow() {
            entered.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    for _ in 0..65 {
        let caller = f.client.clone();
        tasks.spawn(async move {
            keyboard(&caller, "boundedkeys", "NotifyKeyboardKeycode", 42, 0).await
        });
    }
    let mut exceeded = false;
    while let Some(result) = tokio::time::timeout(Duration::from_secs(3), tasks.join_next())
        .await
        .unwrap()
    {
        let result = result.unwrap();
        if let Err(zbus::Error::MethodError(name, _, _)) = &result {
            exceeded |= name.as_str() == "org.freedesktop.DBus.Error.LimitsExceeded";
        }
        assert!(result.is_err());
    }
    assert!(exceeded);
    retired(&f.client, path("boundedkeys")).await;
    assert_eq!(inputs.lock().unwrap().len(), 1);
    assert_eq!(stops.load(Ordering::SeqCst), 1);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_keyboard_calls_preserve_wire_order_across_worker_threads() {
    let mut f = Fixture::new().await;
    f.create("ordered", REMOTE).await.unwrap();
    f.select("ordered", REMOTE, "SelectDevices", types(1))
        .await
        .unwrap();
    let started = f.start("ordered", REMOTE);
    let request = f.presentation().await;
    let (control, _source, stops, _entered, inputs) = observed_grant(1, vec![], false);
    request.reply.send(Ok(Some(control))).ok().unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 0);
    let proxy = zbus::Proxy::new(&f.client, BUS_NAME, OBJECT_PATH, REMOTE)
        .await
        .unwrap();
    let chord = [
        (29_i32, 1_u32),
        (56, 1),
        (54, 1),
        (183, 1),
        (183, 0),
        (54, 0),
        (56, 0),
        (29, 0),
    ];
    for round in 0..32 {
        inputs.lock().unwrap().clear();
        let sequence: Vec<_> = chord.into_iter().cycle().take(32).collect();
        let replies = futures_util::future::join_all(sequence.iter().map(|&(code, state)| {
            let proxy = proxy.clone();
            async move {
                proxy
                    .call::<_, _, ()>(
                        "NotifyKeyboardKeycode",
                        &(path("ordered"), Options::new(), code, state),
                    )
                    .await
            }
        }))
        .await;
        assert!(
            replies.iter().all(Result::is_ok),
            "round {round}: {replies:?}"
        );
        assert_eq!(
            *inputs.lock().unwrap(),
            sequence
                .into_iter()
                .map(|(code, state)| remote::KeyboardInput::Keycode {
                    code: code as u32,
                    pressed: state == 1
                })
                .collect::<Vec<_>>(),
            "round {round}"
        );
    }
    assert_eq!(stops.load(Ordering::SeqCst), 0);
    close(
        &f.client,
        path("ordered"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    retired(&f.client, path("ordered")).await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn pending_consent_does_not_stall_another_sessions_keyboard() {
    let mut f = Fixture::new().await;
    f.create("activekeys", REMOTE).await.unwrap();
    f.select("activekeys", REMOTE, "SelectDevices", types(1))
        .await
        .unwrap();
    let active = f.start("activekeys", REMOTE);
    let approval = f.presentation().await;
    let (control, _source, stops, _entered, inputs) = observed_grant(1, vec![], false);
    approval.reply.send(Ok(Some(control))).ok().unwrap();
    assert_eq!(active.await.unwrap().unwrap().0, 0);
    f.create("waiting", REMOTE).await.unwrap();
    let waiting = f.start("waiting", REMOTE);
    let _unanswered = f.presentation().await;
    tokio::time::timeout(
        Duration::from_secs(3),
        keyboard(&f.client, "activekeys", "NotifyKeyboardKeycode", 42, 1),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        *inputs.lock().unwrap(),
        vec![remote::KeyboardInput::Keycode {
            code: 42,
            pressed: true
        }]
    );
    assert!(!waiting.is_finished());
    close(
        &f.client,
        request("startwaiting"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    assert_eq!(waiting.await.unwrap().unwrap().0, 1);
    retired(&f.client, path("waiting")).await;
    close(
        &f.client,
        path("activekeys"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    retired(&f.client, path("activekeys")).await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn combined_session_uses_one_owner_and_start_and_transfers_only_approved_fd() {
    for sources_first in [true, false] {
        let mut f = Fixture::new().await;
        assert_eq!(f.create("combined", REMOTE).await.unwrap().0, 0);
        assert!(f.create("combined", CAPTURE).await.is_err());
        assert!(connect(&f.client, "combined", APP).await.is_err());
        if sources_first {
            assert_eq!(
                f.select("combined", CAPTURE, "SelectSources", types(1))
                    .await
                    .unwrap()
                    .0,
                0
            );
        }
        assert_eq!(
            f.select("combined", REMOTE, "SelectDevices", types(7))
                .await
                .unwrap()
                .0,
            0
        );
        if !sources_first {
            assert_eq!(
                f.select("combined", CAPTURE, "SelectSources", types(1))
                    .await
                    .unwrap()
                    .0,
                0
            );
        }
        assert!(f.start("combined", CAPTURE).await.unwrap().is_err());
        let started = f.start("combined", REMOTE);
        let interaction = f.presentation().await;
        assert_eq!(interaction.request.devices.types, 7);
        assert_eq!(interaction.request.sources.unwrap().types, 1);
        assert_eq!(
            interaction.request.caller.as_str(),
            f.client.unique_name().unwrap().as_str()
        );
        assert!(connect(&f.client, "combined", APP).await.is_err());
        let (control, _lease, stops, _entered) = grant(1, vec![stream()], false);
        interaction.reply.send(Ok(Some(control))).ok().unwrap();
        let (response, mut result) = started.await.unwrap().unwrap();
        assert_eq!(response, 0);
        assert_eq!(u32::try_from(result.remove("devices").unwrap()).unwrap(), 1);
        assert_eq!(
            u32::try_from(result.remove("persist_mode").unwrap()).unwrap(),
            0
        );
        assert!(!bool::try_from(result.remove("clipboard_enabled").unwrap()).unwrap());
        assert!(result.contains_key("streams"));
        let stranger = f.bus.connect().await;
        assert!(connect(&stranger, "combined", APP).await.is_err());
        assert!(
            connect(&f.client, "combined", "org.canopa.Other")
                .await
                .is_err()
        );
        let fd: OwnedFd = connect(&f.client, "combined", APP).await.unwrap().into();
        let mut peer = UnixStream::from(fd);
        let mut bytes = [0; 8];
        peer.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"approved");
        assert_eq!(stops.load(Ordering::SeqCst), 0);
        assert!(
            f.select("combined", CAPTURE, "SelectSources", types(1))
                .await
                .is_err()
        );
        close(
            &f.client,
            path("combined"),
            "org.freedesktop.impl.portal.Session",
        )
        .await
        .unwrap();
        retired(&f.client, path("combined")).await;
        assert_eq!(stops.load(Ordering::SeqCst), 1);
        assert_eq!(peer.read(&mut bytes).unwrap(), 0);
    }
}
#[tokio::test]
async fn cancellation_and_session_close_retire_pending_consent_or_connect() {
    let mut f = Fixture::new().await;
    f.create("cancel", REMOTE).await.unwrap();
    let started = f.start("cancel", REMOTE);
    let interaction = f.presentation().await;
    let mut cancelled = interaction.cancelled;
    close(
        &f.client,
        request("startcancel"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 1);
    retired(&f.client, path("cancel")).await;
    if !*cancelled.borrow() {
        cancelled.changed().await.unwrap();
    }
    assert!(*cancelled.borrow());
    assert!(interaction.reply.is_closed());
    f.create("pending", REMOTE).await.unwrap();
    let started = f.start("pending", REMOTE);
    let interaction = f.presentation().await;
    let (control, _lease, stops, mut entered) = grant(1, vec![], true);
    interaction.reply.send(Ok(Some(control))).ok().unwrap();
    assert_eq!(started.await.unwrap().unwrap().0, 0);
    let client = f.client.clone();
    let connecting = tokio::spawn(async move { connect(&client, "pending", APP).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        if !*entered.borrow() {
            entered.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(*entered.borrow());
    close(
        &f.client,
        path("pending"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(3), connecting)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    retired(&f.client, path("pending")).await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn invalid_grants_and_unrequested_capture_close_the_one_session() {
    for (name, devices, streams) in [
        ("devices", 2, vec![]),
        ("capture", 1, vec![stream()]),
        ("empty", 0, vec![]),
    ] {
        let mut f = Fixture::new().await;
        f.create(name, REMOTE).await.unwrap();
        f.select(name, REMOTE, "SelectDevices", types(1))
            .await
            .unwrap();
        let started = f.start(name, REMOTE);
        let interaction = f.presentation().await;
        let (control, _lease, stops, _entered) = grant(devices, streams, false);
        interaction.reply.send(Ok(Some(control))).ok().unwrap();
        assert_eq!(started.await.unwrap().unwrap().0, 2);
        retired(&f.client, path(name)).await;
        assert_eq!(stops.load(Ordering::SeqCst), 1);
        assert!(connect(&f.client, name, APP).await.is_err());
    }
}

#[tokio::test]
async fn both_session_kinds_share_capacity_and_shutdown_registry() {
    let f = Fixture::new().await;
    for index in 0..16 {
        assert_eq!(
            f.create(
                &format!("slot{index}"),
                if index % 2 == 0 { REMOTE } else { CAPTURE }
            )
            .await
            .unwrap()
            .0,
            0
        );
    }
    assert!(f.create("excessRemote", REMOTE).await.is_err());
    assert!(f.create("excessCapture", CAPTURE).await.is_err());
    close(
        &f.client,
        path("slot0"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    retired(&f.client, path("slot0")).await;
    assert_eq!(f.create("replacement", REMOTE).await.unwrap().0, 0);
    f.portal.shutdown().await.unwrap();
    retired(&f.client, path("replacement")).await;
    for index in 1..16 {
        retired(&f.client, path(&format!("slot{index}"))).await;
    }
    assert!(f.create("afterShutdown", CAPTURE).await.is_err());
    assert!(f.create("afterShutdownRemote", REMOTE).await.is_err());
}

#[tokio::test]
async fn caller_source_and_backend_loss_stop_an_input_only_session() {
    for cause in ["caller", "source", "backend"] {
        let mut f = Fixture::new().await;
        f.create(cause, REMOTE).await.unwrap();
        let started = f.start(cause, REMOTE);
        let interaction = f.presentation().await;
        assert!(interaction.request.sources.is_none());
        let (control, lease, stops, _entered) = grant(1, vec![], false);
        interaction.reply.send(Ok(Some(control))).ok().unwrap();
        assert_eq!(started.await.unwrap().unwrap().0, 0);
        match cause {
            "caller" => f.client.clone().close().await.unwrap(),
            "backend" => f._server.clone().close().await.unwrap(),
            "source" => {
                lease.send_replace(true);
            }
            _ => unreachable!(),
        }
        tokio::time::timeout(Duration::from_secs(3), async {
            while stops.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(stops.load(Ordering::SeqCst), 1);
        if cause != "backend" {
            retired(&f._server, path(cause)).await;
        }
    }
}

#[path = "remote_desktop/pointer.rs"]
mod pointer;

mod clipboard;
