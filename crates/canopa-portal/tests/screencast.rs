use canopa_portal::{
    BUS_NAME, OBJECT_PATH, Settings,
    requests::{Options, Requests},
    screencast::{Capture, Presenter, ScreenCast, StartRequest, Stream},
    screencast_capture::{Cursor, Geometry, Source},
};
use futures_util::{StreamExt, future::BoxFuture};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
use zbus::{
    Connection,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};
const INTERFACE: &str = "org.freedesktop.impl.portal.ScreenCast";
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
    reply: oneshot::Sender<anyhow::Result<Option<Box<dyn Capture>>>>,
}
struct Probe(mpsc::UnboundedSender<Interaction>);
impl Presenter for Probe {
    fn present<'a>(
        &'a self,
        _: Connection,
        request: StartRequest,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn Capture>>>> {
        Box::pin(async move {
            let (reply, result) = oneshot::channel();
            self.0.send(Interaction { request, reply }).unwrap();
            result.await?
        })
    }
}
struct Live {
    streams: Vec<Stream>,
    closed: watch::Sender<bool>,
    stops: Arc<AtomicUsize>,
    ended: bool,
}
impl Drop for Live {
    fn drop(&mut self) {
        if !self.ended {
            self.stops.fetch_add(1, Ordering::SeqCst);
        }
    }
}
impl Capture for Live {
    fn streams(&self) -> &[Stream] {
        &self.streams
    }
    fn closed(&self) -> watch::Receiver<bool> {
        self.closed.subscribe()
    }
    fn stop(mut self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>> {
        self.ended = true;
        self.stops.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
}
fn grant(streams: Vec<Stream>) -> (Box<dyn Capture>, watch::Sender<bool>, Arc<AtomicUsize>) {
    let (closed, _) = watch::channel(false);
    let stops = Arc::new(AtomicUsize::new(0));
    (
        Box::new(Live {
            streams,
            closed: closed.clone(),
            stops: stops.clone(),
            ended: false,
        }),
        closed,
        stops,
    )
}
fn stream(node: u32, serial: u64) -> Stream {
    Stream {
        node,
        serial,
        mapping: None,
        source: Source::Monitor("DP-1".into()),
        geometry: Some(Geometry {
            position: (-640, 0),
            size: (640, 480),
        }),
    }
}
fn path(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/session/cast/{name}")
        .try_into()
        .unwrap()
}
fn request(name: &str) -> OwnedObjectPath {
    format!("/org/freedesktop/portal/desktop/request/cast/{name}")
        .try_into()
        .unwrap()
}
fn value(value: impl Into<Value<'static>>) -> OwnedValue {
    value.into().try_into_owned().unwrap()
}
struct Fixture {
    bus: Bus,
    server: Connection,
    client: Connection,
    portal: ScreenCast,
    presentations: mpsc::UnboundedReceiver<Interaction>,
}
impl Fixture {
    async fn new() -> Self {
        let bus = Bus::new();
        let (present, presentations) = mpsc::unbounded_channel();
        let portal = ScreenCast::with_requests(Arc::new(Probe(present)), Requests::default());
        let server = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(BUS_NAME)
            .unwrap()
            .serve_at(OBJECT_PATH, portal.clone())
            .unwrap()
            .serve_at(
                OBJECT_PATH,
                Settings::new(canopa_theme::Appearance::default()),
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        let client = bus.connect().await;
        Self {
            bus,
            server,
            client,
            portal,
            presentations,
        }
    }
    async fn create(&self, name: &str) -> (u32, Options) {
        call(
            &self.client,
            "CreateSession",
            &(
                request(&format!("create{name}")),
                path(name),
                "org.canopa.CastTest",
                Options::new(),
            ),
        )
        .await
        .unwrap()
    }
    async fn select(&self, name: &str, options: Options) -> zbus::Result<(u32, Options)> {
        call(
            &self.client,
            "SelectSources",
            &(
                request(&format!("select{name}")),
                path(name),
                "org.canopa.CastTest",
                options,
            ),
        )
        .await
    }
    fn start(&self, name: &str) -> tokio::task::JoinHandle<zbus::Result<(u32, Options)>> {
        let client = self.client.clone();
        let name = name.to_owned();
        tokio::spawn(async move {
            call(
                &client,
                "Start",
                &(
                    request(&format!("start{name}")),
                    path(&name),
                    "org.canopa.CastTest",
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
    method: &str,
    body: &B,
) -> zbus::Result<(u32, Options)> {
    tokio::time::timeout(Duration::from_secs(3), async {
        zbus::Proxy::new(connection, BUS_NAME, OBJECT_PATH, INTERFACE)
            .await?
            .call(method, body)
            .await
    })
    .await
    .unwrap()
}
async fn close(connection: &Connection, key: OwnedObjectPath) -> zbus::Result<()> {
    zbus::Proxy::new(
        connection,
        BUS_NAME,
        key,
        "org.freedesktop.impl.portal.Session",
    )
    .await?
    .call("Close", &())
    .await
}
async fn retired(connection: &Connection, key: OwnedObjectPath) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let xml = connection
                .call_method(
                    Some(BUS_NAME),
                    key.clone(),
                    Some("org.freedesktop.DBus.Introspectable"),
                    "Introspect",
                    &(),
                )
                .await;
            match xml {
                Ok(message) => {
                    if !message
                        .body()
                        .deserialize::<String>()
                        .unwrap()
                        .contains("interface name=\"org.freedesktop.impl.portal.Session\"")
                    {
                        break;
                    }
                }
                Err(zbus::Error::MethodError(name, _, _))
                    if name.as_str() == "org.freedesktop.DBus.Error.UnknownObject" =>
                {
                    break;
                }
                Err(error) => panic!("unexpected retirement error: {error}"),
            };
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn exact_session_contract_keeps_live_capture_after_start_and_settings_responsive() {
    let mut fixture = Fixture::new().await;
    let proxy = zbus::Proxy::new(&fixture.client, BUS_NAME, OBJECT_PATH, INTERFACE)
        .await
        .unwrap();
    assert_eq!(proxy.get_property::<u32>("version").await.unwrap(), 6);
    assert_eq!(
        proxy
            .get_property::<u32>("AvailableSourceTypes")
            .await
            .unwrap(),
        if cfg!(feature = "desktop") { 3 } else { 0 }
    );
    let cursor_modes = proxy
        .get_property::<u32>("AvailableCursorModes")
        .await
        .unwrap();
    if cfg!(feature = "desktop") {
        assert!(matches!(cursor_modes, 3 | 7));
    } else {
        assert_eq!(cursor_modes, 0);
    }
    assert_eq!(fixture.create("one").await.0, 0);
    assert!(fixture.presentations.try_recv().is_err());
    let session = zbus::Proxy::new(
        &fixture.client,
        BUS_NAME,
        path("one"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    assert_eq!(session.get_property::<u32>("version").await.unwrap(), 1);
    let mut closed = session.receive_signal("Closed").await.unwrap();
    assert_eq!(
        fixture
            .select(
                "one",
                Options::from([
                    ("types".into(), value(3_u32)),
                    ("multiple".into(), value(true)),
                    // The session-lifetime contract must also run where Metadata
                    // is unavailable. Its version policy has separate coverage.
                    ("cursor_mode".into(), value(2_u32)),
                    ("persist_mode".into(), value(2_u32)),
                    ("restore_data".into(), value("unrecognized restore payload"))
                ])
            )
            .await
            .unwrap()
            .0,
        0
    );
    let pending = fixture.start("one");
    let presentation = fixture.presentation().await;
    assert!(
        presentation.request.options.multiple
            && matches!(presentation.request.options.cursor, Cursor::Embedded)
    );
    assert_eq!(presentation.request.options.types, 3);
    assert_eq!(presentation.request.parent_window, "wayland:parent");
    let settings = zbus::Proxy::new(
        &fixture.client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.impl.portal.Settings",
    )
    .await
    .unwrap();
    let values: std::collections::HashMap<String, Options> = tokio::time::timeout(
        Duration::from_secs(3),
        settings.call("ReadAll", &(vec!["org.freedesktop.appearance"],)),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(values.contains_key("org.freedesktop.appearance"));
    let stranger = fixture.bus.connect().await;
    assert!(
        matches!(close(&stranger,path("one")).await,Err(zbus::Error::MethodError(name,_,_)) if name.as_str()=="org.freedesktop.DBus.Error.AccessDenied")
    );
    let (capture, _source, stops) = grant(vec![
        stream(101, 9001),
        Stream {
            node: 102,
            serial: 9002,
            mapping: None,
            source: Source::Window(42),
            geometry: None,
        },
    ]);
    presentation.reply.send(Ok(Some(capture))).ok().unwrap();
    let (code, mut values) = pending.await.unwrap().unwrap();
    assert_eq!(code, 0);
    assert_eq!(
        u32::try_from(values.remove("persist_mode").unwrap()).unwrap(),
        0
    );
    let streams = values.remove("streams").unwrap();
    assert_eq!(streams.value_signature().to_string(), "a(ua{sv})");
    let rows: Vec<(u32, Options)> = streams.try_into().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, 101);
    assert_eq!(
        rows[0].1["pipewire-serial"].value_signature().to_string(),
        "t"
    );
    assert_eq!(
        u64::try_from(rows[0].1["pipewire-serial"].try_clone().unwrap()).unwrap(),
        9001
    );
    assert!(!rows[1].1.contains_key("size"));
    tokio::task::yield_now().await;
    assert_eq!(stops.load(Ordering::SeqCst), 0);
    assert!(
        call(
            &fixture.client,
            "Start",
            &(
                request("repeat"),
                path("one"),
                "org.canopa.CastTest",
                "",
                Options::new()
            )
        )
        .await
        .is_err()
    );
    assert_eq!(stops.load(Ordering::SeqCst), 0);
    close(&fixture.client, path("one")).await.unwrap();
    retired(&fixture.client, path("one")).await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(30), closed.next())
            .await
            .is_err()
    );
    fixture.portal.shutdown().await.unwrap();
    fixture.server.close().await.unwrap();
}

#[tokio::test]
async fn request_close_session_close_and_owner_loss_cancel_pending_consent_without_a_grant() {
    let mut fixture = Fixture::new().await;
    assert_eq!(fixture.create("cancel").await.0, 0);
    fixture.select("cancel", Options::new()).await.unwrap();
    let pending = fixture.start("cancel");
    let mut presentation = fixture.presentation().await;
    zbus::Proxy::new(
        &fixture.client,
        BUS_NAME,
        request("startcancel"),
        "org.freedesktop.impl.portal.Request",
    )
    .await
    .unwrap()
    .call::<_, _, ()>("Close", &())
    .await
    .unwrap();
    assert_eq!(pending.await.unwrap().unwrap().0, 1);
    tokio::time::timeout(Duration::from_secs(3), presentation.reply.closed())
        .await
        .unwrap();
    retired(&fixture.client, path("cancel")).await;

    assert_eq!(fixture.create("close").await.0, 0);
    fixture.select("close", Options::new()).await.unwrap();
    let pending = fixture.start("close");
    let mut presentation = fixture.presentation().await;
    close(&fixture.client, path("close")).await.unwrap();
    assert_eq!(pending.await.unwrap().unwrap().0, 1);
    tokio::time::timeout(Duration::from_secs(3), presentation.reply.closed())
        .await
        .unwrap();
    retired(&fixture.client, path("close")).await;

    assert_eq!(fixture.create("lost").await.0, 0);
    fixture.select("lost", Options::new()).await.unwrap();
    let pending = fixture.start("lost");
    let mut presentation = fixture.presentation().await;
    fixture.client.clone().close().await.unwrap();
    let _ = pending.await;
    tokio::time::timeout(Duration::from_secs(3), presentation.reply.closed())
        .await
        .unwrap();
    let observer = fixture.bus.connect().await;
    retired(&observer, path("lost")).await;
    fixture.portal.shutdown().await.unwrap();
    fixture.server.close().await.unwrap();
}

#[tokio::test]
async fn malformed_options_identity_and_invalid_streams_cannot_publish_or_strand_sessions() {
    let mut fixture = Fixture::new().await;
    assert_eq!(fixture.create("bad").await.0, 0);
    let wrong = call(
        &fixture.client,
        "SelectSources",
        &(
            request("wrongapp"),
            path("bad"),
            "org.canopa.Other",
            Options::new(),
        ),
    )
    .await;
    assert!(
        matches!(wrong,Err(zbus::Error::MethodError(name,_,_)) if name.as_str()=="org.freedesktop.DBus.Error.AccessDenied")
    );
    assert!(
        fixture
            .select("bad", Options::from([("cursor_mode".into(), value(3_u32))]))
            .await
            .is_err()
    );
    retired(&fixture.client, path("bad")).await;
    assert!(fixture.presentations.try_recv().is_err());
    for (name, streams) in [
        ("serial", vec![stream(101, 0)]),
        ("count", vec![stream(101, 9001), stream(102, 9002)]),
        (
            "types",
            vec![Stream {
                node: 101,
                serial: 9001,
                mapping: None,
                source: Source::Window(1),
                geometry: None,
            }],
        ),
    ] {
        assert_eq!(fixture.create(name).await.0, 0);
        fixture.select(name, Options::new()).await.unwrap();
        let pending = fixture.start(name);
        let presentation = fixture.presentation().await;
        let (capture, _closed, stops) = grant(streams);
        presentation.reply.send(Ok(Some(capture))).ok().unwrap();
        assert_eq!(pending.await.unwrap().unwrap().0, 2);
        retired(&fixture.client, path(name)).await;
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }
    fixture.portal.shutdown().await.unwrap();
    fixture.server.close().await.unwrap();
}

#[tokio::test]
async fn compositor_loss_closes_active_capture_and_idle_sessions_share_a_bounded_owner() {
    let mut fixture = Fixture::new().await;
    assert_eq!(fixture.create("active").await.0, 0);
    fixture.select("active", Options::new()).await.unwrap();
    let session = zbus::Proxy::new(
        &fixture.client,
        BUS_NAME,
        path("active"),
        "org.freedesktop.impl.portal.Session",
    )
    .await
    .unwrap();
    let mut closed = session.receive_signal("Closed").await.unwrap();
    let pending = fixture.start("active");
    let presentation = fixture.presentation().await;
    let (capture, source, stops) = grant(vec![stream(101, 9001)]);
    presentation.reply.send(Ok(Some(capture))).ok().unwrap();
    assert_eq!(pending.await.unwrap().unwrap().0, 0);
    source.send_replace(true);
    let signal = tokio::time::timeout(Duration::from_secs(3), closed.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(signal.body().signature().to_string(), "");
    retired(&fixture.client, path("active")).await;
    assert_eq!(stops.load(Ordering::SeqCst), 1);
    for index in 0..16 {
        assert_eq!(fixture.create(&format!("idle{index}")).await.0, 0);
    }
    let limit = call(
        &fixture.client,
        "CreateSession",
        &(
            request("limit"),
            path("limit"),
            "org.canopa.CastTest",
            Options::new(),
        ),
    )
    .await;
    assert!(
        matches!(limit,Err(zbus::Error::MethodError(name,_,_)) if name.as_str()=="org.freedesktop.DBus.Error.LimitsExceeded")
    );
    close(&fixture.client, path("idle0")).await.unwrap();
    retired(&fixture.client, path("idle0")).await;
    assert_eq!(fixture.create("replacement").await.0, 0);
    fixture.portal.shutdown().await.unwrap();
    retired(&fixture.client, path("replacement")).await;
    fixture.server.close().await.unwrap();
}

#[path = "screencast/mapping.rs"]
mod mapping;
