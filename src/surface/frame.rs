use iced::Element;
use iced::widget::container;

/// Wrap module content with the `ModuleFrame` visual treatment.
///
/// When `hovered` is true, renders a rounded-rect background fill using
/// Catppuccin Mocha Surface0 (`#313244`) with an accent outline and 4px radius.
/// When `hovered` is false, renders transparently with no background.
///
/// Identical visual treatment is applied for pointer hover and keyboard focus
/// (hover state is managed by the caller via `mouse_area`).
pub fn module_frame<'a, Message: 'a>(
    content: Element<'a, Message>,
    hovered: bool,
) -> Element<'a, Message> {
    container(content)
        // Reserve a dark gap so opaque workspace pills cannot cover the outline.
        .padding(2)
        .style(move |_| {
            if hovered {
                container::Style {
                    background: Some(iced::Background::Color(crate::theme::SURFACE0)),
                    border: iced::Border {
                        color: crate::theme::ACCENT,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }
            } else {
                container::Style::default()
            }
        })
        .into()
}
