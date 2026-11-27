use crate::{background, secret::Secret, settings::Settings};
use anyhow::Result;
use canopa_theme::Rgb;
use image::RgbImage;

mod iced;
mod panel;
mod raster;
mod style;
pub use panel::{
    AUTHORIZATION_DIALOG_SIZE, AuthenticationPanel, DIALOG_MARGIN, DIALOG_MAX_HEIGHT,
    DIALOG_MIN_HEIGHT, DIALOG_SIZE, account_label, initials,
};
pub use raster::Emblem;

pub const REQUEST_PAGE_LINES: usize = 6;

#[derive(Debug)]
pub struct Prompt {
    pub id: u64,
    pub text: String,
    pub echo: bool,
}

/// What a control means, so the shared view lays it out without reading its
/// (translated) label.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ControlKind {
    #[default]
    Action,
    /// Returns to the previous screen; drawn with a back arrow.
    Back,
    /// Opens the account chooser; `text` is the short label drawn on the row.
    AccountChooser { text: String },
    /// Opens the desktop chooser; `session` is the selected desktop's name.
    DesktopChooser { session: String },
    /// Chooses the identity that authenticates, drawn with its avatar.
    Identity,
    /// Opens the power menu; drawn as a round button with the power symbol.
    Power,
    /// A large choice drawn with its icon above the label (power actions).
    Tile(Emblem),
    /// An action that cannot be undone, drawn in the danger colour.
    Destructive,
    /// Signs in as a listed account; `selected` marks the current one, which
    /// shows the consumer's avatar.
    Account {
        name: String,
        detail: Option<String>,
        initials: String,
    },
}

#[derive(Clone, Debug)]
pub struct Control {
    pub id: usize,
    /// The accessible name, and the visible text unless `kind` draws its own.
    pub label: String,
    pub kind: ControlKind,
    /// Keyboard selection (focus ring) in surfaces without pointer focus.
    pub selected: bool,
    pub enabled: bool,
}

impl Control {
    pub fn new(id: usize, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            kind: ControlKind::Action,
            selected: false,
            enabled: true,
        }
    }

    pub fn with_kind(mut self, kind: ControlKind) -> Self {
        self.kind = kind;
        self
    }
}

pub(crate) type VisualRevision = (u64, (usize, u64), usize, usize, bool, u64);
#[derive(PartialEq)]
struct Observed {
    clock: String,
    date: String,
    weather: Option<String>,
}
impl Observed {
    fn read(settings: &Settings) -> Self {
        let now = chrono::Local::now();
        Self {
            clock: settings.clock_text(&now),
            date: settings.date_text(&now),
            weather: settings
                .weather_file
                .as_deref()
                .and_then(|path| {
                    canopa_widgets::weather::Weather::read(
                        path,
                        chrono::Utc::now().timestamp().max(0) as u64,
                    )
                })
                .map(|w| w.label()),
        }
    }
}

