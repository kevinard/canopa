use std::collections::HashMap;

use iced::widget::{Space, container, mouse_area, row};
use iced::{Color, Element, Length, Task};
use iced_layershell::to_layer_message;

use crate::config::Config;
use crate::modules::clock::{ClockEvent, ClockState};
use crate::modules::{Module, ModuleEvent, ModuleId, ModuleState, Zone, build_registry};
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
}

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    // Module data events — ModuleEvent gains concrete variants in Stories 2.3+.
    Module(ModuleId, ModuleEvent),
    ModuleHoverEnter(ModuleId),
    ModuleHoverExit(ModuleId),
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
            _ => Task::none(),
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch(self.modules.iter().map(|(_, m)| m.subscription()))
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content_width = self.config.bar.collapsed_width.saturating_sub(1);

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
            let view: Element<'_, Message> =
                mouse_area(module_frame(module.view_collapsed(&state), hovered))
                    .on_enter(Message::ModuleHoverEnter(id.clone()))
                    .on_exit(Message::ModuleHoverExit(id))
                    .into();

            match zone {
                Zone::Top => top.push(view),
                Zone::Center => center.push(view),
                Zone::Bottom => bottom.push(view),
            }
        }

        row![
            // Main content area — zone layout fills (collapsed_width - 1)px
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
        .height(Length::Fill)
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
}
