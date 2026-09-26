use super::super::error::ParseCtx;
use crate::config::{ConfigResult, NotificationsConfig};
use ::kdl::KdlNode;

pub(super) fn parse_ctx(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<NotificationsConfig> {
    let Some(children) = node.children() else {
        return Ok(NotificationsConfig::default());
    };
    let fields = &[
        "type",
        "actions",
        "tooltip",
        "format",
        "expanded-format",
        "horizontal-format",
    ];
    ctx.validate_child_schema(children, fields, fields)?;
    let (format, tooltip) = super::formats::parse_format_and_tooltip_ctx(
        children,
        ctx,
        &["icon", "value", "label", "state"],
        &[],
    )?;
    Ok(NotificationsConfig { format, tooltip })
}
