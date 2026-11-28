//! Process appearance is replaced only when a complete configuration is installed.
use std::sync::{LazyLock, RwLock};

struct Installed {
    appearance: canopa_theme::Appearance,
    theme: iced::Theme,
    font: iced::Font,
}
static INSTALLED: LazyLock<RwLock<Installed>> = LazyLock::new(|| {
    RwLock::new(Installed {
        appearance: Default::default(),
        theme: canopa_ui::theme(Default::default()),
        font: iced::Font::DEFAULT,
    })
});

pub fn install(appearance: canopa_theme::Appearance) {
    let theme = canopa_ui::theme(appearance.clone());
    let font = canopa_ui::font(&appearance);
    *INSTALLED.write().expect("appearance lock") = Installed {
        appearance,
        theme,
        font,
    };
}

pub fn current() -> iced::Theme {
    INSTALLED.read().expect("appearance lock").theme.clone()
}

pub fn palette() -> canopa_ui::Colors {
    canopa_ui::colors(&current())
}

pub fn appearance() -> canopa_theme::Appearance {
    INSTALLED
        .read()
        .expect("appearance lock")
        .appearance
        .clone()
}
pub fn font() -> iced::Font {
    INSTALLED.read().expect("appearance lock").font
}
pub fn ui(kind: canopa_theme::UiKind) -> canopa_theme::ResolvedStyle {
    appearance().style(kind)
}
pub fn body_size() -> f32 {
    appearance().typography().body_size.px()
}
pub fn label_size() -> f32 {
    appearance().typography().label_size.px()
}
pub fn title_size() -> f32 {
    appearance().typography().title_size.px()
}
pub fn text<'a>(content: impl iced::widget::text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    iced::widget::text(content).font(font()).size(body_size())
}
pub fn strong_text<'a>(
    content: impl iced::widget::text::IntoFragment<'a>,
) -> iced::widget::Text<'a> {
    text(content).font(iced::Font {
        weight: iced::font::Weight::Semibold,
        ..font()
    })
}

pub fn accent_surface() -> iced::Color {
    canopa_ui::color(appearance().role(canopa_theme::ColorRole::AccentSurface))
}

pub fn icon<'a, Message: 'a>(
    bytes: &'static [u8],
    size: f32,
    color: iced::Color,
) -> iced::Element<'a, Message> {
    use iced::widget::svg;
    svg(svg::Handle::from_memory(bytes))
        .width(size)
        .height(size)
        .style(move |_, _| svg::Style { color: Some(color) })
        .into()
}

pub fn divider<'a, Message: 'a>() -> iced::Element<'a, Message> {
    iced::widget::rule::horizontal(1)
        .style(|theme| iced::widget::rule::Style {
            color: canopa_ui::colors(theme).border,
            radius: 0.into(),
            fill_mode: iced::widget::rule::FillMode::Full,
            snap: true,
        })
        .into()
}

pub fn panel_scroll<'a, Message: 'a>(
    content: impl Into<iced::Element<'a, Message>>,
) -> iced::widget::Scrollable<'a, Message> {
    use iced::widget::scrollable;
    scrollable(content)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(8)
                .scroller_width(4)
                .margin(6),
        ))
        .style(|theme, status| {
            let mut style = scrollable::default(theme, status);
            let palette = canopa_ui::colors(theme);
            // A quiet thumb at rest; brighter under the pointer.
            let engaged = !matches!(status, scrollable::Status::Active { .. });
            style.vertical_rail.background = None;
            style.vertical_rail.scroller.background = palette
                .muted
                .scale_alpha(if engaged { 0.70 } else { 0.45 })
                .into();
            style
        })
}
pub fn action<'a, Message: Clone + 'a>(
    content: impl Into<iced::Element<'a, Message>>,
    on_press: Option<Message>,
    treatment: canopa_ui::Action,
    focused: bool,
) -> iced::widget::button::Button<'a, Message> {
    canopa_ui::styled_action(content, on_press, treatment, focused, &appearance())
}
pub fn button<'a, Message: Clone + 'a>(
    content: impl Into<iced::Element<'a, Message>>,
) -> iced::widget::button::Button<'a, Message> {
    action(content, None, canopa_ui::Action::Normal, false)
}
pub fn input<'a, Message: Clone + 'a>(
    placeholder: &str,
    value: &str,
) -> iced::widget::TextInput<'a, Message> {
    input_with_style(placeholder, value, ui(canopa_theme::UiKind::Input))
}

