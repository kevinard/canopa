use std::collections::{HashMap, HashSet};

use anyhow::Context;

use crate::config::{ActionBinding, ActionTrigger, Config, ModuleActionsConfig, ModuleConfig};
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
    let panel_provider_catalog =
        crate::external_panel::PanelProviderCatalog::new(&config.panel_providers);

    let mut ordered = Vec::new();
    let mut seen_names = HashSet::new();
    for bar in &config.bars {
        for (zone, names) in [
            (Zone::Start, &bar.modules.start),
            (Zone::Center, &bar.modules.center),
            (Zone::End, &bar.modules.end),
            (Zone::Start, &bar.panel.modules.start),
            (Zone::Center, &bar.panel.modules.center),
            (Zone::End, &bar.panel.modules.end),
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
            let params = descriptor
                .synthesized_params()
                .with_context(|| format!("module {name:?} must be declared before placement"))?;
            synthesized = ModuleConfig {
                name: name.clone(),
                module_type: descriptor.config_name.to_string(),
                params,
                actions: ModuleActionsConfig::default(),
            };
            (&synthesized, descriptor, descriptor.constructor)
        } else {
            anyhow::bail!("unknown module placement {name:?}");
        };

        let module = constructor(module_config)
            .with_context(|| format!("failed to construct module {:?}", module_config.name))?;
        debug_assert_eq!(module.module_type(), module.kind().as_str());
        debug_assert_eq!(module.kind(), descriptor.kind);
        let actions = effective_module_actions(module_config, descriptor, &panel_provider_catalog);
        modules.push((zone.clone(), module, actions));
    }

    Ok(ModuleGraph { modules })
}

fn effective_module_actions(
    module_config: &ModuleConfig,
    descriptor: &ModuleDescriptor,
    panel_provider_catalog: &crate::external_panel::PanelProviderCatalog<'_>,
) -> ModuleActionsConfig {
    let mut actions = module_config.actions.clone();
    let control_provider = match descriptor.kind {
        crate::modules::ModuleKind::Volume => Some("audio"),
        crate::modules::ModuleKind::Media => Some("media"),
        _ => None,
    };
    if let Some(provider) = control_provider {
        // Built-in control modules use their provider for the detail view.
        // Preserve explicit commands and other configured actions.
        for binding in &mut actions.bindings {
            if matches!(binding.action, crate::config::PanelAction::OpenPanel) {
                binding.action = crate::config::PanelAction::OpenProvider(provider.into());
            }
        }
        if actions.action_for(&ActionTrigger::Click).is_none() {
            actions.bindings.push(ActionBinding {
                trigger: ActionTrigger::Click,
                action: crate::config::PanelAction::OpenProvider(provider.into()),
            });
        }
        return actions;
    }
    if descriptor.kind == crate::modules::ModuleKind::Notifications {
        for (trigger, action) in [
            (
                ActionTrigger::Click,
                crate::config::PanelAction::OpenProvider("notifications".into()),
            ),
            (
                ActionTrigger::RightClick,
                crate::config::PanelAction::Command(crate::config::ConfiguredCommand::argv(
                    crate::external_panel::companion_binary("canopa-notify"),
                    vec!["ctl".into(), "dnd".into(), "toggle".into()],
                    &crate::config::CommandPolicy::default(),
                )),
            ),
        ] {
            if actions.action_for(&trigger).is_none() {
                actions.bindings.push(ActionBinding { trigger, action });
            }
        }
        return actions;
    }
    if descriptor.kind != crate::modules::ModuleKind::Network
        || actions.action_for(&ActionTrigger::Click).is_some()
    {
        return actions;
    }
    let provider = match &module_config.params {
        crate::config::ModuleParams::Network(config) => config.default_provider.clone(),
        _ => crate::config::NetworkConfig::default().default_provider,
    };
    if let Some(provider) = provider.as_deref()
        && panel_provider_catalog.contains(provider)
    {
        actions.bindings.push(ActionBinding {
            trigger: ActionTrigger::Click,
            action: crate::config::PanelAction::OpenProvider(provider.to_string()),
        });
    }
    actions
}

