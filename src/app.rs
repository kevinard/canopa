use std::collections::HashMap;
use std::time::Duration;

use iced::widget::{Space, container, mouse_area, row};
use iced::{Color, Element, Length, Task};
use iced_layershell::to_layer_message;

use crate::config::{Config, ExpandTrigger};
use crate::modules::button::{ButtonEvent, ButtonState};
use crate::modules::clock::{ClockEvent, ClockState};
use crate::modules::script::{ScriptEvent, ScriptState};
use crate::modules::workspaces;
use crate::modules::{
    InteractionEvent, Module, ModuleEvent, ModuleId, ModuleState, Zone, build_registry,
};
use crate::niri::subscription::niri_subscription;
use crate::niri::{NiriEvent, NiriState};
use crate::surface::expand::ExpandState;
use crate::surface::frame::module_frame;
use crate::surface::layout::zone_layout;

/// Shared state: module data visible to all surfaces.
///
/// Module data (battery %, workspace list, etc.) lives here.
/// Per-surface render state (expand/collapse, focus) lives in `SurfaceState` — never here.
#[derive(Default)]
pub struct SharedState {
    pub modules: HashMap<ModuleId, ModuleState>,
    pub niri: NiriState, // populated by Message::Niri handlers (Story 4.1)
}

impl SharedState {
    pub fn new() -> Self {
        SharedState {
            modules: HashMap::new(),
            niri: NiriState::default(),
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
                    _ => {} // Other events ignored; Stories 4.3/4.4 extend this
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
            _ => Task::none(),
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        let module_subs =
            iced::Subscription::batch(self.modules.iter().map(|(_, m)| m.subscription()));
        let niri_sub = niri_subscription();
        if self.expand.is_animating() || self.hover_pending_ms.is_some() {
            // Emit a Tick every ~16ms (≈60fps) while animating or hover delay is pending.
            let tick = iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick);
            iced::Subscription::batch([module_subs, niri_sub, tick])
        } else {
            iced::Subscription::batch([module_subs, niri_sub])
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

            // During Expanding: show expanded content (opacity fade deferred — no iced wrapper).
            // During Collapsing: show collapsed content immediately (no reverse fade).
            let module_view: Element<'_, Message> = if shows_expanded {
                module.view_expanded(&state)
            } else {
                module.view_collapsed(&state)
            };

            let view: Element<'_, Message> = mouse_area(module_frame(module_view, hovered))
                .on_enter(Message::ModuleHoverEnter(id.clone()))
                .on_exit(Message::ModuleHoverExit(id.clone()))
                .on_press(Message::ModuleClick(id))
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
}