pub fn input_with_style<'a, Message: Clone + 'a>(
    placeholder: &str,
    value: &str,
    ui: canopa_theme::ResolvedStyle,
) -> iced::widget::TextInput<'a, Message> {
    let appearance = appearance();
    iced::widget::text_input(placeholder, value)
        .font(font())
        .size(body_size())
        .padding(canopa_ui::content_padding(ui))
        .style(move |theme, status| {
            use iced::widget::text_input::Status;
            let state = ui.state(
                false,
                status == Status::Hovered,
                false,
                status == Status::Disabled,
                matches!(status, Status::Focused { .. }),
            );
            let frame = canopa_ui::frame(&appearance, state);
            let mut base = iced::widget::text_input::default(theme, status);
            base.background = frame.background.unwrap();
            base.border.radius = frame.border.radius;
            if ui.custom_border {
                base.border.width = frame.border.width;
                base.border.color = frame.border.color;
            }
            if matches!(status, Status::Focused { .. }) {
                base.border.width = base.border.width.max(1.0);
                base.border.color =
                    canopa_ui::color(appearance.role(canopa_theme::ColorRole::Focus));
            }
            base.value = frame.text_color.unwrap();
            for (on, s) in [
                false,
                status == Status::Hovered,
                false,
                status == Status::Disabled,
                matches!(status, Status::Focused { .. }),
            ]
            .into_iter()
            .zip(ui.states)
            {
                if on && s.border_color.is_some() {
                    base.border.color = frame.border.color;
                }
            }
            base
        })
}

#[derive(Clone, Copy)]
pub struct MenuMetrics {
    pub row: canopa_theme::ResolvedStyle,
    pub gap: f32,
    pub extent: f32,
}
pub fn menu_metrics() -> MenuMetrics {
    menu_metrics_with_gap(appearance().customization.ui[canopa_theme::UiKind::Menu as usize].gap)
}
pub fn menu_metrics_with_gap(gap: Option<canopa_theme::Pixels>) -> MenuMetrics {
    let row = ui(canopa_theme::UiKind::MenuRow);
    let gap = gap.map(|p| p.px()).unwrap_or(3.0);
    let extent = (body_size() * 1.3).ceil().max(24.0)
        + row.padding.vertical()
        + 2.0 * row.border_width.px()
        + row.margin.vertical()
        + gap;
    MenuMetrics { row, gap, extent }
}

/// Launcher rows: browsing and results share one height, so typing never
/// moves the list. `_browsing` is kept for callers that still distinguish them.
pub fn launcher_menu_metrics(mut metrics: MenuMetrics, _browsing: bool) -> MenuMetrics {
    let labels = (body_size() + label_size() + 3.0) * 1.12 + 3.0;
    let padding = if metrics.row.custom_padding {
        metrics.row.padding.vertical()
    } else {
        6.0
    };
    metrics.extent = labels.ceil().max(27.0)
        + padding
        + metrics.row.margin.vertical()
        + 2.0 * metrics.row.border_width.px()
        + metrics.gap;
    metrics
}

pub fn card(_: &iced::Theme) -> iced::widget::container::Style {
    let ui = ui(canopa_theme::UiKind::Card);
    let mut frame = canopa_ui::frame(&appearance(), ui);
    if !ui.custom_border {
        frame.border.width = 0.0;
    }
    frame
}

/// Compact bar roles in logical pixels; Iced applies output scaling.
pub mod bar_metrics {
    pub const ICON_SIZE: u32 = 20;
}

/// Header size includes the same text and close-button metrics used by the view.
pub fn panel_header_height() -> u32 {
    let padding = canopa_ui::content_padding(ui(canopa_theme::UiKind::Button));
    (24.0 + (title_size() * 1.3).max((label_size() * 1.3 + padding.top + padding.bottom).max(32.0)))
        .ceil() as u32
}

