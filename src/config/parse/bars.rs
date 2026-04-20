use ::kdl::{KdlDocument, KdlNode};

use super::super::{AnimationConfig, BarConfig, BarModules, Edge, ExpandTrigger};
use super::animation;

pub(super) fn parse(doc: &KdlDocument) -> anyhow::Result<Vec<BarConfig>> {
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

    let anim_config = match children.get("animation") {
        None => AnimationConfig::default(),
        Some(anim_node) => animation::parse_node(anim_node, &prefix)?,
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
        animation: anim_config,
        modules,
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
    use crate::config::parse::parse;
    use crate::config::{Edge, ExpandTrigger};

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
        // A named bar node with no children block -- falls back to defaults for that bar.
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
