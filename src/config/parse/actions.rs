use ::kdl::KdlNode;

use crate::config::ModuleActionsConfig;

pub(super) fn parse(
    node: &KdlNode,
    idx: usize,
    module_type: &str,
) -> anyhow::Result<ModuleActionsConfig> {
    let Some(children) = node.children().and_then(|children| children.get("actions")) else {
        return Ok(ModuleActionsConfig::default());
    };
    let Some(action_children) = children.children() else {
        return Ok(ModuleActionsConfig::default());
    };

    let mut config = ModuleActionsConfig::default();
    for (action_idx, action) in action_children.nodes().iter().enumerate() {
        let action_name = action.name().value();
        let path = format!("modules[{idx}].{module_type}.actions.{action_name}");
        let command = parse_action_command(action, &path)?;
        match action_name {
            "click" => {
                if config.click.replace(command).is_some() {
                    anyhow::bail!("{path}: duplicate click action.");
                }
            }
            "right-click" => {
                if config.right_click.replace(command).is_some() {
                    anyhow::bail!("{path}: duplicate right-click action.");
                }
            }
            other => {
                anyhow::bail!(
                    "modules[{idx}].{module_type}.actions[{action_idx}]: unknown action {other:?}. Expected: click or right-click."
                );
            }
        }
    }

    Ok(config)
}

fn parse_action_command(node: &KdlNode, path: &str) -> anyhow::Result<String> {
    let entries = node.entries();
    if entries.len() != 1 {
        anyhow::bail!("{path}: expected exactly one string value.");
    }

    match entries[0].value().as_string() {
        Some("") => anyhow::bail!("{path}: value must not be empty."),
        Some(command) => Ok(command.to_string()),
        None => anyhow::bail!("{path}: expected a string value."),
    }
}
