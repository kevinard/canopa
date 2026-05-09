use ::kdl::KdlNode;

use crate::config::{CommandConfig, CommandPolicy, ConfigResult, ModuleActionsConfig};

use super::error::ParseCtx;

/// Parse a module's `actions { click "<cmd>" right-click { argv "cmd" } }` block.
///
/// String commands are executed via `sh -c "<cmd>"` for compatibility.
/// `argv` blocks keep program and argument boundaries explicit.
pub(super) fn parse(
    node: &KdlNode,
    _idx: usize,
    _module_type: &str,
    module_ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<ModuleActionsConfig> {
    let Some(children) = node.children().and_then(|children| children.get("actions")) else {
        return Ok(ModuleActionsConfig::default());
    };
    let Some(action_children) = children.children() else {
        return Ok(ModuleActionsConfig::default());
    };

    let mut config = ModuleActionsConfig::default();
    for (action_idx, action) in action_children.nodes().iter().enumerate() {
        let action_name = action.name().value();
        let action_ctx = module_ctx.child("actions").child(action_name);
        let command = parse_command_node_ctx(action, &action_ctx, command_policy)?;
        match action_name {
            "click" => {
                if config.click.replace(command).is_some() {
                    return Err(action_ctx.node_err(action, "duplicate click action", None));
                }
            }
            "right-click" => {
                if config.right_click.replace(command).is_some() {
                    return Err(action_ctx.node_err(action, "duplicate right-click action", None));
                }
            }
            other => {
                return Err(module_ctx.child("actions").index(action_idx).node_err(
                    action,
                    format!("unknown action {other:?}"),
                    Some("expected `click` or `right-click`".into()),
                ));
            }
        }
    }

    Ok(config)
}

pub(super) fn parse_command_node_ctx(
    node: &KdlNode,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<CommandConfig> {
    let command = parse_command_node_inner(node, ctx)?;
    if !command_policy.allow_shell && matches!(command, CommandConfig::Shell(_)) {
        return Err(ctx.node_err(
            node,
            "shell commands are disabled by command-policy.allow-shell #false; use an argv block",
            Some("use an argv block, e.g. `command { argv \"program\" \"arg\" }`".into()),
        ));
    }
    Ok(command)
}

fn parse_command_node_inner(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<CommandConfig> {
    let entries = node.entries();

    if entries.len() == 1 && node.children().is_none() {
        return match entries[0].value().as_string() {
            Some("") => Err(ctx.entry_err(&entries[0], "value must not be empty", None)),
            Some(command) => Ok(CommandConfig::Shell(command.to_string())),
            None => Err(ctx.entry_err(&entries[0], "expected string", None)),
        };
    }

    if !entries.is_empty() {
        return Err(ctx.node_err(
            node,
            "expected either one string value or an argv block",
            Some("use `command \"date\"` or `command { argv \"date\" }`".into()),
        ));
    }

    let Some(children) = node.children() else {
        return Err(ctx.node_err(
            node,
            "missing value; expected a string value or argv block",
            None,
        ));
    };
    let nodes = children.nodes();
    if nodes.len() != 1 || nodes[0].name().value() != "argv" {
        return Err(ctx.node_err(
            node,
            "expected exactly one argv child command",
            Some("use `argv \"program\" [\"arg\" ...]`".into()),
        ));
    }

    let argv = &nodes[0];
    let argv_ctx = ctx.child("argv");
    let mut values = Vec::new();
    for entry in argv.entries() {
        let Some(value) = entry.value().as_string() else {
            return Err(argv_ctx.entry_err(entry, "expected string arguments", None));
        };
        values.push(value.to_string());
    }

    let Some(program) = values.first() else {
        return Err(argv_ctx.node_err(
            argv,
            "missing program",
            Some("expected `argv \"program\" [\"arg\" ...]`".into()),
        ));
    };
    if program.is_empty() {
        return Err(argv_ctx.node_err(argv, "program must not be empty", None));
    }

    Ok(CommandConfig::Argv {
        program: program.clone(),
        args: values.into_iter().skip(1).collect(),
    })
}
