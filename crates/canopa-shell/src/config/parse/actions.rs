use ::kdl::KdlNode;

use crate::config::{
    ActionBinding, ActionTrigger, CommandPolicy, CommandPolicyError, ConfigResult,
    ConfiguredCommand, ModuleActionsConfig, PanelAction,
};

use super::error::ParseCtx;
use super::fields;

/// Parse a module's `actions { click { command "<cmd>" } Ctrl+E { open-panel } }` block.
pub(super) fn parse(
    node: &KdlNode,
    _idx: usize,
    _module_type: &str,
    module_ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<ModuleActionsConfig> {
    let Some(module_children) = node.children() else {
        return Ok(ModuleActionsConfig::default());
    };
    let Some(children) = fields::field(module_children, module_ctx, "actions").optional_node()?
    else {
        return Ok(ModuleActionsConfig::default());
    };
    let Some(action_children) = children.children() else {
        return Ok(ModuleActionsConfig::default());
    };

    let mut config = ModuleActionsConfig::default();
    for (action_idx, action) in action_children.nodes().iter().enumerate() {
        let action_name = action.name().value();
        let action_ctx = module_ctx.child("actions").child(action_name);
        let trigger = parse_trigger(
            action_name,
            &module_ctx.child("actions").index(action_idx),
            action,
        )?;
        if config.action_for(&trigger).is_some() {
            return Err(action_ctx.node_err(
                action,
                format!("duplicate {action_name} action"),
                None,
            ));
        }
        let action = parse_panel_action(action, &action_ctx, command_policy)?;
        config.bindings.push(ActionBinding { trigger, action });
    }

    Ok(config)
}

fn parse_trigger(name: &str, ctx: &ParseCtx, node: &KdlNode) -> ConfigResult<ActionTrigger> {
    match name {
        "click" => Ok(ActionTrigger::Click),
        "right-click" => Ok(ActionTrigger::RightClick),
        other if crate::surface::input::parse_keybind(other).is_some() => {
            Ok(ActionTrigger::Key(other.to_string()))
        }
        other => Err(ctx.node_err(
            node,
            format!("unknown action trigger {other:?}"),
            Some("expected `click`, `right-click`, or a keybind such as `Ctrl+E`".into()),
        )),
    }
}

fn parse_panel_action(
    action: &KdlNode,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<PanelAction> {
    let Some(children) = action.children() else {
        return Err(ctx.node_err(
            action,
            "expected exactly one action child",
            Some("expected one child such as `command \"date\"` or `open-panel`".into()),
        ));
    };
    let nodes = children.nodes();
    if nodes.len() != 1 {
        return Err(ctx.node_err(action, "expected exactly one action child", None));
    }

    let operation = &nodes[0];
    let operation_ctx = ctx.child(operation.name().value());
    match operation.name().value() {
        "command" => parse_command_node_ctx(operation, &operation_ctx, command_policy)
            .map(PanelAction::Command),
        "launch" => parse_launch(operation, &operation_ctx).map(PanelAction::Launch),
        "session" => parse_session(operation, &operation_ctx),
        "toggle-overview" => {
            require_no_arguments(operation, &operation_ctx)?;
            Ok(PanelAction::Compositor(
                crate::config::CompositorAction::ToggleOverview,
            ))
        }
        "open-panel" => {
            require_no_arguments(operation, &operation_ctx)?;
            Ok(PanelAction::OpenPanel)
        }
        "toggle-panel" => {
            require_no_arguments(operation, &operation_ctx)?;
            Ok(PanelAction::TogglePanel)
        }
        "open-bar-panel" => {
            require_no_arguments(operation, &operation_ctx)?;
            Ok(PanelAction::OpenBarPanel)
        }
        "toggle-bar-panel" => {
            require_no_arguments(operation, &operation_ctx)?;
            Ok(PanelAction::ToggleBarPanel)
        }
        "close-panel" => {
            require_no_arguments(operation, &operation_ctx)?;
            Ok(PanelAction::ClosePanel)
        }
        "open-menu" => required_menu_name(operation, &operation_ctx).map(PanelAction::OpenMenu),
        "toggle-menu" => {
            required_menu_name(operation, &operation_ctx).map(PanelAction::ToggleMenu)
        }
        "open-provider" => {
            required_provider_name(operation, &operation_ctx).map(PanelAction::OpenProvider)
        }
        other => Err(operation_ctx.node_err(
            operation,
            format!("unknown action operation {other:?}"),
            Some(
                "expected `command`, `launch`, `session`, `toggle-overview`, `open-panel`, `toggle-panel`, `open-bar-panel`, `toggle-bar-panel`, `close-panel`, `open-menu`, `toggle-menu`, or `open-provider`"
                    .into(),
            ),
        )),
    }
}

pub(super) fn parse_launch(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<Vec<String>> {
    if node.children().is_some() || node.entries().is_empty() {
        return Err(ctx.node_err(node, "expected `launch \"program\" [\"arg\" ...]`", None));
    }
    let mut argv = Vec::new();
    for entry in node.entries() {
        if entry.name().is_some() {
            return Err(ctx.entry_err(
                entry,
                "launch accepts only positional string arguments",
                None,
            ));
        }
        let Some(value) = entry.value().as_string() else {
            return Err(ctx.entry_err(entry, "expected string argument", None));
        };
        if value.contains('\0') || (argv.is_empty() && value.is_empty()) {
            return Err(ctx.entry_err(
                entry,
                "launch requires a non-empty program and arguments without NUL",
                None,
            ));
        }
        argv.push(value.to_owned());
    }
    Ok(argv)
}

/// `session "suspend"`: a logind request, never a shell command.
pub(super) fn parse_session(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<PanelAction> {
    let expected = || Some(format!("expected one of {}", crate::session::CONFIG_NAMES));
    if node.children().is_some() || node.entries().len() != 1 {
        return Err(ctx.node_err(node, "expected `session \"action\"`", expected()));
    }
    let entry = &node.entries()[0];
    match entry.value().as_string() {
        Some("log-out") if entry.name().is_none() => Ok(PanelAction::Compositor(
            crate::config::CompositorAction::LogOut,
        )),
        Some(name) if entry.name().is_none() => crate::session::from_config_name(name)
            .map(PanelAction::Session)
            .ok_or_else(|| {
                ctx.entry_err(
                    entry,
                    format!("unknown session action {name:?}"),
                    expected(),
                )
            }),
        _ => Err(ctx.entry_err(entry, "expected a session action string", expected())),
    }
}

fn require_no_arguments(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<()> {
    if !node.entries().is_empty() || node.children().is_some() {
        return Err(ctx.node_err(node, "expected exactly one action child", None));
    }
    Ok(())
}

fn required_menu_name(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<String> {
    if node.entries().len() != 1 {
        return Err(ctx.node_err(node, "expected one menu name string", None));
    }
    match node.entries()[0].value().as_string() {
        Some("") => Err(ctx.entry_err(&node.entries()[0], "menu name must not be empty", None)),
        Some(name) => Ok(name.to_string()),
        None => Err(ctx.entry_err(&node.entries()[0], "expected string", None)),
    }
}

fn required_provider_name(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<String> {
    if node.entries().len() != 1 {
        return Err(ctx.node_err(node, "expected one panel provider name string", None));
    }
    match node.entries()[0].value().as_string() {
        Some("") => Err(ctx.entry_err(
            &node.entries()[0],
            "panel provider name must not be empty",
            None,
        )),
        Some(name) => Ok(name.to_string()),
        None => Err(ctx.entry_err(&node.entries()[0], "expected string", None)),
    }
}

pub(super) fn parse_command_node_ctx(
    node: &KdlNode,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<ConfiguredCommand> {
    let command = parse_command_node_inner(node, ctx, command_policy)?;
    Ok(command)
}

fn parse_command_node_inner(
    node: &KdlNode,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<ConfiguredCommand> {
    let entries = node.entries();

    if entries.len() == 1 && node.children().is_none() {
        return match entries[0].value().as_string() {
            Some("") => Err(ctx.entry_err(&entries[0], "value must not be empty", None)),
            Some(command) => ConfiguredCommand::shell(command.to_string(), command_policy)
                .map_err(|err| match err {
                    CommandPolicyError::ShellDisabled => ctx.node_err(
                        node,
                        "shell commands are disabled by command-policy.allow-shell #false; use an argv block",
                        Some("use an argv block, e.g. `command { argv \"program\" \"arg\" }`".into()),
                    ),
                }),
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

    Ok(ConfiguredCommand::argv(
        program.clone(),
        values.into_iter().skip(1).collect(),
        command_policy,
    ))
}
