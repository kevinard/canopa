use crate::{background, secret::Secret, settings::Settings};
use anyhow::Result;
use canopa_theme::{ColorRole, Rgb};
use image::RgbImage;

mod panel;
mod raster;
pub use panel::{
    AUTHORIZATION_DIALOG_SIZE, AuthenticationPanel, DIALOG_MARGIN, DIALOG_MAX_HEIGHT,
    DIALOG_MIN_HEIGHT, DIALOG_SIZE, account_label,
};
pub use raster::Emblem;

pub const REQUEST_PAGE_LINES: usize = 6;

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
    /// Signs in as a listed account; `selected` marks the current one, which
    /// shows the consumer's avatar.
    Account {
        name: String,
        detail: Option<String>,
        initials: String,
    },
}

#[derive(Clone)]
pub struct Control {
    pub id: usize,
    /// The accessible name, and the visible text unless `kind` draws its own.
    pub label: String,
    pub kind: ControlKind,
    /// Keyboard selection (focus ring) in surfaces without pointer focus.
    pub selected: bool,
}

impl Control {
    pub fn new(id: usize, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            kind: ControlKind::Action,
            selected: false,
        }
    }

    pub fn with_kind(mut self, kind: ControlKind) -> Self {
        self.kind = kind;
        self
    }
}

