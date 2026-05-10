use ::kdl::KdlNode;

use super::super::actions;
use super::super::error::ParseCtx;
use super::super::fields;
use super::formats;
use crate::config::{ButtonConfig, CommandPolicy, ConfigResult, ModuleActionsConfig};

pub(super) fn parse_ctx(
    node: &KdlNode,
    actions: &ModuleActionsConfig,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<ButtonConfig> {
    let Some(children) = node.children() else {
        return Err(ctx.node_err(
            node,
            "missing required parameters block. Expected child nodes: label, icon, on-click",
            None,
        ));
    };

    ctx.validate_child_schema(
        children,
        &[
            "type",
            "label",
            "icon",
            "on-click",
            "actions",
            "format",
            "expanded-format",
            "horizontal-format",
            "tooltip",
        ],
        &[
            "type",
            "label",
            "icon",
            "on-click",
            "actions",
            "format",
            "expanded-format",
            "horizontal-format",
            "tooltip",
        ],
    )?;

    let label = fields::field(children, ctx, "label").required_nonempty_string()?;

    let icon = fields::field(children, ctx, "icon").required_nonempty_string()?;

    let on_click = match fields::field(children, ctx, "on-click").optional_node()? {
        Some(on_click_node) => {
            actions::parse_command_node_ctx(on_click_node, &ctx.child("on-click"), command_policy)?
        }
        None if actions.click.is_some() => actions.click.clone().expect("checked above"),
        None => {
            return Err(ctx.child("on-click").node_err(
                node,
                "missing required parameter. Expected: a string value, argv block, or actions.click",
                None,
            ));
        }
    };
    let (format, tooltip) = formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "label"],
        crate::config::states::EMPTY,
    )?;

    Ok(ButtonConfig {
        label,
        icon,
        on_click,
        format,
        tooltip,
    })
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod button_tests;
