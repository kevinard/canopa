use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::formats;
use super::icons;
use crate::config::{ConfigResult, NetworkConfig};

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<NetworkConfig> {
    let Some(children) = node.children() else {
        return Ok(NetworkConfig::default());
    };

    ctx.validate_child_schema(
        children,
        &[
            "type",
            "actions",
            "icons",
            "format",
            "expanded-format",
            "horizontal-format",
            "tooltip",
        ],
        &[
            "type",
            "actions",
            "icons",
            "format",
            "expanded-format",
            "horizontal-format",
            "tooltip",
        ],
    )?;

    let icons = icons::parse_optional_ctx(children, ctx)?;
    let (format, tooltip) = formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "label", "state"],
        crate::config::states::NETWORK,
    )?;
    Ok(NetworkConfig {
        icons,
        format,
        tooltip,
    })
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod network_tests;
