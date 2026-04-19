use super::App;
use crate::modules::battery::BatteryState;
use crate::modules::focused_window;
use crate::modules::network::{NetworkConnectivity, NetworkState};
use crate::modules::volume::VolumeState;
use crate::modules::{ModuleKind, ModuleState};

impl App {
    fn push_state_if_changed(&mut self, id: crate::modules::ModuleId, state: ModuleState) {
        if self.shared.modules.get(&id) != Some(&state) {
            self.shared.modules.insert(id, state);
        }
    }

    /// Push new battery state to all battery module instances in shared.modules.
    pub(super) fn push_battery_state(&mut self, percentage: f64, is_charging: bool) {
        let ids: Vec<_> = self.modules.ids_by_kind(ModuleKind::Battery).to_vec();
        for id in ids {
            self.push_state_if_changed(
                id,
                ModuleState::Battery(BatteryState {
                    percentage,
                    is_charging,
                }),
            );
        }
    }

    pub(super) fn push_network_state(
        &mut self,
        connectivity: NetworkConnectivity,
        label: Option<String>,
    ) {
        let ids: Vec<_> = self.modules.ids_by_kind(ModuleKind::Network).to_vec();
        for id in ids {
            self.push_state_if_changed(
                id,
                ModuleState::Network(NetworkState {
                    connectivity: connectivity.clone(),
                    label: label.clone(),
                }),
            );
        }
    }

    pub(super) fn push_volume_state(&mut self, volume: f64, muted: bool) {
        let ids: Vec<_> = self.modules.ids_by_kind(ModuleKind::Volume).to_vec();
        for id in ids {
            self.push_state_if_changed(id, ModuleState::Volume(VolumeState { volume, muted }));
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
        let ids: Vec<_> = self
            .modules
            .ids_by_kind(ModuleKind::FocusedWindow)
            .to_vec();
        for id in ids {
            self.push_state_if_changed(
                id,
                ModuleState::FocusedWindow(focused_window::FocusedWindowState {
                    title: title.clone(),
                }),
            );
        }
    }
}
