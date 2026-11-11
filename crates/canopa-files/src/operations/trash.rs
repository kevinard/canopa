//! Freedesktop Trash storage. Public methods run on filesystem workers.
use super::*;
use crate::browse::Entry;
use std::{
    ffi::{OsStr, OsString},
    io::Write,
    os::unix::ffi::{OsStrExt, OsStringExt},
};

const INFO_LIMIT: usize = 64 * 1024;

pub mod catalog;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrashLocation {
    #[serde(with = "crate::path_serde")]
    directory: PathBuf,
    #[serde(with = "crate::path_serde::optional")]
    top: Option<PathBuf>,
    top_identity: Option<Identity>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrashItem {
    #[serde(with = "crate::path_serde")]
    pub path: PathBuf,
    #[serde(with = "crate::path_serde::optional")]
    pub original: Option<PathBuf>,
    pub deletion_date: Option<String>,
    pub problem: Option<String>,
    pub entry: Entry,
    location: TrashLocation,
    info: Option<Info>,
    identity: Identity,
}
impl TrashItem {
    pub fn version(&self) -> PayloadVersion {
        PayloadVersion {
            identity: self.identity,
            size: self.entry.size,
            modified: self.entry.modified,
        }
    }

    pub fn name(&self) -> &OsStr {
        self.original
            .as_deref()
            .and_then(Path::file_name)
            .unwrap_or_else(|| self.path.file_name().unwrap_or(self.path.as_os_str()))
    }
}

/// Presentation invalidation only; this token grants no execution authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PayloadVersion {
    identity: Identity,
    size: u64,
    modified: Option<std::time::SystemTime>,
}

struct LocationDiscovery {
    locations: Vec<TrashLocation>,
    #[cfg(feature = "local-runtime")]
    creation_parents: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct TrashOutcome {
    pub id: Option<u64>,
    pub completed: usize,
    pub paths: Vec<PathBuf>,
    pub failures: Vec<(PathBuf, String)>,
    pub remaining: Vec<PathBuf>,
    pub cancelled: bool,
    /// Failed sources whose volume offers no usable Trash storage. They are
    /// unchanged; only an explicit, confirmed permanent deletion removes them.
    pub unavailable: Vec<PathBuf>,
}

/// Confirmed results survive a later recovery or history-finalization error.
/// An unconfirmed current item stays in remaining and needs reconciliation.
pub(crate) struct InterruptedTrash {
    pub outcome: Box<TrashOutcome>,
    pub error: io::Error,
}

pub(crate) struct InterruptedRestore {
    pub summary: Option<OperationSummary>,
    pub error: io::Error,
    pub needs_review: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Info {
    #[serde(with = "crate::path_serde")]
    path: PathBuf,
    data: Vec<u8>,
}

impl TrashLocation {
    pub fn home(data_home: impl AsRef<Path>) -> io::Result<Self> {
        let data_home = data_home.as_ref();
        if !data_home.is_absolute() {
            return Err(invalid("Trash data directory must be absolute"));
        }
        Ok(Self {
            directory: data_home.join("Trash"),
            top: None,
            top_identity: None,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn for_source(data_home: &Path, source: &Path) -> io::Result<Self> {
        validate_target(source)?;
        let parent = fs::canonicalize(source.parent().unwrap())?;
        let source_mount = mount_id(&parent)?;
        let mut existing = data_home;
        let home_mount = loop {
            match fs::canonicalize(existing) {
                Ok(path) => break mount_id(&path)?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    existing = existing.parent().ok_or(error)?;
                }
                Err(error) => return Err(error),
            }
        };
        if source_mount == home_mount {
            return Self::home(data_home);
        }
        let top = mounts()?
            .into_iter()
            .find_map(|(id, top)| (id == source_mount).then_some(top))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "the source volume is unavailable")
            })?;
        Self::volume(top)
    }

    /// Existing Trash roots only: discovery never creates storage on a volume.
    pub fn available(data_home: &Path) -> io::Result<Vec<Self>> {
        Ok(Self::discover(data_home)?.locations)
    }

