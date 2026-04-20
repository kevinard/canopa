use ::kdl::KdlNode;

use super::super::{error, kdl};
use crate::config::BatteryConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<BatteryConfig> {
    // Optional `critical-threshold <int>` child node.
    let Some(children) = node.children() else {
        return Ok(BatteryConfig::default());
    };
    let critical_threshold = kdl::optional_u8_range_child(
        children,
        "critical-threshold",
        BatteryConfig::default().critical_threshold,
        0,
        100,
        &error::field_path(idx, "battery", "critical-threshold"),
    )?;
    Ok(BatteryConfig { critical_threshold })
}
