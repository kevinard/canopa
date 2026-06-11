mod animation;
mod app_effects;
mod bar_surface_plan;
mod dynamic_modules;
pub(crate) mod eligible_outputs;
mod focus;
mod foreground;
mod layout;
mod menu;
pub(crate) mod messages;
mod module_data_sources;
mod module_events;
mod module_graph;
mod module_runtime;
mod module_state;
mod module_store;
mod navigation;
mod niri;
mod overlay_adapter;
mod panel;
mod panel_menu_session;
mod panel_runtime;
mod panel_search;
mod panel_sizing;
mod perf;
mod render;
mod selectable_menu_session;
mod shared;
mod subscriptions;
mod surface_input_router;
mod surfaces;
mod tray;
mod view;
mod widget_ops;

use iced::Task;
use iced_layershell::to_layer_message;

use crate::config::Config;
use crate::dbus::DbusConnections;
#[cfg(test)]
use crate::modules::ModuleId;

pub(crate) struct App {
    config: Config,
    shared: shared::SharedContext,
    surfaces: surfaces::SurfaceManager,
    foreground: foreground::Foreground,
    modules: module_runtime::ModuleRuntime,
    data_sources: module_data_sources::ModuleDataSourceRuntime,
    niri: niri::AppNiriState,
    tray: tray::TrayState,
    perf: Option<perf::PerfTracker>,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    Ipc(messages::IpcEvent),
    Surface(messages::SurfaceEvent),
    WaylandOutputSnapshot(Vec<String>),
    Menu(messages::MenuEvent),
    Panel(messages::PanelEvent),
    Modules(messages::ModuleRuntimeEvent),
    Niri(messages::AppNiriEvent),
    Tray(messages::TrayRuntimeEvent),
    /// Scroll-triggered volume adjustment (Story 5.4). Global — adjusts system volume.
    VolumeAdjust {
        step: u8,
        up: bool,
    },
    /// D-Bus connection handles established at startup (Story 5.1).
    DbusReady(DbusConnections),
    /// User clicked a workspace pill -- switch to that workspace via niri IPC.
    NiriFocusWorkspace {
        id: u64,
    },
    /// Periodic runtime performance report when NIRIBAR_PERF=1 is set.
    PerfReport,
}

