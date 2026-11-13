//! Iced owns drafts and source views; the core worker owns accepted mutations.
use super::*;
use crate::{
    local_volumes,
    operation_jobs::{self, OperationJobs, Phase, Request, Snapshot},
};
use iced::futures::{StreamExt, channel::mpsc};
use std::{collections::BTreeSet, sync::Mutex};

#[derive(Clone, Debug)]
pub(super) struct Origin {
    pub window: window::Id,
    pub tab: u64,
    pub navigation: u64,
    pub epoch: u64,
    pub directory: PathBuf,
    pub resolved: PathBuf,
    pub selected: Vec<PathBuf>,
    pub focused: Option<PathBuf>,
}

impl Origin {
    pub fn capture(desktop: &Desktop, window: window::Id) -> Option<Arc<Self>> {
        let tab = desktop.active(window).filter(|tab| !tab.loading)?;
        let browser = tab.browser.as_ref()?;
        Some(Arc::new(Self {
            window,
            tab: tab.id,
            navigation: tab.revision,
            epoch: tab.list_epoch,
            directory: browser.path().to_owned(),
            resolved: browser.directory().resolved_path.clone(),
            selected: browser.selected().iter().cloned().collect(),
            focused: browser.focused().map(Path::to_owned),
        }))
    }

    pub fn current(&self, desktop: &Desktop) -> bool {
        desktop.active(self.window).is_some_and(|tab| {
            tab.id == self.tab
                && tab.revision == self.navigation
                && tab.list_epoch == self.epoch
                && !tab.loading
                && tab.browser.as_ref().is_some_and(|browser| {
                    browser.path() == self.directory
                        && browser.focused() == self.focused.as_deref()
                        && browser.selected().iter().eq(self.selected.iter())
                })
        })
    }

    pub(super) fn binding(&self) -> operation_jobs::Source {
        operation_jobs::Source {
            directory: self.directory.clone(),
            resolved: self.resolved.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Submission {
    pub origin: Option<Arc<Origin>>,
    pub owner: window::Id,
    pub dialog: Option<window::Id>,
    pub affected: Vec<PathBuf>,
    pub reveal: Option<PathBuf>,
    pub refresh_all: bool,
}

impl Submission {
    fn affects(&self, directory: &Path, resolved: Option<&Path>) -> bool {
        self.refresh_all
            || self.affected.iter().any(|path| {
                path.parent() == Some(directory)
                    || directory.starts_with(path)
                    || resolved.is_some_and(|directory| {
                        self.origin.as_ref().is_some_and(|origin| {
                            path.strip_prefix(&origin.directory).is_ok_and(|suffix| {
                                let path = origin.resolved.join(suffix);
                                path.parent() == Some(directory) || directory.starts_with(path)
                            })
                        })
                    })
            })
    }
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    InitialRecovery(Result<bool, String>),
    Poll,
    Volumes,
    Admitted(Arc<Submission>, Option<Result<u64, String>>),
    RenamePrepared(Arc<Origin>, Result<operation_jobs::PreparedRename, String>),
    NameOpened(window::Id, Option<window::Id>),
    NameChanged(window::Id, String),
    NameSubmit(window::Id),
    NameCancel(window::Id),
    Undo(window::Id, u64),
    Cancel(u64),
    RetrySave(u64),
    Review(u64),
    Recovery,
    ResumeRecovery(u64, operation_jobs::RecoveryPart),
    ResumeTransfer(u64, operation_jobs::TransferPart),
    AbandonTransfer(u64),
    TransferAbandoned(Result<(), String>),
    DismissRequest(u64),
    RequestDismissed(Result<(), String>),
    Details(u64),
    DetailsOpened(window::Id, Option<window::Id>),
    DetailsClose(window::Id),
    DetailsSection(window::Id, u64, super::operation_details::Section),
    DetailsPage(window::Id, u64, super::operation_details::Section, usize),
    Answer(u64, u64, crate::operations::UndoChoice),
    TransferAnswer(u64, u64, crate::transfers::TransferReply),
    ToggleApply(u64, u64),
    QuestionOpened(window::Id, Option<window::Id>),
    OpenFolder(u64),
    Older,
    Newest,
}

#[derive(Clone)]
struct Notices(Arc<Mutex<Option<mpsc::Receiver<()>>>>, bool);
impl PartialEq for Notices {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) && self.1 == other.1
    }
}
impl Eq for Notices {}
impl std::hash::Hash for Notices {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&Arc::as_ptr(&self.0), state);
        std::hash::Hash::hash(&self.1, state);
    }
}
impl Notices {
    fn new(receiver: mpsc::Receiver<()>, volumes: bool) -> Self {
        Self(Arc::new(Mutex::new(Some(receiver))), volumes)
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::run_with(self.clone(), |key| {
            let receiver = key.0.lock().unwrap_or_else(|e| e.into_inner()).take();
            let volumes = key.1;
            iced::futures::stream::unfold(receiver, move |receiver| async move {
                let mut receiver = receiver?;
                receiver.next().await?;
                Some((
                    Message::FileOperation(if volumes { Event::Volumes } else { Event::Poll }),
                    Some(receiver),
                ))
            })
        })
    }
}

