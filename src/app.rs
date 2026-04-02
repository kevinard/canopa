use std::collections::HashMap;

use iced::widget::{Space, container, mouse_area, row};
use iced::{Color, Element, Length, Task};
use iced_layershell::to_layer_message;

use crate::config::Config;
use crate::modules::button::{ButtonEvent, ButtonState};
use crate::modules::clock::{ClockEvent, ClockState};
use crate::modules::script::{ScriptEvent, ScriptState};
use crate::modules::{
    InteractionEvent, Module, ModuleEvent, ModuleId, ModuleState, Zone, build_registry,
};
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
    // niri: NiriState — added in Story 4.1
}

impl SharedState {
    pub fn new() -> Self {
        SharedState {
            modules: HashMap::new(),
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
                // Toggle expand/collapse state and resize the layer-shell surface.
                self.expand = self.expand.toggle();
                let new_width = if self.expand.is_expanded() {
                    self.config.bar.expanded_width
                } else {
                    self.config.bar.collapsed_width
                };
                // SizeChange and ExclusiveZoneChange are injected by #[to_layer_message].
                // SizeChange((w, h)) — h=0 means "stretch full height" (layer shell convention).
                // ExclusiveZoneChange reserves the correct amount of desktop space.
                Task::batch([
                    Task::done(Message::SizeChange((new_width, 0))),
                    Task::done(Message::ExclusiveZoneChange(
                        i32::try_from(new_width).unwrap_or(i32::MAX),
                    )),
                ])
            }
            _ => Task::none(),
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch(self.modules.iter().map(|(_, m)| m.subscription()))
    }

    pub fn view(&self) -> Element<'_, Message> {
        // Choose content width and module view method based on expand state.
        let content_width = if self.expand.is_expanded() {
            self.config.bar.expanded_width.saturating_sub(1)
        } else {
            self.config.bar.collapsed_width.saturating_sub(1)
        };

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

            // Call view_expanded or view_collapsed based on current expand state (AC #4, #5).
            let module_view = if self.expand.is_expanded() {
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
            // Main content area — zone layout fills (width - 1)px
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

        // Wrap entire bar in mouse_area for click-to-toggle (AC #1, #2).
        // Note: clicking a module fires both ModuleClick(id) AND BarClick (outer area).
        mouse_area(bar_content).on_press(Message::BarClick).into()
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

    // --- Story 3.1 tests ---

    #[test]
    fn app_starts_collapsed() {
        let app = App::new(make_config());
        assert_eq!(app.expand, ExpandState::Collapsed);
    }

    #[test]
    fn bar_click_toggles_collapsed_to_expanded() {
        let mut app = App::new(make_config());
        assert_eq!(app.expand, ExpandState::Collapsed);
        let _ = app.update(Message::BarClick);
        assert_eq!(app.expand, ExpandState::Expanded);
    }

    #[test]
    fn bar_click_toggles_expanded_to_collapsed() {
        let mut app = App::new(make_config());
        let _ = app.update(Message::BarClick); // → Expanded
        let _ = app.update(Message::BarClick); // → Collapsed
        assert_eq!(app.expand, ExpandState::Collapsed);
    }
}
