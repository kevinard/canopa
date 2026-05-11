/// Shared view helpers for collapsed, panel, and horizontal module layouts.
///
/// These cover the layout patterns that repeat across battery, network, volume,
/// and focused-window. Keeping them here avoids duplicating widget construction code
/// in every module.
use iced::widget::{container, row, text};
use iced::{Alignment, Element, Length};

use crate::app::Message;
use crate::config::FormatMode;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum IconLabelLayout {
    Collapsed,
    Expanded,
    Horizontal,
}

impl From<IconLabelLayout> for FormatMode {
    fn from(layout: IconLabelLayout) -> Self {
        match layout {
            IconLabelLayout::Collapsed => FormatMode::Collapsed,
            IconLabelLayout::Expanded => FormatMode::Expanded,
            IconLabelLayout::Horizontal => FormatMode::Horizontal,
        }
    }
}

/// State-derived data for modules that render as an icon plus a short label.
///
/// Modules that need extra rows, overlay badges, focus handling, or custom
/// click targets keep their bespoke `Module` view methods instead of using this.
pub(super) trait IconLabel {
    fn icon(&self, state: &super::ModuleState) -> String;
    fn label(&self, state: &super::ModuleState) -> String;
    fn label_for_layout(&self, state: &super::ModuleState, _layout: IconLabelLayout) -> String {
        self.label(state)
    }
    /// Render styled segments for a given layout if a format template is configured.
    /// Returns `None` to fall back to the default icon+label rendering.
    fn styled_segments_for_layout(
        &self,
        _state: &super::ModuleState,
        _layout: IconLabelLayout,
        _default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        None
    }

    fn color(&self, _state: &super::ModuleState) -> iced::Color {
        crate::theme::TEXT
    }
}

/// Centred icon in a 20 px-tall fill-width container — the standard collapsed view.
///
/// ```text
/// ┌────────────────┐  height: 20px
/// │       ⚡       │  icon centred, size 16
/// └────────────────┘
/// ```
pub(super) fn collapsed_icon_with_style<'a>(
    icon: impl ToString,
    color: iced::Color,
    default_style: &crate::config::Style,
) -> Element<'a, Message> {
    container(styled_segment_text(
        icon.to_string(),
        16,
        color,
        fallback_resolved_style(default_style),
    ))
    .width(Length::Fill)
    .height(20)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

/// Icon + label row that fills available height — the standard horizontal bar view.
///
/// Unlike `expanded_icon_label`, width shrinks to content (no `Length::Fill`), making
/// it suitable for modules laid out side-by-side in a horizontal bar.
///
/// ```text
/// ┌──────────────┐  height: Fill, padding [0, 6]
/// │ ⚡  75%      │  icon size 16 + label size 12 (subtext colour)
/// └──────────────┘
/// ```
pub(super) fn horizontal_icon_label_with_style<'a>(
    icon: impl ToString,
    icon_color: iced::Color,
    label: impl ToString,
    default_style: &crate::config::Style,
) -> Element<'a, Message> {
    container(
        row![
            styled_segment_text(
                icon.to_string(),
                16,
                icon_color,
                fallback_resolved_style(default_style)
            ),
            styled_segment_text(
                label.to_string(),
                12,
                crate::theme::SUBTEXT,
                fallback_resolved_style(default_style)
            ),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .height(Length::Fill)
    .align_y(Alignment::Center)
    .padding([0, 6])
    .into()
}

/// Icon + label row in a padded 20 px-tall container — the standard expanded view.
///
/// ```text
/// ┌───────────────────────────────┐  height: 20px, padding [0, 6]
/// │ ⚡  75%                       │  icon size 16 + label size 12 (subtext colour)
/// └───────────────────────────────┘
/// ```
pub(super) fn expanded_icon_label_with_style<'a>(
    icon: impl ToString,
    icon_color: iced::Color,
    label: impl ToString,
    default_style: &crate::config::Style,
) -> Element<'a, Message> {
    container(
        row![
            styled_segment_text(
                icon.to_string(),
                16,
                icon_color,
                fallback_resolved_style(default_style)
            ),
            styled_segment_text(
                label.to_string(),
                12,
                crate::theme::SUBTEXT,
                fallback_resolved_style(default_style)
            ),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(20)
    .align_y(Alignment::Center)
    .padding([0, 6])
    .into()
}

pub(super) fn icon_label_view_with_default<'a, M>(
    module: &'a M,
    state: &super::ModuleState,
    layout: IconLabelLayout,
    default_style: &crate::config::Style,
) -> Element<'a, Message>
where
    M: IconLabel,
{
    if let Some(segments) = module.styled_segments_for_layout(state, layout, default_style) {
        return match layout {
            IconLabelLayout::Collapsed => collapsed_styled_segments(segments, module.color(state)),
            IconLabelLayout::Expanded => expanded_styled_segments(segments),
            IconLabelLayout::Horizontal => horizontal_styled_segments(segments),
        };
    }

    match layout {
        IconLabelLayout::Collapsed => {
            collapsed_icon_with_style(module.icon(state), module.color(state), default_style)
        }
        IconLabelLayout::Expanded => expanded_icon_label_with_style(
            module.icon(state),
            module.color(state),
            module.label_for_layout(state, layout),
            default_style,
        ),
        IconLabelLayout::Horizontal => horizontal_icon_label_with_style(
            module.icon(state),
            module.color(state),
            module.label_for_layout(state, layout),
            default_style,
        ),
    }
}

