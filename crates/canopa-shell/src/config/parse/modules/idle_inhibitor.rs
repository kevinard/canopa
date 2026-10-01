use super::super::error::ParseCtx;
use crate::config::{ConfigResult, IdleInhibitorConfig};
use ::kdl::KdlNode;

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<IdleInhibitorConfig> {
    let Some(children) = node.children() else {
        return Ok(IdleInhibitorConfig::default());
    };
    let fields = &[
        "type",
        "actions",
        "tooltip",
        "icons",
        "format",
        "expanded-format",
        "horizontal-format",
        "content-rotation",
    ];
    ctx.validate_child_schema(children, fields, fields)?;
    let (format, tooltip) = super::formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "label", "state"],
        &["active", "inactive", "pending", "unavailable"],
    )?;
    Ok(IdleInhibitorConfig {
        icons: super::icons::parse_optional_ctx(children, ctx)?,
        format,
        tooltip,
        content_rotation: super::formats::parse_content_rotation_ctx(children, ctx)?,
    })
}
