//! Parent ownership of an anonymous EI FD and its separately retained worker lease.
use super::*;
use crate::process::DialogProcess;
use crate::process::fd::receive;
use std::{process::Stdio, sync::Arc};
use tokio::{
    io::unix::AsyncFd,
    sync::{Semaphore, oneshot},
};
use zbus::names::OwnedUniqueName;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    owner: OwnedUniqueName,
    pid: u32,
    socket: PathBuf,
}
impl Endpoint {
    /// Resolve without activation and bind the bus endpoint to the IPC process.
    pub async fn resolve(connection: &Connection, socket: PathBuf) -> anyhow::Result<Self> {
        tokio::time::timeout(CALL_TIME, async {
            anyhow::ensure!(socket.is_absolute(), "Input catalog unavailable");
            let bus = zbus::fdo::DBusProxy::new(connection).await?;
            let owner = bus.get_name_owner(SERVICE.try_into()?).await?;
            let pid = bus
                .get_connection_unix_process_id(owner.clone().into())
                .await?;
            let stream = tokio::net::UnixStream::connect(&socket).await?;
            anyhow::ensure!(
                stream
                    .peer_cred()?
                    .pid()
                    .and_then(|p| u32::try_from(p).ok())
                    == Some(pid),
                "Input catalog belongs to another compositor"
            );
            let proxy = zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
                .destination(owner.clone())?
                .path(PATH)?
                .interface(SERVICE)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await?;
            anyhow::ensure!(
                proxy.get_property::<u32>("Version").await? == 1,
                "Unsupported input lease version"
            );
            Ok(Self { owner, pid, socket })
        })
        .await?
    }
    pub fn process_id(&self) -> u32 {
        self.pid
    }
    /// Read-only negotiation against the pinned input owner. Captured absolute
    /// input needs the atomic scale/mapping descriptor; input-only pointer
    /// sessions can use the relative lease without capture metadata.
    pub(crate) async fn devices(
        &self,
        connection: &Connection,
        capture: bool,
    ) -> anyhow::Result<u32> {
        tokio::time::timeout(CALL_TIME, async {
            let bus = zbus::fdo::DBusProxy::new(connection).await?;
            anyhow::ensure!(
                bus.get_name_owner(SERVICE.try_into()?).await? == self.owner,
                "Input owner changed before capability query"
            );
            let proxy = zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
                .destination(self.owner.clone())?
                .path(PATH)?
                .interface(SERVICE)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await?;
            async fn version(proxy: &zbus::Proxy<'_>, name: &str) -> anyhow::Result<Option<u32>> {
                match proxy.get_property::<u32>(name).await {
                    Ok(value) => Ok(Some(value)),
                    Err(zbus::Error::FDO(error))
                        if matches!(*error, zbus::fdo::Error::UnknownProperty(_)) =>
                    {
                        Ok(None)
                    }
                    Err(zbus::Error::MethodError(name, _, _))
                        if name.as_str() == "org.freedesktop.DBus.Error.UnknownProperty" =>
                    {
                        Ok(None)
                    }
                    Err(error) => Err(error.into()),
                }
            }
            let pointer = version(&proxy, "PointerVersion").await? == Some(1)
                && (!capture || version(&proxy, "MappedPointerVersion").await? == Some(2));
            let touch = version(&proxy, "TouchVersion").await? == Some(1);
            anyhow::ensure!(
                bus.get_name_owner(SERVICE.try_into()?).await? == self.owner,
                "Input owner changed during capability query"
            );
            Ok::<_, anyhow::Error>(1 | if pointer { 2 } else { 0 } | if touch { 4 } else { 0 })
        })
        .await?
    }
    fn input(
        &self,
        connection: &Connection,
        caller: OwnedUniqueName,
        grants: u32,
        transport: Transport,
        capture: Option<crate::screencast_capture::Bindings>,
    ) -> anyhow::Result<Input> {
        let input = Input {
            compositor: self.owner.to_string(),
            compositor_pid: self.pid,
            ipc_socket: self.socket.clone(),
            backend: connection
                .unique_name()
                .ok_or_else(|| anyhow::anyhow!("Input backend needs a bus connection"))?
                .to_string(),
            caller: caller.to_string(),
            grants,
            transport,
            capture,
        };
        input.validate()?;
        Ok(input)
    }
    /// Observe loss before consent, without opening a keyboard lease or EIS worker.
    pub(crate) async fn observe(
        &self,
        connection: &Connection,
        caller: OwnedUniqueName,
        stop: watch::Sender<bool>,
    ) -> anyhow::Result<Watch> {
        let input = self.input(connection, caller, 1, Transport::Eis, None)?;
        tokio::time::timeout(CALL_TIME, super::observe(connection, &input, stop)).await?
    }
}

