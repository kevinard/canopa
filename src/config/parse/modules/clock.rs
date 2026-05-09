use ::kdl::KdlNode;

use super::super::error::ParseCtx;
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

    let time_format = optional_nonempty_string_child(
        children,
        "time-format",
        defaults.time_format,
        &ctx.child("time-format"),
        "a chrono format string such as \"%H:%M\"",
    )?;

    let date_format = optional_nonempty_string_child(
        children,
        "date-format",
        defaults.date_format,
        &ctx.child("date-format"),
        "a chrono format string such as \"%a %d %b\"",
    )?;
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

fn optional_nonempty_string_child(
    children: &::kdl::KdlDocument,
    name: &str,
    default: String,
    ctx: &ParseCtx,
    expected: &str,
) -> ConfigResult<String> {
    let Some(node) = children.get(name) else {
        return Ok(default);
    };
    let Some(entry) = node.entries().first() else {
        return Err(ctx.node_err(
            node,
            "expected a string value",
            Some(format!("expected {expected}")),
        ));
    };
    match entry.value().as_string() {
        Some("") => Err(ctx.entry_err(
            entry,
            format!("value must not be empty. Expected: {expected}"),
            None,
        )),
        Some(value) => Ok(value.to_string()),
        None => Err(ctx.entry_err(
            entry,
            "expected a string value",
            Some(format!("expected {expected}")),
        )),
    }
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod clock_tests;
