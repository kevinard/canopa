//! Per-module parameter parsers.

use ::kdl::KdlNode;

use crate::config::{
    BatteryConfig, ButtonConfig, ClockConfig, NetworkConfig, ScriptConfig, TrayConfig,
    VolumeConfig, WorkspacesConfig,
};

mod battery;
mod button;
mod clock;
mod network;
mod script;
mod tray;
mod volume;
mod workspaces;

pub(super) fn parse_battery(node: &KdlNode, idx: usize) -> anyhow::Result<BatteryConfig> {
    battery::parse(node, idx)
}

pub(super) fn parse_button(node: &KdlNode, idx: usize) -> anyhow::Result<ButtonConfig> {
    button::parse(node, idx)
}

pub(super) fn parse_clock(node: &KdlNode, idx: usize) -> anyhow::Result<ClockConfig> {
    clock::parse(node, idx)
}

pub(super) fn parse_network(node: &KdlNode, idx: usize) -> anyhow::Result<NetworkConfig> {
    network::parse(node, idx)
}

pub(super) fn parse_script(node: &KdlNode, idx: usize) -> anyhow::Result<ScriptConfig> {
    script::parse(node, idx)
}

pub(super) fn parse_tray(node: &KdlNode, idx: usize) -> anyhow::Result<TrayConfig> {
    tray::parse(node, idx)
}

pub(super) fn parse_volume(node: &KdlNode, idx: usize) -> anyhow::Result<VolumeConfig> {
    volume::parse(node, idx)
}

pub(super) fn parse_workspaces(node: &KdlNode, idx: usize) -> anyhow::Result<WorkspacesConfig> {
    workspaces::parse(node, idx)
}
