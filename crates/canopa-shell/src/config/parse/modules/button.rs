use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::super::fields;
use super::formats;
use crate::config::{
    ActionTrigger, ButtonConfig, CommandPolicy, ConfigResult, ModuleActionsConfig,
};

pub(super) fn parse_ctx(
    node: &KdlNode,
    actions: &ModuleActionsConfig,
    ctx: &ParseCtx,
    _command_policy: &CommandPolicy,
) -> ConfigResult<ButtonConfig> {
    let Some(children) = node.children() else {
        return Err(ctx.node_err(
            node,
            "missing required parameters block. Expected child nodes: label (or label-id), icon, actions",
            None,
        ));
    };

    ctx.validate_child_schema(
        children,
        &[
            "type",
            "label",
            "label-id",
            "icon",
            "actions",
            "format",
            "expanded-format",
            "horizontal-format",
            "content-rotation",
            "tooltip",
            "tooltip-id",
        ],
        &[
            "type",
            "label",
            "label-id",
            "icon",
            "actions",
            "format",
            "expanded-format",
            "horizontal-format",
            "content-rotation",
            "tooltip",
            "tooltip-id",
        ],
    )?;

    let label = fields::text_or_message(children, ctx, "label", "label-id")?.ok_or_else(|| {
        ctx.child("label").err(
            None,
            "missing required parameter. Expected: a string value",
            Some("set `label \"…\"`, or `label-id` for a built-in text".into()),
        )
    })?;

    let icon = fields::field(children, ctx, "icon").required_nonempty_string()?;

    if actions.action_for(&ActionTrigger::Click).is_none() {
        return Err(ctx.child("actions").child("click").node_err(
            node,
            "missing required action. Expected: actions { click { ... } }",
            None,
        ));
    }

    let (format, tooltip) = formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "label"],
        crate::config::states::EMPTY,
    )?;
    let content_rotation = formats::parse_content_rotation_ctx(children, ctx)?;

    Ok(ButtonConfig {
        label,
        icon,
        format,
        content_rotation,
        tooltip,
    })
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod button_tests;
