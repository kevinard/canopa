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
        pointer::{PointerEvent, PointerHandler, ThemeSpec, ThemedPointer},
    },
    shell::{
        WaylandSurface,
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
        },
    },
    shm::{Shm, ShmHandler},
};
use std::{os::fd::AsFd, time::Duration};
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
};

struct App {
    accessibility: Option<canopa_session_ui::accessibility::Accessibility>,
    input: canopa_session_ui::input::Keyboard,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    compositor: CompositorState,
    shm: Shm,
    graphics: canopa_session_ui::presentation::Graphics,
    window: Option<Window>,
    configured: bool,
    focused: bool,
    keyboards: Vec<(wl_seat::WlSeat, wl_keyboard::WlKeyboard)>,
    pointers: Vec<(wl_seat::WlSeat, ThemedPointer)>,
    cursor_cache: canopa_session_ui::presentation::CursorCache,
    controller: Option<Controller>,
    settings: Settings,
    width: u32,
    height: u32,
    /// The logical height last asked of the compositor; it only grows.
    requested: u32,
    scale: i32,
    dirty: bool,
    exit: bool,
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
    let graphics = canopa_session_ui::presentation::Graphics::new(
        &connection,
        &qh,
        &loop_.handle(),
        |app: &mut App| app.repaint(),
    )?;
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
        graphics,
        window: Some(window),
        configured: false,
        focused: false,
        keyboards: Vec::new(),
        pointers: Vec::new(),
        cursor_cache: Default::default(),
        controller: Some(controller),
        settings: settings.clone(),
        width: DIALOG_SIZE.0,
        height,
        requested: height,
        scale: 1,
        dirty: true,
        exit: false,
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
                app.redraw();
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
    /// Renderer wakeups (animations, timed redraws, retries) repaint the
    /// retained widgets without invalidating the presentation model.
    fn repaint(&mut self) {
        self.dirty = true;
    }
    fn redraw(&mut self) {
        if let Some(c) = &self.controller {
            c.ui.changed();
        }
        self.dirty = true;
    }

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
        self.redraw();
    }

    fn action(&mut self, id: usize) {
        if id == canopa_session_ui::input::LAYOUT_CONTROL {
            self.input.cycle();
            self.redraw();
            return;
        }
        if id == canopa_session_ui::input::REVEAL_CONTROL {
            if let Some(c) = &mut self.controller {
                c.ui.toggle_reveal();
            }
            self.redraw();
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
        self.redraw();
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
            self.redraw();
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
            self.redraw();
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
        self.redraw();
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
        let window = self.window.as_ref().unwrap();
        window.wl_surface().set_buffer_scale(self.scale);
        if let Err(error) = c.ui.present(
            &self.settings,
            &self.graphics,
            self.graphics.window(window),
            canopa_session_ui::presentation::Frame {
                key: "",
                output: "",
                width,
                height,
                scale: self.scale as f32,
                background: None,
            },
        ) {
            eprintln!("session presentation deferred: {error}");
            self.graphics.retry();
            self.dirty = true;
        }
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
        self.redraw();
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
            self.redraw();
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
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        if let Some(c) = &self.controller {
            self.dirty |= c.ui.frame_ready("");
        }
    }
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
            self.redraw();
            if let Some(c) = &self.controller {
                c.ui.cancel_interactions();
            }
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
        self.redraw();
    }
    fn update_keymap(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        keymap: smithay_client_toolkit::seat::keyboard::Keymap<'_>,
    ) {
        self.input.keymap(keymap.as_string());
        self.redraw();
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
        use canopa_session_ui::presentation::native_pointer;
        for event in events {
            if !(self
                .window
                .as_ref()
                .is_some_and(|w| &event.surface == w.wl_surface()))
            {
                continue;
            }
            for input in native_pointer(event, self.scale as f32) {
                let interaction = {
                    let Some(c) = &mut self.controller else {
                        break;
                    };
                    c.ui.pointer(&self.settings, "", input)
                };
                if let Some((_, p)) = self.pointers.iter().find(|(_, p)| p.pointer() == pointer) {
                    self.cursor_cache.set(conn, p, event, interaction.cursor);
                }
                if let Some((anchor, cursor)) = interaction.selection
                    && let Some(c) = &mut self.controller
                {
                    c.select_input(anchor, cursor);
                }
                for id in interaction.controls {
                    self.action(id);
                }
                self.dirty |= interaction.redraw || interaction.selection.is_some();
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
