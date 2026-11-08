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
        pointer::{
            CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer,
        },
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
    protocol::{wl_buffer, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
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
    hits: Vec<(usize, canopa_session_ui::layout::Rect)>,
    output: wl_output::WlOutput,
    role: Role,
    width: u32,
    height: u32,
    scale: i32,
    dirty: bool,
    background: Option<image::RgbImage>,
}
struct Wallpaper {
    base: Settings,
    assignments: crate::wallpaper_state::Saved,
    persisted: crate::wallpaper_state::Saved,
    path: Option<std::path::PathBuf>,
    sources: Vec<std::path::PathBuf>,
    sources_baseline: crate::file_watch::Snapshot,
    observer: Option<crate::file_watch::Observer>,
    timer: Option<calloop::RegistrationToken>,
    slide_cursor: Option<usize>,
}
enum Update {
    WallpaperChanged,
    Accessibility(canopa_session_ui::accessibility::Request),
    Authentication { generation: u64, event: auth::Event },
    Battery(canopa_widgets::battery::BatteryState),
    Media(Option<String>),
    Custom { name: String, text: Option<String> },
}
struct App {
    wallpaper: Option<Wallpaper>,
    accessibility: Option<canopa_session_ui::accessibility::Accessibility>,
    input: canopa_session_ui::input::Keyboard,
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
    pointers: Vec<(wl_seat::WlSeat, ThemedPointer)>,
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
                    Update::WallpaperChanged => {
                        if let Err(error) = app.reload_wallpaper(false) {
                            eprintln!("wallpaper source update rejected: {error}");
                        }
                    }
                    Update::Accessibility(request) => app.accessible(request),
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
    let accessibility = ui.as_ref().map(|_| {
        let sender = updates.clone();
        canopa_session_ui::accessibility::Accessibility::new("Canopa Lock", move |request| {
            let _ = sender.send(Update::Accessibility(request));
        })
    });
    let mut app = App {
        wallpaper: None,
        accessibility,
        input: Default::default(),
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
        pointers: vec![],
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
    let (base, document) = Settings::load(&request, Component::Wallpaper)?;
    let path = crate::wallpaper_state::Saved::path();
    let mut saved = path
        .as_ref()
        .and_then(|p| crate::wallpaper_state::Saved::load(p).ok())
        .unwrap_or_default();
    let mut settings = saved.apply(&base);
    if validate_images(&settings).is_err() {
        eprintln!("saved wallpaper unavailable; using configured assignments");
        saved = Default::default();
        settings = base.clone();
        validate_images(&settings)?;
    }
    let server = ipc::Server::bind()?;
    let (mut app, mut event_loop) = initialize(settings, request, false, None, None)?;
    let sources = app.wallpaper_sources(&document);
    app.wallpaper = Some(Wallpaper {
        base,
        assignments: saved.clone(),
        persisted: saved,
        path,
        sources_baseline: crate::file_watch::snapshot(
            &sources
                .iter()
                .cloned()
                .map(canopa_file::watch::Target::File)
                .collect::<Vec<_>>(),
        ),
        sources,
        observer: None,
        timer: None,
        slide_cursor: None,
    });
    let updates = app.updates.clone();
    let observer = crate::file_watch::Observer::start(app.wallpaper_targets(), move || {
        let _ = updates.send(Update::WallpaperChanged);
    });
    app.wallpaper.as_mut().unwrap().observer = Some(observer);
    app.schedule_slideshow()?;
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
    let mut ui = LockUi::new(&settings, username)?;
    crate::lock_ui::configure(&mut ui);
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
use crate::lock_ui::{CHANGE_PASSWORD, UNLOCK};
const CANCEL_CHANGE: usize = usize::MAX - 3;
impl App {
    fn accessible(&mut self, request: canopa_session_ui::accessibility::Request) {
        use canopa_session_ui::accessibility::{Operation, select};
        if let (Some(a), Some(ui)) = (&mut self.accessibility, &self.ui) {
            a.update("Canopa Lock", ui, &self.settings, self.confirmed);
        }
        if !self.confirmed
            || !self
                .accessibility
                .as_ref()
                .is_some_and(|a| a.accepts(&request))
        {
            return;
        }
        match request.operation {
            Operation::Submit => self.key(KeyEvent {
                time: 0,
                raw_code: 28,
                keysym: Keysym::Return,
                utf8: None,
            }),
            Operation::Cancel => self.key(KeyEvent {
                time: 0,
                raw_code: 1,
                keysym: Keysym::Escape,
                utf8: None,
            }),
            Operation::Control(id) => self.control(id),
            Operation::Replace(secret) => {
                if let Some(ui) = &mut self.ui
                    && ui.prompt.is_some()
                    && !ui.hide_input
                {
                    ui.secret.clear();
                    ui.secret.push(secret.text());
                }
            }
            Operation::Select {
                anchor,
                focus,
                echo,
            } => {
                if let Some(ui) = &mut self.ui
                    && ui.prompt.is_some()
                    && !ui.hide_input
                {
                    select(&mut ui.secret, anchor, focus, echo);
                }
            }
        }
        self.redraw();
    }

    fn redraw(&mut self) {
        if let Some(ui) = &mut self.ui {
            crate::lock_ui::update_actions(ui, self.confirmed && self.authentication.is_none());
        }
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
            hits: Vec::new(),
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
        if let (Some(a), Some(ui)) = (&mut self.accessibility, &self.ui) {
            a.update("Canopa Lock", ui, &self.settings, self.confirmed);
        }

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
                    Some(image) => ui.render_on_output(&self.settings, &name, image.clone()),
                    None => LockUi::prepare_background(&self.settings, &name, width, height)
                        .and_then(|image| ui.render_on_output(&self.settings, &name, image)),
                }
            } else if let Some(image) = record
                .background
                .take()
                .filter(|image| image.dimensions() == (width, height))
            {
                Ok((image, Vec::new()))
            } else {
                let bg = self.settings.for_output(&name);
                background::render(
                    bg.image.as_deref(),
                    bg.fit,
                    self.settings.color,
                    width,
                    height,
                )
                .map(|image| (image, Vec::new()))
            };
            // Unavailable wallpaper must never prevent the locker from presenting
            // an opaque, usable authentication view after the compositor locks.
            let (image, hits) = match render {
                Ok(image) => image,
                Err(error) => {
                    eprintln!("session surface preparation failed: {error}");
                    if let Some(ui) = &self.ui {
                        let mut fallback = self.settings.clone();
                        fallback.background.image = None;
                        fallback.outputs.clear();
                        match LockUi::prepare_background(&fallback, &name, width, height)
                            .and_then(|image| ui.render_on_output(&fallback, &name, image))
                        {
                            Ok(image) => image,
                            Err(_) => continue,
                        }
                    } else {
                        continue;
                    }
                }
            };
            record.hits = hits;
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
    fn wallpaper_sources(&self, document: &canopa_config::Document) -> Vec<std::path::PathBuf> {
        let mut paths = document
            .info()
            .path
            .iter()
            .cloned()
            .chain(document.info().dependencies.iter().map(|d| d.path.clone()))
            .collect::<Vec<_>>();
        if self.request == Request::Discover
            && let Ok(home) = canopa_config::config_home()
        {
            paths.push(home.join("canopa/config.kdl"));
        }
        paths
    }
    fn wallpaper_targets(&self) -> Vec<canopa_file::watch::Target> {
        use canopa_file::watch::Target;
        let Some(wallpaper) = &self.wallpaper else {
            return Vec::new();
        };
        let mut targets = wallpaper
            .sources
            .iter()
            .cloned()
            .map(Target::File)
            .collect::<Vec<_>>();
        for settings in [&self.settings, &wallpaper.base] {
            targets.extend(
                std::iter::once(&settings.background)
                    .chain(settings.outputs.values())
                    .filter_map(|bg| bg.image.clone())
                    .map(Target::File),
            );
        }
        if let Some(slideshow) = &wallpaper.assignments.slideshow {
            targets.push(Target::Directory(slideshow.directory.clone()));
        }
        targets.truncate(256);
        targets
    }
    fn prepare_wallpaper(&self, candidate: &Settings) -> Result<Vec<Option<image::RgbImage>>> {
        type Targets = Vec<(usize, u32, u32, background::Fit)>;
        let mut groups = std::collections::BTreeMap::<Option<std::path::PathBuf>, Targets>::new();
        for bg in std::iter::once(&candidate.background).chain(candidate.outputs.values()) {
            groups.entry(bg.image.clone()).or_default();
        }
        let mut prepared: Vec<Option<image::RgbImage>> =
            (0..self.surfaces.len()).map(|_| None).collect();
        let mut total = 0usize;
        for (index, surface) in self.surfaces.iter().enumerate() {
            if surface.width == 0 || surface.height == 0 {
                continue;
            }
            let width = surface.width.saturating_mul(surface.scale as u32);
            let height = surface.height.saturating_mul(surface.scale as u32);
            total = total
                .checked_add(background::dimensions(width, height)?)
                .context("wallpaper surface budget overflow")?;
            ensure!(
                total <= 256 * 1024 * 1024,
                "combined wallpaper surfaces exceed the 256 MiB admission budget"
            );
            let name = self
                .outputs
                .info(&surface.output)
                .and_then(|i| i.name)
                .unwrap_or_default();
            let bg = candidate.for_output(&name);
            groups
                .entry(bg.image.clone())
                .or_default()
                .push((index, width, height, bg.fit));
        }
        // Decode each distinct file once. Only this source and the bounded set
        // of prepared output buffers are live together during admission.
        for (path, targets) in groups {
            let source = path.as_deref().map(background::load).transpose()?;
            for (index, width, height, fit) in targets {
                prepared[index] = Some(background::render_source(
                    source.as_ref(),
                    fit,
                    candidate.color,
                    width,
                    height,
                )?);
            }
        }
        Ok(prepared)
    }
    fn commit_wallpaper(
        &mut self,
        base: Settings,
        assignments: crate::wallpaper_state::Saved,
        persisted: Option<crate::wallpaper_state::Saved>,
    ) -> Result<()> {
        let candidate = assignments.apply(&base);
        let mut targets = self.wallpaper_targets();
        for bg in std::iter::once(&candidate.background).chain(candidate.outputs.values()) {
            if let Some(path) = &bg.image {
                targets.push(canopa_file::watch::Target::File(path.clone()));
            }
        }
        let baseline = crate::file_watch::retain_snapshot(
            &self
                .wallpaper
                .as_ref()
                .context("wallpaper runtime unavailable")?
                .sources_baseline,
            &targets,
        );
        let prepared = self.prepare_wallpaper(&candidate)?;
        let wallpaper = self
            .wallpaper
            .as_mut()
            .context("wallpaper runtime unavailable")?;
        if let Some(saved) = persisted {
            saved.save(
                wallpaper
                    .path
                    .as_deref()
                    .context("no writable state location is configured")?,
            )?;
            wallpaper.persisted = saved;
        }
        wallpaper.base = base;
        wallpaper.assignments = assignments;
        self.settings = candidate;
        for (surface, image) in self.surfaces.iter_mut().zip(prepared) {
            surface.background = image;
            surface.dirty = true;
        }
        if let Some(observer) = &self.wallpaper.as_ref().unwrap().observer {
            let final_targets = self.wallpaper_targets();
            observer.admitted(
                final_targets.clone(),
                crate::file_watch::retain_snapshot(&baseline, &final_targets),
            );
        }
        self.paint();
        self.conn.flush()?;
        Ok(())
    }
    fn reload_wallpaper(&mut self, reset: bool) -> Result<()> {
        let targets = self.wallpaper_targets();
        let baseline = crate::file_watch::snapshot(&targets);
        let loaded = Settings::load(&self.request, Component::Wallpaper);
        if let Err(error) = loaded {
            if let Some(observer) = self.wallpaper.as_ref().and_then(|w| w.observer.as_ref()) {
                observer.admitted(targets, baseline);
            }
            return Err(error);
        }
        let (base, document) = loaded?;
        let assignments = if reset {
            Default::default()
        } else {
            self.wallpaper
                .as_ref()
                .context("wallpaper runtime unavailable")?
                .assignments
                .clone()
        };
        let sources = self.wallpaper_sources(&document);
        let source_targets = sources
            .iter()
            .cloned()
            .map(canopa_file::watch::Target::File)
            .collect::<Vec<_>>();
        let source_baseline = crate::file_watch::retain_snapshot(&baseline, &source_targets);
        let wallpaper = self
            .wallpaper
            .as_mut()
            .context("wallpaper runtime unavailable")?;
        let previous_sources = std::mem::replace(&mut wallpaper.sources, sources);
        let previous_baseline = std::mem::replace(&mut wallpaper.sources_baseline, source_baseline);
        if let Err(error) = self.commit_wallpaper(base, assignments, reset.then(Default::default)) {
            let wallpaper = self.wallpaper.as_mut().unwrap();
            wallpaper.sources = previous_sources;
            wallpaper.sources_baseline = previous_baseline;
            if let Some(observer) = self.wallpaper.as_ref().and_then(|w| w.observer.as_ref()) {
                observer.admitted(targets, baseline);
            }
            return Err(error);
        }
        if reset {
            self.schedule_slideshow()?;
        }
        Ok(())
    }
    fn schedule_slideshow(&mut self) -> Result<()> {
        let Some(wallpaper) = &mut self.wallpaper else {
            return Ok(());
        };
        if let Some(timer) = wallpaper.timer.take() {
            self.handle.remove(timer);
        }
        wallpaper.slide_cursor = None;
        if let Some(spec) = &wallpaper.assignments.slideshow {
            let timer = self
                .handle
                .insert_source(
                    Timer::from_duration(Duration::from_secs(spec.interval)),
                    |_, _, app: &mut App| {
                        let Some(interval) = app
                            .wallpaper
                            .as_ref()
                            .and_then(|w| w.assignments.slideshow.as_ref())
                            .map(|s| s.interval)
                        else {
                            if let Some(w) = &mut app.wallpaper {
                                w.timer = None;
                            }
                            return TimeoutAction::Drop;
                        };
                        if let Err(error) = app.advance_slideshow() {
                            eprintln!("slideshow image rejected: {error}");
                        }
                        TimeoutAction::ToDuration(Duration::from_secs(interval))
                    },
                )
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            wallpaper.timer = Some(timer);
        }
        Ok(())
    }
    fn advance_slideshow(&mut self) -> Result<()> {
        let wallpaper = self
            .wallpaper
            .as_ref()
            .context("wallpaper runtime unavailable")?;
        let mut assignments = wallpaper.assignments.clone();
        let spec = assignments
            .slideshow
            .clone()
            .context("no slideshow is active")?;
        let images = spec.images()?;
        let current = spec
            .output
            .as_deref()
            .map(|name| self.settings.for_output(name))
            .unwrap_or(&self.settings.background)
            .image
            .as_ref();
        let next = wallpaper.slide_cursor.unwrap_or_else(|| {
            current
                .and_then(|p| images.iter().position(|i| i == p))
                .map(|i| (i + 1) % images.len())
                .unwrap_or(0)
        }) % images.len();
        assignments.set(
            spec.output.as_deref(),
            Background {
                image: Some(images[next].clone()),
                fit: spec.fit,
            },
        )?;
        let base = wallpaper.base.clone();
        self.wallpaper.as_mut().unwrap().slide_cursor = Some((next + 1) % images.len());
        self.commit_wallpaper(base, assignments, None)
    }
    fn command(&mut self, request: ipc::Request) -> Result<ipc::Response> {
        match request {
            ipc::Request::Set {
                image,
                output,
                fit,
                persist,
            } => {
                let wallpaper = self
                    .wallpaper
                    .as_ref()
                    .context("wallpaper runtime unavailable")?;
                let bg = Background {
                    image: Some(image),
                    fit,
                };
                let mut assignments = wallpaper.assignments.clone();
                assignments.set(output.as_deref(), bg.clone())?;
                assignments.slideshow = None;
                let persisted = if persist {
                    let mut saved = wallpaper.persisted.clone();
                    saved.set(output.as_deref(), bg)?;
                    saved.slideshow = None;
                    Some(saved)
                } else {
                    None
                };
                self.commit_wallpaper(wallpaper.base.clone(), assignments, persisted)?;
                self.schedule_slideshow()?;
            }
            ipc::Request::SlideshowStart {
                directory,
                interval,
                output,
                fit,
            } => {
                let spec = crate::wallpaper_state::Slideshow {
                    directory,
                    interval,
                    output,
                    fit,
                };
                ensure!(
                    (10..=86400).contains(&interval),
                    "slideshow interval must be 10..86400 seconds"
                );
                let first = spec.images()?.remove(0);
                let wallpaper = self
                    .wallpaper
                    .as_ref()
                    .context("wallpaper runtime unavailable")?;
                let bg = Background {
                    image: Some(first),
                    fit,
                };
                let mut assignments = wallpaper.assignments.clone();
                assignments.set(spec.output.as_deref(), bg.clone())?;
                assignments.slideshow = Some(spec.clone());
                let mut persisted = wallpaper.persisted.clone();
                persisted.set(spec.output.as_deref(), bg)?;
                persisted.slideshow = Some(spec);
                self.commit_wallpaper(wallpaper.base.clone(), assignments, Some(persisted))?;
                self.schedule_slideshow()?;
            }
            ipc::Request::SlideshowStop {} => {
                let wallpaper = self
                    .wallpaper
                    .as_ref()
                    .context("wallpaper runtime unavailable")?;
                if let Some(spec) = &wallpaper.assignments.slideshow {
                    let mut assignments = wallpaper.assignments.clone();
                    assignments.slideshow = None;
                    let mut persisted = wallpaper.persisted.clone();
                    persisted.set(
                        spec.output.as_deref(),
                        spec.output
                            .as_deref()
                            .map(|o| self.settings.for_output(o))
                            .unwrap_or(&self.settings.background)
                            .clone(),
                    )?;
                    persisted.slideshow = None;
                    self.commit_wallpaper(wallpaper.base.clone(), assignments, Some(persisted))?;
                    self.schedule_slideshow()?;
                }
            }
            ipc::Request::Next {} => self.advance_slideshow()?,
            ipc::Request::Reload {} => self.reload_wallpaper(true)?,
            ipc::Request::Stop {} => self.exit = true,
            ipc::Request::Status {} => {}
        }
        Ok(ipc::Response {
            error: None,
            color: Some([
                self.settings.color.0,
                self.settings.color.1,
                self.settings.color.2,
            ]),
            slideshow: self
                .wallpaper
                .as_ref()
                .and_then(|w| w.assignments.slideshow.clone()),
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
                        scale: s.scale.max(1) as u32,
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
        if let Some(ui) = &mut self.ui {
            ui.interaction_epoch = (self.auth_generation, 0);
        }
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
        if let Some(ui) = &mut self.ui {
            ui.hide_input = false;
            ui.controls.clear();
            ui.action_labels = None;
        }
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
                    ui.hide_input = false;
                    ui.controls.clear();
                    ui.action_labels = None;
                    if ui.message == "Starting authentication…" {
                        ui.message.clear();
                    }
                    ui.prompt = Some(crate::lock_ui::Prompt { id, text, echo });
                }
            }
            auth::Event::PasswordExpired { id } => {
                if let Some(ui) = &mut self.ui {
                    ui.secret.clear();
                    ui.hide_input = true;
                    ui.prompt = Some(crate::lock_ui::Prompt {
                        id,
                        text: "Password change required".into(),
                        echo: false,
                    });
                    ui.message = "Your password has expired. Change it to unlock, or cancel to remain locked.".into();
                    ui.action_labels =
                        Some(("Change password".into(), "Cancel password change".into()));
                    ui.controls = vec![
                        canopa_session_ui::view::Control {
                            id: CHANGE_PASSWORD,
                            label: "Change password · Enter".into(),
                        },
                        canopa_session_ui::view::Control {
                            id: CANCEL_CHANGE,
                            label: "Cancel · Escape".into(),
                        },
                    ];
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
    fn control(&mut self, id: usize) {
        if id == canopa_session_ui::input::LAYOUT_CONTROL {
            self.input.cycle();
            if let Some(ui) = &mut self.ui {
                ui.layout = self.input.label();
            }
        } else if self.confirmed
            && self
                .ui
                .as_ref()
                .is_some_and(|ui| ui.controls.iter().any(|control| control.id == id))
        {
            match id {
                UNLOCK | CHANGE_PASSWORD => self.submit_authentication(),
                CANCEL_CHANGE => {
                    self.cancel_authentication("Authentication cancelled. Enter to retry.")
                }
                _ => return,
            }
        }
        self.redraw();
    }
    fn submit_authentication(&mut self) {
        if self.authentication.is_none() {
            self.begin_authentication();
        } else if let Some(prompt) = self.ui.as_mut().unwrap().prompt.take() {
            let ui = self.ui.as_mut().unwrap();
            ui.hide_input = false;
            ui.controls.clear();
            ui.action_labels = None;
            let answer = std::mem::take(&mut ui.secret);
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
    }
    fn key(&mut self, mut event: KeyEvent) {
        let (symbol, text) = self
            .input
            .translate(event.raw_code, event.keysym, event.utf8.take());
        event.keysym = symbol;
        if !self.confirmed || self.ui.is_none() {
            return;
        }
        if event.keysym == Keysym::F6 {
            self.input.cycle();
            let ui = self.ui.as_mut().unwrap();
            ui.layout = self.input.label();
            ui.can_select_layout = self.input.can_select();
        } else if event.keysym == Keysym::Escape {
            if self.ui.as_ref().is_some_and(|ui| !ui.secret.is_empty()) {
                self.ui.as_mut().unwrap().secret.clear();
            } else {
                self.cancel_authentication("Authentication cancelled. Enter to retry.");
            }
        } else if matches!(event.keysym, Keysym::Return | Keysym::KP_Enter) {
            self.submit_authentication();
        } else if self
            .ui
            .as_ref()
            .is_some_and(|ui| ui.prompt.is_some() && !ui.hide_input)
        {
            let ui = self.ui.as_mut().unwrap();
            if let Some(edit) = self.input.edit(event.keysym) {
                ui.secret.edit(edit);
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
                Ok(keyboard) => self.keyboards.push((seat.clone(), keyboard)),
                Err(error) => eprintln!("keyboard unavailable: {error}"),
            }
        }
        if self.ui.is_some()
            && capability == Capability::Pointer
            && !self.pointers.iter().any(|(s, _)| s == &seat)
            && let Ok(pointer) = self.seats.get_pointer_with_theme(
                qh,
                &seat,
                self.shm.wl_shm(),
                self.compositor.create_surface(qh),
                ThemeSpec::default(),
            )
        {
            self.pointers.push((seat, pointer));
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
        if capability == Capability::Pointer {
            self.remove_pointer(&seat);
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        self.remove_keyboard(&seat);
        self.remove_pointer(&seat);
    }
}
impl App {
    fn remove_pointer(&mut self, seat: &wl_seat::WlSeat) {
        self.pointers.retain(|(s, p)| {
            if s == seat {
                p.pointer().release();
                false
            } else {
                true
            }
        });
    }
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
        if let Some(ui) = &mut self.ui {
            ui.caps = modifiers.caps_lock;
            ui.layout = self.input.label();
            ui.can_select_layout = self.input.can_select();
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
        self.input.keymap(keymap.as_string());
        if let Some(ui) = &mut self.ui {
            ui.layout = self.input.label();
            ui.can_select_layout = self.input.can_select();
        }

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
            if !self.confirmed || !matches!(event.kind, PointerEventKind::Press { button: 272, .. })
            {
                continue;
            }
            if let Some(record) = self
                .surfaces
                .iter()
                .find(|s| s.role.surface() == &event.surface)
            {
                let (x, y) = (
                    event.position.0 as f32 * record.scale as f32,
                    event.position.1 as f32 * record.scale as f32,
                );
                if let Some((id, _)) = record
                    .hits
                    .iter()
                    .find(|(_, r)| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height)
                {
                    self.control(*id);
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

smithay_client_toolkit::delegate_pointer!(App);
