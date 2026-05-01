use ::kdl::KdlNode;

use crate::config::{CommandConfig, ModuleActionsConfig};

/// Parse a module's `actions { click "<cmd>" right-click { argv "cmd" } }` block.
///
/// String commands are executed via `sh -c "<cmd>"` for compatibility.
/// `argv` blocks keep program and argument boundaries explicit.
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
        let command = parse_command_node(action, &path)?;
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

pub(super) fn parse_command_node(node: &KdlNode, path: &str) -> anyhow::Result<CommandConfig> {
    let entries = node.entries();

    if entries.len() == 1 && node.children().is_none() {
        return match entries[0].value().as_string() {
            Some("") => anyhow::bail!("{path}: value must not be empty."),
            Some(command) => Ok(CommandConfig::Shell(command.to_string())),
            None => anyhow::bail!("{path}: expected a string value."),
        };
    }

    if !entries.is_empty() {
        anyhow::bail!("{path}: expected either one string value or an argv block.");
    }

    let Some(children) = node.children() else {
        anyhow::bail!("{path}: expected a string value.");
    };
    let nodes = children.nodes();
    if nodes.len() != 1 || nodes[0].name().value() != "argv" {
        anyhow::bail!("{path}: expected exactly one argv child command.");
    }

    let argv = &nodes[0];
    let mut values = Vec::new();
    for entry in argv.entries() {
        let Some(value) = entry.value().as_string() else {
            anyhow::bail!("{path}.argv: expected string arguments.");
        };
        values.push(value.to_string());
    }

    let Some(program) = values.first() else {
        anyhow::bail!("{path}.argv: missing program. Expected: argv \"program\" [\"arg\" ...].");
    };
    if program.is_empty() {
        anyhow::bail!("{path}.argv: program must not be empty.");
    }

    Ok(CommandConfig::Argv {
        program: program.clone(),
        args: values.into_iter().skip(1).collect(),
    })
}
