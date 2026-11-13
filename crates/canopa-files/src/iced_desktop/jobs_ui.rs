//! Shared Operations presentation. Commands have their own bounded admission
//! alongside the filesystem queue; executing a script never fabricates Undo.
use super::*;
pub(super) use crate::action_jobs::ActionRequest as CommandRequest;
use crate::action_jobs::{JobSnapshot, Phase};
use crate::local_actions::LocalActions;
use iced::futures::{StreamExt, channel::mpsc};
use iced::{
    Length,
    widget::{column, container, row, scrollable, text, text_editor},
};
use std::sync::Mutex;

const PAGE_SIZE: usize = 20;
const OUTPUT_PAGE_BYTES: usize = 8 * 1024;

struct Job {
    state: JobSnapshot,
    output: Option<text_editor::Content>,
    output_full: Option<Arc<str>>,
    output_page: usize,
    notification_error: Option<String>,
}

impl Job {
    fn output_pages(&self) -> usize {
        self.output_full
            .as_ref()
            .map_or(1, |text| text.len().div_ceil(OUTPUT_PAGE_BYTES).max(1))
    }
    fn status(&self) -> String {
        match self.state.phase {
            Phase::Queued => fl!("files-phase-queued"),
            Phase::WaitingForDevice => fl!("files-phase-waiting-for-device"),
            Phase::Running => fl!("files-phase-running"),
            Phase::Cancelling => fl!("files-device-cancelling"),
            Phase::Finished => self.state.completion_status().map_or_else(
                || fl!("files-jobs-cancelled-before-start"),
                |status| status.description(),
            ),
        }
    }
    fn output_text(&self) -> String {
        let Some(Ok(outcome)) = self.state.outcome.as_deref() else {
            return self.status();
        };
        let mut output = String::new();
        for (name, bytes, truncated) in [
            (
                fl!("files-output-stdout"),
                &outcome.stdout,
                outcome.stdout_truncated,
            ),
            (
                fl!("files-output-stderr"),
                &outcome.stderr,
                outcome.stderr_truncated,
            ),
        ] {
            output.push_str(&fl!("files-output-heading", stream = name));
            output.push('\n');
            if bytes.is_empty() {
                output.push_str(&fl!("files-output-empty"));
            } else {
                output.push_str(&String::from_utf8_lossy(bytes));
            }
            if truncated {
                output.push('\n');
                output.push_str(&fl!("files-output-truncated"));
            }
            output.push_str("\n\n");
        }
        output
    }
}

fn output_page(text: &str, page: usize) -> &str {
    let boundary = |offset: usize| {
        let mut offset = offset.min(text.len());
        while !text.is_char_boundary(offset) {
            offset += 1;
        }
        offset
    };
    &text[boundary(page.saturating_mul(OUTPUT_PAGE_BYTES))
        ..boundary(page.saturating_add(1).saturating_mul(OUTPUT_PAGE_BYTES))]
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Poll,
    Admitted(Option<Result<u64, String>>),
    Cancel(u64),
    Output(u64),
    OutputPage(u64, bool),
    CopyOutput(u64),
    Text(u64, text_editor::Action),
    Older,
    Newest,
    OpenFolder(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    NewBrowser,
    Newest,
    Older,
    Cancel(u64),
    Output(u64),
    PreviousOutput(u64),
    NextOutput(u64),
    CopyOutput(u64),
    OpenFolder(u64),
    Text(u64),
}

#[derive(Clone)]
struct Notices(Arc<Mutex<Option<mpsc::Receiver<()>>>>);
impl PartialEq for Notices {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Notices {}
impl std::hash::Hash for Notices {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&Arc::as_ptr(&self.0), state);
    }
}

#[derive(Default)]
pub(super) struct Jobs {
    entries: BTreeMap<u64, Job>,
    before: Option<u64>,
    service: Option<LocalActions>,
    notices: Option<Notices>,
    admissions: usize,
    error: Option<String>,
}

