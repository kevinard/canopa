use super::*;
use crate::browse::{Entry, EntryKind};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::os::unix::fs::PermissionsExt;
mod checkpoint;
#[cfg(test)]
pub(super) mod copy_boundary;
pub(in crate::operations) mod cursor;
pub(super) mod directory;
#[cfg(test)]
pub(super) mod directory_boundary;
#[cfg(test)]
pub(super) mod move_boundary;
mod observation;
#[cfg(test)]
pub(super) mod replacement_boundary;
mod report;
mod restart;
#[cfg(test)]
pub(super) mod saved_boundary;
#[cfg(test)]
pub(super) mod saved_cross_boundary;
#[cfg(test)]
pub(super) mod stage_boundary;
use observation::DestinationObservation;
pub(crate) use observation::ObservedSources;
pub use observation::ObservedTransfer;
pub(crate) use report::StoppedTransfer;
pub use report::{TransferPart, TransferReport, TransferReportFailure, TransferResolution};
pub(crate) use restart::RestartObservation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferKind {
    Copy,
    Move,
}

/// Platform placement checks run on the transfer worker. Source entries preserve
/// their final link; destination folders follow directory aliases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferAccess {
    SourceEntry,
    DestinationFolder,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferRequest {
    pub kind: TransferKind,
    #[serde(with = "crate::path_serde::sequence")]
    pub sources: Vec<PathBuf>,
    #[serde(with = "crate::path_serde")]
    pub destination: PathBuf,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropPreference {
    #[default]
    Automatic,
    Copy,
    Move,
}

impl TransferRequest {
    /// Prepare a drop off the UI thread. A mixed-filesystem selection defaults
    /// to one Copy operation so its pre-drop action remains unambiguous.
    pub fn from_drop(
        sources: Vec<PathBuf>,
        destination: PathBuf,
        preference: DropPreference,
        cancellation: &Cancellation,
    ) -> io::Result<Self> {
        check_cancel(cancellation)?;
        if sources.is_empty()
            || !destination.is_absolute()
            || sources.iter().any(|path| !path.is_absolute())
        {
            return Err(invalid(
                "a drop needs local absolute paths and a destination",
            ));
        }
        if !fs::metadata(&destination)?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "drop destination is not a folder",
            ));
        }
        let destination_mount = drop_mount(&destination, false)?;
        let mut same_mount = true;
        for source in &sources {
            check_cancel(cancellation)?;
            same_mount &= drop_mount(source, true).is_ok_and(|mount| mount == destination_mount);
        }
        let kind = match preference {
            DropPreference::Copy => TransferKind::Copy,
            DropPreference::Move => TransferKind::Move,
            DropPreference::Automatic => {
                if same_mount {
                    TransferKind::Move
                } else {
                    TransferKind::Copy
                }
            }
        };
        Ok(Self {
            sources,
            destination,
            kind,
        })
    }
}

fn drop_mount(path: &Path, nofollow: bool) -> io::Result<u64> {
    let status = rustix::fs::statx(
        rustix::fs::CWD,
        path,
        if nofollow {
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW
        } else {
            rustix::fs::AtFlags::empty()
        },
        rustix::fs::StatxFlags::MNT_ID,
    )?;
    if status.stx_mask & rustix::fs::StatxFlags::MNT_ID.bits() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "cannot identify the drop filesystem",
        ));
    }
    Ok(status.stx_mnt_id)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictChoice {
    Replace,
    KeepBoth,
    Skip,
    Merge,
}
#[derive(Clone, Debug)]
pub enum TransferEvent {
    Verifying {
        source: PathBuf,
        destination: PathBuf,
    },
    Progress {
        path: PathBuf,
        completed: usize,
        bytes: u64,
    },
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferReply {
    Continue,
    Resolve {
        choice: ConflictChoice,
        apply_to_remaining: bool,
    },
    Retry,
    Cancel,
}
#[derive(Clone, Debug)]
pub struct TransferFailure {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub message: String,
    pub kind: io::ErrorKind,
}
#[derive(Clone, Debug)]
pub struct TransferOutcome<C = TransferContinuation> {
    pub id: Option<u64>,
    pub completed: usize,
    pub bytes: u64,
    pub failures: Vec<TransferFailure>,
    pub skipped: Vec<PathBuf>,
    pub remaining: Vec<PathBuf>,
    /// Original source folders still needing review after a decomposed Move.
    /// These are presentation facts, not executable continuation plans.
    pub source_remainders: Vec<PathBuf>,
    pub continuation: Option<C>,
    /// Exact failed-item targets and the directory completions captured by this transfer.
    pub retry_continuation: Option<C>,
    pub destinations: Vec<PathBuf>,
    pub cancelled: bool,
    pub needs_attention: bool,
    pub attention: Option<String>,
}

/// A fatal engine/finalization error can follow committed filesystem changes.
/// Keep the confirmed report with the error; it does not authorize replay of
/// either the original request or its continuations before reconciliation.
#[derive(Debug)]
pub(crate) struct InterruptedTransfer {
    pub outcome: TransferOutcome,
    error: io::Error,
}

impl std::fmt::Display for InterruptedTransfer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for InterruptedTransfer {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl<C> Default for TransferOutcome<C> {
    fn default() -> Self {
        Self {
            id: None,
            completed: 0,
            bytes: 0,
            failures: Vec::new(),
            skipped: Vec::new(),
            remaining: Vec::new(),
            source_remainders: Vec::new(),
            continuation: None,
            retry_continuation: None,
            destinations: Vec::new(),
            cancelled: false,
            needs_attention: false,
            attention: None,
        }
    }
}
impl<C> TransferOutcome<C> {
    pub(crate) fn map_continuations<D>(self, mut map: impl FnMut(C) -> D) -> TransferOutcome<D> {
        TransferOutcome {
            id: self.id,
            completed: self.completed,
            bytes: self.bytes,
            failures: self.failures,
            skipped: self.skipped,
            remaining: self.remaining,
            source_remainders: self.source_remainders,
            continuation: self.continuation.map(&mut map),
            retry_continuation: self.retry_continuation.map(map),
            destinations: self.destinations,
            cancelled: self.cancelled,
            needs_attention: self.needs_attention,
            attention: self.attention,
        }
    }
}

/// Opaque exact targets for a paused/cancelled transfer. Resuming starts a new
/// Undo entry, preserving chronological history if other work happened meanwhile.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(into = "checkpoint::Checkpoint", try_from = "checkpoint::Checkpoint")]
pub struct TransferContinuation {
    pub kind: TransferKind,
    pub destination: PathBuf,
    pending: Vec<(PathBuf, PathBuf, Option<DirectoryFinish>)>,
    directories: BTreeMap<PathBuf, (PathBuf, DirectoryFinish)>,
    saved: BTreeMap<PathBuf, saved_stage::Receipt>,
    copied: BTreeMap<PathBuf, copied_leaf::Cleanup>,
    // Persisted original facts are descriptive; they are not process ownership.
    restart: Option<RestartObservation>,
    // Only a live outcome carries pins. Decoding never observes current paths.
    observed: Option<ObservedTransfer>,
    destination_observation: Option<DestinationObservation>,
}
impl TransferContinuation {
    pub(in crate::operations) fn reopen_stopped(&self) -> io::Result<Self> {
        if !self.saved.is_empty() {
            return Err(invalid(
                "Saved-container movement needs Recovery review before continuing.",
            ));
        }
        for (source, (destination, finish)) in &self.directories {
            if self
                .pending
                .iter()
                .any(|(path, _, _)| path.starts_with(source))
            {
                finish
                    .original
                    .ok_or_else(|| invalid("This older traversal has no original-folder proof."))?
                    .require(source, destination, false)?;
            }
        }
        for (source, proof) in &self.copied {
            if self.pending.iter().any(|(path, _, _)| path == source) {
                proof.require_originals()?;
            }
        }
        let observed = ObservedTransfer::reopen(self)?;
        let mut plan = self.clone();
        plan.destination_observation = Some(observed.destination.clone());
        plan.observed = Some(observed);
        Ok(plan)
    }
    /// The application's saved result is descriptive, with no live continuation
    /// owner. Retire its process pins explicitly, as checkpoint encoding does.
    pub(crate) fn into_saved_plan(mut self) -> Self {
        if self.restart.is_none()
            && let Some(observed) = &self.observed
        {
            self.restart = Some(observed.restart_observation());
        }
        self.observed = None;
        self.destination_observation = None;
        self
    }

