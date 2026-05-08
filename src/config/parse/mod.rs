mod actions;
mod animation;
mod bars;
mod error;
mod kdl;
mod modules;
mod style;

use ::kdl::KdlDocument;

use super::{BarConfig, CommandConfig, CommandPolicy, Config, ModuleConfig, ModuleParams};
use crate::modules::ModuleKind;

/// Parse a KDL string into a fully-typed `Config`.
///
/// Missing sections fall back to defaults; constraint validation happens in `validate()`.
pub(super) fn parse(input: &str) -> anyhow::Result<Config> {
    let doc: KdlDocument = input.parse().map_err(|e| anyhow::anyhow!("config: {e}"))?;
    let bars = bars::parse(&doc)?;
    let bars = if bars.is_empty() {
        vec![BarConfig::default()]
    } else {
        bars
    };
    let modules = parse_modules_section(&doc)?;
    let command_policy = parse_command_policy(&doc)?;
    reject_disallowed_shell_commands(&modules, &command_policy)?;
    Ok(Config {
        bars,
        modules,
        command_policy,
    })
}

fn parse_command_policy(doc: &KdlDocument) -> anyhow::Result<CommandPolicy> {
    let Some(node) = doc.get("command-policy") else {
        return Ok(CommandPolicy::default());
    };
    let Some(children) = node.children() else {
        return Ok(CommandPolicy::default());
    };

    let mut policy = CommandPolicy::default();
    if let Some(value) = children.get_arg("allow-shell") {
        policy.allow_shell = value.as_bool().ok_or_else(|| {
            anyhow::anyhow!("command-policy.allow-shell: expected #true or #false.")
        })?;
    }
    if let Some(value) = children.get_arg("action-timeout-ms") {
        let timeout = value.as_integer().ok_or_else(|| {
            anyhow::anyhow!("command-policy.action-timeout-ms: expected a positive integer.")
        })?;
        policy.action_timeout_ms = u64::try_from(timeout).map_err(|_| {
            anyhow::anyhow!("command-policy.action-timeout-ms: expected a positive integer.")
        })?;
        if policy.action_timeout_ms == 0 {
            anyhow::bail!("command-policy.action-timeout-ms: expected a positive integer.");
        }
    }

    Ok(policy)
}

fn reject_disallowed_shell_commands(
    modules: &[ModuleConfig],
    policy: &CommandPolicy,
) -> anyhow::Result<()> {
    if policy.allow_shell {
        return Ok(());
    }

    for (idx, module) in modules.iter().enumerate() {
        if let Some(command) = module.actions.click.as_ref()
            && matches!(command, CommandConfig::Shell(_))
        {
            anyhow::bail!(
                "modules[{idx}].{}.actions.click: shell commands are disabled by command-policy.allow-shell #false; use an argv block.",
                module.module_type
            );
        }
        if let Some(command) = module.actions.right_click.as_ref()
            && matches!(command, CommandConfig::Shell(_))
        {
            anyhow::bail!(
                "modules[{idx}].{}.actions.right-click: shell commands are disabled by command-policy.allow-shell #false; use an argv block.",
                module.module_type
            );
        }

        match &module.params {
            ModuleParams::Script(config) if matches!(config.command, CommandConfig::Shell(_)) => {
                anyhow::bail!(
                    "modules[{idx}].script.command: shell commands are disabled by command-policy.allow-shell #false; use an argv block."
                );
            }
            ModuleParams::Button(config) if matches!(config.on_click, CommandConfig::Shell(_)) => {
                anyhow::bail!(
                    "modules[{idx}].button.on-click: shell commands are disabled by command-policy.allow-shell #false; use an argv block."
                );
            }
            _ => {}
        }
    }

    Ok(())
}

fn parse_modules_section(doc: &KdlDocument) -> anyhow::Result<Vec<ModuleConfig>> {
    let Some(modules_node) = doc.get("modules") else {
        return Ok(vec![]);
    };
    let Some(children) = modules_node.children() else {
        return Ok(vec![]);
    };

    let mut result = Vec::new();
    for (i, node) in children.nodes().iter().enumerate() {
        let node_name = node.name().value();
        if node_name != "module" {
            anyhow::bail!("modules[{i}]: expected node name \"module\", got {node_name:?}.");
        }
        let name = node
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "modules[{i}]: missing module name. \
                     Expected: a string argument, e.g. module \"clock\"."
                )
            })?
            .to_string();

        let explicit_type = node
            .children()
            .and_then(|c| c.get("type"))
            .and_then(|n| n.entries().first())
            .and_then(|e| e.value().as_string());
        let module_type = if super::is_known_config_module_type(&name) {
            if let Some(explicit_type) = explicit_type
                && explicit_type != name
            {
                anyhow::bail!(
                    "modules[{i}] \"{name}\": reserved built-in Module Name must keep Module Kind {name:?}, got {explicit_type:?}."
                );
            }
            name.clone()
        } else {
            explicit_type
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "modules[{i}] \"{name}\": name is not a known module type. \
                         Add a `type \"<module type>\"` child to specify the type. \
                         Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button, tray."
                    )
                })?
                .to_string()
        };

        let kind = ModuleKind::from_config_name(&module_type).ok_or_else(|| {
            anyhow::anyhow!(
                "{}: unknown module type {module_type:?}",
                error::module_path(i, &module_type)
            )
        })?;
        let actions = actions::parse(node, i, &module_type)?;
        let params = modules::dispatch(kind, node, i, &actions)?;
        result.push(ModuleConfig {
            name,
            module_type,
            params,
            actions,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
