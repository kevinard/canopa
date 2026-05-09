use ::kdl::KdlNode;

use super::super::actions;
use super::super::error::ParseCtx;
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

    if children.get("label").is_none() {
        return Err(ctx.child("label").node_err(
            node,
            "missing required parameter. Expected: a string value",
            None,
        ));
    }
    let label = required_string_node(
        children.get("label").expect("checked above"),
        &ctx.child("label"),
    )?;

    if children.get("icon").is_none() {
        return Err(ctx.child("icon").node_err(
            node,
            "missing required parameter. Expected: a string value",
            None,
        ));
    }
    let icon = required_string_node(
        children.get("icon").expect("checked above"),
        &ctx.child("icon"),
    )?;

    let on_click = if children.get("on-click").is_some() {
        actions::parse_command_node_ctx(
            children.get("on-click").expect("checked above"),
            &ctx.child("on-click"),
            command_policy,
        )?
    } else if let Some(command) = actions.click.as_ref() {
        command.clone()
    } else {
        return Err(ctx.child("on-click").node_err(
            node,
            "missing required parameter. Expected: a string value, argv block, or actions.click",
            None,
        ));
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

fn required_string_node(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<String> {
    let Some(entry) = node.entries().first() else {
        return Err(ctx.node_err(node, "missing value. Expected: a string value", None));
    };
    match entry.value().as_string() {
        Some("") => Err(ctx.entry_err(entry, "value must not be empty", None)),
        Some(value) => Ok(value.to_string()),
        None => Err(ctx.entry_err(entry, "expected a string value", None)),
    }
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod button_tests;
