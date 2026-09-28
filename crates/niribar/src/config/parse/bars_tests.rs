mod tests {
    use crate::config::parse::parse;
    use crate::config::{BarLayer, Edge, OutputTarget};

    #[test]
    fn parse_full_bar_section() {
        let kdl = r#"
bar "main" {
    edge "right"
    collapsed-width 56
}
"#;
        let config = parse(kdl).unwrap();
        assert!(matches!(config.bars[0].edge, Edge::Right));
        assert_eq!(config.bars[0].collapsed_width, 56);
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
    fn parse_bar_with_full_settings() {
        let kdl = r#"
bar "full" {
    output "DP-2"
    edge "right"
    collapsed-width 56
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
    fn parse_bar_actions_and_panel_layout() {
        let kdl = r#"
bar "main" {
    actions {
        click { open-panel }
        Ctrl+E { toggle-panel }
    }
    panel {
        width 420
        max-height 720
        start "workspaces"
        center "focused-window"
        end "network" "volume" "battery" "clock"
        animation { open-ms 180 close-ms 120 }
    }
}
"#;
        let config = parse(kdl).unwrap();
        let bar = &config.bars[0];
        assert_eq!(
            bar.actions.action_for(&crate::config::ActionTrigger::Click),
            Some(&crate::config::PanelAction::OpenPanel)
        );
        assert_eq!(
            bar.actions
                .action_for(&crate::config::ActionTrigger::Key("Ctrl+E".into())),
            Some(&crate::config::PanelAction::TogglePanel)
        );
        assert_eq!(bar.panel.width, Some(420));
        assert_eq!(bar.panel.max_height, Some(720));
        assert_eq!(bar.panel.modules.start, vec!["workspaces"]);
        assert_eq!(bar.panel.modules.center, vec!["focused-window"]);
        assert_eq!(
            bar.panel.modules.end,
            vec!["network", "volume", "battery", "clock"]
        );
        assert_eq!(bar.panel.animation.open_ms, 180.0);
        assert_eq!(bar.panel.animation.close_ms, 120.0);
    }

    #[test]
    fn parse_panel_omitted_dimensions_remain_content_aware() {
        let config = parse(
            r#"
bar "main" {
    panel {
        start "clock"
    }
}
"#,
        )
        .unwrap();

        assert_eq!(config.bars[0].panel.width, None);
        assert_eq!(config.bars[0].panel.height, None);
        assert_eq!(config.bars[0].panel.max_width, None);
        assert_eq!(config.bars[0].panel.max_height, None);
        assert!(!config.bars[0].uses_drawer());
    }

    #[test]
    fn drawer_presentation_applies_only_to_vertical_bars() {
        for edge in ["left", "right", "top", "bottom"] {
            let config = parse(&format!(
                "bar \"main\" {{ edge \"{edge}\"; panel {{ presentation \"drawer\"; }} }}"
            ))
            .unwrap();
            assert_eq!(
                config.bars[0].uses_drawer(),
                matches!(edge, "left" | "right")
            );
        }
        let config = parse("bar \"main\" { panel { presentation \"auto\"; } }").unwrap();
        assert!(!config.bars[0].uses_drawer());
    }

    #[test]
    fn bundled_vertical_bar_opens_module_details_in_its_drawer() {
        let config = parse(crate::config::defaults::DEFAULT_CONFIG).unwrap();
        assert!(config.bars[0].uses_drawer());
        for name in ["clock", "volume", "battery", "focused-window"] {
            let module = config
                .modules
                .iter()
                .find(|module| module.name == name)
                .unwrap();
            assert_eq!(
                module
                    .actions
                    .action_for(&crate::config::ActionTrigger::Click),
                Some(&crate::config::PanelAction::OpenPanel),
                "{name} must open its detail content when clicked",
            );
        }
    }

    #[test]
    fn panel_presentation_rejects_invalid_or_duplicate_values() {
        for fields in [
            "presentation \"floating\";",
            "presentation 1;",
            "presentation \"drawer\"; presentation \"auto\";",
        ] {
            let error = parse(&format!("bar \"main\" {{ panel {{ {fields} }} }}")).unwrap_err();
            assert!(error.to_string().contains("presentation"), "{error}");
        }
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
