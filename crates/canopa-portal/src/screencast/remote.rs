//! Remote-control requests use the capture host's one session registry and actor.
use super::*;
use std::os::fd::OwnedFd;
mod dispatch;
pub use dispatch::RemoteDesktop;
#[cfg(feature = "desktop")]
mod availability;
#[cfg(feature = "desktop")]
pub use availability::AvailabilityWatch;
mod input;
pub use input::{Input, PointerInput, TouchInput};

#[cfg(feature = "desktop")]
mod production;
#[cfg(feature = "desktop")]
pub use production::{DialogInput, Production};

#[derive(Clone, Debug)]
pub struct DeviceOptions {
    pub types: u32,
}
impl DeviceOptions {
    fn parse(mut options: Options) -> fdo::Result<Self> {
        let types = option(&mut options, "types")?.unwrap_or(7_u32);
        if types & !7 != 0 || option::<u32>(&mut options, "persist_mode")?.unwrap_or(0) > 2 {
            return Err(fdo::Error::InvalidArgs(
                "Invalid remote device selection".into(),
            ));
        }
        // Restore data never authorizes a device; every Start asks again.
        Ok(Self { types })
    }
}
#[derive(Clone, Debug)]
pub struct StartRequest {
    pub app_id: String,
    pub caller: OwnedUniqueName,
    pub parent_window: String,
    pub modal: bool,
    pub activation_token: Option<String>,
    pub devices: DeviceOptions,
    pub sources: Option<SelectionOptions>,
    pub clipboard: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardInput {
    Keycode { code: u32, pressed: bool },
    Keysym { symbol: u32, pressed: bool },
}
impl KeyboardInput {
    fn keycode(code: i32, state: u32) -> fdo::Result<Self> {
        if !(1..=0x2ff).contains(&code) || state > 1 {
            return Err(fdo::Error::InvalidArgs(
                "Invalid keyboard keycode/state".into(),
            ));
        }
        Ok(Self::Keycode {
            code: code as u32,
            pressed: state == 1,
        })
    }
    fn keysym(symbol: i32, state: u32) -> fdo::Result<Self> {
        if !(1..=0x1fff_ffff).contains(&symbol) || state > 1 {
            return Err(fdo::Error::InvalidArgs(
                "Invalid keyboard keysym/state".into(),
            ));
        }
        Ok(Self::Keysym {
            symbol: symbol as u32,
            pressed: state == 1,
        })
    }
}

/// One approved session owns all of its input workers, optional capture and UI.
pub trait Control: Send + 'static {
    fn clipboard_enabled(&self) -> bool {
        false
    }
    fn clipboard(&mut self) -> Option<&mut dyn crate::clipboard::Control> {
        None
    }
    fn devices(&self) -> u32;
    fn streams(&self) -> &[Stream];
    fn closed(&self) -> watch::Receiver<bool>;
    fn connect<'a>(
        &'a mut self,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<OwnedFd>>;
    fn keyboard<'a>(
        &'a mut self,
        input: KeyboardInput,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, fdo::Result<()>>;
    fn pointer<'a>(
        &'a mut self,
        _input: PointerInput,
        _cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, fdo::Result<()>> {
        Box::pin(async {
            Err(fdo::Error::NotSupported(
                "Pointer transport unavailable".into(),
            ))
        })
    }
    fn input<'a>(
        &'a mut self,
        input: Input,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, fdo::Result<()>> {
        match input {
            Input::Keyboard(input) => self.keyboard(input, cancelled),
            Input::Pointer(input) => self.pointer(input, cancelled),
            Input::Touch(input) => self.touch(input, cancelled),
        }
    }
    fn touch<'a>(
        &'a mut self,
        _input: TouchInput,
        _cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, fdo::Result<()>> {
        Box::pin(async {
            Err(fdo::Error::NotSupported(
                "Touch transport unavailable".into(),
            ))
        })
    }
    fn stop(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>>;
}
pub trait Presenter: Send + Sync + 'static {
    fn available_devices<'a>(&'a self, _connection: &'a Connection) -> BoxFuture<'a, u32> {
        Box::pin(async { if cfg!(feature = "desktop") { 1 } else { 0 } })
    }
    fn present<'a>(
        &'a self,
        connection: Connection,
        request: StartRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn Control>>>>;
}
pub struct Unavailable;
impl Presenter for Unavailable {
    fn available_devices<'a>(&'a self, _: &'a Connection) -> BoxFuture<'a, u32> {
        Box::pin(async { 0 })
    }
    fn present<'a>(
        &'a self,
        _: Connection,
        _: StartRequest,
        _: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn Control>>>> {
        Box::pin(async { anyhow::bail!("Remote control is unavailable") })
    }
}

