//! Device presentation and browser intent. Platform work stays in LocalOperations.
use super::{chrome::Icon, *};
use crate::local_volumes::{
    DeviceOpen as Opened, DeviceOutcome, DevicePhase, DeviceQuestionToken, DeviceRequestId,
    DeviceRequestSnapshot, DeviceSnapshot, DeviceStatus, DeviceToken, LocalOperations,
};
use iced::{
    Alignment, Length,
    widget::{column, container, row, text, tooltip},
};

#[derive(Clone, Debug)]
pub(super) struct Origin {
    window: window::Id,
    tab: u64,
    navigation: u64,
    search: u64,
    intent: u64,
    location: Option<String>,
}

impl Origin {
    pub fn capture(desktop: &Desktop, window: window::Id) -> Option<Self> {
        let tab = desktop.active(window).filter(|tab| !tab.loading)?;
        Some(Self {
            window,
            tab: tab.id,
            navigation: tab.revision,
            search: tab.search_serial,
            intent: desktop.devices.intents.get(&window).copied().unwrap_or(0),
            location: tab.location.clone(),
        })
    }
    pub fn current(&self, desktop: &Desktop) -> bool {
        desktop
            .devices
            .intents
            .get(&self.window)
            .copied()
            .unwrap_or(0)
            == self.intent
            && desktop.active(self.window).is_some_and(|tab| {
                tab.id == self.tab
                    && tab.revision == self.navigation
                    && tab.search_serial == self.search
                    && tab.location == self.location
                    && !tab.loading
            })
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Field {
    Username,
    Domain,
    Password,
    Pim,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Toggle {
    Anonymous,
    Hidden,
    System,
    /// How long GIO may keep the password (server logins only).
    Remember(crate::local_volumes::Remember),
}

#[derive(Clone)]
pub(super) struct Edited(pub String);
impl std::fmt::Debug for Edited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Open(Origin, DeviceToken),
    Remove(window::Id, DeviceToken),
    Cancel(DeviceRequestId),
    Review(DeviceRequestId),
    OpenResult(DeviceRequestId),
    QuestionOpened(window::Id, Option<window::Id>),
    Text(window::Id, DeviceQuestionToken, Field, Edited),
    Toggle(window::Id, DeviceQuestionToken, Toggle),
    Submit(window::Id, DeviceQuestionToken),
    Choice(window::Id, DeviceQuestionToken, usize),
    CancelQuestion(window::Id),
}

#[derive(Default)]
pub(super) struct State {
    pub catalog: DeviceSnapshot,
    pub requests: Vec<DeviceRequestSnapshot>,
    pub error: Option<String>,
    pub notification_error: Option<String>,
    pub notification_errors: BTreeMap<DeviceRequestId, String>,
    pub dialog: Option<super::device_question::Dialog>,
    pub owners: BTreeMap<DeviceRequestId, window::Id>,
    pub origins: BTreeMap<DeviceRequestId, Origin>,
    pub seen: BTreeMap<DeviceRequestId, DeviceQuestionToken>,
    intents: BTreeMap<window::Id, u64>,
    serial: u64,
}

impl State {
    pub fn visible(&self) -> bool {
        !self.requests.is_empty() || self.error.is_some() || self.notification_error.is_some()
    }
    pub fn busy(&self) -> bool {
        self.requests
            .iter()
            .any(|job| job.phase != DevicePhase::Finished)
    }
    pub fn summary(&self) -> Option<String> {
        self.error
            .as_ref()
            .map(|error| format!("Devices: {error}"))
            .or_else(|| {
                self.requests
                    .iter()
                    .find(|job| job.phase != DevicePhase::Finished)
                    .or_else(|| self.requests.first())
                    .map(|job| format!("{} · {}", job.name, status(job)))
            })
    }
}

pub(super) fn status(job: &DeviceRequestSnapshot) -> String {
    if job.phase == DevicePhase::Cancelling {
        return "Cancelling…".into();
    }
    if job.question.is_some() {
        return "Needs a decision".into();
    }
    if job.phase == DevicePhase::Running {
        return match job.operation {
            crate::local_volumes::DeviceOperation::Mount => "Mounting…".into(),
            crate::local_volumes::DeviceOperation::Remove(kind) => format!("{}…", kind.label()),
            crate::local_volumes::DeviceOperation::Connect => "Connecting…".into(),
        };
    }
    match &job.outcome {
        Some(DeviceOutcome::Mounted { .. }) => "Mounted".into(),
        Some(DeviceOutcome::Connected { .. }) => "Connected".into(),
        Some(DeviceOutcome::ConnectedRemote { .. }) => {
            "Connected without a local folder: GVFS's FUSE daemon (gvfsd-fuse) is not running, so only a read-only listing is available".into()
        }
        Some(DeviceOutcome::Removed) => "Removed".into(),
        Some(DeviceOutcome::Cancelled) => "Cancelled".into(),
        Some(DeviceOutcome::Failed { message }) => format!("Could not finish: {message}"),
        Some(DeviceOutcome::Uncertain { message }) => message.clone(),
        None => "Finished".into(),
    }
}

impl Desktop {
    pub(super) fn device_runtime(&self) -> Option<&LocalOperations> {
        self.file_operations.local_runtime()
    }

    pub(super) fn device_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Open(mut origin, token) => {
                if !origin.current(self) {
                    return Task::none();
                }
                let Some(serial) = self.devices.serial.checked_add(1) else {
                    return Task::none();
                };
                self.devices.serial = serial;
                self.devices.intents.insert(origin.window, serial);
                origin.intent = serial;
                if let Some(tab) = self.active_mut(origin.window) {
                    tab.location = None;
                }
                origin.location = None;
                self.devices.error = None;
                let result = self
                    .device_runtime()
                    .ok_or_else(|| io::Error::other("Devices are unavailable."))
                    .and_then(|runtime| runtime.device_requests().open(token));
                match result {
                    Ok(Opened::Root(root)) => {
                        return self.finish_device_navigation(origin, Ok(root));
                    }
                    Ok(Opened::Mount(id)) => {
                        self.devices.owners.insert(id, origin.window);
                        self.devices.origins.insert(id, origin);
                    }
                    Err(error) => {
                        self.devices.error = Some(error.to_string());
                        if let Some(tab) = self.active_mut(origin.window) {
                            tab.error = Some(error.to_string());
                        }
                    }
                }
            }
            Event::Remove(owner, token) => {
                if !self.windows.contains_key(&owner) {
                    return Task::none();
                }
                let result = self
                    .device_runtime()
                    .ok_or_else(|| io::Error::other("Devices are unavailable."))
                    .and_then(|runtime| runtime.device_requests().remove(token));
                match result {
                    Ok(id) => {
                        self.devices.error = None;
                        self.devices.owners.insert(id, owner);
                    }
                    Err(error) => {
                        self.devices.error = Some(error.to_string());
                    }
                }
            }
            Event::Cancel(id) => {
                if let Some(runtime) = self.device_runtime()
                    && let Err(error) = runtime.device_requests().cancel(id)
                {
                    self.devices.error = Some(error.to_string());
                }
            }
            Event::Review(id) => return self.show_device_question(id, true),
            Event::OpenResult(id) => {
                let result = self
                    .device_runtime()
                    .ok_or_else(|| io::Error::other("Devices are unavailable."))
                    .and_then(|runtime| runtime.device_requests().mounted_root(id));
                return match result {
                    Ok(root) => self.new_window(vec![root]),
                    Err(error) => {
                        self.devices.error = Some(error.to_string());
                        Task::none()
                    }
                };
            }
            other => return self.device_question_event(other),
        }
        self.poll_devices()
    }

