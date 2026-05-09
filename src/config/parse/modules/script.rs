use ::kdl::KdlNode;

use super::super::actions;
use super::super::error::ParseCtx;
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

    if children.get("command").is_none() {
        return Err(ctx.child("command").node_err(
            node,
            "missing required parameter",
            Some("expected `command \"date\"` or `command { argv \"date\" }`".into()),
        ));
    }
    let command_node = children.get("command").expect("checked above");
    let command =
        actions::parse_command_node_ctx(command_node, &ctx.child("command"), command_policy)?;

    let interval = match children.get("interval") {
        None => 5u64,
        Some(n) => ctx.child("interval").required_u64_node(
            n,
            1,
            "use `interval 5` for a five-second polling interval",
        )?,
    };

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