pub(super) struct State {
    pub service: Option<OperationJobs>,
    pub storage: Option<Arc<crate::storage::Storage>>,
    pub snapshot: Snapshot,
    pub error: Option<String>,
    pub storage_error: Option<String>,
    pub before: Option<u64>,
    pub name: Option<super::name_dialog::Dialog>,
    pub preparing_name: Option<Arc<Origin>>,
    pub question: Option<super::question_dialog::Dialog>,
    pub details: Option<super::operation_details::Dialog>,
    pub seen_questions: BTreeSet<(u64, u64)>,
    pub notification_errors: BTreeMap<u64, String>,
    pub notification_error: Option<String>,
    initial_review: Option<Result<bool, String>>,
    initial_review_consumed: bool,
    runtime: Option<local_volumes::LocalOperations>,
    notices: Option<Notices>,
    volume_notices: Option<Notices>,
    admissions: usize,
    inflight: BTreeMap<u64, Arc<Submission>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            service: None,
            storage: None,
            snapshot: Snapshot {
                initializing: false,
                error: None,
                jobs: Vec::new(),
                undo: None,
                pending_attention: 0,
            },
            error: None,
            storage_error: None,
            before: None,
            name: None,
            preparing_name: None,
            question: None,
            details: None,
            seen_questions: BTreeSet::new(),
            notification_errors: BTreeMap::new(),
            notification_error: None,
            initial_review: None,
            initial_review_consumed: false,
            runtime: None,
            notices: None,
            volume_notices: None,
            admissions: 0,
            inflight: BTreeMap::new(),
        }
    }
}

impl State {
    pub(super) fn local_runtime(&self) -> Option<&local_volumes::LocalOperations> {
        self.runtime.as_ref()
    }
    pub fn recovery_presented(&mut self) {
        self.initial_review_consumed = true;
        self.initial_review = None;
    }

