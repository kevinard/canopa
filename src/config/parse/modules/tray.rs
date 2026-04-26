use ::kdl::KdlNode;

use super::super::error;
use super::formats;
use crate::config::TrayConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<TrayConfig> {
    formats::reject_if_present(node.children(), &error::module_path(idx, "tray"))?;
    Ok(TrayConfig)
}
