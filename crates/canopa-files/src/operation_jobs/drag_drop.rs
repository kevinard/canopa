//! A drag owns its original selection; hover plans never grant replay authority.
use super::*;
use crate::{file_clipboard::Selection, transfers::DropPreference};
use std::sync::atomic::AtomicU8;

const READY: u8 = 0;
const SUBMITTED: u8 = 1;
const CANCELLED: u8 = 2;
const MAX_DESTINATIONS: usize = 256;

/// Native source capabilities. A forced action is never silently substituted.
#[derive(Clone, Copy, Debug)]
pub struct DropActions {
    pub copy: bool,
    pub move_files: bool,
}

/// A browser can supply its original directory binding. Other targets are
/// resolved on the metadata worker when their hover is prepared.
#[derive(Clone, Debug)]
pub struct DropTarget {
    pub directory: PathBuf,
    pub binding: Option<Source>,
}

struct Drag {
    selection: PreparedSelection,
    paths: Arc<[PathBuf]>,
    cancellation: Cancellation,
    state: AtomicU8,
    work: Mutex<Option<Work>>,
    destinations: Mutex<BTreeMap<PathBuf, DestinationCapture>>,
}

/// One successful observation per pathname for the whole gesture. View refreshes
/// may prepare another hover, but cannot replace its original storage or pin.
#[derive(Clone)]
struct DestinationCapture {
    kind: TransferKind,
    placement: storage::Snapshot,
    observed: DestinationProof,
}

/// Process-owned source observation and retained-data lease for one gesture.
/// Clones share cancellation and single-use admission, including across windows.
#[derive(Clone)]
pub struct PreparedDrag(Arc<Drag>);

impl std::fmt::Debug for PreparedDrag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PreparedDrag").field(&self.paths()).finish()
    }
}

#[derive(Clone)]
pub struct PreparedDrop(Arc<Destination>);
struct Destination {
    drag: PreparedDrag,
    request: TransferRequest,
    placement: storage::Snapshot,
    cancellation: Cancellation,
    observed: DestinationProof,
}

/// Memory-only directory identity, sharing the bounded selected-entry pins.
/// It preserves the folder object, without freezing changes inside that folder.
#[derive(Clone)]
pub(super) struct DestinationProof {
    directory: PathBuf,
    resolved: PathBuf,
    observed: crate::transfers::ObservedSources,
}
impl DestinationProof {
    fn capture(directory: &std::path::Path) -> io::Result<Self> {
        let resolved = std::fs::canonicalize(directory)?;
        let observed = crate::transfers::ObservedSources::capture(std::slice::from_ref(&resolved))?;
        observed.require_directory(&resolved)?;
        Ok(Self {
            directory: directory.to_owned(),
            resolved,
            observed,
        })
    }

    pub(super) fn check(&self) -> io::Result<()> {
        if std::fs::canonicalize(&self.directory)? != self.resolved {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                fl!("files-drop-destination-changed"),
            ));
        }
        self.observed
            .require_directory(&self.resolved)
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    fl!(
                        "files-drop-destination-unavailable",
                        error = error.to_string()
                    ),
                )
            })
    }
}
impl std::fmt::Debug for PreparedDrop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PreparedDrop").field(self.request()).finish()
    }
}

/// An action chosen by backend negotiation for this exact prepared destination.
/// This is deliberately neither serializable nor constructible from a saved row.
#[derive(Clone, Debug)]
pub struct DropChoice {
    target: PreparedDrop,
    kind: TransferKind,
}
impl DropChoice {
    pub fn kind(&self) -> TransferKind {
        self.kind
    }

    pub(super) fn cancelled(&self) -> bool {
        self.target.0.drag.ended() || self.target.0.cancellation.is_cancelled()
    }

    pub(super) fn claim(&self, runtime: &Arc<()>) -> io::Result<PreparedTransfer> {
        let target = &self.target.0;
        check_cancel(&target.cancellation)?;
        target.drag.available(runtime)?;
        target
            .drag
            .0
            .state
            .compare_exchange(READY, SUBMITTED, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| io::Error::other(fl!("files-drag-ended")))?;
        // Admission runs on the worker. Keep the chosen proof below while
        // releasing observations of other hovered folders and their pins.
        target.drag.0.destinations.lock().unwrap().clear();
        // The submission's accepted-work hold now owns admission and execution.
        target
            .drag
            .0
            .work
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let paste = target.drag.0.selection.claim_drag(runtime, self.kind)?;
        let observed = paste.observe(target.request.destination.clone())?;
        Ok(PreparedTransfer {
            clipboard: paste,
            observed,
            destination: Some(target.placement.clone()),
            destination_observation: Some(target.observed.clone()),
        })
    }
}