    fn discover(data_home: &Path) -> io::Result<LocationDiscovery> {
        let mut locations = vec![Self::home(data_home)?];
        #[cfg(feature = "local-runtime")]
        let mut creation_parents = Vec::new();
        let uid = rustix::process::geteuid().as_raw();
        let mut seen = std::collections::BTreeSet::new();
        for (_, top) in mounts()? {
            if !seen.insert(top.clone()) {
                continue;
            }
            let Ok(identity) = Identity::read(&top) else {
                continue;
            };
            let shared = top.join(".Trash");
            let shared_usable = shared_directory(&shared).is_ok();
            #[cfg(feature = "local-runtime")]
            {
                if shared_usable {
                    creation_parents.push(shared.clone());
                }
                if fs::metadata(&top).is_ok_and(|metadata| metadata.is_dir())
                    && rustix::fs::accessat(
                        rustix::fs::CWD,
                        &top,
                        rustix::fs::Access::WRITE_OK | rustix::fs::Access::EXEC_OK,
                        rustix::fs::AtFlags::EACCESS,
                    )
                    .is_ok()
                {
                    creation_parents.push(top.clone());
                }
            }
            let candidates = [
                (shared.join(uid.to_string()), shared_usable),
                (top.join(format!(".Trash-{uid}")), true),
            ];
            for (directory, usable) in candidates {
                if usable
                    && fs::symlink_metadata(&directory).is_ok_and(|metadata| metadata.uid() == uid)
                {
                    locations.push(Self {
                        directory,
                        top: Some(top.clone()),
                        top_identity: Some(identity),
                    });
                }
            }
        }
        Ok(LocationDiscovery {
            locations,
            #[cfg(feature = "local-runtime")]
            creation_parents,
        })
    }

    /// Prepares per-user storage at a mounted volume's root, creating its private
    /// directories. Use `available` for discovery that must not create storage.
    pub fn volume(top: impl AsRef<Path>) -> io::Result<Self> {
        let top = fs::canonicalize(top)?;
        let identity = Identity::read(&top)?;
        let uid = rustix::process::geteuid().as_raw();
        let shared = top.join(".Trash");
        if shared_directory(&shared).is_ok() {
            let location = Self {
                directory: shared.join(uid.to_string()),
                top: Some(top.clone()),
                top_identity: Some(identity),
            };
            if location.ensure().is_ok() {
                return Ok(location);
            }
        }
        let location = Self {
            directory: top.join(format!(".Trash-{uid}")),
            top: Some(top),
            top_identity: Some(identity),
        };
        location.ensure()?;
        Ok(location)
    }

