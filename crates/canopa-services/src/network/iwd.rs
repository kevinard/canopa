//! iwd's Wi-Fi devices, networks in range and saved ("known") networks for a
//! settings page, and the changes it asks for. Panels use [`super::wifi`],
//! which follows one station; this module reads iwd's whole object tree.
//!
//! Actions address the well-known name and first check that the object at the
//! given path still has the name and type that were read, so a reused path or a
//! restarted iwd is reported as [`Error::Changed`]. iwd keeps every network it
//! joined; there is no "ask each time".
//!
//! ```no_run
//! use canopa_services::network::iwd;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let overview = iwd::read(&system).await?;
//! if let Some(home) = overview.networks.iter().find(|network| network.name == "Home") {
//!     iwd::connect(&system, home, Some("correct horse")).await?;
//! }
//! # Ok(()) }
//! ```
use super::wifi::WifiSecurity;
use crate::{Error, bus::StateStream};
use std::{collections::HashMap, time::Duration};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue};

pub const SERVICE: &str = super::bundle::IWD;
const DEVICE: &str = "net.connman.iwd.Device";
const STATION: &str = "net.connman.iwd.Station";
const NETWORK: &str = "net.connman.iwd.Network";
const KNOWN: &str = "net.connman.iwd.KnownNetwork";
/// iwd answers `Connect` once the connection is up or has failed.
const CONNECT: Duration = Duration::from_secs(60);

/// A Wi-Fi adapter's interface as iwd shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub path: String,
    /// The interface name, such as `wlan0`.
    pub name: String,
    pub powered: bool,
    /// The station state (`connected`, `connecting`, `disconnected`, …);
    /// `None` while the device is off or in another mode.
    pub state: Option<String>,
    pub scanning: bool,
}

/// A network in range of a station.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub path: String,
    /// The station's device path.
    pub device: String,
    pub name: String,
    /// iwd's type: `open`, `psk` or `8021x`.
    pub kind: String,
    /// From 0 to 100.
    pub strength: u8,
    pub connected: bool,
    /// The saved network's path, when iwd knows this network.
    pub known: Option<String>,
}

impl Network {
    pub fn security(&self) -> WifiSecurity {
        security(&self.kind)
    }
}

/// A network iwd joined before and keeps, in range or not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownNetwork {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub hidden: bool,
    pub autoconnect: bool,
}

impl KnownNetwork {
    pub fn security(&self) -> WifiSecurity {
        security(&self.kind)
    }
}

/// Everything the page shows from iwd.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overview {
    /// By path.
    pub devices: Vec<Device>,
    /// Each station's networks, strongest first.
    pub networks: Vec<Network>,
    /// By name.
    pub known: Vec<KnownNetwork>,
}

impl Overview {
    /// The network in range that `known` was saved for.
    pub fn in_range(&self, known: &KnownNetwork) -> Option<&Network> {
        self.networks
            .iter()
            .find(|network| network.known.as_deref() == Some(known.path.as_str()))
    }
}

fn security(kind: &str) -> WifiSecurity {
    match kind {
        "open" => WifiSecurity::Open,
        "psk" => WifiSecurity::Personal,
        _ => WifiSecurity::Enterprise,
    }
}

/// iwd's signal strength in 100 × dBm, from −100 dBm (0) to 0 dBm (100).
pub(crate) fn strength(signal: i16) -> u8 {
    ((i32::from(signal) + 10000).clamp(0, 10000) / 100) as u8
}

pub(crate) type Properties = HashMap<String, OwnedValue>;
pub(crate) type ManagedObjects = HashMap<OwnedObjectPath, HashMap<String, Properties>>;

#[zbus::proxy(
    interface = "org.freedesktop.DBus.ObjectManager",
    default_service = "net.connman.iwd",
    default_path = "/"
)]
pub(crate) trait IwdObjectManager {
    fn get_managed_objects(&self) -> zbus::Result<ManagedObjects>;
}

