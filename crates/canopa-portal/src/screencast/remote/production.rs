//! Fresh combined consent, one control surface and separately owned input leases.
use super::*;
use crate::{
    process::DialogProcess,
    remote_input::{
        Connector, Endpoint as InputEndpoint, InputLease, KeyboardStatus, LegacyInput, Watch,
    },
    screencast::{
        DialogInput as CaptureInput, Production as CaptureProduction, production::inventory,
    },
    screencast_capture::Owner,
    screencast_status::{Indicator, Input as StatusInput},
    screencast_ui::Selection,
};
use serde::{Deserialize, Serialize};

mod pointer;
#[cfg(test)]
mod tests;
mod touch;
use pointer::LegacyPointer;
use touch::LegacyTouch;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogInput {
    pub capture: CaptureInput,
    pub devices: u32,
    #[serde(default)]
    pub clipboard: bool,
}
pub struct Production {
    process: Arc<DialogProcess>,
    capture: Arc<CaptureProduction>,
    connector: Connector,
}
impl Production {
    pub fn new(process: Arc<DialogProcess>, capture: Arc<CaptureProduction>) -> Self {
        Self {
            connector: Connector::new(process.clone()),
            process,
            capture,
        }
    }
}
struct Lifetime(watch::Sender<bool>);
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}
async fn stopped(signal: &mut watch::Receiver<bool>) {
    while !*signal.borrow() {
        if signal.changed().await.is_err() {
            return;
        }
    }
}
fn supervise(source: watch::Receiver<bool>, ended: watch::Sender<bool>) {
    drop(watch_source(source, ended)); // Detach until the common lifetime closes.
}
fn watch_source(
    mut source: watch::Receiver<bool>,
    ended: watch::Sender<bool>,
) -> tokio::task::JoinHandle<()> {
    let mut closing = ended.subscribe();
    tokio::spawn(async move {
        tokio::select! {
            biased;
            _ = stopped(&mut closing) => {},
            _ = stopped(&mut source) => {ended.send_replace(true);},
        }
    })
}
struct Monitor(tokio::task::JoinHandle<()>);
impl Drop for Monitor {
    fn drop(&mut self) {
        self.0.abort();
    }
}
struct Legacy {
    input: LegacyInput,
    monitor: Monitor,
}
impl Legacy {
    async fn stop(self) -> anyhow::Result<()> {
        let Self { input, monitor } = self;
        drop(monitor); // An intentional input-mode transition does not end consent.
        input.stop().await
    }
}
struct Active {
    lifetime: Lifetime,
    _watch: Option<Watch>,
    capture: Option<Box<dyn Capture>>,
    indicator: Option<Indicator>,
    inputs: Vec<InputLease>,
    legacy: Option<Legacy>,
    pointer: Option<LegacyPointer>,
    touch: Option<LegacyTouch>,
    connection: Connection,
    endpoint: Option<InputEndpoint>,
    caller: OwnedUniqueName,
    connector: Connector,
    devices: u32,
    clipboard: Option<crate::clipboard::production::Production>,
    _process_watch: Option<Monitor>,
}
impl Control for Active {
    fn clipboard_enabled(&self) -> bool {
        self.clipboard.is_some()
    }
    fn clipboard(&mut self) -> Option<&mut dyn crate::clipboard::Control> {
        self.clipboard
            .as_mut()
            .map(|c| c as &mut dyn crate::clipboard::Control)
    }

