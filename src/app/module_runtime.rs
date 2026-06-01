use std::collections::{HashMap, HashSet};

use crate::config::ModuleActionsConfig;
use crate::modules::{Module, ModuleId, ModuleKind, ModuleState, Zone};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ModuleOrigin {
    Configured,
    ExpandedFrom(ModuleId),
}

pub(super) struct ModuleRuntime {
    store: super::module_store::ModuleStore,
    states: HashMap<ModuleId, ModuleState>,
    actions: HashMap<ModuleId, ModuleActionsConfig>,
    origins: HashMap<ModuleId, ModuleOrigin>,
}

impl ModuleRuntime {
    pub(super) fn from_modules_with_actions(
        modules: Vec<(Zone, Box<dyn Module>, ModuleActionsConfig)>,
    ) -> Self {
        let mut actions = HashMap::new();
        let mut origins = HashMap::new();
        let ordered = modules
            .into_iter()
            .map(|(zone, module, module_actions)| {
                let id = module.id();
                if module_actions != ModuleActionsConfig::default() {
                    actions.insert(id.clone(), module_actions);
                }
                origins.insert(id, ModuleOrigin::Configured);
                (zone, module)
            })
            .collect();

        Self {
            store: super::module_store::ModuleStore::from_modules(ordered),
            states: HashMap::new(),
            actions,
            origins,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.store.len()
    }

    pub(super) fn state(&self, id: &ModuleId) -> Option<&ModuleState> {
        self.states.get(id)
    }

    pub(super) fn set_state(&mut self, id: ModuleId, state: ModuleState) {
        if self.states.get(&id) != Some(&state) {
            self.states.insert(id, state);
        }
    }

    pub(super) fn apply_outcomes(
        &mut self,
        outcomes: Vec<(ModuleId, crate::modules::data_source::ModuleInstanceOutcome)>,
    ) {
        for (id, outcome) in outcomes {
            self.set_state(id, outcome.into_module_state());
        }
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &(Zone, Box<dyn Module>)> {
        self.store.iter()
    }

    #[cfg(test)]
    pub(super) fn push(&mut self, zone: Zone, module: Box<dyn Module>) {
        self.store.push(zone, module);
    }

    pub(super) fn insert_with_actions_and_origin(
        &mut self,
        index: usize,
        item: (Zone, Box<dyn Module>),
        actions: ModuleActionsConfig,
        origin: ModuleOrigin,
    ) {
        let id = item.1.id();
        self.store.insert(index, item);
        if actions == ModuleActionsConfig::default() {
            self.actions.remove(&id);
        } else {
            self.actions.insert(id.clone(), actions);
        }
        self.origins.insert(id, origin);
    }

    pub(super) fn retain(&mut self, keep: impl FnMut(&(Zone, Box<dyn Module>)) -> bool) {
        self.store.retain(keep);
        let retained_ids: HashSet<_> = self.store.iter().map(|(_, module)| module.id()).collect();
        self.states.retain(|id, _| retained_ids.contains(id));
        self.actions.retain(|id, _| retained_ids.contains(id));
        self.origins.retain(|id, _| retained_ids.contains(id));
    }

    pub(super) fn ids_by_kind(&self, kind: ModuleKind) -> &[ModuleId] {
        self.store.ids_by_kind(kind)
    }

    pub(super) fn get(&self, id: &ModuleId) -> Option<&dyn Module> {
        self.store.get(id)
    }

    pub(super) fn data_source_dependents(
        &self,
        identity: crate::modules::data_source::ModuleDataSourceIdentity,
    ) -> Vec<crate::modules::ModuleId> {
        self.store
            .iter()
            .filter(|(_, module)| module.data_source_identity() == Some(identity))
            .map(|(_, module)| module.id())
            .collect()
    }

    pub(super) fn has_data_source(
        &self,
        identity: crate::modules::data_source::ModuleDataSourceIdentity,
    ) -> bool {
        self.store
            .iter()
            .any(|(_, module)| module.data_source_identity() == Some(identity))
    }

    pub(super) fn get_mut(&mut self, id: &ModuleId) -> Option<&mut (dyn Module + '_)> {
        self.store.get_mut(id)
    }

    pub(super) fn config_name_for(&self, id: &ModuleId) -> Option<&str> {
        self.get(id).and_then(Module::config_name)
    }

    pub(super) fn diagnostic_name_for<'a>(&'a self, id: &'a ModuleId) -> &'a str {
        self.config_name_for(id).unwrap_or(id.0.as_str())
    }

    pub(super) fn action_for(
        &self,
        id: &ModuleId,
        trigger: &crate::config::ActionTrigger,
    ) -> Option<&crate::config::PanelAction> {
        let actions = self.actions.get(id)?;
        actions.action_for(trigger)
    }
}

impl super::App {
    pub(super) fn actions_for_config_module(&self, config_name: &str) -> ModuleActionsConfig {
        self.config
            .modules
            .iter()
            .find(|module| module.name == config_name)
            .map(|module| module.actions.clone())
            .unwrap_or_default()
    }
}

impl std::ops::Index<usize> for ModuleRuntime {
    type Output = (Zone, Box<dyn Module>);

    fn index(&self, index: usize) -> &Self::Output {
        &self.store[index]
    }
}

impl<'a> IntoIterator for &'a ModuleRuntime {
    type Item = &'a (Zone, Box<dyn Module>);
    type IntoIter = std::slice::Iter<'a, (Zone, Box<dyn Module>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.store.iter()
    }
}

impl<'a> IntoIterator for &'a mut ModuleRuntime {
    type Item = &'a mut (Zone, Box<dyn Module>);
    type IntoIter = std::slice::IterMut<'a, (Zone, Box<dyn Module>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.store.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::super::messages::{AppNiriEvent, ModuleRuntimeEvent, TrayRuntimeEvent};
    use super::super::{App, Message};
    use crate::config::{Config, Edge, ModuleConfig, ModuleParams};
    use crate::dbus::sni::host::{ItemStatus, TrayEvent, TrayItem};
    use crate::modules::battery::BatteryEvent;
    use crate::modules::network::{NetworkConnectivity, NetworkEvent};
    use crate::modules::script::ScriptEvent;
    use crate::modules::volume::VolumeEvent;
    use crate::modules::{ModuleEvent, ModuleId, ModuleKind, ModuleState, Zone};
    use crate::niri::NiriEvent;
    use zbus::zvariant::OwnedObjectPath;

