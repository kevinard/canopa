use kdl::KdlDocument;

use super::{
    BarConfig, BarModules, BatteryConfig, ButtonConfig, ClockConfig, Config, Edge, ExpandTrigger,
    ModuleConfig, ModuleParams, NetworkConfig, OutputConfig, ScriptConfig, VolumeConfig,
    WorkspaceLabelConfig, WorkspacesConfig,
};

/// Parse a KDL string into a fully-typed `Config`.
///
/// Missing sections fall back to defaults; constraint validation happens in `validate()`.
pub fn parse(input: &str) -> anyhow::Result<Config> {
    let doc: KdlDocument = input.parse().map_err(|e| anyhow::anyhow!("config: {e}"))?;
    let bar = parse_bar_section(&doc)?;
    let modules = parse_modules_section(&doc)?;
    let outputs = parse_output_sections(&doc)?;
    Ok(Config { bar, modules, outputs })
}

fn parse_output_sections(doc: &KdlDocument) -> anyhow::Result<Vec<OutputConfig>> {
    let mut result = Vec::new();
    for node in doc.nodes() {
        if node.name().value() != "output" {
            continue;
        }
        // output name is the first positional argument
        let name = node
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "output: missing name argument. \
                     Expected: output \"<name>\" {{ ... }}, e.g. output \"DP-1\" {{ enabled #false }}"
                )
            })?
            .to_string();

        let mut enabled = true;
        let mut collapsed_width: Option<u32> = None;
        let mut expanded_width: Option<u32> = None;
        let mut modules: Option<BarModules> = None;
        let mut edge: Option<Edge> = None;
        let mut expand: Option<bool> = None;
        let mut expand_trigger: Option<ExpandTrigger> = None;
        let mut hover_delay: Option<u64> = None;
        let mut expand_keybind: Option<String> = None;

        if let Some(children) = node.children() {
            for child in children.nodes() {
                match child.name().value() {
                    "enabled" => {
                        enabled = child
                            .entries()
                            .iter()
                            .find(|e| e.name().is_none())
                            .and_then(|e| e.value().as_bool())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "output {name:?}: enabled: expected a boolean value (#true or #false)."
                                )
                            })?;
                    }
                    "collapsed-width" => {
                        let v = child
                            .entries()
                            .iter()
                            .find(|e| e.name().is_none())
                            .and_then(|e| e.value().as_integer())
                            .and_then(|v| u32::try_from(v).ok())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "output {name:?}: collapsed-width: expected a positive integer."
                                )
                            })?;
                        collapsed_width = Some(v);
                    }
                    "expanded-width" => {
                        let v = child
                            .entries()
                            .iter()
                            .find(|e| e.name().is_none())
                            .and_then(|e| e.value().as_integer())
                            .and_then(|v| u32::try_from(v).ok())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "output {name:?}: expanded-width: expected a positive integer."
                                )
                            })?;
                        expanded_width = Some(v);
                    }
                    "modules" => {
                        let mods = if let Some(zone_doc) = child.children() {
                            parse_bar_modules_from_doc(zone_doc)?
                        } else {
                            BarModules::default()
                        };
                        modules = Some(mods);
                    }
                    "edge" => {
                        let s = child
                            .entries()
                            .iter()
                            .find(|e| e.name().is_none())
                            .and_then(|e| e.value().as_string())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "output {name:?}: edge: expected a string value."
                                )
                            })?;
                        edge = Some(Edge::from_str(s)?);
                    }
                    "expand" => {
                        let v = child
                            .entries()
                            .iter()
                            .find(|e| e.name().is_none())
                            .and_then(|e| e.value().as_bool())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "output {name:?}: expand: expected a boolean value (#true or #false)."
                                )
                            })?;
                        expand = Some(v);
                    }
                    "expand-trigger" => {
                        let s = child
                            .entries()
                            .iter()
                            .find(|e| e.name().is_none())
                            .and_then(|e| e.value().as_string())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "output {name:?}: expand-trigger: expected a string value."
                                )
                            })?;
                        expand_trigger = Some(ExpandTrigger::from_str(s)?);
                    }
                    "hover-delay" => {
                        let v = child
                            .entries()
                            .iter()
                            .find(|e| e.name().is_none())
                            .and_then(|e| e.value().as_integer())
                            .and_then(|v| u64::try_from(v).ok())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "output {name:?}: hover-delay: expected a non-negative integer."
                                )
                            })?;
                        hover_delay = Some(v);
                    }
                    "expand-keybind" => {
                        let s = child
                            .entries()
                            .iter()
                            .find(|e| e.name().is_none())
                            .and_then(|e| e.value().as_string())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "output {name:?}: expand-keybind: expected a string value."
                                )
                            })?
                            .to_string();
                        expand_keybind = Some(s);
                    }
                    other => {
                        anyhow::bail!(
                            "output {name:?}: unknown key {other:?}. \
                             Expected: one of: enabled, edge, collapsed-width, expanded-width, \
                             expand, expand-trigger, hover-delay, expand-keybind, modules."
                        );
                    }
                }
            }
        }
        result.push(OutputConfig {
            name,
            enabled,
            collapsed_width,
            expanded_width,
            modules,
            edge,
            expand,
            expand_trigger,
            hover_delay,
            expand_keybind,
        });
    }
    Ok(result)
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
            anyhow::bail!(
                "modules[{i}]: expected node name \"module\", got {node_name:?}."
            );
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
                         Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button."
                    )
                })?
                .to_string()
        };

        let params = if module_type == "battery" {
            ModuleParams::Battery(parse_battery_params(node, i)?)
        } else if module_type == "network" {
            ModuleParams::Network(NetworkConfig)
        } else if module_type == "volume" {
            ModuleParams::Volume(parse_volume_params(node, i)?)
        } else if module_type == "clock" {
            ModuleParams::Clock(parse_clock_params(node, i)?)
        } else if module_type == "script" {
            ModuleParams::Script(parse_script_params(node, i)?)
        } else if module_type == "button" {
            ModuleParams::Button(parse_button_params(node, i)?)
        } else if module_type == "workspaces" {
            ModuleParams::Workspaces(parse_workspaces_params(node, i)?)
        } else {
            ModuleParams::None
        };
        result.push(ModuleConfig { name, module_type, params });
    }
    Ok(result)
}

