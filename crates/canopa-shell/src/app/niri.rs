use iced::Task;

use super::{App, Message};
use crate::modules::{ModuleKind, ModuleState};
use crate::niri::NiriEvent;

pub(super) struct AppNiriState {
    state: crate::niri::NiriState,
    /// The last place announced to screen readers.
    announced_place: Option<crate::niri::LayoutPlace>,
    /// Text typed into the overview that the launcher is searching.
    overview_query: String,
    /// The launcher opened for that text has appeared; it may still be
    /// loading its sources before it does.
    overview_launcher_shown: bool,
    #[cfg(not(test))]
    announcer: Option<super::announcer::Announcer>,
    #[cfg(test)]
    announcements: Vec<String>,
}

impl AppNiriState {
    pub(super) fn new() -> Self {
        Self {
            state: crate::niri::NiriState::default(),
            announced_place: None,
            overview_query: String::new(),
            overview_launcher_shown: false,
            #[cfg(not(test))]
            announcer: None,
            #[cfg(test)]
            announcements: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn overview_query(&self) -> &str {
        &self.overview_query
    }

    /// Tell screen readers where focus went when it moves to another column,
    /// window or workspace. Window titles are left to the application's own
    /// focus announcement.
    fn announce_place(&mut self) {
        let Some(place) = self.state.focused_place() else {
            return;
        };
        let announcement = place_announcement(self.announced_place, place);
        self.announced_place = Some(place);
        let Some(text) = announcement else {
            return;
        };
        #[cfg(not(test))]
        self.announcer
            .get_or_insert_with(|| {
                super::announcer::Announcer::new(fl!("shell-a11y-layout-announcer"))
            })
            .announce(&text);
        #[cfg(test)]
        self.announcements.push(text);
    }

    pub(super) fn state(&self) -> &crate::niri::NiriState {
        &self.state
    }

    pub(super) fn state_mut(&mut self) -> &mut crate::niri::NiriState {
        &mut self.state
    }
}

/// What changed between two places, worded for a screen reader, or `None`
/// when focus stayed put.
fn place_announcement(
    previous: Option<crate::niri::LayoutPlace>,
    place: crate::niri::LayoutPlace,
) -> Option<String> {
    if previous.is_some_and(|previous| {
        (previous.workspace_id, previous.column, previous.tile)
            == (place.workspace_id, place.column, place.tile)
    }) {
        return None;
    }
    let mut parts = vec![fl!(
        "shell-a11y-layout-column",
        column = place.column,
        columns = place.columns
    )];
    if place.tiles > 1 {
        parts.push(fl!(
            "shell-a11y-layout-tile",
            tile = place.tile,
            tiles = place.tiles
        ));
    }
    if previous.is_none_or(|previous| previous.workspace_id != place.workspace_id) {
        parts.push(fl!(
            "shell-a11y-layout-workspace",
            workspace = place.workspace
        ));
    }
    Some(parts.join(", "))
}

fn run_niri_action(action: niri_ipc::Action) {
    #[cfg(not(test))]
    tokio::spawn(async move {
        if let Err(error) = crate::command::niri_action(action).await {
            tracing::warn!(target: "canopa_shell::niri", %error, "niri action failed");
        }
    });
    #[cfg(test)]
    let _ = action;
}

fn workspace_outputs(workspaces: &[niri_ipc::Workspace]) -> std::collections::BTreeSet<&str> {
    workspaces
        .iter()
        .filter_map(|workspace| workspace.output.as_deref())
        .collect()
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
                    target: "canopa_shell::niri",
                    "niri ipc connected"
                );
                self.refresh_niri_output_sizes()
            }
            NiriEvent::Unavailable => {
                *self.niri.state_mut() = crate::niri::NiriState::default();
                self.reconcile_workspace_dynamic_modules(&[]);
                self.refresh_taskbar();
                let affected_module_count = self.modules.count_by_kind(ModuleKind::Workspaces)
                    + self.modules.count_by_kind(ModuleKind::FocusedWindow);
                tracing::info!(
                    target: "canopa_shell::niri",
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
                let mut task = Task::none();
                match event {
                    Event::WorkspacesChanged { workspaces } => {
                        let changed = !workspace_render_state_equal(
                            &self.niri.state().workspaces,
                            &workspaces,
                        );
                        tracing::debug!(
                            target: "canopa_shell::niri",
                            workspace_count = workspaces.len(),
                            changed,
                            "niri workspaces changed"
                        );
                        if changed {
                            // Outputs come and go with their workspaces.
                            let outputs_changed = workspace_outputs(&self.niri.state().workspaces)
                                != workspace_outputs(&workspaces);
                            self.rebuild_workspace_item_modules(&workspaces);
                            self.niri.state_mut().workspaces = workspaces;
                            self.push_focused_window_state();
                            if outputs_changed {
                                task = self.refresh_niri_output_sizes();
                            }
                        }
                    }
                    Event::WindowFocusChanged { id } => {
                        tracing::trace!(
                            target: "canopa_shell::niri",
                            focused = id.is_some(),
                            "niri focused window changed"
                        );
                        self.niri.state_mut().focused_window_id = id;
                        self.push_focused_window_state();
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
                        self.push_focused_window_state();
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
                        self.push_focused_window_state();
                    }
                    Event::WindowsChanged { windows } => {
                        tracing::trace!(
                            target: "canopa_shell::niri",
                            window_count = windows.len(),
                            focused_present = windows.iter().any(|w| w.is_focused),
                            "niri windows changed"
                        );
                        self.niri.state_mut().focused_window_id =
                            windows.iter().find(|w| w.is_focused).map(|w| w.id);
                        self.niri.state_mut().windows =
                            windows.into_iter().map(|w| (w.id, w)).collect();
                        self.niri.state_mut().windows_known = true;
                        self.push_focused_window_state();
                    }
                    Event::WindowOpenedOrChanged { window } => {
                        tracing::trace!(
                            target: "canopa_shell::niri",
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
                            target: "canopa_shell::niri",
                            had_focus,
                            "niri window closed"
                        );
                        self.niri.state_mut().windows.remove(&id);
                        if had_focus {
                            self.niri.state_mut().focused_window_id = None;
                        }
                        self.push_focused_window_state();
                    }
                    Event::WindowLayoutsChanged { changes } => {
                        let windows = &mut self.niri.state_mut().windows;
                        for (id, layout) in changes {
                            if let Some(window) = windows.get_mut(&id) {
                                window.layout = layout;
                            }
                        }
                    }
                    Event::WindowUrgencyChanged { id, urgent } => {
                        if let Some(window) = self.niri.state_mut().windows.get_mut(&id) {
                            window.is_urgent = urgent;
                        }
                    }
                    Event::OverviewTyped { text } => {
                        task = self.search_from_overview(&text);
                    }
                    Event::ConfigLoaded { .. } => {
                        // Output modes and scales live in the configuration.
                        task = self.refresh_niri_output_sizes();
                    }
                    _ => {}
                }
                self.refresh_taskbar();
                self.niri.announce_place();
                Task::batch([task, self.sync_window_session()])
            }
        }
    }

    /// Typing in the overview searches the launcher: the first key opens the
    /// launcher over the overview with the text, and keys Niri reports before
    /// the launcher takes the keyboard extend it. Niri gives the keyboard to
    /// the launcher's top-layer surface while the overview stays open, so no
    /// key reaches a window in between; the overview closes with the
    /// launcher (`finish_overview_search`).
    fn search_from_overview(&mut self, text: &str) -> Task<Message> {
        let Some(menu) = self
            .config
            .menus
            .iter()
            .find(|menu| {
                menu.source.as_ref().is_some_and(|source| {
                    matches!(source.kind, crate::config::MenuSourceKind::Launcher(_))
                })
            })
            .map(|menu| menu.name.clone())
        else {
            return Task::none();
        };
        // Keys typed before the launcher takes the keyboard still arrive from
        // the overview; they extend the query of the launcher already opening.
        let launcher_open = self.foreground.active_panel_tree().is_some_and(|tree| {
            tree.content == crate::surface::panel::PanelContent::Menu(menu.clone())
        });
        if launcher_open && !self.niri.overview_query.is_empty() {
            self.niri.overview_query.push_str(text);
            return Task::done(Message::Panel(super::messages::PanelEvent::SearchChanged(
                self.niri.overview_query.clone(),
            )));
        }
        let Some(origin_window) = self
            .surfaces
            .select_global_panel_surface(self.niri.state().focused_output())
        else {
            return Task::none();
        };
        self.niri.overview_query = text.to_owned();
        self.niri.overview_launcher_shown = false;
        let open = self.handle_panel_event(super::messages::PanelEvent::Open {
            origin_window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu(menu),
            anchor: None,
        });
        Task::batch([
            open,
            Task::done(Message::Panel(super::messages::PanelEvent::SearchChanged(
                self.niri.overview_query.clone(),
            ))),
        ])
    }

    /// Close the overview once the launcher opened from it has closed,
    /// whether a result was chosen or the search was dismissed.
    pub(super) fn finish_overview_search(&mut self) {
        if self.niri.overview_query.is_empty() {
            return;
        }
        let launcher_open = self.foreground.active_panel_tree().is_some_and(|tree| {
            matches!(&tree.content, crate::surface::panel::PanelContent::Menu(name)
            if self.config.menus.iter().any(|menu| &menu.name == name
                && menu.source.as_ref().is_some_and(|source| {
                    matches!(source.kind, crate::config::MenuSourceKind::Launcher(_))
                })))
        });
        if launcher_open {
            self.niri.overview_launcher_shown = true;
        } else if self.niri.overview_launcher_shown {
            self.niri.overview_query.clear();
            self.niri.overview_launcher_shown = false;
            run_niri_action(niri_ipc::Action::CloseOverview {});
        }
    }

    /// Ask Niri for output sizes, which set the minimap's scale.
    pub(super) fn refresh_niri_output_sizes(&self) -> Task<Message> {
        if cfg!(test) || self.modules.count_by_kind(ModuleKind::Minimap) == 0 {
            return Task::none();
        }
        Task::perform(crate::niri::output_sizes(), |sizes| {
            Message::Niri(super::messages::AppNiriEvent::OutputSizes(
                sizes.unwrap_or_default(),
            ))
        })
    }

    pub(super) fn handle_app_niri_event(
        &mut self,
        event: super::messages::AppNiriEvent,
    ) -> Task<Message> {
        match event {
            super::messages::AppNiriEvent::Ipc(event) => self.handle_niri_event(event),
            super::messages::AppNiriEvent::OutputSizes(sizes) => {
                self.niri.state_mut().output_sizes = sizes;
                self.sync_window_session()
            }
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
    fn unavailable_clears_workspace_children_and_cached_compositor_state() {
        let mut app = make_workspaces_app();
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            niri_ipc::Event::WorkspacesChanged {
                workspaces: vec![make_workspace(0, true)],
            },
        ))));
        assert!(
            app.modules
                .iter()
                .any(|(_, module)| module.kind() == ModuleKind::WorkspaceItem)
        );
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Unavailable)));
        assert!(
            !app.modules
                .iter()
                .any(|(_, module)| module.kind() == ModuleKind::WorkspaceItem)
        );
        assert!(app.niri.state().workspaces.is_empty());
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
        assert!(
            items
                .iter()
                .any(|(_, m)| m.id().display_name() == "workspace:2")
        );
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
        assert_eq!(items[0].1.id().display_name(), "workspace:1");
        assert_eq!(items[1].1.id().display_name(), "workspace:2");
        assert_eq!(items[2].1.id().display_name(), "workspace:3");
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
            parent_id: None,
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
                pinned: None,
            },
            focus_timestamp: None,
        }
    }

    fn focused_window_state_with_title(
        title: &str,
    ) -> crate::modules::focused_window::FocusedWindowState {
        crate::modules::focused_window::FocusedWindowState {
            window: Some(crate::modules::focused_window::FocusedWindowInfo {
                title: Some(title.into()),
                app_id: None,
                application_icon: None,
            }),
        }
    }

    fn focused_window_state_title(
        state: &crate::modules::focused_window::FocusedWindowState,
    ) -> Option<&str> {
        state.window.as_ref().and_then(|info| info.title.as_deref())
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
                assert_eq!(focused_window_state_title(s), Some("Firefox"));
            }
            other => panic!("expected FocusedWindow state, got {:?}", other),
        }
    }

    #[test]
    fn focused_window_state_cleared_when_focus_moves_to_none() {
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
                assert!(s.window.is_none());
            }
            other => panic!("expected FocusedWindow state, got {:?}", other),
        }
    }

    #[test]
    fn focused_window_state_uses_active_window_when_layer_shell_takes_focus() {
        use niri_ipc::Event;
        let mut app = make_focused_window_app();
        let workspace = make_workspace_on_output(1, 0, "eDP-1", true, true, Some(1));
        let focused_window = make_window(1, Some("Firefox"), true);

        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged {
                workspaces: vec![workspace],
            },
        ))));
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowsChanged {
                windows: vec![focused_window],
            },
        ))));
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowFocusChanged { id: None },
        ))));

        let id = ModuleId::new("focused-window");
        match app.modules.state(&id) {
            Some(ModuleState::FocusedWindow(s)) => {
                assert_eq!(focused_window_state_title(s), Some("Firefox"));
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
                assert_eq!(focused_window_state_title(s), Some("Helix"));
            }
            other => panic!("expected FocusedWindow state, got {:?}", other),
        }
    }

    #[test]
    fn focused_window_state_keeps_window_with_missing_title_and_app_id() {
        use niri_ipc::Event;
        let mut app = make_focused_window_app();
        let mut win = make_window(2, None, true);
        win.app_id = Some("org.example.App".into());

        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowsChanged { windows: vec![win] },
        ))));

        let id = ModuleId::new("focused-window");
        match app.modules.state(&id) {
            Some(ModuleState::FocusedWindow(s)) => {
                let info = s.window.as_ref().expect("window should be present");
                assert!(info.title.is_none());
                assert_eq!(info.app_id.as_deref(), Some("org.example.App"));
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
                assert_eq!(focused_window_state_title(s), Some("Terminal"));
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
                state: ModuleState::FocusedWindow(focused_window_state_with_title("Nautilus")),
            },
        );
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowClosed { id: 4 },
        ))));
        let id = ModuleId::new("focused-window");
        match app.modules.state(&id) {
            Some(ModuleState::FocusedWindow(s)) => {
                assert!(s.window.is_none());
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
                state: ModuleState::FocusedWindow(focused_window_state_with_title("Focused")),
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
                assert_eq!(focused_window_state_title(state), Some("Output Active"));
            }
            state => panic!("expected focused-window state, got {state:?}"),
        }
    }

    #[test]
    fn layout_and_urgency_changes_update_the_window_cache() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowsChanged {
                windows: vec![make_window(1, Some("Files"), false)],
            },
        ))));
        let mut layout = app.niri.state().windows[&1].layout.clone();
        layout.pos_in_scrolling_layout = Some((3, 1));
        layout.tile_size = (960.0, 1000.0);

        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowLayoutsChanged {
                changes: vec![(1, layout.clone()), (99, layout)],
            },
        ))));
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WindowUrgencyChanged {
                id: 1,
                urgent: true,
            },
        ))));

        let window = &app.niri.state().windows[&1];
        assert_eq!(window.layout.pos_in_scrolling_layout, Some((3, 1)));
        assert_eq!(window.layout.tile_size, (960.0, 1000.0));
        assert!(window.is_urgent);
        assert_eq!(app.niri.state().windows.len(), 1);
    }

    #[test]
    fn output_sizes_are_stored() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let _ = app.update(Message::Niri(AppNiriEvent::OutputSizes(
            [("eDP-1".to_owned(), (1920.0, 1080.0))].into(),
        )));
        assert_eq!(
            app.niri.state().output_sizes.get("eDP-1"),
            Some(&(1920.0, 1080.0))
        );
    }

    #[test]
    fn moving_focus_between_columns_is_announced_once_per_place() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        let tiled = |id, column, tile| {
            let mut window = make_window(id, Some("Window"), false);
            window.workspace_id = Some(2);
            window.layout.pos_in_scrolling_layout = Some((column, tile));
            window
        };
        let send = |app: &mut App, event| {
            let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(event))));
        };
        send(
            &mut app,
            Event::WorkspacesChanged {
                workspaces: vec![make_workspace(1, true)],
            },
        );
        send(
            &mut app,
            Event::WindowsChanged {
                windows: vec![tiled(1, 1, 1), tiled(2, 2, 1), tiled(3, 2, 2)],
            },
        );
        send(&mut app, Event::WindowFocusChanged { id: Some(1) });
        send(&mut app, Event::WindowFocusChanged { id: Some(3) });
        // The same place again, and the Shell taking the keyboard, say nothing.
        send(&mut app, Event::WindowFocusChanged { id: Some(3) });
        send(&mut app, Event::WindowFocusChanged { id: None });
        send(&mut app, Event::WindowFocusChanged { id: Some(2) });

        assert_eq!(
            app.niri.announcements,
            [
                "Column 1 of 2, workspace 1",
                "Column 2 of 2, window 2 of 2",
                "Column 2 of 2, window 1 of 2",
            ]
        );
    }
}
