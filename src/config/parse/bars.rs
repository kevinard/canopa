use ::kdl::{KdlDocument, KdlNode};

use super::animation;
use super::super::{AnimationConfig, BarConfig, BarModules, Edge, ExpandTrigger};

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

pub(super) fn parse_single_bar_node(node: &KdlNode, name: &str) -> anyhow::Result<BarConfig> {
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
pub(super) fn parse_bar_modules_from_doc(zone_doc: &KdlDocument) -> anyhow::Result<BarModules> {
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
