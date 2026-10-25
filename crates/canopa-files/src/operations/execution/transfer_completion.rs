//! Typed completed/stopped ends preserve returned results without filesystem replay.
use super::*;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use transfer::{
    StoppedTransfer, TransferContinuation, TransferKind, TransferOutcome, TransferRequest,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompletedTransfer {
    pub kind: TransferKind,
    #[serde(with = "crate::path_serde")]
    pub destination: PathBuf,
    pub operation: Option<u64>,
    pub completed: usize,
    pub bytes: u64,
    #[serde(with = "crate::path_serde::sequence")]
    pub skipped: Vec<PathBuf>,
    #[serde(with = "crate::path_serde::sequence")]
    pub destinations: Vec<PathBuf>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        with = "crate::path_serde::sequence"
    )]
    pub source_remainders: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cancelled: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub needs_attention: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attention: Option<String>,
}

impl CompletedTransfer {
    pub(crate) fn outcome(&self) -> TransferOutcome {
        TransferOutcome {
            id: self.operation,
            completed: self.completed,
            bytes: self.bytes,
            skipped: self.skipped.clone(),
            destinations: self.destinations.clone(),
            source_remainders: self.source_remainders.clone(),
            cancelled: self.cancelled,
            needs_attention: self.needs_attention,
            attention: self.attention.clone(),
            ..Default::default()
        }
    }

    pub(in crate::operations) fn valid(&self) -> bool {
        self.destination.is_absolute()
            && (self.kind == TransferKind::Move || self.source_remainders.is_empty())
            && self.operation != Some(0)
            && (self.completed == 0 || self.operation.is_some())
            && self.skipped.iter().all(|path| path.is_absolute())
            && self.destinations.iter().all(|path| path.is_absolute())
            && self.source_remainders.iter().all(|path| path.is_absolute())
    }
}

pub(crate) struct TransferExecutionOutcome {
    pub outcome: TransferOutcome,
    pub confirmation: Option<TransferConfirmation>,
}

pub(crate) struct RestoredTransferParts {
    pub retry: Option<TransferContinuation>,
    pub remaining: Option<TransferContinuation>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub(crate) enum TransferConfirmation {
    Recorded,
    RecordedStop,
    Pending(PendingTransfer),
}

/// An actual engine return or fully proved restart cursor constructs this
/// engine-scoped witness. It retries metadata; it cannot run filesystem work or
/// serialize. Unverified recovery never creates one.
#[derive(Clone)]
pub(crate) struct PendingTransfer {
    scope: Arc<()>,
    request: RequestRecord,
    end: Arc<TransferEnd>,
    digest: [u8; 32],
}

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum TransferEnd {
    Completed(CompletedTransfer),
    Stopped(Box<StoppedTransfer>),
}

pub(crate) struct RestoredTransfer {
    pub end: TransferEnd,
    pub confirmation: TransferConfirmation,
    pub error: Option<String>,
}

impl PendingTransfer {
    pub(crate) fn recorded(&self) -> TransferConfirmation {
        match self.end.as_ref() {
            TransferEnd::Completed(_) => TransferConfirmation::Recorded,
            TransferEnd::Stopped(_) => TransferConfirmation::RecordedStop,
        }
    }
}

impl OperationEngine {
    pub(crate) fn recover_active_transfer(
        &self,
        request: &RequestRecord,
    ) -> io::Result<Option<RestoredTransfer>> {
        let recovered = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?
            .recover_active_transfer(request)?;
        let Some(recovered) = recovered else {
            return Ok(None);
        };
        let end = match recovered.end {
            transfer::cursor::Recovered::Completed(completion) => {
                TransferEnd::Completed(completion)
            }
            transfer::cursor::Recovered::Stopped(stop) => TransferEnd::Stopped(stop),
        };
        let pending = PendingTransfer {
            scope: self.completion_scope.clone(),
            request: request.clone(),
            digest: Sha256::digest(serde_json::to_vec(&end).map_err(io::Error::other)?).into(),
            end: Arc::new(end),
        };
        Ok(Some(RestoredTransfer {
            end: match pending.end.as_ref() {
                TransferEnd::Completed(completion) => TransferEnd::Completed(completion.clone()),
                TransferEnd::Stopped(stop) => TransferEnd::Stopped(stop.clone()),
            },
            error: recovered.error.as_ref().map(ToString::to_string),
            confirmation: if recovered.error.is_some() {
                TransferConfirmation::Pending(pending)
            } else {
                pending.recorded()
            },
        }))
    }

    pub(crate) fn restore_transfer_parts(
        &self,
        request: &RequestRecord,
        stop: &StoppedTransfer,
    ) -> io::Result<RestoredTransferParts> {
        self.reads
            .snapshot(false)?
            .require_saved_transfer(request, stop)?;
        let mut errors = Vec::new();
        let mut reopen = |plan: &Option<TransferContinuation>| {
            plan.as_ref().and_then(|plan| match plan.reopen_stopped() {
                Ok(plan) => Some(plan),
                Err(error) => {
                    errors.push(error.to_string());
                    None
                }
            })
        };
        let retry = reopen(&stop.retry);
        let remaining = reopen(&stop.remaining);
        Ok(RestoredTransferParts {
            retry,
            remaining,
            error: (!errors.is_empty()).then(|| errors.join("; ")),
        })
    }

    pub(crate) fn handoff_transfer<T: Serialize, U: Serialize>(
        &self,
        request: &RequestRecord,
        before: &StoppedTransfer,
        after: &StoppedTransfer,
        payload: &T,
        successor: &U,
    ) -> io::Result<(RequestRecord, RequestRecord)> {
        self.reads
            .handoff_transfer(request, before, after, payload, successor)
    }

