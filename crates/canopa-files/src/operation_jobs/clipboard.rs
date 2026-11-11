//! Prepared clipboard intent owns source observations and retention together.
use super::*;
use crate::{file_clipboard::Selection, transfers::ObservedSources};
use std::collections::BTreeSet;

/// A process-owned Copy/Cut selection, prepared off the caller's thread. Its
/// sources are never observed again at Paste. It grants no saved replay rights.
#[derive(Clone)]
pub struct PreparedSelection(Arc<Offer>);

struct Offer {
    runtime: Arc<()>,
    placement: storage::Snapshot,
    observed: ObservedSources,
    lease: RetainedSourceLease,
    state: Mutex<ClipboardSnapshot>,
    claims: std::sync::atomic::AtomicU64,
}

impl std::fmt::Debug for PreparedSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PreparedSelection")
            .field(&self.snapshot())
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct ClipboardSnapshot {
    pub selection: Option<Selection>,
    pub revision: u64,
    pub busy: bool,
}

pub(super) struct ClipboardPaste {
    prepared: PreparedSelection,
    selection: Selection,
    revision: u64,
    claim: u64,
}

impl PreparedSelection {
    pub(super) fn prepare(
        engine: &OperationEngine,
        selection: Selection,
        placement: storage::Snapshot,
        runtime: &Arc<()>,
        previous: Option<&Self>,
    ) -> io::Result<Self> {
        let observed = match previous {
            Some(previous) => {
                if !Arc::ptr_eq(runtime, &previous.0.runtime) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        fl!("files-clipboard-other-runtime"),
                    ));
                }
                ObservedSources::capture_replacing(selection.paths(), &previous.0.observed)?
            }
            None => ObservedSources::capture(selection.paths())?,
        };
        let retained = engine.capture_retained_sources(selection.paths())?;
        let lease = engine.lease_retained_sources(&retained)?;
        Ok(Self(Arc::new(Offer {
            runtime: runtime.clone(),
            placement,
            observed,
            lease,
            claims: std::sync::atomic::AtomicU64::new(0),
            state: Mutex::new(ClipboardSnapshot {
                selection: Some(selection),
                revision: 0,
                busy: false,
            }),
        })))
    }

    pub fn snapshot(&self) -> ClipboardSnapshot {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub(super) fn check_runtime(&self, runtime: &Arc<()>) -> io::Result<()> {
        if !Arc::ptr_eq(runtime, &self.0.runtime) {
            return Err(io::Error::other(fl!("files-clipboard-other-history")));
        }
        Ok(())
    }

    pub(super) fn claim(&self, runtime: &Arc<()>) -> io::Result<ClipboardPaste> {
        self.claim_kind(runtime, None)
    }

    pub(super) fn claim_drag(
        &self,
        runtime: &Arc<()>,
        kind: TransferKind,
    ) -> io::Result<ClipboardPaste> {
        self.claim_kind(runtime, Some(kind))
    }

    fn claim_kind(
        &self,
        runtime: &Arc<()>,
        kind: Option<TransferKind>,
    ) -> io::Result<ClipboardPaste> {
        self.check_runtime(runtime)?;
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.busy {
            return Err(io::Error::other(fl!("files-clipboard-cut-pasting")));
        }
        let selection = state
            .selection
            .clone()
            .ok_or_else(|| io::Error::other(fl!("files-clipboard-cut-finished")))?;
        let selection = if let Some(kind) = kind {
            Selection::new(selection.paths().to_vec(), kind)?
        } else {
            selection
        };
        state.busy = selection.kind() == TransferKind::Move;
        let claim = if state.busy {
            self.0.claims.fetch_add(1, Ordering::AcqRel) + 1
        } else {
            self.0.claims.load(Ordering::Acquire)
        };
        Ok(ClipboardPaste {
            prepared: self.clone(),
            selection,
            revision: state.revision,
            claim,
        })
    }
}

