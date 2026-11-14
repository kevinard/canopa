//! Renderer-independent admission and serial filesystem work. Acceptance is a
//! durable scheduling record; reopening history never replays an uncertain task.
use crate::{
    Cancellation,
    local_activity::{LocalActivity, Work},
    operations::{
        OperationEngine, OperationRequest, OperationSummary, UndoChoice, UndoConflict,
        requests::{RequestPhase, RequestRecord, RetainedSourceLease},
    },
    storage::{self, Access, Requirements, Storage},
};
use futures_channel::{mpsc as asynchronous, oneshot};
use serde::{Deserialize, Serialize};
mod archive;
mod attention;
mod clipboard;
mod dismissal;
mod drag_drop;
mod mutation_completion;
mod recovery;
mod rename;
pub use recovery::{RecoveryActions, RecoveryPart, RecoveryReport};
pub use rename::PreparedRename;
mod transfer;
mod transfer_progress;
mod transfer_resume;
mod trash;
use crate::operations::execution::{MutationConfirmation, TransferConfirmation};
use crate::transfers::{ObservedTransfer, TransferKind, TransferReply, TransferRequest};
pub use archive::ArchiveReport;
pub use attention::{Attention, AttentionObserver, AttentionStatus};
use clipboard::ClipboardPaste;
pub use clipboard::{ClipboardSnapshot, PreparedSelection};
pub use drag_drop::{DropActions, DropChoice, DropTarget, PreparedDrag, PreparedDrop};
use std::{
    collections::{BTreeMap, VecDeque},
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
pub use transfer::{
    TransferFailure, TransferProgress, TransferQuestion, TransferReport, TransferTotal,
};
pub use transfer_resume::{TransferActions, TransferPart, TransferResolution};
pub use trash::{DeleteReport, RestoreReport, TrashFailure, TrashReport};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Request {
    Reconcile,
    KeepRecovery {
        item: Box<crate::operations::RecoveryItem>,
    },
    Mutation(OperationRequest),
    Recover {
        item: Box<crate::operations::RecoveryItem>,
        #[serde(with = "crate::path_serde")]
        destination: PathBuf,
    },
    Undo {
        expected: u64,
    },
    Transfer(TransferRequest),
    Trash {
        #[serde(with = "crate::path_serde")]
        data_home: PathBuf,
        #[serde(with = "crate::path_serde::sequence")]
        sources: Vec<PathBuf>,
    },
    Restore {
        item: Box<crate::trash::TrashItem>,
        #[serde(with = "crate::path_serde::optional")]
        destination: Option<PathBuf>,
    },
    /// Permanent, non-undoable removal of a confirmed selection.
    Delete {
        #[serde(with = "crate::path_serde::sequence")]
        sources: Vec<PathBuf>,
    },
    /// Permanent removal of confirmed Trash items; `empty` names Empty Trash.
    DeleteTrash {
        items: Vec<crate::trash::TrashItem>,
        empty: bool,
    },
    /// Compress a selection or extract an archive. Its result is published
    /// under a free name; it is neither undone nor resumed.
    Archive(crate::archives::ArchiveRequest),
}

/// The resolved browsing directory captured when the user invoked the command.
#[derive(Clone, Debug)]
pub struct Source {
    pub directory: PathBuf,
    pub resolved: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Queued,
    Running,
    Waiting,
    AwaitingReview,
    Interrupted,
    Failed,
    Stopped,
    StartSaveFailed,
    SaveFailed,
    Finished,
}

impl Phase {
    pub fn needs_save(self) -> bool {
        matches!(self, Self::StartSaveFailed | Self::SaveFailed)
    }

    pub fn holds_application(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::Waiting) || self.needs_save()
    }
}

#[derive(Clone, Debug)]
pub struct JobSnapshot {
    pub id: u64,
    pub accepted_at: Option<i64>,
    pub title: String,
    pub phase: Phase,
    pub error: Option<String>,
    pub paths: Arc<[PathBuf]>,
    pub detail: Option<String>,
    pub decision: Option<Decision>,
    pub progress: Option<TransferProgress>,
    pub transfer: Option<Arc<TransferReport>>,
    pub trash: Option<Arc<TrashReport>>,
    pub restore: Option<Arc<RestoreReport>>,
    pub delete: Option<Arc<DeleteReport>>,
    pub archive: Option<Arc<ArchiveReport>>,
    pub recovery: Option<Arc<RecoveryReport>>,
    pub recovery_actions: RecoveryActions,
    pub transfer_actions: TransferActions,
    pub dismissible: bool,
}

#[derive(Clone, Debug)]
pub struct Decision {
    pub generation: u64,
    pub question: Question,
}

#[derive(Clone, Debug)]
pub enum Question {
    Undo(UndoConflict),
    Transfer(TransferQuestion),
    Recovery(TransferQuestion),
}

enum Answer {
    Undo(UndoChoice),
    Transfer(TransferReply),
}

enum QuestionReply {
    Undo(mpsc::SyncSender<UndoChoice>),
    Transfer(mpsc::SyncSender<TransferReply>),
}

