mod actions;
mod bars;
mod error;
mod fields;
mod modules;
mod style;

use ::kdl::{KdlDocument, KdlNode};

use super::{
    BarConfig, CommandPolicy, Config, MenuConfig, MenuItemConfig, MenuSourceConfig,
    MenuSourceKind, ModuleConfig,
};
pub(crate) use crate::config::ConfigSourceName;
use crate::config::{ConfigError, ConfigResult, ConfigSource};
use crate::modules::ModuleKind;
use error::ParseCtx;
use fields::field;

#[derive(Debug, Clone)]
pub(crate) struct ConfigInput<'a> {
    pub(crate) name: ConfigSourceName,
    pub(crate) text: &'a str,
}

/// Parse a KDL string into a fully-typed `Config`.
///
/// Missing sections fall back to defaults; constraint validation happens in `validate()`.
#[cfg(test)]
pub(super) fn parse(input: &str) -> ConfigResult<Config> {
    parse_input(ConfigInput {
        name: ConfigSourceName::test("<test config>"),
        text: input,
    })
}

pub(crate) fn parse_input(input: ConfigInput<'_>) -> ConfigResult<Config> {
    let source = ConfigSource::new(input.name, input.text);
    let ctx = ParseCtx::root(source.clone());
    let doc: KdlDocument = input
        .text
        .parse()
        .map_err(|e| ConfigError::from_kdl(source, e))?;
    let bars = bars::parse(&doc, &ctx)?;
    let bars = if bars.is_empty() {
        vec![BarConfig::default()]
    } else {
        bars
    };
    let command_policy = parse_command_policy(&doc, &ctx.child("command-policy"))?;
    let modules = parse_modules_section(&doc, &ctx, &command_policy)?;
    let menus = parse_menus_section(&doc, &ctx, &command_policy)?;
    Ok(Config {
        bars,
        modules,
        menus,
        command_policy,
    })
}

fn parse_command_policy(doc: &KdlDocument, ctx: &ParseCtx) -> ConfigResult<CommandPolicy> {
    let Some(node) = doc.get("command-policy") else {
        return Ok(CommandPolicy::default());
    };
    let Some(children) = node.children() else {
        return Ok(CommandPolicy::default());
    };

    ctx.validate_child_schema(
        children,
        &["allow-shell", "action-timeout-ms"],
        &["allow-shell", "action-timeout-ms"],
    )?;
    let defaults = CommandPolicy::default();
    let allow_shell = field(children, ctx, "allow-shell")
        .optional()
        .help("use `allow-shell #true` or `allow-shell #false`")
        .bool()
        .unwrap_or(defaults.allow_shell)?;
    let action_timeout_ms = field(children, ctx, "action-timeout-ms")
        .optional()
        .help("use `action-timeout-ms 3000` for a three-second timeout")
        .u64_min(1)
        .unwrap_or(defaults.action_timeout_ms)?;

    Ok(CommandPolicy {
        allow_shell,
        action_timeout_ms,
    })
}

