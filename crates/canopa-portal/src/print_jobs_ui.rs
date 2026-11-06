//! Display-only Iced child. The backend retains policy, job proofs and secrets.
use crate::print::{
    jobs_review::{Command, Configuration, Entry, Response},
    live_jobs::Status,
    worker::Destination,
};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{column, container, pick_list, row, scrollable, text},
    window,
};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    runtime::Handle,
    sync::Mutex,
};

#[derive(Clone, Debug, PartialEq)]
struct Choice {
    destination: Destination,
    label: String,
}
impl std::fmt::Display for Choice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}
pub fn run(
    configuration: Configuration,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        configuration.context.parent_window.len() <= 4096
            && !configuration.context.parent_window.contains('\0'),
        "Invalid printer review parent"
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;
    let channel = crate::print::file_target::inherited_channel(configuration.channel)?;
    channel.set_nonblocking(true)?;
    let channel = {
        let _entered = runtime.enter();
        Arc::new(Mutex::new(tokio::net::UnixStream::from_std(channel)?))
    };
    let theme = canopa_ui::theme(appearance.clone());
    let handle = runtime.handle().clone();
    let result = iced::daemon(
        move || {
            let (_, open) = window::open_dialog(
                configuration
                    .context
                    .parent_window
                    .strip_prefix("wayland:")
                    .unwrap_or_default()
                    .to_owned(),
                false,
                window::Settings {
                    size: Size::new(860., 620.),
                    min_size: Some(Size::new(580., 440.)),
                    exit_on_close_request: false,
                    platform_specific: window::settings::PlatformSpecific {
                        application_id: "org.canopa.Portal".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
            let mut app = Review {
                theme: theme.clone(),
                runtime: handle.clone(),
                channel: channel.clone(),
                printers: vec![],
                destination: None,
                entries: vec![],
                selected: None,
                busy: false,
                generation: 0,
                notice: None,
                error: None,
                closed: false,
            };
            let list = app.request(Command::List { destination: None });
            (app, Task::batch([open.map(|_| Message::Opened), list]))
        },
        Review::update,
        Review::view,
    )
    .title(|_: &Review, _| "Printer jobs — Canopa".to_owned())
    .theme(|app: &Review, _| app.theme.clone())
    .subscription(Review::subscription)
    .default_font(canopa_ui::font(&appearance))
    .run();
    runtime.shutdown_timeout(Duration::from_secs(1));
    result.map_err(|error| anyhow::anyhow!(error.to_string()))
}
struct Review {
    theme: Theme,
    runtime: Handle,
    channel: Arc<Mutex<tokio::net::UnixStream>>,
    printers: Vec<Choice>,
    destination: Option<Destination>,
    entries: Vec<Entry>,
    selected: Option<String>,
    busy: bool,
    generation: u64,
    notice: Option<String>,
    error: Option<String>,
    closed: bool,
}
#[derive(Clone, Debug)]
enum Message {
    Opened,
    Refresh,
    Destination(Choice),
    Select(String),
    Authenticate,
    Replied(u64, Result<Response, String>),
    Close,
    Event(Event),
}
impl Review {
    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen().map(Message::Event)
    }
    fn request(&mut self, command: Command) -> Task<Message> {
        self.busy = true;
        self.error = None;
        self.generation += 1;
        let generation = self.generation;
        let channel = self.channel.clone();
        let task = self.runtime.spawn(async move {
            let mut channel = channel.lock().await;
            let mut bytes = serde_json::to_vec(&command)?;
            anyhow::ensure!(bytes.len() < 4096, "Printer review command exceeds limits");
            bytes.push(b'\n');
            channel.write_all(&bytes).await?;
            let mut reader = BufReader::new(&mut *channel);
            let mut bytes = Vec::new();
            (&mut reader)
                .take(crate::process::MAX_MESSAGE_BYTES as u64 + 1)
                .read_until(b'\n', &mut bytes)
                .await?;
            anyhow::ensure!(
                bytes.len() <= crate::process::MAX_MESSAGE_BYTES && bytes.last() == Some(&b'\n'),
                "Printer review response unavailable"
            );
            Ok::<_, anyhow::Error>(serde_json::from_slice::<Response>(&bytes)?)
        });
        Task::perform(
            async move {
                task.await
                    .map_err(|_| "Printer review stopped".to_owned())?
                    .map_err(|_| "Couldn’t communicate with the printer review service.".to_owned())
            },
            move |response| Message::Replied(generation, response),
        )
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.closed {
            return Task::none();
        }
        match message {
            Message::Close
            | Message::Event(Event::Window(window::Event::CloseRequested))
            | Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            })) => {
                self.closed = true;
                return iced::exit();
            }
            Message::Refresh if !self.busy => {
                self.selected = None;
                self.entries.clear();
                return self.request(Command::List {
                    destination: self.destination.clone(),
                });
            }
            Message::Destination(choice) if !self.busy => {
                self.destination = Some(choice.destination);
                self.entries.clear();
                self.selected = None;
                self.notice = None;
                return self.request(Command::List {
                    destination: self.destination.clone(),
                });
            }
            Message::Select(key)
                if !self.busy && self.entries.iter().any(|entry| entry.key == key) =>
            {
                self.selected = Some(key);
            }
            Message::Authenticate if !self.busy => {
                if let Some(entry) = self
                    .entries
                    .iter()
                    .find(|entry| Some(&entry.key) == self.selected.as_ref() && entry.authenticate)
                {
                    let key = entry.key.clone();
                    self.notice = None;
                    return self.request(Command::Authenticate { key });
                }
            }
            Message::Replied(generation, response) if generation == self.generation => {
                self.busy = false;
                match response {
                    Ok(Response::Listing {
                        printers,
                        destination,
                        entries,
                    }) => {
                        self.printers = printers
                            .into_iter()
                            .map(|printer| Choice {
                                label: format!(
                                    "{} — {}",
                                    printer.label,
                                    printer.destination.name()
                                ),
                                destination: printer.destination,
                            })
                            .collect();
                        self.destination = destination;
                        self.selected = self
                            .selected
                            .take()
                            .filter(|key| entries.iter().any(|entry| &entry.key == key));
                        self.entries = entries;
                    }
                    Ok(Response::SignedIn {
                        acknowledged,
                        cancelled,
                    }) => {
                        self.notice = Some(if acknowledged {
                            "Sign-in was sent to the printer service. Refresh to check the job; printing may still be in progress."
                        } else if cancelled { "Sign-in canceled." }
                        else { "Sign-in couldn’t be confirmed. Refresh and check the job before trying again." }.into());
                        self.entries.clear();
                        self.selected = None;
                        return self.request(Command::List {
                            destination: self.destination.clone(),
                        });
                    }
                    Ok(Response::Failed { message }) | Err(message) => {
                        self.entries.clear();
                        self.selected = None;
                        self.error = Some(message);
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let mut entries = column![].spacing(8).width(Length::Fill);
        for entry in &self.entries {
            entries = entries.push(
                canopa_ui::action(
                    column![
                        text(&entry.title).size(16),
                        text(format!("Job {} · {}", entry.job, status(entry.status))).size(13)
                    ]
                    .spacing(5),
                    (!self.busy).then(|| Message::Select(entry.key.clone())),
                    canopa_ui::Action::Normal,
                    self.selected.as_ref() == Some(&entry.key),
                )
                .width(Length::Fill),
            );
        }
        let details: Element<'_, Message> = if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| Some(&entry.key) == self.selected.as_ref())
        {
            let mut details = column![
                text(&entry.title).size(22),
                text(status(entry.status)),
                text(
                    "The printer service already has this job. Sign-in uses its existing document."
                )
            ]
            .spacing(16);
            if entry.status == Status::NeedsAuthentication {
                details = details.push(canopa_ui::action(
                    text("Sign in…"),
                    (!self.busy && entry.authenticate).then_some(Message::Authenticate),
                    canopa_ui::Action::Normal,
                    true,
                ));
                if !entry.authenticate {
                    details = details.push(text("This job’s identity or sign-in method couldn’t be confirmed. Refresh to check again."));
                }
            }
            details.into()
        } else {
            column![
                text(if self.busy {
                    "Checking printer jobs…"
                } else if self.error.is_some() {
                    "Printer jobs unavailable"
                } else if self.destination.is_none() && !self.printers.is_empty() {
                    "Select a printer"
                } else if self.entries.is_empty() {
                    "No active printer jobs"
                } else {
                    "Select a printer job"
                })
                .size(22),
                text("Jobs held for sign-in can continue using the document already in the queue.")
            ]
            .spacing(16)
            .into()
        };
        let selected = self
            .printers
            .iter()
            .find(|choice| Some(&choice.destination) == self.destination.as_ref())
            .cloned();
        let mut content = column![
            row![
                text("Printer jobs").size(28).width(Length::Fill),
                canopa_ui::action(
                    text("Refresh"),
                    (!self.busy).then_some(Message::Refresh),
                    canopa_ui::Action::Normal,
                    false
                )
            ]
            .spacing(10),
            pick_list(self.printers.clone(), selected, Message::Destination).width(Length::Fill),
            row![
                scrollable(entries).spacing(6).width(Length::FillPortion(2)),
                scrollable(details).spacing(6).width(Length::FillPortion(3))
            ]
            .spacing(22)
            .height(Length::Fill)
        ]
        .spacing(18);
        if let Some(notice) = &self.notice {
            content = content.push(text(notice));
        }
        if let Some(error) = &self.error {
            content = content.push(text(error));
        }
        content = content.push(canopa_ui::action(
            text("Close"),
            Some(Message::Close),
            canopa_ui::Action::Normal,
            false,
        ));
        container(content)
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
fn status(status: Status) -> &'static str {
    match status {
        Status::Receiving => "Receiving document",
        Status::Queued => "Waiting to print",
        Status::Held => "On hold",
        Status::NeedsAuthentication => "Sign-in needed",
        Status::Processing => "Printing",
        Status::Stopped => "Stopped",
        Status::Cancelled => "Canceled",
        Status::Aborted => "Couldn’t print",
        Status::Completed => "Completed by printer service",
    }
}
