//! ScreenCast requests configure a separately owned long-lived session.
use crate::{
    requests::{Completion, Options, Requests, option},
    screencast_capture::{Cursor, Geometry, Source},
};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
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

mod capabilities;
#[cfg(feature = "desktop")]
pub(crate) mod production;
pub mod remote;
mod session;
#[cfg(feature = "desktop")]
pub(crate) use production::refresh;
#[cfg(feature = "desktop")]
pub use production::{DialogInput, Item, Production};

pub struct Unavailable;
impl Presenter for Unavailable {
    fn present<'a>(
        &'a self,
        _: Connection,
        _: StartRequest,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn Capture>>>> {
        Box::pin(async { anyhow::bail!("ScreenCast requires desktop capture support") })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectionOptions {
    pub types: u32,
    pub multiple: bool,
    pub cursor: Cursor,
}
impl SelectionOptions {
    fn parse(mut options: Options) -> fdo::Result<Self> {
        let types: u32 = option(&mut options, "types")?.unwrap_or(1);
        if types == 0 || types & !3 != 0 {
            return Err(fdo::Error::InvalidArgs(
                "Unsupported capture source type".into(),
            ));
        }
        let cursor = match option(&mut options, "cursor_mode")?.unwrap_or(1_u32) {
            1 => Cursor::Hidden,
            2 => Cursor::Embedded,
            4 => {
                #[cfg(feature = "desktop")]
                if capabilities::cursor_modes() & 4 == 0 {
                    return Err(fdo::Error::InvalidArgs(
                        "Metadata cursor mode requires PipeWire 1.4.8 or newer".into(),
                    ));
                }
                Cursor::Metadata
            }
            _ => {
                return Err(fdo::Error::InvalidArgs(
                    "Unsupported capture cursor mode".into(),
                ));
            }
        };
        if option::<u32>(&mut options, "persist_mode")?.unwrap_or(0) > 2 {
            return Err(fdo::Error::InvalidArgs(
                "Invalid capture persistence mode".into(),
            ));
        }
        // Always ask again. Restore data never grants a source in this policy.
        Ok(Self {
            types,
            multiple: option(&mut options, "multiple")?.unwrap_or(false),
            cursor,
        })
    }
}

#[derive(Clone, Debug)]
pub struct StartRequest {
    pub app_id: String,
    pub caller: OwnedUniqueName,
    pub parent_window: String,
    pub modal: bool,
    pub activation_token: Option<String>,
    pub options: SelectionOptions,
}
#[derive(Clone, Debug)]
pub struct Stream {
    pub node: u32,
    pub serial: u64,
    pub source: Source,
    pub geometry: Option<Geometry>,
    pub mapping: Option<StreamMapping>,
}
/// Public pairing metadata; compositor object paths stay in the owned capture.
#[derive(Clone, Debug)]
pub struct StreamMapping {
    pub id: String,
    pub size: (i32, i32),
}
/// Owns producer lifetime after the Start request has disappeared.
pub trait Capture: Send + 'static {
    /// Private identity-checked bindings for input ownership, never portal results.
    fn input_bindings(&self) -> Option<&crate::screencast_capture::Bindings> {
        None
    }
    fn streams(&self) -> &[Stream];
    fn closed(&self) -> watch::Receiver<bool>;
    fn stop(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>>;
}
pub trait Presenter: Send + Sync + 'static {
    fn present<'a>(
        &'a self,
        connection: Connection,
        request: StartRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn Capture>>>>;
}

fn context(value: &str) -> fdo::Result<()> {
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(fdo::Error::InvalidArgs("Invalid capture context".into()));
    }
    Ok(())
}
fn sender(header: &Header<'_>) -> fdo::Result<OwnedUniqueName> {
    Ok(header
        .sender()
        .ok_or_else(|| fdo::Error::AccessDenied("Missing sender".into()))?
        .to_owned()
        .into())
}

