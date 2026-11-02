use std::collections::HashMap;

use crate::modules::battery::BatteryEvent;
use crate::modules::data_source::{
    BatteryReading, ModuleDataSourceIdentity, ModuleDataSourceState, NetworkReading, VolumeReading,
};
use crate::modules::network::NetworkEvent;
use crate::modules::volume::VolumeEvent;

use super::{Message, shared::SharedContext};

struct ModuleDataSourceCatalogEntry {
    identity: ModuleDataSourceIdentity,
    subscription: fn(bool, &SharedContext) -> iced::Subscription<Message>,
}

const SOURCE_CATALOG: &[ModuleDataSourceCatalogEntry] = &[
    ModuleDataSourceCatalogEntry {
        identity: ModuleDataSourceIdentity::Stasis,
        subscription: |enabled, _| {
            if enabled {
                crate::modules::idle_inhibitor::stasis_subscription()
            } else {
                iced::Subscription::none()
            }
        },
    },
    ModuleDataSourceCatalogEntry {
        identity: ModuleDataSourceIdentity::Media,
        subscription: |enabled, _| {
            if enabled {
                crate::modules::media::subscription()
            } else {
                iced::Subscription::none()
            }
        },
    },
    ModuleDataSourceCatalogEntry {
        identity: ModuleDataSourceIdentity::IdleInhibitor,
        // Driven by acknowledgements from the Wayland adapter, without polling.
        subscription: |_, _| iced::Subscription::none(),
    },
    ModuleDataSourceCatalogEntry {
        identity: ModuleDataSourceIdentity::Notifications,
        subscription: |enabled, _| {
            if enabled {
                crate::modules::notifications::subscription()
            } else {
                iced::Subscription::none()
            }
        },
    },
    ModuleDataSourceCatalogEntry {
        identity: ModuleDataSourceIdentity::Battery,
        subscription: battery_subscription,
    },
    ModuleDataSourceCatalogEntry {
        identity: ModuleDataSourceIdentity::Network,
        subscription: network_subscription,
    },
    ModuleDataSourceCatalogEntry {
        identity: ModuleDataSourceIdentity::Volume,
        subscription: volume_subscription,
    },
];

fn dbus_subscription<F>(
    enabled: bool,
    conn: Option<zbus::Connection>,
    build: F,
) -> iced::Subscription<Message>
where
    F: FnOnce(zbus::Connection) -> iced::Subscription<Message>,
{
    match (enabled, conn) {
        (true, Some(c)) => build(c),
        _ => iced::Subscription::none(),
    }
}

fn battery_subscription(enabled: bool, shared: &SharedContext) -> iced::Subscription<Message> {
    dbus_subscription(
        enabled,
        shared.dbus.system.clone(),
        crate::dbus::upower::battery_subscription,
    )
}

fn network_subscription(enabled: bool, shared: &SharedContext) -> iced::Subscription<Message> {
    dbus_subscription(
        enabled,
        shared.dbus.system.clone(),
        crate::dbus::networkmanager::network_subscription,
    )
}