fn parse_modules_section(
    doc: &KdlDocument,
    root_ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<Vec<ModuleConfig>> {
    let Some(modules_node) = doc.get("modules") else {
        return Ok(vec![]);
    };
    let Some(children) = modules_node.children() else {
        return Ok(vec![]);
    };

    let mut result = Vec::new();
    for (i, node) in children.nodes().iter().enumerate() {
        let node_name = node.name().value();
        let module_index_ctx = root_ctx.child("modules").index(i);
        if node_name != "module" {
            return Err(module_index_ctx.node_err(
                node,
                format!("expected node name \"module\", got {node_name:?}"),
                None,
            ));
        }
        let name = node
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                module_index_ctx.node_err(
                    node,
                    "missing module name",
                    Some("expected a string argument, e.g. `module \"clock\"`".into()),
                )
            })?
            .to_string();

        let explicit_type = match node.children() {
            Some(module_children) => field(module_children, &module_index_ctx, "type")
                .optional()
                .string()
                .transpose()?,
            None => None,
        };
        let module_type = if super::is_known_config_module_type(&name) {
            if let Some(explicit_type) = explicit_type.as_deref()
                && explicit_type != name.as_str()
            {
                return Err(module_index_ctx.node_err(
                    node,
                    format!(
                        "reserved built-in Module Name must keep Module Kind {name:?}, got {explicit_type:?}"
                    ),
                    None,
                ));
            }
            name.clone()
        } else {
            explicit_type
                .ok_or_else(|| {
                    module_index_ctx.node_err(
                        node,
                        format!("module name {name:?} is not a known module type"),
                        Some(
                            "add a `type \"<module type>\"` child; expected one of: battery, clock, network, volume, workspaces, focused-window, script, button, tray".into(),
                        ),
                    )
                })?
                .to_string()
        };

        let kind = ModuleKind::from_config_name(&module_type).ok_or_else(|| {
            module_index_ctx.node_err(
                node,
                format!("unknown module type {module_type:?}"),
                Some(
                    "expected one of: battery, clock, network, volume, workspaces, focused-window, script, button, tray".into(),
                ),
            )
        })?;
        let module_ctx = module_index_ctx.child(&module_type);
        let actions = actions::parse(node, i, &module_type, &module_ctx, command_policy)?;
        let params = modules::dispatch(kind, node, i, &actions, &module_ctx, command_policy)?;
        result.push(ModuleConfig {
            name,
            module_type,
            params,
            actions,
        });
    }
    Ok(result)
}

