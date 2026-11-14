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
    /// True once Niri has sent its window list since connecting.
    pub windows_known: bool,
    /// Logical (width, height) of each output by name, from Niri's output list.
    pub output_sizes: std::collections::HashMap<String, (f64, f64)>,
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
    #[cfg(test)]
    pub fn active_window_title_for_output(&self, output: &str) -> Option<String> {
        self.active_window_for_output(output)
            .and_then(|w| w.title.clone())
    }

    /// Returns the active window on the given output, or `None` if there is no
    /// active workspace, no active window, or the active window is missing from
    /// the window cache.
    pub fn active_window_for_output(&self, output: &str) -> Option<&Window> {
        self.workspaces
            .iter()
            .find(|ws| ws.is_active && ws.output.as_deref() == Some(output))
            .and_then(|ws| ws.active_window_id)
            .and_then(|id| self.windows.get(&id))
    }

    /// Returns the globally focused window by `focused_window_id`, or `None`
    /// when there is no focused window or the cached window is missing.
    pub fn focused_window(&self) -> Option<&Window> {
        self.focused_window_id.and_then(|id| self.windows.get(&id))
    }

    /// Returns the focused window for module display, falling back to the
    /// active window on the focused workspace when a layer-shell surface has
    /// keyboard focus and niri reports no focused toplevel window.
    pub fn focused_or_active_window(&self) -> Option<&Window> {
        self.focused_window().or_else(|| {
            self.focused_output()
                .and_then(|output| self.active_window_for_output(output))
        })
    }
}

/// The logical size of each enabled output, by name, or `None` when Niri
/// cannot be asked.
pub async fn output_sizes() -> Option<std::collections::HashMap<String, (f64, f64)>> {
    tokio::task::spawn_blocking(|| {
        let mut socket = niri_ipc::socket::Socket::connect().ok()?;
        match socket.send(niri_ipc::Request::Outputs).ok()? {
            Ok(niri_ipc::Response::Outputs(outputs)) => Some(
                outputs
                    .into_iter()
                    .filter_map(|(name, output)| {
                        output.logical.map(|logical| {
                            (name, (f64::from(logical.width), f64::from(logical.height)))
                        })
                    })
                    .collect(),
            ),
            _ => None,
        }
    })
    .await
    .ok()
    .flatten()
}

/// Where the focused window sits in the scrolling layout, all 1-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutPlace {
    pub workspace_id: u64,
    pub workspace: u8,
    pub column: usize,
    pub columns: usize,
    pub tile: usize,
    pub tiles: usize,
}

impl NiriState {
    /// The keyboard-focused tiled window's place, or `None` for floating
    /// windows, no focus, or a Shell surface holding the keyboard.
    pub fn focused_place(&self) -> Option<LayoutPlace> {
        let window = self.focused_window()?;
        let (column, tile) = window.layout.pos_in_scrolling_layout?;
        let workspace_id = window.workspace_id?;
        let workspace = self
            .workspaces
            .iter()
            .find(|workspace| workspace.id == workspace_id)?;
        let tiled = || {
            self.windows.values().filter_map(|other| {
                (other.workspace_id == Some(workspace_id))
                    .then_some(other.layout.pos_in_scrolling_layout)
                    .flatten()
            })
        };
        Some(LayoutPlace {
            workspace_id,
            workspace: workspace.idx,
            column,
            columns: tiled().map(|(column, _)| column).max().unwrap_or(column),
            tile,
            tiles: tiled()
                .filter(|(other, _)| *other == column)
                .count()
                .max(tile),
        })
    }
}

/// Events emitted by the niri IPC subscription into the Iced message bus.
#[derive(Debug, Clone)]
// Moved once per IPC line into the message bus; see `DecodedNiriEvent`.
#[allow(clippy::large_enum_variant)]
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
            parent_id: None,
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
                pinned: None,
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
    fn active_window_for_output_returns_full_window() {
        let mut state = NiriState::default();
        let mut window = make_window(5, Some("Helix"));
        window.app_id = Some("helix.desktop".into());
        state.workspaces = vec![make_workspace(1, "eDP-1", true, true, Some(5))];
        state.windows.insert(5, window);

        let active = state
            .active_window_for_output("eDP-1")
            .expect("active window");

        assert_eq!(active.title.as_deref(), Some("Helix"));
        assert_eq!(active.app_id.as_deref(), Some("helix.desktop"));
    }

    #[test]
    fn focused_window_returns_window_by_focused_id() {
        let mut state = NiriState::default();
        state.focused_window_id = Some(10);
        state.windows.insert(10, make_window(10, Some("Firefox")));

        let focused = state.focused_window().expect("focused window");

        assert_eq!(focused.title.as_deref(), Some("Firefox"));
    }

    #[test]
    fn focused_window_returns_none_for_cached_window_miss() {
        let mut state = NiriState::default();
        state.focused_window_id = Some(10);

        assert!(state.focused_window().is_none());
    }

    #[test]
    fn focused_or_active_window_returns_focused_window_first() {
        let mut state = NiriState::default();
        state.focused_window_id = Some(10);
        state.workspaces = vec![make_workspace(1, "eDP-1", true, true, Some(5))];
        state.windows.insert(5, make_window(5, Some("Active")));
        state.windows.insert(10, make_window(10, Some("Focused")));

        let window = state.focused_or_active_window().expect("window");

        assert_eq!(window.title.as_deref(), Some("Focused"));
    }

    #[test]
    fn focused_or_active_window_falls_back_to_focused_workspace_active_window() {
        let mut state = NiriState::default();
        state.workspaces = vec![make_workspace(1, "eDP-1", true, true, Some(5))];
        state.windows.insert(5, make_window(5, Some("Active")));

        let window = state.focused_or_active_window().expect("window");

        assert_eq!(window.title.as_deref(), Some("Active"));
    }

    #[test]
    fn focused_or_active_window_returns_none_without_focused_or_active_window() {
        let mut state = NiriState::default();
        state.workspaces = vec![make_workspace(1, "eDP-1", true, true, None)];

        assert!(state.focused_or_active_window().is_none());
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
