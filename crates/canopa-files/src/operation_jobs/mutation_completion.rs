//! Restore proven results and retry their metadata without rerunning mutations.
use super::*;
use crate::operations::execution::{MutationConfirmation, RestoredTransfer, TransferEnd};

pub(super) struct Restored {
    pub record: RequestRecord,
    pub saved: Saved,
    pub phase: Phase,
    pub error: Option<String>,
    pub transfer_confirmation: Option<TransferConfirmation>,
}

impl Worker {
    pub(super) fn restore_mutation(
        &self,
        record: &RequestRecord,
        saved: &Saved,
    ) -> io::Result<Option<Restored>> {
        if !matches!(
            saved,
            Saved::Plan {
                request: Request::Mutation(_),
                error: None,
                ..
            }
        ) || record.phase() != RequestPhase::Running
        {
            return Ok(None);
        }
        let summary = match self.engine.completed_mutation(record) {
            Ok(summary) => summary,
            Err(error) => {
                return Ok(Some(Restored {
                    transfer_confirmation: None,
                    record: record.clone(),
                    saved: saved.clone(),
                    phase: Phase::Interrupted,
                    error: Some(format!(
                        "Recorded completion could not be verified; review the interrupted work: {error}"
                    )),
                }));
            }
        };
        let Some(summary) = summary else {
            return Ok(None);
        };
        let saved = Saved::Completed {
            version: 1,
            title: summary.label,
            paths: summary.paths,
            detail: Some(fl!(
                "files-jobs-completed-before-interruption",
                operation = summary.id.to_string()
            )),
            transfer: None,
        };
        let (record, phase, error) = match self.engine.finish_confirmed_mutation(record, &saved) {
            Ok(record) => (record, Phase::Finished, None),
            Err(error) => (
                record.clone(),
                Phase::SaveFailed,
                Some(format!("Recorded completion result not saved: {error}")),
            ),
        };
        Ok(Some(Restored {
            transfer_confirmation: None,
            record,
            saved,
            phase,
            error,
        }))
    }

