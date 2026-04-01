use kdl::KdlDocument;

use super::{BarConfig, Config, Edge, ModuleConfig};
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

        result.push(ModuleConfig { name, zone });
    }
    Ok(result)
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

    Ok(BarConfig {
        edge,
        collapsed_width,
        expanded_width,
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
}
