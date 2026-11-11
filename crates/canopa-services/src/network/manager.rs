//! NetworkManager's devices, saved connections and settings, for a network
//! settings page.
//!
//! [`read`] takes one [`Overview`]: the Wi-Fi switch, devices, nearby access
//! points, saved connections (with their IP, DNS, metered and proxy
//! settings) and installed VPN plugins. [`watch`] reads it again after
//! NetworkManager's signals. Commands address a connection by its settings
//! path and check its UUID first, so a stale page never changes another
//! connection. NetworkManager and polkit decide what is allowed; requests
//! allow interactive authorization. Secrets are never read into the
//! overview; see [`super::secret_agent`] for how they reach NetworkManager.
//!
//! ```no_run
//! use canopa_services::network::manager;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let overview = manager::read(&system).await?;
//! for connection in &overview.connections {
//!     println!("{} {:?} active={}", connection.id, connection.kind, connection.active.is_some());
//! }
//! manager::set_wireless_enabled(&system, !overview.wireless_enabled).await?;
//! # Ok(()) }
//! ```
use crate::{Error, bus::StateStream};
use std::collections::HashMap;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

pub const SERVICE: &str = "org.freedesktop.NetworkManager";
pub const PATH: &str = "/org/freedesktop/NetworkManager";
const MANAGER: &str = "org.freedesktop.NetworkManager";
const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const SETTINGS: &str = "org.freedesktop.NetworkManager.Settings";
const CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";
const DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const ACCESS_POINT: &str = "org.freedesktop.NetworkManager.AccessPoint";
const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";
/// A polkit prompt waits for the person.
const AUTHORIZATION: std::time::Duration = std::time::Duration::from_secs(300);
/// Bound what another process can make the page hold.
const LIMIT: usize = 256;

/// NetworkManager connection settings: setting name → property → value.
pub type Settings = HashMap<String, HashMap<String, OwnedValue>>;

/// Where VPN plugins describe themselves.
pub const VPN_PLUGINS: &str = "/usr/lib/NetworkManager/VPN";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Ethernet,
    Wifi,
    Other,
}

/// A network interface NetworkManager manages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub path: String,
    pub interface: String,
    pub kind: DeviceKind,
    /// NetworkManager's device state: 100 activated, 30 disconnected, 20
    /// unavailable (for example no cable), 10 unmanaged.
    pub state: u32,
    /// The settings path of the connection in use.
    pub connection: Option<String>,
    /// IPv4 addresses with prefix, as in `192.168.1.10/24`.
    pub addresses: Vec<String>,
}

impl Device {
    pub fn connected(&self) -> bool {
        self.state == 100
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Security {
    Open,
    Personal,
    Enterprise,
}

/// One nearby Wi-Fi network, the strongest access point for its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPoint {
    pub path: String,
    pub device: String,
    pub ssid: Vec<u8>,
    pub strength: u8,
    pub security: Security,
    pub active: bool,
}

impl AccessPoint {
    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.ssid).into_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionKind {
    Wifi {
        ssid: Vec<u8>,
        security: Security,
        /// `ap` for a hotspot, otherwise `infrastructure`.
        hotspot: bool,
    },
    Ethernet,
    WireGuard,
    /// A plugin VPN, with its NetworkManager service type.
    Vpn(String),
    Other(String),
}

impl ConnectionKind {
    pub fn is_vpn(&self) -> bool {
        matches!(self, Self::WireGuard | Self::Vpn(_))
    }
}

/// `connection.metered`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Metered {
    /// NetworkManager guesses (for example from the access point).
    #[default]
    Automatic,
    Yes,
    No,
}

impl Metered {
    fn from_nm(value: i32) -> Self {
        match value {
            1 => Self::Yes,
            2 => Self::No,
            _ => Self::Automatic,
        }
    }
    fn nm(self) -> i32 {
        match self {
            Self::Automatic => 0,
            Self::Yes => 1,
            Self::No => 2,
        }
    }
}

/// An IPv4 or IPv6 configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IpConfig {
    /// `auto`, `manual`, `disabled` (IPv4) or `ignore`/`disabled` (IPv6),
    /// `shared`, `link-local`, ...
    pub method: String,
    /// Manual addresses with prefix, as in `192.168.1.10/24`.
    pub addresses: Vec<String>,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
}

