//! Clipboard extends an already owned, explicitly approved sharing session.
use crate::{
    requests::{Options, option},
    screencast::ScreenCast,
};
use futures_util::{future::BoxFuture, stream::BoxStream};
use std::os::fd::OwnedFd;
use tokio::sync::watch;
use zbus::{
    fdo,
    message::Header,
    object_server::SignalEmitter,
    zvariant::{OwnedObjectPath, Value},
};

mod dispatch;
#[cfg(feature = "desktop")]
pub(crate) mod production;
pub use dispatch::Clipboard;

pub(crate) const MAX_TYPES: usize = 128;
pub(crate) const MAX_MIME_BYTES: usize = 256;
pub(crate) const OWNER_PREFIX: &str = "application/x-canopa-portal-owner-";
pub(crate) fn validate_mime(mime: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !mime.is_empty()
            && mime.len() <= MAX_MIME_BYTES
            && !mime.chars().any(char::is_control)
            && !mime.starts_with(OWNER_PREFIX),
        "Invalid clipboard MIME type"
    );
    Ok(())
}
pub(crate) fn validate_types(types: &[String]) -> anyhow::Result<()> {
    anyhow::ensure!(types.len() <= MAX_TYPES, "Too many clipboard MIME types");
    let mut unique = std::collections::HashSet::new();
    for mime in types {
        validate_mime(mime)?;
        anyhow::ensure!(unique.insert(mime), "Duplicate clipboard MIME type");
    }
    Ok(())
}

pub enum Command {
    Request,
    Publish(Vec<String>),
    Read(String),
    Write(u32),
    Done(u32, bool),
}
pub enum Response {
    Unit,
    Fd(OwnedFd),
}
pub enum Event {
    OwnerChanged {
        mime_types: Vec<String>,
        session_is_owner: bool,
    },
    Transfer {
        mime_type: String,
        serial: u32,
    },
}
/// The owning session controls admission, lifetime and when events may be emitted.
pub trait Control: Send + 'static {
    fn events(&mut self) -> Option<BoxStream<'static, Event>>;
    fn perform<'a>(
        &'a mut self,
        command: Command,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, fdo::Result<Response>>;
}
#[derive(Clone)]
struct Methods {
    sharing: ScreenCast,
    capture: Option<crate::input_capture::InputCapture>,
}
impl Methods {
    fn queue(
        &self,
        key: OwnedObjectPath,
        command: Command,
        header: Header<'_>,
    ) -> fdo::Result<tokio::sync::oneshot::Receiver<fdo::Result<Response>>> {
        match &command {
            Command::Publish(types) => {
                validate_types(types).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?
            }
            Command::Read(mime) => {
                validate_mime(mime).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?
            }
            _ => {}
        }
        let owner = header
            .sender()
            .ok_or_else(|| fdo::Error::AccessDenied("Missing sender".into()))?
            .to_owned()
            .into();
        if let Some(capture) = &self.capture
            && capture.has_session(&key)
        {
            return capture.clipboard_command(key, owner, command);
        }
        self.sharing.clipboard_command(key, owner, command)
    }
    async fn perform(
        &self,
        key: OwnedObjectPath,
        command: Command,
        header: Header<'_>,
    ) -> fdo::Result<Response> {
        self.queue(key, command, header)?
            .await
            .map_err(|_| fdo::Error::Failed("Clipboard session stopped".into()))?
    }
    async fn unit(
        &self,
        key: OwnedObjectPath,
        command: Command,
        header: Header<'_>,
    ) -> fdo::Result<()> {
        match self.perform(key, command, header).await? {
            Response::Unit => Ok(()),
            _ => Err(fdo::Error::Failed(
                "Invalid clipboard transport response".into(),
            )),
        }
    }
    async fn fd(
        &self,
        key: OwnedObjectPath,
        command: Command,
        header: Header<'_>,
    ) -> fdo::Result<zbus::zvariant::OwnedFd> {
        match self.perform(key, command, header).await? {
            Response::Fd(fd) => Ok(fd.into()),
            _ => Err(fdo::Error::Failed(
                "Invalid clipboard transport response".into(),
            )),
        }
    }
    pub(crate) async fn emit(
        connection: &zbus::Connection,
        owner: &zbus::names::OwnedUniqueName,
        key: &OwnedObjectPath,
        event: Event,
    ) -> anyhow::Result<()> {
        let emitter = SignalEmitter::new(connection, crate::OBJECT_PATH)?
            .set_destination(owner.clone().into());
        match event {
            Event::OwnerChanged {
                mime_types,
                session_is_owner,
            } => {
                validate_types(&mime_types)?;
                let options = Options::from([
                    ("mime_types".into(), Value::from(mime_types).try_into()?),
                    ("session_is_owner".into(), session_is_owner.into()),
                ]);
                Self::selection_owner_changed(&emitter, key, options).await?;
            }
            Event::Transfer { mime_type, serial } => {
                validate_mime(&mime_type)?;
                Self::selection_transfer(&emitter, key, &mime_type, serial).await?;
            }
        }
        Ok(())
    }
}
#[zbus::interface(name = "org.freedesktop.impl.portal.Clipboard")]
impl Methods {
    async fn request_clipboard(
        &self,
        session_handle: OwnedObjectPath,
        options: Options,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let _ = options;
        self.unit(session_handle, Command::Request, header).await
    }
    async fn set_selection(
        &self,
        session_handle: OwnedObjectPath,
        mut options: Options,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let types: Vec<String> = option(&mut options, "mime_types")?.unwrap_or_default();
        validate_types(&types).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?;
        self.unit(session_handle, Command::Publish(types), header)
            .await
    }
    #[zbus(out_args("fd"))]
    async fn selection_write(
        &self,
        session_handle: OwnedObjectPath,
        serial: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<zbus::zvariant::OwnedFd> {
        self.fd(session_handle, Command::Write(serial), header)
            .await
    }
    async fn selection_write_done(
        &self,
        session_handle: OwnedObjectPath,
        serial: u32,
        success: bool,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        self.unit(session_handle, Command::Done(serial, success), header)
            .await
    }
    #[zbus(out_args("fd"))]
    async fn selection_read(
        &self,
        session_handle: OwnedObjectPath,
        mime_type: String,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<zbus::zvariant::OwnedFd> {
        validate_mime(&mime_type).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?;
        self.fd(session_handle, Command::Read(mime_type), header)
            .await
    }
    #[zbus(signal)]
    async fn selection_owner_changed(
        emitter: &SignalEmitter<'_>,
        session_handle: &OwnedObjectPath,
        options: Options,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn selection_transfer(
        emitter: &SignalEmitter<'_>,
        session_handle: &OwnedObjectPath,
        mime_type: &str,
        serial: u32,
    ) -> zbus::Result<()>;
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }
}