fn parse_battery_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<BatteryConfig> {
    // Optional `critical-threshold <int>` child node.
    let Some(children) = node.children() else {
        return Ok(BatteryConfig::default());
    };
    let critical_threshold = match children.get("critical-threshold") {
        None => BatteryConfig::default().critical_threshold,
        Some(n) => {
            let v = n.entries().first().map(|e| e.value());
            match v.and_then(|v| v.as_integer()) {
                Some(n) if (0..=100).contains(&n) => n as u8,
                Some(n) => anyhow::bail!(
                    "modules[{idx}].battery.critical-threshold: value {n} out of range. \
                     Expected: integer 0–100."
                ),
                None => anyhow::bail!(
                    "modules[{idx}].battery.critical-threshold: expected an integer value."
                ),
            }
        }
    };
    Ok(BatteryConfig { critical_threshold })
}

fn parse_volume_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<VolumeConfig> {
    let Some(children) = node.children() else {
        return Ok(VolumeConfig::default());
    };
    let step = match children.get("step") {
        None => VolumeConfig::default().step,
        Some(n) => {
            let v = n.entries().first().map(|e| e.value());
            match v.and_then(|v| v.as_integer()) {
                Some(n) if (1..=100).contains(&n) => n as u8,
                Some(n) => anyhow::bail!(
                    "modules[{idx}].volume.step: value {n} out of range. \
                     Expected: integer 1–100."
                ),
                None => anyhow::bail!("modules[{idx}].volume.step: expected an integer value."),
            }
        }
    };
    Ok(VolumeConfig { step })
}

