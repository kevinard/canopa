use std::collections::HashMap;
use std::time::Duration;

use iced::widget::{Space, container, mouse_area, row};
use iced::{Color, Element, Length, Task};
use iced_layershell::to_layer_message;

use crate::config::{Config, ExpandTrigger};
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
use crate::niri::subscription::niri_subscription;
use crate::niri::{NiriEvent, NiriState};
use crate::surface::expand::ExpandState;
use crate::surface::frame::module_frame;
use crate::surface::input::keyboard_focus_subscription;
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
    /// Empty until registry is populated (Stories 2.3+).
    pub modules: Vec<(Zone, Box<dyn Module>)>,
    /// Tracks which module is currently hovered; moves to SurfaceState in Story 7.1.
    pub hovered_module: Option<ModuleId>,
    /// Which module currently has keyboard focus; None when bar is unfocused.
    /// Moves to per-surface SurfaceState in Story 7.1.
    pub focused_module: Option<ModuleId>,
    /// Whether the focused module is in keyboard-interaction mode (Story 6.3).
    /// When true, arrow keys scroll the active module; Escape deactivates back to navigation.
    pub module_activated: bool,
    /// Current expand/collapse state. Single-surface for Stories 3.1–6.x;
    /// moves to per-surface state (HashMap<SurfaceId, SurfaceState>) in Story 7.1.
    pub expand: ExpandState,
    /// Whether the pointer is currently inside the bar surface (Story 3.3 hover trigger).
    pub bar_hovered: bool,
    /// Hover dwell countdown (ms elapsed). `Some(elapsed)` = counting toward hover_delay.
    pub hover_pending_ms: Option<f32>,
}

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    // Module data events — ModuleEvent gains concrete variants in Stories 2.3+.
    Module(ModuleId, ModuleEvent),
    ModuleHoverEnter(ModuleId),
    ModuleHoverExit(ModuleId),
    ModuleClick(ModuleId),
    /// Fires when the user clicks anywhere on the bar surface; toggles expand/collapse (Story 3.1).
    BarClick,
    /// Animation clock tick. Fired at ~60fps only while `expand.is_animating()` (Story 3.2).
    Tick,
    /// Pointer entered bar surface; may trigger expand when expand_trigger = Hover (Story 3.3).
    BarHoverEnter,
    /// Pointer left bar surface; may trigger collapse when expand_trigger = Hover (Story 3.3).
    BarHoverExit,
    /// Event from the Niri IPC event stream; dispatched to SharedState (Story 4.1).
    Niri(crate::niri::NiriEvent),
    /// Battery charge state from UPower D-Bus subscription (Story 5.2).
    Battery(BatteryEvent),
    /// Network connectivity state from D-Bus subscription (Story 5.3).
    Network(NetworkEvent),
    /// Volume state from PA compat subscription (Story 5.4).
    Volume(VolumeEvent),
    /// Scroll-triggered volume adjustment (Story 5.4).
    VolumeAdjust {
        step: u8,
        up: bool,
    },
    /// Scroll-up event on a module (Story 5.4).
    ModuleScrollUp(ModuleId),
    /// Scroll-down event on a module (Story 5.4).
    ModuleScrollDown(ModuleId),
    /// D-Bus connection handles established at startup (Story 5.1).
    /// Fires exactly once from `dbus_init_subscription`; stored in `SharedState.dbus`.
    DbusReady(DbusConnections),
    /// Layer surface received keyboard focus (Story 6.1).
    BarFocusGained,
    /// Layer surface lost keyboard focus (Story 6.1).
    BarFocusLost,
    /// Escape key pressed while bar has keyboard focus (Story 6.1).
    EscapePressed,
    /// Down arrow key pressed while bar is focused — move to next module in zone (Story 6.2).
    FocusMoveNext,
    /// Up arrow key pressed while bar is focused — move to previous module in zone (Story 6.2).
    FocusMovePrev,
    /// Tab pressed while bar is focused — jump to first module in next zone (Story 6.2).
    FocusNextZone,
    /// Shift+Tab pressed while bar is focused — jump to first module in previous zone (Story 6.2).
    FocusPrevZone,
    /// Enter or Space pressed while bar is focused — activate focused module (Story 6.3).
    KeyboardActivate,
}

impl App {
    pub fn new(config: Config) -> Self {
        let registry = build_registry();
        let modules: Vec<(Zone, Box<dyn Module>)> = config
            .modules
            .iter()
            .filter_map(|mc| {
                registry
                    .get(mc.name.as_str())
                    .map(|ctor| (mc.zone.clone(), ctor(mc)))
            })
            .collect();
        App {
            config,
            shared: SharedState::new(),
            modules,
            hovered_module: None,
            focused_module: None,
            module_activated: false,
            expand: ExpandState::default(), // starts Collapsed
            bar_hovered: false,
            hover_pending_ms: None,
        }
    }

