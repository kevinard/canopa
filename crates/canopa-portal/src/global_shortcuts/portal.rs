use super::{Actions, editor, history::History, session, transport};
use crate::requests::{Completion, Options, Requests, option};
use futures_util::{StreamExt, future::BoxFuture};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Semaphore, mpsc, oneshot, watch};
use zbus::{Connection, fdo, message::Header, names::OwnedUniqueName, zvariant::OwnedObjectPath};

pub trait Connector: Send + Sync + 'static {
    fn open<'a>(
        &'a self,
        connection: &'a Connection,
    ) -> BoxFuture<'a, anyhow::Result<transport::Lease>>;
}
pub struct Niri {
    socket: Option<PathBuf>,
}
impl Default for Niri {
    fn default() -> Self {
        Self {
            socket: std::env::var_os("NIRI_SOCKET").map(PathBuf::from),
        }
    }
}
impl Connector for Niri {
    fn open<'a>(
        &'a self,
        connection: &'a Connection,
    ) -> BoxFuture<'a, anyhow::Result<transport::Lease>> {
        Box::pin(async move {
            let socket = self
                .socket
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Shortcut compositor unavailable"))?;
            transport::Endpoint::resolve(connection, socket)
                .await?
                .open(connection)
                .await
        })
    }
}
pub(super) fn app_id(value: &str) -> fdo::Result<()> {
    context(value)?;
    if value.is_empty() || value.len() > 256 {
        return Err(super::invalid("Invalid shortcut application ID"));
    }
    Ok(())
}
fn context(value: &str) -> fdo::Result<()> {
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(super::invalid("Invalid shortcut context"));
    }
    Ok(())
}
pub(super) fn no_fds(header: &Header<'_>) -> fdo::Result<()> {
    if header.unix_fds().unwrap_or(0) != 0 {
        return Err(super::invalid("Shortcut methods do not accept descriptors"));
    }
    Ok(())
}
pub(super) fn sender(header: &Header<'_>) -> fdo::Result<OwnedUniqueName> {
    Ok(header
        .sender()
        .ok_or_else(|| fdo::Error::AccessDenied("Missing sender".into()))?
        .to_owned()
        .into())
}
pub(super) fn failed() -> fdo::Error {
    fdo::Error::Failed("Shortcut session stopped".into())
}
pub(super) async fn stopped(value: &mut watch::Receiver<bool>) {
    while !*value.borrow() {
        if value.changed().await.is_err() {
            return;
        }
    }
}
pub(super) async fn retired(mut value: watch::Receiver<bool>) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !*value.borrow() {
            value.changed().await?;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok(())
}
#[derive(Clone)]
pub(super) struct Handle {
    pub owner: OwnedUniqueName,
    pub app: String,
    pub commands: mpsc::Sender<session::Command>,
    pub close: watch::Sender<bool>,
    pub done: watch::Receiver<bool>,
    pub notify: Arc<AtomicBool>,
    pub attempted: Arc<AtomicBool>,
    pub bound: Arc<AtomicBool>,
    pub editing: Arc<AtomicBool>,
}
impl Handle {
    pub fn queue(&self, command: session::Command) -> fdo::Result<()> {
        if *self.close.borrow() {
            return Err(failed());
        }
        self.commands.try_send(command).map_err(|_| {
            fdo::Error::LimitsExceeded("Shortcut session queue is full or stopped".into())
        })
    }
}
pub(super) struct Host {
    pub entries: Mutex<HashMap<OwnedObjectPath, Handle>>,
    pub slots: Arc<Semaphore>,
    pub shutdown: AtomicBool,
    pub connector: Arc<dyn Connector>,
    pub presenter: Arc<dyn editor::Presenter>,
    pub history: History,
}
pub(super) struct Accepted {
    pub options: Options,
    pub close: watch::Sender<bool>,
    pub armed: bool,
}
impl Drop for Accepted {
    fn drop(&mut self) {
        if self.armed {
            self.close.send_replace(true);
        }
    }
}
pub(super) enum Outcome {
    Accepted(Accepted),
    Cancelled,
}
pub(super) enum Listing {
    Live(Options),
    History,
}
#[derive(Clone)]
pub struct GlobalShortcuts {
    host: Arc<Host>,
    requests: Requests,
}
impl GlobalShortcuts {
    pub fn with_requests(
        connector: Arc<dyn Connector>,
        presenter: Arc<dyn editor::Presenter>,
        history: History,
        requests: Requests,
    ) -> Self {
        Self {
            host: Arc::new(Host {
                entries: Mutex::new(HashMap::new()),
                slots: Arc::new(Semaphore::new(16)),
                shutdown: AtomicBool::new(false),
                connector,
                presenter,
                history,
            }),
            requests,
        }
    }
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.host.shutdown.store(true, Ordering::SeqCst);
        let sessions: Vec<_> = self
            .host
            .entries
            .lock()
            .expect("shortcut sessions")
            .values()
            .cloned()
            .collect();
        for session in &sessions {
            session.close.send_replace(true);
        }
        for session in sessions {
            retired(session.done).await?;
        }
        Ok(())
    }
    fn lookup(&self, key: &OwnedObjectPath, owner: OwnedUniqueName) -> fdo::Result<Handle> {
        let handle = self
            .host
            .entries
            .lock()
            .expect("shortcut sessions")
            .get(key)
            .cloned()
            .ok_or_else(|| super::invalid("Unknown shortcut session"))?;
        if handle.owner != owner {
            return Err(fdo::Error::AccessDenied(
                "Shortcut session belongs to another caller".into(),
            ));
        }
        if *handle.close.borrow() {
            return Err(failed());
        }
        Ok(handle)
    }
}
#[zbus::interface(name = "org.freedesktop.impl.portal.GlobalShortcuts")]
impl GlobalShortcuts {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }
    #[zbus(out_args("response", "results"))]
    async fn create_session(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        app_id: String,
        _options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        no_fds(&header)?;
        self::app_id(&app_id)?;
        let host = self.host.clone();
        let result = self
            .requests
            .run(
                connection,
                header,
                handle,
                |owner, mut cancelled| async move {
                    let session = host
                        .create(connection, session_handle, app_id, owner)
                        .await?;
                    let accepted = Accepted {
                        options: Options::new(),
                        close: session.close.clone(),
                        armed: true,
                    };
                    if *cancelled.borrow_and_update() {
                        return Ok(Outcome::Cancelled);
                    }
                    Ok(Outcome::Accepted(accepted))
                },
            )
            .await?;
        finish(result)
    }
    #[allow(clippy::too_many_arguments)]
    #[zbus(out_args("response", "results"))]
    async fn bind_shortcuts(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        shortcuts: Vec<(String, Options)>,
        parent_window: String,
        _options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        no_fds(&header)?;
        let session = self.lookup(&session_handle, sender(&header)?)?;
        if session.attempted.swap(true, Ordering::SeqCst) {
            return Err(fdo::Error::AccessDenied(
                "Shortcut binding was already attempted".into(),
            ));
        }
        context(&parent_window)?;
        let actions = Actions::parse(shortcuts)?;
        session.editing.store(true, Ordering::SeqCst);
        let editing = session.editing.clone();
        let result = self
            .requests
            .run(connection, header, handle, |_, cancelled| async move {
                let (reply, result) = oneshot::channel();
                session.queue(session::Command::Bind {
                    actions,
                    parent_window,
                    cancelled,
                    reply,
                })?;
                result.await.map_err(|_| failed())?
            })
            .await;
        // The actor also clears this after retiring its editor. A failed admission
        // has no editor; Configure is unavailable until a successful Bind anyway.
        if result.is_err() {
            editing.store(false, Ordering::SeqCst);
        }
        finish(result?)
    }
    #[zbus(out_args("response", "results"))]
    async fn list_shortcuts(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        no_fds(&header)?;
        let session = self.lookup(&session_handle, sender(&header)?)?;
        let history = self.host.history.clone();
        let result = self
            .requests
            .run(connection, header, handle, |_, _| async move {
                let (reply, result) = oneshot::channel();
                session.queue(session::Command::List(reply))?;
                let options = match result.await.map_err(|_| failed())?? {
                    Listing::Live(options) => options,
                    Listing::History => history
                        .load(session.app.clone())
                        .await
                        .map_err(|e| fdo::Error::Failed(e.to_string()))?
                        .response()
                        .map_err(|e| fdo::Error::Failed(e.to_string()))?,
                };
                if *session.close.borrow() {
                    return Err(failed());
                }
                Ok(options)
            })
            .await?;
        match result {
            Completion::Cancelled => Ok((1, Options::new())),
            Completion::Finished(result) => Ok((0, result?)),
        }
    }
    async fn configure_shortcuts(
        &self,
        session_handle: OwnedObjectPath,
        parent_window: String,
        mut options: Options,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        no_fds(&header)?;
        let session = self.lookup(&session_handle, sender(&header)?)?;
        context(&parent_window)?;
        let activation_token = option::<String>(&mut options, "activation_token")?;
        if let Some(token) = &activation_token {
            context(token)?;
        }
        if !session.bound.load(Ordering::SeqCst) {
            return Err(fdo::Error::AccessDenied(
                "Shortcuts have not been bound".into(),
            ));
        }
        if session.editing.swap(true, Ordering::SeqCst) {
            return Err(fdo::Error::LimitsExceeded(
                "Shortcut configuration is already open".into(),
            ));
        }
        if let Err(error) = session.queue(session::Command::Configure {
            parent_window,
            activation_token,
        }) {
            session.editing.store(false, Ordering::SeqCst);
            return Err(error);
        }
        Ok(())
    }
    #[zbus(signal)]
    async fn activated(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        session_handle: OwnedObjectPath,
        shortcut_id: String,
        timestamp: u64,
        options: Options,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn deactivated(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        session_handle: OwnedObjectPath,
        shortcut_id: String,
        timestamp: u64,
        options: Options,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn shortcuts_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        session_handle: OwnedObjectPath,
        shortcuts: Vec<(String, Options)>,
    ) -> zbus::Result<()>;
}
fn finish(result: Completion<fdo::Result<Outcome>>) -> fdo::Result<(u32, Options)> {
    match result {
        Completion::Cancelled | Completion::Finished(Ok(Outcome::Cancelled)) => {
            Ok((1, Options::new()))
        }
        Completion::Finished(Err(error)) => Err(error),
        Completion::Finished(Ok(Outcome::Accepted(mut accepted))) => {
            if *accepted.close.borrow() {
                return Ok((1, Options::new()));
            }
            accepted.armed = false;
            Ok((0, std::mem::take(&mut accepted.options)))
        }
    }
}
impl Host {
    async fn create(
        self: &Arc<Self>,
        connection: &Connection,
        key: OwnedObjectPath,
        app: String,
        owner: OwnedUniqueName,
    ) -> fdo::Result<Handle> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(failed());
        }
        if key.as_str().len() > 512
            || !key
                .as_str()
                .starts_with("/org/freedesktop/portal/desktop/session/")
        {
            return Err(super::invalid("Invalid shortcut session path"));
        }
        let slot = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| fdo::Error::LimitsExceeded("Too many shortcut sessions".into()))?;
        let bus = fdo::DBusProxy::new(connection).await?;
        let owners = bus
            .receive_name_owner_changed_with_args(&[(0, owner.as_str())])
            .await?;
        if !bus.name_has_owner(owner.clone().into()).await? {
            return Err(failed());
        }
        let (commands, inbox) = mpsc::channel(16);
        let (close, _) = watch::channel(false);
        let (done, ended) = watch::channel(false);
        let handle = Handle {
            owner,
            app,
            commands,
            close,
            done: ended,
            notify: Arc::new(AtomicBool::new(true)),
            attempted: Arc::new(AtomicBool::new(false)),
            bound: Arc::new(AtomicBool::new(false)),
            editing: Arc::new(AtomicBool::new(false)),
        };
        if !connection
            .object_server()
            .at(key.clone(), session::Endpoint(handle.clone()))
            .await?
        {
            return Err(super::invalid("Shortcut session already exists"));
        }
        if self.shutdown.load(Ordering::SeqCst) {
            handle.close.send_replace(true);
        }
        self.entries
            .lock()
            .expect("shortcut sessions")
            .insert(key.clone(), handle.clone());
        tokio::spawn(session::run(
            self.clone(),
            connection.clone(),
            key,
            handle.clone(),
            inbox,
            owners.map(|_| ()).boxed(),
            done,
            slot,
        ));
        Ok(handle)
    }
}
