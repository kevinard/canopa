//! Transient drafts for the backend's exact device question. No accepted work lives here.
use super::devices_ui::{Edited, Event, Field, Toggle};
use super::*;
use crate::local_volumes::{
    DeviceAnswer, DeviceOperation, DevicePhase, DevicePrompt, DeviceQuestion, DeviceQuestionToken,
    DeviceRequestId, DeviceRequestSnapshot, Remember,
};
use canopa_theme::UiKind;
use iced::{
    Length,
    widget::{checkbox, column, container, row, scrollable, text, text_input},
};

pub(super) struct Dialog {
    pub window: window::Id,
    pub parent: window::Id,
    pub job: DeviceRequestId,
    pub token: DeviceQuestionToken,
    pub question: DeviceQuestion,
    name: String,
    username: String,
    domain: String,
    password: String,
    pim: String,
    anonymous: bool,
    hidden: bool,
    system: bool,
    remember: Remember,
    /// A server login or question, not a local device.
    network: bool,
    error: Option<String>,
}

impl Dialog {
    fn new(
        window: window::Id,
        parent: window::Id,
        job: &DeviceRequestSnapshot,
        prompt: DevicePrompt,
    ) -> Self {
        let (username, domain) = match &prompt.question {
            DeviceQuestion::Password {
                username, domain, ..
            } => (username.clone(), domain.clone()),
            _ => (String::new(), String::new()),
        };
        Self {
            window,
            parent,
            job: job.id,
            token: prompt.token,
            question: prompt.question,
            name: job.name.clone(),
            username,
            domain,
            password: String::new(),
            pim: String::new(),
            anonymous: false,
            hidden: false,
            system: false,
            remember: Remember::Never,
            network: job.operation == DeviceOperation::Connect || job.uri.is_some(),
            error: None,
        }
    }
    pub fn title(&self) -> &'static str {
        match &self.question {
            DeviceQuestion::Password { .. } if self.network => "Connect to server",
            DeviceQuestion::Password { .. } => "Unlock device",
            DeviceQuestion::Choice { blocked: true, .. } => "Device is busy",
            DeviceQuestion::Choice { .. } if self.host_identity() => "Check the server's identity",
            DeviceQuestion::Choice { .. } if self.network => "Server question",
            DeviceQuestion::Choice { .. } => "Device question",
        }
    }

    /// SSH asks to confirm an unknown or changed host key.
    fn host_identity(&self) -> bool {
        let DeviceQuestion::Choice { message, .. } = &self.question else {
            return false;
        };
        let message = message.to_lowercase();
        self.network
            && (message.contains("identity")
                || message.contains("host key")
                || message.contains("fingerprint"))
    }
}

fn control(window: window::Id, name: impl std::fmt::Debug) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-device-question-{window:?}-{name:?}"))
}
fn scroll(window: window::Id) -> iced::widget::Id {
    control(window, "scroll")
}

impl Desktop {
    pub(super) fn auto_device_question(&mut self) -> Task<Message> {
        if self.devices.dialog.is_some() || self.file_operations.question.is_some() {
            return Task::none();
        }
        let question =
            self.devices
                .requests
                .iter()
                .find(|job| {
                    job.phase == DevicePhase::Running
                        && job.question.as_ref().is_some_and(|prompt| {
                            self.devices.seen.get(&job.id) != Some(&prompt.token)
                        })
                        && (self.operations_window.is_some()
                            && self.focused_window == self.operations_window
                            || self.devices.owners.get(&job.id).is_some_and(|owner| {
                                self.focused_window == Some(*owner)
                                    && self
                                        .devices
                                        .origins
                                        .get(&job.id)
                                        .is_none_or(|origin| origin.current(self))
                            }))
                })
                .map(|job| job.id);
        question.map_or_else(Task::none, |job| self.show_device_question(job, false))
    }

