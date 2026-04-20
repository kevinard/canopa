use ::kdl::KdlNode;

use crate::config::TrayConfig;

pub(super) fn parse(_node: &KdlNode, _idx: usize) -> anyhow::Result<TrayConfig> {
    Ok(TrayConfig)
}
