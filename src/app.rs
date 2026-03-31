use iced::widget::{Space, container, row};
use iced::{Color, Element, Length, Task};
use iced_layershell::to_layer_message;

pub struct App {}

#[to_layer_message]
#[derive(Debug, Clone)]
pub enum Message {}

impl App {
    pub fn new() -> Self {
        App {}
    }

    pub fn namespace() -> String {
        String::from("niribar")
    }

    pub fn update(&mut self, _message: Message) -> Task<Message> {
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        row![
            // Main content area — 47px wide (48px total minus 1px right border)
            container(Space::new()).width(47).height(Length::Fill),
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
    fn new_constructs_without_panic() {
        let _app = App::new();
    }
}
