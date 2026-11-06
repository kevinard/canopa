//! Queue clipboard work in receive order, then await resource acknowledgements off
//! the object-server receive loop. RequestClipboard must precede a following Start.
use super::*;
use std::{collections::HashMap, fmt::Write};
use zbus::{
    ObjectServer,
    message::{Flags, Message},
    names::{InterfaceName, MemberName},
    object_server::{DispatchResult2, Interface},
};

#[derive(Clone)]
pub struct Clipboard {
    methods: Methods,
}
impl Clipboard {
    pub fn new(sharing: ScreenCast) -> Self {
        Self {
            methods: Methods {
                sharing,
                capture: None,
            },
        }
    }
    pub fn with_input_capture(
        sharing: ScreenCast,
        capture: crate::input_capture::InputCapture,
    ) -> Self {
        Self {
            methods: Methods {
                sharing,
                capture: Some(capture),
            },
        }
    }
    pub(crate) async fn emit(
        connection: &zbus::Connection,
        owner: &zbus::names::OwnedUniqueName,
        key: &OwnedObjectPath,
        event: Event,
    ) -> anyhow::Result<()> {
        Methods::emit(connection, owner, key, event).await
    }
    fn admit(
        &self,
        message: &Message,
        member: &str,
    ) -> fdo::Result<(bool, tokio::sync::oneshot::Receiver<fdo::Result<Response>>)> {
        fn args<T: serde::de::DeserializeOwned + zbus::zvariant::Type>(
            message: &Message,
        ) -> fdo::Result<T> {
            message
                .body()
                .deserialize()
                .map_err(|_| fdo::Error::InvalidArgs("Invalid clipboard arguments".into()))
        }
        let (key, command, fd) = match member {
            "RequestClipboard" => {
                let (key, _): (OwnedObjectPath, Options) = args(message)?;
                (key, Command::Request, false)
            }
            "SetSelection" => {
                let (key, mut options): (OwnedObjectPath, Options) = args(message)?;
                let types = option(&mut options, "mime_types")?.unwrap_or_default();
                (key, Command::Publish(types), false)
            }
            "SelectionWrite" => {
                let (key, serial): (OwnedObjectPath, u32) = args(message)?;
                (key, Command::Write(serial), true)
            }
            "SelectionWriteDone" => {
                let (key, serial, success): (OwnedObjectPath, u32, bool) = args(message)?;
                (key, Command::Done(serial, success), false)
            }
            "SelectionRead" => {
                let (key, mime): (OwnedObjectPath, String) = args(message)?;
                (key, Command::Read(mime), true)
            }
            _ => return Err(fdo::Error::UnknownMethod("Unknown clipboard method".into())),
        };
        Ok((fd, self.methods.queue(key, command, message.header())?))
    }
}
async fn reply(
    connection: &zbus::Connection,
    message: &Message,
    fd: bool,
    result: fdo::Result<Response>,
) {
    let header = message.header();
    if header.primary().flags().contains(Flags::NoReplyExpected) {
        return;
    }
    let result = match result {
        Ok(Response::Unit) if !fd => {
            let _ = connection.reply(&header, &()).await;
            return;
        }
        Ok(Response::Fd(value)) if fd => {
            let value: zbus::zvariant::OwnedFd = value.into();
            let _ = connection.reply(&header, &value).await;
            return;
        }
        Ok(_) => fdo::Error::Failed("Invalid clipboard transport response".into()),
        Err(error) => error,
    };
    let _ = connection.reply_dbus_error(&header, result).await;
}
#[zbus::export::async_trait::async_trait]
impl Interface for Clipboard {
    fn name() -> InterfaceName<'static> {
        Methods::name()
    }
    fn spawn_tasks_for_methods(&self) -> bool {
        false
    }
    async fn get(
        &self,
        name: &str,
        server: &ObjectServer,
        connection: &zbus::Connection,
        header: Option<&Header<'_>>,
        emitter: &SignalEmitter<'_>,
    ) -> Option<fdo::Result<zbus::zvariant::OwnedValue>> {
        self.methods
            .get(name, server, connection, header, emitter)
            .await
    }
    async fn get_all(
        &self,
        server: &ObjectServer,
        connection: &zbus::Connection,
        header: Option<&Header<'_>>,
        emitter: &SignalEmitter<'_>,
    ) -> fdo::Result<HashMap<String, zbus::zvariant::OwnedValue>> {
        self.methods
            .get_all(server, connection, header, emitter)
            .await
    }
    async fn set_mut(
        &mut self,
        _: &str,
        _: &Value<'_>,
        _: &ObjectServer,
        _: &zbus::Connection,
        _: Option<&Header<'_>>,
        _: &SignalEmitter<'_>,
    ) -> Option<fdo::Result<()>> {
        None
    }
    fn call<'call>(
        &'call self,
        _: &'call ObjectServer,
        connection: &'call zbus::Connection,
        message: &'call Message,
        name: MemberName<'call>,
    ) -> DispatchResult2<'call> {
        if !matches!(
            name.as_str(),
            "RequestClipboard"
                | "SetSelection"
                | "SelectionRead"
                | "SelectionWrite"
                | "SelectionWriteDone"
        ) {
            return DispatchResult2::NotFound;
        }
        let (fd, result) = match self.admit(message, name.as_str()) {
            Ok(value) => value,
            Err(error) => {
                return DispatchResult2::new_async(connection, message, async move {
                    Err::<(), _>(error)
                });
            }
        };
        let connection = connection.clone();
        let message = message.clone();
        tokio::spawn(async move {
            reply(
                &connection,
                &message,
                fd,
                result.await.unwrap_or_else(|_| {
                    Err(fdo::Error::Failed("Clipboard session stopped".into()))
                }),
            )
            .await;
        });
        DispatchResult2::Async(Box::pin(async { Ok(()) }))
    }
    fn call_mut<'call>(
        &'call mut self,
        _: &'call ObjectServer,
        _: &'call zbus::Connection,
        _: &'call Message,
        _: MemberName<'call>,
    ) -> DispatchResult2<'call> {
        DispatchResult2::NotFound
    }
    fn introspect_to_writer(&self, writer: &mut dyn Write, level: usize) {
        self.methods.introspect_to_writer(writer, level);
    }
}