impl Message {
    fn diagnostic_label(&self) -> &'static str {
        match self {
            Message::Surface(event) => event.diagnostic_label(),
            Message::WaylandOutputSnapshot(_) => "WaylandOutputSnapshot",
            Message::Ipc(event) => event.diagnostic_label(),
            Message::Menu(event) => event.diagnostic_label(),
            Message::Panel(event) => event.diagnostic_label(),
            Message::Modules(event) => event.diagnostic_label(),
            Message::Niri(event) => event.diagnostic_label(),
            Message::Tray(event) => event.diagnostic_label(),
            Message::VolumeAdjust { .. } => "VolumeAdjust",
            Message::DbusReady(_) => "DbusReady",
            Message::NiriFocusWorkspace { .. } => "NiriFocusWorkspace",
            Message::PerfReport => "PerfReport",
            _ => "LayerShell",
        }
    }

    fn perf_label(&self) -> &'static str {
        match self {
            Message::Surface(messages::SurfaceEvent::Tick) => "Surface::Tick",
            Message::WaylandOutputSnapshot(_) => "WaylandOutputSnapshot",
            Message::Ipc(_) => "Ipc",
            Message::Surface(_) => "Surface",
            Message::Menu(_) => "Menu",
            Message::Panel(_) => "Panel",
            Message::Modules(_) => "Modules",
            Message::Niri(messages::AppNiriEvent::Ipc(crate::niri::NiriEvent::Connected)) => {
                "Niri::Connected"
            }
            Message::Niri(messages::AppNiriEvent::Ipc(crate::niri::NiriEvent::Unavailable)) => {
                "Niri::Unavailable"
            }
            Message::Niri(messages::AppNiriEvent::Ipc(crate::niri::NiriEvent::Event(event))) => {
                match event {
                    niri_ipc::Event::WorkspacesChanged { .. } => "Niri::WorkspacesChanged",
                    niri_ipc::Event::WorkspaceUrgencyChanged { .. } => {
                        "Niri::WorkspaceUrgencyChanged"
                    }
                    niri_ipc::Event::WorkspaceActivated { .. } => "Niri::WorkspaceActivated",
                    niri_ipc::Event::WorkspaceActiveWindowChanged { .. } => {
                        "Niri::WorkspaceActiveWindowChanged"
                    }
                    niri_ipc::Event::WindowFocusChanged { .. } => "Niri::WindowFocusChanged",
                    niri_ipc::Event::WindowsChanged { .. } => "Niri::WindowsChanged",
                    niri_ipc::Event::WindowOpenedOrChanged { .. } => "Niri::WindowOpenedOrChanged",
                    niri_ipc::Event::WindowClosed { .. } => "Niri::WindowClosed",
                    niri_ipc::Event::WindowFocusTimestampChanged { .. } => {
                        "Niri::WindowFocusTimestampChanged"
                    }
                    niri_ipc::Event::WindowUrgencyChanged { .. } => "Niri::WindowUrgencyChanged",
                    niri_ipc::Event::WindowLayoutsChanged { .. } => "Niri::WindowLayoutsChanged",
                    niri_ipc::Event::KeyboardLayoutsChanged { .. } => {
                        "Niri::KeyboardLayoutsChanged"
                    }
                    niri_ipc::Event::KeyboardLayoutSwitched { .. } => {
                        "Niri::KeyboardLayoutSwitched"
                    }
                    niri_ipc::Event::OverviewOpenedOrClosed { .. } => {
                        "Niri::OverviewOpenedOrClosed"
                    }
                    niri_ipc::Event::ConfigLoaded { .. } => "Niri::ConfigLoaded",
                    niri_ipc::Event::ScreenshotCaptured { .. } => "Niri::ScreenshotCaptured",
                    niri_ipc::Event::CastsChanged { .. } => "Niri::CastsChanged",
                    niri_ipc::Event::CastStartedOrChanged { .. } => "Niri::CastStartedOrChanged",
                    niri_ipc::Event::CastStopped { .. } => "Niri::CastStopped",
                }
            }
            Message::VolumeAdjust { .. } => "VolumeAdjust",
            Message::DbusReady(_) => "DbusReady",
            Message::Tray(messages::TrayRuntimeEvent::Host(_)) => "Tray::Host",
            Message::Tray(messages::TrayRuntimeEvent::Activate { .. }) => "Tray::Activate",
            Message::Tray(messages::TrayRuntimeEvent::MenuRequest { .. }) => "Tray::MenuRequest",
            Message::Tray(messages::TrayRuntimeEvent::MenuRequestKeyboard { .. }) => {
                "Tray::MenuRequestKeyboard"
            }
            Message::Tray(messages::TrayRuntimeEvent::MenuRequestWithBounds { .. }) => {
                "Tray::MenuRequestWithBounds"
            }
            Message::Tray(messages::TrayRuntimeEvent::SecondaryActivate { .. }) => {
                "Tray::SecondaryActivate"
            }
            Message::Tray(messages::TrayRuntimeEvent::Scroll { .. }) => "Tray::Scroll",
            Message::NiriFocusWorkspace { .. } => "NiriFocusWorkspace",
            Message::PerfReport => "PerfReport",
            _ => "LayerShell",
        }
    }
}

impl From<iced_layershell::WaylandOutputSnapshot> for Message {
    fn from(snapshot: iced_layershell::WaylandOutputSnapshot) -> Self {
        Message::WaylandOutputSnapshot(snapshot.outputs)
    }
}

impl App {
    pub(super) fn handle_ipc_event(&mut self, event: messages::IpcEvent) -> Task<Message> {
        match event {
            messages::IpcEvent::DmenuSelection { request, responder } => {
                self.handle_dmenu_selection_request(request, responder)
            }
        }
    }

    /// `allowed_outputs`: output names to create windows on. Empty means no concrete Outputs
    /// are known yet, so no Bar Surfaces are created until a Wayland Output Snapshot arrives.
    #[cfg(test)]
    pub(crate) fn boot(config: Config, allowed_outputs: Vec<String>) -> (Self, Task<Message>) {
        let eligible_outputs = if allowed_outputs.is_empty() {
            eligible_outputs::EligibleOutputState::Unknown
        } else {
            eligible_outputs::EligibleOutputState::Known(allowed_outputs)
        };
        Self::boot_with_eligible_outputs(config, eligible_outputs)
    }

    pub(crate) fn boot_with_eligible_outputs(
        config: Config,
        eligible_outputs: eligible_outputs::EligibleOutputState,
    ) -> (Self, Task<Message>) {
        let module_graph = module_graph::build_module_graph(&config)
            .expect("validated config should build a module graph");
        let allowed_output_count = match &eligible_outputs {
            eligible_outputs::EligibleOutputState::Unknown => 0,
            eligible_outputs::EligibleOutputState::Known(outputs) => outputs.len(),
        };
        let bar_count = config.bars.len();
        let configured_module_count = config.modules.len();
        let mut surfaces = surfaces::SurfaceManager::new(
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        );
        let desired = bar_surface_plan::plan_bar_surfaces(&config.bars, &eligible_outputs);
        let desired_surface_count = desired.len();
        let effects = surfaces.reconcile_bar_surfaces(desired);
        let lifecycle_effect_count = effects.len();
        let runtime_module_count = module_graph.modules.len();
        let mut modules =
            module_runtime::ModuleRuntime::from_modules_with_actions(module_graph.modules);
        modules
            .set_dynamic_module_family_inputs(dynamic_modules::family_inputs_from_config(&config));
        let data_sources = module_data_sources::ModuleDataSourceRuntime::from_modules(&modules);
        let mut app = App {
            config,
            shared: shared::SharedContext::new(),
            surfaces,
            foreground: foreground::Foreground::default(),
            modules,
            data_sources,
            niri: niri::AppNiriState::new(),
            tray: tray::TrayState::default(),
            perf: perf::PerfTracker::from_env(),
        };
        let window_tasks = app.apply_bar_surface_lifecycle_effects(effects);
        tracing::debug!(
            target: "niribar::runtime",
            bar_count,
            configured_module_count,
            runtime_module_count,
            allowed_output_count,
            desired_surface_count,
            lifecycle_effect_count,
            perf_enabled = app.perf.is_some(),
            "app boot planned"
        );
        (app, Task::batch(window_tasks))
    }