pub fn highlighted_frame(
    ui: canopa_theme::ResolvedStyle,
    selected: bool,
    hover: bool,
    disabled: bool,
    focus: bool,
) -> iced::widget::container::Style {
    use canopa_theme::{ColorRole, ColorValue, StateColors};
    let appearance = appearance();
    let mut base = ui;
    if selected || hover || focus {
        if base.background == ColorValue::Rgba(canopa_theme::Rgb(0, 0, 0), 0) {
            base.background = ColorValue::Role(if selected {
                ColorRole::AccentSurface
            } else {
                ColorRole::Raised
            });
        }
        if !base.custom_border {
            base.border_width = canopa_theme::Pixels::whole(1);
            base.border_color = ColorValue::Role(ColorRole::Accent);
        }
    }
    if disabled && base.states[3].foreground.is_none() {
        base.states[3].foreground = Some(ColorValue::Role(ColorRole::Disabled));
    }
    if focus && !disabled {
        base.border_width =
            canopa_theme::Pixels::from_number(base.border_width.px().max(1.0) as f64, 64.0)
                .unwrap();
        let mut state = StateColors {
            border_color: Some(ColorValue::Role(ColorRole::Focus)),
            ..Default::default()
        };
        if let Some(color) = base.states[4].border_color {
            state.border_color = Some(color);
        }
        base.states[4].border_color = state.border_color;
    }
    canopa_ui::frame(
        &appearance,
        base.state(selected, hover, false, disabled, focus),
    )
}

/// Glyph measurement uses the same installed family and shaping as rendered text.
pub fn text_width(content: &str, size: f32) -> f32 {
    text_width_in_font(content, size, font())
}

pub fn text_width_in_font(content: &str, size: f32, font: iced::Font) -> f32 {
    use iced::advanced::text::{self, Paragraph};
    iced::advanced::graphics::text::Paragraph::with_text(text::Text {
        content,
        bounds: iced::Size::new(f32::INFINITY, f32::INFINITY),
        size: iced::Pixels(size),
        line_height: text::LineHeight::default(),
        font,
        align_x: text::Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::None,
    })
    .min_bounds()
    .width
}

/// A round, icon-only control (Close, Back) shared by every panel header so the
/// launcher, Quick settings and provider panels dismiss the same way.
pub fn icon_button<'a, Message: Clone + 'a>(
    bytes: &'static [u8],
    on_press: Option<Message>,
    focused: bool,
) -> iced::widget::button::Button<'a, Message> {
    use iced::widget::button;
    button(iced::widget::container(icon(bytes, 16.0, palette().text)).center(32))
        .padding(0)
        .width(32)
        .height(32)
        .on_press_maybe(on_press)
        .style(move |theme, status| {
            let colors = canopa_ui::colors(theme);
            let engaged = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(
                    if engaged {
                        colors.text.scale_alpha(0.16)
                    } else {
                        colors.text.scale_alpha(0.07)
                    }
                    .into(),
                ),
                text_color: colors.text,
                border: iced::Border {
                    color: colors.focus,
                    width: if focused { 2.0 } else { 0.0 },
                    radius: 16.0.into(),
                },
                ..Default::default()
            }
        })
}

pub const CLOSE_ICON: &[u8] = include_bytes!("../assets/close.svg");

/// A 0-100 level slider for Quick settings: a thick rounded rail with the
/// accent fill and a round handle. Without `on_change` it is a read-only gauge.
pub fn level_slider<'a, Message: Clone + 'a>(
    value: f32,
    on_change: Option<impl Fn(f32) -> Message + 'a>,
    dimmed: bool,
) -> iced::Element<'a, Message> {
    use iced::widget::slider;
    let style = move |theme: &iced::Theme, status: slider::Status| {
        let colors = canopa_ui::colors(theme);
        let fill = if dimmed { colors.muted } else { colors.accent };
        slider::Style {
            rail: slider::Rail {
                backgrounds: (fill.into(), colors.text.scale_alpha(0.14).into()),
                width: 8.0,
                border: iced::Border::default().rounded(4),
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle {
                    radius: if matches!(status, slider::Status::Active) {
                        9.0
                    } else {
                        10.5
                    },
                },
                background: iced::Color::WHITE.into(),
                border_width: 2.5,
                border_color: fill,
            },
        }
    };
    match on_change {
        Some(on_change) => slider(0.0..=100.0, value, on_change)
            .step(1.0)
            .shift_step(10.0)
            .height(22)
            .width(iced::Length::Fill)
            .style(style)
            .into(),
        None => {
            let fill = (value / 100.0).clamp(0.0, 1.0);
            let colors = palette();
            iced::widget::container(
                iced::widget::container(iced::widget::Space::new())
                    .width(iced::Length::FillPortion((fill * 1000.0) as u16 + 1))
                    .height(6)
                    .style(move |_| {
                        iced::widget::container::Style::default()
                            .background(colors.muted)
                            .border(iced::Border::default().rounded(3))
                    }),
            )
            .width(iced::Length::Fill)
            .height(22)
            .align_y(iced::Alignment::Center)
            .into()
        }
    }
}