impl ClipboardPaste {
    pub(super) fn observe(&self, destination: PathBuf) -> io::Result<ObservedTransfer> {
        self.prepared.0.observed.transfer(TransferRequest {
            sources: self.selection.paths().to_vec(),
            kind: self.selection.kind(),
            destination,
        })
    }

    pub(super) fn placement(&self) -> storage::Snapshot {
        self.prepared.0.placement.clone()
    }

    pub(super) fn lease(&self) -> &RetainedSourceLease {
        &self.prepared.0.lease
    }

    /// Called only after saving the actual result. An unsaved result keeps its
    /// Cut claim; retrying that save performs no new filesystem work.
    pub(super) fn complete(
        self,
        id: u64,
        report: Option<&TransferReport>,
        pending: Option<&super::transfer_resume::Pending>,
    ) -> Option<CutProgress> {
        let report = report.filter(|report| {
            self.selection.kind() == TransferKind::Move && report.kind == TransferKind::Move
        })?;
        let requested: BTreeSet<_> = self.selection.paths().iter().cloned().collect();
        let mut keep = BTreeSet::new();
        for path in report
            .failures
            .iter()
            .map(|failure| &failure.source)
            .chain(&report.skipped)
            .chain(&report.remaining)
            .chain(&report.source_remainders)
        {
            for ancestor in path.ancestors() {
                if requested.contains(ancestor) {
                    keep.insert(ancestor.to_owned());
                }
            }
        }
        let remaining = self.selection.retaining(&keep);
        let mut state = self
            .prepared
            .0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !report.needs_attention
            && state.revision == self.revision
            && self.prepared.0.claims.load(Ordering::Acquire) == self.claim
            && remaining.as_ref().map(Selection::paths)
                != state.selection.as_ref().map(Selection::paths)
        {
            state.selection = remaining;
            state.revision += 1;
        }
        pending.map(|pending| {
            CutProgress(Arc::new(Mutex::new(CutState {
                prepared: Arc::downgrade(&self.prepared.0),
                selection: self.selection.clone(),
                revision: state.revision,
                claim: self.claim,
                slots: [(id, CutSlot::outcome(&self.selection, pending, report))].into(),
            })))
        })
    }
}

impl Drop for ClipboardPaste {
    fn drop(&mut self) {
        if self.selection.kind() == TransferKind::Move {
            self.prepared
                .0
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .busy = false;
        }
    }
}

impl Job {
    pub(super) fn complete_clipboard(&mut self) {
        let report = match &self.saved {
            Saved::Completed { transfer, .. } => transfer.as_ref(),
            Saved::TransferPending { outcome, .. } => Some(&outcome.report),
            _ => None,
        };
        if let Some(paste) = self.clipboard.take() {
            self.cut_progress =
                paste.complete(self.record.id(), report, self.transfer_pending.as_ref());
        } else if let Some(cut) = &self.cut_progress
            && let Some(report) = report
        {
            cut.settle(self.record.id(), self.transfer_pending.as_ref(), report);
        }
        self.cut_claim = None;
        if self.transfer_pending.is_none() && !self.saved.uncertain() {
            self.cut_progress = None;
        }
    }
}

/// Shared outstanding Cut roots. It contains only descriptive paths and a weak
/// clipboard link; execution pins belong to the separate live plans.
#[derive(Clone)]
pub(super) struct CutProgress(Arc<Mutex<CutState>>);

struct CutState {
    prepared: Weak<Offer>,
    selection: Selection,
    revision: u64,
    claim: u64,
    slots: BTreeMap<u64, CutSlot>,
}

#[derive(Default)]
struct CutSlot {
    failed: BTreeSet<PathBuf>,
    remaining: BTreeSet<PathBuf>,
    other: BTreeSet<PathBuf>,
}

fn roots<'a>(selection: &Selection, paths: impl Iterator<Item = &'a PathBuf>) -> BTreeSet<PathBuf> {
    paths
        .flat_map(|path| {
            selection
                .paths()
                .iter()
                .filter(move |root| path.starts_with(root))
                .cloned()
        })
        .collect()
}

