use super::App;
use crate::modules::focused_window;
use crate::modules::{ModuleKind, ModuleState};

impl App {
    pub(super) fn focused_window_info_from_window(
        window: &niri_ipc::Window,
    ) -> focused_window::FocusedWindowInfo {
        let application_icon = window
            .app_id
            .as_deref()
            .and_then(crate::focused_window_icons::resolve_application_icon);
        focused_window::FocusedWindowInfo::from_window(window, application_icon)
    }

    /// Derive the current focused window title from NiriState and push it to all
    /// focused-window module instances in ModuleRuntime.
    pub(super) fn push_focused_window_state(&mut self) {
        let window = self
            .niri
            .state()
            .focused_or_active_window()
            .map(Self::focused_window_info_from_window);
        let _ = self.modules.handle(
            super::module_runtime::ModuleRuntimeIntent::ApplyStateToKind {
                kind: ModuleKind::FocusedWindow,
                state: ModuleState::FocusedWindow(focused_window::FocusedWindowState { window }),
            },
        );
    }
}
