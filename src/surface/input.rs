use std::sync::OnceLock;

use iced::event::listen_with;
use iced::keyboard::{self, key::Named};
use iced::window;
use iced::{Event, Subscription};

use crate::app::Message;

/// Parsed expand keybind, initialized once at startup from `BarConfig.expand_keybind`.
/// `None` outer = not yet initialized (before `init_expand_keybind` is called).
/// `Some(None)` = no keybind configured.
/// `Some(Some(...))` = a valid keybind.
static EXPAND_KEYBIND: OnceLock<Option<(keyboard::Modifiers, keyboard::Key)>> = OnceLock::new();

/// Parse a keybind string like `"Super+e"` or `"Ctrl+F5"` into `(Modifiers, Key)`.
///
/// Modifier tokens (prefix, separated by `+`): `Super`/`Logo`/`Win`, `Ctrl`/`Control`,
/// `Alt`, `Shift`. Key token (final): a single character → `Character` key; one of
/// `F1`–`F12`, `Space`, `Enter`/`Return` → `Named` key.
///
/// Returns `None` if the string is empty, contains an unknown modifier, or the key
/// token is not recognised.
pub fn parse_keybind(s: &str) -> Option<(keyboard::Modifiers, keyboard::Key)> {
    let parts: Vec<&str> = s.split('+').collect();
    let key_token = *parts.last().unwrap_or(&"");
    if key_token.is_empty() {
        return None;
    }
    let modifier_tokens = &parts[..parts.len() - 1];

    let mut mods = keyboard::Modifiers::default();
    for &m in modifier_tokens {
        match m {
            "Super" | "Logo" | "Win" => mods |= keyboard::Modifiers::LOGO,
            "Ctrl" | "Control" => mods |= keyboard::Modifiers::CTRL,
            "Alt" => mods |= keyboard::Modifiers::ALT,
            "Shift" => mods |= keyboard::Modifiers::SHIFT,
            _ => return None,
        }
    }

    let key: keyboard::Key = if key_token.chars().count() == 1 {
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

/// Store the parsed expand keybind. Must be called once before the event loop starts
/// (e.g., in `App::new`). Subsequent calls after the first are silently ignored because
/// config is immutable for the lifetime of the process.
pub fn init_expand_keybind(s: Option<&str>) {
    let _ = EXPAND_KEYBIND.set(s.and_then(parse_keybind));
}

/// Event handler for [`keyboard_focus_subscription`]. A `fn` pointer so it can be
/// used with `listen_with`.
fn keyboard_handler(
    event: Event,
    _status: iced::event::Status,
    window: iced::window::Id,
) -> Option<Message> {
    match event {
        Event::Window(window::Event::Focused) => Some(Message::BarFocusGained(window)),
        Event::Window(window::Event::Unfocused) => Some(Message::BarFocusLost(window)),
        Event::Window(window::Event::Opened { size, .. }) => {
            Some(Message::WindowSized(window, size.width as u32, size.height as u32))
        }
        Event::Window(window::Event::Resized(size)) => {
            Some(Message::WindowSized(window, size.width as u32, size.height as u32))
        }
        Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(Message::CursorMoved(window, position))
        }
        Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            // Expand keybind check takes priority over navigation bindings.
            // ToggleExpand is global (IPC design) — no per-surface window ID needed.
            if let Some(Some((kb_mods, kb_key))) = EXPAND_KEYBIND.get()
                && modifiers == *kb_mods
                && key == *kb_key
            {
                // Per-surface toggle: only affect the focused bar, not all bars.
                // (Message::ToggleExpand is reserved for the IPC path which toggles all surfaces.)
                return Some(Message::BarClick(window));
            }
            match key {
                keyboard::Key::Named(Named::Escape) => Some(Message::EscapePressed(window)),
                keyboard::Key::Named(Named::ArrowDown) => Some(Message::FocusMoveNext(window)),
                keyboard::Key::Named(Named::ArrowUp) => Some(Message::FocusMovePrev(window)),
                keyboard::Key::Named(Named::Tab) => {
                    if modifiers.shift() {
                        Some(Message::FocusPrevZone(window))
                    } else {
                        Some(Message::FocusNextZone(window))
                    }
                }
                keyboard::Key::Named(Named::Enter) => {
                    Some(Message::KeyboardActivate(window))
                }
                keyboard::Key::Named(Named::Space) => {
                    Some(Message::KeyboardSecondaryActivate(window))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Subscribe to keyboard focus events and key presses.
///
/// Call [`init_expand_keybind`] once before the event loop to activate the in-bar
/// keybind. While the bar has keyboard focus, pressing the configured combination
/// fires `Message::ToggleExpand`.
pub fn keyboard_focus_subscription() -> Subscription<Message> {
    listen_with(keyboard_handler)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_keybind_super_plus_char() {
        let (mods, key) = parse_keybind("Super+e").unwrap();
        assert!(mods.logo());
        assert!(!mods.control());
        assert_eq!(key, keyboard::Key::Character("e".into()));
    }

    #[test]
    fn parse_keybind_ctrl_f5() {
        let (mods, key) = parse_keybind("Ctrl+F5").unwrap();
        assert!(mods.control());
        assert!(!mods.logo());
        assert_eq!(key, keyboard::Key::Named(Named::F5));
    }

    #[test]
    fn parse_keybind_super_shift_char() {
        let (mods, key) = parse_keybind("Super+Shift+t").unwrap();
        assert!(mods.logo());
        assert!(mods.shift());
        assert_eq!(key, keyboard::Key::Character("t".into()));
    }

    #[test]
    fn parse_keybind_no_modifier() {
        let (mods, key) = parse_keybind("e").unwrap();
        assert_eq!(mods, keyboard::Modifiers::default());
        assert_eq!(key, keyboard::Key::Character("e".into()));
    }

    #[test]
    fn parse_keybind_unknown_modifier_returns_none() {
        assert!(parse_keybind("Meta+e").is_none());
    }

    #[test]
    fn parse_keybind_unknown_key_returns_none() {
        assert!(parse_keybind("Super+Home").is_none());
    }

    #[test]
    fn parse_keybind_empty_string_returns_none() {
        assert!(parse_keybind("").is_none());
    }

    #[test]
    fn parse_keybind_all_fkeys() {
        for (name, named) in [
            ("F1", Named::F1),
            ("F2", Named::F2),
            ("F10", Named::F10),
            ("F12", Named::F12),
        ] {
            let (_, key) = parse_keybind(&format!("Super+{name}")).unwrap();
            assert_eq!(key, keyboard::Key::Named(named));
        }
    }

    #[test]
    fn parse_keybind_space_and_enter() {
        let (_, key) = parse_keybind("Super+Space").unwrap();
        assert_eq!(key, keyboard::Key::Named(Named::Space));

        let (_, key) = parse_keybind("Alt+Enter").unwrap();
        assert_eq!(key, keyboard::Key::Named(Named::Enter));

        let (_, key) = parse_keybind("Ctrl+Return").unwrap();
        assert_eq!(key, keyboard::Key::Named(Named::Enter));
    }
}
