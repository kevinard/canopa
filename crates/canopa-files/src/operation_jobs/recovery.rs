//! Copy-out uses the existing Recovery lane; persisted reports grant no replay.
use super::*;
use crate::{
    operations::RecoveryItem,
    transfers::{TransferAccess, TransferEvent},
};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryReport {
    pub transfer: TransferReport,
    pub error: Option<String>,
    pub interrupted: bool,
    #[serde(default)]
    pub handed_off: bool,
}
impl RecoveryReport {
    fn from_outcome(destination: &Path, outcome: &crate::operations::RecoveryCopyOutcome) -> Self {
        Self {
            transfer: TransferReport::from_outcome(
                TransferKind::Copy,
                destination.to_owned(),
                outcome,
            ),
            error: None,
            interrupted: false,
            handed_off: false,
        }
    }
    pub(super) fn failed(destination: PathBuf, error: String, interrupted: bool) -> Self {
        Self {
            transfer: TransferReport::from_outcome(
                TransferKind::Copy,
                destination,
                &crate::operations::RecoveryCopyOutcome::default(),
            ),
            error: Some(error),
            interrupted,
            handed_off: false,
        }
    }
    pub(super) fn valid(&self) -> bool {
        self.transfer.kind == TransferKind::Copy
            && self.transfer.valid()
            && (!self.interrupted || self.error.is_some())
    }
    pub(super) fn phase(&self) -> RequestPhase {
        if self.handed_off {
            return RequestPhase::Finished;
        }
        match (
            self.interrupted || self.error.is_some() || !self.transfer.failures.is_empty(),
            !self.transfer.remaining.is_empty() || self.transfer.cancelled,
        ) {
            (true, true) => RequestPhase::FailedWithRemaining,
            (true, false) => RequestPhase::Failed,
            (false, true) => RequestPhase::Remaining,
            (false, false) => RequestPhase::Finished,
        }
    }
    pub(super) fn status(&self) -> Phase {
        if self.handed_off {
            return Phase::Finished;
        }
        if self.interrupted {
            Phase::Interrupted
        } else if self.error.is_some() || !self.transfer.failures.is_empty() {
            Phase::Failed
        } else if self.phase() == RequestPhase::Finished {
            Phase::Finished
        } else {
            Phase::Stopped
        }
    }
    pub(super) fn detail(&self) -> String {
        let detail = self.transfer.detail();
        if self.handed_off {
            format!("{detail} · remaining work handed to another request")
        } else {
            detail
        }
    }
}

pub(super) struct Placement<'a> {
    pub source: &'a storage::Snapshot,
    pub destination: &'a storage::Snapshot,
}

