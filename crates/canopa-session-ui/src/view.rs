use crate::{background, secret::Secret, settings::Settings};
use anyhow::Result;
use canopa_theme::{ColorRole, Rgb};
use image::RgbImage;

mod panel;
mod raster;
pub use panel::{AUTHORIZATION_DIALOG_SIZE, AuthenticationPanel, DIALOG_MARGIN, DIALOG_SIZE};
pub use raster::Emblem;

pub const REQUEST_PAGE_LINES: usize = 6;

pub struct Prompt {
    pub id: u64,
    pub text: String,
    pub echo: bool,
}

pub struct Control {
    pub id: usize,
    pub label: String,
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
        })
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
            text: "Password:".into(),
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
                lines: vec![now.format(&settings.clock_format).to_string()],
                size: settings.clock_size as f32,
                color: palette.text,
            },
        );
        content.insert(
            "date".to_owned(),
            Content::Text {
                lines: vec![now.format(&settings.date_format).to_string()],
                size: settings.text_size as f32,
                color: palette.muted,
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
                    format!(
                        "{}%{}",
                        b.percentage.clamp(0., 100.).round(),
                        if b.power_state.is_charging() {
                            " · Charging"
                        } else {
                            ""
                        }
                    )
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
                    },
                );
            }
        }
        let prompt = self.prompt.as_ref().filter(|_| !self.hide_input);
        let label = prompt.map(|prompt| wrap(prompt.text.trim_end_matches(':'), 56, 8));
        let label_height = label.as_ref().map_or(0., |lines| 21. * lines.len() as f32);
        let hints = self.authentication_hints();
        let layout_control = self.can_select_layout.then(|| Control {
            id: crate::input::LAYOUT_CONTROL,
            label: format!("Keyboard: {}   · F6", self.layout),
        });
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
        let panel = self.authentication_panel.as_ref().map(|panel| {
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
            )
        });
        let items: Vec<_> = content
            .iter()
            .map(|(name, content)| {
                let (w, h) = match content {
                    Content::Text { lines, size, .. } => (
                        lines
                            .iter()
                            .map(|s| self.text_width(s, *size))
                            .fold(0., f32::max)
                            .max(1.),
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
                Content::Text { lines, size, color } => {
                    for (index, line) in lines.iter().enumerate() {
                        self.text(
                            &mut image,
                            line,
                            (
                                center,
                                bounds.y + (size * 0.85 + index as f32 * (size + 7.)) * scale,
                            ),
                            *size * scale,
                            bounds.width,
                            *color,
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
                            self.text(
                                &mut image,
                                line,
                                (
                                    bounds.x
                                        + 6. * scale
                                        + self.text_width(line, size).min(available) / 2.,
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
                                    "Type response".into()
                                } else {
                                    "Enter response".into()
                                }
                            } else {
                                input.to_string()
                            }
                        } else if self.busy {
                            "Waiting for authentication…".into()
                        } else {
                            "Enter to authenticate".into()
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
                        let x = bounds.x + (width + gap) * (index % columns) as f32;
                        let primary = self.primary_control == Some(control.id) && !self.busy;
                        let (label, selected) = control
                            .label
                            .strip_prefix("› ")
                            .map_or((control.label.as_str(), false), |label| (label, true));
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
            hints.push("Caps Lock is on".into());
        }
        if hints.is_empty() {
            hints.push(
                if self.busy {
                    "Follow the authentication instructions"
                } else if self.prompt.is_some() && self.controls.is_empty() {
                    "Press Enter to continue"
                } else {
                    "Escape cancels the current attempt"
                }
                .into(),
            );
        }
        hints
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
