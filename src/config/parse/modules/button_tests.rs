mod tests {
    use crate::config::ModuleParams;
    use crate::config::parse::parse;

    fn button_config(kdl: &str) -> crate::config::ButtonConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "button")
            .expect("button module");
        match &m.params {
            ModuleParams::Button(b) => b.clone(),
            _ => panic!("expected Button params"),
        }
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg = button_config(
            "modules { module \"button\" { label \"L\"; icon \">\"; on-click \"x\"; tooltip \"{label}\" } }",
        );
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = button_config(
            "modules { module \"button\" { label \"L\"; icon \">\"; on-click \"x\"; tooltip #false } }",
        );
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = "modules { module \"button\" { label \"L\"; icon \">\"; on-click \"x\"; tooltip #true } }";
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = button_config(
            "modules { module \"button\" { label \"L\"; icon \">\"; on-click \"x\" } }",
        );
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }

    #[test]
    fn parse_button_with_all_params() {
        let kdl = r#"
modules {
    module "button" {
        label "Launch Terminal"
        icon ">"
        on-click "kitty"
    }
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.modules.len(), 1);
        match &config.modules[0].params {
            ModuleParams::Button(cfg) => {
                assert_eq!(cfg.label, "Launch Terminal");
                assert_eq!(cfg.icon, ">");
                assert_eq!(
                    cfg.on_click,
                    crate::config::CommandConfig::Shell("kitty".into())
                );
            }
            _ => panic!("expected ModuleParams::Button"),
        }
    }

    #[test]
    fn parse_button_argv_on_click() {
        let kdl = r#"
modules {
    module "button" {
        label "Run"
        icon ">"
        on-click {
            argv "true"
        }
    }
}
"#;
        let config = parse(kdl).unwrap();
        match &config.modules[0].params {
            ModuleParams::Button(cfg) => {
                assert_eq!(
                    cfg.on_click,
                    crate::config::CommandConfig::Argv {
                        program: "true".into(),
                        args: vec![],
                    }
                );
            }
            _ => panic!("expected ModuleParams::Button"),
        }
    }

    #[test]
    fn parse_button_missing_block_errors() {
        // No child block at all -- distinct from a missing individual field.
        let kdl = r#"modules { module "button" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button:") && msg.contains("missing required parameters block"),
            "expected 'button: missing required parameters block' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_button_missing_label_errors() {
        // Child block present but label node absent.
        let kdl = r#"
modules {
    module "button" {
        icon ">"
        on-click "kitty"
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button.label"),
            "expected 'button.label' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_required_string_child_reports_missing_name() {
        let kdl = r#"modules { module "button" { icon ">" on-click "kitty" } }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button.label"),
            "expected 'button.label' in error, got: {msg}"
        );
        assert!(
            msg.contains("missing required parameter"),
            "expected 'missing required parameter' in error, got: {msg}"
        );
        assert!(
            msg.contains("Expected: a string value"),
            "expected 'Expected: a string value' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_button_missing_icon_errors() {
        // Child block present but icon node absent.
        let kdl = r#"
modules {
    module "button" {
        label "Launch"
        on-click "kitty"
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button.icon"),
            "expected 'button.icon' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_button_missing_on_click_errors() {
        let kdl = r#"
modules {
    module "button" {
        label "Launch"
        icon ">"
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button.on-click"),
            "expected 'button.on-click' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_button_wrong_type_errors() {
        let kdl = r#"
modules {
    module "button" {
        label 42
        icon ">"
        on-click "kitty"
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("button.label"),
            "expected 'button.label' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_button_unknown_field_is_structured_diagnostic() {
        let kdl = r#"modules { module "button" { label "Launch"; icon ">"; on-click "kitty"; lable "x" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].button.lable"));
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("label")),
            "expected label suggestion, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_button_shell_command_policy_error_points_at_on_click() {
        let kdl = r#"
command-policy {
    allow-shell #false
}
modules {
    module "button" {
        label "Launch"
        icon ">"
        on-click "kitty"
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].button.on-click"));
        assert!(
            diagnostic
                .to_string()
                .contains("shell commands are disabled")
        );
        assert!(diagnostic.has_span(), "expected source span");
    }

    #[test]
    fn parse_button_format_unknown_placeholder_points_at_format() {
        let kdl = r#"modules { module "button" { label "Launch"; icon ">"; on-click { argv "true" }; format "{missing}" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].button.format"));
        assert!(diagnostic.to_string().contains("unknown placeholder"));
        assert!(diagnostic.has_span(), "expected source span");
    }
}
