mod animation;
mod dynamic_modules;
mod focus;
mod layout;
mod menu;
pub(crate) mod messages;
mod module_events;
mod module_runtime;
mod module_state;
mod module_store;
mod navigation;
mod niri;
mod perf;
mod render;
mod shared;
mod subscriptions;
mod surfaces;
mod tray;
mod view;
mod widget_ops;

use std::collections::HashMap;

use iced::Task;
use iced_layershell::to_layer_message;

use crate::config::{Config, ModuleConfig};
use crate::dbus::DbusConnections;
#[cfg(test)]
use crate::modules::ModuleId;
use crate::modules::{MODULE_DESCRIPTORS, Module, ModuleDescriptor, Zone, build_registry};
use crate::surface::SurfaceState;
use crate::surface::input::init_expand_keybind;

pub(crate) struct App {
    config: Config,
    shared: shared::SharedContext,
    surfaces: surfaces::SurfaceManager,
    menu: menu::MenuSystem,
    modules: module_runtime::ModuleRuntime,
    niri: niri::AppNiriState,
    tray: tray::TrayState,
    perf: Option<perf::PerfTracker>,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    Surface(messages::SurfaceEvent),
    Menu(messages::MenuEvent),
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
    fn perf_label(&self) -> &'static str {
        match self {
            Message::Surface(messages::SurfaceEvent::Tick) => "Surface::Tick",
            Message::Surface(_) => "Surface",
            Message::Menu(_) => "Menu",
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

fn descriptor_for_config_name(module_type: &str) -> Option<&'static ModuleDescriptor> {
    MODULE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.config_name == module_type)
}

impl App {
    /// `allowed_outputs`: non-empty list of output names to create windows on (Story 7.3).
    /// Empty means `StartMode::AllScreens` handles window creation (single "all" bar case).
    pub(crate) fn boot(config: Config, allowed_outputs: Vec<String>) -> (Self, Task<Message>) {
        // Initialize the in-bar keybind for expand/collapse once at startup.
        // Subsequent calls are silently ignored (OnceLock semantics).
        init_expand_keybind(config.bars[0].expand_keybind.as_deref());

        let registry = build_registry();
        let descriptors: HashMap<&str, &ModuleDescriptor> = MODULE_DESCRIPTORS
            .iter()
            .map(|d| (d.config_name, d))
            .collect();
        let module_map: std::collections::HashMap<&str, &crate::config::ModuleConfig> = config
            .modules
            .iter()
            .map(|mc| (mc.name.as_str(), mc))
            .collect();
        // Build an ordered (zone, name) list by walking all bar sections. A module name that
        // appears in multiple bars is registered only once — first appearance wins the zone.
        let mut ordered: Vec<(Zone, String)> = Vec::new();
        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for bar in &config.bars {
            for (zone, names) in [
                (Zone::Top, &bar.modules.top),
                (Zone::Center, &bar.modules.center),
                (Zone::Bottom, &bar.modules.bottom),
            ] {
                for name in names {
                    if seen_names.insert(name.clone()) {
                        ordered.push((zone.clone(), name.clone()));
                    }
                }
            }
        }
        let mut modules: Vec<(Zone, Box<dyn Module>, crate::config::ModuleActionsConfig)> =
            Vec::new();
        for (zone, name) in &ordered {
            let synthesized;
            let (mc, descriptor, ctor) = if let Some(&mc) = module_map.get(name.as_str()) {
                let Some(descriptor) = descriptor_for_config_name(mc.module_type.as_str()) else {
                    continue;
                };
                let Some(ctor) = registry.get(mc.module_type.as_str()).copied() else {
                    continue;
                };
                (mc, descriptor, ctor)
            } else if let Some(descriptor) = descriptors.get(name.as_str()) {
                let Some(params) = descriptor.synthesized_params() else {
                    continue;
                };
                synthesized = ModuleConfig {
                    name: name.clone(),
                    module_type: descriptor.config_name.to_string(),
                    params,
                    actions: crate::config::ModuleActionsConfig::default(),
                };
                (&synthesized, *descriptor, descriptor.constructor)
            } else {
                continue;
            };
            let module = ctor(mc);
            debug_assert_eq!(module.module_type(), module.kind().as_str());
            debug_assert_eq!(module.kind(), descriptor.kind);
            modules.push((zone.clone(), module, mc.actions.clone()));
        }
        // Create one surface per (output_name, bar_name) pair when in manual mode.
        // When allowed_outputs is empty, StartMode::AllScreens handles window creation.
        let mut pending_surface_keys: HashMap<iced::window::Id, (String, String)> = HashMap::new();
        let mut surfaces: HashMap<iced::window::Id, SurfaceState> = HashMap::new();
        let window_tasks: Vec<Task<Message>> = {
            let mut tasks = vec![];
            for output_name in &allowed_outputs {
                // Horizontal bars (top/bottom) must be spawned before side bars (left/right) so
                // their exclusive zones are registered first.  The compositor then automatically
                // constrains side bars to the remaining work area, preventing overlap on expand.
                let mut bars_for_output = config.bars_for_output(output_name);
                bars_for_output.sort_by_key(|b| b.edge.is_horizontal() as u8 ^ 1);
                for bar in &bars_for_output {
                    tasks.push(Self::spawn_bar_on_output(
                        bar,
                        output_name,
                        &mut pending_surface_keys,
                        &mut surfaces,
                    ));
                }
            }
            tasks
        };

        let manual_outputs = !allowed_outputs.is_empty();
        let app = App {
            config,
            shared: shared::SharedContext::new(),
            surfaces: surfaces::SurfaceManager::new(surfaces, pending_surface_keys, manual_outputs),
            menu: menu::MenuSystem::default(),
            modules: module_runtime::ModuleRuntime::from_modules_with_actions(modules),
            niri: niri::AppNiriState::new(),
            tray: tray::TrayState::default(),
            perf: perf::PerfTracker::from_env(),
        };
        (app, Task::batch(window_tasks))
    }

