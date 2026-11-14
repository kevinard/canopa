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

/// A window thumbnail shown above the tooltip text, at its logical size.
#[derive(Debug, Clone)]
pub struct TooltipPreview {
    pub handle: iced::widget::image::Handle,
    pub width: f32,
    pub height: f32,
}

/// Space between thumbnails and between them and the text.
const PREVIEW_SPACING: f32 = 8.0;

#[derive(Debug, Clone)]
pub struct TooltipTree {
    pub segments: Vec<RenderedSegment>,
    /// Window thumbnails, such as a taskbar application's windows.
    pub previews: Vec<TooltipPreview>,
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
    let (previews_width, previews_height) = previews_size(tree);
    let width = ((natural.width.max(previews_width)
        + (ui.padding.horizontal() + border)
        + ui.margin.horizontal())
    .ceil() as u32)
        .clamp(MIN_WIDTH, MAX_WIDTH.max(previews_width.ceil() as u32 + 64));
    // Re-measure at the rounded surface width, including all wrapped lines.
    let measured = measure(
        (width as f32 - (ui.padding.horizontal() + border) - ui.margin.horizontal()).max(1.0),
    );
    let height = ((measured.height
        + previews_height
        + (ui.padding.vertical() + border)
        + ui.margin.vertical())
    .ceil() as u32)
        .max(MIN_HEIGHT);
    (width, height)
}

/// The thumbnail row's size, including the space below it.
fn previews_size(tree: &TooltipTree) -> (f32, f32) {
    if tree.previews.is_empty() {
        return (0.0, 0.0);
    }
    let width = tree
        .previews
        .iter()
        .map(|preview| preview.width)
        .sum::<f32>()
        + PREVIEW_SPACING * (tree.previews.len() - 1) as f32;
    let height = tree
        .previews
        .iter()
        .map(|preview| preview.height)
        .fold(0.0, f32::max);
    (width, height + PREVIEW_SPACING)
}

/// Render the tooltip element for a popup window.
pub fn view_tooltip<'a, Message: 'a + Clone>(tree: &'a TooltipTree) -> iced::Element<'a, Message> {
    use iced::widget::{column, container, image, rich_text, row};

    let ui = crate::theme::ui(canopa_theme::UiKind::Tooltip);
    let text = rich_text(spans(tree))
        .size(crate::theme::title_size())
        .font(crate::theme::font())
        .wrapping(text::Wrapping::WordOrGlyph);
    let content: iced::Element<'a, Message> = if tree.previews.is_empty() {
        text.into()
    } else {
        let previews = row(tree.previews.iter().map(|preview| {
            image(preview.handle.clone())
                .width(preview.width)
                .height(preview.height)
                .into()
        }))
        .spacing(PREVIEW_SPACING)
        .align_y(iced::Alignment::End);
        column![previews, text].spacing(PREVIEW_SPACING).into()
    };
    let max_width = (MAX_WIDTH as f32).max(previews_size(tree).0 + 64.0);
    let frame = container(content)
        .padding(canopa_ui::content_padding(ui))
        .max_width((max_width - ui.margin.horizontal()).max(1.0))
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
                previews: Vec::new(),
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
            previews: Vec::new(),
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
