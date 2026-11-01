//! Fullscreen xdg-shell client. The kiosk compositor, not this UI, owns DRM/VTs.
use crate::{
    accounts, avatars,
    config::Config,
    controller::{Controller, Phase},
    protocol::{self, Command},
};
use anyhow::{Result, bail};
use calloop::{
    EventLoop, LoopHandle,
    channel::{self, Event},
    timer::{TimeoutAction, Timer},
};
use calloop_wayland_source::WaylandSource;
use canopa_session_ui::{background, layout::Rect, widgets};
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
enum Update {
    Accounts(Vec<accounts::Account>),
    Avatar(u64, Option<std::sync::Arc<image::RgbaImage>>),
    Auth(protocol::Event),
    Widget(widgets::Update),
}
struct App {
    handle: LoopHandle<'static, App>,
    compositor: CompositorState,
    layout_names: Vec<String>,
    registry: RegistryState,
    seats: SeatState,
    outputs: OutputState,
    shm: Shm,
    pool: SlotPool,
    window: Window,
    qh: QueueHandle<Self>,
    controller: Controller,
    config: Config,
    worker: protocol::Worker,
    _widgets: Option<widgets::Runtime>,
    _accounts: Option<accounts::Worker>,
    avatars: Option<avatars::Worker>,
    avatar_key: Option<(String, Vec<PathBuf>)>,
    avatar_generation: u64,
    keyboards: Vec<(wl_seat::WlSeat, wl_keyboard::WlKeyboard)>,
    pointers: Vec<(wl_seat::WlSeat, ThemedPointer)>,
    width: u32,
    height: u32,
    scale: i32,
    configured: bool,
    dirty: bool,
    exit: bool,
    error: bool,
    output_name: String,
    background: Option<image::RgbImage>,
    hits: Vec<(usize, Rect)>,
    ready_socket: Option<PathBuf>,
    frame_requested: bool,
}
pub fn run(config: Config, socket: PathBuf, ready_socket: Option<PathBuf>) -> Result<()> {
    let conn = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<App>(&conn)?;
    let qh = queue.handle();
    let mut event_loop: EventLoop<App> = EventLoop::try_new()?;
    let compositor = CompositorState::bind(&globals, &qh)?;
    let shell = XdgShell::bind(&globals, &qh)?;
    let window = shell.create_window(
        compositor.create_surface(&qh),
        WindowDecorations::RequestServer,
        &qh,
    );
    window.set_app_id("org.canopa.Greeter");
    window.set_title("Canopa Login");
    window.set_fullscreen(None);
    window.commit();
    let (sender, receiver) = channel::channel();
    let auth_sender = sender.clone();
    let worker = protocol::Worker::start(socket, config.view.auth_timeout, move |event| {
        let _ = auth_sender.send(Update::Auth(event));
    });
    let account_sender = sender.clone();
    let account_worker = config.discover_users.then(|| {
        accounts::Worker::start(
            config.user_source,
            config.nss.clone(),
            config.avatars_enabled(),
            move |users| {
                let _ = account_sender.send(Update::Accounts(users));
            },
        )
    });
    let avatar_sender = sender.clone();
    let avatars = config.avatars_enabled().then(|| {
        avatars::Worker::start(move |generation, image| {
            let _ = avatar_sender.send(Update::Avatar(generation, image));
        })
    });
    let observers = widgets::start(&config.view, move |event| {
        sender.send(Update::Widget(event)).is_ok()
    });
    let controller = Controller::new(&config)?;
    let shm = Shm::bind(&globals, &qh)?;
    let pool = SlotPool::new(4096, &shm)?;
    let mut app = App {
        handle: event_loop.handle(),
        compositor,
        layout_names: Vec::new(),
        registry: RegistryState::new(&globals),
        seats: SeatState::new(&globals, &qh),
        outputs: OutputState::new(&globals, &qh),
        shm,
        pool,
        window,
        qh: qh.clone(),
        controller,
        config,
        worker,
        _widgets: observers,
        _accounts: account_worker,
        avatars,
        avatar_key: None,
        avatar_generation: 0,
        keyboards: Vec::new(),
        pointers: Vec::new(),
        width: 1280,
        height: 800,
        scale: 1,
        configured: false,
        dirty: true,
        exit: false,
        error: false,
        output_name: String::new(),
        background: None,
        hits: Vec::new(),
        ready_socket,
        frame_requested: false,
    };
    event_loop
        .handle()
        .insert_source(receiver, |event, _, app| {
            if let Event::Msg(event) = event {
                match event {
                    Update::Accounts(users) => app.controller.apply_accounts(&app.config, users),
                    Update::Avatar(generation, image) => {
                        if generation != app.avatar_generation {
                            return;
                        }
                        app.controller.ui.set_user_avatar(image);
                    }
                    Update::Auth(event) => {
                        let command = app.controller.event(event, &app.config);
                        app.send(command);
                        if app.controller.phase == Phase::Done {
                            app.exit = true;
                        }
                        if app.controller.phase == Phase::Fatal {
                            app.error = true;
                            app.exit = true;
                        }
                    }
                    Update::Widget(widgets::Update::Battery(v)) => {
                        if app.controller.ui.battery.as_ref() == Some(&v) {
                            return;
                        }
                        app.controller.ui.battery = Some(v)
                    }
                    Update::Widget(widgets::Update::Media(_)) => {}
                    Update::Widget(widgets::Update::Custom { name, text }) => {
                        if app.controller.ui.custom.get(&name) == text.as_ref() {
                            return;
                        }
                        if let Some(text) = text {
                            app.controller.ui.custom.insert(name, text);
                        } else {
                            app.controller.ui.custom.remove(&name);
                        }
                    }
                }
                app.dirty = true;
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    event_loop
        .handle()
        .insert_source(Timer::from_duration(Duration::from_secs(1)), |_, _, app| {
            app.dirty = true;
            TimeoutAction::ToDuration(Duration::from_secs(
                if app.config.view.clock_format == "%H:%M"
                    && app.config.view.date_format == "%A, %B %-d"
                {
                    60 - chrono::Local::now().timestamp().rem_euclid(60) as u64
                } else {
                    1
                },
            ))
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    WaylandSource::new(conn.clone(), queue)
        .insert(event_loop.handle())
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    while !app.exit {
        event_loop.dispatch(None, &mut app)?;
        if !app.exit {
            app.paint()?;
        }
        conn.flush()?;
    }
    if app.error || app.controller.phase != Phase::Done {
        bail!("greeter ended without an authenticated session handoff");
    }
    Ok(())
}
impl App {
    fn send(&mut self, command: Option<Command>) {
        if let Some(command) = command
            && self.worker.send(command).is_err()
        {
            self.error = true;
            self.exit = true;
        }
    }
    fn action(&mut self, id: usize) {
        let command = match id {
            1 => {
                if self.controller.phase == Phase::User {
                    self.controller.user_next(false);
                    None
                } else {
                    self.controller.cancel()
                }
            }
            2 => self.controller.session_next(&self.config),
            3 => self.controller.submit(),
            _ => None,
        };
        self.send(command);
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
    fn key(&mut self, mut event: KeyEvent) {
        let text = zeroize::Zeroizing::new(event.utf8.take().unwrap_or_default());
        if text.as_str() == "\u{15}" {
            self.controller.clear_input();
            self.dirty = true;
            return;
        }
        let command = match event.keysym {
            Keysym::Escape => self.controller.escape(),
            Keysym::Return | Keysym::KP_Enter => self.controller.submit(),
            Keysym::Tab | Keysym::ISO_Left_Tab => self.controller.session_next(&self.config),
            Keysym::Up => {
                self.controller.user_next(true);
                None
            }
            Keysym::Down => {
                self.controller.user_next(false);
                None
            }
            Keysym::BackSpace => {
                self.controller.backspace();
                None
            }
            _ => {
                self.controller.input(&text);
                None
            }
        };
        self.send(command);
        self.dirty = true;
    }
    fn paint(&mut self) -> Result<()> {
        if !self.dirty || !self.configured {
            return Ok(());
        }
        self.dirty = false;
        self.controller.refresh(&self.config);
        if let Some(worker) = &self.avatars {
            let key = self.controller.avatar_request(&self.config);
            if self.avatar_key.as_ref() != Some(&key) {
                self.controller.ui.set_user_avatar(None);
                self.avatar_generation = worker.request(key.1.clone());
                self.avatar_key = Some(key);
            }
        }
        let width = self.width.saturating_mul(self.scale as u32);
        let height = self.height.saturating_mul(self.scale as u32);
        background::dimensions(width, height)?;
        if self
            .background
            .as_ref()
            .is_none_or(|b| b.dimensions() != (width, height))
        {
            self.background = Some(
                canopa_session_ui::view::SessionView::prepare_background(
                    &self.config.view,
                    &self.output_name,
                    width,
                    height,
                )
                .unwrap_or_else(|_| {
                    image::RgbImage::from_pixel(
                        width,
                        height,
                        image::Rgb([
                            self.config.view.color.0,
                            self.config.view.color.1,
                            self.config.view.color.2,
                        ]),
                    )
                }),
            );
        }
        let (image, hits) = self
            .controller
            .ui
            .render_with_hits(&self.config.view, self.background.as_ref().unwrap().clone())?;
        self.hits = hits;
        let (buffer, canvas) = self.pool.create_buffer(
            width as i32,
            height as i32,
            width as i32 * 4,
            wl_shm::Format::Xrgb8888,
        )?;
        for (source, target) in image.pixels().zip(canvas.chunks_exact_mut(4)) {
            let pixel = 0xff000000u32
                | u32::from(source[0]) << 16
                | u32::from(source[1]) << 8
                | u32::from(source[2]);
            target.copy_from_slice(&pixel.to_ne_bytes());
        }
        let surface = self.window.wl_surface();
        surface.set_buffer_scale(self.scale);
        buffer.attach_to(surface)?;
        surface.damage_buffer(0, 0, width as i32, height as i32);
        if self.ready_socket.is_some() && !self.frame_requested {
            surface.frame(&self.qh, surface.clone());
            self.frame_requested = true;
        }
        surface.commit();
        // SlotPool retains active storage/objects until wl_buffer.release.
        Ok(())
    }
}
impl WindowHandler for App {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.exit = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &Window,
        c: WindowConfigure,
        _: u32,
    ) {
        self.width = c.new_size.0.map(|v| v.get()).unwrap_or(self.width);
        self.height = c.new_size.1.map(|v| v.get()).unwrap_or(self.height);
        self.configured = true;
        self.dirty = true;
    }
}
impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        factor: i32,
    ) {
        self.scale = factor.max(1);
        self.dirty = true;
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        if let Some(path) = self.ready_socket.take() {
            // Explicit test-only destination. No authentication data is included.
            let result = (|| -> std::io::Result<()> {
                let socket = std::os::unix::net::UnixDatagram::unbound()?;
                let message = serde_json::json!({"pid":std::process::id(),
                    "width":self.width * self.scale as u32,"height":self.height * self.scale as u32});
                socket.send_to(message.to_string().as_bytes(), path)?;
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("greeter readiness notification failed: {error}");
            }
        }
    }
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        if let Some(info) = self.outputs.info(output) {
            self.output_name = info.name.unwrap_or_default();
            self.background = None;
            self.dirty = true;
        }
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
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.background = None;
        self.dirty = true;
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.background = None;
        self.dirty = true;
    }
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
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }
    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
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
        _: RawModifiers,
        layout: u32,
    ) {
        self.controller.ui.caps = modifiers.caps_lock;
        self.controller.ui.layout = self
            .layout_names
            .get(layout as usize)
            .cloned()
            .unwrap_or_else(|| format!("Layout {}", layout + 1));
        self.dirty = true;
    }
    fn update_keymap(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        keymap: smithay_client_toolkit::seat::keyboard::Keymap<'_>,
    ) {
        use xkbcommon::xkb;
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        if let Some(map) = xkb::Keymap::new_from_string(
            &context,
            keymap.as_string(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) {
            self.layout_names = (0..map.num_layouts())
                .map(|i| map.layout_get_name(i).to_owned())
                .collect();
        }
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
            if &event.surface == self.window.wl_surface()
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