    pub fn ready(&self) -> bool {
        self.service.is_some()
            && self.storage.is_some()
            && !self.snapshot.initializing
            && self.snapshot.error.is_none()
            && self.storage_error.is_none()
    }
    pub fn has_active(&self) -> bool {
        self.admissions > 0
            || self
                .snapshot
                .jobs
                .iter()
                .any(|job| job.phase.holds_application())
    }
    pub fn holds_application(&self) -> bool {
        self.admissions > 0
            || self.service.as_ref().map_or_else(
                || self.snapshot.holds_application(),
                |service| service.snapshot().holds_application(),
            )
    }
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(
            [self.notices.as_ref(), self.volume_notices.as_ref()]
                .into_iter()
                .flatten()
                .map(Notices::subscription),
        )
    }
    pub fn page(&self) -> Vec<&operation_jobs::JobSnapshot> {
        let mut jobs: Vec<_> = self
            .snapshot
            .jobs
            .iter()
            .filter(|job| self.before.is_none_or(|before| job.id < before))
            .collect();
        jobs.sort_by_key(|job| std::cmp::Reverse(job.id));
        jobs.truncate(20);
        jobs
    }
    pub fn has_older(&self) -> bool {
        self.page()
            .last()
            .is_some_and(|last| self.snapshot.jobs.iter().any(|job| job.id < last.id))
    }
    /// The newest operation while it is under way or needs the person:
    /// queued, running, waiting, under review or unsaved. Once it ends, its
    /// result is told where it was asked for and Operations keeps it.
    pub fn summary(&self) -> Option<String> {
        self.snapshot
            .jobs
            .iter()
            .max_by_key(|job| job.id)
            .filter(|job| {
                matches!(
                    job.phase,
                    Phase::Queued | Phase::Running | Phase::Waiting | Phase::AwaitingReview
                ) || job.phase.needs_save()
            })
            .map(|job| {
                fl!(
                    "files-operation-summary",
                    status = status(job.phase),
                    operation = crate::history_labels::display(&job.title)
                )
            })
    }
    pub fn starting(&self) -> bool {
        self.snapshot.initializing
    }
    /// The newest operation's state for a screen reader, and whether it needs
    /// attention now: "Copy 3 items: Completed", "…: Could not finish. …".
    pub fn announcement(&self) -> Option<(String, bool)> {
        let job = self.snapshot.jobs.iter().max_by_key(|job| job.id)?;
        let title = crate::history_labels::title(&job.title);
        let failed = matches!(
            job.phase,
            Phase::Failed | Phase::StartSaveFailed | Phase::SaveFailed
        );
        let phase = super::file_operation_rows::phase_label(job);
        let text = match (&job.error, failed) {
            (Some(error), true) => fl!(
                "files-operation-announcement-error",
                operation = title,
                status = phase,
                error = error.clone()
            ),
            _ => fl!(
                "files-operation-announcement",
                operation = title,
                status = phase
            ),
        };
        Some((text, failed))
    }
}

pub(super) fn status(phase: Phase) -> String {
    match phase {
        Phase::Queued => fl!("files-phase-queued"),
        Phase::Running => fl!("files-phase-running"),
        Phase::Waiting => fl!("files-device-needs-decision"),
        Phase::AwaitingReview => fl!("files-phase-awaiting-review"),
        Phase::Interrupted => fl!("files-phase-interrupted"),
        Phase::Failed => fl!("files-phase-failed"),
        Phase::Stopped => fl!("files-phase-stopped"),
        Phase::StartSaveFailed => fl!("files-phase-start-save-failed"),
        Phase::SaveFailed => fl!("files-phase-save-failed"),
        Phase::Finished => fl!("files-device-finished"),
    }
}

impl Desktop {
    pub(super) fn attach_file_operations(
        &mut self,
        runtime: Result<local_volumes::LocalOperations, String>,
    ) -> Task<Message> {
        match runtime {
            Ok(runtime) => {
                let service = runtime.jobs().clone();
                self.file_operations.notices = runtime
                    .take_operation_notices()
                    .map(|receiver| Notices::new(receiver, false));
                self.file_operations.volume_notices = runtime
                    .take_notices()
                    .map(|receiver| Notices::new(receiver, true));
                let attention = runtime.attention_snapshot();
                self.file_operations.snapshot = attention.queue;
                self.file_operations.notification_errors = attention.notification_errors;
                self.file_operations.notification_error = attention.error;
                self.file_operations.storage = service.storage();
                self.file_operations.storage_error = runtime.storage_error();
                let review = service.needs_recovery_review();
                self.file_operations.service = Some(service);
                self.file_operations.runtime = Some(runtime);
                let devices = self.poll_devices();
                return Task::batch([
                    devices,
                    match review {
                        Ok(receiver) => Task::perform(
                            async move {
                                receiver
                                    .await
                                    .unwrap_or_else(|_| Err(fl!("files-recovery-reader-stopped")))
                            },
                            |result| Message::FileOperation(Event::InitialRecovery(result)),
                        ),
                        Err(error) => Task::done(Message::FileOperation(Event::InitialRecovery(
                            Err(error.to_string()),
                        ))),
                    },
                ]);
            }
            Err(error) => {
                self.file_operations.error = Some(error);
            }
        }
        Task::none()
    }