/// `proxy` settings. NetworkManager only stores them; PacRunner and
/// applications that ask it apply them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Proxy {
    #[default]
    None,
    /// A proxy auto-configuration file.
    Automatic { url: String },
    /// One proxy for every protocol, as `host:port`, stored as a PAC script.
    Manual { server: String },
}

/// A saved connection profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub path: String,
    pub uuid: String,
    pub id: String,
    pub kind: ConnectionKind,
    pub autoconnect: bool,
    pub metered: Metered,
    pub ipv4: IpConfig,
    pub ipv6: IpConfig,
    pub proxy: Proxy,
    /// The active connection path and its state (2 activated, 1 activating).
    pub active: Option<(String, u32)>,
}

/// An installed NetworkManager VPN plugin, such as `openvpn`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpnPlugin {
    /// The plugin's short name, as `nmcli connection import type` uses it.
    pub name: String,
    pub service: String,
}

/// One reading of NetworkManager.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overview {
    pub wireless_enabled: bool,
    /// False while a hardware switch or airplane-mode key keeps Wi-Fi off.
    pub wireless_hardware_enabled: bool,
    pub devices: Vec<Device>,
    pub access_points: Vec<AccessPoint>,
    pub connections: Vec<Connection>,
    pub vpn_plugins: Vec<VpnPlugin>,
}

impl Overview {
    pub fn connection(&self, path: &str) -> Option<&Connection> {
        self.connections.iter().find(|c| c.path == path)
    }

    /// The saved Wi-Fi connection for an access point, if any.
    pub fn saved_for(&self, ap: &AccessPoint) -> Option<&Connection> {
        self.connections.iter().find(|connection| {
            matches!(&connection.kind, ConnectionKind::Wifi { ssid, security, hotspot: false }
                if *ssid == ap.ssid && *security == ap.security)
        })
    }
}

async fn proxy<'a>(
    conn: &'a zbus::Connection,
    path: &str,
    interface: &'static str,
) -> Result<zbus::Proxy<'a>, Error> {
    Ok(zbus::proxy::Builder::new(conn)
        .destination(SERVICE)?
        .path(path.to_owned())?
        .interface(interface)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?)
}

async fn all(
    conn: &zbus::Connection,
    path: &str,
    interface: &'static str,
) -> Result<HashMap<String, OwnedValue>, Error> {
    let properties = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(SERVICE)?
        .path(path.to_owned())?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    let interface = zbus::names::InterfaceName::from_static_str(interface)
        .map_err(|_| Error::Invalid("interface name"))?;
    Ok(tokio::time::timeout(crate::bus::DEADLINE, properties.get_all(interface)).await??)
}

fn get<T: TryFrom<OwnedValue>>(map: &HashMap<String, OwnedValue>, key: &str) -> Option<T> {
    map.get(key)
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| T::try_from(value).ok())
}

fn text(map: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    get::<String>(map, key).map(|value| crate::bus::bounded_text(&value, 256))
}

fn path_or_none(value: Option<OwnedObjectPath>) -> Option<String> {
    value
        .map(|path| path.to_string())
        .filter(|path| path != "/")
}

pub(crate) fn security(flags: u32, wpa: u32, rsn: u32) -> Security {
    let key = wpa | rsn;
    if key & (0x200 | 0x2000) != 0 {
        Security::Enterprise
    } else if key & (0x100 | 0x400) != 0 {
        Security::Personal
    } else if flags & 1 != 0 {
        // WEP, or WPA without readable key management: never open.
        Security::Personal
    } else {
        Security::Open
    }
}

fn key_security(key: Option<&str>) -> Security {
    match key {
        None | Some("") => Security::Open,
        Some("wpa-psk" | "sae" | "none" | "owe") => Security::Personal,
        Some(_) => Security::Enterprise,
    }
}

