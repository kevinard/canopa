use ::kdl::KdlNode;

use super::super::{error, kdl};
use crate::config::ButtonConfig;

pub(super) fn parse(node: &KdlNode, idx: usize) -> anyhow::Result<ButtonConfig> {
    let button_path = error::module_path(idx, "button");
    let label_path = error::field_path(idx, "button", "label");
    let icon_path = error::field_path(idx, "button", "icon");
    let on_click_path = error::field_path(idx, "button", "on-click");

    let children = node.children().ok_or_else(|| {
        anyhow::anyhow!(
            "{button_path}: missing required parameters block. Expected child nodes: label, icon, on-click."
        )
    })?;

    if children.get("label").is_none() {
        anyhow::bail!("{label_path}: missing required parameter. Expected: a string value.");
    }
    let label = kdl::required_string_child(children, "label", &label_path)?;

    if children.get("icon").is_none() {
        anyhow::bail!("{icon_path}: missing required parameter. Expected: a string value.");
    }
    let icon = kdl::required_string_child(children, "icon", &icon_path)?;

    if children.get("on-click").is_none() {
        anyhow::bail!("{on_click_path}: missing required parameter. Expected: a string value.");
    }
    let on_click = kdl::required_string_child(children, "on-click", &on_click_path)?;

    Ok(ButtonConfig {
        label,
        icon,
        on_click,
    })
}