    fn check_volume(&self) -> io::Result<()> {
        if let Some(top) = &self.top {
            if !self
                .top_identity
                .is_some_and(|identity| identity.matches(top))
            {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "the Trash volume is unavailable or has changed",
                ));
            }
            if self.directory.parent() == Some(top.join(".Trash").as_path()) {
                shared_directory(&top.join(".Trash"))?;
            }
        }
        Ok(())
    }

    fn ensure(&self) -> io::Result<()> {
        self.check_volume()?;
        if let Some(parent) = self.directory.parent() {
            fs::create_dir_all(parent)?;
        }
        for path in [
            &self.directory,
            &self.directory.join("files"),
            &self.directory.join("info"),
        ] {
            match fs::DirBuilder::new().mode(0o700).create(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            private_directory(path)?;
        }
        Ok(())
    }

    pub fn list(&self, cancellation: &Cancellation) -> io::Result<Vec<TrashItem>> {
        check_cancel(cancellation)?;
        self.check_volume()?;
        if !self.directory.try_exists()? {
            return Ok(Vec::new());
        }
        private_directory(&self.directory)?;
        private_directory(&self.directory.join("files"))?;
        private_directory(&self.directory.join("info"))?;
        let mut items = Vec::new();
        for file in fs::read_dir(self.directory.join("files"))? {
            check_cancel(cancellation)?;
            let file = file?;
            let path = file.path();
            let entry = match Entry::read(&path) {
                Ok(entry) => entry,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            let mut name = file.file_name();
            name.push(".trashinfo");
            let info_path = self.directory.join("info").join(name);
            let (info, original, deletion_date, problem) =
                match read_info(&info_path).and_then(|data| {
                    let (original, date) = parse_info(&data, self)?;
                    Ok((
                        Info {
                            path: info_path.clone(),
                            data,
                        },
                        original,
                        date,
                    ))
                }) {
                    Ok((info, original, date)) => (Some(info), Some(original), date, None),
                    Err(error) => (
                        None,
                        None,
                        None,
                        Some(format!("Original location unavailable: {error}")),
                    ),
                };
            let identity = Identity::read(&path)?;
            items.push(TrashItem {
                path,
                original,
                deletion_date,
                problem,
                entry,
                location: self.clone(),
                info,
                identity,
            });
        }
        items.sort_by(|left, right| {
            left.name()
                .cmp(right.name())
                .then_with(|| left.path.cmp(&right.path))
        });
        Ok(items)
    }
}

impl OperationEngine {
    pub fn restore(
        &self,
        item: &TrashItem,
        destination: Option<&Path>,
        cancellation: &Cancellation,
    ) -> io::Result<OperationSummary> {
        self.restore_with_outcome(item, destination, cancellation)
            .map_err(|failure| failure.error)
    }

    pub(crate) fn restore_with_outcome(
        &self,
        item: &TrashItem,
        destination: Option<&Path>,
        cancellation: &Cancellation,
    ) -> Result<OperationSummary, InterruptedRestore> {
        let mut summary = None;
        let mut needs_review = false;
        let attempt = (|| -> io::Result<()> {
            check_cancel(cancellation)?;
            let current = destination
                .or(item.original.as_deref())
                .ok_or_else(|| {
                    invalid("choose a location for an item without valid Trash information")
                })?
                .to_owned();
            validate_target(&current)?;
            if current
                .components()
                .any(|component| matches!(component, Component::ParentDir))
                || current.starts_with(&item.location.directory)
            {
                return Err(invalid("choose a destination outside Trash storage"));
            }
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("operation history failed"))?;
            recover(&mut state, cancellation)?;
            check_cancel(cancellation)?;
            item.location.check_volume()?;
            private_directory(&item.location.directory)?;
            let info = item.info.clone().ok_or_else(|| {
                invalid("Trash information is unavailable; use recovery to choose a location")
            })?;
            info.validate(&item.path)?;
            info.require()?;
            if !item.identity.matches(&item.path) {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the selected Trash item has been replaced or moved",
                ));
            }
            let label = format!("restore {}", display_name(&current));
            let id = state.begin(&label)?;
            let change = Change::Restored {
                original: item.path.clone(),
                current: current.clone(),
                identity: item.identity,
                info: info.clone(),
            };
            let ordinal = state.next_ordinal(id)?;
            needs_review = true;
            if let Err(error) = state.intent(&Pending::Forward {
                id,
                ordinal,
                change: change.clone(),
                stage: None,
            }) {
                // Intent may refuse before journalling, including a retained
                // source guard. No payload work has started at this point.
                // A remaining or unreadable journal still requires review.
                if state.pending()?.is_none() {
                    needs_review = false;
                    state.discard_empty(id)?;
                }
                return Err(error);
            }
            let result = (|| {
                check_cancel(cancellation)?;
                rename_no_replace(&item.path, &current)?;
                info.remove()?;
                sync_change(&change)?;
                state.append(id, ordinal, &change)
            })();
            if let Err(error) = result {
                recover(&mut state, cancellation)?;
                let committed = state.last_change(id)?.is_some_and(|(_, change)| {
                    matches!(change, Change::Restored { original, current: restored, .. } if original == item.path && restored == current)
                });
                if !committed {
                    needs_review = false;
                    state.discard_empty(id)?;
                    return Err(error);
                }
            }
            summary = Some(OperationSummary {
                id,
                label,
                paths: vec![current],
            });
            state.trim()?;
            needs_review = false;
            Ok(())
        })();
        match attempt {
            Ok(()) => summary.ok_or_else(|| InterruptedRestore {
                summary: None,
                error: invalid("Restore completed without a recorded result"),
                needs_review: true,
            }),
            Err(error) => Err(InterruptedRestore {
                summary,
                error,
                needs_review,
            }),
        }
    }

    pub fn trash(
        &self,
        location: &TrashLocation,
        sources: &[PathBuf],
        cancellation: &Cancellation,
    ) -> io::Result<TrashOutcome> {
        self.trash_with(sources, cancellation, None, |_| Ok(location.clone()))
            .map_err(|failure| failure.error)
    }

    pub fn trash_files(
        &self,
        data_home: &Path,
        sources: &[PathBuf],
        cancellation: &Cancellation,
    ) -> io::Result<TrashOutcome> {
        self.trash_with(sources, cancellation, None, |source| {
            TrashLocation::for_source(data_home, source)
        })
        .map_err(|failure| failure.error)
    }

    pub(crate) fn trash_observed(
        &self,
        data_home: &Path,
        sources: &[PathBuf],
        observed: &super::transfer::ObservedSources,
        cancellation: &Cancellation,
    ) -> Result<TrashOutcome, InterruptedTrash> {
        self.trash_with(sources, cancellation, Some(observed), |source| {
            TrashLocation::for_source(data_home, source)
        })
    }

    fn trash_with(
        &self,
        sources: &[PathBuf],
        cancellation: &Cancellation,
        observed: Option<&super::transfer::ObservedSources>,
        mut locate: impl FnMut(&Path) -> io::Result<TrashLocation>,
    ) -> Result<TrashOutcome, InterruptedTrash> {
        let mut result = TrashOutcome::default();
        let mut pending: std::collections::VecDeque<_> = sources
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let attempt = (|| -> io::Result<()> {
            check_cancel(cancellation)?;
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("operation history failed"))?;
            recover(&mut state, cancellation)?;
            let id = state.begin(&format!(
                "move {} {} to Trash",
                sources.len(),
                if sources.len() == 1 { "item" } else { "items" }
            ))?;
            let mut sources: std::collections::BTreeSet<_> = sources.iter().cloned().collect();
            let folders: Vec<_> = sources
                .iter()
                .filter(|path| fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()))
                .cloned()
                .collect();
            sources.retain(|source| {
                !folders
                    .iter()
                    .any(|folder| source != folder && source.starts_with(folder))
            });
            pending = sources.into_iter().collect();
            while let Some(source) = pending.front().cloned() {
                if cancellation.is_cancelled() {
                    result.cancelled = true;
                    break;
                }
                let mut unavailable = false;
                let operation = (|| -> io::Result<()> {
                    validate_target(&source)?;
                    if source
                        .components()
                        .any(|component| matches!(component, Component::ParentDir))
                    {
                        return Err(invalid("Trash paths cannot contain parent traversal"));
                    }
                    if let Some(observed) = observed {
                        observed.require(&source)?;
                    }
                    // Failures to find or prepare storage mean this volume
                    // cannot hold Trash; the source itself is untouched.
                    let location = locate(&source).inspect_err(|_| unavailable = true)?;
                    location.ensure().inspect_err(|_| unavailable = true)?;
                    let physical = fs::canonicalize(source.parent().unwrap())?
                        .join(source.file_name().unwrap());
                    let storage = fs::canonicalize(&location.directory)?;
                    if physical.starts_with(&storage) || storage.starts_with(&physical) {
                        return Err(invalid("Trash storage cannot be moved into itself"));
                    }
                    let identity = Identity::read(&source)?;
                    let relative = if let Some(top) = &location.top {
                        physical
                            .strip_prefix(top)
                            .map_err(|_| invalid("the file is outside this Trash volume"))?
                    } else {
                        &source
                    };
                    let data = format!(
                        "[Trash Info]\nPath={}\nDeletionDate={}\n",
                        encode(relative.as_os_str().as_bytes()),
                        chrono::Local::now().format("%Y-%m-%dT%H:%M:%S")
                    )
                    .into_bytes();
                    let mut file = tempfile::Builder::new()
                        .prefix("canopa-")
                        .suffix(".trashinfo")
                        .tempfile_in(location.directory.join("info"))
                        .inspect_err(|_| unavailable = true)?;
                    file.write_all(&data)?;
                    file.as_file().sync_all()?;
                    let current = location
                        .directory
                        .join("files")
                        .join(file.path().file_stem().unwrap());
                    let info = Info {
                        path: file.path().to_owned(),
                        data,
                    };
                    let change = Change::Trashed {
                        original: source.clone(),
                        current: current.clone(),
                        identity,
                        info: info.clone(),
                    };
                    let ordinal = state.next_ordinal(id)?;
                    state.intent(&Pending::Forward {
                        id,
                        ordinal,
                        change: change.clone(),
                        stage: None,
                    })?;
                    file.keep().map_err(|error| error.error)?;
                    sync_parent(&info.path)?;
                    if let Err(error) = check_cancel(cancellation)
                        .and_then(|()| rename_no_replace(&source, &current))
                    {
                        // The storage is on another filesystem than the source.
                        unavailable |=
                            error.raw_os_error() == Some(rustix::io::Errno::XDEV.raw_os_error());
                        info.remove()?;
                        state.clear_intent()?;
                        return Err(error);
                    }
                    sync_change(&change)?;
                    state.append(id, ordinal, &change)?;
                    Ok(())
                })();
                match operation {
                    Ok(()) => {
                        result.completed += 1;
                        result.paths.push(source);
                        result.id = Some(id);
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                        result.cancelled = true;
                        break;
                    }
                    Err(error) => {
                        recover(&mut state, cancellation)?;
                        if state.last_change(id)?.is_some_and(|(_, change)| {
                        matches!(change, Change::Trashed { original, .. } if original == source)
                    }) {
                        // Recovery may have completed the pending rename and
                        // appended its actual change after an initial sync or
                        // history error. Report that committed work once.
                        result.completed += 1;
                        result.paths.push(source);
                        result.id = Some(id);
                    } else {
                        if unavailable && fs::symlink_metadata(&source).is_ok() {
                            result.unavailable.push(source.clone());
                        }
                        result.failures.push((source, error.to_string()));
                    }
                    }
                }
                pending.pop_front();
            }
            if state.last_change(id)?.is_some() {
                result.id = Some(id);
            }
            state.discard_empty(id)?;
            state.trim()?;
            Ok(())
        })();
        result.remaining = pending.into_iter().collect();
        match attempt {
            Ok(()) => Ok(result),
            Err(error) => {
                if error.kind() == io::ErrorKind::Interrupted {
                    result.cancelled = true;
                }
                Err(InterruptedTrash {
                    outcome: Box::new(result),
                    error,
                })
            }
        }
    }
}

