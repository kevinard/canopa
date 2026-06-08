use crate::config::parse::parse;
use crate::config::parse::{ConfigInput, ConfigSourceName, parse_input};

#[test]
fn parse_menu_with_prompt_metadata() {
    let kdl = r#"
menus {
    menu "apps" prompt="Launch" {
        source "installed-apps"
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.menus[0].prompt.as_deref(), Some("Launch"));
}

#[test]
fn parse_modules_section_absent() {
    let config = parse("").unwrap();
    assert!(config.modules.is_empty());
}

#[test]
fn syntax_error_preserves_config_source_name() {
    let err = parse_input(ConfigInput {
        name: ConfigSourceName::test("bad-test-config.kdl"),
        text: "modules { module \"script\" { command \"date\" ",
    })
    .unwrap_err();

    assert!(err.to_string().contains("KDL"), "got: {err}");
    assert_eq!(err.source_name(), Some("bad-test-config.kdl"));
    assert!(err.as_diagnostic().is_some());
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
fn parse_reserved_module_name_rejects_different_explicit_type() {
    let kdl = r#"
modules {
    module "clock" {
        type "script"
        command "date"
    }
}
"#;
    let err = parse(kdl).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("reserved"), "got: {msg}");
    assert!(msg.contains("clock"), "got: {msg}");
    assert!(msg.contains("script"), "got: {msg}");
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
fn parse_runtime_only_module_type_is_unknown_config_type() {
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
        msg.contains("unknown module type"),
        "expected unknown type rejection, got: {msg}"
    );
    assert!(
        msg.contains("tray_icon"),
        "expected rejected type in error, got: {msg}"
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
fn parse_typed_module_actions_click_command_and_key_panel() {
    let kdl = r#"
        modules {
            module "clock" {
                actions {
                    click { command "gnome-clocks" }
                    Ctrl+E { open-panel }
                }
            }
        }
    "#;
    let config = parse(kdl).unwrap();
    let actions = &config.modules[0].actions;
    assert_eq!(
        actions.action_for(&crate::config::ActionTrigger::Click),
        Some(&crate::config::PanelAction::Command(
            crate::config::CommandConfig::Shell("gnome-clocks".into())
        ))
    );
    assert_eq!(
        actions.action_for(&crate::config::ActionTrigger::Key("Ctrl+E".into())),
        Some(&crate::config::PanelAction::OpenPanel)
    );
}

#[test]
fn parse_module_actions_empty_block_allowed() {
    let kdl = r#"modules { module "clock" { actions { } } }"#;
    let config = parse(kdl).unwrap();
    assert!(config.modules[0].actions.bindings.is_empty());
}

#[test]
fn parse_module_actions_rejects_unknown_action() {
    let kdl = r#"modules { module "clock" { actions { middle-click { command "x" } } } }"#;
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
            click { command "one" }
            click { command "two" }
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
    let kdl = r#"modules { module "clock" { actions { click { command "" } } } }"#;
    let err = parse(kdl).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("actions.click") && msg.contains("must not be empty"),
        "expected empty action command error, got: {msg}"
    );
}

#[test]
fn parse_button_requires_shared_actions_click() {
    let kdl = r#"
modules {
    module "button" {
        label "Terminal"
        icon ">"
        actions {
            click { command "kitty" }
        }
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(
        config.modules[0]
            .actions
            .action_for(&crate::config::ActionTrigger::Click),
        Some(&crate::config::PanelAction::Command(
            crate::config::CommandConfig::Shell("kitty".into())
        ))
    );
}

#[test]
fn parse_module_actions_argv_command() {
    let kdl = r#"
modules {
    module "clock" {
        actions {
            click {
                command {
                    argv "notify-send" "clock"
                }
            }
        }
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(
        config.modules[0]
            .actions
            .action_for(&crate::config::ActionTrigger::Click),
        Some(&crate::config::PanelAction::Command(
            crate::config::CommandConfig::Argv {
                program: "notify-send".into(),
                args: vec!["clock".into()],
            }
        ))
    );
}

#[test]
fn parse_action_rejects_compact_command_string() {
    let kdl = r#"modules { module "clock" { actions { click "date" } } }"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(
        err.contains("expected exactly one action child"),
        "got: {err}"
    );
}

#[test]
fn parse_action_rejects_multiple_operations() {
    let kdl = r#"modules { module "clock" { actions { click { open-panel command "date" } } } }"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(
        err.contains("expected exactly one action child"),
        "got: {err}"
    );
}

#[test]
fn parse_action_rejects_invalid_trigger() {
    let kdl = r#"modules { module "clock" { actions { clik { open-panel } } } }"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(err.contains("unknown action trigger"), "got: {err}");
}

#[test]
fn parse_menu_with_command_item() {
    let kdl = r#"
menus {
    menu "main" {
        item "Terminal" {
            command "kitty"
        }
    }
}
"#;
    let config = parse(kdl).unwrap();

    assert_eq!(config.menus.len(), 1);
    assert_eq!(config.menus[0].name, "main");
    assert_eq!(config.menus[0].items.len(), 1);
    assert_eq!(config.menus[0].items[0].label, "Terminal");
    assert_eq!(
        config.menus[0].items[0].action,
        Some(crate::config::PanelAction::Command(
            crate::config::CommandConfig::Shell("kitty".into())
        ))
    );
}

#[test]
fn parse_menu_with_entry_icons_enabled() {
    let kdl = r#"
menus {
    menu "main" show-icons=#true {
        item "Terminal" {
            icon "utilities-terminal"
            command "kitty"
        }
    }
}
"#;
    let config = parse(kdl).unwrap();

    assert!(config.menus[0].show_icons);
    assert_eq!(
        config.menus[0].items[0].icon,
        Some("utilities-terminal".into())
    );
}

#[test]
fn parse_menu_rejects_duplicate_item_icons() {
    let kdl = r#"
menus {
    menu "main" {
        item "Terminal" {
            icon "utilities-terminal"
            icon "kitty"
            command "kitty"
        }
    }
}
"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(
        err.contains("menu item can declare only one icon"),
        "got: {err}"
    );
}

#[test]
fn parse_menu_rejects_path_like_item_icons() {
    let kdl = r#"
menus {
    menu "main" {
        item "Terminal" {
            icon "../terminal"
            command "kitty"
        }
    }
}
"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(
        err.contains("icon name must be an XDG icon name"),
        "got: {err}"
    );
}

#[test]
fn parse_menu_with_installed_apps_source() {
    let kdl = r#"
menus {
    menu "apps" {
        source "installed-apps"
    }
}
"#;
    let config = parse(kdl).unwrap();

    assert_eq!(config.menus.len(), 1);
    assert_eq!(config.menus[0].name, "apps");
    assert_eq!(
        config.menus[0].source,
        Some(crate::config::MenuSourceConfig {
            kind: crate::config::MenuSourceKind::InstalledApps,
        })
    );
    assert!(config.menus[0].items.is_empty());
}

#[test]
fn parse_menu_rejects_source_and_items() {
    let kdl = r#"
menus {
    menu "apps" {
        source "installed-apps"
        item "Terminal" {
            command "kitty"
        }
    }
}
"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(
        err.contains("menu cannot mix source and item"),
        "got: {err}"
    );
}

