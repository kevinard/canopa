use kdl::KdlDocument;

use super::{
    BarConfig, BatteryConfig, ButtonConfig, ClockConfig, Config, Edge, ExpandTrigger, ModuleConfig,
    ModuleParams, ScriptConfig, WorkspaceLabelConfig, WorkspacesConfig,
};
use crate::modules::Zone;

/// Parse a KDL string into a fully-typed `Config`.
///
/// Missing sections fall back to defaults; constraint validation happens in `validate()`.
pub fn parse(input: &str) -> anyhow::Result<Config> {
    let doc: KdlDocument = input.parse().map_err(|e| anyhow::anyhow!("config: {e}"))?;
    let bar = parse_bar_section(&doc)?;
    let modules = parse_modules_section(&doc)?;
    Ok(Config { bar, modules })
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
                    "modules[{i}].type: missing module name. \
                     Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button."
                )
            })?
            .to_string();

        let zone_str = node
            .entries()
            .iter()
            .find(|e| e.name().map(|n| n.value() == "zone").unwrap_or(false))
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "modules[{i}].zone: missing required property. \
                     Expected: one of: top, center, bottom."
                )
            })?;

        let zone = match zone_str {
            "top" => Zone::Top,
            "center" => Zone::Center,
            "bottom" => Zone::Bottom,
            other => anyhow::bail!(
                "modules[{i}].zone: unknown value {other:?}. \
                 Expected: one of: top, center, bottom."
            ),
        };

        let params = if name == "battery" {
            ModuleParams::Battery(parse_battery_params(node, i)?)
        } else if name == "clock" {
            ModuleParams::Clock(parse_clock_params(node, i)?)
        } else if name == "script" {
            ModuleParams::Script(parse_script_params(node, i)?)
        } else if name == "button" {
            ModuleParams::Button(parse_button_params(node, i)?)
        } else if name == "workspaces" {
            ModuleParams::Workspaces(parse_workspaces_params(node, i)?)
        } else {
            ModuleParams::None
        };
        result.push(ModuleConfig { name, zone, params });
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

    Ok(BarConfig {
        edge,
        collapsed_width,
        expanded_width,
        expand_trigger,
        hover_delay,
    })
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
    fn parse_single_module_bottom_zone() {
        let kdl = r#"modules { module "clock" zone="bottom" }"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.modules.len(), 1);
        assert_eq!(config.modules[0].name, "clock");
        assert!(matches!(config.modules[0].zone, Zone::Bottom));
    }

    #[test]
    fn parse_multiple_modules_order_preserved() {
        let kdl = r#"
modules {
    module "clock" zone="bottom"
    module "battery" zone="top"
}
"#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.modules.len(), 2);
        assert_eq!(config.modules[0].name, "clock");
        assert!(matches!(config.modules[0].zone, Zone::Bottom));
        assert_eq!(config.modules[1].name, "battery");
        assert!(matches!(config.modules[1].zone, Zone::Top));
    }

    #[test]
    fn parse_unknown_zone_errors() {
        let kdl = r#"modules { module "clock" zone="diagonal" }"#;
        let err = parse(kdl).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("modules[0].zone:"),
            "expected 'modules[0].zone:' prefix, got: {msg}"
        );
        assert!(
            msg.contains("diagonal"),
            "expected unknown value in error: {msg}"
        );
    }

    #[test]
    fn parse_modules_empty_block() {
        let kdl = r#"modules { }"#;
        let config = parse(kdl).unwrap();
        assert!(config.modules.is_empty());
    }

    #[test]
    fn parse_wrong_node_name_errors() {
        let kdl = r#"modules { widget "clock" zone="bottom" }"#;
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
    module "clock" zone="bottom" {
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
        let kdl = r#"modules { module "clock" zone="bottom" }"#;
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
    module "clock" zone="bottom" {
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
    module "clock" zone="bottom" {
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
    module "script" zone="bottom" {
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
    module "script" zone="bottom" {
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
        let kdl = r#"modules { module "script" zone="bottom" }"#;
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
    module "script" zone="bottom" {
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
    module "script" zone="bottom" {
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
    module "button" zone="bottom" {
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
        let kdl = r#"modules { module "button" zone="bottom" }"#;
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
    module "button" zone="bottom" {
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
    module "button" zone="bottom" {
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
    module "button" zone="bottom" {
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
    module "button" zone="bottom" {
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
    module "workspaces" zone="top" {
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
        let kdl = r#"modules { module "workspaces" zone="top" }"#;
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
    module "workspaces" zone="top" {
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
    module "workspaces" zone="top" {
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
    module "workspaces" zone="top" {
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
    module "workspaces" zone="top" {
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
}