impl QuestionReply {
    fn accepts(&self, answer: &Answer) -> bool {
        matches!(
            (self, answer),
            (Self::Undo(_), Answer::Undo(_)) | (Self::Transfer(_), Answer::Transfer(_))
        )
    }
    fn send(self, answer: Answer) {
        match (self, answer) {
            (Self::Undo(reply), Answer::Undo(choice)) => {
                let _ = reply.send(choice);
            }
            (Self::Transfer(reply), Answer::Transfer(choice)) => {
                let _ = reply.send(choice);
            }
            _ => {}
        }
    }
    fn cancel(self) {
        match self {
            Self::Undo(reply) => {
                let _ = reply.send(UndoChoice::Cancel);
            }
            Self::Transfer(reply) => {
                let _ = reply.send(TransferReply::Cancel);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct UndoHead {
    pub id: u64,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub initializing: bool,
    pub error: Option<String>,
    pub jobs: Vec<JobSnapshot>,
    pub undo: Option<UndoHead>,
    pub pending_attention: usize,
}

impl Snapshot {
    pub fn holds_application(&self) -> bool {
        self.initializing
            || self.pending_attention > 0
            || self
                .jobs
                .iter()
                .any(|job| job.phase.holds_application() || job.transfer_actions.abandon)
    }
}

#[derive(Clone)]
pub struct OperationJobs(Arc<Handle>);

struct Handle {
    activity: LocalActivity,
    sender: Arc<mpsc::SyncSender<Control>>,
    snapshot: Arc<Mutex<Snapshot>>,
    notices: Mutex<Option<asynchronous::Receiver<()>>>,
    stopped: Arc<AtomicBool>,
    storage: storage::PreparedStorage,
    attention: Arc<Mutex<attention::Inbox>>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        // Drop the final sending handle as well. A full control queue must never
        // block the UI; receiver disconnection is also a stop request.
        let _ = self.sender.try_send(Control::Stop);
    }
}

enum Control {
    Guarded(Box<Control>, Work),
    NeedsRecoveryReview(oneshot::Sender<Result<bool, String>>),
    ResumeRecovery(u64, RecoveryPart, oneshot::Sender<Result<u64, String>>),
    ResumeTransfer(u64, TransferPart, oneshot::Sender<Result<u64, String>>),
    AbandonTransfer(u64, oneshot::Sender<Result<(), String>>),
    DismissRequest(u64, oneshot::Sender<Result<(), String>>),
    RecoveryPage(
        Option<crate::operations::RecoveryCursor>,
        Cancellation,
        oneshot::Sender<Result<crate::operations::RecoveryPage, String>>,
    ),
    PrepareRename(
        PathBuf,
        Option<Source>,
        oneshot::Sender<Result<PreparedRename, String>>,
    ),
    Rename(
        PreparedRename,
        std::ffi::OsString,
        oneshot::Sender<Result<u64, String>>,
    ),
    PrepareClipboard(
        crate::file_clipboard::Selection,
        storage::Snapshot,
        Option<PreparedSelection>,
        oneshot::Sender<Result<PreparedSelection, String>>,
    ),
    PasteClipboard(
        PreparedSelection,
        PathBuf,
        Option<Source>,
        oneshot::Sender<Result<u64, String>>,
    ),
    PrepareDrag(
        crate::file_clipboard::Selection,
        storage::Snapshot,
        Cancellation,
        Work,
        oneshot::Sender<Result<PreparedDrag, String>>,
    ),
    PrepareDrop(
        PreparedDrag,
        DropTarget,
        storage::Snapshot,
        Cancellation,
        oneshot::Sender<Result<PreparedDrop, String>>,
    ),
    Drop(DropChoice, oneshot::Sender<Result<u64, String>>),
    Submit(
        Request,
        Option<Source>,
        Option<storage::Snapshot>,
        Option<RetainedSourceLease>,
        oneshot::Sender<Result<u64, String>>,
    ),
    LeaseSources(
        Vec<PathBuf>,
        oneshot::Sender<Result<RetainedSourceLease, String>>,
    ),
    Complete(u64, Box<Result<Completion, Failure>>),
    Question(u64, Question, QuestionReply),
    Resolve(u64, u64, Answer),
    Cancel(u64),
    RetrySave(u64),
    Shutdown(oneshot::Sender<Result<(), String>>),
    Stop,
}

impl Control {
    fn preparation_cancelled(&self) -> bool {
        match self {
            Self::RecoveryPage(_, cancellation, _)
            | Self::PrepareDrag(_, _, cancellation, _, _) => cancellation.is_cancelled(),
            Self::PrepareDrop(drag, _, _, cancellation, _) => {
                drag.ended() || cancellation.is_cancelled()
            }
            Self::Drop(choice, _) => choice.cancelled(),
            _ => false,
        }
    }

    fn refuse_admission(self) {
        let message = fl!("files-jobs-admission-full");
        self.refuse_preparation(&message);
    }

    fn refuse_preparation(self, message: &str) {
        match self {
            Self::Submit(_, _, _, _, reply)
            | Self::Rename(_, _, reply)
            | Self::PasteClipboard(_, _, _, reply)
            | Self::Drop(_, reply)
            | Self::ResumeRecovery(_, _, reply)
            | Self::ResumeTransfer(_, _, reply) => {
                let _ = reply.send(Err(message.into()));
            }
            Self::NeedsRecoveryReview(reply) => {
                let _ = reply.send(Err(message.into()));
            }
            Self::AbandonTransfer(_, reply) | Self::DismissRequest(_, reply) => {
                let _ = reply.send(Err(message.into()));
            }
            Self::RecoveryPage(_, _, reply) => {
                let _ = reply.send(Err(message.into()));
            }
            Self::PrepareRename(_, _, reply) => {
                let _ = reply.send(Err(message.into()));
            }
            Self::PrepareClipboard(_, _, _, reply) => {
                let _ = reply.send(Err(message.into()));
            }
            Self::PrepareDrag(_, _, _, _, reply) => {
                let _ = reply.send(Err(message.into()));
            }
            Self::PrepareDrop(_, _, _, _, reply) => {
                let _ = reply.send(Err(message.into()));
            }
            Self::LeaseSources(_, reply) => {
                let _ = reply.send(Err(message.into()));
            }
            Self::Guarded(..)
            | Self::Complete(..)
            | Self::Question(..)
            | Self::Resolve(..)
            | Self::Cancel(..)
            | Self::RetrySave(..)
            | Self::Shutdown(..)
            | Self::Stop => unreachable!("Only new filesystem admissions can be deferred"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Saved {
    Reconciled {
        version: u8,
        previous: Box<Saved>,
    },
    RecoveryResult {
        version: u8,
        title: String,
        report: RecoveryReport,
    },
    Plan {
        version: u8,
        request: Request,
        /// Descriptive exact targets/traversal, never a live execution owner.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transfer_checkpoint: Option<Box<crate::transfers::TransferContinuation>>,
        requirements: Requirements,
        #[serde(default)]
        source_requirements: Option<Requirements>,
        error: Option<String>,
        #[serde(default)]
        rejected_before_execution: bool,
    },
    Completed {
        version: u8,
        title: String,
        #[serde(with = "crate::path_serde::sequence")]
        paths: Vec<PathBuf>,
        #[serde(default)]
        detail: Option<String>,
        #[serde(default)]
        transfer: Option<TransferReport>,
    },
    TransferPending {
        version: u8,
        title: String,
        outcome: Box<transfer::Stopped>,
        requirements: Requirements,
        source_requirements: Requirements,
    },
    TrashResult {
        version: u8,
        title: String,
        report: TrashReport,
    },
    RestoreResult {
        version: u8,
        title: String,
        report: RestoreReport,
    },
    DeleteResult {
        version: u8,
        title: String,
        report: DeleteReport,
    },
    ArchiveResult {
        version: u8,
        title: String,
        report: ArchiveReport,
    },
}

struct Job {
    work: Option<Work>,
    record: RequestRecord,
    saved: Saved,
    mutation_confirmation: Option<MutationConfirmation>,
    transfer_confirmation: Option<TransferConfirmation>,
    reconciliation: Option<ReconciliationReview>,
    recovery_pending: Option<recovery::Pending>,
    recovery_execution: Option<crate::operations::RecoveryCopyContinuation>,
    transfer_pending: Option<transfer_resume::Pending>,
    transfer_execution: Option<crate::transfers::TransferContinuation>,
    cut_progress: Option<clipboard::CutProgress>,
    cut_claim: Option<clipboard::CutClaim>,
    placement: Option<storage::Snapshot>,
    source_placement: Option<storage::Snapshot>,
    observed_transfer: Option<ObservedTransfer>,
    observed_sources: Option<crate::transfers::ObservedSources>,
    destination_observation: Option<drag_drop::DestinationProof>,
    clipboard: Option<ClipboardPaste>,
    cancellation: Cancellation,
    decision: Option<(u64, QuestionReply)>,
    generation: u64,
}

#[derive(Clone)]
struct ReconciliationReview {
    journals: crate::operations::Reconciliation,
    reports: Vec<u64>,
}

struct Completion {
    summary: OperationSummary,
    detail: Option<String>,
    transfer: Option<Box<transfer::Finished>>,
    trash: Option<TrashReport>,
    restore: Option<RestoreReport>,
    delete: Option<DeleteReport>,
    recovery: Option<recovery::Finished>,
    mutation_confirmation: Option<MutationConfirmation>,
}

enum Failure {
    BeforeExecution(String),
    DuringExecution(String),
}

impl Completion {
    fn cancelled_before_start(
        request: Option<&Request>,
        recovery_execution: Option<&crate::operations::RecoveryCopyContinuation>,
        transfer: Option<transfer::Finished>,
    ) -> Self {
        let mut completion = Self {
            summary: OperationSummary {
                id: 0,
                label: fl!("files-jobs-cancelled-before-start"),
                paths: Vec::new(),
            },
            detail: None,
            transfer: None,
            trash: None,
            restore: None,
            delete: None,
            recovery: None,
            mutation_confirmation: None,
        };
        if let Some(request @ (Request::Delete { .. } | Request::DeleteTrash { .. })) = request {
            completion.summary.label = request.title();
            completion.delete = Some(DeleteReport::unstarted(
                matches!(request, Request::DeleteTrash { .. }),
                request.paths(),
                None,
            ));
        }
        if let Some(request @ Request::Trash { sources, .. }) = request {
            completion.summary.label = request.title();
            completion.trash = Some(TrashReport {
                operation: None,
                completed: Vec::new(),
                failures: Vec::new(),
                remaining: sources
                    .iter()
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                cancelled: true,
                interrupted: None,
                unavailable: Vec::new(),
            });
        }
        if let Some(request @ Request::Recover { item, destination }) = request {
            completion.summary.label = request.title();
            completion.recovery = Some(recovery::Finished::cancelled(
                item,
                destination,
                recovery_execution,
            ));
        }
        if let Some(request @ Request::Restore { item, .. }) = request {
            completion.summary.label = request.title();
            completion.restore = Some(RestoreReport::cancelled(item.path.clone()));
        }
        if let Some(transfer) = transfer {
            if let Some(request) = request {
                completion.summary.label = request.title();
            }
            completion.detail = Some(transfer.outcome.detail());
            completion.transfer = Some(Box::new(transfer));
        }
        completion
    }
}

struct WorkerStart<'a> {
    stopped: Arc<AtomicBool>,
    work: &'a Work,
}

struct Worker {
    engine: Arc<OperationEngine>,
    storage: Arc<Storage>,
    snapshot: Arc<Mutex<Snapshot>>,
    sender: Weak<mpsc::SyncSender<Control>>,
    notice: Arc<Mutex<asynchronous::Sender<()>>>,
    stopped: Arc<AtomicBool>,
    jobs: BTreeMap<u64, Job>,
    queue: VecDeque<u64>,
    active: Option<u64>,
    clipboard_owner: Arc<()>,
    attention: Arc<Mutex<attention::Inbox>>,
}

struct PreparedTransfer {
    clipboard: ClipboardPaste,
    observed: ObservedTransfer,
    destination: Option<storage::Snapshot>,
    destination_observation: Option<drag_drop::DestinationProof>,
}

/// A batch that renamed anything finished: its completed steps are one Undo
/// entry. Items left with their name are listed in its detail.
fn batch_rename_completion(
    renames: &[crate::operations::rename_batch::BatchRename],
    outcome: crate::operations::rename_batch::BatchRenameOutcome,
) -> io::Result<Completion> {
    if let Some(error) = outcome.interrupted {
        return Err(io::Error::new(io::ErrorKind::WouldBlock, error));
    }
    let Some(id) = outcome.id else {
        return Err(match outcome.failures.into_iter().next() {
            Some((source, error)) => io::Error::other(fl!(
                "files-jobs-rename-batch-failed",
                name = source
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                error = error
            )),
            None => io::Error::new(io::ErrorKind::Interrupted, "operation cancelled"),
        });
    };
    let total = renames.len();
    let renamed = outcome.renamed.len();
    let detail = if let Some((source, error)) = outcome.failures.first() {
        fl!(
            "files-jobs-rename-batch-partial",
            renamed = renamed,
            total = total,
            failed = outcome.failures.len(),
            name = source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            error = error.clone()
        )
    } else if outcome.cancelled {
        fl!(
            "files-jobs-rename-batch-stopped",
            renamed = renamed,
            total = total
        )
    } else {
        fl!("files-jobs-rename-batch-done", count = renamed)
    };
    Ok(Completion {
        // History rows keep the stored English label and translate it.
        summary: OperationSummary {
            id,
            label: format!(
                "rename {total} {}",
                if total == 1 { "item" } else { "items" }
            ),
            paths: outcome.renamed,
        },
        detail: Some(detail),
        transfer: None,
        trash: None,
        restore: None,
        delete: None,
        recovery: None,
        mutation_confirmation: None,
    })
}

fn check_startup(stopped: &AtomicBool) -> io::Result<()> {
    if stopped.load(Ordering::Acquire) {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            fl!("files-jobs-startup-stopped"),
        ))
    } else {
        Ok(())
    }
}

impl OperationJobs {
    /// Prepares an owned Copy/Cut on the metadata worker, including ordinary
    /// entry identity, original placement and retained-data protection.
    pub fn prepare_clipboard(
        &self,
        selection: crate::file_clipboard::Selection,
        placement: storage::Snapshot,
    ) -> io::Result<oneshot::Receiver<Result<PreparedSelection, String>>> {
        self.prepare_selection(selection, placement, None)
    }

    /// Prepare a deliberate replacement while retaining the previous opaque
    /// intent. Its bounded source-pin overlap belongs only to this runtime.
    /// The caller retains the old intent until native publication succeeds.
    pub fn replace_clipboard(
        &self,
        previous: PreparedSelection,
        selection: crate::file_clipboard::Selection,
        placement: storage::Snapshot,
    ) -> io::Result<oneshot::Receiver<Result<PreparedSelection, String>>> {
        self.prepare_selection(selection, placement, Some(previous))
    }

    fn prepare_selection(
        &self,
        selection: crate::file_clipboard::Selection,
        placement: storage::Snapshot,
        previous: Option<PreparedSelection>,
    ) -> io::Result<oneshot::Receiver<Result<PreparedSelection, String>>> {
        let (sender, receiver) = oneshot::channel();
        self.send_work(Control::PrepareClipboard(
            selection, placement, previous, sender,
        ))
        .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))?;
        Ok(receiver)
    }

    /// Submit the original owned intent. Destination navigation is independent
    /// of the source observation. A Cut can have only one unsettled Paste.
    pub fn paste_clipboard(
        &self,
        selection: PreparedSelection,
        destination: PathBuf,
        binding: Option<Source>,
    ) -> io::Result<oneshot::Receiver<Result<u64, String>>> {
        let (sender, receiver) = oneshot::channel();
        self.send_work(Control::PasteClipboard(
            selection,
            destination,
            binding,
            sender,
        ))
        .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))?;
        Ok(receiver)
    }

    /// Initialization and every history/filesystem access run outside the UI.
    pub fn start(history: PathBuf, volumes: PathBuf) -> io::Result<Self> {
        Self::spawn(history, LocalActivity::default(), move |_| {
            Storage::load(volumes)
        })
    }

    /// Share live platform observations with the request placement guard.
    /// The platform must finish its initial observations before admitting work.
    pub fn start_with_storage(history: PathBuf, storage: Arc<Storage>) -> io::Result<Self> {
        Self::start_with_activity(history, storage, LocalActivity::default())
    }

    /// Share the owning runtime's filesystem/custom-action removal admission.
    pub fn start_with_activity(
        history: PathBuf,
        storage: Arc<Storage>,
        activity: LocalActivity,
    ) -> io::Result<Self> {
        Self::spawn(history, activity, move |_| Ok(storage))
    }

    /// Wait on the queue worker for the platform's first prepared observation.
    /// History and durable admission remain gated until readiness succeeds.
    pub fn start_after_storage(
        history: PathBuf,
        ready: mpsc::Receiver<storage::Ready>,
    ) -> io::Result<Self> {
        Self::start_after_storage_with_activity(history, ready, LocalActivity::default())
    }

