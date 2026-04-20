use ::kdl::KdlNode;

use super::super::{error, kdl};
use crate::config::VolumeConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<VolumeConfig> {
    let Some(children) = node.children() else {
        return Ok(VolumeConfig::default());
    };
    let step = kdl::optional_u8_range_child(
        children,
        "step",
        VolumeConfig::default().step,
        1,
        100,
        &error::field_path(idx, "volume", "step"),
    )?;
    Ok(VolumeConfig { step })
}
