//! CPU-only advanced shaping, bidi layout, font fallback and glyph rasterization.
use canopa_theme::Rgb;
use cosmic_text::{
    Attrs, Buffer, Color, Cursor, Family, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap,
};
use image::RgbImage;
use std::ops::Range;

#[derive(Clone, Copy)]
pub(crate) enum Style {
    Centered(Weight),
    /// Starts at the position (its left edge).
    Leading(Weight),
    Trailing(Weight),
}

impl Style {
    pub(crate) fn mirrored(self) -> Self {
        match self {
            Self::Centered(w) => Self::Centered(w),
            Self::Leading(w) => Self::Trailing(w),
            Self::Trailing(w) => Self::Leading(w),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Anchor {
    Start,
    Center,
    End,
}

pub struct Text {
    fonts: FontSystem,
    cache: SwashCache,
    family: String,
}
impl Text {
    pub fn new(family: String) -> anyhow::Result<Self> {
        let mut fonts = FontSystem::new();
        anyhow::ensure!(
            fonts.db_mut().faces().next().is_some(),
            "no installed font is available"
        );
        Ok(Self {
            fonts,
            cache: SwashCache::new(),
            family,
        })
    }
    fn buffer(&mut self, text: &str, size: f32) -> Buffer {
        self.styled_buffer(text, size, Weight::NORMAL, false)
    }
    fn styled_buffer(&mut self, text: &str, size: f32, weight: Weight, monospace: bool) -> Buffer {
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, size * 1.3));
        buffer.set_wrap(&mut self.fonts, Wrap::None);
        buffer.set_text(
            &mut self.fonts,
            text,
            &Attrs::new()
                .family(if monospace {
                    Family::Monospace
                } else {
                    Family::Name(&self.family)
                })
                .weight(weight),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.fonts, true);
        buffer
    }
    pub fn width(&mut self, text: &str, size: f32) -> f32 {
        self.buffer(text, size)
            .layout_runs()
            .map(|r| r.line_w)
            .fold(0., f32::max)
    }
    pub(crate) fn styled_width(
        &mut self,
        text: &str,
        size: f32,
        weight: Weight,
        monospace: bool,
    ) -> f32 {
        self.styled_buffer(text, size, weight, monospace)
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0., f32::max)
    }
    pub(crate) fn carets(&mut self, text: &str, size: f32, max_width: f32) -> Vec<(usize, f32)> {
        use unicode_segmentation::UnicodeSegmentation;
        let width = self.width(text, size);
        let size = size * (max_width.max(1.) / width.max(1.)).min(1.);
        let buffer = self.buffer(text, size);
        text.grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(std::iter::once(text.len()))
            .map(|i| {
                let x = buffer
                    .layout_runs()
                    .next()
                    .and_then(|run| run.highlight(Cursor::new(0, i), Cursor::new(0, i)))
                    .map_or(0., |(x, _)| x);
                (i, x)
            })
            .collect()
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        image: &mut RgbImage,
        text: &str,
        position: (f32, f32),
        size: f32,
        max_width: f32,
        color: Rgb,
        selection: Option<(Range<usize>, usize, Rgb)>,
    ) {
        self.draw_styled(
            image,
            text,
            position,
            size,
            max_width,
            color,
            selection,
            Style::Centered(Weight::NORMAL),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_styled(
        &mut self,
        image: &mut RgbImage,
        text: &str,
        position: (f32, f32),
        size: f32,
        max_width: f32,
        color: Rgb,
        selection: Option<(Range<usize>, usize, Rgb)>,
        style: Style,
    ) {
        let (weight, anchor, monospace) = match style {
            Style::Centered(weight) => (weight, Anchor::Center, false),
            Style::Leading(weight) => (weight, Anchor::Start, false),
            Style::Trailing(weight) => (weight, Anchor::End, false),
        };
        let mut buffer = self.styled_buffer(text, size, weight, monospace);
        let width = buffer.layout_runs().map(|r| r.line_w).fold(0., f32::max);
        let fit = (max_width.max(1.) / width.max(1.)).min(1.);
        if fit < 1. {
            buffer = self.styled_buffer(text, size * fit, weight, monospace);
        }
        let width = width * fit;
        let baseline = buffer.layout_runs().next().map_or(0., |r| r.line_y);
        let left = match anchor {
            Anchor::Start => position.0,
            Anchor::Center => position.0 - width / 2.,
            Anchor::End => position.0 - width,
        };
        let top = position.1 - baseline;
        if let Some((range, cursor, accent)) = selection {
            for run in buffer.layout_runs() {
                if !range.is_empty()
                    && let Some((x, w)) =
                        run.highlight(Cursor::new(0, range.start), Cursor::new(0, range.end))
                {
                    rect(
                        image,
                        left + x,
                        top + run.line_top,
                        w,
                        run.line_height,
                        accent,
                        0.35,
                    );
                }
                if let Some((x, _)) = run.highlight(Cursor::new(0, cursor), Cursor::new(0, cursor))
                {
                    rect(
                        image,
                        left + x,
                        top + run.line_top,
                        1.5,
                        run.line_height,
                        accent,
                        1.,
                    );
                } else if text.is_empty() {
                    rect(image, left, top, 1.5, size, accent, 1.);
                }
            }
        }
        buffer.draw(
            &mut self.fonts,
            &mut self.cache,
            Color::rgb(color.0, color.1, color.2),
            |x, y, w, h, color| {
                rect(
                    image,
                    (left + x as f32).round(),
                    (top + y as f32).round(),
                    w as f32,
                    h as f32,
                    Rgb(color.r(), color.g(), color.b()),
                    color.a() as f32 / 255.,
                );
            },
        );
    }
}
fn rect(image: &mut RgbImage, x: f32, y: f32, w: f32, h: f32, color: Rgb, alpha: f32) {
    for py in y.floor().max(0.) as u32..(y + h).ceil().max(0.).min(image.height() as f32) as u32 {
        for px in x.floor().max(0.) as u32..(x + w).ceil().max(0.).min(image.width() as f32) as u32
        {
            for (channel, value) in image
                .get_pixel_mut(px, py)
                .0
                .iter_mut()
                .zip([color.0, color.1, color.2])
            {
                *channel = (*channel as f32 * (1. - alpha) + value as f32 * alpha).round() as u8;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arabic_is_shaped_right_to_left_and_combining_marks_share_a_cluster() {
        let mut text = Text::new("DejaVu Sans".into()).unwrap();
        let arabic = text.buffer("سلام", 24.);
        let run = arabic.layout_runs().next().unwrap();
        assert!(run.rtl && run.line_w > 0.);
        assert!(run.glyphs.len() < 4, "lam-alef should form a ligature");
        let combining = text.buffer("e\u{301}", 24.);
        let run = combining.layout_runs().next().unwrap();
        assert!(run.glyphs.iter().all(|g| g.start == 0 && g.end == 3));
    }
}