    fn unique_tmp_path(name: &str) -> String {
        format!("/tmp/niribar-{name}-{}", std::process::id())
    }

    fn wait_for_path(path: &str) -> bool {
        for _ in 0..50 {
            if std::path::Path::new(path).exists() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    fn make_config() -> Config {
        Config::default()
    }

    fn make_battery_app() -> App {
        use crate::config::{BatteryConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "battery".into(),
                module_type: "battery".into(),
                params: ModuleParams::Battery(BatteryConfig {
                    critical_threshold: 15,
                    ..Default::default()
                }),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["battery".into()];
        App::boot(config, vec![]).0
    }

    fn make_network_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams, NetworkConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "network".into(),
                module_type: "network".into(),
                params: ModuleParams::Network(NetworkConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["network".into()];
        App::boot(config, vec![]).0
    }

    fn make_volume_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams, VolumeConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "volume".into(),
                module_type: "volume".into(),
                params: ModuleParams::Volume(VolumeConfig {
                    step: 5,
                    ..Default::default()
                }),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["volume".into()];
        App::boot(config, vec![]).0
    }

    fn make_workspace(idx: u8, is_focused: bool) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id: idx as u64 + 1,
            idx,
            name: None,
            output: Some("eDP-1".to_string()),
            is_urgent: false,
            is_active: is_focused,
            is_focused,
            active_window_id: None,
        }
    }

    fn make_tray_config() -> Config {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "tray".into(),
                module_type: "tray".into(),
                params: ModuleParams::Tray(crate::config::TrayConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Config::default()
        };
        config.bars[0].modules.end = vec!["tray".into()];
        config.bars[0].edge = Edge::Bottom;
        config
    }

    fn tray_item(service: &str) -> TrayItem {
        TrayItem {
            service: service.into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        }
    }

