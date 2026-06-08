use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::super::fields;
use super::formats;
use crate::config::{ConfigResult, FocusedWindowConfig};

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<FocusedWindowConfig> {
    let Some(children) = node.children() else {
        return Ok(FocusedWindowConfig::default());
    };

    ctx.validate_child_schema(
        children,
        &[
            "type",
            "actions",
            "format",
            "expanded-format",
            "horizontal-format",
            "content-rotation",
            "fallback-icon",
            "tooltip",
        ],
        &[
            "type",
            "actions",
            "format",
            "expanded-format",
            "horizontal-format",
            "content-rotation",
            "fallback-icon",
            "tooltip",
        ],
    )?;

    let defaults = FocusedWindowConfig::default();
    let (parsed_format, tooltip) = formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "title", "app-id"],
        crate::config::states::EMPTY,
    )?;
    let mut format = defaults.format;
    if parsed_format.collapsed.is_some() {
        format.collapsed = parsed_format.collapsed;
    }
    if parsed_format.expanded.is_some() {
        format.expanded = parsed_format.expanded;
    }
    if parsed_format.horizontal.is_some() {
        format.horizontal = parsed_format.horizontal;
    }
    let content_rotation =
        formats::parse_content_rotation_with_default_ctx(children, ctx, defaults.content_rotation)?;
    let fallback_icon = fields::field(children, ctx, "fallback-icon")
        .optional()
        .nonempty_string()
        .unwrap_or(defaults.fallback_icon)?;
    Ok(FocusedWindowConfig {
        format,
        content_rotation,
        fallback_icon,
        tooltip,
    })
}

#[cfg(test)]
#[path = "focused_window_tests.rs"]
mod focused_window_tests;
