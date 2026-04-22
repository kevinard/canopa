use std::collections::{HashMap, HashSet};

use crate::modules::{Module, ModuleId, ModuleKind, ModuleState, Zone};

pub(super) struct ModuleRuntime {
    store: super::module_store::ModuleStore,
    states: HashMap<ModuleId, ModuleState>,
}

impl ModuleRuntime {
    pub(super) fn from_modules(modules: Vec<(Zone, Box<dyn Module>)>) -> Self {
        Self {
            store: super::module_store::ModuleStore::from_modules(modules),
            states: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub(super) fn is_empty(&self) -> bool {
        self.store.is_empty()
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

    pub(super) fn iter(&self) -> impl Iterator<Item = &(Zone, Box<dyn Module>)> {
        self.store.iter()
    }

    #[allow(dead_code)]
    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = &mut (Zone, Box<dyn Module>)> {
        self.store.iter_mut()
    }

    #[allow(dead_code)]
    pub(super) fn push(&mut self, zone: Zone, module: Box<dyn Module>) {
        self.store.push(zone, module);
    }

    pub(super) fn insert(&mut self, index: usize, item: (Zone, Box<dyn Module>)) {
        self.store.insert(index, item);
    }

    pub(super) fn retain(&mut self, keep: impl FnMut(&(Zone, Box<dyn Module>)) -> bool) {
        self.store.retain(keep);
        let retained_ids: HashSet<_> = self.store.iter().map(|(_, module)| module.id()).collect();
        self.states.retain(|id, _| retained_ids.contains(id));
    }

    pub(super) fn ids_by_kind(&self, kind: ModuleKind) -> &[ModuleId] {
        self.store.ids_by_kind(kind)
    }

    pub(super) fn get(&self, id: &ModuleId) -> Option<&dyn Module> {
        self.store.get(id)
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
                }),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["battery".into()];
        App::boot(config, vec![]).0
    }

    fn make_network_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams, NetworkConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "network".into(),
                module_type: "network".into(),
                params: ModuleParams::Network(NetworkConfig),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["network".into()];
        App::boot(config, vec![]).0
    }

    fn make_volume_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams, VolumeConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "volume".into(),
                module_type: "volume".into(),
                params: ModuleParams::Volume(VolumeConfig { step: 5 }),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["volume".into()];
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
                params: ModuleParams::Tray(crate::config::TrayConfig),
            }],
            ..Config::default()
        };
        config.bars[0].modules.bottom = vec!["tray".into()];
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
        assert!(app.modules.is_empty());
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
                },
                ModuleConfig {
                    name: "battery2".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(BatteryConfig::default()),
                },
            ],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["battery".into(), "battery2".into()];
        let (mut app, _) = App::boot(config, vec![]);

        app.push_battery_state(42.0, false);

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
        config.bars[0].modules.bottom = vec!["clock".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn undeclared_unknown_name_in_bar_modules_is_skipped() {
        let mut config = Config::default();
        config.bars[0].modules.bottom = vec!["my-script".into()];
        let (app, _) = App::boot(config, vec![]);
        assert!(app.modules.is_empty());
    }

    #[test]
    fn clock_module_instantiated_from_registry() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["clock".into()];
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
                    command: "echo hi".into(),
                    interval: 5,
                }),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["script".into()];
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
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["volume".into()];
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
                    on_click: "kitty".into(),
                }),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["button".into()];
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
    fn script_failed_event_retains_previous_state() {
        use crate::config::{ModuleConfig, ModuleParams, ScriptConfig};

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "script".into(),
                module_type: "script".into(),
                params: ModuleParams::Script(ScriptConfig {
                    command: "echo hi".into(),
                    interval: 5,
                }),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["script".into()];
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

        match app.modules.state(&id) {
            Some(ModuleState::Script(s)) => assert_eq!(s.output, "prev"),
            other => panic!("expected Script state, got {:?}", other),
        }
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
                },
                ModuleConfig {
                    name: "workspaces-right".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::Workspaces(WorkspacesConfig::default()),
                },
            ],
            ..Config::default()
        };
        config.bars[0].modules.top = vec!["workspaces-left".into(), "workspaces-right".into()];
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
            Zone::Bottom,
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
    fn tray_snapshot_handles_multiple_placeholders_after_existing_icons() {
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "tray-left".into(),
                    module_type: "tray".into(),
                    params: ModuleParams::Tray(crate::config::TrayConfig),
                },
                ModuleConfig {
                    name: "tray-right".into(),
                    module_type: "tray".into(),
                    params: ModuleParams::Tray(crate::config::TrayConfig),
                },
            ],
            ..Config::default()
        };
        config.bars[0].modules.top = vec!["tray-left".into(), "tray-right".into()];
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
        };

        let _ = app.handle_tray_event(TrayEvent::Snapshot(vec![item]));
        app.modules
            .set_state(runtime_id.clone(), ModuleState::Loading);
        let _ = app.handle_tray_event(TrayEvent::ItemRemoved(
            ":1.42/CustomStatusNotifierItem".into(),
        ));

        assert!(app.modules.state(&runtime_id).is_none());
    }
}
