pub mod subscription;

use niri_ipc::{Event, Workspace};

/// Live Niri compositor state stored in SharedState.
///
/// Populated by the niri subscription; consumed by the workspace and focused-window modules
/// (Stories 4.2 and 4.4). Until a WorkspacesChanged event arrives, workspaces is empty.
#[derive(Debug, Clone, Default)]
pub struct NiriState {
    /// Current workspace list in Niri's scrollable-tiling order (matches FR18).
    pub workspaces: Vec<Workspace>,
    /// ID of the currently focused window, or None if no window is focused.
    pub focused_window_id: Option<u64>,
    /// True once the IPC connection has established and initial state received.
    pub connected: bool,
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
}
