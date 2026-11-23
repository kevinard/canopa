use super::{Element, Message, State, column, kit, settings_group, text};
use canopa_ui::accessibility::{Node, semantic};
use canopa_ui::direction;
use iced::{Alignment, Length};

impl State {
    pub(super) fn shortcuts_view(&self) -> Element<'_, Message> {
        let editor = match &self.shortcuts {
            None => return kit::caption(fl!("settings-shortcuts-loading")),
            Some(Err(error)) => {
                return settings_group(
                    fl!("settings-shortcuts-unavailable"),
                    text(error).style(canopa_ui::muted),
                );
            }
            Some(Ok(editor)) => editor,
        };
        let editable = self.paths.active() && !self.busy;
        let mut head = column![super::hinted_field(
            "settings-shortcut-search",
            fl!("settings-shortcuts-search"),
            &fl!("settings-shortcuts-search-placeholder"),
            &self.shortcut_query,
            Message::ShortcutSearch,
        )]
        .spacing(12);
        if editable {
            head = head.push(kit::caption(fl!("settings-shortcuts-select-hint")));
        }
        if editor.migration_pending() {
            head = head.push(kit::caption(fl!("settings-shortcuts-migration")));
        }
        if !self.paths.active() {
            head = head.push(kit::caption(fl!("settings-shortcuts-needs-session")));
        }
        let query = self.shortcut_query.trim().to_lowercase();
        // Rows by what they are for, in the shipped order within each group.
        let mut groups: Vec<(crate::shortcuts::Category, Vec<Element<'_, Message>>)> =
            crate::shortcuts::Category::ALL
                .into_iter()
                .map(|category| (category, Vec::new()))
                .collect();
        // Rows whose keys another row also uses, by normalized combination.
        let mut uses: std::collections::HashMap<String, Vec<usize>> = Default::default();
        for (index, shortcut) in editor.rows.iter().enumerate() {
            if let Some(key) = crate::shortcuts::normalize(shortcut.key.trim()) {
                uses.entry(key).or_default().push(index);
            }
        }
        let clash = |index: usize, key: &str| {
            crate::shortcuts::normalize(key.trim())
                .and_then(|key| uses.get(&key))
                .and_then(|rows| rows.iter().find(|other| **other != index))
                .map(|other| editor.rows[*other].label.clone())
        };
        for (index, shortcut) in editor.rows.iter().enumerate() {
            if !shortcut.label.to_lowercase().contains(&query)
                && !shortcut.saved_key.to_lowercase().contains(&query)
                && !readable(&shortcut.key).to_lowercase().contains(&query)
            {
                continue;
            }
            let rows = &mut groups
                .iter_mut()
                .find(|(category, _)| *category == shortcut.category())
                .expect("every category is listed")
                .1;
            let warning = clash(index, &shortcut.key)
                .map(|other| fl!("settings-shortcut-clash", action = other.as_str()));
            let editing = (editable || self.busy) && self.shortcut_editing == Some(index);
            rows.push(if editing && self.shortcut_typing {
                warned_row(
                    shortcut.label.clone(),
                    Some(fl!("settings-shortcuts-edit-hint")),
                    warning,
                    direction::row([
                        super::key_field(
                            format!("settings-shortcut-{index}"),
                            shortcut.label.clone(),
                            &fl!("settings-shortcut-off-placeholder"),
                            &shortcut.key,
                            move |value| Message::ShortcutKey(index, value),
                        ),
                        self.action(
                            format!("settings-shortcut-{index}-done"),
                            fl!("settings-shortcut-done"),
                            Some(Message::EditShortcut(None)),
                            false,
                        ),
                    ])
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
            } else if editing {
                warned_row(
                    shortcut.label.clone(),
                    Some(if self.shortcut_unsupported {
                        fl!("settings-shortcut-unsupported")
                    } else {
                        fl!("settings-shortcut-record-hint")
                    }),
                    None,
                    direction::row([
                        recording(),
                        self.treated_action(
                            format!("settings-shortcut-{index}-type"),
                            fl!("settings-shortcut-type"),
                            Some(Message::TypeShortcut),
                            kit::Treatment::Flat,
                        ),
                        self.action(
                            format!("settings-shortcut-{index}-cancel"),
                            fl!("settings-shortcut-cancel"),
                            Some(Message::EditShortcut(None)),
                            false,
                        ),
                    ])
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
            } else if editable || self.busy {
                warned_row(
                    shortcut.label.clone(),
                    None,
                    warning,
                    keycaps(
                        format!("settings-shortcut-{index}-change"),
                        fl!("settings-shortcut-change", action = shortcut.label.as_str()),
                        &shortcut.key,
                        Message::EditShortcut(Some(index)),
                    ),
                )
            } else {
                kit::row(
                    shortcut.label.clone(),
                    None,
                    text(if shortcut.key.trim().is_empty() {
                        fl!("settings-shortcut-off-placeholder")
                    } else {
                        readable(&shortcut.key)
                    })
                    .style(canopa_ui::muted),
                )
            });
        }
        let mut sections = column![head].spacing(24);
        let mut any = false;
        for (category, rows) in groups {
            if rows.is_empty() {
                continue;
            }
            any = true;
            sections = sections.push(settings_group(category_title(category), kit::rows(rows)));
        }
        if !any {
            sections = sections.push(settings_group(
                fl!("settings-shortcuts-list"),
                kit::empty(fl!("settings-shortcuts-none-match")),
            ));
        }
        if editable {
            sections = sections.push(
                iced::widget::container(kit::row(
                    fl!("settings-shortcuts-reset"),
                    Some(fl!("settings-shortcuts-reset-hint")),
                    self.action(
                        "settings-button-reset-to-defaults",
                        fl!("settings-shortcuts-reset-short"),
                        Some(Message::ResetShortcuts),
                        false,
                    ),
                ))
                .padding(kit::ROW_PADDING)
                .width(Length::Fill)
                .style(kit::card),
            );
        }
        sections
            .push(kit::footnote(fl!("settings-shortcuts-intro")))
            .spacing(24)
            .into()
    }
}

/// A setting row whose explanation may carry a warning in its own colour.
fn warned_row<'a>(
    label: String,
    description: Option<String>,
    warning: Option<String>,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let Some(warning) = warning else {
        return kit::row(label, description, control);
    };
    let mut lead = column![text(label).size(kit::size::BODY)].spacing(2);
    if let Some(description) = description {
        lead = lead.push(kit::caption(description));
    }
    lead = lead.push(
        direction::row([
            kit::glyph("warning", 14.0, kit::Tint::Warning),
            text(warning)
                .size(kit::size::CAPTION)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(canopa_ui::colors(theme).warning),
                })
                .into(),
        ])
        .spacing(6)
        .align_y(Alignment::Center),
    );
    direction::row([lead.width(Length::Fill).into(), control.into()])
        .spacing(16)
        .align_y(Alignment::Center)
        .into()
}

