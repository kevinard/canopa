use ::kdl::{KdlDocument, KdlNode};

use super::super::error::ParseCtx;
use super::super::fields;
use crate::config::{ConfigResult, StatesConfig};

pub(super) fn parse_optional_ctx(
    children: &KdlDocument,
    module_ctx: &ParseCtx,
    defaults: StatesConfig,
) -> ConfigResult<StatesConfig> {
    let (merged, _) = parse_optional_with_names_ctx(children, module_ctx, defaults)?;
    Ok(merged)
}

pub(super) fn parse_optional_with_names_ctx(
    children: &KdlDocument,
    module_ctx: &ParseCtx,
    defaults: StatesConfig,
) -> ConfigResult<(StatesConfig, Vec<String>)> {
    let Some(node) = fields::field(children, module_ctx, "states").optional_node()? else {
        return Ok((defaults, Vec::new()));
    };
    let Some(state_children) = node.children() else {
        return Ok((defaults, Vec::new()));
    };

    let states_ctx = module_ctx.child("states");
    let mut merged = defaults;
    let mut seen: Vec<String> = Vec::new();

    for (idx, child) in state_children.nodes().iter().enumerate() {
        let state_ctx = states_ctx.index(idx);
        let (name, threshold) = parse_state_node_ctx(child, &state_ctx)?;
        if seen.iter().any(|n| n == &name) {
            return Err(state_ctx.node_err(child, format!("duplicate state `{name}`"), None));
        }
        seen.push(name.clone());
        merged.upsert(name, threshold);
    }

    Ok((merged, seen))
}

fn parse_state_node_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<(String, u8)> {
    let node_name = node.name().value();
    if node_name != "state" {
        return Err(ctx.node_err(
            node,
            format!("expected node name `state`, got `{node_name}`"),
            None,
        ));
    }

    let positional: Vec<_> = node
        .entries()
        .iter()
        .filter(|entry| entry.name().is_none())
        .collect();

    let Some(name_entry) = positional.first() else {
        return Err(ctx.node_err(
            node,
            "expected a state name string",
            Some("add the state name as the first positional value".into()),
        ));
    };
    let Some(name) = name_entry.value().as_string() else {
        return Err(ctx.entry_err(name_entry, "expected a state name string", None));
    };
    if name.is_empty() {
        return Err(ctx.entry_err(name_entry, "state name must not be empty", None));
    }

    let threshold = fields::property(node, ctx, "threshold")
        .missing_message("missing `threshold` property")
        .integer_message("`threshold` must be an integer")
        .range_message(|value, min, max| {
            format!("`threshold` {value} out of range. Expected: integer {min}-{max}")
        })
        .u8_range(0, 100)
        .required()?;

    Ok((name.to_string(), threshold))
}
