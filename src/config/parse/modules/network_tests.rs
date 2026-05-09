#[cfg(test)]
mod tests {
    use crate::config::parse::parse;

    fn network_config(kdl: &str) -> crate::config::NetworkConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "network")
            .expect("network module");
        match &m.params {
            crate::config::ModuleParams::Network(n) => n.clone(),
            _ => panic!("expected Network params"),
        }
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg = network_config(r#"modules { module "network" { tooltip "{label}" } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = network_config(r#"modules { module "network" { tooltip #false } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "network" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = network_config(r#"modules { module "network" {} }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }

    #[test]
    fn parse_network_unknown_field_is_structured_diagnostic() {
        let kdl = r#"modules { module "network" { tooltipp "{label}" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].network.tooltipp"));
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("tooltip")),
            "expected tooltip suggestion, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_network_format_unknown_placeholder_points_at_format() {
        let kdl = r#"modules { module "network" { format "{missing}" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].network.format"));
        assert!(diagnostic.to_string().contains("unknown placeholder"));
        assert!(diagnostic.has_span(), "expected source span");
    }

    #[test]
    fn parse_network_icon_wrong_node_points_at_icon_index() {
        let kdl = r#"modules { module "network" { icons { image "x" state="wifi" } } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].network.icons[0]"));
        assert!(diagnostic.to_string().contains("expected node name"));
        assert!(diagnostic.has_span(), "expected source span");
    }

    #[test]
    fn parse_network_icon_duplicate_state_has_span() {
        let kdl = r#"
modules {
    module "network" {
        icons {
            icon "a" state="wifi"
            icon "b" state="wifi"
        }
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].network.icons[1]"));
        assert!(diagnostic.to_string().contains("duplicate state"));
        assert!(diagnostic.has_span(), "expected source span");
    }
}
