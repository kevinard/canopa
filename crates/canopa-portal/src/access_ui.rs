//! Iced consent surface. Only an explicit grant action produces a grant.
use crate::accessibility_ui::{self as a11y, Node, semantic};
use crate::dialog_ui;
use crate::{
    access::{AccessRequest, Grant},
    worker::run as worker,
};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{checkbox, column, container, image, pick_list, text},
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
    let requester = dialog_ui::requester(&request.app_id);
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(680.0, {
                    let lines = request
                        .body
                        .lines()
                        .map(|line| line.chars().count().div_ceil(65))
                        .sum::<usize>();
                    let has_menu = request.choices.iter().any(|choice| !choice.2.is_empty());
                    let choices: usize = request
                        .choices
                        .iter()
                        .map(|choice| if choice.2.is_empty() { 30 } else { 62 })
                        .sum();
                    let menu_room = if has_menu { 64 } else { 0 };
                    let inset = usize::from(!request.choices.is_empty()) * 10;
                    (200 + lines * 20 + choices + menu_room + inset).clamp(248, 504) as f32
                }),
                min_size: Some(Size::new(400.0, 248.0)),
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
                    requester: requester.clone(),
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
    requester: String,
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
        let dialog_title = if self.request.title.is_empty() {
            fl!("portal-access-title")
        } else {
            self.request.title.clone()
        };
        let mut identity = column![semantic(
            Node::heading(&dialog_title, 1),
            dialog_ui::heading(&dialog_title),
        )]
        .spacing(4)
        .width(Length::Fill);
        if !self.request.app_id.is_empty() {
            identity = identity.push(dialog_ui::detail(fl!(
                "portal-requested-by",
                requester = self.requester.as_str()
            )));
        }
        let heading: Element<'_, Message> = if let Some(icon) = &self.icon {
            let icon = container(image(icon.clone()).width(26).height(26))
                .center(48)
                .style(dialog_ui::card);
            canopa_ui::direction::row([icon.into(), identity.into()])
                .spacing(12)
                .align_y(iced::Center)
                .into()
        } else {
            identity.into()
        };
        let mut content = column![heading].spacing(11).width(Length::Fill);
        if !self.request.subtitle.is_empty() && self.request.subtitle != dialog_title {
            content = content.push(text(&self.request.subtitle).size(17));
        }
        if !self.request.body.is_empty() {
            content = content.push(dialog_ui::detail(&self.request.body).size(15));
        }
        let mut choices = column![].spacing(12);
        for (index, (id, label, options, value)) in self.request.choices.iter().enumerate() {
            let key = format!("access-choice-{}", id.escape_default());
            if options.is_empty() {
                let checked = value == "true";
                choices = choices.push(a11y::check_box(
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
                choices = choices.push(
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
        if !self.request.choices.is_empty() {
            content = content.push(
                container(choices)
                    .padding([10, 12])
                    .width(Length::Fill)
                    .style(dialog_ui::card),
            );
        }
        let actions = dialog_ui::footer([
            a11y::button(
                "access-deny",
                &self.request.deny_label,
                Some(Message::Deny),
                dialog_ui::action(
                    text(&self.request.deny_label),
                    Some(Message::Deny),
                    canopa_ui::Action::Normal,
                    false,
                ),
            ),
            a11y::button(
                "access-grant",
                &self.request.grant_label,
                Some(Message::Grant),
                dialog_ui::action(
                    text(&self.request.grant_label),
                    Some(Message::Grant),
                    canopa_ui::Action::Primary,
                    false,
                ),
            ),
        ]);
        // A consent question: an alert dialog, described by what it asks.
        let description = [&self.request.subtitle, &self.request.body]
            .into_iter()
            .filter(|part| !part.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        a11y::dialog_window(
            WINDOW_KEY,
            self.window_title(),
            Node::alert_dialog(dialog_title)
                .key("access-dialog")
                .description(description),
            dialog_ui::layout(content, actions),
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
            requester: request.app_id.clone(),
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
