//! Request-local print draft. Metadata workers cannot approve a destination.
mod draft;
#[cfg(test)]
mod tests;
mod view;

use crate::{
    chooser_ui::{self, Outcome, Picker, PickerOptions, PickerStart},
    file_chooser::{ChooserRequest, Mode},
    print::{
        Format, Layout, PageSetup, PrepareRequest,
        worker::{Capabilities, Client, CustomSize, Destination, JobOptions, Preferences, Printer},
    },
    process::DialogProcess,
};
use draft::Draft;
use iced::{Element, Event, Size, Subscription, Task, Theme, keyboard, window};
use serde::{Deserialize, Serialize};
use std::{
    os::unix::ffi::OsStringExt,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{runtime::Handle, sync::watch};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Prepare,
    Direct { format: Option<Format> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub prepare: PrepareRequest,
    pub purpose: Purpose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<i32>,
    #[serde(
        default,
        skip_serializing_if = "crate::print::worker::runtime::Selection::is_system"
    )]
    pub cups_runtime: crate::print::worker::runtime::Selection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Printer(Destination),
    File {
        path: Vec<u8>,
        format: Format,
        selection: crate::print::file_target::Selection,
    },
}

/// The serialized selection must be accompanied by its retained descriptors.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub target: Target,
    pub layout: Layout,
    pub page_setup: PageSetup,
    pub device: JobOptions,
}

#[derive(Debug)]
pub struct Approved {
    pub grant: Grant,
    pub file: Option<Arc<crate::print::file_target::Reviewed>>,
    pub(crate) authentication: Option<crate::print::authentication::Retained>,
}
impl Approved {
    /// Transfer only the private retained destination/credentials. The public
    /// grant is serialized separately and never contains authentication values.
    pub fn send(&self, channel: &std::os::unix::net::UnixStream) -> anyhow::Result<()> {
        match &self.grant.target {
            Target::Printer(_) => self
                .authentication
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing printer credential context"))?
                .send(channel),
            Target::File { .. } => {
                anyhow::ensure!(
                    self.authentication.is_none(),
                    "Unexpected printer credentials for file output"
                );
                let file = self
                    .file
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Missing file destination"))?;
                file.verify()?;
                file.send(channel)?;
                Ok(())
            }
        }
    }
}

impl Request {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.channel.is_none(),
            "Approval channel must be adopted before opening the dialog"
        );
        let request = &self.prepare;
        request.settings.layout.validate(request.page_choices)?;
        request.page_setup.validate()?;
        anyhow::ensure!(request.formats.len() <= 3, "Too many print output formats");
        anyhow::ensure!(
            request.context.app_id.len() <= 256,
            "Print application ID exceeds limits"
        );
        for value in [
            &request.context.app_id,
            &request.context.parent_window,
            &request.context.title,
            &request.accept_label,
        ] {
            anyhow::ensure!(
                value.len() <= 4096 && !value.contains('\0'),
                "Invalid print dialog text"
            );
        }
        Ok(())
    }
    pub(crate) fn formats(&self) -> Vec<Format> {
        self.prepare
            .formats
            .iter()
            .copied()
            .filter(|_| match self.purpose {
                Purpose::Prepare => true,
                Purpose::Direct { format: actual } => actual.is_some(),
            })
            .collect()
    }
}

