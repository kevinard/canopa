use iced::event::listen_with;
use iced::keyboard::{self, key::Named};
use iced::window;
use iced::{Event, Subscription};

use crate::app::Message;

/// Parse a keybind string like `"Super+e"` or `"Ctrl+F5"` into `(Modifiers, Key)`.
pub fn parse_keybind(s: &str) -> Option<(keyboard::Modifiers, keyboard::Key)> {
    let parts: Vec<&str> = s.split('+').collect();
    let key_token = *parts.last().unwrap_or(&"");
    if key_token.is_empty() {
        return None;
    }
    let modifier_tokens = &parts[..parts.len() - 1];

    let mut mods = keyboard::Modifiers::default();
    for &modifier in modifier_tokens {
        match modifier {
            "Super" | "Logo" | "Win" => mods |= keyboard::Modifiers::LOGO,
            "Ctrl" | "Control" => mods |= keyboard::Modifiers::CTRL,
            "Alt" => mods |= keyboard::Modifiers::ALT,
            "Shift" => mods |= keyboard::Modifiers::SHIFT,
            _ => return None,
        }
    }

    let key = if key_token.chars().count() == 1 {
        keyboard::Key::Character(key_token.into())
    } else {
        match key_token {
            "F1" => keyboard::Key::Named(Named::F1),
            "F2" => keyboard::Key::Named(Named::F2),
            "F3" => keyboard::Key::Named(Named::F3),
            "F4" => keyboard::Key::Named(Named::F4),
            "F5" => keyboard::Key::Named(Named::F5),
            "F6" => keyboard::Key::Named(Named::F6),
            "F7" => keyboard::Key::Named(Named::F7),
            "F8" => keyboard::Key::Named(Named::F8),
            "F9" => keyboard::Key::Named(Named::F9),
            "F10" => keyboard::Key::Named(Named::F10),
            "F11" => keyboard::Key::Named(Named::F11),
            "F12" => keyboard::Key::Named(Named::F12),
            "Space" => keyboard::Key::Named(Named::Space),
            "Enter" | "Return" => keyboard::Key::Named(Named::Enter),
            _ => return None,
        }
    };

    Some((mods, key))
}

/// Event handler for [`keyboard_focus_subscription`]. A `fn` pointer so it can be
/// used with `listen_with`.
fn keyboard_handler(
    event: Event,
    status: iced::event::Status,
    window: iced::window::Id,
) -> Option<Message> {
    match event {
        Event::Window(window::Event::Focused) => Some(Message::Surface(
            crate::app::messages::SurfaceEvent::BarFocusGained(window),
        )),
        Event::Window(window::Event::Unfocused) => Some(Message::Surface(
            crate::app::messages::SurfaceEvent::BarFocusLost(window),
        )),
        Event::Window(window::Event::Opened { size, .. }) => Some(Message::Surface(
            crate::app::messages::SurfaceEvent::WindowSized(
                window,
                size.width as u32,
                size.height as u32,
            ),
        )),
        Event::Window(window::Event::Resized(size)) => Some(Message::Surface(
            crate::app::messages::SurfaceEvent::WindowSized(
                window,
                size.width as u32,
                size.height as u32,
            ),
        )),
        Event::Window(window::Event::Closed) => Some(Message::Surface(
            crate::app::messages::SurfaceEvent::WindowClosed(window),
        )),
        Event::Mouse(iced::mouse::Event::CursorMoved { position }) => Some(Message::Surface(
            crate::app::messages::SurfaceEvent::CursorMoved(window, position),
        )),
        Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => match key {
            keyboard::Key::Named(Named::Escape) if modifiers == keyboard::Modifiers::default() => {
                Some(Message::Surface(
                    crate::app::messages::SurfaceEvent::EscapePressed(window),
                ))
            }
            keyboard::Key::Named(Named::ArrowDown)
                if modifiers == keyboard::Modifiers::default() =>
            {
                Some(Message::Surface(
                    crate::app::messages::SurfaceEvent::FocusMoveNext(window),
                ))
            }
            keyboard::Key::Named(Named::ArrowUp) if modifiers == keyboard::Modifiers::default() => {
                Some(Message::Surface(
                    crate::app::messages::SurfaceEvent::FocusMovePrev(window),
                ))
            }
            keyboard::Key::Named(Named::Tab) => {
                if modifiers.shift() {
                    Some(Message::Surface(
                        crate::app::messages::SurfaceEvent::FocusPrevZone(window),
                    ))
                } else {
                    Some(Message::Surface(
                        crate::app::messages::SurfaceEvent::FocusNextZone(window),
                    ))
                }
            }
            keyboard::Key::Named(Named::Enter) if modifiers == keyboard::Modifiers::default() => {
                Some(Message::Surface(
                    crate::app::messages::SurfaceEvent::KeyboardActivate(window),
                ))
            }
            keyboard::Key::Named(Named::Backspace)
                if modifiers == keyboard::Modifiers::default() =>
            {
                Some(Message::Surface(
                    crate::app::messages::SurfaceEvent::BackPressed(window),
                ))
            }
            keyboard::Key::Named(Named::Space) if modifiers == keyboard::Modifiers::default() => {
                Some(Message::Surface(
                    crate::app::messages::SurfaceEvent::KeyboardSecondaryActivate(window),
                ))
            }
            _ if status == iced::event::Status::Ignored => Some(Message::Surface(
                crate::app::messages::SurfaceEvent::ActionKeyPressed {
                    window,
                    modifiers,
                    key,
                },
            )),
            _ => None,
        },
        _ => None,
    }
}

