mod tests {
    use crate::config::parse::parse;

    fn battery_config(kdl: &str) -> crate::config::BatteryConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "battery")
            .expect("battery module");
        match &m.params {
            crate::config::ModuleParams::Battery(b) => b.clone(),
            _ => panic!("expected Battery params"),
        }
    }

    #[test]
    fn states_block_overrides_builtin_critical() {
        let cfg = battery_config(
            r#"modules { module "battery" { states { state "critical" threshold=10 } } }"#,
        );
        assert_eq!(cfg.states.threshold_of("critical"), Some(10));
        assert_eq!(cfg.critical_threshold, 10);
    }

    #[test]
    fn states_block_adds_custom_state() {
        let cfg = battery_config(
            r#"modules { module "battery" { states { state "needs-charge" threshold=60 } } }"#,
        );
        assert_eq!(cfg.states.threshold_of("needs-charge"), Some(60));
        // Built-in critical retained
        assert_eq!(cfg.states.threshold_of("critical"), Some(15));
    }

    #[test]
    fn resolution_picks_smallest_threshold_above_value() {
        let cfg = battery_config(
            r#"modules { module "battery" { states { state "needs-charge" threshold=60 } } }"#,
        );
        // value 45 → matches needs-charge (60) and normal (100); smallest = 60
        assert_eq!(cfg.states.resolve(45.0), Some("needs-charge"));
        // value 10 → critical (15)
        assert_eq!(cfg.states.resolve(10.0), Some("critical"));
        // value 100 → normal (100)
        assert_eq!(cfg.states.resolve(100.0), Some("normal"));
    }

    #[test]
    fn critical_threshold_field_still_overrides_when_no_states() {
        let cfg = battery_config(r#"modules { module "battery" { critical-threshold 5 } }"#);
        assert_eq!(cfg.critical_threshold, 5);
        assert_eq!(cfg.states.threshold_of("critical"), Some(5));
    }

    #[test]
    fn states_block_wins_over_critical_threshold_field() {
        let cfg = battery_config(
            r#"modules { module "battery" { critical-threshold 5; states { state "critical" threshold=20 } } }"#,
        );
        assert_eq!(cfg.states.threshold_of("critical"), Some(20));
        assert_eq!(cfg.critical_threshold, 20);
    }

    #[test]
    fn duplicate_state_name_in_user_block_errors() {
        let kdl = r#"modules { module "battery" { states { state "low" threshold=20; state "low" threshold=25 } } }"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("duplicate state"), "got: {err}");
    }

    #[test]
    fn missing_threshold_property_errors() {
        let kdl = r#"modules { module "battery" { states { state "x" } } }"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("missing `threshold`"), "got: {err}");
    }

    #[test]
    fn empty_state_name_errors() {
        let kdl = r#"modules { module "battery" { states { state "" threshold=10 } } }"#;
        let err = parse(kdl).unwrap_err().to_string();
        assert!(err.contains("must not be empty"), "got: {err}");
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg = battery_config(r#"modules { module "battery" { tooltip "{value}%" } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = battery_config(r#"modules { module "battery" { tooltip #false } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "battery" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = battery_config(r#"modules { module "battery" {} }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }

    #[test]
    fn custom_state_usable_in_format_selector() {
        let kdl = r#"
modules {
    module "battery" {
        states { state "needs-charge" threshold=60 }
        format {
            segment "{icon}" {
                style state="needs-charge" color="warning"
            }
        }
    }
}
"#;
        // Should parse without "unknown state" error.
        battery_config(kdl);
    }

    #[test]
    fn parse_battery_unknown_field_is_structured_diagnostic() {
        let kdl = r#"modules { module "battery" { critical_threshold 5 } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(
            diagnostic.path(),
            Some("modules[0].battery.critical_threshold")
        );
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("critical-threshold")),
            "expected critical-threshold suggestion, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_battery_critical_threshold_out_of_range_has_span() {
        let kdl = r#"modules { module "battery" { critical-threshold 101 } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(
            diagnostic.path(),
            Some("modules[0].battery.critical-threshold")
        );
        assert!(diagnostic.to_string().contains("out of range"));
        assert!(diagnostic.has_span(), "expected source span");
    }

    #[test]
    fn parse_battery_duplicate_state_has_span() {
        let kdl = r#"
modules {
    module "battery" {
        states {
            state "low" threshold=20
            state "low" threshold=25
        }
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].battery.states[1]"));
        assert!(diagnostic.to_string().contains("duplicate state"));
        assert!(diagnostic.has_span(), "expected source span");
    }

    #[test]
    fn parse_battery_icon_missing_selector_has_span() {
        let kdl = r#"modules { module "battery" { icons { icon "low" } } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].battery.icons[0]"));
        assert!(diagnostic.to_string().contains("expected exactly one"));
        assert!(diagnostic.has_span(), "expected source span");
    }
}