    pub(super) fn finish_device_navigation(
        &mut self,
        origin: Origin,
        root: io::Result<PathBuf>,
    ) -> Task<Message> {
        if !origin.current(self) {
            return Task::none();
        }
        match root {
            Ok(root) => self.navigate(origin.window, root, Navigation::Visit, None),
            Err(error) => {
                self.devices.error = Some(error.to_string());
                Task::none()
            }
        }
    }

    pub(super) fn poll_devices(&mut self) -> Task<Message> {
        let Some(runtime) = self.device_runtime().cloned() else {
            return Task::none();
        };
        self.devices.catalog = runtime.devices();
        let attention = runtime.device_attention_snapshot();
        self.devices.requests = attention.requests;
        self.devices.notification_errors = attention.notification_errors;
        self.devices.notification_error = attention.error;
        let mut tasks = Vec::new();
        let finished: Vec<_> = self
            .devices
            .origins
            .keys()
            .filter_map(|id| {
                self.devices
                    .requests
                    .iter()
                    .find(|job| job.id == *id && job.phase == DevicePhase::Finished)
                    .map(|job| {
                        (
                            job.id,
                            matches!(
                                job.outcome,
                                Some(
                                    DeviceOutcome::Mounted { .. } | DeviceOutcome::Connected { .. }
                                )
                            ),
                        )
                    })
            })
            .collect();
        for (id, mounted) in finished {
            let origin = self.devices.origins.remove(&id).unwrap();
            if mounted {
                tasks.push(
                    self.finish_device_navigation(
                        origin,
                        runtime.device_requests().mounted_root(id),
                    ),
                );
            }
        }
        self.devices.owners.retain(|id, _| {
            self.devices
                .requests
                .iter()
                .any(|job| job.id == *id && job.phase != DevicePhase::Finished)
        });
        self.devices
            .seen
            .retain(|id, _| self.devices.requests.iter().any(|job| job.id == *id));
        self.devices
            .intents
            .retain(|window, _| self.windows.contains_key(window));
        tasks.push(self.poll_network());
        tasks.push(self.sync_device_question());
        tasks.push(self.auto_device_question());
        tasks.push(self.exit_if_empty());
        Task::batch(tasks)
    }

