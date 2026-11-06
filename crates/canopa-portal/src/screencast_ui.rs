//! Explicit, searchable monitor/window consent in the shared Iced appearance.
use crate::{
    screencast::{DialogInput, Item},
    screencast_capture::Source,
};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{column, container, row, scrollable, text, text_input},
    window,
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

pub fn run(
    input: DialogInput,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<Option<Vec<Source>>> {
    Ok(run_inner(input, 0, false, false, appearance)?.map(|selected| selected.sources))
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub sources: Vec<Source>,
    pub devices: u32,
    #[serde(default)]
    pub clipboard: bool,
}

pub fn run_remote(
    input: DialogInput,
    devices: u32,
    clipboard_requested: bool,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<Option<Selection>> {
    anyhow::ensure!(
        devices & !7 == 0 && (devices & 4 == 0 || input.types != 0),
        "Unsupported input device selection"
    );
    run_inner(input, devices, clipboard_requested, false, appearance)
}

pub fn run_screenshot(
    input: DialogInput,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<Option<Vec<Source>>> {
    anyhow::ensure!(
        input.types == 2
            && !input.multiple
            && input
                .items
                .iter()
                .all(|item| matches!(item.source, Source::Window(_))),
        "Invalid screenshot window catalog"
    );
    Ok(run_inner(input, 0, false, true, appearance)?.map(|selected| selected.sources))
}

fn run_inner(
    input: DialogInput,
    devices: u32,
    clipboard_requested: bool,
    screenshot: bool,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<Option<Selection>> {
    anyhow::ensure!(
        input.items.len() <= 528
            && if input.types == 0 {
                input.items.is_empty() && (devices != 0 || clipboard_requested)
            } else {
                !input.items.is_empty()
            },
        "Invalid capture source catalog"
    );
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    iced::daemon(
        move || {
            let mut settings = window::Settings {
                size: if input.types == 0 {
                    Size::new(520.0, if devices & 2 != 0 { 420.0 } else { 340.0 })
                } else {
                    Size::new(
                        680.0,
                        if devices & 2 != 0 {
                            740.0
                        } else if devices == 0 {
                            600.0
                        } else {
                            660.0
                        },
                    )
                },
                min_size: Some(Size::new(
                    420.0,
                    if input.types == 0 { 300.0 } else { 420.0 },
                )),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            if devices & 4 != 0 {
                settings.size.height += 80.0;
            }
            if clipboard_requested && (devices != 0 || input.types != 0) {
                settings.size.height += 80.0;
            }
            let (_window, opening) = window::open_dialog_with_activation(
                input
                    .parent_window
                    .strip_prefix("wayland:")
                    .unwrap_or_default()
                    .to_owned(),
                input.modal,
                input.activation_token.clone(),
                settings,
            );
            (
                Dialog {
                    screenshot,
                    input: input.clone(),
                    items: input.items.clone(),
                    theme: theme.clone(),
                    query: String::new(),
                    selected: HashSet::new(),
                    requested_devices: devices,
                    requested_clipboard: clipboard_requested,
                    clipboard: false,
                    keyboard: false,
                    pointer: false,
                    touch: false,
                    highlight: None,
                    available: true,
                    completed: false,
                    output: output.clone(),
                },
                Task::batch([
                    opening.map(|_| Message::Opened),
                    Task::run(updates(input.clone()), Message::Loaded),
                ]),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|app: &Dialog, _| format!("{} — Canopa", app.title()))
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(Dialog::subscription)
    .default_font(font)
    .run()
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(result.lock().expect("capture consent result").take())
}
#[derive(Clone, Debug)]
enum Message {
    Opened,
    Query(String),
    Toggle(usize),
    Keyboard(bool),
    Pointer(bool),
    Touch(bool),
    Clipboard(bool),
    Loaded(Result<Vec<Item>, String>),
    Share,
    Cancel,
    Event(Event),
}
struct Dialog {
    screenshot: bool,
    input: DialogInput,
    items: Vec<Item>,
    theme: Theme,
    query: String,
    selected: HashSet<Source>,
    requested_devices: u32,
    requested_clipboard: bool,
    clipboard: bool,
    keyboard: bool,
    pointer: bool,
    touch: bool,
    highlight: Option<usize>,
    available: bool,
    completed: bool,
    output: Arc<Mutex<Option<Selection>>>,
}
impl Dialog {
    fn devices(&self) -> u32 {
        (u32::from(self.keyboard) | (u32::from(self.pointer) << 1) | (u32::from(self.touch) << 2))
            & self.requested_devices
    }
    fn visible(&self) -> Vec<usize> {
        let query = self.query.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                format!("{} {}", item.title, item.subtitle)
                    .to_lowercase()
                    .contains(&query)
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn valid(&self) -> bool {
        self.available
            && if self.input.types == 0 {
                self.selected.is_empty()
                    && (self.devices() != 0 || self.clipboard && self.requested_clipboard)
            } else {
                !self.selected.is_empty()
                    && self.selected.len() <= 16
                    && (self.input.multiple || self.selected.len() == 1)
            }
    }
    fn title(&self) -> &'static str {
        if self.screenshot {
            return "Choose a screenshot window";
        }
        if self.requested_clipboard && self.input.types == 0 {
            return if self.requested_devices == 0 {
                "Allow clipboard sharing"
            } else {
                "Allow desktop sharing"
            };
        }
        if self.requested_devices != 0 {
            if self.input.types == 0 {
                match self.requested_devices {
                    1 => "Allow keyboard control",
                    2 => "Allow pointer control",
                    _ => "Allow desktop control",
                }
            } else {
                "Share and allow control"
            }
        } else {
            "Share your screen"
        }
    }
    fn action(&self) -> &'static str {
        if self.screenshot {
            return "Capture window";
        }
        if self.requested_clipboard && self.input.types == 0 && self.requested_devices == 0 {
            return "Share clipboard";
        }
        if self.clipboard && self.requested_clipboard {
            return if self.input.types == 0 {
                if self.devices() == 0 {
                    "Share clipboard"
                } else {
                    "Allow sharing"
                }
            } else if self.devices() == 0 {
                "Share screen and clipboard"
            } else {
                "Share and allow control"
            };
        }
        if self.input.types == 0 && self.requested_devices != 0 {
            return "Allow control";
        }
        if self.devices() != 0 {
            "Share and allow control"
        } else {
            "Share"
        }
    }
    fn reconcile(&mut self) {
        let allowed: HashSet<_> = self
            .visible()
            .into_iter()
            .map(|i| self.items[i].source.clone())
            .collect();
        self.selected.retain(|source| allowed.contains(source));
        self.highlight = None;
    }
    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen_with(|event, status, _| {
            let shortcut = matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            ) || matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Enter),
                    modifiers,
                    ..
                }) if *modifiers == keyboard::Modifiers::CTRL
            ) || matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(
                        keyboard::key::Named::ArrowDown | keyboard::key::Named::ArrowUp
                    ),
                    modifiers,
                    ..
                }) if modifiers.is_empty()
            );
            let shortcut = shortcut
                || matches!(&event, Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Character(value), modifiers, ..
            }) if (value.eq_ignore_ascii_case("c") || value.eq_ignore_ascii_case("t")) && *modifiers == keyboard::Modifiers::ALT);
            (shortcut || status == iced::event::Status::Ignored).then_some(Message::Event(event))
        })
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Opened => {}
            Message::Cancel => {
                self.completed = true;
                return iced::exit();
            }
            Message::Share if self.valid() => {
                let sources = self
                    .items
                    .iter()
                    .filter(|item| self.selected.contains(&item.source))
                    .map(|i| i.source.clone())
                    .collect();
                *self.output.lock().expect("capture consent result") = Some(Selection {
                    sources,
                    devices: self.devices(),
                    clipboard: self.clipboard && self.requested_clipboard,
                });
                self.completed = true;
                return iced::exit();
            }
            Message::Share => {}
            Message::Query(query) => {
                self.query = query;
                self.reconcile();
            }
            Message::Toggle(index) => {
                if self.input.types != 0
                    && self.visible().contains(&index)
                    && let Some(item) = self.items.get(index)
                {
                    if self.selected.remove(&item.source) {
                    } else if !self.input.multiple {
                        self.selected.clear();
                        self.selected.insert(item.source.clone());
                    } else if self.selected.len() < 16 {
                        self.selected.insert(item.source.clone());
                    }
                    self.highlight = Some(index);
                }
            }
            Message::Keyboard(allowed) => {
                if self.requested_devices & 1 != 0 {
                    self.keyboard = allowed;
                }
            }
            Message::Pointer(allowed) => {
                if self.requested_devices & 2 != 0 {
                    self.pointer = allowed;
                }
            }
            Message::Touch(allowed) => {
                if self.requested_devices & 4 != 0 && self.input.types != 0 {
                    self.touch = allowed;
                }
            }
            Message::Clipboard(allowed) => {
                if self.requested_clipboard {
                    self.clipboard = allowed;
                }
            }
            Message::Loaded(result) => {
                let highlighted = self
                    .highlight
                    .and_then(|i| self.items.get(i))
                    .map(|i| i.source.clone());
                match result {
                    Ok(items) => {
                        self.items = items;
                        self.available = true;
                        self.reconcile();
                        self.highlight = highlighted.and_then(|source| {
                            self.visible()
                                .into_iter()
                                .find(|i| self.items[*i].source == source)
                        });
                    }
                    Err(_) => self.available = false,
                }
            }
            Message::Event(Event::Window(window::Event::CloseRequested)) => {
                return self.update(Message::Cancel);
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => match key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                    return self.update(Message::Cancel);
                }
                keyboard::Key::Named(keyboard::key::Named::Enter)
                    if modifiers == keyboard::Modifiers::CTRL =>
                {
                    return self.update(Message::Share);
                }
                keyboard::Key::Character(value)
                    if value.eq_ignore_ascii_case("c") && modifiers == keyboard::Modifiers::ALT =>
                {
                    return self.update(Message::Clipboard(!self.clipboard));
                }
                keyboard::Key::Character(value)
                    if value.eq_ignore_ascii_case("t") && modifiers == keyboard::Modifiers::ALT =>
                {
                    return self.update(Message::Touch(!self.touch));
                }
                keyboard::Key::Named(keyboard::key::Named::Tab) => {
                    return if modifiers.shift() {
                        iced::widget::operation::focus_previous()
                    } else {
                        iced::widget::operation::focus_next()
                    };
                }
                keyboard::Key::Named(
                    keyboard::key::Named::ArrowDown | keyboard::key::Named::ArrowUp,
                ) if modifiers.is_empty() => {
                    let visible = self.visible();
                    if !visible.is_empty() {
                        let old = self
                            .highlight
                            .and_then(|i| visible.iter().position(|v| *v == i));
                        let down = key == keyboard::Key::Named(keyboard::key::Named::ArrowDown);
                        let next = match (old, down) {
                            (None, _) => 0,
                            (Some(i), true) => (i + 1).min(visible.len() - 1),
                            (Some(i), false) => i.saturating_sub(1),
                        };
                        self.highlight = Some(visible[next]);
                        return Task::batch([
                            iced::advanced::widget::operate(
                                iced::advanced::widget::operation::focusable::unfocus(),
                            ),
                            iced::widget::operation::snap_to(
                                "capture-sources",
                                iced::widget::operation::RelativeOffset {
                                    x: None,
                                    y: Some(
                                        next as f32 / visible.len().saturating_sub(1).max(1) as f32,
                                    ),
                                },
                            ),
                        ]);
                    }
                }
                keyboard::Key::Named(keyboard::key::Named::Space) => {
                    if let Some(index) = self.highlight {
                        return self.update(Message::Toggle(index));
                    }
                }
                _ => {}
            },
            Message::Event(_) => {}
        }
        Task::none()
    }
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let requester: String = self.input.app_id.chars().take(120).collect();
        let requester = if requester.is_empty() {
            "An application"
        } else {
            &requester
        };
        let mut choices = column![].spacing(6);
        for index in self.visible() {
            let item = &self.items[index];
            let selected = self.selected.contains(&item.source);
            let marker = match (self.input.multiple, selected) {
                (true, true) => "☑",
                (true, false) => "☐",
                (false, true) => "●",
                (false, false) => "○",
            };
            choices = choices.push(
                canopa_ui::action(
                    row![
                        text(marker).size(18),
                        column![
                            text(&item.title)
                                .size(16)
                                .width(Length::Fill)
                                .height(22)
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(&item.subtitle)
                                .size(12)
                                .style(canopa_ui::muted)
                                .width(Length::Fill)
                                .height(16)
                                .wrapping(iced::widget::text::Wrapping::None)
                        ]
                        .spacing(3)
                        .width(Length::Fill)
                    ]
                    .spacing(10)
                    .width(Length::Fill),
                    Some(Message::Toggle(index)),
                    canopa_ui::Action::Normal,
                    selected || self.highlight == Some(index),
                )
                .width(Length::Fill)
                .height(64),
            );
        }
        let mut body = column![
            text(self.title()).size(22),
            text(format!("Requested by {requester}"))
                .size(14)
                .height(20)
                .wrapping(iced::widget::text::Wrapping::None)
                .style(canopa_ui::muted),
        ]
        .spacing(12);
        if self.requested_devices & 1 != 0 {
            body = body.push(
                canopa_ui::action(
                    row![
                        text(if self.keyboard { "☑" } else { "☐" }).size(18),
                        column![
                            text("Allow keyboard control of your desktop").size(14),
                            text("Type and use keyboard shortcuts.")
                                .size(12)
                                .style(canopa_ui::muted),
                        ]
                        .spacing(4)
                        .width(Length::Fill),
                    ]
                    .spacing(10)
                    .width(Length::Fill),
                    Some(Message::Keyboard(!self.keyboard)),
                    canopa_ui::Action::Normal,
                    self.keyboard,
                )
                .width(Length::Fill),
            );
        }
        if self.requested_devices & 2 != 0 {
            body = body.push(
                canopa_ui::action(
                    row![
                        text(if self.pointer { "☑" } else { "☐" }).size(18),
                        column![
                            text("Allow pointer control of your desktop").size(14),
                            text("Move the pointer, click, drag and scroll.")
                                .size(12)
                                .style(canopa_ui::muted),
                        ]
                        .spacing(4)
                        .width(Length::Fill),
                    ]
                    .spacing(10)
                    .width(Length::Fill),
                    Some(Message::Pointer(!self.pointer)),
                    canopa_ui::Action::Normal,
                    self.pointer,
                )
                .width(Length::Fill),
            );
        }
        if self.requested_devices & 4 != 0 {
            body = body.push(
                canopa_ui::action(
                    row![
                        text(if self.touch { "☑" } else { "☐" }).size(18),
                        column![
                            text("Allow touch control (Alt+T)").size(14),
                            text("Touch and swipe within the screen or window you share.")
                                .size(12)
                                .style(canopa_ui::muted)
                        ]
                        .spacing(4)
                        .width(Length::Fill)
                    ]
                    .spacing(10)
                    .width(Length::Fill),
                    Some(Message::Touch(!self.touch)),
                    canopa_ui::Action::Normal,
                    self.touch,
                )
                .width(Length::Fill),
            );
        }
        if self.requested_clipboard {
            body = body.push(
                canopa_ui::action(
                    row![
                        text(if self.clipboard { "☑" } else { "☐" }).size(18),
                        column![
                            text("Share clipboard contents (Alt+C)").size(14),
                            text("Read and replace copied text, images and other data.")
                                .size(12)
                                .style(canopa_ui::muted)
                        ]
                        .spacing(4)
                        .width(Length::Fill),
                    ]
                    .spacing(10)
                    .width(Length::Fill),
                    Some(Message::Clipboard(!self.clipboard)),
                    canopa_ui::Action::Normal,
                    self.clipboard,
                )
                .width(Length::Fill),
            );
        }
        if self.input.types != 0 {
            body = body
                .push(
                    text(match (self.input.types, self.input.multiple) {
                        (1, false) => "Choose a display to share.",
                        (1, true) => "Choose the displays to share.",
                        (2, false) => "Choose a window to share.",
                        (2, true) => "Choose the windows to share.",
                        (_, false) => "Choose a display or window to share.",
                        (_, true) => "Choose the displays or windows to share.",
                    })
                    .size(13),
                )
                .push(
                    text_input("Search displays and windows", &self.query).on_input(Message::Query),
                )
                .push(
                    scrollable(choices)
                        .id("capture-sources")
                        .height(Length::Fill),
                );
        } else {
            body = body.push(iced::widget::Space::new().height(Length::Fill));
        }
        let mut status = if !self.available {
            if self.input.types == 0 {
                "Desktop unavailable. Retrying…"
            } else {
                "Sources unavailable. Retrying…"
            }
            .to_owned()
        } else if self.input.types == 0 && self.requested_clipboard && self.requested_devices == 0 {
            if self.clipboard {
                "Clipboard sharing allowed"
            } else {
                "Choose whether to share your clipboard"
            }
            .to_owned()
        } else if self.input.types == 0 {
            match self.devices() {
                1 => "Keyboard control allowed",
                2 => "Pointer control allowed",
                3 => "Keyboard and pointer control allowed",
                _ => "Choose whether to allow control",
            }
            .to_owned()
        } else {
            format!(
                "{} selected{}",
                self.selected.len(),
                if self.input.multiple {
                    " · up to 16"
                } else {
                    ""
                }
            )
        };
        if self.input.types != 0 {
            match self.devices() {
                1 => status.push_str(" · Keyboard control allowed"),
                2 => status.push_str(" · Pointer control allowed"),
                3 => status.push_str(" · Keyboard and pointer control allowed"),
                _ => {}
            }
        }
        if self.clipboard
            && self.requested_clipboard
            && !(self.input.types == 0 && self.requested_devices == 0)
        {
            status.push_str(" · Clipboard sharing allowed");
        }
        if self.valid() {
            status.push_str(&format!(
                " · Ctrl+Enter to {}",
                self.action().to_lowercase()
            ));
        }
        body = body
            .push(text(status).size(13).style(canopa_ui::muted))
            .push(
                row![
                    canopa_ui::action(
                        text("Cancel"),
                        Some(Message::Cancel),
                        canopa_ui::Action::Normal,
                        false
                    )
                    .width(Length::Fill),
                    canopa_ui::action(
                        text(self.action()),
                        self.valid().then_some(Message::Share),
                        canopa_ui::Action::Normal,
                        false
                    )
                    .width(Length::Fill),
                ]
                .spacing(12),
            );
        container(body)
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(canopa_ui::surface)
            .into()
    }
}

