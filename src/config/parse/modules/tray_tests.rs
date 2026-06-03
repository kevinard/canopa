mod tests {
    use crate::config::parse::parse;

    fn tray_config(kdl: &str) -> crate::config::TrayConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "tray")
            .expect("tray module");
        match &m.params {
            crate::config::ModuleParams::Tray(t) => t.clone(),
            _ => panic!("expected Tray params"),
        }
    }

    #[test]
    fn format_field_rejected() {
        let kdl = r#"modules { module "tray" { format "{title}" } }"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("format"), "got: {err}");
    }

    #[test]
    fn tooltip_title_string_parsed() {
        let cfg = tray_config(r#"modules { module "tray" { tooltip "{title}" } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = tray_config(r#"modules { module "tray" { tooltip #false } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_unknown_placeholder_errors() {
        let kdl = r#"modules { module "tray" { tooltip "{value}" } }"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("value"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = tray_config(r#"modules { module "tray" {} }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }

    #[test]
    fn menu_search_defaults_to_disabled() {
        let cfg = tray_config(r#"modules { module "tray" {} }"#);
        assert!(!cfg.menu_search);
    }

    #[test]
    fn menu_search_true_enables_sni_menu_search() {
        let cfg = tray_config(r#"modules { module "tray" { menu-search #true } }"#);
        assert!(cfg.menu_search);
    }

    #[test]
    fn parse_tray_unknown_field_is_structured_diagnostic() {
        let kdl = r#"modules { module "tray" { tooltipp "{title}" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].tray.tooltipp"));
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("tooltip")),
            "expected tooltip suggestion, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_tray_format_rejection_points_at_format() {
        let kdl = r#"modules { module "tray" { format "{title}" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].tray.format"));
        assert!(diagnostic.to_string().contains("does not support format"));
        assert!(diagnostic.has_span(), "expected source span");
    }

    #[test]
    fn parse_tray_tooltip_unknown_placeholder_points_at_tooltip() {
        let kdl = r#"modules { module "tray" { tooltip "{value}" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].tray.tooltip"));
        assert!(diagnostic.to_string().contains("unknown placeholder"));
        assert!(diagnostic.has_span(), "expected source span");
    }
}