pub fn run(
    request: Request,
    appearance: canopa_theme::Appearance,
    process: DialogProcess,
) -> anyhow::Result<Option<Approved>> {
    request.validate()?;
    // Iced uses its own executor. Keep the Tokio process/reaper runtime alive
    // until every metadata operation has retired, including aborted UI futures.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;
    let authentication =
        crate::print::authentication::Broker::new(process.clone(), request.prepare.context.clone());
    if let crate::print::worker::runtime::Selection::Image { descriptor } = request.cups_runtime {
        anyhow::ensure!(
            Some(descriptor) != request.channel,
            "CUPS runtime aliases print handoff"
        );
    }
    let cups_runtime = crate::print::worker::runtime::Runtime::receive(request.cups_runtime)?;
    let client = Client::with_runtime(process, cups_runtime)
        .with_authentication(Some(authentication.clone()));
    let reaper = client.clone();
    let handle = runtime.handle().clone();
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    let (cancel, cancelled) = watch::channel(false);
    let app_cancel = cancelled.clone();
    let run = iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(760., 720.),
                min_size: Some(Size::new(480., 400.)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let parent = request
                .prepare
                .context
                .parent_window
                .strip_prefix("wayland:")
                .unwrap_or_default();
            let (_, opening) =
                window::open_dialog(parent.to_owned(), request.prepare.context.modal, settings);
            let mut dialog = Dialog::new(
                request.clone(),
                theme.clone(),
                output.clone(),
                client.clone(),
                handle.clone(),
                app_cancel.clone(),
                authentication.clone(),
            );
            let query = dialog.query(Query::Printers);
            (
                dialog,
                Task::batch([opening.map(|_| Message::Opened), query]),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|app: &Dialog, _| {
        let title = if matches!(app.screen, Screen::Main) {
            fl!("portal-print-title")
        } else {
            fl!("portal-print-save-title")
        };
        fl!("portal-window-title", title = title)
    })
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(Dialog::subscription)
    .default_font(font)
    .run();
    let _ = cancel.send(true);
    let drained = runtime
        .block_on(async { tokio::time::timeout(Duration::from_secs(15), reaper.shutdown()).await });
    runtime.shutdown_timeout(Duration::from_secs(1));
    anyhow::ensure!(drained.is_ok(), "Print metadata workers did not retire");
    run.map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("print dialog result").take())
}

#[derive(Clone, Debug, PartialEq)]
struct Choice<T> {
    value: T,
    label: String,
}
impl<T> std::fmt::Display for Choice<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.label.fmt(formatter)
    }
}
#[derive(Clone, Debug, PartialEq)]
enum DestinationChoice {
    Printer(Destination),
    File,
}