    pub(crate) fn cancelled(&self) -> TransferOutcome {
        TransferOutcome {
            cancelled: true,
            remaining: self
                .pending
                .iter()
                .map(|(source, _, _)| source.clone())
                .collect(),
            continuation: (!self.pending.is_empty()).then(|| self.clone()),
            ..Default::default()
        }
    }

    pub(crate) fn matches_request(&self, request: &TransferRequest) -> bool {
        if self.matches_report(request.kind, &request.destination, &request.sources, false) {
            return true;
        }
        if self.kind != request.kind
            || self.destination != request.destination
            || !self.directories.is_empty()
            || !self.saved.is_empty()
            || self.pending.is_empty()
        {
            return false;
        }
        let Some(expected) = self
            .restart
            .as_ref()
            .and_then(|original| original.initial_sources(request))
        else {
            return false;
        };
        self.pending.iter().all(|(source, destination, finish)| {
            finish.is_none()
                && *destination
                    == source
                        .file_name()
                        .map(|name| request.destination.join(name))
                        .unwrap_or_else(|| request.destination.clone())
        }) && self
            .pending
            .iter()
            .map(|(source, _, _)| source.clone())
            .collect::<BTreeSet<_>>()
            == expected
    }

    /// Check presentation facts without exposing or reconstructing execution
    /// directives. Failed plans may also contain owned directory completions.
    pub(crate) fn matches_report(
        &self,
        kind: TransferKind,
        destination: &Path,
        sources: &[PathBuf],
        failed: bool,
    ) -> bool {
        if self.kind != kind || self.destination != destination || self.pending.is_empty() {
            return false;
        }
        if let Some(original) = &self.restart {
            let request = TransferRequest {
                kind: self.kind,
                destination: self.destination.clone(),
                sources: self
                    .pending
                    .iter()
                    .map(|(source, _, _)| source.clone())
                    .collect(),
            };
            if !original.covers(&request) {
                return false;
            }
        }
        if !failed {
            return self
                .pending
                .iter()
                .map(|(source, _, _)| source)
                .eq(sources.iter());
        }
        let expected: BTreeSet<_> = sources.iter().collect();
        !expected.is_empty()
            && expected.iter().all(|source| {
                self.pending
                    .iter()
                    .any(|(pending, _, _)| pending == *source)
            })
            && self
                .pending
                .iter()
                .all(|(source, _, finish)| finish.is_some() || expected.contains(source))
    }

    pub(super) fn relocate_recovery_source(&mut self, from: &Path, to: &Path) {
        fn apply(path: &mut PathBuf, from: &Path, to: &Path) {
            if let Ok(tail) = path.strip_prefix(from) {
                *path = to.join(tail);
            }
        }
        for (source, _, _) in &mut self.pending {
            apply(source, from, to);
        }
        self.copied = std::mem::take(&mut self.copied)
            .into_iter()
            .map(|(mut source, mut proof)| {
                apply(&mut source, from, to);
                apply(&mut proof.source.path, from, to);
                (source, proof)
            })
            .collect();
        self.directories = std::mem::take(&mut self.directories)
            .into_iter()
            .map(|(mut source, rest)| {
                apply(&mut source, from, to);
                (source, rest)
            })
            .collect();
    }
    pub(super) fn recovery(source: PathBuf, destination: PathBuf) -> Self {
        Self {
            kind: TransferKind::Copy,
            destination: destination.parent().unwrap().to_owned(),
            pending: vec![(source, destination, None)],
            directories: BTreeMap::new(),
            saved: BTreeMap::new(),
            copied: BTreeMap::new(),
            restart: None,
            observed: None,
            destination_observation: None,
        }
    }
    pub(super) fn recovery_root_identity(
        &self,
        source: &Path,
        destination: &Path,
    ) -> Option<Identity> {
        self.directories
            .get(source)
            .filter(|(target, _)| target == destination)
            .map(|(_, finish)| finish.destination_identity)
    }

    pub(crate) fn paused(self, attention: String) -> TransferOutcome {
        TransferOutcome {
            remaining: self
                .pending
                .iter()
                .map(|(source, _, _)| source.clone())
                .collect(),
            continuation: Some(self),
            needs_attention: true,
            attention: Some(attention),
            ..TransferOutcome::default()
        }
    }

    pub(crate) fn storage_destinations(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.pending
            .iter()
            .map(|(_, destination, _)| destination.clone())
    }

    pub(crate) fn source_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.pending.iter().map(|(source, _, _)| source)
    }

    /// Sources still to transfer, without post-order folder removals.
    pub(crate) fn task_sources(&self) -> impl Iterator<Item = &PathBuf> {
        self.pending
            .iter()
            .filter(|(_, _, finish)| finish.is_none())
            .map(|(source, _, _)| source)
    }

    pub(crate) fn observed_transfer(&self) -> Option<&ObservedTransfer> {
        self.observed.as_ref()
    }

