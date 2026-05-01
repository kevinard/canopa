use std::collections::{HashMap, HashSet};

use anyhow::Context;

use crate::config::{Config, ModuleActionsConfig, ModuleConfig};
use crate::modules::{MODULE_DESCRIPTORS, Module, ModuleDescriptor, Zone, build_registry};

pub(super) struct ModuleGraph {
    pub(super) modules: Vec<(Zone, Box<dyn Module>, ModuleActionsConfig)>,
}

pub(super) fn build_module_graph(config: &Config) -> anyhow::Result<ModuleGraph> {
    let registry = build_registry();
    let descriptors: HashMap<&str, &ModuleDescriptor> = MODULE_DESCRIPTORS
        .iter()
        .map(|descriptor| (descriptor.config_name, descriptor))
        .collect();
    let module_map: HashMap<&str, &ModuleConfig> = config
        .modules
        .iter()
        .map(|module_config| (module_config.name.as_str(), module_config))
        .collect();

    let mut ordered = Vec::new();
    let mut seen_names = HashSet::new();
    for bar in &config.bars {
        for (zone, names) in [
            (Zone::Top, &bar.modules.top),
            (Zone::Center, &bar.modules.center),
            (Zone::Bottom, &bar.modules.bottom),
        ] {
            for name in names {
                if seen_names.insert(name.clone()) {
                    ordered.push((zone.clone(), name.clone()));
                }
            }
        }
    }

    let mut modules = Vec::new();
    for (zone, name) in &ordered {
        let synthesized;
        let (module_config, descriptor, constructor) = if let Some(&module_config) =
            module_map.get(name.as_str())
        {
            let descriptor = descriptors
                .get(module_config.module_type.as_str())
                .copied()
                .with_context(|| format!("unknown module type {:?}", module_config.module_type))?;
            let constructor = registry
                .get(module_config.module_type.as_str())
                .copied()
                .with_context(|| {
                    format!("missing constructor for {:?}", module_config.module_type)
                })?;
            (module_config, descriptor, constructor)
        } else if let Some(descriptor) = descriptors.get(name.as_str()).copied() {
            let Some(params) = descriptor.synthesized_params() else {
                continue;
            };
            synthesized = ModuleConfig {
                name: name.clone(),
                module_type: descriptor.config_name.to_string(),
                params,
                actions: ModuleActionsConfig::default(),
            };
            (&synthesized, descriptor, descriptor.constructor)
        } else {
            continue;
        };

        let module = constructor(module_config)
            .with_context(|| format!("failed to construct module {:?}", module_config.name))?;
        debug_assert_eq!(module.module_type(), module.kind().as_str());
        debug_assert_eq!(module.kind(), descriptor.kind);
        modules.push((zone.clone(), module, module_config.actions.clone()));
    }

    Ok(ModuleGraph { modules })
}

#[cfg(test)]
mod tests {
    use crate::config::{Config, Edge};
    use crate::modules::{ModuleKind, Zone};

    #[test]
    fn referenced_builtin_is_synthesized() {
        let mut config = Config::default();
        config.bars[0].modules.bottom = vec!["clock".into()];
        let graph = super::build_module_graph(&config).unwrap();
        assert!(
            graph
                .modules
                .iter()
                .any(|(_, module, _)| module.kind() == ModuleKind::Clock)
        );
    }

    #[test]
    fn duplicate_module_name_uses_first_zone() {
        let mut config = Config::default();
        config.bars[0].modules.top = vec!["clock".into()];
        config.bars[0].modules.bottom = vec!["clock".into()];
        config.bars[0].edge = Edge::Left;

        let graph = super::build_module_graph(&config).unwrap();
        let clock = graph
            .modules
            .iter()
            .find(|(_, module, _)| module.kind() == ModuleKind::Clock)
            .unwrap();
        assert_eq!(clock.0, Zone::Top);
    }
}
