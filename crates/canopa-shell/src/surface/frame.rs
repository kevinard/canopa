use iced::Element;
use iced::widget::container;

/// Wrap module content with the `ModuleFrame` visual treatment.
///
/// When `hovered` is true, renders a rounded-rect background fill using
/// the shared surface color with an accent outline and 4px radius.
/// When `hovered` is false, renders transparently with no background.
///
/// Identical visual treatment is applied for pointer hover and keyboard focus
/// (hover state is managed by the caller via `mouse_area`).
pub fn module_frame<'a, Message: 'a>(
    content: Element<'a, Message>,
    hovered: bool,
    focused: bool,
    selected: bool,
    ui: canopa_theme::ResolvedStyle,
) -> Element<'a, Message> {
    let frame = container(content)
        // Reserve a dark gap so opaque workspace pills cannot cover the outline.
        .padding(canopa_ui::content_padding(ui))
        .style(move |_| crate::theme::highlighted_frame(ui, selected, hovered, false, focused));
    container(frame)
        .padding(canopa_ui::padding(ui.margin))
        .into()
}