#[derive(Clone)]
struct Handle {
    kind: session::Kind,
    owner: OwnedUniqueName,
    app: String,
    commands: mpsc::Sender<session::Command>,
    close: watch::Sender<bool>,
    notify: Arc<AtomicBool>,
    done: watch::Receiver<bool>,
}
struct Host {
    entries: Mutex<HashMap<OwnedObjectPath, Handle>>,
    slots: Arc<Semaphore>,
    shutdown: AtomicBool,
    presenter: Arc<dyn Presenter>,
    remote_presenter: Arc<dyn remote::Presenter>,
}
#[derive(Clone)]
pub struct ScreenCast {
    host: Arc<Host>,
    requests: Requests,
}
impl ScreenCast {
    pub fn with_requests(presenter: Arc<dyn Presenter>, requests: Requests) -> Self {
        Self::with_presenters(presenter, Arc::new(remote::Unavailable), requests)
    }
    pub fn with_presenters(
        presenter: Arc<dyn Presenter>,
        remote_presenter: Arc<dyn remote::Presenter>,
        requests: Requests,
    ) -> Self {
        Self {
            host: Arc::new(Host {
                entries: Mutex::new(HashMap::new()),
                slots: Arc::new(Semaphore::new(16)),
                shutdown: AtomicBool::new(false),
                presenter,
                remote_presenter,
            }),
            requests,
        }
    }
    pub(crate) fn clipboard_command(
        &self,
        key: OwnedObjectPath,
        owner: OwnedUniqueName,
        command: crate::clipboard::Command,
    ) -> fdo::Result<oneshot::Receiver<fdo::Result<crate::clipboard::Response>>> {
        let handle = self
            .host
            .entries
            .lock()
            .expect("sharing sessions")
            .get(&key)
            .cloned()
            .ok_or_else(|| fdo::Error::InvalidArgs("Clipboard session unavailable".into()))?;
        if handle.kind != session::Kind::Remote || handle.owner != owner || *handle.close.borrow() {
            return Err(fdo::Error::AccessDenied(
                "Clipboard session belongs to another caller or interface".into(),
            ));
        }
        let (reply, result) = oneshot::channel();
        handle
            .commands
            .try_send(session::Command::Clipboard(command, reply))
            .map_err(|_| {
                handle.close.send_replace(true);
                fdo::Error::LimitsExceeded("Clipboard session is busy or stopped".into())
            })?;
        Ok(result)
    }
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.host.shutdown.store(true, Ordering::SeqCst);
        let sessions: Vec<_> = self
            .host
            .entries
            .lock()
            .expect("capture sessions")
            .values()
            .cloned()
            .collect();
        for session in &sessions {
            session.close.send_replace(true);
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            for mut session in sessions {
                while !*session.done.borrow() {
                    session.done.changed().await?;
                }
            }
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        Ok(())
    }
    fn lookup(
        &self,
        key: &OwnedObjectPath,
        app: &str,
        owner: OwnedUniqueName,
    ) -> fdo::Result<Handle> {
        let sessions = self.host.entries.lock().expect("capture sessions");
        let session = sessions
            .get(key)
            .ok_or_else(|| fdo::Error::InvalidArgs("Invalid capture session".into()))?;
        if session.owner != owner || session.app != app {
            return Err(fdo::Error::AccessDenied(
                "Capture session belongs to another caller".into(),
            ));
        }
        if *session.close.borrow() {
            return Err(fdo::Error::InvalidArgs("Capture session is closed".into()));
        }
        Ok(session.clone())
    }
}

struct Guard {
    close: watch::Sender<bool>,
    armed: bool,
}
impl Drop for Guard {
    fn drop(&mut self) {
        if self.armed {
            self.close.send_replace(true);
        }
    }
}
struct Accepted {
    options: Options,
    guard: Guard,
}
impl Accepted {
    fn new(options: Options, close: watch::Sender<bool>) -> Self {
        Self {
            options,
            guard: Guard { close, armed: true },
        }
    }
    fn confirm(mut self) -> Option<Options> {
        if *self.guard.close.borrow() {
            return None;
        }
        self.guard.armed = false;
        Some(self.options)
    }
}
enum Outcome {
    Accepted(Accepted),
    Cancelled,
    Failed,
}
fn response(result: Completion<fdo::Result<Outcome>>) -> fdo::Result<(u32, Options)> {
    Ok(match result {
        Completion::Cancelled | Completion::Finished(Ok(Outcome::Cancelled)) => (1, Options::new()),
        Completion::Finished(Ok(Outcome::Failed)) => (2, Options::new()),
        Completion::Finished(Err(error)) => return Err(error),
        Completion::Finished(Ok(Outcome::Accepted(reply))) => match reply.confirm() {
            Some(options) => (0, options),
            None => (1, Options::new()),
        },
    })
}