fn updates(input: DialogInput) -> impl iced::futures::Stream<Item = Result<Vec<Item>, String>> {
    let (mut output, receiver) = futures_channel::mpsc::channel(1);
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        loop {
            let result = match &runtime {
                Ok(runtime) => runtime
                    .block_on(crate::screencast::refresh(&input))
                    .map_err(|_| "Sources unavailable".to_owned()),
                Err(_) => Err("Sources unavailable".to_owned()),
            };
            if let Err(error) = output.try_send(result)
                && error.is_disconnected()
            {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dialog(multiple: bool) -> Dialog {
        let items = vec![
            Item {
                source: Source::Monitor("DP-1".into()),
                title: "Display".into(),
                subtitle: "Desktop".into(),
            },
            Item {
                source: Source::Window(1),
                title: "Editor".into(),
                subtitle: "Text editor".into(),
            },
            Item {
                source: Source::Window(2),
                title: "Browser".into(),
                subtitle: "Web browser".into(),
            },
        ];
        Dialog {
            screenshot: false,
            input: DialogInput {
                app_id: "app".into(),
                parent_window: "".into(),
                modal: true,
                multiple,
                items: items.clone(),
                socket: "/unused".into(),
                compositor_pid: 1,
                compositor_owner: None,
                types: 3,
                activation_token: None,
            },
            items,
            theme: Theme::Dark,
            query: String::new(),
            selected: HashSet::new(),
            requested_devices: 0,
            requested_clipboard: false,
            clipboard: false,
            keyboard: false,
            pointer: false,
            touch: false,
            highlight: None,
            available: true,
            completed: false,
            output: Arc::new(Mutex::new(None)),
        }
    }
    #[test]
    fn screenshot_picker_describes_a_single_capture_without_control_grants() {
        let mut app = dialog(false);
        app.screenshot = true;
        app.input.types = 2;
        app.items
            .retain(|item| matches!(item.source, Source::Window(_)));
        assert_eq!(app.title(), "Choose a screenshot window");
        assert_eq!(app.action(), "Capture window");
        assert!(!app.valid());
        let _ = app.update(Message::Toggle(0));
        assert!(app.valid());
        assert_eq!(app.devices(), 0);
        assert!(!app.clipboard);
    }
    #[test]
    fn replacement_monitor_clears_consent_even_when_its_connector_and_title_are_unchanged() {
        use crate::screencast_capture::Monitor;
        let mut app = dialog(false);
        app.items[0].source = Source::Monitor(Monitor::pinned("DP-1", 77).unwrap());
        let _ = app.update(Message::Toggle(0));
        assert!(app.valid());
        let mut replacement = app.items.clone();
        replacement[0].source = Source::Monitor(Monitor::pinned("DP-1", 78).unwrap());
        let _ = app.update(Message::Loaded(Ok(replacement)));
        assert!(app.selected.is_empty());
        assert!(!app.valid());
        let _ = app.update(Message::Share);
        assert!(app.output.lock().unwrap().is_none());
        let _ = app.update(Message::Toggle(0));
        let _ = app.update(Message::Share);
        assert_eq!(
            app.output.lock().unwrap().as_ref().unwrap().sources,
            [Source::Monitor(Monitor::pinned("DP-1", 78).unwrap())]
        );
    }
    fn key(name: keyboard::key::Named) -> Message {
        modified_key(name, keyboard::Modifiers::empty())
    }
    fn modified_key(name: keyboard::key::Named, modifiers: keyboard::Modifiers) -> Message {
        Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(name),
            modified_key: keyboard::Key::Named(name),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Enter),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        }))
    }
    #[test]
    fn input_only_requires_explicit_current_keyboard_approval() {
        let confirm = || modified_key(keyboard::key::Named::Enter, keyboard::Modifiers::CTRL);
        let mut app = dialog(false);
        app.input.types = 0;
        app.items.clear();
        app.requested_devices = 1;
        let _ = app.update(confirm());
        assert!(app.output.lock().unwrap().is_none());
        let _ = app.update(Message::Keyboard(true));
        let _ = app.update(Message::Loaded(Err("Desktop unavailable".into())));
        let _ = app.update(confirm());
        assert!(app.output.lock().unwrap().is_none());
        let _ = app.update(Message::Loaded(Ok(vec![])));
        let _ = app.update(confirm());
        assert_eq!(
            *app.output.lock().unwrap(),
            Some(Selection {
                sources: vec![],
                devices: 1,
                clipboard: false,
            })
        );
        let _ = app.update(Message::Keyboard(false));
        assert_eq!(app.output.lock().unwrap().as_ref().unwrap().devices, 1);
    }
    #[test]
    fn source_and_device_approval_are_independent_and_cancel_is_final() {
        let mut combined = dialog(false);
        combined.requested_devices = 1;
        let _ = combined.update(Message::Keyboard(true));
        let _ = combined.update(Message::Share);
        assert!(combined.output.lock().unwrap().is_none());
        let _ = combined.update(Message::Toggle(1));
        let _ = combined.update(Message::Share);
        assert_eq!(
            *combined.output.lock().unwrap(),
            Some(Selection {
                sources: vec![Source::Window(1)],
                devices: 1,
                clipboard: false,
            })
        );

        for requested in [0, 1, 2, 3] {
            let mut view_only = dialog(false);
            view_only.requested_devices = requested;
            if requested == 0 {
                let _ = view_only.update(Message::Keyboard(true));
            }
            let _ = view_only.update(Message::Toggle(1));
            let _ = view_only.update(Message::Share);
            assert_eq!(
                *view_only.output.lock().unwrap(),
                Some(Selection {
                    sources: vec![Source::Window(1)],
                    devices: 0,
                    clipboard: false,
                })
            );
        }
        let mut cancelled = dialog(false);
        cancelled.requested_devices = 1;
        let _ = cancelled.update(Message::Keyboard(true));
        let _ = cancelled.update(Message::Toggle(1));
        let _ = cancelled.update(Message::Cancel);
        let _ = cancelled.update(Message::Share);
        assert!(cancelled.output.lock().unwrap().is_none());
    }
    #[test]
    fn pointer_and_keyboard_require_separate_fresh_explicit_approval() {
        for approved in [1, 2, 3] {
            let mut app = dialog(false);
            app.input.types = 0;
            app.items.clear();
            app.requested_devices = 3;
            let _ = app.update(Message::Share);
            assert!(app.output.lock().unwrap().is_none());
            assert!(!app.keyboard && !app.pointer);
            if approved & 1 != 0 {
                let _ = app.update(Message::Keyboard(true));
            }
            if approved & 2 != 0 {
                let _ = app.update(Message::Pointer(true));
            }
            let _ = app.update(Message::Share);
            assert_eq!(
                app.output.lock().unwrap().as_ref().unwrap().devices,
                approved
            );
        }
        let mut pointer = dialog(false);
        pointer.requested_devices = 2;
        let _ = pointer.update(Message::Keyboard(true));
        assert_eq!(pointer.devices(), 0);
        let _ = pointer.update(Message::Pointer(true));
        let _ = pointer.update(Message::Share);
        assert!(pointer.output.lock().unwrap().is_none());
        let _ = pointer.update(Message::Toggle(1));
        let _ = pointer.update(Message::Share);
        assert_eq!(pointer.output.lock().unwrap().as_ref().unwrap().devices, 2);
    }
    #[test]
    fn keyboard_share_requires_an_explicit_available_selection() {
        let confirm = || modified_key(keyboard::key::Named::Enter, keyboard::Modifiers::CTRL);
        let mut app = dialog(false);
        let _ = app.update(confirm());
        assert!(app.output.lock().unwrap().is_none());
        let _ = app.update(Message::Toggle(1));
        let _ = app.update(Message::Loaded(Err("Unavailable".into())));
        let _ = app.update(confirm());
        assert!(app.output.lock().unwrap().is_none());
        let _ = app.update(Message::Loaded(Ok(app.items.clone())));
        let _ = app.update(confirm());
        assert_eq!(
            *app.output.lock().unwrap(),
            Some(Selection {
                sources: vec![Source::Window(1)],
                devices: 0,
                clipboard: false,
            })
        );
        let _ = app.update(Message::Cancel);
        assert_eq!(
            *app.output.lock().unwrap(),
            Some(Selection {
                sources: vec![Source::Window(1)],
                devices: 0,
                clipboard: false,
            })
        );
    }
    #[test]
    fn explicit_share_is_required_and_a_completed_choice_cannot_change() {
        let mut app = dialog(false);
        let _ = app.update(Message::Share);
        assert!(app.output.lock().unwrap().is_none());
        let _ = app.update(Message::Toggle(1));
        let _ = app.update(key(keyboard::key::Named::Enter));
        assert!(app.output.lock().unwrap().is_none());
        let _ = app.update(Message::Share);
        assert_eq!(
            *app.output.lock().unwrap(),
            Some(Selection {
                sources: vec![Source::Window(1)],
                devices: 0,
                clipboard: false,
            })
        );
        let _ = app.update(Message::Toggle(2));
        let _ = app.update(Message::Share);
        assert_eq!(
            *app.output.lock().unwrap(),
            Some(Selection {
                sources: vec![Source::Window(1)],
                devices: 0,
                clipboard: false,
            })
        );
        let mut app = dialog(false);
        let _ = app.update(Message::Toggle(1));
        let _ = app.update(Message::Cancel);
        let _ = app.update(Message::Share);
        assert!(app.output.lock().unwrap().is_none());
    }
    #[test]
    fn refresh_preserves_identity_and_keyboard_highlight_but_cannot_keep_hidden_or_closed_grants() {
        let mut app = dialog(true);
        let _ = app.update(key(keyboard::key::Named::ArrowDown));
        let _ = app.update(key(keyboard::key::Named::Space));
        let mut changed = app.items.clone();
        changed.reverse();
        let _ = app.update(Message::Loaded(Ok(changed)));
        assert_eq!(
            app.items[app.highlight.unwrap()].source,
            Source::Monitor("DP-1".into())
        );
        assert!(app.selected.contains(&Source::Monitor("DP-1".into())));
        let _ = app.update(Message::Query("editor".into()));
        assert!(!app.valid());
        let _ = app.update(Message::Toggle(1));
        assert!(app.valid());
        let _ = app.update(Message::Loaded(Ok(vec![])));
        assert!(!app.valid());
        let _ = app.update(Message::Share);
        assert!(app.output.lock().unwrap().is_none());
    }
    #[test]
    fn multiple_selection_and_unavailable_catalogs_keep_acceptance_bounded() {
        let mut app = dialog(true);
        app.items = (1..=20)
            .map(|id| Item {
                source: Source::Window(id),
                title: format!("Window {id}"),
                subtitle: "app".into(),
            })
            .collect();
        for id in 0..20 {
            let _ = app.update(Message::Toggle(id));
        }
        assert_eq!(app.selected.len(), 16);
        assert!(app.valid());
        let _ = app.update(Message::Loaded(Err("Unavailable".into())));
        assert!(!app.valid());
        let _ = app.update(Message::Share);
        assert!(app.output.lock().unwrap().is_none());
    }
    #[test]
    fn clipboard_approval_is_independent_unchecked_and_allows_clipboard_only() {
        let mut app = dialog(false);
        app.input.types = 0;
        app.items.clear();
        app.requested_clipboard = true;
        assert!(!app.clipboard && !app.valid());
        let _ = app.update(Message::Share);
        assert!(app.output.lock().unwrap().is_none());
        let shortcut = |modifiers| {
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Character("c".into()),
                modified_key: keyboard::Key::Character("c".into()),
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyC),
                location: keyboard::Location::Standard,
                modifiers,
                text: None,
                repeat: false,
            }))
        };
        let _ = app.update(shortcut(keyboard::Modifiers::empty()));
        assert!(!app.clipboard);
        let _ = app.update(shortcut(keyboard::Modifiers::ALT));
        assert!(app.valid());
        assert_eq!(app.devices(), 0);
        assert_eq!(app.action(), "Share clipboard");
        let _ = app.update(Message::Share);
        let result = app.output.lock().unwrap().clone().unwrap();
        assert!(result.clipboard && result.sources.is_empty());
        assert_eq!(result.devices, 0);
        let mut unrequested = dialog(false);
        let _ = unrequested.update(Message::Clipboard(true));
        assert!(!unrequested.clipboard);
        let mut with_capture = dialog(false);
        with_capture.requested_clipboard = true;
        let _ = with_capture.update(Message::Clipboard(true));
        assert!(
            !with_capture.valid(),
            "a requested capture still needs source approval"
        );
    }
    #[test]
    fn touch_requires_its_own_fresh_choice_and_cannot_enable_other_devices() {
        let mut app = dialog(false);
        app.requested_devices = 7;
        assert_eq!(app.devices(), 0);
        let _ = app.update(Message::Touch(true));
        assert_eq!(app.devices(), 4);
        assert!(!app.valid());
        let _ = app.update(Message::Toggle(0));
        assert!(app.valid());
        let _ = app.update(Message::Keyboard(true));
        assert_eq!(app.devices(), 5);
        let _ = app.update(Message::Pointer(true));
        assert_eq!(app.devices(), 7);
        let _ = app.update(Message::Touch(false));
        let _ = app.update(Message::Share);
        assert_eq!(app.output.lock().unwrap().as_ref().unwrap().devices, 3);
        let mut next = dialog(false);
        next.requested_devices = 3;
        let _ = next.update(Message::Touch(true));
        assert_eq!(next.devices(), 0);
        assert!(!next.touch);
    }
}
