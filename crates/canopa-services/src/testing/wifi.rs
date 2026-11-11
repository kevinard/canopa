//! Fake NetworkManager and iwd Wi-Fi services: one managed Wi-Fi device with
//! one WPA-Personal network called "Home" at [`AP`].
//!
//! The fakes assert the requests the backends make (for example the stored
//! passphrase `test-passphrase`), so a wrong request fails the D-Bus call.
use super::TestBus;
use crate::network::wifi::{
    WifiConnectRequest, WifiSecurity,
    backends::{ManagedObjects, Settings, nm_settings},
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

pub fn path(value: &str) -> OwnedObjectPath {
    value.try_into().unwrap()
}
pub const DEVICE: &str = "/devices/wifi";
pub const AP: &str = "/accesspoints/home";
pub const PROFILE: &str = "/settings/home";
pub const ACTIVE: &str = "/connections/home";

struct Manager {
    runtime: tokio::runtime::Handle,
    delay_reply: bool,
    created: Arc<AtomicBool>,
    deactivated: Arc<AtomicBool>,
    selected_ap: Arc<Mutex<Option<OwnedObjectPath>>>,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl Manager {
    fn deactivate_connection(&self, _active: OwnedObjectPath) {
        self.deactivated.store(true, Ordering::SeqCst);
    }
    fn get_devices(&self) -> Vec<OwnedObjectPath> {
        vec![path(DEVICE)]
    }
    fn activate_connection(
        &self,
        connection: OwnedObjectPath,
        device: OwnedObjectPath,
        specific_object: OwnedObjectPath,
    ) -> OwnedObjectPath {
        assert_eq!(connection, path(PROFILE));
        assert_eq!(device, path(DEVICE));
        *self.selected_ap.lock().unwrap() = Some(specific_object);
        path(ACTIVE)
    }
    async fn add_and_activate_connection2(
        &self,
        connection: Settings,
        device: OwnedObjectPath,
        specific_object: OwnedObjectPath,
        options: HashMap<String, OwnedValue>,
    ) -> (
        OwnedObjectPath,
        OwnedObjectPath,
        HashMap<String, OwnedValue>,
    ) {
        assert_eq!(device, path(DEVICE));
        *self.selected_ap.lock().unwrap() = Some(specific_object);
        assert_eq!(<&str>::try_from(&options["persist"]).unwrap(), "memory");
        assert_eq!(
            <&str>::try_from(&connection["802-11-wireless-security"]["psk"]).unwrap(),
            "test-passphrase"
        );
        self.created.store(true, Ordering::SeqCst);
        if self.delay_reply {
            self.runtime
                .spawn(async {
                    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                })
                .await
                .unwrap();
        }
        (path(PROFILE), path(ACTIVE), HashMap::new())
    }
}
struct Device;
#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl Device {
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        2
    }
    #[zbus(property)]
    fn managed(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        100
    }
    #[zbus(property)]
    fn available_connections(&self) -> Vec<OwnedObjectPath> {
        vec![path(PROFILE)]
    }
}
struct Wireless {
    aps: Arc<Mutex<Vec<OwnedObjectPath>>>,
    last: Arc<AtomicI64>,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl Wireless {
    async fn request_scan(
        &self,
        _options: HashMap<String, OwnedValue>,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.last.fetch_add(1, Ordering::SeqCst);
        self.last_scan_changed(&emitter).await?;
        Ok(())
    }
    #[zbus(property)]
    fn last_scan(&self) -> i64 {
        self.last.load(Ordering::SeqCst)
    }
    #[zbus(property)]
    fn access_points(&self) -> Vec<OwnedObjectPath> {
        self.aps.lock().unwrap().clone()
    }
    #[zbus(property)]
    fn active_access_point(&self) -> OwnedObjectPath {
        path(AP)
    }
}
pub struct AccessPoint {
    pub ssid: Vec<u8>,
    pub rsn: u32,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl AccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        self.ssid.clone()
    }
    #[zbus(property)]
    fn strength(&self) -> u8 {
        75
    }
    #[zbus(property)]
    fn flags(&self) -> u32 {
        1
    }
    #[zbus(property)]
    fn wpa_flags(&self) -> u32 {
        0
    }
    #[zbus(property)]
    fn rsn_flags(&self) -> u32 {
        self.rsn
    }
}
struct Profile {
    deleted: Arc<AtomicBool>,
    saved: Arc<AtomicBool>,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings.Connection")]
impl Profile {
    fn get_settings(&self) -> Settings {
        nm_settings(
            &request(None),
            b"Home".to_vec(),
            WifiSecurity::Personal,
            "wpa-psk",
        )
    }
    fn save(&self) {
        self.saved.store(true, Ordering::SeqCst);
    }
    fn delete(&self) {
        self.deleted.store(true, Ordering::SeqCst);
    }
}
struct Active {
    state: u32,
    observed: Arc<AtomicBool>,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl Active {
    #[zbus(property)]
    fn state(&self) -> u32 {
        self.observed.store(true, Ordering::SeqCst);
        self.state
    }
}
pub fn request(secret: Option<&str>) -> WifiConnectRequest {
    WifiConnectRequest {
        device_id: "fixture-device".into(),
        network_id: Some(AP.into()),
        label: "Home".into(),
        credential: secret.map(String::from),
        hidden: false,
    }
}

pub type AgentAddress = Arc<std::sync::Mutex<Option<(String, OwnedObjectPath)>>>;
struct IwdManager;
#[zbus::interface(name = "org.freedesktop.DBus.ObjectManager")]
impl IwdManager {
    fn get_managed_objects(&self) -> ManagedObjects {
        HashMap::from([(
            path(DEVICE),
            HashMap::from([("net.connman.iwd.Station".into(), HashMap::new())]),
        )])
    }
}
struct IwdAgents(AgentAddress);
#[zbus::interface(name = "net.connman.iwd.AgentManager")]
impl IwdAgents {
    fn register_agent(
        &self,
        path: OwnedObjectPath,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) {
        *self.0.lock().unwrap() = Some((header.sender().unwrap().to_string(), path));
    }
    fn unregister_agent(&self, path: OwnedObjectPath) {
        let mut address = self.0.lock().unwrap();
        if address
            .as_ref()
            .is_some_and(|(_, current)| *current == path)
        {
            *address = None;
        }
    }
}
pub async fn ask_secret(conn: &zbus::Connection, address: &AgentAddress) -> zbus::fdo::Result<()> {
    let (owner, agent) = address
        .lock()
        .unwrap()
        .clone()
        .expect("agent registered before connect");
    let proxy = zbus::Proxy::new(conn, owner, agent, "net.connman.iwd.Agent").await?;
    let secret: String = proxy.call("RequestPassphrase", &(path(AP),)).await?;
    assert_eq!(secret, "test-passphrase");
    Ok(())
}
struct IwdStation {
    scanning: AtomicBool,
    address: AgentAddress,
}
#[zbus::interface(name = "net.connman.iwd.Station")]
impl IwdStation {
    #[zbus(property)]
    fn state(&self) -> &str {
        "connecting"
    }
    #[zbus(property)]
    fn connected_network(&self) -> OwnedObjectPath {
        path(AP)
    }
    #[zbus(property)]
    fn scanning(&self) -> bool {
        self.scanning.load(Ordering::SeqCst)
    }
    fn get_ordered_networks(&self) -> Vec<(OwnedObjectPath, i16)> {
        vec![(path(AP), -5500)]
    }
    async fn scan(
        &self,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.scanning.store(true, Ordering::SeqCst);
        self.scanning_changed(&emitter).await?;
        self.scanning.store(false, Ordering::SeqCst);
        self.scanning_changed(&emitter).await?;
        Ok(())
    }
    async fn connect_hidden_network(
        &self,
        ssid: &str,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        assert_eq!(ssid, "Home");
        ask_secret(conn, &self.address).await
    }
}
pub struct IwdNetwork {
    pub name: String,
    pub kind: String,
    pub device: OwnedObjectPath,
    address: AgentAddress,
}
#[zbus::interface(name = "net.connman.iwd.Network")]
impl IwdNetwork {
    #[zbus(property)]
    fn name(&self) -> &str {
        &self.name
    }
    #[zbus(property, name = "Type")]
    fn network_type(&self) -> &str {
        &self.kind
    }
    #[zbus(property)]
    fn device(&self) -> OwnedObjectPath {
        self.device.clone()
    }
    #[zbus(property)]
    fn connected(&self) -> bool {
        true
    }
    async fn connect(&self, #[zbus(connection)] conn: &zbus::Connection) -> zbus::fdo::Result<()> {
        ask_secret(conn, &self.address).await
    }
}

/// A NetworkManager Wi-Fi fake. `active_state` is the active connection's
/// `State` (1 activating, 2 activated); `delay_reply` delays
/// `AddAndActivateConnection2` so a caller can cancel before the reply.
pub struct NetworkManager {
    pub service: zbus::Connection,
    pub created: Arc<AtomicBool>,
    pub saved: Arc<AtomicBool>,
    pub deleted: Arc<AtomicBool>,
    pub deactivated: Arc<AtomicBool>,
    /// Set once a client read the active connection's state.
    pub observed: Arc<AtomicBool>,
    /// The device's `LastScan`, increased by each `RequestScan`.
    pub last: Arc<AtomicI64>,
    pub aps: Arc<Mutex<Vec<OwnedObjectPath>>>,
    /// The access point of the last activation.
    pub selected_ap: Arc<Mutex<Option<OwnedObjectPath>>>,
}

impl NetworkManager {
    pub async fn serve(bus: &TestBus, active_state: u32, delay_reply: bool) -> Self {
        let service = bus.connect().await;
        let created = Arc::new(AtomicBool::new(false));
        let saved = Arc::new(AtomicBool::new(false));
        let deleted = Arc::new(AtomicBool::new(false));
        let deactivated = Arc::new(AtomicBool::new(false));
        let observed = Arc::new(AtomicBool::new(false));
        let last = Arc::new(AtomicI64::new(0));
        let aps = Arc::new(Mutex::new(vec![path(AP)]));
        let selected_ap = Arc::new(Mutex::new(None));
        let server = service.object_server();
        server
            .at(
                "/org/freedesktop/NetworkManager",
                Manager {
                    runtime: tokio::runtime::Handle::current(),
                    delay_reply,
                    created: created.clone(),
                    deactivated: deactivated.clone(),
                    selected_ap: selected_ap.clone(),
                },
            )
            .await
            .unwrap();
        server.at(DEVICE, Device).await.unwrap();
        server
            .at(
                DEVICE,
                Wireless {
                    last: last.clone(),
                    aps: aps.clone(),
                },
            )
            .await
            .unwrap();
        server
            .at(
                AP,
                AccessPoint {
                    ssid: b"Home".to_vec(),
                    rsn: 0x100,
                },
            )
            .await
            .unwrap();
        server
            .at(
                PROFILE,
                Profile {
                    saved: saved.clone(),
                    deleted: deleted.clone(),
                },
            )
            .await
            .unwrap();
        server
            .at(
                ACTIVE,
                Active {
                    state: active_state,
                    observed: observed.clone(),
                },
            )
            .await
            .unwrap();
        service
            .request_name("org.freedesktop.NetworkManager")
            .await
            .unwrap();
        Self {
            service,
            created,
            saved,
            deleted,
            deactivated,
            observed,
            last,
            aps,
            selected_ap,
        }
    }
}

/// An iwd fake whose agent manager records the registered agent in `address`.
pub struct Iwd {
    pub service: zbus::Connection,
    pub address: AgentAddress,
}

impl Iwd {
    pub async fn serve(bus: &TestBus) -> Self {
        let service = bus.connect().await;
        let address: AgentAddress = Arc::new(std::sync::Mutex::new(None));
        let server = service.object_server();
        server.at("/", IwdManager).await.unwrap();
        server
            .at("/net/connman/iwd", IwdAgents(address.clone()))
            .await
            .unwrap();
        server
            .at(
                DEVICE,
                IwdStation {
                    scanning: AtomicBool::new(false),
                    address: address.clone(),
                },
            )
            .await
            .unwrap();
        server
            .at(
                AP,
                IwdNetwork {
                    address: address.clone(),
                    name: "Home".into(),
                    kind: "psk".into(),
                    device: path(DEVICE),
                },
            )
            .await
            .unwrap();
        service.request_name("net.connman.iwd").await.unwrap();
        Self { service, address }
    }

    /// Whether a client's agent is registered.
    pub fn agent_registered(&self) -> bool {
        self.address.lock().unwrap().is_some()
    }

    /// Rename the network, as a reused path with another identity would.
    pub async fn rename_network(&self, name: &str) {
        self.service
            .object_server()
            .interface::<_, IwdNetwork>(AP)
            .await
            .unwrap()
            .get_mut()
            .await
            .name = name.into();
    }
}