#[zbus::interface(name = "org.freedesktop.impl.portal.ScreenCast")]
impl ScreenCast {
    #[allow(clippy::too_many_arguments)]
    #[zbus(out_args("response", "results"))]
    async fn create_session(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        app_id: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        context(&app_id)?;
        let _ = options;
        response(
            self.requests
                .run(connection, header, handle, |owner, _cancelled| async move {
                    self.host
                        .create(
                            connection,
                            session_handle,
                            app_id,
                            owner,
                            session::Kind::Capture,
                        )
                        .await
                })
                .await?,
        )
    }
    #[allow(clippy::too_many_arguments)]
    #[zbus(out_args("response", "results"))]
    async fn select_sources(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        app_id: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        let session = self.lookup(&session_handle, &app_id, sender(&header)?)?;
        let options = match SelectionOptions::parse(options) {
            Ok(options) => options,
            Err(error) => {
                session.close.send_replace(true);
                return Err(error);
            }
        };
        response(
            self.requests
                .run(connection, header, handle, |_, _cancelled| async move {
                    let (reply, result) = oneshot::channel();
                    session
                        .commands
                        .send(session::Command::Select(options, reply))
                        .await
                        .map_err(|_| fdo::Error::Failed("Capture session stopped".into()))?;
                    result
                        .await
                        .map_err(|_| fdo::Error::Failed("Capture session stopped".into()))?
                })
                .await?,
        )
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
        if session.kind != session::Kind::Capture {
            return Err(fdo::Error::InvalidArgs(
                "Remote sessions must start through RemoteDesktop".into(),
            ));
        }
        context(&parent_window)?;
        let activation_token: Option<String> = option(&mut options, "activation_token")?;
        if let Some(token) = &activation_token {
            context(token)?;
        }
        let modal = option(&mut options, "modal")?.unwrap_or(true);
        response(
            self.requests
                .run(connection, header, handle, |_, cancelled| async move {
                    let (reply, result) = oneshot::channel();
                    session
                        .commands
                        .send(session::Command::Start {
                            kind: session::Kind::Capture,
                            response_serial: 0,
                            parent_window,
                            modal,
                            activation_token,
                            cancelled,
                            reply,
                        })
                        .await
                        .map_err(|_| fdo::Error::Failed("Capture session stopped".into()))?;
                    result
                        .await
                        .map_err(|_| fdo::Error::Failed("Capture session stopped".into()))?
                })
                .await?,
        )
    }
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        6
    }
    #[zbus(property)]
    fn available_source_types(&self) -> u32 {
        if cfg!(feature = "desktop") { 3 } else { 0 }
    }
    #[zbus(property)]
    fn available_cursor_modes(&self) -> u32 {
        capabilities::cursor_modes()
    }
}

fn results(streams: &[Stream], options: &SelectionOptions) -> anyhow::Result<Options> {
    anyhow::ensure!(
        !streams.is_empty() && streams.len() <= 16 && (options.multiple || streams.len() == 1),
        "Invalid capture stream count"
    );
    let mut nodes = HashSet::new();
    let mut serials = HashSet::new();
    let mut sources = HashSet::new();
    let mut rows = Vec::new();
    let mut mappings = HashSet::new();
    for stream in streams {
        let source_type = match stream.source {
            Source::Monitor(_) => 1_u32,
            Source::Window(_) => 2_u32,
        };
        anyhow::ensure!(
            stream.node != 0
                && stream.node != u32::MAX
                && stream.serial != 0
                && nodes.insert(stream.node)
                && serials.insert(stream.serial)
                && sources.insert(stream.source.clone())
                && source_type & options.types != 0,
            "Invalid capture stream identity"
        );
        let mut properties = Options::from([
            ("source_type".into(), source_type.into()),
            ("pipewire-serial".into(), stream.serial.into()),
        ]);
        if let Some(mapping) = &stream.mapping {
            anyhow::ensure!(
                crate::screencast_capture::bindings::valid_mapping_id(&mapping.id)
                    && crate::screencast_capture::bindings::valid_size(mapping.size)
                    && mappings.insert(&mapping.id)
                    && stream
                        .geometry
                        .is_none_or(|geometry| geometry.size == mapping.size),
                "Invalid capture mapping metadata"
            );
            properties.insert(
                "mapping_id".into(),
                Value::from(mapping.id.as_str()).try_into_owned()?,
            );
            properties.insert("size".into(), Value::from(mapping.size).try_into_owned()?);
        }
        if let Some(geometry) = stream.geometry {
            anyhow::ensure!(
                source_type == 1
                    && geometry.size.0 > 0
                    && geometry.size.1 > 0
                    && geometry.size.0 <= 32768
                    && geometry.size.1 <= 32768,
                "Invalid capture geometry"
            );
            properties.insert(
                "position".into(),
                Value::from(geometry.position).try_into_owned()?,
            );
            properties.insert("size".into(), Value::from(geometry.size).try_into_owned()?);
        }
        rows.push((stream.node, properties));
    }
    Ok(Options::from([
        ("streams".into(), Value::from(rows).try_into_owned()?),
        ("persist_mode".into(), 0_u32.into()),
    ]))
}
