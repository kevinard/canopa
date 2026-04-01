use std::collections::HashMap;

use iced::widget::{Space, container, row};
use iced::{Color, Element, Length, Task};
use iced_layershell::to_layer_message;

use crate::config::Config;
use crate::modules::{ModuleEvent, ModuleId, ModuleState};
use crate::surface::layout::zone_layout;

/// Shared state: module data visible to all surfaces.
///
/// Module data (battery %, workspace list, etc.) lives here.
/// Per-surface render state (expand/collapse, focus) lives in `SurfaceState` — never here.
#[derive(Default)]
pub struct SharedState {
    // Populated in Story 2.2+ as modules are instantiated and emit events.
    #[allow(dead_code)]
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
    // Module data routed here in Story 2.3+ when ModuleEvent gains concrete variants.
    #[allow(dead_code)]
    pub shared: SharedState,
}

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {
    // Module data events — ModuleEvent gains concrete variants in Stories 2.3+.
    Module(ModuleId, ModuleEvent),
}

impl App {
    pub fn new(config: Config) -> Self {
        App {
            config,
            shared: SharedState::new(),
        }
    }

    pub fn namespace() -> String {
        String::from("niribar")
    }

    pub fn update(&mut self, _message: Message) -> Task<Message> {
        // Module event routing established in Story 2.3 when ModuleEvent gains variants.
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content_width = self.config.bar.collapsed_width.saturating_sub(1);
        row![
            // Main content area — zone layout fills (collapsed_width - 1)px
            container(zone_layout(vec![], vec![], vec![]))
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
}