    pub(crate) fn unstarted(observed: &ObservedTransfer) -> Self {
        let request = observed.request();
        let mut sources: BTreeSet<_> = request.sources.iter().cloned().collect();
        let parents: Vec<_> = sources
            .iter()
            .filter(|source| observed.original_directory(source))
            .cloned()
            .collect();
        sources.retain(|source| {
            !parents
                .iter()
                .any(|parent| source != parent && source.starts_with(parent))
        });
        Self {
            kind: request.kind,
            destination: request.destination.clone(),
            pending: sources
                .into_iter()
                .map(|source| {
                    let destination = source
                        .file_name()
                        .map(|name| request.destination.join(name))
                        .unwrap_or_else(|| request.destination.clone());
                    (source, destination, None)
                })
                .collect(),
            directories: BTreeMap::new(),
            saved: BTreeMap::new(),
            copied: BTreeMap::new(),
            restart: Some(observed.restart_observation()),
            observed: Some(observed.clone()),
            destination_observation: Some(observed.destination.clone()),
        }
    }
    /// Build an independent path-only retry. Prefer an outcome's retry_continuation
    /// when retrying a directory traversal; a path list cannot prove parent ownership.
    pub fn retry_failed(
        kind: TransferKind,
        destination: PathBuf,
        failures: &[TransferFailure],
    ) -> Self {
        Self {
            kind,
            destination,
            pending: failures
                .iter()
                .map(|failure| (failure.source.clone(), failure.destination.clone(), None))
                .collect(),
            directories: BTreeMap::new(),
            saved: BTreeMap::new(),
            copied: BTreeMap::new(),
            restart: None,
            observed: None,
            destination_observation: None,
        }
    }
}

fn retry_continuation(
    request: &TransferContinuation,
    failed: Vec<(PathBuf, PathBuf, Option<DirectoryFinish>)>,
    directories: &BTreeMap<PathBuf, (PathBuf, DirectoryFinish)>,
    saved: &BTreeMap<PathBuf, saved_stage::Receipt>,
    copied: &BTreeMap<PathBuf, copied_leaf::Cleanup>,
    observed: Option<&ObservedTransfer>,
    destination_observation: &DestinationObservation,
) -> Option<TransferContinuation> {
    if failed.is_empty() {
        return None;
    }
    let pending = retry_tasks(request.kind, failed, directories);
    Some(TransferContinuation {
        kind: request.kind,
        destination: request.destination.clone(),
        copied: copied_for(&pending, copied),
        pending,
        directories: directories.clone(),
        saved: saved.clone(),
        restart: observed.map(ObservedTransfer::restart_observation),
        observed: observed.cloned(),
        destination_observation: Some(destination_observation.clone()),
    })
}

fn copied_for(
    pending: &[(PathBuf, PathBuf, Option<DirectoryFinish>)],
    copied: &BTreeMap<PathBuf, copied_leaf::Cleanup>,
) -> BTreeMap<PathBuf, copied_leaf::Cleanup> {
    pending
        .iter()
        .filter_map(|(source, _, _)| {
            copied
                .get(source)
                .map(|proof| (source.clone(), proof.clone()))
        })
        .collect()
}

fn retry_tasks(
    kind: TransferKind,
    failed: Vec<(PathBuf, PathBuf, Option<DirectoryFinish>)>,
    directories: &BTreeMap<PathBuf, (PathBuf, DirectoryFinish)>,
) -> Vec<(PathBuf, PathBuf, Option<DirectoryFinish>)> {
    let mut pending = Vec::new();
    let mut completed = BTreeSet::new();
    for (index, task) in failed.iter().enumerate() {
        pending.push(task.clone());
        if task.2.is_some() {
            completed.insert(task.0.clone());
        }
        if kind != TransferKind::Move {
            continue;
        }
        for ancestor in task.0.ancestors() {
            if failed
                .get(index + 1)
                .is_some_and(|next| next.0.starts_with(ancestor))
            {
                continue;
            }
            if let Some((target, finish)) = directories.get(ancestor)
                && completed.insert(ancestor.to_owned())
            {
                pending.push((ancestor.to_owned(), target.clone(), Some(*finish)));
            }
        }
    }
    pending
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectoryFinish {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original: Option<directory::DirectoryOriginal>,
    identity: Identity,
    parent_identity: Identity,
    mode: u32,
    completed_before: usize,
    #[serde(default, skip_serializing_if = "is_false")]
    was_empty: bool,
    operation: u64,
    first: u64,
    created: Option<u64>,
    destination_identity: Identity,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl OperationEngine {
    pub fn transfer(
        &self,
        request: &TransferRequest,
        cancellation: &Cancellation,
        interact: impl FnMut(TransferEvent) -> TransferReply,
    ) -> io::Result<TransferOutcome> {
        self.transfer_with_check(request, cancellation, |_, _| Ok(()), interact)
    }

    pub fn resume_transfer(
        &self,
        request: &TransferContinuation,
        cancellation: &Cancellation,
        interact: impl FnMut(TransferEvent) -> TransferReply,
    ) -> io::Result<TransferOutcome> {
        self.resume_transfer_with_check(request, cancellation, |_, _| Ok(()), interact)
    }

    pub fn transfer_with_check(
        &self,
        request: &TransferRequest,
        cancellation: &Cancellation,
        check: impl FnMut(&Path, TransferAccess) -> io::Result<()>,
        interact: impl FnMut(TransferEvent) -> TransferReply,
    ) -> io::Result<TransferOutcome> {
        self.transfer_observed_with_check(
            &request.observe_sources()?,
            cancellation,
            check,
            interact,
        )
    }

    /// Execute the immutable request against its earlier selected-entry
    /// observation. Scheduling callers must capture it at admission, not dispatch.
    pub fn transfer_observed_with_check(
        &self,
        observed: &ObservedTransfer,
        cancellation: &Cancellation,
        check: impl FnMut(&Path, TransferAccess) -> io::Result<()>,
        interact: impl FnMut(TransferEvent) -> TransferReply,
    ) -> io::Result<TransferOutcome> {
        let request = observed.request();
        if !request.destination.is_absolute() {
            return Err(invalid("transfer destination must be absolute"));
        }
        self.resume_transfer_checked(
            &TransferContinuation::unstarted(observed),
            cancellation,
            Some(observed),
            None,
            check,
            interact,
        )
    }

    pub fn resume_transfer_with_check(
        &self,
        request: &TransferContinuation,
        cancellation: &Cancellation,
        check: impl FnMut(&Path, TransferAccess) -> io::Result<()>,
        interact: impl FnMut(TransferEvent) -> TransferReply,
    ) -> io::Result<TransferOutcome> {
        self.resume_transfer_checked(
            request,
            cancellation,
            request.observed.as_ref(),
            None,
            check,
            interact,
        )
    }

    pub(super) fn resume_transfer_checked(
        &self,
        request: &TransferContinuation,
        cancellation: &Cancellation,
        observed: Option<&ObservedTransfer>,
        execution: Option<&requests::RequestRecord>,
        mut check: impl FnMut(&Path, TransferAccess) -> io::Result<()>,
        interact: impl FnMut(TransferEvent) -> TransferReply,
    ) -> io::Result<TransferOutcome> {
        if let Some(observed) = observed
            && (request.kind != observed.request().kind
                || request.destination != observed.request().destination)
        {
            return Err(invalid(
                "A continuation must keep its original transfer kind and destination",
            ));
        }
        if request
            .destination_observation
            .as_ref()
            .is_some_and(|original| original.path() != request.destination)
        {
            return Err(invalid(
                "A continuation must keep its original destination folder",
            ));
        }
        if cancellation.is_cancelled() {
            return Ok(request.cancelled());
        }
        if !request.destination.is_absolute() {
            return Err(invalid("transfer destination must be absolute"));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?;
        if let Err(error) = check_location(
            &mut check,
            &request.destination,
            TransferAccess::DestinationFolder,
        )
        .and_then(|()| check_pending(&state, &mut check))
        {
            return Ok(TransferOutcome {
                needs_attention: true,
                attention: Some(error.to_string()),
                remaining: request
                    .pending
                    .iter()
                    .map(|(source, _, _)| source.clone())
                    .collect(),
                continuation: Some(request.clone()),
                ..Default::default()
            });
        }
        recover(&mut state, cancellation)?;
        run_transfer(
            &mut state,
            request,
            cancellation,
            check,
            interact,
            RunContext::Transfer {
                observed,
                execution,
            },
        )
    }
}

pub(super) enum RunContext<'a> {
    Transfer {
        observed: Option<&'a ObservedTransfer>,
        execution: Option<&'a requests::RequestRecord>,
    },
    Recovery(&'a str),
}

/// Cancel stops new copying. Compensation either proves disposal or durably
/// retains this task's partial data before the batch may advance to another item.
fn compensate_failed_task(state: &mut Store, id: u64, destination: &Path) -> io::Result<bool> {
    match recover(state, &Cancellation::default()) {
        Ok(()) => Ok(false),
        Err(error) if fingerprint::source::is_reproduction_problem(&error) => {
            state.retain_failed_partial(id, destination)?;
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

/// Shared recursive execution. The caller owns writer locking, reconciliation
/// and journal-lane policy; Recovery uses its independently protected lane.
pub(super) fn run_transfer(
    state: &mut Store,
    request: &TransferContinuation,
    cancellation: &Cancellation,
    check: impl FnMut(&Path, TransferAccess) -> io::Result<()>,
    interact: impl FnMut(TransferEvent) -> TransferReply,
    context: RunContext<'_>,
) -> io::Result<TransferOutcome> {
    let mut outcome = TransferOutcome::default();
    // New-item Copy batches its barriers; Move and Recovery keep per-step
    // syncs (ADR0165). The final flush makes "done" mean on the device.
    let session = (request.kind == TransferKind::Copy
        && matches!(context, RunContext::Transfer { .. }))
    .then(|| super::durability::Session::begin(state.history_observation_paths()));
    let result = state.with_directory_access(|state| {
        run_transfer_inner(
            state,
            request,
            cancellation,
            check,
            interact,
            context,
            &mut outcome,
        )
    });
    let flushed = session.map_or(Ok(()), super::durability::Session::flush);
    let result = result.and(flushed).and_then(|()| state.trim());
    match result {
        Ok(()) => Ok(outcome),
        Err(error) => Err(io::Error::new(
            error.kind(),
            InterruptedTransfer { outcome, error },
        )),
    }
}

fn run_transfer_inner(
    state: &mut Store,
    request: &TransferContinuation,
    cancellation: &Cancellation,
    mut placement: impl FnMut(&Path, TransferAccess) -> io::Result<()>,
    mut interact: impl FnMut(TransferEvent) -> TransferReply,
    context: RunContext<'_>,
    result: &mut TransferOutcome,
) -> io::Result<()> {
    let (recovery_label, observed, execution) = match context {
        RunContext::Transfer {
            observed,
            execution,
        } => (None, observed, execution),
        RunContext::Recovery(label) => (Some(label), None, None),
    };
    if cancellation.is_cancelled() {
        *result = request.cancelled();
        return Ok(());
    }
    let mut verified = BTreeSet::new();
    let destination_observation = match request.destination_observation.clone() {
        Some(original) => original,
        None => DestinationObservation::capture(request.destination.clone())?,
    };
    if let Some(observed) = observed {
        for (path, (_, finish)) in &request.directories {
            if observed.verified_directory(path, finish.identity) {
                verified.insert(path.clone());
            }
        }
    }
    if recovery_label.is_some() && request.kind != TransferKind::Copy {
        return Err(invalid("Recovery can only copy retained data"));
    }
    let mut pending: VecDeque<_> = request.pending.clone().into();
    let mut directories = request.directories.clone();
    let mut saved_receipts = request.saved.clone();
    let mut copied_receipts = request.copied.clone();
    for (source, destination, finish) in &pending {
        if let Some(finish) = finish {
            directories
                .entry(source.clone())
                .or_insert_with(|| (destination.clone(), *finish));
        }
    }
    let mut first_ordinals = BTreeMap::new();
    let mut failed = Vec::new();
    // Postorder removals from a previous run do not count as new empty folders.
    for (_, _, finish) in &mut pending {
        if let Some(finish) = finish {
            finish.completed_before = usize::MAX;
        }
    }
    if pending.is_empty() {
        return Ok(());
    }
    let label = recovery_label.map(str::to_owned).unwrap_or_else(|| {
        format!(
            "{} to {}",
            if request.kind == TransferKind::Copy {
                "copy"
            } else {
                "move"
            },
            display_name(&request.destination)
        )
    });
    let original = observed.map(ObservedTransfer::restart_observation);
    let id = state.begin_transfer(
        &label,
        execution,
        request.kind,
        &request.destination,
        original.as_ref(),
    )?;
    let mut pending = cursor::Queue::new(pending.into_iter().collect());
    let mut changed_directories = Vec::new();
    let mut cursor = match (execution, observed) {
        (Some(execution), Some(observed)) => {
            let mut initial = request.clone();
            initial.pending = pending.iter().cloned().collect();
            initial.directories = directories.clone();
            Some(cursor::Tracker::begin(
                state,
                execution,
                id,
                &initial,
                observed,
                &destination_observation,
            )?)
        }
        _ => None,
    };
    let mut file_choice = None;
    let mut folder_choice = None;
    let mut mixed_choice = None;
    while !pending.is_empty() {
        state.restore_directory_access()?;
        if let Some(cursor) = &mut cursor {
            // Restart cursors share the history database, so a power loss
            // still leaves a consistent earlier journal prefix (ADR0165).
            state.deferred(true, |state| {
                cursor.checkpoint(
                    state,
                    cursor::Checkpoint {
                        queue: &mut pending,
                        directories: &directories,
                        changed_directories: &mut changed_directories,
                        saved: &saved_receipts,
                        copied: &copied_receipts,
                        failed: &failed,
                        result,
                        root: &destination_observation,
                    },
                )
            })?;
        }
        let (source, mut destination, finish) = pending.pop_front().unwrap();
        if cancellation.is_cancelled() {
            result.cancelled = true;
            pending.push_front((source, destination, finish));
            break;
        }
        if let Err(error) = check_location(
            &mut placement,
            &request.destination,
            TransferAccess::DestinationFolder,
        ) {
            result.needs_attention = true;
            result.attention = Some(error.to_string());
            pending.push_front((source, destination, finish));
            break;
        }
        while let Err(error) = destination_observation.check() {
            super::durability::flush()?;
            match interact(TransferEvent::Unavailable {
                destination: request.destination.clone(),
                message: format!("The destination folder is unavailable or has changed: {error}"),
            }) {
                TransferReply::Retry => {
                    check_cancel(cancellation)?;
                }
                TransferReply::Cancel => {
                    result.cancelled = true;
                    break;
                }
                _ => {
                    result.needs_attention = true;
                    break;
                }
            }
        }
        if result.cancelled || result.needs_attention {
            pending.push_front((source, destination, finish));
            break;
        }
        if let Some(cursor) = &mut cursor {
            cursor.bind_root(state, &destination_observation)?;
        }
        // Placement is checked before binding an initially unavailable root.
        // Root loss keeps the existing Unavailable/Retry interaction above;
        // every actual filesystem step also rechecks that same opened folder.
        let mut check = |path: &Path, access| {
            placement(path, access)?;
            if access == TransferAccess::DestinationFolder {
                destination_observation.check_if_observed()?;
            }
            if access == TransferAccess::SourceEntry
                && let Some(observed) = observed
            {
                observed.check(path, &mut verified)?;
            }
            Ok(())
        };
        let mut reconciled = None;
        let operation = (|| -> io::Result<bool> {
            if let Some(receipt) = saved_receipts.get(&source).copied()
                && state.saved_move_completed(&receipt, &source, &destination)?
            {
                saved_receipts.remove(&source);
                reconciled = Some(receipt.id);
                return Ok(!directories.contains_key(source.parent().unwrap()));
            }
            validate_target(&source)?;
            validate_target(&destination)?;
            let access_parent = directories
                .get(source.parent().unwrap())
                .filter(|(_, finish)| finish.created.is_some())
                .map(|(_, finish)| finish.destination_identity);
            // Reach each owned folder through its original created ancestors.
            // Opening the immediate parent first fails when an outer created
            // folder was made non-searchable while this transfer was stopped.
            let ancestors: Vec<_> = source.ancestors().skip(1).collect();
            for ancestor in ancestors.into_iter().rev() {
                if let Some((target, owner)) = directories.get(ancestor)
                    && owner.created.is_some()
                {
                    let original = owner.original.ok_or_else(|| {
                        invalid(
                            "Temporary transfer access needs its original created folder proof.",
                        )
                    })?;
                    state.permit_transfer_directory(
                        target,
                        owner.destination_identity,
                        original.destination_origins(),
                    )?;
                }
            }
            checkpoint(
                &mut check,
                &source,
                &destination,
                &directories,
                finish.is_some(),
            )?;
            let first_ordinal = state.next_ordinal(id)?;
            for ancestor in source.ancestors() {
                if directories.contains_key(ancestor) {
                    first_ordinals
                        .entry(ancestor.to_owned())
                        .or_insert(first_ordinal);
                }
            }
            if let Some(proof) = copied_receipts.get(&source).cloned() {
                if request.kind != TransferKind::Move || !proof.valid_for(&source, &destination) {
                    return Err(invalid(
                        "The copied-source cleanup differs from its exact task.",
                    ));
                }
                proof.execute(state, id, cancellation, || {
                    checkpoint(&mut check, &source, &destination, &directories, false)
                })?;
                copied_receipts.remove(&source);
                return Ok(true);
            }
            if let Some(DirectoryFinish {
                original,
                identity,
                parent_identity,
                mode,
                completed_before,
                was_empty,
                operation,
                first,
                created,
                destination_identity,
            }) = finish
            {
                if directories.get(&source).is_some_and(|(target, recorded)| {
                    target != &destination
                        || recorded.identity != identity
                        || recorded.destination_identity != destination_identity
                }) {
                    return Ok(false);
                }
                if !destination_identity.matches(&destination) {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "the moved folder's destination changed before source cleanup",
                    ));
                }
                match fs::symlink_metadata(&source) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
                    Err(error) => return Err(error),
                    Ok(_) => {}
                }
                if !identity.matches(&source) {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "the source folder has moved or changed",
                    ));
                }
                if fs::read_dir(&source)?.next().transpose()?.is_none() {
                    let removal = directory_removal::Prepared::open(
                        &source,
                        &destination,
                        original,
                        identity,
                        parent_identity,
                        destination_identity,
                    )?;
                    let change = Change::RemovedDirectory {
                        removal: Some(Box::new(removal.facts().clone())),
                        path: source.clone(),
                        identity,
                        parent_identity,
                        mode,
                        moved: Some(MovedDirectory {
                            current: destination.clone(),
                            identity: destination_identity,
                            first: if operation == id {
                                first
                            } else {
                                *first_ordinals.get(&source).ok_or_else(|| {
                                    invalid("directory continuation has no recorded scope")
                                })?
                            },
                            created: if operation == id { created } else { None },
                        }),
                    };
                    checkpoint(
                        &mut check,
                        &source,
                        &destination,
                        &directories,
                        finish.is_some(),
                    )?;
                    removal.commit(state, id, &change, cancellation, || {
                        checkpoint(&mut check, &source, &destination, &directories, true)
                    })?;
                    return Ok(
                        result.completed == completed_before || (was_empty && created.is_none())
                    );
                }
                // A confirmed nonempty original source folder is descriptive
                // completed work. Persist that observation before later tasks.
                if !source
                    .ancestors()
                    .skip(1)
                    .any(|parent| directories.contains_key(parent))
                {
                    result.source_remainders.push(source.clone());
                }
                return Ok(false);
            }
            if source == destination {
                return Err(invalid("source and destination are the same item"));
            }
            let mut move_selection = (request.kind == TransferKind::Move)
                .then(|| entry_move::Selection::open(&source, &destination))
                .transpose()?;
            let entry = Entry::read(&source)?;
            if let Some(selection) = &move_selection {
                selection.require_current()?;
            }
            let saved_record = if entry.kind == EntryKind::Directory {
                let identity = Identity::read(&source)?;
                if recovery_label.is_some() && state.is_recovery_container(&source, identity)? {
                    None
                } else {
                    state.owned_saved_stage(&source, identity)?
                }
            } else {
                None
            };
            if request.kind == TransferKind::Copy
                && directories.contains_key(source.parent().unwrap())
                && let Some(record) = &saved_record
            {
                saved_stage::omit_nested_copy(record)?;
                return Ok(false);
            }
            if request.kind == TransferKind::Move
                && directories.contains_key(source.parent().unwrap())
                && saved_record.is_some()
                && fs::symlink_metadata(&destination).is_ok()
            {
                destination = unique_name(&destination)?;
            }
            if entry.kind == EntryKind::Directory {
                let real_source = fs::canonicalize(&source)?;
                let real_destination = fs::canonicalize(&request.destination)?;
                if real_destination.starts_with(real_source) {
                    return Err(invalid("a folder cannot be transferred into itself"));
                }
            }
            let mut merge = false;
            let mut merge_opened = None;
            let mut replacement = None;
            match Entry::read(&destination) {
                Ok(existing) => {
                    let existing_kind = existing.kind;
                    let reviewed = matches!(existing_kind, EntryKind::File | EntryKind::Symlink)
                        .then(|| replacement::Reviewed::open(&destination))
                        .transpose()?;
                    let approved_version = match &reviewed {
                        Some(reviewed) => reviewed.version().clone(),
                        None => fingerprint::Version::read(&destination)?,
                    };
                    let folders =
                        entry.kind == EntryKind::Directory && existing.kind == EntryKind::Directory;
                    let approved_folder = if folders {
                        let opened = directory::Opened::open(&destination)?;
                        if opened.identity != approved_version.identity {
                            return Err(io::Error::new(
                                io::ErrorKind::WouldBlock,
                                "The Merge folder changed before review.",
                            ));
                        }
                        Some(opened)
                    } else {
                        None
                    };
                    let captured_destination = folders
                        && directories.get(&source).is_some_and(|(target, finish)| {
                            target == &destination
                                && finish.destination_identity == approved_version.identity
                        });
                    if recovery_label.is_some()
                        && destination.parent() == Some(request.destination.as_path())
                        && !captured_destination
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "the recovery destination already exists; choose a new name",
                        ));
                    }
                    let cached = match (entry.kind, existing.kind) {
                        (EntryKind::Directory, EntryKind::Directory) => &mut folder_choice,
                        (
                            EntryKind::File | EntryKind::Symlink,
                            EntryKind::File | EntryKind::Symlink,
                        ) => &mut file_choice,
                        _ => &mut mixed_choice,
                    };
                    let choice = if captured_destination {
                        ConflictChoice::Merge
                    } else if let Some(choice) = *cached {
                        choice
                    } else {
                        // Earlier copies are durable while the user decides.
                        super::durability::flush()?;
                        match interact(TransferEvent::Conflict {
                            source: entry.clone(),
                            destination: existing,
                            folders,
                        }) {
                            TransferReply::Resolve {
                                choice,
                                apply_to_remaining,
                            } => {
                                if apply_to_remaining {
                                    *cached = Some(choice);
                                }
                                choice
                            }
                            TransferReply::Cancel => {
                                cancellation.cancel();
                                return Err(io::Error::new(
                                    io::ErrorKind::Interrupted,
                                    "transfer cancelled",
                                ));
                            }
                            _ => ConflictChoice::Skip,
                        }
                    };
                    match choice {
                        ConflictChoice::Skip => {
                            result.skipped.push(source.clone());
                            return Ok(false);
                        }
                        ConflictChoice::KeepBoth => {
                            destination = unique_name(&destination)?;
                        }
                        ConflictChoice::Merge if folders => {
                            merge = true;
                            merge_opened = approved_folder;
                        }
                        ConflictChoice::Replace => {
                            if recovery_label.is_some() {
                                return Err(io::Error::new(
                                    io::ErrorKind::AlreadyExists,
                                    "Recovery copies never replace existing work; keep both or skip this item",
                                ));
                            }
                            if entry.kind == EntryKind::Directory
                                || existing_kind == EntryKind::Directory
                            {
                                return Err(invalid("folders can be merged or kept separately"));
                            }
                            replacement = Some(reviewed.ok_or_else(|| {
                                invalid("The replacement entry was not observed.")
                            })?);
                        }
                        _ => {
                            return Err(invalid(
                                "this conflict choice does not apply to these items",
                            ));
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            check_cancel(cancellation)?;
            checkpoint(
                &mut check,
                &source,
                &destination,
                &directories,
                finish.is_some(),
            )?;
            let same_filesystem = match &move_selection {
                Some(selection) => {
                    selection.require_current()?;
                    selection.same_filesystem()?
                }
                None => {
                    fs::symlink_metadata(&source)?.dev()
                        == fs::metadata(destination.parent().unwrap())?.dev()
                }
            };
            if request.kind == TransferKind::Move && merge && saved_record.is_some() {
                return Err(invalid(
                    "saved data must move as one folder; choose Keep both or Skip",
                ));
            }
            if request.kind == TransferKind::Move && !merge && same_filesystem {
                if let Some(record) = &saved_record {
                    let counted = !directories.contains_key(source.parent().unwrap());
                    let context = directories
                        .get(source.parent().unwrap())
                        .map(|(target, _)| saved_stage::ContextMovement {
                            from: source.parent().unwrap().to_owned(),
                            to: target.clone(),
                        });
                    let prepared = move_selection
                        .take()
                        .unwrap()
                        .prepare(&destination, access_parent)?
                        .saved(record, context, counted)?;
                    prepared.commit(state, id, cancellation, || {
                        checkpoint(&mut check, &source, &destination, &directories, false)
                    })?;
                    return Ok(counted);
                }
                let mut prepared = move_selection
                    .take()
                    .unwrap()
                    .prepare(&destination, access_parent)?;
                if let Some(reviewed) = replacement.take() {
                    prepared = prepared.replacing(reviewed)?;
                }
                prepared.commit(state, id, cancellation, || {
                    checkpoint(&mut check, &source, &destination, &directories, false)
                })?;
                return Ok(true);
            }
            if request.kind == TransferKind::Move
                && !same_filesystem
                && let Some(record) = &saved_record
            {
                let ordinal = state.next_ordinal(id)?;
                saved_receipts.insert(
                    source.clone(),
                    saved_stage::Receipt {
                        id,
                        ordinal,
                        identity: record.retained.stage.identity,
                    },
                );
                saved_stage::prepare_forward(
                    state,
                    saved_stage::Forward {
                        selection: move_selection.take().unwrap(),
                        counted: !directories.contains_key(source.parent().unwrap()),
                        cleanup_parent: access_parent,
                        id,
                        ordinal,
                        source: &source,
                        destination: &destination,
                        record,
                        context: directories
                            .get(source.parent().unwrap())
                            .map(|(target, _)| saved_stage::ContextMovement {
                                from: source.parent().unwrap().to_owned(),
                                to: target.clone(),
                            }),
                    },
                    cancellation,
                    |bytes| {
                        result.bytes = result.bytes.saturating_add(bytes);
                        if interact(TransferEvent::Progress {
                            path: source.clone(),
                            completed: result.completed,
                            bytes: result.bytes,
                        }) == TransferReply::Cancel
                        {
                            cancellation.cancel();
                        }
                    },
                    || checkpoint(&mut check, &source, &destination, &directories, false),
                )?;
                #[cfg(test)]
                saved_cross_boundary::boundary("finished", &source, &destination);
                saved_receipts.remove(&source);
                return Ok(!directories.contains_key(source.parent().unwrap()));
            }
            if entry.kind == EntryKind::Directory {
                let opened_source = directory::Opened::open(&source)?;
                checkpoint(&mut check, &source, &destination, &directories, false)?;
                opened_source.require_current()?;
                let metadata = opened_source.metadata()?;
                let parent_identity = opened_source.parent_identity;
                // Read names from the actual opened original folder. A later
                // namespace replacement cannot redirect this enumeration.
                let children = opened_source.children()?;
                let directory_first = state.next_ordinal(id)?;
                let completed_before = result.completed;
                let make_finish = |destination_identity, destination_original| {
                    if let Some((target, old)) = directories.get(&source)
                        && target == &destination
                        && old.destination_identity == destination_identity
                    {
                        // Reprocessing a root after a post-install refusal retains
                        // its earlier creation owner and chosen metadata. Its root
                        // was already counted by the earlier execution.
                        DirectoryFinish {
                            was_empty: children.is_empty(),
                            completed_before: usize::MAX,
                            ..*old
                        }
                    } else {
                        DirectoryFinish {
                            original: directory::DirectoryOriginal::capture(
                                &opened_source,
                                destination_original,
                            ),
                            identity: opened_source.identity,
                            parent_identity,
                            mode: metadata.mode() & 0o777,
                            completed_before,
                            was_empty: children.is_empty(),
                            operation: id,
                            first: directory_first,
                            created: (!merge).then_some(directory_first),
                            destination_identity,
                        }
                    }
                };
                let finish = if !merge {
                    let (created, stage, temporary) =
                        directory::Created::prepare(&destination, metadata.mode())?;
                    let identity = created.identity;
                    let original = created.original();
                    let change = Change::CreatedFolder {
                        access_parent,
                        mode: Some(metadata.mode() & 0o777),
                        path: destination.clone(),
                        identity,
                    };
                    let prepared_finish = make_finish(identity, original);
                    state.deferred(true, |state| {
                        if let Some(cursor) = &mut cursor {
                            cursor.prepare_directory(
                                state,
                                &pending,
                                cursor::expansion::Preparation {
                                    task: (
                                        source.clone(),
                                        destination.clone(),
                                        Some(prepared_finish),
                                    ),
                                    stage: Some(&stage),
                                    change: Some(&change),
                                    names: &children,
                                    operation: id,
                                },
                            )
                        } else {
                            state.register_preparing(
                                id,
                                &destination,
                                &created.registration(&opened_source, &stage)?,
                            )
                        }
                    })?;
                    #[cfg(test)]
                    directory_boundary::boundary("preparing", &source, &destination);
                    checkpoint(
                        &mut check,
                        &source,
                        &destination,
                        &directories,
                        finish.is_some(),
                    )?;
                    opened_source.require_current()?;
                    state.deferred(true, |state| {
                        commit_forward_with_created(
                            state,
                            id,
                            change,
                            Some((stage, temporary)),
                            Some(created.file()),
                            |_| {
                                created.publish()?;
                                #[cfg(test)]
                                directory_boundary::boundary("published", &source, &destination);
                                Ok(())
                            },
                        )
                    })?;
                    super::durability::completed(0)?;
                    #[cfg(test)]
                    directory_boundary::boundary("appended", &source, &destination);
                    result.completed += 1;
                    result.id = Some(id);
                    if destination.parent() == Some(request.destination.as_path()) {
                        result.destinations.push(destination.clone());
                    }
                    prepared_finish
                } else {
                    let opened = merge_opened
                        .as_ref()
                        .ok_or_else(|| invalid("missing approved Merge folder"))?;
                    opened.require_current()?;
                    let finish = make_finish(opened.identity, opened.original());
                    if let Some(cursor) = &mut cursor {
                        state.deferred(true, |state| {
                            cursor.prepare_directory(
                                state,
                                &pending,
                                cursor::expansion::Preparation {
                                    task: (source.clone(), destination.clone(), Some(finish)),
                                    stage: None,
                                    change: None,
                                    names: &children,
                                    operation: id,
                                },
                            )
                        })?;
                    }
                    #[cfg(test)]
                    directory_boundary::boundary("merged", &source, &destination);
                    finish
                };
                directories.insert(source.clone(), (destination.clone(), finish));
                changed_directories.push((source.clone(), destination.clone(), Some(finish)));
                check_location(&mut check, &destination, TransferAccess::DestinationFolder)?;
                checkpoint(&mut check, &source, &destination, &directories, false)?;
                if request.kind == TransferKind::Move {
                    pending.push_front((source.clone(), destination.clone(), Some(finish)));
                }
                for name in children.into_iter().rev() {
                    pending.push_front((source.join(&name), destination.join(name), None));
                }
                return Ok(false);
            } else {
                // Only a new item with no reviewed replacement batches its
                // barriers; Replace starts from a durable state (ADR0165).
                let plain = replacement.is_none();
                if !plain {
                    super::durability::flush()?;
                }
                let bytes_before = result.bytes;
                let prepared_copy::PreparedCopy {
                    source: source_pin,
                    opened,
                    mut stage,
                    temporary,
                    identity,
                    fingerprint,
                    source_identity,
                    source_fingerprint,
                } = state.deferred(plain, |state| {
                    prepared_copy::prepare_copy(
                        &source,
                        destination.parent().unwrap(),
                        cancellation,
                        |created| state.register_preparing(id, &destination, created),
                        |bytes| {
                            result.bytes = result.bytes.saturating_add(bytes);
                            if interact(TransferEvent::Progress {
                                path: source.clone(),
                                completed: result.completed,
                                bytes: result.bytes,
                            }) == TransferReply::Cancel
                            {
                                cancellation.cancel();
                            }
                        },
                        || {
                            checkpoint(
                                &mut check,
                                &source,
                                &destination,
                                &directories,
                                finish.is_some(),
                            )
                        },
                    )
                })?;
                stage.record_parent_access(access_parent);
                let change = Change::Copied {
                    source: copied_leaf::Source::capture(&source_pin, &source_fingerprint),
                    publication: opened.expected_publication().map(Box::new),
                    access_parent,
                    path: destination.clone(),
                    identity,
                    fingerprint: fingerprint.clone(),
                    retained: None,
                };
                checkpoint(
                    &mut check,
                    &source,
                    &destination,
                    &directories,
                    finish.is_some(),
                )?;
                if let Some(reviewed) = replacement.take() {
                    replacement::commit_copy(
                        state,
                        id,
                        replacement::PreparedCopy {
                            reviewed,
                            opened: &opened,
                            stage,
                            temporary,
                            change,
                        },
                        cancellation,
                        || {
                            checkpoint(
                                &mut check,
                                &source,
                                &destination,
                                &directories,
                                finish.is_some(),
                            )
                        },
                    )?;
                } else {
                    state.deferred(true, |state| {
                        commit_forward(state, id, change, Some((stage, temporary)), |_| {
                            opened.require_current()?;
                            opened.publish(&destination)?;
                            #[cfg(test)]
                            copy_boundary::boundary("published", &source);
                            Ok(())
                        })
                    })?;
                    super::durability::completed(result.bytes - bytes_before)?;
                }
                #[cfg(test)]
                copy_boundary::after_copy(&source, &destination);
                if request.kind == TransferKind::Move {
                    if interact(TransferEvent::Verifying {
                        source: source.clone(),
                        destination: destination.clone(),
                    }) == TransferReply::Cancel
                    {
                        cancellation.cancel();
                    }
                    check_cancel(cancellation)?;
                    checkpoint(
                        &mut check,
                        &source,
                        &destination,
                        &directories,
                        finish.is_some(),
                    )?;
                    if !identity.matches(&destination)
                        || !fingerprint.matches(&destination, cancellation)?
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "the committed destination is unavailable or changed; the source is retained",
                        ));
                    }
                    checkpoint(
                        &mut check,
                        &source,
                        &destination,
                        &directories,
                        finish.is_some(),
                    )?;
                    let removal =
                        source_removal::Prepared::open(source_pin, &destination, &opened)?;
                    let change = Change::SourceRemoved {
                        originals: removal.originals(),
                        original: source.clone(),
                        current: destination.clone(),
                        identity: source_identity,
                        copy_identity: identity,
                        fingerprint,
                        source_fingerprint: source_fingerprint.clone(),
                        quarantine: removal.quarantine(),
                        reviewed: None,
                    };
                    checkpoint(
                        &mut check,
                        &source,
                        &destination,
                        &directories,
                        finish.is_some(),
                    )?;
                    removal.commit(state, id, &change, &opened, cancellation, || {
                        checkpoint(
                            &mut check,
                            &source,
                            &destination,
                            &directories,
                            finish.is_some(),
                        )
                    })?;
                }
            }
            Ok(true)
        })();
        if operation.is_err() {
            // Failure handling restores or retains data with full syncs.
            super::durability::flush()?;
        }
        match operation {
            Ok(committed) => {
                if let Some(id) = reconciled {
                    result.id = Some(id);
                }
                if committed {
                    result.completed += 1;
                    result.id = Some(reconciled.unwrap_or(id));
                    if destination.parent() == Some(request.destination.as_path())
                        && finish.is_none()
                    {
                        result.destinations.push(destination.clone());
                    }
                }
            }
            Err(error)
                if placement_failure(&error) == Some(TransferAccess::DestinationFolder)
                    || (recovery_label.is_some() && error.kind() == io::ErrorKind::WouldBlock)
                    || (recovery_label.is_some()
                        && placement_failure(&error) == Some(TransferAccess::SourceEntry))
                    || check_location(
                        &mut check,
                        destination.parent().unwrap(),
                        TransferAccess::DestinationFolder,
                    )
                    .and_then(|()| check_pending(state, &mut check))
                    .is_err() =>
            {
                result.needs_attention = true;
                result.attention = Some(error.to_string());
                pending.push_front((source, destination, finish));
                break;
            }
            Err(error)
                if error.kind() == io::ErrorKind::Interrupted
                    && placement_failure(&error).is_none() =>
            {
                if compensate_failed_task(state, id, &destination)? {
                    result.attention = Some("Partial copy data was kept in Recovery.".into());
                }
                if let Some(receipt) = saved_receipts.get(&source).copied()
                    && state.saved_move_completed(&receipt, &source, &destination)?
                {
                    saved_receipts.remove(&source);
                    result.id = Some(receipt.id);
                    if !directories.contains_key(source.parent().unwrap()) {
                        result.completed += 1;
                        if destination.parent() == Some(request.destination.as_path()) {
                            result.destinations.push(destination.clone());
                        }
                    }
                    result.cancelled = true;
                    break;
                }
                let mut rollback_failed = false;
                if request.kind == TransferKind::Move
                    && !copied_receipts.contains_key(&source)
                    && let Err(error) = undo::rollback_copy(state, id, &destination)
                {
                    result.failures.push(TransferFailure {
                        source: source.clone(),
                        destination: destination.clone(),
                        kind: error.kind(),
                        message: format!(
                            "The source is retained; its changed destination also remains: {error}"
                        ),
                    });
                    failed.push((source.clone(), destination.clone(), finish));
                    rollback_failed = true;
                }
                result.cancelled = true;
                if !rollback_failed {
                    pending.push_front((source.clone(), destination.clone(), finish));
                }
                break;
            }
            Err(error) => {
                let retained = compensate_failed_task(state, id, &destination)?;
                if let Some(receipt) = saved_receipts.get(&source).copied()
                    && state.saved_move_completed(&receipt, &source, &destination)?
                {
                    saved_receipts.remove(&source);
                    result.id = Some(receipt.id);
                    if !directories.contains_key(source.parent().unwrap()) {
                        result.completed += 1;
                        if destination.parent() == Some(request.destination.as_path()) {
                            result.destinations.push(destination.clone());
                        }
                    }
                } else {
                    failed.push((source.clone(), destination.clone(), finish));
                    result.failures.push(TransferFailure {
                        source: source.clone(),
                        destination,
                        message: if retained {
                            format!("{error} Partial copy data was kept in Recovery.")
                        } else {
                            error.to_string()
                        },
                        kind: error.kind(),
                    });
                }
            }
        }
        if interact(TransferEvent::Progress {
            path: source,
            completed: result.completed,
            bytes: result.bytes,
        }) == TransferReply::Cancel
        {
            cancellation.cancel();
        }
    }
    result
        .remaining
        .extend(pending.iter().map(|(source, _, _)| source.clone()));
    if !pending.is_empty() {
        result.continuation = Some(TransferContinuation {
            kind: request.kind,
            destination: request.destination.clone(),
            copied: copied_for(
                &pending.iter().cloned().collect::<Vec<_>>(),
                &copied_receipts,
            ),
            pending: pending.into_tasks().collect(),
            directories: directories.clone(),
            saved: saved_receipts.clone(),
            restart: observed.map(ObservedTransfer::restart_observation),
            observed: observed.cloned(),
            destination_observation: Some(destination_observation.clone()),
        });
    }
    result.retry_continuation = retry_continuation(
        request,
        failed,
        &directories,
        &saved_receipts,
        &copied_receipts,
        observed,
        &destination_observation,
    );
    if request.kind == TransferKind::Move {
        // Normal returns refresh the report. A killed worker instead retains
        // its last durable observations without reopening completed paths.
        result.source_remainders.clear();
        let scopes: BTreeSet<_> = request
            .pending
            .iter()
            .map(|(source, _, _)| source)
            .filter(|source| directories.contains_key(*source))
            .cloned()
            .collect();
        for source in &scopes {
            if source
                .ancestors()
                .skip(1)
                .any(|parent| scopes.contains(parent))
            {
                continue;
            }
            let finish = &directories[source].1;
            // This inspects the original storage and recorded folder identity,
            // not permission to execute another step at its current pathname.
            // A new folder after completed removal is not unfinished work.
            let remains = check_location(&mut placement, source, TransferAccess::SourceEntry)
                .is_err()
                || match fs::symlink_metadata(source) {
                    Ok(metadata) => {
                        metadata.is_dir() && Identity::from_metadata(&metadata) == finish.identity
                    }
                    Err(error) => error.kind() != io::ErrorKind::NotFound,
                };
            if remains {
                result.source_remainders.push(source.clone());
            }
        }
    }
    result.cancelled |= cancellation.is_cancelled();
    if state.last_change(id)?.is_some() {
        result.id = Some(id);
    }
    state.discard_empty(id)?;
    Ok(())
}

