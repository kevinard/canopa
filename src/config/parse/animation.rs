use ::kdl::KdlNode;

use super::super::AnimationConfig;

/// Parse an `animation { expand-ms N collapse-ms N }` KDL node into an `AnimationConfig`.
///
/// `prefix` is prepended in error messages (e.g. `"bar"` -> `"bar.animation.expand-ms: ..."`).
/// Returns `AnimationConfig::default()` when the node has no children block.
pub(super) fn parse_node(anim_node: &KdlNode, prefix: &str) -> anyhow::Result<AnimationConfig> {
    let Some(anim_children) = anim_node.children() else {
        return Ok(AnimationConfig::default());
    };
    let defaults = AnimationConfig::default();

    let expand_ms = match anim_children.get_arg("expand-ms") {
        None => defaults.expand_ms,
        Some(v) => {
            let n = v.as_integer().ok_or_else(|| {
                anyhow::anyhow!(
                    "{prefix}.animation.expand-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 280."
                )
            })?;
            u32::try_from(n).map_err(|_| {
                anyhow::anyhow!(
                    "{prefix}.animation.expand-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 280."
                )
            })? as f32
        }
    };

    let collapse_ms = match anim_children.get_arg("collapse-ms") {
        None => defaults.collapse_ms,
        Some(v) => {
            let n = v.as_integer().ok_or_else(|| {
                anyhow::anyhow!(
                    "{prefix}.animation.collapse-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 200."
                )
            })?;
            u32::try_from(n).map_err(|_| {
                anyhow::anyhow!(
                    "{prefix}.animation.collapse-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 200."
                )
            })? as f32
        }
    };

    Ok(AnimationConfig {
        expand_ms,
        collapse_ms,
    })
}
