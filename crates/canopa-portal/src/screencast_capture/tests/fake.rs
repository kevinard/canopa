use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
};
use tokio::sync::{Notify, Semaphore};
use zbus::{
    Connection,
    object_server::SignalEmitter,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};
type LogicalPair = (i32, i32);

const SERVICE: &str = "org.gnome.Mutter.ScreenCast";
const PATH: &str = "/org/gnome/Mutter/ScreenCast";
pub struct Bus {
    child: Child,
    pub address: String,
}
impl Bus {
    pub fn new() -> Self {
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
    pub async fn connect(&self) -> Connection {
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
#[derive(Default)]
pub struct State {
    pub stops: AtomicU32,
    pub source_version: AtomicU32,
    pub capture_owner_version: AtomicU32,
    pub monitor_identity: AtomicU64,
    pub identity_failure: AtomicBool,
    pub ignore_precondition: AtomicBool,
    pub wrong_stream_identity: AtomicBool,
    pub resize_on_start: AtomicBool,
    pub physical_scale: Mutex<Option<f64>>,
    pub identity_preconditions: Mutex<Vec<Option<u64>>>,
    pub starts: AtomicU32,
    pub recorded: Mutex<Vec<(String, u32)>>,
    pub close_on_start: AtomicBool,
    pub create_gate: Mutex<Option<Arc<Semaphore>>>,
    pub created: Notify,
    pub record_gate: Mutex<Option<Arc<Semaphore>>>,
    pub recording: Notify,
    pub created_sessions: AtomicU32,
    node: AtomicU32,
}
pub struct Fixture {
    pub bus: Bus,
    pub service: Connection,
    pub backend: Connection,
    pub caller: Connection,
    pub state: Arc<State>,
}
impl Fixture {
    pub async fn new() -> Self {
        let bus = Bus::new();
        let state = Arc::new(State::default());
        state.capture_owner_version.store(1, Ordering::SeqCst);
        let service = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .serve_at(PATH, Root(state.clone()))
            .unwrap()
            .build()
            .await
            .unwrap();
        let backend = bus.connect().await;
        let caller = bus.connect().await;
        Self {
            bus,
            service,
            backend,
            caller,
            state,
        }
    }
}
struct Root(Arc<State>);
#[zbus::interface(name = "org.gnome.Mutter.ScreenCast")]
impl Root {
    async fn create_session(
        &self,
        _properties: std::collections::HashMap<String, OwnedValue>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let id = self.0.created_sessions.fetch_add(1, Ordering::SeqCst) + 1;
        let path = format!("/org/gnome/Mutter/ScreenCast/Session/u{id}");
        connection
            .object_server()
            .at(
                path.as_str(),
                Session {
                    state: self.0.clone(),
                    streams: Mutex::new(Vec::new()),
                },
            )
            .await?;
        self.0.created.notify_one();
        let gate = self.0.create_gate.lock().unwrap().clone();
        if let Some(gate) = gate {
            let _permit = gate.acquire().await.unwrap();
        }
        Ok(path.try_into().unwrap())
    }
    #[zbus(property)]
    fn canopa_capture_owner_version(&self) -> zbus::fdo::Result<u32> {
        match self.0.capture_owner_version.load(Ordering::SeqCst) {
            0 => Err(zbus::fdo::Error::UnknownProperty(
                "Creator cleanup unavailable".into(),
            )),
            version => Ok(version),
        }
    }
    #[zbus(property)]
    fn canopa_source_version(&self) -> zbus::fdo::Result<u32> {
        if self.0.identity_failure.load(Ordering::SeqCst) {
            return Err(zbus::fdo::Error::Failed("Source queries failed".into()));
        }
        match self.0.source_version.load(Ordering::SeqCst) {
            0 => Err(zbus::fdo::Error::UnknownProperty(
                "Legacy capture service".into(),
            )),
            version => Ok(version),
        }
    }
    fn monitor_info(&self, name: &str) -> zbus::fdo::Result<(u64, LogicalPair, LogicalPair)> {
        if name != "DP-1" {
            return Err(zbus::fdo::Error::Failed("Missing monitor".into()));
        }
        Ok((
            self.0.monitor_identity.load(Ordering::SeqCst),
            (-640, 0),
            (640, 480),
        ))
    }
    #[zbus(property)]
    fn version(&self) -> i32 {
        4
    }
}
struct Session {
    state: Arc<State>,
    streams: Mutex<Vec<(OwnedObjectPath, u32)>>,
}
impl Session {
    async fn record(
        &self,
        label: String,
        properties: std::collections::HashMap<String, OwnedValue>,
        connection: &Connection,
        _emitter: &SignalEmitter<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        if label == "unavailable" {
            return Err(zbus::fdo::Error::Failed("Missing source".into()));
        }
        let mode =
            u32::try_from(properties.get("cursor-mode").unwrap().try_clone().unwrap()).unwrap();
        let expected = properties
            .get("canopa-source-id")
            .map(|v| u64::try_from(v.try_clone().unwrap()).unwrap());
        self.state
            .identity_preconditions
            .lock()
            .unwrap()
            .push(expected);
        let monitor = !label.starts_with("window:");
        self.state.recorded.lock().unwrap().push((label, mode));
        self.state.recording.notify_one();
        let gate = self.state.record_gate.lock().unwrap().clone();
        if let Some(gate) = gate {
            let _permit = gate.acquire().await.unwrap();
        }
        let identity = if monitor {
            self.state.monitor_identity.load(Ordering::SeqCst)
        } else {
            expected.unwrap_or(0)
        };
        if monitor
            && self.state.source_version.load(Ordering::SeqCst) == 1
            && !self.state.ignore_precondition.load(Ordering::SeqCst)
            && expected.is_some_and(|expected| expected != identity)
        {
            return Err(zbus::fdo::Error::Failed("Source identity changed".into()));
        }
        let index = self.state.node.fetch_add(1, Ordering::SeqCst) + 1;
        let path: OwnedObjectPath = format!("/org/gnome/Mutter/ScreenCast/Stream/u{index}")
            .try_into()
            .unwrap();
        connection
            .object_server()
            .at(
                path.clone(),
                FakeStream {
                    state: self.state.clone(),
                    identity,
                    index,
                },
            )
            .await?;
        self.streams
            .lock()
            .unwrap()
            .push((path.clone(), 100 + index));
        Ok(path)
    }
}
#[zbus::interface(name = "org.gnome.Mutter.ScreenCast.Session")]
impl Session {
    async fn record_monitor(
        &self,
        connector: String,
        properties: std::collections::HashMap<String, OwnedValue>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        self.record(connector, properties, connection, &emitter)
            .await
    }
    async fn record_window(
        &self,
        properties: std::collections::HashMap<String, OwnedValue>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let id = u64::try_from(properties.get("window-id").unwrap().try_clone().unwrap()).unwrap();
        self.record(format!("window:{id}"), properties, connection, &emitter)
            .await
    }
    async fn start(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.state.starts.fetch_add(1, Ordering::SeqCst);
        if self.state.close_on_start.load(Ordering::SeqCst) {
            Self::closed(&emitter).await?;
            return Ok(());
        }
        let streams = self.streams.lock().unwrap().clone();
        for (path, node) in streams.into_iter().rev() {
            let interface = connection
                .object_server()
                .interface::<_, FakeStream>(path)
                .await?;
            FakeStream::pipe_wire_stream_added(interface.signal_emitter(), node).await?;
        }
        Ok(())
    }
    async fn stop(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.state.stops.fetch_add(1, Ordering::SeqCst);
        let streams = std::mem::take(&mut *self.streams.lock().unwrap());
        for (path, _) in streams {
            connection
                .object_server()
                .remove::<FakeStream, _>(path)
                .await?;
        }
        Self::closed(&emitter).await?;
        connection
            .object_server()
            .remove::<Session, _>(emitter.path())
            .await?;
        Ok(())
    }
    #[zbus(signal)]
    async fn closed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}
struct FakeStream {
    state: Arc<State>,
    identity: u64,
    index: u32,
}
#[zbus::interface(name = "org.gnome.Mutter.ScreenCast.Stream")]
impl FakeStream {
    #[zbus(signal)]
    async fn pipe_wire_stream_added(emitter: &SignalEmitter<'_>, node_id: u32) -> zbus::Result<()>;
    #[zbus(property)]
    fn parameters(&self) -> std::collections::HashMap<&str, OwnedValue> {
        let size = if self.state.resize_on_start.load(Ordering::SeqCst)
            && self.state.starts.load(Ordering::SeqCst) != 0
        {
            (1024_i32, 768_i32)
        } else {
            (640_i32, 480_i32)
        };
        let mut values = std::collections::HashMap::from([
            (
                "position",
                Value::from((-640_i32, 0_i32)).try_into_owned().unwrap(),
            ),
            ("size", Value::from(size).try_into_owned().unwrap()),
        ]);
        if let Some(scale) = *self.state.physical_scale.lock().unwrap() {
            values.insert(
                "canopa-physical-scale",
                Value::from(scale).try_into_owned().unwrap(),
            );
        }
        if self.state.source_version.load(Ordering::SeqCst) != 0 {
            let identity =
                self.identity + u64::from(self.state.wrong_stream_identity.load(Ordering::SeqCst));
            values.insert(
                "mapping-id",
                Value::from(format!("canopa-test:u{}", self.index))
                    .try_into_owned()
                    .unwrap(),
            );
            values.insert(
                "canopa-source-id",
                Value::from(identity).try_into_owned().unwrap(),
            );
        }
        values
    }
}
