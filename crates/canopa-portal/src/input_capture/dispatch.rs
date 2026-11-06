//! Queue fire-and-forget controls in D-Bus receive order, without blocking it.
use super::*;
use std::fmt::Write;
use zbus::{
    ObjectServer,
    message::{Flags, Message},
    names::{InterfaceName, MemberName},
    object_server::{DispatchResult2, Interface, SignalEmitter},
    zvariant::OwnedValue,
};

#[derive(Clone)]
pub struct InputCapture {
    methods: Methods,
}
impl InputCapture {
    pub(crate) fn has_session(&self, key: &OwnedObjectPath) -> bool {
        self.methods
            .host
            .entries
            .lock()
            .expect("capture sessions")
            .contains_key(key)
    }
    pub(crate) fn clipboard_command(
        &self,
        key: OwnedObjectPath,
        owner: OwnedUniqueName,
        command: crate::clipboard::Command,
    ) -> fdo::Result<oneshot::Receiver<fdo::Result<crate::clipboard::Response>>> {
        let handle = self
            .methods
            .host
            .entries
            .lock()
            .expect("capture sessions")
            .get(&key)
            .cloned()
            .ok_or_else(|| invalid("Unknown input capture session"))?;
        if handle.owner != owner || *handle.close.borrow() {
            return Err(fdo::Error::AccessDenied(
                "Clipboard session belongs to another caller".into(),
            ));
        }
        let epoch = *handle.clipboard_epoch.borrow();
        if epoch.is_none()
            && !matches!(
                command,
                crate::clipboard::Command::Request | crate::clipboard::Command::Done(..)
            )
        {
            return Err(fdo::Error::AccessDenied(
                "Clipboard requires active input capture".into(),
            ));
        }
        let (reply, result) = oneshot::channel();
        handle.queue(session::Command::Clipboard {
            command,
            epoch,
            reply,
        })?;
        Ok(result)
    }
    fn start_replied(methods: &Methods, message: &Message) {
        let parsed: Result<(OwnedObjectPath, OwnedObjectPath, String, String, Options), _> =
            message.body().deserialize();
        if let Ok((_, key, app, _, _)) = parsed
            && let Ok(owner) = sender(&message.header())
            && let Ok(handle) = methods.lookup(&key, &app, owner)
        {
            let _ = handle.queue(session::Command::StartReplied(
                message.header().primary().serial_num().get(),
            ));
        }
    }
    pub fn with_requests(presenter: Arc<dyn Presenter>, requests: Requests) -> Self {
        Self {
            methods: Methods {
                host: Arc::new(Host {
                    entries: Mutex::new(HashMap::new()),
                    slots: Arc::new(Semaphore::new(16)),
                    shutdown: AtomicBool::new(false),
                    presenter,
                }),
                requests,
            },
        }
    }
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.methods.host.shutdown.store(true, Ordering::SeqCst);
        let sessions: Vec<_> = self
            .methods
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
        tokio::time::timeout(Duration::from_secs(30), async {
            for session in sessions {
                let mut done = session.done;
                while !*done.borrow() {
                    done.changed().await?;
                }
            }
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        Ok(())
    }
    fn admit(
        &self,
        message: &Message,
        member: &str,
    ) -> fdo::Result<(bool, oneshot::Receiver<fdo::Result<Output>>)> {
        let (key, app, mut options): (OwnedObjectPath, String, Options) = message
            .body()
            .deserialize()
            .map_err(|_| invalid("Invalid capture control arguments"))?;
        let operation = match member {
            "Enable" => Operation::Enable,
            "Disable" => Operation::Disable,
            "ConnectToEIS" => Operation::Connect,
            "Release" => {
                let activation = option(&mut options, "activation_id")?;
                if let Some((x, y)) = option::<(f64, f64)>(&mut options, "cursor_position")?
                    && (!x.is_finite() || !y.is_finite())
                {
                    return Err(invalid("Invalid release cursor position"));
                }
                Operation::Release { activation }
            }
            _ => {
                return Err(fdo::Error::UnknownMethod(
                    "Unknown capture operation".into(),
                ));
            }
        };
        Ok((
            member == "ConnectToEIS",
            self.methods
                .queue(&key, &app, sender(&message.header())?, operation)?,
        ))
    }
}
async fn reply(connection: &Connection, message: &Message, fd: bool, result: fdo::Result<Output>) {
    let header = message.header();
    if header.primary().flags().contains(Flags::NoReplyExpected) {
        return;
    }
    match result {
        Ok(Output::Fd(fd_value)) if fd => {
            let _ = connection
                .reply(&header, &(zbus::zvariant::OwnedFd::from(fd_value),))
                .await;
        }
        Ok(output) if !fd => match values(output) {
            Ok(options) => {
                let _ = connection.reply(&header, &(0_u32, options)).await;
            }
            Err(error) => {
                let _ = connection.reply_dbus_error(&header, error).await;
            }
        },
        Ok(_) => {
            let _ = connection.reply_dbus_error(&header, stopped_error()).await;
        }
        Err(error) => {
            let _ = connection.reply_dbus_error(&header, error).await;
        }
    }
}
#[zbus::export::async_trait::async_trait]
impl Interface for InputCapture {
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
        if matches!(
            name.as_str(),
            "Enable" | "Disable" | "Release" | "ConnectToEIS"
        ) {
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
                    result.await.unwrap_or_else(|_| Err(stopped_error())),
                )
                .await;
            });
            DispatchResult2::Async(Box::pin(async { Ok(()) }))
        } else if matches!(
            name.as_str(),
            "CreateSession" | "CreateSession2" | "Start" | "GetZones" | "SetPointerBarriers"
        ) {
            let methods = self.methods.clone();
            let server = server.clone();
            let connection = connection.clone();
            let message = message.clone();
            tokio::spawn(async move {
                let header = message.header();
                let name = header.member().expect("capture method");
                let result = match methods.call(&server, &connection, &message, name.as_ref()) {
                    DispatchResult2::Async(future) => future.await,
                    _ => Err(fdo::Error::UnknownMethod("Unknown capture method".into())),
                };
                if result.is_ok() && matches!(name.as_str(), "Start" | "CreateSession") {
                    Self::start_replied(&methods, &message);
                }
                if let Err(error) = result {
                    reply(&connection, &message, false, Err(error)).await;
                }
            });
            DispatchResult2::Async(Box::pin(async { Ok(()) }))
        } else {
            DispatchResult2::NotFound
        }
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