#[derive(Clone)]
struct Methods {
    capture: ScreenCast,
}
impl Methods {
    fn lookup(
        &self,
        key: &OwnedObjectPath,
        app: &str,
        owner: OwnedUniqueName,
    ) -> fdo::Result<Handle> {
        let session = self.capture.lookup(key, app, owner)?;
        if session.kind != session::Kind::Remote {
            return Err(fdo::Error::InvalidArgs(
                "Not a remote-control session".into(),
            ));
        }
        Ok(session)
    }
    fn queue_keyboard(
        &self,
        key: OwnedObjectPath,
        input: KeyboardInput,
        owner: OwnedUniqueName,
    ) -> fdo::Result<oneshot::Receiver<fdo::Result<()>>> {
        self.queue_input(key, Input::Keyboard(input), owner)
    }
    fn queue_input(
        &self,
        key: OwnedObjectPath,
        input: Input,
        owner: OwnedUniqueName,
    ) -> fdo::Result<oneshot::Receiver<fdo::Result<()>>> {
        let session = self
            .capture
            .host
            .entries
            .lock()
            .expect("sharing sessions")
            .get(&key)
            .cloned()
            .ok_or_else(|| fdo::Error::InvalidArgs("Remote session unavailable".into()))?;
        if session.kind != session::Kind::Remote || session.owner != owner {
            return Err(fdo::Error::AccessDenied(
                "Remote session belongs to another caller or interface".into(),
            ));
        }
        let (reply, result) = oneshot::channel();
        session
            .commands
            .try_send(session::Command::Input(input, reply))
            .map_err(|_| {
                // A discarded key or button release must not leave this grant active.
                session.close.send_replace(true);
                fdo::Error::LimitsExceeded("Sharing session is busy or stopped".into())
            })?;
        Ok(result)
    }
}
#[zbus::interface(name = "org.freedesktop.impl.portal.RemoteDesktop")]
impl Methods {
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
            self.capture
                .requests
                .run(connection, header, handle, |owner, _| async move {
                    self.capture
                        .host
                        .create(
                            connection,
                            session_handle,
                            app_id,
                            owner,
                            session::Kind::Remote,
                        )
                        .await
                })
                .await?,
        )
    }
    #[allow(clippy::too_many_arguments)]
    #[zbus(out_args("response", "results"))]
    async fn select_devices(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        app_id: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        let session = self.lookup(&session_handle, &app_id, sender(&header)?)?;
        let devices = match DeviceOptions::parse(options) {
            Ok(value) => value,
            Err(error) => {
                session.close.send_replace(true);
                return Err(error);
            }
        };
        response(
            self.capture
                .requests
                .run(connection, header, handle, |_, _| async move {
                    let (reply, result) = oneshot::channel();
                    session
                        .commands
                        .send(session::Command::Devices(devices, reply))
                        .await
                        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?;
                    result
                        .await
                        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
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
        context(&parent_window)?;
        let activation_token: Option<String> = option(&mut options, "activation_token")?;
        if let Some(token) = &activation_token {
            context(token)?;
        }
        let modal = option(&mut options, "modal")?.unwrap_or(true);
        let response_serial = header.primary().serial_num().get();
        response(
            self.capture
                .requests
                .run(connection, header, handle, |_, cancelled| async move {
                    let (reply, result) = oneshot::channel();
                    session
                        .commands
                        .send(session::Command::Start {
                            kind: session::Kind::Remote,
                            response_serial,
                            parent_window,
                            modal,
                            activation_token,
                            cancelled,
                            reply,
                        })
                        .await
                        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?;
                    result
                        .await
                        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
                })
                .await?,
        )
    }
    #[zbus(name = "ConnectToEIS", out_args("fd"))]
    async fn connect_to_eis(
        &self,
        session_handle: OwnedObjectPath,
        app_id: String,
        options: Options,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<zbus::zvariant::OwnedFd> {
        let session = self.lookup(&session_handle, &app_id, sender(&header)?)?;
        let _ = options;
        let (reply, result) = oneshot::channel();
        session
            .commands
            .try_send(session::Command::Connect(reply))
            .map_err(|_| fdo::Error::LimitsExceeded("Sharing session is busy or stopped".into()))?;
        Ok(result
            .await
            .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))??
            .into())
    }
    #[zbus(name = "NotifyKeyboardKeycode")]
    async fn notify_keyboard_keycode(
        &self,
        session_handle: OwnedObjectPath,
        options: Options,
        keycode: i32,
        state: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let _ = options;
        self.queue_keyboard(
            session_handle,
            KeyboardInput::keycode(keycode, state)?,
            sender(&header)?,
        )?
        .await
        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
    }
    #[zbus(name = "NotifyKeyboardKeysym")]
    async fn notify_keyboard_keysym(
        &self,
        session_handle: OwnedObjectPath,
        options: Options,
        keysym: i32,
        state: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let _ = options;
        self.queue_keyboard(
            session_handle,
            KeyboardInput::keysym(keysym, state)?,
            sender(&header)?,
        )?
        .await
        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
    }
    #[zbus(name = "NotifyPointerMotion")]
    async fn notify_pointer_motion(
        &self,
        session_handle: OwnedObjectPath,
        options: Options,
        dx: f64,
        dy: f64,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let _ = options;
        self.queue_input(
            session_handle,
            Input::Pointer(PointerInput::motion(dx, dy)?),
            sender(&header)?,
        )?
        .await
        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
    }
    #[zbus(name = "NotifyPointerMotionAbsolute")]
    async fn notify_pointer_motion_absolute(
        &self,
        session_handle: OwnedObjectPath,
        options: Options,
        stream: u32,
        x: f64,
        y: f64,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let _ = options;
        self.queue_input(
            session_handle,
            Input::Pointer(PointerInput::absolute(stream, x, y)?),
            sender(&header)?,
        )?
        .await
        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
    }
    #[zbus(name = "NotifyPointerButton")]
    async fn notify_pointer_button(
        &self,
        session_handle: OwnedObjectPath,
        options: Options,
        button: i32,
        state: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let _ = options;
        self.queue_input(
            session_handle,
            Input::Pointer(PointerInput::button(button, state)?),
            sender(&header)?,
        )?
        .await
        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
    }
    #[zbus(name = "NotifyPointerAxis")]
    async fn notify_pointer_axis(
        &self,
        session_handle: OwnedObjectPath,
        options: Options,
        dx: f64,
        dy: f64,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        self.queue_input(
            session_handle,
            Input::Pointer(PointerInput::axis(dx, dy, options)?),
            sender(&header)?,
        )?
        .await
        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
    }
    #[zbus(name = "NotifyPointerAxisDiscrete")]
    async fn notify_pointer_axis_discrete(
        &self,
        session_handle: OwnedObjectPath,
        options: Options,
        axis: u32,
        steps: i32,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let _ = options;
        self.queue_input(
            session_handle,
            Input::Pointer(PointerInput::discrete(axis, steps)?),
            sender(&header)?,
        )?
        .await
        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
    }
    #[allow(clippy::too_many_arguments)]
    #[zbus(name = "NotifyTouchDown")]
    async fn notify_touch_down(
        &self,
        session_handle: OwnedObjectPath,
        options: Options,
        stream: u32,
        slot: u32,
        x: f64,
        y: f64,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let _ = options;
        self.queue_input(
            session_handle,
            Input::Touch(TouchInput::down(stream, slot, x, y)?),
            sender(&header)?,
        )?
        .await
        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
    }
    #[allow(clippy::too_many_arguments)]
    #[zbus(name = "NotifyTouchMotion")]
    async fn notify_touch_motion(
        &self,
        session_handle: OwnedObjectPath,
        options: Options,
        stream: u32,
        slot: u32,
        x: f64,
        y: f64,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let _ = options;
        self.queue_input(
            session_handle,
            Input::Touch(TouchInput::motion(stream, slot, x, y)?),
            sender(&header)?,
        )?
        .await
        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
    }
    #[zbus(name = "NotifyTouchUp")]
    async fn notify_touch_up(
        &self,
        session_handle: OwnedObjectPath,
        options: Options,
        slot: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let _ = options;
        self.queue_input(
            session_handle,
            Input::Touch(TouchInput::Up { slot }),
            sender(&header)?,
        )?
        .await
        .map_err(|_| fdo::Error::Failed("Sharing session stopped".into()))?
    }
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }
    #[zbus(property)]
    async fn available_device_types(&self, #[zbus(connection)] connection: &Connection) -> u32 {
        self.capture
            .host
            .remote_presenter
            .available_devices(connection)
            .await
    }
}

pub(super) fn results(
    value: &dyn Control,
    devices: &DeviceOptions,
    sources: Option<&SelectionOptions>,
    clipboard_requested: bool,
) -> anyhow::Result<Options> {
    let granted = value.devices();
    anyhow::ensure!(
        granted & !devices.types == 0 && granted & !7 == 0,
        "Invalid remote device grant"
    );
    anyhow::ensure!(
        granted & 4 == 0 || !value.streams().is_empty(),
        "Touch requires an approved capture stream"
    );
    let mut results = match sources {
        Some(options) => super::results(value.streams(), options)?,
        None => {
            anyhow::ensure!(value.streams().is_empty(), "Unrequested capture stream");
            Options::new()
        }
    };
    anyhow::ensure!(
        granted != 0 || !value.streams().is_empty() || value.clipboard_enabled(),
        "Empty sharing grant"
    );
    results.insert("devices".into(), granted.into());
    results.insert("persist_mode".into(), 0_u32.into());
    anyhow::ensure!(
        !value.clipboard_enabled() || clipboard_requested,
        "Unrequested clipboard grant"
    );
    results.insert("clipboard_enabled".into(), value.clipboard_enabled().into());
    Ok(results)
}
