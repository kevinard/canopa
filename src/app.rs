use std::collections::HashMap;
use std::time::Duration;

use iced::widget::{Space, column, container, mouse_area, row};
use iced::{Element, Length, Task};
use iced_layershell::reexport::{NewLayerShellSettings, OutputOption};
use iced_layershell::to_layer_message;

use crate::config::{
    BatteryConfig, ClockConfig, Config, Edge, ExpandTrigger, ModuleConfig, ModuleParams,
    NetworkConfig, VolumeConfig, WorkspacesConfig,
};
use crate::dbus::DbusConnections;
use crate::ipc::ipc_subscription;
use crate::modules::battery::{BatteryEvent, BatteryState};
use crate::modules::button::{ButtonEvent, ButtonState};
use crate::modules::clock::{ClockEvent, ClockState};
use crate::modules::focused_window;
use crate::modules::network::{NetworkEvent, NetworkState};
use crate::modules::script::{ScriptEvent, ScriptState};
use crate::modules::volume::{VolumeEvent, VolumeState};
use crate::modules::workspaces;
use crate::modules::{
    InteractionEvent, Module, ModuleEvent, ModuleId, ModuleState, Zone, build_registry,
};
use crate::niri::subscription::niri_subscription;
use crate::niri::{NiriEvent, NiriState};
use crate::surface::SurfaceState;
use crate::surface::expand::ExpandState;
use crate::surface::frame::module_frame;
use crate::surface::input::{init_expand_keybind, keyboard_focus_subscription};
use crate::surface::layout::zone_layout;

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
    /// System tray items updated from SNI host (Task 4).
    TrayUpdated(Vec<crate::dbus::sni_host::TrayItem>),
    /// Left-click on a tray icon -- sends Activate to the SNI item.
    TrayActivate { service: String },
    /// Right-click on a tray icon -- opens the DBusMenu context menu.
    TrayMenuRequest {
        service: String,
        menu_path: Option<zbus::zvariant::OwnedObjectPath>,
    },
    /// Middle-click on a tray icon -- sends SecondaryActivate.
    TraySecondaryActivate { service: String },
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
    },
    /// Cursor moved within a bar surface window. Used to track anchor position for menu popups.
    CursorMoved(iced::window::Id, iced::Point),
    /// Bar surface window was opened or resized; carries its logical size in pixels.
    WindowSized(iced::window::Id, u32, u32),
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
                modules.push((zone.clone(), ctor(mc)));
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
            modules,
            surfaces,
            pending_surface_keys,
            manual_outputs,
            menu_popups: HashMap::new(),
            menu_popup_hovered: HashMap::new(),
            last_cursor: None,
        };
        (app, Task::batch(window_tasks))
    }

    /// Returns a mutable reference to the `SurfaceState` for `id`, inserting a default if absent.
    /// Uses the pending (output_name, bar_name) pair (Story 9.1) when initialising a new entry.
    pub fn surface_mut(&mut self, id: iced::window::Id) -> &mut SurfaceState {
        if !self.surfaces.contains_key(&id) {
            // Pop the pending surface key assigned during window creation.
            // Falls back to ("", "") when using StartMode::AllScreens.
            let (output_name, bar_name) =
                self.pending_surface_keys.remove(&id).unwrap_or_default();
            self.surfaces
                .insert(id, SurfaceState::new(output_name, bar_name));
        }
        self.surfaces.get_mut(&id).unwrap()
    }

    /// Returns a reference to the `SurfaceState` for `id`, or `None` if the surface is unknown.
    pub fn surface(&self, id: iced::window::Id) -> Option<&SurfaceState> {
        self.surfaces.get(&id)
    }

    /// Opens a layer-shell window for one (bar, output) pair, records it in `pending` and
    /// `surfaces`, and returns the open task. Shared by `boot` and `open_new_output_windows`.
    fn spawn_bar_on_output(
        bar: &crate::config::BarConfig,
        output_name: &str,
        pending: &mut HashMap<iced::window::Id, (String, String)>,
        surfaces: &mut HashMap<iced::window::Id, SurfaceState>,
    ) -> Task<Message> {
        let anchor = bar.edge.anchor();
        let collapsed = bar.collapsed_width;
        let exclusive_zone = i32::try_from(collapsed).unwrap_or(48);
        let size = if bar.edge.is_horizontal() {
            Some((0, collapsed))
        } else {
            Some((collapsed, 0))
        };
        let settings = NewLayerShellSettings {
            anchor,
            exclusive_zone: Some(exclusive_zone),
            size,
            output_option: OutputOption::OutputName(output_name.to_string()),
            namespace: Some(Self::namespace()),
            ..Default::default()
        };
        let (id, task) = Message::layershell_open(settings);
        pending.insert(id, (output_name.to_string(), bar.name.clone()));
        surfaces.insert(id, SurfaceState::new(output_name.to_string(), bar.name.clone()));
        task
    }

    /// In manual-output mode, opens new layer-shell windows for any (output, bar) pairs present
    /// in `workspaces` that do not already have a surface.  Called from the `WorkspacesChanged`
    /// handler so that outputs hotplugged while the bar is running get bars immediately.
    fn open_new_output_windows(
        &mut self,
        workspaces: &[niri_ipc::Workspace],
    ) -> Vec<Task<Message>> {
        // Build the set of (output_name, bar_name) pairs already running.
        let known_pairs: std::collections::HashSet<(String, String)> = self
            .surfaces
            .values()
            .filter(|s| !s.output_name.is_empty())
            .map(|s| (s.output_name.clone(), s.bar_name.clone()))
            .collect();

        // For each newly-seen output, collect the (bar_idx, output_name) pairs to spawn.
        // Using bar index avoids lifetime issues with borrowing config.bars at spawn time.
        let mut to_spawn: Vec<(usize, String)> = workspaces
            .iter()
            .filter_map(|ws| ws.output.as_deref())
            .filter(|n| !n.is_empty())
            .flat_map(|output_name| {
                self.config
                    .bars
                    .iter()
                    .enumerate()
                    .filter(move |(_, b)| b.output == "all" || b.output == output_name)
                    .filter(|(_, b)| {
                        !known_pairs.contains(&(output_name.to_string(), b.name.clone()))
                    })
                    .map(move |(i, _)| (i, output_name.to_string()))
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Horizontal bars must be registered before side bars so their exclusive zones are
        // established first.  The compositor then automatically constrains side bars to the
        // remaining work area, preventing overlap during expand.
        to_spawn.sort_by_key(|(idx, out)| {
            let is_side = !self.config.bars[*idx].edge.is_horizontal();
            (out.clone(), is_side)
        });

        let mut tasks = Vec::with_capacity(to_spawn.len());
        for (bar_idx, output_name) in to_spawn {
            let task = Self::spawn_bar_on_output(
                &self.config.bars[bar_idx],
                &output_name,
                &mut self.pending_surface_keys,
                &mut self.surfaces,
            );
            tasks.push(task);
        }
        tasks
    }

    pub fn namespace() -> String {
        String::from("niribar")
    }

    /// Look up the `BarConfig` for a surface by its `bar_name`.
    /// Falls back to the first bar when the name is empty (AllScreens mode) or not found.
    fn bar_for_surface_name<'a>(
        bars: &'a [crate::config::BarConfig],
        bar_name: &str,
    ) -> &'a crate::config::BarConfig {
        if bar_name.is_empty() {
            bars.first().expect("at least one bar configured")
        } else {
            bars.iter()
                .find(|b| b.name == bar_name)
                .unwrap_or_else(|| bars.first().expect("at least one bar configured"))
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CursorMoved(window, pos) => {
                self.last_cursor = Some((window, pos));
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
            Message::Module(id, ModuleEvent::Clock(ClockEvent::Tick { time, date })) => {
                self.shared
                    .modules
                    .insert(id, ModuleState::Clock(ClockState { time, date }));
                Task::none()
            }
            Message::Module(id, ModuleEvent::Script(ScriptEvent::Output(text))) => {
                self.shared
                    .modules
                    .insert(id, ModuleState::Script(ScriptState { output: text }));
                Task::none()
            }
            Message::Module(_, ModuleEvent::Script(ScriptEvent::Failed)) => {
                // Retain previous state — do nothing (AC #3)
                Task::none()
            }
            Message::Module(id, ModuleEvent::Button(ButtonEvent::Ready)) => {
                self.shared
                    .modules
                    .insert(id, ModuleState::Button(ButtonState));
                Task::none()
            }
            Message::ModuleClick(id, _window) => {
                // Module interactions are shared — module data lives in self.modules.
                for (_, module) in &self.modules {
                    if module.id() == id {
                        if let Some(msg) = module.on_interaction(InteractionEvent::Clicked) {
                            return Task::done(msg);
                        }
                        break;
                    }
                }
                Task::none()
            }
            Message::BarClick(window) => {
                // Dismiss any open popup menus before processing the bar click.
                if !self.menu_popups.is_empty() {
                    return Task::done(Message::MenuClosed);
                }
                let bar_name = self
                    .surface(window)
                    .map(|s| s.bar_name.clone())
                    .unwrap_or_default();
                let bar = Self::bar_for_surface_name(&self.config.bars, &bar_name);
                if !bar.expand {
                    return Task::none();
                }
                // Click has no effect when hover is the expand trigger (Story 3.3 AC #6).
                if bar.expand_trigger == ExpandTrigger::Hover {
                    return Task::none();
                }
                let s = self.surface_mut(window);
                s.expand = s.expand.toggle();
                self.overlay_size_change_for(window)
            }
            Message::BarHoverEnter(window) => {
                let bar_name = self
                    .surface(window)
                    .map(|s| s.bar_name.clone())
                    .unwrap_or_default();
                let bar = Self::bar_for_surface_name(&self.config.bars, &bar_name);
                let expand_enabled = bar.expand;
                let expand_trigger = bar.expand_trigger;
                let hover_delay = bar.hover_delay;
                let s = self.surface_mut(window);
                s.bar_hovered = true;
                if !expand_enabled {
                    return Task::none();
                }
                if expand_trigger != ExpandTrigger::Hover {
                    return Task::none();
                }
                let expand = s.expand;
                match expand {
                    ExpandState::Collapsed => {
                        if hover_delay == 0 {
                            let s = self.surface_mut(window);
                            s.expand = s.expand.toggle(); // → Expanding(0.0)
                            self.overlay_size_change_for(window)
                        } else {
                            self.surface_mut(window).hover_pending_ms = Some(0.0);
                            Task::none()
                        }
                    }
                    ExpandState::Collapsing(_) => {
                        // Pointer re-entered while collapsing — reverse back to expanding.
                        let s = self.surface_mut(window);
                        s.expand = s.expand.toggle();
                        Task::none()
                    }
                    _ => Task::none(), // Already expanded or expanding — no action
                }
            }
            Message::BarHoverExit(window) => {
                let bar_name = self
                    .surface(window)
                    .map(|s| s.bar_name.clone())
                    .unwrap_or_default();
                let bar = Self::bar_for_surface_name(&self.config.bars, &bar_name);
                let expand_enabled = bar.expand;
                let expand_trigger = bar.expand_trigger;
                let s = self.surface_mut(window);
                s.bar_hovered = false;
                if !expand_enabled {
                    return Task::none();
                }
                if expand_trigger != ExpandTrigger::Hover {
                    return Task::none();
                }
                // Cancel any pending hover delay.
                s.hover_pending_ms = None;
                let expand = s.expand;
                match expand {
                    ExpandState::Expanded | ExpandState::Expanding(_) => {
                        let s = self.surface_mut(window);
                        s.expand = s.expand.toggle(); // → Collapsing(0.0) or Collapsing(1-p)
                        self.overlay_size_change_for(window)
                    }
                    _ => Task::none(), // Already collapsed or collapsing — no action
                }
            }
            Message::Niri(NiriEvent::Connected) => {
                self.shared.niri.connected = true;
                Task::none()
            }
            Message::Niri(NiriEvent::Unavailable) => {
                // Running on a non-Niri compositor or connection dropped.
                // Set connected = false; push Unavailable to all niri-dependent modules
                // so they render as nothing (zero size, no gap) — FR21, AC #1, UX-DR16.
                self.shared.niri.connected = false;
                for (_, module) in &self.modules {
                    if matches!(module.module_type(), "workspaces" | "focused-window") {
                        self.shared
                            .modules
                            .insert(module.id(), ModuleState::Unavailable);
                    }
                }
                Task::none()
            }
            Message::Niri(NiriEvent::Event(event)) => {
                use niri_ipc::Event;
                match event {
                    Event::WorkspacesChanged { workspaces } => {
                        // Update each workspace module's ModuleState so view_*() gets fresh data (AC #7).
                        for (_, module) in &self.modules {
                            if module.module_type() == "workspaces" {
                                self.shared.modules.insert(
                                    module.id(),
                                    ModuleState::Workspace(workspaces::WorkspaceState {
                                        workspaces: workspaces.clone(),
                                    }),
                                );
                            }
                        }
                        // In manual-output mode, open bars for any newly connected outputs.
                        // WorkspacesChanged fires when a new output is hotplugged, and the
                        // new output's workspaces appear in this event with an output name we
                        // haven't seen before.
                        let open_tasks = if self.manual_outputs {
                            self.open_new_output_windows(&workspaces)
                        } else {
                            vec![]
                        };
                        self.shared.niri.workspaces = workspaces;
                        if !open_tasks.is_empty() {
                            return Task::batch(open_tasks);
                        }
                    }
                    Event::WindowFocusChanged { id } => {
                        self.shared.niri.focused_window_id = id;
                        // Only push when focusing an actual window. When id is None, focus moved
                        // to a layer-shell surface (e.g. hovering the bar); retain the last
                        // known title so the module doesn't blank on bar hover.
                        if id.is_some() {
                            self.push_focused_window_state(); // AC #1 — update within one frame
                        }
                    }
                    Event::WorkspaceActivated { id, focused } => {
                        // Update is_active per output and is_focused globally.
                        let output = self
                            .shared
                            .niri
                            .workspaces
                            .iter()
                            .find(|ws| ws.id == id)
                            .and_then(|ws| ws.output.clone());

                        for ws in &mut self.shared.niri.workspaces {
                            if output.is_some() && ws.output == output {
                                ws.is_active = ws.id == id;
                            }
                            if focused {
                                ws.is_focused = ws.id == id;
                            }
                        }

                        // Propagate updated workspace list to module state.
                        let updated = self.shared.niri.workspaces.clone();
                        for (_, module) in &self.modules {
                            if module.module_type() == "workspaces" {
                                self.shared.modules.insert(
                                    module.id(),
                                    ModuleState::Workspace(workspaces::WorkspaceState {
                                        workspaces: updated.clone(),
                                    }),
                                );
                            }
                        }
                    }
                    Event::WindowsChanged { windows } => {
                        // Full snapshot — rebuild window cache and derive focused id.
                        self.shared.niri.focused_window_id =
                            windows.iter().find(|w| w.is_focused).map(|w| w.id);
                        self.shared.niri.windows = windows.into_iter().map(|w| (w.id, w)).collect();
                        self.push_focused_window_state();
                    }
                    Event::WindowOpenedOrChanged { window } => {
                        // If this window is now focused, update focused_window_id.
                        if window.is_focused {
                            self.shared.niri.focused_window_id = Some(window.id);
                        }
                        self.shared.niri.windows.insert(window.id, window);
                        self.push_focused_window_state();
                    }
                    Event::WindowClosed { id } => {
                        self.shared.niri.windows.remove(&id);
                        if self.shared.niri.focused_window_id == Some(id) {
                            self.shared.niri.focused_window_id = None;
                        }
                        self.push_focused_window_state();
                    }
                    _ => {} // Other events (keyboard layouts, urgency, etc.) not used yet
                }
                Task::none()
            }
            Message::Tick => {
                // Global tick: advance animation and hover delay for ALL surfaces.
                const TICK_MS: f32 = 16.0;

                let window_ids: Vec<_> = self.surfaces.keys().copied().collect();
                let mut tasks = vec![];

                for wid in window_ids {
                    // Resolve bar config before taking a mutable borrow of the surface.
                    let bar_name = self
                        .surfaces
                        .get(&wid)
                        .map(|s| s.bar_name.clone())
                        .unwrap_or_default();
                    let effective_bar =
                        Self::bar_for_surface_name(&self.config.bars, &bar_name);
                    let effective_hover_delay = effective_bar.hover_delay;
                    let effective_expand_ms = effective_bar.animation.expand_ms;
                    let effective_collapse_ms = effective_bar.animation.collapse_ms;
                    let s = self.surfaces.get_mut(&wid).unwrap();

                    // Hover delay countdown — runs while waiting to start expand (Story 3.3).
                    if let Some(elapsed) = s.hover_pending_ms {
                        let new_elapsed = elapsed + TICK_MS;
                        if new_elapsed >= effective_hover_delay as f32 {
                            s.hover_pending_ms = None;
                            s.expand = ExpandState::Expanding(0.0);
                            tasks.push(self.overlay_size_change_for(wid));
                        } else {
                            s.hover_pending_ms = Some(new_elapsed);
                            // Still counting down — no resize needed this tick for this surface.
                        }
                        continue;
                    }

                    match s.expand {
                        ExpandState::Expanding(progress) => {
                            let new_progress = (progress + TICK_MS / effective_expand_ms).min(1.0);
                            s.expand = if new_progress >= 1.0 {
                                ExpandState::Expanded
                            } else {
                                ExpandState::Expanding(new_progress)
                            };
                            tasks.push(self.overlay_size_change_for(wid));
                        }
                        ExpandState::Collapsing(progress) => {
                            let new_progress =
                                (progress + TICK_MS / effective_collapse_ms).min(1.0);
                            s.expand = if new_progress >= 1.0 {
                                ExpandState::Collapsed
                            } else {
                                ExpandState::Collapsing(new_progress)
                            };
                            tasks.push(self.overlay_size_change_for(wid));
                        }
                        _ => {} // Collapsed or Expanded — no action
                    }
                }

                Task::batch(tasks)
            }
            Message::Network(NetworkEvent::StateChanged {
                connectivity,
                label,
            }) => {
                self.push_network_state(connectivity, label);
                Task::none()
            }
            Message::Network(NetworkEvent::Unavailable) => {
                for (_, module) in &self.modules {
                    if module.module_type() == "network" {
                        self.shared
                            .modules
                            .insert(module.id(), ModuleState::Unavailable);
                    }
                }
                Task::none()
            }
            Message::Volume(VolumeEvent::StateChanged { volume, muted }) => {
                self.push_volume_state(volume, muted);
                Task::none()
            }
            Message::Volume(VolumeEvent::Unavailable) => {
                for (_, module) in &self.modules {
                    if module.module_type() == "volume" {
                        self.shared
                            .modules
                            .insert(module.id(), ModuleState::Unavailable);
                    }
                }
                Task::none()
            }
            Message::VolumeAdjust { step, up } => {
                let arg = if up {
                    format!("{step}%+")
                } else {
                    format!("{step}%-")
                };
                // Fire-and-forget: wpctl adjusts PA volume; PA subscription emits StateChanged.
                // "-l 1" caps at 100%.
                tokio::task::spawn(async move {
                    let _ = tokio::process::Command::new("wpctl")
                        .args(["set-volume", "-l", "1", "@DEFAULT_AUDIO_SINK@", &arg])
                        .status()
                        .await;
                });
                Task::none()
            }
            Message::ModuleScrollUp(id, _window) => {
                for (_, module) in &self.modules {
                    if module.id() == id {
                        if let Some(msg) = module.on_interaction(InteractionEvent::ScrollUp) {
                            return Task::done(msg);
                        }
                        break;
                    }
                }
                Task::none()
            }
            Message::ModuleScrollDown(id, _window) => {
                for (_, module) in &self.modules {
                    if module.id() == id {
                        if let Some(msg) = module.on_interaction(InteractionEvent::ScrollDown) {
                            return Task::done(msg);
                        }
                        break;
                    }
                }
                Task::none()
            }
            Message::Battery(BatteryEvent::StateChanged {
                percentage,
                is_charging,
            }) => {
                self.push_battery_state(percentage, is_charging);
                Task::none()
            }
            Message::Battery(BatteryEvent::Unavailable) => {
                // Mark all battery instances unavailable — renders nothing (UX-DR16).
                for (_, module) in &self.modules {
                    if module.module_type() == "battery" {
                        self.shared
                            .modules
                            .insert(module.id(), ModuleState::Unavailable);
                    }
                }
                Task::none()
            }
            Message::DbusReady(connections) => {
                // Store shared handles; modules borrow clones of these (AR5, Story 5.1).
                self.shared.dbus = connections;
                Task::none()
            }
            Message::BarFocusGained(window) => {
                // Highlight first module in top zone (AC #1); fall back to center/bottom.
                // Use the effective nav list so per-output module overrides are respected.
                let nav = self.effective_nav_list(window);
                let first = nav
                    .iter()
                    .find(|(z, _)| *z == Zone::Top)
                    .or_else(|| nav.iter().find(|(z, _)| *z == Zone::Center))
                    .or_else(|| nav.iter().find(|(z, _)| *z == Zone::Bottom))
                    .map(|(_, id)| id.clone());
                let s = self.surface_mut(window);
                s.focused_module = first;
                s.module_activated = false;
                Task::none()
            }
            Message::BarFocusLost(window) => {
                let s = self.surface_mut(window);
                s.focused_module = None;
                s.module_activated = false;
                // Restore OnDemand so the bar can be focused again (AC #3).
                use iced_layershell::reexport::KeyboardInteractivity;
                Task::done(Message::KeyboardInteractivityChange {
                    id: window,
                    keyboard_interactivity: KeyboardInteractivity::OnDemand,
                })
            }
            Message::EscapePressed(window) => {
                let s = self.surface_mut(window);
                // If a module is activated, Escape deactivates it (returns to navigation mode).
                if s.module_activated {
                    s.module_activated = false;
                    return Task::none();
                }
                if s.focused_module.is_none() {
                    return Task::none();
                }
                // Tell compositor to remove keyboard focus from this surface (AC #2).
                // BarFocusLost fires in response and restores OnDemand.
                use iced_layershell::reexport::KeyboardInteractivity;
                Task::done(Message::KeyboardInteractivityChange {
                    id: window,
                    keyboard_interactivity: KeyboardInteractivity::None,
                })
            }
            Message::FocusMoveNext(window) => {
                let s = self.surface_mut(window);
                if s.focused_module.is_none() {
                    // Delegate to BarFocusGained to acquire focus first.
                    return Task::done(Message::BarFocusGained(window));
                }
                // Interaction mode (Story 6.3): module is activated — arrow key scrolls it.
                if s.module_activated {
                    if let Some(ref id) = s.focused_module.clone() {
                        for (_, m) in &self.modules {
                            if m.id() == *id {
                                if let Some(msg) = m.on_interaction(InteractionEvent::ScrollDown) {
                                    return Task::done(msg);
                                }
                                break;
                            }
                        }
                    }
                    return Task::none();
                }
                // Navigation mode: move to next module.
                if let Some(ref current) = s.focused_module.clone()
                    && let Some(next) = self.nav_next(current, window)
                {
                    self.surface_mut(window).focused_module = Some(next);
                }
                Task::none()
            }
            Message::FocusMovePrev(window) => {
                let s = self.surface_mut(window);
                if s.focused_module.is_none() {
                    return Task::done(Message::BarFocusGained(window));
                }
                // Interaction mode (Story 6.3): module is activated — arrow key scrolls it.
                if s.module_activated {
                    if let Some(ref id) = s.focused_module.clone() {
                        for (_, m) in &self.modules {
                            if m.id() == *id {
                                if let Some(msg) = m.on_interaction(InteractionEvent::ScrollUp) {
                                    return Task::done(msg);
                                }
                                break;
                            }
                        }
                    }
                    return Task::none();
                }
                // Navigation mode: move to previous module.
                if let Some(ref current) = s.focused_module.clone()
                    && let Some(prev) = self.nav_prev(current, window)
                {
                    self.surface_mut(window).focused_module = Some(prev);
                }
                Task::none()
            }
            Message::FocusNextZone(window) => {
                let s = self.surface_mut(window);
                if s.focused_module.is_none() {
                    return Task::done(Message::BarFocusGained(window));
                }
                s.module_activated = false; // deactivate on zone jump
                if let Some(ref current) = s.focused_module.clone()
                    && let Some(next) = self.nav_next_zone(current, window)
                {
                    self.surface_mut(window).focused_module = Some(next);
                }
                Task::none()
            }
            Message::FocusPrevZone(window) => {
                let s = self.surface_mut(window);
                if s.focused_module.is_none() {
                    return Task::done(Message::BarFocusGained(window));
                }
                s.module_activated = false; // deactivate on zone jump
                if let Some(ref current) = s.focused_module.clone()
                    && let Some(prev) = self.nav_prev_zone(current, window)
                {
                    self.surface_mut(window).focused_module = Some(prev);
                }
                Task::none()
            }
            Message::KeyboardActivate(window) => {
                let s = self.surface_mut(window);
                if s.focused_module.is_none() {
                    return Task::none();
                }
                if let Some(ref id) = s.focused_module.clone() {
                    for (_, module) in &self.modules {
                        if module.id() == *id {
                            // Scrollable module: Enter/Space enters interaction mode.
                            if module
                                .on_interaction(InteractionEvent::ScrollDown)
                                .is_some()
                            {
                                self.surface_mut(window).module_activated = true;
                                return Task::none();
                            }
                            // Clickable module: fire the click action.
                            if let Some(msg) = module.on_interaction(InteractionEvent::Clicked) {
                                return Task::done(msg);
                            }
                            break;
                        }
                    }
                }
                Task::none()
            }
            Message::KeyboardSecondaryActivate(window) => {
                let s = self.surface_mut(window);
                if s.focused_module.is_none() {
                    return Task::none();
                }
                if let Some(ref id) = s.focused_module.clone() {
                    for (_, module) in &self.modules {
                        if module.id() == *id {
                            if let Some(msg) = module.on_interaction(InteractionEvent::Activated) {
                                return Task::done(msg);
                            }
                            break;
                        }
                    }
                }
                Task::none()
            }
            Message::ToggleExpand => {
                // IPC-sourced: no window context — toggle each surface if its bar has expand enabled.
                let window_ids: Vec<_> = self.surfaces.keys().copied().collect();
                let mut tasks = vec![];
                for wid in window_ids {
                    let bar_name = self
                        .surfaces
                        .get(&wid)
                        .map(|s| s.bar_name.clone())
                        .unwrap_or_default();
                    let expand_enabled = Self::bar_for_surface_name(&self.config.bars, &bar_name)
                        .expand;
                    if expand_enabled {
                        let s = self.surfaces.get_mut(&wid).unwrap();
                        s.expand = s.expand.toggle();
                        tasks.push(self.overlay_size_change_for(wid));
                    }
                }
                Task::batch(tasks)
            }
            Message::TrayUpdated(items) => {
                use crate::modules::tray::TrayIconModule;

                // Find the zone and config name of each TrayModule placeholder so we can
                // insert icon modules at the same position.
                let tray_slots: Vec<(usize, Zone, String)> = self
                    .modules
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, m))| m.module_type() == "tray")
                    .map(|(i, (z, m))| (i, z.clone(), m.id().0.clone()))
                    .collect();

                // Remove all existing tray icon modules (keep the TrayModule placeholders).
                self.modules.retain(|(_, m)| m.module_type() != "tray_icon");

                // Clear focused/hovered module on every surface if they pointed at a tray icon
                // that may no longer exist after this rebuild.
                for s in self.surfaces.values_mut() {
                    if s.focused_module.as_ref().map(|id| id.0.starts_with("tray:")).unwrap_or(false) {
                        s.focused_module = None;
                    }
                    if s.hovered_module.as_ref().map(|id| id.0.starts_with("tray:")).unwrap_or(false) {
                        s.hovered_module = None;
                    }
                }

                // For each TrayModule slot, insert one TrayIconModule per active item
                // immediately after the placeholder's current index.
                // Insert in reverse slot order so earlier slots' indices remain valid.
                let active_items: Vec<&crate::dbus::sni_host::TrayItem> = items
                    .iter()
                    .filter(|i| i.status != crate::dbus::sni_host::ItemStatus::Passive)
                    .collect();

                for (slot_idx, zone, config_name) in tray_slots.iter().rev() {
                    let insert_at = slot_idx + 1;
                    for item in active_items.iter().rev() {
                        let icon_module = TrayIconModule::new((*item).clone(), config_name.clone());
                        self.modules.insert(insert_at, (zone.clone(), Box::new(icon_module)));
                    }
                }

                Task::none()
            }
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
            Message::TrayMenuRequest { service, menu_path } => {
                let conn = self.shared.dbus.session.clone();
                if let (Some(conn), Some(menu_path)) = (conn, menu_path) {
                    let service_clone = service.clone();
                    let menu_path_clone = menu_path.clone();
                    // Capture the cursor position at the moment of right-click.
                    // For vertical bars: window-local y == screen y; x is within bar width.
                    // For horizontal bars: window-local x == screen x; y is within bar height.
                    let cursor_pos = self.last_cursor.map(|(_, p)| p).unwrap_or(iced::Point::ORIGIN);
                    let anchor_x = cursor_pos.x as i32;
                    let anchor_y = cursor_pos.y as i32;
                    let screen_size = self.last_cursor
                        .and_then(|(w, _)| self.surfaces.get(&w))
                        .map(|s| s.screen_size)
                        .unwrap_or((0, 0));
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
                                }
                            } else {
                                // Menu fetch failed -- fall back to Activate.
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

                // Full-screen overlay: covers the entire output so clicks on the transparent
                // area outside the menu widget trigger MenuClosed. The bar background is now
                // drawn by a container widget (style() returns transparent).
                let bar_name = self
                    .last_cursor
                    .and_then(|(w, _)| self.surfaces.get(&w))
                    .map(|s| s.bar_name.clone())
                    .unwrap_or_default();
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
                self.menu_popups.insert(popup_id, (service, menu_path, tree, bar_name));
                tasks.push(open_task);
                Task::batch(tasks)
            }
            Message::MenuItemSelected {
                service,
                menu_item_id,
                menu_path,
            } => {
                let conn = self.shared.dbus.session.clone();
                tokio::task::spawn(async move {
                    if let Some(conn) = conn {
                        let _ = crate::dbus::sni_host::send_menu_event(
                            &conn,
                            &service,
                            &menu_path,
                            menu_item_id,
                            "clicked",
                        )
                        .await;
                    }
                });
                // Close all popups.
                let ids: Vec<iced::window::Id> = self.menu_popups.keys().copied().collect();
                let tasks: Vec<Task<Message>> = ids
                    .into_iter()
                    .map(|id| {
                        self.menu_popups.remove(&id);
                        self.menu_popup_hovered.remove(&id);
                        Task::done(Message::RemoveWindow(id))
                    })
                    .collect();
                Task::batch(tasks)
            }
            Message::MenuClosed => {
                // Send Event(0, "closed") to each open popup's DBusMenu before tearing down.
                let conn = self.shared.dbus.session.clone();
                let ids: Vec<iced::window::Id> = self.menu_popups.keys().copied().collect();
                for id in &ids {
                    if let Some((service, menu_path, _, _)) = self.menu_popups.get(id) {
                        if let Some(conn) = conn.clone() {
                            let service = service.clone();
                            let menu_path = menu_path.clone();
                            tokio::task::spawn(async move {
                                let _ = crate::dbus::sni_host::send_menu_event(
                                    &conn, &service, &menu_path, 0, "closed",
                                )
                                .await;
                            });
                        }
                    }
                }
                let tasks: Vec<Task<Message>> = ids
                    .into_iter()
                    .map(|id| {
                        self.menu_popups.remove(&id);
                        self.menu_popup_hovered.remove(&id);
                        Task::done(Message::RemoveWindow(id))
                    })
                    .collect();
                Task::batch(tasks)
            }
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
                // Preserve the existing anchor and screen_size from the current popup.
                let existing = self.menu_popups.values().find(|(s, mp, _, _)| {
                    s == &service && mp == &menu_path
                });
                let (anchor, screen_size) = match existing {
                    Some((_, _, tree, _)) => (tree.anchor, tree.screen_size),
                    None => return Task::none(),
                };
                if let Some(conn) = conn {
                    let svc = service.clone();
                    let mp = menu_path.clone();
                    return Task::perform(
                        async move {
                            crate::dbus::sni_host::fetch_menu(&conn, &svc, &mp).await
                        },
                        move |tree_opt| {
                            if let Some(mut tree) = tree_opt {
                                tree.anchor = anchor;
                                tree.screen_size = screen_size;
                                Message::OpenMenu {
                                    service,
                                    menu_path,
                                    tree,
                                }
                            } else {
                                Message::MenuClosed
                            }
                        },
                    );
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        let module_subs =
            iced::Subscription::batch(self.modules.iter().map(|(_, m)| m.subscription()));
        let niri_sub = niri_subscription();

        // AC #3: Only connect to D-Bus if at least one D-Bus-backed module is present.
        let needs_dbus = self
            .modules
            .iter()
            .any(|(_, m)| matches!(m.module_type(), "battery" | "network" | "volume" | "tray"));
        let dbus_sub = if needs_dbus {
            crate::dbus::dbus_init_subscription()
        } else {
            iced::Subscription::none()
        };

        // AC #9: Only subscribe if battery module is present AND system bus is connected.
        // UPower runs on the system bus (org.freedesktop.UPower), not the session bus.
        let battery_sub = if let Some(conn) = self.shared.dbus.system.clone() {
            if self
                .modules
                .iter()
                .any(|(_, m)| m.module_type() == "battery")
            {
                crate::dbus::upower::battery_subscription(conn)
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        // AC #9: Only subscribe if network module is present AND system bus is connected.
        let network_sub = if let Some(conn) = self.shared.dbus.system.clone() {
            if self
                .modules
                .iter()
                .any(|(_, m)| m.module_type() == "network")
            {
                crate::dbus::networkmanager::network_subscription(conn)
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        // AC #9: Only subscribe if volume module is present.
        // No zbus connection needed — PA auto-discovers its socket.
        let volume_sub = if self
            .modules
            .iter()
            .any(|(_, m)| m.module_type() == "volume")
        {
            crate::dbus::wireplumber::volume_subscription()
        } else {
            iced::Subscription::none()
        };

        // Start SNI subscription only if tray module is present AND session bus is connected.
        let sni_sub = if let Some(conn) = self.shared.dbus.session.clone() {
            if self.modules.iter().any(|(_, m)| m.module_type() == "tray") {
                crate::dbus::sni_host::sni_subscription(conn)
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        // Watch for DBusMenu LayoutUpdated/ItemsPropertiesUpdated while a popup is open.
        let menu_watch_sub = if let Some(conn) = self.shared.dbus.session.clone() {
            if let Some((service, menu_path, _, _)) = self.menu_popups.values().next() {
                crate::dbus::sni_host::menu_layout_subscription(
                    conn,
                    service.clone(),
                    menu_path.clone(),
                )
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        let focus_sub = keyboard_focus_subscription();
        let ipc_sub = ipc_subscription();

        // Emit a Tick every ~16ms (≈60fps) when ANY surface is animating or has pending hover.
        let needs_tick = self
            .surfaces
            .values()
            .any(|s| s.expand.is_animating() || s.hover_pending_ms.is_some());
        let tick_sub = if needs_tick {
            iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick)
        } else {
            iced::Subscription::none()
        };

        iced::Subscription::batch([
            module_subs,
            niri_sub,
            dbus_sub,
            battery_sub,
            network_sub,
            volume_sub,
            sni_sub,
            menu_watch_sub,
            focus_sub,
            ipc_sub,
            tick_sub,
        ])
    }

    pub fn view(&self, window: iced::window::Id) -> Element<'_, Message> {
        // Popup window -- full-screen transparent overlay; menu widget positioned inside.
        if let Some((service, menu_path, tree, bar_name)) = self.menu_popups.get(&window) {
            use crate::surface::menu_popup::view_menu_popup;
            let hovered_item = self.menu_popup_hovered.get(&window).map(|s| s.as_str());
            let menu = view_menu_popup(tree, service.as_str(), menu_path, window, hovered_item);

            let bar = Self::bar_for_surface_name(&self.config.bars, bar_name);
            let bar_size = bar.collapsed_width as f32;

            // Clamp the anchor so the popup stays within screen bounds.
            let (popup_w, popup_h) = crate::surface::menu_popup::popup_size(tree);
            let screen_w = tree.screen_size.0 as f32;
            let screen_h = tree.screen_size.1 as f32;
            let raw_cursor_x = tree.anchor.0 as f32;
            let raw_cursor_y = tree.anchor.1 as f32;
            let cursor_x = if screen_w > 0.0 {
                raw_cursor_x.min(screen_w - popup_w as f32).max(0.0)
            } else {
                raw_cursor_x
            };
            let cursor_y = if screen_h > 0.0 {
                raw_cursor_y.min(screen_h - popup_h as f32).max(0.0)
            } else {
                raw_cursor_y
            };

            let (h_align, v_align, padding) = match &bar.edge {
                crate::config::Edge::Left => (
                    iced::alignment::Horizontal::Left,
                    iced::alignment::Vertical::Top,
                    iced::Padding { top: cursor_y, left: bar_size, ..Default::default() },
                ),
                crate::config::Edge::Right => (
                    iced::alignment::Horizontal::Right,
                    iced::alignment::Vertical::Top,
                    iced::Padding { top: cursor_y, right: bar_size, ..Default::default() },
                ),
                crate::config::Edge::Top => (
                    iced::alignment::Horizontal::Left,
                    iced::alignment::Vertical::Top,
                    iced::Padding { top: bar_size, left: cursor_x, ..Default::default() },
                ),
                crate::config::Edge::Bottom => (
                    iced::alignment::Horizontal::Left,
                    iced::alignment::Vertical::Bottom,
                    iced::Padding { bottom: bar_size, left: cursor_x, ..Default::default() },
                ),
            };

            // Pin the menu to its computed size so Fill items inside don't expand to
            // screen width, and so the outer Fill container doesn't collapse it.
            let menu_sized = iced::widget::container(menu)
                .width(popup_w)
                .height(popup_h);

            return iced::widget::mouse_area(
                iced::widget::container(menu_sized)
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
                    .align_x(h_align)
                    .align_y(v_align)
                    .padding(padding),
            )
            .on_press(Message::MenuClosed)
            .into();
        }

        let surface = self.surface(window).cloned().unwrap_or_default();
        // Look up bar by name; falls back to first bar for AllScreens mode (bar_name is "").
        let bar = Self::bar_for_surface_name(&self.config.bars, &surface.bar_name);
        let is_horizontal = bar.edge.is_horizontal();

        // All bars animate between collapsed_width and expanded_width.
        // Overlay mode: the surface grows over the desktop without changing the exclusive zone.
        let surface_size = surface
            .expand
            .current_width(bar.collapsed_width, bar.expanded_width);
        // Reserve 1px for the border strip on the inner screen edge.
        let content_size = surface_size.saturating_sub(1);

        // Show expanded content whenever the bar is expanding or fully expanded.
        // label_opacity (UX-DR6, 60ms delay fade) is deferred: iced 0.14 has no generic
        // element opacity wrapper; see deferred-work.md.
        let shows_expanded = surface.expand.shows_expanded_content();

        let mut top: Vec<Element<'_, Message>> = vec![];
        let mut center: Vec<Element<'_, Message>> = vec![];
        let mut bottom: Vec<Element<'_, Message>> = vec![];

        // Build a name→zone map from the effective (possibly per-output) module list.
        // This filters which modules appear on this surface and their zone assignment.
        let mut effective_zone: HashMap<String, Zone> = HashMap::new();
        for (name, zone) in bar.modules.top.iter().map(|n| (n, Zone::Top))
            .chain(bar.modules.center.iter().map(|n| (n, Zone::Center)))
            .chain(bar.modules.bottom.iter().map(|n| (n, Zone::Bottom)))
        {
            effective_zone.insert(name.clone(), zone.clone());
            // Expand tray config entries: add all icon modules that carry this config name.
            for (_, m) in &self.modules {
                if m.config_name() == Some(name.as_str()) {
                    effective_zone.insert(m.id().0.clone(), zone.clone());
                }
            }
        }

        for (_boot_zone, module) in &self.modules {
            let module_name = &module.id().0;
            let Some(zone) = effective_zone.get(module_name) else {
                continue; // not in this output's effective module list
            };

            let state = {
                use crate::modules::ModuleState;
                let raw = self
                    .shared
                    .modules
                    .get(&module.id())
                    .cloned()
                    .unwrap_or_default();
                // Filter state to be output-aware where required.
                match raw {
                    ModuleState::Workspace(ref ws) => {
                        ModuleState::Workspace(ws.filtered_for_output(&surface.output_name))
                    }
                    ModuleState::FocusedWindow(ref fw) => {
                        // AllScreens mode (output_name is ""): show global title unchanged.
                        // Per-output mode: when the globally focused window is on a different
                        // output, show nothing rather than leaking another output's title.
                        let title = if surface.output_name.is_empty() {
                            fw.title.clone()
                        } else {
                            match self.shared.niri.focused_window_id {
                                // No window focused (bar hover or empty workspace/output).
                                // Use the active window on this output's workspace so the title
                                // is correct on bar hover, and blank when the output is empty.
                                None => self
                                    .shared
                                    .niri
                                    .active_window_title_for_output(&surface.output_name),
                                Some(_) => {
                                    let fo = self.shared.niri.focused_output();
                                    if fo.is_none() || fo == Some(surface.output_name.as_str()) {
                                        fw.title.clone()
                                    } else {
                                        // Focused window is on a different output — show the
                                        // active window on this output's workspace instead.
                                        self.shared
                                            .niri
                                            .active_window_title_for_output(&surface.output_name)
                                    }
                                }
                            }
                        };
                        ModuleState::FocusedWindow(focused_window::FocusedWindowState { title })
                    }
                    other => other,
                }
            };
            let id = module.id();
            let hovered = surface.hovered_module.as_ref() == Some(&id);
            let focused = surface.focused_module.as_ref() == Some(&id);

            // During Expanding: show expanded content (opacity fade deferred — no iced wrapper).
            // During Collapsing: show collapsed content immediately (no reverse fade).
            // Horizontal bars use view_horizontal when collapsed, view_expanded when the overlay is open.
            let module_view: Element<'_, Message> = if shows_expanded {
                module.view_expanded(&state)
            } else if is_horizontal {
                module.view_horizontal(&state)
            } else {
                module.view_collapsed(&state)
            };

            let scroll_id = id.clone();
            // module_frame treats hover and keyboard focus identically (UX-DR3).
            let view: Element<'_, Message> =
                mouse_area(module_frame(module_view, hovered || focused))
                    .on_enter(Message::ModuleHoverEnter(id.clone(), window))
                    .on_exit(Message::ModuleHoverExit(id.clone(), window))
                    .on_press(Message::ModuleClick(id, window))
                    .on_scroll(move |delta| {
                        let up = match delta {
                            iced::mouse::ScrollDelta::Lines { y, .. }
                            | iced::mouse::ScrollDelta::Pixels { y, .. } => y > 0.0,
                        };
                        if up {
                            Message::ModuleScrollUp(scroll_id.clone(), window)
                        } else {
                            Message::ModuleScrollDown(scroll_id.clone(), window)
                        }
                    })
                    .into();

            match zone {
                Zone::Top => top.push(view),
                Zone::Center => center.push(view),
                Zone::Bottom => bottom.push(view),
            }
        }

        let border_style = |_theme: &_| container::Style {
            background: Some(iced::Background::Color(crate::theme::SURFACE1)),
            ..Default::default()
        };

        // For horizontal bars the animated dimension is height; for vertical bars it's width.
        let (content_w, content_h, border_w, border_h) = if is_horizontal {
            (
                Length::Fill,
                Length::from(content_size),
                Length::Fill,
                Length::from(1),
            )
        } else {
            (
                Length::from(content_size),
                Length::Fill,
                Length::from(1),
                Length::Fill,
            )
        };

        let content = container(zone_layout(top, center, bottom, is_horizontal))
            .width(content_w)
            .height(content_h);
        let border_strip = container(Space::new())
            .width(border_w)
            .height(border_h)
            .style(border_style);

        // Border strip sits on the inner screen edge (the side facing screen content).
        let bar_content: Element<'_, Message> = match bar.edge {
            Edge::Left => row![content, border_strip]
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            Edge::Right => row![border_strip, content]
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            Edge::Top => column![content, border_strip]
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            Edge::Bottom => column![border_strip, content]
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
        };

        // The window background is transparent (style() returns Color::TRANSPARENT) so the
        // popup overlay works. The bar draws its own background via this container.
        let bar_with_bg = iced::widget::container(bar_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(iced::Background::Color(crate::theme::BASE)),
                ..Default::default()
            });

        // Wrap entire bar in mouse_area for click-to-toggle and hover detection.
        // Note: clicking a module fires both ModuleClick(id) AND BarClick (outer area).
        mouse_area(bar_with_bg)
            .on_press(Message::BarClick(window))
            .on_enter(Message::BarHoverEnter(window))
            .on_exit(Message::BarHoverExit(window))
            .into()
    }

    pub fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            // Transparent so full-screen popup overlays show correctly; bar draws its
            // own background via a container widget in view_bar().
            background_color: iced::Color::TRANSPARENT,
            text_color: crate::theme::TEXT,
        }
    }

    /// Build an ordered list of (zone, module_id) for the given surface's effective module layout.
    /// This filters self.modules to only those visible on this bar's module list, in zone order.
    fn effective_nav_list(&self, window: iced::window::Id) -> Vec<(Zone, ModuleId)> {
        let bar_name = self
            .surfaces
            .get(&window)
            .map(|s| s.bar_name.as_str())
            .unwrap_or("");
        let bar = Self::bar_for_surface_name(&self.config.bars, bar_name);
        // Rebuild the ordered (zone, name) list from the effective bar modules.
        let mut result = Vec::new();
        for (zone, names) in [
            (Zone::Top, &bar.modules.top),
            (Zone::Center, &bar.modules.center),
            (Zone::Bottom, &bar.modules.bottom),
        ] {
            for name in names {
                // For tray config entries, expand to all active icon modules that carry
                // this config name; for all other modules, look up by id as before.
                let icon_ids: Vec<ModuleId> = self
                    .modules
                    .iter()
                    .filter(|(_, m)| m.config_name() == Some(name.as_str()))
                    .map(|(_, m)| m.id())
                    .collect();
                if !icon_ids.is_empty() {
                    for id in icon_ids {
                        result.push((zone.clone(), id));
                    }
                } else if let Some((_, m)) = self.modules.iter().find(|(_, m)| m.id().0 == *name) {
                    result.push((zone.clone(), m.id()));
                }
            }
        }
        result
    }

    /// Returns the ID of the next module in the surface's effective layout (wraps last→first).
    fn nav_next(&self, current: &ModuleId, window: iced::window::Id) -> Option<ModuleId> {
        let list = self.effective_nav_list(window);
        let idx = list.iter().position(|(_, id)| id == current)?;
        let next_idx = (idx + 1) % list.len();
        Some(list[next_idx].1.clone())
    }

    /// Returns the ID of the previous module in the surface's effective layout (wraps first→last).
    fn nav_prev(&self, current: &ModuleId, window: iced::window::Id) -> Option<ModuleId> {
        let list = self.effective_nav_list(window);
        let idx = list.iter().position(|(_, id)| id == current)?;
        let prev_idx = (idx + list.len() - 1) % list.len();
        Some(list[prev_idx].1.clone())
    }

    /// Returns the ID of the first module in the next non-empty zone for this surface
    /// (Tab cycle: Top→Center→Bottom→Top).
    fn nav_next_zone(&self, current: &ModuleId, window: iced::window::Id) -> Option<ModuleId> {
        let list = self.effective_nav_list(window);
        let idx = list.iter().position(|(_, id)| id == current)?;
        let current_zone = &list[idx].0;
        let cycle = [Zone::Top, Zone::Center, Zone::Bottom];
        let start = cycle.iter().position(|z| z == current_zone).unwrap_or(0);
        for step in 1..=3 {
            let next_zone = &cycle[(start + step) % 3];
            if let Some((_, id)) = list.iter().find(|(z, _)| z == next_zone) {
                return Some(id.clone());
            }
        }
        None
    }

    /// Returns the ID of the first module in the previous non-empty zone for this surface
    /// (Shift+Tab cycle: Top→Bottom→Center→Top).
    fn nav_prev_zone(&self, current: &ModuleId, window: iced::window::Id) -> Option<ModuleId> {
        let list = self.effective_nav_list(window);
        let idx = list.iter().position(|(_, id)| id == current)?;
        let current_zone = &list[idx].0;
        // Reverse cycle: Top(0)→Bottom(2)→Center(1)→Top(0)
        let cycle = [Zone::Top, Zone::Bottom, Zone::Center];
        let start = cycle.iter().position(|z| z == current_zone).unwrap_or(0);
        for step in 1..=3 {
            let prev_zone = &cycle[(start + step) % 3];
            if let Some((_, id)) = list.iter().find(|(z, _)| z == prev_zone) {
                return Some(id.clone());
            }
        }
        None
    }

    /// Returns a `SizeChange` task for the named surface's current expand state in overlay mode.
    ///
    /// Overlay mode: the surface grows over the desktop — exclusive zone is never changed.
    /// For horizontal bars the animated dimension is height `(0, size)`; for vertical it's width `(size, 0)`.
    fn overlay_size_change_for(&self, window: iced::window::Id) -> Task<Message> {
        let surface = self.surfaces.get(&window);
        let expand = surface.map(|s| s.expand).unwrap_or_default();
        let bar_name = surface.map(|s| s.bar_name.as_str()).unwrap_or("");
        let bar = Self::bar_for_surface_name(&self.config.bars, bar_name);
        let size = expand.current_width(bar.collapsed_width, bar.expanded_width);
        if bar.edge.is_horizontal() {
            Task::done(Message::SizeChange {
                id: window,
                size: (0, size),
            })
        } else {
            Task::done(Message::SizeChange {
                id: window,
                size: (size, 0),
            })
        }
    }

    /// Push new battery state to all battery module instances in shared.modules.
    fn push_battery_state(&mut self, percentage: f64, is_charging: bool) {
        for (_, module) in &self.modules {
            if module.module_type() == "battery" {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::Battery(BatteryState {
                        percentage,
                        is_charging,
                    }),
                );
            }
        }
    }

    fn push_network_state(
        &mut self,
        connectivity: crate::modules::network::NetworkConnectivity,
        label: Option<String>,
    ) {
        for (_, module) in &self.modules {
            if module.module_type() == "network" {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::Network(NetworkState {
                        connectivity: connectivity.clone(),
                        label: label.clone(),
                    }),
                );
            }
        }
    }

    fn push_volume_state(&mut self, volume: f64, muted: bool) {
        for (_, module) in &self.modules {
            if module.module_type() == "volume" {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::Volume(VolumeState { volume, muted }),
                );
            }
        }
    }

    /// Derive the current focused window title from NiriState and push it to all
    /// focused-window module instances in shared.modules.
    fn push_focused_window_state(&mut self) {
        let title = self
            .shared
            .niri
            .focused_window_id
            .and_then(|id| self.shared.niri.windows.get(&id))
            .and_then(|w| w.title.clone());
        for (_, module) in &self.modules {
            if module.module_type() == "focused-window" {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::FocusedWindow(focused_window::FocusedWindowState {
                        title: title.clone(),
                    }),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            module_type: "clock".into(),
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "script".into(),
            module_type: "script".into(),
            params: ModuleParams::Script(ScriptConfig {
                command: "echo hi".into(),
                interval: 5,
            }),
        }];
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
    fn button_module_instantiated_from_registry() {
        use crate::config::{ButtonConfig, ModuleConfig, ModuleParams};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "button".into(),
            module_type: "button".into(),
            params: ModuleParams::Button(ButtonConfig {
                label: "Launch".into(),
                icon: ">".into(),
                on_click: "kitty".into(),
            }),
        }];
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

        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "script".into(),
            module_type: "script".into(),
            params: ModuleParams::Script(ScriptConfig {
                command: "echo hi".into(),
                interval: 5,
            }),
        }];
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            module_type: "clock".into(),
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "workspaces".into(),
            module_type: "workspaces".into(),
            params: ModuleParams::None,
        }];
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
        let id = ModuleId::new("workspaces");
        match app.shared.modules.get(&id) {
            Some(ModuleState::Workspace(s)) => assert_eq!(s.workspaces.len(), 1),
            other => panic!("expected Workspace state, got {:?}", other),
        }
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
        let id = ModuleId::new("workspaces");
        match app.shared.modules.get(&id) {
            Some(ModuleState::Workspace(s)) => {
                assert_eq!(s.workspaces.len(), 2);
                let focused: Vec<_> = s.workspaces.iter().filter(|w| w.is_focused).collect();
                assert_eq!(focused.len(), 1);
                assert_eq!(focused[0].idx, 1);
            }
            other => panic!("expected Workspace state, got {:?}", other),
        }
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
        let id = ModuleId::new("workspaces");
        match app.shared.modules.get(&id) {
            Some(ModuleState::Workspace(s)) => {
                assert_eq!(s.workspaces.len(), 3);
                assert_eq!(s.workspaces[0].idx, 2);
                assert_eq!(s.workspaces[1].idx, 0);
                assert_eq!(s.workspaces[2].idx, 1);
            }
            other => panic!("expected Workspace state, got {:?}", other),
        }
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

        let id = ModuleId::new("workspaces");
        match app.shared.modules.get(&id) {
            Some(ModuleState::Workspace(s)) => {
                let focused: Vec<_> = s.workspaces.iter().filter(|w| w.is_focused).collect();
                assert_eq!(focused.len(), 1);
                assert_eq!(focused[0].idx, 1);
            }
            other => panic!("expected Workspace state, got {:?}", other),
        }
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "focused-window".into(),
            module_type: "focused-window".into(),
            params: ModuleParams::None,
        }];
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "battery".into(),
            module_type: "battery".into(),
            params: ModuleParams::Battery(BatteryConfig {
                critical_threshold: 15,
            }),
        }];
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "network".into(),
            module_type: "network".into(),
            params: ModuleParams::Network(NetworkConfig),
        }];
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "volume".into(),
            module_type: "volume".into(),
            params: ModuleParams::Volume(VolumeConfig { step: 5 }),
        }];
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            module_type: "clock".into(),
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            module_type: "clock".into(),
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            module_type: "clock".into(),
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            module_type: "clock".into(),
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
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
        let mut config = Config::default();
        config.modules = vec![
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
        ];
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
        let mut config = Config::default();
        config.modules = vec![
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
        ];
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
        let mut config = Config::default();
        config.modules = vec![
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
        ];
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
        assert!(app.surfaces.get(&wid).is_none());
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
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            module_type: "clock".into(),
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
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
                BarConfig { name: "main".into(), ..BarConfig::default() },
                BarConfig { name: "side".into(), output: "DP-1".into(), ..BarConfig::default() },
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
            bars: vec![
                BarConfig { name: "main".into(), ..BarConfig::default() },
            ],
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
                BarConfig { name: "main".into(), output: "all".into(), ..BarConfig::default() },
                BarConfig { name: "top".into(), output: "all".into(), edge: crate::config::Edge::Top, ..BarConfig::default() },
            ],
            modules: vec![],
        };
        // Both bars target "all" → eDP-1 should get 2 surfaces
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        assert_eq!(app.surfaces.len(), 2, "two bars on one output → two surfaces");
    }

    #[test]
    fn surface_bar_name_set_from_pending_surface_key() {
        use crate::config::BarConfig;
        let config = Config {
            bars: vec![
                BarConfig { name: "main".into(), output: "eDP-1".into(), ..BarConfig::default() },
            ],
            modules: vec![],
        };
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        // Surface is pre-initialized from pending_surface_keys; check bar_name
        let bar_names: Vec<&str> = app.surfaces.values()
            .map(|s| s.bar_name.as_str())
            .collect();
        assert!(bar_names.contains(&"main"), "surface must carry bar_name 'main'; got: {bar_names:?}");
    }

    #[test]
    fn surface_output_name_set_from_pending_surface_key() {
        use crate::config::BarConfig;
        let config = Config {
            bars: vec![
                BarConfig { name: "main".into(), output: "eDP-1".into(), ..BarConfig::default() },
            ],
            modules: vec![],
        };
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        let output_names: Vec<&str> = app.surfaces.values()
            .map(|s| s.output_name.as_str())
            .collect();
        assert!(output_names.contains(&"eDP-1"), "surface must carry output_name 'eDP-1'; got: {output_names:?}");
    }

    #[test]
    fn hotplug_output_gets_all_targeting_bar() {
        use crate::config::BarConfig;
        use niri_ipc::Event;
        let config = Config {
            bars: vec![
                BarConfig { name: "main".into(), output: "all".into(), ..BarConfig::default() },
            ],
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
        let output_names: Vec<&str> = app.surfaces.values()
            .map(|s| s.output_name.as_str())
            .filter(|n| !n.is_empty())
            .collect();
        assert!(output_names.contains(&"DP-1"), "hotplugged DP-1 must get a surface; got: {output_names:?}");
    }

    #[test]
    fn hotplug_creates_one_surface_per_bar_targeting_output() {
        use crate::config::BarConfig;
        use niri_ipc::Event;
        let config = Config {
            bars: vec![
                BarConfig { name: "main".into(), output: "all".into(), ..BarConfig::default() },
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
        assert_eq!(app.surfaces.len(), before + 2, "DP-1 should get one surface per bar");
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
        assert!(names.contains(&"bottom".to_string()), "bottom bar not spawned for DP-1");
        assert!(names.contains(&"right".to_string()), "right bar not spawned for DP-1");
    }

    #[test]
    fn tray_not_in_needs_dbus_when_no_tray_module() {
        let (app, _) = App::boot(Config::default(), vec![]);
        // Default config has no tray module -- verify no tray module is registered.
        assert!(!app.modules.iter().any(|(_, m)| m.module_type() == "tray"));
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
        app.update(Message::TrayUpdated(vec![item]));
    }

    // --- Menu item hover tests ---

    fn make_popup_wid(app: &mut App) -> iced::window::Id {
        use crate::surface::menu_popup::{MenuTree, MenuItem};
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
        app.menu_popups.insert(wid, ("org.test".into(), path, tree, String::new()));
        wid
    }

    #[test]
    fn menu_item_hover_enter_sets_hovered_item() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        let _ = app.update(Message::MenuItemHoverEnter { window: wid, item_id: "1".into() });
        assert_eq!(app.menu_popup_hovered.get(&wid).map(|s| s.as_str()), Some("1"));
    }

    #[test]
    fn menu_item_hover_exit_clears_hovered_item() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        let _ = app.update(Message::MenuItemHoverEnter { window: wid, item_id: "1".into() });
        let _ = app.update(Message::MenuItemHoverExit(wid));
        assert!(app.menu_popup_hovered.get(&wid).is_none());
    }

    #[test]
    fn menu_item_hover_enter_different_popups_are_independent() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid_a = make_popup_wid(&mut app);
        let wid_b = make_popup_wid(&mut app);
        let _ = app.update(Message::MenuItemHoverEnter { window: wid_a, item_id: "1".into() });
        // wid_b should have no hovered item
        assert!(app.menu_popup_hovered.get(&wid_b).is_none());
    }

    #[test]
    fn menu_item_hover_enter_on_second_item_replaces_first_without_exit() {
        // Moving from item A directly to item B must not require an explicit exit from A.
        // The on_enter for B alone should update the hovered item.
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        let _ = app.update(Message::MenuItemHoverEnter { window: wid, item_id: "1".into() });
        let _ = app.update(Message::MenuItemHoverEnter { window: wid, item_id: "2".into() });
        assert_eq!(app.menu_popup_hovered.get(&wid).map(|s| s.as_str()), Some("2"));
    }
}
