use super::*;

/// Actual Trash results. These paths are presentation facts, never a retry plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrashReport {
    pub operation: Option<u64>,
    #[serde(with = "crate::path_serde::sequence")]
    pub completed: Vec<PathBuf>,
    pub failures: Vec<TrashFailure>,
    #[serde(with = "crate::path_serde::sequence")]
    /// Unstarted items, or unconfirmed items requiring reconciliation after an
    /// interruption. These paths never authorize execution.
    pub remaining: Vec<PathBuf>,
    pub cancelled: bool,
    #[serde(default)]
    pub interrupted: Option<String>,
    /// Failed sources whose volume has no usable Trash. Presentation may offer
    /// a separately confirmed permanent deletion; this list authorizes nothing.
    #[serde(
        default,
        with = "crate::path_serde::sequence",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub unavailable: Vec<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrashFailure {
    #[serde(with = "crate::path_serde")]
    pub source: PathBuf,
    pub message: String,
}

/// A Restore result contains confirmed changes and review facts, not the
/// selected TrashItem or an executable original request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreReport {
    #[serde(with = "crate::path_serde")]
    pub source: PathBuf,
    pub operation: Option<u64>,
    #[serde(with = "crate::path_serde::optional")]
    pub restored: Option<PathBuf>,
    pub error: Option<String>,
    pub interrupted: bool,
    pub cancelled: bool,
}

impl RestoreReport {
    pub(super) fn cancelled(source: PathBuf) -> Self {
        Self {
            source,
            operation: None,
            restored: None,
            error: None,
            interrupted: false,
            cancelled: true,
        }
    }

    pub(super) fn completed(source: PathBuf, summary: OperationSummary) -> Self {
        Self {
            source,
            operation: Some(summary.id),
            restored: summary.paths.into_iter().next(),
            error: None,
            interrupted: false,
            cancelled: false,
        }
    }

    pub(super) fn failed(source: PathBuf, error: String, interrupted: bool) -> Self {
        Self {
            source,
            operation: None,
            restored: None,
            error: Some(error),
            interrupted,
            cancelled: false,
        }
    }

    pub(super) fn from_failure(
        source: PathBuf,
        failure: crate::trash::InterruptedRestore,
        cancellation: &Cancellation,
    ) -> Self {
        if failure.summary.is_none()
            && !failure.needs_review
            && cancellation.is_cancelled()
            && failure.error.kind() == io::ErrorKind::Interrupted
        {
            return Self::cancelled(source);
        }
        let mut report = match failure.summary {
            Some(summary) => Self::completed(source, summary),
            None => Self::failed(source, failure.error.to_string(), failure.needs_review),
        };
        report.error = Some(failure.error.to_string());
        report.interrupted = failure.needs_review;
        report
    }

    pub(super) fn valid(&self) -> bool {
        self.source.is_absolute()
            && self.source.file_name().is_some()
            && match (self.operation, &self.restored) {
                (Some(id), Some(path)) => {
                    id != 0 && path.is_absolute() && path.file_name().is_some()
                }
                (None, None) => true,
                _ => false,
            }
            && (!self.interrupted || self.error.is_some())
            && (!self.cancelled
                || (self.restored.is_none() && self.error.is_none() && !self.interrupted))
            && (self.restored.is_some() || self.cancelled || self.error.is_some())
            && (self.restored.is_none() || self.error.is_none() || self.interrupted)
    }

