//! Iced widget trees hosted by the session clients' existing Wayland event loops.
//!
//! Native surface roles and authentication stay with the caller. Only pointer
//! events enter Iced: response text never passes through its event/message queues.
use crate::{layout::Rect, settings::Settings, view::SessionView};
use anyhow::{Result, ensure};
use iced::{Color, Event, Font, Pixels, Point, Rectangle, Size, Theme, mouse};
use iced_renderer::graphics::{self, Compositor as _};
use iced_runtime::{UserInterface, user_interface};
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

mod native;
pub use native::{Graphics, NativeWindow};

type Compositor = iced_renderer::Compositor;
type NativeSurface = <Compositor as graphics::Compositor>::Surface;

/// Upstream graphics constructors/configuration currently panic on platform
/// errors. Contain that boundary so a hotplug failure becomes output recovery.
fn graphics_call<T>(operation: impl FnOnce() -> T) -> Result<T> {
    catch_unwind(AssertUnwindSafe(operation))
        .map_err(|_| anyhow::anyhow!("graphics backend panicked"))
}

fn software_renderer() -> Renderer {
    Renderer::Secondary(iced_tiny_skia::Renderer::new(Font::DEFAULT, Pixels(14.)))
}

#[derive(Clone)]
pub struct Background {
    handle: iced::widget::image::Handle,
    dimensions: (u32, u32),
    /// The clearer picture a resting lock screen fades to.
    rest: Option<iced::widget::image::Handle>,
}
fn rgba_handle(image: image::RgbImage) -> iced::widget::image::Handle {
    let (width, height) = image.dimensions();
    let rgba = image::DynamicImage::ImageRgb8(image).into_rgba8();
    iced::widget::image::Handle::from_rgba(width, height, rgba.into_raw())
}
impl Background {
    pub fn from_rgb(image: image::RgbImage) -> Self {
        let dimensions = image.dimensions();
        Self {
            handle: rgba_handle(image),
            dimensions,
            rest: None,
        }
    }
    /// A background with the picture shown while the surface rests.
    pub fn with_rest(mut self, rest: Option<image::RgbImage>) -> Self {
        self.rest = rest
            .filter(|image| image.dimensions() == self.dimensions)
            .map(rgba_handle);
        self
    }
    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }
}

/// One native frame. Dimensions and pointer coordinates are physical pixels;
/// `scale` controls the session layout, independently of the Wayland buffer scale.
pub struct Frame<'a> {
    pub key: &'a str,
    pub output: &'a str,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub background: Option<Background>,
}

pub(crate) type Renderer = iced::Renderer;
pub(crate) type Element<'a> = iced::Element<'a, Message, Theme, Renderer>;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Message {
    Control(usize),
    Selection { anchor: usize, cursor: usize },
}

