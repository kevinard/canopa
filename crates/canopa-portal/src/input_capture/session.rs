use super::*;

pub(super) enum Command {
    Start {
        request: StartRequest,
        cancelled: watch::Receiver<bool>,
        reply: oneshot::Sender<fdo::Result<Outcome>>,
        serial: u32,
    },
    StartReplied(u32),
    Clipboard {
        command: crate::clipboard::Command,
        epoch: Option<u64>,
        reply: oneshot::Sender<fdo::Result<crate::clipboard::Response>>,
    },
    Operation(Operation, oneshot::Sender<fdo::Result<Output>>),
}
struct Endpoint(Handle);
#[zbus::interface(name = "org.freedesktop.impl.portal.Session")]
impl Endpoint {
    fn close(&self, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        if sender(&header)? != self.0.owner {
            return Err(fdo::Error::AccessDenied(
                "Input capture belongs to another caller".into(),
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
    async fn closed(emitter: &zbus::object_server::SignalEmitter<'_>) -> zbus::Result<()>;
}
impl Host {
    pub(super) async fn create(
        self: &Arc<Self>,
        connection: &Connection,
        key: OwnedObjectPath,
        app: String,
        owner: OwnedUniqueName,
    ) -> fdo::Result<()> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(stopped_error());
        }
        if key.as_str().len() > 512
            || !key
                .as_str()
                .starts_with("/org/freedesktop/portal/desktop/session/")
        {
            return Err(invalid("Invalid input capture session path"));
        }
        let slot =
            self.slots.clone().try_acquire_owned().map_err(|_| {
                fdo::Error::LimitsExceeded("Too many input capture sessions".into())
            })?;
        let bus = fdo::DBusProxy::new(connection).await?;
        let owners = bus
            .receive_name_owner_changed_with_args(&[(0, owner.as_str())])
            .await?;
        if !bus.name_has_owner(owner.clone().into()).await? {
            return Err(stopped_error());
        }
        let (commands, inbox) = mpsc::channel(16);
        let (close, _) = watch::channel(false);
        let (done, ended) = watch::channel(false);
        let (clipboard_phase, clipboard_epoch) = watch::channel(None);
        let handle = Handle {
            owner,
            app,
            commands,
            close,
            notify: Arc::new(AtomicBool::new(true)),
            done: ended,
            clipboard_epoch,
        };
        if !connection
            .object_server()
            .at(key.clone(), Endpoint(handle.clone()))
            .await?
        {
            return Err(invalid("Input capture session already exists"));
        }
        if self.shutdown.load(Ordering::SeqCst) {
            handle.close.send_replace(true);
        }
        self.entries
            .lock()
            .expect("capture sessions")
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
                clipboard_phase,
                _slot: slot,
            }
            .run(),
        );
        Ok(())
    }
}
type Pending = BoxFuture<'static, anyhow::Result<Option<Box<dyn Control>>>>;
struct Actor {
    host: Arc<Host>,
    connection: Connection,
    key: OwnedObjectPath,
    handle: Handle,
    inbox: mpsc::Receiver<Command>,
    owners: BoxStream<'static, ()>,
    done: watch::Sender<bool>,
    clipboard_phase: watch::Sender<Option<u64>>,
    _slot: tokio::sync::OwnedSemaphorePermit,
}
impl Actor {
    async fn emit(
        connection: &Connection,
        owner: &OwnedUniqueName,
        key: &OwnedObjectPath,
        event: Event,
    ) -> anyhow::Result<()> {
        let (member, options) = match event {
            Event::Activated {
                activation,
                barrier,
                position,
            } => {
                anyhow::ensure!(
                    [position.0, position.1].into_iter().all(f64::is_finite),
                    "Invalid capture position"
                );
                (
                    "Activated",
                    HashMap::from([
                        (
                            "activation_id".into(),
                            zbus::zvariant::OwnedValue::from(activation),
                        ),
                        (
                            "barrier_id".into(),
                            zbus::zvariant::OwnedValue::from(barrier),
                        ),
                        ("cursor_position".into(), Value::from(position).try_into()?),
                    ]),
                )
            }
            Event::Deactivated { activation } => (
                "Deactivated",
                HashMap::from([(
                    "activation_id".into(),
                    zbus::zvariant::OwnedValue::from(activation),
                )]),
            ),
            Event::Disabled => ("Disabled", Options::new()),
            Event::ZonesChanged { zone_set } => (
                "ZonesChanged",
                HashMap::from([(
                    "zone_set".into(),
                    zbus::zvariant::OwnedValue::from(zone_set),
                )]),
            ),
        };
        tokio::time::timeout(
            Duration::from_secs(5),
            connection.emit_signal(
                Some(owner),
                crate::OBJECT_PATH,
                INTERFACE,
                member,
                &(key, options),
            ),
        )
        .await??;
        Ok(())
    }
    async fn run(mut self) {
        let mut started = false;
        let mut approved = false;
        let mut requested_capabilities = 0;
        let mut clipboard_requested = false;
        let mut clipboard_ready = false;
        let mut start_serial = 0;
        let mut clipboard_events: Option<BoxStream<'static, ClipboardEvent>> = None;
        let mut connected = false;
        let mut startup: Option<Pending> = None;
        let mut starting_reply: Option<oneshot::Sender<fdo::Result<Outcome>>> = None;
        let mut request_cancelled: Option<watch::Receiver<bool>> = None;
        let mut resource: Option<Box<dyn Control>> = None;
        let mut events: Option<mpsc::Receiver<Event>> = None;
        let mut source_closed: Option<watch::Receiver<bool>> = None;
        let mut closing = self.handle.close.subscribe();
        enum Next {
            Close,
            OwnerLost,
            Command(Option<Command>),
            Ready(anyhow::Result<Option<Box<dyn Control>>>),
            Event(Option<Event>),
            Clipboard(Option<ClipboardEvent>),
        }
        loop {
            let next = tokio::select! {biased;
                _=stopped(&mut closing)=>Next::Close,
                _=self.owners.next()=>Next::OwnerLost,
                _=async {stopped(request_cancelled.as_mut().unwrap()).await},if startup.is_some()=>Next::Close,
                _=async {stopped(source_closed.as_mut().unwrap()).await},if resource.is_some()=>Next::Close,
                value=async {startup.as_mut().unwrap().await},if startup.is_some()=>Next::Ready(value),
                event=async {events.as_mut().unwrap().recv().await},if events.is_some()=>Next::Event(event),
                command=self.inbox.recv()=>Next::Command(command),
                event=async {clipboard_events.as_mut().unwrap().next().await},if clipboard_ready && clipboard_events.is_some()=>Next::Clipboard(event),
            };
            match next {
                Next::Close | Next::Command(None) | Next::Event(None) | Next::Clipboard(None) => {
                    break;
                }
                Next::OwnerLost => {
                    self.handle.notify.store(false, Ordering::SeqCst);
                    break;
                }
                Next::Event(Some(event)) => {
                    if let Some(clipboard) =
                        resource.as_mut().and_then(|control| control.clipboard())
                    {
                        self.clipboard_phase.send_replace(None);
                        let activation = match &event {
                            Event::Activated { activation, .. } => Some(*activation),
                            _ => None,
                        };
                        // The producer's independent gate already revokes on phase
                        // loss. Retain startup here until owned transport cleanup.
                        match tokio::time::timeout(
                            Duration::from_secs(10),
                            clipboard.synchronize(activation),
                        )
                        .await
                        {
                            Ok(Ok(epoch)) => {
                                self.clipboard_phase.send_replace(epoch);
                            }
                            _ => break,
                        }
                    }
                    let mut source = source_closed.as_ref().unwrap().clone();
                    let emitted = tokio::select! {biased;
                        _=stopped(&mut closing)=>false,
                        _=stopped(&mut source)=>false,
                        _=self.owners.next()=>{self.handle.notify.store(false,Ordering::SeqCst);false},
                        result=Self::emit(&self.connection,&self.handle.owner,&self.key,event)=>result.is_ok(),
                    };
                    if !emitted {
                        break;
                    }
                }
                Next::Command(Some(Command::Start {
                    mut request,
                    cancelled,
                    reply,
                    serial,
                })) => {
                    if started {
                        let _ = reply.send(Err(invalid("Input capture can only start once")));
                        continue;
                    }
                    started = true;
                    start_serial = serial;
                    request.clipboard = clipboard_requested;
                    requested_capabilities = request.capabilities;
                    request_cancelled = Some(cancelled);
                    let presenter = self.host.presenter.clone();
                    let connection = self.connection.clone();
                    let cancelled = self.handle.close.subscribe();
                    startup = Some(Box::pin(async move {
                        presenter.present(connection, request, cancelled).await
                    }));
                    starting_reply = Some(reply);
                }
                Next::Ready(value) => {
                    startup = None;
                    let requested = request_cancelled.take().is_some_and(|v| *v.borrow());
                    let reply = starting_reply.take().unwrap();
                    match value {
                        Ok(Some(mut control)) if !requested => {
                            let capabilities = control.capabilities();
                            if capabilities == 0
                                || capabilities & !requested_capabilities != 0
                                || control.clipboard_enabled() && !clipboard_requested
                                || *control.closed().borrow()
                            {
                                resource = Some(control);
                                let _ = reply.send(Ok(Outcome::Failed));
                                break;
                            }
                            events = control.events();
                            if events.is_none() {
                                resource = Some(control);
                                let _ = reply.send(Ok(Outcome::Failed));
                                break;
                            }
                            let clipboard_enabled = control.clipboard_enabled();
                            if clipboard_enabled {
                                clipboard_events =
                                    control.clipboard().and_then(|clipboard| clipboard.events());
                                if clipboard_events.is_none() {
                                    resource = Some(control);
                                    let _ = reply.send(Ok(Outcome::Failed));
                                    break;
                                }
                            }
                            source_closed = Some(control.closed());
                            resource = Some(control);
                            approved = true;
                            let options = HashMap::from([
                                (
                                    "capabilities".into(),
                                    zbus::zvariant::OwnedValue::from(capabilities),
                                ),
                                (
                                    "clipboard_enabled".into(),
                                    zbus::zvariant::OwnedValue::from(clipboard_enabled),
                                ),
                            ]);
                            let _ = reply.send(Ok(Outcome::Accepted(Accepted {
                                options,
                                close: self.handle.close.clone(),
                                armed: true,
                            })));
                        }
                        Ok(value) => {
                            resource = value;
                            let _ = reply.send(Ok(Outcome::Cancelled));
                            break;
                        }
                        Err(error) => {
                            eprintln!(
                                "level=warn component=input-capture event=start-failed error={error}"
                            );
                            let _ = reply.send(Ok(Outcome::Failed));
                            break;
                        }
                    }
                }
                Next::Command(Some(Command::Operation(operation, reply))) => {
                    let Some(control) = resource.as_mut() else {
                        let _ = reply.send(Err(invalid("Input capture is not started")));
                        continue;
                    };
                    if matches!(operation, Operation::Connect) && connected {
                        let _ = reply.send(Err(invalid("EI receiver already connected")));
                        continue;
                    }
                    if matches!(operation, Operation::Enable | Operation::Release { .. })
                        && !connected
                    {
                        let _ = reply.send(Err(invalid("ConnectToEIS must precede capture")));
                        continue;
                    }
                    let connect = matches!(operation, Operation::Connect);
                    let enable = matches!(operation, Operation::Enable);
                    let mut timed_out = false;
                    let result = tokio::select! {biased;
                        _=stopped(&mut closing)=>None,
                        _=self.owners.next()=>{self.handle.notify.store(false,Ordering::SeqCst);None},
                        _=async {stopped(source_closed.as_mut().unwrap()).await}=>None,
                        result=tokio::time::timeout(Duration::from_secs(12),control.perform(operation))=>Some(match result {
                            Ok(value)=>value,Err(_)=>{timed_out=true;Err(stopped_error())},
                        }),
                    };
                    let Some(result) = result else {
                        control.revoke();
                        let _ = reply.send(Err(stopped_error()));
                        break;
                    };
                    if timed_out {
                        control.revoke();
                        let _ = reply.send(result);
                        break;
                    }
                    if connect && result.is_ok() {
                        connected = true;
                    }
                    if enable
                        && result.is_err()
                        && Self::emit(
                            &self.connection,
                            &self.handle.owner,
                            &self.key,
                            Event::Disabled,
                        )
                        .await
                        .is_err()
                    {
                        let _ = reply.send(result);
                        break;
                    }
                    let _ = reply.send(result);
                }
                Next::Command(Some(Command::StartReplied(serial))) => {
                    if approved && serial == start_serial {
                        clipboard_ready = true;
                    }
                }
                Next::Command(Some(Command::Clipboard {
                    command: crate::clipboard::Command::Request,
                    reply,
                    ..
                })) => {
                    let result = if !started && !clipboard_requested {
                        clipboard_requested = true;
                        Ok(crate::clipboard::Response::Unit)
                    } else {
                        Err(invalid("Clipboard must be requested once before Start"))
                    };
                    let _ = reply.send(result);
                }
                Next::Command(Some(Command::Clipboard {
                    command,
                    epoch,
                    reply,
                })) => {
                    let Some(clipboard) = resource.as_mut().and_then(|control| control.clipboard())
                    else {
                        let _ = reply.send(Err(fdo::Error::AccessDenied(
                            "Clipboard access was not granted".into(),
                        )));
                        continue;
                    };
                    let result = tokio::select! {biased;
                        _=stopped(&mut closing)=>None,
                        _=self.owners.next()=>{self.handle.notify.store(false,Ordering::SeqCst);None},
                        _=async {stopped(source_closed.as_mut().unwrap()).await}=>None,
                        result=tokio::time::timeout(Duration::from_secs(8),clipboard.perform(command,epoch,self.handle.close.subscribe()))=>Some(result),
                    };
                    match result {
                        Some(Ok(result)) => {
                            let _ = reply.send(result);
                        }
                        _ => {
                            let _ = reply.send(Err(stopped_error()));
                            break;
                        }
                    }
                }
                Next::Clipboard(Some(ClipboardEvent { epoch, event })) => {
                    let Some(clipboard) = resource.as_mut().and_then(|control| control.clipboard())
                    else {
                        break;
                    };
                    let mut phase = clipboard.phase();
                    if *phase.borrow() != Some(epoch)
                        || *self.clipboard_phase.borrow() != Some(epoch)
                    {
                        continue;
                    }
                    let mut source = source_closed.as_ref().unwrap().clone();
                    enum Emitted {
                        Ready(bool),
                        Inactive,
                        Close,
                        OwnerLost,
                    }
                    let result = tokio::select! {biased;
                        _=stopped(&mut closing)=>Emitted::Close,
                        _=stopped(&mut source)=>Emitted::Close,
                        _=self.owners.next()=>Emitted::OwnerLost,
                        _=inactive(&mut phase,epoch)=>Emitted::Inactive,
                        result=tokio::time::timeout(Duration::from_secs(5),crate::clipboard::Clipboard::emit(&self.connection,&self.handle.owner,&self.key,event))=>Emitted::Ready(matches!(result,Ok(Ok(())))),
                    };
                    match result {
                        Emitted::Ready(true) | Emitted::Inactive => {}
                        Emitted::OwnerLost => {
                            self.handle.notify.store(false, Ordering::SeqCst);
                            break;
                        }
                        _ => break,
                    }
                }
            }
        }
        self.handle.close.send_replace(true);
        self.clipboard_phase.send_replace(None);
        // Cancellation retains the startup future until its owned helpers have
        // retired. Closing the session cannot race ahead of startup cleanup.
        if let Some(pending) = startup {
            match tokio::time::timeout(Duration::from_secs(20), pending).await {
                Ok(Ok(Some(control))) => resource = Some(control),
                Err(_) => {
                    eprintln!("level=warn component=input-capture event=start-cleanup-unconfirmed")
                }
                _ => {}
            }
        }
        if let Some(reply) = starting_reply {
            let _ = reply.send(Ok(Outcome::Cancelled));
        }
        self.inbox.close();
        while let Ok(command) = self.inbox.try_recv() {
            match command {
                Command::Start { reply, .. } => {
                    let _ = reply.send(Ok(Outcome::Cancelled));
                }
                Command::Operation(_, reply) => {
                    let _ = reply.send(Err(stopped_error()));
                }
                Command::Clipboard { reply, .. } => {
                    let _ = reply.send(Err(stopped_error()));
                }
                Command::StartReplied(_) => {}
            }
        }
        if let Some(resource) = resource {
            resource.revoke();
            if resource.stop().await.is_err() {
                eprintln!("level=warn component=input-capture event=cleanup-unconfirmed");
            }
        }
        // A failed Start is retired through its response. Emitting Closed first
        // makes the standard frontend try to export an already closed session
        // and replace an explicit user cancellation with a generic failure.
        if (!started || approved) && self.handle.notify.load(Ordering::SeqCst) {
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                self.connection.emit_signal(
                    Some(&self.handle.owner),
                    self.key.clone(),
                    "org.freedesktop.impl.portal.Session",
                    "Closed",
                    &(),
                ),
            )
            .await;
        }
        let _ = self
            .connection
            .object_server()
            .remove::<Endpoint, _>(self.key.clone())
            .await;
        self.host
            .entries
            .lock()
            .expect("capture sessions")
            .remove(&self.key);
        self.done.send_replace(true);
    }
}
