//! Completion and its metadata retry outlive UI windows, not the live engine.
use super::*;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct MutationOutcome {
    pub summary: OperationSummary,
    pub confirmation: Option<MutationConfirmation>,
}

#[derive(Clone, Debug)]
pub(crate) enum MutationConfirmation {
    Recorded,
    Pending(PendingMutation),
}

/// Created only after the live engine's mutation, cleanup and maintenance
/// return success. It cannot be serialized or rebuilt from a stored request.
#[derive(Clone)]
pub(crate) struct PendingMutation {
    scope: Arc<()>,
    request: RequestRecord,
    operation: u64,
    digest: [u8; 32],
}

impl std::fmt::Debug for PendingMutation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingMutation")
            .field("operation", &self.operation)
            .finish_non_exhaustive()
    }
}

impl OperationEngine {
    pub(in crate::operations) fn mutation_outcome(
        &self,
        state: &mut Store,
        request: Option<&RequestRecord>,
        summary: OperationSummary,
        change: &Change,
    ) -> io::Result<MutationOutcome> {
        let confirmation = if let Some(request) = request {
            let pending = PendingMutation {
                scope: self.completion_scope.clone(),
                request: request.clone(),
                operation: summary.id,
                digest: Sha256::digest(serde_json::to_vec(change).map_err(io::Error::other)?)
                    .into(),
            };
            Some(
                match state.record_mutation_end(request, summary.id, &pending.digest) {
                    Ok(()) => MutationConfirmation::Recorded,
                    // Keep the actual successful outcome and a live metadata-only
                    // retry witness. The scheduler must retain its unsaved hold.
                    Err(_) => MutationConfirmation::Pending(pending),
                },
            )
        } else {
            None
        };
        Ok(MutationOutcome {
            summary,
            confirmation,
        })
    }

    pub(crate) fn retry_mutation_confirmation(&self, pending: &PendingMutation) -> io::Result<()> {
        if !Arc::ptr_eq(&self.completion_scope, &pending.scope) {
            return Err(invalid(
                "The live mutation completion owner is unavailable.",
            ));
        }
        self.state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?
            .record_mutation_end(&pending.request, pending.operation, &pending.digest)
    }

    /// Historical result only; no current path is observed and nothing runs.
    pub(crate) fn completed_mutation(
        &self,
        request: &RequestRecord,
    ) -> io::Result<Option<OperationSummary>> {
        self.reads.snapshot(false)?.completed_mutation(request)
    }

    pub(crate) fn finish_confirmed_mutation<T: Serialize>(
        &self,
        request: &RequestRecord,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.reads.finish_confirmed_mutation(request, payload)
    }
}
