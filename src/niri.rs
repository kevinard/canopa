pub mod subscription;

use niri_ipc::{Event, Window, Workspace};

/// Live Niri compositor state stored in shared app state.
///
/// Populated by the niri subscription; consumed by the workspace and focused-window modules
/// (Stories 4.2 and 4.4). Until a WorkspacesChanged event arrives, workspaces is empty.
#[derive(Debug, Clone, Default)]
pub struct NiriState {
    /// Current workspace list in Niri's scrollable-tiling order (matches FR18).
    pub workspaces: Vec<Workspace>,
    /// ID of the currently focused window, or None if no window is focused.
    pub focused_window_id: Option<u64>,
    /// Cache of all open windows, keyed by window ID.
    /// Populated by WindowsChanged and WindowOpenedOrChanged events.
    /// Used to look up the title of the currently focused window.
    pub windows: std::collections::HashMap<u64, Window>,
    /// True once the IPC connection has established and initial state received.
    pub connected: bool,
}

impl NiriState {
    /// Returns the output name of the currently focused workspace,
    /// i.e., the output where the globally focused window lives.
    pub fn focused_output(&self) -> Option<&str> {
        self.workspaces
            .iter()
            .find(|ws| ws.is_focused)
            .and_then(|ws| ws.output.as_deref())
    }

    /// Returns the title of the active window on the given output, or `None` if
    /// there is no active workspace or active window on that output.
    pub fn active_window_title_for_output(&self, output: &str) -> Option<String> {
        self.workspaces
            .iter()
            .find(|ws| ws.is_active && ws.output.as_deref() == Some(output))
            .and_then(|ws| ws.active_window_id)
            .and_then(|id| self.windows.get(&id))
            .and_then(|w| w.title.clone())
    }
}

/// Events emitted by the niri IPC subscription into the Iced message bus.
#[derive(Debug, Clone)]
pub enum NiriEvent {
    /// Connection established; niri will immediately follow with WorkspacesChanged etc.
    Connected,
    /// Niri IPC socket unavailable — running on a non-Niri compositor. Graceful degradation.
    /// This is NOT an error; it is a valid operational state (FR21, architecture §Niri IPC).
    Unavailable,
    /// A compositor event from the niri-ipc event stream.
    Event(Event),
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn niri_state_default_not_connected() {
        let state = NiriState::default();
        assert!(!state.connected);
        assert!(state.workspaces.is_empty());
        assert!(state.focused_window_id.is_none());
    }

    #[test]
    fn niri_state_windows_default_empty() {
        let state = NiriState::default();
        assert!(state.windows.is_empty());
    }

    #[test]
    fn niri_state_workspaces_updated() {
        let mut state = NiriState::default();
        let ws = Workspace {
            id: 1,
            idx: 0,
            name: None,
            output: Some("eDP-1".to_string()),
            is_urgent: false,
            is_active: true,
            is_focused: true,
            active_window_id: None,
        };
        state.workspaces = vec![ws];
        assert_eq!(state.workspaces.len(), 1);
    }

    fn make_workspace(
        id: u64,
        output: &str,
        is_active: bool,
        is_focused: bool,
        active_window_id: Option<u64>,
    ) -> Workspace {
        Workspace {
            id,
            idx: 0,
            name: None,
            output: Some(output.to_string()),
            is_urgent: false,
            is_active,
            is_focused,
            active_window_id,
        }
    }

    fn make_window(id: u64, title: Option<&str>) -> Window {
        Window {
            id,
            title: title.map(|t| t.to_string()),
            app_id: None,
            pid: None,
            workspace_id: None,
            is_focused: false,
            is_floating: false,
            is_urgent: false,
            layout: niri_ipc::WindowLayout {
                pos_in_scrolling_layout: None,
                tile_size: (0.0, 0.0),
                window_size: (0, 0),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
            },
            focus_timestamp: None,
        }
    }

    // --- focused_output() ---

    #[test]
    fn focused_output_returns_none_when_no_workspaces() {
        let state = NiriState::default();
        assert!(state.focused_output().is_none());
    }

    #[test]
    fn focused_output_returns_output_of_focused_workspace() {
        let mut state = NiriState::default();
        state.workspaces = vec![
            make_workspace(1, "eDP-1", true, false, None),
            make_workspace(2, "DP-1", true, true, None),
        ];
        assert_eq!(state.focused_output(), Some("DP-1"));
    }

    #[test]
    fn focused_output_returns_none_when_no_workspace_is_focused() {
        let mut state = NiriState::default();
        state.workspaces = vec![make_workspace(1, "eDP-1", true, false, None)];
        assert!(state.focused_output().is_none());
    }

    // --- active_window_title_for_output() ---

    #[test]
    fn active_window_title_returns_none_when_no_workspaces() {
        let state = NiriState::default();
        assert!(state.active_window_title_for_output("eDP-1").is_none());
    }

    #[test]
    fn active_window_title_returns_none_when_no_active_workspace_for_output() {
        let mut state = NiriState::default();
        state.workspaces = vec![make_workspace(1, "DP-1", true, true, Some(10))];
        state.windows.insert(10, make_window(10, Some("Firefox")));
        assert!(state.active_window_title_for_output("eDP-1").is_none());
    }

    #[test]
    fn active_window_title_returns_title_of_active_window() {
        let mut state = NiriState::default();
        state.workspaces = vec![
            make_workspace(1, "eDP-1", true, false, Some(5)),
            make_workspace(2, "DP-1", true, true, Some(10)),
        ];
        state.windows.insert(5, make_window(5, Some("Helix")));
        state.windows.insert(10, make_window(10, Some("Firefox")));
        assert_eq!(
            state.active_window_title_for_output("eDP-1"),
            Some("Helix".to_string())
        );
        assert_eq!(
            state.active_window_title_for_output("DP-1"),
            Some("Firefox".to_string())
        );
    }

    #[test]
    fn active_window_title_returns_none_when_active_window_not_in_cache() {
        let mut state = NiriState::default();
        state.workspaces = vec![make_workspace(1, "eDP-1", true, true, Some(99))];
        // window 99 not in cache
        assert!(state.active_window_title_for_output("eDP-1").is_none());
    }

    #[test]
    fn active_window_title_returns_none_when_workspace_has_no_active_window() {
        let mut state = NiriState::default();
        state.workspaces = vec![make_workspace(1, "eDP-1", true, true, None)];
        assert!(state.active_window_title_for_output("eDP-1").is_none());
    }
}