fn volume_subscription(enabled: bool, _shared: &SharedContext) -> iced::Subscription<Message> {
    if enabled {
        crate::dbus::wireplumber::volume_subscription()
    } else {
        iced::Subscription::none()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ModuleDataSourceStatus {
    PendingFirstEmission,
    Available(ModuleDataSourceState),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ModuleDataSourceTransition {
    Available(ModuleDataSourceState),
    Unavailable(ModuleDataSourceIdentity),
}

#[derive(Debug, Default)]
pub(super) struct ModuleDataSourceRuntime {
    pub(super) stasis_toggle_pending: bool,
    pub(super) idle_inhibitor: super::idle_inhibitor::IdleInhibitorRuntime,
    sources: HashMap<ModuleDataSourceIdentity, ModuleDataSourceStatus>,
}

impl ModuleDataSourceRuntime {
    pub(super) fn from_modules(modules: &super::module_runtime::ModuleRuntime) -> Self {
        let mut runtime = Self::default();
        for identity in ModuleDataSourceIdentity::ALL {
            if modules.has_data_source(identity) {
                runtime
                    .sources
                    .insert(identity, ModuleDataSourceStatus::PendingFirstEmission);
            }
        }
        runtime
    }

    /// Keep live source snapshots and the manual inhibitor preference across reloads.
    pub(super) fn reconfigure(&mut self, modules: &mut super::module_runtime::ModuleRuntime) {
        self.sources
            .retain(|identity, _| modules.has_data_source(*identity));
        for identity in ModuleDataSourceIdentity::ALL {
            if modules.has_data_source(identity) {
                let status = self
                    .sources
                    .entry(identity)
                    .or_insert(ModuleDataSourceStatus::PendingFirstEmission);
                use super::module_runtime::ModuleRuntimeIntent;
                match status {
                    ModuleDataSourceStatus::Available(state) => {
                        modules.handle(ModuleRuntimeIntent::ApplyDataSourceState(state.clone()));
                    }
                    ModuleDataSourceStatus::Unavailable => {
                        modules.handle(ModuleRuntimeIntent::MarkDataSourceUnavailable(identity));
                    }
                    ModuleDataSourceStatus::PendingFirstEmission => {}
                }
            }
        }
    }

    pub(super) fn subscriptions(&self, shared: &SharedContext) -> iced::Subscription<Message> {
        iced::Subscription::batch(
            SOURCE_CATALOG
                .iter()
                .map(|entry| (entry.subscription)(self.has_source(entry.identity), shared)),
        )
    }

    #[cfg(test)]
    pub(super) fn is_pending(&self, identity: &ModuleDataSourceIdentity) -> bool {
        matches!(
            self.sources.get(identity),
            Some(ModuleDataSourceStatus::PendingFirstEmission)
        )
    }

    #[cfg(test)]
    pub(super) fn active_identities_for_test(&self) -> Vec<ModuleDataSourceIdentity> {
        ModuleDataSourceIdentity::ALL
            .into_iter()
            .filter(|identity| self.has_source(*identity))
            .collect()
    }

    pub(super) fn handle_battery_event(
        &mut self,
        event: BatteryEvent,
    ) -> ModuleDataSourceTransition {
        match event {
            BatteryEvent::StateChanged {
                percentage,
                power_state,
                present,
            } => self.apply_source_state(ModuleDataSourceState::Battery(BatteryReading {
                percentage,
                power_state,
                present,
            })),
            BatteryEvent::Unavailable => self.mark_unavailable(ModuleDataSourceIdentity::Battery),
        }
    }

    pub(super) fn handle_network_event(
        &mut self,
        event: NetworkEvent,
    ) -> ModuleDataSourceTransition {
        match event {
            NetworkEvent::StateChanged {
                connectivity,
                label,
            } => self.apply_source_state(ModuleDataSourceState::Network(NetworkReading {
                connectivity,
                label,
            })),
            NetworkEvent::Unavailable => self.mark_unavailable(ModuleDataSourceIdentity::Network),
        }
    }

    pub(super) fn handle_media_event(
        &mut self,
        snapshot: canopa_shell::media::Snapshot,
    ) -> ModuleDataSourceTransition {
        if snapshot.available {
            self.apply_source_state(ModuleDataSourceState::Media(snapshot))
        } else {
            self.mark_unavailable(ModuleDataSourceIdentity::Media)
        }
    }

    pub(super) fn handle_notifications_event(
        &mut self,
        summary: Option<canopa_notify_client::Summary>,
    ) -> ModuleDataSourceTransition {
        match summary {
            Some(summary) => self.apply_source_state(ModuleDataSourceState::Notifications(summary)),
            None => self.mark_unavailable(ModuleDataSourceIdentity::Notifications),
        }
    }

    pub(super) fn handle_stasis_event(
        &mut self,
        snapshot: Option<canopa_widgets::idle::Snapshot>,
    ) -> ModuleDataSourceTransition {
        match snapshot {
            Some(snapshot) => self.apply_source_state(ModuleDataSourceState::Stasis(snapshot)),
            None => self.mark_unavailable(ModuleDataSourceIdentity::Stasis),
        }
    }

    pub(super) fn handle_volume_event(&mut self, event: VolumeEvent) -> ModuleDataSourceTransition {
        match event {
            VolumeEvent::StateChanged { volume, muted } => {
                self.apply_source_state(ModuleDataSourceState::Volume(VolumeReading {
                    volume,
                    muted,
                }))
            }
            VolumeEvent::Unavailable => self.mark_unavailable(ModuleDataSourceIdentity::Volume),
        }
    }

    fn has_source(&self, identity: ModuleDataSourceIdentity) -> bool {
        self.sources.contains_key(&identity)
    }

    pub(super) fn apply_source_state(
        &mut self,
        state: ModuleDataSourceState,
    ) -> ModuleDataSourceTransition {
        let identity = state.identity();
        if let Some(status) = self.sources.get_mut(&identity) {
            *status = ModuleDataSourceStatus::Available(state.clone());
        }
        ModuleDataSourceTransition::Available(state)
    }

    fn mark_unavailable(
        &mut self,
        identity: ModuleDataSourceIdentity,
    ) -> ModuleDataSourceTransition {
        if let Some(status) = self.sources.get_mut(&identity) {
            *status = ModuleDataSourceStatus::Unavailable;
        }
        ModuleDataSourceTransition::Unavailable(identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::{BatteryConfig, Config, ModuleConfig, ModuleParams};
    use crate::modules::data_source::{
        BatteryReading, ModuleDataSourceIdentity, ModuleDataSourceState,
    };

    fn app_with_two_battery_modules() -> App {
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "battery-main".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(BatteryConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "battery-side".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(BatteryConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Config::default()
        };
        config.bars[0].modules.end = vec!["battery-main".into(), "battery-side".into()];
        App::boot(config, vec![]).0
    }

    #[test]
    fn notification_instances_share_one_source_and_recover_together() {
        let mut config = Config::default();
        config.bars[0].modules.end = vec!["first".into(), "second".into()];
        config.modules = ["first", "second"]
            .into_iter()
            .map(|name| ModuleConfig {
                name: name.into(),
                module_type: "notifications".into(),
                params: ModuleParams::Notifications(Default::default()),
                actions: Default::default(),
            })
            .collect();
        let (mut app, _) = App::boot(config, vec![]);
        assert_eq!(
            app.data_sources.active_identities_for_test(),
            vec![ModuleDataSourceIdentity::Notifications]
        );
        let summary = canopa_notify_client::Summary {
            instance: [2; 16],
            requires_attention: 9,
            do_not_disturb: true,
        };
        for reading in [Some(summary), None, Some(summary)] {
            let _ = app.handle_module_runtime_event(
                crate::app::messages::ModuleRuntimeEvent::Notifications(reading),
            );
            for name in ["first", "second"] {
                assert_eq!(
                    app.modules.state(&crate::modules::ModuleId::new(name)),
                    Some(&reading.map_or(
                        crate::modules::ModuleState::Unavailable,
                        crate::modules::ModuleState::Notifications
                    ))
                );
            }
        }
    }

    #[test]
    fn notification_source_clears_stale_state_and_accepts_a_new_daemon_snapshot() {
        let mut config = Config::default();
        config.bars[0].modules.end = vec!["notifications".into()];
        let (app, _) = App::boot(config, vec![]);
        let mut runtime = app.data_sources;
        let first = canopa_notify_client::Summary {
            instance: [1; 16],
            requires_attention: 42,
            do_not_disturb: true,
        };
        runtime.handle_notifications_event(Some(first));
        assert!(matches!(
            runtime.handle_notifications_event(None),
            ModuleDataSourceTransition::Unavailable(ModuleDataSourceIdentity::Notifications)
        ));
        let new = canopa_notify_client::Summary {
            instance: [2; 16],
            requires_attention: 1,
            do_not_disturb: false,
        };
        assert_eq!(
            runtime.handle_notifications_event(Some(new)),
            ModuleDataSourceTransition::Available(ModuleDataSourceState::Notifications(new))
        );
        assert_eq!(runtime.sources.len(), 1);
    }

    #[test]
    fn runtime_initializes_pending_sources_from_modules() {
        let app = app_with_two_battery_modules();

        assert!(
            app.data_sources
                .is_pending(&ModuleDataSourceIdentity::Battery)
        );
    }

    #[test]
    fn active_identities_are_initialized_from_modules() {
        let app = app_with_two_battery_modules();

        assert_eq!(
            app.data_sources.active_identities_for_test(),
            vec![ModuleDataSourceIdentity::Battery]
        );
    }

    #[test]
    fn battery_event_returns_available_source_transition() {
        let app = app_with_two_battery_modules();
        let mut runtime = ModuleDataSourceRuntime::from_modules(&app.modules);

        let transition = runtime.handle_battery_event(BatteryEvent::StateChanged {
            percentage: 88.0,
            power_state: crate::modules::battery::BatteryPowerState::Discharging,
            present: true,
        });

        assert_eq!(
            transition,
            ModuleDataSourceTransition::Available(ModuleDataSourceState::Battery(BatteryReading {
                percentage: 88.0,
                power_state: crate::modules::battery::BatteryPowerState::Discharging,
                present: true,
            }))
        );
    }

    #[test]
    fn unavailable_battery_event_returns_unavailable_source_transition() {
        let app = app_with_two_battery_modules();
        let mut runtime = ModuleDataSourceRuntime::from_modules(&app.modules);

        let transition = runtime.handle_battery_event(BatteryEvent::Unavailable);

        assert_eq!(
            transition,
            ModuleDataSourceTransition::Unavailable(ModuleDataSourceIdentity::Battery)
        );
    }
}
