use crate::{background, secret::Secret, settings::Settings};
use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use anyhow::{Context, Result};
use canopa_theme::Rgb;
use image::RgbImage;

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
    font: FontArc,
    avatar: Option<image::RgbaImage>,
    user_avatar: Option<std::sync::Arc<image::RgbaImage>>,
    avatar_initials: Option<String>,
    pub controls: Vec<Control>,
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
    pub battery: Option<canopa_widgets::battery::BatteryState>,
    pub media: Option<String>,
    pub custom: std::collections::BTreeMap<String, String>,
}
impl SessionView {
    pub fn new(settings: &Settings, username: String) -> Result<Self> {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        let id = database
            .query(&fontdb::Query {
                families: &[
                    fontdb::Family::Name(&settings.font),
                    fontdb::Family::SansSerif,
                ],
                ..Default::default()
            })
            .or_else(|| database.faces().next().map(|f| f.id))
            .context("no installed font is available")?;
        let font = database
            .with_face_data(id, |bytes, index| {
                ab_glyph::FontVec::try_from_vec_and_index(bytes.to_vec(), index).map(FontArc::new)
            })
            .context("font data is unavailable")??;
        Ok(Self {
            font,
            avatar: settings
                .avatar
                .as_deref()
                .and_then(|path| background::load(path).ok())
                .map(|image| image::imageops::thumbnail(&image, 256, 256)),
            user_avatar: None,
            avatar_initials: None,
            controls: Vec::new(),
            username,
            secret: Secret::default(),
            busy: false,
            hide_input: false,
            prompt: None,
            message: String::new(),
            failed: false,
            caps: false,
            layout: String::new(),
            battery: None,
            media: None,
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
        self.render_over(
            settings,
            Self::prepare_background(settings, name, width, height)?,
        )
    }
    pub fn prepare_background(
        settings: &Settings,
        name: &str,
        width: u32,
        height: u32,
    ) -> Result<RgbImage> {
        let bg = settings.for_output(name);
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
        let (width, height) = image.dimensions();
        let scale = (width as f32 / 1280.)
            .min(height as f32 / 800.)
            .clamp(0.2, 3.);
        self.render_at_scale(settings, image, scale)
    }
    /// Ordinary dialogs choose their logical scale; fullscreen consumers derive it above.
    pub fn render_at_scale(
        &self,
        settings: &Settings,
        mut image: RgbImage,
        scale: f32,
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
        for widget in &settings.widgets {
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
                let color = definition.map_or(palette.muted, |w| match w.color.as_str() {
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
        let hints = self.authentication_hints();
        let input_offset = if self.hide_input { 52. } else { 0. };
        let auth_height =
            70. - input_offset + 20. * hints.len().max(1) as f32 + 42. * self.controls.len() as f32;
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
                    Content::Authentication => (400., auth_height),
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
                    if !self.hide_input {
                        rounded_rect(
                            &mut image,
                            bounds.x,
                            bounds.y,
                            bounds.width,
                            52. * scale,
                            18. * scale,
                            palette.surface,
                        );
                        let prompt = zeroize::Zeroizing::new(if let Some(prompt) = &self.prompt {
                            if self.secret.is_empty() {
                                if prompt.echo {
                                    "Type response".into()
                                } else {
                                    "Enter response".into()
                                }
                            } else if prompt.echo {
                                self.secret.text().to_owned()
                            } else {
                                "•".repeat(self.secret.count().min(24))
                            }
                        } else if self.busy {
                            "Waiting for authentication…".into()
                        } else {
                            "Enter to authenticate".into()
                        });
                        self.text(
                            &mut image,
                            &prompt,
                            (center, bounds.y + 33. * scale),
                            17. * scale,
                            bounds.width - 20. * scale,
                            palette.text,
                        );
                    }
                    for (index, control) in self.controls.iter().enumerate() {
                        let y = bounds.y
                            + (88. - input_offset
                                + 20. * hints.len().saturating_sub(1) as f32
                                + index as f32 * 42.)
                                * scale;
                        rounded_rect(
                            &mut image,
                            bounds.x,
                            y,
                            bounds.width,
                            34. * scale,
                            10. * scale,
                            palette.surface,
                        );
                        self.text(
                            &mut image,
                            &control.label,
                            (center, y + 22. * scale),
                            16. * scale,
                            bounds.width - 16. * scale,
                            palette.text,
                        );
                        hits.push((
                            control.id,
                            crate::layout::Rect {
                                x: bounds.x,
                                y,
                                width: bounds.width,
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
                                bounds.y + (75. - input_offset + index as f32 * 20.) * scale,
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
        if let Some(prompt) = &self.prompt {
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
                } else {
                    "Escape cancels the current attempt"
                }
                .into(),
            );
        }
        hints
    }
    fn text_width(&self, text: &str, size: f32) -> f32 {
        let font = self.font.as_scaled(PxScale::from(size));
        text.chars().map(|c| font.h_advance(font.glyph_id(c))).sum()
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
        let (center, baseline) = position;
        let scaled = self.font.as_scaled(PxScale::from(size));
        let text: String = text.chars().filter(|c| !c.is_control()).take(100).collect();
        let width: f32 = text
            .chars()
            .map(|c| scaled.h_advance(scaled.glyph_id(c)))
            .sum();
        let fit = (max_width.max(1.) / width.max(1.)).min(1.);
        let scaled = self.font.as_scaled(PxScale::from(size * fit));
        let mut x = center - width * fit / 2.;
        for character in text.chars() {
            let glyph = scaled.scaled_glyph(character);
            let advance = scaled.h_advance(glyph.id);
            let mut glyph = glyph;
            glyph.position = point(x, baseline);
            if let Some(outline) = self.font.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                outline.draw(|gx, gy, coverage| {
                    let px = gx as i32 + bounds.min.x as i32;
                    let py = gy as i32 + bounds.min.y as i32;
                    if px >= 0 && py >= 0 && px < image.width() as i32 && py < image.height() as i32
                    {
                        let pixel = image.get_pixel_mut(px as u32, py as u32);
                        for (channel, value) in pixel.0.iter_mut().zip([color.0, color.1, color.2])
                        {
                            *channel = (*channel as f32 * (1. - coverage) + value as f32 * coverage)
                                .round() as u8;
                        }
                    }
                });
            }
            x += advance;
        }
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
        for chunk in chars.chunks(columns) {
            lines.push(chunk.iter().collect());
            if lines.len() == maximum {
                return lines;
            }
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
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
