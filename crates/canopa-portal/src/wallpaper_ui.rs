//! Review only: the backend keeps the full frozen image and installs after Apply.
use std::sync::{Arc, Mutex};

use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{column, container, image, row, scrollable, text},
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
    let font = canopa_ui::font(&appearance);
    let output = Arc::new(Mutex::new(false));
    let result = output.clone();
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(700.0, 600.0),
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
            let (_, open) = window::open_dialog(parent.to_owned(), true, settings);
            (
                Dialog {
                    app_id: input.app_id.clone(),
                    destination: input.destination,
                    dimensions: (input.width, input.height),
                    preview: preview.clone(),
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
    .title(|_: &Dialog, _| "Set wallpaper — Canopa".to_owned())
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
            app_id: "app".into(),
            destination: crate::wallpaper::Destination::Both,
            dimensions: (100, 50),
            preview: image::Handle::from_rgba(1, 1, vec![1, 2, 3, 255]),
            theme: canopa_ui::theme(canopa_theme::Appearance::default()),
            output: Arc::new(Mutex::new(false)),
            completed: false,
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

struct Dialog {
    app_id: String,
    destination: crate::wallpaper::Destination,
    dimensions: (u32, u32),
    preview: image::Handle,
    theme: Theme,
    output: Arc<Mutex<bool>>,
    completed: bool,
}

#[derive(Clone, Debug)]
enum Message {
    Opened,
    Apply,
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
            Message::Apply => {
                *self.output.lock().expect("wallpaper result") = true;
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
        let requester = if self.app_id.is_empty() {
            "The requesting app"
        } else {
            &self.app_id
        };
        let preview = container(
            image(self.preview.clone())
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(300),
        )
        .style(|_| container::Style::default().background(iced::Color::BLACK));
        let mut content = column![
            text("Change wallpaper?").size(22),
            text(format!("Requested by {requester}")).style(canopa_ui::muted),
            preview,
            text(self.destination.label()).size(18),
            text(format!(
                "{} × {} pixels",
                self.dimensions.0, self.dimensions.1
            ))
            .style(canopa_ui::muted)
        ]
        .spacing(14)
        .width(Length::Fill);
        if self.destination != crate::wallpaper::Destination::Background {
            content = content.push(
                text("The lock screen image is used by Canopa’s Lock action.")
                    .style(canopa_ui::muted),
            );
        }
        let actions = row![
            canopa_ui::action(
                text("Cancel"),
                Some(Message::Cancel),
                canopa_ui::Action::Normal,
                false
            )
            .width(Length::Fill),
            canopa_ui::action(
                text("Apply"),
                Some(Message::Apply),
                canopa_ui::Action::Normal,
                false
            )
            .width(Length::Fill)
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
