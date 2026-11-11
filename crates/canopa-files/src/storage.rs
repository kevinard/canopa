//! Remember volume placement independently of whether a mount point still exists.
//! Platform observations publish unavailability immediately; canonical paths and
//! persistence are prepared on workers. Requests capture their original placement
//! independently of availability, including across process lifetimes.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const LIMIT: usize = 1024 * 1024;
const IDENTITY_LIMIT: usize = 16 * 1024;

pub struct Storage {
    path: PathBuf,
    state: Mutex<State>,
    writer: Mutex<()>,
}

/// The platform's first complete, prepared observation, or its startup failure.
pub type Ready = Result<Arc<Storage>, String>;

/// A read-only publication from the owning host's storage preparation. Clones
/// capture the same live store without opening files or creating an observer.
#[derive(Clone, Default)]
pub struct PreparedStorage(Arc<Mutex<Option<Arc<Storage>>>>);

impl PreparedStorage {
    pub(crate) fn publish(&self, storage: Arc<Storage>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(storage);
    }

    pub(crate) fn current(&self) -> Option<Arc<Storage>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Memory-only capture. An unprepared managed host never falls back to
    /// unguarded command execution.
    pub(crate) fn capture(&self) -> io::Result<Snapshot> {
        let snapshot = self
            .current()
            .map(|storage| storage.snapshot())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotConnected,
                    fl!("files-storage-not-prepared"),
                )
            })?;
        if snapshot.problem {
            Err(unavailable())
        } else {
            Ok(snapshot)
        }
    }
}
#[derive(Default)]
struct State {
    revision: u64,
    roots: BTreeMap<PathBuf, Root>,
    failed: bool,
}
#[derive(Clone)]
struct Root {
    saved: SavedRoot,
    epoch: u64,
    available: bool,
    identity: Option<Identity>,
    resolved: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedRoot {
    #[serde(with = "crate::path_serde")]
    path: PathBuf,
    #[serde(with = "crate::path_serde")]
    canonical: PathBuf,
    device: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    version: u8,
    roots: Vec<SavedRoot>,
}
#[derive(Clone)]
struct RequiredRoot {
    saved: SavedRoot,
    epoch: Option<u64>,
    resolved: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    device: u64,
    inode: u64,
    mount: u64,
}
impl Identity {
    fn read(path: &Path) -> io::Result<Self> {
        let metadata = fs::metadata(path)?;
        if !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "device root is not a folder",
            ));
        }
        let status = rustix::fs::statx(
            rustix::fs::CWD,
            path,
            rustix::fs::AtFlags::empty(),
            rustix::fs::StatxFlags::MNT_ID,
        )?;
        if status.stx_mask & rustix::fs::StatxFlags::MNT_ID.bits() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "cannot identify the mounted filesystem",
            ));
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mount: status.stx_mnt_id,
        })
    }
}

pub struct Observation {
    path: PathBuf,
    pub epoch: u64,
}

#[derive(Clone)]
pub struct Snapshot {
    storage: Arc<Storage>,
    roots: BTreeMap<PathBuf, Root>,
    problem: bool,
    bindings: Vec<(PathBuf, PathBuf)>,
    required: Option<BTreeMap<PathBuf, RequiredRoot>>,
}

/// Portable original placement. Runtime observation counters and mount handles
/// deliberately do not survive a process restart.
#[derive(Clone, Serialize)]
#[serde(transparent)]
pub struct Requirements(SavedRequirements);

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedRequirements {
    version: u8,
    roots: Vec<SavedRoot>,
    bindings: Vec<Binding>,
    problem: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    #[serde(with = "crate::path_serde")]
    path: PathBuf,
    #[serde(with = "crate::path_serde")]
    resolved: PathBuf,
}

