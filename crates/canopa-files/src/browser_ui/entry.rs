//! File list rows used by Files and by request-scoped pickers.
use canopa_theme::{Appearance, ColorRole, UiKind};
use iced::{
    Element,
    widget::{Row, button, container},
};

pub fn entry_padding(appearance: &canopa_theme::Appearance, grid: bool) -> iced::Padding {
    let style = appearance.style(UiKind::MenuRow);
    let mut padding = canopa_ui::content_padding(style);
    if !grid && !style.custom_padding {
        padding.top -= 5.0;
        padding.bottom -= 5.0;
    }
    padding
}

pub fn row_height(appearance: &canopa_theme::Appearance) -> f32 {
    let padding = entry_padding(appearance, false);
    (appearance.typography().body_size.px() * 1.3)
        .ceil()
        .max(18.0)
        + padding.top
        + padding.bottom
}

/// The hover wash for rows and headings: the text colour, faint enough to sit
/// on any surface without reading as a selection.
pub fn hover_wash(appearance: &canopa_theme::Appearance) -> iced::Color {
    let alpha = if appearance.high_contrast() {
        0.16
    } else {
        0.07
    };
    super::ink(appearance, ColorRole::Text).scale_alpha(alpha)
}

/// How a Files list row is presented by either browser host.
#[derive(Clone, Copy, Default)]
pub struct RowState {
    pub selected: bool,
    pub focused: bool,
    /// The path through ancestor columns uses a neutral selection fill.
    pub ancestor: bool,
    pub alternate: bool,
    pub disabled: bool,
}

impl RowState {
    pub fn frame(self, appearance: &Appearance) -> container::Style {
        let Self {
            selected,
            focused,
            ancestor,
            alternate,
            disabled,
        } = self;
        let current = !ancestor;
        let base = appearance.style(UiKind::MenuRow);
        let resolved = base.state(selected, false, false, disabled, focused);
        let mut style = canopa_ui::frame(appearance, resolved);
        // High contrast keeps selection and its edge opaque in every column.
        let contrast = appearance.high_contrast();
        if selected && base.states[0].background.is_none() {
            // Ancestors share one neutral fill; the accent stays with the
            // current folder's selection and keyboard focus.
            let color = if contrast {
                super::ink(appearance, ColorRole::AccentSurface)
            } else if current {
                // A firmer tint than hover so the selection reads at a glance.
                super::mix(
                    super::ink(appearance, ColorRole::AccentSurface),
                    super::ink(appearance, ColorRole::Accent),
                    0.22,
                )
            } else {
                // Ancestors' trail: a neutral fill that still reads on the
                // column's own tint.
                super::mix(
                    super::ink(appearance, ColorRole::Raised),
                    super::ink(appearance, ColorRole::Text),
                    0.07,
                )
            };
            // The current folder's selection is the accent surface without an
            // outline, so it never reads as the keyboard focus ring.
            style.background = Some(color.into());
            if !base.custom_border && base.states[0].border_color.is_none() {
                let mut edge = super::ink(appearance, ColorRole::Accent);
                edge.a = f32::from(contrast);
                style.border.width = if contrast { 1.0 } else { 0.0 };
                style.border.color = edge;
            }
        }
        if alternate
            && !contrast
            && !selected
            && !focused
            && canopa_ui::resolve_color(appearance, base.background).a == 0.0
        {
            style.background = Some(
                super::ink(appearance, ColorRole::Raised)
                    .scale_alpha(0.4)
                    .into(),
            );
        }
        if !base.custom_radius {
            style.border.radius = 7.0.into();
        }
        if focused && base.states[4].border_color.is_none() {
            style.border.width = style.border.width.max(1.0);
            // A quieter ring than controls': rows are focused continuously.
            style.border.color = canopa_ui::color(appearance.role(ColorRole::Focus))
                .scale_alpha(if contrast { 1.0 } else { 0.6 });
        }
        style
    }

    pub fn button(self, appearance: &Appearance, status: button::Status) -> button::Style {
        let frame = self.frame(appearance);
        let hovered = !self.selected
            && !self.disabled
            && matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: if hovered && frame.background.is_none() {
                Some(hover_wash(appearance).into())
            } else {
                frame.background
            },
            text_color: frame
                .text_color
                .unwrap_or_else(|| super::ink(appearance, ColorRole::Text)),
            border: frame.border,
            shadow: frame.shadow,
            snap: frame.snap,
        }
    }
}

/// The list name, icon and room for an optional chevron or metadata.
pub fn name_row<'a, M: 'a>(
    appearance: &Appearance,
    name: String,
    icon: Element<'a, M>,
    state: RowState,
    reveal: bool,
) -> Row<'a, M> {
    let mut font = canopa_ui::font(appearance);
    if state.selected && appearance.high_contrast() {
        font.weight = iced::font::Weight::Semibold;
    }
    let name = super::text_layout::file_name_line(
        name,
        font,
        appearance.typography().body_size.px(),
        None,
        reveal,
    );
    iced::widget::row![icon, name]
        .spacing(8)
        .align_y(iced::Center)
}