/// Waiting for keys: an accent outline where the key caps were.
fn recording<'a>() -> Element<'a, Message> {
    semantic(
        Node::text(fl!("settings-shortcut-recording")),
        iced::widget::container(
            text(fl!("settings-shortcut-recording"))
                .size(kit::size::CAPTION)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(canopa_ui::colors(theme).accent),
                }),
        )
        .padding([5, 12])
        .style(|theme: &iced::Theme| {
            let p = canopa_ui::colors(theme);
            iced::widget::container::Style {
                background: Some(kit::blend(p.surface, p.accent, 0.12).into()),
                border: iced::Border {
                    color: p.accent,
                    width: 1.5,
                    radius: kit::CONTROL_RADIUS.into(),
                },
                ..Default::default()
            }
        }),
    )
}

/// A key combination drawn as key caps; pressing it records a new one.
fn keycaps<'a>(
    key: String,
    label: String,
    combination: &str,
    edit: Message,
) -> Element<'a, Message> {
    let caps: Vec<Element<'a, Message>> = if combination.trim().is_empty() {
        vec![kit::caption(fl!("settings-shortcut-off-placeholder"))]
    } else {
        combination
            .split('+')
            .filter(|part| !part.trim().is_empty())
            .map(|part| {
                iced::widget::container(text(key_label(part.trim())).size(kit::size::SMALL))
                    .padding([2, 7])
                    .style(|theme: &iced::Theme| {
                        let p = canopa_ui::colors(theme);
                        iced::widget::container::Style {
                            background: Some(p.raised.into()),
                            border: iced::Border {
                                color: kit::blend(p.raised, p.border, 0.5),
                                width: 1.0,
                                radius: 5.0.into(),
                            },
                            shadow: iced::Shadow {
                                color: kit::blend(p.raised, p.background, 0.7),
                                offset: iced::Vector::new(0.0, 1.5),
                                blur_radius: 0.0,
                            },
                            ..Default::default()
                        }
                    })
                    .into()
            })
            .collect()
    };
    semantic(
        Node::button(label)
            .key(key.clone())
            .on_click(Some(edit.clone())),
        super::focus::control(
            key,
            Some(edit.clone()),
            None,
            None,
            false,
            // The caps, then a quiet pencil: pressing the row edits it.
            iced::widget::button(
                direction::row(
                    caps.into_iter().chain([iced::widget::container(kit::glyph(
                        "pencil",
                        14.0,
                        kit::Tint::Muted,
                    ))
                    .padding(iced::Padding::ZERO.left(8))
                    .into()]),
                )
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .on_press(edit)
            .padding([4, 6])
            // Quiet until hovered: the caps are the content.
            .style(|theme, status| kit::button_style(theme, status, kit::Treatment::Flat)),
        ),
    )
}

