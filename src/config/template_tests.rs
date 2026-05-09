#[cfg(test)]
mod tests {
    use super::super::*;

    fn vars<'a>(pairs: &[(&'a str, &'a str)]) -> HashMap<&'a str, &'a str> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn text_template_substitutes_value() {
        let template = TextTemplate::parse("{value}% left", &["value"], "test.format").unwrap();

        assert_eq!(template.render(&vars(&[("value", "42")])), "42% left");
    }

    #[test]
    fn format_template_renders_borrowed_pairs() {
        let template =
            TextTemplate::parse("{icon} {value}", &["icon", "value"], "test.format").unwrap();
        let values = [
            ("icon", std::borrow::Cow::Borrowed("I")),
            ("value", std::borrow::Cow::Owned("42".to_string())),
        ];

        assert_eq!(template.render_cow_pairs(&values), "I 42");
    }

    #[test]
    fn text_template_substitutes_repeated_placeholders() {
        let template = TextTemplate::parse("{value}/{value}", &["value"], "test.format").unwrap();

        assert_eq!(template.render(&vars(&[("value", "42")])), "42/42");
    }

    #[test]
    fn text_template_escapes_literal_braces() {
        let template = TextTemplate::parse("{{{value}}}", &["value"], "test.format").unwrap();

        assert_eq!(template.render(&vars(&[("value", "42")])), "{42}");
    }

    #[test]
    fn text_template_rejects_unmatched_open_brace() {
        let err = TextTemplate::parse("{value", &["value"], "test.format").unwrap_err();

        assert!(err.to_string().contains("unmatched"));
    }

    #[test]
    fn text_template_rejects_empty_placeholder() {
        let err = TextTemplate::parse("{}", &["value"], "test.format").unwrap_err();

        assert!(err.to_string().contains("empty placeholder"));
    }

    #[test]
    fn text_template_rejects_disallowed_placeholder() {
        let err = TextTemplate::parse("{missing}", &["value"], "test.format").unwrap_err();

        assert!(err.to_string().contains("unknown placeholder"));
    }

    #[test]
    fn format_template_renders_multiple_segments() {
        let template =
            FormatTemplate::parse(&["{icon}", "{value}%"], &["icon", "value"], "test.format")
                .unwrap();

        assert_eq!(
            template.render_segments(&[("icon", "▓".to_string()), ("value", "42".to_string())]),
            vec!["▓".to_string(), "42%".to_string()]
        );
    }

    #[test]
    fn render_styled_segments_returns_resolved_style_per_segment() {
        use crate::config::style::{ColorValue, SegmentStyles, Style, ThemeToken};

        let mut tmpl =
            FormatTemplate::parse(&["{icon}", "{value}%"], &["icon", "value"], "test.format")
                .unwrap();
        tmpl.set_segment_styles(
            0,
            SegmentStyles::Static(Style {
                color: Some(ColorValue::Token(ThemeToken::Accent)),
                ..Default::default()
            }),
        );
        let rendered =
            tmpl.render_styled_segments(&[("icon", "▓".into()), ("value", "42".into())], None);
        assert_eq!(rendered.len(), 2);
        assert_eq!(rendered[0].0, "▓");
        assert_eq!(rendered[0].1.color, Some(ThemeToken::Accent.resolve()));
        assert_eq!(rendered[1].0, "42%");
        assert_eq!(rendered[1].1, ResolvedStyle::default());
    }

    #[test]
    fn render_styled_segments_picks_state_keyed_style() {
        use crate::config::style::{ColorValue, SegmentStyles, Style, ThemeToken};

        let mut tmpl = FormatTemplate::parse(&["{icon}"], &["icon"], "test.format").unwrap();
        tmpl.set_segment_styles(
            0,
            SegmentStyles::Stateful {
                default: Some(Style {
                    color: Some(ColorValue::Token(ThemeToken::Accent)),
                    ..Default::default()
                }),
                by_state: vec![(
                    "critical".into(),
                    Style {
                        color: Some(ColorValue::Token(ThemeToken::Critical)),
                        ..Default::default()
                    },
                )],
            },
        );
        let crit = tmpl.render_styled_segments(&[("icon", "x".into())], Some("critical"));
        assert_eq!(crit[0].1.color, Some(ThemeToken::Critical.resolve()));
        let norm = tmpl.render_styled_segments(&[("icon", "x".into())], Some("normal"));
        assert_eq!(norm[0].1.color, Some(ThemeToken::Accent.resolve()));
    }

    #[test]
    fn render_styled_segments_merges_default_style_under_segment_style() {
        use crate::config::style::{ColorValue, SegmentStyles, Style, ThemeToken, Weight};

        let mut tmpl =
            FormatTemplate::parse(&["{icon}", "{value}%"], &["icon", "value"], "test.format")
                .unwrap();
        tmpl.set_segment_styles(
            0,
            SegmentStyles::Static(Style {
                weight: Some(Weight::Bold),
                ..Default::default()
            }),
        );

        let default_style = Style {
            color: Some(ColorValue::Token(ThemeToken::Subtext)),
            size: Some(13.0),
            weight: Some(Weight::Light),
        };
        let rendered = tmpl.render_styled_segments_with_default(
            &[("icon", "x".into()), ("value", "42".into())],
            None,
            &default_style,
        );

        assert_eq!(rendered[0].1.color, Some(ThemeToken::Subtext.resolve()));
        assert_eq!(rendered[0].1.size, Some(13.0));
        assert_eq!(rendered[0].1.weight, Some(Weight::Bold));
        assert_eq!(rendered[1].1.color, Some(ThemeToken::Subtext.resolve()));
        assert_eq!(rendered[1].1.size, Some(13.0));
        assert_eq!(rendered[1].1.weight, Some(Weight::Light));
    }
}
