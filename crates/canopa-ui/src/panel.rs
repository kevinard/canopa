//! Shared panel visual roles. Placement, identity and interaction stay with the host.
use iced::{Border, Theme, widget::container};

pub const PADDING: u16 = 16;
pub const HEADER_PADDING: [u16; 2] = [12, PADDING];
pub const HEADER_CONTROL_HEIGHT: u32 = 32;
pub const HEADER_HEIGHT: u32 = HEADER_CONTROL_HEIGHT + 2 * HEADER_PADDING[0] as u32;
pub const ROW_GAP: f32 = 3.0;

pub fn surface(theme: &Theme, attached: bool) -> container::Style {
    let p = crate::colors(theme);
    container::Style {
        background: Some(p.background.into()),
        text_color: Some(p.text),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: if attached {
                0.0
            } else {
                crate::metrics::SURFACE_RADIUS
            }
            .into(),
        },
        ..Default::default()
    }
}

pub fn row(theme: &Theme, selected: bool) -> container::Style {
    let p = crate::colors(theme);
    container::Style {
        background: selected.then(|| p.surface.into()),
        text_color: Some(p.text),
        border: Border {
            color: if selected {
                p.accent
            } else {
                iced::Color::TRANSPARENT
            },
            width: 1.0,
            radius: crate::metrics::CONTROL_RADIUS.into(),
        },
        ..Default::default()
    }
}