impl PreparedDrag {
    pub fn paths(&self) -> &[PathBuf] {
        &self.0.paths
    }

    /// Cancels the gesture and every existing hover plan. An admitted transfer
    /// has its own queue cancellation and is never undone by a source callback.
    pub fn cancel(&self) -> bool {
        if self
            .0
            .state
            .compare_exchange(READY, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        self.0.cancellation.cancel();
        self.0.work.lock().unwrap_or_else(|e| e.into_inner()).take();
        true
    }

    fn available(&self, runtime: &Arc<()>) -> io::Result<()> {
        self.0.selection.check_runtime(runtime)?;
        if self.ended() {
            return Err(io::Error::other(fl!("files-drag-ended")));
        }
        Ok(())
    }

    pub(super) fn ended(&self) -> bool {
        self.0.state.load(Ordering::Acquire) != READY || self.0.cancellation.is_cancelled()
    }

    pub(super) fn prepare(
        engine: &OperationEngine,
        selection: Selection,
        placement: storage::Snapshot,
        runtime: &Arc<()>,
        cancellation: Cancellation,
        work: Work,
    ) -> io::Result<Self> {
        check_cancel(&cancellation)?;
        let paths = selection.paths().to_vec().into();
        let selection = PreparedSelection::prepare(engine, selection, placement, runtime, None)?;
        check_cancel(&cancellation)?;
        Ok(Self(Arc::new(Drag {
            selection,
            paths,
            cancellation,
            state: AtomicU8::new(READY),
            work: Mutex::new(Some(work)),
            destinations: Mutex::new(BTreeMap::new()),
        })))
    }

    fn destination(
        &self,
        target: &DropTarget,
        placement: storage::Snapshot,
        cancellation: &Cancellation,
    ) -> io::Result<DestinationCapture> {
        {
            let destinations = self.0.destinations.lock().unwrap();
            if let Some(captured) = destinations.get(&target.directory) {
                return Ok(captured.clone());
            }
            if destinations.len() >= MAX_DESTINATIONS {
                return Err(io::Error::other(fl!("files-drag-too-many-folders")));
            }
        }
        // Never hold the memory lock across filesystem inspection: cancellation
        // and other callers must not wait on a slow destination.
        let captured = DestinationCapture::prepare(self, target, placement, cancellation)?;
        let mut destinations = self.0.destinations.lock().unwrap();
        if let Some(original) = destinations.get(&target.directory) {
            return Ok(original.clone());
        }
        if destinations.len() >= MAX_DESTINATIONS {
            return Err(io::Error::other(fl!("files-drag-too-many-folders")));
        }
        destinations.insert(target.directory.clone(), captured.clone());
        Ok(captured)
    }
}

impl PreparedDrop {
    /// Descriptive data for hover feedback. It cannot submit or reconstruct work.
    pub fn request(&self) -> &TransferRequest {
        &self.0.request
    }

    /// Memory-only negotiation, suitable for modifier changes without motion.
    pub fn choose(&self, preference: DropPreference, offered: DropActions) -> Option<DropChoice> {
        if self.0.drag.ended() || self.0.cancellation.is_cancelled() {
            return None;
        }
        let preferred = match preference {
            DropPreference::Automatic => self.request().kind,
            DropPreference::Copy => TransferKind::Copy,
            DropPreference::Move => TransferKind::Move,
        };
        let kind = match preferred {
            TransferKind::Copy if offered.copy => TransferKind::Copy,
            TransferKind::Move if offered.move_files => TransferKind::Move,
            _ if preference == DropPreference::Automatic && offered.copy => TransferKind::Copy,
            _ => return None,
        };
        Some(DropChoice {
            target: self.clone(),
            kind,
        })
    }