    pub fn start_after_storage_with_activity(
        history: PathBuf,
        ready: mpsc::Receiver<storage::Ready>,
        activity: LocalActivity,
    ) -> io::Result<Self> {
        Self::spawn(history, activity, move |stopped| {
            loop {
                check_startup(stopped)?;
                match ready.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(result) => return result.map_err(io::Error::other),
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(io::Error::other(fl!("files-jobs-volume-worker-stopped")));
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        })
    }

    /// Prepared storage shared with the platform. Readiness and startup errors
    /// remain in snapshot(); a present store alone does not authorize work.
    pub fn storage(&self) -> Option<Arc<Storage>> {
        self.0.storage.current()
    }

    /// Read-only access to the same prepared placement used by this queue.
    pub fn storage_source(&self) -> storage::PreparedStorage {
        self.0.storage.clone()
    }

    fn spawn(
        history: PathBuf,
        activity: LocalActivity,
        prepare: impl FnOnce(&AtomicBool) -> io::Result<Arc<Storage>> + Send + 'static,
    ) -> io::Result<Self> {
        let startup = activity.work();
        let snapshot = Arc::new(Mutex::new(Snapshot {
            initializing: true,
            error: None,
            jobs: Vec::new(),
            undo: None,
            pending_attention: 0,
        }));
        let (sender, receiver) = mpsc::sync_channel(1024);
        let sender = Arc::new(sender);
        let worker_snapshot = snapshot.clone();
        let worker_sender = Arc::downgrade(&sender);
        let (notice, notices) = asynchronous::channel(0);
        let notice = Arc::new(Mutex::new(notice));
        let worker_notice = notice.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = stopped.clone();
        let storage = storage::PreparedStorage::default();
        let prepared_storage = storage.clone();
        let attention = Arc::new(Mutex::new(attention::Inbox::default()));
        let worker_attention = attention.clone();
        std::thread::Builder::new()
            .name("files-operation-queue".into())
            .spawn(move || {
                let result = startup
                    .wait(|| worker_stopped.load(Ordering::Acquire))
                    .and_then(|()| prepare(&worker_stopped))
                    .and_then(|storage| {
                        check_startup(&worker_stopped)?;
                        prepared_storage.publish(storage.clone());
                        Worker::open(
                            history,
                            storage,
                            worker_snapshot.clone(),
                            worker_sender,
                            worker_notice,
                            worker_attention,
                            WorkerStart {
                                stopped: worker_stopped,
                                work: &startup,
                            },
                        )
                    })
                    .and_then(|worker| {
                        drop(startup);
                        worker.run(receiver)
                    });
                match result {
                    Err(error) => {
                        let mut state = worker_snapshot.lock().unwrap_or_else(|e| e.into_inner());
                        state.initializing = false;
                        state.error = Some(error.to_string());
                    }
                    Ok(Some(reply)) => {
                        let _ = reply.send(Ok(()));
                    }
                    Ok(None) => {}
                }
                let _ = notice
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .try_send(());
            })?;
        Ok(Self(Arc::new(Handle {
            activity,
            sender,
            snapshot,
            notices: Mutex::new(Some(notices)),
            stopped,
            storage,
            attention,
        })))
    }

    /// The receiver resolves only after durable acceptance. A refused submission
    /// has not changed the filesystem and belongs to the caller's draft.
    pub fn submit(
        &self,
        request: Request,
        source: Option<Source>,
    ) -> io::Result<oneshot::Receiver<Result<u64, String>>> {
        let (sender, receiver) = oneshot::channel();
        self.send_work(Control::Submit(request, source, None, None, sender))
            .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))?;
        Ok(receiver)
    }

    /// Inspect unresolved journals and uncertain results without enumerating
    /// retained versions. This never reconciles work or grants execution.
    pub fn needs_recovery_review(&self) -> io::Result<oneshot::Receiver<Result<bool, String>>> {
        let (reply, receiver) = oneshot::channel();
        self.send_work(Control::NeedsRecoveryReview(reply))
            .map_err(|_| io::Error::other(fl!("files-jobs-recovery-busy")))?;
        Ok(receiver)
    }

    /// A bounded committed Recovery page, read away from the frontend thread.
    pub fn recovery_page(
        &self,
        cursor: Option<crate::operations::RecoveryCursor>,
        cancellation: Cancellation,
    ) -> io::Result<oneshot::Receiver<Result<crate::operations::RecoveryPage, String>>> {
        let (reply, receiver) = oneshot::channel();
        self.send_work(Control::RecoveryPage(cursor, cancellation, reply))
            .map_err(|_| io::Error::other(fl!("files-jobs-recovery-busy")))?;
        Ok(receiver)
    }

    pub fn snapshot(&self) -> Snapshot {
        self.0
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// An application-owned clipboard supplies its source observation captured
    /// at Copy/Cut. The invoking destination binding is captured independently.
    pub fn submit_transfer(
        &self,
        request: TransferRequest,
        sources: storage::Snapshot,
        destination: Option<Source>,
    ) -> io::Result<oneshot::Receiver<Result<u64, String>>> {
        self.enqueue_transfer(request, sources, destination, None)
    }

    /// Hold the clipboard's retention through durable admission, including if
    /// the caller stops awaiting its reply. The lease grants no execution or
    /// source authority; admission independently validates the current request.
    pub fn submit_transfer_with_lease(
        &self,
        request: TransferRequest,
        sources: storage::Snapshot,
        destination: Option<Source>,
        retained: RetainedSourceLease,
    ) -> io::Result<oneshot::Receiver<Result<u64, String>>> {
        self.enqueue_transfer(request, sources, destination, Some(retained))
    }

    fn enqueue_transfer(
        &self,
        request: TransferRequest,
        sources: storage::Snapshot,
        destination: Option<Source>,
        retained: Option<RetainedSourceLease>,
    ) -> io::Result<oneshot::Receiver<Result<u64, String>>> {
        let (sender, receiver) = oneshot::channel();
        self.send_work(Control::Submit(
            Request::Transfer(request),
            destination,
            Some(sources),
            retained,
            sender,
        ))
        .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))?;
        Ok(receiver)
    }

    /// Prepare process-owned retained data on the metadata worker. This remains
    /// available while a filesystem operation is waiting for a user decision.
    pub fn lease_sources(
        &self,
        paths: Vec<PathBuf>,
    ) -> io::Result<oneshot::Receiver<Result<RetainedSourceLease, String>>> {
        let (sender, receiver) = oneshot::channel();
        self.send_work(Control::LeaseSources(paths, sender))
            .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))?;
        Ok(receiver)
    }

    /// One coalesced wake-up slot. The renderer reads the latest snapshot after
    /// each wake-up; a slow renderer never accumulates a queue of old snapshots.
    pub fn take_notices(&self) -> Option<asynchronous::Receiver<()>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    /// Answers the exact currently displayed question. A stale answer cannot
    /// apply to a later question from the same operation.
    pub fn resolve(&self, id: u64, generation: u64, choice: UndoChoice) -> io::Result<()> {
        self.0
            .sender
            .try_send(Control::Resolve(id, generation, Answer::Undo(choice)))
            .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))
    }

    pub fn resolve_transfer(
        &self,
        id: u64,
        generation: u64,
        choice: TransferReply,
    ) -> io::Result<()> {
        self.0
            .sender
            .try_send(Control::Resolve(id, generation, Answer::Transfer(choice)))
            .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))
    }

    pub fn cancel(&self, id: u64) -> io::Result<()> {
        self.0
            .sender
            .try_send(Control::Cancel(id))
            .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))
    }

    /// Save the retained stopped result, without executing its original task.
    pub fn retry_save(&self, id: u64) -> io::Result<()> {
        // A valid save retry retains its original accepted Job's work hold.
        self.0
            .sender
            .try_send(Control::RetrySave(id))
            .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))
    }

    /// Orderly shutdown is refused while any admitted work or unsaved result
    /// still needs protection. The reply follows release of the history writer.
    pub fn shutdown(&self) -> io::Result<oneshot::Receiver<Result<(), String>>> {
        let (reply, receiver) = oneshot::channel();
        self.0
            .sender
            .try_send(Control::Shutdown(reply))
            .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))?;
        Ok(receiver)
    }

    fn send_work(&self, control: Control) -> io::Result<()> {
        self.0
            .sender
            .try_send(Control::Guarded(Box::new(control), self.0.activity.work()))
            .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))
    }
}

