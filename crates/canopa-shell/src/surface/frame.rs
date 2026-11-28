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
        .style(move |_| spine_frame(ui, selected, hovered, focused));
    container(frame)
        .padding(canopa_ui::padding(ui.margin))
        .into()
}

/// Spine targets use quiet fills: a soft wash on hover, the accent surface while
/// their panel is open, and the focus ring only for keyboard focus. Configured
/// module borders and backgrounds keep the shared highlighted frame.
fn spine_frame(
    ui: canopa_theme::ResolvedStyle,
    selected: bool,
    hovered: bool,
    focused: bool,
) -> container::Style {
    let configured = ui.custom_border
        || ui.background != canopa_theme::ColorValue::Rgba(canopa_theme::Rgb(0, 0, 0), 0)
        || ui
            .states
            .iter()
            .any(|state| state.background.is_some() || state.border_color.is_some());
    if configured {
        let mut frame = crate::theme::highlighted_frame(ui, selected, hovered, false, focused);
        // Active is a fill on the spine; an outline only ever marks keyboard
        // focus (or a border the configuration asks for).
        if !focused && !ui.custom_border {
            frame.border.width = 0.0;
        }
        return frame;
    }
    let palette = crate::theme::palette();
    let mut frame = canopa_ui::frame(
        &crate::theme::appearance(),
        ui.state(selected, hovered, false, false, focused),
    );
    frame.background = if selected {
        Some(crate::theme::accent_surface().into())
    } else if hovered || focused {
        Some(palette.text.scale_alpha(0.08).into())
    } else {
        None
    };
    frame.border.width = if focused { 2.0 } else { 0.0 };
    frame.border.color = palette.focus;
    frame
}
