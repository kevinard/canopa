use ::kdl::KdlNode;

use super::super::error;
use super::formats;
use super::icons;
use crate::config::NetworkConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<NetworkConfig> {
    let Some(children) = node.children() else {
        return Ok(NetworkConfig::default());
    };
    let icons = icons::parse_optional(children, &error::module_path(idx, "network"))?;
    let format = formats::parse_optional(
        children,
        &error::module_path(idx, "network"),
        &["icon", "value", "label", "state"],
    )?;
    Ok(NetworkConfig { icons, format })
}