impl Jobs {
    pub fn attach(runtime: Result<LocalActions, String>) -> Self {
        let mut jobs = Self::default();
        match runtime {
            Ok(runtime) => {
                jobs.notices = runtime
                    .take_notices()
                    .map(|notices| Notices(Arc::new(Mutex::new(Some(notices)))));
                jobs.service = Some(runtime);
                jobs.refresh();
            }
            Err(error) => jobs.error = Some(error),
        }
        jobs
    }
    fn service(&self) -> io::Result<&LocalActions> {
        self.service.as_ref().ok_or_else(|| {
            io::Error::other(
                self.error
                    .clone()
                    .unwrap_or_else(|| fl!("files-actions-runtime-unavailable")),
            )
        })
    }
    pub fn subscription(&self) -> Subscription<Message> {
        self.notices
            .as_ref()
            .map_or_else(Subscription::none, |notices| {
                Subscription::run_with(notices.clone(), |key| {
                    let receiver = key
                        .0
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .take();
                    iced::futures::stream::unfold(receiver, |receiver| async move {
                        let mut receiver = receiver?;
                        receiver.next().await?;
                        Some((Message::Job(Event::Poll), Some(receiver)))
                    })
                })
            })
    }
    pub fn has_active(&self) -> bool {
        self.admissions > 0
            || self
                .entries
                .values()
                .any(|job| job.state.phase != Phase::Finished)
    }
    pub fn holds_application(&self) -> bool {
        self.admissions > 0
            || self
                .service
                .as_ref()
                .is_some_and(|runtime| runtime.snapshot().holds_application())
    }
    pub fn summary(&self) -> Option<String> {
        self.error
            .as_ref()
            .map(|error| fl!("files-custom-actions-error", error = error.clone()))
            .or_else(|| {
                self.entries.last_key_value().map(|(_, job)| {
                    fl!(
                        "files-operation-announcement",
                        operation = job.state.title.clone(),
                        status = job.status()
                    )
                })
            })
    }
    fn page(&self) -> Vec<(u64, &Job)> {
        self.entries
            .iter()
            .rev()
            .filter(|(id, _)| self.before.is_none_or(|before| **id < before))
            .take(PAGE_SIZE)
            .map(|(id, job)| (*id, job))
            .collect()
    }
    fn refresh(&mut self) {
        let Some(service) = &self.service else {
            return;
        };
        let snapshot = service.snapshot();
        if let Some(error) = snapshot.error {
            self.error = Some(error);
        }
        let errors = snapshot.notification_errors;
        let snapshot = snapshot.queue;
        let ids: std::collections::BTreeSet<_> = snapshot.jobs.iter().map(|job| job.id).collect();
        self.entries.retain(|id, _| ids.contains(id));
        for state in snapshot.jobs {
            let notification_error = errors.get(&state.id).cloned();
            match self.entries.entry(state.id) {
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    let job = entry.get_mut();
                    job.state = state;
                    job.notification_error = notification_error;
                }
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(Job {
                        state,
                        output: None,
                        output_full: None,
                        output_page: 0,
                        notification_error,
                    });
                }
            }
        }
    }

    fn has_older(&self) -> bool {
        self.page()
            .last()
            .is_some_and(|(id, _)| self.entries.range(..*id).next().is_some())
    }

    fn controls(&self) -> Vec<Control> {
        let mut controls = vec![Control::NewBrowser];
        if self.before.is_some() {
            controls.push(Control::Newest);
        }
        if self.has_older() {
            controls.push(Control::Older);
        }
        for (id, job) in self.page() {
            controls.push(if job.state.phase == Phase::Finished {
                Control::Output(id)
            } else {
                Control::Cancel(id)
            });
            controls.push(Control::OpenFolder(id));
            if job.output.is_some() {
                if job.output_page > 0 {
                    controls.push(Control::PreviousOutput(id));
                }
                if job.output_page + 1 < job.output_pages() {
                    controls.push(Control::NextOutput(id));
                }
                controls.push(Control::CopyOutput(id));
                controls.push(Control::Text(id));
            }
        }
        controls
    }
}

fn root_id(window: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-operations-{window:?}"))
}
fn scroll_id(window: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-operations-scroll-{window:?}"))
}
fn control_id(window: window::Id, control: Control) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-operations-control-{window:?}-{control:?}"))
}

impl Desktop {
    pub(super) fn enqueue_command(&mut self, request: CommandRequest) -> Task<Message> {
        let admission = self
            .jobs
            .service()
            .and_then(|service| service.submit(request));
        match admission {
            Ok(receiver) => {
                self.jobs.admissions += 1;
                Task::perform(async move { receiver.await.ok() }, |result| {
                    Message::Job(Event::Admitted(result))
                })
            }
            Err(error) => self.command_admission_error(&error.to_string()),
        }
    }

