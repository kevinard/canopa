use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::formats;
use crate::config::{ConfigResult, TrayConfig};

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<TrayConfig> {
    let Some(children) = node.children() else {
        return Ok(TrayConfig::default());
    };

    ctx.validate_child_schema(
        children,
        &[
            "type",
            "actions",
            "tooltip",
            "format",
            "expanded-format",
            "horizontal-format",
        ],
        &[
            "type",
            "actions",
            "tooltip",
            "format",
            "expanded-format",
            "horizontal-format",
        ],
    )?;
    formats::reject_format_fields_ctx(node.children(), ctx, "tray")?;

    let tooltip = formats::parse_tooltip_field_ctx(children, ctx, &["title", "description"], &[])?;
    Ok(TrayConfig { tooltip })
}

#[cfg(test)]
#[path = "tray_tests.rs"]
mod tray_tests;
