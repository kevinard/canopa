//! Per-module parameter parsers.

use ::kdl::KdlNode;

use crate::config::{BatteryConfig, ClockConfig, VolumeConfig};

mod battery;
mod clock;
mod volume;

pub(super) fn parse_battery(node: &KdlNode, idx: usize) -> anyhow::Result<BatteryConfig> {
    battery::parse(node, idx)
}

pub(super) fn parse_clock(node: &KdlNode, idx: usize) -> anyhow::Result<ClockConfig> {
    clock::parse(node, idx)
}

pub(super) fn parse_volume(node: &KdlNode, idx: usize) -> anyhow::Result<VolumeConfig> {
    volume::parse(node, idx)
}