impl Worker {
    fn open(
        history: PathBuf,
        storage: Arc<Storage>,
        snapshot: Arc<Mutex<Snapshot>>,
        sender: Weak<mpsc::SyncSender<Control>>,
        notice: Arc<Mutex<asynchronous::Sender<()>>>,
        attention: Arc<Mutex<attention::Inbox>>,
        start: WorkerStart<'_>,
    ) -> io::Result<Self> {
        let WorkerStart {
            stopped,
            work: startup,
        } = start;
        let engine = Arc::new(OperationEngine::open_with_recovery_check(history, |_| {
            Err(io::Error::other(fl!("files-jobs-interrupted-needs-review")))
        })?);
        let mut worker = Self {
            engine,
            storage,
            snapshot,
            sender,
            notice,
            stopped,
            jobs: BTreeMap::new(),
            queue: VecDeque::new(),
            active: None,
            clipboard_owner: Arc::new(()),
            attention,
        };
        let mut before = None;
        loop {
            let page = worker.engine.request_page(before)?;
            if page.is_empty() {
                break;
            }
            before = page.last().map(RequestRecord::id);
            for record in page {
                let saved: Saved = record.payload()?;
                saved.validate()?;
                let expected = saved.persisted_phase();
                let valid = match (&saved, record.phase()) {
                    (
                        Saved::Plan { error: None, .. },
                        RequestPhase::Queued | RequestPhase::Running,
                    ) => true,
                    _ => expected == record.phase(),
                };
                if !valid {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Operation result and scheduling phase disagree",
                    ));
                }
                let phase = match record.phase() {
                    RequestPhase::Queued => Phase::AwaitingReview,
                    RequestPhase::Running => Phase::Interrupted,
                    _ => saved.stopped_phase(),
                };
                let restored = match worker.restore_mutation(&record, &saved)? {
                    Some(restored) => Some(restored),
                    None => match worker.restore_archive(&record, &saved)? {
                        Some(restored) => Some(restored),
                        None => worker.restore_transfer(&record, &saved, false)?,
                    },
                };
                let (record, saved, phase, restored_error, transfer_confirmation) = match restored {
                    Some(restored) => (
                        restored.record,
                        restored.saved,
                        restored.phase,
                        restored.error,
                        restored.transfer_confirmation,
                    ),
                    None => (record, saved, phase, None, None),
                };
                let mut snapshot = saved.snapshot(record.id(), phase, record.accepted_at())?;
                if phase == Phase::Interrupted {
                    let effects = worker.engine.request_effects(&record)?;
                    for effect in effects {
                        let (operation, steps) =
                            (effect.operation.to_string(), effect.confirmed_steps);
                        let note = if effect.journal_pending {
                            fl!(
                                "files-jobs-interrupted-journal",
                                operation = operation,
                                steps = steps
                            )
                        } else if !effect.history_available {
                            fl!(
                                "files-jobs-interrupted-no-history",
                                operation = operation,
                                steps = steps
                            )
                        } else {
                            fl!(
                                "files-jobs-interrupted-steps",
                                operation = operation,
                                steps = steps
                            )
                        };
                        let detail = snapshot.detail.get_or_insert_with(String::new);
                        if !detail.is_empty() {
                            detail.push('\n');
                        }
                        detail.push_str(&note);
                    }
                }
                snapshot.error = restored_error.clone().or(snapshot.error);
                let transfer_pending = if let Saved::TransferPending { outcome, .. } = &saved {
                    if matches!(phase, Phase::Stopped | Phase::Failed) {
                        match worker.engine.restore_transfer_parts(&record, outcome) {
                            Ok(parts) => {
                                let pending = transfer_resume::Pending {
                                    retry: parts.retry,
                                    remaining: parts.remaining,
                                    unavailable: parts.error.is_some(),
                                };
                                snapshot.transfer_actions = pending.actions();
                                if let Some(error) = parts.error {
                                    snapshot.error = Some(fl!(
                                        "files-jobs-some-work-cannot-continue",
                                        error = error.to_string()
                                    ));
                                }
                                (!pending.empty()).then_some(pending)
                            }
                            Err(error) => {
                                snapshot.error = Some(fl!(
                                    "files-jobs-transfer-cannot-continue",
                                    error = error.to_string()
                                ));
                                None
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                worker.state().jobs.push(snapshot);
                let placement = saved
                    .requirements()
                    .map(|requirements| worker.storage.restore(requirements));
                let source_placement = saved
                    .source_requirements()
                    .map(|requirements| worker.storage.restore(requirements));
                let id = record.id();
                let restored_transfer = matches!(
                    saved,
                    Saved::Completed {
                        transfer: Some(_),
                        ..
                    } | Saved::TransferPending { .. }
                );
                worker.jobs.insert(
                    record.id(),
                    Job {
                        work: (phase.needs_save() || transfer_pending.is_some())
                            .then(|| startup.clone()),
                        record,
                        saved,
                        mutation_confirmation: (phase.needs_save() && !restored_transfer)
                            .then_some(MutationConfirmation::Recorded),
                        transfer_confirmation,
                        reconciliation: None,
                        placement,
                        source_placement,
                        recovery_pending: None,
                        recovery_execution: None,
                        transfer_pending,
                        transfer_execution: None,
                        cut_progress: None,
                        cut_claim: None,
                        observed_transfer: None,
                        observed_sources: None,
                        destination_observation: None,
                        clipboard: None,
                        cancellation: Cancellation::default(),
                        decision: None,
                        generation: 0,
                    },
                );
                if phase.needs_save() {
                    worker.set_job_phase(id, phase, restored_error);
                }
            }
        }
        worker.refresh_undo();
        worker.state().initializing = false;
        worker.publish();
        Ok(worker)
    }

    fn state(&self) -> std::sync::MutexGuard<'_, Snapshot> {
        self.snapshot.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn refresh_undo(&self) {
        self.state().undo = self
            .engine
            .latest_undo_id()
            .zip(self.engine.undo_label())
            .map(|(id, label)| UndoHead { id, label });
    }

    fn publish(&self) {
        let _ = self
            .notice
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
    }

    fn run(
        mut self,
        receiver: mpsc::Receiver<Control>,
    ) -> io::Result<Option<oneshot::Sender<Result<(), String>>>> {
        // Deferring admission must not prevent a later cancelled Recovery read
        // from retiring. Bound this backlog as well as the input channel.
        let mut pending: VecDeque<(Box<Control>, Work)> = VecDeque::new();
        loop {
            if self.stopped.load(Ordering::Acquire) {
                for job in self.jobs.values() {
                    job.cancellation.cancel();
                }
                break;
            }
            for _ in 0..pending.len() {
                let (control, work) = pending.pop_front().unwrap();
                if control.preparation_cancelled() {
                    let message = if matches!(&*control, Control::RecoveryPage(..)) {
                        fl!("files-jobs-recovery-cancelled-removal")
                    } else {
                        fl!("files-jobs-drag-cancelled-removal")
                    };
                    control.refuse_preparation(&message);
                } else {
                    pending.push_back((control, work));
                }
            }
            let next = if pending.front().is_some_and(|(_, work)| work.ready()) {
                let (control, work) = pending.pop_front().unwrap();
                Some((*control, Some(work)))
            } else if pending.is_empty() {
                receiver.recv().ok().map(|control| (control, None))
            } else {
                match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(control) => Some((control, None)),
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => None,
                }
            };
            let Some((control, mut work)) = next else {
                break;
            };
            let control = match control {
                Control::Guarded(control, work) => {
                    if pending.len() >= 1024 {
                        control.refuse_admission();
                    } else {
                        pending.push_back((control, work));
                    }
                    continue;
                }
                control => control,
            };
            let mut retain_work = None;
            match control {
                Control::Guarded(..) => {
                    return Err(io::Error::other("Nested filesystem admission is invalid."));
                }
                Control::NeedsRecoveryReview(reply) => {
                    let result = self
                        .engine
                        .interrupted_operations(&Cancellation::default())
                        .map(|issues| {
                            !issues.is_empty()
                                || self.jobs.values().any(|job| job.saved.uncertain())
                        })
                        .map_err(|error| error.to_string());
                    let _ = reply.send(result);
                }
                Control::ResumeRecovery(id, part, reply) => {
                    let result = self
                        .resume_recovery(id, part)
                        .map_err(|error| error.to_string());
                    retain_work = result.as_ref().ok().copied();
                    let _ = reply.send(result);
                }
                Control::ResumeTransfer(id, part, reply) => {
                    let result = self.resume_transfer(id, part).map_err(|e| e.to_string());
                    retain_work = result.as_ref().ok().copied();
                    let _ = reply.send(result);
                }
                Control::AbandonTransfer(id, reply) => {
                    let result = self.abandon_transfer(id).map_err(|e| e.to_string());
                    let _ = reply.send(result);
                }
                Control::DismissRequest(id, reply) => {
                    let result = self.dismiss_request(id).map_err(|error| error.to_string());
                    let _ = reply.send(result);
                }
                Control::RecoveryPage(cursor, cancellation, reply) => {
                    let _ = reply.send(
                        self.engine
                            .recovery_items(cursor, &cancellation)
                            .map_err(|error| error.to_string()),
                    );
                }
                Control::PrepareRename(source, binding, reply) => {
                    let result = PreparedRename::prepare(
                        &self.engine,
                        &self.clipboard_owner,
                        source,
                        binding,
                        self.storage.snapshot(),
                    )
                    .map_err(|error| error.to_string());
                    let _ = reply.send(result);
                }
                Control::Rename(prepared, name, reply) => {
                    let result = self
                        .admit_rename(prepared, name)
                        .map_err(|error| error.to_string());
                    retain_work = result.as_ref().ok().copied();
                    let _ = reply.send(result);
                }
                Control::PrepareClipboard(selection, placement, previous, reply) => {
                    let result = PreparedSelection::prepare(
                        &self.engine,
                        selection,
                        placement,
                        &self.clipboard_owner,
                        previous.as_ref(),
                    )
                    .map_err(|error| error.to_string());
                    let _ = reply.send(result);
                }
                Control::PasteClipboard(selection, destination, binding, reply) => {
                    let result = selection
                        .claim(&self.clipboard_owner)
                        .and_then(|paste| {
                            let observed = paste.observe(destination)?;
                            self.admit(
                                Request::Transfer(observed.request().clone()),
                                binding,
                                Some(paste.placement()),
                                Some(PreparedTransfer {
                                    clipboard: paste,
                                    observed,
                                    destination: None,
                                    destination_observation: None,
                                }),
                            )
                        })
                        .map_err(|error| error.to_string());
                    retain_work = result.as_ref().ok().copied();
                    let _ = reply.send(result);
                }
                Control::PrepareDrag(selection, placement, cancellation, drag_work, reply) => {
                    let result = PreparedDrag::prepare(
                        &self.engine,
                        selection,
                        placement,
                        &self.clipboard_owner,
                        cancellation,
                        drag_work,
                    )
                    .map_err(|e| e.to_string());
                    let _ = reply.send(result);
                }
                Control::PrepareDrop(drag, target, placement, cancellation, reply) => {
                    let result = PreparedDrop::prepare(
                        drag,
                        target,
                        placement,
                        &self.clipboard_owner,
                        &cancellation,
                    )
                    .map_err(|e| e.to_string());
                    let _ = reply.send(result);
                }
                Control::Drop(choice, reply) => {
                    let result = choice
                        .claim(&self.clipboard_owner)
                        .and_then(|prepared| {
                            self.admit(
                                Request::Transfer(prepared.observed.request().clone()),
                                None,
                                Some(prepared.clipboard.placement()),
                                Some(prepared),
                            )
                        })
                        .map_err(|e| e.to_string());
                    retain_work = result.as_ref().ok().copied();
                    let _ = reply.send(result);
                }
                Control::Submit(request, source, source_placement, retained, reply) => {
                    let result = self
                        .admit(request, source, source_placement, None)
                        .map_err(|e| e.to_string());
                    retain_work = result.as_ref().ok().copied();
                    let _ = reply.send(result);
                    drop(retained);
                }
                Control::LeaseSources(paths, reply) => {
                    let result = self
                        .engine
                        .capture_retained_sources(&paths)
                        .and_then(|sources| self.engine.lease_retained_sources(&sources))
                        .map_err(|error| error.to_string());
                    let _ = reply.send(result);
                }
                Control::Complete(id, result) => self.complete(id, *result),
                Control::Question(id, question, reply) => {
                    if self.active == Some(id)
                        && let Some(job) = self.jobs.get_mut(&id)
                    {
                        job.generation += 1;
                        let generation = job.generation;
                        job.decision = Some((generation, reply));
                        let mut state = self.state();
                        if let Some(job) = state.jobs.iter_mut().find(|job| job.id == id) {
                            job.phase = Phase::Waiting;
                            job.decision = Some(Decision {
                                generation,
                                question,
                            });
                        }
                        self.record_attention(&mut state, id);
                    }
                }
                Control::Resolve(id, generation, choice) => {
                    let offered = self
                        .state()
                        .jobs
                        .iter()
                        .find(|job| job.id == id)
                        .and_then(|job| job.decision.as_ref())
                        .is_some_and(|decision| {
                            decision.generation == generation
                                && match (&decision.question, &choice) {
                                    (Question::Undo(_), Answer::Undo(_)) => true,
                                    (Question::Transfer(question), Answer::Transfer(reply)) => {
                                        question.accepts(*reply)
                                    }
                                    (Question::Recovery(question), Answer::Transfer(reply)) => {
                                        question.accepts(*reply)
                                            && !matches!(
                                                reply,
                                                TransferReply::Resolve {
                                                    choice:
                                                        crate::transfers::ConflictChoice::Replace,
                                                    ..
                                                }
                                            )
                                    }
                                    _ => false,
                                }
                        });
                    if offered
                        && let Some(job) = self.jobs.get_mut(&id)
                        && job.decision.as_ref().is_some_and(|(current, reply)| {
                            *current == generation && reply.accepts(&choice)
                        })
                        && let Some((_, reply)) = job.decision.take()
                    {
                        reply.send(choice);
                        self.set_job_phase(id, Phase::Running, None);
                    }
                }
                Control::Cancel(id) => {
                    if let Some(job) = self.jobs.get_mut(&id) {
                        if self.active == Some(id) {
                            job.cancellation.cancel();
                            if let Some((_, reply)) = job.decision.take() {
                                reply.cancel();
                            }
                        } else if self.queue.contains(&id) {
                            self.queue.retain(|queued| *queued != id);
                            // No filesystem work has started; finish with a
                            // nonexecutable cancellation result.
                            let request = match &job.saved {
                                Saved::Plan { request, .. } => Some(request),
                                _ => None,
                            };
                            let completion = Completion::cancelled_before_start(
                                request,
                                job.recovery_execution.as_ref(),
                                job.unstarted_transfer(None),
                            );
                            self.settle(id, Ok(completion));
                        }
                    }
                }
                Control::RetrySave(id) => self.retry_save(id),
                Control::Shutdown(reply) => {
                    if !pending.is_empty() || self.state().holds_application() {
                        let _ = reply.send(Err(fl!("files-jobs-close-worker-busy")));
                    } else {
                        return Ok(Some(reply));
                    }
                }
                Control::Stop => {
                    for job in self.jobs.values() {
                        job.cancellation.cancel();
                    }
                    break;
                }
            }
            if let Some(id) = retain_work {
                self.jobs.get_mut(&id).unwrap().work = work.take();
            }
            self.dispatch();
            self.publish();
        }
        Ok(None)
    }

    fn admit(
        &mut self,
        request: Request,
        source: Option<Source>,
        source_placement: Option<storage::Snapshot>,
        prepared: Option<PreparedTransfer>,
    ) -> io::Result<u64> {
        self.admit_observed(request, source, source_placement, prepared, None)
    }

    fn admit_observed(
        &mut self,
        request: Request,
        source: Option<Source>,
        source_placement: Option<storage::Snapshot>,
        prepared: Option<PreparedTransfer>,
        rename: Option<&rename::Ready>,
    ) -> io::Result<u64> {
        if !request.valid()
            || source.as_ref().is_some_and(|source| {
                !source.directory.is_absolute() || !source.resolved.is_absolute()
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "An operation requires a nonempty selection and absolute source and destination paths",
            ));
        }
        self.check_attention_capacity()?;
        let reconciliation = if matches!(request, Request::Reconcile | Request::KeepRecovery { .. })
        {
            Some(ReconciliationReview {
                journals: self.engine.capture_reconciliation()?,
                reports: self
                    .jobs
                    .iter()
                    .filter(|(_, job)| job.saved.uncertain() || job.recorded_running_transfer())
                    .map(|(id, _)| *id)
                    .collect(),
            })
        } else {
            None
        };
        let source_placement = source_placement.or_else(|| {
            matches!(request, Request::Transfer(_) | Request::Recover { .. })
                .then(|| self.storage.snapshot())
        });
        let mut placement = prepared
            .as_ref()
            .and_then(|plan| plan.destination.clone())
            .or_else(|| rename.map(|ready| ready.placement.clone()))
            .unwrap_or_else(|| self.storage.snapshot());
        if let Some(source) = source {
            placement.bind_folder(source.directory, source.resolved);
        }
        let mut saved = Saved::Plan {
            version: 1,
            request: request.clone(),
            transfer_checkpoint: None,
            requirements: placement.requirements(),
            source_requirements: source_placement
                .as_ref()
                .map(storage::Snapshot::requirements),
            error: None,
            rejected_before_execution: false,
        };
        let (clipboard, prepared_observation, destination_observation) = match prepared {
            Some(prepared) => (
                Some(prepared.clipboard),
                Some(prepared.observed),
                prepared.destination_observation,
            ),
            None => (None, None, None),
        };
        let observed_transfer = match (&request, prepared_observation) {
            (Request::Transfer(_), Some(observed)) => Some(observed),
            (Request::Transfer(transfer), None) => Some(transfer.observe_sources()?),
            _ => None,
        };
        if let (
            Some(observed),
            Saved::Plan {
                transfer_checkpoint,
                ..
            },
        ) = (&observed_transfer, &mut saved)
        {
            let checkpoint =
                crate::transfers::TransferContinuation::unstarted(observed).into_saved_plan();
            if checkpoint.matches_request(observed.request()) {
                *transfer_checkpoint = Some(Box::new(checkpoint));
            }
        }
        let observed_sources = match &request {
            Request::Mutation(OperationRequest::Rename {
                source,
                destination,
            }) => Some(match rename {
                Some(ready) => ready.observed.clone(),
                None => crate::transfers::ObservedSources::capture_rename(source, destination)?,
            }),
            Request::Mutation(OperationRequest::RenameBatch { renames }) => {
                Some(crate::transfers::ObservedSources::capture_rename_batch(
                    &renames
                        .iter()
                        .map(|step| step.source.clone())
                        .collect::<Vec<_>>(),
                )?)
            }
            Request::Trash { sources, .. } => {
                Some(crate::transfers::ObservedSources::capture(sources)?)
            }
            Request::Restore { item, .. } => Some(crate::transfers::ObservedSources::capture(
                std::slice::from_ref(&item.path),
            )?),
            Request::Delete { sources } => {
                Some(crate::transfers::ObservedSources::capture(sources)?)
            }
            _ => None,
        };
        let record = if let Some(ready) = rename {
            self.engine
                .queue_request_consuming_source_lease(&saved, &ready.lease)?
        } else if let Request::Transfer(transfer) = &request {
            if let Some(paste) = &clipboard {
                self.engine
                    .queue_request_with_source_lease(&saved, paste.lease())?
            } else {
                let sources = self.engine.capture_retained_sources(&transfer.sources)?;
                self.engine.queue_request_with_sources(&saved, &sources)?
            }
        } else if let Request::Recover { item, .. } = &request {
            let path = self.engine.recovery_copy_source_path(item)?;
            let sources = self.engine.capture_retained_sources(&[path])?;
            self.engine.queue_request_with_sources(&saved, &sources)?
        } else if let Request::Trash { sources, .. } = &request {
            let retained = self.engine.capture_retained_sources(sources)?;
            self.engine.queue_request_with_sources(&saved, &retained)?
        } else if let Request::Mutation(OperationRequest::Rename { source, .. }) = &request {
            let retained = self
                .engine
                .capture_retained_sources(std::slice::from_ref(source))?;
            self.engine.queue_request_with_sources(&saved, &retained)?
        } else if let Request::Mutation(OperationRequest::RenameBatch { renames }) = &request {
            let sources: Vec<_> = renames.iter().map(|step| step.source.clone()).collect();
            let retained = self.engine.capture_retained_sources(&sources)?;
            self.engine.queue_request_with_sources(&saved, &retained)?
        } else {
            self.engine.queue_request(&saved)?
        };
        let id = record.id();
        self.state().jobs.push(JobSnapshot {
            id,
            accepted_at: record.accepted_at(),
            title: request.title(),
            paths: request.paths().into(),
            phase: Phase::Queued,
            error: None,
            detail: None,
            decision: None,
            progress: None,
            transfer: None,
            trash: None,
            restore: None,
            delete: None,
            recovery: None,
            recovery_actions: RecoveryActions::default(),
            archive: None,
            transfer_actions: TransferActions::default(),
            dismissible: false,
        });
        self.jobs.insert(
            id,
            Job {
                work: None,
                record,
                saved,
                mutation_confirmation: None,
                transfer_confirmation: None,
                reconciliation,
                placement: Some(placement),
                source_placement,
                recovery_pending: None,
                recovery_execution: None,
                transfer_pending: None,
                transfer_execution: None,
                cut_progress: None,
                cut_claim: None,
                observed_transfer,
                observed_sources,
                destination_observation,
                clipboard,
                cancellation: Cancellation::default(),
                decision: None,
                generation: 0,
            },
        );
        self.queue.push_back(id);
        Ok(id)
    }

