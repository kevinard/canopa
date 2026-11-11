//! Ordinary xdg-shell dialogs. The compositor retains focus and session-lock policy.
use crate::{
    controller::{Controller, Phase},
    native::{self, Event as AuthEvent},
};
use anyhow::{Result, bail};
use calloop::{
    EventLoop, LoopHandle, RegistrationToken,
    channel::{self, Event},
    timer::{TimeoutAction, Timer},
};
use calloop_wayland_source::WaylandSource;
use canopa_session_ui::{layout::Rect, settings::Settings, view::AUTHORIZATION_DIALOG_SIZE};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{
            CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer,
        },
    },
    shell::{
        WaylandSurface,
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use std::{path::PathBuf, time::Duration};
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

struct App {
    accessibility: Option<canopa_session_ui::accessibility::Accessibility>,
    identities: crate::identity::Worker,
    identity_key: Option<(u64, u32)>,
    identity_epoch: u64,
    input: canopa_session_ui::input::Keyboard,
    handle: LoopHandle<'static, App>,
    qh: QueueHandle<Self>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    compositor: CompositorState,
    shell: XdgShell,
    shm: Shm,
    pool: SlotPool,
    window: Option<Window>,
    configured: bool,
    focused: bool,
    keyboards: Vec<(wl_seat::WlSeat, wl_keyboard::WlKeyboard)>,
    pointers: Vec<(wl_seat::WlSeat, ThemedPointer)>,
    controller: Option<Controller>,
    settings: Settings,
    width: u32,
    height: u32,
    scale: i32,
    dirty: bool,
    exit: bool,
    error: bool,
    ready: bool,
    hits: Vec<(usize, Rect)>,
    watchdog: Option<RegistrationToken>,
    notify: Option<PathBuf>,
}
fn ready(path: &std::path::Path) -> std::io::Result<()> {
    use std::{
        os::unix::ffi::OsStrExt,
        os::{
            linux::net::SocketAddrExt,
            unix::net::{SocketAddr, UnixDatagram},
        },
    };
    let bytes = path.as_os_str().as_bytes();
    let address = if bytes.first() == Some(&b'@') {
        SocketAddr::from_abstract_name(&bytes[1..])?
    } else {
        SocketAddr::from_pathname(path)?
    };
    UnixDatagram::unbound()?.send_to_addr(b"READY=1", &address)?;
    Ok(())
}
pub fn run(settings: Settings, session: &str, notify: Option<PathBuf>) -> Result<()> {
    let connection = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<App>(&connection)?;
    let qh = queue.handle();
    let mut loop_ = EventLoop::<App>::try_new()?;
    let compositor = CompositorState::bind(&globals, &qh)?;
    let shell = XdgShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let pool = SlotPool::new(4096, &shm)?;
    let (sender, receiver) = channel::channel();
    let _native = native::Runtime::start(session, settings.auth_timeout as u32, sender)?;
    let (identity_sender, identity_receiver) = channel::channel();
    let identities = crate::identity::Worker::start(move |epoch, identity| {
        let _ = identity_sender.send((epoch, identity));
    });
    let mut app = App {
        identities,
        identity_key: None,
        identity_epoch: 0,
        accessibility: None,
        input: Default::default(),
        handle: loop_.handle(),
        qh: qh.clone(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        compositor,
        shell,
        shm,
        pool,
        window: None,
        configured: false,
        focused: false,
        keyboards: Vec::new(),
        pointers: Vec::new(),
        controller: None,
        settings,
        width: AUTHORIZATION_DIALOG_SIZE.0,
        height: AUTHORIZATION_DIALOG_SIZE.1,
        scale: 1,
        dirty: false,
        exit: false,
        error: false,
        ready: false,
        hits: Vec::new(),
        watchdog: None,
        notify,
    };
    loop_
        .handle()
        .insert_source(identity_receiver, |event, _, app: &mut App| {
            if let Event::Msg((epoch, Some(identity))) = event
                && epoch == app.identity_epoch
                && let Some(c) = &mut app.controller
            {
                if app.identity_key != Some((c.generation, identity.uid)) {
                    return;
                }
                c.identity(identity);
                app.dirty = true;
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let (a11y_sender, a11y_receiver) = channel::channel();
    app.accessibility = Some(canopa_session_ui::accessibility::Accessibility::new(
        &fl!("polkit-title"),
        move |request| {
            let _ = a11y_sender.send(request);
        },
    ));
    loop_
        .handle()
        .insert_source(a11y_receiver, |event, _, app| {
            if let Event::Msg(request) = event {
                app.accessible(request);
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    app.watchdog = Some(
        app.handle
            .insert_source(
                Timer::from_duration(Duration::from_secs(10)),
                |_, _, app| {
                    app.watchdog = None;
                    if !app.ready {
                        app.error = true;
                        app.exit = true;
                    }
                    TimeoutAction::Drop
                },
            )
            .map_err(|error| anyhow::anyhow!(error.to_string()))?,
    );
    loop_
        .handle()
        .insert_source(receiver, |event, _, app| {
            if let Event::Msg(event) = event
                && let Err(error) = app.event(event)
            {
                eprintln!("canopa-polkit: {error:#}");
                app.error = true;
                app.exit = true;
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    WaylandSource::new(connection.clone(), queue)
        .insert(loop_.handle())
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    while !app.exit {
        loop_.dispatch(None, &mut app)?;
        if !app.exit {
            app.paint()?;
        }
        connection.flush()?;
    }
    if app.error {
        bail!("authentication agent stopped unexpectedly");
    }
    Ok(())
}
impl App {
    fn accessible(&mut self, request: canopa_session_ui::accessibility::Request) {
        if let (Some(a), Some(c)) = (&mut self.accessibility, &self.controller) {
            a.update(&fl!("polkit-title"), &c.ui, &self.settings, self.focused);
        }
        use canopa_session_ui::accessibility::{Operation, select};
        if !self.focused
            || !self
                .accessibility
                .as_ref()
                .is_some_and(|a| a.accepts(&request))
        {
            return;
        }
        match request.operation {
            Operation::Control(id) => self.action(id),
            Operation::Submit => self.action(2),
            Operation::Cancel => self.action(3),
            Operation::Replace(secret) => {
                if let Some(c) = &mut self.controller
                    && c.ui.prompt.is_some()
                    && !c.ui.hide_input
                {
                    c.clear_input();
                    c.input(secret.text());
                }
            }
            Operation::Select {
                anchor,
                focus,
                echo,
            } => {
                if let Some(c) = &mut self.controller
                    && c.ui.prompt.is_some()
                    && !c.ui.hide_input
                {
                    select(&mut c.ui.secret, anchor, focus, echo);
                }
            }
        }
        self.dirty = true;
    }

    fn remove_watchdog(&mut self) {
        if let Some(token) = self.watchdog.take() {
            self.handle.remove(token);
        }
    }
    fn arm_cancel_watchdog(&mut self) -> Result<()> {
        self.remove_watchdog();
        let generation = self.controller.as_ref().map(|c| c.generation).unwrap_or(0);
        self.watchdog = Some(
            self.handle
                .insert_source(
                    Timer::from_duration(Duration::from_secs(3)),
                    move |_, _, app| {
                        app.watchdog = None;
                        if app
                            .controller
                            .as_ref()
                            .is_some_and(|c| c.active() && c.generation == generation)
                        {
                            app.error = true;
                            app.exit = true;
                        }
                        TimeoutAction::Drop
                    },
                )
                .map_err(|error| anyhow::anyhow!(error.to_string()))?,
        );
        Ok(())
    }
    fn event(&mut self, event: AuthEvent) -> Result<()> {
        match event {
            AuthEvent::Ready => {
                self.ready = true;
                self.remove_watchdog();
                if let Some(path) = self.notify.take() {
                    ready(&path)?;
                }
            }
            AuthEvent::Stopped(status) => {
                self.error = status != 0;
                self.exit = true;
            }
            AuthEvent::Stopping => self.exit = true,
            AuthEvent::Fatal => {
                self.error = true;
                self.exit = true;
            }
            event => {
                let begin = matches!(event, AuthEvent::Begin { .. });
                if begin {
                    self.remove_watchdog();
                    if self.controller.is_none() {
                        self.controller = Some(Controller::new(&self.settings)?);
                    }
                    if self.window.is_none() {
                        let window = self.shell.create_window(
                            self.compositor.create_surface(&self.qh),
                            WindowDecorations::RequestServer,
                            &self.qh,
                        );
                        window.set_app_id("org.canopa.Polkit");
                        window.set_title(fl!("polkit-window-title"));
                        window.set_min_size(Some(AUTHORIZATION_DIALOG_SIZE));
                        window.set_max_size(Some(AUTHORIZATION_DIALOG_SIZE));
                        window.commit();
                        self.window = Some(window);
                        self.configured = false;
                        self.focused = false;
                        self.width = AUTHORIZATION_DIALOG_SIZE.0;
                        self.height = AUTHORIZATION_DIALOG_SIZE.1;
                        self.scale = 1;
                    }
                }
                if let Some(c) = &mut self.controller {
                    c.event(event);
                }
                if begin {
                    let generation = self.controller.as_ref().unwrap().generation;
                    self.watchdog =
                        Some(
                            self.handle
                                .insert_source(
                                    Timer::from_duration(Duration::from_secs(
                                        self.settings.auth_timeout + 3,
                                    )),
                                    move |_, _, app| {
                                        app.watchdog = None;
                                        if app.controller.as_ref().is_some_and(|c| {
                                            c.active() && c.generation == generation
                                        }) {
                                            if let Some(c) = &mut app.controller {
                                                c.cancel();
                                            }
                                            app.dirty = true;
                                            if app.arm_cancel_watchdog().is_err() {
                                                app.error = true;
                                                app.exit = true;
                                            }
                                        }
                                        TimeoutAction::Drop
                                    },
                                )
                                .map_err(|error| anyhow::anyhow!(error.to_string()))?,
                        );
                }
                if self.controller.as_ref().is_some_and(|c| !c.active()) {
                    self.remove_watchdog();
                    self.window = None;
                    self.configured = false;
                    self.focused = false;
                    self.hits.clear();
                    // Retire frame storage with the dialog, including visible OTP frames.
                    self.pool = SlotPool::new(4096, &self.shm)?;
                }
                self.dirty = true;
            }
        }
        Ok(())
    }
    fn action(&mut self, id: usize) {
        if id == canopa_session_ui::input::LAYOUT_CONTROL {
            self.input.cycle();
            self.dirty = true;
            return;
        }
        if let Some(c) = &mut self.controller {
            match id {
                1 => c.select(),
                2 => c.submit(),
                3 => c.cancel(),
                4 => c.details(),
                5 => c.previous_page(),
                _ => {}
            }
        }
        if self
            .controller
            .as_ref()
            .is_some_and(|c| c.phase == Phase::Cancelling)
            && self.arm_cancel_watchdog().is_err()
        {
            self.error = true;
            self.exit = true;
        }
        self.dirty = true;
    }
    fn key(&mut self, mut event: KeyEvent) {
        let (symbol, text) = self
            .input
            .translate(event.raw_code, event.keysym, event.utf8.take());
        event.keysym = symbol;
        if !self.focused {
            return;
        }
        if text.as_str() == "\u{15}" {
            if let Some(c) = &mut self.controller {
                c.clear_input();
            }
            self.dirty = true;
            return;
        }
        if event.keysym == Keysym::F6 {
            self.action(canopa_session_ui::input::LAYOUT_CONTROL);
            return;
        }
        if let Some(edit) = self.input.edit(event.keysym)
            && let Some(c) = &mut self.controller
            && !c.ui.hide_input
            && c.ui.prompt.is_some()
        {
            c.edit(edit);
            self.dirty = true;
            return;
        }
        match event.keysym {
            Keysym::Return | Keysym::KP_Enter => self.action(2),
            Keysym::Escape => self.action(3),
            Keysym::Tab | Keysym::ISO_Left_Tab => self.action(1),
            Keysym::Right | Keysym::Page_Down => self.action(4),
            Keysym::Left | Keysym::Page_Up => self.action(5),
            Keysym::BackSpace => {
                if let Some(c) = &mut self.controller {
                    c.backspace();
                }
            }
            _ => {
                if let Some(c) = &mut self.controller {
                    c.input(&text);
                }
            }
        }
        self.dirty = true;
    }
    fn repeated_key(&mut self, event: KeyEvent) {
        if event.keysym == Keysym::BackSpace
            || event
                .utf8
                .as_ref()
                .is_some_and(|s| !s.is_empty() && !s.chars().any(char::is_control))
        {
            self.key(event);
        }
    }
    fn paint(&mut self) -> Result<()> {
        let key = self
            .controller
            .as_ref()
            .and_then(|c| c.selected_uid().map(|uid| (c.generation, uid)));
        if key != self.identity_key {
            self.identity_key = key;
            self.identity_epoch = self.identities.request(key.map(|(_, uid)| uid));
        }

        if !self.dirty || !self.configured || self.window.is_none() {
            return Ok(());
        }
        self.dirty = false;

        if let Some(c) = &mut self.controller {
            c.ui.layout = self.input.label();
            c.ui.can_select_layout = self.input.can_select();
        }

        let Some(c) = &mut self.controller else {
            return Ok(());
        };
        c.refresh();
        if let Some(a) = &mut self.accessibility {
            a.update(&fl!("polkit-title"), &c.ui, &self.settings, self.focused);
        }
        let width = self.width.saturating_mul(self.scale as u32);
        let height = self.height.saturating_mul(self.scale as u32);
        let (image, hits) =
            c.ui.render_dialog(&self.settings, width, height, self.scale as f32)?;
        self.hits = hits;
        let (buffer, canvas) = self.pool.create_buffer(
            width as i32,
            height as i32,
            width as i32 * 4,
            wl_shm::Format::Xrgb8888,
        )?;
        for (source, target) in image.pixels().zip(canvas.chunks_exact_mut(4)) {
            let value = 0xff000000u32
                | u32::from(source[0]) << 16
                | u32::from(source[1]) << 8
                | u32::from(source[2]);
            target.copy_from_slice(&value.to_ne_bytes());
        }
        let surface = self.window.as_ref().unwrap().wl_surface();
        surface.set_buffer_scale(self.scale);
        buffer.attach_to(surface)?;
        surface.damage_buffer(0, 0, width as i32, height as i32);
        surface.commit();
        Ok(())
    }
}
impl WindowHandler for App {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &Window) {
        if self
            .window
            .as_ref()
            .is_some_and(|w| w.wl_surface() == window.wl_surface())
        {
            self.action(3);
        }
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &Window,
        c: WindowConfigure,
        _: u32,
    ) {
        if self
            .window
            .as_ref()
            .is_none_or(|w| w.wl_surface() != window.wl_surface())
        {
            return;
        }
        self.width = c
            .new_size
            .0
            .map(|v| v.get())
            .unwrap_or(AUTHORIZATION_DIALOG_SIZE.0);
        self.height = c
            .new_size
            .1
            .map(|v| v.get())
            .unwrap_or(AUTHORIZATION_DIALOG_SIZE.1);
        self.configured = true;
        self.dirty = true;
    }
}
impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        factor: i32,
    ) {
        if self
            .window
            .as_ref()
            .is_some_and(|w| w.wl_surface() == surface)
        {
            self.scale = factor.clamp(1, 4);
            self.dirty = true;
        }
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}
impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}
impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        c: Capability,
    ) {
        if c == Capability::Keyboard
            && !self.keyboards.iter().any(|(s, _)| s == &seat)
            && let Ok(k) = self.seats.get_keyboard_with_repeat(
                qh,
                &seat,
                None,
                self.handle.clone(),
                Box::new(|app, _, event| app.repeated_key(event)),
            )
        {
            self.keyboards.push((seat.clone(), k));
        }
        if c == Capability::Pointer
            && !self.pointers.iter().any(|(s, _)| s == &seat)
            && let Ok(p) = self.seats.get_pointer_with_theme(
                qh,
                &seat,
                self.shm.wl_shm(),
                self.compositor.create_surface(qh),
                ThemeSpec::default(),
            )
        {
            self.pointers.push((seat, p));
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            self.keyboards.retain(|(s, k)| {
                if s == &seat {
                    k.release();
                    false
                } else {
                    true
                }
            });
        }
        if capability == Capability::Pointer {
            self.pointers.retain(|(s, p)| {
                if s == &seat {
                    p.pointer().release();
                    false
                } else {
                    true
                }
            });
        }
    }
    fn remove_seat(&mut self, conn: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        self.remove_capability(conn, qh, seat.clone(), Capability::Keyboard);
        self.remove_capability(conn, qh, seat, Capability::Pointer);
    }
}

impl KeyboardHandler for App {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        self.focused = self
            .window
            .as_ref()
            .is_some_and(|w| w.wl_surface() == surface);
    }
    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.focused = false;
    }
    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.key(event);
    }
    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.repeated_key(event);
    }
    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        raw: RawModifiers,
        layout: u32,
    ) {
        self.input.modifiers(
            [raw.depressed, raw.latched, raw.locked],
            layout,
            modifiers.shift,
            modifiers.ctrl,
        );
        if let Some(c) = &mut self.controller {
            c.ui.caps = modifiers.caps_lock;
        }
        self.dirty = true;
    }
    fn update_keymap(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        keymap: smithay_client_toolkit::seat::keyboard::Keymap<'_>,
    ) {
        self.input.keymap(keymap.as_string());
        self.dirty = true;
    }
}
impl PointerHandler for App {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        _: &QueueHandle<Self>,
        pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if matches!(event.kind, PointerEventKind::Enter { .. })
                && let Some((_, p)) = self.pointers.iter().find(|(_, p)| p.pointer() == pointer)
            {
                let _ = p.set_cursor(conn, CursorIcon::Default);
            }
            if self
                .window
                .as_ref()
                .is_some_and(|w| &event.surface == w.wl_surface())
                && matches!(event.kind, PointerEventKind::Press { button: 272, .. })
            {
                let (x, y) = (
                    event.position.0 as f32 * self.scale as f32,
                    event.position.1 as f32 * self.scale as f32,
                );
                if let Some((id, _)) = self
                    .hits
                    .iter()
                    .find(|(_, r)| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height)
                {
                    self.action(*id);
                }
            }
        }
    }
}
impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState,];
}
impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
smithay_client_toolkit::delegate_compositor!(App);
smithay_client_toolkit::delegate_output!(App);
smithay_client_toolkit::delegate_seat!(App);
smithay_client_toolkit::delegate_keyboard!(App);
smithay_client_toolkit::delegate_pointer!(App);
smithay_client_toolkit::delegate_xdg_shell!(App);
smithay_client_toolkit::delegate_xdg_window!(App);
smithay_client_toolkit::delegate_shm!(App);
smithay_client_toolkit::delegate_registry!(App);
wayland_client::delegate_noop!(App:ignore wayland_client::protocol::wl_buffer::WlBuffer);