enum Screen {
    Main,
    Preparing(u64),
    Picker(u64, Box<Picker>),
}
enum Query {
    Printers,
    Capabilities(Destination, Option<Box<Preferences>>),
    CheckPrinter(Destination, Box<JobOptions>, Option<CustomSize>),
}
#[derive(Clone, Debug)]
enum Metadata {
    Printers(Vec<Printer>),
    Capabilities(Capabilities),
    CheckedPrinter(Destination, JobOptions),
}
struct Running {
    abort: tokio::task::AbortHandle,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

struct Dialog {
    request: Request,
    theme: Theme,
    output: Arc<Mutex<Option<Approved>>>,
    client: Client,
    authentication: Arc<crate::print::authentication::Broker>,
    runtime: Handle,
    cancelled: watch::Receiver<bool>,
    running: Option<Running>,
    generation: u64,
    loading: bool,
    printers: Vec<Printer>,
    target: Option<DestinationChoice>,
    capabilities: Option<Capabilities>,
    checked: Option<JobOptions>,
    draft: Draft,
    screen: Screen,
    completed: bool,
    error: Option<String>,
    initial: bool,
    notice: Option<String>,
}

#[derive(Clone, Debug)]
enum Message {
    Opened,
    Refresh,
    Metadata(u64, Result<Metadata, String>),
    Destination(Choice<DestinationChoice>),
    Edit(draft::Edit),
    ChooseFile,
    CheckCustomMedia,
    Back,
    PickerReady(u64, Result<Box<PickerStart>, String>),
    Picker(u64, chooser_ui::Message),
    Approve(u64),
    Cancel,
    Event(Event),
}

impl Dialog {
    fn new(
        request: Request,
        theme: Theme,
        output: Arc<Mutex<Option<Approved>>>,
        client: Client,
        runtime: Handle,
        cancelled: watch::Receiver<bool>,
        authentication: Arc<crate::print::authentication::Broker>,
    ) -> Self {
        let draft = Draft::new(&request);
        Self {
            request,
            theme,
            output,
            client,
            authentication,
            runtime,
            cancelled,
            running: None,
            generation: 0,
            loading: false,
            printers: Vec::new(),
            target: None,
            capabilities: None,
            checked: None,
            draft,
            screen: Screen::Main,
            completed: false,
            error: None,
            initial: true,
            notice: None,
        }
    }
    fn query(&mut self, query: Query) -> Task<Message> {
        self.running = None;
        self.generation += 1;
        self.loading = true;
        let generation = self.generation;
        let client = self.client.clone();
        let cancelled = self.cancelled.clone();
        let task = self.runtime.spawn(async move {
            // A superseded execute task owns cleanup after its future is dropped.
            // Wait for it rather than exhausting the two worker slots on clicks.
            client.idle().await?;
            match query {
                Query::Printers => client.printers(cancelled).await?.map(Metadata::Printers),
                Query::Capabilities(destination, preferences) => client
                    .capabilities_with_preferences(destination, preferences, cancelled)
                    .await?
                    .map(Metadata::Capabilities),
                Query::CheckPrinter(destination, options, size) => client
                    .check_printer(destination.clone(), *options, size, cancelled)
                    .await?
                    .map(|options| Metadata::CheckedPrinter(destination, options)),
            }
            .ok_or_else(|| anyhow::anyhow!("Printer query cancelled"))
        });
        self.running = Some(Running {
            abort: task.abort_handle(),
        });
        Task::perform(
            async move {
                task.await
                    .map_err(|error| error.to_string())
                    .and_then(|value| value.map_err(|error| error.to_string()))
            },
            move |result| Message::Metadata(generation, result),
        )
    }
    fn editable(&self) -> bool {
        !self.completed && matches!(self.screen, Screen::Main)
    }
    fn select(&mut self, target: DestinationChoice) -> Task<Message> {
        if !matches!(target, DestinationChoice::File) {
            self.draft.path = None;
            self.draft.reviewed = None;
        }
        self.capabilities = None;
        self.checked = None;
        self.draft.device = JobOptions::default();
        self.draft.custom = false;
        self.error = None;
        self.notice = None;
        self.target = Some(target.clone());
        match target {
            DestinationChoice::Printer(destination) => self.query(Query::Capabilities(
                destination,
                self.initial.then(|| {
                    Box::new(Preferences::new(
                        &self.request.prepare.settings,
                        &self.request.prepare.page_setup,
                    ))
                }),
            )),
            DestinationChoice::File => {
                self.authentication.clear();
                if self.initial
                    && self
                        .request
                        .prepare
                        .settings
                        .advisory("output-file-format")
                        .is_some_and(|value| {
                            !self
                                .request
                                .formats()
                                .iter()
                                .any(|format| format.name() == value)
                        })
                {
                    self.notice = Some(fl!("portal-print-format-unavailable"));
                }
                self.initial = false;
                self.running = None;
                self.generation += 1;
                self.loading = false;
                Task::none()
            }
        }
    }
    fn grant(&self) -> Option<Grant> {
        if !self.editable() || self.loading {
            return None;
        }
        let (layout, mut page_setup, mut device) = self
            .draft
            .values(&self.request, self.target == Some(DestinationChoice::File))
            .ok()?;
        let target = match self.target.as_ref()? {
            DestinationChoice::Printer(destination) => {
                let caps = self.capabilities.as_ref()?;
                if layout.copies > 1 && device.sides.is_none() {
                    device.sides = Some(caps.default_sides?);
                }
                if &caps.destination != destination || !caps.accepts(&device) {
                    return None;
                }
                // Physical copies are materialized before submission; the
                // device receives one copy. Check precisely that option set.
                let mut transport = device.clone();
                transport.copies = 1;
                if self.checked.as_ref() != Some(&transport) {
                    return None;
                }
                if self.draft.custom {
                    let size = self.custom_size()?;
                    if !device
                        .custom_media
                        .as_ref()
                        .is_some_and(|media| size.matches(media))
                    {
                        return None;
                    }
                } else if device.custom_media.is_some() {
                    return None;
                }
                if !self
                    .printers
                    .iter()
                    .any(|printer| &printer.destination == destination && printer.accepting)
                {
                    return None;
                }
                page_setup = self.request.prepare.page_setup.clone();
                page_setup.orientation = Some(layout.orientation);
                if let Some(media) = device.custom_media.as_ref().or_else(|| {
                    device
                        .media
                        .as_ref()
                        .and_then(|id| caps.media.iter().find(|media| &media.id == id))
                }) {
                    page_setup = media.page_setup();
                    page_setup.orientation = Some(layout.orientation);
                }
                Target::Printer(destination.clone())
            }
            DestinationChoice::File => {
                let format = self.draft.format?;
                if !self.request.formats().contains(&format) {
                    return None;
                }
                let path = self.draft.path.clone()?;
                let reviewed = self.draft.reviewed.as_ref()?;
                if reviewed.selection().path() != path {
                    return None;
                }
                if path.is_empty() || path[0] != b'/' || path.contains(&0) {
                    return None;
                }
                device = JobOptions {
                    copies: layout.copies,
                    ..Default::default()
                };
                Target::File {
                    path,
                    format,
                    selection: reviewed.selection().clone(),
                }
            }
        };
        Some(Grant {
            target,
            layout,
            page_setup,
            device,
        })
    }
    fn custom_size(&self) -> Option<CustomSize> {
        if !self.draft.custom || !self.capabilities.as_ref()?.custom_media {
            return None;
        }
        CustomSize::parse(
            &self.draft.custom_dimensions[0],
            &self.draft.custom_dimensions[1],
        )
        .ok()
    }
    fn check_request(&self) -> Option<(Destination, JobOptions, Option<CustomSize>)> {
        let DestinationChoice::Printer(destination) = self.target.as_ref()? else {
            return None;
        };
        self.capabilities.as_ref()?;
        let mut options = self.draft.device.clone();
        options.copies = 1;
        if self
            .draft
            .copies
            .parse::<u32>()
            .is_ok_and(|copies| copies > 1)
            && options.sides.is_none()
        {
            options.sides = self.capabilities.as_ref()?.default_sides;
        }
        options.custom_media = None;
        let size = if self.draft.custom {
            Some(self.custom_size()?)
        } else {
            None
        };
        Some((destination.clone(), options, size))
    }
    fn check(&mut self) -> Task<Message> {
        self.checked = None;
        if let Some((destination, options, size)) = self.check_request() {
            return self.query(Query::CheckPrinter(destination, Box::new(options), size));
        }
        Task::none()
    }
    fn back(&mut self) -> Task<Message> {
        self.generation += 1;
        self.screen = Screen::Main;
        Task::none()
    }
    fn subscription(&self) -> Subscription<Message> {
        if let Screen::Picker(generation, picker) = &self.screen {
            return picker
                .subscription()
                .with(*generation)
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
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Refresh if self.editable() => {
                self.initial = false;
                self.target = None;
                self.capabilities = None;
                self.checked = None;
                self.printers.clear();
                self.error = None;
                return self.query(Query::Printers);
            }
            Message::Metadata(generation, result)
                if generation == self.generation && self.loading =>
            {
                self.running = None;
                self.loading = false;
                match result {
                    Ok(Metadata::Printers(printers)) => {
                        let preferred = self
                            .initial
                            .then(|| self.request.prepare.settings.advisory("printer"))
                            .flatten();
                        let selected = if let Some(name) = preferred {
                            printers.iter().find(|p| p.destination.name() == name)
                        } else {
                            printers
                                .iter()
                                .find(|p| p.is_default)
                                .or_else(|| printers.first())
                        }
                        .map(|p| p.destination.clone());
                        let file = preferred == Some("Print to File")
                            && !self.request.formats().is_empty();
                        let missing = preferred.is_some() && selected.is_none() && !file;
                        self.printers = printers;
                        if file {
                            return self.select(DestinationChoice::File);
                        }
                        if missing {
                            self.notice = Some(fl!("portal-print-printer-unavailable"));
                            return Task::none();
                        }
                        if let Some(destination) = selected {
                            return self.select(DestinationChoice::Printer(destination));
                        }
                        if !self.request.formats().is_empty() {
                            return self.select(DestinationChoice::File);
                        }
                    }
                    Ok(Metadata::Capabilities(caps))
                        if self.target
                            == Some(DestinationChoice::Printer(caps.destination.clone())) =>
                    {
                        self.draft.install(&caps);
                        if self.initial {
                            if let Some(selection) = &caps.preferences {
                                self.draft.restore(selection);
                                if selection.unavailable {
                                    self.notice = Some(fl!("portal-print-preferences-unavailable"));
                                }
                            }
                            self.initial = false;
                        }
                        self.capabilities = Some(caps);
                        return self.check();
                    }
                    Ok(Metadata::CheckedPrinter(destination, options)) => {
                        let current =
                            self.check_request().is_some_and(|(d, mut expected, size)| {
                                if let Some(size) = size {
                                    if !options
                                        .custom_media
                                        .as_ref()
                                        .is_some_and(|m| size.matches(m))
                                    {
                                        return false;
                                    }
                                    expected.custom_media = options.custom_media.clone();
                                }
                                d == destination && expected == options
                            });
                        if current {
                            self.draft.device.custom_media = options.custom_media.clone();
                            self.checked = Some(options);
                            self.error = None;
                        } else {
                            self.error = Some(fl!("portal-print-settings-changed"));
                        }
                    }
                    Ok(_) => self.error = Some(fl!("portal-print-information-changed")),
                    Err(_) => {
                        self.error = Some(if self.capabilities.is_some() {
                            fl!("portal-print-settings-unconfirmed")
                        } else {
                            fl!("portal-print-information-failed")
                        })
                    }
                }
            }
            Message::Destination(choice) if self.editable() => {
                let valid = match &choice.value {
                    DestinationChoice::Printer(destination) => self
                        .printers
                        .iter()
                        .any(|printer| &printer.destination == destination),
                    DestinationChoice::File => !self.request.formats().is_empty(),
                };
                if valid {
                    self.initial = false;
                    return self.select(choice.value);
                }
            }
            Message::Edit(edit) if self.editable() && !self.loading => {
                let device_changed = matches!(
                    &edit,
                    draft::Edit::Media(_)
                        | draft::Edit::Custom(_)
                        | draft::Edit::CustomDimension(..)
                        | draft::Edit::Sides(_)
                        | draft::Edit::Color(_)
                        | draft::Edit::Setting(..)
                ) || (matches!(&edit, draft::Edit::Copies(_))
                    && self.draft.device.sides.is_none());
                let edit = if let draft::Edit::Setting(field, mut choice) = edit {
                    if choice.value.is_none() {
                        choice.value = self
                            .capabilities
                            .as_ref()
                            .and_then(|caps| caps.controls.get(field).default.clone());
                    }
                    draft::Edit::Setting(field, choice)
                } else {
                    edit
                };
                let default_paper =
                    matches!(&edit, draft::Edit::Media(choice) if choice.value.is_none());
                let fixed_paper = matches!(&edit, draft::Edit::Custom(false));
                self.draft.edit(edit);
                if let Some(caps) = &self.capabilities {
                    if default_paper {
                        self.draft.default_paper(caps);
                    } else if fixed_paper {
                        self.draft.fixed_paper(caps);
                    }
                }
                self.generation += 1;
                self.error = None;
                if device_changed {
                    self.checked = None;
                    if !self.draft.custom {
                        return self.check();
                    }
                }
            }
            Message::CheckCustomMedia if self.editable() && !self.loading => {
                self.error = None;
                return self.check();
            }
            Message::ChooseFile
                if self.editable() && self.target == Some(DestinationChoice::File) =>
            {
                let Some(format) = self
                    .draft
                    .format
                    .filter(|format| self.request.formats().contains(format))
                else {
                    return Task::none();
                };
                self.generation += 1;
                let generation = self.generation;
                self.screen = Screen::Preparing(generation);
                let mut request = ChooserRequest::parse(
                    Mode::Save,
                    self.request.prepare.context.app_id.clone(),
                    String::new(),
                    fl!("portal-print-save-title"),
                    Default::default(),
                )
                .expect("local print file picker");
                request.accept_label = fl!("portal-print-select-destination");
                request.current_name = Some(format!(
                    "{}.{}",
                    draft::file_stem(&self.request.prepare.context.title),
                    format.name()
                ));
                let filter = (
                    fl!(
                        "portal-print-format-filter",
                        format = format.name().to_uppercase()
                    ),
                    vec![(0, format!("*.{}", format.name()))],
                );
                request.filters = vec![filter.clone()];
                request.current_filter = Some(filter);
                return Task::perform(
                    crate::worker::run(move || chooser_ui::prepare_picker(request)),
                    move |result| {
                        Message::PickerReady(
                            generation,
                            result.and_then(|value| {
                                value
                                    .map(Box::new)
                                    .map_err(|_| fl!("portal-print-picker-failed"))
                            }),
                        )
                    },
                );
            }
            Message::PickerReady(generation, result) if matches!(self.screen, Screen::Preparing(current) if current == generation) => {
                match result {
                    Ok(start) => {
                        let (picker, task) = Picker::new(
                            *start,
                            PickerOptions {
                                read_only_choice: false,
                                retain_file_target: true,
                            },
                        );
                        self.screen = Screen::Picker(generation, Box::new(picker));
                        return task.map(move |message| Message::Picker(generation, message));
                    }
                    Err(error) => {
                        self.error = Some(error);
                        return self.back();
                    }
                }
            }
            Message::Picker(generation, message) => {
                let Screen::Picker(current, picker) = &mut self.screen else {
                    return Task::none();
                };
                if *current != generation {
                    return Task::none();
                }
                if matches!(
                    &message,
                    chooser_ui::Message::Event(Event::Window(window::Event::CloseRequested))
                ) {
                    return self.update(Message::Cancel);
                }
                let task = picker.update(message);
                match picker.take_outcome() {
                    Some(Outcome::Cancelled) => return self.back(),
                    Some(Outcome::Selected(selection)) => {
                        self.draft.path = selection.paths.into_iter().next();
                        self.draft.reviewed = picker.take_file_target();
                        return self.back();
                    }
                    None => return task.map(move |message| Message::Picker(generation, message)),
                }
            }
            Message::Back if !matches!(self.screen, Screen::Main) => return self.back(),
            Message::Approve(generation) if generation == self.generation => {
                if let Some(grant) = self.grant() {
                    let file = matches!(grant.target, Target::File { .. })
                        .then(|| self.draft.reviewed.clone())
                        .flatten();
                    let authentication = match &grant.target {
                        Target::Printer(destination) => Some(self.authentication.take(destination)),
                        Target::File { .. } => {
                            self.authentication.clear();
                            None
                        }
                    };
                    *self.output.lock().expect("print dialog result") = Some(Approved {
                        grant,
                        file,
                        authentication,
                    });
                    self.running = None;
                    self.completed = true;
                    return iced::exit();
                }
            }
            Message::Cancel => {
                self.running = None;
                self.completed = true;
                return iced::exit();
            }
            Message::Event(Event::Window(window::Event::CloseRequested)) => {
                return self.update(Message::Cancel);
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => {
                if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    return self.update(if matches!(self.screen, Screen::Main) {
                        Message::Cancel
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
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        view::view(self)
    }
}

fn path_label(path: &[u8]) -> String {
    PathBuf::from(std::ffi::OsString::from_vec(path.to_vec()))
        .display()
        .to_string()
}
