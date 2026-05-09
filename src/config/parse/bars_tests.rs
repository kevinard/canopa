#[cfg(test)]
mod tests {
    use crate::config::parse::parse;
    use crate::config::{BarLayer, Edge, ExpandTrigger, OutputTarget};

    #[test]
    fn parse_full_bar_section() {
        let kdl = r#"
bar "main" {
    edge "right"
    collapsed-width 56
    expanded-width 240
}
"#;
        let config = parse(kdl).unwrap();
        assert!(matches!(config.bars[0].edge, Edge::Right));
        assert_eq!(config.bars[0].collapsed_width, 56);
        assert_eq!(config.bars[0].expanded_width, 240);
    }

    #[test]
    fn parse_edge_left() {
        let kdl = r#"bar "main" { edge "left" }"#;
        let config = parse(kdl).unwrap();
        assert!(matches!(config.bars[0].edge, Edge::Left));
    }

    #[test]
    fn parse_unknown_edge_errors() {
        let kdl = r#"bar "main" { edge "diagonal" }"#;
        assert!(parse(kdl).is_err());
    }

    #[test]
    fn parse_empty_string_uses_defaults() {
        let config = parse("").unwrap();
        assert!(matches!(config.bars[0].edge, Edge::Left));
        assert_eq!(config.bars[0].collapsed_width, 48);
        assert_eq!(config.bars[0].expanded_width, 200);
    }

