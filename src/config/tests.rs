#[cfg(test)]
mod icon_rule_tests {
    use super::super::*;

    #[test]
    fn icon_rules_select_highest_threshold_at_or_below_value() {
        let rules = IconRulesConfig {
            states: std::collections::HashMap::new(),
            thresholds: vec![
                IconThreshold {
                    at: 10.0,
                    icon: "low".into(),
                },
                IconThreshold {
                    at: 50.0,
                    icon: "mid".into(),
                },
                IconThreshold {
                    at: 90.0,
                    icon: "high".into(),
                },
            ],
        };

        assert_eq!(rules.icon_for_value(65.0), Some("mid"));
    }

    #[test]
    fn icon_rules_return_none_below_lowest_threshold() {
        let rules = IconRulesConfig {
            states: std::collections::HashMap::new(),
            thresholds: vec![IconThreshold {
                at: 10.0,
                icon: "low".into(),
            }],
        };

        assert_eq!(rules.icon_for_value(9.0), None);
    }

    #[test]
    fn icon_rules_state_takes_precedence_over_value() {
        let mut states = std::collections::HashMap::new();
        states.insert("charging".into(), "plug".into());
        let rules = IconRulesConfig {
            states,
            thresholds: vec![IconThreshold {
                at: 10.0,
                icon: "low".into(),
            }],
        };

        assert_eq!(
            rules.icon_for_state_or_value(Some("charging"), Some(15.0)),
            Some("plug")
        );
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod config_tests {
    use super::super::*;
    use iced_layershell::reexport::Anchor;

    #[test]
    fn known_module_types_include_all_config_facing_names() {
        for name in [
            "battery",
            "button",
            "clock",
            "focused-window",
            "network",
            "script",
            "tray",
            "volume",
            "workspaces",
        ] {
            assert!(
                is_known_config_module_type(name),
                "missing known module type {name:?}"
            );
        }
    }

    #[test]
    fn parse_default_config_succeeds() {
        let config = parse::parse(defaults::DEFAULT_CONFIG).unwrap();
        assert_eq!(config.bars.len(), 1);
        assert_eq!(config.bars[0].collapsed_width, 48);
        // UX-DR18: zone assignments
        assert_eq!(config.bars[0].modules.start, vec!["workspaces"]);
        assert_eq!(config.bars[0].modules.center, vec!["focused-window"]);
        assert_eq!(
            config.bars[0].modules.end,
            vec!["volume", "network", "battery", "clock"]
        );
        // All 6 default modules declared
        assert_eq!(config.modules.len(), 6);
        let names: Vec<&str> = config.modules.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"workspaces"));
        assert!(names.contains(&"focused-window"));
        assert!(names.contains(&"volume"));
        assert!(names.contains(&"network"));
        assert!(names.contains(&"battery"));
        assert!(names.contains(&"clock"));
    }

    #[test]
    fn missing_bar_section_uses_defaults() {
        // Empty input falls back to a single default bar.
        let config = parse::parse("").unwrap();
        assert_eq!(config.bars[0].collapsed_width, 48);
    }

    #[test]
    fn edge_left_gives_correct_anchor() {
        let edge = Edge::Left;
        let anchor = edge.anchor();
        assert!(anchor.contains(Anchor::Left));
        assert!(anchor.contains(Anchor::Top));
        assert!(anchor.contains(Anchor::Bottom));
    }

    #[test]
    fn edge_from_str_known_values() {
        assert!(matches!(Edge::from_str("left").unwrap(), Edge::Left));
        assert!(matches!(Edge::from_str("right").unwrap(), Edge::Right));
        assert!(matches!(Edge::from_str("top").unwrap(), Edge::Top));
        assert!(matches!(Edge::from_str("bottom").unwrap(), Edge::Bottom));
    }

    #[test]
    fn edge_from_str_unknown_returns_error() {
        assert!(Edge::from_str("center").is_err());
    }

