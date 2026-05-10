use ::kdl::KdlNode;

use super::super::AnimationConfig;
use super::error::ParseCtx;
use super::fields;
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

    let expand_ms = fields::field(anim_children, ctx, "expand-ms")
        .optional()
        .help("expected a duration in milliseconds, e.g. 280")
        .integer_message(
            "expected a positive integer. Expected: a duration in milliseconds, e.g. 280",
        )
        .u32()
        .map(|value| value as f32)
        .unwrap_or(defaults.expand_ms)?;

    let collapse_ms = fields::field(anim_children, ctx, "collapse-ms")
        .optional()
        .help("expected a duration in milliseconds, e.g. 200")
        .integer_message(
            "expected a positive integer. Expected: a duration in milliseconds, e.g. 200",
        )
        .u32()
        .map(|value| value as f32)
        .unwrap_or(defaults.collapse_ms)?;

    Ok(AnimationConfig {
        expand_ms,
        collapse_ms,
    })
}

#[cfg(test)]
#[path = "animation_tests.rs"]
mod animation_tests;
