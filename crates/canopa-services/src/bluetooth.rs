//! BlueZ adapters, devices and application-local pairing.
//!
//! Commands address the BlueZ owner a [`Snapshot`] was read from, so a path
//! from an earlier daemon is never acted on by a new one. Pairing registers an
//! agent on a private connection for one attempt; it is never made the default
//! agent and accepts requests only from that owner for the selected device.
//!
//! ```no_run
//! use canopa_services::{ServiceState, bluetooth};
//! use futures_util::StreamExt;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let mut states = bluetooth::watch(system.clone());
//! while let Some(ServiceState::Available(snapshot)) = states.next().await {
//!     let Some(owner) = &snapshot.owner else { continue };
//!     for adapter in snapshot.adapters.iter().filter(|adapter| !adapter.powered) {
//!         bluetooth::set_powered(&system, owner, adapter.path.as_str(), true).await?;
//!     }
//!     for device in &snapshot.devices {
//!         println!("{} paired={} connected={}", device.label, device.paired, device.connected);
//!     }
//! }
//! # Ok(()) }
//! ```
//!
//! For pairing, spawn [`pair`] with an [`Attempt`] and answer each
//! [`AgentEvent::Request`] that carries a reply sender.
use crate::{Error, bus::StateStream};
use std::{collections::HashMap, time::Duration};
use tokio::{
    sync::{mpsc, oneshot},
    time::timeout,
};
use zbus::{
    Connection, Proxy, fdo,
    names::OwnedUniqueName,
    zvariant::{OwnedObjectPath, OwnedValue},
};

pub const SERVICE: &str = "org.bluez";
pub const ADAPTER: &str = "org.bluez.Adapter1";
pub const DEVICE: &str = "org.bluez.Device1";
/// Where [`pair`] exports its agent on its private connection.
pub const AGENT_PATH: &str = "/org/canopa/BluetoothAgent";
const QUERY_TIMEOUT: Duration = Duration::from_secs(1);
/// At most this many adapters and devices are listed.
pub const MAX_ADAPTERS: usize = 8;
pub const MAX_DEVICES: usize = 32;

type Path = OwnedObjectPath;
type Properties = HashMap<String, OwnedValue>;

fn string(properties: &Properties, name: &str) -> Option<String> {
    Some(<&str>::try_from(properties.get(name)?).ok()?.to_owned())
}

fn flag(properties: &Properties, name: &str) -> bool {
    properties.get(name).and_then(|v| bool::try_from(v).ok()) == Some(true)
}

