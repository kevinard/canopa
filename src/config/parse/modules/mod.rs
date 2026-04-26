//! Per-module parameter parsers and dispatch.

use ::kdl::KdlNode;

use super::error;
use crate::config::{ModuleActionsConfig, ModuleParams};
use crate::modules::ModuleKind;

mod battery;
mod button;
mod clock;
mod focused_window;
mod formats;
mod icons;
mod network;
mod script;
mod segment_styles;
mod tray;
mod volume;
mod workspaces;

pub(super) fn dispatch(
    kind: ModuleKind,
    node: &KdlNode,
    idx: usize,
    actions: &ModuleActionsConfig,
) -> anyhow::Result<ModuleParams> {
    match kind {
        ModuleKind::Battery => battery::parse(node, idx).map(ModuleParams::Battery),
        ModuleKind::Clock => clock::parse(node, idx).map(ModuleParams::Clock),
        ModuleKind::Network => network::parse(node, idx).map(ModuleParams::Network),
        ModuleKind::Volume => volume::parse(node, idx).map(ModuleParams::Volume),
        ModuleKind::Script => script::parse(node, idx).map(ModuleParams::Script),
        ModuleKind::Button => button::parse(node, idx, actions).map(ModuleParams::Button),
        ModuleKind::Workspaces => workspaces::parse(node, idx).map(ModuleParams::Workspaces),
        ModuleKind::FocusedWindow => {
            focused_window::parse(node, idx).map(ModuleParams::FocusedWindow)
        }
        ModuleKind::Tray => tray::parse(node, idx).map(ModuleParams::Tray),
        ModuleKind::WorkspaceItem | ModuleKind::TrayIcon => {
            anyhow::bail!(
                "{}: internal error: {:?} is runtime-only and cannot appear in config",
                error::module_path(idx, kind.as_str()),
                kind
            )
        }
    }
}
