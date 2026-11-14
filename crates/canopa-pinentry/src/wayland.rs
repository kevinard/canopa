//! Ordinary xdg-shell dialogs. The compositor retains focus and session-lock policy.
use crate::{
    controller::Controller,
    protocol::{Request, Response},
};
use anyhow::Result;
use calloop::{
    EventLoop, Interest, Mode, PostAction,
    channel::{self, Event},
    generic::Generic,
    timer::{TimeoutAction, Timer},
};
use calloop_wayland_source::WaylandSource;
use canopa_session_ui::{
    layout::Rect,
    settings::Settings,
    view::{DIALOG_MAX_HEIGHT, DIALOG_MIN_HEIGHT, DIALOG_SIZE},
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
use std::{os::fd::AsFd, time::Duration};
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

struct App {
    accessibility: Option<canopa_session_ui::accessibility::Accessibility>,
    input: canopa_session_ui::input::Keyboard,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    compositor: CompositorState,
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
    /// The logical height last asked of the compositor; it only grows.
    requested: u32,
    scale: i32,
    dirty: bool,
    exit: bool,
    hits: Vec<(usize, Rect)>,
}
pub fn run_agent(
    settings: &Settings,
    request: &Request,
    agent: &mut dyn crate::agent::Agent,
) -> Result<Response> {
    std::thread::scope(|scope| {
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let (answers, results) = channel::channel();
        let stopped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = stopped.clone();
        scope.spawn(move || {
            while let Ok(query) = receive.recv() {
                if flag.load(std::sync::atomic::Ordering::Acquire) {
                    break;
                }
                if answers.send(agent.inquire(query)).is_err() {
                    break;
                }
            }
        });
        let result = run_inner(settings, request, send, results, false);
        stopped.store(true, std::sync::atomic::Ordering::Release);
        result
    })
}
pub fn run_keyring(settings: &Settings, request: &Request) -> Result<Response> {
    // A keyring dialog has no GnuPG agent inquiries.
    let (inquiries, _requests) = std::sync::mpsc::sync_channel(1);
    let (_answers, results) = channel::channel();
    run_inner(settings, request, inquiries, results, true)
}

fn run_inner(
    settings: &Settings,
    request: &Request,
    inquiries: std::sync::mpsc::SyncSender<crate::agent::Query>,
    answers: channel::Channel<(u64, crate::agent::Answer)>,
    keyring: bool,
) -> Result<Response> {
    let connection = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<App>(&connection)?;
    let qh = queue.handle();
    let mut loop_ = EventLoop::<App>::try_new()?;
    let compositor = CompositorState::bind(&globals, &qh)?;
    let shell = XdgShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let pool = SlotPool::new(4096, &shm)?;
    let (app_id, name) = if keyring {
        ("org.canopa.Keyring", fl!("pinentry-keyring-title"))
    } else {
        ("org.canopa.Pinentry", fl!("pinentry-title"))
    };
    let heading = if keyring {
        name.clone()
    } else {
        fl!("pinentry-secure-input")
    };
    let mut request = request.clone();
    request.requester = request.owner.and_then(requester);
    let controller = Controller::new_named(settings, request.clone(), heading)?;
    let window = shell.create_window(
        compositor.create_surface(&qh),
        WindowDecorations::RequestServer,
        &qh,
    );
    window.set_app_id(app_id);
    let title: String = request
        .title
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect();
    window.set_title(if title.is_empty() {
        name.clone()
    } else {
        fl!(
            "pinentry-window-title",
            name = name.as_str(),
            title = title.as_str()
        )
    });
    // The window fits the request; it grows if a later step needs more room.
    let height = controller
        .ui
        .dialog_height(DIALOG_MIN_HEIGHT, DIALOG_MAX_HEIGHT);
    window.set_min_size(Some((DIALOG_SIZE.0, height)));
    window.set_max_size(Some((DIALOG_SIZE.0, height)));
    window.commit();
    let mut app = App {
        accessibility: None,
        input: Default::default(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        compositor,
        shm,
        pool,
        window: Some(window),
        configured: false,
        focused: false,
        keyboards: Vec::new(),
        pointers: Vec::new(),
        controller: Some(controller),
        settings: settings.clone(),
        width: DIALOG_SIZE.0,
        height,
        requested: height,
        scale: 1,
        dirty: true,
        exit: false,
        hits: Vec::new(),
    };
    let (a11y_sender, a11y_receiver) = channel::channel();
    app.accessibility = Some(canopa_session_ui::accessibility::Accessibility::new(
        &name,
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

    loop_
        .handle()
        .insert_source(answers, |event, _, app: &mut App| {
            if let Event::Msg((revision, answer)) = event
                && let Some(c) = &mut app.controller
            {
                c.inquiry_result(revision, answer);
                app.exit = c.response.is_some();
                app.dirty = true;
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    if request.timeout > 0 {
        loop_
            .handle()
            .insert_source(
                Timer::from_duration(Duration::from_secs(request.timeout)),
                |_, _, app| {
                    app.controller.as_mut().unwrap().finish(Response::Timeout);
                    app.exit = true;
                    TimeoutAction::Drop
                },
            )
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    loop_
        .handle()
        .insert_source(
            Timer::from_duration(Duration::from_secs(10)),
            |_, _, app| {
                if !app.configured {
                    app.controller
                        .as_mut()
                        .unwrap()
                        .finish(Response::Unavailable);
                    app.exit = true;
                }
                TimeoutAction::Drop
            },
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    // Hangup/error notifications are delivered independently of read interest.
    // Leave queued protocol commands untouched, and never spin on their bytes.
    let input = std::io::stdin().as_fd().try_clone_to_owned()?;
    loop_
        .handle()
        .insert_source(
            Generic::new(input, Interest::EMPTY, Mode::Level),
            |_, _, app| {
                app.controller.as_mut().unwrap().finish(Response::Canceled);
                app.exit = true;
                Ok(PostAction::Remove)
            },
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let wayland_source = WaylandSource::new(connection.clone(), queue)
        .insert(loop_.handle())
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let result = (|| -> Result<Response> {
        let mut revision = 0;
        let mut quality_at: Option<std::time::Instant> = None;
        while !app.exit {
            let wait = quality_at.map(|at| at.saturating_duration_since(std::time::Instant::now()));
            loop_.dispatch(wait, &mut app)?;
            let c = app.controller.as_mut().unwrap();
            if revision != c.revision {
                revision = c.revision;
                quality_at = c
                    .request
                    .quality
                    .as_ref()
                    .map(|_| std::time::Instant::now() + Duration::from_millis(150));
            }
            if quality_at.is_some_and(|at| at <= std::time::Instant::now()) {
                quality_at = None;
                c.quality_query();
            }
            if let Some(query) = c.pending.take() {
                match inquiries.try_send(query) {
                    Ok(()) => {}
                    Err(std::sync::mpsc::TrySendError::Full(query)) => c.pending = Some(query),
                    Err(_) => {
                        c.finish(Response::Unavailable);
                        app.exit = true;
                    }
                }
            }
            if !app.exit {
                app.paint()?;
            }
            connection.flush()?;
        }
        Ok(app
            .controller
            .as_mut()
            .unwrap()
            .response
            .take()
            .unwrap_or(Response::Canceled))
    })();
    // Retire the queue and window before acknowledging completion to the peer.
    // Flush destruction while a connection handle still exists.
    loop_.handle().remove(wayland_source);
    drop(app);
    let flushed = connection.flush();
    drop(inquiries);
    let response = result?;
    flushed?;
    Ok(response)
}
impl App {
    fn accessible(&mut self, request: canopa_session_ui::accessibility::Request) {
        if let (Some(a), Some(c)) = (&mut self.accessibility, &self.controller) {
            a.update(&fl!("pinentry-title"), &c.ui, &self.settings, self.focused);
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
            Operation::Submit => self.action(0),
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

    fn action(&mut self, id: usize) {
        if id == canopa_session_ui::input::LAYOUT_CONTROL {
            self.input.cycle();
            self.dirty = true;
            return;
        }
        if id == canopa_session_ui::input::REVEAL_CONTROL {
            if let Some(c) = &mut self.controller {
                c.ui.toggle_reveal();
            }
            self.dirty = true;
            return;
        }
        let c = self.controller.as_mut().unwrap();
        if id == 1 {
            c.cycle();
        } else if id == 0 {
            c.activate();
        } else {
            c.action(id);
        }
        self.exit = c.response.is_some();
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
            Keysym::Return | Keysym::KP_Enter => self.action(0),
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
    fn paint(&mut self) -> Result<()> {
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
            a.update(&fl!("pinentry-title"), &c.ui, &self.settings, self.focused);
        }
        let desired = c.ui.dialog_height(DIALOG_MIN_HEIGHT, DIALOG_MAX_HEIGHT);
        if desired > self.requested
            && let Some(window) = &self.window
        {
            // Grow with this frame: compositors need not configure a mapped
            // floating window again after its size limits change.
            self.requested = desired;
            self.height = self.height.max(desired);
            window.set_min_size(Some((DIALOG_SIZE.0, desired)));
            window.set_max_size(Some((DIALOG_SIZE.0, desired)));
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
        self.width = c.new_size.0.map(|v| v.get()).unwrap_or(DIALOG_SIZE.0);
        // Never below the content height asked for: that would clip the footer.
        self.height = c
            .new_size
            .1
            .map_or(self.requested, |v| v.get())
            .max(self.requested);
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

/// The program that asked gpg-agent: the client's name, or for GnuPG's own
/// tools the program that ran them (`git` rather than `gpg`).
fn requester(pid: u32) -> Option<String> {
    use canopa_session_ui::process::{parent, program_name};
    let name = program_name(pid)?;
    if matches!(
        name.as_str(),
        "gpg" | "gpg2" | "gpgsm" | "gpg-connect-agent"
    ) && let Some(caller) = parent(pid).and_then(program_name)
        && !matches!(caller.as_str(), "sh" | "bash" | "zsh" | "fish" | "dash")
    {
        return Some(caller);
    }
    Some(name)
}