#[test]
fn parse_menu_rejects_unknown_source() {
    let kdl = r#"
menus {
    menu "apps" {
        source "packages"
    }
}
"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(err.contains("unknown menu source"), "got: {err}");
}

#[test]
fn parse_menu_rejects_source_options_for_now() {
    let kdl = r#"
menus {
    menu "apps" {
        source "installed-apps" show-icons=#true
    }
}
"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(
        err.contains("source options are not supported"),
        "got: {err}"
    );
}

#[test]
fn parse_nested_menu_items() {
    let kdl = r#"
menus {
    menu "main" {
        item "Apps" {
            item "Browser" {
                keywords "web" "internet"
                command "firefox"
            }
        }
    }
}
"#;
    let config = parse(kdl).unwrap();

    let apps = &config.menus[0].items[0];
    assert_eq!(apps.label, "Apps");
    assert_eq!(apps.children.len(), 1);
    assert_eq!(apps.children[0].label, "Browser");
    assert_eq!(apps.children[0].keywords, vec!["web", "internet"]);
    assert_eq!(
        apps.children[0].action,
        Some(crate::config::PanelAction::Command(
            crate::config::CommandConfig::Shell("firefox".into())
        ))
    );
}

#[test]
fn parse_command_policy_rejects_shell_commands_when_disabled() {
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
    let msg = err.to_string();
    assert!(
        msg.contains("allow-shell #false") && msg.contains("argv"),
        "expected shell policy rejection, got: {msg}"
    );
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
            use crate::config::style::{Style, ThemeToken, Weight};
            let rendered = cfg
                .format
                .collapsed
                .as_ref()
                .unwrap()
                .render_styled_segments_with_default(
                    &[
                        ("icon", "i".into()),
                        ("value", "42".into()),
                        ("percent", "42".into()),
                        ("state", "normal".into()),
                    ],
                    None,
                    &Style::default(),
                );
            assert_eq!(rendered[0].1.color, Some(ThemeToken::Accent.resolve()));
            assert_eq!(rendered[0].1.size, Some(18.0));
            assert_eq!(rendered[0].1.weight, Some(Weight::Bold));
            assert_eq!(rendered[1].1, crate::config::ResolvedStyle::default());
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
            use crate::config::style::{Style, ThemeToken};
            let template = cfg.format.collapsed.as_ref().unwrap();
            let values = [
                ("icon", "i".into()),
                ("value", "42".into()),
                ("percent", "42".into()),
                ("state", "critical".into()),
            ];
            let default = Style::default();
            let normal = template.render_styled_segments_with_default(&values, None, &default);
            let critical =
                template.render_styled_segments_with_default(&values, Some("critical"), &default);
            let charging =
                template.render_styled_segments_with_default(&values, Some("charging"), &default);

            assert_eq!(normal[0].1.color, Some(ThemeToken::Accent.resolve()));
            assert_eq!(critical[0].1.color, Some(ThemeToken::Critical.resolve()));
            assert_eq!(charging[0].1.color, Some(ThemeToken::Success.resolve()));
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
fn segment_unknown_state_is_structured_diagnostic() {
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
    let diagnostic = err.as_diagnostic().expect("config diagnostic");
    assert_eq!(
        diagnostic.path(),
        Some("modules[0].battery.format.segment[0].style[0].state")
    );
    assert!(diagnostic.has_span(), "expected source span");
    assert!(
        diagnostic
            .help_text()
            .is_some_and(|help| help.contains("critical")),
        "expected state help, got: {diagnostic:?}"
    );
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
fn bar_style_bad_color_is_structured_diagnostic() {
    let err = parse(
        r##"
        bar "main" {
            style color="#nope"
        }
    "##,
    )
    .unwrap_err();
    let diagnostic = err.as_diagnostic().expect("config diagnostic");
    assert_eq!(diagnostic.path(), Some("bar[0].style.color"));
    assert!(diagnostic.has_span(), "expected source span");
    assert!(diagnostic.to_string().contains("color"));
}

#[test]
fn parse_formats_for_all_non_tray_modules() {
    let kdl = r#"
modules {
    module "volume" {
        format "{icon} {value}"
        expanded-format "{percent} {state}"
        horizontal-format "{icon}"
        content-rotation "clockwise"
    }
    module "network" {
        format "{icon} {label}"
        expanded-format "{value} {state}"
        horizontal-format "{icon}"
        content-rotation "clockwise"
    }
    module "clock" {
        format "{icon} {time}"
        expanded-format "{date}"
        horizontal-format "{value}"
        content-rotation "clockwise"
    }
    module "focused-window" {
        format "{icon}"
        expanded-format "{title}"
        horizontal-format "{value}"
        content-rotation "clockwise"
    }
    module "script" {
        command "date"
        format "{icon} {output}"
        expanded-format "{value}"
        horizontal-format "{icon}"
        content-rotation "clockwise"
    }
    module "button" {
        label "Launch"
        icon ">"
        actions {
            click { command "kitty" }
        }
        format "{icon}"
        expanded-format "{label}"
        horizontal-format "{value}"
        content-rotation "clockwise"
    }
    module "workspaces" {
        format "{icon}{label}"
        expanded-format "{state}"
        horizontal-format "{index}"
        content-rotation "clockwise"
    }
}
"#;
    let config = parse(kdl).unwrap();
    assert_eq!(config.modules.len(), 7);
    for module in &config.modules {
        let rotation = match &module.params {
            crate::config::ModuleParams::Volume(config) => config.content_rotation,
            crate::config::ModuleParams::Network(config) => config.content_rotation,
            crate::config::ModuleParams::Clock(config) => config.content_rotation,
            crate::config::ModuleParams::FocusedWindow(config) => config.content_rotation,
            crate::config::ModuleParams::Script(config) => config.content_rotation,
            crate::config::ModuleParams::Button(config) => config.content_rotation,
            crate::config::ModuleParams::Workspaces(config) => config.content_rotation,
            other => panic!("unexpected module params: {other:?}"),
        };
        assert_eq!(rotation, crate::config::ContentRotation::Clockwise);
    }
}
