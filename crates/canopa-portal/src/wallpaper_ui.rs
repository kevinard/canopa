//! Review only: the backend keeps the full frozen image and installs after Apply.
use crate::dialog_ui;
use std::sync::{Arc, Mutex};

use crate::accessibility_ui::{self as a11y, Node, semantic};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{column, container, image, text},
    window,
};

pub use crate::wallpaper_image::DialogInput;

pub fn run(input: DialogInput, appearance: canopa_theme::Appearance) -> anyhow::Result<bool> {
    anyhow::ensure!(
        input.app_id.len() <= 4096
            && input.parent_window.len() <= 4096
            && input.width > 0
            && input.height > 0
            && input.width <= 16_384
            && input.height <= 16_384,
        "Invalid wallpaper review"
    );
    let (width, height, pixels) = input.preview.pixels()?;
    let preview = image::Handle::from_rgba(width, height, pixels);
    let theme = canopa_ui::theme(appearance.clone());
    let requester = dialog_ui::requester(&input.app_id);
    let font = canopa_ui::font(&appearance);
    let output = Arc::new(Mutex::new(false));
    let result = output.clone();
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(700.0, 570.0),
                min_size: Some(Size::new(420.0, 360.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let parent = input
                .parent_window
                .strip_prefix("wayland:")
                .unwrap_or_default();
            let (window, open) = window::open_dialog(parent.to_owned(), true, settings);
            (
                Dialog {
                    requester: requester.clone(),
                    destination: input.destination,
                    dimensions: (input.width, input.height),
                    preview: preview.clone(),
                    theme: theme.clone(),
                    output: output.clone(),
                    completed: false,
                    window,
                    accessibility: Some(a11y::Accessibility::new("canopa-wallpaper", WINDOW_KEY)),
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
    Ok(*result.lock().expect("wallpaper result"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog() -> Dialog {
        Dialog {
            requester: "app".into(),
            destination: crate::wallpaper::Destination::Both,
            dimensions: (100, 50),
            preview: image::Handle::from_rgba(1, 1, vec![1, 2, 3, 255]),
            theme: canopa_ui::theme(canopa_theme::Appearance::default()),
            output: Arc::new(Mutex::new(false)),
            completed: false,
            window: window::Id::unique(),
            accessibility: None,
        }
    }

    #[test]
    fn only_apply_accepts_and_completed_review_cannot_be_changed() {
        let mut cancelled = dialog();
        let _ = cancelled.update(Message::Opened);
        assert!(!*cancelled.output.lock().unwrap());
        let _ = cancelled.update(Message::Cancel);
        let _ = cancelled.update(Message::Apply);
        assert!(!*cancelled.output.lock().unwrap());
        let mut accepted = dialog();
        let _ = accepted.update(Message::Apply);
        let _ = accepted.update(Message::Cancel);
        assert!(*accepted.output.lock().unwrap());
    }
}

/// The AT-SPI window of the wallpaper review and its title.
const WINDOW_KEY: &str = "wallpaper";
fn window_title() -> String {
    fl!(
        "portal-window-title",
        title = fl!("portal-wallpaper-window")
    )
}

struct Dialog {
    requester: String,
    destination: crate::wallpaper::Destination,
    dimensions: (u32, u32),
    preview: image::Handle,
    theme: Theme,
    output: Arc<Mutex<bool>>,
    completed: bool,
    window: window::Id,
    accessibility: Option<a11y::Accessibility<Message>>,
}

#[derive(Clone, Debug)]
enum Message {
    Opened,
    Apply,
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
            Message::Apply => {
                *self.output.lock().expect("wallpaper result") = true;
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
        let requester = self.requester.as_str();
        let question = fl!("portal-wallpaper-question");
        let size = fl!(
            "portal-pixel-size",
            width = self.dimensions.0.to_string(),
            height = self.dimensions.1.to_string()
        );
        let preview = semantic(
            Node::image(fl!("portal-wallpaper-preview")).key("wallpaper-preview"),
            container(
                image(self.preview.clone())
                    .content_fit(iced::ContentFit::Contain)
                    .width(Length::Fill)
                    .height(290),
            )
            .style(dialog_ui::card),
        );
        let content = column![
            semantic(
                Node::heading(question.clone(), 1),
                dialog_ui::heading(question.clone())
            ),
            text(fl!("portal-requested-by", requester = requester)).style(canopa_ui::muted),
            canopa_ui::direction::row([
                text(fl!(
                    "portal-wallpaper-scope",
                    destination = self.destination.label()
                ))
                .size(16)
                .into(),
                iced::widget::space().width(Length::Fill).into(),
                dialog_ui::detail(size.clone()).into(),
            ])
            .align_y(iced::Center),
            preview
        ]
        .spacing(14)
        .width(Length::Fill);
        let action = |key: &'static str, label: String, message: Message, style| {
            a11y::button(
                key,
                label.clone(),
                Some(message.clone()),
                dialog_ui::action(text(label), Some(message), style, false),
            )
        };
        let actions = dialog_ui::footer([
            action(
                "wallpaper-cancel",
                fl!("portal-cancel"),
                Message::Cancel,
                canopa_ui::Action::Normal,
            ),
            action(
                "wallpaper-apply",
                fl!("portal-wallpaper-apply"),
                Message::Apply,
                canopa_ui::Action::Primary,
            ),
        ]);
        a11y::dialog_window(
            WINDOW_KEY,
            window_title(),
            Node::dialog(question)
                .key("wallpaper-dialog")
                .description(fl!(
                    "portal-wallpaper-description",
                    requester = requester,
                    destination = self.destination.label(),
                    size = size
                )),
            dialog_ui::layout(content, actions),
        )
    }
}
