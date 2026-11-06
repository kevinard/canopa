//! Input capture owns consent, its receiver connection and compositor phases.
use crate::requests::{Completion, Options, Requests, option};
use futures_util::{StreamExt, future::BoxFuture, stream::BoxStream};
use std::{
    collections::HashMap,
    os::fd::OwnedFd,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Semaphore, mpsc, oneshot, watch};
use zbus::{
    Connection, fdo,
    message::Header,
    names::OwnedUniqueName,
    zvariant::{OwnedObjectPath, Value},
};

#[cfg(feature = "desktop")]
mod clipboard;
mod dispatch;
#[cfg(feature = "desktop")]
mod production;
mod session;
pub use dispatch::InputCapture;
#[cfg(feature = "desktop")]
pub use production::{DialogInput, Production};

pub const INTERFACE: &str = "org.freedesktop.impl.portal.InputCapture";
pub type Zones = Vec<(u32, u32, i32, i32)>;
pub type Barriers = Vec<(u32, (i32, i32, i32, i32))>;

#[derive(Clone, Debug)]
pub struct StartRequest {
    pub app_id: String,
    pub caller: OwnedUniqueName,
    pub parent_window: String,
    pub modal: bool,
    pub activation_token: Option<String>,
    pub capabilities: u32,
    pub clipboard: bool,
}
#[derive(Clone, Debug)]
pub enum Operation {
    GetZones,
    SetBarriers { zone_set: u32, barriers: Barriers },
    Enable,
    Disable,
    Release { activation: Option<u32> },
    Connect,
}
pub enum Output {
    Unit,
    Zones { zone_set: u32, zones: Zones },
    Barriers { failed: Vec<u32> },
    Fd(OwnedFd),
}
#[derive(Clone, Debug)]
pub enum Event {
    Activated {
        activation: u32,
        barrier: u32,
        position: (f64, f64),
    },
    Deactivated {
        activation: u32,
    },
    Disabled,
    ZonesChanged {
        zone_set: u32,
    },
}
pub struct ClipboardEvent {
    pub epoch: u64,
    pub event: crate::clipboard::Event,
}
/// Clipboard authority follows the real capture phase, independently of RPC I/O.
pub trait Clipboard: Send {
    fn phase(&self) -> watch::Receiver<Option<u64>>;
    fn events(&mut self) -> Option<BoxStream<'static, ClipboardEvent>>;
    fn synchronize<'a>(
        &'a mut self,
        activation: Option<u32>,
    ) -> BoxFuture<'a, anyhow::Result<Option<u64>>>;
    fn perform<'a>(
        &'a mut self,
        command: crate::clipboard::Command,
        epoch: Option<u64>,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, fdo::Result<crate::clipboard::Response>>;
}
pub trait Control: Send + 'static {
    fn clipboard_enabled(&self) -> bool {
        false
    }
    fn clipboard(&mut self) -> Option<&mut dyn Clipboard> {
        None
    }
    fn capabilities(&self) -> u32;
    fn closed(&self) -> watch::Receiver<bool>;
    fn events(&mut self) -> Option<mpsc::Receiver<Event>>;
    fn revoke(&self);
    fn perform<'a>(&'a mut self, operation: Operation) -> BoxFuture<'a, fdo::Result<Output>>;
    fn stop(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>>;
}
pub trait Presenter: Send + Sync + 'static {
    fn capabilities<'a>(&'a self, connection: &'a Connection) -> BoxFuture<'a, u32>;
    fn present<'a>(
        &'a self,
        connection: Connection,
        request: StartRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn Control>>>>;
}
pub struct Unavailable;
impl Presenter for Unavailable {
    fn capabilities<'a>(&'a self, _: &'a Connection) -> BoxFuture<'a, u32> {
        Box::pin(async { 0 })
    }
    fn present<'a>(
        &'a self,
        _: Connection,
        _: StartRequest,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn Control>>>> {
        Box::pin(async { anyhow::bail!("Input capture is unavailable") })
    }
}

