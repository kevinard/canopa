//! Per-module parameter parsers.

use ::kdl::KdlNode;

use crate::config::{BatteryConfig, VolumeConfig};

mod battery;
mod volume;

pub(super) fn parse_battery(node: &KdlNode, idx: usize) -> anyhow::Result<BatteryConfig> {
    battery::parse(node, idx)
}

pub(super) fn parse_volume(node: &KdlNode, idx: usize) -> anyhow::Result<VolumeConfig> {
    volume::parse(node, idx)
}