impl<'de> Deserialize<'de> for Requirements {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let saved = SavedRequirements::deserialize(deserializer)?;
        let mut roots = std::collections::BTreeSet::new();
        let mut bindings = std::collections::BTreeSet::new();
        if saved.version != 1
            || saved.roots.len() > 1024
            || saved.bindings.len() > 1024
            || saved.roots.iter().any(|root| {
                !root.path.is_absolute()
                    || !root.canonical.is_absolute()
                    || root.device.len() > IDENTITY_LIMIT
                    || !roots.insert(&root.path)
            })
            || saved.bindings.iter().any(|binding| {
                !binding.path.is_absolute()
                    || !binding.resolved.is_absolute()
                    || !bindings.insert(&binding.path)
            })
        {
            return Err(serde::de::Error::custom(invalid()));
        }
        Ok(Self(saved))
    }
}

/// A folder or referent follows its final symlink; an entry preserves that link.
pub struct Access {
    path: PathBuf,
    follow: bool,
}
impl Access {
    pub fn folder(path: impl Into<PathBuf>) -> Self {
        Self::referent(path)
    }
    pub fn referent(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            follow: true,
        }
    }
    pub fn entry(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            follow: false,
        }
    }
}

impl Storage {
    pub fn load(path: PathBuf) -> io::Result<Arc<Self>> {
        let saved = match fs::symlink_metadata(&path) {
            Ok(_) => crate::state_io::read_json::<Saved>(&path, LIMIT)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Saved {
                version: 1,
                roots: Vec::new(),
            },
            Err(error) => return Err(error),
        };
        if saved.version != 1 || saved.roots.len() > 1024 {
            return Err(invalid());
        }
        let mut state = State::default();
        for saved in saved.roots {
            if !saved.path.is_absolute()
                || !saved.canonical.is_absolute()
                || saved.device.len() > IDENTITY_LIMIT
                || state.roots.contains_key(&saved.path)
            {
                return Err(invalid());
            }
            state.revision += 1;
            state.roots.insert(
                saved.path.clone(),
                Root {
                    saved,
                    epoch: state.revision,
                    available: false,
                    identity: None,
                    resolved: true,
                },
            );
        }
        Ok(Arc::new(Self {
            path,
            state: Mutex::new(state),
            writer: Mutex::new(()),
        }))
    }

