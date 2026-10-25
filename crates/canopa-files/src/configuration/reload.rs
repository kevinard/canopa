//! Serialized, debounced source preparation. Call on a worker; no toolkit state.
use super::*;
use std::{os::unix::fs::MetadataExt, path::Path};

#[derive(Clone, Debug)]
pub enum ReloadEvent {
    Unchanged,
    Updated(Box<ConfigurationSnapshot>),
    Rejected(Diagnostic),
}

pub struct ConfigurationRuntime {
    request: Request,
    current: ConfigurationSnapshot,
    previous: Option<Observation>,
    attempted: Option<Observation>,
    last_error: Option<Diagnostic>,
    generation: u64,
    force_publish: bool,
}
impl ConfigurationRuntime {
    pub fn new(request: Request) -> Result<Self, Diagnostic> {
        let current = ConfigurationSnapshot::load(&request)?;
        Ok(Self::from_snapshot(current))
    }
    pub fn from_snapshot(current: ConfigurationSnapshot) -> Self {
        Self {
            request: current.request.clone(),
            current,
            previous: None,
            attempted: None,
            last_error: None,
            generation: 1,
            force_publish: false,
        }
    }
    pub fn current(&self) -> &ConfigurationSnapshot {
        &self.current
    }
    pub fn is_watching(&self) -> bool {
        self.request != Request::Defaults
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn last_error(&self) -> Option<&Diagnostic> {
        self.last_error.as_ref()
    }
    /// Recheck after a caller installs a separate candidate while polling was in flight.
    pub fn invalidate(&mut self) {
        self.previous = None;
        self.attempted = None;
        self.force_publish = true;
    }
    /// Two equal source observations admit one complete candidate. No source edit
    /// is retried repeatedly; missing dependencies remain observable when created.
    pub fn poll(&mut self) -> ReloadEvent {
        if self.request == Request::Defaults {
            return ReloadEvent::Unchanged;
        }
        let observed = observe(&self.request, self.previous.as_ref());
        if self.previous.as_ref() != Some(&observed) {
            self.previous = Some(observed);
            return ReloadEvent::Unchanged;
        }
        if self.attempted.as_ref() == Some(&observed) {
            return ReloadEvent::Unchanged;
        }
        let candidate = ConfigurationSnapshot::load(&self.request);
        let after = observe(&self.request, Some(&observed));
        if after != observed {
            self.previous = Some(after);
            return ReloadEvent::Unchanged;
        }
        self.attempted = Some(observed);
        match candidate {
            Ok(candidate) => {
                let changed = !candidate.same_source(&self.current)
                    || !candidate.same_files(&self.current)
                    || candidate.appearance != self.current.appearance;
                let recovered = self.last_error.take().is_some();
                let publish = std::mem::take(&mut self.force_publish);
                if !changed && !recovered && !publish {
                    return ReloadEvent::Unchanged;
                }
                self.current = candidate.clone();
                self.generation = self.generation.saturating_add(1);
                ReloadEvent::Updated(Box::new(candidate))
            }
            Err(error) => {
                self.last_error = Some(error.clone());
                ReloadEvent::Rejected(error)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Observation {
    selected: Result<Request, Diagnostic>,
    metadata: Option<Result<Stamp, io::ErrorKind>>,
    theme: Option<PathBuf>,
    theme_metadata: Option<Result<Stamp, io::ErrorKind>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    target: PathBuf,
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
fn stamp(path: &Path) -> Result<Stamp, io::ErrorKind> {
    let target = fs::canonicalize(path).map_err(|error| error.kind())?;
    fs::metadata(path)
        .map(|metadata| Stamp {
            target,
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
        .map_err(|error| error.kind())
}
fn observe(request: &Request, previous: Option<&Observation>) -> Observation {
    let selected = request.resolve(Component::Files);
    let metadata = match &selected {
        Ok(Request::File { path, .. }) => Some(stamp(path)),
        _ => None,
    };
    let theme = if let Some(previous) =
        previous.filter(|previous| previous.selected == selected && previous.metadata == metadata)
    {
        previous.theme.clone()
    } else if let Ok(Request::File {
        path,
        format: Format::CanopaV1,
    }) = &selected
    {
        canopa_config::read(path)
            .ok()
            .and_then(|text| canopa_config::theme_path(&text, Some(path)).ok().flatten())
    } else {
        None
    };
    let theme_metadata = theme.as_deref().map(stamp);
    Observation {
        selected,
        metadata,
        theme,
        theme_metadata,
    }
}
