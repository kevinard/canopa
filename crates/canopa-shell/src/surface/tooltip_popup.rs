//! Tooltip popup view. Generic — has no knowledge of any specific module.
//!
//! Measures and renders styled text with the same paragraph layout so wrapped
//! and multiline content fits its layer-shell surface.

use crate::config::Edge;
use crate::config::style::ResolvedStyle;
use iced::advanced::text::{self, Paragraph};

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
                ..crate::theme::font()
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
    /// An interactive card shown instead of the text, such as a taskbar
    /// application's windows or a workspace's layout.
    pub card: Option<super::hover_card::HoverCard>,
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
    let ui = crate::theme::ui(canopa_theme::UiKind::Tooltip);
    let border = if ui.custom_border {
        2.0 * ui.border_width.px()
    } else {
        0.0
    };
    if let Some(card) = &tree.card {
        let (width, height) = card.content_size();
        let padding = 2.0 * super::hover_card::HoverCard::padding() + border;
        return (
            (width + padding + ui.margin.horizontal()).ceil() as u32,
            (height + padding + ui.margin.vertical()).ceil() as u32,
        );
    }
    let spans = spans(tree);
    let measure = |width| {
        iced::advanced::graphics::text::Paragraph::with_spans(text::Text {
            content: spans.as_slice(),
            bounds: iced::Size::new(width, f32::INFINITY),
            size: iced::Pixels(crate::theme::title_size()),
            line_height: text::LineHeight::default(),
            font: crate::theme::font(),
            align_x: text::Alignment::Default,
            align_y: iced::alignment::Vertical::Top,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::WordOrGlyph,
        })
        .min_bounds()
    };
    let natural = measure(
        (MAX_WIDTH as f32 - (ui.padding.horizontal() + border) - ui.margin.horizontal()).max(1.0),
    );
    let width = ((natural.width + (ui.padding.horizontal() + border) + ui.margin.horizontal())
        .ceil() as u32)
        .clamp(MIN_WIDTH, MAX_WIDTH);
    // Re-measure at the rounded surface width, including all wrapped lines.
    let measured = measure(
        (width as f32 - (ui.padding.horizontal() + border) - ui.margin.horizontal()).max(1.0),
    );
    let height =
        ((measured.height + (ui.padding.vertical() + border) + ui.margin.vertical()).ceil() as u32)
            .max(MIN_HEIGHT);
    (width, height)
}

/// Render the tooltip element for a popup window: its text, or its card.
pub fn view_tooltip<'a>(tree: &'a TooltipTree) -> iced::Element<'a, crate::app::Message> {
    use iced::widget::{container, rich_text};

    let ui = crate::theme::ui(canopa_theme::UiKind::Tooltip);
    if let Some(card) = &tree.card {
        let border = if ui.custom_border {
            ui.border_width.px()
        } else {
            0.0
        };
        let frame = container(card.view())
            .padding(super::hover_card::HoverCard::padding() + border)
            .style(move |_| canopa_ui::frame(&crate::theme::appearance(), ui));
        return container(frame)
            .padding(canopa_ui::padding(ui.margin))
            .into();
    }
    let text = rich_text(spans(tree))
        .size(crate::theme::title_size())
        .font(crate::theme::font())
        .wrapping(text::Wrapping::WordOrGlyph);
    let frame = container(text)
        .padding(canopa_ui::content_padding(ui))
        .max_width((MAX_WIDTH as f32 - ui.margin.horizontal()).max(1.0))
        .height(iced::Length::Shrink)
        .style(move |_| canopa_ui::frame(&crate::theme::appearance(), ui));
    container(frame)
        .padding(canopa_ui::padding(ui.margin))
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
                card: None,
                anchor: (24, 500),
                bar_size: (48, 1080),
                screen_size: (1920, 1080),
                bar_edge: Edge::Left,
            };
            let size = popup_size(&tree);
            let mut view = view_tooltip(&tree);
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
            card: None,
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