    /// Publish unavailability first. Preparation must persist the placement
    /// before the root is admitted for filesystem requests.
    pub fn mounted(&self, path: PathBuf, device: String) -> io::Result<Observation> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        if !path.is_absolute() || device.len() > IDENTITY_LIMIT {
            state.failed = true;
            return Err(invalid());
        }
        if !state.roots.contains_key(&path) && state.roots.len() >= 1024 {
            state.failed = true;
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "the saved storage location budget is full",
            ));
        }
        state.revision = state.revision.checked_add(1).ok_or_else(unavailable)?;
        let epoch = state.revision;
        let canonical = state
            .roots
            .get(&path)
            .map(|root| root.saved.canonical.clone())
            .unwrap_or_else(|| path.clone());
        let resolved = state.roots.get(&path).is_some_and(|root| root.resolved);
        state.roots.insert(
            path.clone(),
            Root {
                saved: SavedRoot {
                    path: path.clone(),
                    canonical,
                    device,
                },
                epoch,
                available: false,
                identity: None,
                resolved,
            },
        );
        Ok(Observation { path, epoch })
    }

    pub fn removed(&self, path: &Path, expected: u64) {
        if let Ok(mut state) = self.state.lock() {
            if state
                .roots
                .get(path)
                .is_none_or(|root| root.epoch != expected)
            {
                return;
            }
            state.revision = state.revision.saturating_add(1);
            let epoch = state.revision;
            if let Some(root) = state.roots.get_mut(path) {
                root.epoch = epoch;
                root.available = false;
                root.identity = None;
            }
        }
    }

    /// Loss of the platform observer invalidates authority for every pending
    /// request, including paths outside its last known set of mount points.
    /// A new application lifetime must establish fresh observations.
    pub fn observer_lost(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.failed = true;
        }
    }

    pub fn prepare(&self, observation: Observation) -> io::Result<()> {
        let canonical = fs::canonicalize(&observation.path)?;
        let identity = Identity::read(&canonical)?;
        let _writer = self.writer.lock().map_err(|_| unavailable())?;
        let saved = {
            let mut state = self.state.lock().map_err(|_| unavailable())?;
            let Some(root) = state
                .roots
                .get_mut(&observation.path)
                .filter(|root| root.epoch == observation.epoch)
            else {
                return Ok(());
            };
            root.saved.canonical = canonical;
            root.resolved = true;
            Saved {
                version: 1,
                roots: state
                    .roots
                    .values()
                    .map(|root| root.saved.clone())
                    .collect(),
            }
        };
        crate::state_io::write_json(&self.path, &saved, LIMIT)?;
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        if let Some(root) = state
            .roots
            .get_mut(&observation.path)
            .filter(|root| root.epoch == observation.epoch)
        {
            root.identity = Some(identity);
            root.available = true;
        }
        Ok(())
    }

    pub fn snapshot(self: &Arc<Self>) -> Snapshot {
        let state = self.state.lock();
        Snapshot {
            storage: self.clone(),
            roots: state
                .as_ref()
                .map(|state| state.roots.clone())
                .unwrap_or_default(),
            problem: state.as_ref().map_or(true, |state| state.failed),
            bindings: Vec::new(),
            required: None,
        }
    }

    /// Capture fresh availability without replacing the request's original
    /// requirements. A device without a stable UUID cannot be proven to be the
    /// original device in this new process, even if an epoch number is reused.
    pub fn restore(self: &Arc<Self>, requirements: &Requirements) -> Snapshot {
        let mut snapshot = self.snapshot();
        snapshot.problem |= requirements.0.problem;
        snapshot.bindings = requirements
            .0
            .bindings
            .iter()
            .map(|binding| (binding.path.clone(), binding.resolved.clone()))
            .collect();
        snapshot.required = Some(
            requirements
                .0
                .roots
                .iter()
                .map(|saved| {
                    (
                        saved.path.clone(),
                        RequiredRoot {
                            saved: saved.clone(),
                            epoch: None,
                            resolved: true,
                        },
                    )
                })
                .collect(),
        );
        snapshot
    }
}

impl Snapshot {
    pub fn bind_folder(&mut self, path: PathBuf, resolved: PathBuf) {
        self.bindings.push((path, resolved));
    }

    pub fn retry_from(&mut self, previous: &Self) {
        self.problem |= previous.problem;
        self.bindings = previous.bindings.clone();
        self.required = Some(previous.original_requirements());
    }

    fn original_requirements(&self) -> BTreeMap<PathBuf, RequiredRoot> {
        self.required.clone().unwrap_or_else(|| {
            self.roots
                .iter()
                .map(|(path, root)| {
                    (
                        path.clone(),
                        RequiredRoot {
                            saved: root.saved.clone(),
                            epoch: Some(root.epoch),
                            resolved: root.resolved,
                        },
                    )
                })
                .collect()
        })
    }

    /// Serialize requirements, never current availability or process-local
    /// identity counters. An unresolved original placement remains unprovable.
    pub fn requirements(&self) -> Requirements {
        let roots = self.original_requirements();
        Requirements(SavedRequirements {
            version: 1,
            problem: self.problem || roots.values().any(|root| !root.resolved),
            roots: roots.into_values().map(|root| root.saved).collect(),
            bindings: self
                .bindings
                .iter()
                .map(|(path, resolved)| Binding {
                    path: path.clone(),
                    resolved: resolved.clone(),
                })
                .collect(),
        })
    }
    /// Worker-only: resolve aliases, then check both placement epochs and actual
    /// kernel mount identity. A new mount never silently authorizes an old job.
    pub fn check(&self, paths: &[Access]) -> io::Result<()> {
        self.check_while(paths, || false)
    }