/// What a key press means while a shortcut is recorded.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Press {
    /// A full combination, in the compositor's syntax.
    Combination(String),
    /// Only a modifier so far.
    Modifier,
    /// Escape: keep the shortcut as it was.
    Cancel,
    /// Backspace or Delete: turn the shortcut off.
    Clear,
    /// A key the compositor has no name for.
    Unsupported,
}

/// The combination `key` with `modifiers` makes. The Super key is written
/// `Mod`, as in the shipped shortcuts.
pub(super) fn press(key: &iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> Press {
    use iced::keyboard::{Key, key::Named};
    let plain = modifiers.is_empty();
    let name = match key {
        Key::Named(Named::Escape) if plain => return Press::Cancel,
        Key::Named(Named::Backspace | Named::Delete) if plain => return Press::Clear,
        Key::Named(
            Named::Shift
            | Named::Control
            | Named::Alt
            | Named::AltGraph
            | Named::Super
            | Named::Meta
            | Named::Hyper
            | Named::Fn
            | Named::FnLock
            | Named::CapsLock
            | Named::NumLock
            | Named::Symbol
            | Named::SymbolLock,
        ) => return Press::Modifier,
        Key::Named(named) => match keysym(*named) {
            Some(name) => name.to_owned(),
            None => return Press::Unsupported,
        },
        Key::Character(text) => {
            let mut chars = text.chars();
            let (Some(character), None) = (chars.next(), chars.next()) else {
                return Press::Unsupported;
            };
            let keysym = xkbcommon::xkb::utf32_to_keysym(
                character.to_lowercase().next().unwrap_or(character) as u32,
            );
            let name = xkbcommon::xkb::keysym_get_name(keysym);
            if keysym.raw() == 0 || name.is_empty() || name == "NoSymbol" {
                return Press::Unsupported;
            }
            // Letters as the shipped shortcuts write them: Mod+T.
            if name.len() == 1 {
                name.to_uppercase()
            } else {
                name
            }
        }
        Key::Unidentified => return Press::Unsupported,
    };
    let mut parts: Vec<&str> = Vec::new();
    if modifiers.logo() {
        parts.push("Mod");
    }
    if modifiers.control() {
        parts.push("Ctrl");
    }
    if modifiers.alt() {
        parts.push("Alt");
    }
    if modifiers.shift() {
        parts.push("Shift");
    }
    parts.push(&name);
    Press::Combination(parts.join("+"))
}

/// The keysym name of a named key, as the compositor reads it.
fn keysym(named: iced::keyboard::key::Named) -> Option<&'static str> {
    use iced::keyboard::key::Named::*;
    Some(match named {
        Enter => "Return",
        Tab => "Tab",
        Space => "space",
        ArrowDown => "Down",
        ArrowLeft => "Left",
        ArrowRight => "Right",
        ArrowUp => "Up",
        End => "End",
        Home => "Home",
        PageDown => "Page_Down",
        PageUp => "Page_Up",
        Backspace => "BackSpace",
        Delete => "Delete",
        Insert => "Insert",
        Escape => "Escape",
        PrintScreen => "Print",
        Pause => "Pause",
        ScrollLock => "Scroll_Lock",
        ContextMenu => "Menu",
        AudioVolumeUp => "XF86AudioRaiseVolume",
        AudioVolumeDown => "XF86AudioLowerVolume",
        AudioVolumeMute => "XF86AudioMute",
        MicrophoneVolumeMute => "XF86AudioMicMute",
        MediaPlay | MediaPlayPause => "XF86AudioPlay",
        MediaPause => "XF86AudioPause",
        MediaStop => "XF86AudioStop",
        MediaTrackNext => "XF86AudioNext",
        MediaTrackPrevious => "XF86AudioPrev",
        BrightnessUp => "XF86MonBrightnessUp",
        BrightnessDown => "XF86MonBrightnessDown",
        Eject => "XF86Eject",
        LaunchMail => "XF86Mail",
        BrowserHome => "XF86HomePage",
        BrowserSearch => "XF86Search",
        F1 => "F1",
        F2 => "F2",
        F3 => "F3",
        F4 => "F4",
        F5 => "F5",
        F6 => "F6",
        F7 => "F7",
        F8 => "F8",
        F9 => "F9",
        F10 => "F10",
        F11 => "F11",
        F12 => "F12",
        F13 => "F13",
        F14 => "F14",
        F15 => "F15",
        F16 => "F16",
        F17 => "F17",
        F18 => "F18",
        F19 => "F19",
        F20 => "F20",
        F21 => "F21",
        F22 => "F22",
        F23 => "F23",
        F24 => "F24",
        _ => return None,
    })
}

