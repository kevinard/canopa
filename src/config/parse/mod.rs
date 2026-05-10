mod actions;
mod animation;
mod bars;
mod error;
mod fields;
mod modules;
mod style;

use ::kdl::KdlDocument;

use super::{BarConfig, CommandPolicy, Config, ModuleConfig};
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
    Ok(Config {
        bars,
        modules,
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

#[cfg(test)]
mod tests;
