use ::kdl::KdlNode;

use crate::config::NetworkConfig;

pub(super) fn parse(_node: &KdlNode, _idx: usize) -> anyhow::Result<NetworkConfig> {
    Ok(NetworkConfig)
}
