//! A fake NetworkManager with settings, devices and a secret agent manager,
//! for network settings pages.
//!
//! It starts with a connected Ethernet device (`eth0`, profile "Wired"), a
//! Wi-Fi device (`wlan0`) seeing "Home" (WPA-Personal, saved), "eduroam"
//! (WPA-Enterprise) and "Cafe" (open), and one plugin VPN profile ("Office
//! VPN"). Requests change its state as NetworkManager would, closely enough
//! for clients: activation succeeds at once, and each change emits a signal.
use super::TestBus;
use crate::network::manager::Settings;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};
use zbus::{
    object_server::SignalEmitter,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

const PATH: &str = "/org/freedesktop/NetworkManager";
pub const ETHERNET: &str = "/org/freedesktop/NetworkManager/Devices/1";
pub const WIFI: &str = "/org/freedesktop/NetworkManager/Devices/2";
pub const HOME: &str = "/org/freedesktop/NetworkManager/AccessPoint/1";
pub const EDUROAM: &str = "/org/freedesktop/NetworkManager/AccessPoint/2";
pub const CAFE: &str = "/org/freedesktop/NetworkManager/AccessPoint/3";
const SECRET_KEYS: [&str; 5] = [
    "psk",
    "password",
    "private-key",
    "private-key-password",
    "secrets",
];

fn path(value: &str) -> OwnedObjectPath {
    value.try_into().unwrap()
}

fn owned(value: impl Into<Value<'static>>) -> OwnedValue {
    value.into().try_into().unwrap()
}

fn profile(id: &str, uuid: &str, kind: &str, extra: &[(&str, &[(&str, OwnedValue)])]) -> Settings {
    let mut settings: Settings = HashMap::from([(
        "connection".to_string(),
        HashMap::from([
            ("id".to_string(), owned(id.to_owned())),
            ("uuid".to_string(), owned(uuid.to_owned())),
            ("type".to_string(), owned(kind.to_owned())),
        ]),
    )]);
    for (name, values) in extra {
        settings.insert(
            (*name).into(),
            values
                .iter()
                .map(|(key, value)| ((*key).to_string(), value.try_clone().unwrap()))
                .collect(),
        );
    }
    settings
}

/// What clients did and what the fake holds.
#[derive(Default)]
pub struct State {
    pub wireless_enabled: bool,
    /// Profiles by number, as `/org/freedesktop/NetworkManager/Settings/N`.
    pub connections: BTreeMap<u32, Settings>,
    /// Active connections by number: (profile number, device path).
    pub active: BTreeMap<u32, (u32, String)>,
    /// Method calls, as `Method profile-or-device`.
    pub calls: Vec<String>,
    /// The registered secret agent: its unique name.
    pub agent: Option<String>,
    next: u32,
}

impl State {
    fn connection_number(&self, path: &str) -> Option<u32> {
        path.strip_prefix("/org/freedesktop/NetworkManager/Settings/")?
            .parse()
            .ok()
            .filter(|number| self.connections.contains_key(number))
    }
}

type Shared = Arc<Mutex<State>>;

async fn changed(conn: &zbus::Connection) {
    if let Ok(emitter) = SignalEmitter::new(conn, PATH) {
        let _ = Manager::state_changed(&emitter, 70).await;
    }
}

fn connection_path(number: u32) -> String {
    format!("/org/freedesktop/NetworkManager/Settings/{number}")
}

fn active_path(number: u32) -> String {
    format!("/org/freedesktop/NetworkManager/ActiveConnection/{number}")
}

async fn activate(
    state: &Shared,
    server: &zbus::ObjectServer,
    connection: u32,
    device: String,
) -> OwnedObjectPath {
    let number = {
        let mut state = state.lock().unwrap();
        state.next += 1;
        let number = state.next;
        // One connection per device, as NetworkManager does.
        state
            .active
            .retain(|_, (_, on)| *on != device || device == "/");
        state.active.insert(number, (connection, device));
        number
    };
    server
        .at(
            active_path(number),
            Active {
                state: state.clone(),
                number,
            },
        )
        .await
        .unwrap();
    path(&active_path(number))
}

struct Manager(Shared);

#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl Manager {
    #[zbus(property)]
    fn wireless_enabled(&self) -> bool {
        self.0.lock().unwrap().wireless_enabled
    }
    #[zbus(property)]
    fn set_wireless_enabled(&mut self, enabled: bool) {
        let mut state = self.0.lock().unwrap();
        state.wireless_enabled = enabled;
        state.calls.push(format!("WirelessEnabled {enabled}"));
    }
    #[zbus(property)]
    fn wireless_hardware_enabled(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn connectivity(&self) -> u32 {
        4
    }
    #[zbus(property)]
    fn active_connections(&self) -> Vec<OwnedObjectPath> {
        self.0
            .lock()
            .unwrap()
            .active
            .keys()
            .map(|number| path(&active_path(*number)))
            .collect()
    }
    #[zbus(property)]
    fn primary_connection(&self) -> OwnedObjectPath {
        let state = self.0.lock().unwrap();
        state
            .active
            .iter()
            .find(|(_, (_, device))| device == ETHERNET)
            .map_or_else(|| path("/"), |(number, _)| path(&active_path(*number)))
    }
    #[zbus(property)]
    fn primary_connection_type(&self) -> String {
        if self.primary_connection().as_str() == "/" {
            String::new()
        } else {
            "802-3-ethernet".into()
        }
    }
    fn get_devices(&self) -> Vec<OwnedObjectPath> {
        vec![path(ETHERNET), path(WIFI)]
    }
    async fn activate_connection(
        &self,
        connection: OwnedObjectPath,
        device: OwnedObjectPath,
        _specific: OwnedObjectPath,
        #[zbus(object_server)] server: &zbus::ObjectServer,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let number = {
            let mut state = self.0.lock().unwrap();
            state.calls.push(format!("Activate {connection}"));
            state
                .connection_number(connection.as_str())
                .ok_or_else(|| zbus::fdo::Error::UnknownObject("no such connection".into()))?
        };
        let active = activate(&self.0, server, number, device.to_string()).await;
        changed(conn).await;
        Ok(active)
    }
    async fn add_and_activate_connection(
        &self,
        settings: Settings,
        device: OwnedObjectPath,
        _specific: OwnedObjectPath,
        #[zbus(object_server)] server: &zbus::ObjectServer,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<(OwnedObjectPath, OwnedObjectPath)> {
        let number = add(&self.0, server, settings, "AddAndActivate").await;
        let active = activate(&self.0, server, number, device.to_string()).await;
        changed(conn).await;
        Ok((path(&connection_path(number)), active))
    }
    async fn deactivate_connection(
        &self,
        active: OwnedObjectPath,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        {
            let mut state = self.0.lock().unwrap();
            state.calls.push(format!("Deactivate {active}"));
            let number = active
                .as_str()
                .rsplit('/')
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            if state.active.remove(&number).is_none() {
                return Err(zbus::fdo::Error::UnknownObject("not active".into()));
            }
        }
        changed(conn).await;
        Ok(())
    }
    #[zbus(signal)]
    async fn state_changed(emitter: &SignalEmitter<'_>, state: u32) -> zbus::Result<()>;
}

async fn add(state: &Shared, server: &zbus::ObjectServer, settings: Settings, call: &str) -> u32 {
    let number = {
        let mut state = state.lock().unwrap();
        state.next += 1;
        let number = state.next;
        let id = settings
            .get("connection")
            .and_then(|main| main.get("id"))
            .and_then(|id| <&str>::try_from(id).ok())
            .unwrap_or_default()
            .to_owned();
        state.calls.push(format!("{call} {id}"));
        state.connections.insert(number, settings);
        number
    };
    server
        .at(
            connection_path(number),
            Profile {
                state: state.clone(),
                number,
            },
        )
        .await
        .unwrap();
    number
}

struct SettingsObject(Shared);

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings")]
impl SettingsObject {
    fn list_connections(&self) -> Vec<OwnedObjectPath> {
        self.0
            .lock()
            .unwrap()
            .connections
            .keys()
            .map(|number| path(&connection_path(*number)))
            .collect()
    }
    async fn add_connection(
        &self,
        settings: Settings,
        #[zbus(object_server)] server: &zbus::ObjectServer,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> OwnedObjectPath {
        let number = add(&self.0, server, settings, "Add").await;
        changed(conn).await;
        path(&connection_path(number))
    }
}

struct Profile {
    state: Shared,
    number: u32,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings.Connection")]
impl Profile {
    fn get_settings(&self) -> zbus::fdo::Result<Settings> {
        let state = self.state.lock().unwrap();
        let mut settings = state
            .connections
            .get(&self.number)
            .ok_or_else(|| zbus::fdo::Error::UnknownObject("deleted".into()))?
            .iter()
            .map(|(name, values)| {
                (
                    name.clone(),
                    values
                        .iter()
                        .map(|(key, value)| (key.clone(), value.try_clone().unwrap()))
                        .collect::<HashMap<_, _>>(),
                )
            })
            .collect::<Settings>();
        // NetworkManager never returns secrets here.
        for values in settings.values_mut() {
            values.retain(|key, _| !SECRET_KEYS.contains(&key.as_str()));
        }
        Ok(settings)
    }
    fn get_secrets(&self, setting: &str) -> zbus::fdo::Result<Settings> {
        let state = self.state.lock().unwrap();
        let values = state
            .connections
            .get(&self.number)
            .and_then(|settings| settings.get(setting))
            .map(|values| {
                values
                    .iter()
                    .filter(|(key, _)| SECRET_KEYS.contains(&key.as_str()))
                    .map(|(key, value)| (key.clone(), value.try_clone().unwrap()))
                    .collect()
            })
            .unwrap_or_default();
        Ok(HashMap::from([(setting.to_owned(), values)]))
    }
    async fn update(
        &self,
        settings: Settings,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        {
            let mut state = self.state.lock().unwrap();
            state.calls.push(format!("Update {}", self.number));
            state.connections.insert(self.number, settings);
        }
        changed(conn).await;
        Ok(())
    }
    async fn delete(
        &self,
        #[zbus(object_server)] server: &zbus::ObjectServer,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        {
            let mut state = self.state.lock().unwrap();
            state.calls.push(format!("Delete {}", self.number));
            state.connections.remove(&self.number);
            let number = self.number;
            state
                .active
                .retain(|_, (connection, _)| *connection != number);
        }
        let _ = server
            .remove::<Profile, _>(connection_path(self.number))
            .await;
        changed(conn).await;
        Ok(())
    }
}

struct Active {
    state: Shared,
    number: u32,
}

impl Active {
    fn profile(&self) -> Option<(u32, Settings)> {
        let state = self.state.lock().unwrap();
        let (connection, _) = state.active.get(&self.number)?;
        let settings = state.connections.get(connection)?;
        Some((
            *connection,
            settings
                .iter()
                .map(|(name, values)| {
                    (
                        name.clone(),
                        values
                            .iter()
                            .map(|(key, value)| (key.clone(), value.try_clone().unwrap()))
                            .collect(),
                    )
                })
                .collect(),
        ))
    }
    fn main(&self, key: &str) -> String {
        self.profile()
            .and_then(|(_, settings)| {
                settings
                    .get("connection")?
                    .get(key)
                    .and_then(|value| <&str>::try_from(value).ok())
                    .map(str::to_owned)
            })
            .unwrap_or_default()
    }
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl Active {
    #[zbus(property)]
    fn connection(&self) -> OwnedObjectPath {
        self.profile()
            .map_or_else(|| path("/"), |(number, _)| path(&connection_path(number)))
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        if self.profile().is_some() { 2 } else { 4 }
    }
    #[zbus(property)]
    fn id(&self) -> String {
        self.main("id")
    }
    #[zbus(property)]
    fn uuid(&self) -> String {
        self.main("uuid")
    }
    #[zbus(property, name = "Type")]
    fn kind(&self) -> String {
        self.main("type")
    }
    #[zbus(property)]
    fn vpn(&self) -> bool {
        matches!(self.main("type").as_str(), "vpn" | "wireguard")
    }
}

struct Device {
    state: Shared,
    path: &'static str,
    interface: &'static str,
    kind: u32,
}

impl Device {
    fn active(&self) -> Option<u32> {
        let state = self.state.lock().unwrap();
        state
            .active
            .iter()
            .find(|(_, (_, device))| device == self.path)
            .map(|(number, _)| *number)
    }
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl Device {
    #[zbus(property)]
    fn interface(&self) -> &str {
        self.interface
    }
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        self.kind
    }
    #[zbus(property)]
    fn managed(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        if self.active().is_some() { 100 } else { 30 }
    }
    #[zbus(property)]
    fn active_connection(&self) -> OwnedObjectPath {
        self.active()
            .map_or_else(|| path("/"), |number| path(&active_path(number)))
    }
    #[zbus(property)]
    fn ip4_config(&self) -> OwnedObjectPath {
        if self.active().is_some() && self.kind == 1 {
            path("/org/freedesktop/NetworkManager/IP4Config/1")
        } else {
            path("/")
        }
    }
    async fn disconnect(&self, #[zbus(connection)] conn: &zbus::Connection) {
        {
            let mut state = self.state.lock().unwrap();
            state.calls.push(format!("Disconnect {}", self.interface));
            let device = self.path;
            state.active.retain(|_, (_, on)| on != device);
        }
        changed(conn).await;
    }
}

struct Wireless(Shared);

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl Wireless {
    #[zbus(property)]
    fn access_points(&self) -> Vec<OwnedObjectPath> {
        if self.0.lock().unwrap().wireless_enabled {
            vec![path(HOME), path(EDUROAM), path(CAFE)]
        } else {
            Vec::new()
        }
    }
    #[zbus(property)]
    fn active_access_point(&self) -> OwnedObjectPath {
        path("/")
    }
}

struct AccessPoint {
    ssid: &'static str,
    flags: u32,
    rsn: u32,
    strength: u8,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl AccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        self.ssid.as_bytes().to_vec()
    }
    #[zbus(property)]
    fn flags(&self) -> u32 {
        self.flags
    }
    #[zbus(property)]
    fn wpa_flags(&self) -> u32 {
        0
    }
    #[zbus(property)]
    fn rsn_flags(&self) -> u32 {
        self.rsn
    }
    #[zbus(property)]
    fn strength(&self) -> u8 {
        self.strength
    }
}

struct Ip4;

#[zbus::interface(name = "org.freedesktop.NetworkManager.IP4Config")]
impl Ip4 {
    #[zbus(property)]
    fn address_data(&self) -> Vec<HashMap<String, OwnedValue>> {
        vec![HashMap::from([
            ("address".to_string(), owned("192.168.1.20")),
            ("prefix".to_string(), owned(24u32)),
        ])]
    }
}

struct AgentManager(Shared);

#[zbus::interface(name = "org.freedesktop.NetworkManager.AgentManager")]
impl AgentManager {
    fn register(&self, _identifier: &str, #[zbus(header)] header: zbus::message::Header<'_>) {
        self.0.lock().unwrap().agent = header.sender().map(|sender| sender.to_string());
    }
    fn unregister(&self) {
        self.0.lock().unwrap().agent = None;
    }
}

/// The fake. Close [`FakeNetworkManager::connection`] to simulate it leaving.
pub struct FakeNetworkManager {
    pub connection: zbus::Connection,
    pub state: Shared,
}

/// The "Wired" profile's settings path.
pub const WIRED: &str = "/org/freedesktop/NetworkManager/Settings/1";
/// The saved "Home" Wi-Fi profile.
pub const HOME_PROFILE: &str = "/org/freedesktop/NetworkManager/Settings/2";
/// The "Office VPN" OpenVPN profile.
pub const VPN: &str = "/org/freedesktop/NetworkManager/Settings/3";

impl FakeNetworkManager {
    pub async fn serve(bus: &TestBus) -> Self {
        let connection = bus.connect().await;
        let state: Shared = Arc::default();
        let server = connection.object_server();
        state.lock().unwrap().wireless_enabled = true;
        for settings in [
            profile(
                "Wired",
                "11111111-1111-4111-8111-111111111111",
                "802-3-ethernet",
                &[("ipv4", &[("method", owned("auto"))])],
            ),
            profile(
                "Home",
                "22222222-2222-4222-8222-222222222222",
                "802-11-wireless",
                &[
                    ("802-11-wireless", &[("ssid", owned(b"Home".to_vec()))]),
                    (
                        "802-11-wireless-security",
                        &[
                            ("key-mgmt", owned("wpa-psk")),
                            ("psk", owned("stored-secret")),
                        ],
                    ),
                ],
            ),
            profile(
                "Office VPN",
                "33333333-3333-4333-8333-333333333333",
                "vpn",
                &[(
                    "vpn",
                    &[(
                        "service-type",
                        owned("org.freedesktop.NetworkManager.openvpn"),
                    )],
                )],
            ),
        ] {
            add(&state, server, settings, "Seed").await;
        }
        activate(&state, server, 1, ETHERNET.into()).await;
        state.lock().unwrap().calls.clear();
        server.at(PATH, Manager(state.clone())).await.unwrap();
        server
            .at(
                "/org/freedesktop/NetworkManager/Settings",
                SettingsObject(state.clone()),
            )
            .await
            .unwrap();
        server
            .at(
                "/org/freedesktop/NetworkManager/AgentManager",
                AgentManager(state.clone()),
            )
            .await
            .unwrap();
        for (path, interface, kind) in [(ETHERNET, "eth0", 1), (WIFI, "wlan0", 2)] {
            server
                .at(
                    path,
                    Device {
                        state: state.clone(),
                        path,
                        interface,
                        kind,
                    },
                )
                .await
                .unwrap();
        }
        server.at(WIFI, Wireless(state.clone())).await.unwrap();
        for (path, ssid, flags, rsn, strength) in [
            (HOME, "Home", 1, 0x100, 80),
            (EDUROAM, "eduroam", 1, 0x200, 60),
            (CAFE, "Cafe", 0, 0, 40),
        ] {
            server
                .at(
                    path,
                    AccessPoint {
                        ssid,
                        flags,
                        rsn,
                        strength,
                    },
                )
                .await
                .unwrap();
        }
        server
            .at("/org/freedesktop/NetworkManager/IP4Config/1", Ip4)
            .await
            .unwrap();
        connection
            .request_name("org.freedesktop.NetworkManager")
            .await
            .unwrap();
        Self { connection, state }
    }

    pub fn calls(&self) -> Vec<String> {
        self.state.lock().unwrap().calls.clone()
    }

    /// The stored settings of the profile with this connection id.
    pub fn profile(&self, id: &str) -> Option<Settings> {
        let state = self.state.lock().unwrap();
        state.connections.values().find_map(|settings| {
            let matches = settings
                .get("connection")
                .and_then(|main| main.get("id"))
                .and_then(|value| <&str>::try_from(value).ok())
                == Some(id);
            matches.then(|| {
                settings
                    .iter()
                    .map(|(name, values)| {
                        (
                            name.clone(),
                            values
                                .iter()
                                .map(|(key, value)| (key.clone(), value.try_clone().unwrap()))
                                .collect(),
                        )
                    })
                    .collect()
            })
        })
    }

    /// Ask the registered agent for `setting` of the profile `id`, as
    /// NetworkManager does while connecting.
    pub async fn ask_agent(
        &self,
        id: &str,
        setting: &str,
        hints: &[&str],
        flags: u32,
    ) -> zbus::Result<Settings> {
        let agent = self
            .state
            .lock()
            .unwrap()
            .agent
            .clone()
            .expect("an agent registered");
        let settings = self.profile(id).expect("the profile exists");
        let proxy = zbus::Proxy::new(
            &self.connection,
            agent,
            crate::network::secret_agent::AGENT_PATH,
            "org.freedesktop.NetworkManager.SecretAgent",
        )
        .await?;
        proxy
            .call(
                "GetSecrets",
                &(
                    settings,
                    path(WIRED),
                    setting,
                    hints
                        .iter()
                        .map(|hint| hint.to_string())
                        .collect::<Vec<_>>(),
                    flags,
                ),
            )
            .await
    }
}
