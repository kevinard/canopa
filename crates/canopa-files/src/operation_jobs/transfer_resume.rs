//! Exact live execution ownership, including independently verified stopped ends.
use super::*;
use crate::transfers::TransferContinuation;

pub use crate::operations::transfer::{TransferPart, TransferResolution};

#[derive(Clone, Copy, Debug, Default)]
pub struct TransferActions {
    pub retry: bool,
    pub remaining: bool,
    pub abandon: bool,
}

#[derive(Clone)]
pub(super) struct Pending {
    pub retry: Option<TransferContinuation>,
    pub remaining: Option<TransferContinuation>,
    // A sealed part with unavailable originals can still be abandoned safely.
    pub unavailable: bool,
}

impl Pending {
    pub fn empty(&self) -> bool {
        self.retry.is_none() && self.remaining.is_none() && !self.unavailable
    }

    pub fn actions(&self) -> TransferActions {
        TransferActions {
            retry: self.retry.is_some(),
            remaining: self.remaining.is_some(),
            abandon: !self.empty(),
        }
    }

    fn take(&mut self, part: TransferPart) -> Option<TransferContinuation> {
        match part {
            TransferPart::Failed => self.retry.take(),
            TransferPart::Remaining => self.remaining.take(),
        }
    }
}

impl OperationJobs {
    /// Select an exact retained part. A pathname report grants no replay rights.
    pub fn resume_transfer(
        &self,
        id: u64,
        part: TransferPart,
    ) -> io::Result<oneshot::Receiver<Result<u64, String>>> {
        let (reply, receiver) = oneshot::channel();
        self.send_work(Control::ResumeTransfer(id, part, reply))
            .map_err(|_| {
                io::Error::other("The operation queue is unavailable or full. Try again.")
            })?;
        Ok(receiver)
    }

    /// Persist abandonment before releasing execution ownership. Completed files
    /// and Undo history are retained; this never deletes or reverses file work.
    pub fn abandon_transfer(&self, id: u64) -> io::Result<oneshot::Receiver<Result<(), String>>> {
        let (reply, receiver) = oneshot::channel();
        self.send_work(Control::AbandonTransfer(id, reply))
            .map_err(|_| {
                io::Error::other("The operation queue is unavailable or full. Try again.")
            })?;
        Ok(receiver)
    }
}

impl Worker {
    fn stopped_transfer(&self, id: u64) -> io::Result<&Job> {
        if !self
            .state()
            .jobs
            .iter()
            .any(|job| job.id == id && matches!(job.phase, Phase::Stopped | Phase::Failed))
        {
            return Err(io::Error::other("This transfer is not ready to continue."));
        }
        self.jobs
            .get(&id)
            .filter(|job| job.transfer_pending.is_some())
            .ok_or_else(|| io::Error::other("This owner has no exact live transfer plan. Saved results require review before new work."))
    }

