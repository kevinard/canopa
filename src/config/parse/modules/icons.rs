use ::kdl::{KdlDocument, KdlNode, KdlValue};
use std::collections::HashMap;

use crate::config::{IconRulesConfig, IconThreshold};

pub(super) fn parse_optional(
    children: &KdlDocument,
    path: &str,
) -> anyhow::Result<IconRulesConfig> {
    let Some(icons_node) = children.get("icons") else {
        return Ok(IconRulesConfig::default());
    };
    let Some(icon_children) = icons_node.children() else {
        return Ok(IconRulesConfig::default());
    };

    let mut states = HashMap::new();
    let mut thresholds = Vec::new();

    for (idx, node) in icon_children.nodes().iter().enumerate() {
        parse_icon_node(node, idx, path, &mut states, &mut thresholds)?;
    }

    thresholds.sort_by(|a: &IconThreshold, b| {
        a.at.partial_cmp(&b.at)
            .expect("icon thresholds reject NaN while parsing")
    });

    Ok(IconRulesConfig { states, thresholds })
}

fn parse_icon_node(
    node: &KdlNode,
    idx: usize,
    path: &str,
    states: &mut HashMap<String, String>,
    thresholds: &mut Vec<IconThreshold>,
) -> anyhow::Result<()> {
    let rule_path = format!("{path}.icons[{idx}]");
    let node_name = node.name().value();
    if node_name != "icon" {
        anyhow::bail!("{rule_path}: expected node name \"icon\", got {node_name:?}.");
    }

    let positional: Vec<&KdlValue> = node
        .entries()
        .iter()
        .filter(|entry| entry.name().is_none())
        .map(|entry| entry.value())
        .collect();

    let icon = positional
        .first()
        .and_then(|value| value.as_string())
        .ok_or_else(|| anyhow::anyhow!("{rule_path}: expected a non-empty icon string."))?;
    if icon.is_empty() {
        anyhow::bail!("{rule_path}: expected a non-empty icon string.");
    }

    let state = named_value(node, "state").and_then(KdlValue::as_string);
    let at = named_value(node, "at").and_then(number_value);

    match (state, at) {
        (Some(_), Some(_)) | (None, None) => {
            anyhow::bail!("{rule_path}: expected exactly one of `state` or `at`.")
        }
        (Some(state), None) => {
            if state.is_empty() {
                anyhow::bail!("{rule_path}: state must not be empty.");
            }
            if states.insert(state.to_string(), icon.to_string()).is_some() {
                anyhow::bail!("{rule_path}: duplicate state {state:?}.");
            }
        }
        (None, Some(at)) => {
            if thresholds
                .iter()
                .any(|threshold| (threshold.at - at).abs() < f64::EPSILON)
            {
                anyhow::bail!("{rule_path}: duplicate threshold {at}.");
            }
            thresholds.push(IconThreshold {
                at,
                icon: icon.to_string(),
            });
        }
    }

    Ok(())
}

fn named_value<'a>(node: &'a KdlNode, name: &str) -> Option<&'a KdlValue> {
    node.entries()
        .iter()
        .find(|entry| entry.name().map(|entry_name| entry_name.value()) == Some(name))
        .map(|entry| entry.value())
}

fn number_value(value: &KdlValue) -> Option<f64> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|number| number as f64))
        .filter(|number| number.is_finite())
}
