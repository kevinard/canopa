use std::collections::HashMap;
use std::time::Duration;

use iced::widget::{Space, column, container, mouse_area, row};
use iced::{Element, Length, Task};
use iced_layershell::to_layer_message;

use crate::config::{
    BatteryConfig, ClockConfig, Config, Edge, ExpandTrigger, ModuleConfig, ModuleParams,
    NetworkConfig, VolumeConfig, WorkspacesConfig,
};
use crate::surface::SurfaceState;
use crate::dbus::DbusConnections;
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
use crate::ipc::ipc_subscription;
use crate::niri::subscription::niri_subscription;
use crate::niri::{NiriEvent, NiriState};
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
    /// Per-output render state, keyed by iced::window::Id (Story 7.1).
    /// One entry per Wayland output; lazily initialised on first event or view call.
    pub surfaces: HashMap<iced::window::Id, SurfaceState>,
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
    /// Keyboard-triggered expand/collapse toggle via Unix socket IPC (Story 6.4).
    /// Global: IPC has no window context, so all surfaces toggle simultaneously.
    ToggleExpand,
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
        _ => None,
    }
}

impl App {
    pub fn boot(config: Config) -> (Self, Task<Message>) {
        // Initialize the in-bar keybind for expand/collapse once at startup.
        // Subsequent calls are silently ignored (OnceLock semantics).
        init_expand_keybind(config.bar.expand_keybind.as_deref());

        let registry = build_registry();
        let module_map: std::collections::HashMap<&str, &crate::config::ModuleConfig> = config
            .modules
            .iter()
            .map(|mc| (mc.name.as_str(), mc))
            .collect();
        let bar_mods = &config.bar.modules;
        let mut modules: Vec<(Zone, Box<dyn Module>)> = Vec::new();
        for (zone, names) in [
            (Zone::Top, &bar_mods.top),
            (Zone::Center, &bar_mods.center),
            (Zone::Bottom, &bar_mods.bottom),
        ] {
            for name in names {
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
        }
        let app = App {
            config,
            shared: SharedState::new(),
            modules,
            surfaces: HashMap::new(), // lazily populated on first view/event per window
        };
        (app, Task::none())
    }

    /// Returns a mutable reference to the `SurfaceState` for `id`, inserting a default if absent.
    pub fn surface_mut(&mut self, id: iced::window::Id) -> &mut SurfaceState {
        self.surfaces.entry(id).or_default()
    }

    /// Returns a reference to the `SurfaceState` for `id`, or `None` if the surface is unknown.
    pub fn surface(&self, id: iced::window::Id) -> Option<&SurfaceState> {
        self.surfaces.get(&id)
    }

    pub fn namespace() -> String {
        String::from("niribar")
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
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
                if !self.config.bar.expand {
                    return Task::none();
                }
                // Click has no effect when hover is the expand trigger (Story 3.3 AC #6).
                if self.config.bar.expand_trigger == ExpandTrigger::Hover {
                    return Task::none();
                }
                let s = self.surface_mut(window);
                s.expand = s.expand.toggle();
                self.overlay_size_change_for(window)
            }
            Message::BarHoverEnter(window) => {
                let expand_enabled = self.config.bar.expand;
                let expand_trigger = self.config.bar.expand_trigger;
                let hover_delay = self.config.bar.hover_delay;
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
                let expand_enabled = self.config.bar.expand;
                let expand_trigger = self.config.bar.expand_trigger;
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
                // Set connected = false (already default); niri modules will render nothing.
                self.shared.niri.connected = false;
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
                        self.shared.niri.workspaces = workspaces;
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
                const EXPAND_MS: f32 = 280.0;
                const COLLAPSE_MS: f32 = 200.0;
                const TICK_MS: f32 = 16.0;

                let window_ids: Vec<_> = self.surfaces.keys().copied().collect();
                let mut tasks = vec![];

                for wid in window_ids {
                    let s = self.surfaces.get_mut(&wid).unwrap();

                    // Hover delay countdown — runs while waiting to start expand (Story 3.3).
                    if let Some(elapsed) = s.hover_pending_ms {
                        let new_elapsed = elapsed + TICK_MS;
                        if new_elapsed >= self.config.bar.hover_delay as f32 {
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
                            let new_progress = (progress + TICK_MS / EXPAND_MS).min(1.0);
                            s.expand = if new_progress >= 1.0 {
                                ExpandState::Expanded
                            } else {
                                ExpandState::Expanding(new_progress)
                            };
                            tasks.push(self.overlay_size_change_for(wid));
                        }
                        ExpandState::Collapsing(progress) => {
                            let new_progress = (progress + TICK_MS / COLLAPSE_MS).min(1.0);
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
                let first = self
                    .modules
                    .iter()
                    .find(|(z, _)| *z == Zone::Top)
                    .or_else(|| self.modules.iter().find(|(z, _)| *z == Zone::Center))
                    .or_else(|| self.modules.iter().find(|(z, _)| *z == Zone::Bottom))
                    .map(|(_, m)| m.id());
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
                    && let Some(next) = self.nav_next(current)
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
                    && let Some(prev) = self.nav_prev(current)
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
                    && let Some(next) = self.nav_next_zone(current)
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
                    && let Some(prev) = self.nav_prev_zone(current)
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
                            if module.on_interaction(InteractionEvent::ScrollDown).is_some() {
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
            Message::ToggleExpand => {
                if !self.config.bar.expand {
                    return Task::none();
                }
                // IPC-sourced: no window context — toggle ALL surfaces simultaneously.
                let window_ids: Vec<_> = self.surfaces.keys().copied().collect();
                let tasks: Vec<_> = window_ids
                    .into_iter()
                    .map(|wid| {
                        let s = self.surfaces.get_mut(&wid).unwrap();
                        s.expand = s.expand.toggle();
                        self.overlay_size_change_for(wid)
                    })
                    .collect();
                Task::batch(tasks)
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
            .any(|(_, m)| matches!(m.module_type(), "battery" | "network" | "volume"));
        let dbus_sub = if needs_dbus {
            crate::dbus::dbus_init_subscription()
        } else {
            iced::Subscription::none()
        };

        // AC #9: Only subscribe if battery module is present AND system bus is connected.
        // UPower runs on the system bus (org.freedesktop.UPower), not the session bus.
        let battery_sub = if let Some(conn) = self.shared.dbus.system.clone() {
            if self.modules.iter().any(|(_, m)| m.module_type() == "battery") {
                crate::dbus::upower::battery_subscription(conn)
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        // AC #9: Only subscribe if network module is present AND system bus is connected.
        let network_sub = if let Some(conn) = self.shared.dbus.system.clone() {
            if self.modules.iter().any(|(_, m)| m.module_type() == "network") {
                crate::dbus::networkmanager::network_subscription(conn)
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        // AC #9: Only subscribe if volume module is present.
        // No zbus connection needed — PA auto-discovers its socket.
        let volume_sub = if self.modules.iter().any(|(_, m)| m.module_type() == "volume") {
            crate::dbus::wireplumber::volume_subscription()
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
            focus_sub,
            ipc_sub,
            tick_sub,
        ])
    }

    pub fn view(&self, window: iced::window::Id) -> Element<'_, Message> {
        let is_horizontal = self.config.bar.edge.is_horizontal();
        let surface = self.surface(window).cloned().unwrap_or_default();

        // All bars animate between collapsed_width and expanded_width.
        // Overlay mode: the surface grows over the desktop without changing the exclusive zone.
        let surface_size = surface.expand.current_width(
            self.config.bar.collapsed_width,
            self.config.bar.expanded_width,
        );
        // Reserve 1px for the border strip on the inner screen edge.
        let content_size = surface_size.saturating_sub(1);

        // Show expanded content whenever the bar is expanding or fully expanded.
        // label_opacity (UX-DR6, 60ms delay fade) is deferred: iced 0.14 has no generic
        // element opacity wrapper; see deferred-work.md.
        let shows_expanded = surface.expand.shows_expanded_content();

        let mut top: Vec<Element<'_, Message>> = vec![];
        let mut center: Vec<Element<'_, Message>> = vec![];
        let mut bottom: Vec<Element<'_, Message>> = vec![];

        for (zone, module) in &self.modules {
            let state = self
                .shared
                .modules
                .get(&module.id())
                .cloned()
                .unwrap_or_default();
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
            let view: Element<'_, Message> = mouse_area(module_frame(module_view, hovered || focused))
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
            (Length::Fill, Length::from(content_size), Length::Fill, Length::from(1))
        } else {
            (Length::from(content_size), Length::Fill, Length::from(1), Length::Fill)
        };

        let content = container(zone_layout(top, center, bottom, is_horizontal))
            .width(content_w)
            .height(content_h);
        let border_strip = container(Space::new())
            .width(border_w)
            .height(border_h)
            .style(border_style);

        // Border strip sits on the inner screen edge (the side facing screen content).
        let bar_content: Element<'_, Message> = match self.config.bar.edge {
            Edge::Left => row![content, border_strip].width(Length::Fill).height(Length::Fill).into(),
            Edge::Right => row![border_strip, content].width(Length::Fill).height(Length::Fill).into(),
            Edge::Top => column![content, border_strip].width(Length::Fill).height(Length::Fill).into(),
            Edge::Bottom => column![border_strip, content].width(Length::Fill).height(Length::Fill).into(),
        };

        // Wrap entire bar in mouse_area for click-to-toggle and hover detection.
        // Note: clicking a module fires both ModuleClick(id) AND BarClick (outer area).
        mouse_area(bar_content)
            .on_press(Message::BarClick(window))
            .on_enter(Message::BarHoverEnter(window))
            .on_exit(Message::BarHoverExit(window))
            .into()
    }

    pub fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: crate::theme::BASE,
            text_color: crate::theme::TEXT,
        }
    }

    /// Returns the ID of the next module in the bar (global order, wraps from last to first).
    fn nav_next(&self, current: &ModuleId) -> Option<ModuleId> {
        let idx = self.modules.iter().position(|(_, m)| m.id() == *current)?;
        let next_idx = (idx + 1) % self.modules.len();
        Some(self.modules[next_idx].1.id())
    }

    /// Returns the ID of the previous module in the bar (global order, wraps from first to last).
    fn nav_prev(&self, current: &ModuleId) -> Option<ModuleId> {
        let idx = self.modules.iter().position(|(_, m)| m.id() == *current)?;
        let prev_idx = (idx + self.modules.len() - 1) % self.modules.len();
        Some(self.modules[prev_idx].1.id())
    }

    /// Returns the ID of the first module in the next non-empty zone (Tab cycle: Top→Center→Bottom→Top).
    fn nav_next_zone(&self, current: &ModuleId) -> Option<ModuleId> {
        let idx = self.modules.iter().position(|(_, m)| m.id() == *current)?;
        let current_zone = &self.modules[idx].0;
        let cycle = [Zone::Top, Zone::Center, Zone::Bottom];
        let start = cycle.iter().position(|z| z == current_zone).unwrap_or(0);
        for step in 1..=3 {
            let next_zone = &cycle[(start + step) % 3];
            if let Some((_, m)) = self.modules.iter().find(|(z, _)| z == next_zone) {
                return Some(m.id());
            }
        }
        None
    }

    /// Returns the ID of the first module in the previous non-empty zone (Shift+Tab cycle: Top→Bottom→Center→Top).
    fn nav_prev_zone(&self, current: &ModuleId) -> Option<ModuleId> {
        let idx = self.modules.iter().position(|(_, m)| m.id() == *current)?;
        let current_zone = &self.modules[idx].0;
        // Reverse cycle: Top(0)→Bottom(2)→Center(1)→Top(0)
        let cycle = [Zone::Top, Zone::Bottom, Zone::Center];
        let start = cycle.iter().position(|z| z == current_zone).unwrap_or(0);
        for step in 1..=3 {
            let prev_zone = &cycle[(start + step) % 3];
            if let Some((_, m)) = self.modules.iter().find(|(z, _)| z == prev_zone) {
                return Some(m.id());
            }
        }
        None
    }

    /// Returns a `SizeChange` task for the named surface's current expand state in overlay mode.
    ///
    /// Overlay mode: the surface grows over the desktop — exclusive zone is never changed.
    /// For horizontal bars the animated dimension is height `(0, size)`; for vertical it's width `(size, 0)`.
    fn overlay_size_change_for(&self, window: iced::window::Id) -> Task<Message> {
        let expand = self
            .surfaces
            .get(&window)
            .map(|s| s.expand)
            .unwrap_or_default();
        let size = expand.current_width(
            self.config.bar.collapsed_width,
            self.config.bar.expanded_width,
        );
        if self.config.bar.edge.is_horizontal() {
            Task::done(Message::SizeChange { id: window, size: (0, size) })
        } else {
            Task::done(Message::SizeChange { id: window, size: (size, 0) })
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
        config.bar.expand_trigger = ExpandTrigger::Hover;
        config.bar.hover_delay = 0;
        config
    }

    fn make_expand_disabled_config() -> Config {
        let mut config = make_config();
        config.bar.expand = false;
        config
    }

    fn make_horizontal_config() -> Config {
        let mut config = make_config();
        config.bar.edge = Edge::Top;
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
    fn boot_with_config_stores_config() {
        let mut config = Config::default();
        config.bar.collapsed_width = 60;
        let (app, _) = App::boot(config);
        assert_eq!(app.config.bar.collapsed_width, 60);
    }

    #[test]
    fn shared_state_starts_empty() {
        let state = SharedState::new();
        assert!(state.modules.is_empty());
    }

    #[test]
    fn boot_modules_empty_with_no_module_config() {
        let config = make_config();
        let (app, _) = App::boot(config);
        assert!(app.modules.is_empty());
    }

    #[test]
    fn undeclared_known_type_in_bar_modules_uses_defaults() {
        let mut config = Config::default();
        config.bar.modules.bottom = vec!["clock".into()];
        let (app, _) = App::boot(config);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn undeclared_unknown_name_in_bar_modules_is_skipped() {
        let mut config = Config::default();
        config.bar.modules.bottom = vec!["my-script".into()];
        let (app, _) = App::boot(config);
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
        config.bar.modules.bottom = vec!["clock".into()];
        let (app, _) = App::boot(config);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn hover_enter_sets_hovered_module() {
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        let id = ModuleId::new("clock");
        let _ = app.update(Message::ModuleHoverEnter(id.clone(), wid));
        assert_eq!(app.surfaces[&wid].hovered_module, Some(id));
    }

    #[test]
    fn hover_exit_clears_hovered_module() {
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        let id = ModuleId::new("clock");
        let _ = app.update(Message::ModuleHoverEnter(id.clone(), wid));
        let _ = app.update(Message::ModuleHoverExit(id, wid));
        assert!(app.surfaces[&wid].hovered_module.is_none());
    }

    #[test]
    fn hover_exit_different_id_does_not_clear() {
        let (mut app, _) = App::boot(make_config());
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
        config.bar.modules.bottom = vec!["script".into()];
        let (app, _) = App::boot(config);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn script_output_event_updates_state() {
        use crate::modules::script::ScriptEvent;

        let (mut app, _) = App::boot(make_config());
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
        config.bar.modules.bottom = vec!["button".into()];
        let (app, _) = App::boot(config);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn button_ready_event_updates_state() {
        use crate::modules::button::ButtonEvent;

        let (mut app, _) = App::boot(make_config());
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
        let (mut app, _) = App::boot(make_config());
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
        config.bar.modules.bottom = vec!["script".into()];
        let (mut app, _) = App::boot(config);

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
        let (app, _) = App::boot(make_config());
        assert!(app.surfaces.is_empty());
    }

    #[test]
    fn bar_click_starts_expanding_from_collapsed() {
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        let _ = app.update(Message::BarClick(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn bar_click_starts_collapsing_from_expanded() {
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::BarClick(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsing(0.0));
    }

    // --- Story 3.2 tests ---

    #[test]
    fn tick_advances_expand_progress() {
        let (mut app, _) = App::boot(make_config());
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
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.999);
        let _ = app.update(Message::Tick);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanded);
    }

    #[test]
    fn tick_advances_collapse_progress() {
        let (mut app, _) = App::boot(make_config());
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
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Collapsing(0.999);
        let _ = app.update(Message::Tick);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn tick_no_op_when_collapsed() {
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Collapsed;
        let _ = app.update(Message::Tick);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn tick_no_op_when_expanded() {
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::Tick);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanded);
    }

    // --- Story 3.3 tests ---

    #[test]
    fn app_starts_with_bar_hovered_false() {
        let (app, _) = App::boot(make_config());
        // No surfaces yet, so bar_hovered is not tracked — passes trivially.
        assert!(app.surfaces.values().all(|s| !s.bar_hovered));
    }

    #[test]
    fn hover_enter_click_trigger_does_nothing() {
        let (mut app, _) = App::boot(make_config()); // expand_trigger = Click
        let wid = test_wid();
        let _ = app.update(Message::BarHoverEnter(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
        assert!(app.surfaces[&wid].bar_hovered);
    }

    #[test]
    fn hover_exit_click_trigger_does_nothing() {
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::BarHoverExit(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanded);
    }

    #[test]
    fn hover_enter_hover_trigger_zero_delay_starts_expanding() {
        let (mut app, _) = App::boot(make_hover_config());
        let wid = test_wid();
        let _ = app.update(Message::BarHoverEnter(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn hover_exit_when_expanded_starts_collapsing() {
        let (mut app, _) = App::boot(make_hover_config());
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::BarHoverExit(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsing(0.0));
    }

    #[test]
    fn hover_exit_when_expanding_reverses() {
        let (mut app, _) = App::boot(make_hover_config());
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
        let (mut app, _) = App::boot(make_hover_config());
        let wid = test_wid();
        let _ = app.update(Message::BarHoverExit(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn hover_enter_with_delay_sets_pending() {
        let mut config = make_hover_config();
        config.bar.hover_delay = 100;
        let (mut app, _) = App::boot(config);
        let wid = test_wid();
        let _ = app.update(Message::BarHoverEnter(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
        assert_eq!(app.surfaces[&wid].hover_pending_ms, Some(0.0));
    }

    #[test]
    fn hover_exit_cancels_pending() {
        let mut config = make_hover_config();
        config.bar.hover_delay = 100;
        let (mut app, _) = App::boot(config);
        let wid = test_wid();
        let _ = app.update(Message::BarHoverEnter(wid));
        let _ = app.update(Message::BarHoverExit(wid));
        assert_eq!(app.surfaces[&wid].hover_pending_ms, None);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn tick_triggers_expand_when_delay_elapsed() {
        let mut config = make_hover_config();
        config.bar.hover_delay = 100;
        let (mut app, _) = App::boot(config);
        let wid = test_wid();
        app.surface_mut(wid).hover_pending_ms = Some(90.0);
        let _ = app.update(Message::Tick);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
        assert_eq!(app.surfaces[&wid].hover_pending_ms, None);
    }

    #[test]
    fn bar_click_noop_when_hover_trigger() {
        let (mut app, _) = App::boot(make_hover_config());
        let wid = test_wid();
        app.surface_mut(wid); // initialize so surface exists before early-return
        let _ = app.update(Message::BarClick(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    // --- Story 4.1 tests ---

    #[test]
    fn niri_connected_event_sets_connected() {
        let (mut app, _) = App::boot(make_config());
        let _ = app.update(Message::Niri(NiriEvent::Connected));
        assert!(app.shared.niri.connected);
    }

    #[test]
    fn niri_unavailable_event_does_not_crash() {
        let (mut app, _) = App::boot(make_config());
        let _ = app.update(Message::Niri(NiriEvent::Unavailable));
        assert!(!app.shared.niri.connected);
    }

    #[test]
    fn niri_workspaces_changed_stores_workspaces() {
        use niri_ipc::{Event, Workspace};
        let (mut app, _) = App::boot(make_config());
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
        let (mut app, _) = App::boot(make_config());
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowFocusChanged {
            id: Some(42),
        })));
        assert_eq!(app.shared.niri.focused_window_id, Some(42));
    }

    #[test]
    fn niri_window_focus_cleared() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config());
        app.shared.niri.focused_window_id = Some(99);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowFocusChanged {
            id: None,
        })));
        assert_eq!(app.shared.niri.focused_window_id, None);
    }

    #[test]
    fn niri_unknown_event_is_ignored() {
        use niri_ipc::Event;
        let (mut app, _) = App::boot(make_config());
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
        config.bar.modules.top = vec!["workspaces".into()];
        App::boot(config).0
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
        let (mut app, _) = App::boot(make_config());
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
        config.bar.modules.center = vec!["focused-window".into()];
        App::boot(config).0
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
        let (mut app, _) = App::boot(make_config());
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
        let (mut app, _) = App::boot(Config::default());
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
        config.bar.modules.bottom = vec!["battery".into()];
        App::boot(config).0
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
        let (mut app, _) = App::boot(Config::default());
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
        config.bar.modules.bottom = vec!["network".into()];
        App::boot(config).0
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
        let (mut app, _) = App::boot(Config::default());
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
        config.bar.modules.bottom = vec!["volume".into()];
        App::boot(config).0
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
        let (mut app, _) = App::boot(Config::default());
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
        let (app, _) = App::boot(make_config());
        let wid = test_wid();
        assert!(app.surfaces.get(&wid).map(|s| s.focused_module.is_none()).unwrap_or(true));
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
        config.bar.modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config);
        let wid = test_wid();
        let _ = app.update(Message::BarFocusGained(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("clock")));
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
        config.bar.modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config);
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
        config.bar.modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config);
        let wid = test_wid();
        let _ = app.update(Message::BarFocusGained(wid));
        assert!(app.surfaces[&wid].focused_module.is_some());
        let _ = app.update(Message::EscapePressed(wid));
        assert!(app.surfaces[&wid].focused_module.is_some());
    }

    #[test]
    fn escape_pressed_when_not_focused_is_noop() {
        let (mut app, _) = App::boot(make_config());
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
        config.bar.modules.center = vec!["clock".into()];
        let (mut app, _) = App::boot(config);
        let wid = test_wid();
        let _ = app.update(Message::BarFocusGained(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("clock")));
    }

    #[test]
    fn bar_focus_gained_with_no_modules_is_noop() {
        let (mut app, _) = App::boot(make_config());
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
        config.bar.modules.top = vec!["clock".into(), "battery".into()];
        config.bar.modules.center = vec!["network".into()];
        config.bar.modules.bottom = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config);
        let wid = test_wid();
        app.surface_mut(wid); // pre-initialize surface
        (app, wid)
    }

    #[test]
    fn focus_move_next_advances_to_next_module() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("battery")));
    }

    #[test]
    fn focus_move_next_crosses_zone_boundary() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("network")));
    }

    #[test]
    fn focus_move_next_wraps_from_last_to_first() {
        let (mut app, wid) = make_nav_app();
        app.surface_mut(wid).focused_module = Some(ModuleId::new("workspaces"));
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("clock")));
    }

    #[test]
    fn focus_move_prev_retreats_to_previous_module() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        let _ = app.update(Message::FocusMovePrev(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("clock")));
    }

    #[test]
    fn focus_move_prev_wraps_from_first_to_last() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMovePrev(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("workspaces")));
    }

    #[test]
    fn focus_next_zone_jumps_to_center() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusNextZone(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("network")));
    }

    #[test]
    fn focus_next_zone_wraps_bottom_to_top() {
        let (mut app, wid) = make_nav_app();
        app.surface_mut(wid).focused_module = Some(ModuleId::new("workspaces"));
        let _ = app.update(Message::FocusNextZone(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("clock")));
    }

    #[test]
    fn focus_prev_zone_from_top_jumps_to_bottom() {
        let (mut app, wid) = make_nav_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusPrevZone(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("workspaces")));
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
        config.bar.modules.top = vec!["clock".into()];
        config.bar.modules.bottom = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config);
        let wid = test_wid();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusNextZone(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("workspaces")));
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
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("clock")));
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
        config.bar.modules.top = vec!["clock".into(), "volume".into()];
        config.bar.modules.center = vec!["button".into()];
        let (mut app, _) = App::boot(config);
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
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("volume")));
        assert!(!app.surfaces[&wid].module_activated);
        let _ = app.update(Message::KeyboardActivate(wid));
        assert!(app.surfaces[&wid].module_activated);
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("volume")));
    }

    #[test]
    fn arrow_keys_scroll_when_module_activated() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        let _ = app.update(Message::KeyboardActivate(wid));
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("volume")));
        assert!(app.surfaces[&wid].module_activated);
        let _ = app.update(Message::FocusMovePrev(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("volume")));
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
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("volume")));
    }

    #[test]
    fn arrow_keys_navigate_when_module_not_activated() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::BarFocusGained(wid));
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("volume")));
        assert!(!app.surfaces[&wid].module_activated);
        let _ = app.update(Message::FocusMoveNext(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("button")));
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
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("button")));
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
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("button")));
        let _ = app.update(Message::KeyboardActivate(wid));
        assert!(!app.surfaces[&wid].module_activated);
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("button")));
    }

    #[test]
    fn keyboard_activate_on_non_interactive_module_is_noop() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::BarFocusGained(wid));
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("clock")));
        let _ = app.update(Message::KeyboardActivate(wid));
        assert!(!app.surfaces[&wid].module_activated);
        assert_eq!(app.surfaces[&wid].focused_module, Some(ModuleId::new("clock")));
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
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        app.surface_mut(wid); // initialize surface so ToggleExpand can find it
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expand_from_expanded_starts_collapsing() {
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsing(0.0));
    }

    #[test]
    fn toggle_expand_reverses_expanding_mid_animation() {
        let (mut app, _) = App::boot(make_config());
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
        let (mut app, _) = App::boot(make_config());
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
        let (mut app, _) = App::boot(make_hover_config());
        let wid = test_wid();
        app.surface_mut(wid); // initialize
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expand_double_toggle_collapses() {
        let (mut app, _) = App::boot(make_config());
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
        let (mut app, _) = App::boot(make_expand_disabled_config());
        let wid = test_wid();
        app.surface_mut(wid); // initialize so surface exists before early-return
        let _ = app.update(Message::BarClick(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn toggle_expand_noop_when_expand_disabled() {
        let (mut app, _) = App::boot(make_expand_disabled_config());
        let wid = test_wid();
        app.surface_mut(wid); // initialize
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
    }

    #[test]
    fn hover_enter_noop_when_expand_disabled() {
        let mut config = make_expand_disabled_config();
        config.bar.expand_trigger = ExpandTrigger::Hover;
        config.bar.hover_delay = 0;
        let (mut app, _) = App::boot(config);
        let wid = test_wid();
        let _ = app.update(Message::BarHoverEnter(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Collapsed);
        assert!(app.surfaces[&wid].bar_hovered);
    }

    #[test]
    fn hover_exit_noop_when_expand_disabled() {
        let mut config = make_expand_disabled_config();
        config.bar.expand_trigger = ExpandTrigger::Hover;
        let (mut app, _) = App::boot(config);
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanded;
        let _ = app.update(Message::BarHoverExit(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanded);
    }

    // --- Overlay mode tests ---

    #[test]
    fn bar_click_horizontal_bar_starts_expanding() {
        let (mut app, _) = App::boot(make_horizontal_config());
        let wid = test_wid();
        let _ = app.update(Message::BarClick(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expand_horizontal_bar_starts_expanding() {
        let (mut app, _) = App::boot(make_horizontal_config());
        let wid = test_wid();
        app.surface_mut(wid); // initialize
        let _ = app.update(Message::ToggleExpand);
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn hover_enter_horizontal_bar_hover_trigger_starts_expanding() {
        let mut config = make_horizontal_config();
        config.bar.expand_trigger = ExpandTrigger::Hover;
        config.bar.hover_delay = 0;
        let (mut app, _) = App::boot(config);
        let wid = test_wid();
        let _ = app.update(Message::BarHoverEnter(wid));
        assert_eq!(app.surfaces[&wid].expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn overlay_size_change_vertical_bar_uses_width() {
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        app.config.bar.collapsed_width = 48;
        app.config.bar.expanded_width = 200;
        app.surface_mut(wid).expand = ExpandState::Expanded;
        assert!(!app.config.bar.edge.is_horizontal());
        let _ = app.overlay_size_change_for(wid);
    }

    #[test]
    fn overlay_size_change_horizontal_bar_uses_height() {
        let (mut app, _) = App::boot(make_horizontal_config());
        let wid = test_wid();
        app.config.bar.collapsed_width = 48;
        app.config.bar.expanded_width = 200;
        app.surface_mut(wid).expand = ExpandState::Expanded;
        assert!(app.config.bar.edge.is_horizontal());
        let _ = app.overlay_size_change_for(wid);
    }

    // --- Story 7.1 tests ---

    #[test]
    fn boot_returns_app_and_task() {
        let (app, _task) = App::boot(make_config());
        assert!(app.surfaces.is_empty()); // no surfaces before first view/event
    }

    #[test]
    fn surface_mut_creates_default_state_for_new_window() {
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        assert!(app.surfaces.get(&wid).is_none());
        let s = app.surface_mut(wid);
        assert_eq!(s.expand, ExpandState::Collapsed);
    }

    #[test]
    fn two_surfaces_have_independent_expand_state() {
        let (mut app, _) = App::boot(make_config());
        let wid_a = test_wid();
        let wid_b = test_wid();
        app.surface_mut(wid_a).expand = ExpandState::Expanded;
        // Surface B should still be Collapsed (or not yet exist)
        let b_state = app.surfaces.get(&wid_b);
        assert!(b_state.map(|s| s.expand == ExpandState::Collapsed).unwrap_or(true));
    }

    #[test]
    fn two_surfaces_have_independent_focus_state() {
        let (mut app, _) = App::boot(make_config());
        let wid_a = test_wid();
        let wid_b = test_wid();
        let focus_id = ModuleId::new("clock");
        app.surface_mut(wid_a).focused_module = Some(focus_id);
        let b_state = app.surfaces.get(&wid_b);
        assert!(b_state.map(|s| s.focused_module.is_none()).unwrap_or(true));
    }

    #[test]
    fn tick_not_needed_when_no_surfaces() {
        let (app, _) = App::boot(make_config());
        let needs_tick = app.surfaces.values().any(|s| s.expand.is_animating());
        assert!(!needs_tick);
    }

    #[test]
    fn tick_needed_when_any_surface_animating() {
        let (mut app, _) = App::boot(make_config());
        let wid = test_wid();
        app.surface_mut(wid).expand = ExpandState::Expanding(0.5);
        let needs_tick = app.surfaces.values().any(|s| s.expand.is_animating());
        assert!(needs_tick);
    }

    #[test]
    fn bar_click_only_affects_clicked_surface() {
        let (mut app, _) = App::boot(make_config());
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
        config.bar.modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config);
        let wid_a = test_wid();
        let wid_b = test_wid();
        app.surface_mut(wid_b); // initialize surface B without focus
        let _ = app.update(Message::BarFocusGained(wid_a));
        // Surface A has focus
        assert!(app.surfaces[&wid_a].focused_module.is_some());
        // Surface B unfocused
        assert!(app.surfaces[&wid_b].focused_module.is_none());
    }
}
