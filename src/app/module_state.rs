use super::App;
use crate::modules::ModuleState;
use crate::modules::battery::BatteryState;
use crate::modules::focused_window;
use crate::modules::network::{NetworkConnectivity, NetworkState};
use crate::modules::volume::VolumeState;

impl App {
    /// Push new battery state to all battery module instances in shared.modules.
    pub(super) fn push_battery_state(&mut self, percentage: f64, is_charging: bool) {
        for (_, module) in &self.modules {
            if module.module_type() == "battery" {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::Battery(BatteryState {
                        percentage,
                        is_charging,
                    }),
                );
            }
        }
    }

    pub(super) fn push_network_state(
        &mut self,
        connectivity: NetworkConnectivity,
        label: Option<String>,
    ) {
        for (_, module) in &self.modules {
            if module.module_type() == "network" {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::Network(NetworkState {
                        connectivity: connectivity.clone(),
                        label: label.clone(),
                    }),
                );
            }
        }
    }

    pub(super) fn push_volume_state(&mut self, volume: f64, muted: bool) {
        for (_, module) in &self.modules {
            if module.module_type() == "volume" {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::Volume(VolumeState { volume, muted }),
                );
            }
        }
    }

    /// Derive the current focused window title from NiriState and push it to all
    /// focused-window module instances in shared.modules.
    pub(super) fn push_focused_window_state(&mut self) {
        let title = self
            .shared
            .niri
            .focused_window_id
            .and_then(|id| self.shared.niri.windows.get(&id))
            .and_then(|w| w.title.clone());
        for (_, module) in &self.modules {
            if module.module_type() == "focused-window" {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::FocusedWindow(focused_window::FocusedWindowState {
                        title: title.clone(),
                    }),
                );
            }
        }
    }
}
