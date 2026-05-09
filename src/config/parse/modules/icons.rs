use ::kdl::{KdlDocument, KdlNode, KdlValue};
use std::collections::HashMap;

use super::super::error::ParseCtx;
use crate::config::{ConfigResult, IconRulesConfig, IconThreshold};

pub(super) fn parse_optional_ctx(
    children: &KdlDocument,
    module_ctx: &ParseCtx,
) -> ConfigResult<IconRulesConfig> {
    let Some(icons_node) = children.get("icons") else {
        return Ok(IconRulesConfig::default());
    };
    let Some(icon_children) = icons_node.children() else {
        return Ok(IconRulesConfig::default());
    };

    let icons_ctx = module_ctx.child("icons");
    let mut states = HashMap::new();
    let mut thresholds = Vec::new();

    for (idx, node) in icon_children.nodes().iter().enumerate() {
        parse_icon_node_ctx(node, &icons_ctx.index(idx), &mut states, &mut thresholds)?;
    }

    thresholds.sort_by(|a: &IconThreshold, b| {
        a.at.partial_cmp(&b.at)
            .expect("icon thresholds reject NaN while parsing")
    });

    Ok(IconRulesConfig { states, thresholds })
}

fn parse_icon_node_ctx(
    node: &KdlNode,
    ctx: &ParseCtx,
    states: &mut HashMap<String, String>,
    thresholds: &mut Vec<IconThreshold>,
) -> ConfigResult<()> {
    let node_name = node.name().value();
    if node_name != "icon" {
        return Err(ctx.node_err(
            node,
            format!("expected node name `icon`, got `{node_name}`"),
            None,
        ));
    }

    let positional: Vec<_> = node
        .entries()
        .iter()
        .filter(|entry| entry.name().is_none())
        .collect();
    let Some(icon_entry) = positional.first() else {
        return Err(ctx.node_err(
            node,
            "expected a non-empty icon string",
            Some("add an icon string as the first positional value".into()),
        ));
    };
    let Some(icon) = icon_entry.value().as_string() else {
        return Err(ctx.entry_err(icon_entry, "expected a non-empty icon string", None));
    };
    if icon.is_empty() {
        return Err(ctx.entry_err(icon_entry, "expected a non-empty icon string", None));
    }

    let state = named_entry(node, "state");
    let at = named_entry(node, "at");

    match (state, at) {
        (Some(_), Some(_)) | (None, None) => {
            Err(ctx.node_err(node, "expected exactly one of `state` or `at`", None))
        }
        (Some(state), None) => {
            let Some(state_value) = state.value().as_string() else {
                return Err(ctx.entry_err(state, "`state` must be a string", None));
            };
            if state_value.is_empty() {
                return Err(ctx.entry_err(state, "state must not be empty", None));
            }
            if states
                .insert(state_value.to_string(), icon.to_string())
                .is_some()
            {
                return Err(ctx.entry_err(state, format!("duplicate state `{state_value}`"), None));
            }
            Ok(())
        }
        (None, Some(at)) => {
            let Some(at_value) = number_value(at.value()) else {
                return Err(ctx.entry_err(at, "`at` must be a finite number", None));
            };
            if thresholds
                .iter()
                .any(|threshold| (threshold.at - at_value).abs() < f64::EPSILON)
            {
                return Err(ctx.entry_err(at, format!("duplicate threshold {at_value}"), None));
            }
            thresholds.push(IconThreshold {
                at: at_value,
                icon: icon.to_string(),
            });
            Ok(())
        }
    }
}

fn named_entry<'a>(node: &'a KdlNode, name: &str) -> Option<&'a ::kdl::KdlEntry> {
    node.entries()
        .iter()
        .find(|entry| entry.name().map(|entry_name| entry_name.value()) == Some(name))
}

fn number_value(value: &KdlValue) -> Option<f64> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|number| number as f64))
        .filter(|number| number.is_finite())
}