#[derive(Debug)]
struct PlacementFailure {
    access: TransferAccess,
    error: io::Error,
}
pub(super) fn destination_failure(error: io::Error) -> io::Error {
    if placement_failure(&error).is_some() {
        return error;
    }
    io::Error::new(
        error.kind(),
        PlacementFailure {
            access: TransferAccess::DestinationFolder,
            error,
        },
    )
}
impl std::fmt::Display for PlacementFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}
impl std::error::Error for PlacementFailure {}

fn check_location(
    check: &mut impl FnMut(&Path, TransferAccess) -> io::Result<()>,
    path: &Path,
    access: TransferAccess,
) -> io::Result<()> {
    check(path, access)
        .map_err(|error| io::Error::new(error.kind(), PlacementFailure { access, error }))
}
fn checkpoint(
    check: &mut impl FnMut(&Path, TransferAccess) -> io::Result<()>,
    source: &Path,
    destination: &Path,
    directories: &BTreeMap<PathBuf, (PathBuf, DirectoryFinish)>,
    completion: bool,
) -> io::Result<()> {
    check_location(
        check,
        destination.parent().unwrap(),
        TransferAccess::DestinationFolder,
    )?;
    check_location(check, source, TransferAccess::SourceEntry)?;
    let completed = completion
        && fs::symlink_metadata(source).is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
    for ancestor in source.ancestors() {
        if let Some((target, finish)) = directories.get(ancestor) {
            if let Some(original) = finish.original {
                original.require(ancestor, target, completed)?;
            }
            if !completed {
                let metadata = fs::symlink_metadata(ancestor)?;
                if !metadata.is_dir() || Identity::from_metadata(&metadata) != finish.identity {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "the original source folder changed; its current contents are preserved",
                    ));
                }
            }
            let metadata = fs::symlink_metadata(target)?;
            if !metadata.is_dir()
                || Identity::from_metadata(&metadata) != finish.destination_identity
            {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the original destination folder changed; source data is preserved",
                ));
            }
        }
    }
    Ok(())
}
pub(super) fn placement_failure(error: &io::Error) -> Option<TransferAccess> {
    error
        .get_ref()?
        .downcast_ref::<PlacementFailure>()
        .map(|failure| failure.access)
}

pub(super) fn check_pending(
    state: &Store,
    check: &mut impl FnMut(&Path, TransferAccess) -> io::Result<()>,
) -> io::Result<()> {
    for path in state.reconciliation_paths()? {
        if let Some(parent) = path.parent() {
            check_location(check, parent, TransferAccess::DestinationFolder)?;
        }
    }
    Ok(())
}

pub(super) fn unique_name(path: &Path) -> io::Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("missing destination folder"))?;
    let stem = path
        .file_stem()
        .or_else(|| path.file_name())
        .ok_or_else(|| invalid("missing destination name"))?;
    for number in 2..=100_000 {
        let mut name = stem.to_os_string();
        name.push(format!(" ({number})"));
        if let Some(extension) = path.extension() {
            name.push(".");
            name.push(extension);
        }
        let candidate = parent.join(name);
        match fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(candidate),
            Err(error) => return Err(error),
            _ => {}
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "no available name was found",
    ))
}
