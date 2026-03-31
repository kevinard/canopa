use iced::widget::{Space, container, row};
use iced::{Color, Element, Length, Task};
use iced_layershell::to_layer_message;

use crate::config::Config;

pub struct App {
    // Config is used directly from Story 2.x onward (module instantiation, zone layout)
    #[allow(dead_code)]
    pub config: Config,
}

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {}

impl App {
    pub fn new(config: Config) -> Self {
        App { config }
    }

    pub fn namespace() -> String {
        String::from("niribar")
    }

    pub fn update(&mut self, _message: Message) -> Task<Message> {
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content_width = self.config.bar.collapsed_width.saturating_sub(1);
        row![
            // Main content area — (collapsed_width - 1)px, leaving room for the 1px right border
            container(Space::new())
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
}