fn display_name(properties: &Properties, fallback: &str) -> String {
    let label: String = string(properties, "Alias")
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    if label.trim().is_empty() {
        fallback.into()
    } else {
        label
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adapter {
    pub path: Path,
    pub powered: bool,
    /// The alias without control characters, or the object path.
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub path: Path,
    pub adapter: Path,
    /// The alias without control characters, or the address.
    pub label: String,
    pub paired: bool,
    pub connected: bool,
}

/// Adapters by path, and the devices of powered adapters, connected first,
/// then paired, then by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// The BlueZ owner the snapshot came from; `None` when BlueZ is not running.
    pub owner: Option<OwnedUniqueName>,
    pub adapters: Vec<Adapter>,
    pub devices: Vec<Device>,
}

impl Snapshot {
    pub async fn query(connection: &Connection) -> Result<Self, Error> {
        let bus = fdo::DBusProxy::new(connection).await?;
        let owner = bus
            .get_name_owner(SERVICE.try_into().map_err(zbus::Error::from)?)
            .await?;
        let manager = proxy(
            connection,
            &owner,
            "/",
            "org.freedesktop.DBus.ObjectManager",
        )
        .await?;
        let objects: fdo::ManagedObjects = manager.call("GetManagedObjects", &()).await?;
        let mut adapters = Vec::new();
        let mut devices = Vec::new();
        for (path, interfaces) in objects {
            for (interface, properties) in interfaces {
                match interface.as_str() {
                    ADAPTER => adapters.push(Adapter {
                        label: display_name(&properties, path.as_str()),
                        powered: flag(&properties, "Powered"),
                        path: path.clone(),
                    }),
                    DEVICE => {
                        let Some(adapter) = properties
                            .get("Adapter")
                            .and_then(|v| Path::try_from(v.try_clone().ok()?).ok())
                        else {
                            continue;
                        };
                        let fallback = string(&properties, "Address")
                            .unwrap_or_else(|| "Unnamed device".into());
                        devices.push(Device {
                            label: display_name(&properties, &fallback),
                            adapter,
                            path: path.clone(),
                            paired: flag(&properties, "Paired"),
                            connected: flag(&properties, "Connected"),
                        });
                    }
                    _ => {}
                }
            }
        }
        adapters.sort_by(|a, b| a.path.cmp(&b.path));
        adapters.truncate(MAX_ADAPTERS);
        devices.retain(|device| {
            adapters
                .iter()
                .any(|a| a.powered && a.path == device.adapter)
        });
        devices.sort_by(|a, b| {
            (!a.connected, !a.paired, &a.label, a.path.as_str()).cmp(&(
                !b.connected,
                !b.paired,
                &b.label,
                b.path.as_str(),
            ))
        });
        devices.truncate(MAX_DEVICES);
        Ok(Self {
            owner: Some(owner),
            adapters,
            devices,
        })
    }

    /// [`Snapshot::query`] within a second; an empty snapshot (no owner) when
    /// BlueZ is absent or does not answer.
    pub async fn read(connection: &Connection) -> Self {
        timeout(QUERY_TIMEOUT, Self::query(connection))
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default()
    }
}

/// The snapshot re-read on each BlueZ signal (property changes, devices
/// appearing during discovery), with recovery when BlueZ restarts.
pub fn watch(connection: Connection) -> StateStream<Snapshot> {
    crate::bus::watch_service(connection, SERVICE, |connection| async move {
        timeout(QUERY_TIMEOUT, Snapshot::query(&connection)).await?
    })
}

async fn proxy<'a>(
    connection: &'a Connection,
    owner: &OwnedUniqueName,
    path: &str,
    interface: &str,
) -> zbus::Result<Proxy<'a>> {
    Proxy::new_owned(
        connection.clone(),
        owner.clone(),
        path.to_owned(),
        interface.to_owned(),
    )
    .await
}

/// Turn an adapter on or off.
pub async fn set_powered(
    connection: &Connection,
    owner: &OwnedUniqueName,
    adapter: &str,
    powered: bool,
) -> Result<(), Error> {
    proxy(connection, owner, adapter, ADAPTER)
        .await?
        .set_property("Powered", powered)
        .await?;
    Ok(())
}

/// Start searching. BlueZ ends this connection's discovery when it closes.
pub async fn start_discovery(
    connection: &Connection,
    owner: &OwnedUniqueName,
    adapter: &str,
) -> Result<(), Error> {
    Ok(proxy(connection, owner, adapter, ADAPTER)
        .await?
        .call::<_, _, ()>("StartDiscovery", &())
        .await?)
}

pub async fn stop_discovery(
    connection: &Connection,
    owner: &OwnedUniqueName,
    adapter: &str,
) -> Result<(), Error> {
    Ok(proxy(connection, owner, adapter, ADAPTER)
        .await?
        .call::<_, _, ()>("StopDiscovery", &())
        .await?)
}

/// Connect a paired device.
pub async fn connect(
    connection: &Connection,
    owner: &OwnedUniqueName,
    device: &str,
) -> Result<(), Error> {
    Ok(proxy(connection, owner, device, DEVICE)
        .await?
        .call::<_, _, ()>("Connect", &())
        .await?)
}

pub async fn disconnect(
    connection: &Connection,
    owner: &OwnedUniqueName,
    device: &str,
) -> Result<(), Error> {
    Ok(proxy(connection, owner, device, DEVICE)
        .await?
        .call::<_, _, ()>("Disconnect", &())
        .await?)
}

