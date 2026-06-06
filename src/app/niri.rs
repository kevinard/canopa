use iced::Task;

use super::{App, Message};
use crate::modules::{ModuleKind, ModuleState};
use crate::niri::NiriEvent;

pub(super) struct AppNiriState {
    state: crate::niri::NiriState,
}

impl AppNiriState {
    pub(super) fn new() -> Self {
        Self {
            state: crate::niri::NiriState::default(),
        }
    }

    pub(super) fn state(&self) -> &crate::niri::NiriState {
        &self.state
    }

    pub(super) fn state_mut(&mut self) -> &mut crate::niri::NiriState {
        &mut self.state
    }
}

pub(super) fn workspace_render_state_equal(
    a: &[niri_ipc::Workspace],
    b: &[niri_ipc::Workspace],
) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(left, right)| {
            left.id == right.id
                && left.idx == right.idx
                && left.name == right.name
                && left.output == right.output
                && left.is_active == right.is_active
                && left.is_focused == right.is_focused
                && left.is_urgent == right.is_urgent
                && left.active_window_id == right.active_window_id
        })
}

impl App {
    pub(super) fn handle_niri_event(&mut self, event: NiriEvent) -> Task<Message> {
        match event {
            NiriEvent::Connected => {
                self.niri.state_mut().connected = true;
                tracing::info!(
                    target: "niribar::niri",
                    "niri ipc connected"
                );
                Task::none()
            }
            NiriEvent::Unavailable => {
                self.niri.state_mut().connected = false;
                let affected_module_count = self.modules.count_by_kind(ModuleKind::Workspaces)
                    + self.modules.count_by_kind(ModuleKind::FocusedWindow);
                tracing::info!(
                    target: "niribar::niri",
                    affected_module_count,
                    "niri ipc unavailable"
                );
                let _ = self.modules.handle(
                    super::module_runtime::ModuleRuntimeIntent::ApplyStateToKind {
                        kind: ModuleKind::Workspaces,
                        state: ModuleState::Unavailable,
                    },
                );
                let _ = self.modules.handle(
                    super::module_runtime::ModuleRuntimeIntent::ApplyStateToKind {
                        kind: ModuleKind::FocusedWindow,
                        state: ModuleState::Unavailable,
                    },
                );
                Task::none()
            }
            NiriEvent::Event(event) => {
                use niri_ipc::Event;
                match event {
                    Event::WorkspacesChanged { workspaces } => {
                        let changed = !workspace_render_state_equal(
                            &self.niri.state().workspaces,
                            &workspaces,
                        );
                        let open_tasks = if changed && self.surfaces.manual_outputs() {
                            self.open_new_output_windows(&workspaces)
                        } else {
                            vec![]
                        };
                        tracing::debug!(
                            target: "niribar::niri",
                            workspace_count = workspaces.len(),
                            changed,
                            surface_task_count = open_tasks.len(),
                            "niri workspaces changed"
                        );
                        if changed {
                            self.rebuild_workspace_item_modules(&workspaces);
                            self.niri.state_mut().workspaces = workspaces;
                        }
                        if !open_tasks.is_empty() {
                            return Task::batch(open_tasks);
                        }
                    }
                    Event::WindowFocusChanged { id } => {
                        tracing::trace!(
                            target: "niribar::niri",
                            focused = id.is_some(),
                            "niri focused window changed"
                        );
                        self.niri.state_mut().focused_window_id = id;
                        if id.is_some() {
                            self.push_focused_window_state();
                        }
                    }
                    Event::WorkspaceActivated { id, focused } => {
                        let output = self
                            .niri
                            .state()
                            .workspaces
                            .iter()
                            .find(|ws| ws.id == id)
                            .and_then(|ws| ws.output.clone());

                        for ws in &mut self.niri.state_mut().workspaces {
                            if output.is_some() && ws.output == output {
                                ws.is_active = ws.id == id;
                            }
                            if focused {
                                ws.is_focused = ws.id == id;
                            }
                        }

                        let updated = self.niri.state().workspaces.clone();
                        self.reconcile_workspace_dynamic_modules(&updated);
                    }
                    Event::WorkspaceActiveWindowChanged {
                        workspace_id,
                        active_window_id,
                    } => {
                        if let Some(workspace) = self
                            .niri
                            .state_mut()
                            .workspaces
                            .iter_mut()
                            .find(|workspace| workspace.id == workspace_id)
                        {
                            workspace.active_window_id = active_window_id;
                        }
                    }
                    Event::WindowsChanged { windows } => {
                        tracing::trace!(
                            target: "niribar::niri",
                            window_count = windows.len(),
                            focused_present = windows.iter().any(|w| w.is_focused),
                            "niri windows changed"
                        );
                        self.niri.state_mut().focused_window_id =
                            windows.iter().find(|w| w.is_focused).map(|w| w.id);
                        self.niri.state_mut().windows =
                            windows.into_iter().map(|w| (w.id, w)).collect();
                        self.push_focused_window_state();
                    }
                    Event::WindowOpenedOrChanged { window } => {
                        tracing::trace!(
                            target: "niribar::niri",
                            focused = window.is_focused,
                            "niri window opened or changed"
                        );
                        if window.is_focused {
                            self.niri.state_mut().focused_window_id = Some(window.id);
                        }
                        self.niri.state_mut().windows.insert(window.id, window);
                        self.push_focused_window_state();
                    }
                    Event::WindowClosed { id } => {
                        let had_focus = self.niri.state().focused_window_id == Some(id);
                        tracing::trace!(
                            target: "niribar::niri",
                            had_focus,
                            "niri window closed"
                        );
                        self.niri.state_mut().windows.remove(&id);
                        if had_focus {
                            self.niri.state_mut().focused_window_id = None;
                        }
                        self.push_focused_window_state();
                    }
                    _ => {}
                }
                Task::none()
            }
        }
    }