fn invalid(message: &str) -> fdo::Error {
    fdo::Error::InvalidArgs(message.into())
}
fn stopped_error() -> fdo::Error {
    fdo::Error::Failed("Input capture session stopped".into())
}
fn sender(header: &Header<'_>) -> fdo::Result<OwnedUniqueName> {
    Ok(header
        .sender()
        .ok_or_else(|| fdo::Error::AccessDenied("Missing sender".into()))?
        .to_owned()
        .into())
}
fn context(value: &str) -> fdo::Result<()> {
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(invalid("Invalid input capture context"));
    }
    Ok(())
}
async fn stopped(receiver: &mut watch::Receiver<bool>) {
    while !*receiver.borrow() {
        if receiver.changed().await.is_err() {
            return;
        }
    }
}
async fn inactive(receiver: &mut watch::Receiver<Option<u64>>, epoch: u64) {
    loop {
        if *receiver.borrow() != Some(epoch) {
            return;
        }
        if receiver.changed().await.is_err() {
            return;
        }
    }
}
#[derive(Clone)]
struct Handle {
    owner: OwnedUniqueName,
    app: String,
    commands: mpsc::Sender<session::Command>,
    close: watch::Sender<bool>,
    notify: Arc<AtomicBool>,
    done: watch::Receiver<bool>,
    clipboard_epoch: watch::Receiver<Option<u64>>,
}
impl Handle {
    fn queue(&self, command: session::Command) -> fdo::Result<()> {
        if *self.close.borrow() {
            return Err(stopped_error());
        }
        self.commands.try_send(command).map_err(|_| {
            self.close.send_replace(true);
            fdo::Error::LimitsExceeded("Input capture queue is full or stopped".into())
        })
    }
}
struct Host {
    entries: Mutex<HashMap<OwnedObjectPath, Handle>>,
    slots: Arc<Semaphore>,
    shutdown: AtomicBool,
    presenter: Arc<dyn Presenter>,
}
#[derive(Clone)]
struct Methods {
    host: Arc<Host>,
    requests: Requests,
}
impl Methods {
    fn lookup(
        &self,
        key: &OwnedObjectPath,
        app: &str,
        owner: OwnedUniqueName,
    ) -> fdo::Result<Handle> {
        let entries = self.host.entries.lock().expect("capture sessions");
        let handle = entries
            .get(key)
            .ok_or_else(|| invalid("Unknown input capture session"))?;
        if handle.owner != owner || handle.app != app {
            return Err(fdo::Error::AccessDenied(
                "Input capture belongs to another caller".into(),
            ));
        }
        if *handle.close.borrow() {
            return Err(stopped_error());
        }
        Ok(handle.clone())
    }
    fn queue(
        &self,
        key: &OwnedObjectPath,
        app: &str,
        owner: OwnedUniqueName,
        operation: Operation,
    ) -> fdo::Result<oneshot::Receiver<fdo::Result<Output>>> {
        let handle = self.lookup(key, app, owner)?;
        let (reply, result) = oneshot::channel();
        handle.queue(session::Command::Operation(operation, reply))?;
        Ok(result)
    }
    async fn request_operation(
        &self,
        connection: &Connection,
        header: Header<'_>,
        request: OwnedObjectPath,
        key: OwnedObjectPath,
        app: String,
        operation: Operation,
    ) -> fdo::Result<(u32, Options)> {
        let handle = self.lookup(&key, &app, sender(&header)?)?;
        let result = self
            .requests
            .run(connection, header, request, |_, cancelled| async move {
                let (reply, result) = oneshot::channel();
                handle.queue(session::Command::Operation(operation, reply))?;
                tokio::select! {biased;
                    _=async {let mut cancelled=cancelled;stopped(&mut cancelled).await}=>{
                        handle.close.send_replace(true);Ok(None)
                    },
                    result=result=>result.map_err(|_|stopped_error())?.map(Some),
                }
            })
            .await?;
        match result {
            Completion::Cancelled | Completion::Finished(Ok(None)) => Ok((1, Options::new())),
            Completion::Finished(Ok(Some(output))) => Ok((0, values(output)?)),
            Completion::Finished(Err(error)) => Err(error),
        }
    }
}
fn values(output: Output) -> fdo::Result<Options> {
    Ok(match output {
        Output::Unit => Options::new(),
        Output::Zones { zone_set, zones } => HashMap::from([
            (
                "zone_set".into(),
                zbus::zvariant::OwnedValue::from(zone_set),
            ),
            (
                "zones".into(),
                Value::from(zones).try_into().map_err(|_| stopped_error())?,
            ),
        ]),
        Output::Barriers { failed } => HashMap::from([(
            "failed_barriers".into(),
            Value::from(failed)
                .try_into()
                .map_err(|_| stopped_error())?,
        )]),
        Output::Fd(_) => return Err(stopped_error()),
    })
}
struct Accepted {
    options: Options,
    close: watch::Sender<bool>,
    armed: bool,
}
impl Drop for Accepted {
    fn drop(&mut self) {
        if self.armed {
            self.close.send_replace(true);
        }
    }
}
enum Outcome {
    Accepted(Accepted),
    Cancelled,
    Failed,
}
async fn retired(mut done: watch::Receiver<bool>) -> fdo::Result<()> {
    tokio::time::timeout(Duration::from_secs(30), async {
        while !*done.borrow() {
            done.changed().await.map_err(|_| stopped_error())?;
        }
        Ok::<_, fdo::Error>(())
    })
    .await
    .map_err(|_| stopped_error())?
}
async fn response(
    result: Completion<fdo::Result<Outcome>>,
    close: watch::Sender<bool>,
    done: watch::Receiver<bool>,
) -> fdo::Result<(u32, Options)> {
    let status = match result {
        Completion::Cancelled | Completion::Finished(Ok(Outcome::Cancelled)) => 1,
        Completion::Finished(Ok(Outcome::Failed)) => 2,
        Completion::Finished(Err(error)) => return Err(error),
        Completion::Finished(Ok(Outcome::Accepted(mut accepted))) => {
            if !*accepted.close.borrow() {
                accepted.armed = false;
                return Ok((0, std::mem::take(&mut accepted.options)));
            }
            1
        }
    };
    close.send_replace(true);
    retired(done).await?;
    Ok((status, Options::new()))
}

