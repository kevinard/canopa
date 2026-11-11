use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::super::fields;
use super::formats;
use super::icons;
use super::states;
use crate::config::{ConfigResult, StatesConfig, VolumeConfig};

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<VolumeConfig> {
    let Some(children) = node.children() else {
        return Ok(VolumeConfig::default());
    };

    ctx.validate_child_schema(
        children,
        &[
            "type",
            "actions",
            "step",
            "states",
            "icons",
            "format",
            "expanded-format",
            "horizontal-format",
            "content-rotation",
            "tooltip",
            "tooltip-id",
        ],
        &[
            "type",
            "actions",
            "step",
            "states",
            "icons",
            "format",
            "expanded-format",
            "horizontal-format",
            "content-rotation",
            "tooltip",
            "tooltip-id",
        ],
    )?;

    let defaults = VolumeConfig::default();
    let step = fields::field(children, ctx, "step")
        .optional()
        .u8_range(1, 100)
        .unwrap_or(defaults.step)?;
    let states_config = states::parse_optional_ctx(children, ctx, StatesConfig::volume_default())?;
    let icons = icons::parse_optional_ctx(children, ctx)?;

    let mut allowed_states: Vec<&str> = vec!["muted"];
    allowed_states.extend(states_config.names());
    let (format, tooltip) = formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "percent", "state"],
        &allowed_states,
    )?;
    let content_rotation = formats::parse_content_rotation_ctx(children, ctx)?;
    Ok(VolumeConfig {
        step,
        states: states_config,
        icons,
        format,
        content_rotation,
        tooltip,
    })
}

#[cfg(test)]
#[path = "volume_tests.rs"]
mod volume_tests;
