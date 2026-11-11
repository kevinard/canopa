//! A fake BlueZ with one adapter and one device that pairs after the agent
//! confirms passkey 42.
use super::TestBus;
use crate::bluetooth::{AGENT_PATH, SERVICE};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering::SeqCst},
};
use zbus::{Connection, Proxy, fdo, zvariant::OwnedObjectPath};

pub const ADAPTER_PATH: &str = "/org/bluez/hci0";
pub const DEVICE_PATH: &str = "/org/bluez/hci0/dev_00_11_22_33_44_55";

/// What clients did to the fake.
#[derive(Default)]
pub struct BluezState {
    pub powered: AtomicBool,
    pub scanning: AtomicBool,
    pub paired: AtomicBool,
    pub connected: AtomicBool,
    pub trusted: AtomicBool,
    /// The unique name that registered the pairing agent.
    pub agent_owner: Mutex<Option<String>>,
}

struct Manager(Arc<BluezState>);
struct Adapter(Arc<BluezState>);
struct Device(Arc<BluezState>);

#[zbus::interface(name = "org.bluez.AgentManager1")]
impl Manager {
    fn register_agent(
        &self,
        agent: OwnedObjectPath,
        capability: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> fdo::Result<()> {
        if agent.as_str() != AGENT_PATH || capability != "KeyboardDisplay" {
            return Err(fdo::Error::InvalidArgs("unexpected agent".into()));
        }
        *self.0.agent_owner.lock().unwrap() = header.sender().map(ToString::to_string);
        Ok(())
    }
}

#[zbus::interface(name = "org.bluez.Adapter1")]
impl Adapter {
    fn start_discovery(&self) {
        self.0.scanning.store(true, SeqCst);
    }
    fn stop_discovery(&self) {
        self.0.scanning.store(false, SeqCst);
    }
    #[zbus(property)]
    fn powered(&self) -> bool {
        self.0.powered.load(SeqCst)
    }
    #[zbus(property)]
    fn set_powered(&self, powered: bool) {
        self.0.powered.store(powered, SeqCst);
    }
    #[zbus(property)]
    fn alias(&self) -> &str {
        "Test adapter"
    }
}

#[zbus::interface(name = "org.bluez.Device1")]
impl Device {
    async fn pair(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> fdo::Result<()> {
        let sender = header.sender().unwrap().to_owned();
        let agent = Proxy::new(connection, sender, AGENT_PATH, "org.bluez.Agent1").await?;
        agent
            .call::<_, _, ()>(
                "RequestConfirmation",
                &(OwnedObjectPath::try_from(DEVICE_PATH).unwrap(), 42_u32),
            )
            .await?;
        self.0.paired.store(true, SeqCst);
        Ok(())
    }
    fn connect(&self) -> fdo::Result<()> {
        if !self.0.paired.load(SeqCst) {
            return Err(fdo::Error::Failed("not paired".into()));
        }
        self.0.connected.store(true, SeqCst);
        Ok(())
    }
    fn disconnect(&self) {
        self.0.connected.store(false, SeqCst);
    }
    #[zbus(property)]
    fn adapter(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(ADAPTER_PATH).unwrap()
    }
    #[zbus(property)]
    fn address(&self) -> &str {
        "00:11:22:33:44:55"
    }
    #[zbus(property)]
    fn alias(&self) -> &str {
        "Test headphones"
    }
    #[zbus(property)]
    fn paired(&self) -> bool {
        self.0.paired.load(SeqCst)
    }
    #[zbus(property)]
    fn connected(&self) -> bool {
        self.0.connected.load(SeqCst)
    }
    #[zbus(property)]
    fn trusted(&self) -> bool {
        self.0.trusted.load(SeqCst)
    }
    #[zbus(property)]
    fn set_trusted(&self, trusted: bool) {
        self.0.trusted.store(trusted, SeqCst);
    }
}

/// BlueZ on a private bus, with an object manager at `/`.
pub struct FakeBluez {
    /// Close it to simulate BlueZ leaving.
    pub connection: Connection,
    pub state: Arc<BluezState>,
}

impl FakeBluez {
    pub async fn serve(bus: &TestBus, powered: bool) -> Self {
        let state = Arc::new(BluezState::default());
        state.powered.store(powered, SeqCst);
        let connection = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at("/", fdo::ObjectManager)
            .unwrap()
            .serve_at("/org/bluez", Manager(state.clone()))
            .unwrap()
            .serve_at(ADAPTER_PATH, Adapter(state.clone()))
            .unwrap()
            .serve_at(DEVICE_PATH, Device(state.clone()))
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .build()
            .await
            .unwrap();
        Self { connection, state }
    }
}
