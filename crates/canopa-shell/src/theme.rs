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
            style.vertical_rail.background = None;
            style.vertical_rail.scroller.background = palette.muted.scale_alpha(0.65).into();
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

pub fn launcher_menu_metrics(mut metrics: MenuMetrics, browsing: bool) -> MenuMetrics {
    let labels = if browsing {
        (body_size() + label_size() + 3.0) * 1.12 + 5.0
    } else {
        (body_size() + label_size() + 4.0) * 1.14 + 4.0
    };
    let padding = if metrics.row.custom_padding {
        metrics.row.padding.vertical()
    } else {
        10.0
    };
    metrics.extent = labels.ceil().max(if browsing { 27.0 } else { 32.0 })
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
