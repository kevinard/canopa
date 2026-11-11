use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const MAX_SOURCE_PINS: usize = 4096;
static SOURCE_PINS: AtomicUsize = AtomicUsize::new(0);
const MAX_DESTINATION_PINS: usize = 64;
static DESTINATION_PINS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Default)]
struct Pins(usize);

impl Pins {
    fn acquire(&mut self, selection_limit: usize, process_limit: usize) -> io::Result<()> {
        SOURCE_PINS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(1)
                    .filter(|next| self.0 < selection_limit && *next <= process_limit)
            })
            .map_err(|_| io::Error::other(
                "Too many selected sources are waiting. Wait for other transfers to finish, or select a folder or fewer items.",
            ))?;
        self.0 += 1;
        Ok(())
    }
}

impl Drop for Pins {
    fn drop(&mut self) {
        SOURCE_PINS.fetch_sub(self.0, Ordering::AcqRel);
    }
}

/// A process-local observation of a transfer's selected entries. Holding the
/// opened entries prevents inode reuse from making a replacement look original.
/// This neither freezes contents nor grants permission to replay saved work.
#[derive(Clone, Debug)]
pub struct ObservedTransfer {
    request: TransferRequest,
    sources: ObservedSources,
    pub(super) destination: DestinationObservation,
}

/// One original root folder per transfer intent. Clones and both continuation
/// parts share the opened folder, including while its pathname is unavailable.
#[derive(Clone, Debug)]
pub(super) struct DestinationObservation(Arc<Destination>);

#[derive(Debug)]
struct Destination {
    path: PathBuf,
    opened: Mutex<Option<DestinationPin>>,
}

#[derive(Debug)]
struct DestinationPin {
    restart: super::restart::Evidence<crate::operations::original::Original>,
    _file: fs::File,
    identity: Identity,
    _reservation: DestinationReservation,
}

#[derive(Debug)]
struct DestinationReservation;

impl DestinationReservation {
    fn acquire() -> io::Result<Self> {
        use rustix::process::{Resource, getrlimit};
        // Root folders have their own small allowance; observing a destination
        // must not reduce the existing selected-file or clipboard capacity.
        let limit = getrlimit(Resource::Nofile)
            .current
            .map_or(MAX_DESTINATION_PINS, |soft| {
                (soft / 16).min(MAX_DESTINATION_PINS as u64) as usize
            });
        DESTINATION_PINS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(1).filter(|next| *next <= limit)
            })
            .map_err(|_| io::Error::other(
                "Too many transfer destinations are waiting. Finish other transfers and try again.",
            ))?;
        Ok(Self)
    }
}

impl Drop for DestinationReservation {
    fn drop(&mut self) {
        DESTINATION_PINS.fetch_sub(1, Ordering::AcqRel);
    }
}

