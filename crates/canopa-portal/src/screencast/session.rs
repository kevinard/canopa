use super::*;
use futures_util::{StreamExt, future, stream::BoxStream};
use std::os::fd::OwnedFd;
use zbus::object_server::SignalEmitter;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Capture,
    Remote,
}
pub(super) enum Command {
    Select(SelectionOptions, oneshot::Sender<fdo::Result<Outcome>>),
    Devices(remote::DeviceOptions, oneshot::Sender<fdo::Result<Outcome>>),
    Clipboard(
        crate::clipboard::Command,
        oneshot::Sender<fdo::Result<crate::clipboard::Response>>,
    ),
    ClipboardReady(u32),
    Start {
        kind: Kind,
        response_serial: u32,
        parent_window: String,
        modal: bool,
        activation_token: Option<String>,
        cancelled: watch::Receiver<bool>,
        reply: oneshot::Sender<fdo::Result<Outcome>>,
    },
    Connect(oneshot::Sender<fdo::Result<OwnedFd>>),
    Input(remote::Input, oneshot::Sender<fdo::Result<()>>),
}
struct Endpoint(Handle);
#[zbus::interface(name = "org.freedesktop.impl.portal.Session")]
impl Endpoint {
    fn close(&self, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        if sender(&header)? != self.0.owner {
            return Err(fdo::Error::AccessDenied(
                "Sharing session belongs to another caller".into(),
            ));
        }
        self.0.notify.store(false, Ordering::SeqCst);
        self.0.close.send_replace(true);
        Ok(())
    }
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }
    #[zbus(signal)]
    async fn closed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}
