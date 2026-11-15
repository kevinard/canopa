//! The interactive session view uses the same Iced controls as the applications.
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
}
impl Scene {
    pub(super) fn new(assets: std::rc::Rc<std::cell::RefCell<Assets>>) -> Self {
        Self {
            elements: Vec::new(),
            assets,
            hits: Vec::new(),
        }
    }
    pub fn empty(&self) -> Self {
        Self::new(self.assets.clone())
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
                    y: rect.y + if period { size * 0.54 } else { 0. },
                    width,
                    height: part_size * 1.3,
                },
                part_size,
                color,
                true,
                iced::font::Weight::Light,
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
            .color(canopa_ui::color(color))
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
    pub fn frame(&mut self, rect: Rect, radius: f32, fill: Rgb, border: Rgb, scale: f32) {
        self.place(
            container(iced::widget::space())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(canopa_ui::color(fill).into()),
                    border: iced::Border {
                        color: canopa_ui::color(border),
                        width: scale,
                        radius: radius.into(),
                    },
                    ..Default::default()
                }),
            rect,
        );
    }
    pub fn icon(&mut self, icon: Emblem, rect: Rect, color: Rgb) {
        self.status_icon(icon, rect, color, None);
    }
    pub fn status_icon(&mut self, icon: Emblem, rect: Rect, color: Rgb, level: Option<f32>) {
        let side = rect.width.max(rect.height).ceil().max(1.) as u32;
        let handle = self.assets.borrow_mut().icon(icon, side, color, level);
        self.place(image(handle).width(rect.width).height(rect.height), rect);
    }
    pub fn avatar(&mut self, view: &SessionView, rect: Rect, settings: &Settings) {
        if let Some(source) = view.user_avatar.as_deref().or(view.avatar.as_ref()) {
            let handle = self.assets.borrow_mut().avatar(source);
            self.place(image(handle).width(rect.width).height(rect.height), rect);
        } else if let Some(initials) = view.avatar_initials.as_ref() {
            self.frame(
                rect,
                rect.width / 2.,
                settings.appearance.palette().surface,
                settings.appearance.palette().border,
                0.,
            );
            self.label(
                initials.clone(),
                rect,
                rect.width * 0.36,
                settings.appearance.palette().text,
                true,
                iced::font::Weight::Semibold,
            );
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
        let available_width = rect.width;
        let mut label = clean(&control.label);
        if let ControlKind::AccountChooser { text } = &control.kind {
            label = clean(text);
        }
        if let ControlKind::Account { name, detail, .. } = &control.kind {
            label = clean(name);
            if let Some(detail) = detail {
                label.push('\n');
                label.push_str(&clean(detail));
            }
        }
        if control.kind == ControlKind::Back {
            rect.width = (view.text_width(&label, 14. * scale) + 44. * scale).min(rect.width);
            if canopa_i18n::direction().is_rtl() {
                rect.x += available_width - rect.width;
            }
            label = fl!(
                "session-back-label",
                arrow = if canopa_i18n::direction().is_rtl() {
                    "→"
                } else {
                    "←"
                },
                label = label.as_str()
            );
        }
        if let Some(detail) = detail {
            label.push('\n');
            label.push_str(&clean(detail));
        }
        let primary = view.primary_control == Some(control.id) && !view.busy;
        let action = if primary {
            canopa_ui::Action::Primary
        } else if quiet {
            canopa_ui::Action::Quiet
        } else {
            canopa_ui::Action::Normal
        };
        let focused = control.selected;
        let account = matches!(control.kind, ControlKind::Account { .. });
        let label_size = self.fit_size(
            &label,
            Rect {
                width: (rect.width - if account { 92. } else { 16. } * scale).max(1.),
                height: (rect.height - 16. * scale).max(1.),
                ..rect
            },
            14. * scale,
            iced::font::Weight::Normal,
            false,
        );
        let content = text(label)
            .wrapping(iced::widget::text::Wrapping::None)
            .size(label_size)
            .shaping(iced::advanced::text::Shaping::Advanced)
            .width(Length::Fill)
            .align_x(if account {
                if canopa_i18n::direction().is_rtl() {
                    iced::alignment::Horizontal::Right
                } else {
                    iced::alignment::Horizontal::Left
                }
            } else {
                iced::alignment::Horizontal::Center
            });
        let enabled = control.enabled;
        let widget = button(container(content).clip(true).center_y(Length::Fill))
            .width(rect.width)
            .height(rect.height)
            .padding(if account {
                let mut padding = iced::Padding::new(8. * scale);
                if canopa_i18n::direction().is_rtl() {
                    padding.right = 84. * scale;
                } else {
                    padding.left = 84. * scale;
                }
                padding
            } else {
                iced::Padding::new(8. * scale)
            })
            .on_press_maybe(enabled.then_some(Message::Control(control.id)))
            .style(move |theme, status| canopa_ui::action_style(theme, status, action, focused));
        self.hits.push((control.id, rect));
        self.place(widget, rect);
        if let ControlKind::Account { initials, .. } = &control.kind {
            let avatar = Rect {
                x: if canopa_i18n::direction().is_rtl() {
                    rect.x + rect.width - 72. * scale
                } else {
                    rect.x + 12. * scale
                },
                y: rect.y + (rect.height - 60. * scale) / 2.,
                width: 60. * scale,
                height: 60. * scale,
            };
            if control.selected && (view.user_avatar.is_some() || view.avatar.is_some()) {
                self.avatar(view, avatar, settings);
            } else {
                self.frame(
                    avatar,
                    30. * scale,
                    settings.appearance.palette().raised,
                    settings.appearance.palette().border,
                    scale,
                );
                self.label(
                    initials.clone(),
                    avatar,
                    20. * scale,
                    settings.appearance.palette().text,
                    true,
                    iced::font::Weight::Semibold,
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
        let side = (24. * scale).ceil().max(1.) as u32;
        let primary = view.primary_control == Some(control.id) && !view.busy;
        let color = if primary {
            settings.appearance.role(canopa_theme::ColorRole::OnAccent)
        } else {
            settings.appearance.palette().text
        };
        let handle = self.assets.borrow_mut().icon(icon, side, color, None);
        let content = image(handle).width(24. * scale).height(24. * scale);
        let focused = control.selected;
        let action = if primary {
            canopa_ui::Action::Primary
        } else {
            canopa_ui::Action::Quiet
        };
        let widget = button(container(content).center(Length::Fill))
            .width(rect.width)
            .height(rect.height)
            .padding(4. * scale)
            .on_press_maybe(control.enabled.then_some(Message::Control(control.id)))
            .style(move |theme, status| canopa_ui::action_style(theme, status, action, focused));
        self.hits.push((control.id, rect));
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
                let mut bitmap = ::image::RgbImage::from_pixel(
                    width,
                    height,
                    ::image::Rgb([palette.surface.0, palette.surface.1, palette.surface.2]),
                );
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
                if display.is_empty() {
                    text.draw_styled(
                        &mut bitmap,
                        &placeholder,
                        (
                            if rtl {
                                rect.width - 14. * scale
                            } else {
                                14. * scale
                            },
                            rect.height / 2. + 6. * scale,
                        ),
                        15. * scale,
                        available,
                        if view.caps {
                            palette.warning
                        } else {
                            palette.muted
                        },
                        None,
                        if rtl {
                            crate::text::Style::Leading(cosmic_text::Weight::NORMAL).mirrored()
                        } else {
                            crate::text::Style::Leading(cosmic_text::Weight::NORMAL)
                        },
                    );
                } else {
                    text.draw_styled(
                        &mut bitmap,
                        &display,
                        (
                            if rtl {
                                rect.width - 14. * scale
                            } else {
                                14. * scale
                            },
                            rect.height / 2. + 6. * scale,
                        ),
                        17. * scale,
                        available,
                        if view.caps {
                            palette.warning
                        } else {
                            palette.text
                        },
                        Some((
                            selection,
                            cursor,
                            settings.appearance.role(canopa_theme::ColorRole::Focus),
                        )),
                        if rtl {
                            crate::text::Style::Leading(cosmic_text::Weight::NORMAL).mirrored()
                        } else {
                            crate::text::Style::Leading(cosmic_text::Weight::NORMAL)
                        },
                    );
                }
                // tiny-skia's raster path does not clip an image to rounded corners.
                // Keep the response bitmap inside its pill instead of covering the
                // surrounding Iced frame with a rectangular background.
                let mut bitmap = ::image::DynamicImage::ImageRgb8(bitmap).to_rgba8();
                let radius = height.min(width) as f32 / 2.;
                for (x, y, pixel) in bitmap.enumerate_pixels_mut() {
                    let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
                    let dx = x - x.clamp(radius, width as f32 - radius);
                    let dy = y - y.clamp(radius, height as f32 - radius);
                    pixel[3] = ((radius + 0.5 - dx.hypot(dy)).clamp(0., 1.) * 255.).round() as u8;
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
            iced::advanced::image::Image::new(self.image.clone()),
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
    fn caps_lock_uses_the_warning_colour_in_the_response_bitmap() {
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
        view.caps = true;
        assert!(count(&view) > 5);
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