pub(super) fn run(
    engine: &OperationEngine,
    item: &RecoveryItem,
    destination: &Path,
    cancellation: &Cancellation,
    placement: Placement<'_>,
    publication: transfer::Publication,
    continuation: Option<&crate::operations::RecoveryCopyContinuation>,
) -> Finished {
    let mut interaction = publication.interaction(true);
    let check = |path: &Path, access| match access {
        TransferAccess::SourceEntry => placement.source.check(&[Access::entry(path)]),
        TransferAccess::DestinationFolder => placement.destination.check(&[Access::folder(path)]),
    };
    let interact = |event: TransferEvent| interaction.event(event, cancellation);
    let result = match continuation {
        Some(plan) => engine.resume_recovery_copy_with_check(plan, cancellation, check, interact),
        None => engine.recover_copy_with_check(item, destination, cancellation, check, interact),
    };
    match result {
        Ok(outcome) => Finished::from_outcome(destination, outcome),
        Err(error) => {
            let message = error.to_string();
            let cancelled =
                error.kind() == io::ErrorKind::Interrupted && cancellation.is_cancelled();
            let interrupted = error
                .get_ref()
                .is_some_and(|error| error.is::<crate::transfers::InterruptedTransfer>());
            if cancelled && !interrupted {
                return Finished::cancelled(item, destination, continuation);
            }
            if !interrupted && let Some(plan) = continuation {
                let mut outcome = plan.paused(message);
                outcome.cancelled = cancelled;
                return Finished::from_outcome(destination, outcome);
            }
            let mut report = RecoveryReport::failed(destination.to_owned(), message, interrupted);
            if let Some(error) = error.into_inner().and_then(|error| {
                error
                    .downcast::<crate::transfers::InterruptedTransfer>()
                    .ok()
            }) {
                report.transfer = TransferReport::from_outcome(
                    TransferKind::Copy,
                    destination.to_owned(),
                    &error.outcome,
                );
            }
            Finished {
                report,
                pending: None,
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RecoveryPart {
    Failed,
    Remaining,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct RecoveryActions {
    pub retry: bool,
    pub remaining: bool,
}

pub(super) struct Finished {
    pub report: RecoveryReport,
    pub pending: Option<Pending>,
}
#[derive(Clone)]
pub(super) struct Pending {
    retry: Option<crate::operations::RecoveryCopyContinuation>,
    remaining: Option<crate::operations::RecoveryCopyContinuation>,
}
impl Pending {
    fn empty(&self) -> bool {
        self.retry.is_none() && self.remaining.is_none()
    }
    fn actions(&self) -> RecoveryActions {
        RecoveryActions {
            retry: self.retry.is_some(),
            remaining: self.remaining.is_some(),
        }
    }
    fn take(&mut self, part: RecoveryPart) -> Option<crate::operations::RecoveryCopyContinuation> {
        match part {
            RecoveryPart::Failed => self.retry.take(),
            RecoveryPart::Remaining => self.remaining.take(),
        }
    }
}
impl OperationJobs {
    /// Continue only an exact plan retained by this running owner. Serialized
    /// results and old request IDs do not recreate execution authority.
    pub fn resume_recovery(
        &self,
        id: u64,
        part: RecoveryPart,
    ) -> io::Result<oneshot::Receiver<Result<u64, String>>> {
        let (reply, receiver) = oneshot::channel();
        self.send_work(Control::ResumeRecovery(id, part, reply))
            .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))?;
        Ok(receiver)
    }
}
impl Job {
    pub(super) fn recovery_snapshot(&self, id: u64, phase: Phase) -> io::Result<JobSnapshot> {
        let mut snapshot = self.saved.snapshot(id, phase, self.record.accepted_at())?;
        if matches!(phase, Phase::Stopped | Phase::Failed) {
            snapshot.recovery_actions = self
                .recovery_pending
                .as_ref()
                .map_or_else(RecoveryActions::default, Pending::actions);
            snapshot.transfer_actions = self.transfer_pending.as_ref().map_or_else(
                TransferActions::default,
                super::transfer_resume::Pending::actions,
            );
        }
        Ok(snapshot)
    }
}
impl Worker {
    pub(super) fn resume_recovery(&mut self, id: u64, part: RecoveryPart) -> io::Result<u64> {
        self.check_attention_capacity()?;
        if !self
            .state()
            .jobs
            .iter()
            .any(|job| job.id == id && matches!(job.phase, Phase::Stopped | Phase::Failed))
        {
            return Err(io::Error::other(fl!("files-recovery-not-ready")));
        }
        let previous = self
            .jobs
            .get(&id)
            .ok_or_else(|| io::Error::other(fl!("files-recovery-request-gone")))?;
        let mut pending = previous
            .recovery_pending
            .clone()
            .ok_or_else(|| io::Error::other(fl!("files-recovery-no-plan")))?;
        let plan = pending
            .take(part)
            .ok_or_else(|| io::Error::other(fl!("files-recovery-handed-off")))?;
        let placement = previous
            .placement
            .clone()
            .ok_or_else(|| io::Error::other(fl!("files-original-destination-gone")))?;
        let source_placement = previous
            .source_placement
            .clone()
            .ok_or_else(|| io::Error::other(fl!("files-original-source-gone")))?;
        let request = Request::Recover {
            item: Box::new(plan.item().clone()),
            destination: plan.destination().to_owned(),
        };
        let saved = Saved::Plan {
            version: 1,
            request,
            transfer_checkpoint: None,
            requirements: placement.requirements(),
            source_requirements: Some(source_placement.requirements()),
            error: None,
            rejected_before_execution: false,
        };
        let mut old_saved = previous.saved.clone();
        let Saved::RecoveryResult { report, .. } = &mut old_saved else {
            return Err(io::Error::other(fl!("files-recovery-not-recovery")));
        };
        report.handed_off = pending.empty();
        let phase = old_saved.stopped_phase();
        let (old_record, record) = self.engine.handoff_request(
            &previous.record,
            old_saved.persisted_phase(),
            &old_saved,
            &saved,
        )?;
        // Do not consume the old plan until its source ownership and successor
        // have been committed atomically by the history module.
        let previous = self.jobs.get_mut(&id).unwrap();
        previous.record = old_record;
        previous.saved = old_saved;
        previous.recovery_pending = (!pending.empty()).then_some(pending);
        if previous.recovery_pending.is_none() {
            previous.placement = None;
            previous.source_placement = None;
        }
        let error = previous.saved.error();
        let old_snapshot = previous.recovery_snapshot(id, phase).ok();
        self.update_job(id, phase, error, old_snapshot);
        let next = record.id();
        let snapshot = saved.snapshot(next, Phase::Queued, record.accepted_at())?;
        self.jobs.insert(
            next,
            Job {
                work: None,
                record,
                saved,
                mutation_confirmation: None,
                transfer_confirmation: None,
                reconciliation: None,
                placement: Some(placement),
                source_placement: Some(source_placement),
                observed_transfer: None,
                destination_observation: None,
                observed_sources: None,
                clipboard: None,
                cancellation: Cancellation::default(),
                decision: None,
                generation: 0,
                recovery_pending: None,
                recovery_execution: Some(plan),
                transfer_pending: None,
                transfer_execution: None,
                cut_progress: None,
                cut_claim: None,
            },
        );
        self.state().jobs.push(snapshot);
        self.queue.push_back(next);
        Ok(next)
    }
}

impl Finished {
    pub(super) fn from_outcome(
        destination: &Path,
        outcome: crate::operations::RecoveryCopyOutcome,
    ) -> Self {
        let report = RecoveryReport::from_outcome(destination, &outcome);
        let pending = Pending {
            retry: outcome.retry_continuation,
            remaining: outcome.continuation,
        };
        Self {
            report,
            pending: (!pending.empty()).then_some(pending),
        }
    }
    pub(super) fn paused(
        plan: &crate::operations::RecoveryCopyContinuation,
        message: String,
    ) -> Self {
        Self::from_outcome(plan.destination(), plan.paused(message))
    }
}

impl Finished {
    pub(super) fn cancelled(
        item: &RecoveryItem,
        destination: &Path,
        continuation: Option<&crate::operations::RecoveryCopyContinuation>,
    ) -> Self {
        Self::from_outcome(destination, item.cancelled_copy(continuation))
    }
}
