//! Tooltip popup view. Generic — has no knowledge of any specific module.
//!
//! Measures and renders styled text with the same paragraph layout so wrapped
//! and multiline content fits its layer-shell surface.

use crate::config::Edge;
use crate::config::style::ResolvedStyle;
use iced::advanced::text::{self, Paragraph};

const TEXT_SIZE: f32 = 16.0;
const H_PADDING: f32 = 8.0;
const V_PADDING: f32 = 4.0;
const MIN_WIDTH: u32 = 32;
const MIN_HEIGHT: u32 = 34;
const MAX_WIDTH: u32 = 480;

fn spans(tree: &TooltipTree) -> Vec<text::Span<'_, ()>> {
    let mut spans = Vec::new();
    for segment in &tree.segments {
        if !spans.is_empty() {
            spans.push(text::Span::new(" "));
        }
        let mut span = text::Span::new(segment.text.as_str()).color_maybe(segment.style.color);
        if let Some(size) = segment.style.size {
            span = span.size(size);
        }
        if let Some(weight) = segment.style.weight {
            span = span.font(iced::Font {
                weight: match weight {
                    crate::config::style::Weight::Light => iced::font::Weight::Light,
                    crate::config::style::Weight::Normal => iced::font::Weight::Normal,
                    crate::config::style::Weight::Bold => iced::font::Weight::Bold,
                },
                ..iced::Font::DEFAULT
            });
        }
        spans.push(span);
    }
    spans
}

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

/// Measure the same font, spans, and wrapping policy used by `view_tooltip`.
pub fn popup_size(tree: &TooltipTree) -> (u32, u32) {
    let spans = spans(tree);
    let measure = |width| {
        iced::advanced::graphics::text::Paragraph::with_spans(text::Text {
            content: spans.as_slice(),
            bounds: iced::Size::new(width, f32::INFINITY),
            size: iced::Pixels(TEXT_SIZE),
            line_height: text::LineHeight::default(),
            font: iced::Font::DEFAULT,
            align_x: text::Alignment::Default,
            align_y: iced::alignment::Vertical::Top,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::WordOrGlyph,
        })
        .min_bounds()
    };
    let natural = measure(MAX_WIDTH as f32 - 2.0 * H_PADDING);
    let width = ((natural.width + 2.0 * H_PADDING).ceil() as u32).clamp(MIN_WIDTH, MAX_WIDTH);
    // Re-measure at the rounded surface width, including all wrapped lines.
    let measured = measure(width as f32 - 2.0 * H_PADDING);
    let height = ((measured.height + 2.0 * V_PADDING).ceil() as u32).max(MIN_HEIGHT);
    (width, height)
}

/// Render the tooltip element for a popup window.
pub fn view_tooltip<'a, Message: 'a + Clone>(tree: &'a TooltipTree) -> iced::Element<'a, Message> {
    use iced::widget::{container, rich_text};

    container(
        rich_text(spans(tree))
            .size(TEXT_SIZE)
            .font(iced::Font::DEFAULT)
            .wrapping(text::Wrapping::WordOrGlyph),
    )
    .padding([V_PADDING, H_PADDING])
    .max_width(MAX_WIDTH)
    .height(iced::Length::Shrink)
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(crate::theme::palette().raised)),
        border: iced::Border {
            color: crate::theme::palette().border,
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
    use iced::advanced::{layout, renderer::Headless, widget::Tree};

    #[tokio::test]
    async fn tooltip_popup_fits_rendered_multiline_and_wrapped_content() {
        let renderer =
            iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
                .await
                .expect("software renderer");
        for content in [
            "Network name\nConnected\nSignal: excellent".to_string(),
            "A long notification title with wide WWW characters ".repeat(6),
            "W".repeat(150),
            "Réseau — 接続済み — إشعارات".repeat(5),
        ] {
            let tree = TooltipTree {
                segments: vec![RenderedSegment {
                    text: content.clone(),
                    style: Default::default(),
                }],
                anchor: (24, 500),
                bar_size: (48, 1080),
                screen_size: (1920, 1080),
                bar_edge: Edge::Left,
            };
            let size = popup_size(&tree);
            let mut view = view_tooltip::<()>(&tree);
            let mut state = Tree::new(&view);
            // Constrain width exactly as the compositor does, but allow all the
            // text's height so a too-short popup cannot conceal the failure.
            let node = view.as_widget_mut().layout(
                &mut state,
                &renderer,
                &layout::Limits::new(
                    iced::Size::ZERO,
                    iced::Size::new(size.0 as f32, f32::INFINITY),
                ),
            );
            assert!(
                node.size().height <= size.1 as f32,
                "{content:?}: rendered {:?}, popup {size:?}",
                node.size()
            );
            assert!(size.0 <= 480);
        }
    }

    #[test]
    fn tooltip_size_uses_glyph_metrics_and_segment_styles() {
        let mut tree = TooltipTree {
            segments: vec![RenderedSegment {
                text: "iiii".into(),
                style: Default::default(),
            }],
            anchor: (24, 500),
            bar_size: (48, 1080),
            screen_size: (1920, 1080),
            bar_edge: Edge::Left,
        };
        let narrow = popup_size(&tree);
        tree.segments[0].text = "WWWW".into();
        let wide = popup_size(&tree);
        assert!(wide.0 > narrow.0);
        tree.segments[0].style.size = Some(32.0);
        tree.segments[0].style.weight = Some(crate::config::style::Weight::Bold);
        let large = popup_size(&tree);
        assert!(large.0 > wide.0);
        assert!(large.1 > wide.1);
    }
}
