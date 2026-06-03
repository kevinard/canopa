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
            "menu-search",
        ],
        &[
            "type",
            "actions",
            "tooltip",
            "format",
            "expanded-format",
            "horizontal-format",
            "menu-search",
        ],
    )?;
    formats::reject_format_fields_ctx(node.children(), ctx, "tray")?;

    let tooltip = formats::parse_tooltip_field_ctx(children, ctx, &["title", "description"], &[])?;
    let menu_search = super::super::fields::field(children, ctx, "menu-search")
        .optional()
        .bool()
        .unwrap_or(false)?;
    Ok(TrayConfig {
        tooltip,
        menu_search,
    })
}

#[cfg(test)]
#[path = "tray_tests.rs"]
mod tray_tests;
