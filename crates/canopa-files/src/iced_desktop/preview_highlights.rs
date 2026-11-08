//! Translate prepared lexical spans into immutable renderer coordinates.
//! Lexing and this conversion run on the existing preview worker.
use crate::{
    Cancellation,
    preview::{HighlightKind, PreviewContent, TextHighlight},
};
use iced::{Color, Font, Theme, advanced::text::highlighter};
use std::{io, ops::Range, sync::Arc};

#[derive(Debug)]
struct Line {
    text: String,
    spans: Vec<(Range<usize>, HighlightKind)>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Prepared(Arc<[Line]>);

impl Prepared {
    pub fn new(content: &PreviewContent, cancellation: &Cancellation) -> io::Result<Self> {
        if cancellation.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "preview cancelled",
            ));
        }
        let PreviewContent::Text {
            text, highlights, ..
        } = content
        else {
            return Ok(Self::default());
        };
        if highlights.is_empty() {
            return Ok(Self::default());
        }
        Self::lines(text, highlights, cancellation)
    }

    fn lines(text: &str, spans: &[TextHighlight], cancellation: &Cancellation) -> io::Result<Self> {
        let last = spans
            .iter()
            .map(|span| span.end as usize)
            .max()
            .unwrap_or(0);
        let mut offset = 0;
        let mut first = 0;
        let mut lines = Vec::new();
        for (range, ending) in iced::advanced::graphics::text::cosmic_text::LineIter::new(text) {
            if cancellation.is_cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "preview cancelled",
                ));
            }
            let line = &text[range];
            let length = line.chars().count();
            let end = offset + length;
            while first < spans.len() && spans[first].end as usize <= offset {
                first += 1;
            }
            let coverage = length.min(last.saturating_sub(offset));
            let bytes: Vec<_> = line
                .char_indices()
                .map(|(byte, _)| byte)
                .chain(std::iter::once(line.len()))
                .take(coverage + 1)
                .collect();
            let ranges = spans[first..]
                .iter()
                .take_while(|span| (span.start as usize) < end)
                .filter_map(|span| {
                    let start = (span.start as usize).saturating_sub(offset).min(length);
                    let end = (span.end as usize).saturating_sub(offset).min(length);
                    (start < end).then(|| (bytes[start]..bytes[end], span.kind))
                })
                .collect();
            lines.push(Line {
                text: line.into(),
                spans: ranges,
            });
            offset = end + ending.as_str().chars().count();
            if offset >= last {
                break;
            }
        }
        Ok(Self(lines.into()))
    }

    pub fn settings(&self, theme: &Theme) -> Settings {
        let colors = canopa_ui::colors(theme);
        let rgb = |color: Color| {
            canopa_theme::Rgb(
                (color.r * 255.0).round() as u8,
                (color.g * 255.0).round() as u8,
                (color.b * 255.0).round() as u8,
            )
        };
        Settings {
            prepared: self.clone(),
            colors: [colors.success, colors.warning, colors.muted].map(|candidate| {
                if rgb(candidate).contrast(rgb(colors.background)) >= 4.5
                    && rgb(candidate).contrast(rgb(colors.raised)) >= 4.5
                {
                    candidate
                } else {
                    colors.text
                }
            }),
        }
    }
}

#[derive(Clone)]
pub(super) struct Settings {
    prepared: Prepared,
    colors: [Color; 3],
}
impl PartialEq for Settings {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.prepared.0, &other.prepared.0) && self.colors == other.colors
    }
}

pub(super) struct Highlighter {
    settings: Settings,
    line: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Highlight {
    kind: HighlightKind,
    color: Option<Color>,
}

impl highlighter::Highlighter for Highlighter {
    type Settings = Settings;
    type Highlight = Highlight;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Highlight)>;