    #[test]
    fn validate_rejects_collapsed_width_below_2() {
        for w in [0u32, 1u32] {
            let mut config = Config::default();
            config.bars[0].collapsed_width = w;
            let err = validate(&config).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("collapsed-width:"),
                "collapsed_width={w}: expected 'collapsed-width:' in message, got: {msg}"
            );
            assert!(
                msg.contains("Expected:"),
                "collapsed_width={w}: expected 'Expected:' in message, got: {msg}"
            );
        }
    }

    #[test]
    fn validate_accepts_collapsed_width_at_minimum() {
        let mut config = Config::default();
        config.bars[0].collapsed_width = 2;
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_unknown_module_name_errors() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "volumee".into(),
            module_type: "volumee".into(),
            params: ModuleParams::None,
            actions: crate::config::ModuleActionsConfig::default(),
        }];
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("modules[0].type:"),
            "expected 'modules[0].type:' prefix, got: {msg}"
        );
        assert!(
            msg.contains("volumee"),
            "expected module name in error, got: {msg}"
        );
    }

    #[test]
    fn validate_known_module_name_passes() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            module_type: "clock".into(),
            params: ModuleParams::None,
            actions: crate::config::ModuleActionsConfig::default(),
        }];
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_tray_module_type_passes() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "tray".into(),
            module_type: "tray".into(),
            params: ModuleParams::Tray(TrayConfig::default()),
            actions: crate::config::ModuleActionsConfig::default(),
        }];
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_error_includes_expected_list() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "unknown".into(),
            module_type: "unknown".into(),
            params: ModuleParams::None,
            actions: crate::config::ModuleActionsConfig::default(),
        }];
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Expected one of:"),
            "expected 'Expected one of:' in: {msg}"
        );
        assert!(
            msg.contains("clock"),
            "expected 'clock' in expected list: {msg}"
        );
    }

    #[test]
    fn validate_error_index_reflects_position() {
        let mut config = Config::default();
        config.modules = vec![
            ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::None,
                actions: crate::config::ModuleActionsConfig::default(),
            },
            ModuleConfig {
                name: "bad-module".into(),
                module_type: "bad-module".into(),
                params: ModuleParams::None,
                actions: crate::config::ModuleActionsConfig::default(),
            },
        ];
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("modules[1].type:"),
            "expected 'modules[1].type:' prefix, got: {msg}"
        );
    }

    #[test]
    fn clock_config_default_formats() {
        let cfg = ClockConfig::default();
        assert_eq!(cfg.time_format, "%H:%M");
        assert_eq!(cfg.date_format, "%a %d %b");
    }

    #[test]
    fn module_params_clock_stores_config() {
        let params = ModuleParams::Clock(ClockConfig {
            time_format: "%I:%M %p".into(),
            date_format: "%Y-%m-%d".into(),
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        });
        match params {
            ModuleParams::Clock(cfg) => {
                assert_eq!(cfg.time_format, "%I:%M %p");
                assert_eq!(cfg.date_format, "%Y-%m-%d");
            }
            _ => panic!("expected ModuleParams::Clock"),
        }
    }

    #[test]
    fn script_config_stores_command_and_interval() {
        let cfg = ScriptConfig {
            command: CommandConfig::Shell("echo hi".into()),
            interval: 10,
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        };
        assert_eq!(cfg.command, CommandConfig::Shell("echo hi".into()));
        assert_eq!(cfg.interval, 10);
    }

    #[test]
    fn module_params_script_stores_config() {
        let params = ModuleParams::Script(ScriptConfig {
            command: CommandConfig::Shell("date".into()),
            interval: 30,
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        });
        match params {
            ModuleParams::Script(cfg) => {
                assert_eq!(cfg.command, CommandConfig::Shell("date".into()));
                assert_eq!(cfg.interval, 30);
            }
            _ => panic!("expected ModuleParams::Script"),
        }
    }

    #[test]
    fn button_config_stores_fields() {
        let cfg = ButtonConfig {
            label: "Launch Terminal".into(),
            icon: ">".into(),
            on_click: CommandConfig::Shell("kitty".into()),
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        };
        assert_eq!(cfg.label, "Launch Terminal");
        assert_eq!(cfg.icon, ">");
        assert_eq!(cfg.on_click, CommandConfig::Shell("kitty".into()));
    }

    #[test]
    fn module_params_button_stores_config() {
        let params = ModuleParams::Button(ButtonConfig {
            label: "launch".into(),
            icon: ">".into(),
            on_click: CommandConfig::Shell("kitty".into()),
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        });
        match params {
            ModuleParams::Button(cfg) => {
                assert_eq!(cfg.label, "launch");
                assert_eq!(cfg.icon, ">");
                assert_eq!(cfg.on_click, CommandConfig::Shell("kitty".into()));
            }
            _ => panic!("expected ModuleParams::Button"),
        }
    }

    // --- Story 9.1: Named bar sections with output targeting ---

    #[test]
    fn bars_for_output_all_matches_any_output() {
        let config = Config::default();
        let bars = config.bars_for_output("DP-1");
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].name, "main");
    }

    #[test]
    fn bars_for_output_specific_name_matches_only_that_output() {
        let mut config = Config::default();
        config.bars = vec![
            BarConfig {
                name: "main".into(),
                ..BarConfig::default()
            },
            BarConfig {
                name: "side".into(),
                output_target: crate::config::OutputTarget::Include(vec!["DP-1".into()]),
                ..BarConfig::default()
            },
        ];
        let bars = config.bars_for_output("DP-1");
        assert_eq!(bars.len(), 2); // both "main" (all) and "side" (DP-1)
        let bars_edp = config.bars_for_output("eDP-1");
        assert_eq!(bars_edp.len(), 1); // only "main"
    }

    #[test]
    fn bars_for_output_no_all_bars_specific_output_not_connected() {
        let mut config = Config::default();
        config.bars = vec![BarConfig {
            name: "side".into(),
            output_target: crate::config::OutputTarget::Include(vec!["DP-1".into()]),
            ..BarConfig::default()
        }];
        let bars = config.bars_for_output("eDP-1");
        assert!(bars.is_empty());
    }

    #[test]
    fn bars_for_output_empty_string_matches_only_all() {
        // AllScreens mode: output_name is ""
        let config = Config::default();
        let bars = config.bars_for_output("");
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].output_target, OutputTarget::All);
    }

    #[test]
    fn validate_duplicate_bar_name_errors() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "main".into(),
                    output_target: crate::config::OutputTarget::Include(vec!["DP-1".into()]),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
            menus: vec![],
            command_policy: CommandPolicy::default(),
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("duplicate name"), "got: {err}");
    }

    #[test]
    fn validate_bar_collapsed_width_too_small_errors() {
        let config = Config {
            bars: vec![BarConfig {
                name: "x".into(),
                collapsed_width: 1,
                ..BarConfig::default()
            }],
            modules: vec![],
            menus: vec![],
            command_policy: CommandPolicy::default(),
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("collapsed-width"), "got: {err}");
    }

    fn clock_module(name: &str) -> ModuleConfig {
        ModuleConfig {
            name: name.into(),
            module_type: "clock".into(),
            params: ModuleParams::Clock(ClockConfig::default()),
            actions: ModuleActionsConfig::default(),
        }
    }

    fn script_module(name: &str) -> ModuleConfig {
        ModuleConfig {
            name: name.into(),
            module_type: "script".into(),
            params: ModuleParams::Script(ScriptConfig {
                command: CommandConfig::Argv {
                    program: "date".into(),
                    args: vec![],
                },
                interval: 60,
                format: FormatConfig::default(),
                tooltip: TooltipSetting::Default,
            }),
            actions: ModuleActionsConfig::default(),
        }
    }

    #[test]
    fn validate_duplicate_module_name_errors() {
        let config = Config {
            modules: vec![clock_module("local-clock"), script_module("local-clock")],
            ..Config::default()
        };

        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("duplicate name"), "got: {err}");
        assert!(err.to_string().contains("local-clock"), "got: {err}");
    }

    #[test]
    fn validate_reserved_module_name_must_match_kind() {
        let config = Config {
            modules: vec![script_module("clock")],
            ..Config::default()
        };

        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("reserved"), "got: {err}");
        assert!(err.to_string().contains("clock"), "got: {err}");
    }

    #[test]
    fn validate_reserved_module_name_with_matching_kind_passes() {
        let config = Config {
            modules: vec![clock_module("clock")],
            ..Config::default()
        };

        validate(&config).unwrap();
    }

    #[test]
    fn validate_unknown_module_placement_errors() {
        let mut config = Config::default();
        config.bars[0].modules.start = vec!["utc".into()];

        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("unknown module"), "got: {err}");
        assert!(err.to_string().contains("utc"), "got: {err}");
    }

    #[test]
    fn validate_non_synthesizable_builtin_placement_errors() {
        let mut config = Config::default();
        config.bars[0].modules.start = vec!["script".into()];

        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("must be declared"), "got: {err}");
        assert!(err.to_string().contains("script"), "got: {err}");
    }

    #[test]
    fn validate_duplicate_resolved_module_within_one_bar_errors() {
        let mut config = Config::default();
        config.bars[0].modules.start = vec!["clock".into()];
        config.bars[0].modules.end = vec!["clock".into()];

        let err = validate(&config).unwrap_err();
        assert!(
            err.to_string().contains("placed more than once"),
            "got: {err}"
        );
        assert!(err.to_string().contains("clock"), "got: {err}");
    }

    #[test]
    fn validate_duplicate_resolved_module_within_one_bar_panel_errors() {
        let mut config = Config::default();
        config.bars[0].panel.modules.start = vec!["clock".into()];
        config.bars[0].panel.modules.end = vec!["clock".into()];

        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("bar.main.panel"), "got: {err}");
        assert!(
            err.to_string().contains("placed more than once"),
            "got: {err}"
        );
        assert!(err.to_string().contains("clock"), "got: {err}");
    }

    #[test]
    fn validate_duplicate_menu_name_errors() {
        let config = Config {
            menus: vec![
                MenuConfig {
                    name: "main".into(),
                    show_icons: false,
                    prompt: None,
                    source: None,
                    items: vec![],
                },
                MenuConfig {
                    name: "main".into(),
                    show_icons: false,
                    prompt: None,
                    source: None,
                    items: vec![],
                },
            ],
            ..Config::default()
        };

        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("duplicate menu"), "got: {err}");
        assert!(err.to_string().contains("main"), "got: {err}");
    }

    #[test]
    fn validate_open_menu_missing_errors() {
        let config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: ModuleActionsConfig {
                    bindings: vec![ActionBinding {
                        trigger: ActionTrigger::Click,
                        action: PanelAction::OpenMenu("missing".into()),
                    }],
                },
            }],
            ..Config::default()
        };

        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("missing"), "got: {err}");
        assert!(err.to_string().contains("menu"), "got: {err}");
    }

    #[test]
    fn validate_open_menu_accepts_source_menu() {
        let mut config = Config::default();
        config.menus = vec![MenuConfig {
            name: "apps".into(),
            show_icons: false,
            prompt: None,
            source: Some(MenuSourceConfig {
                kind: MenuSourceKind::InstalledApps,
            }),
            items: vec![],
        }];
        config.bars[0].actions.bindings.push(ActionBinding {
            trigger: ActionTrigger::Click,
            action: PanelAction::OpenMenu("apps".into()),
        });

        validate(&config).unwrap();
    }

    #[test]
    fn validate_same_module_across_different_bars_passes() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "left".into(),
                    modules: BarModules {
                        start: vec!["clock".into()],
                        ..BarModules::default()
                    },
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "right".into(),
                    modules: BarModules {
                        end: vec!["clock".into()],
                        ..BarModules::default()
                    },
                    ..BarConfig::default()
                },
            ],
            ..Config::default()
        };

        validate(&config).unwrap();
    }

    // --- Additional Story 9.1 config tests ---

    #[test]
    fn bar_config_default_name_is_main() {
        assert_eq!(BarConfig::default().name, "main");
    }

    #[test]
    fn bar_config_default_output_target_is_all() {
        assert_eq!(BarConfig::default().output_target, OutputTarget::All);
    }

    #[test]
    fn config_default_has_one_bar() {
        assert_eq!(Config::default().bars.len(), 1);
    }

    #[test]
    fn config_default_bar_name_is_main() {
        assert_eq!(Config::default().bars[0].name, "main");
    }

    #[test]
    fn config_default_bar_output_target_is_all() {
        assert_eq!(Config::default().bars[0].output_target, OutputTarget::All);
    }

    #[test]
    fn bars_for_output_all_targets_any_named_output() {
        let config = Config::default();
        // "all" should match any output name
        assert_eq!(config.bars_for_output("eDP-1").len(), 1);
        assert_eq!(config.bars_for_output("HDMI-A-1").len(), 1);
        assert_eq!(config.bars_for_output("DP-2").len(), 1);
    }

    #[test]
    fn bars_for_output_multiple_all_bars_returns_all() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "a".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "b".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
            menus: vec![],
            command_policy: CommandPolicy::default(),
        };
        let bars = config.bars_for_output("eDP-1");
        assert_eq!(bars.len(), 2);
    }

    #[test]
    fn bars_for_output_mixed_returns_both_matching() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "extra".into(),
                    output_target: crate::config::OutputTarget::Include(vec!["eDP-1".into()]),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "other".into(),
                    output_target: crate::config::OutputTarget::Include(vec!["DP-1".into()]),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
            menus: vec![],
            command_policy: CommandPolicy::default(),
        };
        let bars = config.bars_for_output("eDP-1");
        assert_eq!(bars.len(), 2); // "main" (all) + "extra" (eDP-1)
        let names: Vec<&str> = bars.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"extra"));
        assert!(!names.contains(&"other"));
    }

    #[test]
    fn bars_for_output_include_target_matches_exact_case_sensitive_names() {
        let config = Config {
            bars: vec![BarConfig {
                name: "external".into(),
                output_target: OutputTarget::Include(vec!["DP-1".into(), "HDMI-A-1".into()]),
                ..BarConfig::default()
            }],
            modules: vec![],
            menus: vec![],
            command_policy: CommandPolicy::default(),
        };

        assert_eq!(config.bars_for_output("DP-1").len(), 1);
        assert_eq!(config.bars_for_output("dp-1").len(), 0);
        assert_eq!(config.bars_for_output("eDP-1").len(), 0);
    }

    #[test]
    fn bars_for_output_exclude_target_matches_exact_case_sensitive_names() {
        let config = Config {
            bars: vec![BarConfig {
                name: "not-laptop".into(),
                output_target: OutputTarget::Exclude(vec!["eDP-1".into()]),
                ..BarConfig::default()
            }],
            modules: vec![],
            menus: vec![],
            command_policy: CommandPolicy::default(),
        };

        assert_eq!(config.bars_for_output("DP-1").len(), 1);
        assert_eq!(config.bars_for_output("edp-1").len(), 1);
        assert_eq!(config.bars_for_output("eDP-1").len(), 0);
    }

    #[test]
    fn bars_for_output_preserves_order() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "first".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "second".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "third".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
            menus: vec![],
            command_policy: CommandPolicy::default(),
        };
        let bars = config.bars_for_output("DP-1");
        assert_eq!(bars[0].name, "first");
        assert_eq!(bars[1].name, "second");
        assert_eq!(bars[2].name, "third");
    }

    #[test]
    fn validate_multiple_bars_no_duplicate_names_passes() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "side".into(),
                    output_target: crate::config::OutputTarget::Include(vec!["DP-1".into()]),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
            menus: vec![],
            command_policy: CommandPolicy::default(),
        };
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_duplicate_name_error_includes_bar_name() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "side".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "side".into(),
                    output_target: crate::config::OutputTarget::Include(vec!["DP-1".into()]),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
            menus: vec![],
            command_policy: CommandPolicy::default(),
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("side"), "got: {err}");
    }

    #[test]
    fn bars_for_output_case_sensitive() {
        let config = Config {
            bars: vec![BarConfig {
                name: "main".into(),
                output_target: crate::config::OutputTarget::Include(vec!["dp-1".into()]),
                ..BarConfig::default()
            }],
            modules: vec![],
            menus: vec![],
            command_policy: CommandPolicy::default(),
        };
        // "DP-1" (uppercase) does not match "dp-1" (lowercase)
        assert!(config.bars_for_output("DP-1").is_empty());
        assert_eq!(config.bars_for_output("dp-1").len(), 1);
    }

    #[test]
    fn state_selectors_per_module() {
        use crate::config::states;
        assert_eq!(states::BATTERY, &["charging", "critical", "low", "normal"]);
        assert_eq!(states::VOLUME, &["muted", "unmuted"]);
        assert_eq!(states::NETWORK, &["full", "limited", "none"]);
        assert_eq!(states::WORKSPACES, &["focused", "active", "inactive"]);
        assert!(states::EMPTY.is_empty());
    }
}