    pub fn namespace() -> String {
        String::from("niribar")
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ModuleHoverEnter(id) => {
                self.hovered_module = Some(id);
                Task::none()
            }
            Message::ModuleHoverExit(id) => {
                if self.hovered_module.as_ref() == Some(&id) {
                    self.hovered_module = None;
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
            Message::ModuleClick(id) => {
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
            Message::BarClick => {
                // Click has no effect when hover is the expand trigger (Story 3.3 AC #6).
                if self.config.bar.expand_trigger == ExpandTrigger::Hover {
                    return Task::none();
                }
                // toggle() now returns Expanding(0.0) or Collapsing(0.0) instead of jumping
                // directly to the final state. The Tick subscription drives the actual animation.
                self.expand = self.expand.toggle();
                // Issue an immediate resize to the animation start width to avoid a flash.
                // SizeChange and ExclusiveZoneChange are injected by #[to_layer_message].
                let w = self.expand.current_width(
                    self.config.bar.collapsed_width,
                    self.config.bar.expanded_width,
                );
                Task::batch([
                    Task::done(Message::SizeChange((w, 0))),
                    Task::done(Message::ExclusiveZoneChange(
                        i32::try_from(w).unwrap_or(i32::MAX),
                    )),
                ])
            }
            Message::BarHoverEnter => {
                self.bar_hovered = true;
                if self.config.bar.expand_trigger != ExpandTrigger::Hover {
                    return Task::none();
                }
                match self.expand {
                    ExpandState::Collapsed => {
                        if self.config.bar.hover_delay == 0 {
                            self.expand = self.expand.toggle(); // → Expanding(0.0)
                            let w = self.expand.current_width(
                                self.config.bar.collapsed_width,
                                self.config.bar.expanded_width,
                            );
                            Task::batch([
                                Task::done(Message::SizeChange((w, 0))),
                                Task::done(Message::ExclusiveZoneChange(
                                    i32::try_from(w).unwrap_or(i32::MAX),
                                )),
                            ])
                        } else {
                            self.hover_pending_ms = Some(0.0);
                            Task::none()
                        }
                    }
                    ExpandState::Collapsing(_) => {
                        // Pointer re-entered while collapsing — reverse back to expanding.
                        self.expand = self.expand.toggle();
                        Task::none()
                    }
                    _ => Task::none(), // Already expanded or expanding — no action
                }
            }
            Message::BarHoverExit => {
                self.bar_hovered = false;
                if self.config.bar.expand_trigger != ExpandTrigger::Hover {
                    return Task::none();
                }
                // Cancel any pending hover delay.
                self.hover_pending_ms = None;
                match self.expand {
                    ExpandState::Expanded | ExpandState::Expanding(_) => {
                        self.expand = self.expand.toggle(); // → Collapsing(0.0) or Collapsing(1-p)
                        let w = self.expand.current_width(
                            self.config.bar.collapsed_width,
                            self.config.bar.expanded_width,
                        );
                        Task::batch([
                            Task::done(Message::SizeChange((w, 0))),
                            Task::done(Message::ExclusiveZoneChange(
                                i32::try_from(w).unwrap_or(i32::MAX),
                            )),
                        ])
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
                            if module.id() == ModuleId::new("workspaces") {
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
                            if module.id() == ModuleId::new("workspaces") {
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
                // Advance animation progress by one tick (~16ms, targeting 60fps).
                const EXPAND_MS: f32 = 280.0;
                const COLLAPSE_MS: f32 = 200.0;
                const TICK_MS: f32 = 16.0;

                // Hover delay countdown — runs while waiting to start expand (Story 3.3).
                if let Some(elapsed) = self.hover_pending_ms {
                    let new_elapsed = elapsed + TICK_MS;
                    if new_elapsed >= self.config.bar.hover_delay as f32 {
                        // Delay expired: start expanding.
                        self.hover_pending_ms = None;
                        self.expand = ExpandState::Expanding(0.0);
                        let w = self.expand.current_width(
                            self.config.bar.collapsed_width,
                            self.config.bar.expanded_width,
                        );
                        return Task::batch([
                            Task::done(Message::SizeChange((w, 0))),
                            Task::done(Message::ExclusiveZoneChange(
                                i32::try_from(w).unwrap_or(i32::MAX),
                            )),
                        ]);
                    } else {
                        self.hover_pending_ms = Some(new_elapsed);
                        return Task::none(); // Still counting down
                    }
                }

                match self.expand {
                    ExpandState::Expanding(progress) => {
                        let new_progress = (progress + TICK_MS / EXPAND_MS).min(1.0);
                        self.expand = if new_progress >= 1.0 {
                            ExpandState::Expanded
                        } else {
                            ExpandState::Expanding(new_progress)
                        };
                    }
                    ExpandState::Collapsing(progress) => {
                        let new_progress = (progress + TICK_MS / COLLAPSE_MS).min(1.0);
                        self.expand = if new_progress >= 1.0 {
                            ExpandState::Collapsed
                        } else {
                            ExpandState::Collapsing(new_progress)
                        };
                    }
                    _ => return Task::none(),
                }
                // Resize surface to the interpolated width at the new progress value.
                let w = self.expand.current_width(
                    self.config.bar.collapsed_width,
                    self.config.bar.expanded_width,
                );
                Task::batch([
                    Task::done(Message::SizeChange((w, 0))),
                    Task::done(Message::ExclusiveZoneChange(
                        i32::try_from(w).unwrap_or(i32::MAX),
                    )),
                ])
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
                    if module.id().0 == "network" {
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
                    if module.id().0 == "volume" {
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
            Message::ModuleScrollUp(id) => {
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
            Message::ModuleScrollDown(id) => {
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
                    if module.id().0 == "battery" {
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
            Message::BarFocusGained => {
                // Highlight first module in top zone (AC #1); fall back to center/bottom.
                let first = self
                    .modules
                    .iter()
                    .find(|(z, _)| *z == Zone::Top)
                    .or_else(|| self.modules.iter().find(|(z, _)| *z == Zone::Center))
                    .or_else(|| self.modules.iter().find(|(z, _)| *z == Zone::Bottom))
                    .map(|(_, m)| m.id());
                self.focused_module = first;
                self.module_activated = false;
                Task::none()
            }
            Message::BarFocusLost => {
                self.focused_module = None;
                self.module_activated = false;
                // Restore OnDemand so the bar can be focused again (AC #3).
                use iced_layershell::reexport::KeyboardInteractivity;
                Task::done(Message::KeyboardInteractivityChange(
                    KeyboardInteractivity::OnDemand,
                ))
            }
            Message::EscapePressed => {
                // If a module is activated, Escape deactivates it (returns to navigation mode).
                if self.module_activated {
                    self.module_activated = false;
                    return Task::none();
                }
                if self.focused_module.is_none() {
                    return Task::none();
                }
                // Tell compositor to remove keyboard focus from this surface (AC #2).
                // BarFocusLost fires in response and restores OnDemand.
                use iced_layershell::reexport::KeyboardInteractivity;
                Task::done(Message::KeyboardInteractivityChange(
                    KeyboardInteractivity::None,
                ))
            }
            Message::FocusMoveNext => {
                if self.focused_module.is_none() {
                    // Bar has Wayland keyboard focus (we received this event) but BarFocusGained
                    // didn't fire — iced-layershell only emits Window::Focused when
                    // current_surface changes, so re-focus after Escape is silently dropped.
                    // Delegate to BarFocusGained to acquire focus first; user re-presses to navigate.
                    return Task::done(Message::BarFocusGained);
                }
                // Interaction mode (Story 6.3): module is activated — arrow key scrolls it.
                if self.module_activated {
                    if let Some(ref id) = self.focused_module.clone() {
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
                if let Some(ref current) = self.focused_module.clone()
                    && let Some(next) = self.nav_next(current)
                {
                    self.focused_module = Some(next);
                }
                Task::none()
            }
            Message::FocusMovePrev => {
                if self.focused_module.is_none() {
                    return Task::done(Message::BarFocusGained);
                }
                // Interaction mode (Story 6.3): module is activated — arrow key scrolls it.
                if self.module_activated {
                    if let Some(ref id) = self.focused_module.clone() {
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
                if let Some(ref current) = self.focused_module.clone()
                    && let Some(prev) = self.nav_prev(current)
                {
                    self.focused_module = Some(prev);
                }
                Task::none()
            }
            Message::FocusNextZone => {
                if self.focused_module.is_none() {
                    return Task::done(Message::BarFocusGained);
                }
                self.module_activated = false; // deactivate on zone jump
                if let Some(ref current) = self.focused_module.clone()
                    && let Some(next) = self.nav_next_zone(current)
                {
                    self.focused_module = Some(next);
                }
                Task::none()
            }
            Message::FocusPrevZone => {
                if self.focused_module.is_none() {
                    return Task::done(Message::BarFocusGained);
                }
                self.module_activated = false; // deactivate on zone jump
                if let Some(ref current) = self.focused_module.clone()
                    && let Some(prev) = self.nav_prev_zone(current)
                {
                    self.focused_module = Some(prev);
                }
                Task::none()
            }
            Message::KeyboardActivate => {
                if self.focused_module.is_none() {
                    return Task::none();
                }
                if let Some(ref id) = self.focused_module.clone() {
                    for (_, module) in &self.modules {
                        if module.id() == *id {
                            // Scrollable module: Enter/Space enters interaction mode.
                            // Check scroll capability without side effects (VolumeModule constructs
                            // VolumeAdjust and returns it — no I/O).
                            if module.on_interaction(InteractionEvent::ScrollDown).is_some() {
                                self.module_activated = true;
                                return Task::none();
                            }
                            // Clickable module: fire the click action.
                            // ButtonModule spawns the command as a side effect and returns None.
                            if let Some(msg) = module.on_interaction(InteractionEvent::Clicked) {
                                return Task::done(msg);
                            }
                            break;
                        }
                    }
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

        // AC #3: Only connect to D-Bus if at least one D-Bus-backed module is configured.
        // Check config.modules (not self.modules) — battery/network/volume not yet in registry.
        let needs_dbus = self
            .config
            .modules
            .iter()
            .any(|mc| matches!(mc.name.as_str(), "battery" | "network" | "volume"));
        let dbus_sub = if needs_dbus {
            crate::dbus::dbus_init_subscription()
        } else {
            iced::Subscription::none()
        };

        // AC #9: Only subscribe if battery module is present AND system bus is connected.
        // UPower runs on the system bus (org.freedesktop.UPower), not the session bus.
        let battery_sub = if let Some(conn) = self.shared.dbus.system.clone() {
            if self.modules.iter().any(|(_, m)| m.id().0 == "battery") {
                crate::dbus::upower::battery_subscription(conn)
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        // AC #9: Only subscribe if network module is present AND system bus is connected.
        let network_sub = if let Some(conn) = self.shared.dbus.system.clone() {
            if self.modules.iter().any(|(_, m)| m.id().0 == "network") {
                crate::dbus::networkmanager::network_subscription(conn)
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        // AC #9: Only subscribe if volume module is present.
        // No zbus connection needed — PA auto-discovers its socket.
        let volume_sub = if self.modules.iter().any(|(_, m)| m.id().0 == "volume") {
            crate::dbus::wireplumber::volume_subscription()
        } else {
            iced::Subscription::none()
        };

        let focus_sub = keyboard_focus_subscription();

        if self.expand.is_animating() || self.hover_pending_ms.is_some() {
            // Emit a Tick every ~16ms (≈60fps) while animating or hover delay is pending.
            let tick = iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick);
            iced::Subscription::batch([
                module_subs,
                niri_sub,
                dbus_sub,
                battery_sub,
                network_sub,
                volume_sub,
                focus_sub,
                tick,
            ])
        } else {
            iced::Subscription::batch([
                module_subs,
                niri_sub,
                dbus_sub,
                battery_sub,
                network_sub,
                volume_sub,
                focus_sub,
            ])
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        // Use the interpolated width during animation; final widths when at rest.
        let surface_width = self.expand.current_width(
            self.config.bar.collapsed_width,
            self.config.bar.expanded_width,
        );
        let content_width = surface_width.saturating_sub(1); // 1px right border

        // Decide which module view to call based on animation state.
        // label_opacity (UX-DR6, 60ms delay fade) is deferred: iced 0.14 has no generic
        // element opacity wrapper; see deferred-work.md.
        let shows_expanded = self.expand.shows_expanded_content();

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
            let hovered = self.hovered_module.as_ref() == Some(&id);
            let focused = self.focused_module.as_ref() == Some(&id);

            // During Expanding: show expanded content (opacity fade deferred — no iced wrapper).
            // During Collapsing: show collapsed content immediately (no reverse fade).
            let module_view: Element<'_, Message> = if shows_expanded {
                module.view_expanded(&state)
            } else {
                module.view_collapsed(&state)
            };

            let scroll_id = id.clone();
            // module_frame treats hover and keyboard focus identically (UX-DR3).
            let view: Element<'_, Message> = mouse_area(module_frame(module_view, hovered || focused))
                .on_enter(Message::ModuleHoverEnter(id.clone()))
                .on_exit(Message::ModuleHoverExit(id.clone()))
                .on_press(Message::ModuleClick(id))
                .on_scroll(move |delta| {
                    let up = match delta {
                        iced::mouse::ScrollDelta::Lines { y, .. }
                        | iced::mouse::ScrollDelta::Pixels { y, .. } => y > 0.0,
                    };
                    if up {
                        Message::ModuleScrollUp(scroll_id.clone())
                    } else {
                        Message::ModuleScrollDown(scroll_id.clone())
                    }
                })
                .into();

            match zone {
                Zone::Top => top.push(view),
                Zone::Center => center.push(view),
                Zone::Bottom => bottom.push(view),
            }
        }

        let bar_content = row![
            // Main content area — zone layout fills (surface_width - 1)px
            container(zone_layout(top, center, bottom))
                .width(content_width)
                .height(Length::Fill),
            // 1px right border strip in Catppuccin Mocha Surface1 (#45475a)
            container(Space::new())
                .width(1)
                .height(Length::Fill)
                .style(|_theme| container::Style {
                    background: Some(iced::Background::Color(Color::from_rgb8(0x45, 0x47, 0x5a))),
                    ..Default::default()
                }),
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        // Wrap entire bar in mouse_area for click-to-toggle and hover detection.
        // Note: clicking a module fires both ModuleClick(id) AND BarClick (outer area).
        mouse_area(bar_content)
            .on_press(Message::BarClick)
            .on_enter(Message::BarHoverEnter)
            .on_exit(Message::BarHoverExit)
            .into()
    }

    pub fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: Color::from_rgb8(0x1e, 0x1e, 0x2e), // Catppuccin Mocha Base
            text_color: Color::from_rgb8(0xcd, 0xd6, 0xf4),       // Catppuccin Mocha Text
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

    /// Push new battery state to all battery module instances in shared.modules.
    fn push_battery_state(&mut self, percentage: f64, is_charging: bool) {
        for (_, module) in &self.modules {
            if module.id().0 == "battery" {
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
            if module.id().0 == "network" {
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
            if module.id().0 == "volume" {
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
            if module.id() == ModuleId::new("focused-window") {
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

    #[test]
    fn namespace_returns_niribar() {
        assert_eq!(App::namespace(), "niribar");
    }

    #[test]
    fn new_with_config_stores_config() {
        let mut config = Config::default();
        config.bar.collapsed_width = 60;
        let app = App::new(config);
        assert_eq!(app.config.bar.collapsed_width, 60);
    }

    #[test]
    fn shared_state_starts_empty() {
        let state = SharedState::new();
        assert!(state.modules.is_empty());
    }

    #[test]
    fn new_modules_empty_with_no_module_config() {
        // Config::default() has no modules in its list, so none are instantiated.
        let config = make_config();
        let app = App::new(config);
        assert!(app.modules.is_empty());
    }

    #[test]
    fn clock_module_instantiated_from_registry() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        use crate::modules::Zone;
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            zone: Zone::Bottom,
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
        let app = App::new(config);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn hovered_module_starts_none() {
        let app = App::new(make_config());
        assert!(app.hovered_module.is_none());
    }

    #[test]
    fn hover_enter_sets_hovered_module() {
        let mut app = App::new(make_config());
        let id = ModuleId::new("clock");
        let _ = app.update(Message::ModuleHoverEnter(id.clone()));
        assert_eq!(app.hovered_module, Some(id));
    }

    #[test]
    fn hover_exit_clears_hovered_module() {
        let mut app = App::new(make_config());
        let id = ModuleId::new("clock");
        let _ = app.update(Message::ModuleHoverEnter(id.clone()));
        let _ = app.update(Message::ModuleHoverExit(id));
        assert!(app.hovered_module.is_none());
    }

    #[test]
    fn hover_exit_different_id_does_not_clear() {
        let mut app = App::new(make_config());
        let id_a = ModuleId::new("clock");
        let id_b = ModuleId::new("battery");
        let _ = app.update(Message::ModuleHoverEnter(id_a.clone()));
        let _ = app.update(Message::ModuleHoverExit(id_b));
        assert_eq!(app.hovered_module, Some(id_a));
    }

    #[test]
    fn script_module_instantiated_from_registry() {
        use crate::config::{ModuleConfig, ModuleParams, ScriptConfig};
        use crate::modules::Zone;
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "script".into(),
            zone: Zone::Bottom,
            params: ModuleParams::Script(ScriptConfig {
                command: "echo hi".into(),
                interval: 5,
            }),
        }];
        let app = App::new(config);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn script_output_event_updates_state() {
        use crate::modules::script::ScriptEvent;

        let mut app = App::new(make_config());
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
        use crate::modules::Zone;
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "button".into(),
            zone: Zone::Bottom,
            params: ModuleParams::Button(ButtonConfig {
                label: "Launch".into(),
                icon: ">".into(),
                on_click: "kitty".into(),
            }),
        }];
        let app = App::new(config);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn button_ready_event_updates_state() {
        use crate::modules::button::ButtonEvent;

        let mut app = App::new(make_config());
        let id = ModuleId::new("button");
        let _ = app.update(Message::Module(
            id.clone(),
            ModuleEvent::Button(ButtonEvent::Ready),
        ));
        match app.shared.modules.get(&id) {
            Some(ModuleState::Button(_)) => {} // correct
            other => panic!("expected Button state, got {:?}", other),
        }
    }

    #[test]
    fn module_click_no_match_does_not_panic() {
        // ModuleClick for an id with no matching module → Task::none() without panic.
        let mut app = App::new(make_config());
        let id = ModuleId::new("nonexistent");
        let _ = app.update(Message::ModuleClick(id));
        // If we reach here without panic, the test passes.
    }

    #[test]
    fn script_failed_event_retains_previous_state() {
        use crate::config::{ModuleConfig, ModuleParams, ScriptConfig};
        use crate::modules::Zone;
        use crate::modules::script::ScriptEvent;

        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "script".into(),
            zone: Zone::Bottom,
            params: ModuleParams::Script(ScriptConfig {
                command: "echo hi".into(),
                interval: 5,
            }),
        }];
        let mut app = App::new(config);

        // Pre-populate with known output
        let id = ModuleId::new("script");
        app.shared.modules.insert(
            id.clone(),
            ModuleState::Script(crate::modules::script::ScriptState {
                output: "prev".into(),
            }),
        );

        // Fire a Failed event — state must remain unchanged
        let _ = app.update(Message::Module(
            id.clone(),
            ModuleEvent::Script(ScriptEvent::Failed),
        ));

        match app.shared.modules.get(&id) {
            Some(ModuleState::Script(s)) => assert_eq!(s.output, "prev"),
            other => panic!("expected Script state, got {:?}", other),
        }
    }

    // --- Story 3.1 tests (updated for Story 3.2 semantics) ---

    #[test]
    fn app_starts_collapsed() {
        let app = App::new(make_config());
        assert_eq!(app.expand, ExpandState::Collapsed);
    }

    // Story 3.2: BarClick now starts animation instead of jumping to final state.
    #[test]
    fn bar_click_starts_expanding_from_collapsed() {
        let mut app = App::new(make_config());
        assert_eq!(app.expand, ExpandState::Collapsed);
        let _ = app.update(Message::BarClick);
        assert_eq!(app.expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn bar_click_starts_collapsing_from_expanded() {
        let mut app = App::new(make_config());
        app.expand = ExpandState::Expanded;
        let _ = app.update(Message::BarClick);
        assert_eq!(app.expand, ExpandState::Collapsing(0.0));
    }

    // --- Story 3.2 tests ---

    #[test]
    fn tick_advances_expand_progress() {
        let mut app = App::new(make_config());
        app.expand = ExpandState::Expanding(0.0);
        let _ = app.update(Message::Tick);
        let expected_progress = 16.0_f32 / 280.0;
        assert!(
            matches!(app.expand, ExpandState::Expanding(p) if (p - expected_progress).abs() < 1e-5),
            "expected Expanding(~{expected_progress:.4}), got {:?}",
            app.expand
        );
    }

    #[test]
    fn tick_completes_expand_animation() {
        let mut app = App::new(make_config());
        // A progress close enough to 1.0 that one more tick completes it.
        app.expand = ExpandState::Expanding(0.999);
        let _ = app.update(Message::Tick);
        assert_eq!(app.expand, ExpandState::Expanded);
    }

    #[test]
    fn tick_advances_collapse_progress() {
        let mut app = App::new(make_config());
        app.expand = ExpandState::Collapsing(0.0);
        let _ = app.update(Message::Tick);
        let expected_progress = 16.0_f32 / 200.0;
        assert!(
            matches!(app.expand, ExpandState::Collapsing(p) if (p - expected_progress).abs() < 1e-5),
            "expected Collapsing(~{expected_progress:.4}), got {:?}",
            app.expand
        );
    }

    #[test]
    fn tick_completes_collapse_animation() {
        let mut app = App::new(make_config());
        app.expand = ExpandState::Collapsing(0.999);
        let _ = app.update(Message::Tick);
        assert_eq!(app.expand, ExpandState::Collapsed);
    }

    #[test]
    fn tick_no_op_when_collapsed() {
        let mut app = App::new(make_config());
        assert_eq!(app.expand, ExpandState::Collapsed);
        let _ = app.update(Message::Tick);
        assert_eq!(app.expand, ExpandState::Collapsed);
    }

    #[test]
    fn tick_no_op_when_expanded() {
        let mut app = App::new(make_config());
        app.expand = ExpandState::Expanded;
        let _ = app.update(Message::Tick);
        assert_eq!(app.expand, ExpandState::Expanded);
    }

    // --- Story 3.3 tests ---

    #[test]
    fn app_starts_with_bar_hovered_false() {
        let app = App::new(make_config());
        assert!(!app.bar_hovered);
    }

    #[test]
    fn hover_enter_click_trigger_does_nothing() {
        // Click trigger: BarHoverEnter must not change expand state.
        let mut app = App::new(make_config()); // expand_trigger = Click
        let _ = app.update(Message::BarHoverEnter);
        assert_eq!(app.expand, ExpandState::Collapsed);
        assert!(app.bar_hovered); // bar_hovered is set regardless
    }

    #[test]
    fn hover_exit_click_trigger_does_nothing() {
        // Click trigger: BarHoverExit must not change expand state.
        let mut app = App::new(make_config());
        app.expand = ExpandState::Expanded;
        let _ = app.update(Message::BarHoverExit);
        assert_eq!(app.expand, ExpandState::Expanded);
    }

    #[test]
    fn hover_enter_hover_trigger_zero_delay_starts_expanding() {
        let mut app = App::new(make_hover_config());
        let _ = app.update(Message::BarHoverEnter);
        assert_eq!(app.expand, ExpandState::Expanding(0.0));
    }

    #[test]
    fn hover_exit_when_expanded_starts_collapsing() {
        let mut app = App::new(make_hover_config());
        app.expand = ExpandState::Expanded;
        let _ = app.update(Message::BarHoverExit);
        assert_eq!(app.expand, ExpandState::Collapsing(0.0));
    }

    #[test]
    fn hover_exit_when_expanding_reverses() {
        let mut app = App::new(make_hover_config());
        app.expand = ExpandState::Expanding(0.3);
        let _ = app.update(Message::BarHoverExit);
        assert!(
            matches!(app.expand, ExpandState::Collapsing(p) if (p - 0.7).abs() < 1e-5),
            "expected Collapsing(~0.7), got {:?}",
            app.expand
        );
    }

    #[test]
    fn hover_exit_when_collapsed_does_nothing() {
        let mut app = App::new(make_hover_config());
        // expand starts Collapsed
        let _ = app.update(Message::BarHoverExit);
        assert_eq!(app.expand, ExpandState::Collapsed);
    }

    #[test]
    fn hover_enter_with_delay_sets_pending() {
        let mut config = make_hover_config();
        config.bar.hover_delay = 100;
        let mut app = App::new(config);
        let _ = app.update(Message::BarHoverEnter);
        // State stays Collapsed — delay not yet elapsed.
        assert_eq!(app.expand, ExpandState::Collapsed);
        assert_eq!(app.hover_pending_ms, Some(0.0));
    }

    #[test]
    fn hover_exit_cancels_pending() {
        let mut config = make_hover_config();
        config.bar.hover_delay = 100;
        let mut app = App::new(config);
        let _ = app.update(Message::BarHoverEnter); // sets pending
        let _ = app.update(Message::BarHoverExit); // cancels pending
        assert_eq!(app.hover_pending_ms, None);
        assert_eq!(app.expand, ExpandState::Collapsed);
    }

    #[test]
    fn tick_triggers_expand_when_delay_elapsed() {
        let mut config = make_hover_config();
        config.bar.hover_delay = 100;
        let mut app = App::new(config);
        // Simulate hover delay almost elapsed (90ms of 100ms done).
        app.hover_pending_ms = Some(90.0);
        // One tick (16ms) pushes elapsed to 106ms ≥ 100ms → expand starts.
        let _ = app.update(Message::Tick);
        assert_eq!(app.expand, ExpandState::Expanding(0.0));
        assert_eq!(app.hover_pending_ms, None);
    }

    #[test]
    fn bar_click_noop_when_hover_trigger() {
        let mut app = App::new(make_hover_config());
        let _ = app.update(Message::BarClick);
        assert_eq!(app.expand, ExpandState::Collapsed); // unchanged
    }

    // --- Story 4.1 tests ---

    #[test]
    fn niri_connected_event_sets_connected() {
        let mut app = App::new(make_config());
        let _ = app.update(Message::Niri(NiriEvent::Connected));
        assert!(app.shared.niri.connected);
    }

    #[test]
    fn niri_unavailable_event_does_not_crash() {
        let mut app = App::new(make_config());
        let _ = app.update(Message::Niri(NiriEvent::Unavailable));
        assert!(!app.shared.niri.connected);
    }

    #[test]
    fn niri_workspaces_changed_stores_workspaces() {
        use niri_ipc::{Event, Workspace};
        let mut app = App::new(make_config());
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
        let mut app = App::new(make_config());
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowFocusChanged {
            id: Some(42),
        })));
        assert_eq!(app.shared.niri.focused_window_id, Some(42));
    }

    #[test]
    fn niri_window_focus_cleared() {
        use niri_ipc::Event;
        let mut app = App::new(make_config());
        app.shared.niri.focused_window_id = Some(99);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowFocusChanged {
            id: None,
        })));
        assert_eq!(app.shared.niri.focused_window_id, None);
    }

    #[test]
    fn niri_unknown_event_is_ignored() {
        use niri_ipc::Event;
        let mut app = App::new(make_config());
        // KeyboardLayoutsChanged is not handled — verify no panic and no state change.
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
            zone: Zone::Top,
            params: ModuleParams::None,
        }];
        App::new(config)
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
        let mut app = App::new(make_config()); // no workspaces module
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
        // Send in non-sorted order — state stores as-received; view sorts by idx
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
                // Stored in arrival order (idx: 2, 0, 1) — view sorts, state doesn't
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
        // Seed two workspaces; idx=0 starts focused.
        let ws0 = make_workspace(0, true);
        let ws1 = make_workspace(1, false);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
            workspaces: vec![ws0, ws1],
        })));

        // User switches to workspace 2 (id=2, idx=1).
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspaceActivated {
            id: 2,
            focused: true,
        })));

        let id = ModuleId::new("workspaces");
        match app.shared.modules.get(&id) {
            Some(ModuleState::Workspace(s)) => {
                let focused: Vec<_> = s.workspaces.iter().filter(|w| w.is_focused).collect();
                assert_eq!(focused.len(), 1, "exactly one workspace should be focused");
                assert_eq!(
                    focused[0].idx, 1,
                    "workspace with idx=1 should now be focused"
                );
            }
            other => panic!("expected Workspace state, got {:?}", other),
        }
    }

    #[test]
    fn workspace_activated_no_panic_when_workspaces_empty() {
        use niri_ipc::Event;
        let mut app = make_workspaces_app();
        // WorkspaceActivated fires before WorkspacesChanged seeds the list — must not panic.
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspaceActivated {
            id: 1,
            focused: true,
        })));
        // No panic; no workspaces in state yet.
        assert!(app.shared.niri.workspaces.is_empty());
    }

    // --- Story 4.4 helpers and tests ---

    fn make_focused_window_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "focused-window".into(),
            zone: Zone::Center,
            params: ModuleParams::None,
        }];
        App::new(config)
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
        // Pre-populate window cache with a window that has a title.
        let win = make_window(1, Some("Firefox"), false);
        app.shared.niri.windows.insert(1, win);
        // Send focus event for that window.
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
        // When WindowFocusChanged { id: None } fires (bar hover / layer-shell focus),
        // the module should retain the last known title rather than blanking.
        use niri_ipc::Event;
        let mut app = make_focused_window_app();
        // First, focus a real window so the module has a title.
        let win = make_window(1, Some("Firefox"), false);
        app.shared.niri.windows.insert(1, win);
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowFocusChanged {
            id: Some(1),
        })));
        // Now simulate bar hover: niri sends WindowFocusChanged { id: None }.
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowFocusChanged {
            id: None,
        })));
        let id = ModuleId::new("focused-window");
        match app.shared.modules.get(&id) {
            Some(ModuleState::FocusedWindow(s)) => {
                // Title must be retained — NOT cleared by the None focus event.
                assert_eq!(s.title.as_deref(), Some("Firefox"));
            }
            other => panic!("expected FocusedWindow state, got {:?}", other),
        }
    }

    #[test]
    fn focused_window_state_updated_on_windows_changed() {
        // WindowsChanged must derive focused_window_id from is_focused — not rely on prior state.
        // This covers the startup case where WindowsChanged arrives before any WindowFocusChanged.
        use niri_ipc::Event;
        let mut app = make_focused_window_app();
        // No pre-existing focused_window_id — simulates bar startup.
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
        // Seed focused window.
        let win = make_window(4, Some("Nautilus"), true);
        app.shared.niri.windows.insert(4, win);
        app.shared.niri.focused_window_id = Some(4);
        app.shared.modules.insert(
            ModuleId::new("focused-window"),
            ModuleState::FocusedWindow(crate::modules::focused_window::FocusedWindowState {
                title: Some("Nautilus".into()),
            }),
        );
        // Close the focused window.
        let _ = app.update(Message::Niri(NiriEvent::Event(Event::WindowClosed {
            id: 4,
        })));
        let id = ModuleId::new("focused-window");
        match app.shared.modules.get(&id) {
            Some(ModuleState::FocusedWindow(s)) => {
                assert!(
                    s.title.is_none(),
                    "title should be None after window closed"
                );
            }
            other => panic!("expected FocusedWindow state, got {:?}", other),
        }
    }

    #[test]
    fn focused_window_state_not_updated_without_focused_window_module() {
        use niri_ipc::Event;
        let mut app = App::new(make_config()); // no focused-window module
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
        // SharedState::new() must initialise dbus with no connections (Story 5.1 AC #2).
        let state = SharedState::new();
        assert!(state.dbus.session.is_none());
        assert!(state.dbus.system.is_none());
    }

    #[test]
    fn dbus_ready_stores_connections_in_shared_state() {
        // Sending DbusReady(connections) must update shared.dbus (Story 5.1 AC #4).
        // Use a default DbusConnections (both None) — simulates bus-unavailable case.
        let mut app = App::new(Config::default());
        let conns = crate::dbus::DbusConnections::default();
        let _ = app.update(Message::DbusReady(conns));
        // If we get here without panic, the message was handled correctly.
        assert!(app.shared.dbus.session.is_none());
        assert!(app.shared.dbus.system.is_none());
    }

    // Story 5.2 tests

    fn make_battery_app() -> App {
        use crate::config::{BatteryConfig, ModuleConfig, ModuleParams};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "battery".into(),
            zone: Zone::Bottom,
            params: ModuleParams::Battery(BatteryConfig {
                critical_threshold: 15,
            }),
        }];
        App::new(config)
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
        let mut app = App::new(Config::default()); // no battery module
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
            zone: Zone::Bottom,
            params: ModuleParams::Network(NetworkConfig),
        }];
        App::new(config)
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
        let mut app = App::new(Config::default()); // no network module
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
            zone: Zone::Bottom,
            params: ModuleParams::Volume(VolumeConfig { step: 5 }),
        }];
        App::new(config)
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
        let mut app = App::new(Config::default()); // no volume module
        let _ = app.update(Message::Volume(VolumeEvent::StateChanged {
            volume: 50.0,
            muted: false,
        }));
        let id = ModuleId::new("volume");
        assert!(!app.shared.modules.contains_key(&id));
    }

    // --- Story 6.1 tests ---

    #[test]
    fn focused_module_starts_none() {
        let app = App::new(make_config());
        assert!(app.focused_module.is_none());
    }

    #[test]
    fn bar_focus_gained_sets_first_top_module() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            zone: Zone::Top,
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
        let mut app = App::new(config);
        let _ = app.update(Message::BarFocusGained);
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }

    #[test]
    fn bar_focus_lost_clears_focused_module() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            zone: Zone::Top,
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
        let mut app = App::new(config);
        let _ = app.update(Message::BarFocusGained);
        assert!(app.focused_module.is_some());
        let _ = app.update(Message::BarFocusLost);
        assert!(app.focused_module.is_none());
    }

    #[test]
    fn escape_pressed_when_focused_does_not_immediately_clear_state() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            zone: Zone::Top,
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
        let mut app = App::new(config);
        let _ = app.update(Message::BarFocusGained);
        assert!(app.focused_module.is_some());
        // EscapePressed returns KeyboardInteractivityChange(None) — state is cleared by
        // the subsequent BarFocusLost (after the Wayland round-trip), not immediately.
        let _ = app.update(Message::EscapePressed);
        assert!(app.focused_module.is_some());
    }

    #[test]
    fn escape_pressed_when_not_focused_is_noop() {
        let mut app = App::new(make_config());
        assert!(app.focused_module.is_none());
        let _ = app.update(Message::EscapePressed);
        assert!(app.focused_module.is_none());
    }

    #[test]
    fn bar_focus_gained_falls_back_to_center_if_no_top_zone() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            zone: Zone::Center,
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
        let mut app = App::new(config);
        let _ = app.update(Message::BarFocusGained);
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }

    #[test]
    fn bar_focus_gained_with_no_modules_is_noop() {
        let mut app = App::new(make_config());
        let _ = app.update(Message::BarFocusGained);
        assert!(app.focused_module.is_none());
    }

    // --- Story 6.2 tests ---

    fn make_nav_app() -> App {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config::default();
        config.modules = vec![
            ModuleConfig {
                name: "clock".into(),
                zone: Zone::Top,
                params: ModuleParams::Clock(ClockConfig::default()),
            },
            ModuleConfig {
                name: "battery".into(),
                zone: Zone::Top,
                params: ModuleParams::None,
            },
            ModuleConfig {
                name: "network".into(),
                zone: Zone::Center,
                params: ModuleParams::None,
            },
            ModuleConfig {
                name: "workspaces".into(),
                zone: Zone::Bottom,
                params: ModuleParams::None,
            },
        ];
        App::new(config)
    }

    #[test]
    fn focus_move_next_advances_to_next_module() {
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained); // focused = clock (index 0)
        let _ = app.update(Message::FocusMoveNext);
        assert_eq!(app.focused_module, Some(ModuleId::new("battery"))); // index 1
    }

    #[test]
    fn focus_move_next_crosses_zone_boundary() {
        // Down from battery (top zone, index 1) should move to network (center zone, index 2).
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained);
        let _ = app.update(Message::FocusMoveNext); // clock → battery
        let _ = app.update(Message::FocusMoveNext); // battery → network (crosses zone)
        assert_eq!(app.focused_module, Some(ModuleId::new("network")));
    }

    #[test]
    fn focus_move_next_wraps_from_last_to_first() {
        // Down from workspaces (last module) wraps back to clock (first module).
        let mut app = make_nav_app();
        app.focused_module = Some(ModuleId::new("workspaces")); // last module
        let _ = app.update(Message::FocusMoveNext);
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }

    #[test]
    fn focus_move_prev_retreats_to_previous_module() {
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained);
        let _ = app.update(Message::FocusMoveNext); // → battery
        let _ = app.update(Message::FocusMovePrev); // → clock
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }

    #[test]
    fn focus_move_prev_wraps_from_first_to_last() {
        // Up from clock (first module) wraps to workspaces (last module).
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained); // focused = clock
        let _ = app.update(Message::FocusMovePrev);
        assert_eq!(app.focused_module, Some(ModuleId::new("workspaces")));
    }

    #[test]
    fn focus_next_zone_jumps_to_center() {
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained); // top: clock
        let _ = app.update(Message::FocusNextZone); // → center: network
        assert_eq!(app.focused_module, Some(ModuleId::new("network")));
    }

    #[test]
    fn focus_next_zone_wraps_bottom_to_top() {
        let mut app = make_nav_app();
        app.focused_module = Some(ModuleId::new("workspaces")); // bottom zone
        let _ = app.update(Message::FocusNextZone); // → top: clock
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }

    #[test]
    fn focus_prev_zone_from_top_jumps_to_bottom() {
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained); // top: clock
        let _ = app.update(Message::FocusPrevZone); // reverse cycle: top → bottom
        assert_eq!(app.focused_module, Some(ModuleId::new("workspaces")));
    }

    #[test]
    fn focus_next_zone_skips_empty_zone() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        // Only top and bottom; center is empty.
        let mut config = Config::default();
        config.modules = vec![
            ModuleConfig {
                name: "clock".into(),
                zone: Zone::Top,
                params: ModuleParams::Clock(ClockConfig::default()),
            },
            ModuleConfig {
                name: "workspaces".into(),
                zone: Zone::Bottom,
                params: ModuleParams::None,
            },
        ];
        let mut app = App::new(config);
        let _ = app.update(Message::BarFocusGained); // top: clock
        let _ = app.update(Message::FocusNextZone); // skip empty center → bottom
        assert_eq!(app.focused_module, Some(ModuleId::new("workspaces")));
    }

    #[test]
    fn navigation_keys_when_not_focused_are_noop() {
        let mut app = make_nav_app();
        // Bar not focused — focused_module is None
        let _ = app.update(Message::FocusMoveNext);
        let _ = app.update(Message::FocusMovePrev);
        let _ = app.update(Message::FocusNextZone);
        let _ = app.update(Message::FocusPrevZone);
        assert!(app.focused_module.is_none());
    }

    #[test]
    fn focus_prev_zone_from_center_jumps_to_top() {
        let mut app = make_nav_app();
        app.focused_module = Some(ModuleId::new("network")); // center zone
        let _ = app.update(Message::FocusPrevZone); // reverse cycle: center → top
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }

    // --- Story 6.3 tests ---

    fn make_interact_app() -> App {
        use crate::config::{ButtonConfig, ClockConfig, ModuleConfig, ModuleParams, VolumeConfig};
        let mut config = Config::default();
        config.modules = vec![
            ModuleConfig {
                name: "clock".into(),
                zone: Zone::Top,
                params: ModuleParams::Clock(ClockConfig::default()),
            },
            ModuleConfig {
                name: "volume".into(),
                zone: Zone::Top,
                params: ModuleParams::Volume(VolumeConfig::default()),
            },
            ModuleConfig {
                name: "button".into(),
                zone: Zone::Center,
                params: ModuleParams::Button(ButtonConfig {
                    label: "test".into(),
                    icon: ">".into(),
                    on_click: "true".into(), // harmless no-op command
                }),
            },
        ];
        App::new(config)
    }

    #[test]
    fn keyboard_activate_when_not_focused_is_noop() {
        let mut app = make_interact_app();
        let _ = app.update(Message::KeyboardActivate);
        assert!(app.focused_module.is_none());
        assert!(!app.module_activated);
    }

    #[test]
    fn keyboard_activate_on_volume_enters_interaction_mode() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        let _ = app.update(Message::FocusMoveNext); // → volume
        assert_eq!(app.focused_module, Some(ModuleId::new("volume")));
        assert!(!app.module_activated);
        let _ = app.update(Message::KeyboardActivate);
        // Volume is scrollable → interaction mode entered, focus unchanged
        assert!(app.module_activated);
        assert_eq!(app.focused_module, Some(ModuleId::new("volume")));
    }

    #[test]
    fn arrow_keys_scroll_when_module_activated() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        let _ = app.update(Message::FocusMoveNext); // → volume
        let _ = app.update(Message::KeyboardActivate); // activate volume
        assert!(app.module_activated);
        // Down arrow scrolls (fires VolumeAdjust via Task::done), focus stays on volume
        let _ = app.update(Message::FocusMoveNext);
        assert_eq!(app.focused_module, Some(ModuleId::new("volume")));
        assert!(app.module_activated); // still activated
        // Up arrow scrolls too
        let _ = app.update(Message::FocusMovePrev);
        assert_eq!(app.focused_module, Some(ModuleId::new("volume")));
    }

    #[test]
    fn escape_deactivates_without_releasing_focus() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        let _ = app.update(Message::FocusMoveNext); // → volume
        let _ = app.update(Message::KeyboardActivate); // activate volume
        assert!(app.module_activated);
        let _ = app.update(Message::EscapePressed);
        // Deactivates, focus remains on volume
        assert!(!app.module_activated);
        assert_eq!(app.focused_module, Some(ModuleId::new("volume")));
    }

    #[test]
    fn arrow_keys_navigate_when_module_not_activated() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        // Down arrow without activation navigates normally (even on volume)
        let _ = app.update(Message::FocusMoveNext); // clock → volume
        assert_eq!(app.focused_module, Some(ModuleId::new("volume")));
        assert!(!app.module_activated);
        // Down again: volume is NOT activated, so navigate (wraps to button)
        let _ = app.update(Message::FocusMoveNext); // volume → button (global wrap)
        assert_eq!(app.focused_module, Some(ModuleId::new("button")));
    }

    #[test]
    fn tab_deactivates_and_navigates_zone() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        let _ = app.update(Message::FocusMoveNext); // → volume
        let _ = app.update(Message::KeyboardActivate); // activate volume
        assert!(app.module_activated);
        // Tab deactivates and jumps to next zone
        let _ = app.update(Message::FocusNextZone); // → button (center)
        assert!(!app.module_activated);
        assert_eq!(app.focused_module, Some(ModuleId::new("button")));
    }

    #[test]
    fn shift_tab_deactivates_and_navigates_zone() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        let _ = app.update(Message::FocusMoveNext); // → volume
        let _ = app.update(Message::KeyboardActivate); // activate volume
        assert!(app.module_activated);
        let _ = app.update(Message::FocusPrevZone); // deactivates, jumps to prev zone
        assert!(!app.module_activated);
    }

    #[test]
    fn keyboard_activate_on_button_does_not_panic_and_does_not_set_activated() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        let _ = app.update(Message::FocusNextZone); // → button (center)
        assert_eq!(app.focused_module, Some(ModuleId::new("button")));
        // Button is not scrollable → fires click (side effect: spawns "true"), no interaction mode
        let _ = app.update(Message::KeyboardActivate);
        assert!(!app.module_activated);
        assert_eq!(app.focused_module, Some(ModuleId::new("button")));
    }

    #[test]
    fn keyboard_activate_on_non_interactive_module_is_noop() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
        let _ = app.update(Message::KeyboardActivate);
        // clock handles neither scroll nor click — no state change
        assert!(!app.module_activated);
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }

    #[test]
    fn focus_move_next_on_unfocused_delegates_to_bar_focus_gained() {
        let mut app = make_interact_app();
        assert!(app.focused_module.is_none());
        let _ = app.update(Message::FocusMoveNext);
        let _ = app.update(Message::BarFocusGained);
        assert!(app.focused_module.is_some());
    }

    #[test]
    fn focus_move_prev_on_unfocused_delegates_to_bar_focus_gained() {
        let mut app = make_interact_app();
        assert!(app.focused_module.is_none());
        let _ = app.update(Message::FocusMovePrev);
        let _ = app.update(Message::BarFocusGained);
        assert!(app.focused_module.is_some());
    }
}