impl OperationEngine {
    /// Permanently remove stored Trash items, payload first and then its
    /// information file, as the freedesktop specification orders it. Prefer
    /// the confirmed queue request; this public seam serves headless tests.
    pub fn delete_from_trash(
        &self,
        items: &[TrashItem],
        cancellation: &Cancellation,
    ) -> io::Result<super::deletion::DeleteOutcome> {
        self.delete_trashed(items, cancellation, &mut |_, _| {})
    }

    pub(crate) fn delete_trashed(
        &self,
        items: &[TrashItem],
        cancellation: &Cancellation,
        progress: &mut dyn FnMut(&Path, super::deletion::DeleteProgress),
    ) -> io::Result<super::deletion::DeleteOutcome> {
        let mut outcome = super::deletion::DeleteOutcome::default();
        let mut seen = std::collections::BTreeSet::new();
        let mut pending: std::collections::VecDeque<_> = items
            .iter()
            .filter(|item| seen.insert(item.path.clone()))
            .collect();
        check_cancel(cancellation)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?;
        recover(&mut state, cancellation)?;
        drop(state);
        let mut removal = super::deletion::Removal::new(cancellation, progress);
        let mut forgotten: std::collections::BTreeMap<
            PathBuf,
            std::collections::BTreeSet<OsString>,
        > = std::collections::BTreeMap::new();
        while let Some(item) = pending.front().copied() {
            if cancellation.is_cancelled() {
                outcome.cancelled = true;
                break;
            }
            let result = (|| -> io::Result<()> {
                validate_target(&item.path)?;
                let files = item.location.directory.join("files");
                if item.path.parent() != Some(files.as_path()) {
                    return Err(invalid("the item is not stored in its Trash location"));
                }
                item.location.check_volume()?;
                private_directory(&item.location.directory)?;
                private_directory(&files)?;
                if let Some(info) = &item.info {
                    info.validate(&item.path)?;
                }
                if !item.identity.matches(&item.path) {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "the selected Trash item has been replaced or moved",
                    ));
                }
                let state = self
                    .state
                    .lock()
                    .map_err(|_| io::Error::other("operation history failed"))?;
                removal.remove(&item.path)?;
                // A leftover information file without its payload is ignored
                // by readers, so its removal is best-effort. Changed
                // information belongs to someone else and stays.
                match &item.info {
                    Some(info) => {
                        let _ = info.remove();
                    }
                    None => {
                        let mut name = item.path.file_name().unwrap().to_os_string();
                        name.push(".trashinfo");
                        let path = item.location.directory.join("info").join(name);
                        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
                drop(state);
                forgotten
                    .entry(item.location.directory.clone())
                    .or_default()
                    .insert(item.path.file_name().unwrap().to_os_string());
                Ok(())
            })();
            match result {
                Ok(()) => outcome.completed.push(item.path.clone()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                    outcome.cancelled = true;
                    if fs::symlink_metadata(&item.path).is_ok() {
                        outcome.failures.push((
                            item.path.clone(),
                            "Stopped; part of this folder may already be deleted.".into(),
                        ));
                    } else {
                        outcome.completed.push(item.path.clone());
                    }
                    pending.pop_front();
                    break;
                }
                Err(error) => outcome
                    .failures
                    .push((item.path.clone(), error.to_string())),
            }
            pending.pop_front();
        }
        for (directory, names) in forgotten {
            // The size cache is advisory; a failure only leaves stale lines
            // that readers ignore because their folder no longer exists.
            let _ = forget_sizes(&directory, &names);
        }
        outcome.remaining = pending.into_iter().map(|item| item.path.clone()).collect();
        let totals = removal.totals();
        outcome.entries = totals.entries;
        outcome.bytes = totals.bytes;
        Ok(outcome)
    }
}