#[zbus::proxy(
    interface = "net.connman.iwd.Station",
    default_service = "net.connman.iwd"
)]
pub(crate) trait IwdStation {
    fn connect_hidden_network(&self, ssid: &str) -> zbus::Result<()>;
    fn disconnect(&self) -> zbus::Result<()>;
    fn scan(&self) -> zbus::Result<()>;
    fn get_ordered_networks(&self) -> zbus::Result<Vec<(OwnedObjectPath, i16)>>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn scanning(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn connected_network(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "net.connman.iwd.Network",
    default_service = "net.connman.iwd"
)]
pub(crate) trait IwdNetwork {
    fn connect(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;
    #[zbus(property, name = "Type")]
    fn network_type(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn device(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn connected(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn known_network(&self) -> zbus::Result<OwnedObjectPath>;
}

fn text(properties: &Properties, key: &str) -> String {
    properties
        .get(key)
        .and_then(|value| <&str>::try_from(value).ok())
        .unwrap_or_default()
        .to_owned()
}

fn flag(properties: &Properties, key: &str) -> bool {
    properties
        .get(key)
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or(false)
}

/// An object path property; `None` when absent or `/`.
fn object(properties: &Properties, key: &str) -> Option<String> {
    properties
        .get(key)
        .and_then(|value| <&ObjectPath>::try_from(value).ok())
        .map(|path| path.as_str())
        .filter(|path| *path != "/")
        .map(str::to_owned)
}

async fn station<'a>(conn: &zbus::Connection, path: &'a str) -> Result<IwdStationProxy<'a>, Error> {
    Ok(IwdStationProxy::builder(conn)
        .path(path)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?)
}

/// Read every device, network in range and saved network.
pub async fn read(conn: &zbus::Connection) -> Result<Overview, Error> {
    let objects = tokio::time::timeout(
        crate::bus::DEADLINE,
        IwdObjectManagerProxy::new(conn)
            .await?
            .get_managed_objects(),
    )
    .await??;
    let mut overview = Overview::default();
    for (path, interfaces) in &objects {
        if let Some(device) = interfaces.get(DEVICE) {
            let station = interfaces.get(STATION);
            overview.devices.push(Device {
                path: path.to_string(),
                name: text(device, "Name"),
                powered: flag(device, "Powered"),
                state: station.map(|station| text(station, "State")),
                scanning: station.is_some_and(|station| flag(station, "Scanning")),
            });
        }
        if let Some(known) = interfaces.get(KNOWN) {
            overview.known.push(KnownNetwork {
                path: path.to_string(),
                name: text(known, "Name"),
                kind: text(known, "Type"),
                hidden: flag(known, "Hidden"),
                autoconnect: flag(known, "AutoConnect"),
            });
        }
    }
    overview.devices.sort_by(|a, b| a.path.cmp(&b.path));
    overview
        .known
        .sort_by_key(|known| known.name.to_lowercase());
    // Only a station knows the order and signal of its networks.
    for device in overview
        .devices
        .iter()
        .filter(|device| device.state.is_some())
    {
        let ordered = tokio::time::timeout(
            crate::bus::DEADLINE,
            station(conn, &device.path).await?.get_ordered_networks(),
        )
        .await??;
        for (path, signal) in ordered {
            let Some(network) = objects
                .get(&path)
                .and_then(|interfaces| interfaces.get(NETWORK))
            else {
                continue;
            };
            let name = text(network, "Name");
            if name.is_empty() {
                continue;
            }
            overview.networks.push(Network {
                path: path.to_string(),
                device: device.path.clone(),
                name,
                kind: text(network, "Type"),
                strength: strength(signal),
                connected: flag(network, "Connected"),
                known: object(network, "KnownNetwork"),
            });
        }
    }
    Ok(overview)
}

/// Observe iwd: a new overview after each burst of its signals.
pub fn watch(conn: zbus::Connection) -> StateStream<Overview> {
    crate::bus::watch_service(conn, SERVICE, |conn| async move { read(&conn).await })
}

/// The properties of `interface` at `path`, if it still has `name` (and
/// `kind`, when given); otherwise [`Error::Changed`].
async fn current(
    conn: &zbus::Connection,
    path: &str,
    interface: &str,
    name: &str,
    kind: Option<&str>,
) -> Result<Properties, Error> {
    let properties = crate::bus::properties(conn, SERVICE, path, interface)
        .await
        .map_err(|error| match error {
            Error::TimedOut | Error::NotAuthorized => error,
            _ => Error::Changed,
        })?;
    let same = text(&properties, "Name") == name
        && kind.is_none_or(|kind| text(&properties, "Type") == kind);
    if same {
        Ok(properties)
    } else {
        Err(Error::Changed)
    }
}

fn passphrase(secret: &str) -> Result<(), Error> {
    if (8..=63).contains(&secret.len())
        || (secret.len() == 64 && secret.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        Ok(())
    } else {
        Err(Error::Invalid("A Wi-Fi password has 8 to 63 characters."))
    }
}

/// Turn a device's radio on or off.
pub async fn set_powered(conn: &zbus::Connection, device: &Device, on: bool) -> Result<(), Error> {
    current(conn, &device.path, DEVICE, &device.name, None).await?;
    let proxy = zbus::Proxy::new(conn, SERVICE, device.path.as_str(), DEVICE).await?;
    tokio::time::timeout(crate::bus::DEADLINE, proxy.set_property("Powered", on))
        .await?
        .map_err(zbus::Error::from)?;
    Ok(())
}

/// Ask a station to look for networks; the overview reports the results.
pub async fn scan(conn: &zbus::Connection, device: &Device) -> Result<(), Error> {
    current(conn, &device.path, DEVICE, &device.name, None).await?;
    Ok(tokio::time::timeout(
        crate::bus::DEADLINE,
        station(conn, &device.path).await?.scan(),
    )
    .await??)
}

/// Leave `network`, when its station is connected to it.
pub async fn disconnect(conn: &zbus::Connection, network: &Network) -> Result<(), Error> {
    let properties = current(
        conn,
        &network.path,
        NETWORK,
        &network.name,
        Some(&network.kind),
    )
    .await?;
    if !flag(&properties, "Connected") {
        return Err(Error::Changed);
    }
    Ok(tokio::time::timeout(
        crate::bus::DEADLINE,
        station(conn, &network.device).await?.disconnect(),
    )
    .await??)
}

/// Connect a network in range. A saved or open network needs no
/// `passphrase`; iwd saves the network once it connects.
pub async fn connect(
    conn: &zbus::Connection,
    network: &Network,
    passphrase: Option<&str>,
) -> Result<(), Error> {
    let properties = current(
        conn,
        &network.path,
        NETWORK,
        &network.name,
        Some(&network.kind),
    )
    .await?;
    if object(&properties, "Device").as_deref() != Some(network.device.as_str()) {
        return Err(Error::Changed);
    }
    let device = OwnedObjectPath::try_from(network.device.as_str())
        .map_err(|_| Error::Invalid("object path"))?;
    let _agent = agent(conn, passphrase, &device, &network.name, &network.kind).await?;
    let proxy = IwdNetworkProxy::builder(conn)
        .path(network.path.as_str())?
        .build()
        .await?;
    Ok(tokio::time::timeout(CONNECT, proxy.connect()).await??)
}

/// Join a hidden personal or open network by name.
pub async fn connect_hidden(
    conn: &zbus::Connection,
    device: &Device,
    name: &str,
    passphrase: Option<&str>,
) -> Result<(), Error> {
    if name.is_empty() || name.len() > 32 {
        return Err(Error::Invalid("A network name has 1 to 32 bytes."));
    }
    current(conn, &device.path, DEVICE, &device.name, None).await?;
    let path = OwnedObjectPath::try_from(device.path.as_str())
        .map_err(|_| Error::Invalid("object path"))?;
    let _agent = agent(conn, passphrase, &path, name, "psk").await?;
    Ok(tokio::time::timeout(
        CONNECT,
        station(conn, &device.path)
            .await?
            .connect_hidden_network(name),
    )
    .await??)
}

/// An agent that hands `passphrase` to iwd for this one attempt.
async fn agent(
    conn: &zbus::Connection,
    passphrase: Option<&str>,
    station: &OwnedObjectPath,
    name: &str,
    kind: &str,
) -> Result<Option<super::wifi::backends::IwdAgentRegistration>, Error> {
    let Some(secret) = passphrase else {
        return Ok(None);
    };
    self::passphrase(secret)?;
    let owner = crate::bus::owner(conn, SERVICE)
        .await?
        .ok_or(Error::Changed)?;
    super::wifi::backends::IwdAgentRegistration::register(
        conn,
        owner.as_str(),
        Some(secret.to_owned()),
        station,
        name,
        kind,
        super::wifi::cleanup::Cleanup::default(),
    )
    .await
    .map(Some)
    .map_err(|_| Error::Changed)
}

/// Forget a saved network and the secrets iwd kept for it.
pub async fn forget(conn: &zbus::Connection, known: &KnownNetwork) -> Result<(), Error> {
    current(conn, &known.path, KNOWN, &known.name, Some(&known.kind)).await?;
    let proxy = zbus::Proxy::new(conn, SERVICE, known.path.as_str(), KNOWN).await?;
    Ok(tokio::time::timeout(crate::bus::DEADLINE, proxy.call::<_, _, ()>("Forget", &())).await??)
}

/// Whether iwd joins a saved network by itself when it is in range.
pub async fn set_autoconnect(
    conn: &zbus::Connection,
    known: &KnownNetwork,
    on: bool,
) -> Result<(), Error> {
    current(conn, &known.path, KNOWN, &known.name, Some(&known.kind)).await?;
    let proxy = zbus::Proxy::new(conn, SERVICE, known.path.as_str(), KNOWN).await?;
    tokio::time::timeout(crate::bus::DEADLINE, proxy.set_property("AutoConnect", on))
        .await?
        .map_err(zbus::Error::from)?;
    Ok(())
}

#[cfg(test)]
#[path = "iwd_tests.rs"]
mod tests;
