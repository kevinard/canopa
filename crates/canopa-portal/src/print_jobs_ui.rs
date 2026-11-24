//! Display-only Iced child. The backend retains policy, job proofs and secrets.
use crate::dialog_ui;
use crate::print::{
    jobs_review::{Command, Configuration, Entry, Response},
    live_jobs::Status,
    worker::Destination,
};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{column, container, pick_list, scrollable, text},
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
                    size: Size::new(680., 320.),
                    min_size: Some(Size::new(550., 290.)),
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
    .title(|_: &Review, _| fl!("portal-window-title", title = fl!("portal-jobs-title")))
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
    OpenPrinterSettings,
    PrinterSettingsFailed,
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
                    .map_err(|_| fl!("portal-jobs-stopped"))?
                    .map_err(|_| fl!("portal-jobs-communication-failed"))
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
            Message::OpenPrinterSettings => {
                self.notice = None;
                let launch = self.runtime.spawn(async {
                    let mut command = tokio::process::Command::new("canopa-settings");
                    command.args(["--page", "printers"]);
                    let mut child = command.spawn()?;
                    child.wait().await
                });
                return Task::future(async move {
                    match launch.await {
                        Ok(Ok(status)) if status.success() => Message::Opened,
                        _ => Message::PrinterSettingsFailed,
                    }
                });
            }
            Message::PrinterSettingsFailed => {
                self.notice = Some(fl!("portal-jobs-settings-failed"));
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
                            fl!("portal-jobs-signed-in")
                        } else if cancelled {
                            fl!("portal-jobs-sign-in-cancelled")
                        } else {
                            fl!("portal-jobs-sign-in-unconfirmed")
                        });
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
                        text(fl!(
                            "portal-jobs-entry",
                            job = entry.job.to_string(),
                            status = status(entry.status)
                        ))
                        .size(13)
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
                dialog_ui::heading(&entry.title),
                text(status(entry.status)),
                text(fl!("portal-jobs-existing-document"))
            ]
            .spacing(16);
            if entry.status == Status::NeedsAuthentication {
                details = details.push(canopa_ui::action(
                    text(fl!("portal-jobs-sign-in")),
                    (!self.busy && entry.authenticate).then_some(Message::Authenticate),
                    canopa_ui::Action::Normal,
                    true,
                ));
                if !entry.authenticate {
                    details = details.push(text(fl!("portal-jobs-identity-unconfirmed")));
                }
            }
            details.into()
        } else {
            let empty_detail = if self.busy
                || self.error.is_some()
                || self.destination.is_none() && !self.printers.is_empty()
            {
                None
            } else if self.printers.is_empty() {
                Some(fl!("portal-jobs-setup-detail"))
            } else if self.entries.is_empty() {
                self.printers
                    .iter()
                    .find(|choice| Some(&choice.destination) == self.destination.as_ref())
                    .map(|choice| fl!("portal-jobs-empty-for", printer = choice.to_string()))
            } else {
                Some(fl!("portal-jobs-held-hint"))
            };
            let mut summary = column![
                dialog_ui::heading(if self.busy {
                    fl!("portal-recovery-checking")
                } else if self.error.is_some() {
                    fl!("portal-jobs-unavailable")
                } else if self.printers.is_empty() {
                    fl!("portal-jobs-no-printers")
                } else if self.destination.is_none() {
                    fl!("portal-jobs-select-printer")
                } else if self.entries.is_empty() {
                    fl!("portal-jobs-empty")
                } else {
                    fl!("portal-jobs-select-job")
                })
                .size(18)
            ]
            .spacing(8);
            if let Some(detail) = empty_detail {
                summary = summary.push(dialog_ui::detail(detail));
            }
            if self.printers.is_empty() && !self.busy && self.error.is_none() {
                summary = summary.push(dialog_ui::action(
                    text(fl!("portal-jobs-open-settings")),
                    Some(Message::OpenPrinterSettings),
                    canopa_ui::Action::Normal,
                    false,
                ));
            }
            summary.into()
        };
        let selected = self
            .printers
            .iter()
            .find(|choice| Some(&choice.destination) == self.destination.as_ref())
            .cloned();
        let details = container(details)
            .padding(16)
            .width(Length::Fill)
            .style(dialog_ui::card);
        let mut content = column![
            canopa_ui::direction::row([
                dialog_ui::heading(fl!("portal-jobs-title"))
                    .width(Length::Fill)
                    .into(),
                dialog_ui::action(
                    text(fl!("portal-print-refresh")),
                    (!self.busy).then_some(Message::Refresh),
                    canopa_ui::Action::Normal,
                    false,
                )
                .into(),
            ])
            .align_y(iced::Center)
            .spacing(10),
        ]
        .spacing(14);
        if !self.printers.is_empty() {
            content = content.push(
                column![
                    dialog_ui::section(fl!("portal-jobs-printer")),
                    pick_list(self.printers.clone(), selected, Message::Destination)
                        .placeholder(fl!("portal-jobs-select-printer"))
                        .width(Length::Fill),
                ]
                .spacing(5),
            );
        }
        let details = scrollable(details)
            .style(canopa_ui::scrollbar)
            .height(Length::Fill);
        let body: Element<'_, Message> = if self.entries.is_empty() {
            details.width(Length::Fill).into()
        } else {
            canopa_ui::direction::row([
                scrollable(entries)
                    .style(canopa_ui::scrollbar)
                    .spacing(6)
                    .width(Length::FillPortion(2))
                    .into(),
                details.width(Length::FillPortion(3)).into(),
            ])
            .spacing(16)
            .height(Length::Fill)
            .into()
        };
        content = content.push(body);
        if let Some(notice) = &self.notice {
            content = content.push(text(notice));
        }
        if let Some(error) = &self.error {
            content = content.push(text(error));
        }
        let actions = dialog_ui::footer([dialog_ui::action(
            text(fl!("portal-close")),
            Some(Message::Close),
            canopa_ui::Action::Normal,
            false,
        )
        .into()]);
        dialog_ui::frame(
            container(content)
                .padding([20, 24])
                .width(Length::Fill)
                .height(Length::Fill),
            actions,
        )
    }
}
fn status(status: Status) -> String {
    match status {
        Status::Receiving => fl!("portal-jobs-status-receiving"),
        Status::Queued => fl!("portal-jobs-status-queued"),
        Status::Held => fl!("portal-jobs-status-held"),
        Status::NeedsAuthentication => fl!("portal-jobs-status-needs-sign-in"),
        Status::Processing => fl!("portal-jobs-status-printing"),
        Status::Stopped => fl!("portal-jobs-status-stopped"),
        Status::Cancelled => fl!("portal-jobs-status-cancelled"),
        Status::Aborted => fl!("portal-jobs-status-aborted"),
        Status::Completed => fl!("portal-jobs-status-completed"),
    }
}