/// Drop `directorysizes` entries for removed payloads (Trash specification
/// 1.0, "Directory size cache"). The file is replaced atomically.
fn forget_sizes(directory: &Path, names: &std::collections::BTreeSet<OsString>) -> io::Result<()> {
    const LIMIT: u64 = 16 * 1024 * 1024;
    let path = directory.join("directorysizes");
    let mut file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(
            rustix::fs::OFlags::NOFOLLOW.bits() as i32 | rustix::fs::OFlags::NONBLOCK.bits() as i32,
        )
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > LIMIT {
        return Err(invalid("invalid Trash size cache"));
    }
    let mut data = Vec::new();
    (&mut file).take(LIMIT + 1).read_to_end(&mut data)?;
    let mut kept = Vec::with_capacity(data.len());
    let mut removed = false;
    for line in data.split_inclusive(|byte| *byte == b'\n') {
        let entry = line.strip_suffix(b"\n").unwrap_or(line);
        let name = entry
            .splitn(3, |byte| *byte == b' ')
            .nth(2)
            .and_then(|name| decode(name).ok());
        if name.is_some_and(|name| names.contains(std::ffi::OsStr::from_bytes(&name))) {
            removed = true;
        } else {
            kept.extend_from_slice(line);
        }
    }
    if !removed {
        return Ok(());
    }
    let mut replacement = tempfile::Builder::new()
        .prefix(".directorysizes-")
        .tempfile_in(directory)?;
    replacement.write_all(&kept)?;
    replacement.as_file().sync_all()?;
    replacement.persist(&path).map_err(|error| error.error)?;
    sync_parent(&path)
}

