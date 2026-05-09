use ::kdl::KdlNode;

use super::super::AnimationConfig;
use super::error::ParseCtx;
use crate::config::ConfigResult;

/// Parse an `animation { expand-ms N collapse-ms N }` KDL node into an `AnimationConfig`.
///
/// `prefix` is prepended in error messages (e.g. `"bar"` -> `"bar.animation.expand-ms: ..."`).
/// Returns `AnimationConfig::default()` when the node has no children block.
pub(super) fn parse_node(anim_node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<AnimationConfig> {
    let Some(anim_children) = anim_node.children() else {
        return Ok(AnimationConfig::default());
    };
    ctx.validate_child_schema(
        anim_children,
        &["expand-ms", "collapse-ms"],
        &["expand-ms", "collapse-ms"],
    )?;
    let defaults = AnimationConfig::default();

    let expand_ms = optional_duration_ms(
        anim_children,
        "expand-ms",
        defaults.expand_ms,
        &ctx.child("expand-ms"),
        "a duration in milliseconds, e.g. 280",
    )?;

    let collapse_ms = optional_duration_ms(
        anim_children,
        "collapse-ms",
        defaults.collapse_ms,
        &ctx.child("collapse-ms"),
        "a duration in milliseconds, e.g. 200",
    )?;

    Ok(AnimationConfig {
        expand_ms,
        collapse_ms,
    })
}

fn optional_duration_ms(
    children: &::kdl::KdlDocument,
    name: &str,
    default: f32,
    ctx: &ParseCtx,
    expected: &str,
) -> ConfigResult<f32> {
    let Some(node) = children.get(name) else {
        return Ok(default);
    };
    let Some(entry) = node.entries().first() else {
        return Err(ctx.node_err(
            node,
            format!("expected a positive integer. Expected: {expected}"),
            Some(format!("expected {expected}")),
        ));
    };
    let Some(value) = entry.value().as_integer() else {
        return Err(ctx.entry_err(
            entry,
            format!("expected a positive integer. Expected: {expected}"),
            Some(format!("expected {expected}")),
        ));
    };
    let value = u32::try_from(value).map_err(|_| {
        ctx.entry_err(
            entry,
            format!("expected a positive integer. Expected: {expected}"),
            Some(format!("expected {expected}")),
        )
    })?;
    Ok(value as f32)
}

#[cfg(test)]
#[path = "animation_tests.rs"]
mod animation_tests;
