use ::kdl::{KdlDocument, KdlNode};
use std::collections::HashMap;

use super::super::error::ParseCtx;
use super::super::fields;
use crate::config::{ConfigResult, IconRulesConfig, IconThreshold};

pub(super) fn parse_optional_ctx(
    children: &KdlDocument,
    module_ctx: &ParseCtx,
) -> ConfigResult<IconRulesConfig> {
    let Some(icons_node) = fields::field(children, module_ctx, "icons").optional_node()? else {
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

    let state = fields::property(node, ctx, "state")
        .optional()
        .string_message("`state` must be a string")
        .string()
        .transpose()?;
    let at = fields::property(node, ctx, "at")
        .optional()
        .number_message("`at` must be a finite number")
        .finite_number()
        .transpose()?;

    match (state.as_deref(), at) {
        (Some(_), Some(_)) | (None, None) => {
            Err(ctx.node_err(node, "expected exactly one of `state` or `at`", None))
        }
        (Some(state_value), None) => {
            if state_value.is_empty() {
                return Err(ctx.node_err(node, "state must not be empty", None));
            }
            if states
                .insert(state_value.to_string(), icon.to_string())
                .is_some()
            {
                return Err(ctx.node_err(node, format!("duplicate state `{state_value}`"), None));
            }
            Ok(())
        }
        (None, Some(at_value)) => {
            if thresholds
                .iter()
                .any(|threshold| (threshold.at - at_value).abs() < f64::EPSILON)
            {
                return Err(ctx.node_err(node, format!("duplicate threshold {at_value}"), None));
            }
            thresholds.push(IconThreshold {
                at: at_value,
                icon: icon.to_string(),
            });
            Ok(())
        }
    }
}