    pub(crate) fn release_transfer<T: Serialize>(
        &self,
        request: &RequestRecord,
        before: &StoppedTransfer,
        after: &StoppedTransfer,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.reads.release_transfer(request, before, after, payload)
    }
    pub(in crate::operations) fn transfer_outcome(
        &self,
        request: &RequestRecord,
        plan: &TransferContinuation,
        outcome: TransferOutcome,
    ) -> io::Result<TransferExecutionOutcome> {
        let terminal = outcome.failures.is_empty()
            && outcome.remaining.is_empty()
            && outcome.continuation.is_none()
            && outcome.retry_continuation.is_none();
        let end = if terminal {
            Some(TransferEnd::Completed(CompletedTransfer {
                kind: plan.kind,
                destination: plan.destination.clone(),
                operation: outcome.id,
                completed: outcome.completed,
                bytes: outcome.bytes,
                skipped: outcome.skipped.clone(),
                destinations: outcome.destinations.clone(),
                source_remainders: outcome.source_remainders.clone(),
                cancelled: outcome.cancelled,
                needs_attention: outcome.needs_attention,
                attention: outcome.attention.clone(),
            }))
        } else {
            let stop = StoppedTransfer::new(
                &TransferRequest {
                    kind: plan.kind,
                    destination: plan.destination.clone(),
                    sources: Vec::new(),
                },
                outcome.clone(),
            );
            (stop.phase() != requests::RequestPhase::Finished)
                .then_some(TransferEnd::Stopped(Box::new(stop)))
        };
        // Cancellation or refusal before the engine begins has no execution
        // reservation to end. An unreadable reservation retains a live witness
        // so a real returned result is never discarded on a metadata failure.
        let has_reservation = self
            .reads
            .snapshot(false)
            .and_then(|store| store.has_transfer_reservation(request));
        let confirmation = if let Some(end) = end.filter(|_| !matches!(has_reservation, Ok(false)))
        {
            let pending = PendingTransfer {
                scope: self.completion_scope.clone(),
                request: request.clone(),
                digest: Sha256::digest(serde_json::to_vec(&end).map_err(io::Error::other)?).into(),
                end: Arc::new(end),
            };
            Some(match self.retry_transfer_confirmation(&pending) {
                Ok(()) => pending.recorded(),
                Err(_) => TransferConfirmation::Pending(pending),
            })
        } else {
            None
        };
        Ok(TransferExecutionOutcome {
            outcome,
            confirmation,
        })
    }

    pub(crate) fn retry_transfer_confirmation(&self, pending: &PendingTransfer) -> io::Result<()> {
        if !Arc::ptr_eq(&self.completion_scope, &pending.scope) {
            return Err(invalid(
                "The live transfer completion owner is unavailable.",
            ));
        }
        let mut store = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?;
        match pending.end.as_ref() {
            TransferEnd::Completed(completion) => {
                store.record_transfer_end(&pending.request, completion, &pending.digest)
            }
            TransferEnd::Stopped(stop) => {
                store.record_transfer_stop(&pending.request, stop, &pending.digest)
            }
        }
    }

    pub(crate) fn save_pending_transfer_stop<T: Serialize>(
        &self,
        request: &RequestRecord,
        pending: &PendingTransfer,
        stop: &StoppedTransfer,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        if !Arc::ptr_eq(&self.completion_scope, &pending.scope)
            || pending.request.history != request.history
            || pending.request.id != request.id
            || pending.request.revision != request.revision
            || pending.request.phase != request.phase
            || pending.request.body != request.body
        {
            return Err(invalid(
                "The exact live stopped transfer owner is unavailable.",
            ));
        }
        let TransferEnd::Stopped(original) = pending.end.as_ref() else {
            return Err(invalid(
                "The live transfer returned a different result kind.",
            ));
        };
        let body = serde_json::to_vec(stop).map_err(io::Error::other)?;
        if pending.digest != Sha256::digest(&body).as_slice()
            || body != serde_json::to_vec(original).map_err(io::Error::other)?
        {
            return Err(invalid("The live stopped transfer result changed."));
        }
        self.reads
            .save_witnessed_transfer_stop(request, stop, payload)
    }

    /// Display-only facts leave the original accepted Plan and cursor intact.
    pub(crate) fn save_interrupted_transfer(
        &self,
        request: &RequestRecord,
        stop: &StoppedTransfer,
    ) -> io::Result<Option<RequestRecord>> {
        self.reads.save_interrupted_transfer(request, stop)
    }

    pub(crate) fn interrupted_transfer(
        &self,
        request: &RequestRecord,
    ) -> io::Result<Option<StoppedTransfer>> {
        self.reads.snapshot(false)?.interrupted_transfer(request)
    }

    pub(crate) fn stopped_transfer(
        &self,
        request: &RequestRecord,
    ) -> io::Result<Option<StoppedTransfer>> {
        self.reads.snapshot(false)?.stopped_transfer(request)
    }

    pub(crate) fn save_stopped_transfer<T: Serialize>(
        &self,
        request: &RequestRecord,
        stop: &StoppedTransfer,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.reads.save_stopped_transfer(request, stop, payload)
    }

    pub(crate) fn completed_transfer(
        &self,
        request: &RequestRecord,
    ) -> io::Result<Option<CompletedTransfer>> {
        self.reads.snapshot(false)?.completed_transfer(request)
    }

    pub(crate) fn finish_confirmed_transfer<T: Serialize>(
        &self,
        request: &RequestRecord,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.reads.finish_confirmed_transfer(request, payload)
    }
}
