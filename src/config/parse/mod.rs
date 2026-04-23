mod animation;
mod bars;
mod error;
mod kdl;
mod modules;

use ::kdl::KdlDocument;

use super::{BarConfig, Config, ModuleConfig};
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
    Ok(Config { bars, modules })
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

        let module_type = if super::is_known_config_module_type(&name) {
            name.clone()
        } else {
            node.children()
                .and_then(|c| c.get("type"))
                .and_then(|n| n.entries().first())
                .and_then(|e| e.value().as_string())
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
        let params = modules::dispatch(kind, node, i)?;
        result.push(ModuleConfig {
            name,
            module_type,
            params,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
