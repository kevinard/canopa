use super::App;
use crate::modules::focused_window;
use crate::modules::{ModuleKind, ModuleState};

impl App {
    fn push_state_if_changed(&mut self, id: crate::modules::ModuleId, state: ModuleState) {
        if self.modules.state(&id) != Some(&state) {
            self.modules.set_state(id, state);
        }
    }

    /// Derive the current focused window title from NiriState and push it to all
    /// focused-window module instances in ModuleRuntime.
    pub(super) fn push_focused_window_state(&mut self) {
        let title = self
            .niri
            .state()
            .focused_window_id
            .and_then(|id| self.niri.state().windows.get(&id))
            .and_then(|w| w.title.clone());
        let ids: Vec<_> = self.modules.ids_by_kind(ModuleKind::FocusedWindow).to_vec();
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
