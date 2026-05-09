#[cfg(test)]
mod tests {
    use crate::config::parse::parse;

    #[test]
    fn parse_animation_expand_ms() {
        let config = parse(r#"bar "main" { animation { expand-ms 400 } }"#).unwrap();
        assert_eq!(config.bars[0].animation.expand_ms, 400.0);
        assert_eq!(config.bars[0].animation.collapse_ms, 200.0); // default unchanged
    }

    #[test]
    fn parse_animation_collapse_ms() {
        let config = parse(r#"bar "main" { animation { collapse-ms 100 } }"#).unwrap();
        assert_eq!(config.bars[0].animation.collapse_ms, 100.0);
        assert_eq!(config.bars[0].animation.expand_ms, 280.0); // default unchanged
    }

    #[test]
    fn parse_animation_defaults_when_omitted() {
        let config = parse("").unwrap();
        assert_eq!(config.bars[0].animation.expand_ms, 280.0);
        assert_eq!(config.bars[0].animation.collapse_ms, 200.0);
    }

    #[test]
    fn parse_animation_node_no_children_uses_defaults() {
        // `animation` node with no children block -> defaults
        let config = parse(r#"bar "main" { animation }"#).unwrap();
        assert_eq!(config.bars[0].animation.expand_ms, 280.0);
        assert_eq!(config.bars[0].animation.collapse_ms, 200.0);
    }

    #[test]
    fn parse_animation_negative_expand_ms_errors() {
        let result = parse(r#"bar "main" { animation { expand-ms -1 } }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(".animation.expand-ms"),
            "error should contain '.animation.expand-ms': {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "error should contain 'Expected:': {msg}"
        );
    }

    #[test]
    fn parse_animation_negative_collapse_ms_errors() {
        let result = parse(r#"bar "main" { animation { collapse-ms -1 } }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(".animation.collapse-ms"),
            "error should contain '.animation.collapse-ms': {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "error should contain 'Expected:': {msg}"
        );
    }

    #[test]
    fn parse_animation_negative_expand_ms_is_structured_diagnostic() {
        let err = parse(r#"bar "main" { animation { expand-ms -1 } }"#).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("bar[0].animation.expand-ms"));
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("280")),
            "expected duration help, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_animation_unknown_field_is_structured_diagnostic() {
        let err = parse(r#"bar "main" { animation { expand 280 } }"#).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("bar[0].animation.expand"));
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("expand-ms")),
            "expected expand-ms suggestion, got: {diagnostic:?}"
        );
    }
}