    fn command_admission_error(&mut self, message: &str) -> Task<Message> {
        self.jobs.error = Some(message.into());
        for window in self.windows.values_mut() {
            if let Some(tab) = window.tabs.get_mut(window.active) {
                tab.error = Some(message.into());
            }
        }
        Task::none()
    }

    fn sync_command_jobs(&mut self) -> Task<Message> {
        self.jobs.refresh();
        self.exit_if_empty()
    }

    pub(super) fn job_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::OpenFolder(id) => {
                if let Some(job) = self.jobs.entries.get(&id) {
                    return self.new_window(vec![job.state.directory.clone()]);
                }
            }
            Event::Poll => return self.sync_command_jobs(),
            Event::Admitted(result) => {
                self.jobs.admissions = self.jobs.admissions.saturating_sub(1);
                if let Err(error) =
                    result.unwrap_or_else(|| Err(fl!("files-actions-runtime-stopped-admission")))
                {
                    let _ = self.command_admission_error(&error);
                }
                return self.sync_command_jobs();
            }
            Event::Cancel(id) => {
                if let Some(service) = &self.jobs.service
                    && let Err(error) = service.cancel(id)
                {
                    let _ = self.command_admission_error(&error.to_string());
                }
                return self.sync_command_jobs();
            }
            Event::Output(id) => {
                if let Some(job) = self.jobs.entries.get_mut(&id) {
                    if job.output.is_some() {
                        job.output = None;
                        job.output_full = None;
                    } else {
                        let full = job.output_text();
                        job.output = Some(text_editor::Content::with_text(output_page(&full, 0)));
                        job.output_full = Some(full.into());
                        job.output_page = 0;
                    }
                }
            }
            Event::OutputPage(id, next) => {
                if let Some(job) = self.jobs.entries.get_mut(&id)
                    && let Some(full) = &job.output_full
                {
                    let page = if next {
                        job.output_page
                            .saturating_add(1)
                            .min(job.output_pages() - 1)
                    } else {
                        job.output_page.saturating_sub(1)
                    };
                    if page != job.output_page {
                        job.output_page = page;
                        job.output = Some(text_editor::Content::with_text(output_page(full, page)));
                    }
                }
            }
            Event::CopyOutput(id) => {
                if let Some(full) = self
                    .jobs
                    .entries
                    .get(&id)
                    .and_then(|job| job.output_full.as_ref())
                {
                    return iced::clipboard::write(full.to_string());
                }
            }
            Event::Text(id, action) => {
                if !action.is_edit()
                    && let Some(output) = self
                        .jobs
                        .entries
                        .get_mut(&id)
                        .and_then(|job| job.output.as_mut())
                {
                    output.perform(action);
                }
            }
            Event::Older => {
                if let Some((last, _)) = self.jobs.page().last() {
                    self.jobs.before = Some(*last);
                }
            }
            Event::Newest => {
                self.jobs.before = None;
            }
        }
        Task::none()
    }

    pub(super) fn show_operations(&mut self) -> Task<Message> {
        if let Some(id) = self.operations_window {
            return window::gain_focus(id);
        }
        let height = if self.jobs.entries.is_empty()
            && !self.devices.visible()
            && !self.file_operations.has_active()
        {
            match self.file_operations.snapshot.jobs.len() {
                0 | 1 => 236.0,
                2 | 3 => 412.0,
                _ => 450.0,
            }
        } else {
            450.0
        };
        let (id, opened) = window::open(window::Settings {
            size: Size::new(720.0, height),
            min_size: Some(Size::new(280.0, 210.0)),
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "org.canopa.Files".into(),
                ..Default::default()
            },
            ..Default::default()
        });
        self.operations_window = Some(id);
        opened.map(Message::Opened)
    }

    pub(super) fn operations_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        if modifiers.alt() && key.as_ref() == keyboard::Key::Character("d") {
            let job = self
                .file_operations
                .page()
                .into_iter()
                .find(|job| job.progress.is_some() || job.transfer.is_some())
                .map(|job| job.id);
            if let Some(job) = job {
                return self.show_operation_details(job);
            }
            return Task::none();
        }
        if key_chord(&key, modifiers).is_some_and(|chord| {
            self.configuration.settings.command_for(&chord) == Some(Command::Undo)
        }) {
            return self.command(id, Command::Undo);
        }
        if modifiers.control() && key.as_ref() == keyboard::Key::Character("n") {
            return self.new_window(vec![self.home.clone()]);
        }
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || modifiers.control() && key.as_ref() == keyboard::Key::Character("w")
        {
            return self.update(Message::Close(id));
        }
        Task::none()
    }
}

