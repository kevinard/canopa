#[cfg(test)]
mod tests {
    use crate::config::ModuleParams;
    use crate::config::parse::parse;

    fn script_config(kdl: &str) -> crate::config::ScriptConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "script")
            .expect("script module");
        match &m.params {
            ModuleParams::Script(s) => s.clone(),
            _ => panic!("expected Script params"),
        }
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg = script_config(
            "modules { module \"script\" { command \"date\"; tooltip \"{output}\" } }",
        );
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg =
            script_config("modules { module \"script\" { command \"date\"; tooltip #false } }");
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = "modules { module \"script\" { command \"date\"; tooltip #true } }";
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = script_config(r#"modules { module "script" { command "date" } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }

    #[test]
    fn parse_script_with_command_and_interval() {
        let kdl = r#"
modules {
    module "script" {
        command "date"
        interval 10
    }
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.modules.len(), 1);
        match &config.modules[0].params {
            ModuleParams::Script(cfg) => {
                assert_eq!(
                    cfg.command,
                    crate::config::CommandConfig::Shell("date".into())
                );
                assert_eq!(cfg.interval, 10);
            }
            _ => panic!("expected ModuleParams::Script"),
        }
    }

    #[test]
    fn parse_script_argv_command() {
        let kdl = r#"
modules {
    module "script" {
        command {
            argv "printf" "hello"
        }
    }
}
"#;
        let config = parse(kdl).unwrap();
        match &config.modules[0].params {
            ModuleParams::Script(cfg) => {
                assert_eq!(
                    cfg.command,
                    crate::config::CommandConfig::Argv {
                        program: "printf".into(),
                        args: vec!["hello".into()],
                    }
                );
            }
            _ => panic!("expected ModuleParams::Script"),
        }
    }

    #[test]
    fn parse_script_defaults_interval_to_5() {
        let kdl = r#"
modules {
    module "script" {
        command "date"
    }
}
"#;
        let config = parse(kdl).unwrap();
        match &config.modules[0].params {
            ModuleParams::Script(cfg) => {
                assert_eq!(
                    cfg.command,
                    crate::config::CommandConfig::Shell("date".into())
                );
                assert_eq!(cfg.interval, 5);
            }
            _ => panic!("expected ModuleParams::Script"),
        }
    }

    #[test]
    fn parse_script_missing_command_errors() {
        let kdl = r#"modules { module "script" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("script.command"),
            "expected 'script.command' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_script_empty_command_errors() {
        let kdl = r#"
modules {
    module "script" {
        command ""
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("script.command"),
            "expected 'script.command' in error, got: {msg}"
        );
        assert!(
            msg.contains("must not be empty"),
            "expected 'must not be empty' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_script_command_without_value_errors() {
        let kdl = r#"
modules {
    module "script" {
        command
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("script.command"),
            "expected 'script.command' in error, got: {msg}"
        );
        assert!(
            msg.contains("expected a string value"),
            "expected 'expected a string value' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_script_zero_interval_errors() {
        let kdl = r#"
modules {
    module "script" {
        command "date"
        interval 0
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("script.interval"),
            "expected 'script.interval' in error, got: {msg}"
        );
        assert!(
            msg.contains("at least 1"),
            "expected 'at least 1' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_script_unknown_field_is_structured_diagnostic() {
        let kdl = r#"modules { module "script" { command "date"; intervall 5 } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].script.intervall"));
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("interval")),
            "expected interval suggestion, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_script_duplicate_interval_is_structured_diagnostic() {
        let kdl = r#"modules { module "script" { command "date"; interval 5; interval 10 } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].script.interval"));
        assert!(diagnostic.to_string().contains("duplicate"));
        assert!(diagnostic.has_span(), "expected source span");
    }

    #[test]
    fn parse_script_interval_missing_value_is_distinct_from_wrong_type() {
        let missing =
            parse(r#"modules { module "script" { command "date"; interval } }"#).unwrap_err();
        let wrong_type =
            parse(r#"modules { module "script" { command "date"; interval "5" } }"#).unwrap_err();

        assert!(missing.to_string().contains("missing value"));
        assert!(wrong_type.to_string().contains("expected integer"));
        assert!(!wrong_type.to_string().contains("missing value"));
    }

    #[test]
    fn parse_script_shell_command_policy_error_points_at_command() {
        let kdl = r#"
command-policy {
    allow-shell #false
}
modules {
    module "script" {
        command "date"
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].script.command"));
        assert!(
            diagnostic
                .to_string()
                .contains("shell commands are disabled")
        );
        assert!(diagnostic.has_span(), "expected source span");
    }

    #[test]
    fn parse_script_format_unknown_placeholder_points_at_format() {
        let kdl = r#"modules { module "script" { command "date"; format "{missing}" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].script.format"));
        assert!(diagnostic.to_string().contains("unknown placeholder"));
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("value")),
            "expected placeholder help, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_script_tooltip_unknown_placeholder_points_at_tooltip() {
        let kdl = r#"modules { module "script" { command "date"; tooltip "{missing}" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].script.tooltip"));
        assert!(diagnostic.to_string().contains("unknown placeholder"));
        assert!(diagnostic.has_span(), "expected source span");
    }
}
