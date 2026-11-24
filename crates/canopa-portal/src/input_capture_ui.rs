//! Fresh unchecked input-capture consent in the shared Iced appearance.
use crate::accessibility_ui::{self as a11y, Node, semantic};
use crate::dialog_ui;
use crate::input_capture::DialogInput;
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{checkbox, column, container, text},
    window,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub capabilities: u32,
    pub clipboard: bool,
}

pub fn run(
    input: DialogInput,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<Option<Selection>> {
    anyhow::ensure!(
        input.capabilities != 0
            && input.capabilities & !3 == 0
            && input.app_id.len() <= 4096
            && input.parent_window.len() <= 4096,
        "Invalid capture consent"
    );
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    let theme = canopa_ui::theme(appearance.clone());
    let font = canopa_ui::font(&appearance);
    iced::daemon(
        move || {
            let (window, opening) = window::open_dialog_with_activation(
                input
                    .parent_window
                    .strip_prefix("wayland:")
                    .unwrap_or_default()
                    .to_owned(),
                input.modal,
                input.activation_token.clone(),
                window::Settings {
                    size: Size::new(
                        560.,
                        360. + if input.clipboard { 65. } else { 0. }
                            + if input.capabilities == 3 { 42. } else { 0. },
                    ),
                    min_size: Some(Size::new(440., 340.)),
                    exit_on_close_request: false,
                    platform_specific: window::settings::PlatformSpecific {
                        application_id: "org.canopa.Portal".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
            let mut dialog = Dialog::new(input.clone(), theme.clone(), output.clone());
            dialog.window = window;
            dialog.accessibility =
                Some(a11y::Accessibility::new("canopa-input-capture", WINDOW_KEY));
            (dialog, opening.map(|_| Message::Opened))
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|_: &Dialog, _| window_title())
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(Dialog::subscription)
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("capture consent").take())
}
/// The AT-SPI window of the capture question and its title.
const WINDOW_KEY: &str = "input-capture";
fn window_title() -> String {
    fl!(
        "portal-window-title",
        title = fl!("portal-input-capture-window")
    )
}

struct Dialog {
    input: DialogInput,
    requester: String,
    theme: Theme,
    pointer: bool,
    keyboard: bool,
    clipboard: bool,
    output: Arc<Mutex<Option<Selection>>>,
    completed: bool,
    window: window::Id,
    accessibility: Option<a11y::Accessibility<Message>>,
}
#[derive(Clone, Debug)]
enum Message {
    Opened,
    Pointer(bool),
    Keyboard(bool),
    Clipboard(bool),
    Allow,
    Cancel,
    Event(Event),
    Accessibility(canopa_ui::accessibility::Event<Message>),
}

impl canopa_ui::accessibility::Message for Message {
    fn accessibility(event: canopa_ui::accessibility::Event<Self>) -> Self {
        Self::Accessibility(event)
    }
}
impl Dialog {
    fn new(input: DialogInput, theme: Theme, output: Arc<Mutex<Option<Selection>>>) -> Self {
        Self {
            requester: dialog_ui::requester(&input.app_id),
            input,
            theme,
            pointer: false,
            keyboard: false,
            clipboard: false,
            output,
            completed: false,
            window: window::Id::unique(),
            accessibility: None,
        }
    }
    fn selected(&self) -> u32 {
        (u32::from(self.keyboard) | u32::from(self.pointer) << 1) & self.input.capabilities
    }
    fn subscription(&self) -> Subscription<Message> {
        let input = iced::event::listen().map(Message::Event);
        match &self.accessibility {
            Some(state) => Subscription::batch([input, state.bridge.subscription()]),
            None => input,
        }
    }
    /// Handle a message, then refresh the accessibility tree if one is read.
    fn update(&mut self, message: Message) -> Task<Message> {
        if let Message::Accessibility(event) = message {
            return a11y::update(&mut self.accessibility, event);
        }
        let task = self.apply(message);
        a11y::refresh(&mut self.accessibility, task)
    }
    fn exit(&mut self) -> Task<Message> {
        self.completed = true;
        if let Some(state) = &mut self.accessibility {
            state.close();
        }
        iced::exit()
    }
    fn apply(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Opened => {
                let window = self.window;
                if let Some(state) = &mut self.accessibility {
                    state.opened(&window_title(), window);
                }
            }
            Message::Pointer(value) if self.input.capabilities & 2 != 0 => self.pointer = value,
            Message::Keyboard(value) if self.input.capabilities & 1 != 0 => self.keyboard = value,
            Message::Clipboard(value) if self.input.clipboard => self.clipboard = value,
            Message::Allow if self.selected() != 0 => {
                *self.output.lock().expect("capture consent") = Some(Selection {
                    capabilities: self.selected(),
                    clipboard: self.clipboard,
                });
                return self.exit();
            }
            Message::Cancel | Message::Event(Event::Window(window::Event::CloseRequested)) => {
                return self.exit();
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => {
                if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    return self.apply(Message::Cancel);
                }
                if modifiers.alt()
                    && let keyboard::Key::Character(value) = key
                {
                    match value.as_str() {
                        "p" | "P" => return self.apply(Message::Pointer(!self.pointer)),
                        "k" | "K" => return self.apply(Message::Keyboard(!self.keyboard)),
                        "c" | "C" => return self.apply(Message::Clipboard(!self.clipboard)),
                        "a" | "A" => return self.apply(Message::Allow),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let requester = &self.requester;
        let question = fl!("portal-input-capture-question");
        let mut content = column![
            semantic(
                Node::heading(question.clone(), 1),
                dialog_ui::heading(question.clone())
            ),
            dialog_ui::detail(fl!(
                "portal-input-capture-request",
                requester = requester.as_str()
            )),
            text(fl!("portal-input-capture-explanation")).size(14),
        ]
        .spacing(12);
        let choice = |key: &'static str,
                      label: String,
                      shortcut: &'static str,
                      checked: bool,
                      toggle: fn(bool) -> Message| {
            semantic(
                Node::check_box(label.clone(), checked)
                    .key(key)
                    .shortcut(shortcut)
                    .on_click(Some(toggle(!checked))),
                checkbox(checked)
                    .label(fl!(
                        "portal-with-shortcut",
                        label = label,
                        shortcut = shortcut
                    ))
                    .style(canopa_ui::checkbox_style)
                    .on_toggle(toggle),
            )
        };
        if self.input.capabilities & 2 != 0 {
            content = content.push(choice(
                "input-capture-pointer",
                fl!("portal-input-capture-pointer"),
                "Alt+P",
                self.pointer,
                Message::Pointer,
            ));
        }
        if self.input.capabilities & 1 != 0 {
            content = content.push(choice(
                "input-capture-keyboard",
                fl!("portal-input-capture-keyboard"),
                "Alt+K",
                self.keyboard,
                Message::Keyboard,
            ));
        }
        if self.input.clipboard {
            content = content.push(
                container(
                    column![
                        choice(
                            "input-capture-clipboard",
                            fl!("portal-input-capture-clipboard"),
                            "Alt+C",
                            self.clipboard,
                            Message::Clipboard
                        ),
                        dialog_ui::detail(fl!("portal-input-capture-clipboard-scope")).size(13),
                    ]
                    .spacing(6),
                )
                .padding([10, 12])
                .width(Length::Fill)
                .style(dialog_ui::card),
            );
        }
        let allow = (self.selected() != 0).then_some(Message::Allow);
        let mut content = content.push(
            container(
                column![
                    dialog_ui::section(fl!("portal-input-capture-pause")),
                    dialog_ui::detail(fl!("portal-input-capture-end")),
                ]
                .spacing(5),
            )
            .width(Length::Fill)
            .padding([10, 12])
            .style(dialog_ui::card),
        );
        if allow.is_none() {
            content =
                content.push(dialog_ui::detail(fl!("portal-input-capture-select-input")).size(13));
        }
        let actions = dialog_ui::footer([
            a11y::button(
                "input-capture-cancel",
                fl!("portal-cancel"),
                Some(Message::Cancel),
                dialog_ui::action(
                    text(fl!("portal-cancel")),
                    Some(Message::Cancel),
                    canopa_ui::Action::Quiet,
                    false,
                ),
            ),
            semantic(
                Node::button(fl!("portal-input-capture-allow"))
                    .key("input-capture-allow")
                    .shortcut("Alt+A")
                    .disabled(allow.is_none())
                    .on_click(allow.clone()),
                dialog_ui::action(
                    text(fl!(
                        "portal-with-shortcut",
                        label = fl!("portal-input-capture-allow"),
                        shortcut = "Alt+A"
                    )),
                    allow,
                    canopa_ui::Action::Primary,
                    false,
                ),
            ),
        ]);
        a11y::dialog_window(
            WINDOW_KEY,
            window_title(),
            Node::alert_dialog(question)
                .key("input-capture-dialog")
                .description(fl!(
                    "portal-input-capture-description",
                    requester = requester.as_str()
                )),
            dialog_ui::layout(content, actions),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dialog(capabilities: u32) -> Dialog {
        Dialog::new(
            DialogInput {
                app_id: "org.canopa.Test".into(),
                parent_window: String::new(),
                modal: true,
                activation_token: None,
                capabilities,
                clipboard: false,
            },
            Theme::Dark,
            Arc::new(Mutex::new(None)),
        )
    }
    #[test]
    fn choices_start_unchecked_and_allow_never_creates_a_grant_without_input_selection() {
        let mut dialog = dialog(3);
        assert_eq!(dialog.selected(), 0);
        drop(dialog.update(Message::Allow));
        assert!(!dialog.completed && dialog.output.lock().unwrap().is_none());
        drop(dialog.update(Message::Keyboard(true)));
        assert_eq!(dialog.selected(), 1);
        drop(dialog.update(Message::Allow));
        assert_eq!(
            dialog.output.lock().unwrap().as_ref().unwrap().capabilities,
            1
        );
    }
    #[test]
    fn unavailable_classes_and_unrequested_clipboard_cannot_be_selected() {
        let mut keyboard = dialog(1);
        drop(keyboard.update(Message::Pointer(true)));
        drop(keyboard.update(Message::Clipboard(true)));
        assert_eq!(keyboard.selected(), 0);
        drop(keyboard.update(Message::Keyboard(true)));
        drop(keyboard.update(Message::Allow));
        let result = keyboard.output.lock().unwrap();
        assert_eq!(result.as_ref().unwrap().capabilities, 1);
        assert!(!result.as_ref().unwrap().clipboard);
        let mut pointer = dialog(2);
        drop(pointer.update(Message::Keyboard(true)));
        assert_eq!(pointer.selected(), 0);
        drop(pointer.update(Message::Pointer(true)));
        assert_eq!(pointer.selected(), 2);
    }
}