impl Info {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
    pub(super) fn visit_path(&mut self, mut visit: impl FnMut(&mut PathBuf)) {
        visit(&mut self.path);
    }
    pub(super) fn restore(&self) -> io::Result<bool> {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&self.path)
        {
            Ok(mut file) => {
                file.write_all(&self.data)?;
                file.sync_all()?;
                sync_parent(&self.path)?;
                Ok(true)
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                self.require()?;
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }
    pub(super) fn require(&self) -> io::Result<()> {
        if read_info(&self.path)? != self.data {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Trash information changed; the item is preserved",
            ));
        }
        Ok(())
    }
    pub(super) fn remove(&self) -> io::Result<()> {
        match self.require() {
            Ok(()) => fs::remove_file(&self.path)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
        sync_parent(&self.path)
    }
    pub(super) fn validate(&self, payload: &Path) -> io::Result<()> {
        validate_target(&self.path)?;
        if self.data.len() > INFO_LIMIT
            || self.path.extension() != Some(OsStr::new("trashinfo"))
            || self.path.parent().and_then(Path::file_name) != Some(OsStr::new("info"))
            || payload.parent().and_then(Path::file_name) != Some(OsStr::new("files"))
            || self.path.parent().and_then(Path::parent) != payload.parent().and_then(Path::parent)
            || self.path.file_stem() != payload.file_name()
        {
            return Err(invalid("invalid Trash record"));
        }
        Ok(())
    }
    pub(super) fn sync(&self) -> io::Result<()> {
        sync_parent(&self.path)
    }
}