    fn dispatch(&mut self) {
        if self.active.is_some() || self.state().jobs.iter().any(|job| job.phase.needs_save()) {
            return;
        }
        let Some(id) = self.queue.pop_front() else {
            return;
        };
        let uncertain_operation = self.jobs.values().any(|job| job.saved.uncertain());
        let Some(job) = self.jobs.get_mut(&id) else {
            return;
        };
        let Saved::Plan { request, .. } = &job.saved else {
            return;
        };
        match self
            .engine
            .checkpoint_request(&job.record, RequestPhase::Running, &job.saved)
        {
            Ok(record) => job.record = record,
            Err(error) => {
                self.set_job_phase(id, Phase::StartSaveFailed, Some(error.to_string()));
                return;
            }
        }
        let request = request.clone();
        let execution_record = job.record.clone();
        let work = job.work.clone();
        let observed_transfer = job.observed_transfer.clone().or_else(|| {
            job.transfer_execution
                .as_ref()
                .and_then(crate::transfers::TransferContinuation::observed_transfer)
                .cloned()
        });
        let transfer_execution = job.transfer_execution.clone();
        let cancelled_transfer = job.unstarted_transfer(None);
        let observed_sources = job.observed_sources.clone();
        let destination_observation = job.destination_observation.clone();
        let recovery_execution = job.recovery_execution.clone();
        let reconciliation = job.reconciliation.clone();
        let is_reconciliation =
            matches!(request, Request::Reconcile | Request::KeepRecovery { .. });
        let is_recovery = matches!(request, Request::Recover { .. });
        let is_transfer = matches!(request, Request::Transfer(_));
        let is_trash = matches!(request, Request::Trash { .. });
        let is_restore = matches!(request, Request::Restore { .. });
        let is_delete = matches!(
            request,
            Request::Delete { .. } | Request::DeleteTrash { .. }
        );
        let engine = self.engine.clone();
        let Some(placement) = job.placement.clone() else {
            return;
        };
        let source_placement = job
            .source_placement
            .clone()
            .unwrap_or_else(|| placement.clone());
        let cancellation = job.cancellation.clone();
        let snapshot = self.snapshot.clone();
        let notice = self.notice.clone();
        let Some(sender) = self.sender.upgrade() else {
            return;
        };
        self.active = Some(id);
        self.set_job_phase(id, Phase::Running, None);
        let result = std::thread::Builder::new().name("files-mutation".into()).spawn(move || {
            // The execution thread retains admission even if its coordinator
            // disappears while cancellation or a filesystem call is settling.
            let _work = work;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let execution = engine.execution(&execution_record);
                if uncertain_operation && !is_recovery && !is_reconciliation {
                    return Err(io::Error::other(fl!("files-jobs-needs-reconciliation")));
                }
                    let mut accesses = request.accesses(&engine)?;
                    if let Some(plan) = &recovery_execution { accesses.extend(plan.storage_destinations().map(Access::entry)); }
                    if !matches!(request, Request::Transfer(_)) { placement.check(&accesses)?; }
                let interrupted = match engine.interrupted_operations(&cancellation) {
                    Err(error) if (is_recovery || is_transfer || is_trash || is_restore || is_delete) && cancellation.is_cancelled() && error.kind() == io::ErrorKind::Interrupted => return Ok(Completion::cancelled_before_start(Some(&request), recovery_execution.as_ref(), cancelled_transfer)),
                    result => result?,
                };
                if !interrupted.is_empty() && !is_recovery && !is_reconciliation {
                    return Err(io::Error::other(fl!("files-jobs-interrupted-needs-review")));
                }
                match request {
                    Request::KeepRecovery { item } => {
                        engine.keep_in_recovery(&item, &cancellation)?;
                        Ok(Completion { summary: OperationSummary { id: 0, label: fl!("files-job-keep-in-recovery", name = item.name().to_string_lossy().into_owned()), paths: item.entry.iter().map(|entry| entry.path.clone()).collect() }, detail: Some(fl!("files-jobs-kept-in-recovery")), transfer: None, trash: None, restore: None, delete: None, recovery: None, mutation_confirmation: None })
                    }
                    Request::Reconcile => {
                        let observed = reconciliation.ok_or_else(|| io::Error::other("This owner has no original journal observation. Refresh Recovery before retrying."))?;
                        engine.reconcile_with_check(&observed.journals, &cancellation, |paths| placement.check(&paths.iter().cloned().map(Access::entry).collect::<Vec<_>>()))?;
                        Ok(Completion { summary: OperationSummary { id: 0, label: fl!("files-jobs-retry-interrupted"), paths: observed.journals.paths().to_vec() }, detail: Some(fl!("files-jobs-journals-rechecked")), transfer: None, trash: None, restore: None, delete: None, recovery: None, mutation_confirmation: None })
                    }
                    Request::Recover { item, destination } => {
                        let title = fl!("files-job-recover", name = item.name().to_string_lossy().into_owned());
                        let report = recovery::run(&engine, &item, &destination, &cancellation, recovery::Placement { source: &source_placement, destination: &placement }, transfer::Publication { id, snapshot, notice, sender: sender.clone() }, recovery_execution.as_ref());
                        Ok(Completion { summary: OperationSummary { id: report.report.transfer.operation.unwrap_or(0), label: title, paths: report.report.transfer.destinations.clone() }, detail: Some(report.report.detail()), recovery: Some(report), transfer: None, trash: None, restore: None, delete: None, mutation_confirmation: None })
                    }
                    Request::Mutation(request) => {
                        let result = match request {
                            OperationRequest::Rename { source, destination } => {
                                let observed = observed_sources.as_ref().ok_or_else(|| io::Error::other("Rename needs its original selected entry before execution."))?;
                                observed.require(&source)?;
                                execution.rename(source, destination, observed, &cancellation)
                            }
                            OperationRequest::RenameBatch { renames } => {
                                let observed = observed_sources.as_ref().ok_or_else(|| io::Error::other("Rename needs its original selected entries before execution."))?;
                                let outcome = execution.rename_batch(&renames, observed, &cancellation)?;
                                return batch_rename_completion(&renames, outcome);
                            }
                            request @ (OperationRequest::CreateFolder { .. } | OperationRequest::CreateFile { .. }) => execution.mutate(request, &cancellation),
                        };
                        result
                            .map(|outcome| Completion { summary: outcome.summary, detail: None, transfer: None, trash: None, restore: None, delete: None, recovery: None, mutation_confirmation: outcome.confirmation })
                    }
                    Request::Trash { data_home, sources } => {
                        let observed = observed_sources.ok_or_else(|| io::Error::other("Trash needs its original selected entries before execution."))?;
                        let report = match engine.trash_observed(&data_home, &sources, &observed, &cancellation) {
                            Ok(outcome) => TrashReport::from_outcome(outcome),
                            Err(failure) => {
                                let mut report = TrashReport::from_outcome(*failure.outcome);
                                if failure.error.kind() != io::ErrorKind::Interrupted {
                                    report.interrupted = Some(failure.error.to_string());
                                }
                                report
                            }
                        };
                        Ok(Completion {
                            summary: OperationSummary { id: report.operation.unwrap_or(0), label: fl!("files-job-trash", count = sources.len()), paths: report.completed.clone() },
                            detail: Some(report.detail()), transfer: None, trash: Some(report), restore: None, delete: None, recovery: None, mutation_confirmation: None,
                        })
                    }
                    Request::Restore { item, destination } => {
                        let observed = observed_sources.ok_or_else(|| io::Error::other("Restore needs its original selected entry before execution."))?;
                        observed.require(&item.path)?;
                        let report = match engine.restore_with_outcome(&item, destination.as_deref(), &cancellation) {
                            Ok(summary) => RestoreReport::completed(item.path.clone(), summary),
                            Err(failure) => RestoreReport::from_failure(item.path.clone(), failure, &cancellation),
                        };
                        Ok(Completion {
                            summary: OperationSummary { id: report.operation.unwrap_or(0), label: fl!("files-job-restore", name = item.name().to_string_lossy().into_owned()), paths: report.restored.iter().cloned().collect() },
                            detail: Some(report.detail()), transfer: None, trash: None, restore: Some(report), delete: None, recovery: None, mutation_confirmation: None,
                        })
                    }
                    request @ (Request::Delete { .. } | Request::DeleteTrash { .. }) => {
                        let title = request.title();
                        let publication = transfer::Publication { id, snapshot, notice, sender: sender.clone() };
                        let mut last = None::<std::time::Instant>;
                        let mut progress = |path: &std::path::Path, totals: crate::operations::deletion::DeleteProgress| {
                            // Bound presentation work for folders with many entries.
                            if last.is_none_or(|last| last.elapsed() >= std::time::Duration::from_millis(100)) {
                                last = Some(std::time::Instant::now());
                                publication.progress(TransferProgress { path: path.to_owned(), completed: totals.entries as usize, bytes: totals.bytes, ..TransferProgress::default() });
                            }
                        };
                        let (from_trash, outcome) = match request {
                            Request::DeleteTrash { items, .. } => (true, engine.delete_trashed(&items, &cancellation, &mut progress)?),
                            Request::Delete { sources } => {
                                let observed = observed_sources.ok_or_else(|| io::Error::other("Deletion needs its original selected entries before execution."))?;
                                (false, engine.delete_observed(&sources, &observed, &cancellation, &mut progress)?)
                            }
                            _ => unreachable!(),
                        };
                        let report = DeleteReport::from_outcome(from_trash, outcome);
                        Ok(Completion {
                            summary: OperationSummary { id: 0, label: title, paths: report.completed.clone() },
                            detail: Some(report.detail()), transfer: None, trash: None, restore: None, delete: Some(report), recovery: None, mutation_confirmation: None,
                        })
                    }
                    Request::Archive(archive) => {
                        let title = Request::Archive(archive.clone()).title();
                        let publication = transfer::Publication { id, snapshot, notice, sender: sender.clone() };
                        let mut last = None::<std::time::Instant>;
                        let mut progress = |progress: &crate::archives::Progress| {
                            // Bound presentation work for archives with many entries.
                            if last.is_none_or(|last| last.elapsed() >= std::time::Duration::from_millis(100)) {
                                last = Some(std::time::Instant::now());
                                publication.progress(TransferProgress {
                                    path: progress.path.clone(),
                                    completed: usize::try_from(progress.entries).unwrap_or(usize::MAX),
                                    bytes: progress.bytes,
                                    total: progress.total_entries.zip(progress.total_bytes).map(|(items, bytes)| TransferTotal { items, bytes, complete: true }),
                                    ..TransferProgress::default()
                                });
                            }
                        };
                        let outcome = crate::archives::run(&archive, &cancellation, crate::archives::Limits::default(), &mut progress)?;
                        Ok(Completion {
                            summary: OperationSummary { id: 0, label: title, paths: vec![outcome.output.clone()] },
                            detail: Some(outcome.detail(&archive)), transfer: None, trash: None, restore: None, delete: None, recovery: None, mutation_confirmation: None,
                        })
                    }
                    Request::Undo { expected } => {
                        let outcome = engine.undo_interactive(Some(expected), &cancellation, |conflict| {
                            let (reply, receiver) = mpsc::sync_channel(1);
                            if sender.send(Control::Question(id, Question::Undo(conflict), QuestionReply::Undo(reply))).is_err() {
                                return UndoChoice::Cancel;
                            }
                            loop {
                                if cancellation.is_cancelled() { return UndoChoice::Cancel; }
                                match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                                        Ok(choice) => return if placement.check(&accesses).is_ok() { choice } else { UndoChoice::Cancel },
                                    Err(mpsc::RecvTimeoutError::Timeout) => {},
                                    Err(mpsc::RecvTimeoutError::Disconnected) => return UndoChoice::Cancel,
                                }
                            }
                        })?;
                        let mut detail = fl!(
                            "files-undo-outcome",
                            reversed = outcome.completed,
                            kept = outcome.kept.len(),
                            skipped = outcome.skipped.len(),
                            dismissed = outcome.dismissed.len(),
                            unrestored = outcome.unrestored_previous.len(),
                            unremoved = outcome.unremoved.len(),
                            remaining = outcome.remaining
                        );
                        if outcome.cancelled {
                            detail = fl!("files-undo-outcome-cancelled", outcome = detail);
                        }
                        Ok(Completion { summary: outcome.summary, detail: Some(detail), transfer: None, trash: None, restore: None, delete: None, recovery: None, mutation_confirmation: None })
                    }
                    Request::Transfer(request) => {
                        let title = Request::Transfer(request.clone()).title();
                        let observed = observed_transfer.ok_or_else(|| io::Error::other("The transfer needs its original source observation before execution."))?;
                        let outcome = transfer::run(&execution, &observed, &cancellation, transfer::Placement { source: &source_placement, destination: &placement }, destination_observation.as_ref(),
                            transfer_execution.as_ref(),
                            transfer::Publication { id, snapshot, notice, sender: sender.clone() });
                        Ok(Completion {
                            summary: OperationSummary { id: outcome.outcome.report.operation.unwrap_or(0), label: title, paths: outcome.outcome.report.destinations.clone() },
                            detail: Some(outcome.outcome.detail()), transfer: Some(Box::new(outcome)), trash: None, restore: None, delete: None, recovery: None, mutation_confirmation: None,
                        })
                    }
                }
            }));
            let result = match result {
                Ok(result) => result.map_err(|error| if is_recovery || is_transfer || is_trash || is_restore || is_delete { Failure::BeforeExecution(error.to_string()) } else { Failure::DuringExecution(error.to_string()) }),
                Err(_) => Err(Failure::DuringExecution(fl!("files-jobs-worker-stopped"))),
            };
            drop(engine);
            let _ = sender.send(Control::Complete(id, Box::new(result)));
        });
        if let Err(error) = result {
            self.complete(id, Err(Failure::BeforeExecution(error.to_string())));
        }
    }

    fn set_job_phase(&self, id: u64, phase: Phase, error: Option<String>) {
        self.update_job(id, phase, error, None);
    }

    fn update_job(
        &self,
        id: u64,
        phase: Phase,
        error: Option<String>,
        replacement: Option<JobSnapshot>,
    ) {
        let mut state = self.state();
        if let Some(job) = state.jobs.iter_mut().find(|job| job.id == id) {
            if let Some(replacement) = replacement {
                *job = replacement;
            }
            job.phase = phase;
            job.error = error;
            if !matches!(
                phase,
                Phase::Failed | Phase::AwaitingReview | Phase::Interrupted
            ) {
                job.dismissible = false;
            }
            if phase != Phase::Waiting {
                job.decision = None;
            }
        }
        self.record_attention(&mut state, id);
        // Completion, attention and bounded presentation become visible
        // together. A concurrent snapshot must never observe the extra row
        // between publishing Finished and removing older finished rows.
        let mut finished: Vec<_> = state
            .jobs
            .iter()
            .filter(|job| job.phase == Phase::Finished)
            .map(|job| job.id)
            .collect();
        finished.sort_unstable();
        finished.truncate(finished.len().saturating_sub(100));
        state
            .jobs
            .retain(|job| finished.binary_search(&job.id).is_err());
    }

    fn complete(&mut self, id: u64, result: Result<Completion, Failure>) {
        if self.active != Some(id) {
            return;
        }
        self.active = None;
        if result.is_ok() {
            let reviewed = self
                .jobs
                .get(&id)
                .and_then(|job| job.reconciliation.as_ref())
                .map(|review| review.reports.clone())
                .unwrap_or_default();
            let work = self.jobs.get(&id).and_then(|job| job.work.clone());
            self.acknowledge_reconciled(&reviewed, work);
        }
        self.settle(id, result);
    }

    fn acknowledge_reconciled(&mut self, ids: &[u64], work: Option<Work>) {
        for id in ids {
            if self.restore_reviewed_transfer(*id, work.clone()) {
                continue;
            }
            let Some(job) = self.jobs.get_mut(id) else {
                continue;
            };
            if !job.saved.uncertain() {
                continue;
            }
            // Keep the historical facts and source references. The wrapper
            // records successful review, never a newly executable plan.
            job.saved = Saved::Reconciled {
                version: 1,
                previous: Box::new(job.saved.clone()),
            };
            let (phase, error) = match self.engine.checkpoint_request(
                &job.record,
                job.saved.persisted_phase(),
                &job.saved,
            ) {
                Ok(record) => {
                    job.record = record;
                    (job.saved.stopped_phase(), job.saved.error())
                }
                Err(error) => (
                    Phase::SaveFailed,
                    Some(fl!(
                        "files-jobs-reconciliation-not-saved",
                        error = error.to_string()
                    )),
                ),
            };
            if phase.needs_save() {
                job.work = work.clone();
            }
            let replacement = job.recovery_snapshot(*id, phase).ok();
            self.update_job(*id, phase, error, replacement);
        }
    }

    fn settle(&mut self, id: u64, result: Result<Completion, Failure>) {
        let Some(job) = self.jobs.get_mut(&id) else {
            return;
        };
        let (phase, persisted_phase, error, saved) = match result {
            Ok(mut completion) => {
                job.transfer_confirmation = completion
                    .transfer
                    .as_mut()
                    .and_then(|transfer| transfer.confirmation.take());
                job.mutation_confirmation = completion.mutation_confirmation;
                if let Some(transfer) = completion.transfer.as_ref() {
                    job.transfer_pending = transfer.pending.clone();
                }
                let saved = if let Saved::Plan {
                    request: request @ Request::Archive(_),
                    ..
                } = &job.saved
                {
                    // A successful run has exactly one result; none means it
                    // was cancelled before it started.
                    Saved::ArchiveResult {
                        version: 1,
                        title: request.title(),
                        report: match completion.summary.paths.first() {
                            Some(output) => {
                                ArchiveReport::finished(output.clone(), completion.detail.clone())
                            }
                            None => ArchiveReport::cancelled(),
                        },
                    }
                } else if let Some(finished) = completion.recovery {
                    job.recovery_pending = finished.pending;
                    Saved::RecoveryResult {
                        version: 1,
                        title: completion.summary.label,
                        report: finished.report,
                    }
                } else if let Some(report) = completion.restore {
                    Saved::RestoreResult {
                        version: 1,
                        title: completion.summary.label,
                        report,
                    }
                } else if let Some(report) = completion.trash {
                    Saved::TrashResult {
                        version: 1,
                        title: completion.summary.label,
                        report,
                    }
                } else if let Some(report) = completion.delete {
                    Saved::DeleteResult {
                        version: 1,
                        title: completion.summary.label,
                        report,
                    }
                } else {
                    match completion.transfer {
                        Some(transfer) if transfer.outcome.phase() != RequestPhase::Finished => {
                            Saved::TransferPending {
                                version: 1,
                                title: completion.summary.label,
                                outcome: Box::new(transfer.outcome),
                                requirements: transfer.destination,
                                source_requirements: transfer.sources,
                            }
                        }
                        transfer => Saved::Completed {
                            version: 1,
                            title: completion.summary.label,
                            paths: completion.summary.paths,
                            detail: completion.detail,
                            transfer: transfer.map(|transfer| transfer.outcome.report),
                        },
                    }
                };
                (
                    saved.stopped_phase(),
                    saved.persisted_phase(),
                    saved.error(),
                    saved,
                )
            }
            Err(failure) => {
                let (error, before_execution) = match failure {
                    Failure::BeforeExecution(error) => (error, true),
                    Failure::DuringExecution(error) => (error, false),
                };
                let saved = match &job.saved {
                    Saved::Plan {
                        request: Request::Transfer(_),
                        ..
                    } if before_execution => {
                        let finished = job
                            .unstarted_transfer(Some(error))
                            .expect("admitted transfer has its original execution owner");
                        job.transfer_pending = finished.pending;
                        Saved::TransferPending {
                            version: 1,
                            title: match &job.saved {
                                Saved::Plan { request, .. } => request.title(),
                                _ => unreachable!(),
                            },
                            outcome: Box::new(finished.outcome),
                            requirements: finished.destination,
                            source_requirements: finished.sources,
                        }
                    }
                    Saved::Plan {
                        request: request @ Request::Recover { destination, .. },
                        ..
                    } => {
                        let report = if before_execution
                            && let Some(plan) = job.recovery_execution.as_ref()
                        {
                            let finished = recovery::Finished::paused(plan, error);
                            job.recovery_pending = finished.pending;
                            finished.report
                        } else {
                            RecoveryReport::failed(destination.clone(), error, !before_execution)
                        };
                        Saved::RecoveryResult {
                            version: 1,
                            title: request.title(),
                            report,
                        }
                    }
                    Saved::Plan {
                        request: request @ Request::Restore { item, .. },
                        ..
                    } => Saved::RestoreResult {
                        version: 1,
                        title: request.title(),
                        report: RestoreReport::failed(item.path.clone(), error, !before_execution),
                    },
                    Saved::Plan {
                        request: request @ (Request::Delete { .. } | Request::DeleteTrash { .. }),
                        ..
                    } => Saved::DeleteResult {
                        version: 1,
                        title: request.title(),
                        // An execution failure may follow partial removal;
                        // every selected entry is reported for review.
                        report: DeleteReport::unstarted(
                            matches!(request, Request::DeleteTrash { .. }),
                            request.paths(),
                            Some(error),
                        ),
                    },
                    Saved::Plan {
                        request: request @ Request::Trash { sources, .. },
                        ..
                    } => {
                        let selected: Vec<_> = sources
                            .iter()
                            .cloned()
                            .collect::<std::collections::BTreeSet<_>>()
                            .into_iter()
                            .collect();
                        Saved::TrashResult {
                            version: 1,
                            title: request.title(),
                            report: TrashReport {
                                operation: None,
                                completed: Vec::new(),
                                failures: if before_execution {
                                    selected
                                        .iter()
                                        .map(|source| TrashFailure {
                                            source: source.clone(),
                                            message: error.clone(),
                                        })
                                        .collect()
                                } else {
                                    Vec::new()
                                },
                                remaining: if before_execution {
                                    Vec::new()
                                } else {
                                    selected
                                },
                                cancelled: false,
                                interrupted: (!before_execution).then_some(error),
                                unavailable: Vec::new(),
                            },
                        }
                    }
                    Saved::Plan {
                        request: Request::Transfer(request),
                        requirements,
                        source_requirements: Some(sources),
                        ..
                    } if !before_execution => Saved::TransferPending {
                        version: 1,
                        title: Request::Transfer(request.clone()).title(),
                        outcome: Box::new(transfer::Stopped::interrupted(
                            request,
                            io::Error::other(error),
                        )),
                        requirements: requirements.clone(),
                        source_requirements: sources.clone(),
                    },
                    Saved::Plan {
                        request: request @ Request::Archive(_),
                        ..
                    } => Saved::ArchiveResult {
                        version: 1,
                        title: request.title(),
                        report: if job.cancellation.is_cancelled() {
                            ArchiveReport::cancelled()
                        } else {
                            ArchiveReport::failed(error)
                        },
                    },
                    _ => {
                        let mut saved = job.saved.clone();
                        if let Saved::Plan {
                            request,
                            error: last,
                            rejected_before_execution,
                            ..
                        } = &mut saved
                        {
                            *last = Some(error);
                            *rejected_before_execution =
                                before_execution && matches!(request, Request::Transfer(_));
                        }
                        saved
                    }
                };
                (
                    saved.stopped_phase(),
                    saved.persisted_phase(),
                    saved.error(),
                    saved,
                )
            }
        };
        // Retain the actual stopped outcome if the checkpoint fails. In
        // particular, never leave a successfully completed executable plan.
        job.saved = saved;
        job.reconciliation = None;
        // Saved reports remain descriptive. Live executable ownership is kept
        // separately and is never reconstructed from those payloads.
        job.recovery_execution = None;
        // Only returned, confirmed continuation parts remain executable.
        // An uncertain result retains durable source dependencies and journals;
        // its original request cannot become a replay plan after reconciliation.
        job.transfer_execution = None;
        job.observed_transfer = None;
        job.observed_sources = None;
        if matches!(job.saved, Saved::Completed { .. }) {
            job.destination_observation = None;
        }
        if job.recovery_pending.is_none()
            && job.transfer_pending.is_none()
            && matches!(
                job.saved,
                Saved::Completed { .. }
                    | Saved::TrashResult { .. }
                    | Saved::RestoreResult { .. }
                    | Saved::DeleteResult { .. }
                    | Saved::ArchiveResult { .. }
                    | Saved::RecoveryResult { .. }
            )
        {
            job.placement = None;
            job.source_placement = None;
        }
        job.decision = None;
        let checkpoint = job.checkpoint_result(&self.engine, persisted_phase);
        if let Ok(record) = &checkpoint {
            job.record = record.clone();
            job.complete_clipboard();
        }
        self.refresh_undo();
        let (phase, error) = match checkpoint {
            Ok(_) => (phase, error),
            Err(error) => (
                Phase::SaveFailed,
                Some(fl!(
                    "files-jobs-result-not-saved",
                    error = error.to_string()
                )),
            ),
        };
        if !phase.needs_save() && self.jobs[&id].transfer_pending.is_none() {
            self.jobs.get_mut(&id).unwrap().work = None;
        }
        // Publish the stopped phase and its attention hold in the same lock.
        let replacement = self.jobs[&id].recovery_snapshot(id, phase).ok();
        self.update_job(id, phase, error, replacement);
        self.trim();
    }

    fn trim(&mut self) {
        let retained: std::collections::BTreeSet<_> =
            self.state().jobs.iter().map(|job| job.id).collect();
        self.jobs.retain(|id, _| retained.contains(id));
    }

    fn retry_save(&mut self, id: u64) {
        if !self
            .state()
            .jobs
            .iter()
            .any(|job| job.id == id && job.phase.needs_save())
        {
            return;
        }
        let Some(job) = self.jobs.get_mut(&id) else {
            return;
        };
        let persisted = job.saved.persisted_phase();
        let phase = job.saved.stopped_phase();
        let mut error = job.saved.error();
        match job.checkpoint_result(&self.engine, persisted) {
            Ok(record) => {
                job.record = record;
                if job.transfer_pending.is_none()
                    && matches!(phase, Phase::Stopped | Phase::Failed)
                    && let Saved::TransferPending { outcome, .. } = &job.saved
                {
                    match self.engine.restore_transfer_parts(&job.record, outcome) {
                        Ok(parts) => {
                            let pending = transfer_resume::Pending {
                                retry: parts.retry,
                                remaining: parts.remaining,
                                unavailable: parts.error.is_some(),
                            };
                            job.transfer_pending = (!pending.empty()).then_some(pending);
                            if let Some(reason) = parts.error {
                                error = Some(fl!(
                                    "files-jobs-some-work-cannot-continue",
                                    error = reason.to_string()
                                ));
                            }
                        }
                        Err(reason) => {
                            error = Some(fl!(
                                "files-jobs-transfer-cannot-continue",
                                error = reason.to_string()
                            ));
                        }
                    }
                }
                if phase != Phase::Queued {
                    job.complete_clipboard();
                    if job.transfer_pending.is_none() {
                        job.work = None;
                    }
                }
                self.refresh_undo();
                let replacement = self.jobs[&id].recovery_snapshot(id, phase).ok();
                self.update_job(id, phase, error, replacement);
                if phase == Phase::Queued {
                    self.queue.push_back(id);
                }
                self.trim();
            }
            Err(error) => self.set_job_phase(
                id,
                if phase == Phase::Queued {
                    Phase::StartSaveFailed
                } else {
                    Phase::SaveFailed
                },
                Some(fl!("files-jobs-save-failed", error = error.to_string())),
            ),
        }
    }
}

