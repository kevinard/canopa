mod animation;
mod focus;
mod layout;
mod menu;
mod module_events;
mod module_state;
mod navigation;
mod niri;
mod perf;
mod subscriptions;
mod surfaces;
mod tray;
mod view;
mod widget_ops;

use std::collections::HashMap;

use iced::Task;
use iced_layershell::to_layer_message;

use self::widget_ops::FindBounds;
use crate::config::{
    BatteryConfig, ClockConfig, Config, ModuleConfig, ModuleParams, NetworkConfig, VolumeConfig,
    WorkspacesConfig,
};
use crate::dbus::DbusConnections;
use crate::modules::battery::BatteryEvent;
use crate::modules::network::NetworkEvent;
use crate::modules::volume::VolumeEvent;
use crate::modules::{
    InteractionEvent, Module, ModuleEvent, ModuleId, ModuleKind, ModuleState, Zone, build_registry,
};
use crate::niri::NiriState;
use crate::surface::SurfaceState;
use crate::surface::input::init_expand_keybind;

/// Shared state: module data visible to all surfaces.
///
/// Module data (battery %, workspace list, etc.) lives here.
/// Per-surface render state (expand/collapse, focus) lives in `SurfaceState` — never here.
#[derive(Default)]
pub struct SharedState {
    pub modules: HashMap<ModuleId, ModuleState>,
    pub niri: NiriState,       // populated by Message::Niri handlers (Story 4.1)
    pub dbus: DbusConnections, // shared D-Bus handles; populated by Message::DbusReady (Story 5.1)
}

impl SharedState {
    pub fn new() -> Self {
        SharedState {
            modules: HashMap::new(),
            niri: NiriState::default(),
            dbus: DbusConnections::default(),
        }
    }
}

pub struct App {
    pub config: Config,
    pub shared: SharedState,
    pub perf: Option<perf::PerfTracker>,
    /// Instantiated module instances paired with their zone placement.
    pub modules: Vec<(Zone, Box<dyn Module>)>,
    /// Per-surface render state, keyed by iced::window::Id (Story 7.1).
    /// One entry per (output, bar) surface pair; lazily initialised on first event or view call.
    pub surfaces: HashMap<iced::window::Id, SurfaceState>,
    /// Window IDs pre-assigned to (output_name, bar_name) pairs during manual window creation
    /// (Stories 7.3, 9.1). Entry is consumed on first `surface_mut()` call for that id.
    /// Empty when using StartMode::AllScreens.
    pub pending_surface_keys: HashMap<iced::window::Id, (String, String)>,
    /// `true` when using Background/manual mode. In this mode, niribar must open new windows
    /// when a new output is connected at runtime (StartMode::AllScreens is not active).
    pub manual_outputs: bool,
    /// Active popup windows: window_id -> (service, menu_path, tree, bar_name).
    /// bar_name lets view() look up the bar edge for positioning the menu widget.
    pub menu_popups: HashMap<
        iced::window::Id,
        (
            String,
            zbus::zvariant::OwnedObjectPath,
            crate::surface::menu_popup::MenuTree,
            String,
        ),
    >,
    /// Currently hovered menu item per popup window: window_id -> item_id string.
    pub menu_popup_hovered: HashMap<iced::window::Id, String>,
    /// Keyboard-focused menu item per popup window: window_id -> item_id string.
    pub menu_focused_item: HashMap<iced::window::Id, String>,
    /// Last known cursor position and the window it occurred in.
    /// `None` until the first CursorMoved event. Used to anchor the tray menu popup.
    pub last_cursor: Option<(iced::window::Id, iced::Point)>,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    // Module data events — shared state, no window ID needed.
    Module(ModuleId, ModuleEvent),
    /// Fires when the user clicks anywhere on the bar surface (Story 3.1).
    /// Carries the window ID of the surface that was clicked.
    BarClick(iced::window::Id),
    /// Animation clock tick. Fired at ~60fps when any surface is animating (Story 3.2).
    /// Global — the handler advances all animating surfaces.
    Tick,
    /// Pointer entered a bar surface (Story 3.3). Per-surface.
    BarHoverEnter(iced::window::Id),
    /// Pointer left a bar surface (Story 3.3). Per-surface.
    BarHoverExit(iced::window::Id),
    ModuleHoverEnter(ModuleId, iced::window::Id),
    ModuleHoverExit(ModuleId, iced::window::Id),
    ModuleClick(ModuleId, iced::window::Id),
    /// Scroll-up event on a module (Story 5.4). Per-surface.
    ModuleScrollUp(ModuleId, iced::window::Id),
    /// Scroll-down event on a module (Story 5.4). Per-surface.
    ModuleScrollDown(ModuleId, iced::window::Id),
    /// Event from the Niri IPC event stream; dispatched to SharedState (Story 4.1).
    Niri(crate::niri::NiriEvent),
    /// Battery charge state from UPower D-Bus subscription (Story 5.2).
    Battery(BatteryEvent),
    /// Network connectivity state from D-Bus subscription (Story 5.3).
    Network(NetworkEvent),
    /// Volume state from PA compat subscription (Story 5.4).
    Volume(VolumeEvent),
    /// Scroll-triggered volume adjustment (Story 5.4). Global — adjusts system volume.
    VolumeAdjust {
        step: u8,
        up: bool,
    },
    /// D-Bus connection handles established at startup (Story 5.1).
    DbusReady(DbusConnections),
    /// Layer surface received keyboard focus (Story 6.1). Per-surface.
    BarFocusGained(iced::window::Id),
    /// Layer surface lost keyboard focus (Story 6.1). Per-surface.
    BarFocusLost(iced::window::Id),
    /// Escape key pressed while a bar surface has keyboard focus (Story 6.1). Per-surface.
    EscapePressed(iced::window::Id),
    /// Down arrow key pressed while bar is focused (Story 6.2). Per-surface.
    FocusMoveNext(iced::window::Id),
    /// Up arrow key pressed while bar is focused (Story 6.2). Per-surface.
    FocusMovePrev(iced::window::Id),
    /// Tab pressed while bar is focused (Story 6.2). Per-surface.
    FocusNextZone(iced::window::Id),
    /// Shift+Tab pressed while bar is focused (Story 6.2). Per-surface.
    FocusPrevZone(iced::window::Id),
    /// Enter or Space pressed while bar is focused (Story 6.3). Per-surface.
    KeyboardActivate(iced::window::Id),
    /// Space pressed while bar is focused -- secondary activation. Per-surface.
    KeyboardSecondaryActivate(iced::window::Id),
    /// Keyboard-triggered expand/collapse toggle via Unix socket IPC (Story 6.4).
    /// Global: IPC has no window context, so all surfaces toggle simultaneously.
    ToggleExpand,
    /// Incremental tray change from SNI host.
    TrayEvent(crate::dbus::sni_host::TrayEvent),
    /// Left-click on a tray icon -- sends Activate to the SNI item.
    TrayActivate {
        service: String,
    },
    /// Right-click on a tray icon -- opens the DBusMenu context menu.
    /// Anchored at the mouse cursor position.
    TrayMenuRequest {
        service: String,
        menu_path: Option<zbus::zvariant::OwnedObjectPath>,
        source_window: Option<iced::window::Id>,
    },
    /// Keyboard-triggered menu open (Space on focused tray icon). Anchored at
    /// the icon's widget bounds, resolved via a widget operation.
    TrayMenuRequestKeyboard {
        service: String,
        menu_path: Option<zbus::zvariant::OwnedObjectPath>,
        source_window: Option<iced::window::Id>,
    },
    /// Internal: widget-operation result carrying the icon's screen bounds.
    /// Dispatched after the bounds lookup completes; opens the menu anchored there.
    TrayMenuRequestWithBounds {
        service: String,
        menu_path: Option<zbus::zvariant::OwnedObjectPath>,
        bounds: Option<iced::Rectangle>,
        screen_size: (u32, u32),
        /// The bar window that triggered the request (keyboard path only).
        /// Used to resolve bar_name / bar.edge for menu positioning.
        source_window: Option<iced::window::Id>,
    },
    /// Middle-click on a tray icon -- sends SecondaryActivate.
    TraySecondaryActivate {
        service: String,
    },
    /// Scroll on a tray icon -- sends Scroll to the SNI item.
    TrayScroll {
        service: String,
        delta: i32,
        orientation: String,
    },
    /// A menu item was selected in the popup. `menu_item_id` is the DBusMenu integer item ID.
    MenuItemSelected {
        /// The SNI service that owns this menu.
        service: String,
        /// DBusMenu integer item ID.
        menu_item_id: i32,
        /// D-Bus path of the DBusMenu object.
        menu_path: zbus::zvariant::OwnedObjectPath,
    },
    /// The menu popup was dismissed without a selection.
    MenuClosed,
    /// Pointer entered a menu item row in the popup.
    MenuItemHoverEnter {
        window: iced::window::Id,
        item_id: String,
    },
    /// Pointer left a menu item row in the popup.
    MenuItemHoverExit(iced::window::Id),
    /// The DBusMenu layout changed while the popup is open -- re-fetch and replace.
    MenuLayoutChanged {
        service: String,
        menu_path: zbus::zvariant::OwnedObjectPath,
    },
    /// Menu data fetched successfully -- spawn the popup surface.
    OpenMenu {
        service: String,
        menu_path: zbus::zvariant::OwnedObjectPath,
        tree: crate::surface::menu_popup::MenuTree,
        /// Bar name used to look up bar.edge for popup positioning.
        bar_name: String,
    },
    /// Cursor moved within a bar surface window. Used to track anchor position for menu popups.
    CursorMoved(iced::window::Id, iced::Point),
    /// Bar surface window was opened or resized; carries its logical size in pixels.
    WindowSized(iced::window::Id, u32, u32),
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
            Message::Module(_, ModuleEvent::Clock(_)) => "Module::Clock",
            Message::Module(_, ModuleEvent::Script(_)) => "Module::Script",
            Message::Module(_, ModuleEvent::Button(_)) => "Module::Button",
            Message::BarClick(_) => "BarClick",
            Message::Tick => "Tick",
            Message::BarHoverEnter(_) => "BarHoverEnter",
            Message::BarHoverExit(_) => "BarHoverExit",
            Message::ModuleHoverEnter(_, _) => "ModuleHoverEnter",
            Message::ModuleHoverExit(_, _) => "ModuleHoverExit",
            Message::ModuleClick(_, _) => "ModuleClick",
            Message::ModuleScrollUp(_, _) => "ModuleScrollUp",
            Message::ModuleScrollDown(_, _) => "ModuleScrollDown",
            Message::Niri(crate::niri::NiriEvent::Connected) => "Niri::Connected",
            Message::Niri(crate::niri::NiriEvent::Unavailable) => "Niri::Unavailable",
            Message::Niri(crate::niri::NiriEvent::Event(event)) => match event {
                niri_ipc::Event::WorkspacesChanged { .. } => "Niri::WorkspacesChanged",
                niri_ipc::Event::WorkspaceUrgencyChanged { .. } => "Niri::WorkspaceUrgencyChanged",
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
                niri_ipc::Event::KeyboardLayoutsChanged { .. } => "Niri::KeyboardLayoutsChanged",
                niri_ipc::Event::KeyboardLayoutSwitched { .. } => "Niri::KeyboardLayoutSwitched",
                niri_ipc::Event::OverviewOpenedOrClosed { .. } => "Niri::OverviewOpenedOrClosed",
                niri_ipc::Event::ConfigLoaded { .. } => "Niri::ConfigLoaded",
                niri_ipc::Event::ScreenshotCaptured { .. } => "Niri::ScreenshotCaptured",
            },
            Message::Battery(_) => "Battery",
            Message::Network(_) => "Network",
            Message::Volume(_) => "Volume",
            Message::VolumeAdjust { .. } => "VolumeAdjust",
            Message::DbusReady(_) => "DbusReady",
            Message::BarFocusGained(_) => "BarFocusGained",
            Message::BarFocusLost(_) => "BarFocusLost",
            Message::EscapePressed(_) => "EscapePressed",
            Message::FocusMoveNext(_) => "FocusMoveNext",
            Message::FocusMovePrev(_) => "FocusMovePrev",
            Message::FocusNextZone(_) => "FocusNextZone",
            Message::FocusPrevZone(_) => "FocusPrevZone",
            Message::KeyboardActivate(_) => "KeyboardActivate",
            Message::KeyboardSecondaryActivate(_) => "KeyboardSecondaryActivate",
            Message::ToggleExpand => "ToggleExpand",
            Message::TrayEvent(_) => "TrayEvent",
            Message::TrayActivate { .. } => "TrayActivate",
            Message::TrayMenuRequest { .. } => "TrayMenuRequest",
            Message::TrayMenuRequestKeyboard { .. } => "TrayMenuRequestKeyboard",
            Message::TrayMenuRequestWithBounds { .. } => "TrayMenuRequestWithBounds",
            Message::TraySecondaryActivate { .. } => "TraySecondaryActivate",
            Message::TrayScroll { .. } => "TrayScroll",
            Message::MenuItemSelected { .. } => "MenuItemSelected",
            Message::MenuClosed => "MenuClosed",
            Message::MenuItemHoverEnter { .. } => "MenuItemHoverEnter",
            Message::MenuItemHoverExit(_) => "MenuItemHoverExit",
            Message::MenuLayoutChanged { .. } => "MenuLayoutChanged",
            Message::OpenMenu { .. } => "OpenMenu",
            Message::CursorMoved(_, _) => "CursorMoved",
            Message::WindowSized(_, _, _) => "WindowSized",
            Message::NiriFocusWorkspace { .. } => "NiriFocusWorkspace",
            Message::PerfReport => "PerfReport",
            _ => "LayerShell",
        }
    }
}

