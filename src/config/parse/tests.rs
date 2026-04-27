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
fn parse_module_actions_click_and_right_click() {
    let kdl = r#"
modules {
    module "clock" {
        actions {
            click "gnome-clocks"
            right-click "notify-send clock"
        }
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(
        config.modules[0].actions.click.as_deref(),
        Some("gnome-clocks")
    );
    assert_eq!(
        config.modules[0].actions.right_click.as_deref(),
        Some("notify-send clock")
    );
}

#[test]
fn parse_module_actions_empty_block_allowed() {
    let kdl = r#"modules { module "clock" { actions { } } }"#;
    let config = parse(kdl).unwrap();
    assert!(config.modules[0].actions.click.is_none());
    assert!(config.modules[0].actions.right_click.is_none());
}

#[test]
fn parse_module_actions_rejects_unknown_action() {
    let kdl = r#"modules { module "clock" { actions { middle-click "x" } } }"#;
    let err = parse(kdl).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("actions[0]") && msg.contains("middle-click"),
        "expected unknown action error, got: {msg}"
    );
}

#[test]
fn parse_module_actions_rejects_duplicate_click() {
    let kdl = r#"
modules {
    module "clock" {
        actions {
            click "one"
            click "two"
        }
    }
}
"#;
    let err = parse(kdl).unwrap_err();
    assert!(
        err.to_string().contains("duplicate click"),
        "expected duplicate click error, got: {err}"
    );
}

#[test]
fn parse_module_actions_rejects_empty_command() {
    let kdl = r#"modules { module "clock" { actions { click "" } } }"#;
    let err = parse(kdl).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("actions.click") && msg.contains("must not be empty"),
        "expected empty action command error, got: {msg}"
    );
}

#[test]
fn parse_button_allows_actions_click_without_legacy_on_click() {
    let kdl = r#"
modules {
    module "button" {
        label "Terminal"
        icon ">"
        actions {
            click "kitty"
        }
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules[0].actions.click.as_deref(), Some("kitty"));
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
fn battery_format_accepts_child_node_form() {
    let kdl = r#"
modules {
    module "battery" {
        format {
            segment "{icon}"
            segment "{value}%"
        }
    }
}
"#;
    let config = parse(kdl).unwrap();
    match &config.modules[0].params {
        crate::config::ModuleParams::Battery(cfg) => {
            assert!(cfg.format.collapsed.is_some());
            assert_eq!(
                cfg.format
                    .collapsed
                    .as_ref()
                    .unwrap()
                    .render_segments(&[("icon", "B".to_string()), ("value", "42".to_string()),]),
                vec!["B".to_string(), "42%".to_string()]
            );
        }
        other => panic!("expected Battery params, got {other:?}"),
    }
}

#[test]
fn battery_format_mixed_errors() {
    let kdl = r#"
modules {
    module "battery" {
        format "{icon}" {
            segment "{value}%"
        }
    }
}
"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(
        err.contains("mutually exclusive"),
        "expected 'mutually exclusive' in error, got: {err}"
    );
}

#[test]
fn segment_inline_style_props() {
    let config = parse(
        r#"
        modules {
            module "battery" {
                format {
                    segment "{icon}" color="accent" size=18 weight="bold"
                    segment "{value}%"
                }
            }
        }
    "#,
    )
    .unwrap();
    match &config.modules[0].params {
        crate::config::ModuleParams::Battery(cfg) => {
            let segs = cfg.format.collapsed.as_ref().unwrap().segments();
            use crate::config::style::{ColorValue, SegmentStyles, ThemeToken, Weight};
            match segs[0].styles() {
                SegmentStyles::Static(s) => {
                    assert_eq!(s.color, Some(ColorValue::Token(ThemeToken::Accent)));
                    assert_eq!(s.size, Some(18.0));
                    assert_eq!(s.weight, Some(Weight::Bold));
                }
                other => panic!("expected Static, got {other:?}"),
            }
            matches!(segs[1].styles(), SegmentStyles::None);
        }
        other => panic!("expected Battery params, got {other:?}"),
    }
}

#[test]
fn segment_stateful_style_children() {
    let config = parse(
        r#"
        modules {
            module "battery" {
                format {
                    segment "{icon}" {
                        style color="accent"
                        style state="critical" color="critical"
                        style state="charging" color="success"
                    }
                }
            }
        }
    "#,
    )
    .unwrap();
    match &config.modules[0].params {
        crate::config::ModuleParams::Battery(cfg) => {
            let segs = cfg.format.collapsed.as_ref().unwrap().segments();
            use crate::config::style::SegmentStyles;
            match segs[0].styles() {
                SegmentStyles::Stateful { default, by_state } => {
                    assert!(default.is_some());
                    assert_eq!(by_state.len(), 2);
                    assert_eq!(by_state[0].0, "critical");
                    assert_eq!(by_state[1].0, "charging");
                }
                other => panic!("expected Stateful, got {other:?}"),
            }
        }
        other => panic!("expected Battery params, got {other:?}"),
    }
}

#[test]
fn segment_inline_props_and_style_children_is_error() {
    let err = parse(
        r#"
        modules {
            module "battery" {
                format {
                    segment "{icon}" color="accent" {
                        style state="critical" color="critical"
                    }
                }
            }
        }
    "#,
    )
    .unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("inline") || msg.contains("style children"),
        "got: {msg}"
    );
}

#[test]
fn segment_two_default_styles_is_error() {
    let err = parse(
        r#"
        modules {
            module "battery" {
                format {
                    segment "{icon}" {
                        style color="accent"
                        style color="text"
                    }
                }
            }
        }
    "#,
    )
    .unwrap_err();
    assert!(format!("{err:#}").contains("default"));
}

#[test]
fn segment_duplicate_state_is_error() {
    let err = parse(
        r#"
        modules {
            module "battery" {
                format {
                    segment "{icon}" {
                        style state="critical" color="critical"
                        style state="critical" color="warning"
                    }
                }
            }
        }
    "#,
    )
    .unwrap_err();
    assert!(format!("{err:#}").contains("critical"));
}

#[test]
fn segment_unknown_state_is_error() {
    let err = parse(
        r#"
        modules {
            module "battery" {
                format {
                    segment "{icon}" {
                        style state="bogus" color="critical"
                    }
                }
            }
        }
    "#,
    )
    .unwrap_err();
    assert!(format!("{err:#}").contains("bogus"));
}

#[test]
fn segment_state_on_stateless_module_is_error() {
    let err = parse(
        r#"
        modules {
            module "clock" {
                format {
                    segment "{time}" {
                        style state="critical" color="critical"
                    }
                }
            }
        }
    "#,
    )
    .unwrap_err();
    assert!(format!("{err:#}").contains("state"));
}

#[test]
fn bar_style_parses_shared_module_segment_defaults() {
    let config = parse(
        r##"
        bar "main" {
            style color="#112233" size=13 weight="light"
        }
    "##,
    )
    .unwrap();

    use crate::config::style::{ColorValue, Weight};
    assert_eq!(
        config.bars[0].style.color,
        Some(ColorValue::Rgba(iced::Color::from_rgb8(0x11, 0x22, 0x33)))
    );
    assert_eq!(config.bars[0].style.size, Some(13.0));
    assert_eq!(config.bars[0].style.weight, Some(Weight::Light));
}

#[test]
fn bar_style_rejects_state_selector() {
    let err = parse(
        r#"
        bar "main" {
            style state="critical" color="critical"
        }
    "#,
    )
    .unwrap_err();

    assert!(format!("{err:#}").contains("state"));
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
