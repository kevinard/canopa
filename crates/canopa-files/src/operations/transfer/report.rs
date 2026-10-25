//! Exact descriptive transfer results. Decoding a saved plan grants no execution ownership.
use super::{TransferContinuation, TransferKind, TransferOutcome, TransferRequest};
use crate::operations::requests::RequestPhase;
use serde::{Deserialize, Serialize};
use std::{io, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferPart {
    Failed,
    Remaining,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransferResolution {
    #[default]
    Available,
    HandedOff,
    Abandoned,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferReportFailure {
    #[serde(with = "crate::path_serde")]
    pub source: PathBuf,
    #[serde(with = "crate::path_serde")]
    pub destination: PathBuf,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferReport {
    pub kind: TransferKind,
    #[serde(with = "crate::path_serde")]
    pub destination: PathBuf,
    pub operation: Option<u64>,
    /// Confirmed completions. An interrupted result may have an additional
    /// uncertain filesystem step that needs journal reconciliation.
    pub completed: usize,
    pub bytes: u64,
    pub failures: Vec<TransferReportFailure>,
    #[serde(with = "crate::path_serde::sequence")]
    pub skipped: Vec<PathBuf>,
    #[serde(with = "crate::path_serde::sequence")]
    pub remaining: Vec<PathBuf>,
    /// Descriptive original-folder remainders; never a replacement Retry plan.
    #[serde(default, with = "crate::path_serde::sequence")]
    pub source_remainders: Vec<PathBuf>,
    #[serde(with = "crate::path_serde::sequence")]
    pub destinations: Vec<PathBuf>,
    pub cancelled: bool,
    pub needs_attention: bool,
    pub attention: Option<String>,
    #[serde(default)]
    pub failed_resolution: TransferResolution,
    #[serde(default)]
    pub remaining_resolution: TransferResolution,
}

impl TransferReport {
    pub(crate) fn from_outcome<C>(
        kind: TransferKind,
        destination: PathBuf,
        outcome: &TransferOutcome<C>,
    ) -> Self {
        Self {
            kind,
            destination,
            operation: outcome.id,
            completed: outcome.completed,
            bytes: outcome.bytes,
            failures: outcome
                .failures
                .iter()
                .map(|failure| TransferReportFailure {
                    source: failure.source.clone(),
                    destination: failure.destination.clone(),
                    message: failure.message.clone(),
                })
                .collect(),
            skipped: outcome.skipped.clone(),
            remaining: outcome.remaining.clone(),
            source_remainders: outcome.source_remainders.clone(),
            destinations: outcome.destinations.clone(),
            cancelled: outcome.cancelled,
            needs_attention: outcome.needs_attention,
            attention: outcome.attention.clone(),
            failed_resolution: TransferResolution::Available,
            remaining_resolution: TransferResolution::Available,
        }
    }

    pub(crate) fn valid(&self) -> bool {
        self.destination.is_absolute()
            && (self.kind == TransferKind::Move || self.source_remainders.is_empty())
            && self.operation != Some(0)
            && (self.completed == 0 || self.operation.is_some())
            && self
                .failures
                .iter()
                .all(|failure| failure.source.is_absolute() && failure.destination.is_absolute())
            && self
                .skipped
                .iter()
                .chain(&self.remaining)
                .chain(&self.source_remainders)
                .chain(&self.destinations)
                .all(|path| path.is_absolute())
    }

    pub(crate) fn detail(&self) -> String {
        let mut detail = format!(
            "{} completed · {} bytes · {} failed · {} skipped · {} remaining{}{}",
            self.completed,
            self.bytes,
            self.failures.len(),
            self.skipped.len(),
            self.remaining.len(),
            if self.cancelled { " · cancelled" } else { "" },
            if self.needs_attention {
                " · needs attention"
            } else {
                ""
            }
        );
        if !self.source_remainders.is_empty() {
            detail.push_str(&format!(
                " · {} source {} to review",
                self.source_remainders.len(),
                if self.source_remainders.len() == 1 {
                    "folder"
                } else {
                    "folders"
                }
            ));
        }
        for (name, resolution) in [
            ("failed work", self.failed_resolution),
            ("remaining work", self.remaining_resolution),
        ] {
            match resolution {
                TransferResolution::Available => {}
                TransferResolution::HandedOff => detail.push_str(&format!(" · {name} handed off")),
                TransferResolution::Abandoned => detail.push_str(&format!(" · {name} abandoned")),
            }
        }
        detail
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoppedTransfer {
    pub report: TransferReport,
    pub retry: Option<TransferContinuation>,
    pub remaining: Option<TransferContinuation>,
    #[serde(default)]
    pub interrupted: Option<String>,
    #[serde(default)]
    pub rejected_before_execution: bool,
}

impl StoppedTransfer {
    pub(crate) fn detail(&self) -> String {
        let detail = self.report.detail();
        if self.interrupted.is_some() {
            format!("{detail} · confirmed portion; unfinished work needs reconciliation")
        } else {
            detail
        }
    }

    pub(crate) fn valid(&self) -> bool {
        if !self.report.valid() {
            return false;
        }
        if self.rejected_before_execution
            && (self.report.completed != 0
                || self.report.operation.is_some()
                || !self.report.needs_attention
                || self.interrupted.is_some())
        {
            return false;
        }
        if let Some(reason) = &self.interrupted {
            if reason.is_empty()
                || !self.report.needs_attention
                || self.report.attention.as_ref() != Some(reason)
            {
                return false;
            }
        } else if (self.retry.is_some()
            || self.report.failed_resolution != TransferResolution::Available)
            == self.report.failures.is_empty()
            || (self.remaining.is_some()
                || self.report.remaining_resolution != TransferResolution::Available)
                == self.report.remaining.is_empty()
            || (self.retry.is_some()
                && self.report.failed_resolution != TransferResolution::Available)
            || (self.remaining.is_some()
                && self.report.remaining_resolution != TransferResolution::Available)
            || (self.phase() == RequestPhase::Finished
                && self.report.failed_resolution == TransferResolution::Available
                && self.report.remaining_resolution == TransferResolution::Available)
        {
            return false;
        }
        self.remaining.as_ref().is_none_or(|plan| {
            plan.matches_report(
                self.report.kind,
                &self.report.destination,
                &self.report.remaining,
                false,
            )
        }) && self.retry.as_ref().is_none_or(|plan| {
            plan.matches_report(
                self.report.kind,
                &self.report.destination,
                &self
                    .report
                    .failures
                    .iter()
                    .map(|failure| failure.source.clone())
                    .collect::<Vec<_>>(),
                true,
            )
        })
    }

    pub fn phase(&self) -> RequestPhase {
        if self.interrupted.is_some() {
            return RequestPhase::Running;
        }
        match (self.retry.is_some(), self.remaining.is_some()) {
            (true, true) => RequestPhase::FailedWithRemaining,
            (true, false) => RequestPhase::Failed,
            (false, true) => RequestPhase::Remaining,
            (false, false) => RequestPhase::Finished,
        }
    }

    pub(crate) fn has(&self, part: TransferPart) -> bool {
        match part {
            TransferPart::Failed => self.retry.is_some(),
            TransferPart::Remaining => self.remaining.is_some(),
        }
    }

    pub(crate) fn release(
        &mut self,
        part: TransferPart,
        resolution: TransferResolution,
    ) -> io::Result<()> {
        let (plan, status) = match part {
            TransferPart::Failed => (&mut self.retry, &mut self.report.failed_resolution),
            TransferPart::Remaining => (&mut self.remaining, &mut self.report.remaining_resolution),
        };
        if plan.take().is_none() {
            return Err(io::Error::other(
                "This transfer part has already been released.",
            ));
        }
        *status = resolution;
        Ok(())
    }
}

impl StoppedTransfer {
    pub(crate) fn new(request: &TransferRequest, outcome: TransferOutcome) -> Self {
        let outcome = outcome.map_continuations(TransferContinuation::into_saved_plan);
        Self {
            report: TransferReport::from_outcome(
                request.kind,
                request.destination.clone(),
                &outcome,
            ),
            retry: outcome.retry_continuation,
            remaining: outcome.continuation,
            interrupted: None,
            rejected_before_execution: false,
        }
    }

    pub(crate) fn interrupted(request: &TransferRequest, error: io::Error) -> Self {
        let message = format!("Transfer result needs reconciliation: {error}");
        let outcome = error
            .into_inner()
            .and_then(|error| {
                error
                    .downcast::<crate::transfers::InterruptedTransfer>()
                    .ok()
            })
            .map(|error| error.outcome)
            .unwrap_or_default();
        let mut stopped = Self::new(request, outcome);
        stopped.report.needs_attention = true;
        stopped.report.attention = Some(message.clone());
        stopped.interrupted = Some(message);
        stopped
    }
}
