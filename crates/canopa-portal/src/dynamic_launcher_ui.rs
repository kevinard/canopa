//! Request-owned launcher review. Only an explicit Install accepts the draft.
use crate::accessibility_ui::{self as a11y, Node, semantic};
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
            let (window, open) =
                window::open_dialog(parent.to_owned(), input.draft.modal, settings);
            (
                Dialog {
                    draft: input.draft.clone(),
                    name: input.draft.name.clone(),
                    preview: preview.clone(),
                    theme: theme.clone(),
                    output: output.clone(),
                    invalid: false,
                    completed: false,
                    window,
                    accessibility: Some(a11y::Accessibility::new(
                        "canopa-dynamic-launcher",
                        WINDOW_KEY,
                    )),
                },
                open.map(|_| Message::Opened),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|_: &Dialog, _| WINDOW_TITLE.to_owned())
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(Dialog::subscription)
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("launcher result").take())
}

/// The AT-SPI window of the launcher review and its title.
const WINDOW_KEY: &str = "dynamic-launcher";
const WINDOW_TITLE: &str = "Install launcher — Canopa";

struct Dialog {
    draft: LauncherDraft,
    name: String,
    preview: image::Handle,
    theme: Theme,
    output: Arc<Mutex<Option<String>>>,
    invalid: bool,
    completed: bool,
    window: window::Id,
    accessibility: Option<a11y::Accessibility<Message>>,
}

#[derive(Clone, Debug)]
enum Message {
    Opened,
    Name(String),
    Install,
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
    fn subscription(&self) -> Subscription<Message> {
        let input = iced::event::listen_with(|event, status, _| {
            let escape = matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            );
            (escape || status == iced::event::Status::Ignored).then_some(Message::Event(event))
        });
        match &self.accessibility {
            Some(state) => Subscription::batch([input, state.bridge.subscription()]),
            None => input,
        }
    }

    fn can_install(&self) -> bool {
        !self.completed && !self.invalid && valid_name(&self.name)
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
                    state.opened(WINDOW_TITLE, window);
                }
            }
            Message::Name(value) if self.draft.editable_name => {
                self.invalid = value.len() > NAME_BYTES || value.chars().any(char::is_control);
                if !self.invalid {
                    self.name = value;
                }
            }
            Message::Install if self.can_install() => {
                *self.output.lock().expect("launcher result") = Some(self.name.clone());
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
        let problem = (self.invalid || !valid_name(&self.name)).then_some(if self.invalid {
            "Use a shorter name on one line."
        } else {
            "Enter a launcher name."
        });
        if self.draft.editable_name {
            name = name.push(semantic(
                Node::text_input("Launcher name", &self.name)
                    .key("dynamic-launcher-name")
                    .placeholder("Enter a name")
                    .required(true)
                    .invalid(problem.is_some())
                    .on_text(Message::Name),
                text_input("Enter a name", &self.name)
                    .on_input(Message::Name)
                    .id("launcher-name")
                    .padding(10),
            ));
        } else {
            name = name.push(text(&self.name).size(18));
        }
        if let Some(problem) = problem {
            name = name.push(semantic(
                Node::alert(problem).key("dynamic-launcher-problem"),
                text(problem),
            ));
        }
        let mut content = column![
            semantic(Node::heading(heading, 1), text(heading).size(22)),
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
        let install = self.can_install().then_some(Message::Install);
        let actions = row![
            a11y::button(
                "dynamic-launcher-cancel",
                "Cancel",
                Some(Message::Cancel),
                canopa_ui::action(
                    text("Cancel"),
                    Some(Message::Cancel),
                    canopa_ui::Action::Normal,
                    false
                )
                .width(Length::Fill)
            ),
            a11y::button(
                "dynamic-launcher-install",
                "Install",
                install.clone(),
                canopa_ui::action(text("Install"), install, canopa_ui::Action::Normal, false)
                    .width(Length::Fill)
            ),
        ]
        .spacing(12);
        let mut description = format!("Requested by {requester}.");
        if self.draft.launcher_type == LauncherType::Webapp {
            description.push_str(&format!(" Web address {}.", self.draft.target));
        }
        a11y::dialog_window(
            WINDOW_KEY,
            WINDOW_TITLE,
            Node::dialog(heading)
                .key("dynamic-launcher-dialog")
                .description(description),
            container(column![scrollable(content).height(Length::Fill), actions].spacing(18))
                .padding(20)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(canopa_ui::surface),
        )
    }
}

#[cfg(test)]
mod tests;