/// Subscribe to keyboard focus events and key presses.
pub fn keyboard_focus_subscription() -> Subscription<Message> {
    listen_with(keyboard_handler)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_keybind_accepts_action_keybinds() {
        let (mods, key) = parse_keybind("Ctrl+F5").unwrap();
        assert!(mods.control());
        assert_eq!(key, keyboard::Key::Named(Named::F5));
    }

    #[test]
    fn cursor_motion_emits_cursor_moved_event() {
        let wid = iced::window::Id::unique();
        let msg = keyboard_handler(
            Event::Mouse(iced::mouse::Event::CursorMoved {
                position: iced::Point::new(12.0, 34.0),
            }),
            iced::event::Status::Ignored,
            wid,
        );

        assert!(matches!(
            msg,
            Some(crate::app::Message::Surface(
                crate::app::messages::SurfaceEvent::CursorMoved(w, p)
            )) if w == wid && p == iced::Point::new(12.0, 34.0)
        ));
    }

    #[test]
    fn window_closed_emits_surface_window_closed_event() {
        let wid = iced::window::Id::unique();
        let msg = keyboard_handler(
            Event::Window(window::Event::Closed),
            iced::event::Status::Ignored,
            wid,
        );

        assert!(matches!(
            msg,
            Some(crate::app::Message::Surface(
                crate::app::messages::SurfaceEvent::WindowClosed(w)
            )) if w == wid
        ));
    }

    #[test]
    fn configured_action_key_emits_surface_action_key_event() {
        let wid = iced::window::Id::unique();
        let msg = keyboard_handler(
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Character("e".into()),
                modified_key: keyboard::Key::Character("e".into()),
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyE),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::CTRL,
                text: None,
                repeat: false,
            }),
            iced::event::Status::Ignored,
            wid,
        );

        assert!(matches!(
            msg,
            Some(crate::app::Message::Surface(
                crate::app::messages::SurfaceEvent::ActionKeyPressed {
                    window,
                    modifiers,
                    key,
                }
            )) if window == wid
                && modifiers == keyboard::Modifiers::CTRL
                && key == keyboard::Key::Character("e".into())
        ));
    }

    #[test]
    fn modified_space_emits_surface_action_key_event() {
        let wid = iced::window::Id::unique();
        let msg = keyboard_handler(
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(Named::Space),
                modified_key: keyboard::Key::Named(Named::Space),
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Space),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::LOGO,
                text: None,
                repeat: false,
            }),
            iced::event::Status::Ignored,
            wid,
        );

        assert!(matches!(
            msg,
            Some(crate::app::Message::Surface(
                crate::app::messages::SurfaceEvent::ActionKeyPressed {
                    window,
                    modifiers,
                    key,
                }
            )) if window == wid
                && modifiers == keyboard::Modifiers::LOGO
                && key == keyboard::Key::Named(Named::Space)
        ));
    }

    #[test]
    fn plain_space_keeps_keyboard_secondary_activate() {
        let wid = iced::window::Id::unique();
        let msg = keyboard_handler(
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(Named::Space),
                modified_key: keyboard::Key::Named(Named::Space),
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Space),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::default(),
                text: None,
                repeat: false,
            }),
            iced::event::Status::Ignored,
            wid,
        );

        assert!(matches!(
            msg,
            Some(crate::app::Message::Surface(
                crate::app::messages::SurfaceEvent::KeyboardSecondaryActivate(window)
            )) if window == wid
        ));
    }

    #[test]
    fn captured_action_key_does_not_emit_surface_action_key_event() {
        let wid = iced::window::Id::unique();
        let msg = keyboard_handler(
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Character("e".into()),
                modified_key: keyboard::Key::Character("e".into()),
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyE),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::CTRL,
                text: None,
                repeat: false,
            }),
            iced::event::Status::Captured,
            wid,
        );

        assert!(msg.is_none());
    }
}