#[derive(Clone)]
pub struct Connector {
    process: Arc<DialogProcess>,
    slots: Arc<Semaphore>,
}
impl Connector {
    pub fn new(process: Arc<DialogProcess>) -> Self {
        Self {
            process,
            slots: Arc::new(Semaphore::new(8)),
        }
    }
    /// Call only for a freshly approved active session. Transfer the FD to its
    /// recipient; retain only InputLease, otherwise client loss cannot produce EOF.
    pub async fn start(
        &self,
        connection: &Connection,
        endpoint: &Endpoint,
        caller: OwnedUniqueName,
        grants: u32,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<(OwnedFd, InputLease)> {
        let input = endpoint.input(connection, caller, grants, Transport::Eis, None)?;
        let permit = self.slots.clone().try_acquire_owned()?;
        launch(&self.process, input, cancelled, permit).await
    }
    /// EI devices paired to the approved recording's opaque input bindings.
    pub async fn start_bound(
        &self,
        connection: &Connection,
        endpoint: &Endpoint,
        caller: OwnedUniqueName,
        grants: u32,
        capture: Option<&crate::screencast_capture::Bindings>,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<(OwnedFd, InputLease)> {
        let input = endpoint.input(connection, caller, grants, Transport::Eis, capture.cloned())?;
        let permit = self.slots.clone().try_acquire_owned()?;
        launch(&self.process, input, cancelled, permit).await
    }
    /// Private pointer command socket for a freshly approved session. This is
    /// not an EI FD; absolute selectors are limited to this recording's bindings.
    pub async fn start_pointer(
        &self,
        connection: &Connection,
        endpoint: &Endpoint,
        caller: OwnedUniqueName,
        capture: Option<&crate::screencast_capture::Bindings>,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<super::PointerInput> {
        let input = endpoint.input(
            connection,
            caller,
            2,
            Transport::LegacyPointer,
            capture.cloned(),
        )?;
        let permit = self.slots.clone().try_acquire_owned()?;
        let (fd, lease) = launch(&self.process, input, cancelled, permit).await?;
        super::PointerInput::new(fd, lease)
    }
    /// Touch commands always use this recording's approved live capture bindings.
    pub async fn start_touch(
        &self,
        connection: &Connection,
        endpoint: &Endpoint,
        caller: OwnedUniqueName,
        capture: &crate::screencast_capture::Bindings,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<super::TouchInput> {
        let input = endpoint.input(
            connection,
            caller,
            4,
            Transport::LegacyTouch,
            Some(capture.clone()),
        )?;
        let permit = self.slots.clone().try_acquire_owned()?;
        let (fd, lease) = launch(&self.process, input, cancelled, permit).await?;
        super::TouchInput::new(fd, lease)
    }
    /// Internal typed input socket, never an EI descriptor for an application.
    /// Uses the same worker admission and authenticated endpoint as EIS.
    pub async fn start_legacy(
        &self,
        connection: &Connection,
        endpoint: &Endpoint,
        caller: OwnedUniqueName,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<super::LegacyInput> {
        let input = endpoint.input(connection, caller, 1, Transport::LegacyKeyboard, None)?;
        let permit = self.slots.clone().try_acquire_owned()?;
        let (fd, lease) = launch(&self.process, input, cancelled, permit).await?;
        super::LegacyInput::new(fd, lease)
    }
}

pub struct InputLease {
    release: Option<oneshot::Sender<()>>,
    closed: watch::Receiver<bool>,
    done: oneshot::Receiver<anyhow::Result<()>>,
}
impl InputLease {
    #[cfg(test)]
    pub(super) fn fixture(
        release: oneshot::Sender<()>,
        closed: watch::Receiver<bool>,
        done: oneshot::Receiver<anyhow::Result<()>>,
    ) -> Self {
        Self {
            release: Some(release),
            closed,
            done,
        }
    }
    pub(super) fn revoke(&mut self) {
        self.release.take();
    }
    pub fn closed(&self) -> watch::Receiver<bool> {
        self.closed.clone()
    }
    pub async fn stop(mut self) -> anyhow::Result<()> {
        self.release.take();
        (&mut self.done).await?
    }
}
impl Drop for InputLease {
    fn drop(&mut self) {
        self.release.take();
    }
}

async fn launch(
    process: &DialogProcess,
    input: Input,
    mut cancelled: watch::Receiver<bool>,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> anyhow::Result<(OwnedFd, InputLease)> {
    anyhow::ensure!(!*cancelled.borrow(), "Input cancelled");
    let retirement = if input.grants & 6 != 0 { 20 } else { 10 };
    let mut bytes = serde_json::to_vec(&input)?;
    anyhow::ensure!(bytes.len() < FRAME_BYTES, "Input worker request too large");
    bytes.push(b'\n');
    let (parent, child_socket) = UnixStream::pair()?;
    parent.set_nonblocking(true)?;
    // SAFETY: the stream owns its socket descriptor until the AsyncFd is dropped.
    let output = unsafe { AsyncFd::register(parent) }.map_err(std::io::Error::from)?;
    // The pipe lease, rather than PDEATHSIG, permits cleanup after backend death.
    let mut child = tokio::process::Command::new(&process.executable)
        .args(&process.arguments)
        .arg("--remote-input-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(OwnedFd::from(child_socket)))
        .env_remove("XDG_ACTIVATION_TOKEN")
        .env_remove("DESKTOP_STARTUP_ID")
        .kill_on_drop(false)
        .spawn()?;
    let pid = child.id().expect("spawned input worker");
    let mut pipe = child.stdin.take().expect("piped input");
    let (release, mut released) = oneshot::channel();
    let (ready, response) = oneshot::channel();
    let (done, finished) = oneshot::channel();
    let (closed, stopped) = watch::channel(false);
    tokio::spawn(async move {
        let interaction = async {
            pipe.write_all(&bytes).await?;
            pipe.flush().await?;
            loop {
                let mut readiness = output.readable().await?;
                match readiness.try_io(|output| receive(output.get_ref(), pid)) {
                    Ok(reply) => return Ok::<_, anyhow::Error>(reply?),
                    Err(_) => continue,
                }
            }
        };
        let result = tokio::select! {
            biased;
            _ = &mut released => Err(anyhow::anyhow!("Input cancelled")),
            _ = async { if !*cancelled.borrow() { let _ = cancelled.changed().await; } } => Err(anyhow::anyhow!("Input cancelled")),
            result = tokio::time::timeout(Duration::from_secs(30), interaction) => result.map_err(anyhow::Error::from).and_then(|r| r),
        };
        drop(cancelled); // Request completion does not own the active lease.
        if let Ok(fd) = result {
            if ready.send(Ok(fd)).is_ok() {
                tokio::select! { _ = &mut released => {}, _ = child.wait() => {} }
            }
        } else {
            let _ = ready.send(result);
        }
        drop(output);
        drop(pipe);
        let result = match tokio::time::timeout(Duration::from_secs(retirement), child.wait()).await
        {
            Ok(Ok(status)) if status.success() => Ok(()),
            Ok(_) => Err(anyhow::anyhow!("Input worker cleanup unconfirmed")),
            Err(_) => {
                let _ = child.kill().await;
                Err(anyhow::anyhow!("Input worker did not retire"))
            }
        };
        drop(permit); // Hold admission until the child has actually retired.
        closed.send_replace(true);
        let _ = done.send(result);
    });
    let fd = response.await??;
    Ok((
        fd,
        InputLease {
            release: Some(release),
            closed: stopped,
            done: finished,
        },
    ))
}

#[cfg(test)]
mod tests;
