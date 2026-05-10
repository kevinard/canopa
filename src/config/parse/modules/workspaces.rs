use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::super::fields;
use super::formats;
use crate::config::{ConfigResult, WorkspaceLabelConfig, WorkspacesConfig};

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<WorkspacesConfig> {
    let mut config = WorkspacesConfig::default();

    let Some(children) = node.children() else {
        return Ok(config);
    };

    ctx.validate_child_schema(
        children,
        &[
            "type",
            "actions",
            "workspace",
            "format",
            "expanded-format",
            "horizontal-format",
            "tooltip",
        ],
        &[
            "type",
            "actions",
            "format",
            "expanded-format",
            "horizontal-format",
            "tooltip",
        ],
    )?;

    let (format, tooltip) = formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "index", "name", "label", "state"],
        crate::config::states::WORKSPACES,
    )?;
    config.format = format;
    config.tooltip = tooltip;

    let mut workspace_idx = 0;
    for ws_node in children.nodes() {
        if ws_node.name().value() != "workspace" {
            continue;
        }
        let workspace_ctx = ctx.child("workspace").index(workspace_idx);
        workspace_idx += 1;

        let ws_num = workspace_number(ws_node, &workspace_ctx)?;
        let (label, icon) = workspace_label_icon(ws_node, &workspace_ctx)?;
        config
            .workspaces
            .insert(ws_num, WorkspaceLabelConfig { label, icon });
    }

    Ok(config)
}

fn workspace_number(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<u32> {
    let Some(entry) = node.entries().iter().find(|entry| entry.name().is_none()) else {
        return Err(ctx.node_err(
            node,
            "missing workspace number",
            Some("expected a positive integer such as `workspace 1`".into()),
        ));
    };
    let Some(number) = entry.value().as_integer() else {
        return Err(ctx.entry_err(
            entry,
            "invalid workspace number",
            Some("expected a positive integer such as `workspace 1`".into()),
        ));
    };
    u32::try_from(number)
        .ok()
        .filter(|number| *number >= 1)
        .ok_or_else(|| {
            ctx.entry_err(
                entry,
                "invalid workspace number",
                Some("expected a positive integer such as `workspace 1`".into()),
            )
        })
}

fn workspace_label_icon(
    node: &KdlNode,
    ctx: &ParseCtx,
) -> ConfigResult<(Option<String>, Option<String>)> {
    let Some(children) = node.children() else {
        return Ok((None, None));
    };
    ctx.validate_child_schema(children, &["label", "icon"], &["label", "icon"])?;
    Ok((
        fields::field(children, ctx, "label")
            .optional()
            .string()
            .transpose()?,
        fields::field(children, ctx, "icon")
            .optional()
            .string()
            .transpose()?,
    ))
}

#[cfg(test)]
#[path = "workspaces_tests.rs"]
mod workspaces_tests;
