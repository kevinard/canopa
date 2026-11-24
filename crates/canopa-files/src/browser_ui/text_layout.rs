//! Width-aware labels for Files browsers.
use iced::{
    Color, Element, Font, Length, Theme,
    widget::{container, responsive, text, tooltip},
};
use std::{cell::RefCell, collections::HashMap};

// Visible rows and breadcrumbs reuse measurements across cursor movement,
// scrolling and editor keystrokes. Keep storage bounded across folder changes.
type Widths = HashMap<(Font, u32), HashMap<String, f32>>;
thread_local! {
    static WIDTHS: RefCell<(usize, Widths)> = RefCell::default();
}

pub fn text_width(content: &str, font: Font, size: f32) -> f32 {
    WIDTHS.with_borrow_mut(|(count, widths)| {
        let key = (font, size.to_bits());
        if let Some(width) = widths.get(&key).and_then(|labels| labels.get(content)) {
            return *width;
        }
        let width = measure_text(content, font, size);
        if *count >= 4096 {
            widths.clear();
            *count = 0;
        }
        widths
            .entry(key)
            .or_default()
            .insert(content.to_owned(), width);
        *count += 1;
        width
    })
}

fn measure_text(content: &str, font: Font, size: f32) -> f32 {
    use iced::advanced::text::{self, Paragraph};

    iced::advanced::graphics::text::Paragraph::with_text(text::Text {
        content,
        bounds: iced::Size::new(f32::INFINITY, f32::INFINITY),
        size: iced::Pixels(size),
        line_height: text::LineHeight::default(),
        font,
        align_x: text::Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::None,
    })
    .min_bounds()
    .width
    .ceil()
}

/// `label` shortened with an ellipsis until it is at most `width` wide: at its
/// end, or in its middle so that a file name keeps its distinctive ending and
/// extension (`Quarterly re…port 2026.pdf`).
pub fn ellipsize(label: &str, font: Font, size: f32, width: f32, middle: bool) -> String {
    if text_width(label, font, size) <= width {
        return label.to_owned();
    }
    let mut boundaries: Vec<_> = label.char_indices().map(|(index, _)| index).collect();
    boundaries.push(label.len());
    let characters = boundaries.len() - 1;
    // `kept` characters of the label, around one ellipsis.
    let shorten = |kept: usize| {
        if middle {
            let tail = kept / 2;
            let head = kept - tail;
            format!(
                "{}…{}",
                &label[..boundaries[head]],
                &label[boundaries[characters - tail]..]
            )
        } else {
            format!("{}…", &label[..boundaries[kept]])
        }
    };
    // The most characters that fit, by bisection: widths grow with `kept`.
    let (mut low, mut high) = (0, characters.saturating_sub(1));
    while low < high {
        let middle = (low + high).div_ceil(2);
        if text_width(&shorten(middle), font, size) <= width {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    shorten(low)
}

/// `label` set in at most two lines of `width`: whole when it fits, else
/// broken after a space, underscore or hyphen (never just before the
/// extension), or between characters when no such break fits. A second line
/// that is still too long loses its middle, keeping the extension.
pub fn two_lines(label: &str, font: Font, size: f32, width: f32) -> String {
    if text_width(label, font, size) <= width {
        return label.to_owned();
    }
    let extension = label.rfind('.').filter(|dot| *dot > 0);
    let fits = |end: usize| text_width(label[..end].trim_end(), font, size) <= width;
    let separator = label
        .char_indices()
        .filter(|(_, c)| matches!(c, ' ' | '_' | '-'))
        .map(|(index, c)| index + c.len_utf8())
        .filter(|end| *end < label.len() && Some(*end) != extension)
        .rfind(|end| fits(*end));
    let end = separator.unwrap_or_else(|| {
        // The most characters that fit, by bisection.
        let boundaries: Vec<_> = label
            .char_indices()
            .map(|(index, _)| index)
            .skip(1)
            .collect();
        let (mut low, mut high) = (0, boundaries.len());
        while low < high {
            let middle = (low + high).div_ceil(2);
            if fits(boundaries[middle - 1]) {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        boundaries
            .get(low.max(1) - 1)
            .copied()
            .unwrap_or(label.len())
    });
    let (first, rest) = label.split_at(end);
    format!(
        "{}\n{}",
        first.trim_end(),
        ellipsize(rest.trim_start(), font, size, width, true)
    )
}

pub fn single_line<'a, Message: 'a>(
    label: impl Into<String>,
    font: Font,
    size: f32,
    color: Option<Color>,
    reveal: bool,
) -> Element<'a, Message> {
    line(label.into(), font, size, color, reveal, false)
}

/// The length in graphemes of `name` before its extension, counting a
/// compressed tarball's two suffixes as one; `None` without an extension.
pub fn stem_length(name: &str) -> Option<usize> {
    use unicode_segmentation::UnicodeSegmentation;
    const COMPOUND: [&str; 6] = [
        ".tar.gz",
        ".tar.bz2",
        ".tar.xz",
        ".tar.zst",
        ".tar.lz",
        ".tar.lzma",
    ];
    let end = COMPOUND
        .iter()
        .find_map(|suffix| {
            let start = name
                .len()
                .checked_sub(suffix.len())
                .filter(|start| *start > 0)?;
            (name.is_char_boundary(start) && name[start..].eq_ignore_ascii_case(suffix))
                .then_some(start)
        })
        .or_else(|| name.rfind('.').filter(|dot| *dot > 0))?;
    Some(name[..end].graphemes(true).count())
}

/// A file name on one line, shortened in its middle when it does not fit.
pub fn file_name_line<'a, Message: 'a>(
    label: impl Into<String>,
    font: Font,
    size: f32,
    color: Option<Color>,
    reveal: bool,
) -> Element<'a, Message> {
    line(label.into(), font, size, color, reveal, true)
}

fn line<'a, Message: 'a>(
    label: String,
    font: Font,
    size: f32,
    color: Option<Color>,
    reveal: bool,
    middle: bool,
) -> Element<'a, Message> {
    responsive(move |bounds| {
        let fits = text_width(&label, font, size) <= bounds.width;
        let shown = if fits {
            label.clone()
        } else {
            ellipsize(&label, font, size, bounds.width, middle)
        };
        let line = container(
            text(shown)
                .font(font)
                .size(size)
                .shaping(text::Shaping::Advanced)
                .wrapping(text::Wrapping::None)
                .style(move |_| text::Style { color }),
        )
        .width(Length::Fill)
        .clip(true);
        if reveal && !fits {
            tooltip(
                line,
                text(label.clone()).font(font).size(size),
                tooltip::Position::Bottom,
            )
            .padding(6)
            .style(tooltip_style)
            .delay(std::time::Duration::from_millis(450))
            .into()
        } else {
            line.into()
        }
    })
    .height(Length::Shrink)
    .into()
}

/// A tooltip's bubble: the text and background colours swapped, so it stands
/// clear of whatever it covers, as other desktops draw them.
pub fn tooltip_style(theme: &Theme) -> container::Style {
    let p = canopa_ui::colors(theme);
    container::Style::default()
        .background(p.text)
        .color(p.background)
        .border(iced::Border::default().rounded(6))
        .shadow(iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        })
}
