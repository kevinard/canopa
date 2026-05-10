use ::kdl::KdlNode;

use super::super::actions;
use super::super::error::ParseCtx;
use super::super::fields;
use super::formats;
use crate::config::{CommandPolicy, ConfigResult, ScriptConfig};

pub(super) fn parse(
    node: &KdlNode,
    idx: usize,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<ScriptConfig> {
    let Some(children) = node.children() else {
        return Err(ctx.child("command").node_err(
            node,
            "missing required parameter",
            Some("expected `command \"date\"` or `command { argv \"date\" }`".into()),
        ));
    };

    ctx.validate_child_schema(
        children,
        &[
            "type",
            "command",
            "interval",
            "actions",
            "format",
            "expanded-format",
            "horizontal-format",
            "tooltip",
        ],
        &[
            "type",
            "command",
            "interval",
            "actions",
            "format",
            "expanded-format",
            "horizontal-format",
            "tooltip",
        ],
    )?;

    let command_node = fields::field(children, ctx, "command").required_node(
        "missing required parameter",
        Some("expected `command \"date\"` or `command { argv \"date\" }`".into()),
    )?;
    let command =
        actions::parse_command_node_ctx(command_node, &ctx.child("command"), command_policy)?;

    let interval = fields::field(children, ctx, "interval")
        .optional()
        .help("use `interval 5` for a five-second polling interval")
        .u64_min(1)
        .unwrap_or(5)?;

    let _ = idx;
    let (format, tooltip) = formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "output"],
        crate::config::states::EMPTY,
    )?;

    Ok(ScriptConfig {
        command,
        interval,
        format,
        tooltip,
    })
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod script_tests;
