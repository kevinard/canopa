use ::kdl::KdlNode;

use super::super::error;
use super::formats;
use crate::config::FocusedWindowConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<FocusedWindowConfig> {
    let Some(children) = node.children() else {
        return Ok(FocusedWindowConfig::default());
    };
    let format = formats::parse_optional(
        children,
        &error::module_path(idx, "focused-window"),
        &["icon", "value", "title"],
    )?;
    Ok(FocusedWindowConfig { format })
}