/// One key of a combination as printed on keyboards: "Mod" is Super,
/// keysym names become symbols or words.
fn key_label(part: &str) -> String {
    let lower = part.to_ascii_lowercase();
    match lower.as_str() {
        "mod" | "super" | "win" => fl!("settings-shortcut-super"),
        "ctrl" | "control" => fl!("settings-key-ctrl"),
        "alt" => "Alt".into(),
        "shift" => fl!("settings-key-shift"),
        "return" | "enter" | "kp_enter" => fl!("settings-key-enter"),
        "space" => fl!("settings-key-space"),
        "backspace" => fl!("settings-key-backspace"),
        "delete" => fl!("settings-key-delete"),
        "escape" => fl!("settings-key-escape"),
        "tab" => fl!("settings-key-tab"),
        "page_up" | "prior" => fl!("settings-key-page-up"),
        "page_down" | "next" => fl!("settings-key-page-down"),
        "home" => fl!("settings-key-home"),
        "end" => fl!("settings-key-end"),
        "print" => fl!("settings-key-print"),
        "left" => "←".into(),
        "right" => "→".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "comma" => ",".into(),
        "period" => ".".into(),
        "slash" => "/".into(),
        "backslash" => "\\".into(),
        "minus" => "−".into(),
        "equal" => "=".into(),
        "plus" => "+".into(),
        "semicolon" => ";".into(),
        "apostrophe" => "'".into(),
        "grave" => "`".into(),
        "bracketleft" => "[".into(),
        "bracketright" => "]".into(),
        "xf86audioraisevolume" => fl!("settings-key-volume-up"),
        "xf86audiolowervolume" => fl!("settings-key-volume-down"),
        "xf86audiomute" => fl!("settings-key-mute"),
        "xf86audiomicmute" => fl!("settings-key-mic-mute"),
        "xf86audioplay" | "xf86audiopause" => fl!("settings-key-play"),
        "xf86audiostop" => fl!("settings-key-stop"),
        "xf86audionext" => fl!("settings-key-next"),
        "xf86audioprev" => fl!("settings-key-previous"),
        "xf86monbrightnessup" => fl!("settings-key-brightness-up"),
        "xf86monbrightnessdown" => fl!("settings-key-brightness-down"),
        "wheelscrollup" => fl!("settings-key-scroll-up"),
        "wheelscrolldown" => fl!("settings-key-scroll-down"),
        "wheelscrollleft" => fl!("settings-key-scroll-left"),
        "wheelscrollright" => fl!("settings-key-scroll-right"),
        _ if part.chars().count() == 1 => part.to_uppercase(),
        _ => part.replace('_', " "),
    }
}