pub struct SessionView {
    text_renderer: std::cell::RefCell<crate::text::Text>,
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
}
impl SessionView {
    pub fn new(settings: &Settings, username: String) -> Result<Self> {
        Ok(Self {
            text_renderer: std::cell::RefCell::new(crate::text::Text::new(settings.font.clone())?),
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
        })
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
    /// The consumer owns asynchronous loading, selection and stale-result policy.
    pub fn set_user_avatar(&mut self, image: Option<std::sync::Arc<image::RgbaImage>>) {
        self.user_avatar = image;
    }
    pub fn set_avatar_initials(&mut self, initials: Option<String>) {
        self.avatar_initials = initials.map(|s| s.chars().take(2).collect());
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
        if bg.image.is_none()
            && !settings.solid_color
            && settings.color == settings.appearance.palette().background
        {
            return raster::backdrop(&settings.appearance, width, height);
        }
        let mut image =
            background::render(bg.image.as_deref(), bg.fit, settings.color, width, height)?;
        if bg.image.is_some() {
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
        let (width, height) = image.dimensions();
        let scale = (width as f32 / 1280.)
            .min(height as f32 / 800.)
            .clamp(0.2, 3.);
        self.render_filtered(settings, image, scale, settings.hidden_on(output))
    }

    /// Ordinary windows choose their logical scale; fullscreen consumers derive it above.
    pub fn render_at_scale(
        &self,
        settings: &Settings,
        image: RgbImage,
        scale: f32,
    ) -> Result<(RgbImage, Vec<(usize, crate::layout::Rect)>)> {
        self.render_filtered(settings, image, scale, &[])
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
        mut image: RgbImage,
        scale: f32,
        hidden: &[String],
    ) -> Result<(RgbImage, Vec<(usize, crate::layout::Rect)>)> {
        anyhow::ensure!(
            scale.is_finite() && (0.2..=4.).contains(&scale),
            "invalid presentation scale"
        );
        let mut hits = Vec::new();
        use crate::layout::Item;
        let (width, height) = image.dimensions();
        let palette = settings.appearance.palette();
        let now = chrono::Local::now();
        let mut content = std::collections::BTreeMap::new();
        content.insert(
            "clock".to_owned(),
            Content::Text {
                lines: vec![settings.clock_text(&now)],
                size: settings.clock_size as f32,
                color: palette.text,
                weight: cosmic_text::Weight::LIGHT,
                icon: None,
            },
        );
        content.insert(
            "date".to_owned(),
            Content::Text {
                lines: vec![settings.date_text(&now)],
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
                "weather" => settings
                    .weather_file
                    .as_deref()
                    .and_then(|p| {
                        canopa_widgets::weather::Weather::read(
                            p,
                            chrono::Utc::now().timestamp().max(0) as u64,
                        )
                    })
                    .map(|w| w.label()),
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
        let rtl = canopa_i18n::direction().is_rtl();
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
                height as f32 / scale,
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
                    Content::Authentication => panel
                        .as_ref()
                        .map_or((400., auth_height), |panel| panel.size()),
                };
                Item {
                    name: name.clone(),
                    width: w * scale,
                    height: h * scale,
                }
            })
            .collect();
        let placements = settings
            .layout
            .plan(width as f32, height as f32, scale, &items);
        for placement in placements {
            let bounds = placement.bounds;
            let scale = scale * placement.scale;
            let center = bounds.x + bounds.width / 2.;
            match &content[&placement.name] {
                Content::Text {
                    lines,
                    size,
                    color,
                    weight,
                    icon,
                } => {
                    let center = if let Some(icon) = icon {
                        let side = size * 1.2 * scale;
                        let rtl = canopa_i18n::direction().is_rtl();
                        let x = if rtl {
                            bounds.x + bounds.width - side
                        } else {
                            bounds.x
                        };
                        raster::emblem_at(
                            &mut image,
                            (x, bounds.y + (size * 0.85) * scale - side * 0.78),
                            side / 24.,
                            *icon,
                            *color,
                        );
                        center + if rtl { -1. } else { 1. } * size * 0.8 * scale
                    } else {
                        center
                    };
                    if placement.name == "clock" && lines.len() == 1 {
                        self.draw_clock(
                            &mut image,
                            &lines[0],
                            (center, bounds.y + size * 0.85 * scale),
                            *size * scale,
                            bounds.width,
                            *color,
                            *weight,
                        );
                        continue;
                    }
                    for (index, line) in lines.iter().enumerate() {
                        let text = zeroize::Zeroizing::new(
                            line.chars()
                                .filter(|c| !c.is_control())
                                .take(256)
                                .collect::<String>(),
                        );
                        self.text_renderer.borrow_mut().draw_styled(
                            &mut image,
                            &text,
                            (
                                center,
                                bounds.y + (size * 0.85 + index as f32 * (size + 7.)) * scale,
                            ),
                            *size * scale,
                            bounds.width,
                            *color,
                            None,
                            crate::text::Style::Centered(*weight),
                        );
                    }
                }
                Content::Avatar => {
                    if let Some(avatar) = self.user_avatar.as_deref().or(self.avatar.as_ref()) {
                        let side = (64. * scale).round().max(1.) as u32;
                        let avatar = image::imageops::resize(
                            avatar,
                            side,
                            side,
                            image::imageops::FilterType::Triangle,
                        );
                        let left = bounds.x.max(0.) as u32;
                        let top = bounds.y.max(0.) as u32;
                        for (x, y, pixel) in avatar.enumerate_pixels() {
                            let dx = x as f32 - side as f32 / 2.;
                            let dy = y as f32 - side as f32 / 2.;
                            if dx * dx + dy * dy <= (side as f32 / 2.).powi(2)
                                && left + x < width
                                && top + y < height
                            {
                                let target = image.get_pixel_mut(left + x, top + y);
                                let alpha = u32::from(pixel[3]);
                                for i in 0..3 {
                                    target[i] = ((u32::from(pixel[i]) * alpha
                                        + u32::from(target[i]) * (255 - alpha))
                                        / 255)
                                        as u8;
                                }
                            }
                        }
                    } else if let Some(initials) = &self.avatar_initials {
                        rounded_rect(
                            &mut image,
                            bounds.x,
                            bounds.y,
                            bounds.width,
                            bounds.height,
                            bounds.width / 2.,
                            palette.surface,
                        );
                        self.text(
                            &mut image,
                            initials,
                            (center, bounds.y + 40. * scale),
                            25. * scale,
                            bounds.width - 8. * scale,
                            palette.text,
                        );
                    }
                }
                Content::Authentication => {
                    if let Some(panel) = &panel {
                        hits.extend(panel.draw(self, settings, &mut image, bounds, scale));
                        continue;
                    }
                    if let Some(lines) = &label {
                        let size = 14. * scale;
                        for (index, line) in lines.iter().enumerate() {
                            let available = (bounds.width - 12. * scale).max(0.);
                            let half = self.text_width(line, size).min(available) / 2.;
                            self.text(
                                &mut image,
                                line,
                                (
                                    if rtl {
                                        bounds.x + bounds.width - 6. * scale - half
                                    } else {
                                        bounds.x + 6. * scale + half
                                    },
                                    bounds.y + (14. + index as f32 * 21.) * scale,
                                ),
                                size,
                                available,
                                palette.text,
                            );
                        }
                    }
                    if !self.hide_input {
                        let field_y = bounds.y + label_height * scale;
                        let outline = if prompt.is_some() {
                            settings.appearance.role(ColorRole::Focus)
                        } else {
                            palette.border
                        };
                        rounded_rect(
                            &mut image,
                            bounds.x,
                            field_y,
                            bounds.width,
                            52. * scale,
                            12. * scale,
                            outline,
                        );
                        rounded_rect(
                            &mut image,
                            bounds.x + scale,
                            field_y + scale,
                            bounds.width - 2. * scale,
                            50. * scale,
                            11. * scale,
                            palette.surface,
                        );
                        let (input, selection, cursor) = self
                            .secret
                            .presentation(self.prompt.as_ref().is_some_and(|p| p.echo), 48);
                        let prompt = zeroize::Zeroizing::new(if let Some(prompt) = &self.prompt {
                            if self.secret.is_empty() {
                                if prompt.echo {
                                    fl!("session-placeholder-type-response")
                                } else {
                                    fl!("session-placeholder-enter-response")
                                }
                            } else {
                                input.to_string()
                            }
                        } else if self.busy {
                            fl!("session-waiting-for-authentication")
                        } else {
                            fl!("session-placeholder-authenticate")
                        });
                        self.text_renderer.borrow_mut().draw(
                            &mut image,
                            &prompt,
                            (center, field_y + 32. * scale),
                            16. * scale,
                            bounds.width - 20. * scale,
                            if self.secret.is_empty() {
                                palette.muted
                            } else {
                                palette.text
                            },
                            self.prompt
                                .as_ref()
                                .filter(|_| !self.secret.is_empty())
                                .map(|_| (selection, cursor, palette.accent)),
                        );
                    }
                    for (index, control) in self
                        .controls
                        .iter()
                        .chain(layout_control.iter())
                        .enumerate()
                    {
                        let y = bounds.y
                            + (88. - input_offset
                                + label_height
                                + 20. * hints.len().saturating_sub(1) as f32
                                + (index / columns) as f32 * 42.)
                                * scale;
                        let gap = 8. * scale;
                        let width = (bounds.width - gap * (columns - 1) as f32) / columns as f32;
                        let column = if rtl {
                            columns - 1 - index % columns
                        } else {
                            index % columns
                        };
                        let x = bounds.x + (width + gap) * column as f32;
                        let primary = self.primary_control == Some(control.id) && !self.busy;
                        let (label, selected) = (control.label.as_str(), control.selected);
                        let (fill, ink) = if primary {
                            (
                                palette.accent,
                                settings.appearance.role(ColorRole::OnAccent),
                            )
                        } else if selected {
                            (
                                settings.appearance.role(ColorRole::AccentSurface),
                                palette.text,
                            )
                        } else {
                            (palette.surface, palette.text)
                        };
                        rounded_rect(
                            &mut image,
                            x,
                            y,
                            width,
                            34. * scale,
                            9. * scale,
                            if selected {
                                settings.appearance.role(ColorRole::Focus)
                            } else {
                                palette.border
                            },
                        );
                        rounded_rect(
                            &mut image,
                            x + scale,
                            y + scale,
                            width - 2. * scale,
                            32. * scale,
                            8. * scale,
                            fill,
                        );
                        self.text(
                            &mut image,
                            label,
                            (x + width / 2., y + 22. * scale),
                            16. * scale,
                            width - 16. * scale,
                            ink,
                        );
                        hits.push((
                            control.id,
                            crate::layout::Rect {
                                x,
                                y,
                                width,
                                height: 34. * scale,
                            },
                        ));
                    }
                    for (index, hint) in hints.iter().enumerate() {
                        self.text(
                            &mut image,
                            hint,
                            (
                                center,
                                bounds.y
                                    + (75. - input_offset + label_height + index as f32 * 20.)
                                        * scale,
                            ),
                            14. * scale,
                            bounds.width,
                            if self.failed {
                                palette.danger
                            } else if self.caps {
                                palette.warning
                            } else {
                                palette.muted
                            },
                        );
                    }
                }
            }
        }
        Ok((image, hits))
    }
    fn layout_control(&self) -> Option<Control> {
        self.can_select_layout.then(|| {
            Control::new(
                crate::input::LAYOUT_CONTROL,
                fl!("session-keyboard-control", layout = self.layout.as_str()),
            )
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

    #[allow(clippy::too_many_arguments)]
    fn draw_clock(
        &self,
        image: &mut RgbImage,
        text: &str,
        (center, baseline): (f32, f32),
        size: f32,
        max_width: f32,
        color: Rgb,
        weight: cosmic_text::Weight,
    ) {
        let parts = Self::clock_parts(text);
        let width = self.clock_width(text, size, true).min(max_width);
        let rtl = canopa_i18n::direction().is_rtl();
        let ordered: Vec<_> = if rtl {
            parts.iter().rev().collect()
        } else {
            parts.iter().collect()
        };
        let gap = size * 0.1;
        let mut x = center - width / 2.;
        for (part, period) in ordered {
            let (size, weight) = if *period {
                (size * 0.34, cosmic_text::Weight::LIGHT)
            } else {
                (size, weight)
            };
            self.text_renderer.borrow_mut().draw_styled(
                image,
                part,
                (x, baseline),
                size,
                max_width,
                color,
                None,
                crate::text::Style::Leading(weight),
            );
            x += self.text_width(part, size) + gap;
        }
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
fn rounded_rect(image: &mut RgbImage, x: f32, y: f32, w: f32, h: f32, r: f32, color: Rgb) {
    let r = r.min(w / 2.).min(h / 2.).max(0.);
    for py in y.max(0.) as u32..((y + h) as u32).min(image.height()) {
        for px in x.max(0.) as u32..((x + w) as u32).min(image.width()) {
            let dx = (px as f32 - (px as f32).clamp(x + r, x + w - r)).abs();
            let dy = (py as f32 - (py as f32).clamp(y + r, y + h - r)).abs();
            if dx * dx + dy * dy <= r * r {
                image.put_pixel(px, py, image::Rgb([color.0, color.1, color.2]));
            }
        }
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
    wrap(text, 52, usize::MAX)
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
        assert!(avatar.pixels().filter(|p| p.0 == [128, 0, 127]).count() > 3000);
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
