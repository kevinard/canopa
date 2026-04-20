use ::kdl::KdlNode;

use super::super::error;
use crate::config::{WorkspaceLabelConfig, WorkspacesConfig};

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<WorkspacesConfig> {
    let mut config = WorkspacesConfig::default();

    let Some(children) = node.children() else {
        return Ok(config); // No workspace config — all workspaces show number only.
    };

    for ws_node in children.nodes() {
        if ws_node.name().value() != "workspace" {
            anyhow::bail!(
                "{}: unknown child node {:?}. Expected: \"workspace\".",
                error::module_path(idx, "workspaces"),
                ws_node.name().value()
            );
        }

        let ws_num = ws_node
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_integer())
            .and_then(|n| if n >= 1 { Some(n as u32) } else { None })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "{}: missing or invalid workspace number. \
                     Expected: a positive integer (1-based workspace number, e.g. 1).",
                    error::field_path(idx, "workspaces", "workspace")
                )
            })?;

        let (label, icon) = if let Some(ws_children) = ws_node.children() {
            let label = ws_children
                .get("label")
                .and_then(|n| n.entries().first())
                .and_then(|e| e.value().as_string())
                .map(|s| s.to_string());
            let icon = ws_children
                .get("icon")
                .and_then(|n| n.entries().first())
                .and_then(|e| e.value().as_string())
                .map(|s| s.to_string());
            (label, icon)
        } else {
            (None, None)
        };

        config
            .workspaces
            .insert(ws_num, WorkspaceLabelConfig { label, icon });
    }

    Ok(config)
}