pub(super) fn recover_forward(change: &Change) -> io::Result<()> {
    if let Change::Trashed {
        original,
        current,
        identity,
        info,
    } = change
    {
        info.validate(current)?;
        if identity.matches(current) {
            info.require()?;
        } else if identity.matches(original) {
            info.remove()?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "an interrupted Trash operation needs recovery",
            ));
        }
    }
    if let Change::Restored {
        original,
        current,
        identity,
        info,
    } = change
    {
        info.validate(original)?;
        if identity.matches(current) {
            info.remove()?;
        } else if identity.matches(original) {
            info.require()?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "an interrupted restore needs recovery",
            ));
        }
    }
    Ok(())
}

pub(super) fn undo_restore(
    change: &Change,
    state: &Store,
    id: u64,
    ordinal: u64,
) -> io::Result<()> {
    let Change::Restored {
        original,
        current,
        identity,
        info,
    } = change
    else {
        return Err(invalid("expected a restored item"));
    };
    if !state
        .resolve_for(id, ordinal, current, *identity)?
        .matches(current)
    {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the restored item changed or was moved",
        ));
    }
    match fs::symlink_metadata(original) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "the Trash location is occupied by another item",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let created = info.restore()?;
    if let Err(error) = rename_no_replace(current, original) {
        if created {
            info.remove()?;
        }
        return Err(error);
    }
    Ok(())
}

pub(super) fn recover_reverse(change: &Change) -> io::Result<()> {
    if let Change::Restored {
        original,
        current,
        identity,
        info,
    } = change
    {
        if identity.matches(original) {
            info.require()?;
        } else if identity.matches(current) {
            info.remove()?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "an interrupted restore Undo needs recovery",
            ));
        }
    }
    Ok(())
}

fn private_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Trash directories must be real, owned, private directories",
        ));
    }
    Ok(())
}
fn shared_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.mode() & 0o1000 == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "shared Trash must be a real directory with its sticky bit set",
        ));
    }
    Ok(())
}