fn ip_config(settings: &Settings, name: &str) -> IpConfig {
    let Some(ip) = settings.get(name) else {
        return IpConfig::default();
    };
    let addresses = get::<Vec<HashMap<String, OwnedValue>>>(ip, "address-data")
        .unwrap_or_default()
        .iter()
        .filter_map(|data| {
            Some(format!(
                "{}/{}",
                get::<String>(data, "address")?,
                get::<u32>(data, "prefix")?
            ))
        })
        .take(16)
        .collect();
    let dns = if name == "ipv4" {
        get::<Vec<u32>>(ip, "dns")
            .unwrap_or_default()
            .into_iter()
            // NetworkManager stores IPv4 DNS servers in network byte order.
            .map(|value| std::net::Ipv4Addr::from(u32::from_be(value)).to_string())
            .take(8)
            .collect()
    } else {
        get::<Vec<Vec<u8>>>(ip, "dns")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|bytes| <[u8; 16]>::try_from(bytes).ok())
            .map(|bytes| std::net::Ipv6Addr::from(bytes).to_string())
            .take(8)
            .collect()
    };
    IpConfig {
        method: text(ip, "method").unwrap_or_else(|| "auto".into()),
        addresses,
        gateway: text(ip, "gateway").filter(|value| !value.is_empty()),
        dns,
    }
}

fn proxy_settings(settings: &Settings) -> Proxy {
    let Some(proxy) = settings.get("proxy") else {
        return Proxy::None;
    };
    if get::<i32>(proxy, "method") != Some(1) {
        return Proxy::None;
    }
    let script = text(proxy, "pac-script").unwrap_or_default();
    if let Some(server) = manual_server(&script) {
        return Proxy::Manual { server };
    }
    Proxy::Automatic {
        url: text(proxy, "pac-url").unwrap_or_default(),
    }
}

/// The marker Canopa writes into a manual proxy's PAC script.
const MANUAL: &str = "// Canopa manual proxy: ";

fn manual_script(server: &str) -> String {
    format!(
        "{MANUAL}{server}\nfunction FindProxyForURL(url, host) {{ return \"PROXY {server}; DIRECT\"; }}\n"
    )
}

fn manual_server(script: &str) -> Option<String> {
    script
        .lines()
        .next()?
        .strip_prefix(MANUAL)
        .map(str::to_owned)
}

/// Parse one profile's settings into a [`Connection`].
pub fn connection_from(path: &str, settings: &Settings) -> Option<Connection> {
    let main = settings.get("connection")?;
    let kind = match text(main, "type")?.as_str() {
        "802-11-wireless" => {
            let wireless = settings.get("802-11-wireless")?;
            ConnectionKind::Wifi {
                ssid: get::<Vec<u8>>(wireless, "ssid")?,
                security: key_security(
                    settings
                        .get("802-11-wireless-security")
                        .and_then(|security| text(security, "key-mgmt"))
                        .as_deref(),
                ),
                hotspot: text(wireless, "mode").as_deref() == Some("ap"),
            }
        }
        "802-3-ethernet" => ConnectionKind::Ethernet,
        "wireguard" => ConnectionKind::WireGuard,
        "vpn" => ConnectionKind::Vpn(
            settings
                .get("vpn")
                .and_then(|vpn| text(vpn, "service-type"))
                .unwrap_or_default(),
        ),
        other => ConnectionKind::Other(other.into()),
    };
    Some(Connection {
        path: path.into(),
        uuid: text(main, "uuid")?,
        id: text(main, "id").unwrap_or_default(),
        kind,
        autoconnect: get::<bool>(main, "autoconnect").unwrap_or(true),
        metered: Metered::from_nm(get::<i32>(main, "metered").unwrap_or(0)),
        ipv4: ip_config(settings, "ipv4"),
        ipv6: ip_config(settings, "ipv6"),
        proxy: proxy_settings(settings),
        active: None,
    })
}

/// The installed VPN plugins under `root` (normally [`VPN_PLUGINS`]).
pub fn vpn_plugins(root: &std::path::Path) -> Vec<VpnPlugin> {
    let mut plugins: Vec<VpnPlugin> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|e| e == "name"))
        .take(64)
        .filter_map(|entry| {
            let text = std::fs::read_to_string(entry.path()).ok()?;
            let field = |key: &str| {
                text.lines()
                    .find_map(|line| line.trim().strip_prefix(key)?.trim().strip_prefix('='))
                    .map(|value| value.trim().to_owned())
            };
            Some(VpnPlugin {
                name: field("name")?,
                service: field("service")?,
            })
        })
        .collect();
    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    plugins.dedup_by(|a, b| a.name == b.name);
    plugins
}

/// Read NetworkManager's state. [`Error::Unavailable`] when it does not run.
pub async fn read(conn: &zbus::Connection) -> Result<Overview, Error> {
    read_with_plugins(conn, std::path::Path::new(VPN_PLUGINS)).await
}