impl DestinationObservation {
    fn reopen(path: PathBuf, original: crate::operations::original::Original) -> io::Result<Self> {
        use rustix::fs::{Mode, OFlags, open};
        let reservation = DestinationReservation::acquire()?;
        let file = fs::File::from(open(
            &path,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        original.require(&file)?;
        let identity = Identity::from_metadata(&file.metadata()?);
        let observed = Self(Arc::new(Destination {
            path,
            opened: Mutex::new(Some(DestinationPin {
                restart: super::restart::Evidence::Available(original),
                _file: file,
                identity,
                _reservation: reservation,
            })),
        }));
        observed.check()?;
        Ok(observed)
    }
    pub(super) fn capture(path: PathBuf) -> io::Result<Self> {
        let observation = Self(Arc::new(Destination {
            path,
            opened: Mutex::new(None),
        }));
        // An initially unavailable destination still follows the existing
        // placement/question workflow. Bind its first available folder once.
        if fs::metadata(&observation.0.path).is_ok_and(|metadata| metadata.is_dir()) {
            observation.check()?;
        }
        Ok(observation)
    }

    pub(super) fn restart_original(
        &self,
    ) -> super::restart::Evidence<crate::operations::original::Original> {
        let opened = self
            .0
            .opened
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        opened
            .as_ref()
            .map(|pin| pin.restart.clone())
            .unwrap_or_else(|| {
                super::restart::Evidence::unavailable(
                    "The original destination has not been observed.",
                )
            })
    }

    pub(super) fn path(&self) -> &Path {
        &self.0.path
    }

    fn require_original(&self, opened: &DestinationPin) -> io::Result<()> {
        use rustix::fs::{Mode, OFlags, open};
        let current = fs::File::from(open(
            &self.0.path,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        if Identity::from_metadata(&current.metadata()?) != opened.identity {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "The destination folder has changed. Restore the original folder before continuing.",
            ));
        }
        if let super::restart::Evidence::Available(original) = &opened.restart {
            original.require(&current)?;
        }
        Ok(())
    }

    /// Ask the filesystem itself whether the observed root still exists, past
    /// the kernel's attribute and name caches. A network share or GVFS location
    /// that went away keeps answering from FUSE caches for a while, so a failed
    /// step there must not be mistaken for a per-item failure (ADR0168). Local
    /// filesystems ignore the forced synchronization.
    pub(super) fn confirm_reachable(&self) -> io::Result<()> {
        let opened = self
            .0
            .opened
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(opened) = opened.as_ref() else {
            return Ok(());
        };
        rustix::fs::statx(
            rustix::fs::CWD,
            &self.0.path,
            rustix::fs::AtFlags::STATX_FORCE_SYNC,
            rustix::fs::StatxFlags::BASIC_STATS,
        )
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                format!("The destination folder is no longer reachable: {error}"),
            )
        })?;
        self.require_original(opened)
    }

    pub(super) fn check_if_observed(&self) -> io::Result<()> {
        let opened = self
            .0
            .opened
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        opened
            .as_ref()
            .map_or(Ok(()), |opened| self.require_original(opened))
    }

    pub(super) fn check(&self) -> io::Result<()> {
        let mut opened = self
            .0
            .opened
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if opened.is_none() {
            use rustix::fs::{Mode, OFlags, open};
            let reservation = DestinationReservation::acquire()?;
            // Destination aliases follow their target folder, unlike selected
            // source links. O_PATH never reads the folder or a special file.
            let file = fs::File::from(open(
                &self.0.path,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            let identity = Identity::from_metadata(&file.metadata()?);
            let restart = match crate::operations::original::Original::capture(&file) {
                Ok(original) => super::restart::Evidence::Available(original),
                Err(error) => super::restart::Evidence::unavailable(error),
            };
            *opened = Some(DestinationPin {
                restart,
                _file: file,
                identity,
                _reservation: reservation,
            });
        }
        self.require_original(opened.as_ref().unwrap())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ObservedSources(Arc<Selection>);

#[derive(Debug)]
struct Selection {
    entries: BTreeMap<PathBuf, io::Result<SelectedEntry>>,
    parents: BTreeMap<PathBuf, SelectedEntry>,
    _pins: Pins,
}

#[derive(Debug)]
struct SelectedEntry {
    restart: super::restart::CapturedEntry,
    _file: fs::File,
    identity: Identity,
}

impl TransferRequest {
    /// Capture selected entries and an available destination folder on a worker
    /// before acknowledging admission. Live continuations retain both.
    /// Failed observations remain per-source failures, even if those pathnames
    /// later become readable. Final symlinks are observed without following them;
    /// opening a FIFO or device here must not read it or wait for another process.
    /// Exhausting the shared pin budget refuses the whole admission and releases
    /// partial captures, preserving capacity for already accepted work.
    pub fn observe_sources(&self) -> io::Result<ObservedTransfer> {
        ObservedSources::capture(&self.sources)?.transfer(self.clone())
    }
}

impl ObservedSources {
    /// Read the original pinned facts, never a fresh pathname observation.
    pub(in crate::operations) fn rename_bindings(
        &self,
        source: &Path,
        destination: &Path,
    ) -> io::Result<crate::operations::rename::Bindings> {
        let selected = self
            .0
            .entries
            .get(source)
            .ok_or_else(|| invalid("Rename needs its originally selected entry"))?
            .as_ref()
            .map_err(|error| io::Error::new(error.kind(), error.to_string()))?;
        let parent = |path: &Path| {
            self.0
                .parents
                .get(path.parent().unwrap())
                .map(|entry| entry.identity)
                .ok_or_else(|| invalid("Rename needs its originally observed parent folders"))
        };
        Ok(crate::operations::rename::Bindings {
            source: selected.identity,
            source_parent: parent(source)?,
            destination_parent: parent(destination)?,
        })
    }

    /// A captured directory must remain the original opened directory, including
    /// when its pathname is recycled before queued work starts.
    pub(crate) fn require_directory(&self, path: &Path) -> io::Result<()> {
        self.require(path)?;
        let selected = self
            .0
            .entries
            .get(path)
            .ok_or_else(|| invalid("Work must use its originally observed directory"))?
            .as_ref()
            .map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        if !selected._file.metadata()?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "The drop destination is not a folder.",
            ));
        }
        Ok(())
    }

    /// Require the exact selected entry, retaining any original read failure.
    /// The open pin prevents an unlinked inode from being recycled while queued.
    pub(crate) fn require(&self, path: &Path) -> io::Result<()> {
        for (path, parent) in &self.0.parents {
            if !fs::metadata(path).is_ok_and(|metadata| {
                metadata.is_dir() && Identity::from_metadata(&metadata) == parent.identity
            }) {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "The source or destination folder has changed. Its current contents are preserved; select the intended location again to start new work.",
                ));
            }
        }
        let selected = self
            .0
            .entries
            .get(path)
            .ok_or_else(|| invalid("Work must use its originally observed selection"))?
            .as_ref()
            .map_err(|error| io::Error::new(error.kind(), error.to_string()))?;
        if !selected.identity.matches(path) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "The selected item has changed. Its current contents are preserved; select it again to start new work.",
            ));
        }
        Ok(())
    }

    pub(crate) fn capture(paths: &[PathBuf]) -> io::Result<Self> {
        Self::capture_with_credit(paths, 0)
    }

    /// Rename preserves its final selected link but follows both parent aliases.
    /// All three observations share the ordinary source budget and lifetime.
    pub(crate) fn capture_rename(source: &Path, destination: &Path) -> io::Result<Self> {
        let parents = [source.parent(), destination.parent()]
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| invalid("Rename needs source and destination folders"))?;
        Self::capture_entries(&[source.to_owned()], 0, &parents)
    }

    /// The previous opaque selection stays alive throughout preparation. Credit
    /// is limited to its actual reservation; clones cannot multiply that credit.
    pub(crate) fn capture_replacing(paths: &[PathBuf], previous: &Self) -> io::Result<Self> {
        Self::capture_with_credit(paths, previous.0._pins.0)
    }

    fn capture_with_credit(paths: &[PathBuf], credit: usize) -> io::Result<Self> {
        Self::capture_entries(paths, credit, &[])
    }

    fn capture_entries(paths: &[PathBuf], credit: usize, parents: &[&Path]) -> io::Result<Self> {
        use rustix::process::{Resource, getrlimit};
        // Ordinary captures leave three quarters of the descriptor allowance
        // for SQLite, transport and work. Replacement can temporarily overlap
        // its old owner, with an absolute ceiling of half the allowance/8192.
        let limit = getrlimit(Resource::Nofile)
            .current
            .map_or(MAX_SOURCE_PINS, |soft| {
                (soft / 4).min(MAX_SOURCE_PINS as u64) as usize
            });
        let process_limit = limit + credit.min(limit);
        let mut selection = Selection {
            entries: BTreeMap::new(),
            parents: BTreeMap::new(),
            _pins: Pins::default(),
        };
        for &parent in parents {
            if selection.parents.contains_key(parent) {
                continue;
            }
            selection._pins.acquire(limit, process_limit)?;
            use rustix::fs::{Mode, OFlags, open};
            let file = fs::File::from(open(
                parent,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            let identity = Identity::from_metadata(&file.metadata()?);
            selection.parents.insert(
                parent.to_path_buf(),
                SelectedEntry {
                    restart: super::restart::CapturedEntry::capture(&file, parent),
                    _file: file,
                    identity,
                },
            );
        }
        for path in paths {
            if selection.entries.contains_key(path) {
                continue;
            }
            selection._pins.acquire(limit, process_limit)?;
            let observation = (|| -> io::Result<SelectedEntry> {
                use rustix::fs::{Mode, OFlags, open};
                let (file, restart) =
                    if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
                        let parent = fs::File::from(open(
                            parent,
                            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                            Mode::empty(),
                        )?);
                        let file = fs::File::from(rustix::fs::openat(
                            &parent,
                            name,
                            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                            Mode::empty(),
                        )?);
                        let restart = super::restart::CapturedEntry::capture_in(&file, &parent);
                        (file, restart)
                    } else {
                        let file = fs::File::from(open(
                            path,
                            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                            Mode::empty(),
                        )?);
                        let restart = super::restart::CapturedEntry::capture(&file, path);
                        (file, restart)
                    };
                let identity = Identity::from_metadata(&file.metadata()?);
                Ok(SelectedEntry {
                    restart,
                    _file: file,
                    identity,
                })
            })();
            if observation.as_ref().is_err_and(|error| {
                matches!(error.raw_os_error(), Some(code) if code == rustix::io::Errno::MFILE.raw_os_error() || code == rustix::io::Errno::NFILE.raw_os_error())
            }) {
                return Err(io::Error::other(
                    "Source observation is out of file handles. Close other work and try again.",
                ));
            }
            selection.entries.insert(path.clone(), observation);
        }
        Ok(Self(Arc::new(selection)))
    }

    pub(crate) fn transfer(&self, request: TransferRequest) -> io::Result<ObservedTransfer> {
        if request
            .sources
            .iter()
            .any(|path| !self.0.entries.contains_key(path))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A transfer must use its originally observed selection",
            ));
        }
        Ok(ObservedTransfer {
            destination: DestinationObservation::capture(request.destination.clone())?,
            request,
            sources: self.clone(),
        })
    }
}