/// Build a centred, 4px-spaced row of styled segments. Container wrapping
/// (width/height/padding) is the caller's responsibility — this helper exists
/// only to eliminate the per-module map+spacing+align boilerplate.
pub(super) fn styled_segments_row<'a>(
    segments: Vec<(String, crate::config::ResolvedStyle)>,
    size: u32,
    color: iced::Color,
) -> iced::widget::Row<'a, Message> {
    row(segments
        .into_iter()
        .map(|(segment, style)| styled_segment_text(segment, size, color, style))
        .collect::<Vec<_>>())
    .spacing(4)
    .align_y(Alignment::Center)
}

pub(super) fn styled_segment_text<'a>(
    segment: String,
    fallback_size: u32,
    fallback_color: iced::Color,
    style: crate::config::ResolvedStyle,
) -> Element<'a, Message> {
    let mut widget = text(segment)
        .size(style.size.unwrap_or(fallback_size as f32))
        .color(style.color.unwrap_or(fallback_color));
    if let Some(weight) = style.weight {
        let mut font = iced::Font::DEFAULT;
        font.weight = match weight {
            crate::config::Weight::Light => iced::font::Weight::Light,
            crate::config::Weight::Normal => iced::font::Weight::Normal,
            crate::config::Weight::Bold => iced::font::Weight::Bold,
        };
        widget = widget.font(font);
    }
    widget.into()
}

pub(super) fn fallback_resolved_style(
    default_style: &crate::config::Style,
) -> crate::config::ResolvedStyle {
    default_style.resolve()
}

pub(super) fn render_format_segments<'a>(
    format: &crate::config::FormatConfig,
    mode: crate::config::FormatMode,
    values: &[crate::config::FormatValue<'a>],
    state: Option<&str>,
    default_style: &crate::config::Style,
) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
    format.template_for(mode).map(|template| {
        template.render_styled_cow_segments_with_default(values, state, default_style)
    })
}

fn collapsed_styled_segments<'a>(
    segments: Vec<(String, crate::config::ResolvedStyle)>,
    color: iced::Color,
) -> Element<'a, Message> {
    container(styled_segments_row(segments, 16, color))
        .width(Length::Fill)
        .height(20)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn expanded_styled_segments<'a>(
    segments: Vec<(String, crate::config::ResolvedStyle)>,
) -> Element<'a, Message> {
    container(styled_segments_row(segments, 12, crate::theme::SUBTEXT))
        .width(Length::Fill)
        .height(20)
        .align_y(Alignment::Center)
        .padding([0, 6])
        .into()
}

fn horizontal_styled_segments<'a>(
    segments: Vec<(String, crate::config::ResolvedStyle)>,
) -> Element<'a, Message> {
    container(styled_segments_row(segments, 12, crate::theme::SUBTEXT))
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0, 6])
        .into()
}

#[cfg(test)]
mod tests {
    use crate::config::{ColorValue, Style, ThemeToken, Weight};

    #[test]
    fn bar_style_resolves_as_fallback_text_style() {
        let bar_style = Style {
            color: Some(ColorValue::Token(ThemeToken::Accent)),
            size: Some(14.0),
            weight: Some(Weight::Bold),
        };

        let resolved = super::fallback_resolved_style(&bar_style);

        assert_eq!(resolved.color, Some(crate::theme::ACCENT));
        assert_eq!(resolved.size, Some(14.0));
        assert_eq!(resolved.weight, Some(Weight::Bold));
    }
}
