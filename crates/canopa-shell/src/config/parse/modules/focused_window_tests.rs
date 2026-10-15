mod tests {
    use crate::config::parse::parse;

    fn focused_window_config(kdl: &str) -> crate::config::FocusedWindowConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "focused-window")
            .expect("focused-window module");
        match &m.params {
            crate::config::ModuleParams::FocusedWindow(f) => f.clone(),
            _ => panic!("expected FocusedWindow params"),
        }
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg =
            focused_window_config(r#"modules { module "focused-window" { tooltip "{title}" } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg =
            focused_window_config(r#"modules { module "focused-window" { tooltip #false } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "focused-window" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = focused_window_config(r#"modules { module "focused-window" {} }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }

    #[test]
    fn focused_window_defaults_to_icon_title_auto_rotation_and_fallback_icon() {
        let cfg = focused_window_config(r#"modules { module "focused-window" {} }"#);

        assert_eq!(
            cfg.format
                .collapsed
                .as_ref()
                .expect("default format")
                .render_segments(&[
                    ("icon", "I".into()),
                    ("title", "Window".into()),
                    ("value", "Window".into()),
                    ("app-id", "app".into()),
                ]),
            vec!["I".to_string(), "Window".to_string()]
        );
        assert!(cfg.format.expanded.is_none());
        assert!(cfg.format.horizontal.is_none());
        assert_eq!(cfg.content_rotation, crate::config::ContentRotation::Auto);
        assert_eq!(cfg.fallback_icon, "□");
    }

    #[test]
    fn max_length_defaults_to_fifty_and_accepts_custom_limits_or_zero() {
        let cfg = focused_window_config(r#"modules { module "focused-window" {} }"#);
        assert_eq!(cfg.max_length, 50);
        for limit in [0, 1, 20, 100] {
            let cfg = focused_window_config(&format!(
                r#"modules {{ module "focused-window" {{ max-length {limit} }} }}"#,
            ));
            assert_eq!(cfg.max_length, limit);
        }
    }

    #[test]
    fn max_length_is_supported_for_custom_focused_window_module_names() {
        let cfg =
            parse(r#"modules { module "window-title" { type "focused-window"; max-length 24 } }"#)
                .unwrap();
        let module = cfg
            .modules
            .iter()
            .find(|m| m.name == "window-title")
            .unwrap();
        let crate::config::ModuleParams::FocusedWindow(cfg) = &module.params else {
            panic!("expected focused-window config");
        };
        assert_eq!(cfg.max_length, 24);
    }

    #[test]
    fn invalid_max_length_reports_the_field_and_source_span() {
        for value in ["-1", "4294967296", "\"50\"", "1.5", "#true", ""] {
            let err = parse(&format!(
                r#"modules {{ module "focused-window" {{ max-length {value} }} }}"#,
            ))
            .unwrap_err();
            let diagnostic = err.as_diagnostic().expect("config diagnostic");
            assert_eq!(
                diagnostic.path(),
                Some("modules[0].focused-window.max-length")
            );
            assert!(diagnostic.has_span(), "expected source span for {value:?}");
        }
    }

    #[test]
    fn duplicate_max_length_is_rejected() {
        let err = parse(r#"modules { module "focused-window" { max-length 20; max-length 30 } }"#)
            .unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(
            diagnostic.path(),
            Some("modules[0].focused-window.max-length")
        );
        assert!(diagnostic.has_span(), "expected source span");
    }

    #[test]
    fn parse_focused_window_fallback_icon() {
        let cfg =
            focused_window_config(r#"modules { module "focused-window" { fallback-icon "app" } }"#);

        assert_eq!(cfg.fallback_icon, "app");
    }

    #[test]
    fn parse_focused_window_empty_fallback_icon_errors() {
        let kdl = r#"modules { module "focused-window" { fallback-icon "" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(
            diagnostic.path(),
            Some("modules[0].focused-window.fallback-icon")
        );
        assert!(diagnostic.to_string().contains("must not be empty"));
        assert!(diagnostic.has_span(), "expected source span");
    }

    #[test]
    fn parse_focused_window_app_id_placeholder_in_format_and_tooltip() {
        let cfg = focused_window_config(
            r#"modules { module "focused-window" { format "{app-id}"; tooltip "{app-id}" } }"#,
        );

        assert_eq!(
            cfg.format
                .collapsed
                .as_ref()
                .expect("format")
                .render_segments(&[("app-id", "org.app".into())]),
            vec!["org.app".to_string()]
        );
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn parse_focused_window_unknown_field_is_structured_diagnostic() {
        let kdl = r#"modules { module "focused-window" { tooltipp "{title}" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(
            diagnostic.path(),
            Some("modules[0].focused-window.tooltipp")
        );
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("tooltip")),
            "expected tooltip suggestion, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_focused_window_format_unknown_placeholder_points_at_format() {
        let kdl = r#"modules { module "focused-window" { format "{missing}" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].focused-window.format"));
        assert!(diagnostic.to_string().contains("unknown placeholder"));
        assert!(diagnostic.has_span(), "expected source span");
    }
}