/// [`read`] with VPN plugins listed from `plugins`.
pub async fn read_with_plugins(
    conn: &zbus::Connection,
    plugins: &std::path::Path,
) -> Result<Overview, Error> {
    if crate::bus::owner(conn, SERVICE).await?.is_none() {
        return Err(Error::Unavailable);
    }
    let manager = all(conn, PATH, MANAGER).await?;
    let mut overview = Overview {
        wireless_enabled: get(&manager, "WirelessEnabled").unwrap_or(false),
        wireless_hardware_enabled: get(&manager, "WirelessHardwareEnabled").unwrap_or(true),
        vpn_plugins: vpn_plugins(plugins),
        ..Default::default()
    };
    // Active connections, by their settings path.
    let mut active = HashMap::new();
    for path in get::<Vec<OwnedObjectPath>>(&manager, "ActiveConnections")
        .unwrap_or_default()
        .into_iter()
        .take(LIMIT)
    {
        let Ok(properties) = all(conn, path.as_str(), ACTIVE).await else {
            continue;
        };
        if let Some(settings) = path_or_none(get(&properties, "Connection")) {
            active.insert(
                settings,
                (
                    path.to_string(),
                    get::<u32>(&properties, "State").unwrap_or(0),
                ),
            );
        }
    }
    let settings = proxy(conn, SETTINGS_PATH, SETTINGS).await?;
    let paths: Vec<OwnedObjectPath> =
        tokio::time::timeout(crate::bus::DEADLINE, settings.call("ListConnections", &())).await??;
    for path in paths.into_iter().take(LIMIT) {
        let Ok(profile) = proxy(conn, path.as_str(), CONNECTION).await else {
            continue;
        };
        // Profiles this user may not read are not shown.
        let Ok(Ok(values)) = tokio::time::timeout(
            crate::bus::DEADLINE,
            profile.call::<_, _, Settings>("GetSettings", &()),
        )
        .await
        else {
            continue;
        };
        if let Some(mut connection) = connection_from(path.as_str(), &values) {
            connection.active = active.get(path.as_str()).cloned();
            overview.connections.push(connection);
        }
    }
    overview
        .connections
        .sort_by(|a, b| a.id.cmp(&b.id).then(a.uuid.cmp(&b.uuid)));
    let devices: Vec<OwnedObjectPath> = tokio::time::timeout(
        crate::bus::DEADLINE,
        proxy(conn, PATH, MANAGER).await?.call("GetDevices", &()),
    )
    .await??;
    let mut best: HashMap<(Vec<u8>, Security), AccessPoint> = HashMap::new();
    for path in devices.into_iter().take(64) {
        let Ok(device) = all(conn, path.as_str(), DEVICE).await else {
            continue;
        };
        let kind = match get::<u32>(&device, "DeviceType") {
            Some(1) => DeviceKind::Ethernet,
            Some(2) => DeviceKind::Wifi,
            _ => DeviceKind::Other,
        };
        if kind == DeviceKind::Other || get::<bool>(&device, "Managed") == Some(false) {
            continue;
        }
        let active = path_or_none(get(&device, "ActiveConnection"));
        let connection = match &active {
            Some(active) => all(conn, active, ACTIVE)
                .await
                .ok()
                .and_then(|properties| path_or_none(get(&properties, "Connection"))),
            None => None,
        };
        let mut addresses = Vec::new();
        if let Some(ip4) = path_or_none(get(&device, "Ip4Config"))
            && let Ok(ip) = all(conn, &ip4, "org.freedesktop.NetworkManager.IP4Config").await
        {
            addresses = get::<Vec<HashMap<String, OwnedValue>>>(&ip, "AddressData")
                .unwrap_or_default()
                .iter()
                .filter_map(|data| {
                    Some(format!(
                        "{}/{}",
                        get::<String>(data, "address")?,
                        get::<u32>(data, "prefix")?
                    ))
                })
                .take(16)
                .collect();
        }
        if kind == DeviceKind::Wifi
            && let Ok(wireless) = all(conn, path.as_str(), WIRELESS).await
        {
            let current = path_or_none(get(&wireless, "ActiveAccessPoint"));
            for ap in get::<Vec<OwnedObjectPath>>(&wireless, "AccessPoints")
                .unwrap_or_default()
                .into_iter()
                .take(LIMIT)
            {
                let Ok(properties) = all(conn, ap.as_str(), ACCESS_POINT).await else {
                    continue;
                };
                let Some(ssid) =
                    get::<Vec<u8>>(&properties, "Ssid").filter(|s| !s.is_empty() && s.len() <= 32)
                else {
                    continue;
                };
                let point = AccessPoint {
                    path: ap.to_string(),
                    device: path.to_string(),
                    security: security(
                        get(&properties, "Flags").unwrap_or(0),
                        get(&properties, "WpaFlags").unwrap_or(0),
                        get(&properties, "RsnFlags").unwrap_or(0),
                    ),
                    strength: get(&properties, "Strength").unwrap_or(0),
                    active: current.as_deref() == Some(ap.as_str()),
                    ssid,
                };
                let key = (point.ssid.clone(), point.security);
                let better = best.get(&key).is_none_or(|current| {
                    (point.active, point.strength) > (current.active, current.strength)
                });
                if better {
                    best.insert(key, point);
                }
            }
        }
        overview.devices.push(Device {
            path: path.to_string(),
            interface: text(&device, "Interface").unwrap_or_default(),
            kind,
            state: get(&device, "State").unwrap_or(0),
            connection,
            addresses,
        });
    }
    overview.access_points = best.into_values().collect();
    overview.access_points.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then(b.strength.cmp(&a.strength))
            .then(a.ssid.cmp(&b.ssid))
    });
    Ok(overview)
}

