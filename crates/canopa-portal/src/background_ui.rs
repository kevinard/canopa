//! Opened only by an explicit review action on the background notification.
use crate::accessibility_ui::{self as a11y, Node, semantic};
use crate::background::{BackgroundRequest, Decision};
use crate::dialog_ui;
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{column, container, text},
    window,
};
use std::sync::{Arc, Mutex};

pub fn run(
    request: BackgroundRequest,
    appearance: canopa_theme::Appearance,
    activation_token: Option<String>,
) -> anyhow::Result<Option<Decision>> {
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(640.0, 410.0),
                min_size: Some(Size::new(470.0, 340.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let (window, open) = window::open_dialog_with_activation(
                String::new(),
                false,
                activation_token.clone(),
                settings,
            );
            (
                Dialog {
                    request: request.clone(),
                    theme: theme.clone(),
                    output: output.clone(),
                    completed: false,
                    window,
                    accessibility: Some(a11y::Accessibility::new("canopa-background", WINDOW_KEY)),
                },
                open.map(|_| Message::Opened),
            )
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
    Ok(result.lock().expect("background result").take())
}

/// The AT-SPI window of the background question and its title.
const WINDOW_KEY: &str = "background";
fn window_title() -> String {
    fl!(
        "portal-window-title",
        title = fl!("portal-background-window")
    )
}

struct Dialog {
    request: BackgroundRequest,
    theme: Theme,
    output: Arc<Mutex<Option<Decision>>>,
    completed: bool,
    window: window::Id,
    accessibility: Option<a11y::Accessibility<Message>>,
}
#[derive(Clone, Debug)]
enum Message {
    Opened,
    Decide(Decision),
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
            Message::Decide(decision) => {
                *self.output.lock().expect("background result") = Some(decision);
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
        let question = fl!("portal-background-question");
        let choice = |key: &'static str, label: String, consequence, decision, primary| {
            let message = Message::Decide(decision);
            let button = if decision == Decision::Forbid {
                dialog_ui::danger(label.clone(), message.clone())
            } else {
                let treatment = if primary {
                    canopa_ui::Action::Primary
                } else {
                    canopa_ui::Action::Normal
                };
                dialog_ui::action(text(label.clone()), Some(message.clone()), treatment, false)
            };
            container(
                canopa_ui::direction::row([
                    dialog_ui::detail(consequence).width(Length::Fill).into(),
                    a11y::button(key, label.clone(), Some(message.clone()), button),
                ])
                .spacing(14)
                .align_y(iced::Center),
            )
            .padding([10, 12])
            .width(Length::Fill)
            .style(dialog_ui::card)
        };
        let content = column![
            semantic(
                Node::heading(question.clone(), 1),
                dialog_ui::heading(question.clone())
            ),
            dialog_ui::detail(fl!(
                "portal-requested-by",
                requester = self.request.name.as_str()
            )),
            text(fl!("portal-background-running")),
            choice(
                "background-allow-once",
                fl!("portal-background-allow-once"),
                fl!("portal-background-once-detail"),
                Decision::AllowOnce,
                true
            ),
            choice(
                "background-allow",
                fl!("portal-background-allow-future"),
                fl!("portal-background-future-detail"),
                Decision::Allow,
                false
            ),
            choice(
                "background-block",
                fl!("portal-background-block"),
                fl!("portal-background-block-detail"),
                Decision::Forbid,
                false
            ),
        ]
        .spacing(10)
        .width(Length::Fill);
        let close = fl!("portal-close");
        let footer = canopa_ui::direction::row([
            dialog_ui::detail(fl!("portal-background-close-detail"))
                .width(Length::Fill)
                .into(),
            a11y::button(
                "background-cancel",
                close.clone(),
                Some(Message::Cancel),
                dialog_ui::action(
                    text(close),
                    Some(Message::Cancel),
                    canopa_ui::Action::Quiet,
                    false,
                ),
            ),
        ])
        .spacing(8)
        .align_y(iced::Center);
        a11y::dialog_window(
            WINDOW_KEY,
            window_title(),
            Node::alert_dialog(question)
                .key("background-dialog")
                .description(fl!(
                    "portal-background-description",
                    name = self.request.name.as_str()
                )),
            dialog_ui::layout(content, footer),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dialog() -> Dialog {
        Dialog {
            request: BackgroundRequest::parse("org.canopa.Fixture".into(), "Fixture".into())
                .unwrap(),
            theme: Theme::Dark,
            output: Arc::default(),
            completed: false,
            window: window::Id::unique(),
            accessibility: None,
        }
    }
    #[test]
    fn opening_enter_cancel_and_late_buttons_do_not_change_permission() {
        let mut app = dialog();
        let _ = app.update(Message::Opened);
        let _ = app.update(Message::Event(Event::Keyboard(
            keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Enter),
                modified_key: keyboard::Key::Named(keyboard::key::Named::Enter),
                physical_key: keyboard::key::Physical::Unidentified(
                    keyboard::key::NativeCode::Unidentified,
                ),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::default(),
                text: None,
                repeat: false,
            },
        )));
        assert!(app.output.lock().unwrap().is_none());
        let _ = app.update(Message::Cancel);
        let _ = app.update(Message::Decide(Decision::Forbid));
        assert!(app.output.lock().unwrap().is_none());
    }
    #[test]
    fn each_explicit_decision_is_preserved_exactly_once() {
        for decision in [Decision::Forbid, Decision::Allow, Decision::AllowOnce] {
            let mut app = dialog();
            let _ = app.update(Message::Decide(decision));
            let _ = app.update(Message::Decide(Decision::AllowOnce));
            assert_eq!(*app.output.lock().unwrap(), Some(decision));
        }
    }
}
