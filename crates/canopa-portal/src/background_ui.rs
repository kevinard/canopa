//! Opened only by an explicit review action on the background notification.
use crate::background::{BackgroundRequest, Decision};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{column, container, row, scrollable, text},
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
                size: Size::new(680.0, 360.0),
                min_size: Some(Size::new(440.0, 300.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let (_, open) = window::open_dialog_with_activation(
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
                },
                open.map(|_| Message::Opened),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|_: &Dialog, _| "Background activity — Canopa".to_owned())
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(Dialog::subscription)
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("background result").take())
}

struct Dialog {
    request: BackgroundRequest,
    theme: Theme,
    output: Arc<Mutex<Option<Decision>>>,
    completed: bool,
}
#[derive(Clone, Debug)]
enum Message {
    Opened,
    Decide(Decision),
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
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Decide(decision) => {
                *self.output.lock().expect("background result") = Some(decision);
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
        let content = column![
            text("Allow background activity?").size(22),
            text(&self.request.name).size(18),
            text(&self.request.app_id).style(canopa_ui::muted),
            text("This app is running without open windows. Allow once applies only to its current run. Block stops the current run."),
            text("Closing this question keeps the current run without changing its permission.").style(canopa_ui::muted),
        ].spacing(14).width(Length::Fill);
        let choices = row![
            canopa_ui::action(
                text("Block"),
                Some(Message::Decide(Decision::Forbid)),
                canopa_ui::Action::Normal,
                false
            )
            .width(Length::Fill),
            canopa_ui::action(
                text("Allow once"),
                Some(Message::Decide(Decision::AllowOnce)),
                canopa_ui::Action::Normal,
                false
            )
            .width(Length::Fill),
            canopa_ui::action(
                text("Allow"),
                Some(Message::Decide(Decision::Allow)),
                canopa_ui::Action::Normal,
                false
            )
            .width(Length::Fill),
        ]
        .spacing(12);
        let footer = column![
            choices,
            canopa_ui::action(
                text("Cancel"),
                Some(Message::Cancel),
                canopa_ui::Action::Quiet,
                false
            )
            .width(Length::Fill)
        ]
        .spacing(8);
        container(column![scrollable(content).height(Length::Fill), footer].spacing(18))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(canopa_ui::surface)
            .into()
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
