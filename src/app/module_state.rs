use super::App;
use crate::modules::focused_window;
use crate::modules::{ModuleKind, ModuleState};

impl App {
    /// Derive the current focused window title from NiriState and push it to all
    /// focused-window module instances in ModuleRuntime.
    pub(super) fn push_focused_window_state(&mut self) {
        let title = self
            .niri
            .state()
            .focused_window_id
            .and_then(|id| self.niri.state().windows.get(&id))
            .and_then(|w| w.title.clone());
        let _ = self.modules.handle(
            super::module_runtime::ModuleRuntimeIntent::ApplyStateToKind {
                kind: ModuleKind::FocusedWindow,
                state: ModuleState::FocusedWindow(focused_window::FocusedWindowState { title }),
            },
        );
    }
}
