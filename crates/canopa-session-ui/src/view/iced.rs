//! The interactive session view uses the same Iced controls as the applications.
use super::style::{self, Kind, Look};
use super::{Control, ControlKind, Emblem, SessionView, raster};
use crate::{
    layout::Rect,
    presentation::{Element, Message, Renderer},
    settings::Settings,
};
use canopa_theme::Rgb;
use iced::advanced::image::Renderer as _;
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, renderer,
    widget::{self, Tree},
};
use iced::widget::{button, container, image, pin, scrollable, stack, text, tooltip};
use iced::{Font, Length, Point, Rectangle, Size, Theme, mouse};

pub(super) fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(256)
        .collect()
}

#[derive(Clone)]
struct PreparedField {
    handle: image::Handle,
    positions: Vec<(usize, f32)>,
}
type AvatarKey = (usize, (u32, u32));
type IconKey = (Emblem, u32, (u8, u8, u8), Option<u16>, bool);
#[derive(Default)]
pub(super) struct Assets {
    text: Option<std::rc::Rc<std::cell::RefCell<crate::text::Text>>>,
    icons: std::collections::VecDeque<(IconKey, image::Handle)>,
    avatar: Option<(AvatarKey, image::Handle)>,
    fields: std::collections::VecDeque<(String, PreparedField)>,
    field_stamp: Option<String>,
}
impl Assets {
    pub(super) fn with_text(text: std::rc::Rc<std::cell::RefCell<crate::text::Text>>) -> Self {
        Self {
            text: Some(text),
            ..Default::default()
        }
    }

    pub(super) fn clear_fields(&mut self) {
        self.fields.clear();
        self.field_stamp = None;
    }
    pub(super) fn clear_avatar(&mut self) {
        self.avatar = None;
    }
    fn icon(&mut self, icon: Emblem, side: u32, color: Rgb, level: Option<f32>) -> image::Handle {
        let rtl = canopa_i18n::direction().is_rtl();
        let level = level.map(|v| (v.clamp(0., 1.) * 1000.).round() as u16);
        let key = (icon, side, (color.0, color.1, color.2), level, rtl);
        if let Some(index) = self.icons.iter().position(|(old, _)| old == &key) {
            let entry = self.icons.remove(index).unwrap();
            let handle = entry.1.clone();
            self.icons.push_back(entry);
            return handle;
        }
        let icon = if rtl {
            match icon {
                Emblem::Arrow => Emblem::ArrowBack,
                Emblem::ArrowBack => Emblem::Arrow,
                Emblem::Chevron => Emblem::ChevronBack,
                Emblem::ChevronBack => Emblem::Chevron,
                other => other,
            }
        } else {
            icon
        };
        let mut bitmap = ::image::RgbImage::new(side, side);
        raster::emblem_at(
            &mut bitmap,
            (0., 0.),
            side as f32 / 24.,
            icon,
            Rgb(255, 255, 255),
        );
        if icon == Emblem::Battery
            && let Some(level) = level
        {
            let scale = side as f32 / 24.;
            for (x, y, pixel) in bitmap.enumerate_pixels_mut() {
                if x as f32 >= 5. * scale
                    && (x as f32) < (5. + 12. * f32::from(level) / 1000.) * scale
                    && y as f32 >= 9. * scale
                    && (y as f32) < 15. * scale
                {
                    *pixel = ::image::Rgb([255; 3]);
                }
            }
        }
        let rgba: Vec<_> = bitmap
            .pixels()
            .flat_map(|p| [color.0, color.1, color.2, p[0]])
            .collect();
        let handle = image::Handle::from_rgba(side, side, rgba);
        if self.icons.len() == 128 {
            self.icons.pop_front();
        }
        self.icons.push_back((key, handle.clone()));
        handle
    }
    fn avatar(&mut self, source: &::image::RgbaImage) -> image::Handle {
        let key = (source.as_raw().as_ptr() as usize, source.dimensions());
        if let Some((old, handle)) = &self.avatar
            && *old == key
        {
            return handle.clone();
        }
        let mut bitmap = source.clone();
        let (w, h) = bitmap.dimensions();
        for (x, y, pixel) in bitmap.enumerate_pixels_mut() {
            let dx = (x as f32 + 0.5) / w as f32 - 0.5;
            let dy = (y as f32 + 0.5) / h as f32 - 0.5;
            if dx * dx + dy * dy > 0.25 {
                pixel[3] = 0;
            }
        }
        let handle = image::Handle::from_rgba(w, h, bitmap.into_raw());
        self.avatar = Some((key, handle.clone()));
        handle
    }
}

pub(crate) struct Scene {
    elements: Vec<Element<'static>>,
    assets: std::rc::Rc<std::cell::RefCell<Assets>>,
    pub hits: Vec<(usize, Rect)>,
    /// Controls float over the wallpaper (full-screen surfaces).
    pub glass: bool,
    /// Motion: opacity and offset applied to everything placed, the values
    /// outside the authentication content, the field's shake and the spinner.
    pub alpha: f32,
    pub offset: (f32, f32),
    pub base: (f32, (f32, f32)),
    pub shake: f32,
    pub spinner: Option<f32>,
    /// A response field has keyboard focus; buttons then show no focus ring.
    pub field_focus: bool,
    /// The light colour scheme (the clock takes a heavier weight).
    pub glass_light: bool,
}

/// What a styled button shows.
#[derive(Clone, Copy)]
pub(crate) enum Content<'a> {
    /// A centred icon of the given logical size.
    Icon(Emblem, f32),
    Text {
        label: &'a str,
        size: f32,
        weight: iced::font::Weight,
        centered: bool,
    },
    /// Optional icon, label and trailing indicator, centred.
    Chip {
        icon: Option<Emblem>,
        label: &'a str,
        trailing: Option<Emblem>,
    },
    /// A list row: leading picture, title and detail, and a trailing mark.
    Row {
        leading: Option<Leading<'a>>,
        title: &'a str,
        detail: Option<&'a str>,
        checked: bool,
    },
    /// A large icon above its label.
    Tile { icon: Emblem, label: &'a str },
}

