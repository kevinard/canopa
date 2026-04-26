use ::kdl::KdlNode;

use super::super::error;
use super::icons;
use crate::config::NetworkConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<NetworkConfig> {
    let Some(children) = node.children() else {
        return Ok(NetworkConfig::default());
    };
    let icons = icons::parse_optional(children, &error::module_path(idx, "network"))?;
    Ok(NetworkConfig { icons })
}