fn mount_id(path: &Path) -> io::Result<u64> {
    let status = rustix::fs::statx(
        rustix::fs::CWD,
        path,
        rustix::fs::AtFlags::empty(),
        rustix::fs::StatxFlags::MNT_ID,
    )?;
    if status.stx_mask & rustix::fs::StatxFlags::MNT_ID.bits() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this system cannot identify mounted filesystems for Trash",
        ));
    }
    Ok(status.stx_mnt_id)
}
fn mounts() -> io::Result<Vec<(u64, PathBuf)>> {
    let mut data = Vec::new();
    fs::File::open("/proc/self/mountinfo")?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut data)?;
    if data.len() > 8 * 1024 * 1024 {
        return Err(invalid("mount information is too large"));
    }
    let mut mounts = Vec::new();
    for line in data
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let fields: Vec<_> = line.split(|byte| *byte == b' ').collect();
        let id = fields
            .first()
            .and_then(|id| std::str::from_utf8(id).ok())
            .and_then(|id| id.parse().ok())
            .ok_or_else(|| invalid("invalid mount identifier"))?;
        let encoded = fields
            .get(4)
            .ok_or_else(|| invalid("missing mount location"))?;
        let mut path = Vec::new();
        let mut offset = 0;
        while offset < encoded.len() {
            if encoded[offset] == b'\\' {
                let value = encoded
                    .get(offset + 1..offset + 4)
                    .and_then(|digits| std::str::from_utf8(digits).ok())
                    .and_then(|digits| u8::from_str_radix(digits, 8).ok())
                    .ok_or_else(|| invalid("invalid mount location escaping"))?;
                path.push(value);
                offset += 4;
            } else {
                path.push(encoded[offset]);
                offset += 1;
            }
        }
        mounts.push((id, PathBuf::from(OsString::from_vec(path))));
    }
    Ok(mounts)
}
fn read_info(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(
            rustix::fs::OFlags::NOFOLLOW.bits() as i32 | rustix::fs::OFlags::NONBLOCK.bits() as i32,
        )
        .open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > INFO_LIMIT as u64 {
        return Err(invalid("invalid Trash information file"));
    }
    let mut data = Vec::new();
    (&mut file)
        .take(INFO_LIMIT as u64 + 1)
        .read_to_end(&mut data)?;
    if data.len() > INFO_LIMIT {
        return Err(invalid("Trash information is too large"));
    }
    Ok(data)
}
fn parse_info(data: &[u8], location: &TrashLocation) -> io::Result<(PathBuf, Option<String>)> {
    let mut lines = data
        .split(|byte| *byte == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line));
    if lines.next() != Some(b"[Trash Info]".as_slice()) {
        return Err(invalid("missing Trash header"));
    }
    let mut path = None;
    let mut date = None;
    for line in lines {
        if path.is_none()
            && let Some(value) = line.strip_prefix(b"Path=")
        {
            path = Some(decode(value)?);
        }
        if date.is_none()
            && let Some(value) = line.strip_prefix(b"DeletionDate=")
        {
            date = Some(String::from_utf8_lossy(value).into_owned());
        }
    }
    let mut original = PathBuf::from(OsString::from_vec(
        path.ok_or_else(|| invalid("missing original location"))?,
    ));
    if original.as_os_str().is_empty()
        || original.as_os_str().as_bytes().contains(&0)
        || original
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(invalid("invalid original location"));
    }
    if !original.is_absolute() {
        let base = location
            .top
            .as_deref()
            .or_else(|| location.directory.parent())
            .ok_or_else(|| invalid("missing Trash parent"))?;
        original = base.join(original);
    } else if location.top.is_some() {
        return Err(invalid(
            "volume Trash requires a relative original location",
        ));
    }
    validate_target(&original)?;
    Ok((original, date))
}
fn encode(bytes: &[u8]) -> String {
    let mut result = String::new();
    for byte in bytes {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(byte) {
            result.push(char::from(*byte));
        } else {
            result.push_str(&format!("%{byte:02X}"));
        }
    }
    result
}
fn decode(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        if bytes[offset] == b'%' {
            let digits = bytes
                .get(offset + 1..offset + 3)
                .ok_or_else(|| invalid("invalid original path escaping"))?;
            let value = std::str::from_utf8(digits)
                .ok()
                .and_then(|digits| u8::from_str_radix(digits, 16).ok())
                .ok_or_else(|| invalid("invalid original path escaping"))?;
            result.push(value);
            offset += 3;
        } else {
            result.push(bytes[offset]);
            offset += 1;
        }
    }
    Ok(result)
}