pub struct SessionView {
    scene_revision: std::cell::Cell<u64>,
    observed: std::cell::RefCell<Observed>,
    text_renderer: std::rc::Rc<std::cell::RefCell<crate::text::Text>>,
    assets: std::rc::Rc<std::cell::RefCell<iced::Assets>>,
    pub(crate) presentation: std::cell::RefCell<crate::presentation::Host>,
    avatar: Option<image::RgbaImage>,
    user_avatar: Option<std::sync::Arc<image::RgbaImage>>,
    avatar_initials: Option<String>,
    pub controls: Vec<Control>,
    pub primary_control: Option<usize>,
    pub authentication_panel: Option<AuthenticationPanel>,
    /// Pair controls when the consumer has a dedicated settings surface.
    pub pair_controls: bool,
    pub username: String,
    pub secret: Secret,
    pub busy: bool,
    /// Confirmation/message dialogs retain controls without a response field.
    pub hide_input: bool,
    /// Draw only the background: the neutral frame a surface leaves on screen
    /// when it hands the display to the next program.
    pub blank: bool,
    pub prompt: Option<Prompt>,
    pub message: String,
    pub failed: bool,
    pub caps: bool,
    pub layout: String,
    pub can_select_layout: bool,
    /// Consumer-owned generation for asynchronous actions; contains no response data.
    pub interaction_epoch: (u64, u64),
    pub action_labels: Option<(String, String)>,
    pub battery: Option<canopa_widgets::battery::BatteryState>,
    pub media: Option<String>,
    /// Lock-screen notification lines from Notify, display text only.
    pub notifications: Option<String>,
    pub custom: std::collections::BTreeMap<String, String>,
    /// The prompt whose secret response is shown; any other prompt is masked.
    reveal: Option<u64>,
    motion: crate::motion::Motion,
    /// The full-screen submit action's last label, kept while the system checks
    /// so the button keeps its size with a spinner in it.
    pub(crate) last_submit: std::cell::RefCell<Option<String>>,
    /// The keyboard layout control is shown but unavailable (a consumer's
    /// panel covers the form).
    pub layout_unavailable: bool,
}
impl SessionView {
    pub fn new(settings: &Settings, username: String) -> Result<Self> {
        let text_renderer = std::rc::Rc::new(std::cell::RefCell::new(crate::text::Text::new(
            settings.font.clone(),
        )?));
        let assets = std::rc::Rc::new(std::cell::RefCell::new(iced::Assets::with_text(
            text_renderer.clone(),
        )));
        Ok(Self {
            scene_revision: std::cell::Cell::new(1),
            observed: std::cell::RefCell::new(Observed::read(settings)),
            presentation: Default::default(),
            assets,
            text_renderer,
            avatar: settings
                .avatar
                .as_deref()
                .and_then(|path| background::load(path).ok())
                .map(|image| image::imageops::thumbnail(&image, 256, 256)),
            user_avatar: None,
            avatar_initials: None,
            controls: Vec::new(),
            primary_control: None,
            authentication_panel: None,
            pair_controls: false,
            username,
            secret: Secret::default(),
            busy: false,
            hide_input: false,
            blank: false,
            prompt: None,
            message: String::new(),
            failed: false,
            caps: false,
            layout: String::new(),
            can_select_layout: false,
            interaction_epoch: (0, 0),
            action_labels: None,
            battery: None,
            media: None,
            notifications: None,
            custom: Default::default(),
            reveal: None,
            last_submit: Default::default(),
            layout_unavailable: false,
            motion: {
                let motion = crate::motion::Motion::default();
                motion.set_reduced(settings.appearance.reduced_motion());
                motion
            },
        })
    }
    /// Whether the settings name a picture for this surface.
    pub fn has_configured_avatar(&self) -> bool {
        self.avatar.is_some()
    }
    /// The surface's motion: hosts render while it animates; previews fix its clock.
    pub fn motion(&self) -> &crate::motion::Motion {
        &self.motion
    }
    /// Whether the next frame differs from the current one by motion alone.
    pub fn animating(&self) -> bool {
        self.observe_motion();
        self.motion.animating()
    }
    fn observe_motion(&self) {
        use std::hash::{Hash, Hasher};
        let mut stage = std::collections::hash_map::DefaultHasher::new();
        if let Some(panel) = &self.authentication_panel {
            panel.heading.hash(&mut stage);
            panel.list_actions.hash(&mut stage);
            panel.facts.is_empty().hash(&mut stage);
            panel.confirm_identity.hash(&mut stage);
        }
        self.hide_input.hash(&mut stage);
        self.blank.hash(&mut stage);
        self.motion.observe(
            stage.finish(),
            self.failed,
            self.busy,
        );
    }
    /// Show or mask the current secret response. Returns false when no masked
    /// response is being entered. A new prompt is always masked again.
    pub fn toggle_reveal(&mut self) -> bool {
        let Some(prompt) = self.prompt.as_ref().filter(|p| !p.echo && !self.hide_input) else {
            return false;
        };
        self.reveal = if self.revealed() {
            None
        } else {
            Some(prompt.id)
        };
        true
    }
    /// Whether the current secret response is shown as typed.
    pub fn revealed(&self) -> bool {
        self.prompt
            .as_ref()
            .is_some_and(|p| !p.echo && !self.hide_input && self.reveal == Some(p.id))
    }
    /// Whether the reveal control is offered for the current prompt.
    pub fn can_reveal(&self) -> bool {
        self.prompt.as_ref().is_some_and(|p| !p.echo) && !self.hide_input
    }
    /// A fresh backend challenge always starts masked, even if it reuses an ID.
    pub fn mask_response(&mut self) {
        self.reveal = None;
        self.assets.borrow_mut().clear_fields();
        self.cancel_interactions();
    }
    /// The consumer owns asynchronous loading, selection and stale-result policy.
    pub fn set_user_avatar(&mut self, image: Option<std::sync::Arc<image::RgbaImage>>) {
        let unchanged = match (&self.user_avatar, &image) {
            (Some(old), Some(new)) => std::sync::Arc::ptr_eq(old, new),
            (None, None) => true,
            _ => false,
        };
        if unchanged {
            return;
        }
        self.assets.borrow_mut().clear_avatar();
        self.user_avatar = image;
        self.changed();
    }
    pub fn set_avatar_initials(&mut self, initials: Option<String>) {
        let initials = initials.map(|s| s.chars().take(2).collect());
        if self.avatar_initials != initials {
            self.avatar_initials = initials;
            self.changed();
        }
    }
    /// Draw a passive label in consumer-owned content, using shared shaping/fallback.
    pub fn draw_label(
        &self,
        image: &mut RgbImage,
        text: &str,
        position: (f32, f32),
        size: f32,
        max_width: f32,
        color: Rgb,
    ) {
        self.text(image, text, position, size, max_width, color);
    }
    pub fn preview(settings: &Settings, username: String) -> Result<Self> {
        let mut ui = Self::new(settings, username)?;
        ui.prompt = Some(Prompt {
            id: 0,
            text: fl!("session-password-prompt"),
            echo: false,
        });
        ui.custom = settings
            .custom
            .iter()
            .filter(|(_, w)| !w.preview.is_empty())
            .map(|(n, w)| (n.clone(), w.preview.clone()))
            .collect();
        Ok(ui)
    }
    pub fn render(
        &self,
        settings: &Settings,
        name: &str,
        width: u32,
        height: u32,
    ) -> Result<RgbImage> {
        let image = Self::prepare_background(settings, name, width, height)?;
        Ok(self.render_on_output(settings, name, image)?.0)
    }
    pub fn prepare_background(
        settings: &Settings,
        name: &str,
        width: u32,
        height: u32,
    ) -> Result<RgbImage> {
        let bg = settings.for_output(name);
        // Match a desktop that shows the embedded wallpaper on this output.
        let built_in = bg.image.is_none()
            && (settings.built_in_wallpaper.contains(name)
                || settings.built_in_everywhere
                    && background::shows_built_in(settings.solid_color, &settings.appearance));
        if bg.image.is_none()
            && !built_in
            && !settings.solid_color
            && settings.color == settings.appearance.palette().background
        {
            return raster::backdrop(&settings.appearance, width, height);
        }
        background::dimensions(width, height)?;
        let source = if built_in {
            Some(background::built_in(&settings.appearance)?)
        } else {
            bg.image.as_deref().map(background::load).transpose()?
        };
        let mut image =
            background::render_source(source.as_ref(), bg.fit, settings.color, width, height)?;
        if source.is_some() {
            background::soften(
                &mut image,
                f32::from(settings.blur) * (width as f32 / 1280.).min(height as f32 / 800.),
            );
            // A gentle scrim keeps the clock and the bottom controls legible
            // over bright parts of a picture.
            let dark = settings.appearance.color_scheme == canopa_theme::ColorScheme::Dark;
            let shade = if dark { 0. } else { 255. };
            for (y, row) in image.rows_mut().enumerate() {
                let t = y as f32 / height as f32;
                let weight = 0.22 * (1. - t / 0.35).max(0.).powi(2)
                    + 0.30 * ((t - 0.72) / 0.28).max(0.).powi(2);
                for pixel in row {
                    for channel in pixel.0.iter_mut() {
                        *channel = (f32::from(*channel) * (1. - weight) + shade * weight) as u8;
                    }
                }
            }
            for pixel in image.pixels_mut() {
                for (channel, color) in
                    pixel
                        .0
                        .iter_mut()
                        .zip([settings.color.0, settings.color.1, settings.color.2])
                {
                    *channel = ((u16::from(*channel) * u16::from(100 - settings.dim)
                        + u16::from(color) * u16::from(settings.dim))
                        / 100) as u8;
                }
            }
        }
        Ok(image)
    }
    pub fn render_over(&self, settings: &Settings, image: RgbImage) -> Result<RgbImage> {
        Ok(self.render_with_hits(settings, image)?.0)
    }
    pub fn render_with_hits(
        &self,
        settings: &Settings,
        image: RgbImage,
    ) -> Result<(RgbImage, Vec<(usize, crate::layout::Rect)>)> {
        self.render_on_output(settings, "", image)
    }