/// [`read`] again after NetworkManager's signals, as a recovering stream.
pub fn watch(conn: zbus::Connection) -> StateStream<Overview> {
    crate::bus::watch_service(conn, SERVICE, |conn| async move { read(&conn).await })
}

async fn interactive<R>(
    proxy: &zbus::Proxy<'_>,
    method: &str,
    body: &(impl serde::Serialize + zbus::zvariant::DynamicType),
) -> Result<R, Error>
where
    R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
{
    let reply = tokio::time::timeout(
        AUTHORIZATION,
        proxy.call_with_flags::<_, _, R>(
            method,
            zbus::proxy::MethodFlags::AllowInteractiveAuth.into(),
            body,
        ),
    )
    .await??;
    reply.ok_or(Error::Refused("no reply".into()))
}

/// Turn Wi-Fi on or off for every device.
pub async fn set_wireless_enabled(conn: &zbus::Connection, enabled: bool) -> Result<(), Error> {
    let properties = proxy(conn, PATH, "org.freedesktop.DBus.Properties").await?;
    let value = Value::from(enabled);
    let reply: Result<Option<()>, _> = tokio::time::timeout(
        AUTHORIZATION,
        properties.call_with_flags(
            "Set",
            zbus::proxy::MethodFlags::AllowInteractiveAuth.into(),
            &(MANAGER, "WirelessEnabled", value),
        ),
    )
    .await?;
    reply?;
    Ok(())
}

/// A connection the page read: its settings path and UUID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub path: String,
    pub uuid: String,
}

impl From<&Connection> for Target {
    fn from(connection: &Connection) -> Self {
        Self {
            path: connection.path.clone(),
            uuid: connection.uuid.clone(),
        }
    }
}

/// The profile at `target`, after checking it is still the same one.
async fn current<'a>(
    conn: &'a zbus::Connection,
    target: &Target,
) -> Result<(zbus::Proxy<'a>, Settings), Error> {
    let profile = proxy(conn, &target.path, CONNECTION).await?;
    let settings: Settings =
        profile
            .call("GetSettings", &())
            .await
            .map_err(|error| match Error::from(error) {
                Error::Unavailable => Error::Changed,
                other => other,
            })?;
    let uuid = settings
        .get("connection")
        .and_then(|main| text(main, "uuid"));
    if uuid.as_deref() != Some(target.uuid.as_str()) {
        return Err(Error::Changed);
    }
    Ok((profile, settings))
}

fn object(path: &str) -> Result<OwnedObjectPath, Error> {
    OwnedObjectPath::try_from(path.to_owned()).map_err(|_| Error::Invalid("object path"))
}

/// Connect a saved profile on `device` (`/` lets NetworkManager choose), for
/// Wi-Fi optionally to one access point. Returns the active connection path.
pub async fn activate(
    conn: &zbus::Connection,
    target: &Target,
    device: &str,
    access_point: &str,
) -> Result<String, Error> {
    current(conn, target).await?;
    let manager = proxy(conn, PATH, MANAGER).await?;
    let active: OwnedObjectPath = interactive(
        &manager,
        "ActivateConnection",
        &(
            object(&target.path)?,
            object(device)?,
            object(access_point)?,
        ),
    )
    .await?;
    Ok(active.to_string())
}