    fn new(settings: &Settings) -> Self {
        Self {
            settings: settings.clone(),
            line: 0,
        }
    }
    fn update(&mut self, settings: &Settings) {
        self.settings = settings.clone();
        self.line = 0;
    }
    fn change_line(&mut self, line: usize) {
        self.line = self.line.min(line);
    }
    fn highlight_line(&mut self, text: &str) -> Self::Iterator<'_> {
        let line = self.settings.prepared.0.get(self.line);
        self.line = self.line.saturating_add(1);
        // The editor first lays out an empty buffer. Never apply the prepared
        // ranges to that buffer, nor to a line from a different source.
        line.filter(|line| line.text == text)
            .into_iter()
            .flat_map(|line| &line.spans)
            .map(|(range, kind)| {
                let color = match kind {
                    HighlightKind::Keyword => None,
                    HighlightKind::String => Some(self.settings.colors[0]),
                    HighlightKind::Number => Some(self.settings.colors[1]),
                    HighlightKind::Comment => Some(self.settings.colors[2]),
                };
                (range.clone(), Highlight { kind: *kind, color })
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
    fn current_line(&self) -> usize {
        if self.line < self.settings.prepared.0.len() {
            self.line
        } else {
            usize::MAX
        }
    }
}

pub(super) fn format(highlight: &Highlight, _: &Theme) -> highlighter::Format<Font> {
    highlighter::Format {
        color: highlight.color,
        font: match highlight.kind {
            HighlightKind::Keyword => Some(Font {
                weight: iced::font::Weight::Bold,
                ..Font::MONOSPACE
            }),
            HighlightKind::Comment => Some(Font {
                style: iced::font::Style::Italic,
                ..Font::MONOSPACE
            }),
            _ => None,
        },
    }
}

pub(super) fn style(
    theme: &Theme,
    status: iced::widget::text_editor::Status,
) -> iced::widget::text_editor::Style {
    let mut style = iced::widget::text_editor::default(theme, status);
    // Keep all lexical foregrounds readable when their text is selected.
    let palette = canopa_ui::colors(theme);
    style.selection = palette.raised;
    style.background = if theme.extended_palette().is_dark {
        palette.background.into()
    } else {
        palette.background.scale_alpha(0.42).into()
    };
    style.border.color = palette.border.scale_alpha(0.58);
    style.border.width = 1.0;
    style.border.radius = 9.0.into();
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::PreviewRequest;
    use iced::advanced::text::highlighter::Highlighter as _;

    fn source(text: &str) -> PreviewContent {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("café.rs");
        std::fs::write(&path, text).unwrap();
        PreviewRequest::new(path)
            .prepare(&Cancellation::default())
            .unwrap()
            .content
    }
    fn theme(scheme: canopa_theme::ColorScheme) -> Theme {
        canopa_ui::theme(canopa_theme::Appearance {
            color_scheme: scheme,
            ..Default::default()
        })
    }

    #[test]
    fn prepared_character_spans_follow_unicode_and_multiline_comment_endings() {
        let text =
            "let café = \"éclair\";\r\n/* 日本語\r\n second */\n\rfn main() { let count = 42; }\r";
        let prepared = Prepared::new(&source(text), &Cancellation::default()).unwrap();
        let mut highlighter =
            Highlighter::new(&prepared.settings(&theme(canopa_theme::ColorScheme::Dark)));
        let actual: Vec<_> = iced::advanced::graphics::text::cosmic_text::LineIter::new(text)
            .map(|(range, _)| {
                let line = &text[range];
                highlighter
                    .highlight_line(line)
                    .map(|(range, style)| (line[range].to_owned(), style.kind))
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(
            actual[0],
            vec![
                ("let".into(), HighlightKind::Keyword),
                ("\"éclair\"".into(), HighlightKind::String)
            ]
        );
        assert_eq!(
            actual[1],
            vec![("/* 日本語".into(), HighlightKind::Comment)]
        );
        assert_eq!(
            actual[2],
            vec![(" second */".into(), HighlightKind::Comment)]
        );
        assert_eq!(
            actual[3],
            vec![
                ("fn".into(), HighlightKind::Keyword),
                ("let".into(), HighlightKind::Keyword),
                ("42".into(), HighlightKind::Number)
            ]
        );
        assert_eq!(highlighter.current_line(), usize::MAX);
    }

    #[test]
    fn empty_layout_and_other_lines_cannot_receive_prepared_ranges() {
        let prepared =
            Prepared::new(&source("let café = 42;\n"), &Cancellation::default()).unwrap();
        let settings = prepared.settings(&theme(canopa_theme::ColorScheme::Dark));
        let mut highlighter = Highlighter::new(&settings);
        assert_eq!(highlighter.highlight_line("").count(), 0);
        highlighter.change_line(0);
        assert_eq!(highlighter.highlight_line("a different line").count(), 0);
        highlighter.change_line(0);
        assert_eq!(highlighter.highlight_line("let café = 42;").count(), 2);
        assert!(settings == prepared.settings(&theme(canopa_theme::ColorScheme::Dark)));
    }

    #[test]
    fn cancellation_and_unstyled_text_keep_the_existing_bounded_preparation_contract() {
        let content = source(&"let café = 42;\n".repeat(20_000));
        let prepared = Prepared::new(&content, &Cancellation::default()).unwrap();
        let PreviewContent::Text { highlights, .. } = &content else {
            panic!("text expected")
        };
        assert!(prepared.0.len() < 20_000);
        assert!(
            prepared
                .0
                .iter()
                .map(|line| line.text.chars().count())
                .sum::<usize>()
                <= 128 * 1024
        );
        assert!(highlights.len() <= 8192);
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert_eq!(
            Prepared::new(&content, &cancellation).unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        let plain = PreviewContent::Text {
            text: "plain\n".repeat(30_000),
            language: None,
            truncated: false,
            highlights: Vec::new(),
        };
        let plain = Prepared::new(&plain, &Cancellation::default()).unwrap();
        assert!(plain.0.is_empty());
        assert_eq!(
            Highlighter::new(&plain.settings(&Theme::Dark)).current_line(),
            usize::MAX
        );
    }

    #[test]
    fn palette_changes_restyle_cached_spans_and_selection_keeps_readable_foregrounds() {
        let prepared = Prepared::new(
            &source("let café = 42; // comment\n"),
            &Cancellation::default(),
        )
        .unwrap();
        let dark = prepared.settings(&theme(canopa_theme::ColorScheme::Dark));
        let light = prepared.settings(&theme(canopa_theme::ColorScheme::Light));
        assert!(dark != light);
        let mut highlighter = Highlighter::new(&dark);
        let old: Vec<_> = highlighter
            .highlight_line("let café = 42; // comment")
            .collect();
        highlighter.update(&light);
        assert_eq!(highlighter.current_line(), 0);
        let new: Vec<_> = highlighter
            .highlight_line("let café = 42; // comment")
            .collect();
        assert_ne!(old[1].1.color, new[1].1.color);
        assert_eq!(
            format(&new[0].1, &Theme::Light).font.unwrap().weight,
            iced::font::Weight::Bold
        );
        assert_eq!(
            format(&new[2].1, &Theme::Light).font.unwrap().style,
            iced::font::Style::Italic
        );
        for scheme in [
            canopa_theme::ColorScheme::Dark,
            canopa_theme::ColorScheme::Light,
        ] {
            let theme = theme(scheme);
            let palette = canopa_ui::colors(&theme);
            let rgb = |color: Color| {
                canopa_theme::Rgb(
                    (color.r * 255.0).round() as u8,
                    (color.g * 255.0).round() as u8,
                    (color.b * 255.0).round() as u8,
                )
            };
            for color in prepared.settings(&theme).colors {
                assert!(rgb(color).contrast(rgb(palette.background)) >= 4.5);
                assert!(rgb(color).contrast(rgb(palette.raised)) >= 4.5);
            }
        }
        let mut palette = Theme::Light.palette();
        palette.success = Color::WHITE;
        palette.warning = Color::WHITE;
        let low_contrast = Theme::custom("low contrast", palette);
        let settings = prepared.settings(&low_contrast);
        assert_eq!(settings.colors[0], canopa_ui::colors(&low_contrast).text);
        assert_eq!(settings.colors[1], canopa_ui::colors(&low_contrast).text);
    }
}
