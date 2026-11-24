use super::Input;
use crate::dialog_ui;
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
                    500.0,
                    if input.sources.is_empty() {
                        360.0
                    } else {
                        420.0
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
    .title(|app: &Controls, _| fl!("portal-window-title", title = app.input.title()))
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
            fl!("portal-an-application")
        } else {
            self.input.requester.clone()
        };
        let mut details = column![].spacing(10);
        if self.input.devices != 0 || self.input.clipboard {
            let mut access =
                column![dialog_ui::section(fl!("portal-sharing-active-permissions")).size(16)]
                    .spacing(5);
            if self.input.devices != 0 {
                access = access.push(text(self.input.control_label()).size(14));
            }
            if self.input.clipboard {
                access = access.push(
                    text(if self.input.capture_input {
                        fl!("portal-sharing-clipboard-capture")
                    } else {
                        fl!("portal-sharing-clipboard")
                    })
                    .size(14),
                );
            }
            details = details.push(
                container(access)
                    .padding([10, 12])
                    .width(Length::Fill)
                    .style(dialog_ui::card),
            );
        }
        if !self.input.sources.is_empty() {
            let mut sources =
                column![dialog_ui::section(fl!("portal-sharing-active-sources")).size(16)]
                    .spacing(5);
            for name in &self.input.sources {
                sources = sources.push(
                    text(name)
                        .size(14)
                        .width(Length::Fill)
                        .wrapping(iced::widget::text::Wrapping::None),
                );
            }
            details = details.push(
                container(sources)
                    .padding([10, 12])
                    .width(Length::Fill)
                    .style(dialog_ui::card),
            );
        }
        let body = column![
            dialog_ui::heading(self.input.heading()),
            text(if self.input.capture_input {
                fl!("portal-sharing-input-to", requester = requester)
            } else if self.input.devices != 0 && !self.input.clipboard {
                fl!("portal-sharing-control-to", requester = requester)
            } else {
                fl!("portal-sharing-with", requester = requester)
            })
            .size(14)
            .width(Length::Fill)
            .height(20)
            .wrapping(iced::widget::text::Wrapping::None),
            scrollable(details)
                .style(canopa_ui::scrollbar)
                .spacing(5)
                .height(Length::Fill),
            text(self.input.hint()).size(14),
        ]
        .spacing(12);
        let actions =
            dialog_ui::footer([dialog_ui::danger(self.input.stop_label(), Message::Stop).into()]);
        dialog_ui::frame(
            container(body)
                .padding([18, 24])
                .width(Length::Fill)
                .height(Length::Fill),
            actions,
        )
    }
}