fn parse_clock_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<ClockConfig> {
    let Some(children) = node.children() else {
        return Ok(ClockConfig::default());
    };

    let defaults = ClockConfig::default();

    let time_format = match children.get("time-format") {
        None => defaults.time_format,
        Some(n) => {
            let v = n.entries().first().map(|e| e.value());
            match v.and_then(|v| v.as_string()) {
                Some("") => anyhow::bail!(
                    "modules[{idx}].clock.time-format: value must not be empty. \
                     Expected: a chrono format string such as \"%H:%M\"."
                ),
                Some(s) => s.to_string(),
                None => anyhow::bail!("modules[{idx}].clock.time-format: expected a string value."),
            }
        }
    };

    let date_format = match children.get("date-format") {
        None => defaults.date_format,
        Some(n) => {
            let v = n.entries().first().map(|e| e.value());
            match v.and_then(|v| v.as_string()) {
                Some("") => anyhow::bail!(
                    "modules[{idx}].clock.date-format: value must not be empty. \
                     Expected: a chrono format string such as \"%a %d %b\"."
                ),
                Some(s) => s.to_string(),
                None => anyhow::bail!("modules[{idx}].clock.date-format: expected a string value."),
            }
        }
    };

    Ok(ClockConfig {
        time_format,
        date_format,
    })
}

fn parse_script_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<ScriptConfig> {
    let children = node.children().ok_or_else(|| {
        anyhow::anyhow!(
            "modules[{idx}].script.command: missing required parameter. Expected: a string value."
        )
    })?;

    let command = match children.get("command") {
        None => anyhow::bail!(
            "modules[{idx}].script.command: missing required parameter. Expected: a string value."
        ),
        Some(n) => {
            let v = n.entries().first().map(|e| e.value());
            match v.and_then(|v| v.as_string()) {
                Some("") => anyhow::bail!(
                    "modules[{idx}].script.command: value must not be empty. \
                     Expected: a shell command string."
                ),
                Some(s) => s.to_string(),
                None => anyhow::bail!("modules[{idx}].script.command: expected a string value."),
            }
        }
    };

    let interval = match children.get("interval") {
        None => 5u64,
        Some(n) => {
            let v = n.entries().first().map(|e| e.value());
            match v.and_then(|v| v.as_integer()) {
                Some(i) if i >= 1 => i as u64,
                Some(_) => anyhow::bail!(
                    "modules[{idx}].script.interval: value must be at least 1. Expected: a positive integer."
                ),
                None => {
                    anyhow::bail!("modules[{idx}].script.interval: expected a positive integer.")
                }
            }
        }
    };

    Ok(ScriptConfig { command, interval })
}

fn parse_button_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<ButtonConfig> {
    let children = node.children().ok_or_else(|| {
        anyhow::anyhow!(
            "modules[{idx}].button: missing required parameters block. Expected child nodes: label, icon, on-click."
        )
    })?;

    let label = match children.get("label") {
        None => anyhow::bail!(
            "modules[{idx}].button.label: missing required parameter. Expected: a string value."
        ),
        Some(n) => {
            let v = n.entries().first().map(|e| e.value());
            match v.and_then(|v| v.as_string()) {
                Some(s) => s.to_string(),
                None => anyhow::bail!("modules[{idx}].button.label: expected a string value."),
            }
        }
    };

    let icon = match children.get("icon") {
        None => anyhow::bail!(
            "modules[{idx}].button.icon: missing required parameter. Expected: a string value."
        ),
        Some(n) => {
            let v = n.entries().first().map(|e| e.value());
            match v.and_then(|v| v.as_string()) {
                Some(s) => s.to_string(),
                None => anyhow::bail!("modules[{idx}].button.icon: expected a string value."),
            }
        }
    };

    let on_click = match children.get("on-click") {
        None => anyhow::bail!(
            "modules[{idx}].button.on-click: missing required parameter. Expected: a string value."
        ),
        Some(n) => {
            let v = n.entries().first().map(|e| e.value());
            match v.and_then(|v| v.as_string()) {
                Some(s) => s.to_string(),
                None => anyhow::bail!("modules[{idx}].button.on-click: expected a string value."),
            }
        }
    };

    Ok(ButtonConfig {
        label,
        icon,
        on_click,
    })
}

