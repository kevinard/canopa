//! User-owned recovery review. Retained paths never become write capabilities.
#[cfg(test)]
mod tests;
mod view;
use crate::{
    chooser_ui::{self, Outcome, Picker, PickerOptions, PickerStart},
    file_chooser::Mode,
    print::{
        file_target::{
            Pinned, RecoveryBody, RecoveryItem, RecoveryStatus, RecoveryStore, Reviewed,
        },
        worker::{
            Client, PrinterRecoveryItem, PrinterRecoveryReport, PrinterRecoveryStatus, Recovered,
        },
    },
    process::DialogProcess,
};
use iced::{Element, Event, Size, Subscription, Task, Theme, keyboard, window};
use std::{
    os::unix::{ffi::OsStringExt, net::UnixStream},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::{runtime::Handle, sync::watch};

pub fn run(
    store: RecoveryStore,
    appearance: canopa_theme::Appearance,
    process: DialogProcess,
) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;
    let client = Client::new(process).with_recovery(store);
    let reaper = client.clone();
    let handle = runtime.handle().clone();
    let (cancel, cancelled) = watch::channel(false);
    let font = canopa_ui::font(&appearance);
    let result = iced::daemon(
        move || {
            let (_, open) = window::open(window::Settings {
                size: Size::new(680., 300.),
                min_size: Some(Size::new(550., 290.)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            });
            let mut app = Recovery::new(
                appearance.clone(),
                client.clone(),
                handle.clone(),
                cancelled.clone(),
            );
            let list = app.refresh();
            (app, Task::batch([open.map(|_| Message::Opened), list]))
        },
        Recovery::update,
        Recovery::view,
    )
    .title(|_: &Recovery, _| fl!("portal-window-title", title = fl!("portal-recovery-title")))
    .theme(|app: &Recovery, _| app.theme.clone())
    .subscription(Recovery::subscription)
    .default_font(font)
    .run();
    cancel.send_replace(true);
    runtime.block_on(reaper.shutdown());
    runtime.shutdown_timeout(Duration::from_secs(1));
    result.map_err(|error| anyhow::anyhow!(error.to_string()))
}

enum Screen {
    List,
    Preparing,
    Saving,
    Picker(Box<Picker>),
    Dismiss,
    PrinterDismiss,
    CheckingPrinters,
    RemovingPrinter,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tab {
    Files,
    Printers,
}

#[derive(Clone, Debug)]
struct Listing {
    files: Result<Vec<RecoveryItem>, String>,
    printers: Result<Vec<PrinterRecoveryItem>, String>,
}
struct Running(tokio::task::AbortHandle);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct Recovery {
    appearance: canopa_theme::Appearance,
    theme: Theme,
    client: Client,
    runtime: Handle,
    cancelled: watch::Receiver<bool>,
    items: Vec<RecoveryItem>,
    selected: Option<String>,
    printers: Vec<PrinterRecoveryItem>,
    selected_printer: Option<String>,
    tab: Tab,
    loaded: bool,
    file_load_error: Option<String>,
    printer_load_error: Option<String>,
    screen: Screen,
    generation: u64,
    busy: bool,
    closed: bool,
    running: Option<Running>,
    recovered: Option<Recovered>,
    error: Option<String>,
    notice: Option<String>,
}

#[derive(Clone, Debug)]
enum Message {
    Opened,
    Refresh,
    Select(String),
    Save(RecoveryBody),
    AskDismiss,
    Keep,
    Dismiss,
    Listed(u64, Result<Vec<RecoveryItem>, String>),
    Refreshed(u64, Listing),
    Tab(Tab),
    SelectPrinter(String),
    AskPrinterDismiss,
    DismissPrinter,
    PrinterDismissed(u64, Result<(), String>),
    CheckPrinters,
    PrintersChecked(u64, Result<PrinterRecoveryReport, String>),
    Snapshot(u64, RecoveryBody, Result<Recovered, String>),
    PickerReady(u64, Result<Box<PickerStart>, String>),
    Picker(u64, chooser_ui::Message),
    Saved(u64, Vec<u8>, Result<(), String>),
    Dismissed(u64, Result<(), String>),
    OpenFolder,
    FolderOpened(u64, Result<(), String>),
    Back,
    Close,
    Event(Event),
}

impl Recovery {
    fn new(
        appearance: canopa_theme::Appearance,
        client: Client,
        runtime: Handle,
        cancelled: watch::Receiver<bool>,
    ) -> Self {
        Self {
            theme: canopa_ui::theme(appearance.clone()),
            appearance,
            client,
            runtime,
            cancelled,
            items: vec![],
            selected: None,
            printers: vec![],
            selected_printer: None,
            tab: Tab::Files,
            loaded: false,
            file_load_error: None,
            printer_load_error: None,
            screen: Screen::List,
            generation: 0,
            busy: false,
            closed: false,
            running: None,
            recovered: None,
            error: None,
            notice: None,
        }
    }
    fn item(&self) -> Option<&RecoveryItem> {
        if self.tab != Tab::Files {
            return None;
        }
        self.items
            .iter()
            .find(|item| Some(&item.id) == self.selected.as_ref())
    }
    fn printer(&self) -> Option<&PrinterRecoveryItem> {
        if self.tab != Tab::Printers {
            return None;
        }
        self.printers
            .iter()
            .find(|item| Some(&item.id) == self.selected_printer.as_ref())
    }
    fn editable(&self) -> bool {
        !self.closed && !self.busy && matches!(self.screen, Screen::List)
    }
    fn begin(&mut self) -> u64 {
        self.running = None;
        self.generation += 1;
        self.busy = true;
        self.error = None;
        self.generation
    }
    fn refresh(&mut self) -> Task<Message> {
        let generation = self.begin();
        let (client, cancelled) = (self.client.clone(), self.cancelled.clone());
        let task = self.runtime.spawn(async move {
            let result = async {
                client.idle().await?;
                let files = client
                    .recovery(cancelled.clone())
                    .await
                    .and_then(|items| {
                        items.ok_or_else(|| anyhow::anyhow!("Recovery query cancelled"))
                    })
                    .map_err(|error| error.to_string());
                let printers = client
                    .printer_recovery(cancelled)
                    .await
                    .and_then(|items| {
                        items.ok_or_else(|| anyhow::anyhow!("Printer query cancelled"))
                    })
                    .map_err(|error| error.to_string());
                Ok::<_, anyhow::Error>(Listing { files, printers })
            }
            .await;
            result.map_err(|error| error.to_string())
        });
        self.running = Some(Running(task.abort_handle()));
        Task::perform(
            async move {
                match task
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result)
                {
                    Ok(listing) => listing,
                    Err(error) => Listing {
                        files: Err(error.clone()),
                        printers: Err(error),
                    },
                }
            },
            move |result| Message::Refreshed(generation, result),
        )
    }
    fn back(&mut self) -> Task<Message> {
        self.running = None;
        self.generation += 1;
        self.busy = false;
        self.recovered = None;
        self.screen = Screen::List;
        Task::none()
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.closed {
            return Task::none();
        }
        match message {
            Message::Refresh if self.editable() => return self.refresh(),
            Message::Tab(tab) if self.editable() => {
                self.tab = tab;
                self.error = None;
                self.notice = None;
            }
            Message::Refreshed(generation, listing)
                if generation == self.generation && self.busy =>
            {
                let files_failed = listing.files.is_err();
                let _ = self.update(Message::Listed(generation, listing.files));
                self.file_load_error = if files_failed {
                    self.error.take()
                } else {
                    None
                };
                match listing.printers {
                    Ok(items) => {
                        self.printer_load_error = None;
                        self.printers = items;
                        if !self
                            .printers
                            .iter()
                            .any(|item| Some(&item.id) == self.selected_printer.as_ref())
                        {
                            self.selected_printer =
                                self.printers.first().map(|item| item.id.clone());
                        }
                    }
                    Err(error) => {
                        eprintln!(
                            "level=warn component=print_recovery event=printer_list_failed error={error}"
                        );
                        for item in &mut self.printers {
                            item.status = PrinterRecoveryStatus::Unavailable;
                        }
                        self.printer_load_error = Some(fl!("portal-recovery-printers-failed"));
                    }
                }
                if !self.loaded && self.items.is_empty() && !self.printers.is_empty() {
                    self.tab = Tab::Printers;
                }
                self.loaded = true;
            }
            Message::Select(id)
                if self.editable()
                    && self.tab == Tab::Files
                    && self.items.iter().any(|item| item.id == id) =>
            {
                self.selected = Some(id);
                self.error = None;
                self.notice = None;
            }
            Message::SelectPrinter(id)
                if self.editable()
                    && self.tab == Tab::Printers
                    && self.printers.iter().any(|item| item.id == id) =>
            {
                self.selected_printer = Some(id);
                self.error = None;
                self.notice = None;
            }
            Message::AskPrinterDismiss
                if self.editable()
                    && self.printer().is_some_and(PrinterRecoveryItem::dismissible) =>
            {
                self.screen = Screen::PrinterDismiss;
            }
            Message::DismissPrinter
                if !self.busy
                    && matches!(self.screen, Screen::PrinterDismiss)
                    && self.printer().is_some_and(PrinterRecoveryItem::dismissible) =>
            {
                let item = self.printer().unwrap().clone();
                let generation = self.begin();
                self.screen = Screen::RemovingPrinter;
                let (client, cancelled) = (self.client.clone(), self.cancelled.clone());
                let task = self.runtime.spawn(async move {
                    client.idle().await?;
                    client
                        .dismiss_printer_recovery(item, cancelled)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("Printer removal cancelled"))
                });
                self.running = Some(Running(task.abort_handle()));
                return Task::perform(
                    async move {
                        task.await
                            .map_err(|error| error.to_string())
                            .and_then(|result| result.map_err(|error| error.to_string()))
                    },
                    move |result| Message::PrinterDismissed(generation, result),
                );
            }
            Message::PrinterDismissed(generation, result)
                if generation == self.generation
                    && matches!(self.screen, Screen::RemovingPrinter) =>
            {
                let _ = self.back();
                match result {
                    Ok(()) => {
                        self.notice = Some(fl!("portal-recovery-record-removed"));
                        return self.refresh();
                    }
                    Err(_) => {
                        self.error = Some(fl!("portal-recovery-record-removal-unconfirmed"));
                    }
                }
            }
            Message::CheckPrinters if self.editable() && self.tab == Tab::Printers => {
                let generation = self.begin();
                self.screen = Screen::CheckingPrinters;
                let (client, cancelled) = (self.client.clone(), self.cancelled.clone());
                let task = self.runtime.spawn(async move {
                    client.idle().await?;
                    client
                        .review_printer_jobs(cancelled)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("Printer check cancelled"))
                });
                self.running = Some(Running(task.abort_handle()));
                return Task::perform(
                    async move {
                        task.await
                            .map_err(|error| error.to_string())
                            .and_then(|result| result.map_err(|error| error.to_string()))
                    },
                    move |result| Message::PrintersChecked(generation, result),
                );
            }
            Message::PrintersChecked(generation, result)
                if generation == self.generation
                    && matches!(self.screen, Screen::CheckingPrinters) =>
            {
                let _ = self.back();
                self.notice = Some(match result {
                    Ok(report) => fl!(
                        "portal-recovery-cleanup-report",
                        cleared = report.retired,
                        unconfirmed = report.unresolved,
                        busy = report.busy
                    ),
                    Err(_) => fl!("portal-recovery-cleanup-unconfirmed"),
                });
                return self.refresh();
            }
            Message::Listed(generation, result) if generation == self.generation && self.busy => {
                self.running = None;
                self.busy = false;
                match result {
                    Ok(items) => {
                        self.items = items;
                        if !self
                            .items
                            .iter()
                            .any(|item| Some(&item.id) == self.selected.as_ref())
                        {
                            self.selected = self.items.first().map(|item| item.id.clone());
                        }
                    }
                    Err(error) => {
                        eprintln!(
                            "level=warn component=print_recovery event=list_failed error={error}"
                        );
                        for item in &mut self.items {
                            item.status = RecoveryStatus::Unavailable;
                            item.document = false;
                            item.displaced = false;
                        }
                        self.error = Some(fl!("portal-recovery-files-failed"));
                    }
                }
            }
            Message::Save(body)
                if self.editable() && self.item().is_some_and(|item| available(item, body)) =>
            {
                let id = self.item().unwrap().id.clone();
                let generation = self.begin();
                self.notice = None;
                self.screen = Screen::Preparing;
                let (client, cancelled) = (self.client.clone(), self.cancelled.clone());
                let task = self.runtime.spawn(async move {
                    client.idle().await?;
                    client
                        .recover_snapshot(id, body, cancelled)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("Recovery preparation cancelled"))
                });
                self.running = Some(Running(task.abort_handle()));
                return Task::perform(
                    async move {
                        task.await
                            .map_err(|error| error.to_string())
                            .and_then(|result| result.map_err(|error| error.to_string()))
                    },
                    move |result| Message::Snapshot(generation, body, result),
                );
            }
            Message::Snapshot(generation, body, result)
                if generation == self.generation && matches!(self.screen, Screen::Preparing) =>
            {
                self.running = None;
                match result {
                    Ok(snapshot) => {
                        self.recovered = Some(snapshot);
                        let Some(item) = self.item() else {
                            return self.back();
                        };
                        let mut request = crate::file_chooser::parse_request(
                            Mode::Save,
                            String::new(),
                            String::new(),
                            fl!("portal-recovery-save-title"),
                            Default::default(),
                        )
                        .expect("bounded recovery picker");
                        request.accept_label = fl!("portal-recovery-save-copy");
                        request.current_name = Some(suggested(item, body));
                        let appearance = self.appearance.clone();
                        return Task::perform(
                            crate::worker::run(move || {
                                chooser_ui::prepare_picker(request, appearance)
                            }),
                            move |result| {
                                Message::PickerReady(
                                    generation,
                                    result.and_then(|result| {
                                        result.map(Box::new).map_err(|error| error.to_string())
                                    }),
                                )
                            },
                        );
                    }
                    Err(error) => {
                        eprintln!(
                            "level=warn component=print_recovery event=snapshot_failed error={error}"
                        );
                        if let Some(item) = self
                            .items
                            .iter_mut()
                            .find(|item| Some(&item.id) == self.selected.as_ref())
                        {
                            match body {
                                RecoveryBody::Document => item.document = false,
                                RecoveryBody::Displaced => item.displaced = false,
                            }
                        }
                        self.error = Some(fl!("portal-recovery-verify-failed"));
                        return self.back();
                    }
                }
            }
            Message::PickerReady(generation, result)
                if generation == self.generation && matches!(self.screen, Screen::Preparing) =>
            {
                match result {
                    Ok(start) => {
                        let (picker, task) = Picker::new(
                            *start,
                            PickerOptions {
                                read_only_choice: false,
                                retain_file_target: true,
                            },
                        );
                        self.screen = Screen::Picker(Box::new(picker));
                        return task.map(move |message| Message::Picker(generation, message));
                    }
                    Err(_) => {
                        self.error = Some(fl!("portal-print-picker-failed"));
                        return self.back();
                    }
                }
            }
            Message::Picker(generation, message) if generation == self.generation => {
                let Screen::Picker(picker) = &mut self.screen else {
                    return Task::none();
                };
                if matches!(
                    &message,
                    chooser_ui::Message::Event(Event::Window(window::Event::CloseRequested))
                ) {
                    return self.update(Message::Close);
                }
                let task = picker.update(message);
                match picker.take_outcome() {
                    Some(Outcome::Cancelled) => return self.back(),
                    Some(Outcome::Selected(_)) => {
                        let reviewed = picker.take_file_target();
                        let Some(recovered) = self.recovered.take() else {
                            return self.back();
                        };
                        let Some(reviewed) = reviewed else {
                            return self.back();
                        };
                        let path = reviewed.selection().path().to_vec();
                        let pin = match handoff(&reviewed) {
                            Ok(pin) => pin,
                            Err(_) => {
                                self.error = Some(fl!("portal-recovery-choose-again"));
                                return self.back();
                            }
                        };
                        if !recovered.allows_destination(pin.selection()) {
                            self.error = Some(fl!("portal-recovery-outside-folder"));
                            return self.back();
                        }
                        self.screen = Screen::Saving;
                        let (client, cancelled) = (self.client.clone(), self.cancelled.clone());
                        let task = self.runtime.spawn(async move {
                            client.idle().await?;
                            client
                                .publish_recovered(&recovered, pin, cancelled)
                                .await?
                                .filter(|result| result.published)
                                .map(|_| ())
                                .ok_or_else(|| anyhow::anyhow!("Save was not confirmed"))
                        });
                        self.running = Some(Running(task.abort_handle()));
                        return Task::perform(
                            async move {
                                task.await
                                    .map_err(|error| error.to_string())
                                    .and_then(|result| result.map_err(|error| error.to_string()))
                            },
                            move |result| Message::Saved(generation, path.clone(), result),
                        );
                    }
                    None => return task.map(move |message| Message::Picker(generation, message)),
                }
            }
            Message::Saved(generation, path, result)
                if generation == self.generation && matches!(self.screen, Screen::Saving) =>
            {
                let _ = self.back();
                match result {
                    Ok(()) => {
                        self.notice = Some(fl!("portal-recovery-saved", path = label(&path)));
                        return self.refresh();
                    }
                    Err(error) => {
                        eprintln!(
                            "level=warn component=print_recovery event=save_unconfirmed error={error}"
                        );
                        self.error =
                            Some(fl!("portal-recovery-save-unconfirmed", path = label(&path)));
                    }
                }
            }
            Message::AskDismiss if self.editable() && self.item().is_some_and(dismissible) => {
                self.screen = Screen::Dismiss;
            }
            Message::Keep if matches!(self.screen, Screen::Dismiss | Screen::PrinterDismiss) => {
                return self.back();
            }
            Message::Dismiss
                if matches!(self.screen, Screen::Dismiss)
                    && self.item().is_some_and(dismissible) =>
            {
                let id = self.item().unwrap().id.clone();
                let generation = self.begin();
                self.screen = Screen::Preparing;
                let (client, cancelled) = (self.client.clone(), self.cancelled.clone());
                let task = self.runtime.spawn(async move {
                    client.idle().await?;
                    client
                        .dismiss_recovery(id, cancelled)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("Dismissal cancelled"))
                });
                self.running = Some(Running(task.abort_handle()));
                return Task::perform(
                    async move {
                        task.await
                            .map_err(|error| error.to_string())
                            .and_then(|result| result.map_err(|error| error.to_string()))
                    },
                    move |result| Message::Dismissed(generation, result),
                );
            }
            Message::Dismissed(generation, result) if generation == self.generation => {
                let _ = self.back();
                match result {
                    Ok(()) => {
                        self.notice = Some(fl!("portal-recovery-removed"));
                        return self.refresh();
                    }
                    Err(_) => {
                        self.error = Some(fl!("portal-recovery-removal-unconfirmed"));
                    }
                }
            }
            Message::OpenFolder if self.editable() && self.item().is_some() => {
                let path = self.item().unwrap().directory.clone();
                let generation = self.begin();
                let task = self.runtime.spawn(async move {
                    open_folder(path).await.map_err(|error| error.to_string())
                });
                self.running = Some(Running(task.abort_handle()));
                return Task::perform(
                    async move { task.await.unwrap_or_else(|error| Err(error.to_string())) },
                    move |result| Message::FolderOpened(generation, result),
                );
            }
            Message::FolderOpened(generation, result) if generation == self.generation => {
                self.running = None;
                self.busy = false;
                if result.is_err() {
                    self.error = Some(fl!("portal-recovery-open-folder-failed"));
                }
            }
            Message::Back
                if !matches!(
                    self.screen,
                    Screen::Saving | Screen::CheckingPrinters | Screen::RemovingPrinter
                ) =>
            {
                return self.back();
            }
            Message::Close | Message::Event(Event::Window(window::Event::CloseRequested)) => {
                self.closed = true;
                self.running = None;
                return iced::exit();
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => {
                if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    return self.update(if matches!(self.screen, Screen::List) {
                        Message::Close
                    } else {
                        Message::Back
                    });
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
    fn subscription(&self) -> Subscription<Message> {
        if let Screen::Picker(picker) = &self.screen {
            return picker
                .subscription()
                .with(self.generation)
                .map(|(generation, message)| Message::Picker(generation, message));
        }
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
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        view::view(self)
    }
}

fn available(item: &RecoveryItem, body: RecoveryBody) -> bool {
    !matches!(
        item.status,
        RecoveryStatus::Busy | RecoveryStatus::Unavailable | RecoveryStatus::Missing
    ) && match body {
        RecoveryBody::Document => item.document,
        RecoveryBody::Displaced => item.displaced,
    }
}
fn dismissible(item: &RecoveryItem) -> bool {
    // The catalog worker rechecks liveness under its exclusive lock. This lets
    // the user dismiss changed/missing files without trusting display status.
    item.status != RecoveryStatus::Busy
}
fn label(path: &[u8]) -> String {
    PathBuf::from(std::ffi::OsString::from_vec(path.to_vec()))
        .display()
        .to_string()
}
fn suggested(item: &RecoveryItem, body: RecoveryBody) -> String {
    let path = PathBuf::from(std::ffi::OsString::from_vec(item.destination.clone()));
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let prefix = if body == RecoveryBody::Document {
        "Recovered"
    } else {
        "Displaced"
    };
    format!(
        "{prefix}-{}",
        name.chars()
            .map(|c| if c.is_control() { '_' } else { c })
            .take(180)
            .collect::<String>()
    )
}
fn handoff(reviewed: &Arc<Reviewed>) -> std::io::Result<Pinned> {
    let (send, receive) = UnixStream::pair()?;
    receive.set_nonblocking(true)?;
    reviewed.send(&send)?;
    drop(send);
    Pinned::receive_handoff(&receive, reviewed.selection().clone())
}
async fn open_folder(path: Vec<u8>) -> anyhow::Result<()> {
    let path = PathBuf::from(std::ffi::OsString::from_vec(path));
    let uri = url::Url::from_directory_path(path)
        .map_err(|_| anyhow::anyhow!("Invalid retained folder"))?;
    tokio::time::timeout(Duration::from_secs(5), async {
        let connection = zbus::Connection::session().await?;
        let proxy = zbus::Proxy::new(
            &connection,
            "org.freedesktop.FileManager1",
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1",
        )
        .await?;
        proxy
            .call::<_, _, ()>("ShowFolders", &(vec![uri.as_str()], ""))
            .await
    })
    .await??;
    Ok(())
}
