use std::collections::HashMap;

use crate::modules::ModuleId;
use crate::modules::data_source::{
    ModuleDataSourceIdentity, ModuleDataSourceState, ModuleInstanceOutcome,
};

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
        for identity in [
            ModuleDataSourceIdentity::Battery,
            ModuleDataSourceIdentity::Network,
            ModuleDataSourceIdentity::Volume,
        ] {
            if modules.has_data_source(identity) {
                runtime
                    .sources
                    .insert(identity, ModuleDataSourceStatus::PendingFirstEmission);
            }
        }
        runtime
    }

    #[cfg(test)]
    pub(super) fn is_pending(&self, identity: &ModuleDataSourceIdentity) -> bool {
        matches!(
            self.sources.get(identity),
            Some(ModuleDataSourceStatus::PendingFirstEmission)
        )
    }

    pub(super) fn apply_source_state(
        &mut self,
        state: ModuleDataSourceState,
        modules: &super::module_runtime::ModuleRuntime,
    ) -> Vec<(ModuleId, ModuleInstanceOutcome)> {
        let identity = state.identity();
        self.sources
            .insert(identity, ModuleDataSourceStatus::Available(state.clone()));
        self.map_dependents(identity, &state, modules)
    }

    pub(super) fn mark_unavailable(
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
    use crate::modules::data_source::{
        BatteryReading, ModuleDataSourceIdentity, ModuleDataSourceState, ModuleInstanceOutcome,
    };
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
    fn available_source_state_fans_out_to_dependent_modules() {
        let app = app_with_two_battery_modules();
        let mut runtime = ModuleDataSourceRuntime::from_modules(&app.modules);

        let updates = runtime.apply_source_state(
            ModuleDataSourceState::Battery(BatteryReading {
                percentage: 88.0,
                is_charging: false,
            }),
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
    fn unavailable_source_fans_out_unavailable_outcomes() {
        let app = app_with_two_battery_modules();
        let mut runtime = ModuleDataSourceRuntime::from_modules(&app.modules);

        let updates = runtime.mark_unavailable(ModuleDataSourceIdentity::Battery, &app.modules);

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