    pub(crate) fn check_while(
        &self,
        paths: &[Access],
        cancelled: impl Fn() -> bool,
    ) -> io::Result<()> {
        let check_cancelled = || {
            if cancelled() {
                Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    fl!("files-storage-cancelled"),
                ))
            } else {
                Ok(())
            }
        };
        check_cancelled()?;
        if self.problem {
            return Err(unavailable());
        }
        let (revision, mut current) = {
            let state = self.storage.state.lock().map_err(|_| unavailable())?;
            if state.failed {
                return Err(unavailable());
            }
            (state.revision, state.roots.clone())
        };
        let mut expected_roots = self.roots.clone();
        for root in expected_roots.values_mut().chain(current.values_mut()) {
            check_cancelled()?;
            if !root.resolved {
                root.saved.canonical = resolve(&root.saved.path, 0)?;
            }
        }
        for access in paths {
            check_cancelled()?;
            for (path, expected) in &self.bindings {
                if access.path.starts_with(path)
                    && (access.follow || access.path != *path)
                    && resolve(path, 0)? != *expected
                {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        fl!("files-storage-location-changed"),
                    ));
                }
            }
            let resolved = if access.follow {
                resolve(&access.path, 0)?
            } else {
                let parent = access.path.parent().ok_or_else(invalid)?;
                resolve(parent, 0)?.join(access.path.file_name().ok_or_else(invalid)?)
            };
            let expected = containing(&expected_roots, &resolved);
            let actual = containing(&current, &resolved);
            if let Some(required) = &self.required {
                let original = required
                    .values()
                    .filter(|root| resolved.starts_with(&root.saved.canonical))
                    .max_by_key(|root| root.saved.canonical.components().count());
                match (original, actual) {
                    (None, None) => {}
                    (Some(original), Some(now))
                        if original.saved.canonical == now.saved.canonical
                            && original.saved.device == now.saved.device
                            && (original.saved.device.starts_with("uuid:")
                                || original.epoch == Some(now.epoch)) => {}
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::NotConnected,
                            fl!("files-storage-not-returned"),
                        ));
                    }
                }
            }
            match (expected, actual) {
                (None, None) => {}
                (Some(before), Some(now))
                    if before.saved.path == now.saved.path
                        && before.saved.device == now.saved.device
                        && before.epoch == now.epoch =>
                {
                    if !now.available
                        || now.identity.is_none_or(|identity| {
                            Identity::read(&now.saved.canonical).ok() != Some(identity)
                        })
                    {
                        return Err(unavailable());
                    }
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        fl!("files-storage-device-changed"),
                    ));
                }
            }
        }
        check_cancelled()?;
        if self
            .storage
            .state
            .lock()
            .map_err(|_| unavailable())?
            .revision
            != revision
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                fl!("files-storage-changed"),
            ));
        }
        Ok(())
    }
}

fn containing<'a>(roots: &'a BTreeMap<PathBuf, Root>, path: &Path) -> Option<&'a Root> {
    roots
        .values()
        .filter(|root| path.starts_with(&root.saved.canonical))
        .max_by_key(|root| root.saved.canonical.components().count())
}

// Canonicalize the existing prefix, retaining missing names. Broken directory
// aliases must still resolve to their intended (currently absent) volume root.
fn resolve(path: &Path, depth: usize) -> io::Result<PathBuf> {
    if depth > 64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "too many path aliases",
        ));
    }
    match fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_symlink()) {
                let target = fs::read_link(path)?;
                return resolve(
                    &if target.is_absolute() {
                        target
                    } else {
                        path.parent().ok_or_else(invalid)?.join(target)
                    },
                    depth + 1,
                );
            }
            let parent = path
                .parent()
                .filter(|parent| *parent != path)
                .ok_or(error)?;
            Ok(resolve(parent, depth + 1)?.join(path.file_name().ok_or_else(invalid)?))
        }
        Err(error) => Err(error),
    }
}

fn unavailable() -> io::Error {
    io::Error::new(
        io::ErrorKind::NotConnected,
        fl!("files-storage-device-unavailable"),
    )
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid saved storage locations",
    )
}