#[cfg(test)]
mod tests {
    #[test]
    fn media_placement_synthesizes_a_shared_source_and_opens_the_media_provider() {
        use crate::config::{ActionTrigger, Config, PanelAction};
        use crate::modules::{ModuleKind, data_source::ModuleDataSourceIdentity};
        let mut config = Config::default();
        config.bars[0].modules.center = vec!["media".into()];
        let graph = super::build_module_graph(&config).unwrap();
        let (_, module, actions) = graph
            .modules
            .iter()
            .find(|(_, module, _)| module.kind() == ModuleKind::Media)
            .unwrap();
        assert_eq!(
            module.data_source().unwrap().data_source_identity(),
            ModuleDataSourceIdentity::Media
        );
        assert_eq!(
            actions.action_for(&ActionTrigger::Click),
            Some(&PanelAction::OpenProvider("media".into()))
        );
    }

    use crate::config::{BarConfig, BarModules, Config};
    use crate::modules::{ModuleKind, Zone};

    #[test]
    fn volume_sound_panel_defaults_migrate_detail_bindings_and_preserve_custom_actions() {
        use crate::config::{ActionBinding, ActionTrigger, PanelAction};
        let (mut config, _) =
            crate::config::load(&canopa_config::Request::Defaults, false).unwrap();
        let index = config
            .modules
            .iter()
            .position(|module| module.module_type == "volume")
            .unwrap();
        for (configured, expected) in [
            (None, PanelAction::OpenProvider("audio".into())),
            (
                Some(PanelAction::OpenPanel),
                PanelAction::OpenProvider("audio".into()),
            ),
            (
                Some(PanelAction::OpenMenu("main".into())),
                PanelAction::OpenMenu("main".into()),
            ),
        ] {
            config.modules[index].actions.bindings.clear();
            if let Some(action) = configured {
                config.modules[index].actions.bindings.push(ActionBinding {
                    trigger: ActionTrigger::Click,
                    action,
                });
            }
            let graph = super::build_module_graph(&config).unwrap();
            let (_, _, actions) = graph
                .modules
                .iter()
                .find(|(_, module, _)| module.kind() == ModuleKind::Volume)
                .unwrap();
            assert_eq!(actions.action_for(&ActionTrigger::Click), Some(&expected));
        }
    }

    #[test]
    fn referenced_builtin_is_synthesized() {
        let mut config = Config::default();
        config.bars[0].modules.end = vec!["clock".into()];
        let graph = super::build_module_graph(&config).unwrap();
        assert!(
            graph
                .modules
                .iter()
                .any(|(_, module, _)| module.kind() == ModuleKind::Clock)
        );
    }