impl ObservedTransfer {
    pub(super) fn reopen(plan: &TransferContinuation) -> io::Result<Self> {
        let original = plan
            .restart
            .as_ref()
            .ok_or_else(|| invalid("This older plan has no original-object proof."))?;
        let destination = DestinationObservation::reopen(
            plan.destination.clone(),
            original.destination_original()?,
        )?;
        let mut selection = Selection {
            entries: BTreeMap::new(),
            parents: BTreeMap::new(),
            _pins: Pins::default(),
        };
        // Use the same bounded pin allowance as ordinary admissions. Reopening
        // proves the old objects; it never captures new durable originals.
        use rustix::process::{Resource, getrlimit};
        let limit = getrlimit(Resource::Nofile)
            .current
            .map_or(MAX_SOURCE_PINS, |soft| {
                (soft / 4).min(MAX_SOURCE_PINS as u64) as usize
            });
        for (path, captured) in original.required_sources(plan)? {
            selection._pins.acquire(limit, limit)?;
            let file = captured.reopen(&path)?;
            let identity = Identity::from_metadata(&file.metadata()?);
            selection.entries.insert(
                path,
                Ok(SelectedEntry {
                    restart: captured,
                    _file: file,
                    identity,
                }),
            );
        }
        let request = TransferRequest {
            kind: plan.kind,
            destination: plan.destination.clone(),
            sources: selection.entries.keys().cloned().collect(),
        };
        Ok(Self {
            request,
            sources: ObservedSources(Arc::new(selection)),
            destination,
        })
    }
    pub(super) fn original_directory(&self, path: &Path) -> bool {
        self.sources
            .0
            .entries
            .get(path)
            .and_then(|entry| entry.as_ref().ok())
            .is_some_and(|entry| entry.restart.is_directory())
    }
    pub(crate) fn restart_observation(&self) -> super::RestartObservation {
        let sources = self
            .request
            .sources
            .iter()
            .map(|path| {
                let captured = match self.sources.0.entries.get(path) {
                    Some(Ok(entry)) => entry.restart.clone(),
                    Some(Err(error)) => super::restart::CapturedEntry::unavailable(error),
                    None => super::restart::CapturedEntry::unavailable(
                        "The original selected entry is unavailable.",
                    ),
                };
                (path.clone(), captured)
            })
            .collect();
        super::RestartObservation::capture(
            &self.request,
            self.destination.restart_original(),
            sources,
        )
    }

