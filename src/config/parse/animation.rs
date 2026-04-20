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

#[cfg(test)]
mod tests {
    use crate::config::parse::parse;

    #[test]
    fn parse_animation_expand_ms() {
        let config = parse(r#"bar "main" { animation { expand-ms 400 } }"#).unwrap();
        assert_eq!(config.bars[0].animation.expand_ms, 400.0);
        assert_eq!(config.bars[0].animation.collapse_ms, 200.0); // default unchanged
    }

    #[test]
    fn parse_animation_collapse_ms() {
        let config = parse(r#"bar "main" { animation { collapse-ms 100 } }"#).unwrap();
        assert_eq!(config.bars[0].animation.collapse_ms, 100.0);
        assert_eq!(config.bars[0].animation.expand_ms, 280.0); // default unchanged
    }

    #[test]
    fn parse_animation_defaults_when_omitted() {
        let config = parse("").unwrap();
        assert_eq!(config.bars[0].animation.expand_ms, 280.0);
        assert_eq!(config.bars[0].animation.collapse_ms, 200.0);
    }

    #[test]
    fn parse_animation_node_no_children_uses_defaults() {
        // `animation` node with no children block -> defaults
        let config = parse(r#"bar "main" { animation }"#).unwrap();
        assert_eq!(config.bars[0].animation.expand_ms, 280.0);
        assert_eq!(config.bars[0].animation.collapse_ms, 200.0);
    }

    #[test]
    fn parse_animation_negative_expand_ms_errors() {
        let result = parse(r#"bar "main" { animation { expand-ms -1 } }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(".animation.expand-ms"),
            "error should contain '.animation.expand-ms': {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "error should contain 'Expected:': {msg}"
        );
    }

    #[test]
    fn parse_animation_negative_collapse_ms_errors() {
        let result = parse(r#"bar "main" { animation { collapse-ms -1 } }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(".animation.collapse-ms"),
            "error should contain '.animation.collapse-ms': {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "error should contain 'Expected:': {msg}"
        );
    }
}
