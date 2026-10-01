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
            "content-rotation",
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
            "content-rotation",
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
    for (name, format) in [("time-format", &time_format), ("date-format", &date_format)] {
        if let Some(field) = children.get(name)
            && chrono::DateTime::from_timestamp(1_700_000_000, 0)
                .expect("valid validation timestamp")
                .format(format)
                .write_to(&mut String::new())
                .is_err()
        {
            // Exercise Chrono's formatter as well as its parser: parse-only
            // directives such as %#z are not valid clock output formats.
            return Err(ctx.child(name).node_err(
                field,
                "invalid clock format directive",
                Some(
                    "use Chrono output directives such as %H:%M, or %% for a literal percent"
                        .into(),
                ),
            ));
        }
    }
    let (format, tooltip) = formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "time", "date"],
        crate::config::states::EMPTY,
    )?;
    let content_rotation = formats::parse_content_rotation_ctx(children, ctx)?;

    Ok(ClockConfig {
        time_format,
        date_format,
        format,
        content_rotation,
        tooltip,
    })
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod clock_tests;