fn parse_menus_section(
    doc: &KdlDocument,
    root_ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<Vec<MenuConfig>> {
    let Some(menus_node) = doc.get("menus") else {
        return Ok(vec![]);
    };
    let Some(children) = menus_node.children() else {
        return Ok(vec![]);
    };

    let mut menus = Vec::new();
    for (menu_idx, node) in children.nodes().iter().enumerate() {
        let menu_ctx = root_ctx.child("menus").index(menu_idx);
        if node.name().value() != "menu" {
            return Err(menu_ctx.node_err(
                node,
                format!("expected node name \"menu\", got {:?}", node.name().value()),
                None,
            ));
        }

        let name = required_string_argument(node, &menu_ctx, "menu name")?;
        let mut source = None;
        let mut item_nodes = Vec::new();
        if let Some(menu_children) = node.children() {
            for child in menu_children.nodes() {
                match child.name().value() {
                    "source" => {
                        if source.is_some() {
                            return Err(menu_ctx.node_err(
                                node,
                                "menu can declare only one source",
                                None,
                            ));
                        }
                        source = Some(parse_menu_source(child, &menu_ctx.child("source"))?);
                    }
                    "item" => item_nodes.push(child),
                    other => {
                        return Err(menu_ctx.node_err(
                            child,
                            format!("expected node name \"item\" or \"source\", got {other:?}"),
                            None,
                        ));
                    }
                }
            }
        }

        if source.is_some() && !item_nodes.is_empty() {
            return Err(menu_ctx.node_err(node, "menu cannot mix source and item nodes", None));
        }

        let items = parse_menu_items_from_nodes(item_nodes, &menu_ctx, command_policy)?;
        menus.push(MenuConfig {
            name,
            source,
            items,
        });
    }
    Ok(menus)
}

fn parse_menu_source(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<MenuSourceConfig> {
    if node.children().is_some() || node.entries().iter().any(|entry| entry.name().is_some()) {
        return Err(ctx.node_err(node, "source options are not supported yet", None));
    }
    if node.entries().len() != 1 {
        return Err(ctx.node_err(node, "expected one menu source string", None));
    }
    let Some(kind) = node.entries()[0].value().as_string() else {
        return Err(ctx.entry_err(&node.entries()[0], "expected menu source string", None));
    };
    match kind {
        "installed-apps" => Ok(MenuSourceConfig {
            kind: MenuSourceKind::InstalledApps,
        }),
        other => Err(ctx.entry_err(
            &node.entries()[0],
            format!("unknown menu source {other:?}"),
            Some("expected `installed-apps`".into()),
        )),
    }
}

fn parse_menu_items_from_nodes(
    nodes: Vec<&KdlNode>,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<Vec<MenuItemConfig>> {
    let mut items = Vec::new();
    for (item_idx, node) in nodes.into_iter().enumerate() {
        let item_ctx = ctx.child("item").index(item_idx);
        if node.name().value() != "item" {
            return Err(item_ctx.node_err(
                node,
                format!("expected node name \"item\", got {:?}", node.name().value()),
                None,
            ));
        }
        items.push(parse_menu_item(node, &item_ctx, command_policy)?);
    }
    Ok(items)
}

fn parse_menu_item(
    node: &KdlNode,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<MenuItemConfig> {
    let label = required_string_argument(node, ctx, "item label")?;
    let Some(children) = node.children() else {
        return Ok(MenuItemConfig {
            label,
            keywords: Vec::new(),
            action: None,
            children: Vec::new(),
        });
    };

    let mut keywords = Vec::new();
    let mut action = None;
    let mut item_nodes = Vec::new();
    for child in children.nodes() {
        match child.name().value() {
            "keywords" => keywords = parse_keywords(child, &ctx.child("keywords"))?,
            "item" => item_nodes.push(child),
            operation => {
                if action.is_some() {
                    return Err(ctx.node_err(node, "expected exactly one action operation", None));
                }
                action = Some(parse_menu_action_operation(
                    child,
                    &ctx.child(operation),
                    command_policy,
                )?);
            }
        }
    }

    if action.is_some() && !item_nodes.is_empty() {
        return Err(ctx.node_err(
            node,
            "menu item cannot mix an action operation and child item nodes",
            None,
        ));
    }

    let mut children_items = Vec::new();
    for (idx, child) in item_nodes.into_iter().enumerate() {
        children_items.push(parse_menu_item(
            child,
            &ctx.child("item").index(idx),
            command_policy,
        )?);
    }

    Ok(MenuItemConfig {
        label,
        keywords,
        action,
        children: children_items,
    })
}

fn parse_menu_action_operation(
    node: &KdlNode,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<super::PanelAction> {
    match node.name().value() {
        "command" => {
            actions::parse_command_node_ctx(node, ctx, command_policy).map(super::PanelAction::Command)
        }
        "open-panel" => {
            require_no_arguments(node, ctx)?;
            Ok(super::PanelAction::OpenPanel)
        }
        "toggle-panel" => {
            require_no_arguments(node, ctx)?;
            Ok(super::PanelAction::TogglePanel)
        }
        "close-panel" => {
            require_no_arguments(node, ctx)?;
            Ok(super::PanelAction::ClosePanel)
        }
        "open-menu" => required_menu_name(node, ctx).map(super::PanelAction::OpenMenu),
        "toggle-menu" => required_menu_name(node, ctx).map(super::PanelAction::ToggleMenu),
        other => Err(ctx.node_err(
            node,
            format!("unknown menu item operation {other:?}"),
            Some(
                "expected `command`, `open-panel`, `toggle-panel`, `close-panel`, `open-menu`, or `toggle-menu`"
                    .into(),
            ),
        )),
    }
}

fn require_no_arguments(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<()> {
    if !node.entries().is_empty() || node.children().is_some() {
        return Err(ctx.node_err(node, "expected no arguments", None));
    }
    Ok(())
}

fn required_menu_name(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<String> {
    let value = required_string_argument(node, ctx, "menu name")?;
    Ok(value)
}

fn parse_keywords(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<Vec<String>> {
    let mut values = Vec::new();
    for entry in node.entries() {
        let Some(value) = entry.value().as_string() else {
            return Err(ctx.entry_err(entry, "expected string keyword", None));
        };
        values.push(value.to_string());
    }
    Ok(values)
}

fn required_string_argument(node: &KdlNode, ctx: &ParseCtx, label: &str) -> ConfigResult<String> {
    if node.entries().len() != 1 {
        return Err(ctx.node_err(node, format!("expected one {label} string"), None));
    }
    match node.entries()[0].value().as_string() {
        Some("") => Err(ctx.entry_err(
            &node.entries()[0],
            format!("{label} must not be empty"),
            None,
        )),
        Some(value) => Ok(value.to_string()),
        None => Err(ctx.entry_err(&node.entries()[0], "expected string", None)),
    }
}

#[cfg(test)]
mod tests;
