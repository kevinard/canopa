//! A name draft holds original source authority until atomic queue admission.
use super::*;
use crate::transfers::ObservedSources;
use std::{ffi::OsString, path::Component};

/// A single-use, process-owned Rename selection. Clones share its consumption;
/// stored paths never recreate this capability after restart.
#[derive(Clone)]
pub struct PreparedRename(Arc<Preparation>);

struct Preparation {
    runtime: Arc<()>,
    ready: Mutex<Option<Ready>>,
}

pub(super) struct Ready {
    source: PathBuf,
    binding: Option<Source>,
    pub observed: ObservedSources,
    pub lease: RetainedSourceLease,
    pub placement: storage::Snapshot,
}

impl std::fmt::Debug for PreparedRename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedRename").finish_non_exhaustive()
    }
}

impl PreparedRename {
    pub(super) fn prepare(
        engine: &OperationEngine,
        runtime: &Arc<()>,
        source: PathBuf,
        binding: Option<Source>,
        placement: storage::Snapshot,
    ) -> io::Result<Self> {
        if !source.is_absolute()
            || source.file_name().is_none()
            || binding
                .as_ref()
                .is_some_and(|b| !b.directory.is_absolute() || !b.resolved.is_absolute())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                fl!("files-rename-needs-entry"),
            ));
        }
        // Renaming a name draft stays in this same parent. Final links are
        // observed as entries; parent aliases follow the original directory.
        let observed = ObservedSources::capture_rename(&source, &source)?;
        observed.require(&source)?;
        let retained = engine.capture_retained_sources(std::slice::from_ref(&source))?;
        let lease = engine.lease_retained_sources(&retained)?;
        Ok(Self(Arc::new(Preparation {
            runtime: runtime.clone(),
            ready: Mutex::new(Some(Ready {
                source,
                binding,
                observed,
                lease,
                placement,
            })),
        })))
    }
}

impl OperationJobs {
    /// Capture source, parent and retained-data ownership off the caller thread,
    /// before presenting a Rename draft.
    pub fn prepare_rename(
        &self,
        source: PathBuf,
        binding: Option<Source>,
    ) -> io::Result<oneshot::Receiver<Result<PreparedRename, String>>> {
        let (reply, receive) = oneshot::channel();
        self.send_work(Control::PrepareRename(source, binding, reply))?;
        Ok(receive)
    }

    /// Atomically consume a prepared selection into durable queue ownership.
    /// Refusal preserves the same capability for correction or cancellation.
    pub fn submit_rename(
        &self,
        prepared: PreparedRename,
        name: OsString,
    ) -> io::Result<oneshot::Receiver<Result<u64, String>>> {
        let (reply, receive) = oneshot::channel();
        self.send_work(Control::Rename(prepared, name, reply))?;
        Ok(receive)
    }
}

impl Worker {
    pub(super) fn admit_rename(
        &mut self,
        prepared: PreparedRename,
        name: OsString,
    ) -> io::Result<u64> {
        if !Arc::ptr_eq(&prepared.0.runtime, &self.clipboard_owner) {
            return Err(io::Error::other(fl!("files-rename-other-runtime")));
        }
        let mut state = prepared.0.ready.lock().unwrap_or_else(|e| e.into_inner());
        let ready = state
            .as_ref()
            .ok_or_else(|| io::Error::other(fl!("files-rename-accepted")))?;
        let mut components = std::path::Path::new(&name).components();
        if !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
            || name.as_encoded_bytes().contains(&0)
            || name.as_encoded_bytes().contains(&b'/')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                fl!("files-name-invalid"),
            ));
        }
        ready.observed.require(&ready.source)?;
        let destination = ready.source.parent().unwrap().join(name);
        let id = self.admit_observed(
            Request::Mutation(OperationRequest::Rename {
                source: ready.source.clone(),
                destination,
            }),
            ready.binding.clone(),
            None,
            None,
            Some(ready),
        )?;
        // Only a committed admission consumes the live capability. Its now empty
        // lease retires normally; the accepted request already owns protection.
        state.take();
        Ok(id)
    }
}
