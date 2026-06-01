mod tests {
    use crate::config::ModuleParams;
    use crate::config::parse::parse;

    fn workspaces_config(kdl: &str) -> crate::config::WorkspacesConfig {
        let cfg = parse(kdl).unwrap();
        let m = cfg
            .modules
            .iter()
            .find(|m| m.name == "workspaces")
            .expect("workspaces module");
        match &m.params {
            ModuleParams::Workspaces(w) => w.clone(),
            _ => panic!("expected Workspaces params"),
        }
    }

    #[test]
    fn tooltip_string_parsed() {
        let cfg = workspaces_config(r#"modules { module "workspaces" { tooltip "{label}" } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Custom(_)
        ));
    }

    #[test]
    fn tooltip_false_disables() {
        let cfg = workspaces_config(r#"modules { module "workspaces" { tooltip #false } }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Disabled
        ));
    }

    #[test]
    fn tooltip_true_errors() {
        let kdl = r#"modules { module "workspaces" { tooltip #true } }"#;
        let err = crate::config::parse::parse(kdl).unwrap_err().to_string();
        assert!(err.contains("tooltip true"), "got: {err}");
    }

    #[test]
    fn tooltip_absent_is_default() {
        let cfg = workspaces_config(r#"modules { module "workspaces" {} }"#);
        assert!(matches!(
            cfg.tooltip,
            crate::config::TooltipSetting::Default
        ));
    }

    #[test]
    fn parse_workspaces_with_label_and_icon() {
        let kdl = r#"
modules {
    module "workspaces" {
        workspace 1 {
            label "web"
            icon ">"
        }
    }
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.modules.len(), 1);
        match &config.modules[0].params {
            ModuleParams::Workspaces(cfg) => {
                let ws1 = cfg
                    .workspaces
                    .get(&1)
                    .expect("workspace 1 should be configured");
                assert_eq!(ws1.label.as_deref(), Some("web"));
                assert_eq!(ws1.icon.as_deref(), Some(">"));
            }
            _ => panic!("expected ModuleParams::Workspaces"),
        }
    }

    #[test]
    fn parse_workspaces_no_children_is_empty() {
        let kdl = r#"modules { module "workspaces" }"#;
        let config = parse(kdl).unwrap();
        match &config.modules[0].params {
            ModuleParams::Workspaces(cfg) => {
                assert!(cfg.workspaces.is_empty());
            }
            _ => panic!("expected ModuleParams::Workspaces"),
        }
    }

    #[test]
    fn parse_workspaces_label_only() {
        let kdl = r#"
modules {
    module "workspaces" {
        workspace 2 {
            label "code"
        }
    }
}
"#;
        let config = parse(kdl).unwrap();
        match &config.modules[0].params {
            ModuleParams::Workspaces(cfg) => {
                let ws2 = cfg
                    .workspaces
                    .get(&2)
                    .expect("workspace 2 should be configured");
                assert_eq!(ws2.label.as_deref(), Some("code"));
                assert!(ws2.icon.is_none());
            }
            _ => panic!("expected ModuleParams::Workspaces"),
        }
    }

    #[test]
    fn parse_workspaces_icon_only() {
        let kdl = r#"
modules {
    module "workspaces" {
        workspace 3 {
            icon ">"
        }
    }
}
"#;
        let config = parse(kdl).unwrap();
        match &config.modules[0].params {
            ModuleParams::Workspaces(cfg) => {
                let ws3 = cfg
                    .workspaces
                    .get(&3)
                    .expect("workspace 3 should be configured");
                assert!(ws3.label.is_none());
                assert_eq!(ws3.icon.as_deref(), Some(">"));
            }
            _ => panic!("expected ModuleParams::Workspaces"),
        }
    }

    #[test]
    fn parse_workspaces_multiple_workspaces() {
        let kdl = r#"
modules {
    module "workspaces" {
        workspace 1 { label "web" }
        workspace 3 { label "media" icon ">" }
    }
}
"#;
        let config = parse(kdl).unwrap();
        match &config.modules[0].params {
            ModuleParams::Workspaces(cfg) => {
                assert_eq!(cfg.workspaces.len(), 2);
                assert!(cfg.workspaces.contains_key(&1));
                assert!(!cfg.workspaces.contains_key(&2));
                assert!(cfg.workspaces.contains_key(&3));
            }
            _ => panic!("expected ModuleParams::Workspaces"),
        }
    }

    #[test]
    fn parse_workspaces_zero_workspace_number_errors() {
        let kdl = r#"
modules {
    module "workspaces" {
        workspace 0 { label "bad" }
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("workspace number"),
            "expected 'workspace number' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_workspaces_unknown_field_is_structured_diagnostic() {
        let kdl = r#"modules { module "workspaces" { workspacce 1 } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].workspaces.workspacce"));
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("workspace")),
            "expected workspace suggestion, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_workspaces_invalid_number_points_at_workspace_index() {
        let kdl = r#"modules { module "workspaces" { workspace 0 { label "bad" } } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(
            diagnostic.path(),
            Some("modules[0].workspaces.workspace[0]")
        );
        assert!(diagnostic.to_string().contains("workspace number"));
        assert!(diagnostic.has_span(), "expected source span");
    }

    #[test]
    fn parse_workspaces_nested_unknown_field_has_span() {
        let kdl = r#"modules { module "workspaces" { workspace 1 { lable "web" } } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(
            diagnostic.path(),
            Some("modules[0].workspaces.workspace[0].lable")
        );
        assert!(diagnostic.has_span(), "expected source span");
        assert!(
            diagnostic
                .help_text()
                .is_some_and(|help| help.contains("label")),
            "expected label suggestion, got: {diagnostic:?}"
        );
    }

    #[test]
    fn parse_workspaces_format_unknown_placeholder_points_at_format() {
        let kdl = r#"modules { module "workspaces" { format "{missing}" } }"#;
        let err = parse(kdl).unwrap_err();
        let diagnostic = err.as_diagnostic().expect("config diagnostic");
        assert_eq!(diagnostic.path(), Some("modules[0].workspaces.format"));
        assert!(diagnostic.to_string().contains("unknown placeholder"));
        assert!(diagnostic.has_span(), "expected source span");
    }
}