    #[test]
    fn parse_bar_without_children_uses_defaults() {
        // A named bar node with no children block -- falls back to defaults for that bar.
        let config = parse(r#"bar "main""#).unwrap();
        assert_eq!(config.bars[0].collapsed_width, 48);
    }

    #[test]
    fn unknown_edge_error_message() {
        let kdl = r#"bar "main" { edge "diagonal" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(".edge:"),
            "expected '.edge:' in message, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

    #[test]
    fn wrong_type_edge_error_message() {
        let kdl = r#"bar "main" { edge 42 }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(".edge:"),
            "expected '.edge:' in message, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

    #[test]
    fn parse_expand_trigger_hover() {
        let kdl = r#"bar "main" { expand-trigger "hover" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].expand_trigger, ExpandTrigger::Hover);
    }

    #[test]
    fn parse_expand_trigger_click() {
        let kdl = r#"bar "main" { expand-trigger "click" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].expand_trigger, ExpandTrigger::Click);
    }

    #[test]
    fn parse_expand_trigger_defaults_to_click() {
        let config = parse("").unwrap();
        assert_eq!(config.bars[0].expand_trigger, ExpandTrigger::Click);
    }

    #[test]
    fn parse_hover_delay() {
        let kdl = r#"bar "main" { hover-delay 200 }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].hover_delay, 200);
    }

    #[test]
    fn parse_hover_delay_defaults_to_zero() {
        let config = parse("").unwrap();
        assert_eq!(config.bars[0].hover_delay, 0);
    }

    #[test]
    fn parse_tooltip_delay() {
        let kdl = r#"bar "main" { tooltip-delay 250 }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].tooltip_delay_ms, 250);
    }

    #[test]
    fn parse_tooltip_delay_defaults_to_500() {
        let config = parse(r#"bar "main" {}"#).unwrap();
        assert_eq!(config.bars[0].tooltip_delay_ms, 500);
    }

    #[test]
    fn parse_bar_layer_defaults_to_top() {
        let config = parse(r#"bar "main" {}"#).unwrap();
        assert_eq!(config.bars[0].layer, BarLayer::Top);
    }

    #[test]
    fn parse_bar_layer_accepts_all_layer_values() {
        for (raw, expected) in [
            ("background", BarLayer::Background),
            ("bottom", BarLayer::Bottom),
            ("top", BarLayer::Top),
            ("overlay", BarLayer::Overlay),
        ] {
            let config = parse(&format!(r#"bar "main" {{ layer "{raw}" }}"#)).unwrap();
            assert_eq!(config.bars[0].layer, expected);
        }
    }

    #[test]
    fn parse_bar_layer_invalid_value_errors() {
        let err = parse(r#"bar "main" { layer "middle" }"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains(".layer:"), "got: {err}");
        assert!(
            err.contains("Expected: one of: background, bottom, top, overlay"),
            "got: {err}"
        );
    }

    #[test]
    fn parse_tooltip_delay_negative_errors() {
        let kdl = r#"bar "main" { tooltip-delay -1 }"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip-delay"), "got: {err}");
    }

    #[test]
    fn parse_expand_keybind() {
        let kdl = r#"bar "main" { expand-keybind "Super+e" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].expand_keybind, Some("Super+e".to_string()));
    }

    #[test]
    fn parse_expand_keybind_defaults_to_none() {
        let config = parse("").unwrap();
        assert_eq!(config.bars[0].expand_keybind, None);
    }

    #[test]
    fn parse_bar_without_expand_keybind_uses_none() {
        let kdl = r#"bar "main" { edge "left" }"#;
        let config = parse(kdl).unwrap();
        assert!(config.bars[0].expand_keybind.is_none());
    }

    #[test]
    fn parse_expand_false_disables_expand() {
        let kdl = r#"bar "main" { expand #false }"#;
        let config = parse(kdl).unwrap();
        assert!(!config.bars[0].expand);
    }

    #[test]
    fn parse_expand_true_enables_expand() {
        let kdl = r#"bar "main" { expand #true }"#;
        let config = parse(kdl).unwrap();
        assert!(config.bars[0].expand);
    }

    #[test]
    fn parse_expand_defaults_to_true() {
        let config = parse("").unwrap();
        assert!(config.bars[0].expand);
    }

    #[test]
    fn parse_expand_wrong_type_errors() {
        let kdl = r#"bar "main" { expand "yes" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(".expand:"),
            "expected '.expand:' in message, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

    #[test]
    fn parse_unknown_expand_trigger_errors() {
        let kdl = r#"bar "main" { expand-trigger "double-click" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(".expand-trigger:"),
            "expected '.expand-trigger:' in message, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

    #[test]
    fn parse_named_bar_section() {
        let kdl = r#"bar "main" { edge "right" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars.len(), 1);
        assert_eq!(config.bars[0].name, "main");
        assert_eq!(config.bars[0].output_target, OutputTarget::All);
        assert!(matches!(config.bars[0].edge, Edge::Right));
    }

    #[test]
    fn parse_bar_output_field() {
        let kdl = r#"bar "side" { output "DP-1" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].name, "side");
        assert_eq!(
            config.bars[0].output_target,
            OutputTarget::Include(vec!["DP-1".into()])
        );
    }

    #[test]
    fn parse_bar_output_target_accepts_exact_include_list() {
        let kdl = r#"bar "side" { output "DP-1" "HDMI-A-1" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(
            config.bars[0].output_target,
            OutputTarget::Include(vec!["DP-1".into(), "HDMI-A-1".into()])
        );
    }

    #[test]
    fn parse_bar_output_target_accepts_exact_exclude_list() {
        let kdl = r#"bar "side" { output "!eDP-1" "!HDMI-A-1" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(
            config.bars[0].output_target,
            OutputTarget::Exclude(vec!["eDP-1".into(), "HDMI-A-1".into()])
        );
    }

    #[test]
    fn parse_bar_output_target_rejects_mixed_include_and_exclude() {
        let err = parse(r#"bar "side" { output "DP-1" "!eDP-1" }"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("cannot mix"), "got: {err}");
    }

    #[test]
    fn parse_bar_output_target_rejects_duplicate_entries() {
        let err = parse(r#"bar "side" { output "DP-1" "DP-1" }"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("duplicate Output Target entry"), "got: {err}");
    }

    #[test]
    fn parse_multiple_named_bar_sections() {
        let kdl = r#"
bar "main" { output "all" }
bar "top-strip" {
    output "DP-1"
    edge "top"
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars.len(), 2);
        assert_eq!(config.bars[0].name, "main");
        assert_eq!(config.bars[0].output_target, OutputTarget::All);
        assert_eq!(config.bars[1].name, "top-strip");
        assert_eq!(
            config.bars[1].output_target,
            OutputTarget::Include(vec!["DP-1".into()])
        );
        assert!(matches!(config.bars[1].edge, Edge::Top));
    }

    #[test]
    fn parse_bar_missing_name_errors() {
        // Unnamed bar node (no positional string argument) must error.
        let kdl = r#"bar { edge "left" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("missing name"),
            "expected 'missing name' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_bar_defaults_output_target_to_all() {
        let kdl = r#"bar "main" { }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].output_target, OutputTarget::All);
    }

    #[test]
    fn parse_negative_collapsed_width_errors() {
        let result = parse(r#"bar "main" { collapsed-width -5 }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(".collapsed-width"),
            "error should contain '.collapsed-width': {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "error should contain 'Expected:': {msg}"
        );
    }

    #[test]
    fn parse_negative_expanded_width_errors() {
        let result = parse(r#"bar "main" { expanded-width -10 }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(".expanded-width"),
            "error should contain '.expanded-width': {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "error should contain 'Expected:': {msg}"
        );
    }

    #[test]
    fn parse_bar_with_full_settings() {
        let kdl = r#"
bar "full" {
    output "DP-2"
    edge "right"
    collapsed-width 56
    expanded-width 240
    expand #true
    expand-trigger "click"
    hover-delay 0
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].name, "full");
        assert_eq!(
            config.bars[0].output_target,
            OutputTarget::Include(vec!["DP-2".into()])
        );
        assert!(matches!(config.bars[0].edge, Edge::Right));
        assert_eq!(config.bars[0].collapsed_width, 56);
        assert_eq!(config.bars[0].expanded_width, 240);
        assert!(config.bars[0].expand);
    }

    #[test]
    fn parse_bar_name_preserved() {
        let kdl = r#"bar "my-custom-bar" { }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].name, "my-custom-bar");
    }

    #[test]
    fn parse_bar_output_all_explicit() {
        let kdl = r#"bar "main" { output "all" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].output_target, OutputTarget::All);
    }

    #[test]
    fn parse_empty_doc_produces_single_default_bar() {
        let config = parse("").unwrap();
        assert_eq!(config.bars.len(), 1);
        assert_eq!(config.bars[0].name, "main");
        assert_eq!(config.bars[0].output_target, OutputTarget::All);
    }

    #[test]
    fn parse_two_bars_different_outputs() {
        let kdl = r#"
bar "left" {
    output "eDP-1"
    edge "left"
}
bar "right" {
    output "DP-1"
    edge "right"
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars.len(), 2);
        assert_eq!(
            config.bars[0].output_target,
            OutputTarget::Include(vec!["eDP-1".into()])
        );
        assert_eq!(
            config.bars[1].output_target,
            OutputTarget::Include(vec!["DP-1".into()])
        );
    }

    #[test]
    fn parse_bar_with_modules_and_output() {
        let kdl = r#"
bar "main" {
    output "eDP-1"
    modules {
        start "workspaces"
        end "clock"
    }
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(
            config.bars[0].output_target,
            OutputTarget::Include(vec!["eDP-1".into()])
        );
        assert_eq!(config.bars[0].modules.start, vec!["workspaces"]);
        assert_eq!(config.bars[0].modules.end, vec!["clock"]);
    }

    #[test]
    fn parse_bar_rejects_legacy_physical_zone_names() {
        let kdl = r#"
bar "main" {
    modules {
        top "clock"
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown zone \"top\""), "got: {msg}");
        assert!(
            msg.contains("Expected: one of: start, center, end"),
            "got: {msg}"
        );
    }

    #[test]
    fn parse_bar_edge_wrong_type_is_structured_diagnostic() {
        let err = parse(r#"bar "main" { edge 42 }"#).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("bar[0].edge"));
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("left")),
            "expected edge help, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_bar_unknown_field_is_structured_diagnostic() {
        let err = parse(r#"bar "main" { collaped-width 32 }"#).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("bar[0].collaped-width"));
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("collapsed-width")),
            "expected collapsed-width suggestion, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_bar_unknown_zone_is_structured_diagnostic() {
        let err = parse(r#"bar "main" { modules { top "clock" } }"#).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("bar[0].modules.top"));
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("start")),
            "expected zone help, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_bar_missing_name_error_format() {
        let kdl = r#"bar { }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("bar:"), "expected 'bar:' in error: {msg}");
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in error: {msg}"
        );
    }

    #[test]
    fn parse_multiple_bars_preserves_declaration_order() {
        let kdl = r#"
bar "alpha" { }
bar "beta" { }
bar "gamma" { }
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars.len(), 3);
        assert_eq!(config.bars[0].name, "alpha");
        assert_eq!(config.bars[1].name, "beta");
        assert_eq!(config.bars[2].name, "gamma");
    }

    #[test]
    fn parse_two_bars_both_target_all() {
        let kdl = r#"
bar "side" { output "all" }
bar "top" {
    output "all"
    edge "top"
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars.len(), 2);
        assert_eq!(config.bars[0].output_target, OutputTarget::All);
        assert_eq!(config.bars[1].output_target, OutputTarget::All);
        assert!(matches!(config.bars[1].edge, Edge::Top));
    }

    #[test]
    fn parse_bar_output_target_field_round_trip() {
        for output in &["all", "eDP-1", "DP-2", "HDMI-A-1"] {
            let kdl = format!(r#"bar "main" {{ output "{output}" }}"#);
            let config = parse(&kdl).unwrap();
            let expected = if *output == "all" {
                OutputTarget::All
            } else {
                OutputTarget::Include(vec![output.to_string()])
            };
            assert_eq!(config.bars[0].output_target, expected);
        }
    }
}
