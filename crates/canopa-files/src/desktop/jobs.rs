//! One shared operation queue. Workers own filesystem work; windows only observe it.
use super::{Desktop, clipboard, tab::Tab};
use crate::{
    Cancellation,
    operations::{
        OperationRequest, OperationSummary, UndoChoice, UndoConflict, UndoItem, UndoOutcome,
    },
    transfers::{
        ConflictChoice, TransferAccess, TransferContinuation, TransferEvent, TransferKind,
        TransferOutcome, TransferReply, TransferRequest,
    },
};
use gtk::{gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    path::PathBuf,
    rc::{Rc, Weak},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct Queue {
    pending: RefCell<VecDeque<(Rc<Job>, Task)>>,
    busy: Cell<bool>,
    entries: RefCell<Vec<Rc<Job>>>,
    panel: RefCell<Option<gtk::Box>>,
    window: RefCell<Option<gtk::ApplicationWindow>>,
    undo_id: Cell<Option<u64>>,
}
impl Queue {
    pub(super) fn has_active(&self) -> bool {
        self.entries.borrow().iter().any(|job| job.active.get())
    }
}

pub(super) fn device_finished(app: &Rc<Desktop>) {
    dispatch(app);
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
enum Task {
    Action(
        crate::actions::CustomAction,
        #[serde(with = "crate::path_serde::sequence")] Vec<PathBuf>,
        #[serde(with = "crate::path_serde")] PathBuf,
    ),
    Mutation(Option<OperationRequest>),
    UndoOperation(u64),
    Trash(#[serde(with = "crate::path_serde::sequence")] Vec<PathBuf>),
    Restore(
        Box<crate::trash::TrashItem>,
        #[serde(with = "crate::path_serde::optional")] Option<PathBuf>,
    ),
    Recover(
        Box<crate::operations::RecoveryItem>,
        #[serde(with = "crate::path_serde")] PathBuf,
    ),
    ResumeRecover(Box<crate::operations::RecoveryCopyContinuation>),
    RetryRecovery,
    Transfer(
        TransferRequest,
        #[serde(skip)] Option<u64>,
        Option<TransferContinuation>,
    ),
}
impl Task {
    fn storage_paths(
        &self,
        engine: Option<&crate::operations::OperationEngine>,
        data_home: &std::path::Path,
    ) -> std::io::Result<Vec<super::storage::Access>> {
        use super::storage::Access;
        let mut paths = Vec::new();
        match self {
            Self::Action(_, selected, directory) => {
                paths.push(Access::folder(directory.clone()));
                paths.extend(selected.iter().cloned().map(Access::entry));
            }
            Self::Mutation(Some(crate::operations::OperationRequest::CreateFolder { path })) => {
                paths.push(Access::entry(path.clone()))
            }
            Self::Mutation(Some(crate::operations::OperationRequest::Rename {
                source,
                destination,
            })) => {
                paths.extend([
                    Access::entry(source.clone()),
                    Access::entry(destination.clone()),
                ]);
            }
            Self::Mutation(None) | Self::UndoOperation(_) => {
                if let Some(engine) = engine {
                    let expected = if let Self::UndoOperation(id) = self {
                        Some(*id)
                    } else {
                        None
                    };
                    paths.extend(
                        engine
                            .undo_storage_paths(expected)?
                            .into_iter()
                            .map(Access::entry),
                    );
                }
            }
            Self::Trash(selected) => {
                paths.extend(selected.iter().cloned().map(Access::entry));
                paths.push(Access::folder(data_home.to_owned()));
            }
            Self::Restore(item, destination) => {
                paths.push(Access::entry(item.path.clone()));
                if let Some(folder) = destination {
                    paths.push(Access::folder(folder.clone()));
                } else if let Some(original) = &item.original {
                    paths.push(Access::entry(original.clone()));
                }
            }
            Self::Recover(item, destination) => {
                if let Some(engine) = engine {
                    paths.push(Access::entry(engine.recovery_copy_source_path(item)?));
                }
                paths.push(Access::entry(destination.clone()));
            }
            Self::ResumeRecover(plan) => {
                if let Some(engine) = engine {
                    paths.push(Access::entry(
                        engine.recovery_copy_source_path(plan.item())?,
                    ));
                }
                paths.extend(plan.storage_destinations().map(Access::entry));
            }
            Self::Transfer(request, _, continuation) => {
                paths.push(Access::folder(request.destination.clone()));
                if let Some(continuation) = continuation {
                    paths.extend(continuation.storage_destinations().map(Access::entry));
                }
            }
            Self::RetryRecovery => {}
        }
        if !matches!(
            self,
            Self::Action(..) | Self::Recover(..) | Self::ResumeRecover(..)
        ) && let Some(engine) = engine
        {
            for interrupted in engine.interrupted_operations(&Cancellation::default())? {
                paths.extend(interrupted.paths.into_iter().map(Access::entry));
            }
        }
        Ok(paths)
    }
}
enum ResultBody {
    PreparationFailed(Box<Task>, String),
    Reconciled,
    Action(crate::actions::ActionOutcome),
    Trash(crate::trash::TrashOutcome),
    Mutation(OperationSummary),
    Undo(UndoOutcome),
    Transfer(Box<TransferOutcome<Task>>, Option<(u64, Vec<PathBuf>)>),
}
struct Job {
    title: String,
    row: gtk::Box,
    status: gtk::Label,
    progress: gtk::ProgressBar,
    details: gtk::Label,
    cancel_button: gtk::Button,
    review_button: gtk::Button,
    undo_button: gtk::Button,
    retry_button: gtk::Button,
    resume_button: gtk::Button,
    retry: RefCell<Option<Task>>,
    remainder: RefCell<Option<Task>>,
    cancellation: Cancellation,
    active: Cell<bool>,
    operation: Cell<Option<u64>>,
    origin: Weak<Tab>,
    owner: Option<glib::WeakRef<gtk::Window>>,
    hold: RefCell<Option<gio::ApplicationHoldGuard>>,
    prompt: RefCell<Option<Prompt>>,
    followup_until: Cell<Option<Instant>>,
    drop: RefCell<Option<(gtk::gdk::Drop, gtk::gdk::DragAction)>>,
    storage: super::storage::Snapshot,
    source_storage: Option<super::storage::Snapshot>,
}
#[derive(Clone)]
enum PromptEvent {
    Transfer(TransferEvent),
    Recovery(TransferEvent),
    Undo(UndoConflict),
}
enum PromptContent {
    Transfer(TransferEvent),
    Unavailable(UndoConflict),
}
enum PromptReply {
    Transfer(mpsc::Sender<TransferReply>),
    Undo(mpsc::Sender<UndoChoice>),
}
struct Prompt {
    event: PromptEvent,
    reply: PromptReply,
    dialog: Option<gtk::Window>,
}
struct Question {
    event: PromptEvent,
    reply: PromptReply,
}

pub(super) fn initialize(app: &Rc<Desktop>) {
    let action = gio::SimpleAction::new("operations", None);
    let weak = Rc::downgrade(app);
    action.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            show(&app);
        }
    });
    app.application.add_action(&action);
    let action = gio::SimpleAction::new("undo-operation", Some(&u64::static_variant_type()));
    let weak = Rc::downgrade(app);
    action.connect_activate(move |_, target| {
        if let Some(app) = weak.upgrade()
            && let Some(id) = target.and_then(|value| value.get::<u64>())
        {
            enqueue(
                &app,
                Weak::new(),
                String::from("Undo from notification"),
                Task::UndoOperation(id),
            );
            show(&app);
        }
    });
    app.application.add_action(&action);
}

pub(super) fn ready(app: &Rc<Desktop>, id: Option<u64>) {
    app.jobs.undo_id.set(id);
    if let Some(window) = app.recovery_window.borrow().clone() {
        window.refresh();
    }
    dispatch(app);
}

pub(super) fn unavailable(app: &Rc<Desktop>, message: &str) {
    for (job, _) in app.jobs.pending.borrow_mut().drain(..) {
        job.status.set_label("Failed");
        job.details.set_label(message);
        job.details.set_visible(true);
        job.cancel_button.set_visible(false);
        job.progress.set_visible(false);
        job.active.set(false);
        job.hold.borrow_mut().take();
        finish_drop(&job, false);
    }
}

fn finish_drop(job: &Job, success: bool) {
    if let Some((drop, action)) = job.drop.borrow_mut().take() {
        drop.finish(if success {
            action
        } else {
            gtk::gdk::DragAction::empty()
        });
    }
}

pub(super) fn mutation(app: &Rc<Desktop>, origin: &Rc<Tab>, request: Option<OperationRequest>) {
    if request.is_none() && app.jobs.busy.get() {
        origin.show_error("Wait for the current operation to finish, or cancel it in Operations, before using Undo.");
        return;
    }
    let title = match &request {
        Some(OperationRequest::CreateFolder { .. }) => String::from("Create folder"),
        Some(OperationRequest::Rename { .. }) => String::from("Rename"),
        None => app.undo_label.borrow().clone(),
    };
    enqueue(app, Rc::downgrade(origin), title, Task::Mutation(request));
}

