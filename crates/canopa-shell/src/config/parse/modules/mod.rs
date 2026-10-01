//! Per-module parameter parsers and dispatch.

use ::kdl::KdlNode;

use super::error::ParseCtx;
use crate::config::{CommandPolicy, ConfigResult};
use crate::config::{ModuleActionsConfig, ModuleParams};
use crate::modules::ModuleKind;

mod battery;
mod button;
mod clock;
mod focused_window;
mod formats;
mod icons;
mod idle_inhibitor;
mod network;
mod notifications;
mod script;
mod segment_styles;
mod states;
mod tray;
mod volume;
mod workspaces;

pub(super) fn dispatch(
    kind: ModuleKind,
    node: &KdlNode,
    idx: usize,
    actions: &ModuleActionsConfig,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<ModuleParams> {
    match kind {
        ModuleKind::IdleInhibitor => {
            idle_inhibitor::parse_ctx(node, ctx).map(ModuleParams::IdleInhibitor)
        }
        ModuleKind::Script => {
            script::parse(node, idx, ctx, command_policy).map(ModuleParams::Script)
        }
        ModuleKind::Battery => battery::parse_ctx(node, ctx).map(ModuleParams::Battery),
        ModuleKind::Button => {
            button::parse_ctx(node, actions, ctx, command_policy).map(ModuleParams::Button)
        }
        ModuleKind::Clock => clock::parse_ctx(node, ctx).map(ModuleParams::Clock),
        ModuleKind::Notifications => {
            notifications::parse_ctx(node, ctx).map(ModuleParams::Notifications)
        }
        ModuleKind::Network => network::parse_ctx(node, ctx).map(ModuleParams::Network),
        ModuleKind::Volume => volume::parse_ctx(node, ctx).map(ModuleParams::Volume),
        ModuleKind::Workspaces => workspaces::parse_ctx(node, ctx).map(ModuleParams::Workspaces),
        ModuleKind::FocusedWindow => {
            focused_window::parse_ctx(node, ctx).map(ModuleParams::FocusedWindow)
        }
        ModuleKind::Tray => tray::parse_ctx(node, ctx).map(ModuleParams::Tray),
        ModuleKind::WorkspaceItem | ModuleKind::TrayIcon => Err(ctx.node_err(
            node,
            format!(
                "internal error: {:?} is runtime-only and cannot appear in config",
                kind
            ),
            None,
        )),
    }
}

#[cfg(test)]
mod tests {
    use crate::modules::ModuleKind;

    fn has_config_parser(kind: ModuleKind) -> bool {
        matches!(
            kind,
            ModuleKind::Battery
                | ModuleKind::IdleInhibitor
                | ModuleKind::Clock
                | ModuleKind::Notifications
                | ModuleKind::Network
                | ModuleKind::Volume
                | ModuleKind::Script
                | ModuleKind::Button
                | ModuleKind::Workspaces
                | ModuleKind::FocusedWindow
                | ModuleKind::Tray
        )
    }

    #[test]
    fn every_config_descriptor_has_parser() {
        for descriptor in crate::modules::MODULE_DESCRIPTORS {
            assert!(
                has_config_parser(descriptor.kind),
                "missing parser for {:?}",
                descriptor.kind
            );
        }
    }
}
