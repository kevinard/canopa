//! Tooltip popup view. Generic — has no knowledge of any specific module.
//!
//! Renders a single row of pre-rendered styled text segments inside a
//! layer-shell popup surface.

use crate::config::Edge;
use crate::config::style::ResolvedStyle;

#[derive(Debug, Clone)]
pub struct RenderedSegment {
    pub text: String,
    pub style: ResolvedStyle,
}

#[derive(Debug, Clone)]
pub struct TooltipTree {
    pub segments: Vec<RenderedSegment>,
    /// Module's screen-space anchor point.
    pub anchor: (i32, i32),
    /// Logical size of the bar surface the tooltip points at.
    pub bar_size: (u32, u32),
    /// Logical screen size of the output (width, height) for clamping.
    pub screen_size: (u32, u32),
    /// Bar edge that hosts the module.
    pub bar_edge: Edge,
}

/// Estimate the logical pixel size `(width, height)` of the tooltip popup.
/// Used to size the layer-shell content so the popup can be positioned
/// correctly within the full-screen overlay.
pub fn popup_size(tree: &TooltipTree) -> (u32, u32) {
    const CHAR_WIDTH: u32 = 8;
    const ROW_HEIGHT: u32 = 26;
    const SPACING: u32 = 4;
    const H_PADDING: u32 = 16;
    const V_PADDING: u32 = 8;
    const MIN_WIDTH: u32 = 32;
    const MAX_WIDTH: u32 = 480;

    let segs_chars: u32 = tree
        .segments
        .iter()
        .map(|s| s.text.chars().count() as u32)
        .sum();
    let spacing_total = (tree.segments.len().saturating_sub(1) as u32) * SPACING;
    let width = (segs_chars * CHAR_WIDTH + spacing_total + H_PADDING).clamp(MIN_WIDTH, MAX_WIDTH);
    let height = ROW_HEIGHT + V_PADDING;
    (width, height)
}

/// Render the tooltip element for a popup window.
pub fn view_tooltip<'a, Message: 'a + Clone>(tree: &'a TooltipTree) -> iced::Element<'a, Message> {
    use iced::widget::{container, row, text};

    let row_widget = row(tree.segments.iter().map(|seg| {
        let mut t = text(seg.text.as_str());
        if let Some(c) = seg.style.color {
            t = t.color(c);
        }
        t.into()
    }))
    .spacing(4);

    container(row_widget)
        .padding([4, 8])
        .style(|_| iced::widget::container::Style {
            background: Some(iced::Background::Color(crate::theme::SURFACE1)),
            border: iced::Border {
                color: crate::theme::BORDER,
                width: 1.0,
                radius: iced::border::Radius::from(4.0),
            },
            ..Default::default()
        })
        .into()
}