pub(super) fn transfer(
    app: &Rc<Desktop>,
    origin: Weak<Tab>,
    request: TransferRequest,
    token: Option<u64>,
    source: Option<super::storage::Snapshot>,
) {
    let title = transfer_title(&request);
    enqueue_inner(
        app,
        origin,
        title,
        Task::Transfer(request, token, None),
        None,
        source,
    );
}
fn transfer_title(request: &TransferRequest) -> String {
    format!(
        "{} {} item{} to {}",
        if request.kind == TransferKind::Copy {
            "Copy"
        } else {
            "Move"
        },
        request.sources.len(),
        if request.sources.len() == 1 { "" } else { "s" },
        request
            .destination
            .file_name()
            .unwrap_or(request.destination.as_os_str())
            .to_string_lossy()
    )
}
pub(super) fn dropped(
    app: &Rc<Desktop>,
    origin: Weak<Tab>,
    request: TransferRequest,
    drop: gtk::gdk::Drop,
    action: gtk::gdk::DragAction,
) -> bool {
    if let Some(job) = enqueue(
        app,
        origin,
        transfer_title(&request),
        Task::Transfer(request, None, None),
    ) {
        *job.drop.borrow_mut() = Some((drop, action));
        true
    } else {
        false
    }
}

pub(super) fn trash(app: &Rc<Desktop>, origin: Weak<Tab>, paths: Vec<PathBuf>) {
    let title = format!("Move {} items to Trash", paths.len());
    enqueue(app, origin, title, Task::Trash(paths));
}
pub(super) fn custom_action(
    app: &Rc<Desktop>,
    origin: &Rc<Tab>,
    action: crate::actions::CustomAction,
) {
    let Some(directory) = origin.path() else {
        return;
    };
    let title = format!("Run {}", action.name);
    enqueue(
        app,
        Rc::downgrade(origin),
        title,
        Task::Action(action, origin.selection_paths(), directory),
    );
    show(app);
}
pub(super) fn restore(
    app: &Rc<Desktop>,
    item: crate::trash::TrashItem,
    destination: Option<PathBuf>,
) {
    let title = format!("Restore {}", item.name().to_string_lossy());
    enqueue(
        app,
        Weak::new(),
        title,
        Task::Restore(Box::new(item), destination),
    );
}
pub(super) fn undo_latest(app: &Rc<Desktop>) -> Result<(), &'static str> {
    if app.jobs.busy.get() {
        return Err("Wait for the current operation to finish, or cancel it in Operations.");
    }
    let id = app.jobs.undo_id.get().ok_or("Nothing to undo")?;
    enqueue(
        app,
        Weak::new(),
        app.undo_label.borrow().clone(),
        Task::UndoOperation(id),
    );
    Ok(())
}

pub(super) fn recover_copy(
    app: &Rc<Desktop>,
    item: crate::operations::RecoveryItem,
    destination: PathBuf,
) {
    let title = format!("Recover {}", item.name().to_string_lossy());
    enqueue(
        app,
        Weak::new(),
        title,
        Task::Recover(Box::new(item), destination),
    );
    show(app);
}

pub(super) fn retry_recovery(app: &Rc<Desktop>) {
    enqueue(
        app,
        Weak::new(),
        "Retry interrupted operation".into(),
        Task::RetryRecovery,
    );
}

fn enqueue(app: &Rc<Desktop>, origin: Weak<Tab>, title: String, task: Task) -> Option<Rc<Job>> {
    enqueue_inner(app, origin, title, task, None, None)
}

fn enqueue_inner(
    app: &Rc<Desktop>,
    origin: Weak<Tab>,
    title: String,
    task: Task,
    previous: Option<&Job>,
    source: Option<super::storage::Snapshot>,
) -> Option<Rc<Job>> {
    if let Some(error) = app.operation_error.borrow().as_ref()
        && !matches!(task, Task::Action(..))
    {
        if let Some(origin) = origin.upgrade() {
            origin.show_error(&format!("File operations are unavailable: {error}"));
        }
        return None;
    }
    let row = gtk::Box::new(gtk::Orientation::Vertical, 8);
    row.set_margin_top(12);
    row.set_margin_bottom(12);
    let heading = gtk::Label::new(Some(&title));
    heading.set_xalign(0.0);
    heading.set_wrap(true);
    heading.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    heading.add_css_class("heading");
    row.append(&heading);
    let status = gtk::Label::new(Some("Queued"));
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    row.append(&status);
    let progress = gtk::ProgressBar::new();
    row.append(&progress);
    let details = gtk::Label::new(None);
    details.set_xalign(0.0);
    details.set_wrap(true);
    details.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    details.set_selectable(true);
    details.set_visible(false);
    row.append(&details);
    let buttons = gtk::FlowBox::new();
    buttons.add_css_class("operation-actions");
    buttons.set_selection_mode(gtk::SelectionMode::None);
    buttons.set_homogeneous(false);
    buttons.set_min_children_per_line(1);
    buttons.set_max_children_per_line(5);
    buttons.set_row_spacing(6);
    buttons.set_column_spacing(8);
    let cancel_button = gtk::Button::with_label("Cancel");
    let review_button = gtk::Button::with_mnemonic("Re_view");
    review_button.set_tooltip_text(Some("Review the pending decision · Alt+V"));
    review_button.set_visible(false);
    let undo_button = gtk::Button::with_label("Undo completed changes");
    undo_button.set_visible(false);
    let retry_button = gtk::Button::with_mnemonic("_Retry failed items");
    let resume_button = gtk::Button::with_mnemonic("_Continue remaining items");
    retry_button.set_visible(false);
    resume_button.set_visible(false);
    for button in [
        &retry_button,
        &resume_button,
        &cancel_button,
        &review_button,
        &undo_button,
    ] {
        let child = gtk::FlowBoxChild::new();
        child.set_focusable(false);
        child.set_child(Some(button));
        button
            .bind_property("visible", &child, "visible")
            .sync_create()
            .build();
        buttons.insert(&child, -1);
    }
    row.append(&buttons);
    row.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let owner = origin
        .upgrade()
        .and_then(|tab| tab.window())
        .map(|window| {
            window
                .preview
                .borrow()
                .as_ref()
                .filter(|preview| preview.native.is_active())
                .map(|preview| preview.native.clone().upcast::<gtk::Window>())
                .unwrap_or_else(|| window.native.clone().upcast::<gtk::Window>())
        })
        .or_else(|| app.application.active_window())
        .map(|window| window.downgrade());
    let mut storage = app.storage.snapshot();
    if let Some((path, resolved)) = origin.upgrade().and_then(|tab| tab.storage_location()) {
        storage.bind_folder(path, resolved);
    }
    let source_storage = if let Some(previous) = previous {
        storage.retry_from(&previous.storage);
        previous.source_storage.as_ref().map(|previous_source| {
            let mut source = app.storage.snapshot();
            source.retry_from(previous_source);
            source
        })
    } else {
        source
    };
    let job = Rc::new(Job {
        title,
        row,
        status,
        progress,
        details,
        cancel_button,
        review_button,
        undo_button,
        retry_button,
        resume_button,
        retry: RefCell::new(None),
        remainder: RefCell::new(None),
        cancellation: Cancellation::default(),
        active: Cell::new(true),
        operation: Cell::new(None),
        origin,
        owner,
        hold: RefCell::new(Some(app.application.hold())),
        prompt: RefCell::new(None),
        followup_until: Cell::new(None),
        drop: RefCell::new(None),
        storage,
        source_storage,
    });
    let weak = Rc::downgrade(&job);
    let desktop = Rc::downgrade(app);
    job.cancel_button.connect_clicked(move |_| {
        if let Some(job) = weak.upgrade() {
            job.cancellation.cancel();
            if let Some(app) = desktop.upgrade() {
                let index = app
                    .jobs
                    .pending
                    .borrow()
                    .iter()
                    .position(|(candidate, _)| Rc::ptr_eq(candidate, &job));
                if let Some(index) = index {
                    app.jobs.pending.borrow_mut().remove(index);
                    job.status.set_label("Cancelled before starting");
                    job.active.set(false);
                    job.progress.set_visible(false);
                    job.cancel_button.set_visible(false);
                    job.hold.borrow_mut().take();
                    finish_drop(&job, false);
                    notify(&app, &job.title, "Cancelled before starting", None);
                    return;
                }
            }
            respond(&job, TransferReply::Cancel);
            job.status.set_label("Cancelling…");
            job.cancel_button.set_sensitive(false);
        }
    });
    let weak = Rc::downgrade(&job);
    let desktop = Rc::downgrade(app);
    job.review_button.connect_clicked(move |_| {
        if let (Some(app), Some(job)) = (desktop.upgrade(), weak.upgrade()) {
            present_prompt(&app, &job);
        }
    });
    let weak = Rc::downgrade(&job);
    let desktop = Rc::downgrade(app);
    job.undo_button.connect_clicked(move |_| {
        if let (Some(app), Some(job)) = (desktop.upgrade(), weak.upgrade()) {
            if app.jobs.undo_id.get() != job.operation.get() {
                job.details
                    .set_label("A more recent operation must be undone first.");
                job.details.set_visible(true);
            } else if let Some(id) = job.operation.get() {
                enqueue(
                    &app,
                    job.origin.clone(),
                    app.undo_label.borrow().clone(),
                    Task::UndoOperation(id),
                );
            }
        }
    });
    for (button, resume) in [(&job.retry_button, false), (&job.resume_button, true)] {
        let weak = Rc::downgrade(&job);
        let desktop = Rc::downgrade(app);
        button.connect_clicked(move |button| {
            if let (Some(app), Some(job)) = (desktop.upgrade(), weak.upgrade()) {
                let plan = if resume {
                    job.remainder.borrow_mut().take()
                } else {
                    job.retry.borrow_mut().take()
                };
                if let Some(task) = plan {
                    button.set_visible(false);
                    let mut title = job.title.as_str();
                    while let Some(rest) = title
                        .strip_prefix("Resume: ")
                        .or_else(|| title.strip_prefix("Retry failed: "))
                    {
                        title = rest;
                    }
                    enqueue_inner(
                        &app,
                        job.origin.clone(),
                        format!(
                            "{}: {}",
                            if resume { "Resume" } else { "Retry failed" },
                            title
                        ),
                        task,
                        Some(&job),
                        None,
                    );
                }
            }
        });
    }
    let list = panel(app);
    list.insert_child_after(&job.row, list.first_child().as_ref());
    app.jobs.entries.borrow_mut().push(job.clone());
    if matches!(task, Task::Action(..)) && !app.devices.removing() {
        start(app, job.clone(), task, false);
    } else {
        app.jobs.pending.borrow_mut().push_back((job.clone(), task));
        dispatch(app);
    }
    Some(job)
}

