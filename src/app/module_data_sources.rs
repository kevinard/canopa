use std::collections::HashMap;

use crate::modules::ModuleId;
use crate::modules::battery::BatteryEvent;
use crate::modules::data_source::{
    BatteryReading, ModuleDataSourceIdentity, ModuleDataSourceState, ModuleInstanceOutcome,
    NetworkReading, VolumeReading,
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

#[derive(Debug, Default)]
pub(super) struct ModuleDataSourceRuntime {
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
        modules: &super::module_runtime::ModuleRuntime,
    ) -> Vec<(ModuleId, ModuleInstanceOutcome)> {
        match event {
            BatteryEvent::StateChanged {
                percentage,
                is_charging,
            } => self.apply_source_state(
                ModuleDataSourceState::Battery(BatteryReading {
                    percentage,
                    is_charging,
                }),
                modules,
            ),
            BatteryEvent::Unavailable => {
                self.mark_unavailable(ModuleDataSourceIdentity::Battery, modules)
            }
        }
    }

    pub(super) fn handle_network_event(
        &mut self,
        event: NetworkEvent,
        modules: &super::module_runtime::ModuleRuntime,
    ) -> Vec<(ModuleId, ModuleInstanceOutcome)> {
        match event {
            NetworkEvent::StateChanged {
                connectivity,
                label,
            } => self.apply_source_state(
                ModuleDataSourceState::Network(NetworkReading {
                    connectivity,
                    label,
                }),
                modules,
            ),
            NetworkEvent::Unavailable => {
                self.mark_unavailable(ModuleDataSourceIdentity::Network, modules)
            }
        }
    }

    pub(super) fn handle_volume_event(
        &mut self,
        event: VolumeEvent,
        modules: &super::module_runtime::ModuleRuntime,
    ) -> Vec<(ModuleId, ModuleInstanceOutcome)> {
        match event {
            VolumeEvent::StateChanged { volume, muted } => self.apply_source_state(
                ModuleDataSourceState::Volume(VolumeReading { volume, muted }),
                modules,
            ),
            VolumeEvent::Unavailable => {
                self.mark_unavailable(ModuleDataSourceIdentity::Volume, modules)
            }
        }
    }

    fn has_source(&self, identity: ModuleDataSourceIdentity) -> bool {
        self.sources.contains_key(&identity)
    }

    fn apply_source_state(
        &mut self,
        state: ModuleDataSourceState,
        modules: &super::module_runtime::ModuleRuntime,
    ) -> Vec<(ModuleId, ModuleInstanceOutcome)> {
        let identity = state.identity();
        self.sources
            .insert(identity, ModuleDataSourceStatus::Available(state.clone()));
        self.map_dependents(identity, &state, modules)
    }

    fn mark_unavailable(
        &mut self,
        identity: ModuleDataSourceIdentity,
        modules: &super::module_runtime::ModuleRuntime,
    ) -> Vec<(ModuleId, ModuleInstanceOutcome)> {
        self.sources
            .insert(identity, ModuleDataSourceStatus::Unavailable);
        modules
            .data_source_dependents(identity)
            .into_iter()
            .map(|id| (id, ModuleInstanceOutcome::Unavailable))
            .collect()
    }

    fn map_dependents(
        &self,
        identity: ModuleDataSourceIdentity,
        state: &ModuleDataSourceState,
        modules: &super::module_runtime::ModuleRuntime,
    ) -> Vec<(ModuleId, ModuleInstanceOutcome)> {
        modules
            .data_source_dependents(identity)
            .into_iter()
            .filter_map(|id| {
                modules
                    .get(&id)
                    .map(|module| (id, module.map_data_source_state(state)))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::{BatteryConfig, Config, ModuleConfig, ModuleParams};
    use crate::modules::data_source::{ModuleDataSourceIdentity, ModuleInstanceOutcome};
    use crate::modules::{ModuleId, ModuleState};

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
    fn battery_event_fans_out_to_dependent_modules() {
        let app = app_with_two_battery_modules();
        let mut runtime = ModuleDataSourceRuntime::from_modules(&app.modules);

        let updates = runtime.handle_battery_event(
            BatteryEvent::StateChanged {
                percentage: 88.0,
                is_charging: false,
            },
            &app.modules,
        );

        assert_eq!(updates.len(), 2);
        assert!(updates.iter().any(|(id, outcome)| {
            id == &ModuleId::new("battery-main")
                && matches!(
                    outcome,
                    ModuleInstanceOutcome::Visible(ModuleState::Battery(state))
                    if (state.percentage - 88.0).abs() < 0.001 && !state.is_charging
                )
        }));
        assert!(updates.iter().any(|(id, outcome)| {
            id == &ModuleId::new("battery-side")
                && matches!(
                    outcome,
                    ModuleInstanceOutcome::Visible(ModuleState::Battery(state))
                    if (state.percentage - 88.0).abs() < 0.001 && !state.is_charging
                )
        }));
    }

    #[test]
    fn unavailable_battery_event_fans_out_unavailable_outcomes() {
        let app = app_with_two_battery_modules();
        let mut runtime = ModuleDataSourceRuntime::from_modules(&app.modules);

        let updates = runtime.handle_battery_event(BatteryEvent::Unavailable, &app.modules);

        assert_eq!(
            updates,
            vec![
                (
                    ModuleId::new("battery-main"),
                    ModuleInstanceOutcome::Unavailable
                ),
                (
                    ModuleId::new("battery-side"),
                    ModuleInstanceOutcome::Unavailable
                ),
            ]
        );
    }
}