impl Request {
    fn valid(&self) -> bool {
        self.paths().iter().all(|path| path.is_absolute())
            && match self {
                Self::KeepRecovery { item } => item.entry.is_some(),
                Self::Recover { item, destination } => {
                    item.entry.is_some() && destination.file_name().is_some()
                }
                Self::Restore { item, destination } => destination
                    .as_ref()
                    .or(item.original.as_ref())
                    .is_some_and(|path| path.file_name().is_some()),
                Self::Trash { data_home, sources } => {
                    data_home.is_absolute()
                        && !sources.is_empty()
                        && sources.iter().all(|source| source.file_name().is_some())
                }
                Self::Delete { sources } => {
                    !sources.is_empty() && sources.iter().all(|source| source.file_name().is_some())
                }
                Self::DeleteTrash { items, .. } => {
                    !items.is_empty() && items.iter().all(|item| item.path.file_name().is_some())
                }
                Self::Transfer(request) => {
                    !request.sources.is_empty()
                        && request
                            .sources
                            .iter()
                            .all(|source| source.file_name().is_some())
                }
                Self::Undo { expected } => *expected != 0,
                Self::Archive(request) => request.valid(),
                Self::Mutation(OperationRequest::CreateFile { path, template }) => {
                    path.file_name().is_some()
                        && template.as_ref().is_none_or(|template| {
                            template.is_absolute() && template.file_name().is_some()
                        })
                }
                Self::Mutation(OperationRequest::RenameBatch { renames }) => {
                    crate::operations::rename_batch::order(renames).is_ok()
                }
                _ => true,
            }
    }

