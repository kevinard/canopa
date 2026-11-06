//! Private capture producer and isolated EI Receiver. This is not portal consent.
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{os::unix::net::UnixStream, path::PathBuf, time::Duration};
use tokio::sync::watch;
use zbus::{Connection, names::OwnedUniqueName};

mod parent;
mod server;
#[cfg(test)]
mod tests;
mod wire;
mod worker;
pub use parent::{Bridge, Connector, Revoker};
pub use wire::{Barriers, Command, Notice, Reply, Zones};
pub use worker::run_worker;

const SERVICE: &str = "org.canopa.Niri.InputCapture1";
const PATH: &str = "/org/canopa/Niri/InputCapture1";
const CALL_TIME: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct Endpoint {
    owner: OwnedUniqueName,
    pid: u32,
    socket: PathBuf,
    capabilities: u32,
}
impl Endpoint {
    /// Read-only capability discovery; no capture lease is opened before approval.
    pub async fn resolve(connection: &Connection, socket: PathBuf) -> anyhow::Result<Self> {
        tokio::time::timeout(CALL_TIME, async {
            anyhow::ensure!(socket.is_absolute(), "Capture catalog unavailable");
            let bus = zbus::fdo::DBusProxy::new(connection).await?;
            let owner = bus.get_name_owner(SERVICE.try_into()?).await?;
            let pid = bus
                .get_connection_unix_process_id(owner.clone().into())
                .await?;
            let ipc = tokio::net::UnixStream::connect(&socket).await?;
            anyhow::ensure!(
                ipc.peer_cred()?.pid().and_then(|p| u32::try_from(p).ok()) == Some(pid),
                "Capture catalog belongs to another compositor"
            );
            let proxy = proxy(connection, owner.clone()).await?;
            let version = proxy.get_property::<u32>("Version").await?;
            let advertised = proxy.get_property::<u32>("SupportedCapabilities").await?;
            let capabilities = provider_capabilities(version, advertised)?;
            anyhow::ensure!(
                bus.get_name_owner(SERVICE.try_into()?).await? == owner,
                "Capture owner changed during discovery"
            );
            Ok(Self {
                owner,
                pid,
                socket,
                capabilities,
            })
        })
        .await?
    }
    pub fn capabilities(&self) -> u32 {
        self.capabilities
    }
    pub fn process_id(&self) -> u32 {
        self.pid
    }
    pub(crate) async fn watch(
        &self,
        connection: &Connection,
        caller: OwnedUniqueName,
        stop: watch::Sender<bool>,
    ) -> anyhow::Result<Watch> {
        let input = Input {
            compositor: self.owner.to_string(),
            compositor_pid: self.pid,
            ipc_socket: self.socket.clone(),
            backend: connection
                .unique_name()
                .ok_or_else(|| anyhow::anyhow!("Capture backend has no owner"))?
                .to_string(),
            caller: caller.to_string(),
            grants: self.capabilities,
        };
        tokio::time::timeout(
            CALL_TIME,
            observe(
                connection,
                &input,
                rustix::process::getpid().as_raw_nonzero().get() as u32,
                stop,
            ),
        )
        .await?
    }
}
fn provider_capabilities(version: u32, advertised: u32) -> anyhow::Result<u32> {
    let known = match version {
        1 => 2,
        2 => 3,
        _ => anyhow::bail!("Unsupported capture provider"),
    };
    let capabilities = advertised & known;
    anyhow::ensure!(capabilities != 0, "No supported capture capability");
    Ok(capabilities)
}
async fn proxy(connection: &Connection, owner: OwnedUniqueName) -> zbus::Result<zbus::Proxy<'_>> {
    zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
        .destination(owner)?
        .path(PATH)?
        .interface(SERVICE)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    compositor: String,
    compositor_pid: u32,
    ipc_socket: PathBuf,
    backend: String,
    caller: String,
    grants: u32,
}
impl Input {
    fn validate(&self) -> anyhow::Result<()> {
        for name in [&self.compositor, &self.backend, &self.caller] {
            anyhow::ensure!(name.len() <= 256, "Capture owner name too large");
            let _: zbus::names::UniqueName<'_> = name.as_str().try_into()?;
        }
        anyhow::ensure!(
            self.compositor_pid != 0
                && self.ipc_socket.is_absolute()
                && self.grants != 0
                && self.grants & !3 == 0,
            "Unsupported capture configuration"
        );
        Ok(())
    }
}
pub(crate) struct Watch(tokio::task::JoinHandle<()>);
impl Drop for Watch {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn observe(
    connection: &Connection,
    input: &Input,
    parent_pid: u32,
    stop: watch::Sender<bool>,
) -> anyhow::Result<Watch> {
    let bus = zbus::fdo::DBusProxy::new(connection).await?;
    let mut compositor = bus
        .receive_name_owner_changed_with_args(&[(0, SERVICE)])
        .await?;
    let mut backend = bus
        .receive_name_owner_changed_with_args(&[(0, input.backend.as_str())])
        .await?;
    let mut caller = bus
        .receive_name_owner_changed_with_args(&[(0, input.caller.as_str())])
        .await?;
    anyhow::ensure!(
        bus.get_name_owner(SERVICE.try_into()?).await?.as_str() == input.compositor
            && bus
                .get_connection_unix_process_id(input.compositor.as_str().try_into()?)
                .await?
                == input.compositor_pid
            && bus
                .get_connection_unix_process_id(input.backend.as_str().try_into()?)
                .await?
                == parent_pid
            && bus
                .name_has_owner(input.caller.as_str().try_into()?)
                .await?,
        "Capture owner unavailable"
    );
    let ipc = tokio::net::UnixStream::connect(&input.ipc_socket).await?;
    anyhow::ensure!(
        ipc.peer_cred()?.pid().and_then(|p| u32::try_from(p).ok()) == Some(input.compositor_pid),
        "Capture catalog changed process"
    );
    Ok(Watch(tokio::spawn(async move {
        tokio::select! {_=compositor.next()=>{},_=backend.next()=>{},_=caller.next()=>{}}
        stop.send_replace(true);
    })))
}

fn socket(fd: std::os::fd::OwnedFd, pid: u32) -> anyhow::Result<UnixStream> {
    use rustix::net::{self, AddressFamily, SocketType};
    anyhow::ensure!(
        net::sockopt::socket_type(&fd)? == SocketType::STREAM
            && net::sockopt::socket_domain(&fd)? == AddressFamily::UNIX
            && u32::try_from(
                net::sockopt::socket_peercred(&fd)?
                    .pid
                    .as_raw_nonzero()
                    .get()
            )
            .ok()
                == Some(pid),
        "Capture FD belongs to another process"
    );
    let socket = UnixStream::from(fd);
    socket.set_nonblocking(true)?;
    Ok(socket)
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub zone_set: u32,
    pub zones: Zones,
    pub connected: bool,
    pub receiver_ready: bool,
    pub enabled: bool,
    pub activation: Option<u32>,
    pub zones_valid: bool,
}
impl Snapshot {
    fn new(zone_set: u32, zones: Zones) -> Self {
        Self {
            zone_set,
            zones,
            connected: false,
            receiver_ready: false,
            enabled: false,
            activation: None,
            zones_valid: true,
        }
    }
    fn notice(&mut self, notice: &Notice) {
        match notice {
            Notice::ReceiverReady { ready } => self.receiver_ready = *ready,
            Notice::Activated { activation, .. } => self.activation = Some(*activation),
            Notice::Deactivated { .. } => self.activation = None,
            Notice::Disabled => {
                self.enabled = false;
                self.activation = None;
            }
            Notice::ZonesChanged { .. } => {
                self.enabled = false;
                self.activation = None;
                self.zones_valid = false;
            }
        }
    }
}
