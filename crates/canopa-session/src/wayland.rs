use crate::{
    auth, background, ipc,
    lock_ui::LockUi,
    settings::{Background, Settings},
};
use anyhow::{Context, Result, ensure};
use calloop::{
    EventLoop, LoopHandle,
    channel::{self, Event},
    timer::{TimeoutAction, Timer},
};
use calloop_wayland_source::WaylandSource;
use canopa_config::{Component, Request};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
    },
    session_lock::{
        SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface,
        SessionLockSurfaceConfigure,
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, raw::RawPool},
};
use std::{
    io::Write,
    os::fd::{FromRawFd, OwnedFd},
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_buffer, wl_keyboard, wl_output, wl_seat, wl_shm, wl_surface},
};

pub fn check_config(request: Request, component: Component, show_source: bool) -> Result<()> {
    let result = Settings::load(&request, component);
    match result {
        Ok((_, document)) => {
            println!(
                "{}",
                serde_json::json!({"component":component,"source":document.info()})
            );
            Ok(())
        }
        Err(error) => {
            if show_source {
                eprintln!("{error:#}");
            }
            let diagnostic = if let Some(d) = error.downcast_ref::<canopa_config::Diagnostic>() {
                serde_json::to_value(d)?
            } else {
                serde_json::json!({"code":"session-schema","message":"invalid session configuration"})
            };
            println!(
                "{}",
                serde_json::json!({"component":component,"error":diagnostic})
            );
            anyhow::bail!("configuration check failed")
        }
    }
}
fn validate_images(settings: &Settings) -> Result<()> {
    for bg in std::iter::once(&settings.background).chain(settings.outputs.values()) {
        if let Some(path) = &bg.image {
            background::load(path)?;
        }
    }
    Ok(())
}
enum Role {
    Wallpaper(LayerSurface),
    Lock(SessionLockSurface),
}
impl Role {
    fn surface(&self) -> &wl_surface::WlSurface {
        match self {
            Self::Wallpaper(s) => s.wl_surface(),
            Self::Lock(s) => s.wl_surface(),
        }
    }
}
struct DisplaySurface {
    output: wl_output::WlOutput,
    role: Role,
    width: u32,
    height: u32,
    scale: i32,
    dirty: bool,
    background: Option<image::RgbImage>,
}
enum Update {
    Authentication { generation: u64, event: auth::Event },
    Battery(canopa_widgets::battery::BatteryState),
    Media(Option<String>),
    Custom { name: String, text: Option<String> },
}
struct App {
    conn: Connection,
    qh: QueueHandle<Self>,
    handle: LoopHandle<'static, Self>,
    registry: RegistryState,
    compositor: CompositorState,
    outputs: OutputState,
    seats: SeatState,
    shm: Shm,
    layer: Option<LayerShell>,
    session: Option<SessionLock>,
    surfaces: Vec<DisplaySurface>,
    keyboards: Vec<(wl_seat::WlSeat, wl_keyboard::WlKeyboard)>,
    layouts: Vec<(wl_keyboard::WlKeyboard, Vec<String>)>,
    settings: Settings,
    request: Request,
    ui: Option<LockUi>,
    confirmed: bool,
    exit: bool,
    error: Option<String>,
    ready: Option<OwnedFd>,
    updates: channel::Sender<Update>,
    authentication: Option<auth::Worker>,
    auth_generation: u64,
    auth_timeout: Option<calloop::RegistrationToken>,
    retry_after: Instant,
    logind: Option<crate::logind::Tracker>,
    widgets: Option<WidgetRuntime>,
}
fn initialize(
    settings: Settings,
    request: Request,
    locker: bool,
    ui: Option<LockUi>,
    ready: Option<OwnedFd>,
) -> Result<(App, EventLoop<'static, App>)> {
    let conn = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init(&conn)?;
    let qh = queue.handle();
    let event_loop = EventLoop::try_new()?;
    let (updates, receiver) = channel::channel();
    event_loop
        .handle()
        .insert_source(receiver, |event, _, app: &mut App| {
            if let Event::Msg(update) = event {
                match update {
                    Update::Authentication { generation, event } => {
                        app.authentication_event(generation, event);
                    }
                    Update::Battery(battery) => {
                        if let Some(ui) = &mut app.ui {
                            if ui.battery.as_ref() == Some(&battery) {
                                return;
                            }
                            ui.battery = Some(battery);
                        }
                    }
                    Update::Media(label) => {
                        if let Some(ui) = &mut app.ui {
                            if ui.media == label {
                                return;
                            }
                            ui.media = label;
                        }
                    }
                    Update::Custom { name, text } => {
                        if let Some(ui) = &mut app.ui {
                            if ui.custom.get(&name) == text.as_ref() {
                                return;
                            }
                            if let Some(text) = text {
                                ui.custom.insert(name, text);
                            } else {
                                ui.custom.remove(&name);
                            }
                        }
                    }
                }
                app.redraw();
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let layer = if locker {
        None
    } else {
        Some(LayerShell::bind(&globals, &qh)?)
    };
    let session = if locker {
        Some(
            SessionLockState::new(&globals, &qh)
                .lock(&qh)
                .context("compositor does not support ext-session-lock-v1")?,
        )
    } else {
        None
    };
    let mut app = App {
        compositor: CompositorState::bind(&globals, &qh)?,
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        registry: RegistryState::new(&globals),
        shm: Shm::bind(&globals, &qh)?,
        conn: conn.clone(),
        qh: qh.clone(),
        handle: event_loop.handle(),
        layer,
        session,
        surfaces: vec![],
        keyboards: vec![],
        layouts: vec![],
        settings,
        request,
        ui,
        confirmed: false,
        exit: false,
        error: None,
        ready,
        updates,
        authentication: None,
        auth_generation: 0,
        auth_timeout: None,
        retry_after: Instant::now(),
        logind: None,
        widgets: None,
    };
    for output in app.outputs.outputs() {
        app.add_output(output);
    }
    WaylandSource::new(conn, queue)
        .insert(event_loop.handle())
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok((app, event_loop))
}
pub fn wallpaper(request: Request) -> Result<()> {
    let (settings, _) = Settings::load(&request, Component::Wallpaper)?;
    validate_images(&settings)?;
    let server = ipc::Server::bind()?;
    let (mut app, mut event_loop) = initialize(settings, request, false, None, None)?;
    let (sender, receiver) = channel::sync_channel(1);
    // One socket reader, one outstanding command, no thread/task per client.
    let listener = server.listener.try_clone()?;
    std::thread::spawn(move || {
        for connection in listener.incoming() {
            let Ok(mut stream) = connection else {
                break;
            };
            match ipc::read_request(&mut stream) {
                Ok(request) => {
                    if sender.send((request, stream)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = ipc::respond(&mut stream, &ipc::Response::error(error));
                }
            }
        }
    });
    event_loop
        .handle()
        .insert_source(receiver, |event, _, app: &mut App| {
            if let Event::Msg((request, mut stream)) = event {
                let response = match app.command(request) {
                    Ok(response) => response,
                    Err(error) => ipc::Response::error(error),
                };
                let _ = ipc::respond(&mut stream, &response);
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    run(&mut app, &mut event_loop)
}
pub fn lock(
    mut settings: Settings,
    ready_fd: Option<i32>,
    logind_session: Option<String>,
) -> Result<()> {
    let username = auth::preflight()?;
    if settings.use_wallpaper
        && let Ok(snapshot) =
            ipc::send_with_timeout(&ipc::Request::Status {}, Duration::from_millis(300))
    {
        for output in snapshot.outputs {
            if settings.background.image.is_none() && !settings.outputs.contains_key(&output.name) {
                settings.outputs.insert(output.name, output.background);
            }
        }
    }
    // A deleted or invalid wallpaper must never turn the Lock action into a
    // no-op. Admit valid assets, and fall back to a solid theme background.
    for background in std::iter::once(&mut settings.background).chain(settings.outputs.values_mut())
    {
        if background
            .image
            .as_deref()
            .is_some_and(|path| background::load(path).is_err())
        {
            background.image = None;
            eprintln!("lock wallpaper unavailable; using the theme background");
        }
    }
    let ui = LockUi::new(&settings, username)?;
    let ready = if let Some(fd) = ready_fd {
        ensure!(
            fd > 2,
            "ready-fd must be an inherited descriptor above stderr"
        );
        // Duplicate so the caller's ownership is untouched. A bad descriptor fails
        // before locking; the byte is sent only from the locked protocol callback.
        let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
        ensure!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } >= 0,
            "cannot protect readiness descriptor"
        );
        ensure!(copy >= 0, "ready-fd is invalid");
        Some(unsafe { OwnedFd::from_raw_fd(copy) })
    } else {
        None
    };
    let (mut app, mut event_loop) = initialize(settings, Request::Defaults, true, Some(ui), ready)?;
    app.logind = Some(crate::logind::Tracker::start(logind_session));
    app.handle
        .insert_source(Timer::immediate(), move |_, _, app: &mut App| {
            app.redraw();
            TimeoutAction::ToDuration(Duration::from_secs(
                if app.settings.clock_format == "%H:%M" && app.settings.date_format == "%A, %B %-d"
                {
                    60 - chrono::Timelike::second(&chrono::Local::now()) as u64
                } else {
                    1
                },
            ))
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    run(&mut app, &mut event_loop)
}
fn run(app: &mut App, event_loop: &mut EventLoop<'static, App>) -> Result<()> {
    while !app.exit {
        event_loop.dispatch(None, app)?;
        app.paint();
        app.conn.flush()?;
    }
    if let Some(error) = app.error.take() {
        anyhow::bail!("{error}");
    }
    Ok(())
}
impl App {
    fn redraw(&mut self) {
        for surface in &mut self.surfaces {
            surface.dirty = true;
        }
    }
    fn add_output(&mut self, output: wl_output::WlOutput) {
        if self.surfaces.iter().any(|s| s.output == output) {
            return;
        }
        let surface = self.compositor.create_surface(&self.qh);
        let role = if let Some(session) = &self.session {
            Role::Lock(session.create_lock_surface(surface, &output, &self.qh))
        } else {
            let layer = self.layer.as_ref().unwrap().create_layer_surface(
                &self.qh,
                surface,
                Layer::Background,
                Some("canopa-wallpaper"),
                Some(&output),
            );
            layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
            layer.set_size(0, 0);
            layer.set_exclusive_zone(-1);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            let region = smithay_client_toolkit::compositor::Region::new(&self.compositor)
                .expect("wl_compositor region");
            layer
                .wl_surface()
                .set_input_region(Some(region.wl_region()));
            layer.commit();
            Role::Wallpaper(layer)
        };
        self.surfaces.push(DisplaySurface {
            output,
            role,
            width: 0,
            height: 0,
            scale: 1,
            dirty: true,
            background: None,
        });
    }
    fn paint(&mut self) {
        let mut prepared = false;
        for record in &mut self.surfaces {
            if !record.dirty || record.width == 0 || record.height == 0 {
                continue;
            }
            record.dirty = false;
            prepared = true;
            let name = self
                .outputs
                .info(&record.output)
                .and_then(|i| i.name)
                .unwrap_or_default();
            let width = record.width.saturating_mul(record.scale as u32);
            let height = record.height.saturating_mul(record.scale as u32);
            let render = if let Some(ui) = &self.ui {
                if record
                    .background
                    .as_ref()
                    .is_none_or(|image| image.dimensions() != (width, height))
                {
                    record.background =
                        LockUi::prepare_background(&self.settings, &name, width, height).ok();
                }
                match &record.background {
                    Some(image) => ui.render_over(&self.settings, image.clone()),
                    None => ui.render(&self.settings, &name, width, height),
                }
            } else {
                let bg = self.settings.for_output(&name);
                background::render(
                    bg.image.as_deref(),
                    bg.fit,
                    self.settings.color,
                    width,
                    height,
                )
            };
            // Unavailable wallpaper must never prevent the locker from presenting
            // an opaque, usable authentication view after the compositor locks.
            let image = match render {
                Ok(image) => image,
                Err(error) => {
                    eprintln!("session surface preparation failed: {error}");
                    if let Some(ui) = &self.ui {
                        let mut fallback = self.settings.clone();
                        fallback.background.image = None;
                        fallback.outputs.clear();
                        match ui.render(&fallback, &name, width, height) {
                            Ok(image) => image,
                            Err(_) => continue,
                        }
                    } else {
                        continue;
                    }
                }
            };
            let result = (|| -> Result<()> {
                let mut pool = RawPool::new(background::dimensions(width, height)?, &self.shm)?;
                for (src, dst) in image.pixels().zip(pool.mmap().chunks_exact_mut(4)) {
                    let color = 0xff00_0000u32
                        | u32::from(src[0]) << 16
                        | u32::from(src[1]) << 8
                        | u32::from(src[2]);
                    dst.copy_from_slice(&color.to_ne_bytes());
                }
                let buffer = pool.create_buffer(
                    0,
                    width as i32,
                    height as i32,
                    width as i32 * 4,
                    wl_shm::Format::Xrgb8888,
                    (),
                    &self.qh,
                );
                let surface = record.role.surface();
                surface.set_buffer_scale(record.scale);
                surface.attach(Some(&buffer), 0, 0);
                surface.damage_buffer(0, 0, width as i32, height as i32);
                surface.commit();
                buffer.destroy();
                // wl_shm pool and local pixels can now be dropped. The compositor
                // retains its backing fd until it no longer needs the submitted buffer.
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("session surface submission failed: {error}");
            }
        }
        if prepared && self.ui.is_none() {
            // Decoding and resampling can leave large free allocator arenas. An idle
            // daemon has no reason to retain that scratch memory for the next image.
            #[cfg(target_env = "gnu")]
            unsafe {
                libc::malloc_trim(0);
            }
        }
    }
    fn command(&mut self, request: ipc::Request) -> Result<ipc::Response> {
        match request {
            ipc::Request::Set { image, output, fit } => {
                ensure!(image.is_absolute(), "image path must be absolute");
                background::load(&image)?;
                if let Some(output) = output {
                    ensure!(!output.is_empty(), "empty output name");
                    self.settings.outputs.insert(
                        output,
                        Background {
                            image: Some(image),
                            fit,
                        },
                    );
                } else {
                    self.settings.background = Background {
                        image: Some(image),
                        fit,
                    };
                    self.settings.outputs.clear();
                }
                self.redraw();
                self.paint();
                self.conn.flush()?;
            }
            ipc::Request::Reload {} => {
                let (candidate, _) = Settings::load(&self.request, Component::Wallpaper)?;
                validate_images(&candidate)?;
                self.settings = candidate;
                self.redraw();
                self.paint();
                self.conn.flush()?;
            }
            ipc::Request::Stop {} => self.exit = true,
            ipc::Request::Status {} => {}
        }
        Ok(ipc::Response {
            error: None,
            outputs: self
                .surfaces
                .iter()
                .filter_map(|s| {
                    let name = self.outputs.info(&s.output)?.name?;
                    Some(ipc::Output {
                        name: name.clone(),
                        background: self.settings.for_output(&name).clone(),
                        width: s.width,
                        height: s.height,
                    })
                })
                .collect(),
        })
    }
    fn begin_authentication(&mut self) {
        if !self.confirmed || self.authentication.is_some() || Instant::now() < self.retry_after {
            return;
        }
        self.auth_generation = self.auth_generation.wrapping_add(1);
        let generation = self.auth_generation;
        let updates = self.updates.clone();
        match auth::Worker::start(move |event| {
            let _ = updates.send(Update::Authentication { generation, event });
        }) {
            Ok(worker) => {
                self.authentication = Some(worker);
                if let Some(ui) = &mut self.ui {
                    ui.busy = true;
                    ui.failed = false;
                    ui.prompt = None;
                    ui.secret.clear();
                    ui.message = "Starting authentication…".into();
                }
                let duration = Duration::from_secs(self.settings.auth_timeout);
                match self.handle.insert_source(
                    Timer::from_duration(duration),
                    move |_, _, app: &mut App| {
                        app.auth_timeout = None;
                        if generation == app.auth_generation && app.authentication.is_some() {
                            app.cancel_authentication("Authentication timed out. Enter to retry.");
                        }
                        TimeoutAction::Drop
                    },
                ) {
                    Ok(token) => self.auth_timeout = Some(token),
                    Err(_) => self
                        .cancel_authentication("Authentication could not start. Enter to retry."),
                }
            }
            Err(error) => {
                eprintln!("authentication worker could not start: {error:#}");
                self.cancel_authentication("Authentication could not start. Enter to retry.");
            }
        }
        self.redraw();
    }
    fn stop_authentication(&mut self) {
        if let Some(token) = self.auth_timeout.take() {
            self.handle.remove(token);
        }
        self.authentication.take();
    }
    fn cancel_authentication(&mut self, message: &str) {
        self.auth_generation = self.auth_generation.wrapping_add(1);
        self.stop_authentication();
        if let Some(ui) = &mut self.ui {
            ui.busy = false;
            ui.prompt = None;
            ui.secret.clear();
            ui.message = message.into();
        }
        self.redraw();
    }
    fn authentication_event(&mut self, generation: u64, event: auth::Event) {
        if generation != self.auth_generation || self.authentication.is_none() {
            return;
        }
        match event {
            auth::Event::Prompt { id, text, echo } => {
                if let Some(ui) = &mut self.ui {
                    ui.secret.clear();
                    if ui.message == "Starting authentication…" {
                        ui.message.clear();
                    }
                    ui.prompt = Some(crate::lock_ui::Prompt { id, text, echo });
                }
            }
            auth::Event::Message { text, error } => {
                if let Some(ui) = &mut self.ui {
                    ui.message = text;
                    ui.failed = error;
                }
            }
            auth::Event::Complete { accepted } => {
                if !accepted {
                    let elapsed = self.authentication.as_ref().unwrap().started.elapsed();
                    self.retry_after =
                        Instant::now() + Duration::from_secs(2).saturating_sub(elapsed);
                    self.stop_authentication();
                    if let Some(ui) = &mut self.ui {
                        ui.busy = false;
                        ui.failed = true;
                        ui.prompt = None;
                        ui.secret.clear();
                        ui.message = "Authentication failed. Enter to retry.".into();
                    }
                } else if self.confirmed {
                    self.stop_authentication();
                    if let Some(session) = &self.session {
                        session.unlock();
                    }
                    match self.conn.roundtrip() {
                        Ok(_) => {
                            if let Some(tracker) = &self.logind {
                                tracker.set_locked(false);
                            }
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                    self.exit = true;
                }
            }
        }
        self.redraw();
    }
    fn key(&mut self, mut event: KeyEvent) {
        let text = auth::take_text(event.utf8.take());
        if !self.confirmed || self.ui.is_none() {
            return;
        }
        if event.keysym == Keysym::Escape {
            if self.ui.as_ref().is_some_and(|ui| !ui.secret.is_empty()) {
                self.ui.as_mut().unwrap().secret.clear();
            } else {
                self.cancel_authentication("Authentication cancelled. Enter to retry.");
            }
        } else if matches!(event.keysym, Keysym::Return | Keysym::KP_Enter) {
            if self.authentication.is_none() {
                self.begin_authentication();
            } else if let Some(prompt) = self.ui.as_mut().unwrap().prompt.take() {
                let answer = std::mem::take(&mut self.ui.as_mut().unwrap().secret);
                if self
                    .authentication
                    .as_mut()
                    .unwrap()
                    .respond(prompt.id, answer)
                    .is_err()
                {
                    self.cancel_authentication("Authentication failed. Enter to retry.");
                }
            }
        } else if self.ui.as_ref().unwrap().prompt.is_some() {
            let ui = self.ui.as_mut().unwrap();
            if event.keysym == Keysym::BackSpace {
                ui.secret.pop();
            } else {
                ui.secret.push(&text);
            }
        }
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
        if let Some(s) = self
            .surfaces
            .iter_mut()
            .find(|s| s.role.surface() == surface)
        {
            s.scale = factor.max(1);
            s.dirty = true;
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
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.add_output(output);
    }
    fn update_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(s) = self.surfaces.iter_mut().find(|s| s.output == output) {
            s.dirty = true;
        }
    }
    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.surfaces.retain(|s| s.output != output);
    }
}
impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.surfaces
            .retain(|s| s.role.surface() != layer.wl_surface());
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some(s) = self
            .surfaces
            .iter_mut()
            .find(|s| s.role.surface() == layer.wl_surface())
        {
            (s.width, s.height) = configure.new_size;
            s.dirty = true;
        }
    }
}
impl SessionLockHandler for App {
    fn locked(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        self.confirmed = true;
        if let Some(tracker) = &self.logind {
            tracker.set_locked(true);
        }
        self.begin_authentication();
        self.widgets = start_widgets(&self.settings, self.updates.clone());
        if let Some(fd) = self.ready.take() {
            let mut file = std::fs::File::from(fd);
            if file.write_all(b"1").is_err() {
                eprintln!("could not publish lock readiness");
            }
        }
    }
    fn finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        // The compositor owns this terminal event. It may revoke a previously
        // acquired lock after an alternative secure authentication mechanism.
        if self.confirmed {
            if let Some(session) = &self.session {
                session.unlock();
            }
            if self.conn.roundtrip().is_ok()
                && let Some(tracker) = &self.logind
            {
                tracker.set_locked(false);
            }
        } else {
            self.error =
                Some("compositor refused session lock (another locker may be active)".into());
        }
        self.exit = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        _: u32,
    ) {
        if let Some(s) = self
            .surfaces
            .iter_mut()
            .find(|s| s.role.surface() == surface.wl_surface())
        {
            (s.width, s.height) = configure.new_size;
            s.dirty = true;
        }
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
        capability: Capability,
    ) {
        if self.ui.is_some()
            && capability == Capability::Keyboard
            && !self.keyboards.iter().any(|(s, _)| s == &seat)
        {
            match self.seats.get_keyboard(qh, &seat, None) {
                Ok(keyboard) => self.keyboards.push((seat, keyboard)),
                Err(error) => eprintln!("keyboard unavailable: {error}"),
            }
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
            self.remove_keyboard(&seat);
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        self.remove_keyboard(&seat);
    }
}
impl App {
    fn remove_keyboard(&mut self, seat: &wl_seat::WlSeat) {
        self.layouts.retain(|(keyboard, _)| {
            !self
                .keyboards
                .iter()
                .any(|(s, k)| s == seat && k == keyboard)
        });
        self.keyboards.retain(|(s, k)| {
            if s == seat {
                k.release();
                false
            } else {
                true
            }
        });
        if let Some(ui) = &mut self.ui {
            ui.secret.clear();
        }
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
        self.key(event);
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
        keyboard: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: RawModifiers,
        layout: u32,
    ) {
        if let Some(ui) = &mut self.ui {
            ui.caps = modifiers.caps_lock;
            ui.layout = self
                .layouts
                .iter()
                .find(|(k, _)| k == keyboard)
                .and_then(|(_, names)| names.get(layout as usize))
                .cloned()
                .unwrap_or_else(|| format!("Layout {}", layout + 1));
            self.redraw();
        }
    }
    fn update_keymap(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        keyboard: &wl_keyboard::WlKeyboard,
        keymap: smithay_client_toolkit::seat::keyboard::Keymap<'_>,
    ) {
        use xkbcommon::xkb;
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        if let Some(keymap) = xkb::Keymap::new_from_string(
            &context,
            keymap.as_string(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) {
            let names = (0..keymap.num_layouts())
                .map(|i| keymap.layout_get_name(i).to_owned())
                .collect();
            self.layouts.retain(|(k, _)| k != keyboard);
            self.layouts.push((keyboard.clone(), names));
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
smithay_client_toolkit::delegate_layer!(App);
smithay_client_toolkit::delegate_session_lock!(App);
smithay_client_toolkit::delegate_shm!(App);
smithay_client_toolkit::delegate_registry!(App);
wayland_client::delegate_noop!(App:ignore wl_buffer::WlBuffer);

type WidgetRuntime = canopa_session_ui::widgets::Runtime;
fn start_widgets(settings: &Settings, updates: channel::Sender<Update>) -> Option<WidgetRuntime> {
    canopa_session_ui::widgets::start(settings, move |event| {
        use canopa_session_ui::widgets::Update as Widget;
        updates
            .send(match event {
                Widget::Battery(v) => Update::Battery(v),
                Widget::Media(v) => Update::Media(v),
                Widget::Custom { name, text } => Update::Custom { name, text },
            })
            .is_ok()
    })
}
