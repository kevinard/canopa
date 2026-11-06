//! Request-owned launcher review. Only an explicit Install accepts the draft.
use crate::{
    dynamic_launcher::{LauncherDraft, LauncherType, NAME_BYTES, valid_name},
    dynamic_launcher_icon::DialogInput,
};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{column, container, image, row, scrollable, text, text_input},
    window,
};
use std::sync::{Arc, Mutex};

pub fn run(
    input: DialogInput,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<Option<String>> {
    input.draft.validate()?;
    anyhow::ensure!(input.preview.valid(), "Invalid launcher preview");
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    let output = Arc::new(Mutex::new(None));
    let result = output.clone();
    let preview = image::Handle::from_rgba(
        input.preview.width,
        input.preview.height,
        input.preview.pixels,
    );
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(680.0, 520.0),
                min_size: Some(Size::new(440.0, 340.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let parent = input
                .draft
                .parent_window
                .strip_prefix("wayland:")
                .unwrap_or_default();
            let (_, open) = window::open_dialog(parent.to_owned(), input.draft.modal, settings);
            (
                Dialog {
                    draft: input.draft.clone(),
                    name: input.draft.name.clone(),
                    preview: preview.clone(),
                    theme: theme.clone(),
                    output: output.clone(),
                    invalid: false,
                    completed: false,
                },
                open.map(|_| Message::Opened),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|_: &Dialog, _| "Install launcher — Canopa".to_owned())
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(Dialog::subscription)
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("launcher result").take())
}

struct Dialog {
    draft: LauncherDraft,
    name: String,
    preview: image::Handle,
    theme: Theme,
    output: Arc<Mutex<Option<String>>>,
    invalid: bool,
    completed: bool,
}

#[derive(Clone, Debug)]
enum Message {
    Opened,
    Name(String),
    Install,
    Cancel,
    Event(Event),
}

impl Dialog {
    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen_with(|event, status, _| {
            let escape = matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            );
            (escape || status == iced::event::Status::Ignored).then_some(Message::Event(event))
        })
    }

    fn can_install(&self) -> bool {
        !self.completed && !self.invalid && valid_name(&self.name)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Name(value) if self.draft.editable_name => {
                self.invalid = value.len() > NAME_BYTES || value.chars().any(char::is_control);
                if !self.invalid {
                    self.name = value;
                }
            }
            Message::Install if self.can_install() => {
                *self.output.lock().expect("launcher result") = Some(self.name.clone());
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
                if key == keyboard::Key::Named(keyboard::key::Named::Tab) {
                    return if modifiers.shift() {
                        iced::widget::operation::focus_previous()
                    } else {
                        iced::widget::operation::focus_next()
                    };
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let heading = match self.draft.launcher_type {
            LauncherType::Application => "Install application launcher?",
            LauncherType::Webapp => "Install web app?",
        };
        let requester = if self.draft.app_id.is_empty() {
            "The requesting app"
        } else {
            &self.draft.app_id
        };
        let icon = container(image(self.preview.clone()).width(96).height(96))
            .padding(8)
            .style(|_| container::Style::default().background(iced::Color::WHITE));
        let mut name = column![text("Launcher name")]
            .spacing(8)
            .width(Length::Fill);
        if self.draft.editable_name {
            name = name.push(
                text_input("Enter a name", &self.name)
                    .on_input(Message::Name)
                    .id("launcher-name")
                    .padding(10),
            );
        } else {
            name = name.push(text(&self.name).size(18));
        }
        if self.invalid || !valid_name(&self.name) {
            name = name.push(text(if self.invalid {
                "Use a shorter name on one line."
            } else {
                "Enter a launcher name."
            }));
        }
        let mut content = column![
            text(heading).size(22),
            text(format!("Requested by {requester}")).style(canopa_ui::muted),
            row![icon, name].spacing(18).align_y(iced::Top),
        ]
        .spacing(14)
        .width(Length::Fill);
        if self.draft.launcher_type == LauncherType::Webapp {
            content = content
                .push(text("Web address"))
                .push(text(&self.draft.target));
        }
        content = content
            .push(text("The launcher will appear in your applications.").style(canopa_ui::muted));
        let actions = row![
            canopa_ui::action(
                text("Cancel"),
                Some(Message::Cancel),
                canopa_ui::Action::Normal,
                false
            )
            .width(Length::Fill),
            canopa_ui::action(
                text("Install"),
                self.can_install().then_some(Message::Install),
                canopa_ui::Action::Normal,
                false
            )
            .width(Length::Fill),
        ]
        .spacing(12);
        container(column![scrollable(content).height(Length::Fill), actions].spacing(18))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(canopa_ui::surface)
            .into()
    }
}

#[cfg(test)]
mod tests;