fn parse_workspaces_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<WorkspacesConfig> {
    let mut config = WorkspacesConfig::default();

    let Some(children) = node.children() else {
        return Ok(config); // No workspace config — all workspaces show number only.
    };

    for ws_node in children.nodes() {
        if ws_node.name().value() != "workspace" {
            anyhow::bail!(
                "modules[{idx}].workspaces: unknown child node {:?}. Expected: \"workspace\".",
                ws_node.name().value()
            );
        }

        let ws_num = ws_node
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_integer())
            .and_then(|n| if n >= 1 { Some(n as u32) } else { None })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "modules[{idx}].workspaces.workspace: missing or invalid workspace number. \
                     Expected: a positive integer (1-based workspace number, e.g. 1)."
                )
            })?;

        let (label, icon) = if let Some(ws_children) = ws_node.children() {
            let label = ws_children
                .get("label")
                .and_then(|n| n.entries().first())
                .and_then(|e| e.value().as_string())
                .map(|s| s.to_string());
            let icon = ws_children
                .get("icon")
                .and_then(|n| n.entries().first())
                .and_then(|e| e.value().as_string())
                .map(|s| s.to_string());
            (label, icon)
        } else {
            (None, None)
        };

        config
            .workspaces
            .insert(ws_num, WorkspaceLabelConfig { label, icon });
    }

    Ok(config)
}