    #[test]
    fn wrapped_script_output_updates_runtime_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let id = ModuleId::new("script");

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Module(
            id.clone(),
            ModuleEvent::Script(ScriptEvent::Output("hello".into())),
        )));

        match app.modules.state(&id) {
            Some(ModuleState::Script(s)) => assert_eq!(s.output, "hello"),
            other => panic!("expected Script state, got {:?}", other),
        }
    }

    #[test]
    fn boot_modules_empty_with_no_module_config() {
        let (app, _) = App::boot(make_config(), vec![]);
        assert_eq!(app.modules.len(), 0);
    }

    #[test]
    fn module_runtime_applies_data_source_outcomes() {
        let mut app = make_battery_app();

        app.modules.apply_outcomes(vec![(
            ModuleId::new("battery"),
            crate::modules::data_source::ModuleInstanceOutcome::Visible(ModuleState::Battery(
                crate::modules::battery::BatteryState {
                    percentage: 91.0,
                    is_charging: false,
                },
            )),
        )]);

        match app.modules.state(&ModuleId::new("battery")) {
            Some(ModuleState::Battery(state)) => {
                assert!((state.percentage - 91.0).abs() < 0.001);
                assert!(!state.is_charging);
            }
            other => panic!("expected battery state, got {other:?}"),
        }

        app.modules.apply_outcomes(vec![(
            ModuleId::new("battery"),
            crate::modules::data_source::ModuleInstanceOutcome::Unavailable,
        )]);

        assert!(matches!(
            app.modules.state(&ModuleId::new("battery")),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn battery_state_updates_all_battery_instances() {
        use crate::config::{BatteryConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "battery".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(BatteryConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "battery2".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(BatteryConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["battery".into(), "battery2".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::StateChanged {
                percentage: 42.0,
                is_charging: false,
            },
        )));

        for id in ["battery", "battery2"] {
            let Some(ModuleState::Battery(state)) = app.modules.state(&ModuleId::new(id)) else {
                panic!("expected battery state for {id}");
            };
            assert_eq!(state.percentage, 42.0);
            assert!(!state.is_charging);
        }
    }

    #[test]
    fn undeclared_known_type_in_bar_modules_uses_defaults() {
        let mut config = Config::default();
        config.bars[0].modules.end = vec!["clock".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn undeclared_unknown_name_in_bar_modules_is_configuration_error() {
        let mut config = Config::default();
        config.bars[0].modules.end = vec!["my-script".into()];
        let err = crate::config::validate(&config).unwrap_err();
        assert!(err.to_string().contains("unknown module"), "got: {err}");
    }

    #[test]
    fn clock_module_instantiated_from_registry() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["clock".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn script_module_instantiated_from_registry() {
        use crate::config::{ModuleConfig, ModuleParams, ScriptConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "script".into(),
                module_type: "script".into(),
                params: ModuleParams::Script(ScriptConfig {
                    command: crate::config::CommandConfig::Shell("echo hi".into()),
                    interval: 5,
                    format: crate::config::FormatConfig::default(),
                    tooltip: crate::config::TooltipSetting::Default,
                }),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["script".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn script_output_event_updates_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let id = ModuleId::new("script");
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Module(
            id.clone(),
            ModuleEvent::Script(ScriptEvent::Output("hello".into())),
        )));
        match app.modules.state(&id) {
            Some(ModuleState::Script(s)) => assert_eq!(s.output, "hello"),
            other => panic!("expected Script state, got {:?}", other),
        }
    }

    #[test]
    fn repeated_clock_tick_keeps_existing_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let id = ModuleId::new("clock");
        let first = ModuleState::Clock(crate::modules::clock::ClockState {
            time: "10:00".into(),
            date: "Sat".into(),
        });
        app.modules.set_state(id.clone(), first.clone());

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Module(
            id.clone(),
            ModuleEvent::Clock(crate::modules::clock::ClockEvent::Tick {
                time: "10:00".into(),
                date: "Sat".into(),
            }),
        )));

        assert_eq!(app.modules.state(&id), Some(&first));
    }

    #[test]
    fn repeated_volume_state_keeps_existing_state() {
        use crate::config::{ModuleConfig, ModuleParams, VolumeConfig};

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "volume".into(),
                module_type: "volume".into(),
                params: ModuleParams::Volume(VolumeConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["volume".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let id = ModuleId::new("volume");
        let first = ModuleState::Volume(crate::modules::volume::VolumeState {
            volume: 55.0,
            muted: false,
        });
        app.modules.set_state(id.clone(), first.clone());

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Volume(
            crate::modules::volume::VolumeEvent::StateChanged {
                volume: 55.0,
                muted: false,
            },
        )));

        assert_eq!(app.modules.state(&id), Some(&first));
    }

    #[test]
    fn button_module_instantiated_from_registry() {
        use crate::config::{ButtonConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "button".into(),
                module_type: "button".into(),
                params: ModuleParams::Button(ButtonConfig {
                    label: "Launch".into(),
                    icon: ">".into(),
                    on_click: crate::config::CommandConfig::Shell("kitty".into()),
                    format: crate::config::FormatConfig::default(),
                    tooltip: crate::config::TooltipSetting::Default,
                }),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["button".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn button_ready_event_updates_state() {
        use crate::modules::button::ButtonEvent;

        let (mut app, _) = App::boot(make_config(), vec![]);
        let id = ModuleId::new("button");
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Module(
            id.clone(),
            ModuleEvent::Button(ButtonEvent::Ready),
        )));
        match app.modules.state(&id) {
            Some(ModuleState::Button(_)) => {}
            other => panic!("expected Button state, got {:?}", other),
        }
    }

    #[test]
    fn module_click_no_match_does_not_panic() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = iced::window::Id::unique();
        let id = ModuleId::new("nonexistent");
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::ModuleClick(id, wid)));
    }

    #[test]
    fn module_actions_click_overrides_button_legacy_on_click() {
        use crate::config::{ButtonConfig, ModuleActionsConfig, ModuleConfig, ModuleParams};

        let action_path = unique_tmp_path("action-click");
        let legacy_path = unique_tmp_path("legacy-click");
        let _ = std::fs::remove_file(&action_path);
        let _ = std::fs::remove_file(&legacy_path);

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "button".into(),
                module_type: "button".into(),
                params: ModuleParams::Button(ButtonConfig {
                    label: "Launch".into(),
                    icon: ">".into(),
                    on_click: crate::config::CommandConfig::Shell(format!(
                        "printf legacy >{legacy_path}"
                    )),
                    format: crate::config::FormatConfig::default(),
                    tooltip: crate::config::TooltipSetting::Default,
                }),
                actions: ModuleActionsConfig {
                    bindings: vec![crate::config::ActionBinding {
                        trigger: crate::config::ActionTrigger::Click,
                        action: crate::config::PanelAction::Command(
                            crate::config::CommandConfig::Shell(format!(
                                "printf action >{action_path}"
                            )),
                        ),
                    }],
                },
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["button".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::ModuleClick(
            ModuleId::new("button"),
            iced::window::Id::unique(),
        )));

        assert!(
            wait_for_path(&action_path),
            "configured click action should create {action_path}"
        );
        assert!(
            !std::path::Path::new(&legacy_path).exists(),
            "legacy on-click should not run when actions.click is configured"
        );
        let _ = std::fs::remove_file(&action_path);
        let _ = std::fs::remove_file(&legacy_path);
    }

    #[test]
    fn module_actions_right_click_runs_configured_command() {
        use crate::config::{ClockConfig, ModuleActionsConfig, ModuleConfig, ModuleParams};

        let action_path = unique_tmp_path("action-right-click");
        let _ = std::fs::remove_file(&action_path);

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: ModuleActionsConfig {
                    bindings: vec![crate::config::ActionBinding {
                        trigger: crate::config::ActionTrigger::RightClick,
                        action: crate::config::PanelAction::Command(
                            crate::config::CommandConfig::Shell(format!(
                                "printf right >{action_path}"
                            )),
                        ),
                    }],
                },
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::ModuleRightClick(
            ModuleId::new("clock"),
            iced::window::Id::unique(),
        )));

        assert!(
            wait_for_path(&action_path),
            "configured right-click action should create {action_path}"
        );
        let _ = std::fs::remove_file(&action_path);
    }

    #[test]
    fn script_failed_event_marks_module_unavailable_for_that_refresh() {
        use crate::config::{ModuleConfig, ModuleParams, ScriptConfig};

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "script".into(),
                module_type: "script".into(),
                params: ModuleParams::Script(ScriptConfig {
                    command: crate::config::CommandConfig::Shell("echo hi".into()),
                    interval: 5,
                    format: crate::config::FormatConfig::default(),
                    tooltip: crate::config::TooltipSetting::Default,
                }),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["script".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let id = ModuleId::new("script");
        app.modules.set_state(
            id.clone(),
            ModuleState::Script(crate::modules::script::ScriptState {
                output: "prev".into(),
            }),
        );

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Module(
            id.clone(),
            ModuleEvent::Script(ScriptEvent::Failed),
        )));

        assert!(matches!(
            app.modules.state(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn battery_state_changed_updates_module() {
        let mut app = make_battery_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::StateChanged {
                percentage: 72.0,
                is_charging: false,
            },
        )));
        let id = ModuleId::new("battery");
        match app.modules.state(&id) {
            Some(ModuleState::Battery(s)) => {
                assert!((s.percentage - 72.0).abs() < 0.001);
                assert!(!s.is_charging);
            }
            other => panic!("expected Battery state, got {:?}", other),
        }
    }

    #[test]
    fn battery_unavailable_marks_module_unavailable() {
        let mut app = make_battery_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::Unavailable,
        )));
        let id = ModuleId::new("battery");
        assert!(matches!(
            app.modules.state(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn battery_source_recovers_after_unavailable_event() {
        let mut app = make_battery_app();

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::Unavailable,
        )));
        assert!(matches!(
            app.modules.state(&ModuleId::new("battery")),
            Some(ModuleState::Unavailable)
        ));

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::StateChanged {
                percentage: 74.0,
                is_charging: true,
            },
        )));

        match app.modules.state(&ModuleId::new("battery")) {
            Some(ModuleState::Battery(state)) => {
                assert!((state.percentage - 74.0).abs() < 0.001);
                assert!(state.is_charging);
            }
            other => panic!("expected recovered battery state, got {other:?}"),
        }
    }

    #[test]
    fn battery_state_not_updated_without_battery_module() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::StateChanged {
                percentage: 50.0,
                is_charging: true,
            },
        )));
        let id = ModuleId::new("battery");
        assert!(app.modules.state(&id).is_none());
    }

    #[test]
    fn network_state_changed_updates_module() {
        let mut app = make_network_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Network(
            NetworkEvent::StateChanged {
                connectivity: NetworkConnectivity::Full,
                label: Some("Home".into()),
            },
        )));
        let id = ModuleId::new("network");
        match app.modules.state(&id) {
            Some(ModuleState::Network(s)) => {
                assert_eq!(s.connectivity, NetworkConnectivity::Full);
                assert_eq!(s.label.as_deref(), Some("Home"));
            }
            other => panic!("expected Network state, got {:?}", other),
        }
    }

    #[test]
    fn network_unavailable_marks_module_unavailable() {
        let mut app = make_network_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Network(
            NetworkEvent::Unavailable,
        )));
        let id = ModuleId::new("network");
        assert!(matches!(
            app.modules.state(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn network_state_not_updated_without_network_module() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Network(
            NetworkEvent::StateChanged {
                connectivity: NetworkConnectivity::Full,
                label: None,
            },
        )));
        let id = ModuleId::new("network");
        assert!(app.modules.state(&id).is_none());
    }

    #[test]
    fn volume_state_changed_updates_module() {
        let mut app = make_volume_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Volume(
            VolumeEvent::StateChanged {
                volume: 60.0,
                muted: false,
            },
        )));
        let id = ModuleId::new("volume");
        match app.modules.state(&id) {
            Some(ModuleState::Volume(s)) => {
                assert!((s.volume - 60.0).abs() < 0.001);
                assert!(!s.muted);
            }
            other => panic!("expected Volume state, got {:?}", other),
        }
    }

    #[test]
    fn volume_unavailable_marks_module_unavailable() {
        let mut app = make_volume_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Volume(
            VolumeEvent::Unavailable,
        )));
        let id = ModuleId::new("volume");
        assert!(matches!(
            app.modules.state(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn volume_state_not_updated_without_volume_module() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Volume(
            VolumeEvent::StateChanged {
                volume: 50.0,
                muted: false,
            },
        )));
        let id = ModuleId::new("volume");
        assert!(app.modules.state(&id).is_none());
    }

    #[test]
    fn workspace_rebuild_handles_multiple_placeholders_after_existing_items() {
        use crate::config::WorkspacesConfig;
        use niri_ipc::Event;

        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "workspaces-left".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::Workspaces(WorkspacesConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "workspaces-right".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::Workspaces(WorkspacesConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["workspaces-left".into(), "workspaces-right".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let first = vec![make_workspace(0, true), make_workspace(1, false)];
        let second = vec![make_workspace(0, false), make_workspace(1, true)];

        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged { workspaces: first },
        ))));
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged { workspaces: second },
        ))));

        let layout: Vec<(ModuleKind, Option<String>)> = app
            .modules
            .iter()
            .map(|(_, m)| (m.kind(), m.config_name().map(str::to_string)))
            .collect();
        assert_eq!(
            layout,
            vec![
                (ModuleKind::Workspaces, None),
                (ModuleKind::WorkspaceItem, Some("workspaces-left".into())),
                (ModuleKind::WorkspaceItem, Some("workspaces-left".into())),
                (ModuleKind::Workspaces, None),
                (ModuleKind::WorkspaceItem, Some("workspaces-right".into())),
                (ModuleKind::WorkspaceItem, Some("workspaces-right".into())),
            ]
        );
    }

    #[test]
    fn tray_snapshot_removes_existing_tray_icons_before_insert() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);

        let old_item = tray_item("org.old");
        app.modules.push(
            Zone::End,
            Box::new(crate::modules::tray::TrayIconModule::new(
                old_item,
                "tray".into(),
            )),
        );

        let new_item = tray_item("org.new");
        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![new_item],
        ))));

        assert!(app.modules.iter().any(|(_, m)| m.id().0 == "tray:org.new"));
        assert!(!app.modules.iter().any(|(_, m)| m.id().0 == "tray:org.old"));
    }

    #[test]
    fn wrapped_tray_snapshot_updates_runtime_icons() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);

        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![tray_item("org.test.App")],
        ))));

        assert!(
            app.modules
                .iter()
                .any(|(_, m)| m.kind() == ModuleKind::TrayIcon && m.id().0 == "tray:org.test.App")
        );
    }

    #[test]
    fn diagnostic_module_name_uses_config_name_for_expanded_modules() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);

        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![tray_item("org.private.App")],
        ))));

        assert_eq!(
            app.modules
                .diagnostic_name_for(&ModuleId::new("tray:org.private.App")),
            "tray"
        );
    }

    #[test]
    fn tray_icon_inherits_actions_from_tray_placeholder() {
        let mut config = make_tray_config();
        config.modules[0]
            .actions
            .bindings
            .push(crate::config::ActionBinding {
                trigger: crate::config::ActionTrigger::RightClick,
                action: crate::config::PanelAction::Command(crate::config::CommandConfig::Shell(
                    "notify-send tray".into(),
                )),
            });
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![tray_item("org.test.App")],
        ))));

        assert_eq!(
            app.modules.action_for(
                &ModuleId::new("tray:org.test.App"),
                &crate::config::ActionTrigger::RightClick,
            ),
            Some(&crate::config::PanelAction::Command(
                crate::config::CommandConfig::Shell("notify-send tray".into())
            ))
        );
    }

    #[test]
    fn workspace_item_inherits_actions_from_workspaces_placeholder() {
        use niri_ipc::Event;

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "workspaces".into(),
                module_type: "workspaces".into(),
                params: ModuleParams::Workspaces(crate::config::WorkspacesConfig::default()),
                actions: crate::config::ModuleActionsConfig {
                    bindings: vec![crate::config::ActionBinding {
                        trigger: crate::config::ActionTrigger::Click,
                        action: crate::config::PanelAction::Command(
                            crate::config::CommandConfig::Shell("notify-send workspace".into()),
                        ),
                    }],
                },
            }],
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged {
                workspaces: vec![make_workspace(0, true)],
            },
        ))));

        assert_eq!(
            app.modules.action_for(
                &ModuleId::new("workspace:1"),
                &crate::config::ActionTrigger::Click,
            ),
            Some(&crate::config::PanelAction::Command(
                crate::config::CommandConfig::Shell("notify-send workspace".into())
            ))
        );
    }

    #[test]
    fn tray_snapshot_handles_multiple_placeholders_after_existing_icons() {
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "tray-left".into(),
                    module_type: "tray".into(),
                    params: ModuleParams::Tray(crate::config::TrayConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "tray-right".into(),
                    module_type: "tray".into(),
                    params: ModuleParams::Tray(crate::config::TrayConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["tray-left".into(), "tray-right".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let item_a = tray_item("org.a");
        let item_b = tray_item("org.b");

        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![item_a.clone(), item_b.clone()],
        ))));
        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![item_a, item_b],
        ))));

        let layout: Vec<(ModuleKind, Option<String>, String)> = app
            .modules
            .iter()
            .map(|(_, m)| {
                (
                    m.kind(),
                    m.config_name().map(str::to_string),
                    m.id().0.clone(),
                )
            })
            .collect();
        assert_eq!(
            layout,
            vec![
                (ModuleKind::Tray, None, "tray-left".into()),
                (
                    ModuleKind::TrayIcon,
                    Some("tray-left".into()),
                    "tray:org.a".into()
                ),
                (
                    ModuleKind::TrayIcon,
                    Some("tray-left".into()),
                    "tray:org.b".into()
                ),
                (ModuleKind::Tray, None, "tray-right".into()),
                (
                    ModuleKind::TrayIcon,
                    Some("tray-right".into()),
                    "tray:org.a".into()
                ),
                (
                    ModuleKind::TrayIcon,
                    Some("tray-right".into()),
                    "tray:org.b".into()
                ),
            ]
        );
    }

    #[test]
    fn tray_item_removed_event_removes_path_qualified_runtime_icon() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);
        let item = TrayItem {
            service: ":1.42".into(),
            object_path: OwnedObjectPath::try_from("/CustomStatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        };

        let _ = app.handle_tray_event(TrayEvent::Snapshot(vec![item]));
        assert!(
            app.modules
                .iter()
                .any(|(_, m)| m.id().0 == "tray::1.42/CustomStatusNotifierItem")
        );

        let _ = app.handle_tray_event(TrayEvent::ItemRemoved(
            ":1.42/CustomStatusNotifierItem".into(),
        ));

        assert!(
            !app.modules
                .iter()
                .any(|(_, m)| m.id().0 == "tray::1.42/CustomStatusNotifierItem")
        );
    }

    #[test]
    fn tray_item_removed_event_removes_default_path_unique_name_icon() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);
        let item = TrayItem {
            service: ":1.77".into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        };

        let _ = app.handle_tray_event(TrayEvent::Snapshot(vec![item]));
        assert!(
            app.modules
                .iter()
                .any(|(_, m)| m.id().0 == "tray::1.77/StatusNotifierItem")
        );

        let _ = app.handle_tray_event(TrayEvent::ItemRemoved(":1.77/StatusNotifierItem".into()));

        assert!(
            !app.modules
                .iter()
                .any(|(_, m)| m.id().0 == "tray::1.77/StatusNotifierItem")
        );
    }

    #[test]
    fn tray_item_added_replaces_existing_path_qualified_icon() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);
        let item = TrayItem {
            service: ":1.88".into(),
            object_path: OwnedObjectPath::try_from("/CustomStatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app-old".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        };
        let replacement = TrayItem {
            icon: crate::dbus::sni::host::TrayIcon::Named("app-new".into()),
            ..item.clone()
        };

        let _ = app.handle_tray_event(TrayEvent::ItemAdded(item));
        let _ = app.handle_tray_event(TrayEvent::ItemAdded(replacement));

        let matches: Vec<_> = app
            .modules
            .iter()
            .filter(|(_, module)| module.id().0 == "tray::1.88/CustomStatusNotifierItem")
            .collect();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn removing_dynamic_tray_icon_prunes_cached_module_state() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);
        let runtime_id = ModuleId::new("tray::1.42/CustomStatusNotifierItem");
        let item = TrayItem {
            service: ":1.42".into(),
            object_path: OwnedObjectPath::try_from("/CustomStatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        };

        let _ = app.handle_tray_event(TrayEvent::Snapshot(vec![item]));
        app.modules
            .set_state(runtime_id.clone(), ModuleState::Empty);
        let _ = app.handle_tray_event(TrayEvent::ItemRemoved(
            ":1.42/CustomStatusNotifierItem".into(),
        ));

        assert!(app.modules.state(&runtime_id).is_none());
    }
}
