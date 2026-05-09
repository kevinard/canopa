use ::kdl::KdlNode;

use super::super::error::ParseCtx;
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
        &["icon", "value", "title"],
        crate::config::states::EMPTY,
    )?;
    Ok(FocusedWindowConfig { format, tooltip })
}

#[cfg(test)]
#[path = "focused_window_tests.rs"]
mod focused_window_tests;
