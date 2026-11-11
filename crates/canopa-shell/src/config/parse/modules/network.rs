use ::kdl::KdlNode;

use super::super::error::ParseCtx;
use super::super::fields;
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
            "content-rotation",
            "tooltip",
            "tooltip-id",
            "provider",
        ],
        &[
            "type",
            "actions",
            "icons",
            "format",
            "expanded-format",
            "horizontal-format",
            "content-rotation",
            "tooltip",
            "tooltip-id",
            "provider",
        ],
    )?;

    let icons = icons::parse_optional_ctx(children, ctx)?;
    let (format, tooltip) = formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "label", "state"],
        crate::config::states::NETWORK,
    )?;
    let content_rotation = formats::parse_content_rotation_ctx(children, ctx)?;
    let default_provider = parse_provider_setting(children, ctx)?;
    Ok(NetworkConfig {
        icons,
        format,
        content_rotation,
        tooltip,
        default_provider,
    })
}

fn parse_provider_setting(
    children: &::kdl::KdlDocument,
    ctx: &ParseCtx,
) -> ConfigResult<Option<String>> {
    let Some(node) = fields::field(children, ctx, "provider").optional_node()? else {
        return Ok(NetworkConfig::default().default_provider);
    };
    if node.children().is_some() || node.entries().len() != 1 {
        return Err(ctx.child("provider").node_err(
            node,
            "expected one provider name string or #false",
            None,
        ));
    }
    let entry = &node.entries()[0];
    if let Some(false) = entry.value().as_bool() {
        return Ok(None);
    }
    match entry.value().as_string() {
        Some("") => {
            Err(ctx
                .child("provider")
                .entry_err(entry, "provider name must not be empty", None))
        }
        Some(name) => Ok(Some(name.to_string())),
        None => Err(ctx.child("provider").entry_err(
            entry,
            "expected provider name string or #false",
            None,
        )),
    }
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod network_tests;