    /// Review reconciles the captured journal first. Only the original running
    /// request and its independently verified cursor/end can regain an owner.
    pub(super) fn restore_reviewed_transfer(&mut self, id: u64, work: Option<Work>) -> bool {
        let Some(job) = self
            .jobs
            .get(&id)
            .filter(|job| job.saved.uncertain() || job.recorded_running_transfer())
        else {
            return false;
        };
        let restored = if job.saved.uncertain() {
            // A refused descriptive checkpoint leaves the accepted Plan intact.
            // Only that durable record plus the verified cursor can restore an
            // execution owner after reconciliation; the displayed error cannot.
            job.record
                .payload::<Saved>()
                .and_then(|accepted| self.restore_transfer(&job.record, &accepted, true))
        } else {
            self.restore_transfer(&job.record, &job.saved, true)
        };
        let restored = match restored {
            Ok(Some(restored)) => restored,
            Ok(None) => return false,
            Err(reason) => {
                self.set_job_phase(
                    id,
                    Phase::Interrupted,
                    Some(format!("Reviewed transfer could not be verified: {reason}")),
                );
                return true;
            }
        };
        let mut error = restored.error;
        let phase = restored.phase;
        let job = self.jobs.get_mut(&id).unwrap();
        job.record = restored.record;
        job.saved = restored.saved;
        if matches!(phase, Phase::Stopped | Phase::Failed)
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
                        error = Some(format!("Some saved work cannot yet continue: {reason}"));
                    }
                }
                Err(reason) => {
                    error = Some(format!("Saved transfer cannot yet continue: {reason}"));
                }
            }
        }
        job.transfer_confirmation = restored.transfer_confirmation;
        job.work = (phase.needs_save() || job.transfer_pending.is_some())
            .then_some(work)
            .flatten();
        let replacement = job.recovery_snapshot(id, phase).ok();
        self.update_job(id, phase, error, replacement);
        true
    }

    pub(super) fn restore_transfer(
        &self,
        record: &RequestRecord,
        saved: &Saved,
        reviewed: bool,
    ) -> io::Result<Option<Restored>> {
        let Saved::Plan {
            request: Request::Transfer(request),
            error: None,
            requirements,
            source_requirements,
            ..
        } = saved
        else {
            return Ok(None);
        };
        if record.phase() != RequestPhase::Running {
            return Ok(None);
        }
        let description = self.engine.interrupted_transfer(record)?;
        let interrupted = |reason: String| Restored {
            record: record.clone(),
            saved: match (&description, source_requirements) {
                (Some(outcome), Some(sources)) => Saved::TransferPending {
                    version: 1,
                    title: Request::Transfer(request.clone()).title(),
                    outcome: Box::new(outcome.clone()),
                    requirements: requirements.clone(),
                    source_requirements: sources.clone(),
                },
                _ => saved.clone(),
            },
            phase: Phase::Interrupted,
            error: Some(reason),
            transfer_confirmation: None,
        };
        if !reviewed && let Some(description) = &description {
            // A persisted uncertain result still needs explicit Recovery review.
            // Its accepted Plan is retained for verification, not auto-resumption.
            return Ok(Some(interrupted(description.interrupted.clone().unwrap())));
        }
        let restored = (|| {
            if let Some(stop) = self.engine.stopped_transfer(record)? {
                return Ok(Some(RestoredTransfer {
                    end: TransferEnd::Stopped(Box::new(stop)),
                    confirmation: TransferConfirmation::RecordedStop,
                    error: None,
                }));
            }
            if let Some(restored) = self.engine.recover_active_transfer(record)? {
                return Ok(Some(restored));
            }
            Ok::<_, io::Error>(self.engine.completed_transfer(record)?.map(|completion| {
                RestoredTransfer {
                    end: TransferEnd::Completed(completion),
                    confirmation: TransferConfirmation::Recorded,
                    error: None,
                }
            }))
        })();
        let restored = match restored {
            Ok(Some(restored)) => restored,
            Ok(None) => return Ok(None),
            Err(error) => {
                return Ok(Some(interrupted(format!(
                    "Recorded transfer could not be verified; review the interrupted work: {error}"
                ))));
            }
        };
        let confirmation = restored.confirmation;
        let (saved, stopped) = match restored.end {
            TransferEnd::Stopped(outcome) => {
                let Some(source_requirements) = source_requirements else {
                    return Ok(Some(interrupted(
                        "The recorded stopped transfer has no original source placement.".into(),
                    )));
                };
                if outcome.report.kind != request.kind
                    || outcome.report.destination != request.destination
                {
                    return Ok(Some(interrupted(
                        "Recorded stopped transfer disagrees with its original request.".into(),
                    )));
                }
                (
                    Saved::TransferPending {
                        version: 1,
                        title: Request::Transfer(request.clone()).title(),
                        outcome,
                        requirements: requirements.clone(),
                        source_requirements: source_requirements.clone(),
                    },
                    true,
                )
            }
            TransferEnd::Completed(completion) => {
                if completion.kind != request.kind || completion.destination != request.destination
                {
                    return Ok(Some(interrupted(
                        "Recorded transfer completion disagrees with its original request.".into(),
                    )));
                }
                let report = TransferReport::from_outcome(
                    completion.kind,
                    completion.destination.clone(),
                    &completion.outcome(),
                );
                (
                    Saved::Completed {
                        version: 1,
                        title: Request::Transfer(request.clone()).title(),
                        paths: report.destinations.clone(),
                        detail: Some(fl!("files-jobs-result-restored", detail = report.detail())),
                        transfer: Some(report),
                    },
                    false,
                )
            }
        };
        // A recovered end that failed to record must first use its exact
        // engine-scoped witness. Do not attempt an unguarded application save.
        let result = if let Some(error) = restored.error {
            Err(io::Error::other(format!("Transfer end not saved: {error}")))
        } else if stopped {
            let Saved::TransferPending { outcome, .. } = &saved else {
                unreachable!()
            };
            self.engine.save_stopped_transfer(record, outcome, &saved)
        } else {
            self.engine.finish_confirmed_transfer(record, &saved)
        };
        let (record, phase, error) = match result {
            Ok(record) => (record, saved.stopped_phase(), saved.error()),
            Err(error) => (
                record.clone(),
                Phase::SaveFailed,
                Some(format!("Recorded transfer result not saved: {error}")),
            ),
        };
        Ok(Some(Restored {
            record,
            saved,
            phase,
            error,
            transfer_confirmation: phase.needs_save().then_some(confirmation),
        }))
    }
}

