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
    let width = (segs_chars * CHAR_WIDTH + spacing_total + H_PADDING)
        .max(MIN_WIDTH)
        .min(MAX_WIDTH);
    let height = ROW_HEIGHT + V_PADDING;
    (width, height)
}

/// Compute the popup's top-left position (screen coordinates) given the
/// tree, the popup's logical (width, height), and a small gap in pixels.
pub fn compute_position(tree: &TooltipTree, size: (u32, u32), gap: i32) -> (i32, i32) {
    let (sw, sh) = (tree.screen_size.0 as i32, tree.screen_size.1 as i32);
    let (w, h) = (size.0 as i32, size.1 as i32);
    let (ax, ay) = tree.anchor;
    let (mut x, mut y) = match tree.bar_edge {
        Edge::Top => (ax - w / 2, ay + gap),
        Edge::Bottom => (ax - w / 2, ay - gap - h),
        Edge::Left => (ax + gap, ay - h / 2),
        Edge::Right => (ax - gap - w, ay - h / 2),
    };
    x = x.clamp(0, (sw - w).max(0));
    y = y.clamp(0, (sh - h).max(0));
    (x, y)
}

/// Render the tooltip element for a popup window.
pub fn view_tooltip<'a, Message: 'a + Clone>(
    tree: &'a TooltipTree,
) -> iced::Element<'a, Message> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(edge: Edge, anchor: (i32, i32), screen: (u32, u32)) -> TooltipTree {
        TooltipTree {
            segments: vec![],
            anchor,
            screen_size: screen,
            bar_edge: edge,
        }
    }

    #[test]
    fn top_bar_places_below_anchor_centered() {
        let t = tree(Edge::Top, (500, 30), (1920, 1080));
        let (x, y) = compute_position(&t, (200, 40), 4);
        assert_eq!(y, 34);
        assert_eq!(x, 400);
    }

    #[test]
    fn bottom_bar_places_above_anchor() {
        let t = tree(Edge::Bottom, (500, 1050), (1920, 1080));
        let (_, y) = compute_position(&t, (200, 40), 4);
        assert_eq!(y, 1006);
    }

    #[test]
    fn left_bar_places_right_of_anchor() {
        let t = tree(Edge::Left, (30, 500), (1920, 1080));
        let (x, _) = compute_position(&t, (200, 40), 4);
        assert_eq!(x, 34);
    }

    #[test]
    fn right_bar_places_left_of_anchor() {
        let t = tree(Edge::Right, (1900, 500), (1920, 1080));
        let (x, _) = compute_position(&t, (200, 40), 4);
        assert_eq!(x, 1696);
    }

    #[test]
    fn clamps_to_screen_left_edge() {
        let t = tree(Edge::Top, (10, 30), (1920, 1080));
        let (x, _) = compute_position(&t, (200, 40), 4);
        assert_eq!(x, 0);
    }

    #[test]
    fn clamps_to_screen_right_edge() {
        let t = tree(Edge::Top, (1900, 30), (1920, 1080));
        let (x, _) = compute_position(&t, (200, 40), 4);
        assert_eq!(x, 1720);
    }
}