#[derive(Clone, Copy)]
pub(crate) enum Leading<'a> {
    Initials(&'a str),
    Avatar(&'a SessionView),
    Icon(Emblem),
}
impl Scene {
    pub(super) fn new(assets: std::rc::Rc<std::cell::RefCell<Assets>>) -> Self {
        Self {
            elements: Vec::new(),
            assets,
            hits: Vec::new(),
            glass: false,
            alpha: 1.,
            offset: (0., 0.),
            base: (1., (0., 0.)),
            shake: 0.,
            spinner: None,
            field_focus: false,
            glass_light: false,
        }
    }
    pub fn empty(&self) -> Self {
        let mut scene = Self::new(self.assets.clone());
        scene.glass = self.glass;
        scene.alpha = self.alpha;
        scene.base = (self.alpha, (0., 0.));
        scene.shake = self.shake;
        scene.spinner = self.spinner;
        scene.field_focus = self.field_focus;
        scene
    }
    fn fade(&self, mut color: iced::Color) -> iced::Color {
        color.a *= self.alpha;
        color
    }
    fn fade_background(&self, background: iced::Background) -> iced::Background {
        match background {
            iced::Background::Color(color) => iced::Background::Color(self.fade(color)),
            iced::Background::Gradient(iced::Gradient::Linear(mut linear)) => {
                for stop in linear.stops.iter_mut().flatten() {
                    stop.color = self.fade(stop.color);
                }
                iced::Background::Gradient(iced::Gradient::Linear(linear))
            }
        }
    }
    /// A turning arc, while waiting on the system.
    pub fn spinner(&mut self, rect: Rect, angle: f32, color: Rgb, track: iced::Color) {
        let side = rect.width.max(rect.height).ceil().max(1.) as u32;
        let (ring, arc) = {
            let mut assets = self.assets.borrow_mut();
            (
                assets.icon(
                    Emblem::Ring,
                    side,
                    Rgb(
                        (track.r * 255.).round() as u8,
                        (track.g * 255.).round() as u8,
                        (track.b * 255.).round() as u8,
                    ),
                    None,
                ),
                assets.icon(Emblem::Spinner, side, color, None),
            )
        };
        let mut track_image = image(ring).width(rect.width).height(rect.height);
        track_image = track_image.opacity(track.a * self.alpha);
        self.place(track_image, rect);
        self.place(
            image(arc)
                .width(rect.width)
                .height(rect.height)
                .opacity(self.alpha)
                .rotation(iced::Rotation::Floating(iced::Radians(angle))),
            rect,
        );
    }
    /// Draw one label per line. `size` and `advance` are physical: callers
    /// scale both, so the rows match the height their layout reserved.
    #[allow(clippy::too_many_arguments)]
    pub fn lines(
        &mut self,
        lines: &[String],
        rect: Rect,
        size: f32,
        advance: f32,
        color: Rgb,
        centered: bool,
        weight: iced::font::Weight,
    ) {
        for (i, line) in lines.iter().enumerate() {
            self.label(
                line.clone(),
                Rect {
                    y: rect.y + i as f32 * advance,
                    height: advance.min(rect.height),
                    ..rect
                },
                size,
                color,
                centered,
                weight,
            );
        }
    }
    pub fn clock(&mut self, view: &SessionView, value: &str, rect: Rect, size: f32, color: Rgb) {
        let value = clean(value);
        let mut parts = SessionView::clock_parts(&value);
        if canopa_i18n::direction().is_rtl() {
            parts.reverse();
        }
        let fit = (rect.width / view.clock_width(&value, size, true).max(1.)).min(1.);
        let size = size * fit;
        let total = view.clock_width(&value, size, true);
        let mut x = rect.x + (rect.width - total) / 2.;
        for (part, period) in parts {
            let part_size = size * if period { 0.34 } else { 1. };
            let width = view.text_width(part, part_size) + 1.;
            self.label(
                part,
                Rect {
                    x,
                    // The day period sits on the digits' baseline.
                    y: rect.y + if period { size * 0.665 } else { 0. },
                    width,
                    height: part_size * 1.3,
                },
                part_size,
                color,
                true,
                // Light is too thin on a light wallpaper.
                if self.glass_light {
                    iced::font::Weight::Normal
                } else {
                    iced::font::Weight::Light
                },
            );
            x += width - 1. + size * 0.1;
        }
    }
    pub fn element(self, size: Size) -> Element<'static> {
        stack(self.elements)
            .width(size.width)
            .height(size.height)
            .into()
    }
    pub fn place(&mut self, element: impl Into<Element<'static>>, rect: Rect) {
        let rect = Rect {
            x: rect.x + self.offset.0,
            y: rect.y + self.offset.1,
            ..rect
        };
        self.elements.push(
            pin(container(element)
                .clip(true)
                .width(rect.width.max(1.))
                .height(rect.height.max(1.)))
            .position(Point::new(rect.x, rect.y))
            .into(),
        );
    }
    pub fn scroll(&mut self, content: Scene, size: Size, rect: Rect) {
        self.hits.extend(content.hits.iter().map(|(id, bounds)| {
            (
                *id,
                Rect {
                    x: bounds.x + rect.x,
                    y: bounds.y + rect.y,
                    ..*bounds
                },
            )
        }));
        self.place(
            scrollable(content.element(size))
                .width(rect.width)
                .height(rect.height)
                .style(canopa_ui::scrollbar),
            rect,
        );
    }
    pub fn label(
        &mut self,
        value: impl Into<String>,
        rect: Rect,
        size: f32,
        color: Rgb,
        centered: bool,
        weight: iced::font::Weight,
    ) {
        self.styled_label(&value.into(), rect, size, color, centered, weight, false);
    }
    pub fn code(&mut self, value: &str, rect: Rect, size: f32, color: Rgb) {
        self.styled_label(
            value,
            rect,
            size,
            color,
            false,
            iced::font::Weight::Medium,
            true,
        );
    }
    fn fit_size(
        &self,
        value: &str,
        rect: Rect,
        size: f32,
        weight: iced::font::Weight,
        monospace: bool,
    ) -> f32 {
        let weight = cosmic_text::Weight(match weight {
            iced::font::Weight::Thin => 100,
            iced::font::Weight::ExtraLight => 200,
            iced::font::Weight::Light => 300,
            iced::font::Weight::Normal => 400,
            iced::font::Weight::Medium => 500,
            iced::font::Weight::Semibold => 600,
            iced::font::Weight::Bold => 700,
            iced::font::Weight::ExtraBold => 800,
            iced::font::Weight::Black => 900,
        });
        let text = self
            .assets
            .borrow()
            .text
            .as_ref()
            .expect("scene text metrics")
            .clone();
        let width = text
            .borrow_mut()
            .styled_width(value, size, weight, monospace);
        let lines = value.lines().count().max(1) as f32;
        size * (rect.width.max(1.) / width.max(1.))
            .min(rect.height.max(1.) / (lines * size * 1.3))
            .min(1.)
    }
    #[allow(clippy::too_many_arguments)]
    fn styled_label(
        &mut self,
        value: &str,
        rect: Rect,
        size: f32,
        color: Rgb,
        centered: bool,
        weight: iced::font::Weight,
        monospace: bool,
    ) {
        let value = clean(value);
        let size = self.fit_size(&value, rect, size, weight, monospace);
        let label = text(value)
            .wrapping(iced::widget::text::Wrapping::None)
            .size(size)
            .color(style::rgba(color, self.alpha))
            .font(Font {
                weight,
                family: if monospace {
                    iced::font::Family::Monospace
                } else {
                    Font::DEFAULT.family
                },
                ..Font::DEFAULT
            })
            .shaping(iced::advanced::text::Shaping::Advanced)
            .align_x(if centered {
                iced::alignment::Horizontal::Center
            } else if canopa_i18n::direction().is_rtl() {
                iced::alignment::Horizontal::Right
            } else {
                iced::alignment::Horizontal::Left
            })
            .width(Length::Fill);
        self.place(container(label).clip(true).center_y(Length::Fill), rect);
    }
    pub fn icon(&mut self, icon: Emblem, rect: Rect, color: Rgb) {
        self.status_icon(icon, rect, color, None);
    }
    pub fn status_icon(&mut self, icon: Emblem, rect: Rect, color: Rgb, level: Option<f32>) {
        let side = rect.width.max(rect.height).ceil().max(1.) as u32;
        let handle = self.assets.borrow_mut().icon(icon, side, color, level);
        self.place(
            image(handle)
                .width(rect.width)
                .height(rect.height)
                .opacity(self.alpha),
            rect,
        );
    }
    /// A rounded surface with an optional gradient fill, border and shadow.
    #[allow(clippy::too_many_arguments)]
    pub fn surface(
        &mut self,
        rect: Rect,
        radius: f32,
        fill: impl Into<iced::Background>,
        border: iced::Color,
        border_width: f32,
        shadow: iced::Shadow,
    ) {
        let fill = self.fade_background(fill.into());
        let border = self.fade(border);
        let shadow = iced::Shadow {
            color: self.fade(shadow.color),
            ..shadow
        };
        self.place(
            container(iced::widget::space())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(fill),
                    border: iced::Border {
                        color: border,
                        width: border_width,
                        radius: radius.into(),
                    },
                    shadow,
                    ..Default::default()
                }),
            Rect {
                // Shadows extend past the placed bounds; the pin clips them,
                // so place the shadowed node with room around it.
                ..rect
            },
        );
    }
    /// A surface whose shadow is drawn around it, outside its own bounds.
    pub fn raised_surface(
        &mut self,
        rect: Rect,
        radius: f32,
        fill: impl Into<iced::Background>,
        border: iced::Color,
        shadow: iced::Shadow,
    ) {
        let fill = self.fade_background(fill.into());
        let border = self.fade(border);
        let shadow = iced::Shadow {
            color: self.fade(shadow.color),
            ..shadow
        };
        let room = shadow.blur_radius * 1.5 + shadow.offset.y.abs();
        self.place(
            container(
                container(iced::widget::space())
                    .width(rect.width.max(1.))
                    .height(rect.height.max(1.))
                    .style(move |_| container::Style {
                        background: Some(fill),
                        border: iced::Border {
                            color: border,
                            width: if border.a > 0. { 1. } else { 0. },
                            radius: radius.into(),
                        },
                        shadow,
                        ..Default::default()
                    }),
            )
            .padding(room),
            Rect {
                x: rect.x - room,
                y: rect.y - room,
                width: rect.width + 2. * room,
                height: rect.height + 2. * room,
            },
        );
    }
    pub fn avatar(&mut self, view: &SessionView, rect: Rect, settings: &Settings) {
        if let Some(source) = view.user_avatar.as_deref().or(view.avatar.as_ref()) {
            let handle = self.assets.borrow_mut().avatar(source);
            self.place(
                image(handle)
                    .width(rect.width)
                    .height(rect.height)
                    .opacity(self.alpha),
                rect,
            );
        } else if let Some(initials) = view.avatar_initials.as_ref() {
            self.initials(initials, rect, settings);
        }
    }
    /// A monogram on a gradient chosen from the initials.
    pub fn initials(&mut self, initials: &str, rect: Rect, settings: &Settings) {
        let (light, deep) = style::avatar_tones(initials);
        let look = Look::new(settings);
        self.raised_surface(
            rect,
            rect.width / 2.,
            iced::Gradient::Linear(
                iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI * 0.75))
                    .add_stop(0., light)
                    .add_stop(1., deep),
            ),
            iced::Color::from_rgba(1., 1., 1., 0.14),
            iced::Shadow {
                color: iced::Color::from_rgba(0., 0., 0., if look.dark { 0.22 } else { 0.10 }),
                offset: iced::Vector::new(0., rect.width * 0.03),
                blur_radius: (rect.width * 0.12).min(12.),
            },
        );
        self.label(
            initials.to_uppercase(),
            rect,
            rect.width * 0.34,
            Rgb(255, 255, 255),
            true,
            iced::font::Weight::Medium,
        );
    }
    /// An account without a picture or initials.
    pub fn anonymous(&mut self, rect: Rect, settings: &Settings) {
        let look = Look::new(settings);
        let p = look.palette;
        self.raised_surface(
            rect,
            rect.width / 2.,
            // Someone not yet identified: the accent, quietly, not a disabled grey.
            iced::Gradient::Linear(
                iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI * 0.75))
                    .add_stop(0., style::rgba(raster::mix(p.raised, p.accent, 0.45), 1.))
                    .add_stop(1., style::rgba(raster::mix(p.raised, p.accent, 0.15), 1.)),
            ),
            iced::Color::from_rgba(1., 1., 1., 0.16),
            look.shadow(rect.width * 0.16),
        );
        let side = rect.width * 0.5;
        self.icon(
            Emblem::Person,
            Rect {
                x: rect.x + (rect.width - side) / 2.,
                y: rect.y + (rect.height - side) / 2.,
                width: side,
                height: side,
            },
            p.text,
        );
    }
    /// A button whose look is chosen by the caller.
    #[allow(clippy::too_many_arguments)]
    pub fn styled_button(
        &mut self,
        settings: &Settings,
        control: &Control,
        rect: Rect,
        scale: f32,
        kind: Kind,
        radius: f32,
        content: Content<'_>,
    ) {
        let look = Look::new(settings);
        let color = if control.enabled {
            look.foreground(kind)
        } else {
            look.palette.disabled
        };
        let icon_side = 20. * scale;
        let alpha = self.alpha;
        let icon = |assets: &mut Assets, emblem: Emblem, side: f32, color: Rgb| {
            image(assets.icon(emblem, side.ceil().max(1.) as u32, color, None))
                .width(side)
                .height(side)
                .opacity(alpha)
        };
        let rtl = canopa_i18n::direction().is_rtl();
        let element: Element<'static> = match content {
            Content::Icon(emblem, side) => {
                let side = side * scale;
                container(icon(&mut self.assets.borrow_mut(), emblem, side, color))
                    .center(Length::Fill)
                    .into()
            }
            Content::Text {
                label,
                size,
                weight,
                centered,
            } => {
                let label = clean(label);
                let size = self.fit_size(
                    &label,
                    Rect {
                        width: (rect.width - 24. * scale).max(1.),
                        height: (rect.height - 8. * scale).max(1.),
                        ..rect
                    },
                    size * scale,
                    weight,
                    false,
                );
                container(
                    text(label)
                        .wrapping(iced::widget::text::Wrapping::None)
                        .size(size)
                        .font(Font {
                            weight,
                            ..Font::DEFAULT
                        })
                        .shaping(iced::advanced::text::Shaping::Advanced)
                        .width(Length::Fill)
                        .align_x(if centered {
                            iced::alignment::Horizontal::Center
                        } else if rtl {
                            iced::alignment::Horizontal::Right
                        } else {
                            iced::alignment::Horizontal::Left
                        }),
                )
                .clip(true)
                .center_y(Length::Fill)
                .into()
            }
            Content::Chip {
                icon: leading,
                label,
                trailing,
            } => {
                let label = clean(label);
                let mut assets = self.assets.borrow_mut();
                let mut items: Vec<Element<'static>> = Vec::new();
                if let Some(emblem) = leading {
                    items.push(icon(&mut assets, emblem, icon_side * 0.9, color).into());
                }
                if !label.is_empty() {
                    items.push(
                        text(label)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .size(15. * scale)
                            .font(Font {
                                weight: iced::font::Weight::Medium,
                                ..Font::DEFAULT
                            })
                            .shaping(iced::advanced::text::Shaping::Advanced)
                            .into(),
                    );
                }
                if let Some(emblem) = trailing {
                    let tone = if matches!(kind, Kind::Primary | Kind::Resting) {
                        color
                    } else {
                        look.palette.muted
                    };
                    items.push(icon(&mut assets, emblem, icon_side * 0.7, tone).into());
                }
                if rtl {
                    items.reverse();
                }
                container(
                    iced::widget::Row::with_children(items)
                        .spacing(8. * scale)
                        .align_y(iced::alignment::Vertical::Center),
                )
                .clip(true)
                .center(Length::Fill)
                .into()
            }
            Content::Row {
                leading,
                title,
                detail,
                checked,
            } => {
                let mut items: Vec<Element<'static>> = Vec::new();
                // Sign-in account rows show larger pictures than list rows.
                // Account and choice rows share one picture size.
                let lead = if rect.height >= 60. * scale { 48. } else { 40. } * scale;
                if leading.is_some() {
                    items.push(iced::widget::space().width(lead).height(lead).into());
                }
                let mut lines = vec![
                    text(clean(title))
                        .wrapping(iced::widget::text::Wrapping::None)
                        .size(15. * scale)
                        .font(Font {
                            weight: iced::font::Weight::Semibold,
                            ..Font::DEFAULT
                        })
                        .shaping(iced::advanced::text::Shaping::Advanced)
                        .into(),
                ];
                if let Some(detail) = detail.filter(|d| !d.is_empty()) {
                    lines.push(
                        text(clean(detail))
                            .wrapping(iced::widget::text::Wrapping::None)
                            .size(13. * scale)
                            .color(style::rgba(look.palette.muted, alpha))
                            .shaping(iced::advanced::text::Shaping::Advanced)
                            .into(),
                    );
                }
                items.push(
                    container(iced::widget::Column::with_children(lines).spacing(1. * scale))
                        .clip(true)
                        .width(Length::Fill)
                        .align_x(if rtl {
                            iced::alignment::Horizontal::Right
                        } else {
                            iced::alignment::Horizontal::Left
                        })
                        .into(),
                );
                if checked {
                    items.push(
                        icon(
                            &mut self.assets.borrow_mut(),
                            Emblem::Selected,
                            22. * scale,
                            look.palette.accent,
                        )
                        .into(),
                    );
                } else if matches!(
                    leading,
                    Some(Leading::Initials(_) | Leading::Avatar(_) | Leading::Icon(Emblem::Person))
                ) {
                    // Account rows lead on to that account's sign-in step.
                    items.push(
                        icon(
                            &mut self.assets.borrow_mut(),
                            Emblem::Chevron,
                            16. * scale,
                            look.palette.muted,
                        )
                        .into(),
                    );
                }
                if rtl {
                    items.reverse();
                }
                container(
                    iced::widget::Row::with_children(items)
                        .spacing(14. * scale)
                        .align_y(iced::alignment::Vertical::Center),
                )
                .center_y(Length::Fill)
                .into()
            }
            Content::Tile { icon: emblem, label } => {
                let label = clean(label);
                let mut assets = self.assets.borrow_mut();
                container(
                    iced::widget::column![
                        icon(&mut assets, emblem, 30. * scale, color),
                        text(label)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .size(14. * scale)
                            .font(Font {
                                weight: iced::font::Weight::Medium,
                                ..Font::DEFAULT
                            })
                            .shaping(iced::advanced::text::Shaping::Advanced),
                    ]
                    .spacing(10. * scale)
                    .align_x(iced::alignment::Horizontal::Center),
                )
                .clip(true)
                .center(Length::Fill)
                .into()
            }
        };
        let focused = control.selected;
        let widget = button(element)
            .width(rect.width)
            .height(rect.height)
            .padding(match content {
                Content::Row { .. } => iced::Padding::from([6. * scale, 14. * scale]),
                Content::Icon(..) => iced::Padding::new(0.),
                _ => iced::Padding::from([4. * scale, 12. * scale]),
            })
            .on_press_maybe(control.enabled.then_some(Message::Control(control.id)))
            // Keyboard focus is a separate ring outside the control, so it never
            // reads as the current choice.
            .style(style::button_style(look, kind, radius, false, alpha));
        if focused && control.enabled && !self.field_focus {
            let gap = 2.5 * scale;
            let ring = 2. * scale;
            self.surface(
                Rect {
                    x: rect.x - gap - ring,
                    y: rect.y - gap - ring,
                    width: rect.width + 2. * (gap + ring),
                    height: rect.height + 2. * (gap + ring),
                },
                radius + gap + ring,
                iced::Color::TRANSPARENT,
                style::rgba(look.focus, 0.95),
                ring,
                iced::Shadow::default(),
            );
        }
        self.hits.push((control.id, rect));
        if matches!(content, Content::Icon(..)) {
            self.place(
                tooltip(
                    widget,
                    container(text(clean(&control.label)).size(13. * scale))
                        .padding(8. * scale)
                        .style(canopa_ui::surface),
                    tooltip::Position::Top,
                )
                .snap_within_viewport(true),
                rect,
            );
        } else {
            self.place(widget, rect);
        }
        if let Content::Row {
            leading: Some(leading),
            ..
        } = content
        {
            let side = if rect.height >= 60. * scale { 48. } else { 40. } * scale;
            let avatar = Rect {
                x: if rtl {
                    rect.x + rect.width - 14. * scale - side
                } else {
                    rect.x + 14. * scale
                },
                y: rect.y + (rect.height - side) / 2.,
                width: side,
                height: side,
            };
            match leading {
                Leading::Initials(initials) => self.initials(initials, avatar, settings),
                Leading::Avatar(view) => self.avatar(view, avatar, settings),
                Leading::Icon(emblem) => {
                    self.surface(
                        avatar,
                        side / 2.,
                        look.tint(1.),
                        iced::Color::TRANSPARENT,
                        0.,
                        iced::Shadow::default(),
                    );
                    let inner = 22. * scale;
                    self.icon(
                        emblem,
                        Rect {
                            x: avatar.x + (side - inner) / 2.,
                            y: avatar.y + (side - inner) / 2.,
                            width: inner,
                            height: inner,
                        },
                        look.palette.accent,
                    );
                }
            }
        }
    }
    pub fn button(
        &mut self,
        view: &SessionView,
        settings: &Settings,
        control: &Control,
        rect: Rect,
        scale: f32,
        quiet: bool,
    ) {
        self.button_detail(view, settings, control, rect, scale, quiet, None);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn button_detail(
        &mut self,
        view: &SessionView,
        settings: &Settings,
        control: &Control,
        mut rect: Rect,
        scale: f32,
        quiet: bool,
        detail: Option<&str>,
    ) {
        let primary = view.primary_control == Some(control.id) && !view.busy;
        let kind = if primary
            && view.prompt.as_ref().is_some_and(|p| !p.echo)
            && !view.hide_input
            && view.secret.is_empty()
        {
            // Nothing typed yet: the default action rests until there is.
            Kind::Resting
        } else if primary {
            Kind::Primary
        } else if control.kind == ControlKind::Back {
            // One style for going back everywhere: a secondary button with an arrow.
            if self.glass { Kind::Glass } else { Kind::Secondary }
        } else if quiet {
            Kind::Quiet
        } else if self.glass {
            Kind::Glass
        } else {
            Kind::Secondary
        };
        let radius = if self.glass {
            rect.height.min(40. * scale) / 2.
        } else {
            10. * scale
        };
        match &control.kind {
            ControlKind::Back => {
                let label = clean(&control.label);
                let available_width = rect.width;
                rect.width = (view.text_width(&label, 14. * scale) + 64. * scale).min(rect.width);
                if canopa_i18n::direction().is_rtl() {
                    rect.x += available_width - rect.width;
                }
                self.styled_button(
                    settings,
                    control,
                    rect,
                    scale,
                    kind,
                    radius,
                    Content::Chip {
                        icon: Some(Emblem::ArrowBack),
                        label: &label,
                        trailing: None,
                    },
                );
            }
            ControlKind::Account {
                name,
                detail: account_detail,
                initials,
            } => {
                let leading = if control.selected
                    && (view.user_avatar.is_some() || view.avatar.is_some())
                {
                    Leading::Avatar(view)
                } else {
                    Leading::Initials(initials)
                };
                self.styled_button(
                    settings,
                    control,
                    rect,
                    scale,
                    if self.glass { Kind::Glass } else { kind },
                    16. * scale,
                    Content::Row {
                        leading: Some(leading),
                        title: name,
                        detail: account_detail.as_deref().or(detail),
                        checked: false,
                    },
                );
            }
            ControlKind::Tile(emblem) => self.styled_button(
                settings,
                control,
                rect,
                scale,
                // Keyboard selection is the focus ring alone: no choice is preselected.
                kind,
                18. * scale,
                Content::Tile {
                    icon: *emblem,
                    label: &control.label,
                },
            ),
            ControlKind::AccountChooser { text } => self.styled_button(
                settings,
                control,
                rect,
                scale,
                kind,
                radius,
                Content::Text {
                    label: text,
                    size: 14.,
                    weight: iced::font::Weight::Medium,
                    centered: true,
                },
            ),
            _ => {
                let mut label = clean(&control.label);
                if let Some(detail) = detail {
                    label.push_str(" · ");
                    label.push_str(&clean(detail));
                }
                self.styled_button(
                    settings,
                    control,
                    rect,
                    scale,
                    if control.kind == ControlKind::Destructive {
                        Kind::Danger
                    } else {
                        kind
                    },
                    radius,
                    Content::Text {
                        label: &label,
                        size: 14.,
                        weight: if kind == Kind::Primary {
                            iced::font::Weight::Semibold
                        } else {
                            iced::font::Weight::Medium
                        },
                        centered: true,
                    },
                );
            }
        }
    }
    pub fn icon_button(
        &mut self,
        view: &SessionView,
        settings: &Settings,
        control: &Control,
        icon: Emblem,
        rect: Rect,
        scale: f32,
    ) {
        let primary = view.primary_control == Some(control.id) && !view.busy;
        let kind = if primary {
            Kind::Primary
        } else if self.glass && control.kind == ControlKind::Power {
            Kind::Glass
        } else {
            Kind::Quiet
        };
        self.styled_button(
            settings,
            control,
            rect,
            scale,
            kind,
            rect.width.min(rect.height) / 2.,
            Content::Icon(icon, if primary { 22. } else { 20. }),
        );
    }
    pub fn field(
        &mut self,
        view: &SessionView,
        settings: &Settings,
        rect: Rect,
        scale: f32,
        placeholder: &str,
    ) {
        self.hits.push((crate::input::INPUT_CONTROL, rect));
        let palette = settings.appearance.palette();
        let reveal = view.prompt.as_ref().is_some_and(|p| p.echo) || view.revealed();
        let maximum = ((rect.width / (10. * scale)).floor() as usize).clamp(1, 64);
        let placeholder = clean(placeholder);
        let stamp = (
            view.secret.revision(),
            view.secret.anchor(),
            view.secret.cursor(),
            reveal,
            view.interaction_epoch,
        );
        let key = format!(
            "{:?}",
            (
                stamp,
                rect.width,
                rect.height,
                scale,
                &placeholder,
                palette,
                view.caps,
                canopa_i18n::direction().is_rtl()
            )
        );
        let mut assets = self.assets.borrow_mut();
        if assets.field_stamp.as_ref() != Some(&format!("{stamp:?}")) {
            assets.fields.clear();
            assets.field_stamp = Some(format!("{stamp:?}"));
        }
        let prepared = if let Some((_, prepared)) =
            assets.fields.iter().find(|(old, _)| old == &key)
        {
            prepared.clone()
        } else {
            let prepared = {
                let (display, selection, cursor) = view.secret.presentation(reveal, maximum);
                let width = rect.width.ceil().max(1.) as u32;
                let height = rect.height.ceil().max(1.) as u32;
                let available = (rect.width - 28. * scale).max(1.);
                let mut text = view.text_renderer.borrow_mut();
                let carets = text.carets(&display, 17. * scale, available);
                let indices = view.secret.display_indices(reveal, maximum);
                let rtl = canopa_i18n::direction().is_rtl();
                let text_width = text.width(&display, 17. * scale).min(available);
                let offset = 14. * scale + if rtl { available - text_width } else { 0. };
                let positions = indices
                    .iter()
                    .map(|(display, original)| {
                        (
                            *original,
                            offset
                                + carets
                                    .iter()
                                    .find(|(i, _)| i == display)
                                    .map_or(0., |(_, x)| *x),
                        )
                    })
                    .collect();
                let style = if rtl {
                    crate::text::Style::Leading(cosmic_text::Weight::NORMAL).mirrored()
                } else {
                    crate::text::Style::Leading(cosmic_text::Weight::NORMAL)
                };
                let x = if rtl {
                    rect.width - 14. * scale
                } else {
                    14. * scale
                };
                let origin = (
                    x,
                    rect.height / 2. + 6. * scale,
                );
                // Draw over black and over white, then recover coverage and
                // colour, so the field's own (translucent) surface shows through.
                let caret = view.prompt.is_some() && !view.busy && !view.hide_input;
                let mut draw = |base: u8| {
                    let mut bitmap = ::image::RgbImage::from_pixel(width, height, ::image::Rgb([base; 3]));
                    if display.is_empty() {
                        text.draw_styled(
                            &mut bitmap,
                            &placeholder,
                            // Leave the caret its own space before the placeholder.
                            (
                                origin.0 + if !caret {
                                    0.
                                } else if rtl {
                                    -6. * scale
                                } else {
                                    6. * scale
                                },
                                origin.1,
                            ),
                            15. * scale,
                            available,
                            palette.muted,
                            None,
                            style,
                        );
                        // An empty field that takes input still shows where typing goes.
                        if caret {
                            let focus = settings.appearance.role(canopa_theme::ColorRole::Focus);
                            let x = (origin.0 - if rtl { 2. * scale } else { 2. * scale })
                                .max(0.) as u32;
                            let half = (9. * scale) as u32;
                            let middle = (rect.height / 2.) as u32;
                            for y in middle.saturating_sub(half)..(middle + half).min(height) {
                                for dx in 0..(1.5 * scale).ceil().max(1.) as u32 {
                                    if x + dx < width {
                                        bitmap.put_pixel(
                                            x + dx,
                                            y,
                                            ::image::Rgb([focus.0, focus.1, focus.2]),
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        text.draw_styled(
                            &mut bitmap,
                            &display,
                            origin,
                            17. * scale,
                            available,
                            palette.text,
                            // A read-only field (being checked) shows no caret.
                            (!view.busy).then(|| {
                                (
                                    selection.clone(),
                                    cursor,
                                    settings.appearance.role(canopa_theme::ColorRole::Focus),
                                )
                            }),
                            style,
                        );
                    }
                    bitmap
                };
                let black = draw(0);
                let white = draw(255);
                let mut bitmap = ::image::RgbaImage::new(width, height);
                for ((target, dark), light) in bitmap
                    .pixels_mut()
                    .zip(black.pixels())
                    .zip(white.pixels())
                {
                    let coverage = (0..3)
                        .map(|c| 255 - i32::from(light[c]) + i32::from(dark[c]))
                        .max()
                        .unwrap_or(0)
                        .clamp(0, 255);
                    if coverage == 0 {
                        continue;
                    }
                    let alpha = coverage as f32 / 255.;
                    *target = ::image::Rgba([
                        (f32::from(dark[0]) / alpha).round().min(255.) as u8,
                        (f32::from(dark[1]) / alpha).round().min(255.) as u8,
                        (f32::from(dark[2]) / alpha).round().min(255.) as u8,
                        coverage as u8,
                    ]);
                }
                let handle = image::Handle::from_rgba(width, height, bitmap.into_raw());
                PreparedField { handle, positions }
            };
            if assets.fields.len() == 16 {
                assets.fields.pop_front();
            }
            assets.fields.push_back((key, prepared.clone()));
            prepared
        };
        drop(assets);
        self.place(
            Element::new(SecureInput {
                image: prepared.handle,
                size: Size::new(rect.width, rect.height),
                positions: prepared.positions,
                anchor: view.secret.anchor(),
                enabled: view.prompt.is_some() && !view.hide_input,
                opacity: self.alpha,
            }),
            rect,
        );
    }
}

/// The widget owns pixels and caret positions, never a String containing a response.
struct SecureInput {
    image: image::Handle,
    size: Size,
    positions: Vec<(usize, f32)>,
    anchor: usize,
    enabled: bool,
    opacity: f32,
}
#[derive(Default)]
struct Selection {
    anchor: Option<usize>,
}
impl Widget<Message, Theme, Renderer> for SecureInput {
    fn size(&self) -> Size<Length> {
        Size::new(self.size.width.into(), self.size.height.into())
    }
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<Selection>()
    }
    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(Selection::default())
    }
    fn layout(&mut self, _: &mut Tree, _: &Renderer, _: &layout::Limits) -> layout::Node {
        layout::Node::new(self.size)
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _: &Renderer,
        _: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _: &Rectangle,
    ) {
        if !self.enabled {
            return;
        }
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<Selection>();
        let position = cursor.position().map(|p| p.x - bounds.x);
        let at = |x: f32| {
            self.positions
                .iter()
                .min_by(|(_, a), (_, b)| (a - x).abs().total_cmp(&(b - x).abs()))
                .map_or(0, |(i, _)| *i)
        };
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if cursor.is_over(bounds) =>
            {
                let cursor = at(position.unwrap_or(0.));
                state.anchor = Some(cursor);
                shell.publish(Message::Selection {
                    anchor: cursor,
                    cursor,
                });
                shell.capture_event();
                shell.request_redraw();
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) if state.anchor.is_some() => {
                shell.publish(Message::Selection {
                    anchor: state.anchor.unwrap_or(self.anchor),
                    cursor: at(position.unwrap_or(0.)),
                });
                shell.capture_event();
                shell.request_redraw();
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.anchor = None;
            }
            iced::Event::Mouse(mouse::Event::CursorLeft) => {
                state.anchor = None;
            }
            _ => {}
        }
    }
    fn mouse_interaction(
        &self,
        _: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _: &Rectangle,
        _: &Renderer,
    ) -> mouse::Interaction {
        if self.enabled && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::None
        }
    }
    fn draw(
        &self,
        _: &Tree,
        renderer: &mut Renderer,
        _: &Theme,
        _: &renderer::Style,
        layout: Layout<'_>,
        _: mouse::Cursor,
        _: &Rectangle,
    ) {
        let bounds = layout.bounds();
        renderer.draw_image(
            iced::advanced::image::Image::new(self.image.clone()).opacity(self.opacity),
            bounds,
            bounds,
        );
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    fn fixture() -> (Settings, SessionView) {
        let settings = Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Lockscreen,
        )
        .unwrap()
        .0;
        let view = SessionView::new(&settings, "Alice".into()).unwrap();
        (settings, view)
    }
    pub(in crate::view) fn rasterize(
        scene: Scene,
        size: Size,
        settings: &Settings,
    ) -> tiny_skia::Pixmap {
        let mut renderer = Renderer::Secondary(iced_tiny_skia::Renderer::new(
            Font::DEFAULT,
            iced::Pixels(14.),
        ));
        let mut ui = iced_runtime::UserInterface::build(
            scene.element(size),
            size,
            iced_runtime::user_interface::Cache::new(),
            &mut renderer,
        );
        ui.draw(
            &mut renderer,
            &canopa_ui::theme(settings.appearance.clone()),
            &renderer::Style {
                text_color: iced::Color::WHITE,
            },
            mouse::Cursor::Unavailable,
        );
        let Renderer::Secondary(renderer) = &mut renderer else {
            unreachable!()
        };
        let mut pixels = tiny_skia::Pixmap::new(size.width as u32, size.height as u32).unwrap();
        let mut mask = tiny_skia::Mask::new(size.width as u32, size.height as u32).unwrap();
        renderer.draw(
            &mut pixels.as_mut(),
            &mut mask,
            &iced_renderer::graphics::Viewport::with_physical_size(
                Size::new(size.width as u32, size.height as u32),
                1.,
            ),
            &[Rectangle::with_size(size)],
            iced::Color::TRANSPARENT,
        );
        pixels
    }
    #[test]
    fn title_newline_cannot_create_an_extra_rendered_dialog_line() {
        let (settings, view) = fixture();
        let render = |title: &str| {
            let mut scene = Scene::new(view.assets.clone());
            scene.label(
                title,
                Rect {
                    x: 0.,
                    y: 0.,
                    width: 400.,
                    height: 80.,
                },
                21.,
                Rgb(255, 255, 255),
                false,
                iced::font::Weight::Semibold,
            );
            rasterize(scene, Size::new(400., 80.), &settings)
        };
        assert_eq!(
            render("Trusted\nForged").data(),
            render("TrustedForged").data()
        );
    }
    #[test]
    fn labels_strip_controls_and_cap_unicode_without_accepting_injected_lines() {
        assert_eq!(clean("Trusted\nForged\r\t\0label"), "TrustedForgedlabel");
        assert_eq!(clean(&"é".repeat(300)).chars().count(), 256);
    }
    #[test]
    fn hidpi_lines_keep_their_size_and_the_reserved_row_advance() {
        let _english = crate::i18n::english();
        let (settings, view) = fixture();
        // A 4K lock: widget text of size 19 at scale 2.7 reserves
        // (19 + 7) * 2.7 per line; the advance must scale with the text.
        let scale = 2.7;
        let size = 19. * scale;
        let advance = (19. + 7.) * scale;
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 600.,
            height: 2. * advance,
        };
        let mut scene = Scene::new(view.assets.clone());
        let row = Rect {
            height: advance,
            ..rect
        };
        assert_eq!(
            scene.fit_size("Monday", row, size, iced::font::Weight::Normal, false),
            size,
            "a reserved row must not shrink its text"
        );
        scene.lines(
            &["Monday".into(), "Monday".into()],
            rect,
            size,
            advance,
            Rgb(255, 255, 255),
            true,
            iced::font::Weight::Normal,
        );
        let pixels = rasterize(scene, Size::new(600., 2. * advance), &settings);
        let rows: Vec<_> = pixels
            .data()
            .chunks_exact(4)
            .enumerate()
            .filter(|(_, p)| p[3] != 0)
            .map(|(i, _)| (i / 600) as f32)
            .collect();
        let second: Vec<_> = rows.iter().copied().filter(|y| *y >= advance).collect();
        assert!(!second.is_empty());
        let centre = second.iter().sum::<f32>() / second.len() as f32;
        // Centred in its own row at 1.5 * advance, not packed at size + 7.
        assert!(
            (centre - 1.5 * advance).abs() < 0.15 * advance,
            "second line centred at {centre}, expected about {}",
            1.5 * advance
        );
    }

    #[test]
    fn long_latin_and_cjk_labels_fit_and_clip_to_their_panel_bounds() {
        let _english = crate::i18n::english();
        let (settings, view) = fixture();
        let rect = Rect {
            x: 20.,
            y: 10.,
            width: 360.,
            height: 32.,
        };
        for value in [
            format!("{}END", "Wide account name ".repeat(14)),
            "漢字かな交じり文".repeat(20),
        ] {
            let value = clean(&value);
            let mut scene = Scene::new(view.assets.clone());
            let fitted = scene.fit_size(&value, rect, 21., iced::font::Weight::Semibold, false);
            assert!(fitted < 21.);
            assert!(
                view.text_renderer.borrow_mut().styled_width(
                    &value,
                    fitted,
                    cosmic_text::Weight::SEMIBOLD,
                    false
                ) <= rect.width + 1.
            );
            scene.label(
                value.clone(),
                rect,
                21.,
                Rgb(255, 255, 255),
                false,
                iced::font::Weight::Semibold,
            );
            let pixels = rasterize(scene, Size::new(400., 60.), &settings);
            let visible: Vec<_> = pixels
                .data()
                .chunks_exact(4)
                .enumerate()
                .filter(|(_, p)| p[3] != 0)
                .map(|(i, _)| (i % 400, i / 400))
                .collect();
            assert!(!visible.is_empty());
            assert!(
                visible
                    .iter()
                    .all(|&(x, y)| (20..380).contains(&x) && (10..42).contains(&y)),
                "text must stay in its panel"
            );
            assert!(
                visible.iter().any(|&(x, _)| x > 360),
                "the end of the fitted label must remain visible"
            );
        }
    }

    #[test]
    fn rtl_twelve_hour_clock_puts_the_small_period_before_the_digits() {
        let _rtl = crate::i18n::Selected::new("en", true);
        let (settings, view) = fixture();
        let width = view.clock_width("11:59 PM", 100., true).ceil();
        let mut scene = Scene::new(view.assets.clone());
        scene.clock(
            &view,
            "11:59 PM",
            Rect {
                x: 0.,
                y: 0.,
                width,
                height: 130.,
            },
            100.,
            Rgb(255, 255, 255),
        );
        let pixels = rasterize(scene, Size::new(width, 130.), &settings);
        let period_width = view.text_width("PM", 34.).ceil() as usize;
        let period_y: Vec<_> = pixels
            .data()
            .chunks_exact(4)
            .enumerate()
            .filter(|(i, p)| i % (width as usize) < period_width && p[3] > 0)
            .map(|(i, _)| i / width as usize)
            .collect();
        assert!(!period_y.is_empty(), "PM must appear on the left in RTL");
        assert!(
            period_y.iter().max().unwrap() - period_y.iter().min().unwrap() < 45,
            "the left part must be the small period, not the full-size clock digits"
        );
        assert!(*period_y.iter().min().unwrap() > 40);
    }
    #[test]
    fn cached_icons_avatars_and_fields_keep_their_image_identity() {
        let (settings, mut view) = fixture();
        let mut assets = Assets::default();
        assert_eq!(
            assets.icon(Emblem::Shield, 24, Rgb(1, 2, 3), None).id(),
            assets.icon(Emblem::Shield, 24, Rgb(1, 2, 3), None).id()
        );
        let source = ::image::RgbaImage::from_pixel(64, 64, ::image::Rgba([255; 4]));
        assert_eq!(assets.avatar(&source).id(), assets.avatar(&source).id());
        view.prompt = Some(super::super::Prompt {
            id: 1,
            text: "Password".into(),
            echo: false,
        });
        view.secret.push("fixture");
        let mut scene = Scene::new(view.assets.clone());
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 300.,
            height: 48.,
        };
        scene.field(&view, &settings, rect, 1., "Enter response");
        let first = view.assets.borrow().fields[0].1.handle.id();
        scene.field(&view, &settings, rect, 1., "Enter response");
        assert_eq!(view.assets.borrow().fields[0].1.handle.id(), first);
        assert_eq!(view.assets.borrow().fields.len(), 1);
        view.secret.clear();
        scene.field(&view, &settings, rect, 1., "Enter response");
        assert_ne!(view.assets.borrow().fields[0].1.handle.id(), first);
        assert_eq!(
            view.assets.borrow().fields.len(),
            1,
            "old response pixels must retire"
        );
    }
    #[test]
    fn battery_charge_changes_the_fill_and_rtl_mirrors_response_carets() {
        let (settings, mut view) = fixture();
        let mut assets = Assets::default();
        let count = |handle| {
            let image::Handle::Rgba { pixels, .. } = handle else {
                panic!("RGBA icon")
            };
            pixels.chunks_exact(4).filter(|p| p[3] > 0).count()
        };
        let empty = count(assets.icon(Emblem::Battery, 48, Rgb(1, 2, 3), Some(0.)));
        let full = count(assets.icon(Emblem::Battery, 48, Rgb(1, 2, 3), Some(1.)));
        assert!(full > empty + 200);
        view.prompt = Some(super::super::Prompt {
            id: 1,
            text: "Password".into(),
            echo: false,
        });
        view.secret.push("abc");
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 300.,
            height: 48.,
        };
        let x = |view: &SessionView| {
            let mut scene = Scene::new(view.assets.clone());
            scene.field(view, &settings, rect, 1., "Enter response");
            view.assets.borrow().fields.back().unwrap().1.positions[0].1
        };
        let ltr = x(&view);
        let rtl = {
            let _rtl = crate::i18n::Selected::new("en", true);
            x(&view)
        };
        assert!(
            rtl > ltr + 150.,
            "masked input must align with the RTL field edge"
        );
    }
    #[test]
    fn caps_lock_leaves_the_response_bitmap_in_the_text_colour() {
        let (settings, mut view) = fixture();
        view.prompt = Some(super::super::Prompt {
            id: 1,
            text: "Password".into(),
            echo: true,
        });
        view.secret.push("AAABBB");
        let warning = settings.appearance.palette().warning;
        let count = |view: &SessionView| {
            let mut scene = Scene::new(view.assets.clone());
            scene.field(
                view,
                &settings,
                Rect {
                    x: 0.,
                    y: 0.,
                    width: 300.,
                    height: 48.,
                },
                1.,
                "Enter response",
            );
            let assets = view.assets.borrow();
            let image::Handle::Rgba { pixels, .. } = &assets.fields.back().unwrap().1.handle else {
                unreachable!()
            };
            pixels
                .chunks_exact(4)
                .filter(|p| p[..3] == [warning.0, warning.1, warning.2])
                .count()
        };
        assert_eq!(count(&view), 0);
        // The warning is a message below the field; the typed text keeps its
        // colour, so it never reads as an error.
        view.caps = true;
        assert_eq!(count(&view), 0);
    }

    #[test]
    fn twelve_hour_clock_keeps_day_period_within_one_clock_line() {
        let (settings, view) = fixture();
        let width = view.clock_width("11:59 PM", 100., true);
        let mut scene = Scene::new(view.assets.clone());
        scene.clock(
            &view,
            "11:59 PM",
            Rect {
                x: 0.,
                y: 0.,
                width,
                height: 130.,
            },
            100.,
            Rgb(255, 255, 255),
        );
        let size = Size::new(width.ceil(), 130.);
        let pixels = rasterize(scene, size, &settings);
        let start = view.text_width("11:59", 100.).ceil() as u32 + 8;
        let ys: Vec<_> = pixels
            .data()
            .chunks_exact(4)
            .enumerate()
            .filter(|(i, p)| (*i as u32 % size.width as u32) >= start && p[3] > 0)
            .map(|(i, _)| i as u32 / size.width as u32)
            .collect();
        assert!(
            !ys.is_empty(),
            "the day period must remain visible beside the digits"
        );
        assert!(
            ys.iter().max().unwrap() - ys.iter().min().unwrap() < 45,
            "day period must use the smaller font"
        );
        assert!(
            *ys.iter().max().unwrap() < 110,
            "day period must not wrap into the date"
        );
    }
}
