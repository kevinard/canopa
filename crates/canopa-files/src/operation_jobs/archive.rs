//! Archive jobs. [`crate::archives`] does the work; a job reports its result and
//! is never undone or resumed. An interrupted run is cleaned up when history
//! reopens: only its own hidden partial result is removed.
use super::{mutation_completion::Restored, *};
use crate::archives::ArchiveRequest;

/// The saved result of an archive job.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveReport {
    /// The published archive or extracted item.
    #[serde(default, with = "crate::path_serde::optional")]
    pub output: Option<PathBuf>,
    #[serde(default)]
    pub detail: Option<String>,
    pub cancelled: bool,
    pub interrupted: bool,
    pub error: Option<String>,
}

impl ArchiveReport {
    pub(super) fn finished(output: PathBuf, detail: Option<String>) -> Self {
        Self {
            output: Some(output),
            detail,
            cancelled: false,
            interrupted: false,
            error: None,
        }
    }

    pub(super) fn cancelled() -> Self {
        Self {
            output: None,
            detail: Some(fl!("files-archive-cancelled")),
            cancelled: true,
            interrupted: false,
            error: None,
        }
    }

    pub(super) fn failed(error: String) -> Self {
        Self {
            output: None,
            detail: None,
            cancelled: false,
            interrupted: false,
            error: Some(error),
        }
    }

    fn interrupted(error: String) -> Self {
        Self {
            interrupted: true,
            ..Self::failed(error)
        }
    }

    pub(super) fn valid(&self) -> bool {
        match (&self.output, self.cancelled, self.interrupted, &self.error) {
            (Some(output), false, false, None) => output.is_absolute(),
            (None, true, false, None) => true,
            (None, false, _, Some(_)) => true,
            _ => false,
        }
    }

    pub(super) fn phase(&self) -> RequestPhase {
        if self.output.is_some() {
            RequestPhase::Finished
        } else if self.cancelled {
            RequestPhase::Remaining
        } else {
            RequestPhase::Failed
        }
    }

    pub(super) fn status(&self) -> Phase {
        if self.interrupted {
            Phase::Interrupted
        } else if self.cancelled {
            Phase::Stopped
        } else if self.error.is_some() {
            Phase::Failed
        } else {
            Phase::Finished
        }
    }
}

impl Worker {
    /// Settle an archive run that history recorded as running: it stopped with
    /// the previous process. Its partial result is removed and the job reports
    /// the interruption; nothing is replayed.
    pub(super) fn restore_archive(
        &self,
        record: &RequestRecord,
        saved: &Saved,
    ) -> io::Result<Option<Restored>> {
        let Saved::Plan {
            request: request @ Request::Archive(archive),
            error: None,
            ..
        } = saved
        else {
            return Ok(None);
        };
        if record.phase() != RequestPhase::Running {
            return Ok(None);
        }
        let message = discard(archive);
        let result = Saved::ArchiveResult {
            version: 1,
            title: request.title(),
            report: ArchiveReport::interrupted(message.clone()),
        };
        Ok(Some(
            match self
                .engine
                .checkpoint_request(record, RequestPhase::Failed, &result)
            {
                Ok(record) => Restored {
                    record,
                    saved: result,
                    phase: Phase::Interrupted,
                    error: None,
                    transfer_confirmation: None,
                },
                // The plan stays recorded as running; the next start repeats
                // this idempotent cleanup.
                Err(error) => Restored {
                    record: record.clone(),
                    saved: saved.clone(),
                    phase: Phase::Interrupted,
                    error: Some(format!(
                        "{message}\n{}",
                        fl!("files-jobs-result-not-saved", error = error.to_string())
                    )),
                    transfer_confirmation: None,
                },
            },
        ))
    }
}

fn discard(request: &ArchiveRequest) -> String {
    match request.discard_partial() {
        Ok(_) => fl!("files-archive-interrupted"),
        Err(error) => fl!(
            "files-archive-interrupted-kept",
            name = request.partial_path().to_string_lossy().into_owned(),
            error = error.to_string()
        ),
    }
}