    pub(super) fn close_device_parent(&mut self, parent: window::Id) -> Task<Message> {
        if self
            .devices
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.parent == parent)
        {
            let dialog = self.devices.dialog.take().unwrap();
            // Parent/window disposal cannot answer or cancel accepted work.
            return window::close(dialog.window);
        }
        Task::none()
    }
}

pub(super) fn places<'a>(desktop: &'a Desktop, window: window::Id) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let size = appearance.typography().body_size.px();
    let font = canopa_ui::font(appearance);
    let mut body = column![chrome::section(appearance, "Devices")].spacing(2);
    let ready = desktop.devices.catalog.status == DeviceStatus::Ready;
    let origin = Origin::capture(desktop, window);
    let model = super::network_ui::sidebar(&desktop.devices.catalog, &desktop.network.bookmarks);
    for device in model.devices {
        let open = if device.remote_only {
            Some(Message::Network(super::network_ui::Event::ShowRemote(
                window,
                device.token,
            )))
        } else {
            origin
                .as_ref()
                .filter(|_| {
                    ready && !desktop.devices.busy() && (device.root.is_some() || device.can_mount)
                })
                .map(|origin| Message::Device(Event::Open(origin.clone(), device.token)))
        };
        let active = device
            .root
            .as_deref()
            .is_some_and(|root| desktop.active(window).is_some_and(|tab| tab.path() == root));
        let label = chrome::single_line(device.name.as_str(), font, size, None, true);
        let open = canopa_ui::styled_action(
            row![
                desktop.icons.chrome(
                    super::network_ui::kind_icon(device.kind),
                    17.0,
                    if open.is_some() { 0.74 } else { 0.35 }
                ),
                label
            ]
            .spacing(9)
            .align_y(Alignment::Center),
            open,
            if active {
                canopa_ui::Action::Normal
            } else {
                canopa_ui::Action::Quiet
            },
            false,
            appearance,
        )
        .width(Length::Fill);
        let mut item = row![super::drag_ui::device_region(
            desktop,
            window,
            device.token,
            open.into()
        )]
        .align_y(Alignment::Center);
        if let Some(kind) = device.removal {
            let enabled = ready && !desktop.devices.busy();
            let button = canopa_ui::styled_action(
                desktop
                    .icons
                    .chrome(Icon::Eject, 18.0, if enabled { 0.95 } else { 0.35 }),
                enabled.then_some(Message::Device(Event::Remove(window, device.token))),
                canopa_ui::Action::Quiet,
                false,
                appearance,
            )
            .padding([7, 8]);
            item = item.push(tooltip(
                button,
                text(kind.label())
                    .font(font)
                    .size(appearance.typography().label_size.px()),
                tooltip::Position::Right,
            ));
        }
        body = body.push(super::row_state::keyed_row(
            super::view::RowKey::Device(device.id()),
            item,
        ));
    }
    if let DeviceStatus::Failed(error) = &desktop.devices.catalog.status {
        body = body.push(text(error).size(size));
    }
    if let Some(error) = &desktop.devices.error {
        body = body.push(text(error).size(size));
    }
    body.into()
}