    pub(super) fn phase(&self) -> RequestPhase {
        if self.cancelled {
            RequestPhase::Remaining
        } else if self.interrupted && self.restored.is_none() {
            RequestPhase::FailedWithRemaining
        } else if self.error.is_some() {
            RequestPhase::Failed
        } else {
            RequestPhase::Finished
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

    pub(super) fn detail(&self) -> String {
        if self.interrupted && self.restored.is_some() {
            fl!("files-restore-needs-finalization")
        } else if self.interrupted {
            fl!("files-restore-needs-review")
        } else if self.cancelled {
            fl!("files-restore-cancelled")
        } else if self.error.is_some() {
            fl!("files-restore-not-restored")
        } else {
            fl!("files-restore-done")
        }
    }
}

impl TrashReport {
    pub(super) fn from_outcome(outcome: crate::trash::TrashOutcome) -> Self {
        Self {
            operation: outcome.id,
            completed: outcome.paths,
            failures: outcome
                .failures
                .into_iter()
                .map(|(source, message)| TrashFailure { source, message })
                .collect(),
            remaining: outcome.remaining,
            cancelled: outcome.cancelled,
            interrupted: None,
            unavailable: outcome.unavailable,
        }
    }

    pub(super) fn valid(&self) -> bool {
        let mut paths = std::collections::BTreeSet::new();
        let valid_operation = match self.operation {
            Some(id) => id != 0 && !self.completed.is_empty(),
            None => self.completed.is_empty(),
        };
        valid_operation
            && self
                .completed
                .iter()
                .chain(self.failures.iter().map(|failure| &failure.source))
                .chain(&self.remaining)
                .all(|path| path.is_absolute() && path.file_name().is_some() && paths.insert(path))
            && (self.cancelled || self.interrupted.is_some() || self.remaining.is_empty())
            && self
                .unavailable
                .iter()
                .all(|path| self.failures.iter().any(|failure| &failure.source == path))
    }

    pub(super) fn phase(&self) -> RequestPhase {
        if self.interrupted.is_some() {
            return if self.remaining.is_empty() {
                RequestPhase::Failed
            } else {
                RequestPhase::FailedWithRemaining
            };
        }
        match (self.failures.is_empty(), self.remaining.is_empty()) {
            (true, true) => RequestPhase::Finished,
            (false, true) => RequestPhase::Failed,
            (true, false) => RequestPhase::Remaining,
            (false, false) => RequestPhase::FailedWithRemaining,
        }
    }

    pub(super) fn detail(&self) -> String {
        let mut parts = vec![
            fl!("files-report-trashed", count = self.completed.len()),
            fl!("files-report-failed", count = self.failures.len()),
            if self.interrupted.is_some() {
                fl!("files-report-need-review", count = self.remaining.len())
            } else {
                fl!("files-report-not-started", count = self.remaining.len())
            },
        ];
        if self.cancelled {
            parts.push(fl!("files-report-cancelled"));
        }
        parts.join(" · ")
    }
}

/// Actual permanent-deletion results. Nothing here is undoable or replayable.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteReport {
    /// Deleted from Trash storage rather than from ordinary folders.
    pub from_trash: bool,
    #[serde(with = "crate::path_serde::sequence")]
    pub completed: Vec<PathBuf>,
    pub failures: Vec<TrashFailure>,
    #[serde(with = "crate::path_serde::sequence")]
    pub remaining: Vec<PathBuf>,
    pub cancelled: bool,
    pub entries: u64,
    pub bytes: u64,
}

impl DeleteReport {
    pub(super) fn from_outcome(
        from_trash: bool,
        outcome: crate::operations::deletion::DeleteOutcome,
    ) -> Self {
        Self {
            from_trash,
            completed: outcome.completed,
            failures: outcome
                .failures
                .into_iter()
                .map(|(source, message)| TrashFailure { source, message })
                .collect(),
            remaining: outcome.remaining,
            cancelled: outcome.cancelled,
            entries: outcome.entries,
            bytes: outcome.bytes,
        }
    }

    pub(super) fn unstarted(
        from_trash: bool,
        selected: Vec<PathBuf>,
        error: Option<String>,
    ) -> Self {
        let selected: Vec<_> = selected
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let (failures, remaining) = match error {
            Some(message) => (
                selected
                    .into_iter()
                    .map(|source| TrashFailure {
                        source,
                        message: message.clone(),
                    })
                    .collect(),
                Vec::new(),
            ),
            None => (Vec::new(), selected),
        };
        Self {
            from_trash,
            completed: Vec::new(),
            failures,
            cancelled: !remaining.is_empty(),
            remaining,
            entries: 0,
            bytes: 0,
        }
    }

    pub(super) fn valid(&self) -> bool {
        let mut paths = std::collections::BTreeSet::new();
        self.completed
            .iter()
            .chain(self.failures.iter().map(|failure| &failure.source))
            .chain(&self.remaining)
            .all(|path| path.is_absolute() && path.file_name().is_some() && paths.insert(path))
            && (self.cancelled || self.remaining.is_empty())
    }

    pub(super) fn phase(&self) -> RequestPhase {
        match (self.failures.is_empty(), self.remaining.is_empty()) {
            (true, true) => RequestPhase::Finished,
            (false, true) => RequestPhase::Failed,
            (true, false) => RequestPhase::Remaining,
            (false, false) => RequestPhase::FailedWithRemaining,
        }
    }

    pub(super) fn status(&self) -> Phase {
        match self.phase() {
            RequestPhase::Finished => Phase::Finished,
            _ if self.completed.is_empty() && !self.cancelled => Phase::Failed,
            _ => Phase::Stopped,
        }
    }

    pub(super) fn detail(&self) -> String {
        let mut parts = vec![
            fl!("files-report-deleted", count = self.completed.len()),
            fl!("files-report-failed", count = self.failures.len()),
            fl!("files-report-not-started", count = self.remaining.len()),
        ];
        if self.cancelled {
            parts.push(fl!("files-report-cancelled"));
        }
        parts.join(" · ")
    }
}
