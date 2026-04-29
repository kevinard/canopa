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
mod states;
mod tray;
mod volume;
mod workspaces;

type ModuleParamParser = fn(&KdlNode, usize, &ModuleActionsConfig) -> anyhow::Result<ModuleParams>;

pub(super) fn parser_for(kind: ModuleKind) -> Option<ModuleParamParser> {
    match kind {
        ModuleKind::Battery => {
            Some(|node, idx, _| battery::parse(node, idx).map(ModuleParams::Battery))
        }
        ModuleKind::Clock => Some(|node, idx, _| clock::parse(node, idx).map(ModuleParams::Clock)),
        ModuleKind::Network => {
            Some(|node, idx, _| network::parse(node, idx).map(ModuleParams::Network))
        }
        ModuleKind::Volume => {
            Some(|node, idx, _| volume::parse(node, idx).map(ModuleParams::Volume))
        }
        ModuleKind::Script => {
            Some(|node, idx, _| script::parse(node, idx).map(ModuleParams::Script))
        }
        ModuleKind::Button => {
            Some(|node, idx, actions| button::parse(node, idx, actions).map(ModuleParams::Button))
        }
        ModuleKind::Workspaces => {
            Some(|node, idx, _| workspaces::parse(node, idx).map(ModuleParams::Workspaces))
        }
        ModuleKind::FocusedWindow => {
            Some(|node, idx, _| focused_window::parse(node, idx).map(ModuleParams::FocusedWindow))
        }
        ModuleKind::Tray => Some(|node, idx, _| tray::parse(node, idx).map(ModuleParams::Tray)),
        ModuleKind::WorkspaceItem | ModuleKind::TrayIcon => None,
    }
}

pub(super) fn dispatch(
    kind: ModuleKind,
    node: &KdlNode,
    idx: usize,
    actions: &ModuleActionsConfig,
) -> anyhow::Result<ModuleParams> {
    let Some(parser) = parser_for(kind) else {
        anyhow::bail!(
            "{}: internal error: {:?} is runtime-only and cannot appear in config",
            error::module_path(idx, kind.runtime_name()),
            kind
        );
    };
    parser(node, idx, actions)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_config_descriptor_has_parser() {
        for descriptor in crate::modules::MODULE_DESCRIPTORS {
            assert!(
                super::parser_for(descriptor.kind).is_some(),
                "missing parser for {:?}",
                descriptor.kind
            );
        }
    }
}