    pub(super) fn prepare(
        drag: PreparedDrag,
        target: DropTarget,
        placement: storage::Snapshot,
        runtime: &Arc<()>,
        cancellation: &Cancellation,
    ) -> io::Result<Self> {
        drag.available(runtime)?;
        check_cancel(cancellation)?;
        if !target.directory.is_absolute()
            || target.binding.as_ref().is_some_and(|binding| {
                !binding.directory.is_absolute() || !binding.resolved.is_absolute()
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                fl!("files-drop-binding-invalid"),
            ));
        }
        if target
            .binding
            .as_ref()
            .is_some_and(|binding| !target.directory.starts_with(&binding.directory))
        {
            return Err(io::Error::other(fl!("files-drop-outside-folder")));
        }
        let captured = drag.destination(&target, placement, cancellation)?;
        captured
            .placement
            .check(&[Access::folder(&target.directory)])?;
        captured.observed.check()?;
        drag.available(runtime)?;
        check_cancel(cancellation)?;
        let request = TransferRequest {
            kind: captured.kind,
            sources: drag.paths().to_vec(),
            destination: target.directory,
        };
        Ok(Self(Arc::new(Destination {
            drag,
            request,
            placement: captured.placement,
            cancellation: cancellation.clone(),
            observed: captured.observed,
        })))
    }
}

impl DestinationCapture {
    fn prepare(
        drag: &PreparedDrag,
        target: &DropTarget,
        mut placement: storage::Snapshot,
        cancellation: &Cancellation,
    ) -> io::Result<Self> {
        let binding = match &target.binding {
            Some(binding) => binding.clone(),
            None => Source {
                resolved: std::fs::canonicalize(&target.directory)?,
                directory: target.directory.clone(),
            },
        };
        placement.bind_folder(binding.directory, binding.resolved);
        placement.check(&[Access::folder(&target.directory)])?;
        let request = TransferRequest::from_drop(
            drag.paths().to_vec(),
            target.directory.clone(),
            DropPreference::Automatic,
            cancellation,
        )?;
        let observed = DestinationProof::capture(&request.destination)?;
        Ok(Self {
            kind: request.kind,
            placement,
            observed,
        })
    }
}

fn check_cancel(cancellation: &Cancellation) -> io::Result<()> {
    if cancellation.is_cancelled() {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            fl!("files-drag-cancelled"),
        ))
    } else {
        Ok(())
    }
}

impl OperationJobs {
    /// Captures current storage in memory, then prepares original source entries
    /// and retained data on the queue worker. No caller performs filesystem I/O.
    pub fn prepare_drag(
        &self,
        paths: Vec<PathBuf>,
        cancellation: Cancellation,
    ) -> io::Result<oneshot::Receiver<Result<PreparedDrag, String>>> {
        let selection = Selection::new(paths, TransferKind::Move)?;
        let placement = self.0.storage.capture()?;
        let (send, receive) = oneshot::channel();
        let work = self.0.activity.work();
        self.send_work(Control::PrepareDrag(
            selection,
            placement,
            cancellation,
            work,
            send,
        ))
        .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))?;
        Ok(receive)
    }

    /// Prepares a hover destination off the caller, preserving its original
    /// storage observation and optional browser alias binding until release.
    /// Re-preparing the same pathname within one drag reuses its first successful
    /// observation, including after hover cancellation or a frontend refresh.
    pub fn prepare_drop(
        &self,
        drag: PreparedDrag,
        target: DropTarget,
        cancellation: Cancellation,
    ) -> io::Result<oneshot::Receiver<Result<PreparedDrop, String>>> {
        let placement = self.0.storage.capture()?;
        let (send, receive) = oneshot::channel();
        self.send_work(Control::PrepareDrop(
            drag,
            target,
            placement,
            cancellation,
            send,
        ))
        .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))?;
        Ok(receive)
    }

    /// Accepts one negotiated release into the ordinary queue, with its existing
    /// conflicts, cancellation, notifications, durable result saving and Undo.
    pub fn submit_drop(
        &self,
        choice: DropChoice,
    ) -> io::Result<oneshot::Receiver<Result<u64, String>>> {
        let (send, receive) = oneshot::channel();
        self.send_work(Control::Drop(choice, send))
            .map_err(|_| io::Error::other(fl!("files-jobs-queue-full")))?;
        Ok(receive)
    }
}