/// One pairing attempt: the device and the BlueZ owner it belongs to. The
/// serial distinguishes events of successive attempts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub serial: u64,
    pub owner: OwnedUniqueName,
    pub device: Path,
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum AgentError {
    Canceled,
    Rejected,
}

/// What BlueZ asks the person during pairing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Question {
    /// Does the other device show this six-digit code?
    Confirm(u32),
    /// Allow this pairing request?
    Authorize,
    /// Allow this device to connect to a service?
    Service,
    /// Enter the PIN shown by the other device (up to 16 letters or digits).
    Pin,
    /// Enter the six-digit passkey shown by the other device.
    Passkey,
    /// Type this PIN on the other device.
    DisplayPinCode(String),
    /// Type this passkey on the other device; `entered` digits are typed so far.
    DisplayPasskey { passkey: u32, entered: u16 },
}

impl Question {
    /// Whether this question expects typed text.
    pub fn needs_entry(&self) -> bool {
        matches!(self, Self::Pin | Self::Passkey)
    }

    /// The reply to send for `value`, or `None` when BlueZ would reject it.
    /// Display questions take no reply.
    pub fn accepts(&self, value: &str) -> Option<String> {
        let accepted = match self {
            Self::Pin => {
                !value.is_empty()
                    && value.len() <= 16
                    && value.bytes().all(|b| b.is_ascii_alphanumeric())
            }
            Self::Passkey => value.len() == 6 && value.bytes().all(|b| b.is_ascii_digit()),
            Self::DisplayPinCode(_) | Self::DisplayPasskey { .. } => false,
            _ => true,
        };
        accepted.then(|| value.to_owned())
    }
}

/// A question, and where to send its reply. Dropping the sender cancels the
/// request; display questions have none.
pub struct PairingRequest {
    pub question: Question,
    pub reply: Option<oneshot::Sender<String>>,
}

/// Agent output for one [`Attempt`], tagged with its serial.
pub enum AgentEvent {
    Request(u64, PairingRequest),
    /// BlueZ cancelled the open request.
    Dismiss(u64),
}

struct Agent {
    attempt: Attempt,
    events: mpsc::Sender<AgentEvent>,
}

impl Agent {
    fn identify(
        &self,
        header: &zbus::message::Header<'_>,
        device: Option<&Path>,
    ) -> Result<u64, AgentError> {
        if header.sender().map(|s| s.as_str()) != Some(self.attempt.owner.as_str())
            || device.is_some_and(|device| device != &self.attempt.device)
        {
            return Err(AgentError::Rejected);
        }
        Ok(self.attempt.serial)
    }

    async fn ask(&self, serial: u64, question: Question) -> Result<String, AgentError> {
        let (reply, result) = oneshot::channel();
        self.events
            .try_send(AgentEvent::Request(
                serial,
                PairingRequest {
                    question,
                    reply: Some(reply),
                },
            ))
            .map_err(|_| AgentError::Canceled)?;
        result.await.map_err(|_| AgentError::Canceled)
    }

    fn display(&self, serial: u64, question: Question) -> Result<(), AgentError> {
        self.events
            .try_send(AgentEvent::Request(
                serial,
                PairingRequest {
                    question,
                    reply: None,
                },
            ))
            .map_err(|_| AgentError::Canceled)
    }
}