    pub(super) fn resume_transfer(&mut self, id: u64, part: TransferPart) -> io::Result<u64> {
        self.check_attention_capacity()?;
        let previous = self.stopped_transfer(id)?;
        let mut pending = previous.transfer_pending.clone().unwrap();
        let plan = pending.take(part).ok_or_else(|| {
            io::Error::other("This transfer part was already handed off or is unavailable.")
        })?;
        let placement = previous.placement.clone().ok_or_else(|| {
            io::Error::other("The original destination placement is unavailable.")
        })?;
        let source_placement = previous
            .source_placement
            .clone()
            .ok_or_else(|| io::Error::other("The original source placement is unavailable."))?;
        let saved = Saved::Plan {
            version: 1,
            transfer_checkpoint: Some(Box::new(plan.clone().into_saved_plan())),
            request: Request::Transfer(TransferRequest {
                kind: plan.kind,
                destination: plan.destination.clone(),
                sources: plan.source_paths().cloned().collect(),
            }),
            requirements: placement.requirements(),
            source_requirements: Some(source_placement.requirements()),
            error: None,
            rejected_before_execution: false,
        };
        let mut snapshot = saved.snapshot(0, Phase::Queued, None)?;
        let mut old_saved = previous.saved.clone();
        let Saved::TransferPending { outcome, .. } = &mut old_saved else {
            return Err(io::Error::other("The stopped result is not a transfer."));
        };
        outcome.release(part, TransferResolution::HandedOff)?;
        let phase = old_saved.stopped_phase();
        let destination_observation = previous.destination_observation.clone();
        let cut_progress = previous.cut_progress.clone();
        let cut_claim = cut_progress
            .as_ref()
            .map(clipboard::CutProgress::claim)
            .transpose()?
            .flatten();
        let Saved::TransferPending {
            outcome: before, ..
        } = &previous.saved
        else {
            unreachable!()
        };
        let Saved::TransferPending { outcome: after, .. } = &old_saved else {
            unreachable!()
        };
        let (old_record, record) =
            self.engine
                .handoff_transfer(&previous.record, before, after, &old_saved, &saved)?;
        let next = record.id();
        snapshot.id = next;
        snapshot.accepted_at = record.accepted_at();
        if let Some(cut) = &cut_progress {
            cut.handoff(id, next, part, &plan);
        }
        // All fallible persistence is finished. Move only this part's private
        // execution owner; keep the unselected part and historical report facts.
        let previous = self.jobs.get_mut(&id).unwrap();
        previous.record = old_record;
        previous.saved = old_saved;
        previous.transfer_pending = (!pending.empty()).then_some(pending);
        previous.release_transfer_holds();
        let old_snapshot = previous.recovery_snapshot(id, phase).ok();
        let error = previous.saved.error();
        self.update_job(id, phase, error, old_snapshot);
        self.jobs.insert(
            next,
            Job {
                work: None,
                record,
                saved,
                mutation_confirmation: None,
                transfer_confirmation: None,
                reconciliation: None,
                recovery_pending: None,
                recovery_execution: None,
                transfer_pending: None,
                transfer_execution: Some(plan),
                cut_progress,
                cut_claim,
                placement: Some(placement),
                source_placement: Some(source_placement),
                observed_transfer: None,
                observed_sources: None,
                destination_observation,
                clipboard: None,
                cancellation: Cancellation::default(),
                decision: None,
                generation: 0,
            },
        );
        self.state().jobs.push(snapshot);
        self.queue.push_back(next);
        self.trim();
        Ok(next)
    }

    pub(super) fn abandon_transfer(&mut self, id: u64) -> io::Result<()> {
        let previous = self.stopped_transfer(id)?;
        let mut saved = previous.saved.clone();
        let Saved::TransferPending { outcome, .. } = &mut saved else {
            return Err(io::Error::other("The stopped result is not a transfer."));
        };
        for part in [TransferPart::Failed, TransferPart::Remaining] {
            if outcome.has(part) {
                outcome.release(part, TransferResolution::Abandoned)?;
            }
        }
        let Saved::TransferPending {
            outcome: before, ..
        } = &previous.saved
        else {
            unreachable!()
        };
        let Saved::TransferPending { outcome: after, .. } = &saved else {
            unreachable!()
        };
        let record = self
            .engine
            .release_transfer(&previous.record, before, after, &saved)?;
        let job = self.jobs.get_mut(&id).unwrap();
        job.record = record;
        job.saved = saved;
        job.transfer_pending = None;
        // Abandoned roots remain in Cut; no file completion was confirmed.
        job.release_transfer_holds();
        let phase = job.saved.stopped_phase();
        let snapshot = job.recovery_snapshot(id, phase).ok();
        let error = job.saved.error();
        self.update_job(id, phase, error, snapshot);
        self.trim();
        Ok(())
    }
}

impl Job {
    pub(super) fn unstarted_transfer(&self, message: Option<String>) -> Option<transfer::Finished> {
        let Saved::Plan {
            request: Request::Transfer(request),
            requirements,
            source_requirements: Some(sources),
            ..
        } = &self.saved
        else {
            return None;
        };
        let plan = self.transfer_execution.clone().or_else(|| {
            self.observed_transfer
                .as_ref()
                .map(crate::transfers::TransferContinuation::unstarted)
        })?;
        let rejected = message.is_some();
        let outcome = match message {
            Some(message) => plan.paused(message),
            None => plan.cancelled(),
        };
        let mut finished = transfer::Finished::from_outcome(
            request,
            outcome,
            requirements.clone(),
            sources.clone(),
        );
        finished.outcome.rejected_before_execution = rejected;
        Some(finished)
    }

    fn release_transfer_holds(&mut self) {
        if self.transfer_pending.is_none() && self.transfer_execution.is_none() {
            self.placement = None;
            self.source_placement = None;
            self.destination_observation = None;
            self.work = None;
            self.cut_progress = None;
        }
    }
}
