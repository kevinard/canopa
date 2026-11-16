use crate::config::parse::parse;
use crate::config::parse::{ConfigInput, ConfigSourceName, parse_input};

#[test]
fn idle_inhibitor_supports_implicit_placement_and_named_customization() {
    let config = parse(
        r#"
        bar "main" { modules { start "idle-inhibitor" "awake"; }; }
        modules { module "awake" {
            type "idle-inhibitor"
            icons { icon "ON" state="active"; icon "OFF" state="inactive"; }
            format "{icon}" "{value}"
            horizontal-format "{state}"
            expanded-format "{icon}" "{label}"
            tooltip "{label}"
            content-rotation "auto"
        }; }
    "#,
    )
    .unwrap();
    assert_eq!(config.modules[0].module_type, "idle-inhibitor");
    let crate::config::ModuleParams::IdleInhibitor(params) = &config.modules[0].params else {
        panic!("wrong module params")
    };
    assert_eq!(params.icons.icon_for_state("active"), Some("ON"));
    assert!(
        parse(r#"modules { module "awake" { type "idle-inhibitor"; format "{unknown}"; }; }"#)
            .is_err()
    );
    assert!(
        parse(r#"modules { module "awake" { type "idle-inhibitor"; enabled #true; }; }"#).is_err()
    );
}

#[test]
fn canopa_panel_provider_selects_explicit_transport() {
    let config = parse(
        r#"
        panel-providers {
            provider "notifications" {
                protocol "canopa-v4"
                command { argv "canopa-notify" "panel-provider"; }
            }
        }
    "#,
    )
    .expect("Canopa protocol should be selectable without changing existing providers");
    assert_eq!(
        config.panel_providers[0].protocol,
        crate::external_panel::ProviderProtocol::CanopaV4
    );
    assert!(
        parse(r#"panel-providers { provider "bad" { protocol "canopa-v99"; command "true"; }; }"#)
            .is_err()
    );
}

#[test]
fn parse_panel_provider_and_open_provider_action() {
    let kdl = r#"
panel-providers {
    provider "wifi" {
        command {
            argv "canopa-wifi-provider" "--scan"
        }
    }
}
modules {
    module "network" {
        actions {
            click { open-provider "wifi" }
        }
    }
}
"#;
    let config = parse(kdl).unwrap();

    assert_eq!(config.panel_providers.len(), 1);
    assert_eq!(config.panel_providers[0].name, "wifi");
    assert_eq!(
        config.panel_providers[0].command,
        crate::config::ConfiguredCommand::argv_for_test(
            "canopa-wifi-provider",
            vec!["--scan".into()]
        )
    );
    assert_eq!(
        config.modules[0]
            .actions
            .action_for(&crate::config::ActionTrigger::Click),
        Some(&crate::config::PanelAction::OpenProvider("wifi".into()))
    );
}

#[test]
fn parse_panel_provider_rejects_shell_command_when_policy_disables_shell() {
    let kdl = r#"
command-policy {
    allow-shell #false
}
panel-providers {
    provider "wifi" {
        command "canopa-wifi-provider"
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
        ModuleParams::Clock(cfg) => assert_eq!(cfg.time_format.as_deref(), Some("%H:%M")),
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
            crate::config::ConfiguredCommand::shell_for_test("gnome-clocks")
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
            crate::config::ConfiguredCommand::shell_for_test("kitty")
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
            crate::config::ConfiguredCommand::argv_for_test("notify-send", vec!["clock".into()])
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
            crate::config::ConfiguredCommand::shell_for_test("kitty")
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
            crate::config::ConfiguredCommand::shell_for_test("firefox")
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
fn parse_command_policy_rejects_bar_action_shell_commands_when_disabled() {
    let kdl = r#"
command-policy {
    allow-shell #false
}
bar "main" {
    actions {
        click { command "date" }
    }
}
"#;
    let err = parse(kdl).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("allow-shell #false") && msg.contains("argv"),
        "expected bar action shell policy rejection, got: {msg}"
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
#[test]
fn application_launch_is_argv_only_and_independent_of_command_timeout() {
    let config = parse(
        r#"
        command-policy { allow-shell #false; action-timeout-ms 10; }
        menus { menu "apps" { item "App" { launch "app" "two words" "$(literal)"; }; }; }
        modules { module "clock" { actions { click { launch "app" ""; }; }; }; }
    "#,
    )
    .unwrap();
    assert_eq!(
        config.menus[0].items[0].action,
        Some(crate::config::PanelAction::Launch(vec![
            "app".into(),
            "two words".into(),
            "$(literal)".into()
        ]))
    );
    assert_eq!(
        config.modules[0]
            .actions
            .action_for(&crate::config::ActionTrigger::Click),
        Some(&crate::config::PanelAction::Launch(vec![
            "app".into(),
            "".into()
        ]))
    );
    for operation in [
        r#"launch """#,
        r#"launch program="app""#,
        "launch 1",
        r#"launch "app" { argv "x"; }"#,
    ] {
        assert!(
            parse(&format!(
                "modules {{ module \"clock\" {{ actions {{ click {{ {operation}; }}; }}; }}; }}"
            ))
            .is_err()
        );
    }
}

#[test]
fn v5_provider_selection_is_explicit_and_legacy_default_is_preserved() {
    let input = r#"panel-providers {
        provider "next" { protocol "canopa-v5"; command { argv "example" "--protocol" "canopa-v5"; }; }
        provider "legacy" { command { argv "example"; }; }
    }"#;
    let config = parse(input).unwrap();
    assert_eq!(
        config.panel_providers[0].protocol,
        crate::external_panel::ProviderProtocol::CanopaV5
    );
    assert_eq!(
        config.panel_providers[1].protocol,
        crate::external_panel::ProviderProtocol::NiribarV1
    );
}

#[test]
fn osd_block_sets_key_steps_and_display_timeout_within_bounds() {
    let defaults = parse("").unwrap().osd;
    assert_eq!(defaults, crate::config::OsdConfig::default());
    assert!(defaults.enabled);
    let config =
        parse("osd { enabled #false; volume-step 10; brightness-step 8; timeout-ms 2500; }")
            .unwrap();
    assert_eq!(
        config.osd,
        crate::config::OsdConfig {
            enabled: false,
            volume_step: 10,
            brightness_step: 8,
            timeout_ms: 2500,
        }
    );
    assert_eq!(config.osd.steps().volume, 10);
    for refused in [
        "osd { volume-step 0; }",
        "osd { volume-step 101; }",
        "osd { brightness-step 51; }",
        "osd { timeout-ms 100; }",
        "osd { timeout-ms 20000; }",
        "osd { unknown 1; }",
        "osd { volume-step 5; volume-step 6; }",
        "osd {}\nosd {}",
    ] {
        assert!(parse(refused).is_err(), "{refused}");
    }
}

#[test]
fn message_ids_give_built_in_texts_and_literals_stay_literal() {
    use crate::config::{ModuleParams, TooltipSetting};
    let _english = crate::i18n_testing::english();
    let config = parse(
        r#"
menus {
    menu "Reboot?" title-id="shell-menu-reboot-title" prompt-id="shell-menu-unsaved-work" {
        item label-id="shell-menu-cancel" { open-bar-panel; }
        item "Restart now" { session "reboot"; }
    }
}
modules {
    module "session-lock" {
        type "button"
        label-id "shell-module-lock"
        icon "canopa:lock"
        tooltip-id "shell-tooltip-launcher"
        actions { click { launch "canopa-lock"; } }
    }
    module "mine" {
        type "button"
        label "My lock"
        icon "canopa:lock"
        actions { click { launch "canopa-lock"; } }
    }
}
"#,
    )
    .unwrap();
    let menu = &config.menus[0];
    assert_eq!(menu.name, "Reboot?");
    assert_eq!(menu.title.as_deref(), Some("Reboot?"));
    assert_eq!(menu.prompt.as_deref(), Some("Unsaved work may be lost."));
    assert_eq!(menu.items[0].label, "Cancel");
    assert_eq!(menu.items[1].label, "Restart now");
    let ModuleParams::Button(lock) = &config.modules[0].params else {
        panic!("button");
    };
    assert_eq!(lock.label, "Lock");
    assert!(matches!(lock.tooltip, TooltipSetting::Custom(_)));
    let ModuleParams::Button(mine) = &config.modules[1].params else {
        panic!("button");
    };
    assert_eq!(mine.label, "My lock");

    for refused in [
        r#"modules { module "b" { type "button"; label "A"; label-id "shell-module-lock"; icon "x"; actions { click { launch "a"; }; }; }; }"#,
        r#"modules { module "b" { type "button"; label-id "shell-no-such-text"; icon "x"; actions { click { launch "a"; }; }; }; }"#,
        r#"menus { menu "m" prompt="A" prompt-id="shell-menu-unsaved-work" { item "x" { open-bar-panel; }; }; }"#,
        r#"menus { menu "m" { item "x" label-id="shell-menu-cancel" { open-bar-panel; }; }; }"#,
    ] {
        assert!(parse(refused).is_err(), "{refused}");
    }
}

#[test]
fn shipped_defaults_follow_the_language() {
    let _french = crate::i18n_testing::select("fr", false);
    let (config, _) =
        crate::config::load(&canopa_config::Request::Defaults, false).expect("defaults load");
    assert_eq!(
        config.bars[0].panel.title.as_deref(),
        Some("Réglages rapides")
    );
    let logout = config
        .menus
        .iter()
        .find(|menu| menu.name == "Log out?")
        .unwrap();
    assert_eq!(logout.title.as_deref(), Some("Se déconnecter\u{a0}?"));
    assert_eq!(logout.items[0].label, "Annuler");
}

#[test]
fn windows_restore_is_opt_in() {
    assert!(!super::parse("").unwrap().windows.restore);
    assert!(
        super::parse("windows { restore #true; }")
            .unwrap()
            .windows
            .restore
    );
    assert!(super::parse("windows { restore #true; keep 1; }").is_err());
}

#[test]
fn templates_list_apps_with_optional_widths() {
    let config = super::parse(
        r#"templates {
            template "dev" {
                app "code" width=66.667
                app "foot.desktop" width=33
            }
            template "chat" { app "signal.desktop"; }
        }"#,
    )
    .unwrap();
    assert_eq!(config.templates.len(), 2);
    let dev = &config.templates[0];
    assert_eq!(dev.name, "dev");
    assert_eq!(dev.apps[0].desktop_id, "code.desktop");
    assert_eq!(dev.apps[0].width_percent, Some(66.667));
    assert_eq!(dev.apps[1].width_percent, Some(33.0));
    assert_eq!(config.templates[1].apps[0].width_percent, None);

    for refused in [
        r#"templates { template "a" { app "x" width=2; } }"#,
        r#"templates { template "a" { } }"#,
        r#"templates { template "a" { app "x"; } template "a" { app "y"; } }"#,
        r#"templates { layout "a" { app "x"; } }"#,
    ] {
        assert!(super::parse(refused).is_err(), "{refused}");
    }
}

#[test]
fn compositor_actions_use_the_shell_niri_socket() {
    let config = parse(
        r#"
        menus {
            menu "Bye" { item "Log out" { session "log-out"; }; }
        }
        modules { module "ov" { type "button"; label "Overview"; icon "view-grid"; actions { click { toggle-overview; } } } }
    "#,
    )
    .unwrap();
    assert_eq!(
        config.menus[0].items[0].action,
        Some(crate::config::PanelAction::Compositor(
            crate::config::CompositorAction::LogOut
        ))
    );
    assert!(matches!(
        crate::config::CompositorAction::LogOut.niri_action(),
        niri_ipc::Action::Quit {
            skip_confirmation: true
        }
    ));
}

/// The shipped configuration ends the session and opens the overview through
/// the compositor socket; it never runs a `niri` executable from `PATH`.
#[test]
fn shipped_configuration_never_runs_the_niri_cli() {
    let defaults = include_str!("../../../config/default.kdl");
    for line in defaults
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
    {
        assert!(!line.contains("argv \"niri\""), "{line}");
        assert!(!line.contains("\"niri msg"), "{line}");
    }
}