impl Job {
    /// A pending witness can establish only this live execution's end. A
    /// recorded end then guards the independent result/source-release write.
    pub(super) fn checkpoint_result(
        &mut self,
        engine: &OperationEngine,
        phase: RequestPhase,
    ) -> io::Result<RequestRecord> {
        if self.transfer_confirmation.is_none()
            && let Saved::TransferPending { outcome, .. } = &self.saved
            && outcome.interrupted.is_some()
            && let Some(record) = engine.save_interrupted_transfer(&self.record, outcome)?
        {
            // This is a display checkpoint, not a stopped-end receipt or a
            // replacement of the accepted executable request.
            return Ok(record);
        }
        if let Some(TransferConfirmation::Pending(pending)) = &self.transfer_confirmation
            && matches!(pending.recorded(), TransferConfirmation::RecordedStop)
        {
            let Saved::TransferPending { outcome, .. } = &self.saved else {
                return Err(io::Error::other(
                    "The live transfer stop requires its actual result.",
                ));
            };
            if phase != outcome.phase() {
                return Err(io::Error::other(
                    "The live transfer stop has a different result phase.",
                ));
            }
            let record =
                engine.save_pending_transfer_stop(&self.record, pending, outcome, &self.saved)?;
            self.transfer_confirmation = None;
            return Ok(record);
        }
        if let Some(TransferConfirmation::Pending(pending)) = &self.transfer_confirmation {
            engine.retry_transfer_confirmation(pending).map_err(|error| io::Error::new(error.kind(),
                format!("File work completed, but its transfer completion receipt was not saved: {error}")))?;
            self.transfer_confirmation = Some(pending.recorded());
        }
        if let Some(MutationConfirmation::Pending(pending)) = &self.mutation_confirmation {
            engine
                .retry_mutation_confirmation(pending)
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "File work completed, but its completion receipt was not saved: {error}"
                        ),
                    )
                })?;
            self.mutation_confirmation = Some(MutationConfirmation::Recorded);
        }
        let record = if matches!(
            self.transfer_confirmation,
            Some(TransferConfirmation::RecordedStop)
        ) {
            let Saved::TransferPending { outcome, .. } = &self.saved else {
                return Err(io::Error::other(
                    "Recorded transfer stop requires its actual stopped result.",
                ));
            };
            if phase != outcome.phase() {
                return Err(io::Error::other(
                    "Recorded transfer stop has a different result phase.",
                ));
            }
            engine.save_stopped_transfer(&self.record, outcome, &self.saved)?
        } else if self.transfer_confirmation.is_some() {
            if phase != RequestPhase::Finished
                || !matches!(
                    self.saved,
                    Saved::Completed {
                        transfer: Some(_),
                        ..
                    }
                )
            {
                return Err(io::Error::other(
                    "Recorded transfer completion requires its actual finished result.",
                ));
            }
            engine.finish_confirmed_transfer(&self.record, &self.saved)?
        } else if self.mutation_confirmation.is_some() {
            if phase != RequestPhase::Finished || !matches!(self.saved, Saved::Completed { .. }) {
                return Err(io::Error::other(
                    "Recorded mutation completion requires its actual finished result.",
                ));
            }
            engine.finish_confirmed_mutation(&self.record, &self.saved)?
        } else {
            engine.checkpoint_request(&self.record, phase, &self.saved)?
        };
        self.transfer_confirmation = None;
        if phase == RequestPhase::Finished {
            self.mutation_confirmation = None;
        }
        Ok(record)
    }
}

impl Job {
    pub(super) fn recorded_running_transfer(&self) -> bool {
        self.record.phase() == RequestPhase::Running
            && matches!(
                self.saved,
                Saved::Plan {
                    request: Request::Transfer(_),
                    error: None,
                    ..
                }
            )
    }
}
