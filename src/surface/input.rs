use iced::event::listen_with;
use iced::keyboard::{self, key::Named};
use iced::window;
use iced::{Event, Subscription};

use crate::app::Message;

/// Subscribe to keyboard focus events (Window::Focused, Window::Unfocused) and
/// Escape key presses. Returns messages that update bar keyboard focus state.
pub fn keyboard_focus_subscription() -> Subscription<Message> {
    listen_with(|event, _status, _window| match event {
        Event::Window(window::Event::Focused) => Some(Message::BarFocusGained),
        Event::Window(window::Event::Unfocused) => Some(Message::BarFocusLost),
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Escape),
            ..
        }) => Some(Message::EscapePressed),
        _ => None,
    })
}