    pub fn request(&self) -> &TransferRequest {
        &self.request
    }

    /// A live continuation's recorded traversal already verified this selected
    /// directory. Another part may have completed its postorder cleanup; absence
    /// still grants no mutation rights, and a replacement must still be refused.
    pub(super) fn verified_directory(&self, path: &Path, identity: Identity) -> bool {
        self.sources
            .0
            .entries
            .get(path)
            .and_then(|entry| entry.as_ref().ok())
            .is_some_and(|entry| entry.identity == identity)
    }

    pub(super) fn check(&self, path: &Path, verified: &mut BTreeSet<PathBuf>) -> io::Result<()> {
        for ancestor in path.ancestors() {
            let Some(observation) = self.sources.0.entries.get(ancestor) else {
                continue;
            };
            let entry = observation
                .as_ref()
                .map_err(|error| io::Error::new(error.kind(), error.to_string()))?;
            match Identity::read(ancestor) {
                Ok(identity) if identity == entry.identity => {
                    entry.restart.require_name(ancestor)?;
                    verified.insert(ancestor.to_owned());
                }
                // The engine also checks placement after moving a source into
                // quarantine. Missing originals are expected then; its journal,
                // payload identity and fingerprint own the removal proof. Before
                // first access, a missing original is an ordinary source failure.
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound && verified.contains(ancestor) => {}
                Err(error) => return Err(error),
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "The selected source has changed; its current contents are preserved. Select it again to start a new transfer.",
                    ));
                }
            }
        }
        Ok(())
    }
}
