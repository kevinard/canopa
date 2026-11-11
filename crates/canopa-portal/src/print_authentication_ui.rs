//! Small Iced authentication surface shared by metadata and document delivery.
//! Only the private descriptor carries its answer; ordinary output is a bool.
use crate::print::{
    Context,
    authentication::{Challenge, Credentials, Field, Kind, Secret, Transport},
};
use iced::{
    Element, Event, Length, Size, Task, Theme, keyboard,
    widget::{column, container, scrollable, text, text_input},
    window,
};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub(crate) challenge: Challenge,
    pub(crate) context: Context,
    pub(crate) channel: i32,
}
impl Request {
    fn validate(&self) -> anyhow::Result<()> {
        self.challenge.validate()?;
        anyhow::ensure!(
            self.challenge.interactive,
            "Cached authentication cannot open a dialog"
        );
        anyhow::ensure!(
            self.context.app_id.len() <= 256
                && [
                    &self.context.app_id,
                    &self.context.title,
                    &self.context.parent_window
                ]
                .iter()
                .all(|v| v.len() <= 4096 && !v.contains('\0')),
            "Invalid printer authentication context"
        );
        Ok(())
    }
}
pub fn run(request: Request, appearance: canopa_theme::Appearance) -> anyhow::Result<bool> {
    request.validate()?;
    let mut channel = crate::print::file_target::inherited_channel(request.channel)?;
    channel.set_write_timeout(Some(Duration::from_secs(2)))?;
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    let output = Arc::new(Mutex::new(None));
    let result = output.clone();
    let challenge = request.challenge.clone();
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(580., 540.),
                min_size: Some(Size::new(420., 340.)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let parent = request
                .context
                .parent_window
                .strip_prefix("wayland:")
                .unwrap_or_default();
            let (_, opening) =
                window::open_dialog(parent.to_owned(), request.context.modal, settings);
            (
                Form::new(request.clone(), theme.clone(), result.clone()),
                opening.map(|_| Message::Opened),
            )
        },
        Form::update,
        Form::view,
    )
    .title(|_: &Form, _| {
        fl!(
            "portal-window-title",
            title = fl!("portal-print-auth-title")
        )
    })
    .theme(|app: &Form, _| app.theme.clone())
    .subscription(|_: &Form| iced::event::listen().map(Message::Event))
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let credentials = output.lock().expect("printer authentication result").take();
    if let Some(credentials) = credentials {
        let frame = Credentials::encode(&challenge, Some(&credentials))?;
        channel.write_all(&(frame.bytes().len() as u32).to_be_bytes())?;
        channel.write_all(frame.bytes())?;
        Ok(true)
    } else {
        Ok(false)
    }
}
#[derive(Clone)]
enum Message {
    Opened,
    Value(usize, Arc<Secret>),
    Authenticate,
    Cancel,
    Event(Event),
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Keyboard events can contain typed text too. Neither edits nor events
        // expose that text through an Iced message's debugging implementation.
        f.write_str(match self {
            Self::Opened => "Opened",
            Self::Value(..) => "Value(<redacted>)",
            Self::Authenticate => "Authenticate",
            Self::Cancel => "Cancel",
            Self::Event(_) => "Event(<redacted>)",
        })
    }
}
struct Form {
    request: Request,
    theme: Theme,
    values: Vec<Secret>,
    output: Arc<Mutex<Option<Credentials>>>,
    completed: bool,
}
impl Form {
    fn new(request: Request, theme: Theme, output: Arc<Mutex<Option<Credentials>>>) -> Self {
        let values = request
            .challenge
            .fields()
            .iter()
            .map(|field| {
                Secret::copy(if *field == Field::Username {
                    request.challenge.suggested_user.as_bytes()
                } else {
                    b""
                })
            })
            .collect();
        Self {
            request,
            theme,
            values,
            output,
            completed: false,
        }
    }
    fn valid(&self) -> bool {
        Credentials::validate_values(&self.values, &self.request.challenge).is_ok()
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Opened => {
                let index = self
                    .request
                    .challenge
                    .fields()
                    .iter()
                    .enumerate()
                    .find(|(i, field)| {
                        **field == Field::Username && self.values[*i].bytes().is_empty()
                    })
                    .map(|(i, _)| i)
                    .or_else(|| {
                        self.request
                            .challenge
                            .fields()
                            .iter()
                            .position(|field| *field == Field::Password)
                    })
                    .unwrap_or(0);
                return iced::widget::operation::focus(format!("print-auth-field-{index}"));
            }
            Message::Value(index, value)
                if index < self.values.len() && value.bytes().len() <= 1023 =>
            {
                self.values[index] =
                    Arc::try_unwrap(value).unwrap_or_else(|value| Secret::copy(value.bytes()));
            }
            Message::Authenticate if self.valid() => {
                *self.output.lock().expect("printer authentication result") = Some(Credentials {
                    values: std::mem::take(&mut self.values),
                });
                self.completed = true;
                return iced::exit();
            }
            Message::Cancel | Message::Event(Event::Window(window::Event::CloseRequested)) => {
                self.values.clear();
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
        if self.completed {
            return container(text(fl!("portal-print-auth-closing")))
                .padding(20)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(canopa_ui::surface)
                .into();
        }
        let challenge = &self.request.challenge;
        let mut content = column![
            text(fl!("portal-print-auth-title")).size(24),
            text(if self.request.context.title.is_empty() {
                fl!("portal-print-document")
            } else {
                self.request.context.title.clone()
            })
            .size(18),
            text(if self.request.context.app_id.is_empty() {
                fl!("portal-requested-by-application")
            } else {
                self.request.context.app_id.clone()
            })
            .style(canopa_ui::muted)
        ]
        .spacing(12)
        .width(Length::Fill);
        content = content.push(text(match &challenge.kind {
            Kind::Server { realm, .. } if !realm.is_empty() => fl!(
                "portal-print-auth-server-realm",
                server = format!("{}:{}", challenge.endpoint.server, challenge.endpoint.port),
                realm = realm.as_str()
            ),
            Kind::Server { .. } => fl!(
                "portal-print-auth-server",
                server = format!("{}:{}", challenge.endpoint.server, challenge.endpoint.port)
            ),
            Kind::Queue { destination, .. } => fl!(
                "portal-print-auth-queue",
                printer = format!(
                    "{}{}",
                    destination.queue,
                    destination
                        .instance
                        .as_ref()
                        .map(|i| format!(" / {i}"))
                        .unwrap_or_default()
                ),
                server = format!("{}:{}", challenge.endpoint.server, challenge.endpoint.port)
            ),
        }));
        content = content.push(
            text(match challenge.endpoint.transport {
                Transport::Local => fl!("portal-print-auth-local"),
                Transport::Loopback => fl!("portal-print-auth-loopback"),
                Transport::Tls => fl!("portal-print-auth-tls"),
            })
            .size(14)
            .style(canopa_ui::muted),
        );
        if challenge.attempt > 1 {
            content = content.push(text(fl!("portal-print-auth-rejected")).size(14));
        }
        for (index, field) in challenge.fields().iter().enumerate() {
            let label = match field {
                Field::Domain => fl!("portal-print-auth-domain"),
                Field::Username => fl!("portal-print-auth-username"),
                Field::Password => fl!("portal-print-auth-password"),
            };
            let value = std::str::from_utf8(self.values[index].bytes()).expect("Iced UTF-8 input");
            content = content.push(
                column![
                    text(label.clone()).size(14),
                    text_input(&label, value)
                        .id(format!("print-auth-field-{index}"))
                        .secure(*field == Field::Password)
                        .on_input(move |value| Message::Value(
                            index,
                            Arc::new(Secret::from_string(value))
                        ))
                ]
                .spacing(4),
            );
        }
        if !self.valid() {
            content = content.push(text(fl!("portal-print-auth-invalid")).size(14));
        }
        content = content.push(
            text(fl!("portal-print-auth-not-saved"))
                .size(13)
                .style(canopa_ui::muted),
        );
        let actions = canopa_ui::direction::row([
            canopa_ui::action(
                text(fl!("portal-cancel")),
                Some(Message::Cancel),
                canopa_ui::Action::Normal,
                false,
            )
            .into(),
            iced::widget::Space::new().width(Length::Fill).into(),
            canopa_ui::action(
                text(fl!("portal-print-auth-authenticate")),
                self.valid().then_some(Message::Authenticate),
                canopa_ui::Action::Normal,
                true,
            )
            .into(),
        ])
        .spacing(10);
        container(column![scrollable(content).height(Length::Fill), actions].spacing(16))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(canopa_ui::surface)
            .into()
    }
}

#[cfg(test)]
mod tests;
