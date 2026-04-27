use ::kdl::KdlNode;

use super::super::error;
use super::formats;
use crate::config::TrayConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<TrayConfig> {
    formats::reject_format_fields(node.children(), &error::module_path(idx, "tray"), "tray")?;
    Ok(TrayConfig)
}