    pub(super) fn show_device_question(
        &mut self,
        id: DeviceRequestId,
        manual: bool,
    ) -> Task<Message> {
        if let Some(dialog) = &self.devices.dialog {
            return window::gain_focus(dialog.window);
        }
        let Some(job) = self.devices.requests.iter().find(|job| job.id == id) else {
            return Task::none();
        };
        let Some(prompt) = job.question.clone() else {
            return Task::none();
        };
        let parent = if manual || self.focused_window == self.operations_window {
            self.operations_window
        } else {
            self.devices
                .owners
                .get(&id)
                .copied()
                .filter(|parent| self.windows.contains_key(parent))
        };
        let Some(parent) = parent else {
            return Task::none();
        };
        // Server questions (SSH host keys) carry long explanations.
        let network = job.operation == crate::local_volumes::DeviceOperation::Connect;
        let short = !network
            && matches!(&prompt.question, DeviceQuestion::Choice { blocked: false, choices, .. } if choices.len() <= 3);
        let height = prompt.question.password_fields().map_or(
            if short {
                205.0
            } else if network {
                330.0
            } else {
                240.0
            },
            |fields| {
                let count = [
                    fields.username,
                    fields.domain,
                    fields.password,
                    fields.anonymous,
                ]
                .into_iter()
                .filter(|shown| *shown)
                .count();
                205.0
                    + count as f32 * 68.0
                    + if fields.tcrypt { 115.0 } else { 0.0 }
                    + if fields.save { 132.0 } else { 0.0 }
            },
        );
        let (window, opened) = window::open_transient(
            parent,
            window::Settings {
                size: Size::new(520.0, height),
                min_size: Some(Size::new(320.0, if short { 190.0 } else { 240.0 })),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Files".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        self.devices.seen.insert(id, prompt.token);
        self.devices.dialog = Some(Dialog::new(window, parent, job, prompt));
        opened.map(move |opened| Message::Device(Event::QuestionOpened(window, opened)))
    }

    pub(super) fn sync_device_question(&mut self) -> Task<Message> {
        let Some(dialog) = &self.devices.dialog else {
            return Task::none();
        };
        let current = self
            .devices
            .requests
            .iter()
            .find(|job| job.id == dialog.job);
        if let Some((job, prompt)) =
            current.and_then(|job| job.question.clone().map(|prompt| (job, prompt)))
        {
            if prompt.token != dialog.token {
                let (window, parent) = (dialog.window, dialog.parent);
                self.devices.seen.insert(job.id, prompt.token);
                self.devices.dialog = Some(Dialog::new(window, parent, job, prompt));
                // Replacing a question also retires its drafts and old focus.
                return iced::widget::operation::focus(control(window, "cancel"));
            }
            return Task::none();
        }
        self.dismiss_device_question(dialog.window)
    }

    pub(super) fn dismiss_device_question(&mut self, id: window::Id) -> Task<Message> {
        let Some(dialog) = self.devices.dialog.take_if(|dialog| dialog.window == id) else {
            return Task::none();
        };
        let focus = if self.focused_window == Some(id)
            && (self.windows.contains_key(&dialog.parent)
                || self.operations_window == Some(dialog.parent))
        {
            window::gain_focus(dialog.parent)
        } else {
            Task::none()
        };
        focus.chain(window::close(id))
    }

    pub(super) fn device_question_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::QuestionOpened(id, opened) => {
                if self
                    .devices
                    .dialog
                    .as_ref()
                    .is_none_or(|dialog| dialog.window != id)
                {
                    return window::close(id);
                }
                if opened.is_none() {
                    self.devices.dialog = None;
                    self.devices.error = Some(
                        "Could not open the device question. Review it in Operations to try again."
                            .into(),
                    );
                    return self.exit_if_empty();
                }
                return iced::widget::operation::focus(control(id, "cancel"))
                    .chain(window::gain_focus(id));
            }
            Event::Text(id, token, field, Edited(value)) => {
                if let Some(dialog) = self
                    .devices
                    .dialog
                    .as_mut()
                    .filter(|dialog| dialog.window == id && dialog.token == token)
                {
                    if value.len() > 4096 || value.contains('\0') {
                        dialog.error = Some("The field exceeds its input limit.".into());
                        return Task::none();
                    }
                    match field {
                        Field::Username => dialog.username = value,
                        Field::Domain => dialog.domain = value,
                        Field::Password => dialog.password = value,
                        Field::Pim => dialog.pim = value,
                    }
                    dialog.error = None;
                }
            }
            Event::Toggle(id, token, toggle) => {
                if let Some(dialog) = self
                    .devices
                    .dialog
                    .as_mut()
                    .filter(|dialog| dialog.window == id && dialog.token == token)
                {
                    match toggle {
                        Toggle::Anonymous => {
                            dialog.anonymous = !dialog.anonymous;
                            dialog.password.clear();
                        }
                        Toggle::Hidden => dialog.hidden = !dialog.hidden,
                        Toggle::System => dialog.system = !dialog.system,
                        Toggle::Remember(remember) => dialog.remember = remember,
                    }
                }
            }
            Event::Submit(id, token) => {
                let Some(dialog) = self.devices.dialog.as_mut().filter(|dialog| {
                    dialog.window == id
                        && dialog.token == token
                        && matches!(dialog.question, DeviceQuestion::Password { .. })
                }) else {
                    return Task::none();
                };
                let pim = if dialog.pim.is_empty() {
                    0
                } else {
                    match dialog.pim.parse::<u32>() {
                        Ok(pim) => pim,
                        Err(_) => {
                            dialog.error = Some("Enter a whole number for PIM.".into());
                            return Task::none();
                        }
                    }
                };
                let token = dialog.token;
                let answer = DeviceAnswer::Password {
                    username: dialog.username.clone(),
                    domain: dialog.domain.clone(),
                    password: std::mem::take(&mut dialog.password),
                    anonymous: dialog.anonymous,
                    pim,
                    hidden_volume: dialog.hidden,
                    system_volume: dialog.system,
                    save: if dialog
                        .question
                        .password_fields()
                        .is_some_and(|fields| fields.save)
                        && !dialog.anonymous
                    {
                        dialog.remember
                    } else {
                        Remember::Never
                    },
                };
                return self.answer_device_question(id, token, answer);
            }
            Event::Choice(id, token, choice) => {
                let Some(dialog) = self
                    .devices
                    .dialog
                    .as_ref()
                    .filter(|dialog| dialog.window == id && dialog.token == token)
                else {
                    return Task::none();
                };
                return self.answer_device_question(
                    id,
                    dialog.token,
                    DeviceAnswer::Choice { choice },
                );
            }
            Event::CancelQuestion(id) => {
                let Some(job) = self
                    .devices
                    .dialog
                    .as_ref()
                    .filter(|dialog| dialog.window == id)
                    .map(|dialog| dialog.job)
                else {
                    return Task::none();
                };
                if let Some(runtime) = self.device_runtime() {
                    match runtime.device_requests().cancel(job) {
                        Ok(()) => {
                            return Task::batch([
                                self.dismiss_device_question(id),
                                self.poll_devices(),
                            ]);
                        }
                        Err(error) => {
                            if let Some(dialog) = &mut self.devices.dialog {
                                dialog.error = Some(error.to_string());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn answer_device_question(
        &mut self,
        window: window::Id,
        token: DeviceQuestionToken,
        answer: DeviceAnswer,
    ) -> Task<Message> {
        let result = self
            .device_runtime()
            .ok_or_else(|| io::Error::other("Devices are unavailable."))
            .and_then(|runtime| runtime.device_requests().answer(token, answer));
        match result {
            Ok(()) => Task::batch([self.dismiss_device_question(window), self.poll_devices()]),
            Err(error) => {
                if let Some(dialog) = &mut self.devices.dialog {
                    dialog.error = Some(error.to_string());
                }
                Task::none()
            }
        }
    }

    pub(super) fn device_question_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || modifiers.control() && key.as_ref() == keyboard::Key::Character("w")
        {
            return self.device_question_event(Event::CancelQuestion(id));
        }
        super::scrolling::key(scroll(id), key, modifiers)
    }
}

fn input<'a>(
    desktop: &'a Desktop,
    dialog: &'a Dialog,
    field: Field,
    label: &'static str,
    value: &'a str,
    secure: bool,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let id = dialog.window;
    let token = dialog.token;
    let mut input = text_input("", value)
        .id(control(id, field))
        .secure(secure)
        .font(canopa_ui::font(appearance))
        .size(appearance.typography().body_size.px())
        .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
        .style(move |theme, status| super::view::input_style(appearance, theme, status));
    if !dialog.anonymous || matches!(field, Field::Pim) {
        input = input
            .on_input(move |value| Message::Device(Event::Text(id, token, field, Edited(value))))
            .on_submit(Message::Device(Event::Submit(id, token)));
    }
    column![
        text(label).size(appearance.typography().label_size.px()),
        input
    ]
    .spacing(4)
    .into()
}

pub(super) fn view<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let id = dialog.window;
    let mut order = vec![control(id, "cancel")];
    let button = |label: String, name: String, event: Event| {
        let message = Message::Device(event);
        let primary = name == "submit" || matches!(label.as_str(), "Continue" | "Try again");
        super::focus::control(
            control(id, name.clone()),
            Some(message.clone()),
            appearance,
            canopa_ui::styled_action(
                text(label).font(font).size(small),
                Some(message),
                if primary {
                    canopa_ui::Action::Primary
                } else if name == "cancel" {
                    canopa_ui::Action::Quiet
                } else {
                    canopa_ui::Action::Normal
                },
                false,
                appearance,
            ),
        )
    };
    let message = match &dialog.question {
        DeviceQuestion::Password { message, .. } | DeviceQuestion::Choice { message, .. } => {
            message
        }
    };
    let many_choices =
        matches!(&dialog.question, DeviceQuestion::Choice { choices, .. } if choices.len() > 3);
    let mut body = column![
        chrome::dialog_heading(desktop, chrome::Icon::Drive, dialog.title(), &dialog.name),
        text(message)
            .font(font)
            .size(size)
            .wrapping(text::Wrapping::WordOrGlyph),
    ]
    .spacing(12);
    let mut actions = row![button(
        "Cancel".into(),
        "cancel".into(),
        Event::CancelQuestion(id)
    )]
    .spacing(8);
    if let Some(fields) = dialog.question.password_fields() {
        for (show, field, label, value, secure) in [
            (
                fields.username,
                Field::Username,
                "Username",
                &dialog.username,
                false,
            ),
            (
                fields.domain,
                Field::Domain,
                "Domain",
                &dialog.domain,
                false,
            ),
            (
                fields.password,
                Field::Password,
                "Password",
                &dialog.password,
                true,
            ),
            (
                fields.tcrypt,
                Field::Pim,
                "PIM (optional)",
                &dialog.pim,
                false,
            ),
        ] {
            if show {
                body = body.push(input(desktop, dialog, field, label, value, secure));
                if !dialog.anonymous || matches!(field, Field::Pim) {
                    order.push(control(id, field));
                }
            }
        }
        for (show, toggle, label, value) in [
            (
                fields.anonymous,
                Toggle::Anonymous,
                "Connect anonymously",
                dialog.anonymous,
            ),
            (
                fields.tcrypt,
                Toggle::Hidden,
                "Hidden volume",
                dialog.hidden,
            ),
            (
                fields.tcrypt,
                Toggle::System,
                "System volume",
                dialog.system,
            ),
        ] {
            if show {
                let event = Event::Toggle(id, dialog.token, toggle);
                order.push(control(id, toggle));
                body = body.push(super::focus::control(
                    control(id, toggle),
                    Some(Message::Device(event.clone())),
                    appearance,
                    checkbox(value)
                        .label(label)
                        .font(font)
                        .text_size(size)
                        .style(canopa_ui::checkbox_style)
                        .on_toggle(move |_| Message::Device(event.clone())),
                ));
            }
        }
        if fields.save && !dialog.anonymous {
            // GVFS keeps a remembered password in the Secret Service keyring;
            // Files only passes the choice on and never stores it.
            for remember in [Remember::Never, Remember::ForSession, Remember::Permanently] {
                let event = Event::Toggle(id, dialog.token, Toggle::Remember(remember));
                let name = format!("remember-{remember:?}");
                order.push(control(id, name.clone()));
                body = body.push(super::focus::control(
                    control(id, name),
                    Some(Message::Device(event.clone())),
                    appearance,
                    checkbox(dialog.remember == remember)
                        .label(remember.label())
                        .font(font)
                        .text_size(size)
                        .style(canopa_ui::checkbox_style)
                        .on_toggle(move |_| Message::Device(event.clone())),
                ));
            }
            body = body.push(
                text("A remembered password is kept in your keyring by GVFS, not by Files.")
                    .size(small)
                    .font(font)
                    .style(canopa_ui::muted),
            );
        } else {
            body = body.push(
                text("Files uses these credentials for this request only.")
                    .size(small)
                    .font(font)
                    .style(canopa_ui::muted),
            );
        }
        order.push(control(id, "submit"));
        actions = actions.push(button(
            if dialog.network { "Connect" } else { "Unlock" }.into(),
            "submit".into(),
            Event::Submit(id, dialog.token),
        ));
    } else if let DeviceQuestion::Choice {
        choices, blocked, ..
    } = &dialog.question
    {
        if *blocked {
            body = body.push(
                text("Close the application before retrying.")
                    .size(small)
                    .font(font),
            );
        }
        if dialog.host_identity() {
            body = body.push(
                text("Continue only if you expect to connect to this server, for example on your own network. SSH remembers its identity for next time; if it changes later, Files cannot connect until it is checked.")
                    .size(small)
                    .font(font)
                    .wrapping(text::Wrapping::WordOrGlyph),
            );
        }
        // The shared Cancel already cancels this request. Keep provider indices
        // for every other action without showing two identical Cancel buttons.
        for (index, choice) in choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| !is_cancel(choice))
        {
            let name = format!("choice-{index}");
            order.push(control(id, name.clone()));
            actions = actions.push(button(
                choice.clone(),
                name,
                Event::Choice(id, dialog.token, index),
            ));
        }
    }
    if let Some(error) = &dialog.error {
        body = body.push(text(error).size(small).font(font));
    }
    let reveal = order
        .iter()
        .cloned()
        .map(|control| (control, scroll(id)))
        .collect();
    let footer = super::question_dialog::decision_actions(actions, None);
    let scrollbar = scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(6)
            .scroller_width(6)
            .spacing(6),
    );
    let body: Element<'_, Message> = if !many_choices {
        column![
            scrollable(body)
                .id(scroll(id))
                .direction(scrollbar)
                .height(Length::Fill),
            footer,
        ]
        .spacing(10)
        .into()
    } else {
        scrollable(body.push(footer))
            .id(scroll(id))
            .direction(scrollbar)
            .height(Length::Fill)
            .into()
    };
    let content = container(body)
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| canopa_ui::frame(appearance, appearance.style(UiKind::Panel)));
    super::focus::group(content, order, reveal, Some(control(id, "cancel")))
}

/// The provider's own cancel choice ("Cancel", or SSH's "Cancel Login"): the
/// shared Cancel button already cancels the request.
fn is_cancel(choice: &str) -> bool {
    choice == "Cancel" || choice.starts_with("Cancel ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn provider_cancel_choices_are_folded_into_the_shared_cancel() {
        assert!(super::is_cancel("Cancel"));
        assert!(super::is_cancel("Cancel Login"));
        assert!(!super::is_cancel("Log In Anyway"));
        assert!(!super::is_cancel("Cancellation policy"));
    }
}