    pub(super) fn handle_app_niri_event(
        &mut self,
        event: super::messages::AppNiriEvent,
    ) -> Task<Message> {
        match event {
            super::messages::AppNiriEvent::Ipc(event) => self.handle_niri_event(event),
        }
    }

    /// Rebuild one `WorkspaceItemModule` per workspace for each `WorkspacesModule` placeholder.
    pub(super) fn rebuild_workspace_item_modules(&mut self, workspaces: &[niri_ipc::Workspace]) {
        if let Some(perf) = &self.perf {
            perf.record_workspace_rebuild();
        }

        self.reconcile_workspace_dynamic_modules(workspaces);
    }
}

#[cfg(test)]
mod tests {
    use super::super::messages::AppNiriEvent;
    use super::super::{App, Message};
    use crate::config::Config;
    use crate::modules::{ModuleId, ModuleKind, ModuleState};
    use crate::niri::NiriEvent;

    fn make_config() -> Config {
        Config::default()
    }

    // --- Story 4.1 tests ---

    #[test]
    fn niri_connected_event_sets_connected() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Connected)));
        assert!(app.niri.state().connected);
    }

    #[test]
    fn niri_unavailable_event_does_not_crash() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Unavailable)));
        assert!(!app.niri.state().connected);
    }

    #[test]
    fn niri_unavailable_sets_workspaces_module_unavailable() {
        let mut app = make_workspaces_app();
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Unavailable)));
        let id = ModuleId::new("workspaces");
        assert!(matches!(
            app.modules.state(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn niri_unavailable_sets_focused_window_module_unavailable() {
        let mut app = make_focused_window_app();
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Unavailable)));
        let id = ModuleId::new("focused-window");
        assert!(matches!(
            app.modules.state(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn niri_unavailable_does_not_affect_non_niri_modules() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.center = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Unavailable)));
        let id = ModuleId::new("clock");
        assert!(!matches!(
            app.modules.state(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn niri_workspaces_changed_stores_workspaces() {
        use niri_ipc::{Event, Workspace};
        let (mut app, _) = App::boot(make_config(), vec![]);
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
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged {
                workspaces: vec![ws],
            },
        ))));
        assert_eq!(app.niri.state().workspaces.len(), 1);
    }

    #[test]
    fn niri_window_focus_changed_stores_id() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowFocusChanged { id: Some(42) },
        ))));
        assert_eq!(app.niri.state().focused_window_id, Some(42));
    }

    #[test]
    fn niri_window_focus_cleared() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        app.niri.state_mut().focused_window_id = Some(99);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowFocusChanged { id: None },
        ))));
        assert_eq!(app.niri.state().focused_window_id, None);
    }

    #[test]
    fn niri_unknown_event_is_ignored() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::KeyboardLayoutsChanged {
                keyboard_layouts: niri_ipc::KeyboardLayouts {
                    names: vec![],
                    current_idx: 0,
                },
            },
        ))));
        assert!(!app.niri.state().connected);
        assert!(app.niri.state().workspaces.is_empty());
    }

    // --- Story 4.2 tests ---

    fn make_workspaces_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "workspaces".into(),
                module_type: "workspaces".into(),
                params: ModuleParams::None,
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["workspaces".into()];
        App::boot(config, vec![]).0
    }

    fn make_workspace(idx: u8, is_focused: bool) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id: idx as u64 + 1,
            idx,
            name: None,
            output: Some("eDP-1".to_string()),
            is_urgent: false,
            is_active: is_focused,
            is_focused,
            active_window_id: None,
        }
    }

    fn make_workspace_on_output(
        id: u64,
        idx: u8,
        output: &str,
        is_active: bool,
        is_focused: bool,
        active_window_id: Option<u64>,
    ) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id,
            idx,
            name: None,
            output: Some(output.to_string()),
            is_urgent: false,
            is_active,
            is_focused,
            active_window_id,
        }
    }

    #[test]
    fn workspaces_module_state_updated_on_workspace_change() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        let ws = make_workspace(0, true);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged {
                workspaces: vec![ws],
            },
        ))));
        let items: Vec<_> = app
            .modules
            .iter()
            .filter(|(_, m)| m.kind() == ModuleKind::WorkspaceItem)
            .collect();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn workspace_render_state_detects_equivalent_snapshots() {
        let workspaces = vec![make_workspace(0, true), make_workspace(1, false)];

        assert!(super::workspace_render_state_equal(
            &workspaces,
            &workspaces.clone()
        ));
    }

    #[test]
    fn equivalent_workspace_event_keeps_module_count() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        let workspaces = vec![make_workspace(0, true)];
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged {
                workspaces: workspaces.clone(),
            },
        ))));
        let count = app.modules.len();

        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged { workspaces },
        ))));

        assert_eq!(app.modules.len(), count);
    }

    #[test]
    fn workspace_module_focused_workspace_reflected_in_state() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        let ws0 = make_workspace(0, false);
        let ws1 = make_workspace(1, true);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged {
                workspaces: vec![ws0, ws1],
            },
        ))));
        let items: Vec<_> = app
            .modules
            .iter()
            .filter(|(_, m)| m.kind() == ModuleKind::WorkspaceItem)
            .collect();
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|(_, m)| m.id().0 == "workspace:2"));
    }

    #[test]
    fn workspaces_module_state_not_updated_without_workspaces_module_in_config() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        let ws = make_workspace(0, true);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged {
                workspaces: vec![ws],
            },
        ))));
        let id = ModuleId::new("workspaces");
        assert!(app.modules.state(&id).is_none());
    }

    #[test]
    fn workspaces_changed_order_preserved_in_state() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        let ws2 = make_workspace(2, false);
        let ws0 = make_workspace(0, true);
        let ws1 = make_workspace(1, false);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged {
                workspaces: vec![ws2, ws0, ws1],
            },
        ))));
        let items: Vec<_> = app
            .modules
            .iter()
            .filter(|(_, m)| m.kind() == ModuleKind::WorkspaceItem)
            .collect();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].1.id().0, "workspace:1");
        assert_eq!(items[1].1.id().0, "workspace:2");
        assert_eq!(items[2].1.id().0, "workspace:3");
    }

    #[test]
    fn workspace_activated_updates_focused_pill() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        let ws0 = make_workspace(0, true);
        let ws1 = make_workspace(1, false);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged {
                workspaces: vec![ws0, ws1],
            },
        ))));

        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspaceActivated {
                id: 2,
                focused: true,
            },
        ))));

        let focused: Vec<_> = app
            .niri
            .state()
            .workspaces
            .iter()
            .filter(|w| w.is_focused)
            .collect();
        assert_eq!(focused.len(), 1);
        assert_eq!(focused[0].idx, 1);
    }

    #[test]
    fn workspace_activated_does_not_rebuild_workspace_items() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        app.perf = Some(super::super::perf::PerfTracker::default());
        let ws0 = make_workspace(0, true);
        let ws1 = make_workspace(1, false);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged {
                workspaces: vec![ws0, ws1],
            },
        ))));
        let _ = app.perf.as_ref().unwrap().snapshot_and_reset();
        let count = app.modules.len();

        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspaceActivated {
                id: 2,
                focused: true,
            },
        ))));

        let snapshot = app.perf.as_ref().unwrap().snapshot_and_reset();
        assert_eq!(snapshot.workspace_rebuilds, 0);
        assert_eq!(app.modules.len(), count);
    }

    #[test]
    fn workspace_activated_no_panic_when_workspaces_empty() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspaceActivated {
                id: 1,
                focused: true,
            },
        ))));
        assert!(app.niri.state().workspaces.is_empty());
    }

    // --- Story 4.4 helpers and tests ---

    fn make_focused_window_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "focused-window".into(),
                module_type: "focused-window".into(),
                params: ModuleParams::None,
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.center = vec!["focused-window".into()];
        App::boot(config, vec![]).0
    }

    fn make_window(id: u64, title: Option<&str>, is_focused: bool) -> niri_ipc::Window {
        niri_ipc::Window {
            id,
            title: title.map(|t| t.to_string()),
            app_id: None,
            pid: None,
            workspace_id: None,
            is_focused,
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

    #[test]
    fn focused_window_state_updated_on_window_focus_changed() {
        use niri_ipc::Event;
        let mut app = make_focused_window_app();
        let win = make_window(1, Some("Firefox"), false);
        app.niri.state_mut().windows.insert(1, win);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowFocusChanged { id: Some(1) },
        ))));
        let id = ModuleId::new("focused-window");
        match app.modules.state(&id) {
            Some(ModuleState::FocusedWindow(s)) => {
                assert_eq!(s.title.as_deref(), Some("Firefox"));
            }
            other => panic!("expected FocusedWindow state, got {:?}", other),
        }
    }

    #[test]
    fn focused_window_title_retained_when_focus_moves_to_none() {
        use niri_ipc::Event;
        let mut app = make_focused_window_app();
        let win = make_window(1, Some("Firefox"), false);
        app.niri.state_mut().windows.insert(1, win);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowFocusChanged { id: Some(1) },
        ))));
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowFocusChanged { id: None },
        ))));
        let id = ModuleId::new("focused-window");
        match app.modules.state(&id) {
            Some(ModuleState::FocusedWindow(s)) => {
                assert_eq!(s.title.as_deref(), Some("Firefox"));
            }
            other => panic!("expected FocusedWindow state, got {:?}", other),
        }
    }

    #[test]
    fn focused_window_state_updated_on_windows_changed() {
        use niri_ipc::Event;
        let mut app = make_focused_window_app();
        assert!(app.niri.state().focused_window_id.is_none());
        let win = make_window(2, Some("Helix"), true);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowsChanged { windows: vec![win] },
        ))));
        let id = ModuleId::new("focused-window");
        match app.modules.state(&id) {
            Some(ModuleState::FocusedWindow(s)) => {
                assert_eq!(s.title.as_deref(), Some("Helix"));
            }
            other => panic!("expected FocusedWindow state, got {:?}", other),
        }
    }

    #[test]
    fn focused_window_state_updated_on_window_opened_or_changed() {
        use niri_ipc::Event;
        let mut app = make_focused_window_app();
        let win = make_window(3, Some("Terminal"), true);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowOpenedOrChanged { window: win },
        ))));
        let id = ModuleId::new("focused-window");
        match app.modules.state(&id) {
            Some(ModuleState::FocusedWindow(s)) => {
                assert_eq!(s.title.as_deref(), Some("Terminal"));
            }
            other => panic!("expected FocusedWindow state, got {:?}", other),
        }
    }

    #[test]
    fn focused_window_state_cleared_on_window_closed() {
        use niri_ipc::Event;
        let mut app = make_focused_window_app();
        let win = make_window(4, Some("Nautilus"), true);
        app.niri.state_mut().windows.insert(4, win);
        app.niri.state_mut().focused_window_id = Some(4);
        let _ = app.modules.handle(
            super::super::module_runtime::ModuleRuntimeIntent::ApplyModuleState {
                id: ModuleId::new("focused-window"),
                state: ModuleState::FocusedWindow(
                    crate::modules::focused_window::FocusedWindowState {
                        title: Some("Nautilus".into()),
                    },
                ),
            },
        );
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowClosed { id: 4 },
        ))));
        let id = ModuleId::new("focused-window");
        match app.modules.state(&id) {
            Some(ModuleState::FocusedWindow(s)) => {
                assert!(s.title.is_none());
            }
            other => panic!("expected FocusedWindow state, got {:?}", other),
        }
    }

    #[test]
    fn focused_window_state_not_updated_without_focused_window_module() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        let win = make_window(1, Some("Firefox"), true);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowsChanged { windows: vec![win] },
        ))));
        let id = ModuleId::new("focused-window");
        assert!(app.modules.state(&id).is_none());
    }

    #[test]
    fn workspace_active_window_change_updates_output_focused_window_title() {
        use crate::modules::focused_window::FocusedWindowState;
        use crate::surface::SurfaceState;
        use niri_ipc::Event;

        let mut app = make_focused_window_app();
        let focused_window = make_window(1, Some("Focused"), true);
        let output_window = make_window(2, Some("Output Active"), false);
        app.niri.state_mut().focused_window_id = Some(1);
        app.niri.state_mut().windows.insert(1, focused_window);
        app.niri.state_mut().windows.insert(2, output_window);
        app.niri.state_mut().workspaces = vec![
            make_workspace_on_output(10, 1, "eDP-1", true, true, Some(1)),
            make_workspace_on_output(20, 2, "DP-1", true, false, None),
        ];
        let _ = app.modules.handle(
            super::super::module_runtime::ModuleRuntimeIntent::ApplyModuleState {
                id: ModuleId::new("focused-window"),
                state: ModuleState::FocusedWindow(FocusedWindowState {
                    title: Some("Focused".into()),
                }),
            },
        );
        let window = iced::window::Id::unique();
        app.surfaces
            .surfaces
            .insert(window, SurfaceState::new("DP-1".into(), String::new()));

        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspaceActiveWindowChanged {
                workspace_id: 20,
                active_window_id: Some(2),
            },
        ))));

        let model = app.bar_render_model(window);
        match &model.center[0].state {
            ModuleState::FocusedWindow(state) => {
                assert_eq!(state.title.as_deref(), Some("Output Active"));
            }
            state => panic!("expected focused-window state, got {state:?}"),
        }
    }
}
