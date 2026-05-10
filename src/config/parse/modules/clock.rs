use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::super::fields;
use super::formats;
use crate::config::{ClockConfig, ConfigResult};

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<ClockConfig> {
    let Some(children) = node.children() else {
        return Ok(ClockConfig::default());
    };

    ctx.validate_child_schema(
        children,
        &[
            "type",
            "time-format",
            "date-format",
            "actions",
            "format",
            "expanded-format",
            "horizontal-format",
            "tooltip",
        ],
        &[
            "type",
            "time-format",
            "date-format",
            "actions",
            "format",
            "expanded-format",
            "horizontal-format",
            "tooltip",
        ],
    )?;

    let defaults = ClockConfig::default();

    let time_format = fields::field(children, ctx, "time-format")
        .optional()
        .help("expected a chrono format string such as \"%H:%M\"")
        .nonempty_string()
        .unwrap_or(defaults.time_format)?;

    let date_format = fields::field(children, ctx, "date-format")
        .optional()
        .help("expected a chrono format string such as \"%a %d %b\"")
        .nonempty_string()
        .unwrap_or(defaults.date_format)?;
    let (format, tooltip) = formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "time", "date"],
        crate::config::states::EMPTY,
    )?;

    Ok(ClockConfig {
        time_format,
        date_format,
        format,
        tooltip,
    })
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod clock_tests;
