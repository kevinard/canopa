//! Per-module parameter parsers.

use ::kdl::KdlNode;

use crate::config::BatteryConfig;

mod battery;

pub(super) fn parse_battery(node: &KdlNode, idx: usize) -> anyhow::Result<BatteryConfig> {
    battery::parse(node, idx)
}
