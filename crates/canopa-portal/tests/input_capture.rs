use canopa_portal::{
    BUS_NAME, OBJECT_PATH,
    input_capture::{
        self, Control, Event, InputCapture, Operation, Output, Presenter, StartRequest,
    },
    requests::{Options, Requests},
};
use futures_util::{StreamExt, future::BoxFuture};
use std::{
    io::{BufRead, BufReader, Read},
    os::unix::net::UnixStream,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
use zbus::{
    Connection, Proxy,
    zvariant::{OwnedObjectPath, OwnedValue},
};

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
    request: StartRequest,
    cancelled: watch::Receiver<bool>,
    reply: oneshot::Sender<Option<Box<dyn Control>>>,
}
struct Probe(mpsc::Sender<Interaction>);
impl Presenter for Probe {
    fn capabilities<'a>(&'a self, _: &'a Connection) -> BoxFuture<'a, u32> {
        Box::pin(async { 2 })
    }
    fn present<'a>(
        &'a self,
        _: Connection,
        request: StartRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn Control>>>> {
        Box::pin(async move {
            let (reply, result) = oneshot::channel();
            self.0
                .send(Interaction {
                    request,
                    cancelled,
                    reply,
                })
                .await?;
            Ok(result.await?)
        })
    }
}
struct Live {
    closed: watch::Sender<bool>,
    events: Option<mpsc::Receiver<Event>>,
    commands: Arc<Mutex<Vec<&'static str>>>,
    stops: Arc<AtomicUsize>,
    peer: Option<UnixStream>,
}
impl Control for Live {
    fn capabilities(&self) -> u32 {
        2
    }
    fn closed(&self) -> watch::Receiver<bool> {
        self.closed.subscribe()
    }
    fn events(&mut self) -> Option<mpsc::Receiver<Event>> {
        self.events.take()
    }
    fn revoke(&self) {
        self.closed.send_replace(true);
    }
    fn perform<'a>(&'a mut self, operation: Operation) -> BoxFuture<'a, zbus::fdo::Result<Output>> {
        Box::pin(async move {
            Ok(match operation {
                Operation::Connect => {
                    self.commands.lock().unwrap().push("connect");
                    let (peer, client) = UnixStream::pair().unwrap();
                    self.peer = Some(peer);
                    Output::Fd(client.into())
                }
                Operation::GetZones => Output::Zones {
                    zone_set: 7,
                    zones: vec![(100, 80, -100, 0)],
                },
                Operation::SetBarriers { .. } => Output::Barriers { failed: Vec::new() },
                Operation::Enable => {
                    self.commands.lock().unwrap().push("enable");
                    Output::Unit
                }
                Operation::Disable => {
                    self.commands.lock().unwrap().push("disable");
                    Output::Unit
                }
                Operation::Release { .. } => {
                    self.commands.lock().unwrap().push("release");
                    Output::Unit
                }
            })
        })
    }
    fn stop(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>> {
        Box::pin(async move {
            self.stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}
struct Fixture {
    bus: Bus,
    _server: Connection,
    client: Connection,
    portal: InputCapture,
    presentations: mpsc::Receiver<Interaction>,
}
fn key() -> OwnedObjectPath {
    "/org/freedesktop/portal/desktop/session/capture/test"
        .try_into()
        .unwrap()
}
fn request(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/capture/{name}")
        .try_into()
        .unwrap()
}
impl Fixture {
    async fn new() -> Self {
        let bus = Bus::new();
        let (sender, presentations) = mpsc::channel(2);
        let portal = InputCapture::with_requests(Arc::new(Probe(sender)), Requests::default());
        let server = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(BUS_NAME)
            .unwrap()
            .serve_at(OBJECT_PATH, portal.clone())
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
    async fn proxy(&self) -> Proxy<'_> {
        Proxy::new(
            &self.client,
            BUS_NAME,
            OBJECT_PATH,
            input_capture::INTERFACE,
        )
        .await
        .unwrap()
    }
    async fn create(&self) {
        let results: Options = self
            .proxy()
            .await
            .call("CreateSession2", &(key(), "app", Options::new()))
            .await
            .unwrap();
        assert!(results.is_empty());
    }
    fn start(&self) -> tokio::task::JoinHandle<zbus::Result<(u32, Options)>> {
        let client = self.client.clone();
        tokio::spawn(async move {
            let proxy =
                Proxy::new(&client, BUS_NAME, OBJECT_PATH, input_capture::INTERFACE).await?;
            proxy
                .call(
                    "Start",
                    &(
                        request("start"),
                        key(),
                        "app",
                        "wayland:parent",
                        Options::from([("capabilities".into(), OwnedValue::from(2_u32))]),
                    ),
                )
                .await
        })
    }
    async fn grant(
        &mut self,
    ) -> (
        Arc<AtomicUsize>,
        Arc<Mutex<Vec<&'static str>>>,
        mpsc::Sender<Event>,
    ) {
        let starting = self.start();
        let interaction = self.presentations.recv().await.unwrap();
        assert_eq!(interaction.request.app_id, "app");
        assert_eq!(interaction.request.parent_window, "wayland:parent");
        assert_eq!(interaction.request.capabilities, 2);
        let stops = Arc::new(AtomicUsize::new(0));
        let commands = Arc::new(Mutex::new(Vec::new()));
        let (closed, _) = watch::channel(false);
        let (sender, events) = mpsc::channel(8);
        interaction
            .reply
            .send(Some(Box::new(Live {
                closed,
                events: Some(events),
                commands: commands.clone(),
                stops: stops.clone(),
                peer: None,
            })))
            .ok()
            .unwrap();
        let (status, values) = starting.await.unwrap().unwrap();
        assert_eq!(status, 0);
        assert_eq!(
            u32::try_from(values["capabilities"].try_clone().unwrap()).unwrap(),
            2
        );
        (stops, commands, sender)
    }
}
async fn wait_stops(stops: &AtomicUsize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while stops.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(stops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn one_approved_connection_preserves_receive_order_and_directs_phase_signals() {
    let mut f = Fixture::new().await;
    f.create().await;
    assert!(
        f.proxy()
            .await
            .call::<_, _, (u32, Options)>(
                "GetZones",
                &(request("early"), key(), "app", Options::new())
            )
            .await
            .is_err()
    );
    let (stops, commands, events) = f.grant().await;
    let proxy = f.proxy().await;
    let foreign = f.bus.connect().await;
    let other = Proxy::new(&foreign, BUS_NAME, OBJECT_PATH, input_capture::INTERFACE)
        .await
        .unwrap();
    assert!(
        other
            .call::<_, _, zbus::zvariant::OwnedFd>("ConnectToEIS", &(key(), "app", Options::new()))
            .await
            .is_err()
    );
    assert!(
        proxy
            .call::<_, _, zbus::zvariant::OwnedFd>(
                "ConnectToEIS",
                &(key(), "other", Options::new())
            )
            .await
            .is_err()
    );
    let fd: zbus::zvariant::OwnedFd = proxy
        .call("ConnectToEIS", &(key(), "app", Options::new()))
        .await
        .unwrap();
    let mut client = UnixStream::from(std::os::fd::OwnedFd::from(fd));
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    assert!(
        proxy
            .call::<_, _, zbus::zvariant::OwnedFd>("ConnectToEIS", &(key(), "app", Options::new()))
            .await
            .is_err()
    );
    for name in ["Enable", "Disable", "Enable", "Release"] {
        proxy
            .call_noreply(name, &(key(), "app", Options::new()))
            .await
            .unwrap();
    }
    let zones: (u32, Options) = proxy
        .call(
            "GetZones",
            &(request("flush"), key(), "app", Options::new()),
        )
        .await
        .unwrap();
    assert_eq!(zones.0, 0);
    assert_eq!(
        *commands.lock().unwrap(),
        ["connect", "enable", "disable", "enable", "release"]
    );
    let mut activated = proxy.receive_signal("Activated").await.unwrap();
    let mut foreign_events = other.receive_signal("Activated").await.unwrap();
    events
        .send(Event::Activated {
            activation: 42,
            barrier: 7,
            position: (101.25, 4.),
        })
        .await
        .unwrap();
    let event = tokio::time::timeout(Duration::from_secs(2), activated.next())
        .await
        .unwrap()
        .unwrap();
    let (session, options): (OwnedObjectPath, Options) = event.body().deserialize().unwrap();
    assert_eq!(session, key());
    assert_eq!(
        u32::try_from(options["activation_id"].try_clone().unwrap()).unwrap(),
        42
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(30), foreign_events.next())
            .await
            .is_err()
    );
    let session = Proxy::new(
        &f.client,
        BUS_NAME,
        key(),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    session.call::<_, _, ()>("Close", &()).await.unwrap();
    wait_stops(&stops).await;
    assert_eq!(client.read(&mut [0]).unwrap(), 0);
    f.portal.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancelled_start_waits_for_presenter_retirement_before_returning_its_response() {
    let mut f = Fixture::new().await;
    f.create().await;
    let session = Proxy::new(
        &f.client,
        BUS_NAME,
        key(),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    let mut closed = session.receive_signal("Closed").await.unwrap();
    let starting = f.start();
    let mut interaction = f.presentations.recv().await.unwrap();
    let pending = Proxy::new(
        &f.client,
        BUS_NAME,
        request("start"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap();
    pending.call::<_, _, ()>("Close", &()).await.unwrap();
    assert!(!starting.is_finished());
    tokio::time::timeout(Duration::from_secs(2), async {
        while !*interaction.cancelled.borrow() {
            interaction.cancelled.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(30), closed.next())
            .await
            .is_err()
    );
    interaction.reply.send(None).ok().unwrap();
    assert_eq!(starting.await.unwrap().unwrap().0, 1);
    assert!(session.get_property::<u32>("version").await.is_err());
    assert!(
        tokio::time::timeout(Duration::from_millis(30), closed.next())
            .await
            .is_err()
    );
    f.portal.shutdown().await.unwrap();
}

#[tokio::test]
async fn losing_the_owner_retires_a_live_receiver_and_a_second_start_cannot_change_its_grant() {
    let mut f = Fixture::new().await;
    f.create().await;
    let (stops, _, _events) = f.grant().await;
    assert!(f.start().await.unwrap().is_err());
    f.client.clone().close().await.unwrap();
    wait_stops(&stops).await;
    f.portal.shutdown().await.unwrap();
}

#[tokio::test]
async fn invalid_deprecated_create_retires_its_provisional_session_before_returning() {
    let f = Fixture::new().await;
    assert!(
        f.proxy()
            .await
            .call::<_, _, (u32, Options)>(
                "CreateSession",
                &(request("invalid"), key(), "app", "", Options::new())
            )
            .await
            .is_err()
    );
    f.create().await;
    f.portal.shutdown().await.unwrap();
}

#[tokio::test]
async fn clipboard_request_is_ordered_before_start_and_unapproved_access_stays_denied() {
    let mut f = Fixture::new().await;
    let sharing = canopa_portal::screencast::ScreenCast::with_requests(
        Arc::new(canopa_portal::screencast::Unavailable),
        Requests::default(),
    );
    f._server
        .object_server()
        .at(
            OBJECT_PATH,
            canopa_portal::clipboard::Clipboard::with_input_capture(sharing, f.portal.clone()),
        )
        .await
        .unwrap();
    f.create().await;
    let foreign = f.bus.connect().await;
    let other = Proxy::new(
        &foreign,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Clipboard",
    )
    .await
    .unwrap();
    let denied = other
        .call::<_, _, ()>("RequestClipboard", &(key(), Options::new()))
        .await
        .unwrap_err();
    assert!(denied.to_string().contains("AccessDenied"));
    let clipboard = Proxy::new(
        &f.client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Clipboard",
    )
    .await
    .unwrap();
    clipboard
        .call_noreply("RequestClipboard", &(key(), Options::new()))
        .await
        .unwrap();
    let starting = f.start();
    let interaction = f.presentations.recv().await.unwrap();
    assert!(
        interaction.request.clipboard,
        "Start overtook the receive-ordered clipboard request"
    );
    let (closed, _) = watch::channel(false);
    let (_events, events) = mpsc::channel(8);
    interaction
        .reply
        .send(Some(Box::new(Live {
            closed,
            events: Some(events),
            commands: Arc::new(Mutex::new(Vec::new())),
            stops: Arc::new(AtomicUsize::new(0)),
            peer: None,
        })))
        .ok()
        .unwrap();
    let (status, values) = starting.await.unwrap().unwrap();
    assert_eq!(status, 0);
    assert!(!bool::try_from(values["clipboard_enabled"].try_clone().unwrap()).unwrap());
    assert!(
        clipboard
            .call::<_, _, ()>("RequestClipboard", &(key(), Options::new()))
            .await
            .is_err()
    );
    assert!(
        clipboard
            .call::<_, _, zbus::zvariant::OwnedFd>("SelectionRead", &(key(), "text/plain"))
            .await
            .is_err()
    );
    f.portal.shutdown().await.unwrap();
}
