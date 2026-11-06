//! Fresh unchecked input-capture consent in the shared Iced appearance.
use crate::input_capture::DialogInput;
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{checkbox, column, container, row, text},
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
            let (_, opening) = window::open_dialog_with_activation(
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
                        380. + if input.clipboard { 80. } else { 0. }
                            + if input.capabilities == 3 { 60. } else { 0. },
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
            (
                Dialog::new(input.clone(), theme.clone(), output.clone()),
                opening.map(|_| Message::Opened),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|_: &Dialog, _| "Input capture — Canopa".to_owned())
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(Dialog::subscription)
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("capture consent").take())
}
struct Dialog {
    input: DialogInput,
    theme: Theme,
    pointer: bool,
    keyboard: bool,
    clipboard: bool,
    output: Arc<Mutex<Option<Selection>>>,
    completed: bool,
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
}
impl Dialog {
    fn new(input: DialogInput, theme: Theme, output: Arc<Mutex<Option<Selection>>>) -> Self {
        Self {
            input,
            theme,
            pointer: false,
            keyboard: false,
            clipboard: false,
            output,
            completed: false,
        }
    }
    fn selected(&self) -> u32 {
        (u32::from(self.keyboard) | u32::from(self.pointer) << 1) & self.input.capabilities
    }
    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen().map(Message::Event)
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Pointer(value) if self.input.capabilities & 2 != 0 => self.pointer = value,
            Message::Keyboard(value) if self.input.capabilities & 1 != 0 => self.keyboard = value,
            Message::Clipboard(value) if self.input.clipboard => self.clipboard = value,
            Message::Allow if self.selected() != 0 => {
                *self.output.lock().expect("capture consent") = Some(Selection {
                    capabilities: self.selected(),
                    clipboard: self.clipboard,
                });
                self.completed = true;
                return iced::exit();
            }
            Message::Cancel | Message::Event(Event::Window(window::Event::CloseRequested)) => {
                self.completed = true;
                return iced::exit();
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => {
                if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    return self.update(Message::Cancel);
                }
                if modifiers.alt()
                    && let keyboard::Key::Character(value) = key
                {
                    match value.as_str() {
                        "p" | "P" => return self.update(Message::Pointer(!self.pointer)),
                        "k" | "K" => return self.update(Message::Keyboard(!self.keyboard)),
                        "c" | "C" => return self.update(Message::Clipboard(!self.clipboard)),
                        "a" | "A" => return self.update(Message::Allow),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let requester = if self.input.app_id.is_empty() {
            "An application"
        } else {
            &self.input.app_id
        };
        let mut content = column![
            text("Allow input capture?").size(24),
            text(format!("{requester} wants to receive input from this desktop.")).size(16),
            text("Moving the pointer across a screen edge chosen by the application activates capture of the input you approve.").size(14),
        ].spacing(18);
        if self.input.capabilities & 2 != 0 {
            content = content.push(
                checkbox(self.pointer)
                    .label("Allow pointer input (Alt+P)")
                    .style(canopa_ui::checkbox_style)
                    .on_toggle(Message::Pointer),
            );
        }
        if self.input.capabilities & 1 != 0 {
            content = content.push(
                checkbox(self.keyboard)
                    .label("Allow keyboard input (Alt+K)")
                    .style(canopa_ui::checkbox_style)
                    .on_toggle(Message::Keyboard),
            );
        }
        if self.input.clipboard {
            content = content.push(
                checkbox(self.clipboard)
                    .label("Share clipboard while input is captured (Alt+C)")
                    .style(canopa_ui::checkbox_style)
                    .on_toggle(Message::Clipboard),
            );
        }
        container(content.extend([
            text("Ctrl+Alt+Esc pauses capture. You can end this session from its control window.").size(12).style(canopa_ui::muted).into(),
            row![
                canopa_ui::action(text("Cancel"),Some(Message::Cancel),canopa_ui::Action::Quiet,false),
                canopa_ui::action(text("Allow input capture (Alt+A)"),(self.selected() != 0).then_some(Message::Allow),canopa_ui::Action::Normal,false),
            ].spacing(12).into()
        ])).padding(24).width(Length::Fill).height(Length::Fill).style(canopa_ui::surface).into()
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