fn parse_bar_section(doc: &KdlDocument) -> anyhow::Result<BarConfig> {
    let Some(bar_node) = doc.get("bar") else {
        return Ok(BarConfig::default());
    };

    let children = match bar_node.children() {
        Some(c) => c,
        None => return Ok(BarConfig::default()),
    };

    let defaults = BarConfig::default();

    let edge = match children.get_arg("edge") {
        None => defaults.edge,
        Some(v) => {
            let s = v.as_string().ok_or_else(|| {
                anyhow::anyhow!(
                    "bar.edge: expected a string value. Expected: one of: left, right, top, bottom."
                )
            })?;
            Edge::from_str(s)?
        }
    };

    let collapsed_width = children
        .get_arg("collapsed-width")
        .and_then(|v| v.as_integer())
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(defaults.collapsed_width);

    let expanded_width = children
        .get_arg("expanded-width")
        .and_then(|v| v.as_integer())
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(defaults.expanded_width);

    let expand_trigger = match children.get_arg("expand-trigger") {
        None => defaults.expand_trigger,
        Some(v) => {
            let s = v.as_string().ok_or_else(|| {
                anyhow::anyhow!(
                    "bar.expand-trigger: expected a string value. \
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
            anyhow::anyhow!(
                "bar.expand: expected a boolean value. Expected: #true or #false."
            )
        })?,
    };

    let expand_keybind = children
        .get_arg("expand-keybind")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let modules = parse_bar_modules(children)?;

    Ok(BarConfig {
        edge,
        collapsed_width,
        expanded_width,
        expand,
        expand_trigger,
        hover_delay,
        expand_keybind,
        modules,
    })
}

fn parse_bar_modules(children: &kdl::KdlDocument) -> anyhow::Result<BarModules> {
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
fn parse_bar_modules_from_doc(zone_doc: &kdl::KdlDocument) -> anyhow::Result<BarModules> {
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
bar {
    edge "right"
    collapsed-width 56
    expanded-width 240
}
"#;
        let config = parse(kdl).unwrap();
        assert!(matches!(config.bar.edge, Edge::Right));
        assert_eq!(config.bar.collapsed_width, 56);
        assert_eq!(config.bar.expanded_width, 240);
    }

    #[test]
    fn parse_edge_left() {
        let kdl = r#"bar { edge "left" }"#;
        let config = parse(kdl).unwrap();
        assert!(matches!(config.bar.edge, Edge::Left));
    }

    #[test]
    fn parse_unknown_edge_errors() {
        let kdl = r#"bar { edge "diagonal" }"#;
        assert!(parse(kdl).is_err());
    }

    #[test]
    fn parse_empty_string_uses_defaults() {
        let config = parse("").unwrap();
        assert!(matches!(config.bar.edge, Edge::Left));
        assert_eq!(config.bar.collapsed_width, 48);
        assert_eq!(config.bar.expanded_width, 200);
    }

    #[test]
    fn parse_bar_without_children_uses_defaults() {
        // A bar node with no children block
        let config = parse("bar\n").unwrap();
        assert_eq!(config.bar.collapsed_width, 48);
    }

    #[test]
    fn unknown_edge_error_message() {
        let kdl = r#"bar { edge "diagonal" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("bar.edge:"),
            "expected 'bar.edge:' prefix, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

    #[test]
    fn wrong_type_edge_error_message() {
        let kdl = r#"bar { edge 42 }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("bar.edge:"),
            "expected 'bar.edge:' prefix, got: {msg}"
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
        let kdl = r#"bar { expand-trigger "hover" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bar.expand_trigger, ExpandTrigger::Hover);
    }

    #[test]
    fn parse_expand_trigger_click() {
        let kdl = r#"bar { expand-trigger "click" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bar.expand_trigger, ExpandTrigger::Click);
    }

    #[test]
    fn parse_expand_trigger_defaults_to_click() {
        let config = parse("").unwrap();
        assert_eq!(config.bar.expand_trigger, ExpandTrigger::Click);
    }

    #[test]
    fn parse_hover_delay() {
        let kdl = r#"bar { hover-delay 200 }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bar.hover_delay, 200);
    }

    #[test]
    fn parse_hover_delay_defaults_to_zero() {
        let config = parse("").unwrap();
        assert_eq!(config.bar.hover_delay, 0);
    }

    // --- Story 6.4 tests: expand-keybind ---

    #[test]
    fn parse_expand_keybind() {
        let kdl = r#"bar { expand-keybind "Super+e" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bar.expand_keybind, Some("Super+e".to_string()));
    }

    #[test]
    fn parse_expand_keybind_defaults_to_none() {
        let config = parse("").unwrap();
        assert_eq!(config.bar.expand_keybind, None);
    }

    #[test]
    fn parse_bar_without_expand_keybind_uses_none() {
        let kdl = r#"bar { edge "left" }"#;
        let config = parse(kdl).unwrap();
        assert!(config.bar.expand_keybind.is_none());
    }

    #[test]
    fn parse_expand_false_disables_expand() {
        let kdl = r#"bar { expand #false }"#;
        let config = parse(kdl).unwrap();
        assert!(!config.bar.expand);
    }

    #[test]
    fn parse_expand_true_enables_expand() {
        let kdl = r#"bar { expand #true }"#;
        let config = parse(kdl).unwrap();
        assert!(config.bar.expand);
    }

    #[test]
    fn parse_expand_defaults_to_true() {
        let config = parse("").unwrap();
        assert!(config.bar.expand);
    }

    #[test]
    fn parse_expand_wrong_type_errors() {
        let kdl = r#"bar { expand "yes" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("bar.expand:"),
            "expected 'bar.expand:' prefix, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

    #[test]
    fn parse_unknown_expand_trigger_errors() {
        let kdl = r#"bar { expand-trigger "double-click" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("bar.expand-trigger:"),
            "expected 'bar.expand-trigger:' prefix, got: {msg}"
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

    // --- Story 7.4: Per-output override parsing tests ---

    #[test]
    fn output_collapsed_width_override_parses() {
        let config = parse(r#"output "eDP-1" { collapsed-width 60 }"#).unwrap();
        assert_eq!(config.outputs[0].collapsed_width, Some(60));
        assert!(config.outputs[0].expanded_width.is_none());
        assert!(config.outputs[0].modules.is_none());
    }

    #[test]
    fn output_expanded_width_override_parses() {
        let config = parse(r#"output "eDP-1" { expanded-width 300 }"#).unwrap();
        assert_eq!(config.outputs[0].expanded_width, Some(300));
        assert!(config.outputs[0].collapsed_width.is_none());
    }

    #[test]
    fn output_both_widths_override_parse() {
        let kdl = "output \"eDP-1\" {\n    collapsed-width 60\n    expanded-width 250\n}";
        let config = parse(kdl).unwrap();
        assert_eq!(config.outputs[0].collapsed_width, Some(60));
        assert_eq!(config.outputs[0].expanded_width, Some(250));
    }

    #[test]
    fn output_modules_override_parses() {
        let kdl = r#"output "eDP-1" { modules { top "clock" "battery" } }"#;
        let config = parse(kdl).unwrap();
        let mods = config.outputs[0].modules.as_ref().unwrap();
        assert_eq!(mods.top, vec!["clock", "battery"]);
        assert!(mods.center.is_empty());
        assert!(mods.bottom.is_empty());
    }

    #[test]
    fn output_no_override_fields_are_none() {
        let config = parse(r#"output "eDP-1" { enabled #true }"#).unwrap();
        let oc = &config.outputs[0];
        assert!(oc.collapsed_width.is_none());
        assert!(oc.expanded_width.is_none());
        assert!(oc.modules.is_none());
    }

    #[test]
    fn output_unknown_key_error_lists_new_keys() {
        // The unknown-key error should now mention the new valid keys
        let result = parse(r#"output "DP-1" { zoom 2 }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unknown key"), "got: {msg}");
        assert!(msg.contains("collapsed-width"), "error should list new keys: {msg}");
        assert!(msg.contains("expanded-width"), "error should list new keys: {msg}");
        assert!(msg.contains("modules"), "error should list new keys: {msg}");
    }

    // --- Story 7.5: Parse tests for 5 new per-output fields ---

    #[test]
    fn output_edge_override_parses() {
        let config = parse(r#"output "DP-2" { edge "top" }"#).unwrap();
        assert!(matches!(config.outputs[0].edge, Some(Edge::Top)));
    }

    #[test]
    fn output_edge_invalid_errors() {
        let result = parse(r#"output "DP-2" { edge "diagonal" }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("edge"), "got: {msg}");
    }

    #[test]
    fn output_expand_override_parses() {
        let config = parse(r#"output "DP-2" { expand #false }"#).unwrap();
        assert_eq!(config.outputs[0].expand, Some(false));
    }

    #[test]
    fn output_expand_trigger_override_parses() {
        let config = parse(r#"output "DP-2" { expand-trigger "hover" }"#).unwrap();
        assert_eq!(config.outputs[0].expand_trigger, Some(ExpandTrigger::Hover));
    }

    #[test]
    fn output_hover_delay_override_parses() {
        let config = parse(r#"output "DP-2" { hover-delay 300 }"#).unwrap();
        assert_eq!(config.outputs[0].hover_delay, Some(300));
    }

    #[test]
    fn output_expand_keybind_override_parses() {
        let config = parse(r#"output "DP-2" { expand-keybind "Ctrl+b" }"#).unwrap();
        assert_eq!(config.outputs[0].expand_keybind, Some("Ctrl+b".into()));
    }

    #[test]
    fn output_new_fields_default_to_none_when_absent() {
        let config = parse(r#"output "DP-2" { enabled #true }"#).unwrap();
        let oc = &config.outputs[0];
        assert!(oc.edge.is_none());
        assert!(oc.expand.is_none());
        assert!(oc.expand_trigger.is_none());
        assert!(oc.hover_delay.is_none());
        assert!(oc.expand_keybind.is_none());
    }

    #[test]
    fn output_unknown_key_error_lists_all_valid_keys() {
        let result = parse(r#"output "DP-1" { zoom 2 }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("edge"), "error should mention 'edge': {msg}");
        assert!(msg.contains("expand"), "error should mention 'expand': {msg}");
    }
}