/// Disconnect a connection; `active` is its active connection path.
pub async fn deactivate(conn: &zbus::Connection, active: &str) -> Result<(), Error> {
    let manager = proxy(conn, PATH, MANAGER).await?;
    interactive::<()>(&manager, "DeactivateConnection", &(object(active)?,)).await
}

/// Disconnect a device until it is connected again by hand.
pub async fn disconnect_device(conn: &zbus::Connection, device: &str) -> Result<(), Error> {
    let device = proxy(conn, device, DEVICE).await?;
    interactive::<()>(&device, "Disconnect", &()).await
}

/// Forget a saved profile, including the secrets NetworkManager stored.
pub async fn delete(conn: &zbus::Connection, target: &Target) -> Result<(), Error> {
    let (profile, _) = current(conn, target).await?;
    interactive::<()>(&profile, "Delete", &()).await
}

/// Save a new profile. Returns its settings path.
pub async fn add(conn: &zbus::Connection, settings: Settings) -> Result<String, Error> {
    let manager = proxy(conn, SETTINGS_PATH, SETTINGS).await?;
    let path: OwnedObjectPath = interactive(&manager, "AddConnection", &(settings,)).await?;
    Ok(path.to_string())
}

/// Save a new profile and connect it. Returns its settings and active paths.
pub async fn add_and_activate(
    conn: &zbus::Connection,
    settings: Settings,
    device: &str,
    access_point: &str,
) -> Result<(String, String), Error> {
    let manager = proxy(conn, PATH, MANAGER).await?;
    let (path, active): (OwnedObjectPath, OwnedObjectPath) = interactive(
        &manager,
        "AddAndActivateConnection",
        &(settings, object(device)?, object(access_point)?),
    )
    .await?;
    Ok((path.to_string(), active.to_string()))
}

/// A change to a saved profile's ordinary settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Autoconnect(bool),
    Metered(Metered),
    /// `ipv4` or `ipv6` and the new configuration.
    Ip(&'static str, IpConfig),
    Proxy(Proxy),
}

fn owned(value: impl Into<Value<'static>>) -> OwnedValue {
    value
        .into()
        .try_into()
        .expect("plain values have no file descriptors")
}

fn parse_address(address: &str, v6: bool) -> Result<(String, u32), Error> {
    let (ip, prefix) = address.split_once('/').ok_or(Error::Invalid(
        "Write addresses with a prefix, as in 192.168.1.10/24.",
    ))?;
    let prefix: u32 = prefix
        .trim()
        .parse()
        .map_err(|_| Error::Invalid("The prefix must be a number."))?;
    let valid = if v6 {
        ip.trim().parse::<std::net::Ipv6Addr>().is_ok() && prefix <= 128
    } else {
        ip.trim().parse::<std::net::Ipv4Addr>().is_ok() && (1..=32).contains(&prefix)
    };
    if !valid {
        return Err(Error::Invalid("An address is not valid."));
    }
    Ok((ip.trim().to_owned(), prefix))
}