/// Coordinates are physical buffer pixels, independent of the graphics backend.
#[derive(Clone, Copy, Debug)]
pub enum Pointer {
    Move { x: f32, y: f32 },
    Leave,
    Press,
    Release,
    Scroll { x: f32, y: f32 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor {
    #[default]
    Default,
    Pointer,
    Text,
}

/// Translate the host's pointer frame without passing any keyboard text to Iced.
pub fn native_pointer(
    event: &smithay_client_toolkit::seat::pointer::PointerEvent,
    scale: f32,
) -> Vec<Pointer> {
    use smithay_client_toolkit::seat::pointer::PointerEventKind as Kind;
    let motion = Pointer::Move {
        x: event.position.0 as f32 * scale,
        y: event.position.1 as f32 * scale,
    };
    match &event.kind {
        Kind::Enter { .. } | Kind::Motion { .. } => vec![motion],
        Kind::Leave { .. } => vec![Pointer::Leave],
        Kind::Press { button: 272, .. } => vec![motion, Pointer::Press],
        Kind::Release { button: 272, .. } => vec![motion, Pointer::Release],
        Kind::Axis {
            horizontal,
            vertical,
            ..
        } => vec![
            motion,
            Pointer::Scroll {
                x: -horizontal.absolute as f32 * scale,
                y: -vertical.absolute as f32 * scale,
            },
        ],
        _ => Vec::new(),
    }
}

pub fn cursor_icon(cursor: Cursor) -> smithay_client_toolkit::seat::pointer::CursorIcon {
    use smithay_client_toolkit::seat::pointer::CursorIcon;
    match cursor {
        Cursor::Default => CursorIcon::Default,
        Cursor::Pointer => CursorIcon::Pointer,
        Cursor::Text => CursorIcon::Text,
    }
}

#[derive(Default)]
struct CursorState {
    shown: Option<Cursor>,
}
impl CursorState {
    fn needs_update(&mut self, desired: Cursor, entered: bool) -> bool {
        if entered {
            self.shown = None;
        }
        self.shown != Some(desired)
    }
}
/// Bounded per-pointer cursor state. Enter events need a fresh serial even when
/// the shape matches; ordinary motion only sends a request for a new shape.
#[derive(Default)]
pub struct CursorCache {
    pointers:
        std::collections::VecDeque<(wayland_client::protocol::wl_pointer::WlPointer, CursorState)>,
}
impl CursorCache {
    pub fn set(
        &mut self,
        connection: &wayland_client::Connection,
        pointer: &smithay_client_toolkit::seat::pointer::ThemedPointer,
        event: &smithay_client_toolkit::seat::pointer::PointerEvent,
        desired: Cursor,
    ) {
        use smithay_client_toolkit::seat::pointer::PointerEventKind;
        let at = self
            .pointers
            .iter()
            .position(|(p, _)| p == pointer.pointer());
        if matches!(event.kind, PointerEventKind::Leave { .. }) {
            if let Some(at) = at {
                self.pointers.remove(at);
            }
            return;
        }
        let (key, mut state) = at
            .and_then(|at| self.pointers.remove(at))
            .unwrap_or_else(|| (pointer.pointer().clone(), CursorState::default()));
        if state.needs_update(
            desired,
            matches!(event.kind, PointerEventKind::Enter { .. }),
        ) && pointer.set_cursor(connection, cursor_icon(desired)).is_ok()
        {
            state.shown = Some(desired);
        }
        if self.pointers.len() == 16 {
            self.pointers.pop_front();
        }
        self.pointers.push_back((key, state));
    }
}

#[derive(Default)]
pub struct Interaction {
    pub controls: Vec<usize>,
    pub cursor: Cursor,
    pub selection: Option<(usize, usize)>,
    pub redraw: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct Revision {
    epoch: (u64, u64),
    prompt: Option<(u64, String, bool)>,
    hidden: bool,
    busy: bool,
    controls: Vec<(usize, String, crate::view::ControlKind, bool)>,
    revealed: bool,
}
impl Revision {
    fn matches(&self, view: &SessionView) -> bool {
        self.epoch == view.interaction_epoch
            && self
                .prompt
                .as_ref()
                .map(|(id, text, echo)| (*id, text.as_str(), *echo))
                == view
                    .prompt
                    .as_ref()
                    .map(|p| (p.id, p.text.as_str(), p.echo))
            && self.hidden == view.hide_input
            && self.busy == view.busy
            && self.revealed == view.revealed()
            && self.controls.len() == view.controls.len()
            && self.controls.iter().zip(&view.controls).all(
                |((id, label, kind, enabled), control)| {
                    *id == control.id
                        && label == &control.label
                        && kind == &control.kind
                        && *enabled == control.enabled
                },
            )
    }
    fn of(view: &SessionView) -> Self {
        Self {
            epoch: view.interaction_epoch,
            prompt: view.prompt.as_ref().map(|p| (p.id, p.text.clone(), p.echo)),
            hidden: view.hide_input,
            busy: view.busy,
            controls: view
                .controls
                .iter()
                .map(|c| (c.id, c.label.clone(), c.kind.clone(), c.enabled))
                .collect(),
            revealed: view.revealed(),
        }
    }
}

/// At most one submitted frame per native surface. Model/input updates remain
/// pending while an output is asleep; its callback wakes only that surface.
#[derive(Default)]
struct FrameGate {
    pending: bool,
    dirty: bool,
}
impl FrameGate {
    fn ready(&mut self) -> bool {
        self.pending = false;
        self.dirty
    }
    fn defer(&mut self) -> bool {
        self.dirty = true;
        self.pending
    }
    fn submitted(&mut self) {
        self.pending = true;
        self.dirty = false;
    }
}

struct Surface {
    // Drop the graphics surface before its renderer and retained native owners.
    target: Option<NativeSurface>,
    gate: FrameGate,
    software: bool,
    retry_at: Option<std::time::Instant>,
    layout: u64,
    background: Option<Background>,
    cache: user_interface::Cache,
    ui: Option<UserInterface<'static, Message, Theme, Renderer>>,
    visual_revision: Option<crate::view::VisualRevision>,
    #[cfg(test)]
    scene_builds: usize,
    hits: Vec<(usize, Rect)>,
    renderer: Renderer,
    cursor: mouse::Cursor,
    size: Size,
    scale: f32,
    hidden: Vec<String>,
    revision: Revision,
}
impl Surface {
    fn new(
        view: &SessionView,
        settings: &Settings,
        size: Size,
        scale: f32,
        hidden: &[String],
    ) -> Self {
        iced_tiny_skia::graphics::text::font_system()
            .write()
            .expect("font system")
            .raw()
            .db_mut()
            .set_sans_serif_family(settings.font.clone());
        Self {
            target: None,
            gate: FrameGate::default(),
            software: false,
            retry_at: None,
            layout: 0,
            background: None,
            cache: user_interface::Cache::new(),
            ui: None,
            visual_revision: None,
            #[cfg(test)]
            scene_builds: 0,
            hits: Vec::new(),
            renderer: software_renderer(),
            cursor: mouse::Cursor::Unavailable,
            size,
            scale,
            hidden: hidden.to_vec(),
            revision: Revision::of(view),
        }
    }
    fn synchronize(&mut self, view: &SessionView) {
        if !self.revision.matches(view) {
            let revision = Revision::of(view);
            // A release from a previous prompt must never activate a new one.
            self.cache = user_interface::Cache::new();
            self.ui = None;
            self.visual_revision = None;
            // Clear the old frame, including transient response image handles,
            // without changing the renderer/backend paired with the surface.
            iced::advanced::Renderer::reset(&mut self.renderer, Rectangle::with_size(self.size));
            self.revision = revision;
        }
    }
    fn failed(&mut self, error: &graphics::compositor::SurfaceError) {
        if *error != graphics::compositor::SurfaceError::Timeout {
            self.target = None;
            self.renderer = software_renderer();
            self.invalidate();
            self.software = true;
        }
        self.gate.dirty = true;
        self.retry_at = Some(std::time::Instant::now() + std::time::Duration::from_millis(250));
    }
    fn setup<T>(&mut self, operation: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let result = graphics_call(|| operation(self)).and_then(std::convert::identity);
        if result.is_err() {
            self.failed(&graphics::compositor::SurfaceError::Other);
        }
        result
    }
    fn defer_retry(&self, graphics: &Graphics) -> bool {
        if let Some(at) = self.retry_at.filter(|at| *at > std::time::Instant::now()) {
            // An earlier coalesced wakeup must not consume a later retry.
            graphics.redraw_at(at);
            true
        } else {
            false
        }
    }
    /// Renderer wakeups keep the retained widget tree; only a relayout
    /// request rebuilds it (reusing the widget cache).
    fn observe_layout(&mut self, graphics: &Graphics) {
        let layout = graphics.layout_generation();
        if self.layout != layout {
            self.layout = layout;
            self.visual_revision = None;
        }
    }
    fn invalidate(&mut self) {
        self.ui = None;
        self.visual_revision = None;
        self.cache = user_interface::Cache::new();
    }
    fn prepare_ui(&mut self, view: &SessionView, settings: &Settings) {
        let key = view.visual_revision();
        if self.visual_revision.as_ref() == Some(&key) && self.ui.is_some() {
            return;
        }
        let cache = self
            .ui
            .take()
            .map(UserInterface::into_cache)
            .unwrap_or_else(|| std::mem::take(&mut self.cache));
        #[cfg(test)]
        {
            self.scene_builds += 1;
        }
        let scene = view.scene(settings, self.size, self.scale, &self.hidden);
        self.hits = scene.hits.clone();
        let rest = scene.rest;
        let foreground = scene.element(self.size);
        let element: Element<'static> = if let Some(background) = &self.background {
            let mut layers = vec![
                iced::widget::image(background.handle.clone())
                    .width(self.size.width)
                    .height(self.size.height)
                    .into(),
            ];
            if let Some(clear) = background.rest.as_ref().filter(|_| rest > 0.) {
                layers.push(
                    iced::widget::image(clear.clone())
                        .width(self.size.width)
                        .height(self.size.height)
                        .opacity(rest)
                        .into(),
                );
            }
            layers.push(foreground);
            iced::widget::Stack::with_children(layers).into()
        } else {
            foreground
        };
        self.ui = Some(UserInterface::build(
            element,
            self.size,
            cache,
            &mut self.renderer,
        ));
        self.visual_revision = Some(key);
    }
    fn draw_ui(&mut self, settings: &Settings) -> iced::window::RedrawRequest {
        let ui = self.ui.as_mut().expect("prepared session widgets");
        let (state, _) = ui.update(
            &[Event::Window(iced::window::Event::RedrawRequested(
                std::time::Instant::now(),
            ))],
            self.cursor,
            &mut self.renderer,
            &mut iced::advanced::clipboard::Null,
            &mut Vec::new(),
        );
        ui.draw(
            &mut self.renderer,
            &canopa_ui::theme(settings.appearance.clone()),
            &iced::advanced::renderer::Style {
                text_color: canopa_ui::color(settings.appearance.palette().text),
            },
            self.cursor,
        );
        match state {
            user_interface::State::Updated { redraw_request, .. } => redraw_request,
            user_interface::State::Outdated => {
                self.visual_revision = None;
                iced::window::RedrawRequest::NextFrame
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct Host {
    surfaces: BTreeMap<String, Surface>,
    compositor: Option<Compositor>,
    software: Option<Compositor>,
    notifier: Option<Graphics>,
}

impl Host {
    pub(crate) fn render(
        &mut self,
        view: &SessionView,
        settings: &Settings,
        output: &str,
        mut image: image::RgbImage,
        scale: f32,
        hidden: &[String],
    ) -> Result<(image::RgbImage, Vec<(usize, Rect)>)> {
        view.tick(settings);
        view.changed();
        let (width, height) = image.dimensions();
        crate::background::dimensions(width, height)?;
        ensure!(
            scale.is_finite() && (0.2..=4.).contains(&scale),
            "invalid presentation scale"
        );
        let size = Size::new(width as f32, height as f32);
        // Surface identifiers are bounded by the native hosts' lifetimes.
        // Prune explicitly on output retirement as well as on session teardown.
        let surface = self
            .surfaces
            .entry(output.to_owned())
            .or_insert_with(|| Surface::new(view, settings, size, scale, hidden));
        surface.synchronize(view);
        if surface.size != size || surface.scale != scale || surface.hidden != hidden {
            surface.invalidate();
        }
        surface.size = size;
        surface.scale = scale;
        surface.hidden = hidden.to_vec();
        surface.prepare_ui(view, settings);
        let hits = surface.hits.clone();
        surface.draw_ui(settings);
        let mut pixels = tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| anyhow::anyhow!("presentation buffer unavailable"))?;
        let mut mask = tiny_skia::Mask::new(width, height)
            .ok_or_else(|| anyhow::anyhow!("presentation mask unavailable"))?;
        let viewport =
            iced_tiny_skia::graphics::Viewport::with_physical_size(Size::new(width, height), 1.);
        let Renderer::Secondary(renderer) = &mut surface.renderer else {
            anyhow::bail!("native presentation cannot be read back as a preview");
        };
        renderer.draw(
            &mut pixels.as_mut(),
            &mut mask,
            &viewport,
            &[Rectangle::with_size(size)],
            Color::TRANSPARENT,
        );
        for (target, source) in image
            .as_mut()
            .chunks_exact_mut(3)
            .zip(pixels.data().chunks_exact(4))
        {
            // Iced's tiny-skia renderer targets a premultiplied BGRA
            // softbuffer, including for images and text. Most of the full-screen
            // overlay is transparent: avoid blending the wallpaper again there.
            match source[3] {
                0 => {}
                255 => target.copy_from_slice(&[source[2], source[1], source[0]]),
                alpha => {
                    for channel in 0..3 {
                        target[channel] = (u32::from(source[2 - channel])
                            + (u32::from(target[channel]) * (255 - u32::from(alpha)) + 127) / 255)
                            .min(255) as u8;
                    }
                }
            }
        }
        Ok((image, hits))
    }

    fn present(
        &mut self,
        view: &SessionView,
        settings: &Settings,
        graphics: &Graphics,
        window: NativeWindow,
        frame: Frame<'_>,
    ) -> Result<()> {
        let Frame {
            key,
            output,
            width,
            height,
            scale,
            background,
        } = frame;
        crate::background::dimensions(width, height)?;
        ensure!(
            scale.is_finite() && (0.2..=4.).contains(&scale),
            "invalid presentation scale"
        );
        self.notifier = Some(graphics.clone());
        let hidden = settings.hidden_on(output);
        let size = Size::new(width as f32, height as f32);
        if !self.surfaces.contains_key(key) {
            self.surfaces.insert(
                key.to_owned(),
                Surface::new(view, settings, size, scale, hidden),
            );
        }
        // One borrow for the whole frame; the compositors are disjoint fields.
        let Some(surface) = self.surfaces.get_mut(key) else {
            unreachable!("surface inserted above");
        };
        if surface.gate.defer() {
            return Ok(());
        }
        if surface.defer_retry(graphics) {
            return Ok(());
        }
        let software = surface.software;
        let compositor = if software {
            &mut self.software
        } else {
            &mut self.compositor
        };
        if compositor.is_none() {
            let created = surface.setup(|_| {
                iced::futures::executor::block_on(Compositor::with_backend(
                    graphics::Settings {
                        vsync: false,
                        ..Default::default()
                    },
                    graphics.display(),
                    window.clone(),
                    graphics.shell(),
                    software.then_some("tiny-skia"),
                ))
                .map_err(Into::into)
            });
            let created = match created {
                Ok(created) => created,
                Err(error) => {
                    graphics.redraw_at(surface.retry_at.unwrap());
                    eprintln!("session renderer initialization deferred: {error}");
                    return Ok(());
                }
            };
            let info = created.information();
            let renderer = match &created {
                Compositor::Primary(_) => "wgpu",
                Compositor::Secondary(_) => "tiny-skia",
            };
            eprintln!(
                "session renderer: {renderer} ({}; {})",
                info.backend, info.adapter
            );
            *compositor = Some(created);
        }
        let compositor = compositor.as_mut().unwrap();
        surface.synchronize(view);
        surface.observe_layout(graphics);
        let prepared = surface.setup(|surface| {
            if let Some(target) = &mut surface.target {
                if surface.size != size {
                    compositor.configure_surface(target, width, height);
                }
            } else {
                surface.invalidate();
                surface.renderer = compositor.create_renderer();
                surface.target = Some(compositor.create_surface(window.clone(), width, height));
            }
            Ok(())
        });
        if let Err(error) = prepared {
            graphics.redraw_at(surface.retry_at.unwrap());
            eprintln!("session surface creation/configuration deferred: {error}");
            return Ok(());
        }
        if surface.size != size || surface.scale != scale || surface.hidden != hidden {
            surface.invalidate();
        }
        surface.size = size;
        surface.scale = scale;
        surface.hidden = hidden.to_vec();
        if surface.background.as_ref().map(|b| b.handle.id())
            != background.as_ref().map(|b| b.handle.id())
        {
            if let Some(background) = &background {
                ensure!(
                    background.dimensions() == (width, height),
                    "background size mismatch"
                );
            }
            surface.background = background;
            surface.visual_revision = None;
        }
        surface.prepare_ui(view, settings);
        let redraw_request = surface.draw_ui(settings);
        let viewport = graphics::Viewport::with_physical_size(Size::new(width, height), 1.);
        let color = if view.authentication_panel.as_ref().is_some_and(|p| p.dialog) {
            settings.appearance.palette().surface
        } else {
            settings.color
        };
        let background = canopa_ui::color(color);
        let target = surface.target.as_mut().unwrap();
        let on_pre_present = || graphics.request_frame(&window);
        let result = graphics_call(|| {
            let mut result = compositor.present(
                &mut surface.renderer,
                target,
                &viewport,
                background,
                on_pre_present,
            );
            if matches!(
                result,
                Err(graphics::compositor::SurfaceError::Lost
                    | graphics::compositor::SurfaceError::Outdated)
            ) {
                compositor.configure_surface(target, width, height);
                result = compositor.present(
                    &mut surface.renderer,
                    target,
                    &viewport,
                    background,
                    on_pre_present,
                );
            }
            result
        })
        .unwrap_or(Err(graphics::compositor::SurfaceError::Other));
        match result {
            Ok(()) => {
                surface.gate.submitted();
                surface.retry_at = None;
                // Motion continues on the next frame callback, and only while it runs.
                if view.animating() {
                    surface.gate.dirty = true;
                }
                match redraw_request {
                    iced::window::RedrawRequest::NextFrame => surface.gate.dirty = true,
                    iced::window::RedrawRequest::At(at) => {
                        if at <= std::time::Instant::now() {
                            surface.gate.dirty = true;
                        } else {
                            graphics.redraw_at(at);
                        }
                    }
                    iced::window::RedrawRequest::Wait => {}
                }
            }
            Err(error) => {
                // A transient timeout is common for suspended outputs. Persistent
                // GPU errors downgrade only this output, without ending auth or
                // disrupting the other outputs sharing the device.
                surface.failed(&error);
                graphics.retry();
                eprintln!("session surface presentation deferred: {error}");
            }
        }
        Ok(())
    }

    pub(crate) fn pointer(
        &mut self,
        view: &SessionView,
        _settings: &Settings,
        output: &str,
        pointer: Pointer,
    ) -> (Vec<Message>, Cursor, bool) {
        let Some(surface) = self.surfaces.get_mut(output) else {
            return (Vec::new(), Cursor::Default, false);
        };
        surface.synchronize(view);
        let event = match pointer {
            Pointer::Move { x, y } => {
                surface.cursor = mouse::Cursor::Available(Point::new(x, y));
                Event::Mouse(mouse::Event::CursorMoved {
                    position: Point::new(x, y),
                })
            }
            Pointer::Leave => {
                surface.cursor = mouse::Cursor::Unavailable;
                Event::Mouse(mouse::Event::CursorLeft)
            }
            Pointer::Press => Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Pointer::Release => Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            Pointer::Scroll { x, y } => Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x, y },
            }),
        };
        let Some(ui) = surface.ui.as_mut() else {
            return (Vec::new(), Cursor::Default, true);
        };
        let mut messages = Vec::new();
        let (state, _) = ui.update(
            &[event],
            surface.cursor,
            &mut surface.renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        let redraw = match &state {
            user_interface::State::Outdated => true,
            user_interface::State::Updated { redraw_request, .. } => match redraw_request {
                iced::window::RedrawRequest::Wait => false,
                iced::window::RedrawRequest::NextFrame => true,
                iced::window::RedrawRequest::At(at) => {
                    let due = *at <= std::time::Instant::now();
                    if !due && let Some(notifier) = &self.notifier {
                        notifier.redraw_at(*at);
                    }
                    due
                }
            },
        };
        let cursor = match state {
            user_interface::State::Updated {
                mouse_interaction: mouse::Interaction::Pointer,
                ..
            } => Cursor::Pointer,
            user_interface::State::Updated {
                mouse_interaction: mouse::Interaction::Text,
                ..
            } => Cursor::Text,
            _ => Cursor::Default,
        };
        if matches!(state, user_interface::State::Outdated) {
            surface.invalidate();
        }
        (messages, cursor, redraw)
    }

    pub(crate) fn retire(&mut self, output: &str) {
        self.surfaces.remove(output);
        if self.surfaces.is_empty() {
            // Long-running hosts (the polkit agent) can sit idle for hours
            // between dialogs: release the GPU device and its caches with the
            // last surface instead of keeping them for the process lifetime.
            self.compositor = None;
            self.software = None;
        }
    }
    fn cancel(&mut self) {
        for surface in self.surfaces.values_mut() {
            surface.invalidate();
            surface.cursor = mouse::Cursor::Unavailable;
        }
    }
}

impl SessionView {
    pub fn pointer(&self, settings: &Settings, output: &str, pointer: Pointer) -> Interaction {
        let (messages, cursor, redraw) = self
            .presentation
            .borrow_mut()
            .pointer(self, settings, output, pointer);
        let mut controls = Vec::new();
        let mut selection = None;
        for message in messages {
            match message {
                Message::Control(id) => controls.push(id),
                Message::Selection { anchor, cursor } => {
                    selection = Some((anchor, cursor));
                }
            }
        }
        Interaction {
            controls,
            cursor,
            selection,
            redraw,
        }
    }

    /// Release this surface's frame throttle, retaining pending model updates.
    pub fn frame_ready(&self, key: &str) -> bool {
        self.presentation
            .borrow_mut()
            .surfaces
            .get_mut(key)
            .is_some_and(|surface| surface.gate.ready())
    }

    /// Present directly through Iced's GPU-first compositor (software fallback).
    pub fn present(
        &self,
        settings: &Settings,
        graphics: &Graphics,
        window: NativeWindow,
        frame: Frame<'_>,
    ) -> Result<()> {
        self.presentation
            .borrow_mut()
            .present(self, settings, graphics, window, frame)
    }

    /// Drop widget state and renderer caches when an output or dialog retires.
    pub fn retire_presentation(&self, output: &str) {
        self.presentation.borrow_mut().retire(output);
        self.clear_response_images();
    }
    pub fn cancel_interactions(&self) {
        self.presentation.borrow_mut().cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        layout::{Anchor, Layout, Region},
        view::{Control, Prompt},
    };

    fn fixture() -> (Settings, SessionView) {
        let (mut settings, _) = Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Lockscreen,
        )
        .unwrap();
        settings.widgets.clear();
        settings.layout = Layout {
            regions: vec![Region {
                anchor: Anchor::Center,
                widgets: vec!["authentication".into()],
                row: false,
                x: 0.,
                y: 0.,
            }],
            ..Default::default()
        };
        let mut view = SessionView::new(&settings, "Alice".into()).unwrap();
        view.hide_input = true;
        view.controls = vec![Control::new(7, "Continue")];
        view.primary_control = Some(7);
        (settings, view)
    }
    fn frame(
        view: &SessionView,
        settings: &Settings,
        output: &str,
    ) -> (image::RgbImage, Vec<(usize, Rect)>) {
        view.render_on_output(
            settings,
            output,
            image::RgbImage::from_pixel(640, 400, image::Rgb([20, 25, 30])),
        )
        .unwrap()
    }
    fn move_to(view: &SessionView, settings: &Settings, output: &str, r: Rect) {
        view.pointer(
            settings,
            output,
            Pointer::Move {
                x: r.x + r.width / 2.,
                y: r.y + r.height / 2.,
            },
        );
    }

    #[test]
    fn sleeping_output_keeps_one_frame_and_does_not_throttle_its_peer() {
        let mut asleep = FrameGate::default();
        let mut awake = FrameGate::default();
        assert!(!asleep.defer());
        asleep.submitted();
        for _ in 0..100 {
            assert!(asleep.defer());
        }
        assert!(!awake.defer());
        awake.submitted();
        assert!(
            !awake.ready(),
            "a callback alone must not start an idle redraw loop"
        );
        assert!(
            asleep.ready(),
            "a returning output must draw its latest pending model"
        );
        assert!(!asleep.defer());
    }

    #[test]
    fn presentation_errors_retry_and_downgrade_only_the_failed_output() {
        let (settings, view) = fixture();
        frame(&view, &settings, "one");
        frame(&view, &settings, "two");
        let mut host = view.presentation.borrow_mut();
        host.surfaces
            .get_mut("one")
            .unwrap()
            .failed(&graphics::compositor::SurfaceError::Timeout);
        let failed = &host.surfaces["one"];
        assert!(
            !failed.software,
            "a timeout must preserve the existing renderer"
        );
        assert!(failed.gate.dirty && failed.retry_at.is_some());
        host.surfaces
            .get_mut("one")
            .unwrap()
            .failed(&graphics::compositor::SurfaceError::OutOfMemory);
        assert!(host.surfaces["one"].software);
        assert!(!host.surfaces["two"].software);
        assert!(host.surfaces["two"].ui.is_some());
    }

    #[test]
    fn graphics_constructor_panics_leave_other_outputs_and_authentication_alive() {
        let (settings, mut view) = fixture();
        view.secret.push("protected response");
        frame(&view, &settings, "existing");
        let mut added = Surface::new(&view, &settings, Size::new(640., 400.), 1., &[]);
        let result = added.setup::<()>(|_| panic!("Create surface"));
        assert!(result.is_err());
        assert!(added.software && added.target.is_none() && added.retry_at.is_some());
        let host = view.presentation.borrow();
        assert!(!host.surfaces["existing"].software);
        assert!(host.surfaces["existing"].ui.is_some());
        assert_eq!(view.secret.text(), "protected response");
        assert_eq!(
            graphics_call(|| Ok::<_, anyhow::Error>(7))
                .unwrap()
                .unwrap(),
            7
        );
    }

    #[test]
    fn compositor_initialization_errors_keep_the_output_eligible_for_retry() {
        let (settings, view) = fixture();
        let mut surface = Surface::new(&view, &settings, Size::new(640., 400.), 1., &[]);
        assert!(
            surface
                .setup::<()>(|_| anyhow::bail!("no graphics adapter"))
                .is_err()
        );
        assert!(surface.software && surface.gate.dirty && surface.retry_at.is_some());
        assert!(!surface.gate.pending);
        assert_eq!(surface.setup(|_| Ok(7)).unwrap(), 7);
    }

    #[test]
    fn earlier_wakeup_rearms_a_deferred_output_retry() {
        use std::time::{Duration, Instant};
        use wayland_client::{Dispatch, QueueHandle, protocol::wl_callback};
        struct App(usize);
        impl Dispatch<wl_callback::WlCallback, wayland_client::protocol::wl_surface::WlSurface> for App {
            fn event(
                _: &mut Self,
                _: &wl_callback::WlCallback,
                _: wl_callback::Event,
                _: &wayland_client::protocol::wl_surface::WlSurface,
                _: &wayland_client::Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }
        // A private socket gives Graphics a display handle without a compositor
        // or access to the desktop running these tests.
        let (client, _server) = std::os::unix::net::UnixStream::pair().unwrap();
        let connection = wayland_client::Connection::from_socket(client).unwrap();
        let queue = connection.new_event_queue::<App>();
        let mut event_loop = calloop::EventLoop::<App>::try_new().unwrap();
        let graphics = Graphics::new(&connection, &queue.handle(), &event_loop.handle(), |app| {
            app.0 += 1;
        })
        .unwrap();
        let (settings, view) = fixture();
        let mut surface = Surface::new(&view, &settings, Size::new(640., 400.), 1., &[]);
        surface.retry_at = Some(Instant::now() + Duration::from_secs(1));
        graphics.redraw_at(Instant::now());
        assert!(surface.defer_retry(&graphics));
        let mut app = App(0);
        event_loop.dispatch(Duration::ZERO, &mut app).unwrap();
        assert_eq!(app.0, 1);
        assert!(surface.defer_retry(&graphics));
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.0 < 2 && Instant::now() < deadline {
            event_loop
                .dispatch(Duration::from_millis(100), &mut app)
                .unwrap();
        }
        assert_eq!(
            app.0, 2,
            "the retry must fire without another input or clock tick"
        );

        // Redraw wakeups repaint the retained widgets; a relayout rebuilds once.
        use iced_renderer::graphics::shell::Notifier;
        surface.visual_revision = Some(view.visual_revision());
        surface.observe_layout(&graphics);
        graphics.request_redraw();
        surface.observe_layout(&graphics);
        assert!(surface.visual_revision.is_some());
        graphics.invalidate_layout();
        surface.observe_layout(&graphics);
        assert!(surface.visual_revision.is_none());
        surface.visual_revision = Some(view.visual_revision());
        surface.observe_layout(&graphics);
        assert!(surface.visual_revision.is_some());
    }

    #[test]
    fn background_clones_share_the_prepared_rgba_allocation() {
        let background =
            Background::from_rgb(image::RgbImage::from_pixel(80, 60, image::Rgb([1, 2, 3])));
        let clone = background.clone();
        assert_eq!(background.dimensions(), (80, 60));
        assert_eq!(background.handle.id(), clone.handle.id());
        let iced::widget::image::Handle::Rgba {
            pixels: original, ..
        } = &background.handle
        else {
            panic!("prepared RGBA background")
        };
        let iced::widget::image::Handle::Rgba { pixels: cloned, .. } = &clone.handle else {
            unreachable!()
        };
        assert_eq!(original.as_ptr(), cloned.as_ptr());
        assert_eq!(original.len(), 80 * 60 * 4);
        assert_eq!(&original[..4], &[1, 2, 3, 255]);
    }

    #[test]
    fn cursor_changes_only_on_enter_or_a_new_shape() {
        let mut state = CursorState::default();
        assert!(state.needs_update(Cursor::Default, true));
        state.shown = Some(Cursor::Default);
        for _ in 0..100 {
            assert!(!state.needs_update(Cursor::Default, false));
        }
        assert!(state.needs_update(Cursor::Text, false));
        state.shown = Some(Cursor::Text);
        assert!(!state.needs_update(Cursor::Text, false));
        assert!(state.needs_update(Cursor::Text, true));
        assert!(
            state.needs_update(Cursor::Text, false),
            "a failed cursor request must remain retryable"
        );
    }

    #[test]
    fn pointer_motion_reuses_prepared_widgets_and_does_not_redraw_unchanged_hover() {
        let (settings, mut view) = fixture();
        view.hide_input = false;
        view.prompt = Some(Prompt {
            id: 1,
            text: "Password".into(),
            echo: false,
        });
        view.secret.push("private-fixture");
        frame(&view, &settings, "");
        let builds = view.presentation.borrow().surfaces[""].scene_builds;
        for _ in 0..100 {
            view.pointer(&settings, "", Pointer::Move { x: 1., y: 1. });
        }
        assert!(
            !view
                .pointer(&settings, "", Pointer::Move { x: 2., y: 1. })
                .redraw
        );
        view.presentation
            .borrow_mut()
            .surfaces
            .get_mut("")
            .unwrap()
            .prepare_ui(&view, &settings);
        assert_eq!(view.presentation.borrow().surfaces[""].scene_builds, builds);

        view.secret.clear();
        view.secret.push("changed-fixture");
        frame(&view, &settings, "");
        assert_eq!(
            view.presentation.borrow().surfaces[""].scene_builds,
            builds + 1
        );
    }

    #[test]
    fn weather_observation_invalidates_only_when_displayed_data_changes() {
        let (mut settings, view) = fixture();
        settings.clock_format = "fixed clock".into();
        settings.date_format = "fixed date".into();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("weather.json");
        settings.weather_file = Some(path.clone());
        let mut weather = canopa_widgets::weather::Weather {
            temperature_celsius: 18.,
            description: "Clear".into(),
            updated_unix: chrono::Utc::now().timestamp() as u64,
        };
        std::fs::write(&path, serde_json::to_vec(&weather).unwrap()).unwrap();
        assert!(view.tick(&settings));
        frame(&view, &settings, "");
        let revision = view.visual_revision();
        let builds = view.presentation.borrow().surfaces[""].scene_builds;
        for _ in 0..10 {
            assert!(!view.tick(&settings));
            view.presentation
                .borrow_mut()
                .surfaces
                .get_mut("")
                .unwrap()
                .prepare_ui(&view, &settings);
        }
        assert_eq!(view.visual_revision(), revision);
        assert_eq!(view.presentation.borrow().surfaces[""].scene_builds, builds);
        weather.description = "Cloudy".into();
        std::fs::write(&path, serde_json::to_vec(&weather).unwrap()).unwrap();
        view.presentation
            .borrow_mut()
            .surfaces
            .get_mut("")
            .unwrap()
            .prepare_ui(&view, &settings);
        assert_eq!(
            view.presentation.borrow().surfaces[""].scene_builds,
            builds,
            "present must not read the weather file"
        );
        assert!(view.tick(&settings));
        view.presentation
            .borrow_mut()
            .surfaces
            .get_mut("")
            .unwrap()
            .prepare_ui(&view, &settings);
        assert_eq!(
            view.presentation.borrow().surfaces[""].scene_builds,
            builds + 1
        );
    }

    #[test]
    fn failure_and_caps_warnings_stay_between_the_field_and_buttons() {
        let (settings, mut view) = fixture();
        view.hide_input = false;
        view.prompt = Some(Prompt {
            id: 1,
            text: "Password".into(),
            echo: false,
        });
        view.message = "Authentication failed".into();
        view.failed = true;
        view.caps = true;
        let (image, hits) = view
            .render_on_output(&settings, "", image::RgbImage::new(1280, 800))
            .unwrap();
        let field = hits
            .iter()
            .find(|(id, _)| *id == crate::input::INPUT_CONTROL)
            .unwrap()
            .1;
        let button = hits.iter().find(|(id, _)| *id == 7).unwrap().1;
        let palette = settings.appearance.palette();
        for color in [palette.warning, palette.danger] {
            let rows: Vec<_> = image
                .enumerate_pixels()
                .filter(|(_, y, p)| {
                    *y as f32 >= field.y + field.height && p.0 == [color.0, color.1, color.2]
                })
                .map(|(_, y, _)| y as f32)
                .collect();
            assert!(
                !rows.is_empty(),
                "warning text must remain visible: {color:?}"
            );
            assert!(
                rows.iter().all(|y| *y < button.y),
                "warnings must stay next to the field"
            );
        }
    }

    #[test]
    fn buttons_show_hover_and_pressed_feedback_and_activate_on_release() {
        let (settings, view) = fixture();
        let (rest, hits) = frame(&view, &settings, "");
        let rect = hits[0].1;
        move_to(&view, &settings, "", rect);
        let hover = frame(&view, &settings, "").0;
        assert!(rest != hover, "hover must be visible without a click");
        assert!(
            view.pointer(&settings, "", Pointer::Press)
                .controls
                .is_empty()
        );
        let pressed = frame(&view, &settings, "").0;
        assert!(rest != pressed, "pressed feedback must be visible");
        assert_eq!(
            view.pointer(&settings, "", Pointer::Release).controls,
            vec![7]
        );
        assert!(
            view.pointer(&settings, "", Pointer::Release)
                .controls
                .is_empty()
        );
    }

    #[test]
    fn cancelled_outside_cross_output_and_stale_releases_do_not_activate() {
        let (settings, mut view) = fixture();
        let rect = frame(&view, &settings, "one").1[0].1;
        frame(&view, &settings, "two");
        move_to(&view, &settings, "one", rect);
        view.pointer(&settings, "one", Pointer::Press);
        move_to(&view, &settings, "two", rect);
        assert!(
            view.pointer(&settings, "two", Pointer::Release)
                .controls
                .is_empty()
        );
        view.pointer(&settings, "one", Pointer::Move { x: 0., y: 0. });
        assert!(
            view.pointer(&settings, "one", Pointer::Release)
                .controls
                .is_empty()
        );
        move_to(&view, &settings, "one", rect);
        view.pointer(&settings, "one", Pointer::Press);
        view.interaction_epoch = (2, 0);
        assert!(
            view.pointer(&settings, "one", Pointer::Release)
                .controls
                .is_empty()
        );
        view.pointer(&settings, "one", Pointer::Press);
        view.cancel_interactions();
        assert!(
            view.pointer(&settings, "one", Pointer::Release)
                .controls
                .is_empty()
        );
        view.retire_presentation("one");
        assert_eq!(view.presentation.borrow().surfaces.len(), 1);
    }

    #[test]
    fn disabled_controls_cancel_existing_presses_and_oversized_choosers_scroll() {
        // The footer places its default action by its label width and text direction.
        let _english = crate::i18n::english();
        let (settings, mut view) = fixture();
        let rect = frame(&view, &settings, "").1[0].1;
        move_to(&view, &settings, "", rect);
        view.pointer(&settings, "", Pointer::Press);
        view.controls[0].enabled = false;
        assert!(
            view.pointer(&settings, "", Pointer::Release)
                .controls
                .is_empty()
        );
        view.pointer(&settings, "", Pointer::Press);
        assert!(
            view.pointer(&settings, "", Pointer::Release)
                .controls
                .is_empty()
        );

        view.authentication_panel = Some(crate::view::AuthenticationPanel {
            height_reserve: 0.,
            heading: "Choose account".into(),
            subtitle: String::new(),
            emblem: crate::view::Emblem::Accounts,
            dialog: false,
            choices: &[],
            utilities: &[],
            disclosure: None,
            list_actions: true,
            checked_control: None,
            choice_details: Default::default(),
            account: None,
            confirm_identity: false,
            facts: Vec::new(),
        });
        view.controls = (0..30)
            .map(|id| Control::new(id, format!("Account {id}")))
            .collect();
        view.primary_control = None;
        let first = frame(&view, &settings, "").0;
        view.pointer(&settings, "", Pointer::Move { x: 320., y: 200. });
        view.pointer(&settings, "", Pointer::Scroll { x: 0., y: -240. });
        let second = frame(&view, &settings, "").0;
        assert!(
            first != second,
            "long choosers must scroll within the output"
        );

        let panel = view.authentication_panel.as_mut().unwrap();
        panel.dialog = true;
        panel.list_actions = false;
        panel.account = Some("Alice".into());
        view.custom.insert("subject".into(), "Request".into());
        view.custom
            .insert("message-0".into(), "Request detail ".repeat(250));
        view.message = "Status detail ".repeat(250);
        view.controls = vec![Control::new(42, "Continue")];
        frame(&view, &settings, "");
        view.pointer(&settings, "", Pointer::Move { x: 320., y: 200. });
        view.pointer(&settings, "", Pointer::Scroll { x: 0., y: -10000. });
        frame(&view, &settings, "");
        // The default action sits at the footer's trailing edge.
        view.pointer(&settings, "", Pointer::Move { x: 417., y: 372. });
        view.pointer(&settings, "", Pointer::Press);
        assert_eq!(
            view.pointer(&settings, "", Pointer::Release).controls,
            vec![42],
            "overflow dialogs must keep their footer actions reachable by pointer"
        );
    }

    #[test]
    fn scrolling_sheets_keep_corner_controls_fixed_and_clickable() {
        let (settings, mut view) = fixture();
        view.authentication_panel = Some(crate::view::AuthenticationPanel {
            height_reserve: 0.,
            heading: "Choose".into(),
            subtitle: String::new(),
            emblem: crate::view::Emblem::Accounts,
            dialog: false,
            choices: &[],
            utilities: &[],
            disclosure: None,
            list_actions: true,
            checked_control: None,
            choice_details: Default::default(),
            account: None,
            confirm_identity: false,
            facts: Vec::new(),
        });
        view.primary_control = None;
        view.can_select_layout = true;
        view.layout = "English".into();
        view.controls = (0..30)
            .map(|id| Control::new(id, format!("Choice {id}")))
            .collect();
        view.controls.push(Control::new(31, "Desktop").with_kind(
            crate::view::ControlKind::DesktopChooser {
                session: "Canopa".into(),
            },
        ));
        view.controls
            .push(Control::new(32, "Power").with_kind(crate::view::ControlKind::Power));
        let hits = frame(&view, &settings, "").1;
        let corner = hits
            .iter()
            .find(|(id, _)| *id == 31)
            .expect("desktop corner")
            .1;
        assert!(
            corner.y > 300. && corner.y + corner.height <= 400.,
            "corner must stay inside the viewport: {corner:?}"
        );
        view.pointer(&settings, "", Pointer::Move { x: 320., y: 180. });
        view.pointer(&settings, "", Pointer::Scroll { x: 0., y: -1000. });
        frame(&view, &settings, "");
        move_to(&view, &settings, "", corner);
        view.pointer(&settings, "", Pointer::Press);
        assert_eq!(
            view.pointer(&settings, "", Pointer::Release).controls,
            vec![31]
        );
    }

    #[test]
    fn masked_secret_pointer_selection_reports_only_complete_storage_boundaries() {
        let (settings, mut view) = fixture();
        view.hide_input = false;
        view.prompt = Some(Prompt {
            id: 1,
            text: "Password".into(),
            echo: false,
        });
        view.secret.push("ae\u{301}👩‍💻z");
        let rect = frame(&view, &settings, "")
            .1
            .iter()
            .find(|(id, _)| *id == crate::input::INPUT_CONTROL)
            .unwrap()
            .1;
        let (x, y) = (rect.x + 8., rect.y + rect.height / 2.);
        assert_eq!(
            view.pointer(&settings, "", Pointer::Move { x, y }).cursor,
            Cursor::Text
        );
        let selected = view
            .pointer(&settings, "", Pointer::Press)
            .selection
            .expect("caret selection");
        assert!(view.secret.set_selection(selected.0, selected.1));
        let dragged = view
            .pointer(&settings, "", Pointer::Move { x: x + 80., y })
            .selection
            .expect("drag selection");
        assert!(view.secret.set_selection(dragged.0, dragged.1));
        assert_eq!(view.secret.text(), "ae\u{301}👩‍💻z");
        view.prompt = Some(Prompt {
            id: 2,
            text: "One-time code".into(),
            echo: true,
        });
        assert!(
            view.pointer(&settings, "", Pointer::Move { x, y })
                .selection
                .is_none()
        );
    }
}
