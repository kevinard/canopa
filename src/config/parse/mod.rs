mod error;

use ::kdl::{KdlDocument, KdlNode};

mod kdl;
mod modules;

use super::{
    AnimationConfig, BarConfig, BarModules, Config, Edge, ExpandTrigger, ModuleConfig,
    ModuleParams,
};

/// Parse a KDL string into a fully-typed `Config`.
///
/// Missing sections fall back to defaults; constraint validation happens in `validate()`.
pub fn parse(input: &str) -> anyhow::Result<Config> {
    let doc: KdlDocument = input.parse().map_err(|e| anyhow::anyhow!("config: {e}"))?;
    let bars = parse_bar_sections(&doc)?;
    let bars = if bars.is_empty() {
        vec![BarConfig::default()]
    } else {
        bars
    };
    let modules = parse_modules_section(&doc)?;
    Ok(Config { bars, modules })
}

fn parse_bar_sections(doc: &KdlDocument) -> anyhow::Result<Vec<BarConfig>> {
    let mut result = Vec::new();
    for node in doc.nodes() {
        if node.name().value() != "bar" {
            continue;
        }
        let name = node
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "bar: missing name argument. \
                     Expected: bar \"<name>\" {{ ... }}, e.g. bar \"main\" {{ ... }}"
                )
            })?
            .to_string();
        let bar = parse_single_bar_node(node, &name)?;
        result.push(bar);
    }
    Ok(result)
}

fn parse_single_bar_node(node: &KdlNode, name: &str) -> anyhow::Result<BarConfig> {
    let prefix = format!("bar {name:?}");
    let children = match node.children() {
        Some(c) => c,
        None => {
            return Ok(BarConfig {
                name: name.to_string(),
                ..BarConfig::default()
            });
        }
    };

    let defaults = BarConfig::default();

    let output = children
        .get_arg("output")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string())
        .unwrap_or_else(|| defaults.output.clone());

    let edge = match children.get_arg("edge") {
        None => defaults.edge,
        Some(v) => {
            let s = v.as_string().ok_or_else(|| {
                anyhow::anyhow!(
                    "{prefix}.edge: expected a string value. Expected: one of: left, right, top, bottom."
                )
            })?;
            Edge::from_str(s)?
        }
    };

    let collapsed_width = match children.get_arg("collapsed-width") {
        None => defaults.collapsed_width,
        Some(v) => {
            let n = v.as_integer().ok_or_else(|| {
                anyhow::anyhow!(
                    "{prefix}.collapsed-width: expected a positive integer. \
                     Expected: a width in logical pixels, e.g. 48."
                )
            })?;
            u32::try_from(n).map_err(|_| {
                anyhow::anyhow!(
                    "{prefix}.collapsed-width: expected a positive integer. \
                     Expected: a width in logical pixels, e.g. 48."
                )
            })?
        }
    };

    let expanded_width = match children.get_arg("expanded-width") {
        None => defaults.expanded_width,
        Some(v) => {
            let n = v.as_integer().ok_or_else(|| {
                anyhow::anyhow!(
                    "{prefix}.expanded-width: expected a positive integer. \
                     Expected: a width in logical pixels, e.g. 200."
                )
            })?;
            u32::try_from(n).map_err(|_| {
                anyhow::anyhow!(
                    "{prefix}.expanded-width: expected a positive integer. \
                     Expected: a width in logical pixels, e.g. 200."
                )
            })?
        }
    };

    let expand_trigger = match children.get_arg("expand-trigger") {
        None => defaults.expand_trigger,
        Some(v) => {
            let s = v.as_string().ok_or_else(|| {
                anyhow::anyhow!(
                    "{prefix}.expand-trigger: expected a string value. \
                     Expected: one of: click, hover."
                )
            })?;
            ExpandTrigger::from_str(s)?
        }
    };

    let hover_delay = children
        .get_arg("hover-delay")
        .and_then(|v| v.as_integer())
        .and_then(|v| u64::try_from(v).ok())
        .unwrap_or(defaults.hover_delay);

    let expand = match children.get_arg("expand") {
        None => defaults.expand,
        Some(v) => v.as_bool().ok_or_else(|| {
            anyhow::anyhow!("{prefix}.expand: expected a boolean value. Expected: #true or #false.")
        })?,
    };

    let expand_keybind = children
        .get_arg("expand-keybind")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let animation = match children.get("animation") {
        None => AnimationConfig::default(),
        Some(anim_node) => parse_animation_node(anim_node, &prefix)?,
    };

    let modules = parse_bar_modules(children)?;

    Ok(BarConfig {
        name: name.to_string(),
        output,
        edge,
        collapsed_width,
        expanded_width,
        expand,
        expand_trigger,
        hover_delay,
        expand_keybind,
        animation,
        modules,
    })
}