    #[test]
    fn shared_module_across_bars_uses_first_boot_zone() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    modules: BarModules {
                        start: vec!["clock".into()],
                        ..BarModules::default()
                    },
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "secondary".into(),
                    modules: BarModules {
                        end: vec!["clock".into()],
                        ..BarModules::default()
                    },
                    ..BarConfig::default()
                },
            ],
            ..Config::default()
        };

        let graph = super::build_module_graph(&config).unwrap();
        let clock = graph
            .modules
            .iter()
            .find(|(_, module, _)| module.kind() == ModuleKind::Clock)
            .unwrap();
        assert_eq!(clock.0, Zone::Start);
    }

    #[test]
    fn panel_only_module_is_instantiated() {
        let mut config = Config::default();
        config.bars[0].modules.start = vec![];
        config.bars[0].panel.modules.end = vec!["clock".into()];

        let graph = super::build_module_graph(&config).unwrap();

        assert!(
            graph
                .modules
                .iter()
                .any(|(zone, module, _)| zone == &Zone::End && module.kind() == ModuleKind::Clock)
        );
    }

    #[test]
    fn configured_wifi_provider_adds_network_builtin_primary_action() {
        let mut config = Config::default();
        config.bars[0].modules.end = vec!["network".into()];
        config.panel_providers = vec![crate::config::PanelProviderConfig {
            protocol: Default::default(),
            name: "wifi".into(),
            command: crate::config::ConfiguredCommand::argv_for_test("wifi-provider", Vec::new()),
        }];

        let graph = super::build_module_graph(&config).unwrap();
        let (_, _, actions) = graph
            .modules
            .iter()
            .find(|(_, module, _)| module.kind() == ModuleKind::Network)
            .expect("network module");

        assert_eq!(
            actions.action_for(&crate::config::ActionTrigger::Click),
            Some(&crate::config::PanelAction::OpenProvider("wifi".into()))
        );
    }

    #[test]
    fn builtin_wifi_provider_adds_network_primary_action_without_declaration() {
        let mut config = Config::default();
        config.bars[0].modules.end = vec!["network".into()];

        let graph = super::build_module_graph(&config).unwrap();
        let (_, _, actions) = graph
            .modules
            .iter()
            .find(|(_, module, _)| module.kind() == ModuleKind::Network)
            .expect("network module");

        assert_eq!(
            actions.action_for(&crate::config::ActionTrigger::Click),
            Some(&crate::config::PanelAction::OpenProvider("wifi".into()))
        );
    }

    #[test]
    fn configured_network_click_action_wins_over_builtin_provider() {
        let mut config = Config::default();
        config.bars[0].modules.end = vec!["network".into()];
        config.panel_providers = vec![crate::config::PanelProviderConfig {
            protocol: Default::default(),
            name: "wifi".into(),
            command: crate::config::ConfiguredCommand::argv_for_test("wifi-provider", Vec::new()),
        }];
        config.modules = vec![crate::config::ModuleConfig {
            name: "network".into(),
            module_type: "network".into(),
            params: crate::config::ModuleParams::Network(crate::config::NetworkConfig::default()),
            actions: crate::config::ModuleActionsConfig {
                bindings: vec![crate::config::ActionBinding {
                    trigger: crate::config::ActionTrigger::Click,
                    action: crate::config::PanelAction::Command(
                        crate::config::ConfiguredCommand::argv_for_test("notify-send", Vec::new()),
                    ),
                }],
            },
        }];

        let graph = super::build_module_graph(&config).unwrap();
        let (_, _, actions) = graph
            .modules
            .iter()
            .find(|(_, module, _)| module.kind() == ModuleKind::Network)
            .expect("network module");

        assert!(matches!(
            actions.action_for(&crate::config::ActionTrigger::Click),
            Some(crate::config::PanelAction::Command(_))
        ));
    }

    #[test]
    fn network_provider_false_disables_builtin_provider_action() {
        let mut config = Config::default();
        config.bars[0].modules.end = vec!["network".into()];
        config.panel_providers = vec![crate::config::PanelProviderConfig {
            protocol: Default::default(),
            name: "wifi".into(),
            command: crate::config::ConfiguredCommand::argv_for_test("wifi-provider", Vec::new()),
        }];
        let network = crate::config::NetworkConfig {
            default_provider: None,
            ..Default::default()
        };
        config.modules = vec![crate::config::ModuleConfig {
            name: "network".into(),
            module_type: "network".into(),
            params: crate::config::ModuleParams::Network(network),
            actions: crate::config::ModuleActionsConfig::default(),
        }];

        let graph = super::build_module_graph(&config).unwrap();
        let (_, _, actions) = graph
            .modules
            .iter()
            .find(|(_, module, _)| module.kind() == ModuleKind::Network)
            .expect("network module");

        assert_eq!(
            actions.action_for(&crate::config::ActionTrigger::Click),
            None
        );
    }
}

#[cfg(test)]
mod notification_tests {
    use super::*;
    #[test]
    fn notifications_get_default_actions_and_preserve_configured_overrides() {
        let (config, _) = crate::config::load(&canopa_config::Request::Defaults, false).unwrap();
        let graph = build_module_graph(&config).unwrap();
        let (_, _, actions) = graph
            .modules
            .iter()
            .find(|(_, module, _)| module.kind() == crate::modules::ModuleKind::Notifications)
            .expect("default notification module");
        assert_eq!(
            actions.action_for(&ActionTrigger::Click),
            Some(&crate::config::PanelAction::OpenProvider(
                "notifications".into()
            ))
        );
        let Some(crate::config::PanelAction::Command(command)) =
            actions.action_for(&ActionTrigger::RightClick)
        else {
            panic!("expected DND command")
        };
        let crate::config::ConfiguredCommandSpec::Argv { program, args } = command.spec() else {
            panic!("expected argv")
        };
        assert!(program.ends_with("canopa-notify"));
        assert_eq!(args, &["ctl", "dnd", "toggle"]);
        let mut config = config;
        let module = config
            .modules
            .iter_mut()
            .find(|module| module.module_type == "notifications")
            .unwrap();
        module.actions.bindings.push(ActionBinding {
            trigger: ActionTrigger::Click,
            action: crate::config::PanelAction::OpenPanel,
        });
        let graph = build_module_graph(&config).unwrap();
        let (_, _, actions) = graph
            .modules
            .iter()
            .find(|(_, module, _)| module.kind() == crate::modules::ModuleKind::Notifications)
            .unwrap();
        assert_eq!(
            actions.action_for(&ActionTrigger::Click),
            Some(&crate::config::PanelAction::OpenPanel)
        );
        assert!(actions.action_for(&ActionTrigger::RightClick).is_some());
    }
}