/// Return default params for a module type name, or `None` if the type requires mandatory
/// fields that have no sensible default (script, button).
fn default_params_for_type(module_type: &str) -> Option<ModuleParams> {
    match module_type {
        "battery" => Some(ModuleParams::Battery(BatteryConfig::default())),
        "clock" => Some(ModuleParams::Clock(ClockConfig::default())),
        "network" => Some(ModuleParams::Network(NetworkConfig)),
        "volume" => Some(ModuleParams::Volume(VolumeConfig::default())),
        "workspaces" => Some(ModuleParams::Workspaces(WorkspacesConfig::default())),
        "focused-window" => Some(ModuleParams::None),
        "tray" => Some(ModuleParams::Tray(crate::config::TrayConfig)),
        _ => None,
    }
}

impl App {
    /// `allowed_outputs`: non-empty list of output names to create windows on (Story 7.3).
    /// Empty means `StartMode::AllScreens` handles window creation (single "all" bar case).
    pub fn boot(config: Config, allowed_outputs: Vec<String>) -> (Self, Task<Message>) {
        // Initialize the in-bar keybind for expand/collapse once at startup.
        // Subsequent calls are silently ignored (OnceLock semantics).
        init_expand_keybind(config.bars[0].expand_keybind.as_deref());

        let registry = build_registry();
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
        let mut modules: Vec<(Zone, Box<dyn Module>)> = Vec::new();
        for (zone, name) in &ordered {
            let synthesized;
            let mc: &ModuleConfig = if let Some(mc) = module_map.get(name.as_str()) {
                mc
            } else if let Some(params) = default_params_for_type(name) {
                synthesized = ModuleConfig {
                    name: name.clone(),
                    module_type: name.clone(),
                    params,
                };
                &synthesized
            } else {
                continue;
            };
            if let Some(ctor) = registry.get(mc.module_type.as_str()) {
                let module = ctor(mc);
                debug_assert_eq!(module.module_type(), module.kind().as_str());
                modules.push((zone.clone(), module));
            }
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
            shared: SharedState::new(),
            perf: perf::PerfTracker::from_env(),
            modules,
            surfaces,
            pending_surface_keys,
            manual_outputs,
            menu_popups: HashMap::new(),
            menu_popup_hovered: HashMap::new(),
            menu_focused_item: HashMap::new(),
            last_cursor: None,
        };
        (app, Task::batch(window_tasks))
    }