fn parse_modules_section(doc: &KdlDocument) -> anyhow::Result<Vec<ModuleConfig>> {
    let Some(modules_node) = doc.get("modules") else {
        return Ok(vec![]);
    };
    let Some(children) = modules_node.children() else {
        return Ok(vec![]);
    };

    let mut result = Vec::new();
    for (i, node) in children.nodes().iter().enumerate() {
        let node_name = node.name().value();
        if node_name != "module" {
            anyhow::bail!("modules[{i}]: expected node name \"module\", got {node_name:?}.");
        }
        let name = node
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "modules[{i}]: missing module name. \
                     Expected: a string argument, e.g. module \"clock\"."
                )
            })?
            .to_string();

        let module_type = if super::KNOWN_MODULE_TYPES.contains(&name.as_str()) {
            name.clone()
        } else {
            node.children()
                .and_then(|c| c.get("type"))
                .and_then(|n| n.entries().first())
                .and_then(|e| e.value().as_string())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "modules[{i}] \"{name}\": name is not a known module type. \
                         Add a `type \"<module type>\"` child to specify the type. \
                         Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button, tray."
                    )
                })?
                .to_string()
        };

        let params = if module_type == "battery" {
            ModuleParams::Battery(modules::parse_battery(node, i)?)
        } else if module_type == "network" {
            ModuleParams::Network(modules::parse_network(node, i)?)
        } else if module_type == "volume" {
            ModuleParams::Volume(modules::parse_volume(node, i)?)
        } else if module_type == "clock" {
            ModuleParams::Clock(modules::parse_clock(node, i)?)
        } else if module_type == "script" {
            ModuleParams::Script(modules::parse_script(node, i)?)
        } else if module_type == "button" {
            ModuleParams::Button(modules::parse_button(node, i)?)
        } else if module_type == "workspaces" {
            ModuleParams::Workspaces(modules::parse_workspaces(node, i)?)
        } else if module_type == "tray" {
            ModuleParams::Tray(modules::parse_tray(node, i)?)
        } else if module_type == "focused-window" {
            ModuleParams::FocusedWindow(modules::parse_focused_window(node, i)?)
        } else {
            ModuleParams::None
        };
        result.push(ModuleConfig {
            name,
            module_type,
            params,
        });
    }
    Ok(result)
}

/// Parse an `animation { expand-ms N collapse-ms N }` KDL node into an `AnimationConfig`.
///
/// `prefix` is prepended in error messages (e.g. `"bar"` → `"bar.animation.expand-ms: ..."`).
/// Returns `AnimationConfig::default()` when the node has no children block.
fn parse_animation_node(anim_node: &KdlNode, prefix: &str) -> anyhow::Result<AnimationConfig> {
    let Some(anim_children) = anim_node.children() else {
        return Ok(AnimationConfig::default());
    };
    let defaults = AnimationConfig::default();

    let expand_ms = match anim_children.get_arg("expand-ms") {
        None => defaults.expand_ms,
        Some(v) => {
            let n = v.as_integer().ok_or_else(|| {
                anyhow::anyhow!(
                    "{prefix}.animation.expand-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 280."
                )
            })?;
            u32::try_from(n).map_err(|_| {
                anyhow::anyhow!(
                    "{prefix}.animation.expand-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 280."
                )
            })? as f32
        }
    };

    let collapse_ms = match anim_children.get_arg("collapse-ms") {
        None => defaults.collapse_ms,
        Some(v) => {
            let n = v.as_integer().ok_or_else(|| {
                anyhow::anyhow!(
                    "{prefix}.animation.collapse-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 200."
                )
            })?;
            u32::try_from(n).map_err(|_| {
                anyhow::anyhow!(
                    "{prefix}.animation.collapse-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 200."
                )
            })? as f32
        }
    };

    Ok(AnimationConfig {
        expand_ms,
        collapse_ms,
    })
}