pub(super) fn view(desktop: &Desktop) -> Element<'_, Message> {
    iced::widget::responsive(move |size| contents(desktop, size.width)).into()
}

fn contents(desktop: &Desktop, width: f32) -> Element<'_, Message> {
    let wide = width >= 560.0;
    let history_wide = width >= 660.0;
    let appearance = &desktop.configuration.appearance;
    let small = appearance.typography().label_size.px();
    let font = canopa_ui::font(appearance);
    let window = desktop.operations_window.unwrap();
    let action = |label: String, control, message: Option<Message>| {
        let label = text(label).size(small).font(font);
        let content: Element<'_, Message> = if control == Control::NewBrowser {
            row![
                desktop.icons.chrome(chrome::Icon::Columns, 16.0, 0.86),
                label
            ]
            .spacing(7)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            label.into()
        };
        super::focus::button(
            control_id(window, control),
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                content,
                message,
                if control == Control::NewBrowser {
                    canopa_ui::Action::Quiet
                } else {
                    canopa_ui::Action::Normal
                },
                false,
                appearance,
            ),
        )
    };
    let (file_rows, file_controls, recovery) =
        super::file_operation_rows::rows(desktop, window, history_wide);
    let tools = row![
        action(
            fl!("files-new-browser"),
            Control::NewBrowser,
            Some(Message::NewBrowser(window))
        ),
        recovery,
    ]
    .spacing(6);
    let subtitle = fl!("files-operations-subtitle");
    let heading: Element<'_, Message> = if wide {
        chrome::heading_with_action(
            desktop,
            chrome::Icon::Activity,
            fl!("files-command-operations"),
            subtitle.clone(),
            Some(
                tools
                    .push(chrome::dismiss(desktop, window))
                    .align_y(iced::Alignment::Center)
                    .into(),
            ),
        )
    } else {
        column![
            chrome::heading_with_action(
                desktop,
                chrome::Icon::Activity,
                fl!("files-command-operations"),
                subtitle,
                Some(chrome::dismiss(desktop, window))
            ),
            tools.wrap(),
        ]
        .spacing(6)
        .into()
    };
    // The zero-width live region shares a spacing-free row with the heading.
    let heading = row![desktop.operations_announcer(window), heading];
    let mut body = column![heading].spacing(14);
    if let Some(error) = &desktop.jobs.error {
        body = body.push(text(error).size(small).font(font));
    }
    let page = desktop.jobs.page();
    let more = desktop.jobs.has_older();
    let (device_rows, device_controls) = super::devices_ui::rows(desktop, window);
    let mut rows = column![file_rows].spacing(16);
    if desktop.devices.visible() {
        rows = rows.push(device_rows);
    }
    if !desktop.jobs.entries.is_empty() {
        rows = rows
            .push(iced::widget::rule::horizontal(1).style(chrome::divider))
            .push(chrome::section_title(
                appearance,
                fl!("files-custom-commands"),
            ));
        if desktop.jobs.before.is_some() || more {
            rows = rows.push(
                row![
                    action(
                        fl!("files-latest-commands"),
                        Control::Newest,
                        desktop.jobs.before.map(|_| Message::Job(Event::Newest))
                    ),
                    action(
                        fl!("files-older-commands"),
                        Control::Older,
                        more.then_some(Message::Job(Event::Older))
                    )
                ]
                .spacing(8),
            );
        }
    }
    for (id, job) in page {
        let folder = super::question_dialog::display_location(&desktop.home, &job.state.directory);
        let identity = row![
            container(desktop.icons.chrome(chrome::Icon::Terminal, 18.0, 0.9))
                .padding(8)
                .style(move |_| chrome::highlight(
                    appearance,
                    canopa_theme::ColorRole::AccentSurface
                )),
            column![
                chrome::section_title(appearance, job.state.title.clone())
                    .wrapping(text::Wrapping::WordOrGlyph),
                text(fl!(
                    "files-command-job-folder",
                    folder = folder,
                    count = job.state.count
                ))
                .size(small)
                .font(font)
                .style(canopa_ui::muted)
                .wrapping(text::Wrapping::WordOrGlyph),
            ]
            .spacing(3)
            .width(Length::Fill),
            container(
                text(job.status())
                    .size(small)
                    .font(font)
                    .wrapping(text::Wrapping::WordOrGlyph)
            )
            .padding([5, 8])
            .max_width(145)
            .style(move |_| chrome::highlight(appearance, canopa_theme::ColorRole::Spine)),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center);
        let primary = if job.state.phase == Phase::Finished {
            action(
                if job.output.is_some() {
                    fl!("files-hide-output")
                } else {
                    fl!("files-show-output")
                },
                Control::Output(id),
                Some(Message::Job(Event::Output(id))),
            )
        } else {
            action(
                fl!("files-cancel"),
                Control::Cancel(id),
                Some(Message::Job(Event::Cancel(id))),
            )
        };
        let mut content = column![
            identity,
            row![
                primary,
                action(
                    fl!("files-open-folder"),
                    Control::OpenFolder(id),
                    Some(Message::Job(Event::OpenFolder(id))),
                )
            ]
            .spacing(8)
            .wrap(),
        ]
        .spacing(9);
        if let Some(error) = &job.notification_error {
            content = content.push(text(error).size(small).font(font));
        }
        if let Some(output) = &job.output {
            content = content.push(
                row![
                    action(
                        fl!("files-previous-page"),
                        Control::PreviousOutput(id),
                        (job.output_page > 0).then_some(Message::Job(Event::OutputPage(id, false)))
                    ),
                    text(format!("{} / {}", job.output_page + 1, job.output_pages()))
                        .size(small)
                        .font(font),
                    action(
                        fl!("files-next-page"),
                        Control::NextOutput(id),
                        (job.output_page + 1 < job.output_pages())
                            .then_some(Message::Job(Event::OutputPage(id, true)))
                    ),
                    action(
                        fl!("files-copy-all-output"),
                        Control::CopyOutput(id),
                        Some(Message::Job(Event::CopyOutput(id)))
                    ),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .wrap(),
            );
            content = content.push(
                text_editor(output)
                    .id(control_id(window, Control::Text(id)))
                    .height(240)
                    .font(iced::Font::MONOSPACE)
                    .size(small)
                    .padding(10)
                    .style(super::preview_highlights::style)
                    .on_action(move |action| Message::Job(Event::Text(id, action))),
            );
        }
        rows = rows.push(super::row_state::keyed_row(
            view::RowKey::Job(id),
            chrome::card(appearance, content),
        ));
    }
    body = body.push(
        scrollable(rows.padding(iced::Padding {
            bottom: 8.0,
            ..iced::Padding::ZERO
        }))
        .style(canopa_ui::scrollbar)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .spacing(8),
        ))
        .id(scroll_id(window))
        .height(Length::Fill),
    );
    let controls = desktop.jobs.controls();
    let header = |control: &&Control| {
        matches!(
            control,
            Control::NewBrowser | Control::Newest | Control::Older
        )
    };
    let order = controls
        .iter()
        .filter(header)
        .map(|control| control_id(window, *control))
        .chain(file_controls.iter().cloned())
        .chain(device_controls.iter().cloned())
        .chain(
            controls
                .iter()
                .filter(|control| !header(control))
                .map(|control| control_id(window, *control)),
        )
        .chain(std::iter::once(chrome::dismiss_id(window)))
        .collect();
    let reveal = file_controls
        .iter()
        .chain(device_controls.iter())
        .cloned()
        .map(|id| (id, scroll_id(window)))
        .chain(
            controls
                .iter()
                .filter(|control| {
                    !matches!(
                        control,
                        Control::NewBrowser | Control::Newest | Control::Older
                    )
                })
                .map(|control| (control_id(window, *control), scroll_id(window))),
        )
        .collect();
    let appearance = appearance.clone();
    let content = chrome::centered(body, 920.0)
        .id(root_id(window))
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| chrome::window_surface(&appearance, false));
    super::focus::group(content, order, reveal, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displayed_output_pages_preserve_unicode_and_bound_each_layout() {
        let text = "a💾β".repeat(20000);
        let pages: Vec<_> = (0..text.len().div_ceil(OUTPUT_PAGE_BYTES))
            .map(|page| output_page(&text, page))
            .collect();
        assert_eq!(pages.concat(), text);
        assert!(pages.iter().all(|page| page.len() <= OUTPUT_PAGE_BYTES + 3));
        assert_eq!(output_page(&text, usize::MAX), "");
        assert_eq!(output_page("", 0), "");
    }
}