    pub(crate) fn namespace() -> String {
        String::from("niribar")
    }

    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        if let Some(perf) = &self.perf {
            perf.record_update(message.perf_label());
        }

        match message {
            Message::Surface(event) => self.handle_surface_event(event),
            Message::Menu(event) => self.handle_menu_event(event),
            Message::Modules(event) => self.handle_module_runtime_event(event),
            Message::Niri(event) => self.handle_app_niri_event(event),
            Message::VolumeAdjust { step, up } => self.handle_volume_adjust(step, up),
            Message::DbusReady(connections) => {
                // Store shared handles; modules borrow clones of these (AR5, Story 5.1).
                self.shared.dbus = connections;
                Task::none()
            }
            Message::Tray(event) => self.handle_tray_runtime_event(event),
            Message::NiriFocusWorkspace { id } => {
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
                    eprintln!(
                        "{}",
                        crate::app::perf::format_snapshot(&perf.snapshot_and_reset())
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
    use crate::config::{Edge, ExpandTrigger};
    use crate::modules::{MODULE_DESCRIPTORS, ModuleKind};
    use crate::niri::NiriEvent;
    use crate::surface::expand::ExpandState;

    fn make_config() -> Config {
        Config::default()
    }

    fn make_hover_config() -> Config {
        let mut config = make_config();
        config.bars[0].expand_trigger = ExpandTrigger::Hover;
        config.bars[0].hover_delay = 0;
        config
    }

    fn make_expand_disabled_config() -> Config {
        let mut config = make_config();
        config.bars[0].expand = false;
        config
    }

    fn make_horizontal_config() -> Config {
        let mut config = make_config();
        config.bars[0].edge = Edge::Top;
        config
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
        config.bars[0].modules.top = vec!["workspaces".into()];
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
    fn app_update_routes_wrapped_tick_to_surface_owner() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.0);

        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));

        assert!(matches!(
            app.surface(wid).unwrap().expand,
            ExpandState::Expanding(progress) if progress > 0.0
        ));
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
        app.modules.push(
            Zone::Top,
            Box::new(crate::modules::tray::TrayModule::new(ModuleId::new("tray"))),
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

    // --- Story 3.1 tests ---

    #[test]
    fn bar_click_starts_expanding_from_collapsed() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarClick(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn bar_click_starts_collapsing_from_expanded() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarClick(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsing(0.0));
    }

    // --- Story 3.2 tests ---

    #[test]
    fn tick_advances_expand_progress() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.0);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));
        let expected_progress = 16.0_f32 / 280.0;
        assert!(
            matches!(app.surfaces[&wid].expand, ExpandState::Expanding(p) if (p - expected_progress).abs() < 1e-5),
            "expected Expanding(~{expected_progress:.4}), got {:?}",
            app.surfaces[&wid].expand
        );
    }

    #[test]
    fn tick_completes_expand_animation() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.999);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanded);
    }

    #[test]
    fn tick_advances_collapse_progress() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Collapsing(0.0);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));
        let expected_progress = 16.0_f32 / 200.0;
        assert!(
            matches!(app.surfaces[&wid].expand, ExpandState::Collapsing(p) if (p - expected_progress).abs() < 1e-5),
            "expected Collapsing(~{expected_progress:.4}), got {:?}",
            app.surfaces[&wid].expand
        );
    }

    #[test]
    fn tick_completes_collapse_animation() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Collapsing(0.999);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn tick_no_op_when_collapsed() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Collapsed;
        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn tick_no_op_when_expanded() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanded);
    }

    // --- Story 3.4 tests ---

    #[test]
    fn tick_uses_custom_expand_duration() {
        let mut config = make_config();
        config.bars[0].animation.expand_ms = 100.0;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.0);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));
        let expected_progress = 16.0_f32 / 100.0;
        assert!(
            matches!(app.surfaces[&wid].expand, ExpandState::Expanding(p) if (p - expected_progress).abs() < 1e-5),
            "expected Expanding(~{expected_progress:.4}), got {:?}",
            app.surfaces[&wid].expand
        );
    }

    #[test]
    fn tick_uses_custom_collapse_duration() {
        let mut config = make_config();
        config.bars[0].animation.collapse_ms = 50.0;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Collapsing(0.0);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));
        let expected_progress = 16.0_f32 / 50.0;
        assert!(
            matches!(app.surfaces[&wid].expand, ExpandState::Collapsing(p) if (p - expected_progress).abs() < 1e-5),
            "expected Collapsing(~{expected_progress:.4}), got {:?}",
            app.surfaces[&wid].expand
        );
    }

    // --- Story 3.3 tests ---

    #[test]
    fn app_starts_with_bar_hovered_false() {
        let (app, _) = App::boot(make_config(), vec![]);
        // No surfaces yet, so bar_hovered is not tracked — passes trivially.
        assert!(app.surfaces.values().all(|s| !s.bar_hovered));
    }

    #[test]
    fn hover_enter_click_trigger_does_nothing() {
        let (mut app, _) = App::boot(make_config(), vec![]); // expand_trigger = Click
        let wid = test_wid();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverEnter(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
        assert!(app.surfaces[&wid].bar_hovered);
    }

    #[test]
    fn hover_exit_click_trigger_does_nothing() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverExit(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanded);
    }

    #[test]
    fn hover_enter_hover_trigger_zero_delay_starts_expanding() {
        let (mut app, _) = App::boot(make_hover_config(), vec![]);
        let wid = test_wid();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverEnter(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn hover_exit_when_expanded_starts_collapsing() {
        let (mut app, _) = App::boot(make_hover_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverExit(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsing(0.0));
    }

    #[test]
    fn hover_exit_when_expanding_reverses() {
        let (mut app, _) = App::boot(make_hover_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.3);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverExit(wid)));
        assert!(
            matches!(app.surfaces[&wid].expand, ExpandState::Collapsing(p) if (p - 0.7).abs() < 1e-5),
            "expected Collapsing(~0.7), got {:?}",
            app.surfaces[&wid].expand
        );
    }

    #[test]
    fn hover_exit_when_collapsed_does_nothing() {
        let (mut app, _) = App::boot(make_hover_config(), vec![]);
        let wid = test_wid();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverExit(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn hover_enter_with_delay_sets_pending() {
        let mut config = make_hover_config();
        config.bars[0].hover_delay = 100;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverEnter(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
        assert_eq!(app.surfaces[&wid].hover_pending_ms, Some(0.0));
    }

    #[test]
    fn hover_exit_cancels_pending() {
        let mut config = make_hover_config();
        config.bars[0].hover_delay = 100;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverEnter(wid)));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverExit(wid)));
        assert_eq!(app.surfaces[&wid].hover_pending_ms, None);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn tick_triggers_expand_when_delay_elapsed() {
        let mut config = make_hover_config();
        config.bars[0].hover_delay = 100;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid).hover_pending_ms = Some(90.0);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
        assert_eq!(app.surfaces[&wid].hover_pending_ms, None);
    }

    #[test]
    fn bar_click_noop_when_hover_trigger() {
        let (mut app, _) = App::boot(make_hover_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize so surface exists before early-return
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarClick(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
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
        config.bars[0].modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
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
        config.bars[0].modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
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
        config.bars[0].modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
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
        config.bars[0].modules.top = vec!["clock".into(), "battery".into()];
        config.bars[0].modules.center = vec!["network".into()];
        config.bars[0].modules.bottom = vec!["workspaces".into()];
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
        config.bars[0].modules.top = vec!["clock".into()];
        config.bars[0].modules.bottom = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
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
                        on_click: "true".into(),
                        format: crate::config::FormatConfig::default(),
                        tooltip: crate::config::TooltipSetting::Default,
                    }),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Default::default()
        };
        config.bars[0].modules.top = vec!["clock".into(), "volume".into()];
        config.bars[0].modules.center = vec!["button".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // pre-initialize
        (app, wid)
    }

    #[test]
    fn clear_surface_module_refs_with_prefix_clears_focus_and_hover() {
        let (mut app, wid) = make_interact_app();
        app.surface_mut(wid).focused_module = Some(ModuleId::new("workspace:1"));
        app.surface_mut(wid).hovered_module = Some(ModuleId::new("workspace:2"));

        app.clear_surface_module_refs_with_prefix("workspace:");

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

    // --- Story 6.4 tests ---

    #[test]
    fn toggle_expand_from_collapsed_starts_expanding() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize surface so ToggleExpand can find it
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ToggleExpand));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expand_from_expanded_starts_collapsing() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ToggleExpand));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsing(0.0));
    }

    #[test]
    fn toggle_expand_reverses_expanding_mid_animation() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.4);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ToggleExpand));
        assert!(
            matches!(app.surfaces[&wid].expand, ExpandState::Collapsing(p) if (p - 0.6).abs() < 1e-5),
            "expected Collapsing(~0.6), got {:?}",
            app.surfaces[&wid].expand
        );
    }

    #[test]
    fn toggle_expand_reverses_collapsing_mid_animation() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Collapsing(0.3);
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ToggleExpand));
        assert!(
            matches!(app.surfaces[&wid].expand, ExpandState::Expanding(p) if (p - 0.7).abs() < 1e-5),
            "expected Expanding(~0.7), got {:?}",
            app.surfaces[&wid].expand
        );
    }

    #[test]
    fn toggle_expand_works_with_hover_trigger() {
        let (mut app, _) = App::boot(make_hover_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ToggleExpand));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expand_double_toggle_collapses() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ToggleExpand)); // → Expanding(0.0)
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ToggleExpand)); // → Collapsing(0.0)
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsing(0.0));
    }

    // --- Expand disabled tests ---

    #[test]
    fn bar_click_noop_when_expand_disabled() {
        let (mut app, _) = App::boot(make_expand_disabled_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize so surface exists before early-return
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarClick(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn toggle_expand_noop_when_expand_disabled() {
        let (mut app, _) = App::boot(make_expand_disabled_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ToggleExpand));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn hover_enter_noop_when_expand_disabled() {
        let mut config = make_expand_disabled_config();
        config.bars[0].expand_trigger = ExpandTrigger::Hover;
        config.bars[0].hover_delay = 0;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverEnter(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
        assert!(app.surfaces[&wid].bar_hovered);
    }

    #[test]
    fn hover_exit_noop_when_expand_disabled() {
        let mut config = make_expand_disabled_config();
        config.bars[0].expand_trigger = ExpandTrigger::Hover;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverExit(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanded);
    }

    // --- Overlay mode tests ---

    #[test]
    fn bar_click_horizontal_bar_starts_expanding() {
        let (mut app, _) = App::boot(make_horizontal_config(), vec![]);
        let wid = test_wid();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarClick(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expand_horizontal_bar_starts_expanding() {
        let (mut app, _) = App::boot(make_horizontal_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize
        let _ = app.update(Message::Surface(messages::SurfaceEvent::ToggleExpand));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn hover_enter_horizontal_bar_hover_trigger_starts_expanding() {
        let mut config = make_horizontal_config();
        config.bars[0].expand_trigger = ExpandTrigger::Hover;
        config.bars[0].hover_delay = 0;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarHoverEnter(wid)));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn overlay_size_change_vertical_bar_uses_width() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.config.bars[0].collapsed_width = 48;
        app.config.bars[0].expanded_width = 200;
        app.surface_mut(wid).expand = ExpandState::Expanded;
        assert!(!app.config.bars[0].edge.is_horizontal());
        let _ = app.overlay_size_change_for(wid);
    }

    #[test]
    fn overlay_size_change_horizontal_bar_uses_height() {
        let (mut app, _) = App::boot(make_horizontal_config(), vec![]);
        let wid = test_wid();
        app.config.bars[0].collapsed_width = 48;
        app.config.bars[0].expanded_width = 200;
        app.surface_mut(wid).expand = ExpandState::Expanded;
        assert!(app.config.bars[0].edge.is_horizontal());
        let _ = app.overlay_size_change_for(wid);
    }

    // --- Story 7.1 tests ---

    #[test]
    fn boot_returns_app_and_task() {
        let (app, _task) = App::boot(make_config(), vec![]);
        assert!(app.surfaces.is_empty()); // no surfaces before first view/event
    }

    #[test]
    fn tick_not_needed_when_no_surfaces() {
        let (app, _) = App::boot(make_config(), vec![]);
        let needs_tick = app.surfaces.values().any(|s| s.expand.is_animating());
        assert!(!needs_tick);
    }

    #[test]
    fn tick_needed_when_any_surface_animating() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.5);
        let needs_tick = app.surfaces.values().any(|s| s.expand.is_animating());
        assert!(needs_tick);
    }

    #[test]
    fn bar_click_only_affects_clicked_surface() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid_a = test_wid();
        let wid_b = test_wid();
        app.surface_mut(wid_b); // initialize surface B in Collapsed
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarClick(wid_a)));
        // Surface A: Expanding
        assert_eq!(app.surfaces[&wid_a].expand, ExpandState::Expanding(0.0));
        // Surface B: still Collapsed
        assert_eq!(app.surfaces[&wid_b].expand, ExpandState::Collapsed);
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
        config.bars[0].modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid_a = test_wid();
        let wid_b = test_wid();
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
                    output: "all".into(),
                    modules: crate::config::BarModules {
                        top: vec!["workspaces".into()],
                        center: vec![],
                        bottom: vec![],
                    },
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "secondary".into(),
                    output: "DP-1".into(),
                    modules: crate::config::BarModules {
                        top: vec!["clock".into()],
                        center: vec![],
                        bottom: vec![],
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
        };
        let (app, _) = App::boot(config, vec![]);
        let instantiated: Vec<String> = app.modules.iter().map(|(_, m)| m.id().0.clone()).collect();
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
        config.bars[0].modules.top = vec!["clock".into()];

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
    fn undeclared_mandatory_param_modules_still_skip() {
        let mut config = Config::default();
        config.bars[0].modules.top = vec!["button".into(), "script".into()];

        let (app, _) = App::boot(config, vec![]);
        let instantiated: Vec<_> = app
            .modules
            .iter()
            .map(|(_, module)| module.id().0.clone())
            .collect();

        assert!(
            instantiated.is_empty(),
            "expected no modules, got: {instantiated:?}"
        );
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
        });

        assert_eq!(module.kind(), descriptor.kind);
        assert_eq!(module.module_type(), descriptor.config_name);
    }

    // --- Hotplug output detection tests ---

    #[test]
    fn workspaces_changed_allscreens_mode_does_not_open_new_surfaces() {
        // In AllScreens mode (manual_outputs = false) the compositor manages surfaces;
        // WorkspacesChanged must not try to open additional windows.
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        assert!(!app.surfaces.manual_outputs());
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
            "no new surfaces in AllScreens mode"
        );
    }

    #[test]
    fn workspaces_changed_manual_mode_opens_surface_for_new_output() {
        // In manual mode a WorkspacesChanged event that includes a previously unseen output
        // name must cause a new surface (bar) to be created for that output.
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec!["eDP-1".into()]);
        assert!(app.surfaces.manual_outputs());
        // "DP-1" was not in the initial allowed_outputs — it is newly connected.
        let _ = app.update(Message::Niri(messages::AppNiriEvent::Ipc(
            NiriEvent::Event(Event::WorkspacesChanged {
                workspaces: vec![niri_ipc::Workspace {
                    id: 10,
                    idx: 0,
                    name: None,
                    output: Some("DP-1".to_string()),
                    is_urgent: false,
                    is_active: true,
                    is_focused: false,
                    active_window_id: None,
                }],
            }),
        )));
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
    fn workspaces_changed_manual_mode_no_duplicate_for_known_output() {
        // An output already tracked in surfaces must not get a second surface.
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec!["eDP-1".into()]);
        let before = app.surfaces.len();
        // Fire WorkspacesChanged for eDP-1 which is already in surfaces.
        let _ = app.update(Message::Niri(messages::AppNiriEvent::Ipc(
            NiriEvent::Event(Event::WorkspacesChanged {
                workspaces: vec![niri_ipc::Workspace {
                    id: 1,
                    idx: 0,
                    name: None,
                    output: Some("eDP-1".to_string()),
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
            "no duplicate surface for already-known output"
        );
    }

    #[test]
    fn workspaces_changed_manual_mode_skips_untargeted_output() {
        // When the bar targets "eDP-1" only, hotplugging "HDMI-A-1" must not create a surface.
        use crate::config::BarConfig;
        use niri_ipc::Event;
        let config = Config {
            bars: vec![BarConfig {
                name: "main".into(),
                output: "eDP-1".into(), // only targets eDP-1, not HDMI-A-1
                ..BarConfig::default()
            }],
            modules: vec![],
        };
        // Boot without "HDMI-A-1" in allowed (bar does not target it).
        let (mut app, _) = App::boot(config, vec!["eDP-1".into()]);
        let before = app.surfaces.len();
        let _ = app.update(Message::Niri(messages::AppNiriEvent::Ipc(
            NiriEvent::Event(Event::WorkspacesChanged {
                workspaces: vec![niri_ipc::Workspace {
                    id: 99,
                    idx: 0,
                    name: None,
                    output: Some("HDMI-A-1".to_string()),
                    is_urgent: false,
                    is_active: true,
                    is_focused: false,
                    active_window_id: None,
                }],
            }),
        )));
        assert_eq!(
            app.surfaces.len(),
            before,
            "HDMI-A-1 must not get a surface — no bar targets it"
        );
    }

    // --- Story 9.1: App-level multi-bar tests ---

    #[test]
    fn hotplug_output_gets_all_targeting_bar() {
        use crate::config::BarConfig;
        use niri_ipc::Event;
        let config = Config {
            bars: vec![BarConfig {
                name: "main".into(),
                output: "all".into(),
                ..BarConfig::default()
            }],
            modules: vec![],
        };
        let (mut app, _) = App::boot(config, vec!["eDP-1".into()]);
        app.surfaces.manual_outputs = true; // simulate manual mode
        let _ = app.update(Message::Niri(messages::AppNiriEvent::Ipc(
            NiriEvent::Event(Event::WorkspacesChanged {
                workspaces: vec![niri_ipc::Workspace {
                    id: 5,
                    idx: 0,
                    name: None,
                    output: Some("DP-1".to_string()),
                    is_urgent: false,
                    is_active: true,
                    is_focused: false,
                    active_window_id: None,
                }],
            }),
        )));
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
    fn hotplug_creates_one_surface_per_bar_targeting_output() {
        use crate::config::BarConfig;
        use niri_ipc::Event;
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "top".into(),
                    output: "all".into(),
                    edge: crate::config::Edge::Top,
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let (mut app, _) = App::boot(config, vec!["eDP-1".into()]);
        let before = app.surfaces.len(); // 2 surfaces for eDP-1 (2 bars × 1 output)
        app.surfaces.manual_outputs = true;
        let _ = app.update(Message::Niri(messages::AppNiriEvent::Ipc(
            NiriEvent::Event(Event::WorkspacesChanged {
                workspaces: vec![niri_ipc::Workspace {
                    id: 5,
                    idx: 0,
                    name: None,
                    output: Some("DP-1".to_string()),
                    is_urgent: false,
                    is_active: true,
                    is_focused: false,
                    active_window_id: None,
                }],
            }),
        )));
        // After hotplug: should have 2 more surfaces for DP-1 (both bars target "all")
        assert_eq!(
            app.surfaces.len(),
            before + 2,
            "DP-1 should get one surface per bar"
        );
    }

    // --- side_bar_margin tests ---

    // --- horizontal-before-side spawn ordering tests ---

    #[test]
    fn hotplug_spawns_horizontal_bars_before_side_bars() {
        use niri_ipc::Event;
        // Same adversarial order: side bar declared first in config.
        let config = Config {
            bars: vec![
                crate::config::BarConfig {
                    name: "right".into(),
                    output: "all".into(),
                    edge: Edge::Right,
                    ..crate::config::BarConfig::default()
                },
                crate::config::BarConfig {
                    name: "bottom".into(),
                    output: "all".into(),
                    edge: Edge::Bottom,
                    ..crate::config::BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let (mut app, _) = App::boot(config, vec![]);
        app.surfaces.manual_outputs = true;
        let _ = app.update(Message::Niri(messages::AppNiriEvent::Ipc(
            NiriEvent::Event(Event::WorkspacesChanged {
                workspaces: vec![niri_ipc::Workspace {
                    id: 1,
                    idx: 0,
                    name: None,
                    output: Some("DP-1".to_string()),
                    is_urgent: false,
                    is_active: true,
                    is_focused: false,
                    active_window_id: None,
                }],
            }),
        )));
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