    fn title(&self) -> String {
        let name = |path: &std::path::Path| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        match self {
            Self::Reconcile => fl!("files-jobs-retry-interrupted"),
            Self::KeepRecovery { item } => fl!(
                "files-job-keep-in-recovery",
                name = item.name().to_string_lossy().into_owned()
            ),
            Self::Recover { item, .. } => fl!(
                "files-job-recover",
                name = item.name().to_string_lossy().into_owned()
            ),
            Self::Mutation(OperationRequest::CreateFolder { path }) => {
                fl!("files-job-create-folder", name = name(path))
            }
            Self::Mutation(OperationRequest::CreateFile { path, .. }) => {
                fl!("files-job-create-file", name = name(path))
            }
            Self::Mutation(OperationRequest::RenameBatch { renames }) => {
                fl!("files-job-rename-batch", count = renames.len())
            }
            Self::Mutation(OperationRequest::Rename {
                source,
                destination,
            }) => fl!(
                "files-job-rename",
                source = name(source),
                destination = name(destination)
            ),
            Self::Undo { .. } => fl!("files-command-undo"),
            Self::Restore { item, .. } => fl!(
                "files-job-restore",
                name = item.name().to_string_lossy().into_owned()
            ),
            Self::Trash { sources, .. } => fl!("files-job-trash", count = sources.len()),
            Self::Delete { sources } => fl!("files-job-delete", count = sources.len()),
            Self::DeleteTrash { items, empty: true } => {
                fl!("files-job-empty-trash", count = items.len())
            }
            Self::DeleteTrash {
                items,
                empty: false,
            } => fl!("files-job-delete-from-trash", count = items.len()),
            Self::Transfer(request) => {
                let (count, destination) = (request.sources.len(), name(&request.destination));
                if request.kind == TransferKind::Copy {
                    fl!("files-job-copy", count = count, destination = destination)
                } else {
                    fl!("files-job-move", count = count, destination = destination)
                }
            }
            Self::Archive(crate::archives::ArchiveRequest::Compress {
                sources,
                name,
                format,
                ..
            }) => {
                let mut name = name.as_os_str().to_os_string();
                name.push(format.suffix());
                fl!(
                    "files-job-compress",
                    count = sources.len(),
                    name = name.to_string_lossy().into_owned()
                )
            }
            Self::Archive(crate::archives::ArchiveRequest::Extract { archive, .. }) => {
                fl!("files-job-extract", name = name(archive))
            }
        }
    }
    fn paths(&self) -> Vec<PathBuf> {
        match self {
            Self::Reconcile => Vec::new(),
            Self::KeepRecovery { item } => {
                item.entry.iter().map(|entry| entry.path.clone()).collect()
            }
            Self::Recover { item, destination } => item
                .entry
                .iter()
                .map(|entry| entry.path.clone())
                .chain(std::iter::once(destination.clone()))
                .collect(),
            Self::Mutation(
                OperationRequest::CreateFolder { path } | OperationRequest::CreateFile { path, .. },
            ) => vec![path.clone()],
            Self::Mutation(OperationRequest::RenameBatch { renames }) => renames
                .iter()
                .flat_map(|step| [step.source.clone(), step.destination.clone()])
                .collect(),
            Self::Mutation(OperationRequest::Rename {
                source,
                destination,
            }) => vec![source.clone(), destination.clone()],
            Self::Undo { .. } => Vec::new(),
            Self::Restore { item, destination } => std::iter::once(item.path.clone())
                .chain(destination.as_ref().or(item.original.as_ref()).cloned())
                .collect(),
            Self::Trash { sources, .. } | Self::Delete { sources } => sources.clone(),
            Self::DeleteTrash { items, .. } => items.iter().map(|item| item.path.clone()).collect(),
            Self::Transfer(request) => request
                .sources
                .iter()
                .cloned()
                .chain(std::iter::once(request.destination.clone()))
                .collect(),
            Self::Archive(request) => request.paths(),
        }
    }
    fn accesses(&self, engine: &OperationEngine) -> io::Result<Vec<Access>> {
        if let Self::Recover { item, destination } = self {
            return Ok(vec![
                Access::entry(engine.recovery_copy_source_path(item)?),
                Access::entry(destination.clone()),
            ]);
        }

        if let Self::Transfer(request) = self {
            return Ok(vec![Access::folder(request.destination.clone())]);
        }
        if let Self::Archive(request) = self {
            let mut paths = request.paths();
            paths.pop();
            return Ok(paths
                .into_iter()
                .map(Access::entry)
                .chain(std::iter::once(Access::folder(
                    request.destination().to_owned(),
                )))
                .collect());
        }
        if let Self::Trash { sources, data_home } = self {
            return Ok(sources
                .iter()
                .cloned()
                .map(Access::entry)
                .chain(std::iter::once(Access::folder(data_home.clone())))
                .collect());
        }
        let paths = match self {
            Self::Undo { expected } => engine.undo_storage_paths(Some(*expected))?,
            _ => self.paths(),
        };
        Ok(paths.into_iter().map(Access::entry).collect())
    }
}