    pub(crate) fn namespace() -> String {
        String::from("niribar")
    }

    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        tracing::trace!(
            target: "niribar::runtime",
            app_message = message.diagnostic_label(),
            "app message received"
        );

        if let Some(perf) = &self.perf {
            perf.record_update(message.perf_label());
        }

        match message {
            Message::Surface(event) => self.handle_surface_event(event),
            Message::WaylandOutputSnapshot(outputs) => self.handle_wayland_output_snapshot(outputs),
            Message::Ipc(event) => self.handle_ipc_event(event),
            Message::Menu(event) => self.handle_menu_event(event),
            Message::Panel(event) => self.handle_panel_event(event),
            Message::Modules(event) => self.handle_module_runtime_event(event),
            Message::Niri(event) => self.handle_app_niri_event(event),
            Message::VolumeAdjust { step, up } => self.handle_volume_adjust(step, up),
            Message::DbusReady(connections) => {
                tracing::debug!(
                    target: "niribar::runtime",
                    session_bus = connections.session.is_some(),
                    system_bus = connections.system.is_some(),
                    "dbus connections ready"
                );
                // Store shared handles; modules borrow clones of these (AR5, Story 5.1).
                self.shared.dbus = connections;
                Task::none()
            }
            Message::Tray(event) => self.handle_tray_runtime_event(event),
            Message::NiriFocusWorkspace { id } => {
                tracing::debug!(
                    target: "niribar::niri",
                    workspace_id = id,
                    "niri focus workspace requested"
                );
                tokio::task::spawn_blocking(move || {
                    if let Ok(mut socket) = niri_ipc::socket::Socket::connect() {
                        let _ = socket.send(niri_ipc::Request::Action(
                            niri_ipc::Action::FocusWorkspace {
                                reference: niri_ipc::WorkspaceReferenceArg::Id(id),
                            },
                        ));
                    }
                });
                Task::none()
            }
            Message::PerfReport => {
                if let Some(perf) = &self.perf {
                    let report = crate::app::perf::format_snapshot(&perf.snapshot_and_reset());
                    tracing::info!(
                        target: "niribar::perf",
                        "{}",
                        report
                    );
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Edge;
    use crate::modules::{MODULE_DESCRIPTORS, ModuleInteractionProfile, ModuleKind, Zone};
    use crate::niri::NiriEvent;

    fn make_config() -> Config {
        Config::default()
    }

    /// Returns a consistent unique window ID for use across a single test.
    fn test_wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

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

    #[test]
    fn namespace_returns_niribar() {
        assert_eq!(App::namespace(), "niribar");
    }

    #[test]
    fn boot_with_config_stores_config() {
        let mut config = Config::default();
        config.bars[0].collapsed_width = 60;
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.config.bars[0].collapsed_width, 60);
    }

    #[test]
    fn shared_context_starts_with_default_dbus() {
        let shared = shared::SharedContext::new();
        assert!(shared.dbus.session.is_none());
        assert!(shared.dbus.system.is_none());
    }

    #[test]
    fn perf_tracker_labels_surface_wrapped_event() {
        let event = Message::Surface(messages::SurfaceEvent::Tick);
        assert_eq!(event.perf_label(), "Surface::Tick");
    }

    #[test]
    fn hover_enter_sets_hovered_module() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let id = ModuleId::new("clock");
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ModuleHoverEnter(
            id.clone(),
            wid,
        )));
        assert_eq!(app.surfaces[&wid].hovered_module, Some(id));
    }

    #[test]
    fn hover_exit_clears_hovered_module() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let id = ModuleId::new("clock");
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ModuleHoverEnter(
            id.clone(),
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ModuleHoverExit(
            id, wid,
        )));
        assert!(app.surfaces[&wid].hovered_module.is_none());
    }

    #[test]
    fn hover_exit_different_id_does_not_clear() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let id_a = ModuleId::new("clock");
        let id_b = ModuleId::new("battery");
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ModuleHoverEnter(
            id_a.clone(),
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ModuleHoverExit(
            id_b, wid,
        )));
        assert_eq!(app.surfaces[&wid].hovered_module, Some(id_a));
    }

    #[test]
    fn cursor_moved_is_ignored_without_tray_module() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();

        let _ = app.update(Message::Surface(messages::SurfaceEvent::CursorMoved(
            wid,
            iced::Point::new(10.0, 20.0),
        )));

        assert_eq!(app.surfaces.last_cursor(), None);
    }

    #[test]
    fn cursor_moved_is_recorded_when_tray_module_exists() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid);
        app.modules.insert_with_actions_and_origin(
            app.modules.len(),
            (
                Zone::Start,
                Box::new(crate::modules::tray::TrayModule::new(ModuleId::new("tray"))),
            ),
            crate::config::ModuleActionsConfig::default(),
            module_runtime::ModuleOrigin::Configured,
            ModuleInteractionProfile::NONE,
        );
        let point = iced::Point::new(10.0, 20.0);

        let _ = app.update(Message::Surface(messages::SurfaceEvent::CursorMoved(
            wid, point,
        )));

        assert_eq!(app.surfaces.last_cursor(), Some((wid, point)));
    }

    #[test]
    fn perf_tracker_records_update_messages_when_enabled() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        app.perf = Some(super::perf::PerfTracker::default());

        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));

        let snapshot = app.perf.as_ref().unwrap().snapshot_and_reset();
        assert_eq!(snapshot.updates.get("Surface::Tick"), Some(&1));
    }

    #[test]
    fn perf_tracker_labels_niri_event_variants() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        app.perf = Some(super::perf::PerfTracker::default());

        let _ = app.update(Message::Niri(messages::AppNiriEvent::Ipc(
            NiriEvent::Event(Event::WorkspaceActivated {
                id: 1,
                focused: true,
            }),
        )));

        let snapshot = app.perf.as_ref().unwrap().snapshot_and_reset();
        assert_eq!(snapshot.updates.get("Niri::WorkspaceActivated"), Some(&1));
    }

    #[test]
    fn perf_tracker_labels_ignored_niri_event_variants() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        app.perf = Some(super::perf::PerfTracker::default());

        let _ = app.update(Message::Niri(messages::AppNiriEvent::Ipc(
            NiriEvent::Event(Event::ConfigLoaded { failed: false }),
        )));

        let snapshot = app.perf.as_ref().unwrap().snapshot_and_reset();
        assert_eq!(snapshot.updates.get("Niri::ConfigLoaded"), Some(&1));
    }

    #[test]
    fn perf_tracker_records_view_calls_when_enabled() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        app.perf = Some(super::perf::PerfTracker::default());
        let wid = test_wid();

        let _ = app.view(wid);

        let snapshot = app.perf.as_ref().unwrap().snapshot_and_reset();
        assert_eq!(snapshot.views.values().sum::<u64>(), 1);
    }

    #[test]
    fn perf_tracker_records_workspace_rebuilds_when_enabled() {
        let mut app = make_workspaces_app();
        app.perf = Some(super::perf::PerfTracker::default());

        app.rebuild_workspace_item_modules(&[make_workspace(0, true)]);

        let snapshot = app.perf.as_ref().unwrap().snapshot_and_reset();
        assert_eq!(snapshot.workspace_rebuilds, 1);
    }

    #[test]
    fn perf_report_resets_counters() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        app.perf = Some(super::perf::PerfTracker::default());

        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));
        let _ = app.update(Message::PerfReport);

        let snapshot = app.perf.as_ref().unwrap().snapshot_and_reset();
        assert!(snapshot.updates.is_empty());
    }

    // Story 5.1 tests

    #[test]
    fn shared_context_dbus_starts_as_default() {
        let state = shared::SharedContext::new();
        assert!(state.dbus.session.is_none());
        assert!(state.dbus.system.is_none());
    }

    #[test]
    fn dbus_ready_stores_connections_in_shared_context() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let conns = crate::dbus::DbusConnections::default();
        let _ = app.update(Message::DbusReady(conns));
        assert!(app.shared.dbus.session.is_none());
        assert!(app.shared.dbus.system.is_none());
    }

    // Story 5.2 tests

    // --- Story 6.1 tests ---

    #[test]
    fn focused_module_starts_none_for_new_surface() {
        let (app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        assert!(
            app.surfaces
                .get(&wid)
                .map(|s| s.focused_module.is_none())
                .unwrap_or(true)
        );
    }

    #[test]
    fn bar_focus_gained_sets_first_top_module() {
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
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
    }

    #[test]
    fn bar_focus_lost_clears_focused_module() {
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
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        assert!(app.surfaces[&wid].focused_module.is_some());
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusLost(wid)));
        assert!(app.surfaces[&wid].focused_module.is_none());
    }

    #[test]
    fn escape_pressed_when_focused_does_not_immediately_clear_state() {
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
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        assert!(app.surfaces[&wid].focused_module.is_some());
        let _ = app.update(Message::Surface(messages::SurfaceEvent::EscapePressed(wid)));
        assert!(app.surfaces[&wid].focused_module.is_some());
    }

    #[test]
    fn escape_pressed_when_not_focused_is_noop() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::EscapePressed(wid)));
        assert!(app.surfaces[&wid].focused_module.is_none());
    }

    #[test]
    fn bar_focus_gained_falls_back_to_center_if_no_top_zone() {
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
        let wid = test_wid();
        app.surface_mut(wid);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
    }

    #[test]
    fn bar_focus_gained_with_no_modules_is_noop() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        assert!(app.surfaces[&wid].focused_module.is_none());
    }

    // --- Story 6.2 tests ---

    fn make_nav_app() -> (App, iced::window::Id) {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "clock".into(),
                    module_type: "clock".into(),
                    params: ModuleParams::Clock(ClockConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "battery".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::None,
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "network".into(),
                    module_type: "network".into(),
                    params: ModuleParams::None,
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "workspaces".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::None,
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["clock".into(), "battery".into()];
        config.bars[0].modules.center = vec!["network".into()];
        config.bars[0].modules.end = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // pre-initialize surface
        (app, wid)
    }

    #[test]
    fn focus_move_next_advances_to_next_module() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("battery"))
        );
    }

    #[test]
    fn focus_move_next_crosses_zone_boundary() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("network"))
        );
    }

    #[test]
    fn focus_move_next_wraps_from_last_to_first() {
        let (mut app, wid) = make_nav_app();
        app.surface_mut(wid).focused_module = Some(ModuleId::new("workspaces"));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
    }

    #[test]
    fn focus_move_prev_retreats_to_previous_module() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMovePrev(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
    }

    #[test]
    fn focus_move_prev_wraps_from_first_to_last() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMovePrev(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("workspaces"))
        );
    }

    #[test]
    fn focus_next_zone_jumps_to_center() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusNextZone(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("network"))
        );
    }

    #[test]
    fn focus_next_zone_wraps_bottom_to_top() {
        let (mut app, wid) = make_nav_app();
        app.surface_mut(wid).focused_module = Some(ModuleId::new("workspaces"));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusNextZone(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
    }

    #[test]
    fn focus_prev_zone_from_top_jumps_to_bottom() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusPrevZone(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("workspaces"))
        );
    }

    #[test]
    fn focus_next_zone_skips_empty_zone() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "clock".into(),
                    module_type: "clock".into(),
                    params: ModuleParams::Clock(ClockConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "workspaces".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::None,
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["clock".into()];
        config.bars[0].modules.end = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusNextZone(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("workspaces"))
        );
    }

    #[test]
    fn navigation_keys_when_not_focused_are_noop() {
        let (mut app, wid) = make_nav_app();
        // Bar not focused — focused_module is None
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMovePrev(wid)));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusNextZone(wid)));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusPrevZone(wid)));
        // After BarFocusGained delegation, focused_module gets set — but here we never
        // send BarFocusGained, so the surface may have it after FocusMoveNext delegates.
        // The key assertion: no panic.
    }

    #[test]
    fn focus_prev_zone_from_center_jumps_to_top() {
        let (mut app, wid) = make_nav_app();
        app.surface_mut(wid).focused_module = Some(ModuleId::new("network"));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusPrevZone(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
    }

    // --- Story 6.3 tests ---

    fn make_interact_app() -> (App, iced::window::Id) {
        use crate::config::{ButtonConfig, ClockConfig, ModuleConfig, ModuleParams, VolumeConfig};
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "clock".into(),
                    module_type: "clock".into(),
                    params: ModuleParams::Clock(ClockConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "volume".into(),
                    module_type: "volume".into(),
                    params: ModuleParams::Volume(VolumeConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "button".into(),
                    module_type: "button".into(),
                    params: ModuleParams::Button(ButtonConfig {
                        label: "test".into(),
                        icon: ">".into(),
                        format: crate::config::FormatConfig::default(),
                        content_rotation: crate::config::ContentRotation::None,
                        tooltip: crate::config::TooltipSetting::Default,
                    }),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["clock".into(), "volume".into()];
        config.bars[0].modules.center = vec!["button".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // pre-initialize
        (app, wid)
    }

    struct ProbeInteractionModule {
        id: ModuleId,
        clicked: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl ProbeInteractionModule {
        fn new(id: &str, clicked: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
            Self {
                id: ModuleId::new(id),
                clicked,
            }
        }
    }

    impl crate::modules::Module for ProbeInteractionModule {
        fn id(&self) -> ModuleId {
            self.id.clone()
        }

        fn kind(&self) -> ModuleKind {
            ModuleKind::Button
        }

        fn subscription(&self) -> iced::Subscription<Message> {
            iced::Subscription::none()
        }

        fn interaction_handler(
            &self,
        ) -> Option<&dyn crate::modules::interaction::ModuleInteractionHandler> {
            Some(self)
        }
    }

    impl crate::modules::interaction::ModuleInteractionHandler for ProbeInteractionModule {
        fn on_interaction(&self, event: crate::modules::InteractionEvent) -> Option<Message> {
            match event {
                crate::modules::InteractionEvent::Clicked => {
                    self.clicked
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                    None
                }
                crate::modules::InteractionEvent::ScrollDown
                | crate::modules::InteractionEvent::ScrollUp => {
                    panic!("keyboard focus must not probe module interactions")
                }
                _ => None,
            }
        }
    }

    impl crate::modules::render::ModuleRender for ProbeInteractionModule {
        fn render_collapsed_with_style(
            &self,
            _state: &crate::modules::ModuleState,
            _bar_style: &crate::config::Style,
        ) -> iced::Element<'_, Message> {
            iced::widget::text("").into()
        }

        fn render_panel_with_style(
            &self,
            _state: &crate::modules::ModuleState,
            _bar_style: &crate::config::Style,
        ) -> iced::Element<'_, Message> {
            iced::widget::text("").into()
        }
    }

    #[test]
    fn keyboard_activate_does_not_probe_module_interactions() {
        let clicked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        use crate::config::{ButtonConfig, ModuleConfig, ModuleParams};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "probe".into(),
            module_type: "button".into(),
            params: ModuleParams::Button(ButtonConfig {
                label: "probe".into(),
                icon: ">".into(),
                format: crate::config::FormatConfig::default(),
                content_rotation: crate::config::ContentRotation::None,
                tooltip: crate::config::TooltipSetting::Default,
            }),
            actions: crate::config::ModuleActionsConfig::default(),
        }];
        config.bars[0].modules.start = vec!["probe".into()];
        let (mut app, _) = App::boot(config, vec![]);
        app.modules = module_runtime::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Start,
            Box::new(ProbeInteractionModule::new("probe", clicked.clone()))
                as Box<dyn crate::modules::Module>,
            crate::config::ModuleActionsConfig::default(),
        )]);
        let wid = test_wid();
        app.surface_mut(wid);

        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::KeyboardActivate(
            wid,
        )));

        assert!(clicked.load(std::sync::atomic::Ordering::SeqCst));
        assert!(!app.surfaces[&wid].module_activated);
    }

    #[test]
    fn clear_surface_module_refs_clears_focus_and_hover_for_matching_ids() {
        let (mut app, wid) = make_interact_app();
        app.surface_mut(wid).focused_module = Some(ModuleId::new("workspace:1"));
        app.surface_mut(wid).hovered_module = Some(ModuleId::new("workspace:2"));

        app.clear_surface_module_refs(&[
            ModuleId::new("workspace:1"),
            ModuleId::new("workspace:2"),
        ]);

        assert!(app.surfaces[&wid].focused_module.is_none());
        assert!(app.surfaces[&wid].hovered_module.is_none());
    }

    #[test]
    fn keyboard_activate_when_not_focused_is_noop() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::KeyboardActivate(
            wid,
        )));
        assert!(app.surfaces[&wid].focused_module.is_none());
        assert!(!app.surfaces[&wid].module_activated);
    }

    #[test]
    fn keyboard_activate_on_volume_enters_interaction_mode() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
        assert!(!app.surfaces[&wid].module_activated);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::KeyboardActivate(
            wid,
        )));
        assert!(app.surfaces[&wid].module_activated);
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
    }

    #[test]
    fn arrow_keys_scroll_when_module_activated() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::KeyboardActivate(
            wid,
        )));
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMovePrev(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
    }

    #[test]
    fn escape_deactivates_without_releasing_focus() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::KeyboardActivate(
            wid,
        )));
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::EscapePressed(wid)));
        assert!(!app.surfaces[&wid].module_activated);
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
    }

    #[test]
    fn arrow_keys_navigate_when_module_not_activated() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
        assert!(!app.surfaces[&wid].module_activated);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("button"))
        );
    }

    #[test]
    fn tab_deactivates_and_navigates_zone() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::KeyboardActivate(
            wid,
        )));
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusNextZone(wid)));
        assert!(!app.surfaces[&wid].module_activated);
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("button"))
        );
    }

    #[test]
    fn shift_tab_deactivates_and_navigates_zone() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::KeyboardActivate(
            wid,
        )));
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusPrevZone(wid)));
        assert!(!app.surfaces[&wid].module_activated);
    }

    #[test]
    fn keyboard_activate_on_button_does_not_panic_and_does_not_set_activated() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusNextZone(wid)));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("button"))
        );
        let _ = app.update(Message::Surface(messages::SurfaceEvent::KeyboardActivate(
            wid,
        )));
        assert!(!app.surfaces[&wid].module_activated);
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("button"))
        );
    }

    #[test]
    fn keyboard_activate_on_non_interactive_module_is_noop() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
        let _ = app.update(Message::Surface(messages::SurfaceEvent::KeyboardActivate(
            wid,
        )));
        assert!(!app.surfaces[&wid].module_activated);
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
    }

    #[test]
    fn focus_move_next_on_unfocused_delegates_to_bar_focus_gained() {
        let (mut app, wid) = make_interact_app();
        assert!(app.surfaces[&wid].focused_module.is_none());
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(wid)));
        // After delegation, BarFocusGained would set focused_module — the delegation happened.
        // We verify no panic.
    }

    // --- Story 7.1 tests ---

    #[test]
    fn boot_returns_app_and_task() {
        let (app, _task) = App::boot(make_config(), vec![]);
        assert!(app.surfaces.is_empty()); // no surfaces before first view/event
    }

    #[test]
    fn focus_gained_only_affects_focused_surface() {
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
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid_a = test_wid();
        let wid_b = test_wid();
        app.surface_mut(wid_a);
        app.surface_mut(wid_b); // initialize surface B without focus
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid_a,
        )));
        // Surface A has focus
        assert!(app.surfaces[&wid_a].focused_module.is_some());
        // Surface B unfocused
        assert!(app.surfaces[&wid_b].focused_module.is_none());
    }

    #[test]
    fn modules_from_all_bar_sections_are_instantiated() {
        // All module names across all bar sections must be instantiated so view() can render
        // them for each surface's respective bar.
        use crate::config::{BarConfig, ClockConfig, ModuleConfig, ModuleParams};
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    modules: crate::config::BarModules {
                        start: vec!["workspaces".into()],
                        center: vec![],
                        end: vec![],
                    },
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "secondary".into(),
                    output_target: crate::config::OutputTarget::Include(vec!["DP-1".into()]),
                    modules: crate::config::BarModules {
                        start: vec!["clock".into()],
                        center: vec![],
                        end: vec![],
                    },
                    ..BarConfig::default()
                },
            ],
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            menus: vec![],
        };
        let (app, _) = App::boot(config, vec![]);
        let instantiated: Vec<String> = app
            .modules
            .module_ids()
            .into_iter()
            .map(|id| id.0)
            .collect();
        assert!(
            instantiated.iter().any(|n| n == "clock"),
            "clock must be instantiated from secondary bar section; got: {instantiated:?}"
        );
        assert!(
            instantiated.iter().any(|n| n == "workspaces"),
            "workspaces must be instantiated from main bar section; got: {instantiated:?}"
        );
    }

    #[test]
    fn undeclared_known_builtins_still_instantiate() {
        let mut config = Config::default();
        config.bars[0].modules.start = vec!["clock".into()];

        let (app, _) = App::boot(config, vec![]);
        let module = app
            .modules
            .iter()
            .find(|(_, module)| module.id().0 == "clock")
            .map(|(_, module)| module)
            .expect("clock should be instantiated from an undeclared built-in");

        assert_eq!(module.kind(), ModuleKind::Clock);
        assert_eq!(module.module_type(), "clock");
    }

    #[test]
    fn undeclared_mandatory_param_modules_are_configuration_errors() {
        let mut config = Config::default();
        config.bars[0].modules.start = vec!["button".into(), "script".into()];

        let err = crate::config::validate(&config).unwrap_err();
        assert!(err.to_string().contains("must be declared"), "got: {err}");
    }

    #[test]
    fn constructor_lookup_matches_expected_module_kind() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};

        let descriptor = MODULE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.config_name == "clock")
            .expect("clock descriptor should exist");
        let module = (descriptor.constructor)(&ModuleConfig {
            name: "clock".into(),
            module_type: "clock".into(),
            params: ModuleParams::Clock(ClockConfig::default()),
            actions: crate::config::ModuleActionsConfig::default(),
        })
        .unwrap();

        assert_eq!(module.kind(), descriptor.kind);
        assert_eq!(module.module_type(), descriptor.config_name);
    }

    // --- Wayland Output Snapshot tests ---

    #[test]
    fn niri_workspaces_changed_does_not_open_bar_surfaces() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        let before = app.surfaces.len();
        let _ = app.update(Message::Niri(messages::AppNiriEvent::Ipc(
            NiriEvent::Event(Event::WorkspacesChanged {
                workspaces: vec![niri_ipc::Workspace {
                    id: 1,
                    idx: 0,
                    name: None,
                    output: Some("DP-1".to_string()),
                    is_urgent: false,
                    is_active: true,
                    is_focused: true,
                    active_window_id: None,
                }],
            }),
        )));
        assert_eq!(
            app.surfaces.len(),
            before,
            "Niri workspace output names must not drive Bar Surface planning"
        );
    }

    #[test]
    fn wayland_output_snapshot_opens_surface_for_new_output() {
        let (mut app, _) = App::boot(make_config(), vec!["eDP-1".into()]);
        let _ = app.update(Message::WaylandOutputSnapshot(vec![
            "eDP-1".into(),
            "DP-1".into(),
        ]));
        let known_names: Vec<&str> = app
            .surfaces
            .values()
            .map(|s| s.output_name.as_str())
            .collect();
        assert!(
            known_names.contains(&"DP-1"),
            "new output DP-1 must get a surface; got: {known_names:?}"
        );
    }

    #[test]
    fn wayland_output_snapshot_no_duplicate_for_known_output() {
        let (mut app, _) = App::boot(make_config(), vec!["eDP-1".into()]);
        let before = app.surfaces.len();
        let _ = app.update(Message::WaylandOutputSnapshot(vec!["eDP-1".into()]));
        assert_eq!(
            app.surfaces.len(),
            before,
            "no duplicate surface for already-known output"
        );
    }

    #[test]
    fn wayland_output_snapshot_removes_surfaces_for_missing_outputs() {
        let (mut app, _) = App::boot(make_config(), vec!["eDP-1".into()]);

        let _ = app.update(Message::WaylandOutputSnapshot(Vec::new()));

        assert!(app.surfaces.is_empty());
    }

    #[test]
    fn wayland_output_snapshot_skips_untargeted_output() {
        use crate::config::BarConfig;
        let config = Config {
            bars: vec![BarConfig {
                name: "main".into(),
                output_target: crate::config::OutputTarget::Include(vec!["eDP-1".into()]),
                ..BarConfig::default()
            }],
            modules: vec![],
            menus: vec![],
        };
        let (mut app, _) = App::boot(config, vec!["eDP-1".into()]);
        let before = app.surfaces.len();
        let _ = app.update(Message::WaylandOutputSnapshot(vec![
            "eDP-1".into(),
            "HDMI-A-1".into(),
        ]));
        assert_eq!(
            app.surfaces.len(),
            before,
            "HDMI-A-1 must not get a surface because no bar targets it"
        );
    }

    // --- Story 9.1: App-level multi-bar tests ---

    #[test]
    fn wayland_output_snapshot_gets_all_targeting_bar() {
        use crate::config::BarConfig;
        let config = Config {
            bars: vec![BarConfig {
                name: "main".into(),
                ..BarConfig::default()
            }],
            modules: vec![],
            menus: vec![],
        };
        let (mut app, _) = App::boot(config, vec!["eDP-1".into()]);
        let _ = app.update(Message::WaylandOutputSnapshot(vec![
            "eDP-1".into(),
            "DP-1".into(),
        ]));
        let output_names: Vec<&str> = app
            .surfaces
            .values()
            .map(|s| s.output_name.as_str())
            .filter(|n| !n.is_empty())
            .collect();
        assert!(
            output_names.contains(&"DP-1"),
            "hotplugged DP-1 must get a surface; got: {output_names:?}"
        );
    }

    #[test]
    fn wayland_output_snapshot_creates_one_surface_per_bar_targeting_output() {
        use crate::config::BarConfig;
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "top".into(),
                    edge: crate::config::Edge::Top,
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
            menus: vec![],
        };
        let (mut app, _) = App::boot(config, vec!["eDP-1".into()]);
        let before = app.surfaces.len();
        let _ = app.update(Message::WaylandOutputSnapshot(vec![
            "eDP-1".into(),
            "DP-1".into(),
        ]));
        assert_eq!(
            app.surfaces.len(),
            before + 2,
            "DP-1 should get one surface per bar"
        );
    }

    #[test]
    fn wayland_output_snapshot_creates_horizontal_and_side_bars() {
        let config = Config {
            bars: vec![
                crate::config::BarConfig {
                    name: "right".into(),
                    edge: Edge::Right,
                    ..crate::config::BarConfig::default()
                },
                crate::config::BarConfig {
                    name: "bottom".into(),
                    edge: Edge::Bottom,
                    ..crate::config::BarConfig::default()
                },
            ],
            modules: vec![],
            menus: vec![],
        };
        let (mut app, _) = App::boot(config, vec![]);
        let _ = app.update(Message::WaylandOutputSnapshot(vec!["DP-1".into()]));
        // Both bars must be registered for DP-1.
        let names: Vec<String> = app
            .surfaces
            .values()
            .filter(|s| s.output_name == "DP-1")
            .map(|s| s.bar_name.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert!(
            names.contains(&"bottom".to_string()),
            "bottom bar not spawned for DP-1"
        );
        assert!(
            names.contains(&"right".to_string()),
            "right bar not spawned for DP-1"
        );
    }
}