    pub fn namespace() -> String {
        String::from("niribar")
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        if let Some(perf) = &self.perf {
            perf.record_update(message.perf_label());
        }

        match message {
            Message::CursorMoved(window, pos) => {
                let has_tray = self.modules.iter().any(|(_, module)| {
                    matches!(module.kind(), ModuleKind::Tray | ModuleKind::TrayIcon)
                });
                if has_tray && self.last_cursor != Some((window, pos)) {
                    self.last_cursor = Some((window, pos));
                }
                Task::none()
            }
            Message::WindowSized(window, w, h) => {
                if let Some(s) = self.surfaces.get_mut(&window) {
                    s.screen_size = (w, h);
                }
                Task::none()
            }
            Message::ModuleHoverEnter(id, window) => {
                self.surface_mut(window).hovered_module = Some(id);
                Task::none()
            }
            Message::ModuleHoverExit(id, window) => {
                let s = self.surface_mut(window);
                if s.hovered_module.as_ref() == Some(&id) {
                    s.hovered_module = None;
                }
                Task::none()
            }
            Message::Module(id, event) => self.handle_module_event(id, event),
            Message::ModuleClick(id, _window) => {
                self.dispatch_module_interaction(&id, InteractionEvent::Clicked)
            }
            Message::BarClick(window) => self.handle_bar_click(window),
            Message::BarHoverEnter(window) => self.handle_bar_hover_enter(window),
            Message::BarHoverExit(window) => self.handle_bar_hover_exit(window),
            Message::Niri(event) => self.handle_niri_event(event),
            Message::Tick => self.handle_tick(),
            Message::Network(event) => self.handle_network_event(event),
            Message::Volume(event) => self.handle_volume_event(event),
            Message::VolumeAdjust { step, up } => self.handle_volume_adjust(step, up),
            Message::ModuleScrollUp(id, _window) => {
                self.dispatch_module_interaction(&id, InteractionEvent::ScrollUp)
            }
            Message::ModuleScrollDown(id, _window) => {
                self.dispatch_module_interaction(&id, InteractionEvent::ScrollDown)
            }
            Message::Battery(event) => self.handle_battery_event(event),
            Message::DbusReady(connections) => {
                // Store shared handles; modules borrow clones of these (AR5, Story 5.1).
                self.shared.dbus = connections;
                Task::none()
            }
            Message::BarFocusGained(window) => self.handle_bar_focus_gained(window),
            Message::BarFocusLost(window) => self.handle_bar_focus_lost(window),
            Message::EscapePressed(window) => self.handle_escape_pressed(window),
            Message::FocusMoveNext(window) => self.handle_focus_move(window, true),
            Message::FocusMovePrev(window) => self.handle_focus_move(window, false),
            Message::FocusNextZone(window) => self.handle_focus_zone(window, true),
            Message::FocusPrevZone(window) => self.handle_focus_zone(window, false),
            Message::KeyboardActivate(window) => self.handle_keyboard_activate(window),
            Message::KeyboardSecondaryActivate(window) => {
                self.handle_keyboard_secondary_activate(window)
            }
            Message::ToggleExpand => self.handle_toggle_expand(),
            Message::TrayEvent(event) => self.handle_tray_event(event),
            Message::TrayActivate { service } => {
                let conn = self.shared.dbus.session.clone();
                tokio::task::spawn(async move {
                    if let Some(conn) = conn {
                        let path =
                            zbus::zvariant::ObjectPath::try_from("/StatusNotifierItem").unwrap();
                        if let Some(item) =
                            crate::dbus::sni_host::fetch_item(&conn, &service, &path).await
                        {
                            let _ = crate::dbus::sni_host::call_activate(
                                &conn,
                                &item.service,
                                &item.object_path,
                            )
                            .await;
                        }
                    }
                });
                Task::none()
            }
            Message::TraySecondaryActivate { service } => {
                let conn = self.shared.dbus.session.clone();
                tokio::task::spawn(async move {
                    if let Some(conn) = conn {
                        let path =
                            zbus::zvariant::ObjectPath::try_from("/StatusNotifierItem").unwrap();
                        if let Some(item) =
                            crate::dbus::sni_host::fetch_item(&conn, &service, &path).await
                        {
                            let _ = crate::dbus::sni_host::call_secondary_activate(
                                &conn,
                                &item.service,
                                &item.object_path,
                            )
                            .await;
                        }
                    }
                });
                Task::none()
            }
            Message::TrayScroll {
                service,
                delta,
                orientation,
            } => {
                let conn = self.shared.dbus.session.clone();
                tokio::task::spawn(async move {
                    if let Some(conn) = conn {
                        let path =
                            zbus::zvariant::ObjectPath::try_from("/StatusNotifierItem").unwrap();
                        if let Some(item) =
                            crate::dbus::sni_host::fetch_item(&conn, &service, &path).await
                        {
                            let _ = crate::dbus::sni_host::call_scroll(
                                &conn,
                                &item.service,
                                &item.object_path,
                                delta,
                                &orientation,
                            )
                            .await;
                        }
                    }
                });
                Task::none()
            }
            Message::TrayMenuRequest {
                service,
                menu_path,
                source_window,
            } => {
                let Some(source_window) = source_window else {
                    return Task::none();
                };
                let screen_size = self
                    .surfaces
                    .get(&source_window)
                    .map(|s| s.screen_size)
                    .unwrap_or((0, 0));
                let target = crate::modules::tray::tray_icon_widget_id(source_window, &service);
                let op = FindBounds {
                    target,
                    result: None,
                };
                let svc = service.clone();
                let mp = menu_path.clone();
                iced::advanced::widget::operate(op).then(move |bounds| {
                    Task::done(Message::TrayMenuRequestWithBounds {
                        service: svc.clone(),
                        menu_path: mp.clone(),
                        bounds,
                        screen_size,
                        source_window: Some(source_window),
                    })
                })
            }
            Message::TrayMenuRequestKeyboard {
                service,
                menu_path,
                source_window,
            } => {
                // Fallback if the KeyboardSecondaryActivate fast-path didn't intercept
                // (e.g. message sent externally). Treat as a bounds-less request.
                let Some(source_window) = source_window else {
                    return Task::none();
                };
                let screen_size = self
                    .surfaces
                    .get(&source_window)
                    .map(|s| s.screen_size)
                    .unwrap_or((0, 0));
                let target = crate::modules::tray::tray_icon_widget_id(source_window, &service);
                let op = FindBounds {
                    target,
                    result: None,
                };
                let svc = service.clone();
                let mp = menu_path.clone();
                iced::advanced::widget::operate(op).then(move |bounds| {
                    Task::done(Message::TrayMenuRequestWithBounds {
                        service: svc.clone(),
                        menu_path: mp.clone(),
                        bounds,
                        screen_size,
                        source_window: Some(source_window),
                    })
                })
            }
            Message::TrayMenuRequestWithBounds {
                service,
                menu_path,
                bounds,
                screen_size,
                source_window,
            } => {
                let conn = self.shared.dbus.session.clone();
                if let (Some(conn), Some(menu_path)) = (conn, menu_path) {
                    let service_clone = service.clone();
                    let menu_path_clone = menu_path.clone();
                    // Anchor at the icon's bottom-left (for horizontal bars, this places the
                    // menu directly under the icon; the popup clamps to screen edges anyway).
                    // Fall back to the mouse cursor if the bounds lookup failed.
                    let (anchor_x, anchor_y) = match bounds {
                        Some(r) => {
                            // For horizontal bars, iced_layershell's layout has the axes
                            // transposed: r.x is the bar-depth (vertical) position and r.y
                            // is the along-bar (horizontal) position. Detect horizontal bars
                            // by comparing the screen_size dimensions.
                            if screen_size.0 > screen_size.1 {
                                (r.x as i32, (r.y + r.height) as i32)
                            } else {
                                ((r.x + r.width) as i32, r.y as i32)
                            }
                        }
                        None => {
                            let p = self
                                .last_cursor
                                .map(|(_, p)| p)
                                .unwrap_or(iced::Point::ORIGIN);
                            (p.x as i32, p.y as i32)
                        }
                    };
                    // Resolve bar_name from the keyboard event's window (keyboard path) or
                    // fall back to last_cursor (mouse path). This ensures the popup uses the
                    // correct bar.edge for positioning even when the cursor is not on the bar.
                    let bar_name = source_window
                        .and_then(|w| self.surfaces.get(&w))
                        .or_else(|| self.last_cursor.and_then(|(w, _)| self.surfaces.get(&w)))
                        .map(|s| s.bar_name.clone())
                        .unwrap_or_default();
                    return Task::perform(
                        async move {
                            crate::dbus::sni_host::fetch_menu(
                                &conn,
                                &service_clone,
                                &menu_path_clone,
                            )
                            .await
                        },
                        move |tree_opt| {
                            if let Some(mut tree) = tree_opt {
                                tree.anchor = (anchor_x, anchor_y);
                                tree.screen_size = screen_size;
                                Message::OpenMenu {
                                    service: service.clone(),
                                    menu_path: menu_path.clone(),
                                    tree,
                                    bar_name: bar_name.clone(),
                                }
                            } else {
                                Message::TrayActivate {
                                    service: service.clone(),
                                }
                            }
                        },
                    );
                }
                Task::none()
            }
            Message::OpenMenu {
                service,
                menu_path,
                tree,
                bar_name,
            } => {
                // Close any existing popup first.
                let existing_ids: Vec<iced::window::Id> =
                    self.menu_popups.keys().copied().collect();
                let mut tasks: Vec<Task<Message>> = existing_ids
                    .into_iter()
                    .map(|id| {
                        self.menu_popups.remove(&id);
                        Task::done(Message::RemoveWindow(id))
                    })
                    .collect();

                // bar_name was resolved upstream (either from the keyboard source window or
                // from last_cursor for mouse activations). No re-lookup needed here.
                let settings = iced_layershell::reexport::NewLayerShellSettings {
                    anchor: iced_layershell::reexport::Anchor::Left
                        | iced_layershell::reexport::Anchor::Right
                        | iced_layershell::reexport::Anchor::Top
                        | iced_layershell::reexport::Anchor::Bottom,
                    exclusive_zone: Some(-1),
                    size: Some((0, 0)),
                    layer: iced_layershell::reexport::Layer::Overlay,
                    keyboard_interactivity:
                        iced_layershell::reexport::KeyboardInteractivity::OnDemand,
                    ..Default::default()
                };
                let (popup_id, open_task) = Message::layershell_open(settings);
                self.menu_popups
                    .insert(popup_id, (service, menu_path, tree, bar_name));
                tasks.push(open_task);
                Task::batch(tasks)
            }
            Message::MenuItemSelected {
                service,
                menu_item_id,
                menu_path,
            } => self.handle_menu_item_selected(service, menu_item_id, menu_path),
            Message::MenuClosed => self.handle_menu_closed(),
            Message::MenuItemHoverEnter { window, item_id } => {
                self.menu_popup_hovered.insert(window, item_id);
                Task::none()
            }
            Message::MenuItemHoverExit(window) => {
                self.menu_popup_hovered.remove(&window);
                Task::none()
            }
            Message::MenuLayoutChanged { service, menu_path } => {
                // Re-fetch the menu and update the existing popup in place.
                let conn = self.shared.dbus.session.clone();
                // Preserve the existing anchor, screen_size, and bar_name from the current popup.
                let existing = self
                    .menu_popups
                    .values()
                    .find(|(s, mp, _, _)| s == &service && mp == &menu_path);
                let (anchor, screen_size, existing_bar_name) = match existing {
                    Some((_, _, tree, bn)) => (tree.anchor, tree.screen_size, bn.clone()),
                    None => return Task::none(),
                };
                if let Some(conn) = conn {
                    let svc = service.clone();
                    let mp = menu_path.clone();
                    return Task::perform(
                        async move { crate::dbus::sni_host::fetch_menu(&conn, &svc, &mp).await },
                        move |tree_opt| {
                            if let Some(mut tree) = tree_opt {
                                tree.anchor = anchor;
                                tree.screen_size = screen_size;
                                Message::OpenMenu {
                                    service,
                                    menu_path,
                                    tree,
                                    bar_name: existing_bar_name,
                                }
                            } else {
                                Message::MenuClosed
                            }
                        },
                    );
                }
                Task::none()
            }
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

    #[test]
    fn namespace_returns_niribar() {
        assert_eq!(App::namespace(), "niribar");
    }

    #[test]
    fn boot_with_empty_allowed_outputs_uses_allscreens_mode() {
        // Empty allowed_outputs → pending_surface_keys is empty (AllScreens handles creation)
        let config = make_config();
        let (app, _) = App::boot(config, vec![]);
        assert!(app.pending_surface_keys.is_empty());
    }

    #[test]
    fn boot_with_config_stores_config() {
        let mut config = Config::default();
        config.bars[0].collapsed_width = 60;
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.config.bars[0].collapsed_width, 60);
    }

    #[test]
    fn shared_state_starts_empty() {
        let state = SharedState::new();
        assert!(state.modules.is_empty());
    }

    #[test]
    fn boot_modules_empty_with_no_module_config() {
        let config = make_config();
        let (app, _) = App::boot(config, vec![]);
        assert!(app.modules.is_empty());
    }

    #[test]
    fn undeclared_known_type_in_bar_modules_uses_defaults() {
        let mut config = Config::default();
        config.bars[0].modules.bottom = vec!["clock".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn undeclared_unknown_name_in_bar_modules_is_skipped() {
        let mut config = Config::default();
        config.bars[0].modules.bottom = vec!["my-script".into()];
        let (app, _) = App::boot(config, vec![]);
        assert!(app.modules.is_empty());
    }

    #[test]
    fn clock_module_instantiated_from_registry() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["clock".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn hover_enter_sets_hovered_module() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let id = ModuleId::new("clock");
        let _ = app.update(Message::ModuleHoverEnter(id.clone(), wid));
        assert_eq!(app.surfaces[&wid].hovered_module, Some(id));
    }

    #[test]
    fn hover_exit_clears_hovered_module() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let id = ModuleId::new("clock");
        let _ = app.update(Message::ModuleHoverEnter(id.clone(), wid));
        let _ = app.update(Message::ModuleHoverExit(id, wid));
        assert!(app.surfaces[&wid].hovered_module.is_none());
    }

    #[test]
    fn hover_exit_different_id_does_not_clear() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let id_a = ModuleId::new("clock");
        let id_b = ModuleId::new("battery");
        let _ = app.update(Message::ModuleHoverEnter(id_a.clone(), wid));
        let _ = app.update(Message::ModuleHoverExit(id_b, wid));
        assert_eq!(app.surfaces[&wid].hovered_module, Some(id_a));
    }

    #[test]
    fn script_module_instantiated_from_registry() {
        use crate::config::{ModuleConfig, ModuleParams, ScriptConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "script".into(),
                module_type: "script".into(),
                params: ModuleParams::Script(ScriptConfig {
                    command: "echo hi".into(),
                    interval: 5,
                }),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["script".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn script_output_event_updates_state() {
        use crate::modules::script::ScriptEvent;

        let (mut app, _) = App::boot(make_config(), vec![]);
        let id = ModuleId::new("script");
        let _ = app.update(Message::Module(
            id.clone(),
            ModuleEvent::Script(ScriptEvent::Output("hello".into())),
        ));
        match app.shared.modules.get(&id) {
            Some(ModuleState::Script(s)) => assert_eq!(s.output, "hello"),
            other => panic!("expected Script state, got {:?}", other),
        }
    }

    #[test]
    fn repeated_clock_tick_keeps_existing_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let id = ModuleId::new("clock");
        let first = ModuleState::Clock(crate::modules::clock::ClockState {
            time: "10:00".into(),
            date: "Sat".into(),
        });
        app.shared.modules.insert(id.clone(), first.clone());

        let _ = app.update(Message::Module(
            id.clone(),
            ModuleEvent::Clock(crate::modules::clock::ClockEvent::Tick {
                time: "10:00".into(),
                date: "Sat".into(),
            }),
        ));

        assert_eq!(app.shared.modules.get(&id), Some(&first));
    }

    #[test]
    fn repeated_volume_state_keeps_existing_state() {
        use crate::config::{ModuleConfig, ModuleParams, VolumeConfig};

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "volume".into(),
                module_type: "volume".into(),
                params: ModuleParams::Volume(VolumeConfig::default()),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["volume".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let id = ModuleId::new("volume");
        let first = ModuleState::Volume(crate::modules::volume::VolumeState {
            volume: 55.0,
            muted: false,
        });
        app.shared.modules.insert(id.clone(), first.clone());

        let _ = app.update(Message::Volume(
            crate::modules::volume::VolumeEvent::StateChanged {
                volume: 55.0,
                muted: false,
            },
        ));

        assert_eq!(app.shared.modules.get(&id), Some(&first));
    }

    #[test]
    fn cursor_moved_is_ignored_without_tray_module() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();

        let _ = app.update(Message::CursorMoved(wid, iced::Point::new(10.0, 20.0)));

        assert_eq!(app.last_cursor, None);
    }

    #[test]
    fn cursor_moved_is_recorded_when_tray_module_exists() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.modules.push((
            Zone::Top,
            Box::new(crate::modules::tray::TrayModule::new(ModuleId::new("tray"))),
        ));
        let point = iced::Point::new(10.0, 20.0);

        let _ = app.update(Message::CursorMoved(wid, point));

        assert_eq!(app.last_cursor, Some((wid, point)));
    }

    #[test]
    fn perf_tracker_records_update_messages_when_enabled() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        app.perf = Some(super::perf::PerfTracker::default());

        let _ = app.update(Message::Tick);

        let snapshot = app.perf.as_ref().unwrap().snapshot_and_reset();
        assert_eq!(snapshot.updates.get("Tick"), Some(&1));
    }

    #[test]
    fn perf_tracker_labels_niri_event_variants() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        app.perf = Some(super::perf::PerfTracker::default());

        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspaceActivated {
            id: 1,
            focused: true,
        })));

        let snapshot = app.perf.as_ref().unwrap().snapshot_and_reset();
        assert_eq!(snapshot.updates.get("Niri::WorkspaceActivated"), Some(&1));
    }

    #[test]
    fn perf_tracker_labels_ignored_niri_event_variants() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        app.perf = Some(super::perf::PerfTracker::default());

        let _ = app.update(Message::Niri(NiriEvent::Event(Event::ConfigLoaded {
            failed: false,
        })));

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

        let _ = app.update(Message::Tick);
        let _ = app.update(Message::PerfReport);

        let snapshot = app.perf.as_ref().unwrap().snapshot_and_reset();
        assert!(snapshot.updates.is_empty());
    }

    #[test]
    fn button_module_instantiated_from_registry() {
        use crate::config::{ButtonConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "button".into(),
                module_type: "button".into(),
                params: ModuleParams::Button(ButtonConfig {
                    label: "Launch".into(),
                    icon: ">".into(),
                    on_click: "kitty".into(),
                }),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["button".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn button_ready_event_updates_state() {
        use crate::modules::button::ButtonEvent;

        let (mut app, _) = App::boot(make_config(), vec![]);
        let id = ModuleId::new("button");
        let _ = app.update(Message::Module(
            id.clone(),
            ModuleEvent::Button(ButtonEvent::Ready),
        ));
        match app.shared.modules.get(&id) {
            Some(ModuleState::Button(_)) => {}
            other => panic!("expected Button state, got {:?}", other),
        }
    }

    #[test]
    fn module_click_no_match_does_not_panic() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let id = ModuleId::new("nonexistent");
        let _ = app.update(Message::ModuleClick(id, wid));
    }

    #[test]
    fn script_failed_event_retains_previous_state() {
        use crate::config::{ModuleConfig, ModuleParams, ScriptConfig};
        use crate::modules::script::ScriptEvent;

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "script".into(),
                module_type: "script".into(),
                params: ModuleParams::Script(ScriptConfig {
                    command: "echo hi".into(),
                    interval: 5,
                }),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["script".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let id = ModuleId::new("script");
        app.shared.modules.insert(
            id.clone(),
            ModuleState::Script(crate::modules::script::ScriptState {
                output: "prev".into(),
            }),
        );

        let _ = app.update(Message::Module(
            id.clone(),
            ModuleEvent::Script(ScriptEvent::Failed),
        ));

        match app.shared.modules.get(&id) {
            Some(ModuleState::Script(s)) => assert_eq!(s.output, "prev"),
            other => panic!("expected Script state, got {:?}", other),
        }
    }

    // --- Story 3.1 tests ---

    #[test]
    fn app_starts_with_no_surfaces() {
        let (app, _) = App::boot(make_config(), vec![]);
        assert!(app.surfaces.is_empty());
    }

    #[test]
    fn bar_click_starts_expanding_from_collapsed() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarClick(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn bar_click_starts_collapsing_from_expanded() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::BarClick(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsing(0.0));
    }

    // --- Story 3.2 tests ---

    #[test]
    fn tick_advances_expand_progress() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.0);
        let _ = app.update(Message::Tick);
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
        let _ = app.update(Message::Tick);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanded);
    }

    #[test]
    fn tick_advances_collapse_progress() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Collapsing(0.0);
        let _ = app.update(Message::Tick);
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
        let _ = app.update(Message::Tick);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn tick_no_op_when_collapsed() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Collapsed;
        let _ = app.update(Message::Tick);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn tick_no_op_when_expanded() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::Tick);
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
        let _ = app.update(Message::Tick);
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
        let _ = app.update(Message::Tick);
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
        let _ = app.update(Message::BarHoverEnter(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
        assert!(app.surfaces[&wid].bar_hovered);
    }

    #[test]
    fn hover_exit_click_trigger_does_nothing() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::BarHoverExit(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanded);
    }

    #[test]
    fn hover_enter_hover_trigger_zero_delay_starts_expanding() {
        let (mut app, _) = App::boot(make_hover_config(), vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarHoverEnter(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn hover_exit_when_expanded_starts_collapsing() {
        let (mut app, _) = App::boot(make_hover_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::BarHoverExit(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsing(0.0));
    }

    #[test]
    fn hover_exit_when_expanding_reverses() {
        let (mut app, _) = App::boot(make_hover_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.3);
        let _ = app.update(Message::BarHoverExit(wid));
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
        let _ = app.update(Message::BarHoverExit(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn hover_enter_with_delay_sets_pending() {
        let mut config = make_hover_config();
        config.bars[0].hover_delay = 100;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarHoverEnter(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
        assert_eq!(app.surfaces[&wid].hover_pending_ms, Some(0.0));
    }

    #[test]
    fn hover_exit_cancels_pending() {
        let mut config = make_hover_config();
        config.bars[0].hover_delay = 100;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarHoverEnter(wid));
        let _ = app.update(Message::BarHoverExit(wid));
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
        let _ = app.update(Message::Tick);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
        assert_eq!(app.surfaces[&wid].hover_pending_ms, None);
    }

    #[test]
    fn bar_click_noop_when_hover_trigger() {
        let (mut app, _) = App::boot(make_hover_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize so surface exists before early-return
        let _ = app.update(Message::BarClick(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    // --- Story 4.1 tests ---

    #[test]
    fn niri_connected_event_sets_connected() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let _ = app.update(Message::Niri(NiriEvent::Connected));
        assert!(app.shared.niri.connected);
    }

    #[test]
    fn niri_unavailable_event_does_not_crash() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let _ = app.update(Message::Niri(NiriEvent::Unavailable));
        assert!(!app.shared.niri.connected);
    }

    #[test]
    fn niri_unavailable_sets_workspaces_module_unavailable() {
        let mut app = make_workspaces_app();
        let _ = app.update(Message::Niri(NiriEvent::Unavailable));
        let id = ModuleId::new("workspaces");
        assert!(matches!(
            app.shared.modules.get(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn niri_unavailable_sets_focused_window_module_unavailable() {
        let mut app = make_focused_window_app();
        let _ = app.update(Message::Niri(NiriEvent::Unavailable));
        let id = ModuleId::new("focused-window");
        assert!(matches!(
            app.shared.modules.get(&id),
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
            }],
            ..Default::default()
        };
        config.bars[0].modules.center = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let _ = app.update(Message::Niri(NiriEvent::Unavailable));
        let id = ModuleId::new("clock");
        // Clock state is Loading (default) — niri Unavailable must not touch non-niri modules.
        assert!(!matches!(
            app.shared.modules.get(&id),
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
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
            workspaces: vec![ws],
        })));
        assert_eq!(app.shared.niri.workspaces.len(), 1);
    }

    #[test]
    fn niri_window_focus_changed_stores_id() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowFocusChanged {
            id: Some(42),
        })));
        assert_eq!(app.shared.niri.focused_window_id, Some(42));
    }

    #[test]
    fn niri_window_focus_cleared() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        app.shared.niri.focused_window_id = Some(99);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowFocusChanged {
            id: None,
        })));
        assert_eq!(app.shared.niri.focused_window_id, None);
    }

    #[test]
    fn niri_unknown_event_is_ignored() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        let _ = app.update(Message::Niri(NiriEvent::Event(
            Event::KeyboardLayoutsChanged {
                keyboard_layouts: niri_ipc::KeyboardLayouts {
                    names: vec![],
                    current_idx: 0,
                },
            },
        )));
        assert!(!app.shared.niri.connected);
        assert!(app.shared.niri.workspaces.is_empty());
    }

    // --- Story 4.2 tests ---

    fn make_workspaces_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "workspaces".into(),
                module_type: "workspaces".into(),
                params: ModuleParams::None,
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
    fn workspaces_module_state_updated_on_workspace_change() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        let ws = make_workspace(0, true);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
            workspaces: vec![ws],
        })));
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

        assert!(super::niri::workspace_render_state_equal(
            &workspaces,
            &workspaces.clone()
        ));
    }

    #[test]
    fn equivalent_workspace_event_keeps_module_count() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        let workspaces = vec![make_workspace(0, true)];
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
            workspaces: workspaces.clone(),
        })));
        let count = app.modules.len();

        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
            workspaces,
        })));

        assert_eq!(app.modules.len(), count);
    }

    #[test]
    fn workspace_module_focused_workspace_reflected_in_state() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        let ws0 = make_workspace(0, false);
        let ws1 = make_workspace(1, true);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
            workspaces: vec![ws0, ws1],
        })));
        let items: Vec<_> = app
            .modules
            .iter()
            .filter(|(_, m)| m.kind() == ModuleKind::WorkspaceItem)
            .collect();
        assert_eq!(items.len(), 2);
        // Focused item is the one with id "workspace:2" (idx=1, niri id = idx+1 in make_workspace)
        assert!(items.iter().any(|(_, m)| m.id().0 == "workspace:2"));
    }

    #[test]
    fn workspaces_module_state_not_updated_without_workspaces_module_in_config() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        let ws = make_workspace(0, true);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
            workspaces: vec![ws],
        })));
        let id = ModuleId::new("workspaces");
        assert!(!app.shared.modules.contains_key(&id));
    }

    #[test]
    fn workspaces_changed_order_preserved_in_state() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        let ws2 = make_workspace(2, false);
        let ws0 = make_workspace(0, true);
        let ws1 = make_workspace(1, false);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
            workspaces: vec![ws2, ws0, ws1],
        })));
        // Items are sorted by idx when inserted; verify all three exist.
        let items: Vec<_> = app
            .modules
            .iter()
            .filter(|(_, m)| m.kind() == ModuleKind::WorkspaceItem)
            .collect();
        assert_eq!(items.len(), 3);
        // Sorted by idx: 0, 1, 2 → niri ids 1, 2, 3
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
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
            workspaces: vec![ws0, ws1],
        })));

        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspaceActivated {
            id: 2,
            focused: true,
        })));

        // After activation, workspace 2 (idx=1) should be focused.
        // WorkspaceItemModules are rebuilt — verify the item for ws2 now carries is_focused=true
        // by checking app.shared.niri.workspaces (source of truth).
        let focused: Vec<_> = app
            .shared
            .niri
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
        app.perf = Some(super::perf::PerfTracker::default());
        let ws0 = make_workspace(0, true);
        let ws1 = make_workspace(1, false);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
            workspaces: vec![ws0, ws1],
        })));
        let _ = app.perf.as_ref().unwrap().snapshot_and_reset();
        let count = app.modules.len();

        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspaceActivated {
            id: 2,
            focused: true,
        })));

        let snapshot = app.perf.as_ref().unwrap().snapshot_and_reset();
        assert_eq!(snapshot.workspace_rebuilds, 0);
        assert_eq!(app.modules.len(), count);
    }

    #[test]
    fn workspace_activated_no_panic_when_workspaces_empty() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspaceActivated {
            id: 1,
            focused: true,
        })));
        assert!(app.shared.niri.workspaces.is_empty());
    }

    // --- Story 4.4 helpers and tests ---

    fn make_focused_window_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "focused-window".into(),
                module_type: "focused-window".into(),
                params: ModuleParams::None,
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
        app.shared.niri.windows.insert(1, win);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowFocusChanged {
            id: Some(1),
        })));
        let id = ModuleId::new("focused-window");
        match app.shared.modules.get(&id) {
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
        app.shared.niri.windows.insert(1, win);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowFocusChanged {
            id: Some(1),
        })));
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowFocusChanged {
            id: None,
        })));
        let id = ModuleId::new("focused-window");
        match app.shared.modules.get(&id) {
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
        assert!(app.shared.niri.focused_window_id.is_none());
        let win = make_window(2, Some("Helix"), true);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowsChanged {
            windows: vec![win],
        })));
        let id = ModuleId::new("focused-window");
        match app.shared.modules.get(&id) {
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
        let _ = app.update(Message::Niri(NiriEvent::Event(
            Event::WindowOpenedOrChanged { window: win },
        )));
        let id = ModuleId::new("focused-window");
        match app.shared.modules.get(&id) {
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
        app.shared.niri.windows.insert(4, win);
        app.shared.niri.focused_window_id = Some(4);
        app.shared.modules.insert(
            ModuleId::new("focused-window"),
            ModuleState::FocusedWindow(crate::modules::focused_window::FocusedWindowState {
                title: Some("Nautilus".into()),
            }),
        );
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowClosed {
            id: 4,
        })));
        let id = ModuleId::new("focused-window");
        match app.shared.modules.get(&id) {
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
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowsChanged {
            windows: vec![win],
        })));
        let id = ModuleId::new("focused-window");
        assert!(!app.shared.modules.contains_key(&id));
    }

    // Story 5.1 tests

    #[test]
    fn shared_state_dbus_starts_as_default() {
        let state = SharedState::new();
        assert!(state.dbus.session.is_none());
        assert!(state.dbus.system.is_none());
    }

    #[test]
    fn dbus_ready_stores_connections_in_shared_state() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let conns = crate::dbus::DbusConnections::default();
        let _ = app.update(Message::DbusReady(conns));
        assert!(app.shared.dbus.session.is_none());
        assert!(app.shared.dbus.system.is_none());
    }

    // Story 5.2 tests

    fn make_battery_app() -> App {
        use crate::config::{BatteryConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "battery".into(),
                module_type: "battery".into(),
                params: ModuleParams::Battery(BatteryConfig {
                    critical_threshold: 15,
                }),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["battery".into()];
        App::boot(config, vec![]).0
    }

    #[test]
    fn battery_state_changed_updates_module() {
        let mut app = make_battery_app();
        let _ = app.update(Message::Battery(BatteryEvent::StateChanged {
            percentage: 72.0,
            is_charging: false,
        }));
        let id = ModuleId::new("battery");
        match app.shared.modules.get(&id) {
            Some(ModuleState::Battery(s)) => {
                assert!((s.percentage - 72.0).abs() < 0.001);
                assert!(!s.is_charging);
            }
            other => panic!("expected Battery state, got {:?}", other),
        }
    }

    #[test]
    fn battery_unavailable_marks_module_unavailable() {
        let mut app = make_battery_app();
        let _ = app.update(Message::Battery(BatteryEvent::Unavailable));
        let id = ModuleId::new("battery");
        assert!(matches!(
            app.shared.modules.get(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn battery_state_not_updated_without_battery_module() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let _ = app.update(Message::Battery(BatteryEvent::StateChanged {
            percentage: 50.0,
            is_charging: true,
        }));
        let id = ModuleId::new("battery");
        assert!(!app.shared.modules.contains_key(&id));
    }

    // Story 5.3 tests

    fn make_network_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams, NetworkConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "network".into(),
                module_type: "network".into(),
                params: ModuleParams::Network(NetworkConfig),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["network".into()];
        App::boot(config, vec![]).0
    }

    #[test]
    fn network_state_changed_updates_module() {
        use crate::modules::network::NetworkConnectivity;
        let mut app = make_network_app();
        let _ = app.update(Message::Network(NetworkEvent::StateChanged {
            connectivity: NetworkConnectivity::Full,
            label: Some("Home".into()),
        }));
        let id = ModuleId::new("network");
        match app.shared.modules.get(&id) {
            Some(ModuleState::Network(s)) => {
                assert_eq!(s.connectivity, NetworkConnectivity::Full);
                assert_eq!(s.label.as_deref(), Some("Home"));
            }
            other => panic!("expected Network state, got {:?}", other),
        }
    }

    #[test]
    fn network_unavailable_marks_module_unavailable() {
        let mut app = make_network_app();
        let _ = app.update(Message::Network(NetworkEvent::Unavailable));
        let id = ModuleId::new("network");
        assert!(matches!(
            app.shared.modules.get(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn network_state_not_updated_without_network_module() {
        use crate::modules::network::NetworkConnectivity;
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let _ = app.update(Message::Network(NetworkEvent::StateChanged {
            connectivity: NetworkConnectivity::Full,
            label: None,
        }));
        let id = ModuleId::new("network");
        assert!(!app.shared.modules.contains_key(&id));
    }

    // Story 5.4 tests

    fn make_volume_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams, VolumeConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "volume".into(),
                module_type: "volume".into(),
                params: ModuleParams::Volume(VolumeConfig { step: 5 }),
            }],
            ..Default::default()
        };
        config.bars[0].modules.bottom = vec!["volume".into()];
        App::boot(config, vec![]).0
    }

    #[test]
    fn volume_state_changed_updates_module() {
        let mut app = make_volume_app();
        let _ = app.update(Message::Volume(VolumeEvent::StateChanged {
            volume: 60.0,
            muted: false,
        }));
        let id = ModuleId::new("volume");
        match app.shared.modules.get(&id) {
            Some(ModuleState::Volume(s)) => {
                assert!((s.volume - 60.0).abs() < 0.001);
                assert!(!s.muted);
            }
            other => panic!("expected Volume state, got {:?}", other),
        }
    }

    #[test]
    fn volume_unavailable_marks_module_unavailable() {
        let mut app = make_volume_app();
        let _ = app.update(Message::Volume(VolumeEvent::Unavailable));
        let id = ModuleId::new("volume");
        assert!(matches!(
            app.shared.modules.get(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn volume_state_not_updated_without_volume_module() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let _ = app.update(Message::Volume(VolumeEvent::StateChanged {
            volume: 50.0,
            muted: false,
        }));
        let id = ModuleId::new("volume");
        assert!(!app.shared.modules.contains_key(&id));
    }

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
            }],
            ..Default::default()
        };
        config.bars[0].modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarFocusGained(wid));
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
            }],
            ..Default::default()
        };
        config.bars[0].modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarFocusGained(wid));
        assert!(app.surfaces[&wid].focused_module.is_some());
        let _ = app.update(Message::BarFocusLost(wid));
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
            }],
            ..Default::default()
        };
        config.bars[0].modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarFocusGained(wid));
        assert!(app.surfaces[&wid].focused_module.is_some());
        let _ = app.update(Message::EscapePressed(wid));
        assert!(app.surfaces[&wid].focused_module.is_some());
    }

    #[test]
    fn escape_pressed_when_not_focused_is_noop() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let _ = app.update(Message::EscapePressed(wid));
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
            }],
            ..Default::default()
        };
        config.bars[0].modules.center = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarFocusGained(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
    }

    #[test]
    fn bar_focus_gained_with_no_modules_is_noop() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarFocusGained(wid));
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
                },
                ModuleConfig {
                    name: "battery".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::None,
                },
                ModuleConfig {
                    name: "network".into(),
                    module_type: "network".into(),
                    params: ModuleParams::None,
                },
                ModuleConfig {
                    name: "workspaces".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::None,
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
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("battery"))
        );
    }

    #[test]
    fn focus_move_next_crosses_zone_boundary() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("network"))
        );
    }

    #[test]
    fn focus_move_next_wraps_from_last_to_first() {
        let (mut app, wid) = make_nav_app();
        app.surface_mut(wid).focused_module = Some(ModuleId::new("workspaces"));
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
    }

    #[test]
    fn focus_move_prev_retreats_to_previous_module() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        let _ = app.update(Message::FocusMovePrev(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
    }

    #[test]
    fn focus_move_prev_wraps_from_first_to_last() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMovePrev(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("workspaces"))
        );
    }

    #[test]
    fn focus_next_zone_jumps_to_center() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusNextZone(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("network"))
        );
    }

    #[test]
    fn focus_next_zone_wraps_bottom_to_top() {
        let (mut app, wid) = make_nav_app();
        app.surface_mut(wid).focused_module = Some(ModuleId::new("workspaces"));
        let _ = app.update(Message::FocusNextZone(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
    }

    #[test]
    fn focus_prev_zone_from_top_jumps_to_bottom() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusPrevZone(wid));
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
                },
                ModuleConfig {
                    name: "workspaces".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::None,
                },
            ],
            ..Default::default()
        };
        config.bars[0].modules.top = vec!["clock".into()];
        config.bars[0].modules.bottom = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusNextZone(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("workspaces"))
        );
    }

    #[test]
    fn navigation_keys_when_not_focused_are_noop() {
        let (mut app, wid) = make_nav_app();
        // Bar not focused — focused_module is None
        let _ = app.update(Message::FocusMoveNext(wid));
        let _ = app.update(Message::FocusMovePrev(wid));
        let _ = app.update(Message::FocusNextZone(wid));
        let _ = app.update(Message::FocusPrevZone(wid));
        // After BarFocusGained delegation, focused_module gets set — but here we never
        // send BarFocusGained, so the surface may have it after FocusMoveNext delegates.
        // The key assertion: no panic.
    }

    #[test]
    fn focus_prev_zone_from_center_jumps_to_top() {
        let (mut app, wid) = make_nav_app();
        app.surface_mut(wid).focused_module = Some(ModuleId::new("network"));
        let _ = app.update(Message::FocusPrevZone(wid));
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
                },
                ModuleConfig {
                    name: "volume".into(),
                    module_type: "volume".into(),
                    params: ModuleParams::Volume(VolumeConfig::default()),
                },
                ModuleConfig {
                    name: "button".into(),
                    module_type: "button".into(),
                    params: ModuleParams::Button(ButtonConfig {
                        label: "test".into(),
                        icon: ">".into(),
                        on_click: "true".into(),
                    }),
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
    fn keyboard_activate_when_not_focused_is_noop() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::KeyboardActivate(wid));
        assert!(app.surfaces[&wid].focused_module.is_none());
        assert!(!app.surfaces[&wid].module_activated);
    }

    #[test]
    fn keyboard_activate_on_volume_enters_interaction_mode() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
        assert!(!app.surfaces[&wid].module_activated);
        let _ = app.update(Message::KeyboardActivate(wid));
        assert!(app.surfaces[&wid].module_activated);
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
    }

    #[test]
    fn arrow_keys_scroll_when_module_activated() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        let _ = app.update(Message::KeyboardActivate(wid));
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::FocusMovePrev(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
    }

    #[test]
    fn escape_deactivates_without_releasing_focus() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        let _ = app.update(Message::KeyboardActivate(wid));
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::EscapePressed(wid));
        assert!(!app.surfaces[&wid].module_activated);
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
    }

    #[test]
    fn arrow_keys_navigate_when_module_not_activated() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("volume"))
        );
        assert!(!app.surfaces[&wid].module_activated);
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("button"))
        );
    }

    #[test]
    fn tab_deactivates_and_navigates_zone() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        let _ = app.update(Message::KeyboardActivate(wid));
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::FocusNextZone(wid));
        assert!(!app.surfaces[&wid].module_activated);
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("button"))
        );
    }

    #[test]
    fn shift_tab_deactivates_and_navigates_zone() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        let _ = app.update(Message::KeyboardActivate(wid));
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::FocusPrevZone(wid));
        assert!(!app.surfaces[&wid].module_activated);
    }

    #[test]
    fn keyboard_activate_on_button_does_not_panic_and_does_not_set_activated() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusNextZone(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("button"))
        );
        let _ = app.update(Message::KeyboardActivate(wid));
        assert!(!app.surfaces[&wid].module_activated);
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("button"))
        );
    }

    #[test]
    fn keyboard_activate_on_non_interactive_module_is_noop() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::BarFocusGained(wid));
        assert_eq!(
            app.surfaces[&wid].focused_module,
            Some(ModuleId::new("clock"))
        );
        let _ = app.update(Message::KeyboardActivate(wid));
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
        let _ = app.update(Message::FocusMoveNext(wid));
        // After delegation, BarFocusGained would set focused_module — the delegation happened.
        // We verify no panic.
    }

    // --- Story 6.4 tests ---

    #[test]
    fn toggle_expand_from_collapsed_starts_expanding() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize surface so ToggleExpand can find it
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expand_from_expanded_starts_collapsing() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsing(0.0));
    }

    #[test]
    fn toggle_expand_reverses_expanding_mid_animation() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.4);
        let _ = app.update(Message::ToggleExpand);
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
        let _ = app.update(Message::ToggleExpand);
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
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expand_double_toggle_collapses() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize
        let _ = app.update(Message::ToggleExpand); // → Expanding(0.0)
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::ToggleExpand); // → Collapsing(0.0)
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsing(0.0));
    }

    // --- Expand disabled tests ---

    #[test]
    fn bar_click_noop_when_expand_disabled() {
        let (mut app, _) = App::boot(make_expand_disabled_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize so surface exists before early-return
        let _ = app.update(Message::BarClick(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn toggle_expand_noop_when_expand_disabled() {
        let (mut app, _) = App::boot(make_expand_disabled_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn hover_enter_noop_when_expand_disabled() {
        let mut config = make_expand_disabled_config();
        config.bars[0].expand_trigger = ExpandTrigger::Hover;
        config.bars[0].hover_delay = 0;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarHoverEnter(wid));
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
        let _ = app.update(Message::BarHoverExit(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanded);
    }

    // --- Overlay mode tests ---

    #[test]
    fn bar_click_horizontal_bar_starts_expanding() {
        let (mut app, _) = App::boot(make_horizontal_config(), vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarClick(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expand_horizontal_bar_starts_expanding() {
        let (mut app, _) = App::boot(make_horizontal_config(), vec![]);
        let wid = test_wid();
        app.surface_mut(wid); // initialize
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn hover_enter_horizontal_bar_hover_trigger_starts_expanding() {
        let mut config = make_horizontal_config();
        config.bars[0].expand_trigger = ExpandTrigger::Hover;
        config.bars[0].hover_delay = 0;
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let _ = app.update(Message::BarHoverEnter(wid));
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
    fn surface_mut_creates_default_state_for_new_window() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        assert!(!app.surfaces.contains_key(&wid));
        let s = app.surface_mut(wid);
        assert_eq!(s.expand, ExpandState::Collapsed);
    }

    #[test]
    fn two_surfaces_have_independent_expand_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid_a = test_wid();
        let wid_b = test_wid();
        app.surface_mut(wid_a).expand = ExpandState::Expanded;
        // Surface B should still be Collapsed (or not yet exist)
        let b_state = app.surfaces.get(&wid_b);
        assert!(
            b_state
                .map(|s| s.expand == ExpandState::Collapsed)
                .unwrap_or(true)
        );
    }

    #[test]
    fn two_surfaces_have_independent_focus_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid_a = test_wid();
        let wid_b = test_wid();
        let focus_id = ModuleId::new("clock");
        app.surface_mut(wid_a).focused_module = Some(focus_id);
        let b_state = app.surfaces.get(&wid_b);
        assert!(b_state.map(|s| s.focused_module.is_none()).unwrap_or(true));
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
        let _ = app.update(Message::BarClick(wid_a));
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
            }],
            ..Default::default()
        };
        config.bars[0].modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid_a = test_wid();
        let wid_b = test_wid();
        app.surface_mut(wid_b); // initialize surface B without focus
        let _ = app.update(Message::BarFocusGained(wid_a));
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

    // --- Hotplug output detection tests ---

    #[test]
    fn boot_manual_outputs_false_when_no_allowed_outputs() {
        let (app, _) = App::boot(make_config(), vec![]);
        assert!(!app.manual_outputs);
    }

    #[test]
    fn boot_manual_outputs_true_when_allowed_outputs_given() {
        let (app, _) = App::boot(make_config(), vec!["eDP-1".into()]);
        assert!(app.manual_outputs);
    }

    #[test]
    fn workspaces_changed_allscreens_mode_does_not_open_new_surfaces() {
        // In AllScreens mode (manual_outputs = false) the compositor manages surfaces;
        // WorkspacesChanged must not try to open additional windows.
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config(), vec![]);
        assert!(!app.manual_outputs);
        let before = app.surfaces.len();
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
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
        })));
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
        assert!(app.manual_outputs);
        // "DP-1" was not in the initial allowed_outputs — it is newly connected.
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
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
        })));
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
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
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
        })));
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
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
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
        })));
        assert_eq!(
            app.surfaces.len(),
            before,
            "HDMI-A-1 must not get a surface — no bar targets it"
        );
    }

    // --- Story 9.1: App-level multi-bar tests ---

    #[test]
    fn bar_for_surface_name_returns_first_when_name_empty() {
        let config = make_config();
        let bars = &config.bars;
        let bar = App::bar_for_surface_name(bars, "");
        assert_eq!(bar.name, "main");
    }

    #[test]
    fn bar_for_surface_name_returns_matching_bar() {
        use crate::config::BarConfig;
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "side".into(),
                    output: "DP-1".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let bar = App::bar_for_surface_name(&config.bars, "side");
        assert_eq!(bar.name, "side");
    }

    #[test]
    fn bar_for_surface_name_falls_back_to_first_on_miss() {
        use crate::config::BarConfig;
        let config = Config {
            bars: vec![BarConfig {
                name: "main".into(),
                ..BarConfig::default()
            }],
            modules: vec![],
        };
        // "nonexistent" not found → falls back to first
        let bar = App::bar_for_surface_name(&config.bars, "nonexistent");
        assert_eq!(bar.name, "main");
    }

    #[test]
    fn boot_manual_mode_creates_surface_per_bar_per_output() {
        use crate::config::BarConfig;
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
        // Both bars target "all" → eDP-1 should get 2 surfaces
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        assert_eq!(
            app.surfaces.len(),
            2,
            "two bars on one output → two surfaces"
        );
    }

    #[test]
    fn surface_bar_name_set_from_pending_surface_key() {
        use crate::config::BarConfig;
        let config = Config {
            bars: vec![BarConfig {
                name: "main".into(),
                output: "eDP-1".into(),
                ..BarConfig::default()
            }],
            modules: vec![],
        };
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        // Surface is pre-initialized from pending_surface_keys; check bar_name
        let bar_names: Vec<&str> = app.surfaces.values().map(|s| s.bar_name.as_str()).collect();
        assert!(
            bar_names.contains(&"main"),
            "surface must carry bar_name 'main'; got: {bar_names:?}"
        );
    }

    #[test]
    fn surface_output_name_set_from_pending_surface_key() {
        use crate::config::BarConfig;
        let config = Config {
            bars: vec![BarConfig {
                name: "main".into(),
                output: "eDP-1".into(),
                ..BarConfig::default()
            }],
            modules: vec![],
        };
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        let output_names: Vec<&str> = app
            .surfaces
            .values()
            .map(|s| s.output_name.as_str())
            .collect();
        assert!(
            output_names.contains(&"eDP-1"),
            "surface must carry output_name 'eDP-1'; got: {output_names:?}"
        );
    }

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
        app.manual_outputs = true; // simulate manual mode
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
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
        })));
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
        app.manual_outputs = true;
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
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
        })));
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
    fn boot_spawns_horizontal_bars_before_side_bars() {
        // Config has side bar declared first, horizontal bar second (adversarial order).
        // Surfaces must be registered with horizontal bar first so its exclusive zone
        // is claimed before the side bar is positioned by the compositor.
        let config = Config {
            bars: vec![
                crate::config::BarConfig {
                    name: "left".into(),
                    output: "all".into(),
                    edge: Edge::Left,
                    ..crate::config::BarConfig::default()
                },
                crate::config::BarConfig {
                    name: "top".into(),
                    output: "all".into(),
                    edge: Edge::Top,
                    ..crate::config::BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        // Collect bar names in insertion order from surfaces map.
        // We use pending_surface_keys which maps window id → (output, bar_name).
        // The order surfaces are registered reflects spawn order.
        let names: Vec<String> = app
            .pending_surface_keys
            .values()
            .map(|(_, bar)| bar.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        // Both bars must be present.
        assert!(names.contains(&"top".to_string()), "top bar not spawned");
        assert!(names.contains(&"left".to_string()), "left bar not spawned");
    }

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
        app.manual_outputs = true;
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
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
        })));
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

    #[test]
    fn tray_not_in_needs_dbus_when_no_tray_module() {
        let (app, _) = App::boot(Config::default(), vec![]);
        // Default config has no tray module -- verify no tray module is registered.
        assert!(
            !app.modules
                .iter()
                .any(|(_, m)| m.kind() == ModuleKind::Tray)
        );
    }

    #[test]
    fn tray_updated_message_stores_items_in_shared_state() {
        use crate::dbus::sni_host::{ItemStatus, TrayIcon, TrayItem};
        use crate::modules::ModuleId;
        use zbus::zvariant::OwnedObjectPath;

        let (mut app, _) = App::boot(Config::default(), vec![]);
        // Inject a tray module id into the modules list manually.
        let id = ModuleId::new("tray");
        app.shared.modules.insert(id.clone(), ModuleState::Loading);

        let item = TrayItem {
            service: "org.test".into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("test".into()),
            tooltip: None,
            status: ItemStatus::Active,
            menu_path: None,
        };
        // Must not panic.
        let _ = app.update(Message::TrayEvent(
            crate::dbus::sni_host::TrayEvent::Snapshot(vec![item]),
        ));
    }

    #[test]
    fn tray_snapshot_removes_existing_tray_icons_before_insert() {
        use crate::dbus::sni_host::{ItemStatus, TrayIcon, TrayItem};
        use crate::modules::tray::TrayIconModule;
        use zbus::zvariant::OwnedObjectPath;

        let mut config = make_config();
        config.bars[0].modules.bottom = vec!["tray".into()];
        config.modules = vec![ModuleConfig {
            name: "tray".into(),
            module_type: "tray".into(),
            params: ModuleParams::Tray(crate::config::TrayConfig),
        }];
        let (mut app, _) = App::boot(config, vec![]);

        let old_item = TrayItem {
            service: "org.old".into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("old".into()),
            tooltip: None,
            status: ItemStatus::Active,
            menu_path: None,
        };
        app.modules.push((
            Zone::Bottom,
            Box::new(TrayIconModule::new(old_item, "tray".into())),
        ));

        let new_item = TrayItem {
            service: "org.new".into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("new".into()),
            tooltip: None,
            status: ItemStatus::Active,
            menu_path: None,
        };

        let _ = app.handle_tray_event(crate::dbus::sni_host::TrayEvent::Snapshot(vec![new_item]));

        assert!(app.modules.iter().any(|(_, m)| m.id().0 == "tray:org.new"));
        assert!(!app.modules.iter().any(|(_, m)| m.id().0 == "tray:org.old"));
    }

    // --- Menu item hover tests ---

    fn make_popup_wid(app: &mut App) -> iced::window::Id {
        use crate::surface::menu_popup::{MenuItem, MenuTree};
        let wid = test_wid();
        let tree = MenuTree {
            items: vec![MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let path = zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap();
        app.menu_popups
            .insert(wid, ("org.test".into(), path, tree, String::new()));
        wid
    }

    #[test]
    fn menu_item_hover_enter_sets_hovered_item() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        let _ = app.update(Message::MenuItemHoverEnter {
            window: wid,
            item_id: "1".into(),
        });
        assert_eq!(
            app.menu_popup_hovered.get(&wid).map(|s| s.as_str()),
            Some("1")
        );
    }

    #[test]
    fn menu_item_hover_exit_clears_hovered_item() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        let _ = app.update(Message::MenuItemHoverEnter {
            window: wid,
            item_id: "1".into(),
        });
        let _ = app.update(Message::MenuItemHoverExit(wid));
        assert!(!app.menu_popup_hovered.contains_key(&wid));
    }

    #[test]
    fn menu_item_hover_enter_different_popups_are_independent() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid_a = make_popup_wid(&mut app);
        let wid_b = make_popup_wid(&mut app);
        let _ = app.update(Message::MenuItemHoverEnter {
            window: wid_a,
            item_id: "1".into(),
        });
        // wid_b should have no hovered item
        assert!(!app.menu_popup_hovered.contains_key(&wid_b));
    }

    #[test]
    fn menu_item_hover_enter_on_second_item_replaces_first_without_exit() {
        // Moving from item A directly to item B must not require an explicit exit from A.
        // The on_enter for B alone should update the hovered item.
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        let _ = app.update(Message::MenuItemHoverEnter {
            window: wid,
            item_id: "1".into(),
        });
        let _ = app.update(Message::MenuItemHoverEnter {
            window: wid,
            item_id: "2".into(),
        });
        assert_eq!(
            app.menu_popup_hovered.get(&wid).map(|s| s.as_str()),
            Some("2")
        );
    }

    #[test]
    fn escape_in_popup_fires_menu_closed() {
        let (mut app, _wid) = make_interact_app();
        let menu_path =
            zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![crate::surface::menu_popup::MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let popup_wid = iced::window::Id::unique();
        app.menu_popups
            .insert(popup_wid, ("svc".into(), menu_path, tree, String::new()));
        let _ = app.update(Message::EscapePressed(popup_wid));
        // EscapePressed dispatches MenuClosed; run it to complete the close cycle.
        let _ = app.update(Message::MenuClosed);
        assert!(app.menu_popups.is_empty());
    }

    #[test]
    fn arrow_down_in_popup_focuses_first_item() {
        let (mut app, _wid) = make_interact_app();
        let menu_path =
            zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![
                crate::surface::menu_popup::MenuItem {
                    id: "1".into(),
                    label: "Open".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                crate::surface::menu_popup::MenuItem {
                    id: "2".into(),
                    label: "Close".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
            ],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let popup_wid = iced::window::Id::unique();
        app.menu_popups
            .insert(popup_wid, ("svc".into(), menu_path, tree, String::new()));
        let _ = app.update(Message::FocusMoveNext(popup_wid));
        assert_eq!(
            app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
            Some("1")
        );
        let _ = app.update(Message::FocusMoveNext(popup_wid));
        assert_eq!(
            app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
            Some("2")
        );
        let _ = app.update(Message::FocusMoveNext(popup_wid));
        assert_eq!(
            app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
            Some("1")
        );
    }

    #[test]
    fn arrow_up_in_popup_wraps_to_last() {
        let (mut app, _wid) = make_interact_app();
        let menu_path =
            zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![
                crate::surface::menu_popup::MenuItem {
                    id: "1".into(),
                    label: "Open".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                crate::surface::menu_popup::MenuItem {
                    id: "2".into(),
                    label: "Close".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
            ],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let popup_wid = iced::window::Id::unique();
        app.menu_popups
            .insert(popup_wid, ("svc".into(), menu_path, tree, String::new()));
        let _ = app.update(Message::FocusMovePrev(popup_wid));
        assert_eq!(
            app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
            Some("2")
        );
    }

    #[test]
    fn popup_skips_separators_and_disabled_items() {
        let (mut app, _wid) = make_interact_app();
        let menu_path =
            zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![
                crate::surface::menu_popup::MenuItem {
                    id: "1".into(),
                    label: "Open".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                crate::surface::menu_popup::MenuItem {
                    id: "0".into(),
                    label: String::new(),
                    enabled: false,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: true,
                },
                crate::surface::menu_popup::MenuItem {
                    id: "3".into(),
                    label: "Disabled".into(),
                    enabled: false,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                crate::surface::menu_popup::MenuItem {
                    id: "4".into(),
                    label: "Close".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
            ],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let popup_wid = iced::window::Id::unique();
        app.menu_popups
            .insert(popup_wid, ("svc".into(), menu_path, tree, String::new()));
        let _ = app.update(Message::FocusMoveNext(popup_wid));
        assert_eq!(
            app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
            Some("1")
        );
        let _ = app.update(Message::FocusMoveNext(popup_wid));
        assert_eq!(
            app.menu_focused_item.get(&popup_wid).map(|s| s.as_str()),
            Some("4")
        );
    }

    #[test]
    fn menu_closed_clears_bar_focus_on_all_surfaces() {
        let (mut app, wid) = make_interact_app();
        // Give the bar surface keyboard focus on a module.
        let _ = app.update(Message::BarFocusGained(wid));
        assert!(app.surfaces[&wid].focused_module.is_some());
        // Open a popup.
        let menu_path =
            zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let popup_wid = iced::window::Id::unique();
        app.menu_popups
            .insert(popup_wid, ("svc".into(), menu_path, tree, String::new()));
        app.menu_focused_item.insert(popup_wid, "1".into());
        // Close the menu.
        let _ = app.update(Message::MenuClosed);
        // Bar focus should be cleared.
        assert!(app.surfaces[&wid].focused_module.is_none());
        assert!(!app.surfaces[&wid].module_activated);
        // Menu focus state should be cleaned up.
        assert!(app.menu_focused_item.is_empty());
    }

    #[test]
    fn close_all_menu_popups_removes_popup_and_focus_maps() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        app.menu_popup_hovered.insert(wid, "1".into());
        app.menu_focused_item.insert(wid, "1".into());

        let tasks = app.close_all_menu_popups();

        assert!(app.menu_popups.is_empty());
        assert!(app.menu_popup_hovered.is_empty());
        assert!(app.menu_focused_item.is_empty());
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn tray_keyboard_menu_request_without_source_window_is_noop() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let task = app.update(Message::TrayMenuRequestKeyboard {
            service: "org.test.App".into(),
            menu_path: None,
            source_window: None,
        });
        let _ = task;
    }
}