impl CutSlot {
    fn outcome(
        selection: &Selection,
        pending: &super::transfer_resume::Pending,
        report: &TransferReport,
    ) -> Self {
        let failed = roots(
            selection,
            pending
                .retry
                .iter()
                .flat_map(crate::transfers::TransferContinuation::source_paths),
        );
        let remaining = roots(
            selection,
            pending
                .remaining
                .iter()
                .flat_map(crate::transfers::TransferContinuation::source_paths),
        );
        // Original folders still containing unowned or skipped children must
        // remain in Cut even after all executable parts have completed.
        let mut other = roots(selection, report.skipped.iter());
        other.extend(
            roots(selection, report.source_remainders.iter())
                .into_iter()
                .filter(|root| !failed.contains(root) && !remaining.contains(root)),
        );
        Self {
            failed,
            remaining,
            other,
        }
    }
    fn paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.failed.iter().chain(&self.remaining).chain(&self.other)
    }
}

pub(super) struct CutClaim(PreparedSelection);
impl Drop for CutClaim {
    fn drop(&mut self) {
        self.0
            .0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .busy = false;
    }
}

impl CutProgress {
    pub fn claim(&self) -> io::Result<Option<CutClaim>> {
        let cut = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let Some(prepared) = cut.prepared.upgrade() else {
            return Ok(None);
        };
        let mut state = prepared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.revision != cut.revision || prepared.claims.load(Ordering::Acquire) != cut.claim {
            return Ok(None);
        }
        if state.busy {
            return Err(io::Error::other(fl!(
                "files-clipboard-cut-pasting-continue"
            )));
        }
        state.busy = true;
        drop(state);
        Ok(Some(CutClaim(PreparedSelection(prepared))))
    }

    pub fn handoff(
        &self,
        id: u64,
        next: u64,
        part: TransferPart,
        plan: &crate::transfers::TransferContinuation,
    ) {
        let mut cut = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let selected = roots(&cut.selection, plan.source_paths());
        if let Some(old) = cut.slots.get_mut(&id) {
            match part {
                TransferPart::Failed => old.failed.clear(),
                TransferPart::Remaining => old.remaining.clear(),
            }
        }
        cut.slots.insert(
            next,
            CutSlot {
                remaining: selected,
                ..Default::default()
            },
        );
    }

    pub fn settle(
        &self,
        id: u64,
        pending: Option<&super::transfer_resume::Pending>,
        report: &TransferReport,
    ) {
        let mut cut = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let empty = super::transfer_resume::Pending {
            unavailable: false,
            retry: None,
            remaining: None,
        };
        let mut slot = CutSlot::outcome(&cut.selection, pending.unwrap_or(&empty), report);
        let skipped = roots(&cut.selection, report.skipped.iter());
        let owned_elsewhere: BTreeSet<_> = cut
            .slots
            .iter()
            .filter(|(owner, _)| **owner != id)
            .flat_map(|(_, slot)| slot.failed.iter().chain(&slot.remaining))
            .cloned()
            .collect();
        // Another executable part can temporarily keep the shared original
        // folder nonempty. Its own eventual outcome will report any truly
        // unowned children. Explicitly skipped children remain permanent roots.
        slot.other
            .retain(|root| skipped.contains(root) || !owned_elsewhere.contains(root));
        cut.slots.insert(id, slot);
        if report.needs_attention {
            return;
        }
        let Some(prepared) = cut.prepared.upgrade() else {
            return;
        };
        let mut state = prepared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.revision != cut.revision || prepared.claims.load(Ordering::Acquire) != cut.claim {
            return;
        }
        let keep = cut
            .slots
            .values()
            .flat_map(CutSlot::paths)
            .cloned()
            .collect();
        let selection = cut.selection.retaining(&keep);
        if selection.as_ref().map(Selection::paths)
            != state.selection.as_ref().map(Selection::paths)
        {
            state.selection = selection;
            state.revision += 1;
            cut.revision = state.revision;
        }
    }
}
