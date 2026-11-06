//! Admit all legacy input in receive order, without waiting for replies.
use super::*;
use std::{collections::HashMap, fmt::Write};
use zbus::{
    ObjectServer,
    message::{Flags, Message},
    names::{InterfaceName, MemberName},
    object_server::{DispatchResult2, Interface, SignalEmitter},
    zvariant::{OwnedValue, Value},
};

#[derive(Clone)]
pub struct RemoteDesktop {
    pub(super) methods: Methods,
}
impl RemoteDesktop {
    pub fn new(capture: ScreenCast) -> Self {
        Self {
            methods: Methods { capture },
        }
    }
    fn start_replied(methods: &Methods, message: &Message) {
        let parsed: Result<(OwnedObjectPath, OwnedObjectPath, String, String, Options), _> =
            message.body().deserialize();
        if let Ok((_, key, app, _, _)) = parsed
            && let Ok(owner) = sender(&message.header())
            && let Ok(handle) = methods.lookup(&key, &app, owner)
            && handle
                .commands
                .try_send(session::Command::ClipboardReady(
                    message.header().primary().serial_num().get(),
                ))
                .is_err()
        {
            handle.close.send_replace(true);
        }
    }
    fn input(
        &self,
        message: &Message,
        member: &str,
    ) -> fdo::Result<oneshot::Receiver<fdo::Result<()>>> {
        fn arguments<T: serde::de::DeserializeOwned + zbus::zvariant::Type>(
            message: &Message,
        ) -> fdo::Result<T> {
            message
                .body()
                .deserialize()
                .map_err(|_| fdo::Error::InvalidArgs("Invalid input arguments".into()))
        }
        let (key, input) = match member {
            "NotifyKeyboardKeycode" | "NotifyKeyboardKeysym" => {
                let (key, _options, value, state): (OwnedObjectPath, Options, i32, u32) =
                    arguments(message)?;
                let input = if member == "NotifyKeyboardKeysym" {
                    KeyboardInput::keysym(value, state)?
                } else {
                    KeyboardInput::keycode(value, state)?
                };
                (key, Input::Keyboard(input))
            }
            "NotifyPointerMotion" => {
                let (key, _options, x, y): (OwnedObjectPath, Options, f64, f64) =
                    arguments(message)?;
                (key, Input::Pointer(PointerInput::motion(x, y)?))
            }
            "NotifyPointerMotionAbsolute" => {
                let (key, _options, stream, x, y): (OwnedObjectPath, Options, u32, f64, f64) =
                    arguments(message)?;
                (key, Input::Pointer(PointerInput::absolute(stream, x, y)?))
            }
            "NotifyPointerButton" => {
                let (key, _options, code, state): (OwnedObjectPath, Options, i32, u32) =
                    arguments(message)?;
                (key, Input::Pointer(PointerInput::button(code, state)?))
            }
            "NotifyPointerAxis" => {
                let (key, options, x, y): (OwnedObjectPath, Options, f64, f64) =
                    arguments(message)?;
                (key, Input::Pointer(PointerInput::axis(x, y, options)?))
            }
            "NotifyPointerAxisDiscrete" => {
                let (key, _options, axis, steps): (OwnedObjectPath, Options, u32, i32) =
                    arguments(message)?;
                (key, Input::Pointer(PointerInput::discrete(axis, steps)?))
            }
            "NotifyTouchDown" | "NotifyTouchMotion" => {
                let (key, _options, stream, slot, x, y): (
                    OwnedObjectPath,
                    Options,
                    u32,
                    u32,
                    f64,
                    f64,
                ) = arguments(message)?;
                let input = if member == "NotifyTouchDown" {
                    TouchInput::down(stream, slot, x, y)?
                } else {
                    TouchInput::motion(stream, slot, x, y)?
                };
                (key, Input::Touch(input))
            }
            "NotifyTouchUp" => {
                let (key, _options, slot): (OwnedObjectPath, Options, u32) = arguments(message)?;
                (key, Input::Touch(TouchInput::Up { slot }))
            }
            _ => return Err(fdo::Error::UnknownMethod("Unknown input method".into())),
        };
        self.methods
            .queue_input(key, input, sender(&message.header())?)
    }
}