    pub(super) fn present_initial_recovery(&mut self) -> Task<Message> {
        if self.file_operations.initial_review_consumed || self.windows.is_empty() {
            return Task::none();
        }
        let Some(result) = self.file_operations.initial_review.take() else {
            return Task::none();
        };
        self.file_operations.recovery_presented();
        match result {
            Ok(true) if self.recovery_window.is_none() => self.show_recovery(),
            Err(error) => {
                self.file_operations.error = Some(error);
                self.show_operations()
            }
            _ => Task::none(),
        }
    }

    pub(super) fn submit_file_operation(
        &mut self,
        request: Request,
        submission: Submission,
    ) -> Task<Message> {
        let Some(service) = &self.file_operations.service else {
            return self.show_operations();
        };
        let result = service.submit(
            request,
            submission.origin.as_ref().map(|origin| origin.binding()),
        );
        self.await_file_admission(result, submission)
    }

    pub(super) fn submit_clipboard_paste(
        &mut self,
        prepared: operation_jobs::PreparedSelection,
        submission: Submission,
    ) -> Task<Message> {
        let Some(service) = &self.file_operations.service else {
            return self.show_operations();
        };
        let Some(origin) = &submission.origin else {
            return Task::none();
        };
        let result =
            service.paste_clipboard(prepared, origin.directory.clone(), Some(origin.binding()));
        self.await_file_admission(result, submission)
    }

    pub(super) fn submit_file_transfer(
        &mut self,
        request: crate::transfers::TransferRequest,
        sources: crate::storage::Snapshot,
        submission: Submission,
    ) -> Task<Message> {
        let Some(service) = &self.file_operations.service else {
            return self.show_operations();
        };
        let destination = submission.origin.as_ref().map(|origin| origin.binding());
        let result = service.submit_transfer(request, sources, destination);
        self.await_file_admission(result, submission)
    }

    pub(super) fn submit_drag_drop(
        &mut self,
        id: u64,
        choice: operation_jobs::DropChoice,
        submission: Submission,
    ) -> Task<Message> {
        let result = self
            .file_operations
            .service
            .as_ref()
            .ok_or_else(|| io::Error::other(fl!("files-operation-queue-unavailable-short")))
            .and_then(|service| service.submit_drop(choice));
        self.file_operations.admissions += 1;
        let submission = Arc::new(submission);
        match result {
            Ok(receiver) => Task::perform(async move { receiver.await.ok() }, move |result| {
                Message::Drag(super::drag_ui::Event::Admitted(
                    id,
                    submission.clone(),
                    result,
                ))
            }),
            Err(error) => Task::done(Message::Drag(super::drag_ui::Event::Admitted(
                id,
                submission,
                Some(Err(error.to_string())),
            ))),
        }
    }

    pub(super) fn await_file_admission(
        &mut self,
        result: io::Result<futures_channel::oneshot::Receiver<Result<u64, String>>>,
        submission: Submission,
    ) -> Task<Message> {
        match result {
            Ok(receiver) => {
                self.file_operations.admissions += 1;
                let submission = Arc::new(submission);
                Task::perform(async move { receiver.await.ok() }, move |result| {
                    Message::FileOperation(Event::Admitted(submission.clone(), result))
                })
            }
            Err(error) => {
                self.file_admission_error(&submission, error.to_string());
                self.exit_if_empty()
            }
        }
    }

