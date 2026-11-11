//! Ordinary software-rendered wallpaper picker. Previewing never changes the daemon.
use crate::{
    ipc,
    picker::{self, Controller},
};
use anyhow::{Context, Result};
use calloop::{
    EventLoop,
    channel::{self, Event},
    timer::{TimeoutAction, Timer},
};
use calloop_wayland_source::WaylandSource;
use canopa_session_ui::{
    accessibility::{Accessibility, Operation},
    layout::Rect,
    settings::Settings,
};
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
    Preview(u64, Result<picker::Preview>),
    Applied(Result<()>),
    Accessibility(canopa_session_ui::accessibility::Request),
    Sources,
}
struct App {
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    compositor: CompositorState,
    shm: Shm,
    pool: SlotPool,
    window: Option<Window>,
    controller: Option<Controller>,
    input: canopa_session_ui::input::Keyboard,
    worker: picker::Worker,
    observer: crate::file_watch::Observer,
    accessibility: Accessibility,
    sender: channel::Sender<Update>,
    keyboards: Vec<(wl_seat::WlSeat, wl_keyboard::WlKeyboard)>,
    pointers: Vec<(wl_seat::WlSeat, ThemedPointer)>,
    width: u32,
    height: u32,
    scale: i32,
    configured: bool,
    focused: bool,
    dirty: bool,
    exit: bool,
    hits: Vec<(usize, Rect)>,
}
pub fn run(mut settings: Settings, images: Vec<PathBuf>) -> Result<()> {
    let snapshot = ipc::send_with_timeout(&ipc::Request::Status {}, Duration::from_secs(10))
        .context("reading wallpaper daemon status")?;
    if let Some([r, g, b]) = snapshot.color {
        settings.color = canopa_theme::Rgb(r, g, b);
    }
    let controller = Controller::new(settings, images, snapshot.outputs)?;
    let connection = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<App>(&connection)?;
    let qh = queue.handle();
    let mut loop_ = EventLoop::<App>::try_new()?;
    let compositor = CompositorState::bind(&globals, &qh)?;
    let shell = XdgShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let pool = SlotPool::new(4096, &shm)?;
    let window = shell.create_window(
        compositor.create_surface(&qh),
        WindowDecorations::RequestServer,
        &qh,
    );
    window.set_app_id("org.canopa.WallpaperPicker");
    window.set_title(fl!("wallpaper-title"));
    window.set_min_size(Some((800, 800)));
    window.commit();
    let (sender, receiver) = channel::channel();
    let preview_sender = sender.clone();
    let worker = picker::Worker::start(move |generation, preview| {
        let _ = preview_sender.send(Update::Preview(generation, preview));
    });
    let source_sender = sender.clone();
    let observer = crate::file_watch::Observer::start(controller.watch_targets(), move || {
        let _ = source_sender.send(Update::Sources);
    });
    let action_sender = sender.clone();
    let accessibility = Accessibility::new(&fl!("wallpaper-title"), move |request| {
        let _ = action_sender.send(Update::Accessibility(request));
    });
    let mut app = App {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        compositor,
        shm,
        pool,
        window: Some(window),
        controller: Some(controller),
        input: Default::default(),
        worker,
        observer,
        accessibility,
        sender,
        keyboards: Vec::new(),
        pointers: Vec::new(),
        width: 800,
        height: 800,
        scale: 1,
        configured: false,
        focused: false,
        dirty: true,
        exit: false,
        hits: Vec::new(),
    };
    loop_
        .handle()
        .insert_source(receiver, |event, _, app| {
            if let Event::Msg(event) = event {
                match event {
                    Update::Preview(generation, preview) => app
                        .controller
                        .as_mut()
                        .unwrap()
                        .previewed(generation, preview),
                    Update::Applied(result) => app.controller.as_mut().unwrap().applied(result),
                    Update::Sources => {
                        if !app.controller.as_ref().unwrap().applying {
                            let _ = app.controller.as_mut().unwrap().rescan();
                            app.request_preview();
                        }
                    }
                    Update::Accessibility(request) => {
                        if app.focused && app.accessibility.accepts(&request) {
                            match request.operation {
                                Operation::Control(id) => app.action(id),
                                Operation::Submit => app.action(0),
                                Operation::Cancel => app.action(5),
                                _ => {}
                            }
                        }
                    }
                }
                app.dirty = true;
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    loop_
        .handle()
        .insert_source(
            Timer::from_duration(Duration::from_secs(10)),
            |_, _, app| {
                if !app.configured {
                    app.exit = true;
                }
                TimeoutAction::Drop
            },
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    WaylandSource::new(connection.clone(), queue)
        .insert(loop_.handle())
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    app.request_preview();
    while !app.exit {
        loop_.dispatch(None, &mut app)?;
        if !app.exit {
            app.paint()?;
        }
        connection.flush()?;
    }
    Ok(())
}
impl App {
    fn request_preview(&mut self) {
        let c = self.controller.as_mut().unwrap();
        let spec = c.request_preview();
        self.observer.set(c.watch_targets());
        self.worker.request(spec);
        self.dirty = true;
    }
    fn outputs_changed(&mut self) {
        if let Ok(snapshot) =
            ipc::send_with_timeout(&ipc::Request::Status {}, Duration::from_millis(300))
        {
            if let Some([r, g, b]) = snapshot.color {
                self.controller.as_mut().unwrap().preview_color = canopa_theme::Rgb(r, g, b);
            }
            self.controller
                .as_mut()
                .unwrap()
                .set_outputs(snapshot.outputs);
            self.request_preview();
        }
    }
    fn action(&mut self, id: usize) {
        match self.controller.as_mut().unwrap().action(id) {
            picker::Action::None => {}
            picker::Action::Close => self.exit = true,
            picker::Action::Preview => self.request_preview(),
            picker::Action::Apply(request) => {
                let sender = self.sender.clone();
                std::thread::spawn(move || {
                    let result = ipc::send(&request).map(|_| ());
                    let _ = sender.send(Update::Applied(result));
                });
            }
        }
        self.dirty = true;
    }
    fn key(&mut self, mut event: KeyEvent) {
        let text = zeroize::Zeroizing::new(event.utf8.take().unwrap_or_default());
        if !self.focused {
            return;
        }
        match event.keysym {
            Keysym::Escape => self.action(5),
            Keysym::Return | Keysym::KP_Enter => {
                self.action(self.controller.as_ref().unwrap().focused)
            }
            Keysym::Left | Keysym::Page_Up => self.action(1),
            Keysym::Right | Keysym::Page_Down => self.action(2),
            Keysym::Up | Keysym::Down => self.action(3),
            Keysym::f | Keysym::F => self.action(4),
            Keysym::Tab | Keysym::ISO_Left_Tab => self.controller.as_mut().unwrap().cycle(),
            Keysym::F5 => {
                let _ = self.controller.as_mut().unwrap().rescan();
                self.outputs_changed();
            }
            _ => {}
        }
        drop(text);
        self.dirty = true;
    }
    fn paint(&mut self) -> Result<()> {
        if !self.dirty || !self.configured {
            return Ok(());
        }
        self.dirty = false;
        let c = self.controller.as_mut().unwrap();
        c.refresh();
        self.accessibility
            .update(&fl!("wallpaper-title"), &c.ui, &c.settings, self.focused);
        let width = self.width.saturating_mul(self.scale as u32);
        let height = self.height.saturating_mul(self.scale as u32);
        let (image, hits) = c.render(width, height)?;
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
            self.action(5);
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
        self.width = c.new_size.0.map(|v| v.get()).unwrap_or(800);
        self.height = c.new_size.1.map(|v| v.get()).unwrap_or(800);
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
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.outputs_changed();
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.outputs_changed();
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.outputs_changed();
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
            // Do not let the toolkit retain cloned cleartext key events for repeat.
            && let Ok(k) = self.seats.get_keyboard(qh, &seat, None)
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
        self.dirty = true;
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
        self.dirty = true;
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
        mut event: KeyEvent,
    ) {
        // This client uses a keyboard without a repeat source.
        let _text = zeroize::Zeroizing::new(event.utf8.take().unwrap_or_default());
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