async fn reply(connection: &Connection, message: &Message, result: fdo::Result<()>) {
    let header = message.header();
    if header.primary().flags().contains(Flags::NoReplyExpected) {
        return;
    }
    let _ = match result {
        Ok(()) => connection.reply(&header, &()).await,
        Err(error) => connection.reply_dbus_error(&header, error).await,
    };
}

#[zbus::export::async_trait::async_trait]
impl Interface for RemoteDesktop {
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
        connection: &Connection,
        header: Option<&Header<'_>>,
        emitter: &SignalEmitter<'_>,
    ) -> Option<fdo::Result<OwnedValue>> {
        self.methods
            .get(name, server, connection, header, emitter)
            .await
    }
    async fn get_all(
        &self,
        server: &ObjectServer,
        connection: &Connection,
        header: Option<&Header<'_>>,
        emitter: &SignalEmitter<'_>,
    ) -> fdo::Result<HashMap<String, OwnedValue>> {
        self.methods
            .get_all(server, connection, header, emitter)
            .await
    }
    async fn set_mut(
        &mut self,
        _: &str,
        _: &Value<'_>,
        _: &ObjectServer,
        _: &Connection,
        _: Option<&Header<'_>>,
        _: &SignalEmitter<'_>,
    ) -> Option<fdo::Result<()>> {
        None
    }
    fn call<'call>(
        &'call self,
        server: &'call ObjectServer,
        connection: &'call Connection,
        message: &'call Message,
        name: MemberName<'call>,
    ) -> DispatchResult2<'call> {
        match name.as_str() {
            "NotifyKeyboardKeycode"
            | "NotifyKeyboardKeysym"
            | "NotifyPointerMotion"
            | "NotifyPointerMotionAbsolute"
            | "NotifyPointerButton"
            | "NotifyPointerAxis"
            | "NotifyPointerAxisDiscrete"
            | "NotifyTouchDown"
            | "NotifyTouchMotion"
            | "NotifyTouchUp" => {
                // This runs before spawning, in the object server's receive loop.
                // Waiting for the worker here would stall every portal interface.
                let receiver = match self.input(message, name.as_str()) {
                    Ok(receiver) => receiver,
                    Err(error) => {
                        return DispatchResult2::new_async(connection, message, async move {
                            Err::<(), _>(error)
                        });
                    }
                };
                let connection = connection.clone();
                let message = message.clone();
                tokio::spawn(async move {
                    let result = receiver.await.unwrap_or_else(|_| {
                        Err(fdo::Error::Failed("Sharing session stopped".into()))
                    });
                    reply(&connection, &message, result).await;
                });
            }
            "CreateSession" | "SelectDevices" | "Start" | "ConnectToEIS" => {
                let methods = self.methods.clone();
                let server = server.clone();
                let connection = connection.clone();
                let message = message.clone();
                tokio::spawn(async move {
                    let header = message.header();
                    let name = header.member().expect("dispatched method");
                    let result = match methods.call(&server, &connection, &message, name.as_ref()) {
                        DispatchResult2::Async(future) => future.await,
                        _ => Err(fdo::Error::UnknownMethod(
                            "Unknown remote desktop method".into(),
                        )),
                    };
                    // Macro dispatch completes after its typed response is sent. Match
                    // the successful Start actor by serial, so an errored concurrent
                    // Start cannot release the original Start's clipboard signals.
                    if result.is_ok() && name.as_str() == "Start" {
                        Self::start_replied(&methods, &message);
                    }
                    if let Err(error) = result {
                        reply(&connection, &message, Err(error)).await;
                    }
                });
            }
            _ => return DispatchResult2::NotFound,
        }
        DispatchResult2::Async(Box::pin(async { Ok(()) }))
    }
    fn call_mut<'call>(
        &'call mut self,
        _: &'call ObjectServer,
        _: &'call Connection,
        _: &'call Message,
        _: MemberName<'call>,
    ) -> DispatchResult2<'call> {
        DispatchResult2::NotFound
    }
    fn introspect_to_writer(&self, writer: &mut dyn Write, level: usize) {
        self.methods.introspect_to_writer(writer, level);
    }
}