/// Apply a [`Change`] to `settings`, validating what the person typed.
pub fn apply_change(settings: &mut Settings, change: &Change) -> Result<(), Error> {
    match change {
        Change::Autoconnect(value) => {
            settings
                .entry("connection".into())
                .or_default()
                .insert("autoconnect".into(), owned(*value));
        }
        Change::Metered(value) => {
            settings
                .entry("connection".into())
                .or_default()
                .insert("metered".into(), owned(value.nm()));
        }
        Change::Ip(family, config) => {
            let v6 = *family == "ipv6";
            let allowed: &[&str] = if v6 {
                &["auto", "dhcp", "manual", "ignore", "disabled", "link-local"]
            } else {
                &["auto", "manual", "disabled", "link-local", "shared"]
            };
            if !allowed.contains(&config.method.as_str()) {
                return Err(Error::Invalid("Unknown address method."));
            }
            let ip = settings.entry((*family).into()).or_default();
            // The deprecated forms would override or conflict with the new ones.
            for key in ["addresses", "address-data", "gateway", "dns", "dns-data"] {
                ip.remove(key);
            }
            ip.insert("method".into(), owned(config.method.clone()));
            if config.method == "manual" {
                if config.addresses.is_empty() {
                    return Err(Error::Invalid("Manual addressing needs an address."));
                }
                let mut data = Vec::new();
                for address in &config.addresses {
                    let (address, prefix) = parse_address(address, v6)?;
                    data.push(HashMap::from([
                        ("address".to_string(), owned(address)),
                        ("prefix".to_string(), owned(prefix)),
                    ]));
                }
                ip.insert(
                    "address-data".into(),
                    OwnedValue::try_from(Value::from(data)).expect("plain values"),
                );
                if let Some(gateway) = config.gateway.as_ref().filter(|g| !g.trim().is_empty()) {
                    let valid = if v6 {
                        gateway.trim().parse::<std::net::Ipv6Addr>().is_ok()
                    } else {
                        gateway.trim().parse::<std::net::Ipv4Addr>().is_ok()
                    };
                    if !valid {
                        return Err(Error::Invalid("The gateway is not a valid address."));
                    }
                    ip.insert("gateway".into(), owned(gateway.trim().to_owned()));
                }
            }
            if !config.dns.is_empty() {
                if v6 {
                    let mut servers = Vec::new();
                    for server in &config.dns {
                        let address: std::net::Ipv6Addr = server
                            .trim()
                            .parse()
                            .map_err(|_| Error::Invalid("A DNS server is not a valid address."))?;
                        servers.push(address.octets().to_vec());
                    }
                    ip.insert("dns".into(), owned(servers));
                } else {
                    let mut servers = Vec::new();
                    for server in &config.dns {
                        let address: std::net::Ipv4Addr = server
                            .trim()
                            .parse()
                            .map_err(|_| Error::Invalid("A DNS server is not a valid address."))?;
                        servers.push(u32::from(address).to_be());
                    }
                    ip.insert("dns".into(), owned(servers));
                }
                // Manual servers replace the ones the network offers.
                ip.insert("ignore-auto-dns".into(), owned(true));
            } else {
                ip.insert("ignore-auto-dns".into(), owned(false));
            }
        }
        Change::Proxy(proxy) => {
            let mut values = HashMap::new();
            match proxy {
                Proxy::None => {
                    values.insert("method".to_string(), owned(0i32));
                }
                Proxy::Automatic { url } => {
                    if !(url.starts_with("http://")
                        || url.starts_with("https://")
                        || url.starts_with("file://"))
                    {
                        return Err(Error::Invalid(
                            "The configuration address must start with http://, https:// or file://.",
                        ));
                    }
                    values.insert("method".to_string(), owned(1i32));
                    values.insert("pac-url".to_string(), owned(url.clone()));
                }
                Proxy::Manual { server } => {
                    let (host, port) = server
                        .rsplit_once(':')
                        .ok_or(Error::Invalid("Write the proxy as host:port."))?;
                    if host.is_empty()
                        || port.parse::<u16>().is_err()
                        || !host
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b".-[]:".contains(&b))
                    {
                        return Err(Error::Invalid("Write the proxy as host:port."));
                    }
                    values.insert("method".to_string(), owned(1i32));
                    values.insert("pac-script".to_string(), owned(manual_script(server)));
                }
            }
            settings.insert("proxy".into(), values);
        }
    }
    Ok(())
}

/// Change a saved profile, keep its stored secrets, save it to disk, and
/// apply it again if it is connected.
pub async fn update(
    conn: &zbus::Connection,
    target: &Target,
    change: &Change,
) -> Result<(), Error> {
    let (profile, mut settings) = current(conn, target).await?;
    // GetSettings leaves secrets out; Update would otherwise drop them.
    for name in ["802-11-wireless-security", "802-1x", "vpn", "wireguard"] {
        if !settings.contains_key(name) {
            continue;
        }
        if let Ok(Ok(secrets)) = tokio::time::timeout(
            crate::bus::DEADLINE,
            profile.call::<_, _, Settings>("GetSecrets", &(name,)),
        )
        .await
        {
            for (setting, values) in secrets {
                settings.entry(setting).or_default().extend(values);
            }
        }
    }
    apply_change(&mut settings, change)?;
    interactive::<()>(&profile, "Update", &(settings,)).await?;
    let overview = read(conn).await?;
    if let Some(connection) = overview.connection(&target.path)
        && connection.active.is_some()
        && matches!(change, Change::Ip(..) | Change::Proxy(_))
    {
        activate(conn, target, "/", "/").await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