#[zbus::interface(name = "org.freedesktop.impl.portal.InputCapture")]
impl Methods {
    #[allow(clippy::too_many_arguments)]
    #[zbus(out_args("response", "results"))]
    async fn create_session(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        context(&app_id)?;
        self.host
            .create(
                connection,
                session_handle.clone(),
                app_id.clone(),
                sender(&header)?,
            )
            .await?;
        let session = self.lookup(&session_handle, &app_id, sender(&header)?)?;
        let result = self
            .start(
                handle,
                session_handle,
                app_id,
                parent_window,
                options,
                connection,
                header,
            )
            .await;
        if result.is_err() {
            session.close.send_replace(true);
            retired(session.done).await?;
        }
        result
    }
    #[zbus(out_args("results"))]
    async fn create_session2(
        &self,
        session_handle: OwnedObjectPath,
        app_id: String,
        _options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<Options> {
        context(&app_id)?;
        self.host
            .create(connection, session_handle, app_id, sender(&header)?)
            .await?;
        Ok(Options::new())
    }
    #[allow(clippy::too_many_arguments)]
    #[zbus(out_args("response", "results"))]
    async fn start(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        mut options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        let session = self.lookup(&session_handle, &app_id, sender(&header)?)?;
        context(&parent_window)?;
        let capabilities = option::<u32>(&mut options, "capabilities")?
            .ok_or_else(|| invalid("Missing capture capabilities"))?;
        if capabilities == 0
            || capabilities & !7 != 0
            || option::<u32>(&mut options, "persist_mode")?.unwrap_or(0) > 2
        {
            return Err(invalid("Invalid input capture capabilities or persistence"));
        }
        let activation_token = option::<String>(&mut options, "activation_token")?;
        if let Some(token) = &activation_token {
            context(token)?;
        }
        let request = StartRequest {
            app_id,
            caller: session.owner.clone(),
            parent_window,
            modal: option(&mut options, "modal")?.unwrap_or(true),
            activation_token,
            capabilities,
            clipboard: false,
        };
        let retirement = session.done.clone();
        let close = session.close.clone();
        let serial = header.primary().serial_num().get();
        let result = self
            .requests
            .run(connection, header, handle, |_, cancelled| async move {
                let (reply, result) = oneshot::channel();
                session.queue(session::Command::Start {
                    request,
                    cancelled,
                    reply,
                    serial,
                })?;
                result.await.map_err(|_| stopped_error())?
            })
            .await?;
        response(result, close, retirement).await
    }
    #[allow(clippy::too_many_arguments)]
    #[zbus(out_args("response", "results"))]
    async fn get_zones(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        app_id: String,
        _options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        self.request_operation(
            connection,
            header,
            handle,
            session_handle,
            app_id,
            Operation::GetZones,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    #[zbus(out_args("response", "results"))]
    async fn set_pointer_barriers(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        app_id: String,
        _options: Options,
        barriers: Vec<Options>,
        zone_set: u32,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        if barriers.len() > 128 {
            return Err(invalid("Too many pointer barriers"));
        }
        let barriers = barriers
            .into_iter()
            .map(|mut barrier| {
                Ok((
                    option(&mut barrier, "barrier_id")?
                        .ok_or_else(|| invalid("Missing barrier ID"))?,
                    option(&mut barrier, "position")?
                        .ok_or_else(|| invalid("Missing barrier position"))?,
                ))
            })
            .collect::<fdo::Result<Barriers>>()?;
        self.request_operation(
            connection,
            header,
            handle,
            session_handle,
            app_id,
            Operation::SetBarriers { zone_set, barriers },
        )
        .await
    }
    #[zbus(out_args("response", "results"))]
    async fn enable(
        &self,
        session_handle: OwnedObjectPath,
        app_id: String,
        _options: Options,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        let output = self
            .queue(
                &session_handle,
                &app_id,
                sender(&header)?,
                Operation::Enable,
            )?
            .await
            .map_err(|_| stopped_error())??;
        Ok((0, values(output)?))
    }
    #[zbus(out_args("response", "results"))]
    async fn disable(
        &self,
        session_handle: OwnedObjectPath,
        app_id: String,
        _options: Options,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        let output = self
            .queue(
                &session_handle,
                &app_id,
                sender(&header)?,
                Operation::Disable,
            )?
            .await
            .map_err(|_| stopped_error())??;
        Ok((0, values(output)?))
    }
    #[zbus(out_args("response", "results"))]
    async fn release(
        &self,
        session_handle: OwnedObjectPath,
        app_id: String,
        mut options: Options,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        let activation = option(&mut options, "activation_id")?;
        if let Some((x, y)) = option::<(f64, f64)>(&mut options, "cursor_position")?
            && (!x.is_finite() || !y.is_finite())
        {
            return Err(invalid("Invalid release cursor position"));
        }
        let output = self
            .queue(
                &session_handle,
                &app_id,
                sender(&header)?,
                Operation::Release { activation },
            )?
            .await
            .map_err(|_| stopped_error())??;
        Ok((0, values(output)?))
    }
    #[zbus(name = "ConnectToEIS", out_args("fd"))]
    async fn connect_to_eis(
        &self,
        session_handle: OwnedObjectPath,
        app_id: String,
        _options: Options,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<zbus::zvariant::OwnedFd> {
        match self
            .queue(
                &session_handle,
                &app_id,
                sender(&header)?,
                Operation::Connect,
            )?
            .await
            .map_err(|_| stopped_error())??
        {
            Output::Fd(fd) => Ok(fd.into()),
            _ => Err(stopped_error()),
        }
    }
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }
    #[zbus(property)]
    async fn supported_capabilities(&self, #[zbus(connection)] connection: &Connection) -> u32 {
        self.host.presenter.capabilities(connection).await & 7
    }
    #[zbus(signal)]
    async fn activated(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        session_handle: OwnedObjectPath,
        options: Options,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn deactivated(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        session_handle: OwnedObjectPath,
        options: Options,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn disabled(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        session_handle: OwnedObjectPath,
        options: Options,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn zones_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        session_handle: OwnedObjectPath,
        options: Options,
    ) -> zbus::Result<()>;
}
