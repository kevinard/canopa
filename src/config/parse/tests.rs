use crate::config::parse::parse;

#[test]
fn parse_modules_section_absent() {
    let config = parse("").unwrap();
    assert!(config.modules.is_empty());
}

#[test]
fn parse_single_module() {
    let kdl = r#"modules { module "clock" }"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules.len(), 1);
    assert_eq!(config.modules[0].name, "clock");
}

#[test]
fn parse_multiple_modules_order_preserved() {
    let kdl = r#"
modules {
    module "clock"
    module "battery"
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules.len(), 2);
    assert_eq!(config.modules[0].name, "clock");
    assert_eq!(config.modules[1].name, "battery");
}

#[test]
fn parse_modules_empty_block() {
    let kdl = r#"modules { }"#;
    let config = parse(kdl).unwrap();
    assert!(config.modules.is_empty());
}

#[test]
fn parse_wrong_node_name_errors() {
    let kdl = r#"modules { widget "clock" }"#;
    let err = parse(kdl).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.starts_with("modules[0]:"),
        "expected 'modules[0]:' prefix, got: {msg}"
    );
    assert!(
        msg.contains("widget"),
        "expected wrong node name in error: {msg}"
    );
}

#[test]
fn parse_named_module_infers_type_from_name() {
    // When name matches a known type, module_type is inferred automatically.
    let kdl = r#"modules { module "clock" }"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules[0].name, "clock");
    assert_eq!(config.modules[0].module_type, "clock");
}

#[test]
fn parse_named_module_explicit_type() {
    // When name doesn't match a known type, explicit `type` child is required.
    let kdl = r#"
modules {
    module "work-clock" {
        type "clock"
        time-format "%H:%M"
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules[0].name, "work-clock");
    assert_eq!(config.modules[0].module_type, "clock");
    use crate::config::ModuleParams;
    match &config.modules[0].params {
        ModuleParams::Clock(cfg) => assert_eq!(cfg.time_format, "%H:%M"),
        _ => panic!("expected Clock params"),
    }
}

#[test]
fn parse_named_module_missing_explicit_type_errors() {
    let kdl = r#"modules { module "my-custom" }"#;
    let err = parse(kdl).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("my-custom"),
        "expected instance name in error, got: {msg}"
    );
    assert!(
        msg.contains("type"),
        "expected 'type' hint in error, got: {msg}"
    );
}

#[test]
fn parse_runtime_only_module_type_reaches_runtime_only_error() {
    let kdl = r#"
modules {
    module "tray-copy" {
        type "tray_icon"
    }
}
"#;
    let err = parse(kdl).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("runtime-only and cannot appear in config"),
        "expected runtime-only rejection, got: {msg}"
    );
    assert!(
        msg.contains("tray_icon"),
        "expected runtime-only kind in error, got: {msg}"
    );
}