impl Saved {
    fn uncertain(&self) -> bool {
        match self {
            Self::TransferPending { outcome, .. } => outcome.interrupted.is_some(),
            Self::TrashResult { report, .. } => report.interrupted.is_some(),
            Self::RestoreResult { report, .. } => report.interrupted,
            Self::RecoveryResult { report, .. } => report.interrupted,
            _ => false,
        }
    }

    fn validate(&self) -> io::Result<()> {
        let valid = match self {
            Self::Reconciled {
                version: 1,
                previous,
            } => previous.uncertain() && previous.validate().is_ok(),
            Self::Plan {
                version: 1,
                request,
                transfer_checkpoint,
                source_requirements,
                error,
                rejected_before_execution,
                ..
            } => {
                request.valid()
                    && match (request, transfer_checkpoint) {
                        (Request::Transfer(request), Some(plan)) => plan.matches_request(request),
                        (_, None) => true,
                        (_, Some(_)) => false,
                    }
                    && if matches!(request, Request::Transfer(_) | Request::Recover { .. }) {
                        source_requirements.is_some()
                            && (!matches!(request, Request::Transfer(_))
                                || *rejected_before_execution == error.is_some())
                    } else {
                        source_requirements.is_none() && !rejected_before_execution
                    }
            }
            Self::Completed {
                version: 1,
                paths,
                transfer,
                ..
            } => {
                paths.iter().all(|path| path.is_absolute())
                    && transfer.as_ref().is_none_or(|report| {
                        report.valid()
                            && report.failures.is_empty()
                            && report.remaining.is_empty()
                            && report.destinations == *paths
                    })
            }
            Self::TransferPending {
                version: 1,
                outcome,
                ..
            } => outcome.valid(),
            Self::TrashResult {
                version: 1, report, ..
            } => report.valid(),
            Self::RestoreResult {
                version: 1, report, ..
            } => report.valid(),
            Self::DeleteResult {
                version: 1, report, ..
            } => report.valid(),
            Self::ArchiveResult {
                version: 1, report, ..
            } => report.valid(),
            Self::RecoveryResult {
                version: 1, report, ..
            } => report.valid(),
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid or incompatible saved operation result",
            ))
        }
    }

    fn requirements(&self) -> Option<&Requirements> {
        match self {
            Self::Reconciled { previous, .. } => previous.requirements(),
            Self::Plan { requirements, .. } | Self::TransferPending { requirements, .. } => {
                Some(requirements)
            }
            Self::Completed { .. }
            | Self::TrashResult { .. }
            | Self::RestoreResult { .. }
            | Self::DeleteResult { .. }
            | Self::ArchiveResult { .. }
            | Self::RecoveryResult { .. } => None,
        }
    }

    fn source_requirements(&self) -> Option<&Requirements> {
        match self {
            Self::Reconciled { previous, .. } => previous.source_requirements(),
            Self::Plan {
                source_requirements,
                ..
            } => source_requirements.as_ref(),
            Self::TransferPending {
                source_requirements,
                ..
            } => Some(source_requirements),
            Self::Completed { .. }
            | Self::TrashResult { .. }
            | Self::RestoreResult { .. }
            | Self::DeleteResult { .. }
            | Self::ArchiveResult { .. }
            | Self::RecoveryResult { .. } => None,
        }
    }

    fn persisted_phase(&self) -> RequestPhase {
        match self {
            Self::Reconciled { previous, .. } => previous.persisted_phase(),
            Self::Plan { error: None, .. } => RequestPhase::Queued,
            Self::Plan { error: Some(_), .. } => RequestPhase::Failed,
            Self::Completed { .. } => RequestPhase::Finished,
            Self::TrashResult { report, .. } => report.phase(),
            Self::RestoreResult { report, .. } => report.phase(),
            Self::DeleteResult { report, .. } => report.phase(),
            Self::ArchiveResult { report, .. } => report.phase(),
            Self::RecoveryResult { report, .. } => report.phase(),
            Self::TransferPending { outcome, .. } => outcome.phase(),
        }
    }

    fn stopped_phase(&self) -> Phase {
        match self {
            Self::Reconciled { .. } => Phase::Stopped,
            Self::Plan { error: None, .. } => Phase::Queued,
            Self::Plan { error: Some(_), .. } => Phase::Failed,
            Self::Completed { .. } => Phase::Finished,
            Self::RestoreResult { report, .. } => report.status(),
            Self::DeleteResult { report, .. } => report.status(),
            Self::ArchiveResult { report, .. } => report.status(),
            Self::RecoveryResult { report, .. } => report.status(),
            Self::TrashResult { report, .. } => {
                if report.interrupted.is_some() {
                    Phase::Interrupted
                } else if report.phase() == RequestPhase::Finished {
                    Phase::Finished
                } else {
                    Phase::Stopped
                }
            }
            Self::TransferPending { outcome, .. } if outcome.interrupted.is_some() => {
                Phase::Interrupted
            }
            Self::TransferPending { outcome, .. } if outcome.phase() == RequestPhase::Finished => {
                Phase::Finished
            }
            Self::TransferPending { outcome, .. } if outcome.rejected_before_execution => {
                Phase::Failed
            }
            Self::TransferPending { .. } => Phase::Stopped,
        }
    }

    fn error(&self) -> Option<String> {
        match self {
            Self::Reconciled { previous, .. } => previous.error(),
            Self::Plan { error, .. } => error.clone(),
            Self::TransferPending { outcome, .. } => {
                outcome.report.attention.clone().or_else(|| {
                    outcome
                        .report
                        .failures
                        .first()
                        .map(|failure| format!("{}: {}", failure.source.display(), failure.message))
                })
            }
            Self::Completed { .. } => None,
            Self::ArchiveResult { report, .. } => report.error.clone(),
            Self::RestoreResult { report, .. } => report.error.clone(),
            Self::RecoveryResult { report, .. } => report
                .error
                .clone()
                .or_else(|| report.transfer.attention.clone())
                .or_else(|| {
                    report
                        .transfer
                        .failures
                        .first()
                        .map(|failure| failure.message.clone())
                }),
            Self::DeleteResult { report, .. } => report
                .failures
                .first()
                .map(|failure| format!("{}: {}", failure.source.display(), failure.message)),
            Self::TrashResult { report, .. } => report.interrupted.clone().or_else(|| {
                report
                    .failures
                    .first()
                    .map(|failure| format!("{}: {}", failure.source.display(), failure.message))
            }),
        }
    }

    fn snapshot(&self, id: u64, phase: Phase, accepted_at: Option<i64>) -> io::Result<JobSnapshot> {
        let mut snapshot = JobSnapshot {
            id,
            accepted_at,
            title: String::new(),
            phase,
            error: self.error(),
            paths: Arc::from([]),
            detail: None,
            decision: None,
            progress: None,
            transfer: None,
            trash: None,
            restore: None,
            delete: None,
            recovery: None,
            recovery_actions: RecoveryActions::default(),
            archive: None,
            transfer_actions: TransferActions::default(),
            dismissible: false,
        };
        match self {
            Self::Reconciled {
                version: 1,
                previous,
            } => {
                snapshot = previous.snapshot(id, phase, accepted_at)?;
                let detail = snapshot.detail.get_or_insert_with(String::new);
                if !detail.is_empty() {
                    detail.push_str(" · ");
                }
                detail.push_str(&fl!("files-jobs-reconciliation-retained"));
            }
            Self::Plan {
                version: 1,
                request,
                error,
                ..
            } => {
                snapshot.title = request.title();
                snapshot.paths = request.paths().into();
                snapshot.error = error.clone();
                snapshot.dismissible = matches!(
                    request,
                    Request::Mutation(_)
                        | Request::KeepRecovery { .. }
                        | Request::Delete { .. }
                        | Request::DeleteTrash { .. }
                        | Request::Archive(_)
                ) && matches!(
                    phase,
                    Phase::Failed | Phase::AwaitingReview | Phase::Interrupted
                );
            }
            Self::Completed {
                version: 1,
                title,
                paths,
                detail,
                transfer,
            } => {
                snapshot.title = title.clone();
                snapshot.paths = paths.clone().into();
                snapshot.detail = detail.clone();
                snapshot.transfer = transfer.clone().map(Arc::new);
            }
            Self::TransferPending {
                version: 1,
                title,
                outcome,
                ..
            } => {
                snapshot.title = title.clone();
                snapshot.paths = outcome.report.destinations.clone().into();

                snapshot.detail = Some(outcome.detail());
                snapshot.transfer = Some(Arc::new(outcome.report.clone()));
            }
            Self::TrashResult {
                version: 1,
                title,
                report,
            } => {
                snapshot.title = title.clone();
                snapshot.paths = report.completed.clone().into();
                snapshot.detail = Some(report.detail());
                snapshot.trash = Some(Arc::new(report.clone()));
            }
            Self::RecoveryResult {
                version: 1,
                title,
                report,
            } => {
                snapshot.title = title.clone();
                snapshot.paths = report.transfer.destinations.clone().into();
                snapshot.detail = Some(report.detail());
                snapshot.transfer = Some(Arc::new(report.transfer.clone()));
                snapshot.recovery = Some(Arc::new(report.clone()));
            }
            Self::RestoreResult {
                version: 1,
                title,
                report,
            } => {
                snapshot.title = title.clone();
                snapshot.paths = report.restored.iter().cloned().collect::<Vec<_>>().into();
                snapshot.detail = Some(report.detail());
                snapshot.restore = Some(Arc::new(report.clone()));
            }
            Self::DeleteResult {
                version: 1,
                title,
                report,
            } => {
                snapshot.title = title.clone();
                snapshot.paths = report.completed.clone().into();
                snapshot.detail = Some(report.detail());
                snapshot.delete = Some(Arc::new(report.clone()));
            }
            Self::ArchiveResult {
                version: 1,
                title,
                report,
            } => {
                snapshot.title = title.clone();
                snapshot.paths = report.output.iter().cloned().collect::<Vec<_>>().into();
                snapshot.detail = report.detail.clone();
                snapshot.archive = Some(Arc::new(report.clone()));
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Unsupported operation record version",
                ));
            }
        }
        Ok(snapshot)
    }
}
