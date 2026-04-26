/// Shared view helpers for collapsed, expanded, and horizontal module layouts.
///
/// These cover the layout patterns that repeat across battery, network, volume,
/// and focused-window. Keeping them here avoids duplicating widget construction code
/// in every module.
use iced::widget::{container, row, text};
use iced::{Alignment, Element, Length};

use crate::app::Message;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum IconLabelLayout {
    Collapsed,
    Expanded,
    Horizontal,
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
    fn formatted_text_for_layout(
        &self,
        _state: &super::ModuleState,
        _layout: IconLabelLayout,
    ) -> Option<String> {
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
pub(super) fn collapsed_icon<'a>(icon: impl ToString, color: iced::Color) -> Element<'a, Message> {
    container(text(icon.to_string()).size(16).color(color))
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
pub(super) fn horizontal_icon_label<'a>(
    icon: impl ToString,
    icon_color: iced::Color,
    label: impl ToString,
) -> Element<'a, Message> {
    container(
        row![
            text(icon.to_string()).size(16).color(icon_color),
            text(label.to_string())
                .size(12)
                .color(crate::theme::SUBTEXT),
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
pub(super) fn expanded_icon_label<'a>(
    icon: impl ToString,
    icon_color: iced::Color,
    label: impl ToString,
) -> Element<'a, Message> {
    container(
        row![
            text(icon.to_string()).size(16).color(icon_color),
            text(label.to_string())
                .size(12)
                .color(crate::theme::SUBTEXT)
                .width(Length::Fill),
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

pub(super) fn icon_label_view<'a, M>(
    module: &'a M,
    state: &super::ModuleState,
    layout: IconLabelLayout,
) -> Element<'a, Message>
where
    M: IconLabel,
{
    if let Some(text) = module.formatted_text_for_layout(state, layout) {
        return match layout {
            IconLabelLayout::Collapsed => collapsed_icon(text, module.color(state)),
            IconLabelLayout::Expanded => expanded_text(text),
            IconLabelLayout::Horizontal => horizontal_text(text),
        };
    }

    match layout {
        IconLabelLayout::Collapsed => collapsed_icon(module.icon(state), module.color(state)),
        IconLabelLayout::Expanded => expanded_icon_label(
            module.icon(state),
            module.color(state),
            module.label_for_layout(state, layout),
        ),
        IconLabelLayout::Horizontal => horizontal_icon_label(
            module.icon(state),
            module.color(state),
            module.label_for_layout(state, layout),
        ),
    }
}

fn expanded_text<'a>(label: impl ToString) -> Element<'a, Message> {
    container(
        text(label.to_string())
            .size(12)
            .color(crate::theme::SUBTEXT)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(20)
    .align_y(Alignment::Center)
    .padding([0, 6])
    .into()
}

fn horizontal_text<'a>(label: impl ToString) -> Element<'a, Message> {
    container(
        text(label.to_string())
            .size(12)
            .color(crate::theme::SUBTEXT),
    )
    .height(Length::Fill)
    .align_y(Alignment::Center)
    .padding([0, 6])
    .into()
}
