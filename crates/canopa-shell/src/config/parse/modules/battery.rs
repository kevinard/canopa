use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::super::fields;
use super::formats;
use super::icons;
use super::states;
use crate::config::{BatteryConfig, ConfigResult, StatesConfig};

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<BatteryConfig> {
    let Some(children) = node.children() else {
        return Ok(BatteryConfig::default());
    };

    ctx.validate_child_schema(
        children,
        &[
            "type",
            "actions",
            "critical-threshold",
            "states",
            "icons",
            "format",
            "expanded-format",
            "horizontal-format",
            "content-rotation",
            "tooltip",
        ],
        &[
            "type",
            "actions",
            "critical-threshold",
            "states",
            "icons",
            "format",
            "expanded-format",
            "horizontal-format",
            "content-rotation",
            "tooltip",
        ],
    )?;

    let (mut states_config, user_state_names) =
        states::parse_optional_with_names_ctx(children, ctx, StatesConfig::battery_default())?;

    let critical_override = fields::field(children, ctx, "critical-threshold")
        .optional()
        .u8_range(0, 100)
        .transpose()?;

    if let Some(t) = critical_override
        && !user_state_names.iter().any(|n| n == "critical")
    {
        states_config.upsert("critical".into(), t);
    }

    let critical_threshold = states_config.threshold_of("critical").unwrap_or(15);
    let icons = icons::parse_optional_ctx(children, ctx)?;

    let mut allowed_states: Vec<&str> = vec![
        "charging",
        "full",
        "pending-charge",
        "pending-discharge",
        "unknown",
    ];
    allowed_states.extend(states_config.names());
    let (format, tooltip) = formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "percent", "state"],
        &allowed_states,
    )?;
    let content_rotation = formats::parse_content_rotation_ctx(children, ctx)?;

    Ok(BatteryConfig {
        critical_threshold,
        states: states_config,
        icons,
        format,
        content_rotation,
        tooltip,
    })
}

#[cfg(test)]
#[path = "battery_tests.rs"]
mod battery_tests;