/// `text` shortened with a trailing ellipsis to fit `width` at `size`.
pub fn ellipsize(text: &str, width: f32, size: f32) -> String {
    if text_width(text, size) <= width {
        return text.to_owned();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        // Never end on a separator such as "Notes —…".
        let candidate: String = chars
            .iter()
            .collect::<String>()
            .trim_end_matches(|c: char| c.is_whitespace() || "—–-·:,;|/".contains(c))
            .to_owned()
            + "…";
        if text_width(&candidate, size) <= width {
            return candidate;
        }
    }
    "…".into()
}

/// How much of a glass surface's own fill covers the blurred background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glass {
    /// The bar: the most see-through, it sits on the wallpaper beside windows.
    Bar,
    /// Panels, the launcher and drawers: text-heavy, so denser.
    Panel,
    /// Menus, tooltips, hover cards and the on-screen display.
    Popover,
}

impl Glass {
    fn opacity(self) -> f32 {
        let dark = appearance().color_scheme == canopa_theme::ColorScheme::Dark;
        match (self, dark) {
            (Glass::Bar, true) => 0.56,
            (Glass::Bar, false) => 0.66,
            (Glass::Panel, true) => 0.72,
            (Glass::Panel, false) => 0.78,
            (Glass::Popover, true) => 0.78,
            (Glass::Popover, false) => 0.84,
        }
    }
}

/// The fill a glass surface of `kind` shows over its blur, for overlays such as
/// fades that blend into it.
pub fn glass_fill(kind: Glass) -> iced::Color {
    let ui = ui(match kind {
        Glass::Bar => canopa_theme::UiKind::Bar,
        Glass::Panel => canopa_theme::UiKind::Panel,
        Glass::Popover => canopa_theme::UiKind::Menu,
    });
    let background = match canopa_ui::frame(&appearance(), ui).background {
        Some(iced::Background::Color(color)) => color,
        _ => palette().background,
    };
    if glass_enabled() && background.a >= 0.999 {
        background.scale_alpha(kind.opacity())
    } else {
        background
    }
}

/// Whether surfaces are drawn as translucent glass. High contrast keeps every
/// surface opaque.
pub fn glass_enabled() -> bool {
    !appearance().high_contrast()
}

/// A glass version of a surface's frame: an opaque fill becomes translucent
/// for the compositor's blur to show through, and the default outline becomes
/// a faint light edge. Configured translucent fills are kept as they are.
pub fn glass(
    mut style: iced::widget::container::Style,
    kind: Glass,
) -> iced::widget::container::Style {
    if !glass_enabled() {
        return style;
    }
    if let Some(iced::Background::Color(color)) = style.background
        && color.a >= 0.999
    {
        style.background = Some(color.scale_alpha(kind.opacity()).into());
    }
    let palette = palette();
    if style.border.width > 0.0 && style.border.color == palette.border {
        style.border.color = palette.text.scale_alpha(0.24);
    }
    // Panels cast a soft shadow into the transparent margin kept around them.
    // The bar sits on the wallpaper, and popovers fill their surfaces exactly,
    // so a shadow there would be cut off square.
    style.shadow = match kind {
        Glass::Bar | Glass::Popover => iced::Shadow::default(),
        Glass::Panel => iced::Shadow {
            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.32),
            offset: iced::Vector::new(0.0, 6.0),
            blur_radius: 22.0,
        },
    };
    style
}

/// Report a glass surface's shape so the compositor blurs behind it.
pub fn glass_region<'a>(
    content: impl Into<iced::Element<'a, crate::app::Message>>,
    radius: [canopa_theme::Pixels; 4],
) -> iced::Element<'a, crate::app::Message> {
    glass_region_visible(content, radius, true)
}

/// [`glass_region`] that drops its blur while `visible` is false.
pub fn glass_region_visible<'a>(
    content: impl Into<iced::Element<'a, crate::app::Message>>,
    radius: [canopa_theme::Pixels; 4],
    visible: bool,
) -> iced::Element<'a, crate::app::Message> {
    if !glass_enabled() {
        return content.into();
    }
    canopa_ui::glass::region(content, radius.map(|r| r.px()), visible, |shape| {
        crate::app::Message::Glass {
            window: None,
            shape,
        }
    })
}