fn panel(app: &Rc<Desktop>) -> gtk::Box {
    if let Some(panel) = app.jobs.panel.borrow().as_ref() {
        return panel.clone();
    }
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
    panel.set_margin_start(20);
    panel.set_margin_end(20);
    panel.set_margin_top(12);
    panel.set_margin_bottom(12);
    let hint = gtk::Label::new(Some("Transfers keep running when browser windows close."));
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    hint.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    panel.append(&hint);
    let recovery = gtk::Button::with_label("Recovery…");
    recovery.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(app);
    recovery.connect_clicked(move |_| {
        if let Some(app) = weak.upgrade() {
            super::recovery_ui::show(&app);
        }
    });
    panel.append(&recovery);
    *app.jobs.panel.borrow_mut() = Some(panel.clone());
    panel
}

pub(super) fn show(app: &Rc<Desktop>) {
    if let Some(window) = app.jobs.window.borrow().as_ref() {
        window.present();
        return;
    }
    let window = gtk::ApplicationWindow::builder()
        .application(&app.application)
        .title("Operations — Canopa Files")
        .default_width(620)
        .default_height(480)
        .build();
    window.add_css_class("canopa-files");
    window.set_titlebar(Some(&gtk::HeaderBar::new()));
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&panel(app))
        .build();
    window.set_child(Some(&scroll));
    let weak = Rc::downgrade(app);
    window.connect_close_request(move |window| {
        // Rows survive presentation windows; closing this window never cancels work.
        if let Some(scroll) = window.child().and_downcast::<gtk::ScrolledWindow>() {
            scroll.set_child(None::<&gtk::Widget>);
        }
        if let Some(app) = weak.upgrade() {
            app.jobs.window.borrow_mut().take();
        }
        glib::Propagation::Proceed
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = window.downgrade();
    let desktop = Rc::downgrade(app);
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if key == gtk::gdk::Key::Escape {
            if let Some(window) = weak.upgrade() {
                window.close();
            }
            glib::Propagation::Stop
        } else if let Some(app) = desktop.upgrade() {
            let undo = super::key_chord(key, modifiers).is_some_and(|chord| {
                app.action_configuration
                    .borrow()
                    .settings
                    .command_for(&chord)
                    == Some(crate::interface::Command::Undo)
            });
            if undo {
                let result = undo_latest(&app);
                if let Some(hint) = panel(&app).first_child().and_downcast::<gtk::Label>() {
                    hint.set_label(
                        result
                            .err()
                            .unwrap_or("Transfers keep running when browser windows close."),
                    );
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);
    *app.jobs.window.borrow_mut() = Some(window.clone());
    window.present();
    // An outstanding decision may have been put aside before the last browser closed.
    let waiting = app
        .jobs
        .entries
        .borrow()
        .iter()
        .find(|job| job.prompt.borrow().is_some())
        .cloned();
    if let Some(job) = waiting {
        let desktop = Rc::downgrade(app);
        glib::idle_add_local_once(move || {
            if let Some(app) = desktop.upgrade() {
                present_prompt(&app, &job);
            }
        });
    }
}

fn dispatch(app: &Rc<Desktop>) {
    if app.devices.removing() {
        return;
    }
    // Actions normally run alongside transfers. Only a device removal holds
    // newly requested actions; release those without serializing them behind
    // a file transfer once the device operation finishes.
    loop {
        let action = {
            let mut pending = app.jobs.pending.borrow_mut();
            let index = pending
                .iter()
                .position(|(_, task)| matches!(task, Task::Action(..)));
            index.and_then(|index| pending.remove(index))
        };
        let Some((job, task)) = action else { break };
        start(app, job, task, false);
    }
    if app.jobs.busy.get() {
        return;
    }
    if app.operations.borrow().is_none() {
        return;
    }
    let next = app.jobs.pending.borrow_mut().pop_front();
    let Some((job, task)) = next else { return };
    app.jobs.busy.set(true);
    start(app, job, task, true);
}

fn recovery_display_path(path: &mut PathBuf, source: &std::path::Path, display: &std::path::Path) {
    if let Ok(tail) = path.strip_prefix(source) {
        *path = display.join(tail);
    }
}

// Display hints never enter a continuation or a filesystem check.
fn recovery_display_event(
    mut event: TransferEvent,
    source: &std::path::Path,
    display: &std::path::Path,
) -> TransferEvent {
    match &mut event {
        TransferEvent::Progress { path, .. } | TransferEvent::Verifying { source: path, .. } => {
            recovery_display_path(path, source, display)
        }
        TransferEvent::Conflict {
            source: incoming, ..
        } => recovery_display_path(&mut incoming.path, source, display),
        TransferEvent::Unavailable { .. } => {}
    }
    event
}

fn recovery_result(
    outcome: crate::operations::RecoveryCopyOutcome,
    source: &std::path::Path,
    display: &std::path::Path,
) -> ResultBody {
    let mut outcome = outcome.map_continuations(|plan| Task::ResumeRecover(Box::new(plan)));
    for failure in &mut outcome.failures {
        recovery_display_path(&mut failure.source, source, display);
    }
    for path in outcome.skipped.iter_mut().chain(&mut outcome.remaining) {
        recovery_display_path(path, source, display);
    }
    ResultBody::Transfer(Box::new(outcome), None)
}

fn start(app: &Rc<Desktop>, job: Rc<Job>, task: Task, serialized: bool) {
    let engine = app.operations.borrow().clone();
    job.status.set_label("Starting…");
    let progress = Arc::new(Mutex::new(None::<TransferEvent>));
    let latest = progress.clone();
    let (questions, receiver) = mpsc::channel::<Question>();
    let weak = Rc::downgrade(&job);
    let desktop = Rc::downgrade(app);
    glib::timeout_add_local(Duration::from_millis(80), move || {
        let (Some(app), Some(job)) = (desktop.upgrade(), weak.upgrade()) else {
            return glib::ControlFlow::Break;
        };
        if !job.active.get() {
            return glib::ControlFlow::Break;
        }
        job.progress.pulse();
        if let Ok(mut progress) = latest.lock()
            && let Some(event) = progress.take()
        {
            match event {
                TransferEvent::Progress {
                    path,
                    completed,
                    bytes,
                } => job.status.set_label(&format!(
                    "{completed} {} completed · {} bytes\n{}",
                    if completed == 1 { "item" } else { "items" },
                    bytes,
                    path.display()
                )),
                TransferEvent::Verifying { source, .. } => job.status.set_label(&format!(
                    "Verifying {} before removing its source",
                    source.display()
                )),
                _ => {}
            }
        }
        while let Ok(question) = receiver.try_recv() {
            job.status.set_label("Waiting for your decision");
            *job.prompt.borrow_mut() = Some(Prompt {
                event: question.event,
                reply: question.reply,
                dialog: None,
            });
            job.review_button.set_visible(true);
            notify(
                &app,
                &job.title,
                "Waiting for your decision. Open Operations to continue.",
                None,
            );
            if prompt_owner_active(&app, &job) {
                present_prompt(&app, &job);
            }
        }
        if let Some(until) = job.followup_until.get() {
            if Instant::now() > until {
                job.followup_until.set(None);
            } else {
                let waiting = job
                    .prompt
                    .borrow()
                    .as_ref()
                    .is_some_and(|prompt| prompt.dialog.is_none());
                // Closing the answered dialog can precede the compositor's
                // owner-activation round trip. Keep an immediate follow-up
                // eligible until that handoff completes, without presenting
                // over a different foreground application.
                if waiting && prompt_owner_active(&app, &job) {
                    present_prompt(&app, &job);
                }
            }
        }
        glib::ControlFlow::Continue
    });
    let undoing = matches!(task, Task::Mutation(None) | Task::UndoOperation(_));
    let cancellation = job.cancellation.clone();
    let desktop = Rc::downgrade(app);
    let data_home = app.data_home.clone();
    let storage = job.storage.clone();
    let source_storage = job.source_storage.clone();
    glib::MainContext::default().spawn_local(async move {
        let result = gio::spawn_blocking(move || {
            let paths = match task.storage_paths(engine.as_deref(), &data_home) {
                Ok(paths) => paths,
                Err(error) => {
                    let label = engine.as_ref().and_then(|engine| engine.undo_label());
                    let id = engine.as_ref().and_then(|engine| engine.latest_undo_id());
                    return (Err(error), label, id);
                }
            };
            if let Err(error) = storage.check(&paths) {
                let label = engine.as_ref().and_then(|engine| engine.undo_label());
                let id = engine.as_ref().and_then(|engine| engine.latest_undo_id());
                return (
                    Ok(ResultBody::PreparationFailed(
                        Box::new(task),
                        error.to_string(),
                    )),
                    label,
                    id,
                );
            }
            let action = if let Task::Action(action, paths, directory) = &task {
                Some((action.clone(), paths.clone(), directory.clone()))
            } else {
                None
            };
            let Some(engine) = engine else {
                let result = action
                    .ok_or_else(|| std::io::Error::other("file operations are unavailable"))
                    .and_then(|(action, paths, directory)| {
                        action
                            .prepare(&paths, &directory)
                            .and_then(|prepared| prepared.run(&cancellation))
                    })
                    .map(ResultBody::Action);
                return (result, None, None);
            };
            let undo_interact = |conflict| {
                let (sender, answer) = mpsc::channel();
                if cancellation.is_cancelled()
                    || questions
                        .send(Question {
                            event: PromptEvent::Undo(conflict),
                            reply: PromptReply::Undo(sender),
                        })
                        .is_err()
                {
                    return UndoChoice::Cancel;
                }
                loop {
                    if cancellation.is_cancelled() {
                        return UndoChoice::Cancel;
                    }
                    match answer.recv_timeout(Duration::from_millis(50)) {
                        Ok(reply) => return reply,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(_) => return UndoChoice::Cancel,
                    }
                }
            };
            let check = |path: &std::path::Path, access| match access {
                TransferAccess::SourceEntry => source_storage
                    .as_ref()
                    .unwrap_or(&storage)
                    .check(&[super::storage::Access::entry(path)]),
                TransferAccess::DestinationFolder => {
                    storage.check(&[super::storage::Access::folder(path)])
                }
            };
            let transfer_interact =
                |event, recovery: Option<(&std::path::Path, &std::path::Path)>| {
                    let event = if let Some((source, display)) = recovery {
                        recovery_display_event(event, source, display)
                    } else {
                        event
                    };
                    if cancellation.is_cancelled() {
                        return TransferReply::Cancel;
                    }
                    if matches!(
                        event,
                        TransferEvent::Conflict { .. } | TransferEvent::Unavailable { .. }
                    ) {
                        let (sender, answer) = mpsc::channel();
                        if questions
                            .send(Question {
                                event: if recovery.is_some() {
                                    PromptEvent::Recovery(event)
                                } else {
                                    PromptEvent::Transfer(event)
                                },
                                reply: PromptReply::Transfer(sender),
                            })
                            .is_err()
                        {
                            return TransferReply::Cancel;
                        }
                        loop {
                            if cancellation.is_cancelled() {
                                return TransferReply::Cancel;
                            }
                            match answer.recv_timeout(Duration::from_millis(50)) {
                                Ok(reply) => return reply,
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(_) => return TransferReply::Cancel,
                            }
                        }
                    } else {
                        if let Ok(mut latest) = progress.lock() {
                            *latest = Some(event);
                        }
                        TransferReply::Continue
                    }
                };
            let result = match task {
                Task::Action(action, paths, directory) => action
                    .prepare(&paths, &directory)
                    .and_then(|prepared| prepared.run(&cancellation))
                    .map(ResultBody::Action),
                Task::Trash(paths) => engine
                    .trash_files(&data_home, &paths, &cancellation)
                    .map(ResultBody::Trash),
                Task::Restore(item, destination) => engine
                    .restore(&item, destination.as_deref(), &cancellation)
                    .map(ResultBody::Mutation),
                Task::Recover(item, destination) => {
                    engine.recovery_copy_source_path(&item).and_then(|source| {
                        let display = item
                            .original
                            .clone()
                            .unwrap_or_else(|| PathBuf::from(item.name()));
                        engine
                            .recover_copy_with_check(
                                &item,
                                &destination,
                                &cancellation,
                                check,
                                |event| transfer_interact(event, Some((&source, &display))),
                            )
                            .map(|outcome| recovery_result(outcome, &source, &display))
                    })
                }
                Task::ResumeRecover(plan) => engine
                    .recovery_copy_source_path(plan.item())
                    .and_then(|source| {
                        let item = plan.item();
                        let display = item
                            .original
                            .clone()
                            .unwrap_or_else(|| PathBuf::from(item.name()));
                        engine
                            .resume_recovery_copy_with_check(&plan, &cancellation, check, |event| {
                                transfer_interact(event, Some((&source, &display)))
                            })
                            .map(|outcome| recovery_result(outcome, &source, &display))
                    }),
                Task::RetryRecovery => engine
                    .retry_recovery(&cancellation)
                    .map(|()| ResultBody::Reconciled),
                Task::UndoOperation(id) => engine
                    .undo_interactive(Some(id), &cancellation, undo_interact)
                    .map(ResultBody::Undo),
                Task::Mutation(request) => match request {
                    Some(request) => engine
                        .execute(request, &cancellation)
                        .map(ResultBody::Mutation),
                    None => engine
                        .undo_interactive(None, &cancellation, undo_interact)
                        .map(ResultBody::Undo),
                },
                Task::Transfer(request, token, plan) => {
                    let outcome = if let Some(plan) = plan {
                        engine.resume_transfer_with_check(&plan, &cancellation, check, |event| {
                            transfer_interact(event, None)
                        })
                    } else {
                        engine.transfer_with_check(&request, &cancellation, check, |event| {
                            transfer_interact(event, None)
                        })
                    };
                    let remaining = if token.is_some() {
                        request
                            .sources
                            .iter()
                            .filter(|path| {
                                std::fs::symlink_metadata(path).is_ok()
                                    || outcome.as_ref().is_ok_and(|outcome| {
                                        outcome
                                            .failures
                                            .iter()
                                            .map(|failure| &failure.source)
                                            .chain(&outcome.remaining)
                                            .chain(&outcome.skipped)
                                            .any(|pending| pending.starts_with(path.as_path()))
                                    })
                            })
                            .cloned()
                            .collect()
                    } else {
                        Vec::new()
                    };
                    outcome.map(|outcome| {
                        ResultBody::Transfer(
                            Box::new(outcome.map_continuations(|plan| {
                                Task::Transfer(request.clone(), token, Some(plan))
                            })),
                            token.map(|token| (token, remaining)),
                        )
                    })
                }
            };
            if serialized {
                (result, engine.undo_label(), engine.latest_undo_id())
            } else {
                (result, None, None)
            }
        })
        .await;
        let Some(app) = desktop.upgrade() else { return };
        let (status, detail, paths, operation) = match result {
            Ok((result, label, id)) => {
                if serialized {
                    *app.undo_label.borrow_mut() =
                        label.unwrap_or_else(|| String::from("Nothing to undo"));
                    app.jobs.undo_id.set(id);
                }
                match result {
                    Ok(ResultBody::PreparationFailed(task, message)) => {
                        *job.retry.borrow_mut() = Some(*task);
                        job.retry_button.set_label("_Retry request");
                        job.retry_button.set_visible(true);
                        (
                            String::from("Needs attention · request was not started"),
                            message,
                            Vec::new(),
                            None,
                        )
                    }
                    Ok(ResultBody::Reconciled) => (
                        "Completed · interrupted operation reconciled".into(),
                        String::new(),
                        Vec::new(),
                        None,
                    ),
                    Ok(ResultBody::Action(outcome)) => {
                        let status = if outcome.cancelled {
                            String::from("Cancelled")
                        } else if outcome.success {
                            String::from("Completed")
                        } else {
                            outcome.exit_code.map_or_else(
                                || String::from("Failed · terminated by signal"),
                                |code| format!("Failed · exit code {code}"),
                            )
                        };
                        let mut detail = String::new();
                        for (label, bytes, truncated) in [
                            ("Output", &outcome.stdout, outcome.stdout_truncated),
                            ("Error output", &outcome.stderr, outcome.stderr_truncated),
                        ] {
                            if !bytes.is_empty() {
                                detail.push_str(&format!(
                                    "{label}{}:\n{}\n",
                                    if truncated { " (first 64 KiB)" } else { "" },
                                    String::from_utf8_lossy(bytes).replace('\0', "�")
                                ));
                            }
                        }
                        (status, detail, Vec::new(), None)
                    }
                    Ok(ResultBody::Trash(outcome)) => {
                        if !outcome.failures.is_empty() {
                            *job.retry.borrow_mut() = Some(Task::Trash(
                                outcome
                                    .failures
                                    .iter()
                                    .map(|(path, _)| path.clone())
                                    .collect(),
                            ));
                            job.retry_button.set_visible(true);
                        }
                        let status = if outcome.cancelled {
                            "Cancelled"
                        } else if outcome.failures.is_empty() {
                            "Completed"
                        } else {
                            "Completed with errors"
                        };
                        let detail = outcome
                            .failures
                            .iter()
                            .map(|(path, error)| format!("{}\n{error}", path.display()))
                            .collect::<Vec<_>>()
                            .join("\n\n");
                        (
                            format!("{status} · {} items moved to Trash", outcome.completed),
                            detail,
                            Vec::new(),
                            outcome.id,
                        )
                    }
                    Ok(ResultBody::Undo(outcome)) => {
                        for previous in
                            app.jobs.entries.borrow().iter().filter(|previous| {
                                previous.operation.get() == Some(outcome.summary.id)
                            })
                        {
                            if outcome.remaining == 0 {
                                previous.status.set_label(if !outcome.unremoved.is_empty() {
                                    "Undo finished · some folders not removed"
                                } else if !outcome.dismissed.is_empty()
                                    || !outcome.unrestored_previous.is_empty()
                                {
                                    "Undo finished · some items not restored"
                                } else if outcome.kept.is_empty() {
                                    "Undone"
                                } else {
                                    "Undo finished · items kept"
                                });
                                previous.operation.set(None);
                            } else {
                                previous.status.set_label("Partly undone");
                            }
                        }
                        let status = if outcome.cancelled {
                            "Cancelled"
                        } else if outcome.remaining > 0 {
                            "Partly undone"
                        } else {
                            "Completed"
                        };
                        (
                            format!(
                                "{status} · {} {} undone{}{}{}{}",
                                outcome.completed,
                                if outcome.completed == 1 {
                                    "change"
                                } else {
                                    "changes"
                                },
                                if outcome.kept.is_empty() {
                                    String::new()
                                } else {
                                    format!(
                                        " · {} {} kept",
                                        outcome.kept.len(),
                                        if outcome.kept.len() == 1 {
                                            "item"
                                        } else {
                                            "items"
                                        }
                                    )
                                },
                                if outcome.dismissed.is_empty() {
                                    String::new()
                                } else {
                                    format!(
                                        " · {} {} not returned",
                                        outcome.dismissed.len(),
                                        if outcome.dismissed.len() == 1 {
                                            "item"
                                        } else {
                                            "items"
                                        }
                                    )
                                },
                                if outcome.unrestored_previous.is_empty() {
                                    String::new()
                                } else {
                                    format!(
                                        " · {} saved {} not restored",
                                        outcome.unrestored_previous.len(),
                                        if outcome.unrestored_previous.len() == 1 {
                                            "version"
                                        } else {
                                            "versions"
                                        }
                                    )
                                },
                                if outcome.unremoved.is_empty() {
                                    String::new()
                                } else {
                                    format!(
                                        " · {} {} not removed",
                                        outcome.unremoved.len(),
                                        if outcome.unremoved.len() == 1 {
                                            "folder"
                                        } else {
                                            "folders"
                                        }
                                    )
                                }
                            ),
                            {
                                let mut detail = if outcome.remaining > 0 {
                                    format!(
                                        "{} changes remain available to Undo.\n{}",
                                        outcome.remaining,
                                        outcome
                                            .skipped
                                            .iter()
                                            .map(|path| format!("Skipped {}", path.display()))
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    )
                                } else {
                                    String::new()
                                };
                                for path in outcome.unremoved.iter().take(50) {
                                    if !detail.is_empty() {
                                        detail.push('\n');
                                    }
                                    detail.push_str(&format!(
                                        "Did not remove the created folder recorded at {}",
                                        path.display()
                                    ));
                                }
                                for path in outcome.dismissed.iter().take(50) {
                                    if !detail.is_empty() {
                                        detail.push('\n');
                                    }
                                    detail.push_str(&format!(
                                        "Did not return the moved item recorded at {}",
                                        path.display()
                                    ));
                                }
                                for path in outcome.unrestored_previous.iter().take(50) {
                                    if !detail.is_empty() {
                                        detail.push('\n');
                                    }
                                    detail.push_str(&format!(
                                        "Did not restore the saved previous destination at {}",
                                        path.display()
                                    ));
                                }
                                detail
                            },
                            outcome.summary.paths,
                            None,
                        )
                    }
                    Ok(ResultBody::Mutation(summary)) => {
                        if undoing {
                            for previous in app
                                .jobs
                                .entries
                                .borrow()
                                .iter()
                                .filter(|previous| previous.operation.get() == Some(summary.id))
                            {
                                previous.status.set_label("Undone");
                                previous.operation.set(None);
                            }
                        }
                        (
                            String::from("Completed"),
                            String::new(),
                            summary.paths,
                            (!undoing).then_some(summary.id),
                        )
                    }
                    Ok(ResultBody::Transfer(outcome, clipboard)) => {
                        finish_drop(
                            &job,
                            !outcome.cancelled
                                && !outcome.needs_attention
                                && outcome.failures.is_empty()
                                && outcome.skipped.is_empty()
                                && outcome.remaining.is_empty(),
                        );
                        if let Some(retry) = outcome.retry_continuation {
                            *job.retry.borrow_mut() = Some(retry);
                            job.retry_button.set_visible(true);
                        }
                        if let Some(plan) = outcome.continuation {
                            *job.remainder.borrow_mut() = Some(plan);
                            job.resume_button.set_visible(true);
                        }
                        if let Some((token, remaining)) = clipboard {
                            clipboard::moved(&app, token, &remaining);
                        }
                        let status = if outcome.cancelled {
                            "Cancelled"
                        } else if outcome.needs_attention {
                            "Paused"
                        } else if outcome.failures.is_empty() {
                            "Completed"
                        } else {
                            "Completed with errors"
                        };
                        let failures = outcome
                            .failures
                            .iter()
                            .map(|failure| {
                                format!(
                                    "{} → {}\n{}",
                                    failure.source.display(),
                                    failure.destination.display(),
                                    failure.message
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n\n");
                        let mut detail = outcome.attention.unwrap_or_default();
                        if !failures.is_empty() {
                            if !detail.is_empty() {
                                detail.push_str("\n\n");
                            }
                            detail.push_str(&failures);
                        }
                        if !outcome.skipped.is_empty() {
                            detail.push_str(&format!("\n{} items skipped", outcome.skipped.len()));
                        }
                        if !outcome.remaining.is_empty() {
                            detail.push_str(&format!(
                                "\n{} items remain at source:\n",
                                outcome.remaining.len()
                            ));
                            for path in outcome.remaining.iter().take(50) {
                                detail.push_str(&format!("{}\n", path.display()));
                            }
                            if outcome.remaining.len() > 50 {
                                detail.push_str(
                                    "Additional items are included in Continue remaining items.\n",
                                );
                            }
                        }
                        (
                            if outcome.bytes == 0 {
                                format!(
                                    "{status} · {} {} completed",
                                    outcome.completed,
                                    if outcome.completed == 1 {
                                        "item"
                                    } else {
                                        "items"
                                    }
                                )
                            } else {
                                format!(
                                    "{status} · {} {} completed · {} bytes processed",
                                    outcome.completed,
                                    if outcome.completed == 1 {
                                        "item"
                                    } else {
                                        "items"
                                    },
                                    outcome.bytes
                                )
                            },
                            detail,
                            outcome.destinations,
                            outcome.id,
                        )
                    }
                    Err(error) => (
                        if error.kind() == std::io::ErrorKind::Interrupted {
                            String::from("Cancelled")
                        } else {
                            String::from("Failed")
                        },
                        error.to_string(),
                        Vec::new(),
                        None,
                    ),
                }
            }
            Err(_) => (
                String::from("Failed"),
                String::from("The file-operation worker failed"),
                Vec::new(),
                None,
            ),
        };
        let trash_window = app.trash_window.borrow().clone();
        if let Some(window) = trash_window {
            window.refresh();
        }
        let recovery_window = app.recovery_window.borrow().clone();
        if let Some(window) = recovery_window {
            window.refresh();
        }
        job.operation.set(operation);
        finish_drop(&job, false);
        job.status.set_label(&status);
        job.details.set_label(&detail);
        job.details.set_visible(!detail.is_empty());
        job.progress
            .set_fraction(if status.starts_with("Completed") {
                1.0
            } else {
                0.0
            });
        job.cancel_button.set_visible(false);
        job.review_button.set_visible(false);
        job.progress.set_visible(false);
        job.active.set(false);
        respond(&job, TransferReply::Cancel);
        if let Some(tab) = job.origin.upgrade() {
            if let Some(path) = paths
                .into_iter()
                .find(|path| path.parent() == tab.path().as_deref())
            {
                tab.reveal_path(path);
            }
            if !detail.is_empty() && !status.starts_with("Completed")
                || status.starts_with("Completed with errors")
            {
                tab.show_error(&format!("{status}. Open Operations for details."));
            }
        }
        for candidate in app.jobs.entries.borrow().iter() {
            candidate.undo_button.set_visible(
                candidate.operation.get().is_some()
                    && candidate.operation.get() == app.jobs.undo_id.get(),
            );
        }
        notify(
            &app,
            &job.title,
            &status,
            operation.filter(|id| app.jobs.undo_id.get() == Some(*id)),
        );
        if serialized {
            app.jobs.busy.set(false);
            dispatch(&app);
        }
        // Notification delivery has its own bounded hold; this job is finished.
        job.hold.borrow_mut().take();
        prune(&app);
    });
}

fn prune(app: &Rc<Desktop>) {
    let mut entries = app.jobs.entries.borrow_mut();
    let finished = |job: &Rc<Job>| {
        !job.active.get()
            && job.retry.borrow().is_none()
            && job.remainder.borrow().is_none()
            && job.prompt.borrow().is_none()
    };
    let excess = entries
        .iter()
        .filter(|job| finished(job))
        .count()
        .saturating_sub(100);
    for _ in 0..excess {
        let Some(index) = entries.iter().position(finished) else {
            break;
        };
        let job = entries.remove(index);
        panel(app).remove(&job.row);
    }
}

pub(super) fn notify(app: &Rc<Desktop>, title: &str, body: &str, undo: Option<u64>) {
    let notification = gio::Notification::new(title);
    notification.set_body(Some(body));
    notification.set_default_action("app.operations");
    notification.add_button("Show operations", "app.operations");
    if let Some(id) = undo {
        notification.add_button_with_target_value(
            "Undo",
            "app.undo-operation",
            Some(&id.to_variant()),
        );
    }
    app.application.send_notification(None, &notification);
    let hold = app.application.hold();
    glib::timeout_add_local_once(Duration::from_millis(500), move || drop(hold));
}

fn respond(job: &Rc<Job>, reply: TransferReply) {
    let prompt = job.prompt.borrow_mut().take();
    if let Some(prompt) = prompt {
        let merge_root = matches!(&prompt.event, PromptEvent::Undo(conflict) if matches!(conflict.source, crate::operations::UndoSource::MergeSource | crate::operations::UndoSource::MergeDestination));
        let nonempty_folder = matches!(&prompt.event, PromptEvent::Undo(conflict) if matches!(conflict.source, crate::operations::UndoSource::NonemptyFolder | crate::operations::UndoSource::MergeRemainder));
        let changed_folder = matches!(&prompt.event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::ChangedCreatedFolder);
        let unavailable_folder = matches!(&prompt.event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::UnavailableCreatedFolder);
        let changed_copy = matches!(&prompt.event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::ChangedCopy);
        let saved_version = matches!(&prompt.event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::ChangedRetained);
        let changed_move = matches!(&prompt.event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::ChangedMove);
        let changed_cross = matches!(&prompt.event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::ChangedCrossMove);
        let unavailable_move = matches!(&prompt.event, PromptEvent::Undo(conflict) if matches!(conflict.source, crate::operations::UndoSource::UnavailableMove | crate::operations::UndoSource::UnavailableCrossMove));
        let unavailable_saved = matches!(&prompt.event, PromptEvent::Undo(conflict) if matches!(conflict.source, crate::operations::UndoSource::UnavailableRetained | crate::operations::UndoSource::UnavailableMergeRetained));
        job.followup_until.set(
            prompt
                .dialog
                .as_ref()
                .is_some_and(|dialog| dialog.is_active())
                .then(|| Instant::now() + Duration::from_secs(1)),
        );
        if let Some(dialog) = prompt.dialog {
            dialog.destroy();
        }
        match prompt.reply {
            PromptReply::Transfer(sender) => {
                let _ = sender.send(reply);
            }
            PromptReply::Undo(sender) => {
                let reply = match reply {
                    TransferReply::Resolve {
                        choice: ConflictChoice::Merge,
                        ..
                    } => UndoChoice::Merge,
                    TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        ..
                    } => {
                        if unavailable_move {
                            UndoChoice::RestorePreviousDestination
                        } else if changed_copy {
                            UndoChoice::MoveToRecovery
                        } else if changed_move || changed_cross || unavailable_saved {
                            UndoChoice::ReturnItem
                        } else {
                            UndoChoice::Replace
                        }
                    }
                    TransferReply::Resolve {
                        choice: ConflictChoice::KeepBoth,
                        ..
                    } => {
                        if merge_root || unavailable_folder || unavailable_move || unavailable_saved
                        {
                            UndoChoice::FinishStep
                        } else if saved_version {
                            UndoChoice::UseSavedVersion
                        } else if nonempty_folder
                            || changed_folder
                            || changed_copy
                            || changed_move
                            || changed_cross
                        {
                            UndoChoice::KeepItem
                        } else {
                            UndoChoice::KeepBoth
                        }
                    }
                    TransferReply::Resolve {
                        choice: ConflictChoice::Skip,
                        ..
                    } => UndoChoice::Skip,
                    _ => UndoChoice::Cancel,
                };
                job.status.set_label(if reply == UndoChoice::Cancel {
                    "Cancelling Undo…"
                } else {
                    "Undoing…"
                });
                let _ = sender.send(reply);
            }
        }
    }
    job.review_button.set_visible(false);
}

fn prompt_owner_active(app: &Desktop, job: &Job) -> bool {
    app.jobs
        .window
        .borrow()
        .as_ref()
        .is_some_and(|window| window.is_active())
        || job
            .owner
            .as_ref()
            .and_then(glib::WeakRef::upgrade)
            .is_some_and(|window| window.is_active())
}

fn present_prompt(app: &Rc<Desktop>, job: &Rc<Job>) {
    job.followup_until.set(None);
    if let Some(window) = job
        .prompt
        .borrow()
        .as_ref()
        .and_then(|prompt| prompt.dialog.clone())
    {
        window.present();
        return;
    }
    let Some(event) = job
        .prompt
        .borrow()
        .as_ref()
        .map(|prompt| prompt.event.clone())
    else {
        return;
    };
    let undo = matches!(event, PromptEvent::Undo(_));
    let recovery_copy = matches!(event, PromptEvent::Recovery(_));
    let merge_root = matches!(&event, PromptEvent::Undo(conflict) if matches!(conflict.source, crate::operations::UndoSource::MergeSource | crate::operations::UndoSource::MergeDestination));
    let merge_source = matches!(&event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::MergeSource);
    let merge_other = match &event {
        PromptEvent::Undo(conflict) if merge_root => conflict.destination.display().to_string(),
        _ => String::new(),
    };
    let merge_explanation = "A folder used by this partial Merge has moved, changed or cannot be inspected. Restore the original folder or its access, then retry Undo. Finish this Merge leaves files in place, removes this branch's unfinished Undo steps and preserves any saved previous destination in Recovery. Already returned items stay where they are. Skip lets you decide later.";
    let merge_remainder = matches!(&event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::MergeRemainder);
    let nonempty_folder = merge_remainder
        || matches!(&event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::NonemptyFolder);
    let changed_folder = matches!(&event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::ChangedCreatedFolder);
    let unavailable_folder = matches!(&event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::UnavailableCreatedFolder);
    let changed_copy = matches!(&event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::ChangedCopy);
    let saved_version = matches!(&event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::ChangedRetained);
    let changed_move = matches!(&event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::ChangedMove);
    let changed_cross = matches!(&event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::ChangedCrossMove);
    let unavailable_cross = matches!(&event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::UnavailableCrossMove);
    let unavailable_move = matches!(&event, PromptEvent::Undo(conflict) if matches!(conflict.source, crate::operations::UndoSource::UnavailableMove | crate::operations::UndoSource::UnavailableCrossMove));
    let merge_saved = matches!(&event, PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::UnavailableMergeRetained);
    let unavailable_saved = matches!(&event, PromptEvent::Undo(conflict) if matches!(conflict.source, crate::operations::UndoSource::UnavailableRetained | crate::operations::UndoSource::UnavailableMergeRetained));
    let return_item = matches!(&event, PromptEvent::Undo(conflict) if conflict.can_return_item());
    let use_saved_version =
        matches!(&event, PromptEvent::Undo(conflict) if conflict.can_use_saved_version());
    let undo_replace = matches!(&event, PromptEvent::Undo(conflict) if conflict.can_replace() || conflict.can_move_to_recovery());
    let undo_merge = matches!(&event, PromptEvent::Undo(conflict) if conflict.can_merge());
    let incoming_hint = match &event {
        PromptEvent::Undo(conflict) if conflict.source == crate::operations::UndoSource::Trash => {
            Some(format!(
                "{} (from Trash)",
                conflict
                    .destination
                    .file_name()
                    .unwrap_or(conflict.destination.as_os_str())
                    .to_string_lossy()
            ))
        }
        PromptEvent::Undo(conflict)
            if conflict.source == crate::operations::UndoSource::PreviousDestination =>
        {
            Some(format!(
                "{} (previous destination)",
                conflict.destination.display()
            ))
        }
        PromptEvent::Undo(conflict) if saved_version || changed_move || changed_cross => {
            Some(conflict.destination.display().to_string())
        }
        _ => None,
    };
    let event = match event {
        PromptEvent::Undo(UndoConflict {
            incoming: UndoItem::Available(source),
            existing,
            ..
        }) => {
            let destination = existing.unwrap_or_else(|| source.clone());
            PromptContent::Transfer(TransferEvent::Conflict {
                source,
                destination,
                folders: undo_merge,
            })
        }
        PromptEvent::Undo(conflict) => PromptContent::Unavailable(conflict),
        PromptEvent::Transfer(event) | PromptEvent::Recovery(event) => {
            PromptContent::Transfer(event)
        }
    };
    let conflict = matches!(
        event,
        PromptContent::Unavailable(_) | PromptContent::Transfer(TransferEvent::Conflict { .. })
    );
    let dialog = gtk::Window::builder()
        .application(&app.application)
        .title(if merge_root {
            "Merge cannot continue — Canopa Files"
        } else if merge_remainder {
            "Items remain after Merge — Canopa Files"
        } else if nonempty_folder {
            "Folder not empty — Canopa Files"
        } else if changed_folder {
            "Created folder changed — Canopa Files"
        } else if unavailable_folder {
            "Created folder unavailable — Canopa Files"
        } else if merge_saved {
            "Saved version unavailable during Merge — Canopa Files"
        } else if unavailable_saved {
            "Saved version unavailable — Canopa Files"
        } else if unavailable_cross {
            "Moved destination unavailable — Canopa Files"
        } else if unavailable_move {
            "Moved item unavailable — Canopa Files"
        } else if changed_cross {
            "Moved destination changed — Canopa Files"
        } else if changed_move {
            "Moved item changed — Canopa Files"
        } else if saved_version {
            "Saved version changed — Canopa Files"
        } else if undo {
            "Undo conflict — Canopa Files"
        } else if conflict {
            "File conflict — Canopa Files"
        } else {
            "Destination unavailable — Canopa Files"
        })
        .default_width(620)
        .build();
    dialog.add_css_class("canopa-files");
    dialog.set_titlebar(Some(&gtk::HeaderBar::new()));
    let parent = app
        .jobs
        .window
        .borrow()
        .as_ref()
        .filter(|window| window.is_active())
        .map(|window| window.clone().upcast::<gtk::Window>())
        .or_else(|| job.owner.as_ref().and_then(glib::WeakRef::upgrade));
    if let Some(parent) = parent {
        dialog.set_transient_for(Some(&parent));
    }
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
    panel.set_margin_start(20);
    panel.set_margin_end(20);
    panel.set_margin_top(20);
    panel.set_margin_bottom(20);
    dialog.set_child(Some(&panel));
    let describe = |entry: &crate::browse::Entry, hint: Option<&str>| {
        let modified = entry
            .modified
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|time| glib::DateTime::from_unix_local(time.as_secs().try_into().ok()?).ok())
            .and_then(|time| time.format("%Y-%m-%d %H:%M:%S %Z").ok())
            .map(|time| time.to_string())
            .unwrap_or_else(|| String::from("Unknown modification time"));
        let size = match entry.kind {
            crate::browse::EntryKind::Directory => String::from("Folder"),
            crate::browse::EntryKind::Symlink => format!("Link · {} bytes", entry.size),
            crate::browse::EntryKind::Other => String::from("Special item"),
            _ => format!("{} bytes", entry.size),
        };
        let label = hint
            .map(str::to_owned)
            .unwrap_or_else(|| entry.path.display().to_string());
        format!("{label}\n{size} · {modified}")
    };
    let (message, choices) = match event {
        PromptContent::Unavailable(question) => {
            let can_restore = question.can_restore_previous_destination();
            let can_return = question.can_return_item();
            let current = question
                .existing
                .as_ref()
                .map(|entry| describe(entry, None));
            let return_destination = question
                .return_destination
                .as_ref()
                .map(|path| path.display().to_string());
            let saved = question
                .saved_previous
                .as_ref()
                .map(|entry| describe(entry, Some(&question.incoming.path().to_string_lossy())));
            let UndoItem::Unavailable { path, message, .. } = question.incoming else {
                unreachable!()
            };
            let mut choices = vec![
                (
                    if merge_root || merge_saved {
                        "_Finish this Merge"
                    } else {
                        "_Finish this step"
                    },
                    ConflictChoice::KeepBoth,
                ),
                ("_Skip", ConflictChoice::Skip),
            ];
            if can_return {
                choices.insert(
                    0,
                    (
                        if merge_saved {
                            "_Continue without saved version"
                        } else {
                            "_Return moved item only"
                        },
                        ConflictChoice::Replace,
                    ),
                );
            }
            if can_restore {
                choices.insert(
                    0,
                    ("_Restore previous destination", ConflictChoice::Replace),
                );
            }
            if merge_root {
                (
                    format!(
                        "{merge_explanation}\n\n{}\n{}\n{}\n\nOther folder\n{}",
                        if merge_source {
                            "Incoming folder location"
                        } else {
                            "Merged folder location"
                        },
                        path.display(),
                        message,
                        question.destination.display()
                    ),
                    choices,
                )
            } else if unavailable_folder {
                (
                    format!(
                        "The created folder cannot be inspected. Reconnect its storage or restore access, then retry Undo. Finish this step leaves files untouched and removes only this folder-removal step from Undo. Skip leaves it available for later.\n\nRecorded folder\n{}\n{}",
                        path.display(),
                        message
                    ),
                    choices,
                )
            } else if merge_saved {
                (
                    format!(
                        "The recorded items have returned to the merged folder, but their saved previous destination cannot be inspected. Continue without saved version removes only the empty incoming folder and keeps saved restoration pending in Undo. Finish this Merge leaves files in place and preserves the saved version in Recovery. Skip lets you decide later.\n\nPrevious destination\n{}\n\nRecorded saved location\n{}\n{}\n\nMerged folder\n{}",
                        question.destination.display(),
                        path.display(),
                        message,
                        return_destination.unwrap_or_default()
                    ),
                    choices,
                )
            } else if unavailable_saved {
                (
                    format!(
                        "The saved previous destination cannot be inspected. Reconnect its storage or restore access, then retry Undo.{} Finish this step leaves the current files in place, removes this step from Undo and preserves the saved version's recovery record. Skip leaves this part of Undo available for later.\n\nPrevious destination\n{}\n\nRecorded saved location\n{}\n{}{}{}",
                        if can_return {
                            " Return moved item only returns the current item and keeps restoration of the saved version pending in Undo."
                        } else {
                            ""
                        },
                        question.destination.display(),
                        path.display(),
                        message,
                        current
                            .map(|current| format!("\n\nCurrent item\n{current}"))
                            .unwrap_or_default(),
                        return_destination
                            .map(|path| format!("\n\nReturn location\n{path}"))
                            .unwrap_or_default()
                    ),
                    choices,
                )
            } else {
                (
                    format!(
                        "{} It may have moved elsewhere, been removed, or be on unavailable storage.{} Finish this step leaves current files in place, finishes this part of Undo and preserves any saved previous destination in Recovery. Skip leaves this part of Undo available for later.\n\nRecorded location\n{}\n{}\n\nOriginal location\n{}{}",
                        if unavailable_cross {
                            "The destination copy of the moved item cannot be inspected."
                        } else {
                            "The moved or renamed item cannot be inspected at its recorded location."
                        },
                        if can_restore {
                            " Restore previous destination restores the saved version shown below. The unavailable moved item will not be returned."
                        } else {
                            ""
                        },
                        path.display(),
                        message,
                        question.destination.display(),
                        saved
                            .map(|saved| format!("\n\nSaved previous destination\n{saved}"))
                            .unwrap_or_default()
                    ),
                    choices,
                )
            }
        }
        PromptContent::Transfer(event) => match event {
            TransferEvent::Conflict {
                source,
                destination,
                folders,
            } => {
                if merge_root {
                    (
                        format!(
                            "{merge_explanation}\n\n{}\n{}\n\nOther folder\n{merge_other}",
                            if merge_source {
                                "Incoming folder location"
                            } else {
                                "Merged folder location"
                            },
                            describe(&source, None)
                        ),
                        vec![
                            ("_Finish this Merge", ConflictChoice::KeepBoth),
                            ("_Skip", ConflictChoice::Skip),
                        ],
                    )
                } else if merge_remainder {
                    (
                        format!(
                            "Some items remain in the incoming folder. Merge remaining items reviews them as additional work. Keep remaining items leaves this folder and its contents in place and finishes this part of Undo. Already returned items stay in the merged folder. Any saved previous destination stays protected in Recovery. Skip lets you decide later.\n\nRemaining folder\n{}\n\nMerged folder\n{}",
                            describe(&source, None),
                            describe(&destination, None)
                        ),
                        vec![
                            ("_Merge remaining items", ConflictChoice::Merge),
                            ("_Keep remaining items", ConflictChoice::KeepBoth),
                            ("_Skip", ConflictChoice::Skip),
                        ],
                    )
                } else if nonempty_folder || changed_folder {
                    (
                        format!(
                            "{} Skip leaves this part of Undo available for later.\n\n{}\n{}",
                            if changed_folder {
                                "Another item now occupies the location of the created folder. Keep item preserves the shown item and any original folder elsewhere, and finishes this created-folder step."
                            } else {
                                "This folder still contains items. Keep folder leaves the folder and its remaining contents in place and finishes this folder's Undo step."
                            },
                            if changed_folder {
                                "Current item"
                            } else {
                                "Created folder"
                            },
                            describe(&source, None)
                        ),
                        vec![
                            (
                                if changed_folder {
                                    "_Keep item"
                                } else {
                                    "_Keep folder"
                                },
                                ConflictChoice::KeepBoth,
                            ),
                            ("_Skip", ConflictChoice::Skip),
                        ],
                    )
                } else if changed_cross {
                    let mut choices = vec![
                        ("_Keep item", ConflictChoice::KeepBoth),
                        ("_Skip", ConflictChoice::Skip),
                    ];
                    if return_item {
                        choices.insert(0, ("_Return this version", ConflictChoice::Replace));
                    }
                    (
                        format!(
                            "The moved file has changed at its destination.{} Keep item leaves it here, finishes this part of Undo and preserves any saved previous destination in Recovery. Skip leaves this part of Undo available for later.\n\nCurrent destination version\n{}\n\nOriginal location\n{}",
                            if return_item {
                                " Return this version returns the version shown below to its original location, then restores any saved previous destination."
                            } else {
                                " This kind of item can be kept here; returning it to its original location is unavailable."
                            },
                            describe(&source, None),
                            incoming_hint.as_deref().unwrap_or_default()
                        ),
                        choices,
                    )
                } else if changed_move {
                    let mut choices = vec![
                        ("_Keep item", ConflictChoice::KeepBoth),
                        ("_Skip", ConflictChoice::Skip),
                    ];
                    if return_item {
                        choices.insert(0, ("_Return this item", ConflictChoice::Replace));
                    }
                    (
                        format!(
                            "Another item has taken the place of the moved or renamed item.{} Keep item leaves it here, finishes this part of Undo and preserves any saved previous destination in Recovery. Skip keeps this Undo available for later.\n\nCurrent item\n{}\n\nOriginal location\n{}",
                            if return_item {
                                " Return this item moves the version shown below to the original location."
                            } else {
                                ""
                            },
                            describe(&source, None),
                            incoming_hint.as_deref().unwrap_or_default()
                        ),
                        choices,
                    )
                } else if saved_version {
                    let mut choices = vec![("_Skip", ConflictChoice::Skip)];
                    if use_saved_version {
                        choices.insert(0, ("_Restore this version", ConflictChoice::KeepBoth));
                    }
                    (
                        format!(
                            "This saved previous destination has changed since it was retained.{} Skip leaves this part of Undo available for later.\n\nCurrent saved version\n{}",
                            if use_saved_version {
                                " Restore this version continues Undo using its current contents."
                            } else {
                                " This kind of item can be inspected in Recovery."
                            },
                            describe(&source, incoming_hint.as_deref())
                        ),
                        choices,
                    )
                } else if changed_copy {
                    let mut choices = vec![
                        ("_Keep item", ConflictChoice::KeepBoth),
                        ("_Skip", ConflictChoice::Skip),
                    ];
                    if undo_replace {
                        choices.insert(0, ("_Move to Recovery", ConflictChoice::Replace));
                    }
                    (
                        format!(
                            "This copied item has changed since the copy. Keep item leaves it in place and finishes this part of Undo.{} Skip leaves it available for a later Undo.\n\nCurrent item\n{}",
                            if undo_replace {
                                " Move to Recovery removes it from this folder while preserving a recoverable version."
                            } else {
                                ""
                            },
                            describe(&destination, None)
                        ),
                        choices,
                    )
                } else {
                    (
                        format!(
                            "{}\n\nIncoming\n{}\n\nExisting\n{}",
                            if undo {
                                if undo_merge {
                                    "Undo would return a folder where another folder now exists. Merge combines their contents and asks about individual file conflicts. Keep both returns the incoming folder with a new name."
                                } else if undo_replace {
                                    "Undo would restore an item where newer work now exists. Replace preserves the newer file in Recovery before restoring the incoming item. Keep both restores the incoming item with a new name."
                                } else {
                                    "Undo would restore an item where newer work now exists. Keep both restores the incoming item with a new name."
                                }
                            } else {
                                "An item with this name already exists."
                            },
                            describe(&source, incoming_hint.as_deref()),
                            describe(&destination, None)
                        ),
                        if undo {
                            let mut choices = vec![
                                ("_Keep both", ConflictChoice::KeepBoth),
                                ("_Skip", ConflictChoice::Skip),
                            ];
                            if undo_replace {
                                choices.insert(0, ("_Replace", ConflictChoice::Replace));
                            }
                            if undo_merge {
                                choices.insert(0, ("_Merge folders", ConflictChoice::Merge));
                            }
                            choices
                        } else if folders {
                            vec![
                                ("_Merge folders", ConflictChoice::Merge),
                                ("_Keep both", ConflictChoice::KeepBoth),
                                ("_Skip", ConflictChoice::Skip),
                            ]
                        } else if recovery_copy
                            || source.kind == crate::browse::EntryKind::Directory
                            || destination.kind == crate::browse::EntryKind::Directory
                        {
                            vec![
                                ("_Keep both", ConflictChoice::KeepBoth),
                                ("_Skip", ConflictChoice::Skip),
                            ]
                        } else {
                            vec![
                                ("_Replace", ConflictChoice::Replace),
                                ("_Keep both", ConflictChoice::KeepBoth),
                                ("_Skip", ConflictChoice::Skip),
                            ]
                        },
                    )
                }
            }
            TransferEvent::Unavailable {
                destination,
                message,
            } => (
                format!(
                    "{message}\n\n{}\n\nReconnect the destination, then retry. Completed changes can still be undone after this transfer stops.",
                    destination.display()
                ),
                Vec::new(),
            ),
            _ => return,
        },
    };
    let label = gtk::Label::new(Some(&message));
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_max_width_chars(72);
    label.set_xalign(0.0);
    label.set_selectable(true);
    panel.append(&label);
    let apply = gtk::CheckButton::with_mnemonic("_Apply to remaining conflicts of this kind");
    apply.set_visible(conflict && !undo);
    panel.append(&apply);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    // Allow an unattended transfer to remain paused with no visible windows.
    let later = gtk::Button::with_mnemonic("_Decide later");
    let weak = Rc::downgrade(job);
    let desktop = Rc::downgrade(app);
    later.connect_clicked(move |_| {
        if let Some(job) = weak.upgrade() {
            let window = job
                .prompt
                .borrow_mut()
                .as_mut()
                .and_then(|prompt| prompt.dialog.take());
            if let Some(window) = window {
                window.destroy();
            }
            if let Some(app) = desktop.upgrade() {
                notify(
                    &app,
                    &job.title,
                    "Waiting for your decision. Open Operations to continue.",
                    None,
                );
            }
        }
    });
    panel.append(&later);
    let cancel = gtk::Button::with_label(if undo {
        "Cancel Undo"
    } else {
        "Cancel transfer"
    });
    let weak = Rc::downgrade(job);
    cancel.connect_clicked(move |_| {
        if let Some(job) = weak.upgrade() {
            job.cancellation.cancel();
            respond(&job, TransferReply::Cancel);
        }
    });
    buttons.append(&cancel);
    if conflict {
        for (title, choice) in choices {
            let button = gtk::Button::with_mnemonic(title);
            let weak = Rc::downgrade(job);
            let apply = apply.clone();
            button.connect_clicked(move |_| {
                if let Some(job) = weak.upgrade() {
                    respond(
                        &job,
                        TransferReply::Resolve {
                            choice,
                            apply_to_remaining: apply.is_active(),
                        },
                    );
                }
            });
            buttons.append(&button);
            if ((merge_root
                || nonempty_folder
                || changed_folder
                || unavailable_folder
                || saved_version
                || changed_move
                || changed_cross
                || unavailable_move
                || unavailable_saved)
                && choice == ConflictChoice::Skip)
                || (!merge_root
                    && !nonempty_folder
                    && !changed_folder
                    && !unavailable_folder
                    && !saved_version
                    && !changed_move
                    && !changed_cross
                    && !unavailable_move
                    && !unavailable_saved
                    && choice == ConflictChoice::KeepBoth)
            {
                dialog.set_default_widget(Some(&button));
            }
        }
    } else {
        let button = gtk::Button::with_mnemonic("_Retry");
        let weak = Rc::downgrade(job);
        button.connect_clicked(move |_| {
            if let Some(job) = weak.upgrade() {
                respond(&job, TransferReply::Retry);
            }
        });
        buttons.append(&button);
        dialog.set_default_widget(Some(&button));
    }
    panel.append(&buttons);
    let weak = Rc::downgrade(job);
    dialog.connect_close_request(move |_| {
        if let Some(job) = weak.upgrade() {
            job.cancellation.cancel();
            respond(&job, TransferReply::Cancel);
        }
        glib::Propagation::Proceed
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = dialog.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(window) = weak.upgrade() {
                window.close();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    dialog.add_controller(keys);
    if let Some(prompt) = job.prompt.borrow_mut().as_mut() {
        prompt.dialog = Some(dialog.clone());
    }
    let initial_focus = Cell::new(true);
    let metadata = label.downgrade();
    dialog.connect_is_active_notify(move |window| {
        if window.is_active() && initial_focus.replace(false) {
            let weak = window.downgrade();
            let metadata = metadata.clone();
            glib::idle_add_local_once(move || {
                if let Some(window) = weak.upgrade()
                    && window.is_active()
                    && let Some(button) = window.default_widget()
                {
                    button.grab_focus();
                    if let Some(metadata) = metadata.upgrade() {
                        metadata.select_region(0, 0);
                    }
                }
            });
        }
    });
    dialog.present();
    if let Some(button) = dialog.default_widget() {
        button.grab_focus();
    }
}