#[test]
fn parse_two_instances_same_type() {
    let kdl = r#"
modules {
    module "clock"
    module "clock2" {
        type "clock"
        time-format "%I:%M %p"
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules.len(), 2);
    assert_eq!(config.modules[0].name, "clock");
    assert_eq!(config.modules[0].module_type, "clock");
    assert_eq!(config.modules[1].name, "clock2");
    assert_eq!(config.modules[1].module_type, "clock");
}

#[test]
fn parse_bar_and_modules_sections_coexist() {
    let kdl = r#"
bar "main" { }
modules {
    module "clock"
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.bars.len(), 1);
    assert_eq!(config.modules.len(), 1);
}

#[test]
fn parse_battery_icons() {
    let kdl = r#"
modules {
    module "battery" {
        icons {
            icon "low" at=10
            icon "plug" state="charging"
        }
    }
}
"#;
    let config = parse(kdl).unwrap();
    match &config.modules[0].params {
        crate::config::ModuleParams::Battery(cfg) => {
            assert_eq!(cfg.icons.thresholds[0].at, 10.0);
            assert_eq!(cfg.icons.thresholds[0].icon, "low");
            assert_eq!(
                cfg.icons.states.get("charging").map(String::as_str),
                Some("plug")
            );
        }
        other => panic!("expected Battery params, got {other:?}"),
    }
}

#[test]
fn parse_icons_rejects_missing_matcher() {
    let kdl = r#"modules { module "battery" { icons { icon "low" } } }"#;
    let err = parse(kdl).unwrap_err();
    assert!(err.to_string().contains("expected exactly one of"));
}

#[test]
fn parse_icons_rejects_both_matchers() {
    let kdl = r#"modules { module "battery" { icons { icon "low" at=10 state="charging" } } }"#;
    let err = parse(kdl).unwrap_err();
    assert!(err.to_string().contains("expected exactly one of"));
}

#[test]
fn parse_icons_rejects_duplicate_state() {
    let kdl = r#"
modules {
    module "battery" {
        icons {
            icon "plug" state="charging"
            icon "bolt" state="charging"
        }
    }
}
"#;
    let err = parse(kdl).unwrap_err();
    assert!(err.to_string().contains("duplicate state"));
}

#[test]
fn parse_icons_rejects_duplicate_threshold() {
    let kdl = r#"
modules {
    module "battery" {
        icons {
            icon "low" at=10
            icon "also-low" at=10
        }
    }
}
"#;
    let err = parse(kdl).unwrap_err();
    assert!(err.to_string().contains("duplicate threshold"));
}

#[test]
fn parse_volume_icons_preserves_step() {
    let kdl = r#"
modules {
    module "volume" {
        step 8
        icons {
            icon "muted" state="muted"
            icon "loud" at=66
        }
    }
}
"#;
    let config = parse(kdl).unwrap();
    match &config.modules[0].params {
        crate::config::ModuleParams::Volume(cfg) => {
            assert_eq!(cfg.step, 8);
            assert_eq!(cfg.icons.thresholds[0].at, 66.0);
            assert_eq!(
                cfg.icons.states.get("muted").map(String::as_str),
                Some("muted")
            );
        }
        other => panic!("expected Volume params, got {other:?}"),
    }
}

#[test]
fn parse_network_icons() {
    let kdl = r#"
modules {
    module "network" {
        icons {
            icon "ok" state="full"
            icon "off" state="none"
        }
    }
}
"#;
    let config = parse(kdl).unwrap();
    match &config.modules[0].params {
        crate::config::ModuleParams::Network(cfg) => {
            assert_eq!(cfg.icons.states.get("full").map(String::as_str), Some("ok"));
            assert_eq!(
                cfg.icons.states.get("none").map(String::as_str),
                Some("off")
            );
        }
        other => panic!("expected Network params, got {other:?}"),
    }
}

#[test]
fn parse_battery_formats() {
    let kdl = r#"
modules {
    module "battery" {
        format "{icon}" "{value}%"
        expanded-format "{icon} {state} {percent}%"
        horizontal-format "{icon}{value}"
    }
}
"#;
    let config = parse(kdl).unwrap();
    match &config.modules[0].params {
        crate::config::ModuleParams::Battery(cfg) => {
            assert!(cfg.format.collapsed.is_some());
            assert_eq!(
                cfg.format.collapsed.as_ref().unwrap().render_segments(&[
                    ("icon", "▓".to_string()),
                    ("value", "42".to_string()),
                    ("percent", "42".to_string()),
                    ("state", "discharging".to_string()),
                ]),
                vec!["▓".to_string(), "42%".to_string()]
            );
            assert!(cfg.format.expanded.is_some());
            assert!(cfg.format.horizontal.is_some());
        }
        other => panic!("expected Battery params, got {other:?}"),
    }
}

#[test]
fn parse_format_rejects_unknown_placeholder() {
    let kdl = r#"modules { module "battery" { format "{missing}" } }"#;
    let err = parse(kdl).unwrap_err();
    assert!(err.to_string().contains("unknown placeholder"));
    assert!(err.to_string().contains("battery.format"));
}

#[test]
fn parse_tray_rejects_format() {
    let kdl = r#"modules { module "tray" { format "{icon}" } }"#;
    let err = parse(kdl).unwrap_err();
    assert!(err.to_string().contains("tray.format"));
}

#[test]
fn parse_formats_for_all_non_tray_modules() {
    let kdl = r#"
modules {
    module "volume" {
        format "{icon} {value}"
        expanded-format "{percent} {state}"
        horizontal-format "{icon}"
    }
    module "network" {
        format "{icon} {label}"
        expanded-format "{value} {state}"
        horizontal-format "{icon}"
    }
    module "clock" {
        format "{icon} {time}"
        expanded-format "{date}"
        horizontal-format "{value}"
    }
    module "focused-window" {
        format "{icon}"
        expanded-format "{title}"
        horizontal-format "{value}"
    }
    module "script" {
        command "date"
        format "{icon} {output}"
        expanded-format "{value}"
        horizontal-format "{icon}"
    }
    module "button" {
        label "Launch"
        icon ">"
        on-click "kitty"
        format "{icon}"
        expanded-format "{label}"
        horizontal-format "{value}"
    }
    module "workspaces" {
        format "{icon}{label}"
        expanded-format "{state}"
        horizontal-format "{index}"
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules.len(), 7);
}