impl Host {
    pub(super) async fn create(
        self: &Arc<Self>,
        connection: &Connection,
        key: OwnedObjectPath,
        app: String,
        owner: OwnedUniqueName,
        kind: Kind,
    ) -> fdo::Result<Outcome> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(fdo::Error::Failed("Sharing host is shutting down".into()));
        }
        if key.as_str().len() > 512
            || !key
                .as_str()
                .starts_with("/org/freedesktop/portal/desktop/session/")
        {
            return Err(fdo::Error::InvalidArgs(
                "Invalid sharing session path".into(),
            ));
        }
        let slot = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| fdo::Error::LimitsExceeded("Too many sharing sessions".into()))?;
        let bus = fdo::DBusProxy::new(connection).await?;
        let owners = bus
            .receive_name_owner_changed_with_args(&[(0, owner.as_str())])
            .await?;
        if !bus.name_has_owner(owner.clone().into()).await? {
            return Ok(Outcome::Cancelled);
        }
        if self
            .entries
            .lock()
            .expect("sharing sessions")
            .contains_key(&key)
        {
            return Err(fdo::Error::InvalidArgs(
                "Sharing session already exists".into(),
            ));
        }
        // The frontend forwards Notify without waiting for input acknowledgements.
        // Allow ordinary chords/repeats while keeping each session's work finite.
        let (commands, inbox) = mpsc::channel(64);
        let (close, _) = watch::channel(false);
        let (done, ended) = watch::channel(false);
        let handle = Handle {
            kind,
            owner,
            app,
            commands,
            close,
            notify: Arc::new(AtomicBool::new(true)),
            done: ended,
        };
        let accepted = Accepted::new(Options::new(), handle.close.clone());
        if !connection
            .object_server()
            .at(key.clone(), Endpoint(handle.clone()))
            .await?
        {
            return Err(fdo::Error::InvalidArgs(
                "Sharing session already exists".into(),
            ));
        }
        // Registration, publication and cleanup ownership have no intervening await.
        if self.shutdown.load(Ordering::SeqCst) {
            handle.close.send_replace(true);
        }
        self.entries
            .lock()
            .expect("sharing sessions")
            .insert(key.clone(), handle.clone());
        tokio::spawn(
            Actor {
                host: self.clone(),
                connection: connection.clone(),
                key,
                handle,
                inbox,
                owners: owners.map(|_| ()).boxed(),
                done,
                slot,
            }
            .run(),
        );
        Ok(Outcome::Accepted(accepted))
    }
}
#[derive(PartialEq, Eq)]
enum Phase {
    Configuring,
    Starting,
    Active,
}
enum Configuration {
    Capture(SelectionOptions),
    Remote {
        devices: remote::DeviceOptions,
        sources: Option<SelectionOptions>,
        clipboard: bool,
    },
}
enum Resources {
    Capture(Box<dyn Capture>),
    Remote(Box<dyn remote::Control>),
}
impl Resources {
    fn closed(&self) -> watch::Receiver<bool> {
        match self {
            Self::Capture(v) => v.closed(),
            Self::Remote(v) => v.closed(),
        }
    }
    fn results(&self, config: &Configuration) -> anyhow::Result<Options> {
        match (self, config) {
            (Self::Capture(v), Configuration::Capture(options)) => results(v.streams(), options),
            (
                Self::Remote(v),
                Configuration::Remote {
                    devices,
                    sources,
                    clipboard,
                },
            ) => remote::results(v.as_ref(), devices, sources.as_ref(), *clipboard),
            _ => anyhow::bail!("Sharing resource kind mismatch"),
        }
    }
    fn stop(self) -> BoxFuture<'static, anyhow::Result<()>> {
        match self {
            Self::Capture(v) => v.stop(),
            Self::Remote(v) => v.stop(),
        }
    }
}
type PendingSharing = BoxFuture<'static, anyhow::Result<Option<Resources>>>;
struct Actor {
    host: Arc<Host>,
    connection: Connection,
    key: OwnedObjectPath,
    handle: Handle,
    inbox: mpsc::Receiver<Command>,
    owners: BoxStream<'static, ()>,
    done: watch::Sender<bool>,
    slot: tokio::sync::OwnedSemaphorePermit,
}
impl Actor {
    async fn run(mut self) {
        let mut phase = Phase::Configuring;
        let mut eis_connected = false;
        let mut sources = None;
        let mut devices = None;
        let mut clipboard_requested = false;
        let mut start_response_serial = 0;
        let mut clipboard_ready = false;
        let mut clipboard_events: Option<BoxStream<'static, crate::clipboard::Event>> = None;
        let mut closing = self.handle.close.subscribe();
        let mut startup: Option<PendingSharing> = None;
        let mut pending: Option<oneshot::Sender<fdo::Result<Outcome>>> = None;
        let mut config = None;
        let mut request_cancelled: Option<watch::Receiver<bool>> = None;
        let mut resources: Option<Resources> = None;
        let mut source_closed: Option<watch::Receiver<bool>> = None;
        enum Event {
            Close,
            OwnerLost,
            Command(Option<Command>),
            Ready(anyhow::Result<Option<Resources>>),
            SourceLost,
            RequestCancelled,
            Clipboard(Option<crate::clipboard::Event>),
        }
        loop {
            let starting = startup.is_some();
            let active = resources.is_some();
            let event = tokio::select! {
                biased;
                _=stopped(&mut closing)=>Event::Close,
                _=self.owners.next()=>Event::OwnerLost,
                _=async {stopped(request_cancelled.as_mut().expect("pending request")).await},if starting=>Event::RequestCancelled,
                _=async {stopped(source_closed.as_mut().expect("active resource")).await},if active=>Event::SourceLost,
                result=async {match startup.as_mut(){Some(f)=>f.await,None=>future::pending().await}},if starting=>Event::Ready(result),
                command=self.inbox.recv()=>Event::Command(command),
                event=async {clipboard_events.as_mut().expect("clipboard events").next().await}, if clipboard_ready && clipboard_events.is_some()=>Event::Clipboard(event),
            };
            match event {
                Event::Close
                | Event::SourceLost
                | Event::RequestCancelled
                | Event::Command(None) => break,
                Event::OwnerLost => {
                    self.handle.notify.store(false, Ordering::SeqCst);
                    break;
                }
                Event::Clipboard(None) => break,
                Event::Clipboard(Some(event)) => {
                    enum Emitted {
                        Ready(bool),
                        Close,
                        OwnerLost,
                    }
                    let result = {
                        let emitting = crate::clipboard::Clipboard::emit(
                            &self.connection,
                            &self.handle.owner,
                            &self.key,
                            event,
                        );
                        tokio::select! {
                            biased;
                            _=stopped(&mut closing)=>Emitted::Close,
                            _=self.owners.next()=>Emitted::OwnerLost,
                            _=stopped(source_closed.as_mut().expect("active sharing resource"))=>Emitted::Close,
                            result=tokio::time::timeout(Duration::from_secs(5), emitting)=>Emitted::Ready(matches!(result, Ok(Ok(())))),
                        }
                    };
                    match result {
                        Emitted::Ready(true) => {}
                        Emitted::Ready(false) | Emitted::Close => break,
                        Emitted::OwnerLost => {
                            self.handle.notify.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                }
                Event::Command(Some(Command::ClipboardReady(serial))) => {
                    if phase == Phase::Active && serial == start_response_serial {
                        clipboard_ready = true;
                    }
                }
                Event::Command(Some(Command::Clipboard(
                    crate::clipboard::Command::Request,
                    reply,
                ))) => {
                    let result = if self.handle.kind == Kind::Remote
                        && phase == Phase::Configuring
                        && !clipboard_requested
                    {
                        clipboard_requested = true;
                        Ok(crate::clipboard::Response::Unit)
                    } else {
                        Err(fdo::Error::InvalidArgs(
                            "Clipboard must be requested once before Start".into(),
                        ))
                    };
                    let _ = reply.send(result);
                }
                Event::Command(Some(Command::Clipboard(command, reply))) => {
                    let clipboard = match resources.as_mut() {
                        Some(Resources::Remote(control)) if control.clipboard_enabled() => {
                            control.clipboard()
                        }
                        _ => None,
                    };
                    let Some(clipboard) = clipboard else {
                        let _ = reply.send(Err(fdo::Error::AccessDenied(
                            "Clipboard access was not granted".into(),
                        )));
                        continue;
                    };
                    enum Performed {
                        Ready(fdo::Result<crate::clipboard::Response>),
                        Close,
                        OwnerLost,
                    }
                    let result = {
                        let pending = clipboard.perform(command, self.handle.close.subscribe());
                        tokio::select! {
                            biased;
                            _=stopped(&mut closing)=>Performed::Close,
                            _=self.owners.next()=>Performed::OwnerLost,
                            _=stopped(source_closed.as_mut().expect("active sharing resource"))=>Performed::Close,
                            result=pending=>Performed::Ready(result),
                        }
                    };
                    match result {
                        Performed::Ready(result) => {
                            let _ = reply.send(result);
                        }
                        Performed::Close => break,
                        Performed::OwnerLost => {
                            self.handle.notify.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                }
                Event::Command(Some(Command::Select(options, reply))) => {
                    let outcome = if phase == Phase::Configuring && sources.is_none() {
                        sources = Some(options);
                        Ok(Outcome::Accepted(Accepted::new(
                            Options::new(),
                            self.handle.close.clone(),
                        )))
                    } else {
                        Err(fdo::Error::InvalidArgs(
                            "Sources already selected or session started".into(),
                        ))
                    };
                    let _ = reply.send(outcome);
                }
                Event::Command(Some(Command::Devices(options, reply))) => {
                    let outcome = if self.handle.kind == Kind::Remote
                        && phase == Phase::Configuring
                        && devices.is_none()
                    {
                        devices = Some(options);
                        Ok(Outcome::Accepted(Accepted::new(
                            Options::new(),
                            self.handle.close.clone(),
                        )))
                    } else {
                        Err(fdo::Error::InvalidArgs(
                            "Devices already selected or session started".into(),
                        ))
                    };
                    let _ = reply.send(outcome);
                }
                Event::Command(Some(Command::Start {
                    kind,
                    response_serial,
                    parent_window,
                    modal,
                    activation_token,
                    cancelled,
                    reply,
                })) => {
                    if phase != Phase::Configuring
                        || kind != self.handle.kind
                        || kind == Kind::Capture && sources.is_none()
                    {
                        let _ = reply.send(Err(fdo::Error::InvalidArgs(
                            "Sharing session is not ready for this Start".into(),
                        )));
                        continue;
                    }
                    start_response_serial = response_serial;
                    let connection = self.connection.clone();
                    request_cancelled = Some(cancelled.clone());
                    match kind {
                        Kind::Capture => {
                            let options =
                                sources.as_ref().expect("capture source selection").clone();
                            let request = StartRequest {
                                app_id: self.handle.app.clone(),
                                caller: self.handle.owner.clone(),
                                parent_window,
                                modal,
                                activation_token,
                                options: options.clone(),
                            };
                            let presenter = self.host.presenter.clone();
                            startup = Some(Box::pin(async move {
                                Ok(presenter
                                    .present(connection, request, cancelled)
                                    .await?
                                    .map(Resources::Capture))
                            }));
                            config = Some(Configuration::Capture(options));
                        }
                        Kind::Remote => {
                            let devices = devices
                                .clone()
                                .unwrap_or(remote::DeviceOptions { types: 7 });
                            let request = remote::StartRequest {
                                app_id: self.handle.app.clone(),
                                caller: self.handle.owner.clone(),
                                parent_window,
                                modal,
                                activation_token,
                                devices: devices.clone(),
                                sources: sources.clone(),
                                clipboard: clipboard_requested,
                            };
                            let presenter = self.host.remote_presenter.clone();
                            startup = Some(Box::pin(async move {
                                Ok(presenter
                                    .present(connection, request, cancelled)
                                    .await?
                                    .map(Resources::Remote))
                            }));
                            config = Some(Configuration::Remote {
                                devices,
                                sources: sources.clone(),
                                clipboard: clipboard_requested,
                            });
                        }
                    }
                    pending = Some(reply);
                    phase = Phase::Starting;
                }
                Event::Command(Some(Command::Connect(reply))) => {
                    let Some(Resources::Remote(control)) = resources.as_mut() else {
                        let _ = reply.send(Err(fdo::Error::InvalidArgs(
                            "Remote session is not active".into(),
                        )));
                        continue;
                    };
                    if control.devices() & 7 == 0 {
                        let _ = reply.send(Err(fdo::Error::AccessDenied(
                            "No input device was granted".into(),
                        )));
                        continue;
                    }
                    enum Connected {
                        Ready(anyhow::Result<OwnedFd>),
                        Close,
                        OwnerLost,
                    }
                    let result = {
                        let connecting = control.connect(self.handle.close.subscribe());
                        tokio::select! {
                            biased;
                            _=stopped(&mut closing)=>Connected::Close,
                            _=self.owners.next()=>Connected::OwnerLost,
                            _=stopped(source_closed.as_mut().expect("active sharing resource"))=>Connected::Close,
                            result=connecting=>Connected::Ready(result),
                        }
                    };
                    match result {
                        Connected::Ready(Ok(fd)) => {
                            eis_connected = true;
                            let _ = reply.send(Ok(fd));
                        }
                        Connected::Ready(Err(_)) => {
                            let _ = reply
                                .send(Err(fdo::Error::Failed("Input connection failed".into())));
                            break;
                        }
                        Connected::OwnerLost => {
                            self.handle.notify.store(false, Ordering::SeqCst);
                            break;
                        }
                        Connected::Close => break,
                    }
                }
                Event::Command(Some(Command::Input(input, reply))) => {
                    let Some(Resources::Remote(control)) = resources.as_mut() else {
                        let _ = reply.send(Err(fdo::Error::InvalidArgs(
                            "Remote session is not active".into(),
                        )));
                        continue;
                    };
                    if eis_connected || control.devices() & input.device() == 0 {
                        let _ = reply.send(Err(fdo::Error::AccessDenied(
                            if eis_connected {
                                "D-Bus input is unavailable after EIS connection"
                            } else {
                                "This input device was not granted"
                            }
                            .into(),
                        )));
                        continue;
                    }
                    if let remote::Input::Pointer(remote::PointerInput::Absolute { stream, .. }) =
                        input
                        && !control.streams().iter().any(|s| s.node == stream)
                    {
                        let _ = reply.send(Err(fdo::Error::InvalidArgs(
                            "Pointer stream was not approved".into(),
                        )));
                        continue;
                    }
                    if let remote::Input::Touch(
                        remote::TouchInput::Down { stream, .. }
                        | remote::TouchInput::Motion { stream, .. },
                    ) = input
                        && !control.streams().iter().any(|s| s.node == stream)
                    {
                        let _ = reply.send(Err(fdo::Error::InvalidArgs(
                            "Touch stream was not approved".into(),
                        )));
                        continue;
                    }
                    enum Notified {
                        Ready(fdo::Result<()>),
                        Close,
                        OwnerLost,
                    }
                    let result = {
                        let notifying = control.input(input, self.handle.close.subscribe());
                        tokio::select! {
                            biased;
                            _ = stopped(&mut closing) => Notified::Close,
                            _ = self.owners.next() => Notified::OwnerLost,
                            _ = stopped(source_closed.as_mut().expect("active sharing resource")) => Notified::Close,
                            result = notifying => Notified::Ready(result),
                        }
                    };
                    match result {
                        Notified::Ready(result) => {
                            let _ = reply.send(result);
                        }
                        Notified::Close => break,
                        Notified::OwnerLost => {
                            self.handle.notify.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                }
                Event::Ready(result) => {
                    startup = None;
                    let reply = pending.take().expect("pending sharing reply");
                    let cancelled = request_cancelled.take().is_some_and(|v| *v.borrow());
                    match result {
                        Ok(Some(mut value)) if !cancelled => {
                            if let Resources::Remote(control) = &mut value
                                && control.clipboard_enabled()
                            {
                                clipboard_events =
                                    control.clipboard().and_then(|clipboard| clipboard.events());
                                if clipboard_events.is_none() {
                                    resources = Some(value);
                                    let _ = reply.send(Ok(Outcome::Failed));
                                    break;
                                }
                            }
                            match value.results(config.as_ref().expect("sharing options")) {
                                Ok(results) if !*value.closed().borrow() => {
                                    source_closed = Some(value.closed());
                                    resources = Some(value);
                                    phase = Phase::Active;
                                    let _ = reply.send(Ok(Outcome::Accepted(Accepted::new(
                                        results,
                                        self.handle.close.clone(),
                                    ))));
                                }
                                _ => {
                                    resources = Some(value);
                                    let _ = reply.send(Ok(Outcome::Failed));
                                    break;
                                }
                            }
                        }
                        Ok(Some(value)) => {
                            resources = Some(value);
                            let _ = reply.send(Ok(Outcome::Cancelled));
                            break;
                        }
                        Ok(None) => {
                            let _ = reply.send(Ok(Outcome::Cancelled));
                            break;
                        }
                        Err(_) => {
                            let _ = reply.send(Ok(Outcome::Failed));
                            break;
                        }
                    }
                }
            }
        }
        self.handle.close.send_replace(true);
        drop(startup);
        if let Some(reply) = pending {
            let _ = reply.send(Ok(Outcome::Cancelled));
        }
        self.inbox.close();
        while let Ok(command) = self.inbox.try_recv() {
            match command {
                Command::Select(_, reply)
                | Command::Devices(_, reply)
                | Command::Start { reply, .. } => {
                    let _ = reply.send(Ok(Outcome::Cancelled));
                }
                Command::Connect(reply) => {
                    let _ = reply.send(Err(fdo::Error::Failed("Sharing session stopped".into())));
                }
                Command::Input(_, reply) => {
                    let _ = reply.send(Err(fdo::Error::Failed("Sharing session stopped".into())));
                }
                Command::Clipboard(_, reply) => {
                    let _ = reply.send(Err(fdo::Error::Failed("Clipboard session stopped".into())));
                }
                Command::ClipboardReady(_) => {}
            }
        }
        if let Some(resources) = resources
            && resources.stop().await.is_err()
        {
            eprintln!("level=warn component=sharing_portal event=cleanup_unconfirmed");
        }
        if self.handle.notify.load(Ordering::SeqCst)
            && let Ok(emitter) = SignalEmitter::new(&self.connection, self.key.clone())
        {
            let _ = Endpoint::closed(&emitter).await;
        }
        let _ = self
            .connection
            .object_server()
            .remove::<Endpoint, _>(self.key.clone())
            .await;
        self.host
            .entries
            .lock()
            .expect("sharing sessions")
            .remove(&self.key);
        drop(self.slot);
        self.done.send_replace(true);
    }
}
async fn stopped(receiver: &mut watch::Receiver<bool>) {
    while !*receiver.borrow() {
        if receiver.changed().await.is_err() {
            return;
        }
    }
}
