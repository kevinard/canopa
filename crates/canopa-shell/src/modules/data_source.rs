use super::ModuleState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleDataSourceIdentity {
    Media,
    IdleInhibitor,
    Stasis,
    Battery,
    Network,
    Notifications,
    Volume,
}

pub trait ModuleDataSourceMapper: Send + Sync {
    fn data_source_identity(&self) -> ModuleDataSourceIdentity;

    fn map_data_source_state(&self, state: &ModuleDataSourceState) -> ModuleInstanceOutcome;
}

impl ModuleDataSourceIdentity {
    pub const ALL: [Self; 7] = [
        Self::Media,
        Self::IdleInhibitor,
        Self::Stasis,
        Self::Battery,
        Self::Network,
        Self::Volume,
        Self::Notifications,
    ];
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatteryReading {
    pub percentage: f64,
    pub power_state: crate::modules::battery::BatteryPowerState,
    pub present: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkReading {
    pub connectivity: crate::modules::network::NetworkConnectivity,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VolumeReading {
    pub volume: f64,
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleDataSourceState {
    Media(canopa_shell::media::Snapshot),
    IdleInhibitor(super::idle_inhibitor::IdleInhibitorState),
    Stasis(canopa_widgets::idle::Snapshot),
    Battery(BatteryReading),
    Network(NetworkReading),
    Notifications(canopa_notify_client::Summary),
    Volume(VolumeReading),
}

impl ModuleDataSourceState {
    pub fn identity(&self) -> ModuleDataSourceIdentity {
        match self {
            Self::Stasis(_) => ModuleDataSourceIdentity::Stasis,
            Self::IdleInhibitor(_) => ModuleDataSourceIdentity::IdleInhibitor,
            Self::Battery(_) => ModuleDataSourceIdentity::Battery,
            Self::Media(_) => ModuleDataSourceIdentity::Media,
            Self::Notifications(_) => ModuleDataSourceIdentity::Notifications,
            Self::Network(_) => ModuleDataSourceIdentity::Network,
            Self::Volume(_) => ModuleDataSourceIdentity::Volume,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleInstanceOutcome {
    Visible(ModuleState),
    Empty,
    Unavailable,
}

impl ModuleInstanceOutcome {
    pub fn into_module_state(self) -> ModuleState {
        match self {
            Self::Visible(state) => state,
            Self::Empty => ModuleState::Empty,
            Self::Unavailable => ModuleState::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_state_reports_its_identity() {
        assert_eq!(
            ModuleDataSourceState::Battery(BatteryReading {
                percentage: 42.0,
                power_state: crate::modules::battery::BatteryPowerState::Discharging,
                present: true,
            })
            .identity(),
            ModuleDataSourceIdentity::Battery
        );
        assert_eq!(
            ModuleDataSourceState::Network(NetworkReading {
                connectivity: crate::modules::network::NetworkConnectivity::Full,
                label: Some("wifi".into()),
            })
            .identity(),
            ModuleDataSourceIdentity::Network
        );
        assert_eq!(
            ModuleDataSourceState::Volume(VolumeReading {
                volume: 55.0,
                muted: false,
            })
            .identity(),
            ModuleDataSourceIdentity::Volume
        );
    }

    #[test]
    fn outcome_converts_to_existing_runtime_state() {
        assert_eq!(
            ModuleInstanceOutcome::Empty.into_module_state(),
            crate::modules::ModuleState::Empty
        );
        assert_eq!(
            ModuleInstanceOutcome::Unavailable.into_module_state(),
            crate::modules::ModuleState::Unavailable
        );
    }
}
