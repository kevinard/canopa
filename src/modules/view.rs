/// Shared view helpers for collapsed and expanded module layouts.
///
/// These cover the two layout patterns that repeat across battery, network, volume,
/// and focused-window. Keeping them here avoids duplicating widget construction code
/// in every module.
use iced::widget::{container, row, text};
use iced::{Alignment, Element, Length};

use crate::app::Message;

/// Centred icon in a 20 px-tall fill-width container — the standard collapsed view.
///
/// ```text
/// ┌────────────────┐  height: 20px
/// │       ⚡       │  icon centred, size 16
/// └────────────────┘
/// ```
pub fn collapsed_icon<'a>(icon: impl ToString, color: iced::Color) -> Element<'a, Message> {
    container(text(icon.to_string()).size(16).color(color))
        .width(Length::Fill)
        .height(20)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

/// Icon + label row in a padded 20 px-tall container — the standard expanded view.
///
/// ```text
/// ┌───────────────────────────────┐  height: 20px, padding [0, 6]
/// │ ⚡  75%                       │  icon size 16 + label size 12 (subtext colour)
/// └───────────────────────────────┘
/// ```
pub fn expanded_icon_label<'a>(
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
