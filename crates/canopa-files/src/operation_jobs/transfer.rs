//! Live transfer interaction and durable, non-replayed stopped outcomes.
use super::*;
use crate::{
    browse::Entry,
    transfers::{
        TransferAccess, TransferContinuation, TransferEvent, TransferOutcome, TransferReply,
        TransferRequest,
    },
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub enum TransferQuestion {
    Conflict {
        source: Entry,
        destination: Entry,
        folders: bool,
    },
    Unavailable {
        destination: PathBuf,
        message: String,
    },
}

impl TransferQuestion {
    pub fn can_merge(&self) -> bool {
        matches!(self, Self::Conflict { source, destination, folders: true }
            if source.kind == crate::browse::EntryKind::Directory && destination.kind == crate::browse::EntryKind::Directory)
    }

    pub fn can_replace(&self) -> bool {
        let file = |entry: &Entry| {
            matches!(
                entry.kind,
                crate::browse::EntryKind::File | crate::browse::EntryKind::Symlink
            )
        };
        matches!(self, Self::Conflict { source, destination, .. } if file(source) && file(destination))
    }

    pub(super) fn accepts(&self, reply: TransferReply) -> bool {
        use crate::transfers::ConflictChoice;
        match (self, reply) {
            (_, TransferReply::Cancel) => true,
            (Self::Unavailable { .. }, TransferReply::Retry) => true,
            (Self::Conflict { .. }, TransferReply::Resolve { choice, .. }) => match choice {
                ConflictChoice::KeepBoth | ConflictChoice::Skip => true,
                ConflictChoice::Merge => self.can_merge(),
                ConflictChoice::Replace => self.can_replace(),
            },
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TransferProgress {
    pub path: PathBuf,
    pub completed: usize,
    pub bytes: u64,
    pub verifying: bool,
}

pub(super) use crate::operations::transfer::StoppedTransfer as Stopped;
pub use crate::operations::transfer::{TransferReport, TransferReportFailure as TransferFailure};

pub(super) struct Finished {
    pub confirmation: Option<crate::operations::execution::TransferConfirmation>,
    pub outcome: Stopped,
    pub pending: Option<super::transfer_resume::Pending>,
    pub destination: Requirements,
    pub sources: Requirements,
}

impl Finished {
    pub(super) fn from_outcome(
        request: &TransferRequest,
        outcome: TransferOutcome,
        destination: Requirements,
        sources: Requirements,
    ) -> Self {
        let pending = super::transfer_resume::Pending {
            retry: outcome.retry_continuation.clone(),
            remaining: outcome.continuation.clone(),
            unavailable: false,
        };
        Self {
            confirmation: None,
            outcome: Stopped::new(request, outcome),
            pending: (!pending.empty()).then_some(pending),
            destination,
            sources,
        }
    }
}

pub(super) struct Publication {
    pub id: u64,
    pub snapshot: Arc<Mutex<Snapshot>>,
    pub notice: Arc<Mutex<asynchronous::Sender<()>>>,
    pub sender: Arc<mpsc::SyncSender<Control>>,
}

impl Publication {
    pub(super) fn progress(&self, progress: TransferProgress) {
        let mut snapshot = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = snapshot
            .jobs
            .iter_mut()
            .find(|job| job.id == self.id && job.phase == Phase::Running)
        {
            job.progress = Some(progress);
            drop(snapshot);
            let _ = self
                .notice
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .try_send(());
        }
    }

    fn ask(
        &self,
        question: TransferQuestion,
        recovery: bool,
        cancellation: &Cancellation,
    ) -> TransferReply {
        let (reply, receiver) = mpsc::sync_channel(1);
        if self
            .sender
            .send(Control::Question(
                self.id,
                if recovery {
                    Question::Recovery(question)
                } else {
                    Question::Transfer(question)
                },
                QuestionReply::Transfer(reply),
            ))
            .is_err()
        {
            return TransferReply::Cancel;
        }
        loop {
            if cancellation.is_cancelled() {
                return TransferReply::Cancel;
            }
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(reply) => return reply,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return TransferReply::Cancel,
            }
        }
    }
}

pub(super) struct Placement<'a> {
    pub source: &'a storage::Snapshot,
    pub destination: &'a storage::Snapshot,
}

pub(super) fn run(
    execution: &crate::operations::execution::Execution<'_>,
    observed: &crate::transfers::ObservedTransfer,
    cancellation: &Cancellation,
    placement: Placement<'_>,
    destination_observation: Option<&super::drag_drop::DestinationProof>,
    continuation: Option<&TransferContinuation>,
    publication: Publication,
) -> Finished {
    let Placement {
        source,
        destination,
    } = placement;
    let request = observed.request();
    let mut interaction = publication.interaction(false);
    let check = |path: &Path, access| match access {
        TransferAccess::SourceEntry => source.check(&[Access::entry(path)]),
        TransferAccess::DestinationFolder => {
            if let Some(observed) = destination_observation {
                observed.check()?;
            }
            destination.check(&[Access::folder(path)])
        }
    };
    let interact = |event| interaction.event(event, cancellation);
    let result = execution.transfer(observed, continuation, cancellation, check, interact);
    let (outcome, pending) = match result {
        Ok(executed) => {
            let mut finished = Finished::from_outcome(
                request,
                executed.outcome,
                destination.requirements(),
                source.requirements(),
            );
            finished.confirmation = executed.confirmation;
            return finished;
        }
        Err(error)
            if error
                .get_ref()
                .is_none_or(|error| !error.is::<crate::transfers::InterruptedTransfer>()) =>
        {
            // The engine wraps every error after entering recursive execution
            // in InterruptedTransfer. Earlier refusal has not executed this
            // request; retain its exact private plan, including chosen targets.
            let plan = continuation
                .cloned()
                .unwrap_or_else(|| TransferContinuation::unstarted(observed));
            let cancelled =
                cancellation.is_cancelled() && error.kind() == io::ErrorKind::Interrupted;
            let outcome = if cancelled {
                plan.cancelled()
            } else {
                plan.paused(error.to_string())
            };
            let mut finished = Finished::from_outcome(
                request,
                outcome,
                destination.requirements(),
                source.requirements(),
            );
            finished.outcome.rejected_before_execution = !cancelled;
            return finished;
        }
        Err(error) => (Stopped::interrupted(request, error), None),
    };
    Finished {
        confirmation: None,
        outcome,
        pending,
        destination: destination.requirements(),
        sources: source.requirements(),
    }
}

pub(super) struct Interaction {
    publication: Publication,
    recovery: bool,
    last: Option<Instant>,
    completed: usize,
    bytes: u64,
}
impl Publication {
    pub(super) fn interaction(self, recovery: bool) -> Interaction {
        Interaction {
            publication: self,
            recovery,
            last: None,
            completed: 0,
            bytes: 0,
        }
    }
}
impl Interaction {
    pub(super) fn event(
        &mut self,
        event: TransferEvent,
        cancellation: &Cancellation,
    ) -> TransferReply {
        if cancellation.is_cancelled() {
            return TransferReply::Cancel;
        }
        let progress = match event {
            TransferEvent::Conflict {
                source,
                destination,
                folders,
            } => {
                self.publication.progress(TransferProgress {
                    path: source.path.clone(),
                    completed: self.completed,
                    bytes: self.bytes,
                    verifying: false,
                });
                return self.publication.ask(
                    TransferQuestion::Conflict {
                        source,
                        destination,
                        folders,
                    },
                    self.recovery,
                    cancellation,
                );
            }
            TransferEvent::Unavailable {
                destination,
                message,
            } => {
                self.publication.progress(TransferProgress {
                    path: destination.clone(),
                    completed: self.completed,
                    bytes: self.bytes,
                    verifying: false,
                });
                return self.publication.ask(
                    TransferQuestion::Unavailable {
                        destination,
                        message,
                    },
                    self.recovery,
                    cancellation,
                );
            }
            TransferEvent::Verifying { source, .. } => TransferProgress {
                path: source,
                completed: self.completed,
                bytes: self.bytes,
                verifying: true,
            },
            TransferEvent::Progress {
                path,
                completed,
                bytes,
            } => {
                self.completed = completed;
                self.bytes = bytes;
                TransferProgress {
                    path,
                    completed,
                    bytes,
                    verifying: false,
                }
            }
        };
        if self
            .last
            .is_none_or(|last| last.elapsed() >= Duration::from_millis(80))
        {
            self.publication.progress(progress);
            self.last = Some(Instant::now());
        }
        TransferReply::Continue
    }
}