fn parse_bar_modules(children: &KdlDocument) -> anyhow::Result<BarModules> {
    let Some(modules_node) = children.get("modules") else {
        return Ok(BarModules::default());
    };
    let Some(zone_doc) = modules_node.children() else {
        return Ok(BarModules::default());
    };
    parse_bar_modules_from_doc(zone_doc)
}

/// Parse `top`, `center`, `bottom` zone nodes from a `KdlDocument`.
/// Shared by the global `bar.modules` section and per-output `modules` overrides.
fn parse_bar_modules_from_doc(zone_doc: &KdlDocument) -> anyhow::Result<BarModules> {
    let mut result = BarModules::default();
    for zone_node in zone_doc.nodes() {
        let zone_name = zone_node.name().value();
        let names: Vec<String> = zone_node
            .entries()
            .iter()
            .filter(|e| e.name().is_none())
            .filter_map(|e| e.value().as_string())
            .map(|s| s.to_string())
            .collect();
        match zone_name {
            "top" => result.top = names,
            "center" => result.center = names,
            "bottom" => result.bottom = names,
            other => anyhow::bail!(
                "modules: unknown zone {other:?}. \
                 Expected: one of: top, center, bottom."
            ),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_bar_section() {
        let kdl = r#"
bar "main" {
    edge "right"
    collapsed-width 56
    expanded-width 240
}
"#;
        let config = parse(kdl).unwrap();
        assert!(matches!(config.bars[0].edge, Edge::Right));
        assert_eq!(config.bars[0].collapsed_width, 56);
        assert_eq!(config.bars[0].expanded_width, 240);
    }

    #[test]
    fn parse_edge_left() {
        let kdl = r#"bar "main" { edge "left" }"#;
        let config = parse(kdl).unwrap();
        assert!(matches!(config.bars[0].edge, Edge::Left));
    }

    #[test]
    fn parse_unknown_edge_errors() {
        let kdl = r#"bar "main" { edge "diagonal" }"#;
        assert!(parse(kdl).is_err());
    }

    #[test]
    fn parse_empty_string_uses_defaults() {
        let config = parse("").unwrap();
        assert!(matches!(config.bars[0].edge, Edge::Left));
        assert_eq!(config.bars[0].collapsed_width, 48);
        assert_eq!(config.bars[0].expanded_width, 200);
    }

    #[test]
    fn parse_bar_without_children_uses_defaults() {
        // A named bar node with no children block — falls back to defaults for that bar.
        let config = parse(r#"bar "main""#).unwrap();
        assert_eq!(config.bars[0].collapsed_width, 48);
    }

    #[test]
    fn unknown_edge_error_message() {
        let kdl = r#"bar "main" { edge "diagonal" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(".edge:"),
            "expected '.edge:' in message, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

    #[test]
    fn wrong_type_edge_error_message() {
        let kdl = r#"bar "main" { edge 42 }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(".edge:"),
            "expected '.edge:' in message, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

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
    fn parse_clock_with_custom_formats() {
        let kdl = r#"
modules {
    module "clock" {
        time-format "%I:%M %p"
        date-format "%Y-%m-%d"
    }
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.modules.len(), 1);
        match &config.modules[0].params {
            ModuleParams::Clock(cfg) => {
                assert_eq!(cfg.time_format, "%I:%M %p");
                assert_eq!(cfg.date_format, "%Y-%m-%d");
            }
            _ => panic!("expected ModuleParams::Clock"),
        }
    }

    #[test]
    fn parse_clock_uses_defaults_without_children() {
        let kdl = r#"modules { module "clock" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.modules.len(), 1);
        match &config.modules[0].params {
            ModuleParams::Clock(cfg) => {
                assert_eq!(cfg.time_format, "%H:%M");
                assert_eq!(cfg.date_format, "%a %d %b");
            }
            _ => panic!("expected ModuleParams::Clock"),
        }
    }

    #[test]
    fn parse_clock_rejects_empty_time_format() {
        let kdl = r#"
modules {
    module "clock" {
        time-format ""
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("time-format"),
            "expected 'time-format' in error, got: {msg}"
        );
        assert!(
            msg.contains("must not be empty"),
            "expected 'must not be empty' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_clock_rejects_empty_date_format() {
        let kdl = r#"
modules {
    module "clock" {
        date-format ""
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("date-format"),
            "expected 'date-format' in error, got: {msg}"
        );
        assert!(
            msg.contains("must not be empty"),
            "expected 'must not be empty' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_clock_time_format_without_value_errors() {
        let kdl = r#"
modules {
    module "clock" {
        time-format
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("clock.time-format"),
            "expected 'clock.time-format' in error, got: {msg}"
        );
        assert!(
            msg.contains("expected a string value"),
            "expected 'expected a string value' in error, got: {msg}"
        );
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
                assert_eq!(cfg.command, "date");
                assert_eq!(cfg.interval, 10);
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
                assert_eq!(cfg.command, "date");
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
                assert_eq!(cfg.on_click, "kitty");
            }
            _ => panic!("expected ModuleParams::Button"),
        }
    }

    #[test]
    fn parse_button_missing_block_errors() {
        // No child block at all — distinct from a missing individual field.
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
        assert!(msg.contains("button.label"), "expected 'button.label' in error, got: {msg}");
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
    fn parse_optional_u8_range_reports_out_of_range() {
        let kdl = r#"modules { module "volume" { step 101 } }"#;
        let err = parse(kdl).unwrap_err();
        assert!(err.to_string().contains("volume.step"));
        assert!(err.to_string().contains("1–100"));
    }

    #[test]
    fn parse_volume_step_without_value_errors() {
        let kdl = r#"
modules {
    module "volume" {
        step
    }
}
"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("volume.step"),
            "expected 'volume.step' in error, got: {msg}"
        );
        assert!(
            msg.contains("expected an integer value"),
            "expected 'expected an integer value' in error, got: {msg}"
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

    // --- Story 3.3 tests: expand-trigger and hover-delay ---

    #[test]
    fn parse_expand_trigger_hover() {
        let kdl = r#"bar "main" { expand-trigger "hover" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].expand_trigger, ExpandTrigger::Hover);
    }

    #[test]
    fn parse_expand_trigger_click() {
        let kdl = r#"bar "main" { expand-trigger "click" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].expand_trigger, ExpandTrigger::Click);
    }

    #[test]
    fn parse_expand_trigger_defaults_to_click() {
        let config = parse("").unwrap();
        assert_eq!(config.bars[0].expand_trigger, ExpandTrigger::Click);
    }

    #[test]
    fn parse_hover_delay() {
        let kdl = r#"bar "main" { hover-delay 200 }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].hover_delay, 200);
    }

    #[test]
    fn parse_hover_delay_defaults_to_zero() {
        let config = parse("").unwrap();
        assert_eq!(config.bars[0].hover_delay, 0);
    }

    // --- Story 6.4 tests: expand-keybind ---

    #[test]
    fn parse_expand_keybind() {
        let kdl = r#"bar "main" { expand-keybind "Super+e" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].expand_keybind, Some("Super+e".to_string()));
    }

    #[test]
    fn parse_expand_keybind_defaults_to_none() {
        let config = parse("").unwrap();
        assert_eq!(config.bars[0].expand_keybind, None);
    }

    #[test]
    fn parse_bar_without_expand_keybind_uses_none() {
        let kdl = r#"bar "main" { edge "left" }"#;
        let config = parse(kdl).unwrap();
        assert!(config.bars[0].expand_keybind.is_none());
    }

    #[test]
    fn parse_expand_false_disables_expand() {
        let kdl = r#"bar "main" { expand #false }"#;
        let config = parse(kdl).unwrap();
        assert!(!config.bars[0].expand);
    }

    #[test]
    fn parse_expand_true_enables_expand() {
        let kdl = r#"bar "main" { expand #true }"#;
        let config = parse(kdl).unwrap();
        assert!(config.bars[0].expand);
    }

    #[test]
    fn parse_expand_defaults_to_true() {
        let config = parse("").unwrap();
        assert!(config.bars[0].expand);
    }

    #[test]
    fn parse_expand_wrong_type_errors() {
        let kdl = r#"bar "main" { expand "yes" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(".expand:"),
            "expected '.expand:' in message, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

    #[test]
    fn parse_unknown_expand_trigger_errors() {
        let kdl = r#"bar "main" { expand-trigger "double-click" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(".expand-trigger:"),
            "expected '.expand-trigger:' in message, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
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

    // --- Story 4.3 tests: workspace label/icon config ---

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

    // --- Named module tests ---

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

    // --- Story 9.1: Named bar sections with output targeting (parse tests) ---

    #[test]
    fn parse_named_bar_section() {
        let kdl = r#"bar "main" { edge "right" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars.len(), 1);
        assert_eq!(config.bars[0].name, "main");
        assert_eq!(config.bars[0].output, "all"); // default
        assert!(matches!(config.bars[0].edge, Edge::Right));
    }

    #[test]
    fn parse_bar_output_field() {
        let kdl = r#"bar "side" { output "DP-1" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].name, "side");
        assert_eq!(config.bars[0].output, "DP-1");
    }

    #[test]
    fn parse_multiple_named_bar_sections() {
        let kdl = r#"
bar "main" { output "all" }
bar "top-strip" {
    output "DP-1"
    edge "top"
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars.len(), 2);
        assert_eq!(config.bars[0].name, "main");
        assert_eq!(config.bars[0].output, "all");
        assert_eq!(config.bars[1].name, "top-strip");
        assert_eq!(config.bars[1].output, "DP-1");
        assert!(matches!(config.bars[1].edge, Edge::Top));
    }

    #[test]
    fn parse_bar_missing_name_errors() {
        // Unnamed bar node (no positional string argument) must error.
        let kdl = r#"bar { edge "left" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("missing name"),
            "expected 'missing name' in error, got: {msg}"
        );
    }

    #[test]
    fn parse_bar_defaults_output_to_all() {
        let kdl = r#"bar "main" { }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].output, "all");
    }

    // ── Story 3.4: AnimationConfig parsing ─────────────────────────────────

    #[test]
    fn parse_animation_expand_ms() {
        let config = parse(r#"bar "main" { animation { expand-ms 400 } }"#).unwrap();
        assert_eq!(config.bars[0].animation.expand_ms, 400.0);
        assert_eq!(config.bars[0].animation.collapse_ms, 200.0); // default unchanged
    }

    #[test]
    fn parse_animation_collapse_ms() {
        let config = parse(r#"bar "main" { animation { collapse-ms 100 } }"#).unwrap();
        assert_eq!(config.bars[0].animation.collapse_ms, 100.0);
        assert_eq!(config.bars[0].animation.expand_ms, 280.0); // default unchanged
    }

    #[test]
    fn parse_animation_defaults_when_omitted() {
        let config = parse("").unwrap();
        assert_eq!(config.bars[0].animation.expand_ms, 280.0);
        assert_eq!(config.bars[0].animation.collapse_ms, 200.0);
    }

    #[test]
    fn parse_animation_node_no_children_uses_defaults() {
        // `animation` node with no children block → defaults
        let config = parse(r#"bar "main" { animation }"#).unwrap();
        assert_eq!(config.bars[0].animation.expand_ms, 280.0);
        assert_eq!(config.bars[0].animation.collapse_ms, 200.0);
    }

    #[test]
    fn parse_animation_negative_expand_ms_errors() {
        let result = parse(r#"bar "main" { animation { expand-ms -1 } }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(".animation.expand-ms"),
            "error should contain '.animation.expand-ms': {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "error should contain 'Expected:': {msg}"
        );
    }

    #[test]
    fn parse_animation_negative_collapse_ms_errors() {
        let result = parse(r#"bar "main" { animation { collapse-ms -1 } }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(".animation.collapse-ms"),
            "error should contain '.animation.collapse-ms': {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "error should contain 'Expected:': {msg}"
        );
    }

    #[test]
    fn parse_negative_collapsed_width_errors() {
        let result = parse(r#"bar "main" { collapsed-width -5 }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(".collapsed-width"),
            "error should contain '.collapsed-width': {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "error should contain 'Expected:': {msg}"
        );
    }

    #[test]
    fn parse_negative_expanded_width_errors() {
        let result = parse(r#"bar "main" { expanded-width -10 }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(".expanded-width"),
            "error should contain '.expanded-width': {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "error should contain 'Expected:': {msg}"
        );
    }

    // --- Additional Story 9.1 parse tests ---

    #[test]
    fn parse_bar_with_full_settings() {
        let kdl = r#"
bar "full" {
    output "DP-2"
    edge "right"
    collapsed-width 56
    expanded-width 240
    expand #true
    expand-trigger "click"
    hover-delay 0
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].name, "full");
        assert_eq!(config.bars[0].output, "DP-2");
        assert!(matches!(config.bars[0].edge, Edge::Right));
        assert_eq!(config.bars[0].collapsed_width, 56);
        assert_eq!(config.bars[0].expanded_width, 240);
        assert!(config.bars[0].expand);
    }

    #[test]
    fn parse_bar_name_preserved() {
        let kdl = r#"bar "my-custom-bar" { }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].name, "my-custom-bar");
    }

    #[test]
    fn parse_bar_output_all_explicit() {
        let kdl = r#"bar "main" { output "all" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].output, "all");
    }

    #[test]
    fn parse_empty_doc_produces_single_default_bar() {
        let config = parse("").unwrap();
        assert_eq!(config.bars.len(), 1);
        assert_eq!(config.bars[0].name, "main");
        assert_eq!(config.bars[0].output, "all");
    }

    #[test]
    fn parse_two_bars_different_outputs() {
        let kdl = r#"
bar "left" { output "eDP-1" edge "left" }
bar "right" { output "DP-1" edge "right" }
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars.len(), 2);
        assert_eq!(config.bars[0].output, "eDP-1");
        assert_eq!(config.bars[1].output, "DP-1");
    }

    #[test]
    fn parse_bar_with_modules_and_output() {
        let kdl = r#"
bar "main" {
    output "eDP-1"
    modules {
        top "workspaces"
        bottom "clock"
    }
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars[0].output, "eDP-1");
        assert_eq!(config.bars[0].modules.top, vec!["workspaces"]);
        assert_eq!(config.bars[0].modules.bottom, vec!["clock"]);
    }

    #[test]
    fn parse_bar_missing_name_error_format() {
        let kdl = r#"bar { }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("bar:"), "expected 'bar:' in error: {msg}");
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in error: {msg}"
        );
    }

    #[test]
    fn parse_multiple_bars_preserves_declaration_order() {
        let kdl = r#"
bar "alpha" { }
bar "beta" { }
bar "gamma" { }
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars.len(), 3);
        assert_eq!(config.bars[0].name, "alpha");
        assert_eq!(config.bars[1].name, "beta");
        assert_eq!(config.bars[2].name, "gamma");
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
    fn parse_two_bars_both_target_all() {
        let kdl = r#"
bar "side" { output "all" }
bar "top" { output "all" edge "top" }
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars.len(), 2);
        assert_eq!(config.bars[0].output, "all");
        assert_eq!(config.bars[1].output, "all");
        assert!(matches!(config.bars[1].edge, Edge::Left));
    }

    #[test]
    fn parse_bar_output_field_round_trip() {
        for output in &["all", "eDP-1", "DP-2", "HDMI-A-1"] {
            let kdl = format!(r#"bar "main" {{ output "{output}" }}"#);
            let config = parse(&kdl).unwrap();
            assert_eq!(
                &config.bars[0].output, output,
                "output round-trip failed for {output}"
            );
        }
    }
}