pub(super) fn rows(
    desktop: &Desktop,
    window: window::Id,
) -> (Element<'_, Message>, Vec<iced::widget::Id>) {
    let appearance = &desktop.configuration.appearance;
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let font = canopa_ui::font(appearance);
    let mut body = column![].spacing(12);
    let mut order = Vec::new();
    if !desktop.devices.visible() {
        return (body.into(), order);
    }
    body = body.push(text("Devices").size(size).font(font));
    for error in [&desktop.devices.error, &desktop.devices.notification_error]
        .into_iter()
        .flatten()
    {
        body = body.push(text(error).size(small).font(font));
    }
    for job in &desktop.devices.requests {
        let mut button = |label: &'static str, name: &str, event: Option<Event>| {
            let id =
                iced::widget::Id::from(format!("files-device-row-{window:?}-{:?}-{name}", job.id));
            let message = event.map(Message::Device);
            if message.is_some() {
                order.push(id.clone());
            }
            super::focus::control(
                id,
                message.clone(),
                appearance,
                canopa_ui::styled_action(
                    text(label).size(small).font(font),
                    message,
                    canopa_ui::Action::Quiet,
                    false,
                    appearance,
                ),
            )
        };
        let mut controls = row![].spacing(8);
        if job.phase == DevicePhase::Running {
            controls = controls.push(button("Cancel", "cancel", Some(Event::Cancel(job.id))));
            if job.question.is_some() {
                controls = controls.push(button("Review", "review", Some(Event::Review(job.id))));
            }
        }
        let available = matches!(
            job.outcome,
            Some(DeviceOutcome::Mounted { .. } | DeviceOutcome::Connected { .. })
        ) && desktop
            .device_runtime()
            .is_some_and(|runtime| runtime.device_requests().mounted_root(job.id).is_ok());
        if available {
            controls = controls.push(button(
                "Open folder",
                "open",
                Some(Event::OpenResult(job.id)),
            ));
        }
        let mut item = column![
            text(&job.name).size(size).font(font),
            text(status(job)).size(small).font(font)
        ]
        .spacing(6);
        if job.phase == DevicePhase::Running || available {
            item = item.push(controls.wrap());
        }
        for error in [
            job.error.as_ref(),
            desktop.devices.notification_errors.get(&job.id),
        ]
        .into_iter()
        .flatten()
        {
            item = item.push(text(error).size(small).font(font));
        }
        body = body.push(super::row_state::keyed_row(
            super::view::RowKey::DeviceJob(job.id),
            container(item).padding(12).width(Length::Fill),
        ));
    }
    (body.into(), order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn a_mount_result_only_navigates_its_unchanged_existing_tab() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("device");
        let other = root.path().join("other");
        fs::create_dir(&destination).unwrap();
        fs::create_dir(&other).unwrap();
        for case in 0..6 {
            let (mut desktop, window) = super::super::tests::desktop(
                root.path(),
                vec![root.path().to_owned(), other.clone()],
            );
            let tab = desktop.active(window).unwrap().id;
            super::super::tests::loaded(&mut desktop, window, tab, 1, root.path());
            let origin = Origin::capture(&desktop, window).unwrap();
            match case {
                1 => {
                    let _ = desktop.navigate(window, other.clone(), Navigation::Visit, None);
                }
                2 => {
                    let _ = desktop.command(window, Command::NewTab);
                }
                3 => {
                    let _ = desktop.command(window, Command::CloseTab);
                }
                4 => {
                    let _ = desktop.update(Message::Location(window, "/new draft".into()));
                }
                5 => {
                    let _ = desktop.update(Message::Closed(window));
                }
                _ => {}
            }
            let before = desktop
                .active(window)
                .map(|tab| (tab.id, tab.target.clone()));
            let _ = desktop.finish_device_navigation(origin, Ok(destination.clone()));
            if case == 0 {
                let current = desktop.active(window).unwrap();
                assert_eq!(current.id, tab);
                assert_eq!(current.target, destination);
                assert!(current.loading);
            } else {
                assert_eq!(
                    desktop
                        .active(window)
                        .map(|tab| (tab.id, tab.target.clone())),
                    before,
                    "case {case}"
                );
            }
        }
    }

    #[test]
    fn credential_edits_redact_their_debug_representation() {
        let edit = Edited("private-secret-do-not-format".into());
        let formatted = format!("{edit:?}");
        assert!(!formatted.contains("private-secret"));
        assert_eq!(formatted, "[redacted]");
    }
}