    pub fn render_on_output(
        &self,
        settings: &Settings,
        output: &str,
        image: RgbImage,
    ) -> Result<(RgbImage, Vec<(usize, crate::layout::Rect)>)> {
        self.render_on_surface(settings, output, output, image)
    }

    /// Native hosts identify widget state by the surface, independently of the
    /// optional output name used for configuration and per-output visibility.
    pub fn render_on_surface(
        &self,
        settings: &Settings,
        surface: &str,
        output: &str,
        image: RgbImage,
    ) -> Result<(RgbImage, Vec<(usize, crate::layout::Rect)>)> {
        let (width, height) = image.dimensions();
        let scale = (width as f32 / 1280.)
            .min(height as f32 / 800.)
            .clamp(0.2, 3.);
        self.render_filtered(settings, surface, image, scale, settings.hidden_on(output))
    }

    /// Ordinary windows choose their logical scale; fullscreen consumers derive it above.
    pub fn render_at_scale(
        &self,
        settings: &Settings,
        image: RgbImage,
        scale: f32,
    ) -> Result<(RgbImage, Vec<(usize, crate::layout::Rect)>)> {
        self.render_filtered(settings, "", image, scale, &[])
    }

    pub fn render_dialog(
        &self,
        settings: &Settings,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<(RgbImage, Vec<(usize, crate::layout::Rect)>)> {
        background::dimensions(width, height)?;
        let Rgb(r, g, b) = settings.appearance.palette().surface;
        self.render_at_scale(
            settings,
            RgbImage::from_pixel(width, height, image::Rgb([r, g, b])),
            scale,
        )
    }

    fn render_filtered(
        &self,
        settings: &Settings,
        output: &str,
        image: RgbImage,
        scale: f32,
        hidden: &[String],
    ) -> Result<(RgbImage, Vec<(usize, crate::layout::Rect)>)> {
        self.presentation
            .borrow_mut()
            .render(self, settings, output, image, scale, hidden)
    }

    pub(crate) fn scene(
        &self,
        settings: &Settings,
        size: ::iced::Size,
        scale: f32,
        hidden: &[String],
    ) -> iced::Scene {
        use crate::layout::Item;
        if self.blank {
            return iced::Scene::new(self.assets.clone());
        }
        let (width, height) = (size.width, size.height);
        let palette = settings.appearance.palette();
        let observed = self.observed.borrow();
        let mut content = std::collections::BTreeMap::new();
        content.insert(
            "clock".to_owned(),
            Content::Text {
                lines: vec![observed.clock.clone()],
                size: settings.clock_size as f32,
                color: palette.text,
                weight: cosmic_text::Weight::LIGHT,
                icon: None,
            },
        );
        content.insert(
            "date".to_owned(),
            Content::Text {
                lines: vec![observed.date.clone()],
                size: settings.text_size as f32,
                color: raster::mix(palette.muted, palette.text, 0.3),
                weight: cosmic_text::Weight::MEDIUM,
                icon: None,
            },
        );
        content.insert("authentication".to_owned(), Content::Authentication);
        content.retain(|name, _| name == "authentication" || !hidden.contains(name));
        for widget in settings
            .widgets
            .iter()
            .filter(|name| !hidden.contains(name))
        {
            if self.authentication_panel.as_ref().is_some_and(|panel| {
                panel.dialog
                    && matches!(
                        widget.as_str(),
                        "heading"
                            | "subject"
                            | "request-title"
                            | "message-0"
                            | "message-1"
                            | "action"
                            | "account"
                            | "subject-process"
                            | "subject-name"
                            | "policy-publisher"
                            | "avatar"
                    )
            }) {
                continue;
            }
            let value = match widget.as_str() {
                "username" => Some(self.username.clone()),
                "keyboard" => Some(self.layout.clone()).filter(|s| !s.is_empty()),
                "battery" => self.battery.as_ref().filter(|b| b.present).map(|b| {
                    let level = canopa_i18n::format::percent(b.percentage.clamp(0., 100.).round());
                    if b.power_state.is_charging() {
                        fl!("session-battery-charging", level = level)
                    } else {
                        level
                    }
                }),
                "media" => self.media.clone(),
                "notifications" => self.notifications.clone(),
                "weather" => observed.weather.clone(),
                "avatar" => {
                    if self.avatar.is_some()
                        || self.user_avatar.is_some()
                        || self.avatar_initials.is_some()
                    {
                        content.insert(widget.clone(), Content::Avatar);
                    }
                    None
                }
                _ => self.custom.get(widget).cloned(),
            };
            if let Some(value) = value.filter(|s| !s.is_empty()) {
                let definition = settings.custom.get(widget);
                let text =
                    definition.map_or(value.clone(), |w| w.format.replace("{output}", &value));
                let size = definition.map_or(15., |w| w.size as f32);
                let default_color = if widget == "keyboard" && self.authentication_panel.is_some() {
                    palette.text
                } else {
                    palette.muted
                };
                let color = definition.map_or(default_color, |w| match w.color.as_str() {
                    "text" => palette.text,
                    "accent" => palette.accent,
                    "warning" => palette.warning,
                    "danger" => palette.danger,
                    "success" => palette.success,
                    _ => Rgb::parse_hex(&w.color).unwrap_or(palette.muted),
                });
                content.insert(
                    widget.clone(),
                    Content::Text {
                        lines: wrap(&text, 64, 4),
                        size,
                        color,
                        weight: cosmic_text::Weight::NORMAL,
                        icon: match widget.as_str() {
                            "battery" => Some(Emblem::Battery),
                            "keyboard" => Some(Emblem::Keyboard),
                            _ => None,
                        },
                    },
                );
            }
        }
        let prompt = self.prompt.as_ref().filter(|_| !self.hide_input);
        let label = prompt.map(|prompt| wrap(prompt.text.trim_end_matches(':'), 56, 8));
        let label_height = label.as_ref().map_or(0., |lines| 21. * lines.len() as f32);
        let hints = self.authentication_hints();
        let layout_control = self.layout_control();
        let input_offset = if self.hide_input { 52. } else { 0. };
        let columns = if self.pair_controls { 2 } else { 1 };
        let control_rows =
            (self.controls.len() + usize::from(layout_control.is_some())).div_ceil(columns);
        let auth_height = 70. - input_offset
            + label_height
            + 20. * hints.len().max(1) as f32
            + 42. * control_rows as f32;
        // Adjacent identity widgets share the authentication header in a column.
        // Independently placed widgets and per-output visibility remain intact.
        let auth_region = settings.layout.regions.iter().find(|region| {
            !region.row && region.widgets.iter().any(|name| name == "authentication")
        });
        let inline_identity = auth_region
            .and_then(|region| {
                region
                    .widgets
                    .windows(2)
                    .find(|pair| pair[1] == "authentication")
            })
            .map(|pair| pair[0].as_str());
        let pin_origin = auth_region.is_some_and(|region| {
            use crate::layout::Anchor;
            matches!(
                region.anchor,
                Anchor::Center | Anchor::CenterLeft | Anchor::CenterRight
            ) && region
                .widgets
                .iter()
                .any(|name| name == "clock" || name == "date")
        });
        let corners_free = !settings.layout.regions.iter().any(|region| {
            use crate::layout::Anchor;
            matches!(region.anchor, Anchor::BottomLeft | Anchor::BottomRight)
                && region
                    .widgets
                    .iter()
                    .any(|name| content.contains_key(name) || settings.widgets.contains(name))
        });
        let mut panel = self.authentication_panel.as_ref().map(|panel| {
            let username =
                inline_identity == Some("username") && content.remove("username").is_some();
            let avatar = inline_identity == Some("avatar") && content.remove("avatar").is_some();
            let keyboard_inline = !panel.dialog
                && auth_region.is_some_and(|region| {
                    region
                        .widgets
                        .windows(2)
                        .any(|pair| pair[0] == "authentication" && pair[1] == "keyboard")
                })
                && content.remove("keyboard").is_some();
            panel.plan(
                self,
                layout_control.as_ref(),
                keyboard_inline,
                username,
                avatar,
                pin_origin,
                corners_free,
                height / scale,
            )
        });
        // A battery level at the bottom centre joins the corner controls.
        if let Some(plan) = panel.as_mut()
            && settings.layout.regions.iter().any(|region| {
                region.anchor == crate::layout::Anchor::BottomCenter
                    && region.widgets.iter().any(|name| name == "battery")
            })
            && let Some(Content::Text { lines, .. }) = content.get("battery")
            && plan.add_status(
                Emblem::Battery,
                lines.concat(),
                self.battery
                    .as_ref()
                    .map(|battery| (battery.percentage / 100.).clamp(0., 1.) as f32),
            )
        {
            content.remove("battery");
        }
        let items: Vec<_> = content
            .iter()
            .map(|(name, content)| {
                let (w, h) = match content {
                    Content::Text {
                        lines, size, icon, ..
                    } => (
                        lines
                            .iter()
                            .map(|s| self.clock_width(s, *size, name == "clock"))
                            .fold(0., f32::max)
                            .max(1.)
                            + if icon.is_some() { size * 1.6 } else { 0. },
                        (size + 7.) * lines.len() as f32,
                    ),
                    Content::Avatar => (64., 64.),
                    Content::Authentication => {
                        panel.as_ref().map_or((400., auth_height), |panel| {
                            panel.size((height / scale - 2. * settings.layout.margin).max(1.))
                        })
                    }
                };
                Item {
                    name: name.clone(),
                    width: w * scale,
                    height: h * scale,
                }
            })
            .collect();
        let placements = settings.layout.plan(width, height, scale, &items);
        let mut scene = iced::Scene::new(self.assets.clone());
        let dialog = self
            .authentication_panel
            .as_ref()
            .is_some_and(|panel| panel.dialog);
        scene.glass = !dialog;
        scene.glass_light = settings.appearance.color_scheme == canopa_theme::ColorScheme::Light;
        self.observe_motion();
        let motion = self.motion.frame();
        // The compositor animates dialog windows opening; full-screen surfaces
        // bring their own content in.
        if !dialog {
            scene.alpha = motion.opacity;
            scene.offset = (0., motion.rise * scale);
        }
        scene.base = (scene.alpha, scene.offset);
        scene.shake = motion.shake * scale;
        scene.spinner = motion.spinner;
        // A chosen account confirms with one action: its list shows focus.
        scene.field_focus = self.prompt.is_some()
            && !self.hide_input
            && !self
                .authentication_panel
                .as_ref()
                .is_some_and(|panel| panel.confirm_identity);
        for placement in placements {
            let bounds = placement.bounds;
            let scale = scale * placement.scale;
            match &content[&placement.name] {
                Content::Text {
                    lines,
                    size,
                    color,
                    weight,
                    icon,
                } => {
                    let mut text_bounds = bounds;
                    if let Some(icon) = icon {
                        let side = size * 1.2 * scale;
                        scene.icon(
                            *icon,
                            crate::layout::Rect {
                                x: if canopa_i18n::direction().is_rtl() {
                                    bounds.x + bounds.width - side
                                } else {
                                    bounds.x
                                },
                                y: bounds.y,
                                width: side,
                                height: side,
                            },
                            *color,
                        );
                        if !canopa_i18n::direction().is_rtl() {
                            text_bounds.x += side + 6. * scale;
                        }
                        text_bounds.width = (text_bounds.width - side - 6. * scale).max(1.);
                    }
                    // The layout control is already reachable from the panel's
                    // system chips or the fallback control rows; this widget
                    // only reports the active layout.
                    if placement.name == "clock" && lines.len() == 1 {
                        scene.clock(self, &lines[0], text_bounds, size * scale, *color);
                    } else {
                        scene.lines(
                            lines,
                            text_bounds,
                            size * scale,
                            (size + 7.) * scale,
                            *color,
                            true,
                            if *weight == cosmic_text::Weight::LIGHT {
                                ::iced::font::Weight::Light
                            } else if *weight == cosmic_text::Weight::MEDIUM {
                                ::iced::font::Weight::Medium
                            } else {
                                ::iced::font::Weight::Normal
                            },
                        );
                    }
                }
                Content::Avatar => scene.avatar(self, bounds, settings),
                Content::Authentication => {
                    if let Some(panel) = &panel {
                        let base = scene.base;
                        scene.alpha = base.0 * motion.stage_opacity;
                        scene.offset = (base.1.0, base.1.1 + motion.stage_rise * scale);
                        panel.populate(self, settings, &mut scene, bounds, scale, size);
                        (scene.alpha, scene.offset) = base;
                    } else {
                        let mut y = bounds.y;
                        if let Some(lines) = &label {
                            scene.lines(
                                lines,
                                crate::layout::Rect {
                                    y,
                                    height: label_height * scale,
                                    ..bounds
                                },
                                14. * scale,
                                21. * scale,
                                palette.text,
                                false,
                                ::iced::font::Weight::Normal,
                            );
                            y += label_height * scale;
                        }
                        if !self.hide_input {
                            scene.field(
                                self,
                                settings,
                                crate::layout::Rect {
                                    y,
                                    height: 52. * scale,
                                    ..bounds
                                },
                                scale,
                                &self.field_placeholder(),
                            );
                            y += 62. * scale;
                        }
                        for (index, hint) in hints.iter().enumerate() {
                            scene.label(
                                hint,
                                crate::layout::Rect {
                                    y: y + 20. * index as f32 * scale,
                                    height: 20. * scale,
                                    ..bounds
                                },
                                14. * scale,
                                if self.caps && index + 1 == hints.len() {
                                    palette.warning
                                } else if self.failed {
                                    palette.danger
                                } else {
                                    palette.muted
                                },
                                true,
                                ::iced::font::Weight::Normal,
                            );
                        }
                        y += 20. * hints.len().max(1) as f32 * scale + 8. * scale;
                        let controls: Vec<_> = self
                            .controls
                            .iter()
                            .cloned()
                            .chain(layout_control.iter().cloned())
                            .collect();
                        let gap = 10. * scale;
                        let width = (bounds.width - gap * (columns - 1) as f32) / columns as f32;
                        for row in controls.chunks(columns) {
                            for (index, control) in row.iter().enumerate() {
                                let column = if canopa_i18n::direction().is_rtl() {
                                    row.len() - 1 - index
                                } else {
                                    index
                                };
                                scene.button(
                                    self,
                                    settings,
                                    control,
                                    crate::layout::Rect {
                                        x: bounds.x + (width + gap) * column as f32,
                                        y,
                                        width,
                                        height: 34. * scale,
                                    },
                                    scale,
                                    false,
                                );
                            }
                            y += 42. * scale;
                        }
                    }
                }
            }
        }
        scene
    }
    pub(crate) fn clear_response_images(&self) {
        self.assets.borrow_mut().clear_fields();
    }
    pub(crate) fn field_placeholder(&self) -> String {
        if self.busy
            && self.secret.is_empty()
            && !self.authentication_panel.as_ref().is_some_and(|panel| panel.dialog)
        {
            // The instruction above the field says what is happening.
            String::new()
        } else if let Some(prompt) = &self.prompt {
            if prompt.echo {
                fl!("session-placeholder-type-response")
            } else {
                fl!("session-placeholder-enter-response")
            }
        } else if self.busy {
            fl!("session-verifying")
        } else {
            fl!("session-placeholder-authenticate")
        }
    }
    /// Native hosts call this after changing presentation state. Widget-only
    /// hover/press redraws keep their tree and image handles.
    pub fn changed(&self) {
        self.scene_revision
            .set(self.scene_revision.get().wrapping_add(1));
    }
    /// Observe passive data only from the host's clock/watch event, never from
    /// present(). An unchanged weather document does not invalidate the scene.
    pub fn tick(&self, settings: &Settings) -> bool {
        let next = Observed::read(settings);
        let mut previous = self.observed.borrow_mut();
        if *previous == next {
            return false;
        }
        *previous = next;
        self.changed();
        true
    }
    pub(crate) fn visual_revision(&self) -> VisualRevision {
        self.observe_motion();
        (
            self.scene_revision.get(),
            self.secret.revision(),
            self.secret.anchor(),
            self.secret.cursor(),
            self.revealed(),
            self.motion.revision(),
        )
    }
    fn layout_control(&self) -> Option<Control> {
        self.can_select_layout.then(|| {
            let mut control = Control::new(
                crate::input::LAYOUT_CONTROL,
                fl!("session-keyboard-control", layout = self.layout.as_str()),
            );
            control.enabled = !self.layout_unavailable;
            control
        })
    }

    /// The logical height a dialog window needs for its current content, between
    /// `minimum` and `maximum`. Dialog clients size their windows from it.
    pub fn dialog_height(&self, minimum: u32, maximum: u32) -> u32 {
        let Some(panel) = self.authentication_panel.as_ref().filter(|p| p.dialog) else {
            return maximum;
        };
        let layout_control = self.layout_control();
        let plan = panel.plan(
            self,
            layout_control.as_ref(),
            false,
            false,
            false,
            false,
            true,
            maximum as f32,
        );
        (plan.natural_dialog_height(self).ceil() as u32).clamp(minimum, maximum)
    }

    fn authentication_hints(&self) -> Vec<String> {
        let mut hints = Vec::new();
        if self.hide_input
            && let Some(prompt) = &self.prompt
        {
            hints.extend(wrap(&prompt.text, 56, 8));
        }
        if !self.message.is_empty() {
            hints.extend(wrap(&self.message, 56, 8));
        }
        if self.caps {
            hints.push(fl!("session-caps-lock-on"));
        }
        if hints.is_empty() {
            hints.push(if self.busy {
                fl!("session-hint-follow-instructions")
            } else if self.prompt.is_some() && self.controls.is_empty() {
                fl!("session-hint-press-enter")
            } else {
                fl!("session-hint-escape-cancels")
            });
        }
        hints
    }
    /// The clock's digits, and its day period ("PM") at a smaller size.
    fn clock_parts(text: &str) -> Vec<(&str, bool)> {
        // ICU separates the day period with a narrow no-break space.
        let parts: Vec<_> = text
            .split(char::is_whitespace)
            .filter(|part| !part.is_empty())
            .map(|part| {
                let period =
                    !part.chars().any(|c| c.is_numeric()) && part.chars().any(char::is_alphabetic);
                (part, period)
            })
            .collect();
        if parts.iter().any(|(_, period)| *period) && parts.iter().any(|(_, period)| !period) {
            parts
        } else {
            vec![(text, false)]
        }
    }

    fn clock_width(&self, text: &str, size: f32, clock: bool) -> f32 {
        if !clock {
            return self.text_width(text, size);
        }
        let parts = Self::clock_parts(text);
        let gap = size * 0.1;
        parts
            .iter()
            .map(|(part, period)| self.text_width(part, if *period { size * 0.34 } else { size }))
            .sum::<f32>()
            + gap * parts.len().saturating_sub(1) as f32
    }

    fn text_width(&self, text: &str, size: f32) -> f32 {
        self.text_renderer.borrow_mut().width(text, size)
    }
    fn text(
        &self,
        image: &mut RgbImage,
        text: &str,
        position: (f32, f32),
        size: f32,
        max_width: f32,
        color: Rgb,
    ) {
        let text = zeroize::Zeroizing::new(
            text.chars()
                .filter(|c| !c.is_control())
                .take(256)
                .collect::<String>(),
        );
        self.text_renderer
            .borrow_mut()
            .draw(image, &text, position, size, max_width, color, None);
    }
}

enum Content {
    Text {
        lines: Vec<String>,
        size: f32,
        color: Rgb,
        weight: cosmic_text::Weight,
        /// A glyph before a one-line passive status.
        icon: Option<Emblem>,
    },
    Avatar,
    Authentication,
}
fn wrap(text: &str, columns: usize, maximum: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.lines() {
        let chars: Vec<_> = line.chars().filter(|c| !c.is_control()).collect();
        let mut remaining = chars.as_slice();
        while !remaining.is_empty() {
            let limit = remaining.len().min(columns);
            let end = if limit < remaining.len() {
                remaining[..=limit]
                    .iter()
                    .rposition(|c| c.is_whitespace())
                    .filter(|&position| position > 0)
                    // Long identifiers break after a separator, not mid-word.
                    .or_else(|| {
                        remaining[..limit]
                            .iter()
                            .rposition(|c| matches!(c, '.' | '/' | '-' | '_'))
                            .filter(|&position| position > limit / 2)
                            .map(|position| position + 1)
                    })
                    .unwrap_or(limit)
            } else {
                limit
            };
            lines.push(remaining[..end].iter().collect());
            if lines.len() == maximum {
                return lines;
            }
            remaining = &remaining[end..];
            let start = remaining
                .iter()
                .position(|c| !c.is_whitespace())
                .unwrap_or(remaining.len());
            remaining = &remaining[start..];
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Wrap request text before paging so each displayed line is part of one page.
pub fn request_lines(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    wrap(text, 54, usize::MAX)
}

/// Keep the summary short enough for the credentials and actions to remain full size.
pub fn request_summary(text: &str) -> Vec<String> {
    let mut lines = request_lines(text);
    if lines.len() > 2 {
        lines.truncate(2);
        lines[1].push('…');
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_matches_a_desktop_showing_the_embedded_wallpaper() {
        let mut settings = Settings::parse(
            &kdl::KdlDocument::new(),
            Default::default(),
            None,
            canopa_config::Component::Lockscreen,
        )
        .unwrap();
        let backdrop = SessionView::prepare_background(&settings, "DP-1", 320, 200).unwrap();
        assert_eq!(
            backdrop,
            raster::backdrop(&settings.appearance, 320, 200).unwrap()
        );

        settings.built_in_wallpaper.insert("DP-1".into());
        let wallpaper = SessionView::prepare_background(&settings, "DP-1", 320, 200).unwrap();
        assert_ne!(wallpaper, backdrop);
        let other = SessionView::prepare_background(&settings, "HDMI-A-1", 320, 200).unwrap();
        assert_eq!(other, backdrop);
    }

    #[test]
    fn unchanged_identity_refreshes_preserve_the_scene_revision() {
        let settings = Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Lockscreen,
        )
        .unwrap()
        .0;
        let mut view = SessionView::new(&settings, "Alice".into()).unwrap();
        view.set_avatar_initials(Some("Alice".into()));
        let revision = view.visual_revision();
        view.set_avatar_initials(Some("Al".into()));
        view.set_user_avatar(None);
        assert_eq!(view.visual_revision(), revision);
        let avatar =
            std::sync::Arc::new(image::RgbaImage::from_pixel(64, 64, image::Rgba([255; 4])));
        view.set_user_avatar(Some(avatar.clone()));
        let revision = view.visual_revision();
        view.set_user_avatar(Some(avatar));
        assert_eq!(view.visual_revision(), revision);
        view.set_user_avatar(None);
        assert_ne!(view.visual_revision(), revision);
    }

    #[test]
    fn response_placeholder_tracks_prompt_and_authentication_state() {
        let _english = crate::i18n::english();
        let settings = Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Lockscreen,
        )
        .unwrap()
        .0;
        let mut view = SessionView::new(&settings, "Alice".into()).unwrap();
        assert_eq!(
            view.field_placeholder(),
            fl!("session-placeholder-authenticate")
        );
        // While checking, the instruction above a full-screen field says so.
        view.busy = true;
        assert_eq!(view.field_placeholder(), "");
        view.prompt = Some(Prompt {
            id: 1,
            text: "Password".into(),
            echo: false,
        });
        assert_eq!(view.field_placeholder(), "", "a submitted response is being checked");
        view.busy = false;
        assert_eq!(
            view.field_placeholder(),
            fl!("session-placeholder-enter-response")
        );
        view.prompt.as_mut().unwrap().echo = true;
        assert_eq!(
            view.field_placeholder(),
            fl!("session-placeholder-type-response")
        );
    }

    #[test]
    fn monitor_visibility_hides_passive_content_and_keeps_authentication() {
        use canopa_config::Component;
        let nodes = r#"
widgets "username"
layout {
    top-left "username"
    bottom-center "authentication"
}
output "DP-1" { hide-widgets "username"; }
"#;
        let settings = Settings::parse(
            &nodes.parse().unwrap(),
            Default::default(),
            None,
            Component::Lockscreen,
        )
        .unwrap();
        let view = SessionView::preview(&settings, "Sample user".into()).unwrap();
        let visible = view.render(&settings, "eDP-1", 640, 400).unwrap();
        let hidden = view.render(&settings, "DP-1", 640, 400).unwrap();
        let background = SessionView::prepare_background(&settings, "DP-1", 640, 400).unwrap();
        assert!(
            visible
                .enumerate_pixels()
                .any(|(x, y, pixel)| y < 100 && pixel != background.get_pixel(x, y))
        );
        assert!(
            hidden
                .enumerate_pixels()
                .all(|(x, y, pixel)| y >= 100 || pixel == background.get_pixel(x, y))
        );
        assert!(
            hidden
                .enumerate_pixels()
                .any(|(x, y, pixel)| y > 250 && pixel != background.get_pixel(x, y))
        );
        assert!(
            Settings::parse(
                &nodes
                    .replace(
                        "hide-widgets \"username\"",
                        "hide-widgets \"authentication\""
                    )
                    .parse()
                    .unwrap(),
                Default::default(),
                None,
                Component::Lockscreen
            )
            .is_err()
        );
    }

    #[test]
    fn right_to_left_mirrors_panel_rows_and_back_buttons() {
        let settings = Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Greeter,
        )
        .unwrap()
        .0;
        let mut view = SessionView::new(&settings, "Alice".into()).unwrap();
        view.hide_input = true;
        view.authentication_panel = Some(AuthenticationPanel {
            height_reserve: 0.,
            heading: "Choose".into(),
            subtitle: String::new(),
            emblem: Emblem::Accounts,
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
        view.controls = vec![
            Control::new(1, "Alice"),
            Control::new(2, "Back").with_kind(ControlKind::Back),
        ];
        let hits = |view: &SessionView| {
            view.render_with_hits(&settings, RgbImage::new(1280, 800))
                .unwrap()
                .1
        };
        let ltr = hits(&view);
        let rtl = {
            let _rtl = crate::i18n::Selected::new("en", true);
            hits(&view)
        };
        let find = |hits: &[(usize, crate::layout::Rect)], id| {
            hits.iter().find(|(hit, _)| *hit == id).unwrap().1
        };
        let (row, back) = (find(&ltr, 1), find(&ltr, 2));
        let (mirrored_row, mirrored_back) = (find(&rtl, 1), find(&rtl, 2));
        assert_eq!(row.x, mirrored_row.x, "full-width rows keep their place");
        assert!(back.width < row.width);
        // The narrow back button moves from the start on the left to the right.
        assert!((back.x - row.x).abs() < 1.);
        let end = |rect: crate::layout::Rect| rect.x + rect.width;
        assert!((end(mirrored_back) - end(mirrored_row)).abs() < 1.);
    }

    #[test]
    fn consumer_avatars_are_circular_blend_alpha_and_return_to_initials_or_static_fallback() {
        let mut settings = Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Lockscreen,
        )
        .unwrap()
        .0;
        settings.widgets = vec!["avatar".into()];
        settings.layout.regions = vec![crate::layout::Region {
            anchor: crate::layout::Anchor::Center,
            widgets: vec!["avatar".into()],
            row: false,
            x: 0.,
            y: 0.,
        }];
        let mut view = SessionView::new(&settings, "Alice".into()).unwrap();
        let render = |view: &SessionView| {
            view.render_over(
                &settings,
                RgbImage::from_pixel(1280, 800, image::Rgb([0, 0, 255])),
            )
            .unwrap()
        };
        let empty = render(&view);
        view.set_avatar_initials(Some("AE".into()));
        let initials = render(&view);
        assert_ne!(empty, initials);
        view.set_user_avatar(Some(std::sync::Arc::new(image::RgbaImage::from_pixel(
            128,
            128,
            image::Rgba([255, 0, 0, 128]),
        ))));
        let avatar = render(&view);
        assert!(
            avatar.pixels().filter(|p| p.0 == [128, 0, 127]).count() > 3000,
            "center: {:?}",
            avatar.get_pixel(640, 400)
        );
        assert_eq!(avatar.get_pixel(608, 368).0, [0, 0, 255]);
        view.set_user_avatar(None);
        assert_eq!(render(&view), initials);
        view.avatar = Some(image::RgbaImage::from_pixel(
            64,
            64,
            image::Rgba([0, 255, 0, 255]),
        ));
        assert!(
            render(&view)
                .pixels()
                .filter(|p| p.0 == [0, 255, 0])
                .count()
                > 3000
        );
        view.avatar = None;
        view.set_avatar_initials(None);
        assert_eq!(render(&view), empty);
    }
}