/// A combination read aloud and searched: its key labels joined by "+".
fn readable(combination: &str) -> String {
    combination
        .split('+')
        .filter(|part| !part.trim().is_empty())
        .map(|part| key_label(part.trim()))
        .collect::<Vec<_>>()
        .join(" + ")
}

fn category_title(category: crate::shortcuts::Category) -> String {
    use crate::shortcuts::Category;
    match category {
        Category::Session => fl!("settings-shortcuts-group-session"),
        Category::Windows => fl!("settings-shortcuts-group-windows"),
        Category::Workspaces => fl!("settings-shortcuts-group-workspaces"),
        Category::Screenshots => fl!("settings-shortcuts-group-screenshots"),
        Category::Keys => fl!("settings-shortcuts-group-keys"),
        Category::Other => fl!("settings-shortcuts-group-other"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::{Key, Modifiers, key::Named};

    #[test]
    fn a_press_reads_as_the_compositor_writes_it() {
        assert_eq!(
            press(&Key::Character("t".into()), Modifiers::LOGO),
            Press::Combination("Mod+T".into())
        );
        assert_eq!(
            press(
                &Key::Character(",".into()),
                Modifiers::LOGO | Modifiers::SHIFT
            ),
            Press::Combination("Mod+Shift+comma".into())
        );
        assert_eq!(
            press(
                &Key::Named(Named::ArrowLeft),
                Modifiers::CTRL | Modifiers::LOGO
            ),
            Press::Combination("Mod+Ctrl+Left".into())
        );
        assert_eq!(
            press(&Key::Named(Named::AudioVolumeUp), Modifiers::empty()),
            Press::Combination("XF86AudioRaiseVolume".into())
        );
        assert_eq!(
            press(&Key::Named(Named::Shift), Modifiers::SHIFT),
            Press::Modifier
        );
        assert_eq!(
            press(&Key::Named(Named::Escape), Modifiers::empty()),
            Press::Cancel
        );
        assert_eq!(
            press(&Key::Named(Named::Backspace), Modifiers::empty()),
            Press::Clear
        );
        assert_eq!(
            press(&Key::Named(Named::Backspace), Modifiers::LOGO),
            Press::Combination("Mod+BackSpace".into())
        );
        // Every recorded combination is one the shortcut editor accepts.
        for key in [Key::Character("é".into()), Key::Named(Named::PrintScreen)] {
            let Press::Combination(combination) = press(&key, Modifiers::LOGO) else {
                panic!("{key:?} is a key");
            };
            assert!(
                crate::shortcuts::normalize(&combination).is_some(),
                "{combination}"
            );
        }
    }

    #[test]
    fn key_caps_name_keys_as_keyboards_print_them() {
        assert_eq!(readable("Mod+Shift+comma"), "Super + Shift + ,");
        assert_eq!(readable("Mod+Return"), "Super + Enter");
        assert_eq!(readable("XF86AudioRaiseVolume"), "Volume Up");
        assert_eq!(readable("Mod+Page_Down"), "Super + Page Down");
    }
}
