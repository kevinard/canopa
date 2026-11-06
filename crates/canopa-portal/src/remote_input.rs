//! Session-owned EIS workers. This private transport does not publish a portal grant.
use crate::process::fd::send_ready;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, Read},
    os::{
        fd::{AsFd, OwnedFd},
        unix::net::UnixStream,
    },
    path::PathBuf,
    time::Duration,
};
use tokio::{io::AsyncWriteExt, sync::watch};
use zbus::{Connection, names::UniqueName};

mod parent;
pub use parent::{Connector, Endpoint, InputLease};
mod legacy;
pub use legacy::{LegacyInput, Status as KeyboardStatus};
mod pending;
mod pointer;
mod server;
mod status;
mod touch;
pub use pointer::{Command as PointerCommand, PointerInput, Status as PointerStatus};
pub use touch::{Command as TouchCommand, Status as TouchStatus, TouchInput};

pub(crate) const SERVICE: &str = "org.canopa.Niri.RemoteInput1";
pub(crate) const PATH: &str = "/org/canopa/Niri/RemoteInput1";
const CALL_TIME: Duration = Duration::from_secs(5);
const FRAME_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Transport {
    #[default]
    Eis,
    LegacyKeyboard,
    LegacyPointer,
    LegacyTouch,
}
impl Transport {
    fn is_eis(&self) -> bool {
        matches!(self, Self::Eis)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    compositor: String,
    compositor_pid: u32,
    ipc_socket: PathBuf,
    backend: String,
    caller: String,
    grants: u32,
    #[serde(default, skip_serializing_if = "Transport::is_eis")]
    transport: Transport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capture: Option<crate::screencast_capture::Bindings>,
}
impl Input {
    fn validate(&self) -> anyhow::Result<()> {
        for name in [&self.compositor, &self.backend, &self.caller] {
            let _: UniqueName<'_> = name.as_str().try_into()?;
        }
        anyhow::ensure!(
            self.compositor_pid != 0 && self.ipc_socket.is_absolute(),
            "Invalid compositor identity"
        );
        match self.transport {
            Transport::LegacyKeyboard => anyhow::ensure!(
                self.grants == 1 && self.capture.is_none(),
                "Unsupported keyboard grant"
            ),
            Transport::Eis => {
                anyhow::ensure!((1..=7).contains(&self.grants), "Unsupported EI grant");
                anyhow::ensure!(
                    self.grants & 4 == 0 || self.capture.is_some(),
                    "Touch requires capture bindings"
                );
                if let Some(binding) = &self.capture {
                    anyhow::ensure!(
                        self.grants & 6 != 0,
                        "Capture requires a positioned input grant"
                    );
                    binding.validate()?;
                }
            }
            Transport::LegacyPointer => {
                anyhow::ensure!(self.grants == 2, "Unsupported pointer grant");
                if let Some(binding) = &self.capture {
                    binding.validate()?;
                }
            }
            Transport::LegacyTouch => {
                anyhow::ensure!(self.grants == 4, "Unsupported touch grant");
                self.capture
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Touch requires capture bindings"))?
                    .validate()?;
            }
        }
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
    stop: watch::Sender<bool>,
) -> anyhow::Result<Watch> {
    let bus = zbus::fdo::DBusProxy::new(connection).await?;
    let mut backend = bus
        .receive_name_owner_changed_with_args(&[(0, input.backend.as_str())])
        .await?;
    let mut caller = bus
        .receive_name_owner_changed_with_args(&[(0, input.caller.as_str())])
        .await?;
    let mut compositor = bus
        .receive_name_owner_changed_with_args(&[(0, SERVICE)])
        .await?;
    let mut capture = if input.capture.is_some() {
        Some(
            bus.receive_name_owner_changed_with_args(&[(0, crate::screencast_capture::SERVICE)])
                .await?,
        )
    } else {
        None
    };
    if let Some(binding) = &input.capture {
        anyhow::ensure!(
            bus.get_name_owner(crate::screencast_capture::SERVICE.try_into()?)
                .await?
                .as_str()
                == binding.compositor,
            "Capture owner unavailable"
        );
    }
    // Subscribe before validating; never activate or switch to a replacement owner.
    anyhow::ensure!(
        bus.get_name_owner(SERVICE.try_into()?).await?.as_str() == input.compositor
            && bus
                .name_has_owner(input.backend.as_str().try_into()?)
                .await?
            && bus
                .name_has_owner(input.caller.as_str().try_into()?)
                .await?
            && bus
                .get_connection_unix_process_id(input.compositor.as_str().try_into()?)
                .await?
                == input.compositor_pid,
        "Input owner unavailable"
    );
    let ipc = tokio::net::UnixStream::connect(&input.ipc_socket).await?;
    anyhow::ensure!(
        ipc.peer_cred()?.pid().and_then(|v| u32::try_from(v).ok()) == Some(input.compositor_pid),
        "Input catalog belongs to another compositor"
    );
    Ok(Watch(tokio::spawn(async move {
        tokio::select! { _=backend.next()=>{}, _=caller.next()=>{}, _=compositor.next()=>{},
        _=async { match &mut capture { Some(stream) => { stream.next().await; }, None => std::future::pending().await } }=>{} }
        stop.send_replace(true);
    })))
}

async fn run(
    input: Input,
    stop: watch::Sender<bool>,
    ready: impl FnOnce(UnixStream) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    input.validate()?;
    let mut stopped = stop.subscribe();
    let connection = tokio::time::timeout(CALL_TIME, Connection::session()).await??;
    let _watch = tokio::time::timeout(CALL_TIME, observe(&connection, &input, stop)).await??;
    anyhow::ensure!(!*stopped.borrow(), "Input cancelled");
    let proxy = zbus::proxy::Builder::<zbus::Proxy<'_>>::new(&connection)
        .destination(input.compositor.as_str())?
        .path(PATH)?
        .interface(SERVICE)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    let version: u32 = tokio::time::timeout(CALL_TIME, proxy.get_property("Version")).await??;
    anyhow::ensure!(version == 1, "Unsupported input lease version");
    if matches!(input.transport, Transport::LegacyPointer) {
        return pointer::run(&connection, &input, &mut stopped, ready).await;
    }
    if matches!(input.transport, Transport::LegacyTouch) {
        return touch::run(&connection, &input, &mut stopped, ready).await;
    }
    if matches!(input.transport, Transport::Eis) {
        return server::run(&connection, &proxy, &input, &mut stopped, ready).await;
    }
    if matches!(input.transport, Transport::LegacyKeyboard) {
        let version: u32 =
            tokio::time::timeout(CALL_TIME, proxy.get_property("SymbolVersion")).await??;
        anyhow::ensure!(version == 1, "Unsupported symbol lease version");
    }
    let (mut lease, map, _mods) = open_method(&proxy, input.transport).await?;
    let (server, client) = UnixStream::pair()?;
    let result = async {
        anyhow::ensure!(!*stopped.borrow(), "Input cancelled");
        anyhow::ensure!(
            !map.is_empty() && map.len() < 1024 * 1024 && !map.as_bytes().contains(&0),
            "Invalid input keymap"
        );
        ready(client)?;
        legacy::bridge(server, &mut lease, &mut stopped).await
    }
    .await;
    release(&mut lease).await?;
    result
}

type Lease = (tokio::net::UnixStream, String, [u32; 4]);
async fn open(proxy: &zbus::Proxy<'_>) -> anyhow::Result<Lease> {
    open_method(proxy, Transport::Eis).await
}
async fn open_method(proxy: &zbus::Proxy<'_>, transport: Transport) -> anyhow::Result<Lease> {
    let method = match transport {
        Transport::Eis => "OpenKeyboard",
        Transport::LegacyKeyboard => "OpenKeyboardSymbols",
        Transport::LegacyPointer => anyhow::bail!("Pointer transport requires capture bindings"),
        Transport::LegacyTouch => anyhow::bail!("Touch transport requires capture bindings"),
    };
    let (fd, map, mods): (zbus::zvariant::OwnedFd, String, (u32, u32, u32, u32)) =
        tokio::time::timeout(CALL_TIME, proxy.call(method, &())).await??;
    anyhow::ensure!(
        !map.is_empty() && map.len() < 1024 * 1024 && !map.as_bytes().contains(&0),
        "Invalid input keymap"
    );
    let socket = UnixStream::from(OwnedFd::from(fd));
    socket.set_nonblocking(true)?;
    Ok((
        tokio::net::UnixStream::from_std(socket)?,
        map,
        [mods.0, mods.1, mods.2, mods.3],
    ))
}

async fn release(lease: &mut tokio::net::UnixStream) -> anyhow::Result<()> {
    // Niri's EOF follows source release and is our cleanup acknowledgement.
    tokio::time::timeout(Duration::from_secs(2), async {
        let _ = lease.shutdown().await;
        let mut bytes = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(
            &mut tokio::io::AsyncReadExt::take(lease, 1024 * 1024 + 1),
            &mut bytes,
        )
        .await?;
        anyhow::ensure!(bytes.len() <= 1024 * 1024, "Input cleanup exceeded limit");
        Ok::<_, anyhow::Error>(())
    })
    .await?
}

/// Private child entry. Stdin is a bounded request followed by a parent lease;
/// stdout is an anonymous Unix socket carrying exactly one ready byte and EI FD.
pub fn run_worker() -> anyhow::Result<()> {
    let mut line = Vec::new();
    std::io::stdin()
        .lock()
        .take(FRAME_BYTES as u64 + 1)
        .read_until(b'\n', &mut line)?;
    anyhow::ensure!(
        line.len() <= FRAME_BYTES && line.last() == Some(&b'\n'),
        "Invalid input worker request"
    );
    let input: Input = serde_json::from_slice(&line)?;
    input.validate()?;
    let output = UnixStream::from(rustix::io::dup(std::io::stdout().as_fd())?);
    output.set_write_timeout(Some(Duration::from_secs(2)))?;
    let (stop, _) = watch::channel(false);
    let control = stop.clone();
    std::thread::spawn(move || {
        let _ = std::io::stdin().read(&mut [0]);
        control.send_replace(true);
    });
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
            let mut interrupt =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
            let control = stop.clone();
            let _signals = Watch(tokio::spawn(async move {
                tokio::select! { _=term.recv()=>{}, _=interrupt.recv()=>{} }
                control.send_replace(true);
            }));
            run(input, stop, |client| send_ready(&output, &client)).await
        })
}