    fn file_admission_error(&mut self, submission: &Submission, error: String) {
        if let Some(dialog) = self
            .file_operations
            .name
            .as_mut()
            .filter(|dialog| Some(dialog.window) == submission.dialog)
        {
            dialog.busy = false;
            dialog.error = Some(error);
        } else if let Some(dialog) = self
            .trash_restore
            .as_mut()
            .filter(|dialog| Some(dialog.window) == submission.dialog)
        {
            dialog.busy = false;
            dialog.error = Some(error);
        } else if let Some(dialog) = self
            .recovery_destination
            .as_mut()
            .filter(|d| Some(d.window) == submission.dialog)
        {
            dialog.busy = false;
            dialog.job = None;
            dialog.error = Some(error);
        } else {
            if let Some(recovery) = self
                .recovery_window
                .as_mut()
                .filter(|w| w.window == submission.owner)
            {
                recovery.error = Some(error.clone());
            }
            if let Some(trash) = self
                .trash_window
                .as_mut()
                .filter(|trash| trash.window == submission.owner)
            {
                trash.error = Some(error.clone());
            }
            self.file_operations.error = Some(error);
        }
    }

    pub(super) fn file_operation_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::InitialRecovery(result) => {
                if self.file_operations.initial_review_consumed {
                    return Task::none();
                }
                self.file_operations.initial_review = Some(result);
                return self.present_initial_recovery();
            }
            Event::Poll => return self.file_operation_poll(),
            Event::Volumes => {
                if let Some(trash) = &self.trash_window {
                    trash.refresh();
                }
                if let Some(recovery) = &self.recovery_window {
                    recovery.refresh();
                }
                self.file_operations.storage_error = self
                    .file_operations
                    .runtime
                    .as_ref()
                    .and_then(local_volumes::LocalOperations::storage_error);
                return Task::batch([self.poll_devices(), self.file_operation_poll()]);
            }
            Event::Admitted(submission, result) => {
                self.file_operations.admissions = self.file_operations.admissions.saturating_sub(1);
                match result
                    .unwrap_or_else(|| Err(fl!("files-operation-worker-stopped-acceptance")))
                {
                    Ok(id) => {
                        self.file_operations.inflight.insert(id, submission.clone());
                        let close = submission.dialog.map_or_else(Task::none, |dialog| {
                            if let Some(draft) = self
                                .recovery_destination
                                .as_mut()
                                .filter(|d| d.window == dialog)
                            {
                                draft.job = Some(id);
                                Task::none()
                            } else if self
                                .trash_restore
                                .as_ref()
                                .is_some_and(|draft| draft.window == dialog)
                            {
                                self.close_trash_restore(dialog)
                            } else {
                                self.finish_name_dialog(dialog)
                            }
                        });
                        let show = if self.windows.is_empty()
                            && self.operations_window.is_none()
                            && self.trash_window.is_none()
                            && self.recovery_window.is_none()
                        {
                            self.show_operations()
                        } else {
                            Task::none()
                        };
                        return Task::batch([close, show, self.file_operation_poll()]);
                    }
                    Err(error) => {
                        self.file_admission_error(&submission, error);
                    }
                }
            }
            Event::RenamePrepared(origin, result) => return self.rename_prepared(origin, result),
            Event::NameOpened(id, opened) => return self.name_opened(id, opened),
            Event::NameChanged(id, value) => {
                if let Some(dialog) = self
                    .file_operations
                    .name
                    .as_mut()
                    .filter(|dialog| dialog.window == id && !dialog.busy)
                {
                    dialog.value = value;
                    dialog.error = None;
                }
            }
            Event::NameSubmit(id) => return self.submit_name_dialog(id),
            Event::NameCancel(id) => return self.close_name_dialog(id, false),
            Event::Undo(owner, expected) => {
                if self.file_operations.ready() {
                    return self.submit_file_operation(
                        Request::Undo { expected },
                        Submission {
                            origin: None,
                            owner,
                            dialog: None,
                            affected: Vec::new(),
                            reveal: None,
                            refresh_all: true,
                        },
                    );
                }
            }
            Event::Cancel(id) => {
                if let Some(service) = &self.file_operations.service
                    && let Err(error) = service.cancel(id)
                {
                    self.file_operations.error = Some(error.to_string());
                }
            }
            Event::RetrySave(id) => {
                if let Some(service) = &self.file_operations.service
                    && let Err(error) = service.retry_save(id)
                {
                    self.file_operations.error = Some(error.to_string());
                }
            }
            Event::Recovery => return self.show_recovery(),
            Event::ResumeTransfer(id, part) => {
                let Some(owner) = self.operations_window else {
                    return self.show_operations();
                };
                let Some(service) = &self.file_operations.service else {
                    return Task::none();
                };
                return self.await_file_admission(
                    service.resume_transfer(id, part),
                    Submission {
                        origin: None,
                        owner,
                        dialog: None,
                        affected: Vec::new(),
                        reveal: None,
                        refresh_all: true,
                    },
                );
            }
            Event::AbandonTransfer(id) => {
                let Some(service) = &self.file_operations.service else {
                    return Task::none();
                };
                match service.abandon_transfer(id) {
                    Ok(receiver) => {
                        return Task::perform(
                            async move {
                                receiver
                                    .await
                                    .unwrap_or_else(|_| Err(fl!("files-abandon-not-saved")))
                            },
                            |result| Message::FileOperation(Event::TransferAbandoned(result)),
                        );
                    }
                    Err(error) => self.file_operations.error = Some(error.to_string()),
                }
            }
            Event::TransferAbandoned(result) => {
                self.file_operations.error = result.err();
            }
            Event::DismissRequest(id) => {
                let Some(service) = &self.file_operations.service else {
                    return Task::none();
                };
                match service.dismiss_request(id) {
                    Ok(receiver) => {
                        return Task::perform(
                            async move {
                                receiver
                                    .await
                                    .unwrap_or_else(|_| Err(fl!("files-dismissal-not-saved")))
                            },
                            |result| Message::FileOperation(Event::RequestDismissed(result)),
                        );
                    }
                    Err(error) => self.file_operations.error = Some(error.to_string()),
                }
            }
            Event::RequestDismissed(result) => {
                self.file_operations.error = result.err();
            }
            Event::ResumeRecovery(id, part) => {
                let Some(owner) = self.operations_window else {
                    return self.show_operations();
                };
                let Some(service) = &self.file_operations.service else {
                    return Task::none();
                };
                let result = service.resume_recovery(id, part);
                return self.await_file_admission(
                    result,
                    Submission {
                        origin: None,
                        owner,
                        dialog: None,
                        affected: Vec::new(),
                        reveal: None,
                        refresh_all: true,
                    },
                );
            }
            Event::Review(id) => return self.show_file_question(id, true),
            Event::Details(id) => return self.show_operation_details(id),
            Event::DetailsOpened(id, opened) => return self.operation_details_opened(id, opened),
            Event::DetailsClose(id) => return self.close_operation_details(id),
            Event::DetailsSection(window, job, section) => {
                return self.operation_details_section(window, job, section);
            }
            Event::DetailsPage(window, job, section, page) => {
                return self.operation_details_page(window, job, section, page);
            }
            Event::Answer(id, generation, choice) => {
                return self.answer_file_question(
                    id,
                    generation,
                    super::question_dialog::Reply::Undo(choice),
                );
            }
            Event::TransferAnswer(id, generation, choice) => {
                return self.answer_file_question(
                    id,
                    generation,
                    super::question_dialog::Reply::Transfer(choice),
                );
            }
            Event::ToggleApply(id, generation) => {
                if let Some(dialog) = self.file_operations.question.as_mut().filter(|dialog| {
                    dialog.job == id
                        && dialog.generation == generation
                        && matches!(
                            dialog.question,
                            operation_jobs::Question::Transfer(_)
                                | operation_jobs::Question::Recovery(_)
                        )
                }) {
                    dialog.apply = !dialog.apply;
                }
            }
            Event::QuestionOpened(id, opened) => return self.file_question_opened(id, opened),
            Event::OpenFolder(id) => {
                if let Some(path) = self
                    .file_operations
                    .snapshot
                    .jobs
                    .iter()
                    .find(|job| job.id == id)
                    .and_then(|job| job.paths.first())
                    .and_then(|path| path.parent())
                {
                    return self.new_window(vec![path.to_owned()]);
                }
            }
            Event::Older => {
                if let Some(last) = self.file_operations.page().last() {
                    self.file_operations.before = Some(last.id);
                }
            }
            Event::Newest => self.file_operations.before = None,
        }
        self.exit_if_empty()
    }

    fn file_operation_poll(&mut self) -> Task<Message> {
        if let Some(service) = &self.file_operations.service {
            if let Some(runtime) = &self.file_operations.runtime {
                let attention = runtime.attention_snapshot();
                self.file_operations.snapshot = attention.queue;
                self.file_operations.notification_errors = attention.notification_errors;
                self.file_operations.notification_error = attention.error;
            } else {
                self.file_operations.snapshot = service.snapshot();
            }
            self.file_operations.storage = service.storage();
        }
        let mut completed = Vec::new();
        for job in &self.file_operations.snapshot.jobs {
            let stopped = matches!(
                job.phase,
                Phase::Finished
                    | Phase::Failed
                    | Phase::Stopped
                    | Phase::Interrupted
                    | Phase::SaveFailed
            );
            let submission = stopped
                .then(|| self.file_operations.inflight.remove(&job.id))
                .flatten();
            if let Some(submission) = submission {
                completed.push((job.clone(), submission));
            }
        }
        let mut tasks = Vec::new();
        for (job, submission) in completed {
            if let Some(recovery) = self.recovery_window.as_mut() {
                if submission.owner == recovery.window {
                    recovery.error = job.error.clone();
                }
                recovery.refresh();
            }
            tasks.push(self.recovery_destination_completed(&job));
            if let Some(trash) = self.trash_window.as_mut() {
                if submission.owner == trash.window {
                    trash.error = job.error.clone();
                }
                if job.trash.is_some()
                    || job.restore.is_some()
                    || job.delete.is_some()
                    || submission.refresh_all
                {
                    trash.refresh();
                }
            }
            let mut refreshed = (*submission).clone();
            if let Some(report) = &job.transfer {
                refreshed.reveal = report.destinations.first().cloned();
            }
            tasks.push(self.refresh_after_operation(&refreshed));
            if let Some(report) = &job.trash
                && !report.unavailable.is_empty()
            {
                tasks.push(self.offer_permanent_deletion(&submission, report.unavailable.clone()));
            }
            // A failure, or what a stop left unfinished, is reported where
            // the operation was asked for.
            if let Some(error) = &job.error
                && let Some(origin) = &submission.origin
                && let Some(tab) = self
                    .windows
                    .get_mut(&origin.window)
                    .and_then(|window| window.tabs.iter_mut().find(|tab| tab.id == origin.tab))
            {
                tab.error = Some(error.clone());
                tab.error_job = Some(job.id);
            }
        }
        // An operation's failure leaves the status bar once it is cancelled,
        // dismissed, retried or otherwise no longer failing.
        let jobs = &self.file_operations.snapshot.jobs;
        for tab in self
            .windows
            .values_mut()
            .flat_map(|window| window.tabs.iter_mut())
        {
            let Some(id) = tab.error_job else {
                continue;
            };
            let failing = jobs.iter().find(|job| job.id == id).is_some_and(|job| {
                job.error.is_some()
                    && matches!(
                        job.phase,
                        Phase::Failed | Phase::Interrupted | Phase::SaveFailed | Phase::Stopped
                    )
            });
            if !failing {
                tab.error = None;
                tab.error_job = None;
            }
        }
        let retained: BTreeSet<_> = self
            .file_operations
            .snapshot
            .jobs
            .iter()
            .map(|job| job.id)
            .collect();
        self.file_operations
            .seen_questions
            .retain(|(id, _)| retained.contains(id));
        tasks.push(self.sync_clipboard_cut());
        tasks.push(self.sync_file_question());
        tasks.push(self.auto_file_question());
        tasks.push(self.exit_if_empty());
        Task::batch(tasks)
    }

    fn refresh_after_operation(&mut self, submission: &Submission) -> Task<Message> {
        let reveal = submission
            .origin
            .as_ref()
            .filter(|origin| origin.current(self))
            .map(|origin| (origin.window, origin.tab));
        let mut refresh = Vec::new();
        let mut columns = Vec::new();
        for (id, window) in &mut self.windows {
            for (index, tab) in window.tabs.iter_mut().enumerate() {
                if tab.loading {
                    // A pending destination may be an alias whose resolved
                    // binding is not known yet. Keep its navigation and rescan
                    // that destination once its first snapshot arrives.
                    tab.needs_refresh = true;
                    continue;
                }
                let affected = submission.affects(
                    tab.path(),
                    tab.browser
                        .as_ref()
                        .map(|browser| browser.directory().resolved_path.as_path()),
                );
                if affected {
                    if index == window.active {
                        refresh.push((
                            *id,
                            reveal
                                .filter(|(window, tab_id)| window == id && *tab_id == tab.id)
                                .and(submission.reveal.clone()),
                        ));
                    } else {
                        tab.needs_refresh = true;
                    }
                }
                if submission.refresh_all
                    || tab.columns.iter().any(|(path, column)| {
                        submission.affects(
                            path,
                            column.result.as_ref().and_then(|result| {
                                result
                                    .as_ref()
                                    .ok()
                                    .map(|directory| directory.resolved_path.as_path())
                            }),
                        )
                    })
                {
                    tab.columns.clear();
                    if index == window.active {
                        columns.push(*id);
                    }
                }
            }
        }
        let mut tasks = Vec::new();
        for (window, selected) in refresh {
            tasks.push(self.refresh_operation_browser(window, selected));
        }
        for window in columns {
            tasks.push(self.load_columns(window));
        }
        Task::batch(tasks)
    }

    pub(super) fn refresh_operation_browser(
        &mut self,
        window: window::Id,
        selected: Option<PathBuf>,
    ) -> Task<Message> {
        if self.active(window).is_some_and(|tab| {
            tab.search
                .as_ref()
                .is_some_and(|search| !search.query.is_empty())
        }) {
            return self.restart_search(window);
        }
        let home = self.home.clone();
        let Some(tab) = self.active_mut(window) else {
            return Task::none();
        };
        let path = tab.path().to_owned();
        tab.needs_refresh = false;
        tab.supersede_watch();
        tab.revision += 1;
        tab.scan_cancel.cancel();
        tab.scan_cancel = Cancellation::default();
        tab.loading = true;
        tab.columns.clear();
        let (id, revision, cancel) = (tab.id, tab.revision, tab.scan_cancel.clone());
        Task::perform(
            async move { scan(path, &home, None, selected, &cancel).map_err(|error| error.to_string()) },
            move |result| {
                Message::Loaded(window, id, revision, Navigation::Refresh, Arc::new(result))
            },
        )
    }

    pub(super) fn close_operation_source(&mut self, source: window::Id) -> Task<Message> {
        let mut tasks = Vec::new();
        if let Some(dialog) = self.delete_dialog.take_if(|dialog| dialog.owner == source) {
            tasks.push(window::close(dialog.window));
        }
        if let Some(dialog) = self
            .file_operations
            .name
            .as_ref()
            .filter(|dialog| dialog.origin.window == source)
        {
            let id = dialog.window;
            self.file_operations.name = None;
            tasks.push(window::close(id));
        }
        if let Some(dialog) = self
            .file_operations
            .question
            .as_ref()
            .filter(|dialog| dialog.parent == source)
        {
            let id = dialog.window;
            self.file_operations.question = None;
            tasks.push(window::close(id));
        }
        Task::batch(tasks)
    }

    pub(super) fn file_owner(&self, id: u64) -> Option<window::Id> {
        self.file_operations
            .inflight
            .get(&id)
            .map(|submission| submission.owner)
    }
}