    fn devices(&self) -> u32 {
        self.devices
    }
    fn streams(&self) -> &[Stream] {
        self.capture
            .as_ref()
            .map_or(&[], |capture| capture.streams())
    }
    fn closed(&self) -> watch::Receiver<bool> {
        if self
            .capture
            .as_ref()
            .is_some_and(|capture| *capture.closed().borrow())
            || self
                .indicator
                .as_ref()
                .is_some_and(|indicator| *indicator.closed().borrow())
            || self
                .clipboard
                .as_ref()
                .is_some_and(|clipboard| clipboard.is_closed())
            || self.inputs.iter().any(|input| *input.closed().borrow())
            || self
                .legacy
                .as_ref()
                .is_some_and(|legacy| *legacy.input.closed().borrow())
            || self
                .pointer
                .as_ref()
                .is_some_and(|pointer| *pointer.closed().borrow())
            || self
                .touch
                .as_ref()
                .is_some_and(|touch| *touch.closed().borrow())
        {
            self.lifetime.0.send_replace(true);
        }
        self.lifetime.0.subscribe()
    }
    fn connect<'a>(
        &'a mut self,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<OwnedFd>> {
        Box::pin(async move {
            anyhow::ensure!(
                self.devices != 0 && !*self.closed().borrow() && self.inputs.len() < 4,
                "Input connection unavailable"
            );
            let keyboard = self.legacy.take();
            let pointer = self.pointer.take();
            let touch = self.touch.take();
            let (keyboard, pointer, touch) = tokio::join!(
                async {
                    match keyboard {
                        Some(input) => input.stop().await,
                        None => Ok(()),
                    }
                },
                async {
                    match pointer {
                        Some(input) => input.stop().await,
                        None => Ok(()),
                    }
                },
                async {
                    match touch {
                        Some(input) => input.stop().await,
                        None => Ok(()),
                    }
                },
            );
            if keyboard.is_err() || pointer.is_err() || touch.is_err() {
                self.lifetime.0.send_replace(true);
            }
            keyboard?;
            pointer?;
            touch?;
            anyhow::ensure!(
                !*self.closed().borrow() && !*cancelled.borrow(),
                "Sharing session ended during input transition"
            );
            let endpoint = self
                .endpoint
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Input endpoint unavailable"))?;
            let (fd, lease) = self
                .connector
                .start_bound(
                    &self.connection,
                    endpoint,
                    self.caller.clone(),
                    self.devices,
                    if self.devices & 6 != 0 {
                        self.capture
                            .as_ref()
                            .and_then(|capture| capture.input_bindings())
                    } else {
                        None
                    },
                    cancelled,
                )
                .await?;
            supervise(lease.closed(), self.lifetime.0.clone());
            self.inputs.push(lease); // Never retain another copy of the transferred EI FD.
            anyhow::ensure!(!*self.closed().borrow(), "Sharing session ended");
            Ok(fd)
        })
    }
    fn keyboard<'a>(
        &'a mut self,
        input: KeyboardInput,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, fdo::Result<()>> {
        Box::pin(async move {
            if self.devices & 1 == 0 || !self.inputs.is_empty() {
                return Err(fdo::Error::AccessDenied(
                    "Legacy keyboard input was not granted or EIS is connected".into(),
                ));
            }
            if *self.closed().borrow() || *cancelled.borrow() {
                return Err(fdo::Error::Failed("Sharing session ended".into()));
            }
            if self.legacy.is_none() {
                let endpoint = self
                    .endpoint
                    .as_ref()
                    .ok_or_else(|| fdo::Error::Failed("Input endpoint unavailable".into()))?;
                let opening = self.connector.start_legacy(
                    &self.connection,
                    endpoint,
                    self.caller.clone(),
                    cancelled,
                );
                match opening.await {
                    Ok(input) => {
                        let monitor =
                            Monitor(watch_source(input.closed(), self.lifetime.0.clone()));
                        self.legacy = Some(Legacy { input, monitor });
                    }
                    Err(_) => {
                        self.lifetime.0.send_replace(true);
                        return Err(fdo::Error::Failed(
                            "Legacy keyboard input unavailable".into(),
                        ));
                    }
                }
            }
            if *self.closed().borrow() {
                return Err(fdo::Error::Failed("Sharing session ended".into()));
            }
            match self
                .legacy
                .as_mut()
                .expect("active legacy input")
                .input
                .send(input)
                .await
            {
                Ok(KeyboardStatus::Accepted) => Ok(()),
                Ok(KeyboardStatus::Invalid) => Err(fdo::Error::InvalidArgs(
                    "Invalid keyboard transition".into(),
                )),
                Ok(KeyboardStatus::Unavailable) => Err(fdo::Error::NotSupported(
                    "Keyboard symbol unavailable in current state".into(),
                )),
                Ok(KeyboardStatus::Conflict) => Err(fdo::Error::InvalidArgs(
                    "Keyboard symbol conflicts with a held key".into(),
                )),
                Ok(KeyboardStatus::Limit) => Err(fdo::Error::LimitsExceeded(
                    "Too many held keyboard keys".into(),
                )),
                Err(_) => {
                    self.lifetime.0.send_replace(true);
                    Err(fdo::Error::Failed("Legacy keyboard transport ended".into()))
                }
            }
        })
    }
    fn pointer<'a>(
        &'a mut self,
        input: PointerInput,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, fdo::Result<()>> {
        Box::pin(async move {
            if self.devices & 2 == 0 || !self.inputs.is_empty() {
                return Err(fdo::Error::AccessDenied(
                    "Legacy pointer input was not granted or EIS is connected".into(),
                ));
            }
            if *self.closed().borrow() || *cancelled.borrow() {
                return Err(fdo::Error::Failed("Sharing session ended".into()));
            }
            if self.pointer.is_none() {
                if pointer::zero(input) {
                    return Ok(());
                }
                let endpoint = self
                    .endpoint
                    .as_ref()
                    .ok_or_else(|| fdo::Error::Failed("Input endpoint unavailable".into()))?;
                let binding = self
                    .capture
                    .as_ref()
                    .and_then(|capture| capture.input_bindings());
                match LegacyPointer::open(
                    &self.connector,
                    &self.connection,
                    endpoint,
                    self.caller.clone(),
                    binding,
                    cancelled,
                    self.lifetime.0.clone(),
                )
                .await
                {
                    Ok(pointer) => self.pointer = Some(pointer),
                    Err(_) => {
                        self.lifetime.0.send_replace(true);
                        return Err(fdo::Error::Failed(
                            "Legacy pointer input unavailable".into(),
                        ));
                    }
                }
            }
            if *self.closed().borrow() {
                return Err(fdo::Error::Failed("Sharing session ended".into()));
            }
            self.pointer
                .as_mut()
                .expect("active legacy pointer")
                .send(input)
                .await
        })
    }
    fn touch<'a>(
        &'a mut self,
        input: TouchInput,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, fdo::Result<()>> {
        Box::pin(async move {
            if self.devices & 4 == 0 || !self.inputs.is_empty() {
                return Err(fdo::Error::AccessDenied(
                    "Legacy touch input was not granted or EIS is connected".into(),
                ));
            }
            if *self.closed().borrow() || *cancelled.borrow() {
                return Err(fdo::Error::Failed("Sharing session ended".into()));
            }
            if self.touch.is_none() {
                let endpoint = self
                    .endpoint
                    .as_ref()
                    .ok_or_else(|| fdo::Error::Failed("Input endpoint unavailable".into()))?;
                let binding = self
                    .capture
                    .as_ref()
                    .and_then(|capture| capture.input_bindings())
                    .ok_or_else(|| {
                        fdo::Error::Failed("Touch capture bindings unavailable".into())
                    })?;
                match LegacyTouch::open(
                    &self.connector,
                    &self.connection,
                    endpoint,
                    self.caller.clone(),
                    binding,
                    cancelled,
                    self.lifetime.0.clone(),
                )
                .await
                {
                    Ok(touch) => self.touch = Some(touch),
                    Err(_) => {
                        self.lifetime.0.send_replace(true);
                        return Err(fdo::Error::Failed("Legacy touch input unavailable".into()));
                    }
                }
            }
            if *self.closed().borrow() {
                return Err(fdo::Error::Failed("Sharing session ended".into()));
            }
            self.touch
                .as_mut()
                .expect("active legacy touch")
                .send(input)
                .await
        })
    }
    fn stop(mut self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>> {
        Box::pin(async move {
            self.lifetime.0.send_replace(true);
            self._watch.take();
            let capture = self.capture.take();
            let clipboard = self.clipboard.take();
            self._process_watch.take();
            let indicator = self.indicator.take();
            let inputs = std::mem::take(&mut self.inputs);
            let legacy = self.legacy.take();
            let pointer = self.pointer.take();
            let touch = self.touch.take();
            let (capture, pointer, controls, inputs, legacy, clipboard, touch) = tokio::join!(
                async {
                    match capture {
                        Some(capture) => capture.stop().await,
                        None => Ok(()),
                    }
                },
                async {
                    match pointer {
                        Some(pointer) => pointer.stop().await,
                        None => Ok(()),
                    }
                },
                async {
                    match indicator {
                        Some(indicator) => indicator.stop().await,
                        None => Ok(()),
                    }
                },
                futures_util::future::join_all(inputs.into_iter().map(InputLease::stop)),
                async {
                    match legacy {
                        Some(legacy) => legacy.stop().await,
                        None => Ok(()),
                    }
                },
                async {
                    match clipboard {
                        Some(clipboard) => clipboard.stop().await,
                        None => Ok(()),
                    }
                },
                async {
                    match touch {
                        Some(input) => input.stop().await,
                        None => Ok(()),
                    }
                },
            );
            clipboard?;
            capture?;
            controls?;
            for input in inputs {
                input?;
            }
            legacy?;
            pointer?;
            touch?;
            Ok(())
        })
    }
}
impl Presenter for Production {
    fn available_devices<'a>(&'a self, connection: &'a Connection) -> BoxFuture<'a, u32> {
        Box::pin(async move {
            let Ok(socket) = self.capture.socket() else {
                return 0;
            };
            let Ok(endpoint) = InputEndpoint::resolve(connection, socket.clone()).await else {
                return 0;
            };
            endpoint.devices(connection, false).await.unwrap_or(0)
        })
    }
    fn present<'a>(
        &'a self,
        connection: Connection,
        request: StartRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn Control>>>> {
        Box::pin(async move {
            if *cancelled.borrow() {
                return Ok(None);
            }
            let socket = self.capture.socket()?.clone();
            let mut endpoint = if request.devices.types & 7 != 0 {
                Some(InputEndpoint::resolve(&connection, socket.clone()).await?)
            } else {
                None
            };
            let devices = request.devices.types
                & match &endpoint {
                    Some(endpoint) => {
                        endpoint
                            .devices(&connection, request.sources.is_some())
                            .await?
                    }
                    None => 0,
                };
            let devices = if request.sources.is_some() {
                devices
            } else {
                devices & !4
            };
            anyhow::ensure!(
                devices != 0 || request.sources.is_some() || request.clipboard,
                "No supported sharing controls; touch requires a shared screen or window"
            );
            let owner = if request.sources.is_some() {
                Some(Owner::resolve(&connection).await?)
            } else {
                None
            };
            let pid = match &owner {
                Some(owner) => owner.process_id(&connection).await?,
                None => match &endpoint {
                    Some(endpoint) => endpoint.process_id(),
                    None if request.clipboard => {
                        let stream = tokio::time::timeout(
                            Duration::from_secs(2),
                            tokio::net::UnixStream::connect(&socket),
                        )
                        .await??;
                        stream
                            .peer_cred()?
                            .pid()
                            .and_then(|p| u32::try_from(p).ok())
                            .filter(|p| *p != 0)
                            .ok_or_else(|| anyhow::anyhow!("Clipboard compositor unavailable"))?
                    }
                    None => anyhow::bail!("No supported sharing request"),
                },
            };
            anyhow::ensure!(
                endpoint
                    .as_ref()
                    .is_none_or(|input| input.process_id() == pid),
                "Input and capture belong to different compositors"
            );
            let (types, multiple) = request
                .sources
                .as_ref()
                .map_or((0, false), |options| (options.types, options.multiple));
            let items = inventory(&connection, owner.as_ref(), &socket, pid, types).await?;
            anyhow::ensure!(
                types == 0 || !items.is_empty(),
                "No capture sources available"
            );
            let (closed, mut ending) = watch::channel(false);
            let lifetime = Lifetime(closed);
            let process_watch = if request.clipboard {
                let process = rustix::process::Pid::from_raw(i32::try_from(pid)?)
                    .ok_or_else(|| anyhow::anyhow!("Invalid compositor PID"))?;
                let fd =
                    rustix::process::pidfd_open(process, rustix::process::PidfdFlags::empty())?;
                // SAFETY: the OwnedFd keeps its pidfd open until the AsyncFd is dropped.
                let fd = unsafe { tokio::io::unix::AsyncFd::register(fd) }
                    .map_err(std::io::Error::from)?;
                let ended = lifetime.0.clone();
                let mut ending = ended.subscribe();
                Some(Monitor(tokio::spawn(async move {
                    tokio::select! { biased; _=stopped(&mut ending)=>{}, _=fd.readable()=>{ended.send_replace(true);}, }
                })))
            } else {
                None
            };
            let mut monitor = match &endpoint {
                Some(endpoint) => Some(
                    endpoint
                        .observe(&connection, request.caller.clone(), lifetime.0.clone())
                        .await?,
                ),
                None => None,
            };
            let mut request_ended = cancelled.clone();
            let opening = async move {
                let input = DialogInput {
                    capture: CaptureInput {
                        app_id: request.app_id.clone(),
                        parent_window: request.parent_window,
                        modal: request.modal,
                        multiple,
                        items,
                        socket: socket.clone(),
                        compositor_pid: pid,
                        compositor_owner: owner
                            .as_ref()
                            .map(|owner| owner.unique_name().to_owned()),
                        types,
                        activation_token: request.activation_token.clone(),
                    },
                    devices,
                    clipboard: request.clipboard,
                };
                let selected: Option<Selection> = self
                    .process
                    .exchange_activated(
                        "--remote-desktop-dialog",
                        input,
                        cancelled.clone(),
                        request.activation_token.as_deref(),
                    )
                    .await?;
                let Some(selected) = selected else {
                    return Ok(None);
                };
                anyhow::ensure!(
                    selected.devices & !devices == 0 && selected.devices & !7 == 0,
                    "Invalid input approval"
                );
                anyhow::ensure!(
                    !selected.clipboard || request.clipboard,
                    "Unrequested clipboard approval"
                );
                let current = inventory(&connection, owner.as_ref(), &socket, pid, types).await?;
                if let Some(options) = &request.sources {
                    let unique: HashSet<_> = selected.sources.iter().collect();
                    anyhow::ensure!(
                        !selected.sources.is_empty()
                            && selected.sources.len() <= 16
                            && unique.len() == selected.sources.len()
                            && (options.multiple || selected.sources.len() == 1)
                            && selected
                                .sources
                                .iter()
                                .all(|source| current.iter().any(|item| &item.source == source)),
                        "Invalid capture approval"
                    );
                } else {
                    anyhow::ensure!(selected.sources.is_empty(), "Unrequested capture approval");
                }
                anyhow::ensure!(
                    selected.devices != 0 || !selected.sources.is_empty() || selected.clipboard,
                    "Empty sharing approval"
                );
                if selected.devices != 0 {
                    anyhow::ensure!(
                        Some(InputEndpoint::resolve(&connection, socket.clone()).await?)
                            == endpoint,
                        "Input compositor was replaced during consent"
                    );
                    anyhow::ensure!(
                        selected.devices
                            & !endpoint
                                .as_ref()
                                .unwrap()
                                .devices(&connection, request.sources.is_some())
                                .await?
                            == 0,
                        "Input capabilities changed during consent"
                    );
                } else {
                    monitor.take();
                    endpoint = None;
                }
                anyhow::ensure!(
                    !*lifetime.0.borrow() && !*cancelled.borrow(),
                    "Sharing ended during consent"
                );
                let labels = selected
                    .sources
                    .iter()
                    .map(|source| {
                        let item = current
                            .iter()
                            .find(|item| &item.source == source)
                            .expect("validated source");
                        let kind = match source {
                            Source::Monitor(_) => "Display",
                            Source::Window(_) => "Window",
                        };
                        if item.title.trim().is_empty() {
                            kind.to_owned()
                        } else {
                            format!("{kind} · {}", item.title)
                        }
                    })
                    .collect();
                let capture = match (&owner, &request.sources) {
                    (Some(owner), Some(options)) => Some(
                        self.capture
                            .start_approved(
                                &connection,
                                owner,
                                request.caller.clone(),
                                selected.sources,
                                options.cursor,
                                cancelled.clone(),
                            )
                            .await?,
                    ),
                    _ => None,
                };
                let capture = match capture {
                    Some(recording) if selected.devices & 6 != 0 => {
                        let valid = recording.input_bindings().is_some_and(|binding| {
                            binding.validate().is_ok()
                                && binding
                                    .regions
                                    .iter()
                                    .all(|region| region.physical_scale.is_some())
                        });
                        if !valid {
                            recording.stop().await?;
                            anyhow::bail!("Captured input regions unavailable");
                        }
                        Some(recording)
                    }
                    other => other,
                };
                let clipboard = if selected.clipboard {
                    Some(
                        crate::clipboard::production::Production::start(pid, lifetime.0.clone())
                            .await?,
                    )
                } else {
                    None
                };
                let mut active = Active {
                    lifetime,
                    _watch: monitor,
                    capture,
                    indicator: None,
                    inputs: Vec::new(),
                    legacy: None,
                    pointer: None,
                    touch: None,
                    connection,
                    endpoint,
                    caller: request.caller,
                    connector: self.connector.clone(),
                    devices: selected.devices,
                    clipboard,
                    _process_watch: process_watch,
                };
                if let Some(capture) = &active.capture {
                    supervise(capture.closed(), active.lifetime.0.clone());
                }
                let controls = StatusInput::for_remote_clipboard(
                    &request.app_id,
                    labels,
                    active.devices,
                    active.clipboard_enabled(),
                )?;
                let indicator =
                    Indicator::start(&self.process, controls, active.closed(), cancelled).await?;
                supervise(indicator.closed(), active.lifetime.0.clone());
                active.indicator = Some(indicator);
                anyhow::ensure!(
                    !*active.closed().borrow(),
                    "Sharing ended before controls ready"
                );
                Ok(Some(Box::new(active) as Box<dyn Control>))
            };
            tokio::select! {
                biased;
                _ = stopped(&mut ending) => anyhow::bail!("Sharing compositor ended"),
                _ = stopped(&mut request_ended) => Ok(None),
                result = opening => result,
            }
        })
    }
}
