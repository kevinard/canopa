//! Iced consent surface. Only an explicit grant action produces a grant.
use crate::accessibility_ui::{self as a11y, Node, semantic};
use crate::{
    access::{AccessRequest, Grant},
    worker::run as worker,
};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{checkbox, column, container, image, pick_list, scrollable, text},
    window,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Item(usize, String);
impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

pub fn run(
    request: AccessRequest,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<Option<Grant>> {
    let foreground = appearance.palette().text;
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(680.0, 440.0),
                min_size: Some(Size::new(400.0, 280.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let parent = request
                .parent_window
                .strip_prefix("wayland:")
                .unwrap_or_default();
            let (window, opening) = window::open_dialog(parent.to_owned(), request.modal, settings);
            let name = request
                .icon
                .clone()
                .filter(|name| !name.is_empty() && !name.contains('/'));
            let load = Task::perform(
                worker(move || {
                    name.and_then(|name| {
                        crate::icons::prepare(std::slice::from_ref(&name), foreground).remove(&name)
                    })
                }),
                |result| Message::Icon(result.ok().flatten()),
            );
            (
                Dialog {
                    request: request.clone(),
                    theme: theme.clone(),
                    icon: None,
                    output: output.clone(),
                    window,
                    accessibility: Some(a11y::Accessibility::new("canopa-access", WINDOW_KEY)),
                },
                Task::batch([opening.map(|_| Message::Opened), load]),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|app: &Dialog, _| app.window_title())
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(Dialog::subscription)
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("access result").take())
}

/// The AT-SPI window of the consent dialog.
const WINDOW_KEY: &str = "access";

struct Dialog {
    request: AccessRequest,
    theme: Theme,
    icon: Option<image::Handle>,
    output: Arc<Mutex<Option<Grant>>>,
    window: window::Id,
    accessibility: Option<a11y::Accessibility<Message>>,
}

#[derive(Clone, Debug)]
enum Message {
    Opened,
    Icon(Option<image::Handle>),
    Choice(usize, String),
    Grant,
    Deny,
    Event(Event),
    Accessibility(canopa_ui::accessibility::Event<Message>),
}

impl canopa_ui::accessibility::Message for Message {
    fn accessibility(event: canopa_ui::accessibility::Event<Self>) -> Self {
        Self::Accessibility(event)
    }
}

impl Dialog {
    fn window_title(&self) -> String {
        let title = if self.request.title.is_empty() {
            fl!("portal-access-title")
        } else {
            self.request.title.clone()
        };
        fl!("portal-window-title", title = title)
    }
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
    /// Handle a message, then refresh the accessibility tree if one is read.
    fn update(&mut self, message: Message) -> Task<Message> {
        if let Message::Accessibility(event) = message {
            return a11y::update(&mut self.accessibility, event);
        }
        let task = self.apply(message);
        a11y::refresh(&mut self.accessibility, task)
    }
    fn exit(&mut self) -> Task<Message> {
        if let Some(state) = &mut self.accessibility {
            state.close();
        }
        iced::exit()
    }
    fn apply(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Opened => {
                let title = self.window_title();
                let window = self.window;
                if let Some(state) = &mut self.accessibility {
                    state.opened(&title, window);
                }
            }
            Message::Accessibility(_) => {}
            Message::Icon(icon) => self.icon = icon,
            Message::Deny => return self.exit(),
            Message::Grant => {
                *self.output.lock().expect("access result") = Some(Grant {
                    choices: self
                        .request
                        .choices
                        .iter()
                        .map(|(id, _, _, value)| (id.clone(), value.clone()))
                        .collect(),
                });
                return self.exit();
            }
            Message::Choice(index, value) => {
                if let Some((_, _, options, selected)) = self.request.choices.get_mut(index)
                    && ((options.is_empty() && matches!(value.as_str(), "true" | "false"))
                        || options.iter().any(|(id, _)| id == &value))
                {
                    *selected = value;
                }
            }
            Message::Event(Event::Window(window::Event::CloseRequested))
            | Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            })) => return self.apply(Message::Deny),
            Message::Event(_) => {}
        }
        Task::none()
    }
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let title = semantic(
            Node::heading(&self.request.title, 1),
            text(&self.request.title).size(22),
        );
        let heading: Element<'_, Message> = if let Some(icon) = &self.icon {
            canopa_ui::direction::row([image(icon.clone()).width(40).height(40).into(), title])
                .spacing(12)
                .align_y(iced::Center)
                .into()
        } else {
            title
        };
        let mut content = column![heading].spacing(14).width(Length::Fill);
        if !self.request.app_id.is_empty() {
            content = content.push(text(&self.request.app_id).size(13).style(canopa_ui::muted));
        }
        if !self.request.subtitle.is_empty() {
            content = content.push(text(&self.request.subtitle).size(18));
        }
        if !self.request.body.is_empty() {
            content = content.push(text(&self.request.body));
        }
        for (index, (id, label, options, value)) in self.request.choices.iter().enumerate() {
            let key = format!("access-choice-{}", id.escape_default());
            if options.is_empty() {
                let checked = value == "true";
                content = content.push(a11y::check_box(
                    key,
                    label,
                    checked,
                    Some(Message::Choice(index, (!checked).to_string())),
                    checkbox(checked)
                        .style(canopa_ui::checkbox_style)
                        .label(label)
                        .on_toggle(move |checked| Message::Choice(index, checked.to_string())),
                ));
            } else {
                let items: Vec<_> = options
                    .iter()
                    .enumerate()
                    .map(|(i, (_, label))| Item(i, label.clone()))
                    .collect();
                let current = options.iter().position(|(id, _)| id == value);
                let selected = current.map(|i| items[i].clone());
                let messages = options
                    .iter()
                    .map(|(option, label)| (label.clone(), Message::Choice(index, option.clone())))
                    .collect();
                content = content.push(
                    column![
                        text(label),
                        a11y::combo_box(
                            key,
                            label,
                            messages,
                            current,
                            pick_list(items, selected, move |item| Message::Choice(
                                index,
                                options[item.0].0.clone()
                            ))
                        )
                    ]
                    .spacing(6),
                );
            }
        }
        let actions = canopa_ui::direction::row([
            a11y::button(
                "access-deny",
                &self.request.deny_label,
                Some(Message::Deny),
                canopa_ui::action(
                    text(&self.request.deny_label),
                    Some(Message::Deny),
                    canopa_ui::Action::Normal,
                    false,
                )
                .width(Length::Fill),
            ),
            a11y::button(
                "access-grant",
                &self.request.grant_label,
                Some(Message::Grant),
                canopa_ui::action(
                    text(&self.request.grant_label),
                    Some(Message::Grant),
                    canopa_ui::Action::Normal,
                    false,
                )
                .width(Length::Fill),
            ),
        ])
        .spacing(12)
        .align_y(iced::Center);
        // A consent question: an alert dialog, described by what it asks.
        let description = [&self.request.subtitle, &self.request.body]
            .into_iter()
            .filter(|part| !part.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        let dialog_title = if self.request.title.is_empty() {
            fl!("portal-access-title")
        } else {
            self.request.title.clone()
        };
        a11y::dialog_window(
            WINDOW_KEY,
            self.window_title(),
            Node::alert_dialog(dialog_title)
                .key("access-dialog")
                .description(description),
            container(column![scrollable(content).height(Length::Fill), actions].spacing(18))
                .padding(20)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(canopa_ui::surface),
        )
    }
}

#[cfg(all(test, feature = "accessibility-testing"))]
mod accessibility_tests;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_enter_and_unknown_controls_cannot_grant() {
        let request = AccessRequest::parse(
            "example.App".into(),
            String::new(),
            "Permission".into(),
            String::new(),
            String::new(),
            Default::default(),
        )
        .unwrap();
        let mut dialog = Dialog {
            request,
            theme: canopa_ui::theme(Default::default()),
            icon: None,
            output: Arc::default(),
            window: window::Id::unique(),
            accessibility: None,
        };
        let _ = dialog.update(Message::Event(Event::Keyboard(
            keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Enter),
                modified_key: keyboard::Key::Named(keyboard::key::Named::Enter),
                physical_key: keyboard::key::Physical::Unidentified(
                    keyboard::key::NativeCode::Unidentified,
                ),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::empty(),
                text: None,
                repeat: false,
            },
        )));
        assert!(dialog.output.lock().unwrap().is_none());
        let _ = dialog.update(Message::Choice(0, "not-requested".into()));
        assert!(dialog.output.lock().unwrap().is_none());
        let _ = dialog.update(Message::Deny);
        assert!(dialog.output.lock().unwrap().is_none());
    }
}
