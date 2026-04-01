use iced::widget::container;
use iced::{Color, Element};

/// Wrap module content with the `ModuleFrame` visual treatment.
///
/// When `hovered` is true, renders a rounded-rect background fill using
/// Catppuccin Mocha Surface0 (`#313244`) with a 4px border radius.
/// When `hovered` is false, renders transparently with no background.
///
/// Identical visual treatment is applied for pointer hover and keyboard focus
/// (hover state is managed by the caller via `mouse_area`).
pub fn module_frame<'a, Message: 'a>(
    content: Element<'a, Message>,
    hovered: bool,
) -> Element<'a, Message> {
    container(content)
        .style(move |_| {
            if hovered {
                container::Style {
                    background: Some(iced::Background::Color(Color::from_rgb8(0x31, 0x32, 0x44))),
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            } else {
                container::Style::default()
            }
        })
        .into()
}