#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    async fn request_pin_code(
        &self,
        device: Path,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<String, AgentError> {
        self.ask(self.identify(&header, Some(&device))?, Question::Pin)
            .await
    }

    async fn request_passkey(
        &self,
        device: Path,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<u32, AgentError> {
        self.ask(self.identify(&header, Some(&device))?, Question::Passkey)
            .await?
            .parse()
            .map_err(|_| AgentError::Rejected)
    }

    async fn request_confirmation(
        &self,
        device: Path,
        passkey: u32,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), AgentError> {
        self.ask(
            self.identify(&header, Some(&device))?,
            Question::Confirm(passkey),
        )
        .await
        .map(drop)
    }

    async fn request_authorization(
        &self,
        device: Path,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), AgentError> {
        self.ask(self.identify(&header, Some(&device))?, Question::Authorize)
            .await
            .map(drop)
    }

    async fn authorize_service(
        &self,
        device: Path,
        _uuid: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), AgentError> {
        self.ask(self.identify(&header, Some(&device))?, Question::Service)
            .await
            .map(drop)
    }

    fn display_pin_code(
        &self,
        device: Path,
        pincode: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), AgentError> {
        let serial = self.identify(&header, Some(&device))?;
        self.display(serial, Question::DisplayPinCode(pincode.to_owned()))
    }

    fn display_passkey(
        &self,
        device: Path,
        passkey: u32,
        entered: u16,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), AgentError> {
        let serial = self.identify(&header, Some(&device))?;
        self.display(serial, Question::DisplayPasskey { passkey, entered })
    }

    fn cancel(&self, #[zbus(header)] header: zbus::message::Header<'_>) {
        if let Ok(serial) = self.identify(&header, None) {
            let _ = self.events.try_send(AgentEvent::Dismiss(serial));
        }
    }

    fn release(&self, #[zbus(header)] header: zbus::message::Header<'_>) {
        self.cancel(header);
    }
}

/// Pair, trust and connect the attempt's device on a private system-bus
/// connection, sending agent questions to `events`. Completing `stop` (or
/// dropping its sender) ends the attempt; BlueZ cancels pairing when the
/// private connection closes.
pub async fn pair(
    attempt: Attempt,
    events: mpsc::Sender<AgentEvent>,
    stop: oneshot::Receiver<()>,
) -> Result<(), Error> {
    pair_with(attempt, events, stop, || async {
        Ok(timeout(QUERY_TIMEOUT, Connection::system()).await??)
    })
    .await
}

/// [`pair`] on a connection from `connect`, for tests on a private bus.
pub async fn pair_with<Connect, Fut>(
    attempt: Attempt,
    events: mpsc::Sender<AgentEvent>,
    mut stop: oneshot::Receiver<()>,
    connect: Connect,
) -> Result<(), Error>
where
    Connect: FnOnce() -> Fut,
    Fut: Future<Output = Result<Connection, Error>>,
{
    // BlueZ watches the unique sender of Pair and cancels that attempt when
    // it disconnects. CancelPairing can also cancel someone else's attempt.
    let connection = tokio::select! {
        biased;
        _ = &mut stop => return Ok(()),
        result = connect() => result?,
    };
    let result = tokio::select! {
        biased;
        _ = &mut stop => Ok(()),
        result = pair_on_bus(&connection, attempt, events) => result,
    };
    let _ = timeout(QUERY_TIMEOUT, connection.close()).await;
    result
}

async fn pair_on_bus(
    connection: &Connection,
    attempt: Attempt,
    events: mpsc::Sender<AgentEvent>,
) -> Result<(), Error> {
    connection
        .object_server()
        .at(
            AGENT_PATH,
            Agent {
                attempt: attempt.clone(),
                events,
            },
        )
        .await?;
    let manager = proxy(
        connection,
        &attempt.owner,
        "/org/bluez",
        "org.bluez.AgentManager1",
    )
    .await?;
    let device = proxy(connection, &attempt.owner, attempt.device.as_str(), DEVICE).await?;
    let path = Path::try_from(AGENT_PATH).map_err(zbus::Error::from)?;
    timeout(
        QUERY_TIMEOUT,
        manager.call::<_, _, ()>("RegisterAgent", &(&path, "KeyboardDisplay")),
    )
    .await??;
    timeout(
        Duration::from_secs(60),
        device.call::<_, _, ()>("Pair", &()),
    )
    .await??;
    timeout(QUERY_TIMEOUT, device.set_property("Trusted", true)).await??;
    if !timeout(QUERY_TIMEOUT, device.get_property::<bool>("Connected")).await?? {
        timeout(
            Duration::from_secs(8),
            device.call::<_, _, ()>("Connect", &()),
        )
        .await??;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
