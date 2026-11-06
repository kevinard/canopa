use super::Input;
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{column, container, scrollable, text},
    window,
};
use std::io::Write;
use std::sync::Mutex;

pub(super) fn run(
    input: Input,
    appearance: canopa_theme::Appearance,
    mut lease: impl std::io::BufRead + Send + 'static,
) -> anyhow::Result<()> {
    let (gone, lost) = futures_channel::oneshot::channel();
    std::thread::Builder::new()
        .name("sharing-control-lease".into())
        .spawn(move || {
            let _ = lease.fill_buf(); // EOF or any unexpected command retires the surface.
            let _ = gone.send(());
        })?;
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    let lost = Mutex::new(Some(lost));
    iced::daemon(
        move || {
            let (_, opening) = window::open(window::Settings {
                size: Size::new(
                    480.0,
                    if input.capture_input && input.clipboard {
                        340.0
                    } else {
                        280.0
                    },
                ),
                resizable: false,
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            });
            (
                Controls {
                    input: input.clone(),
                    theme: theme.clone(),
                    ready: false,
                    completed: false,
                },
                Task::batch([
                    opening.map(|_| Message::Opened),
                    Task::perform(
                        lost.lock()
                            .expect("sharing lease")
                            .take()
                            .expect("sharing lease receiver"),
                        |_| Message::Stop,
                    ),
                ]),
            )
        },
        Controls::update,
        Controls::view,
    )
    .title(|app: &Controls, _| format!("{} — Canopa", app.input.title()))
    .theme(|app: &Controls, _| app.theme.clone())
    .subscription(Controls::subscription)
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}

#[derive(Clone, Debug)]
enum Message {
    Opened,
    Stop,
    Event(Event),
}
struct Controls {
    input: Input,
    theme: Theme,
    ready: bool,
    completed: bool,
}
impl Controls {
    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen().map(Message::Event)
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Opened if !self.ready => {
                let mut output = std::io::stdout().lock();
                if output
                    .write_all(b"{\"ready\":true}\n")
                    .and_then(|_| output.flush())
                    .is_err()
                {
                    self.completed = true;
                    return iced::exit();
                }
                self.ready = true;
            }
            Message::Stop
            | Message::Event(Event::Window(window::Event::CloseRequested))
            | Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            })) => {
                self.completed = true;
                return iced::exit();
            }
            _ => {}
        }
        Task::none()
    }
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let requester = if self.input.requester.is_empty() {
            "An application"
        } else {
            &self.input.requester
        };
        let sources = self
            .input
            .sources
            .iter()
            .fold(column![].spacing(6), |items, name| {
                items.push(
                    text(name)
                        .size(14)
                        .width(Length::Fill)
                        .height(22)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
            });
        let sources = if self.input.clipboard {
            sources.push(
                text(if self.input.capture_input {
                    "Clipboard · Only while input is captured"
                } else {
                    "Clipboard · Read and replace copied content"
                })
                .size(14),
            )
        } else {
            sources
        };
        let sources = if self.input.devices != 0 {
            column![text(self.input.control_label()).size(14), sources].spacing(6)
        } else {
            sources
        };
        container(
            column![
                text(self.input.heading()).size(22),
                text(if self.input.capture_input {
                    format!("Input may be sent to {requester}")
                } else if self.input.devices != 0 && !self.input.clipboard {
                    format!("Control granted to {requester}")
                } else {
                    format!("Sharing with {requester}")
                })
                .size(14)
                .width(Length::Fill)
                .height(20)
                .wrapping(iced::widget::text::Wrapping::None)
                .style(canopa_ui::muted),
                scrollable(sources).height(Length::Fill),
                text(self.input.hint()).size(12).style(canopa_ui::muted),
                canopa_ui::action(
                    text(self.input.stop_label()),
                    Some(Message::Stop),
                    canopa_ui::Action::Destructive,
                    false
                )
                .width(Length::Fill)
            ]
            .spacing(12),
        )
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(canopa_ui::surface)
        .into()
    }
}
