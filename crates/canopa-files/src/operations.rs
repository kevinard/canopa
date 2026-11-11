//! Serialized filesystem mutations and persistent, item-level Undo transactions.
//! Run this interface on workers; the application runtime owns scheduling and
//! typed interaction independently of the renderer.
use crate::Cancellation;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
    sync::Mutex,
};
mod copy_backend;
pub(crate) mod execution;
mod fingerprint;
mod history_lock;
mod prepared_copy;
pub use execution::RequestEffects;
mod interrupted;
mod recovery;
mod rename;
#[cfg(test)]
mod rename_boundary;
mod replacement;
#[cfg(test)]
mod replacement_boundary;
pub mod requests;
mod saved_stage;
mod source_leases;
mod store;
pub use interrupted::InterruptedOperation;
pub(crate) use interrupted::Reconciliation;
pub use recovery::{
    RecoveryCopyContinuation, RecoveryCopyOutcome, RecoveryCursor, RecoveryIdentity, RecoveryItem,
    RecoveryPage, RecoveryStatus, RecoveryVersion,
};
mod copied_leaf;
pub mod deletion;
mod directory_removal;
#[cfg(test)]
mod directory_removal_boundary;
mod durability;
mod entry_move;
#[cfg(test)]
mod entry_move_boundary;
mod original;
mod serialized;
mod source_removal;
#[cfg(test)]
mod source_removal_boundary;
mod staging;
pub mod transfer;
pub mod trash;
mod undo;
use fingerprint::Fingerprint;
use store::{IntentLane, Store};
pub use undo::{UndoChoice, UndoConflict, UndoItem, UndoOutcome, UndoSource};

const HISTORY_LIMIT: usize = 100;
const LEGACY_LIMIT: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum OperationRequest {
    CreateFolder {
        #[serde(with = "crate::path_serde")]
        path: PathBuf,
    },
    Rename {
        #[serde(with = "crate::path_serde")]
        source: PathBuf,
        #[serde(with = "crate::path_serde")]
        destination: PathBuf,
    },
}
#[derive(Clone, Debug)]
pub struct OperationSummary {
    pub id: u64,
    pub label: String,
    pub paths: Vec<PathBuf>,
}
pub struct OperationEngine {
    state: Mutex<Store>,
    reads: store::ReadSource,
    source_leases: Mutex<Option<source_leases::Releaser>>,
    completion_scope: std::sync::Arc<()>,
    _lock: std::sync::Arc<history_lock::HistoryLock>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    device: u64,
    inode: u64,
}
impl Identity {
    fn read(path: &Path) -> io::Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        Ok(Self::from_metadata(&metadata))
    }
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    fn matches(self, path: &Path) -> bool {
        Self::read(path).is_ok_and(|other| other == self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Change {
    DirectoryMerge {
        change: Box<Change>,
        destination_identity: Identity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        destination_original: Option<original::Original>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        destination_mode: Option<u32>,
        #[serde(default, skip_serializing_if = "is_false")]
        defer_previous: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        destination_owned: bool,
    },
    Trashed {
        #[serde(with = "crate::path_serde")]
        original: PathBuf,
        #[serde(with = "crate::path_serde")]
        current: PathBuf,
        identity: Identity,
        info: trash::Info,
    },
    Restored {
        #[serde(with = "crate::path_serde")]
        original: PathBuf,
        #[serde(with = "crate::path_serde")]
        current: PathBuf,
        identity: Identity,
        info: trash::Info,
    },
    CreatedFolder {
        #[serde(with = "crate::path_serde")]
        path: PathBuf,
        identity: Identity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        access_parent: Option<Identity>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<u32>,
    },
    Rename {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        move_facts: Option<Box<entry_move::Facts>>,
        #[serde(with = "crate::path_serde")]
        original: PathBuf,
        #[serde(with = "crate::path_serde")]
        current: PathBuf,
        identity: Identity,
        #[serde(default)]
        retained: Option<Retained>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reviewed: Option<fingerprint::Version>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        saved: Option<saved_stage::Movement>,
    },
    Copied {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<Box<copied_leaf::Source>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        publication: Option<Box<staging::Published>>,
        #[serde(with = "crate::path_serde")]
        path: PathBuf,
        identity: Identity,
        fingerprint: Fingerprint,
        #[serde(default)]
        retained: Option<Retained>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        access_parent: Option<Identity>,
    },
    RetainedOnly {
        #[serde(with = "crate::path_serde")]
        path: PathBuf,
        retained: Retained,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        replaced: Option<Identity>,
    },
    SourceRemoved {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        originals: Option<Box<source_removal::Originals>>,
        #[serde(with = "crate::path_serde")]
        original: PathBuf,
        #[serde(with = "crate::path_serde")]
        current: PathBuf,
        identity: Identity,
        copy_identity: Identity,
        fingerprint: Fingerprint,
        source_fingerprint: Fingerprint,
        quarantine: Stage,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reviewed: Option<fingerprint::Version>,
    },
    RemovedDirectory {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        removal: Option<Box<directory_removal::Facts>>,
        #[serde(with = "crate::path_serde")]
        path: PathBuf,
        identity: Identity,
        parent_identity: Identity,
        mode: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        moved: Option<MovedDirectory>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MovedDirectory {
    #[serde(with = "crate::path_serde")]
    current: PathBuf,
    identity: Identity,
    first: u64,
    created: Option<u64>,
}
impl Change {
    fn forward_path(&self) -> &Path {
        match self {
            Self::DirectoryMerge { change, .. } => match change.as_ref() {
                Self::RemovedDirectory {
                    moved: Some(moved), ..
                } => &moved.current,
                _ => change.forward_path(),
            },
            Self::CreatedFolder { path, .. }
            | Self::Copied { path, .. }
            | Self::RetainedOnly { path, .. }
            | Self::RemovedDirectory { path, .. } => path,
            Self::Rename { current, .. }
            | Self::Trashed { current, .. }
            | Self::Restored { current, .. } => current,
            Self::SourceRemoved { original, .. } => original,
        }
    }
    fn reverse_path(&self) -> Option<&Path> {
        match self {
            Self::DirectoryMerge { change, .. } => change.reverse_path(),
            Self::Rename { original, .. }
            | Self::SourceRemoved { original, .. }
            | Self::Trashed { original, .. }
            | Self::Restored { original, .. } => Some(original),
            Self::RetainedOnly { path, .. } | Self::RemovedDirectory { path, .. } => Some(path),
            Self::Copied {
                path,
                retained: Some(_),
                ..
            } => Some(path),
            _ => None,
        }
    }
    fn retained(&self) -> Option<&Retained> {
        match self {
            Self::DirectoryMerge { change, .. } => change.retained(),
            Self::Rename { retained, .. } | Self::Copied { retained, .. } => retained.as_ref(),
            Self::RetainedOnly { retained, .. } => Some(retained),
            _ => None,
        }
    }
    fn retained_mut(&mut self) -> Option<&mut Retained> {
        match self {
            Self::DirectoryMerge { change, .. } => change.retained_mut(),
            Self::Rename { retained, .. } | Self::Copied { retained, .. } => retained.as_mut(),
            Self::RetainedOnly { retained, .. } => Some(retained),
            _ => None,
        }
    }
    fn committed(&self) -> bool {
        match self {
            Self::DirectoryMerge { .. } => false,
            Self::CreatedFolder { path, identity, .. } | Self::Copied { path, identity, .. } => {
                identity.matches(path)
            }
            Self::Rename {
                original,
                current,
                identity,
                ..
            }
            | Self::Trashed {
                original,
                current,
                identity,
                ..
            }
            | Self::Restored {
                original,
                current,
                identity,
                ..
            } => !identity.matches(original) && identity.matches(current),
            Self::RetainedOnly { retained, .. } => {
                retained.observed_identity().matches(&retained.path())
            }
            Self::SourceRemoved {
                original,
                identity,
                source_fingerprint,
                quarantine,
                ..
            } => {
                fs::symlink_metadata(quarantine.payload()).is_err()
                    && !(identity.matches(original)
                        && source_fingerprint
                            .matches(original, &Cancellation::default())
                            .unwrap_or(false))
            }
            Self::RemovedDirectory { path, identity, .. } => !identity.matches(path),
        }
    }
    fn reversed(&self) -> bool {
        match self {
            Self::DirectoryMerge {
                change,
                destination_identity,
                defer_previous,
                ..
            } => {
                destination_identity.matches(change.reverse_path().unwrap())
                    && if *defer_previous {
                        matches!(fs::symlink_metadata(change.forward_path()), Err(error) if error.kind() == io::ErrorKind::NotFound)
                    } else {
                        change.retained().map_or_else(
                        || matches!(fs::symlink_metadata(change.forward_path()), Err(error) if error.kind() == io::ErrorKind::NotFound),
                        |retained| retained.restored_at(change.forward_path()),
                    )
                    }
            }
            Self::CreatedFolder { path, .. }
            | Self::Copied {
                path,
                retained: None,
                ..
            } => {
                matches!(fs::symlink_metadata(path), Err(error) if error.kind() == io::ErrorKind::NotFound)
            }
            Self::Copied {
                path,
                retained: Some(retained),
                ..
            }
            | Self::RetainedOnly { path, retained, .. } => retained.restored_at(path),
            Self::Rename {
                original,
                current,
                identity,
                retained,
                reviewed,
                ..
            } => {
                let observed = reviewed
                    .as_ref()
                    .map_or(*identity, |version| version.identity);
                observed.matches(original)
                    && retained.as_ref().map_or_else(
                        || !observed.matches(current),
                        |retained| retained.restored_at(current),
                    )
            }
            Self::Trashed {
                original,
                current,
                identity,
                ..
            }
            | Self::Restored {
                original,
                current,
                identity,
                ..
            } => identity.matches(original) && !identity.matches(current),
            Self::SourceRemoved { .. } | Self::RemovedDirectory { .. } => false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Retained {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    replacement_facts: Option<Box<replacement::Facts>>,
    stage: Stage,
    identity: Identity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    physical_identity: Option<Identity>,
    #[serde(default)]
    recovery: bool,
    #[serde(default)]
    version: Option<fingerprint::Version>,
}
impl Retained {
    fn original_identity(&self) -> Identity {
        self.physical_identity.unwrap_or(self.identity)
    }
    fn observed_identity(&self) -> Identity {
        // User review can authorize a different physical saved version while
        // identity keeps the original logical item for older Undo bindings.
        self.version
            .as_ref()
            .map_or_else(|| self.original_identity(), |version| version.identity)
    }
    fn path(&self) -> PathBuf {
        self.stage.directory.join("previous")
    }
    fn restored_at(&self, path: &Path) -> bool {
        self.observed_identity().matches(path)
            && self.stage.identity.matches(&self.stage.directory)
            && fs::symlink_metadata(self.path())
                .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "identity",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum StageParentAccess {
    Existing,
    Created(Identity),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    directory_claim: Option<Box<directory_removal::Claim>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    replacement: Option<Box<replacement::Claim>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    entry_move: Option<Box<entry_move::Claim>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_claim: Option<Box<source_removal::Claim>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ownership: Option<Box<staging::Ownership>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_access: Option<StageParentAccess>,
    #[serde(with = "crate::path_serde")]
    directory: PathBuf,
    identity: Identity,
    #[serde(default)]
    payload_identity: Option<Identity>,
    #[serde(default)]
    parent_identity: Option<Identity>,
    #[serde(default, skip_serializing_if = "is_false")]
    sibling_payload: bool,
}
impl Stage {
    fn record_parent_access(&mut self, created: Option<Identity>) {
        self.parent_access =
            Some(created.map_or(StageParentAccess::Existing, StageParentAccess::Created));
    }
    fn created_parent(&self) -> Option<Identity> {
        match self.parent_access {
            Some(StageParentAccess::Created(identity)) => Some(identity),
            _ => None,
        }
    }
    fn prepare(parent: &Path) -> io::Result<(Self, tempfile::TempDir)> {
        let temporary = tempfile::Builder::new()
            .prefix(".canopa-files-stage-")
            .tempdir_in(parent)?;
        let directory = temporary.path().to_owned();
        Ok((
            Self {
                replacement: None,
                entry_move: None,
                source_claim: None,
                directory_claim: None,
                ownership: None,
                parent_access: Some(StageParentAccess::Existing),
                identity: Identity::read(&directory)?,
                directory,
                payload_identity: None,
                parent_identity: Some(Identity::from_metadata(&fs::metadata(parent)?)),
                sibling_payload: false,
            },
            temporary,
        ))
    }
    fn payload(&self) -> PathBuf {
        if self.sibling_payload
            && let Some(name) = self.directory.file_name()
        {
            let mut name = name.to_os_string();
            name.push("-entry");
            self.directory.with_file_name(name)
        } else {
            // Startup enumerates malformed journals before validating them.
            // An unnamed stage must remain inspectable; validation refuses it
            // before any filesystem operation can use the returned path.
            self.directory.join("folder")
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "direction", rename_all = "kebab-case", deny_unknown_fields)]
enum Pending {
    Expiry {
        entry: u64,
        record: Box<recovery::Record>,
        parent: Identity,
    },
    StageMigration {
        migration: Box<saved_stage::Migration>,
    },
    DirectoryDestination {
        id: u64,
        ordinal: u64,
        change: Change,
        stage: Stage,
        relocation: Option<undo::Relocation>,
    },
    Preparing {
        id: u64,
        #[serde(with = "crate::path_serde")]
        destination: PathBuf,
        stage: Stage,
    },
    Disposing {
        disposal: Box<staging::disposal::Disposal>,
    },
    Cleanup {
        id: u64,
        ordinal: u64,
        #[serde(with = "crate::path_serde")]
        destination: PathBuf,
        stage: Stage,
    },
    UndoCleanup {
        cleanup: Box<store::reverse_cleanup::Cleanup>,
    },
    UndoSavedReturn {
        returning: Box<undo::saved_return::Return>,
    },
    UndoCopied {
        removal: Box<undo::copied::Removal>,
    },
    Forward {
        id: u64,
        ordinal: u64,
        change: Change,
        stage: Option<Stage>,
    },
    Undo {
        id: u64,
        ordinal: u64,
        change: Change,
        stage: Option<Stage>,
        #[serde(default)]
        relocation: Option<undo::Relocation>,
        #[serde(default)]
        displaced: Option<Box<recovery::Record>>,
    },
}

impl OperationEngine {
    /// Check the placement of interrupted work before startup reconciliation.
    /// A refused check leaves the history open and the interrupted data intact.
    pub fn open_with_recovery_check(
        path: impl AsRef<Path>,
        check: impl FnOnce(&[PathBuf]) -> io::Result<()>,
    ) -> io::Result<Self> {
        use rustix::fs::{Mode, OFlags, open};
        let path = std::path::absolute(path)?;
        let parent = path
            .parent()
            .ok_or_else(|| invalid("history requires a parent directory"))?;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
        let lock = history_lock::HistoryLock::acquire(&path.with_extension("lock"))?;
        let backup = path.with_extension("legacy-json");
        if is_legacy(&path)? {
            rename_no_replace(&path, &backup)?;
            sync_parent(&path)?;
        }
        let database = open(
            &path,
            OFlags::CREATE | OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR,
        )?;
        if !fs::File::from(database).metadata()?.is_file() {
            return Err(invalid("history database is not a regular file"));
        }
        let mut state = Store::open(&path)?;
        if !state.imported()? {
            let old = if backup.exists() {
                Some(backup)
            } else if path
                .file_name()
                .is_some_and(|name| name == "operations.sqlite3")
            {
                let candidate = path.with_file_name("operations.json");
                if is_legacy(&candidate)? {
                    Some(candidate)
                } else {
                    None
                }
            } else {
                None
            };
            let legacy = old
                .map(|path| crate::state_io::read_json::<LegacyJournal>(&path, LEGACY_LIMIT))
                .transpose()?;
            if let Some(legacy) = &legacy {
                validate_legacy(legacy)?;
            }
            state.import(legacy)?;
        }
        let paths: Vec<_> = state
            .interrupted()?
            .into_iter()
            .flat_map(|issue| issue.paths)
            .collect();
        let mut recovery = if paths.is_empty() {
            Ok(())
        } else {
            check(&paths)
        };
        if recovery.is_err() {
            state.recovery_result(&recovery);
            state.in_recovery_copy(|state| {
                state.recovery_result(&recovery);
                Ok(())
            })?;
        } else {
            recovery = recover(&mut state, &Cancellation::default());
        }
        if let Err(error) = recovery
            && !state.has_pending()?
        {
            return Err(error);
        }
        if !state.has_pending()? {
            state.trim()?;
        }
        sync_parent(&path)?;
        Ok(Self {
            reads: state.read_source(),
            state: Mutex::new(state),
            source_leases: Mutex::new(None),
            completion_scope: std::sync::Arc::new(()),
            _lock: lock,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::open_with_recovery_check(path, |_| Ok(()))
    }

    fn read_history(&self) -> io::Result<Store> {
        use std::sync::TryLockError;
        match self.state.try_lock() {
            // Anchor an idle snapshot before another mutation can start. An
            // unfinished journal left by a stopped job is recovery work even
            // if its error has not been classified by reconciliation yet.
            Ok(_idle) => self.reads.snapshot(false),
            Err(TryLockError::WouldBlock) => self.reads.snapshot(true),
            Err(TryLockError::Poisoned(_idle)) => self.reads.snapshot(false),
        }
    }

    pub fn latest_undo_id(&self) -> Option<u64> {
        self.read_history().ok()?.latest().ok()?.map(|(id, _)| id)
    }

    pub fn undo_label(&self) -> Option<String> {
        self.read_history()
            .ok()?
            .latest()
            .ok()?
            .map(|(_, label)| format!("Undo {label}"))
    }

    /// Read-only placement information for the native storage preflight. Rows
    /// remain paged through the same decoder used by Undo, without journal edits.
    pub(crate) fn undo_storage_paths(&self, expected: Option<u64>) -> io::Result<Vec<PathBuf>> {
        let state = self.read_history()?;
        let Some((id, _)) = state.latest()? else {
            return Ok(Vec::new());
        };
        if expected.is_some_and(|expected| expected != id) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "a more recent operation must be undone first",
            ));
        }
        let mut paths = Vec::new();
        let mut before = None;
        while let Some((ordinal, mut change)) = state.change_before(id, before)? {
            before = Some(ordinal);
            change.visit_paths(|path| paths.push(path.clone()));
        }
        Ok(paths)
    }

    pub fn execute(
        &self,
        request: OperationRequest,
        cancellation: &Cancellation,
    ) -> io::Result<OperationSummary> {
        self.execute_in_request(request, cancellation, None, None)
            .map(|outcome| outcome.summary)
    }

    fn execute_in_request(
        &self,
        request: OperationRequest,
        cancellation: &Cancellation,
        execution: Option<&requests::RequestRecord>,
        observed: Option<&transfer::ObservedSources>,
    ) -> io::Result<execution::MutationOutcome> {
        check_cancel(cancellation)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?;
        check_cancel(cancellation)?;
        recover(&mut state, cancellation)?;
        let mut rename_bindings = None;
        let (label, change, owned) = match request {
            OperationRequest::CreateFolder { path } => {
                validate_target(&path)?;
                let (stage, temporary) = Stage::prepare(path.parent().unwrap())?;
                fs::create_dir(stage.payload())?;
                let change = Change::CreatedFolder {
                    access_parent: None,
                    mode: None,
                    identity: Identity::read(&stage.payload())?,
                    path: path.clone(),
                };
                (
                    format!("create folder {}", display_name(&path)),
                    change,
                    Some((stage, temporary)),
                )
            }
            OperationRequest::Rename {
                source,
                destination,
            } => {
                validate_target(&source)?;
                validate_target(&destination)?;
                if source == destination {
                    return Err(invalid("source and destination are the same path"));
                }
                let label = format!(
                    "rename {} to {}",
                    display_name(&source),
                    display_name(&destination)
                );
                #[cfg(test)]
                rename_boundary::before_capture(&source, &destination);
                rename_bindings = observed
                    .map(|observed| observed.rename_bindings(&source, &destination))
                    .transpose()?;
                let identity = rename_bindings
                    .as_ref()
                    .map_or_else(|| Identity::read(&source), |bindings| Ok(bindings.source))?;
                let saved = state
                    .owned_saved_stage(&source, identity)?
                    .map(|record| {
                        saved_stage::Movement::capture(
                            &source,
                            &destination,
                            record.retained.stage.parent_access,
                            StageParentAccess::Existing,
                            None,
                        )
                    })
                    .transpose()?;
                let change = Change::Rename {
                    move_facts: None,
                    saved,
                    identity,
                    original: source,
                    current: destination,
                    retained: None,
                    reviewed: None,
                };
                (label, change, None)
            }
        };
        validate_change(&change)?;
        check_cancel(cancellation)?;
        let rename = match &change {
            Change::Rename {
                original,
                current,
                identity,
                ..
            } => Some(rename::Prepared::open(
                original,
                current,
                *identity,
                rename_bindings.as_ref(),
            )?),
            _ => None,
        };
        let id = state.begin_mutation(&label, execution, &change)?;
        let result = if let Some(rename) = rename {
            rename.commit(&mut state, id, &change)
        } else {
            commit_forward(
                &mut state,
                id,
                change.clone(),
                owned,
                |stage| match &change {
                    Change::CreatedFolder { path, .. } => {
                        rename_no_replace(&stage.unwrap().payload(), path)
                    }
                    Change::Rename { .. }
                    | Change::Copied { .. }
                    | Change::RetainedOnly { .. }
                    | Change::SourceRemoved { .. } => unreachable!(),
                    Change::RemovedDirectory { .. }
                    | Change::Trashed { .. }
                    | Change::Restored { .. } => unreachable!(),
                    Change::DirectoryMerge { .. } => unreachable!(),
                },
            )
        };
        if result.is_err() {
            state.discard_empty(id)?;
        }
        result?;
        state.trim()?;
        let summary = OperationSummary {
            id,
            label,
            paths: vec![change.forward_path().to_owned()],
        };
        self.mutation_outcome(&mut state, execution, summary, &change)
    }

    pub fn undo(&self, cancellation: &Cancellation) -> io::Result<OperationSummary> {
        undo::run(self, None, cancellation)
    }

    /// Targets a concrete entry, e.g. from a notification created before newer work.
    pub fn undo_operation(
        &self,
        id: u64,
        cancellation: &Cancellation,
    ) -> io::Result<OperationSummary> {
        undo::run(self, Some(id), cancellation)
    }

    /// Resolve conflicts on a worker; an optional ID protects stale UI actions.
    pub fn undo_interactive(
        &self,
        expected: Option<u64>,
        cancellation: &Cancellation,
        mut resolve: impl FnMut(UndoConflict) -> UndoChoice,
    ) -> io::Result<UndoOutcome> {
        undo::run_interactive(self, expected, cancellation, Some(&mut resolve))
    }
}

fn commit_forward(
    state: &mut Store,
    id: u64,
    change: Change,
    owned: Option<(Stage, tempfile::TempDir)>,
    mutation: impl FnOnce(Option<&Stage>) -> io::Result<()>,
) -> io::Result<()> {
    commit_forward_with_created(state, id, change, owned, None, mutation)
}

fn commit_forward_with_created(
    state: &mut Store,
    id: u64,
    mut change: Change,
    owned: Option<(Stage, tempfile::TempDir)>,
    opened_created: Option<&fs::File>,
    mutation: impl FnOnce(Option<&Stage>) -> io::Result<()>,
) -> io::Result<()> {
    let ordinal = state.next_ordinal(id)?;
    let stage = owned.as_ref().map(|(stage, _)| stage.clone());
    let created = match (&change, &stage, opened_created) {
        (
            Change::CreatedFolder {
                identity,
                mode: Some(_),
                ..
            },
            Some(_),
            Some(file),
        ) => {
            let metadata = file.metadata()?;
            if !metadata.is_dir() || Identity::from_metadata(&metadata) != *identity {
                return Err(invalid(
                    "The opened created folder does not match its journal.",
                ));
            }
            Some(file.try_clone()?)
        }
        (Change::CreatedFolder { mode: Some(_), .. }, Some(stage), None) => {
            Some(fs::File::open(stage.payload())?)
        }
        (_, _, Some(_)) => {
            return Err(invalid(
                "An opened created folder needs its folder journal.",
            ));
        }
        _ => None,
    };
    state.intent(&Pending::Forward {
        id,
        ordinal,
        change: change.clone(),
        stage: stage.clone(),
    })?;
    if let Some((_, temporary)) = owned {
        let _retained = temporary.keep();
    }
    if let Err(error) = mutation(stage.as_ref()) {
        if transfer::placement_failure(&error).is_some() {
            // Reconciliation also mutates paths. Keep the durable intent and
            // its owned data until the platform admits their storage again.
            let refused = Err(error);
            state.recovery_result(&refused);
            return refused;
        }
        let restored_source = restore_interrupted_source(state, id, ordinal, &change)?;
        if !restored_source && let Some(retained) = actual_retained(&change) {
            append_forward(
                state,
                id,
                ordinal,
                &Change::RetainedOnly {
                    path: change.forward_path().to_owned(),
                    retained,
                    replaced: None,
                },
                stage.as_ref(),
            )?;
            return Err(error);
        }
        if let Some(stage) = &stage {
            clean_stage(stage)?;
        }
        state.clear_intent()?;
        return Err(error);
    }
    restore_created_mode(&change, created.as_ref())?;
    refresh_retained(&mut change)?;
    sync_change(&change)?;
    if let Some(stage) = &stage
        && stage.identity.matches(&stage.directory)
    {
        durability::sync(&fs::File::open(&stage.directory)?)?;
    }
    append_forward(state, id, ordinal, &change, stage.as_ref())?;
    Ok(())
}

fn append_forward(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    stage: Option<&Stage>,
) -> io::Result<()> {
    if let Some(stage) = stage.filter(|stage| stage.ownership.is_some()) {
        // Append the completed change and retain its cleanup intent atomically.
        // Failed cleanup must not erase the only record of private work.
        let cleanup = state.append_with_cleanup(id, ordinal, change, stage)?;
        clean_stage(&cleanup)?;
        state.clear_intent()
    } else {
        state.append(id, ordinal, change)?;
        if let Some(stage) = stage {
            clean_stage(stage)?;
        }
        Ok(())
    }
}

fn recover(state: &mut Store, cancellation: &Cancellation) -> io::Result<()> {
    state.in_recovery_copy(|state| recover_current(state, cancellation))?;
    recover_current(state, cancellation)
}

fn recover_current(state: &mut Store, cancellation: &Cancellation) -> io::Result<()> {
    let result = state.require_bound_forward().and_then(|()| {
        state.with_directory_access(|state| {
            state.prepare_directory_access()?;
            recover_pending(state, cancellation)
        })
    });
    state.recovery_result(&result);
    result
}

fn recover_pending(state: &mut Store, cancellation: &Cancellation) -> io::Result<()> {
    let Some(pending) = state.pending()? else {
        return Ok(());
    };
    match pending {
        Pending::Expiry {
            entry,
            record,
            parent,
        } => state.recover_expiry(entry, &record, parent)?,
        Pending::StageMigration { migration } => saved_stage::recover(state, &migration)?,
        Pending::DirectoryDestination {
            id,
            ordinal,
            change,
            stage,
            relocation,
        } => {
            undo::directory_return::cross_filesystem::recover_destination(
                state,
                id,
                ordinal,
                &change,
                &stage,
                relocation.as_ref(),
            )?;
        }
        Pending::Preparing {
            id,
            destination,
            stage,
        } => {
            if state.promote_directory_preparing(&Pending::Preparing {
                id,
                destination: destination.clone(),
                stage: stage.clone(),
            })? {
                #[cfg(test)]
                transfer::directory_boundary::after_promotion(&destination);
                state.require_bound_forward()?;
                return recover_pending(state, cancellation);
            }
            validate_target(&destination)?;
            validate_stage(Some(&stage), &destination)?;
            staging::disposal::begin(state, id, destination, stage, cancellation)?;
        }
        Pending::Disposing { mut disposal } => {
            staging::disposal::recover(state, &mut disposal, cancellation)?
        }
        Pending::UndoCleanup { .. } => store::reverse_cleanup::recover(state)?,
        Pending::UndoSavedReturn { returning } => undo::saved_return::recover(state, *returning)?,
        Pending::UndoCopied { removal } => undo::copied::recover(state, *removal, cancellation)?,
        Pending::Cleanup {
            id,
            ordinal,
            destination,
            stage,
        } => {
            let before = ordinal
                .checked_add(1)
                .ok_or_else(|| invalid("Invalid cleanup step."))?;
            let (recorded, change) = state
                .change_before(id, Some(before))?
                .ok_or_else(|| invalid("The cleanup's committed step is unavailable."))?;
            if recorded != ordinal || change.forward_path() != destination {
                return Err(invalid("The cleanup does not match its committed step."));
            }
            validate_change(&change)?;
            validate_stage(Some(&stage), &destination)?;
            if stage.ownership.is_none() {
                return Err(invalid("A cleanup record needs its original stage facts."));
            }
            clean_stage(&staging::completed_cleanup(&stage)?)?;
            state.clear_intent()?;
        }
        Pending::Forward {
            id,
            ordinal,
            mut change,
            stage,
        } => {
            validate_change(&change)?;
            if let Some(stage) = &stage
                && stage.directory_claim.is_some()
            {
                return directory_removal::recover(state, id, ordinal, &change, stage);
            }
            if matches!(
                &change,
                Change::RemovedDirectory {
                    removal: Some(_),
                    ..
                }
            ) {
                return Err(directory_removal::interrupted());
            }
            if let Some(stage) = &stage
                && stage.entry_move.is_some()
            {
                if stage.source_claim.is_some() {
                    return Err(invalid("A private stage has conflicting operation roles."));
                }
                return entry_move::recover(state, id, ordinal, &change, stage);
            }
            if let Some(stage) = &stage
                && stage.replacement.is_some()
            {
                return replacement::recover_copy(state, id, ordinal, &change, stage);
            }
            if change
                .retained()
                .is_some_and(|retained| retained.replacement_facts.is_some())
            {
                return Err(invalid("A replacement step is missing its claim role."));
            }
            if matches!(
                &change,
                Change::Rename {
                    move_facts: Some(_),
                    ..
                }
            ) {
                return Err(invalid(
                    "A whole-entry Move step is missing its claim role.",
                ));
            }
            if let Some(stage) = &stage
                && stage.source_claim.is_some()
            {
                return source_removal::recover(state, id, ordinal, &change, stage);
            }
            if matches!(change, Change::CreatedFolder { mode: Some(_), .. }) {
                return transfer::directory::recover_created(
                    state,
                    id,
                    ordinal,
                    &change,
                    stage.as_ref(),
                );
            }
            if matches!(change, Change::Copied { retained: None, .. }) {
                return staging::recover_copy(state, id, ordinal, &change, stage.as_ref());
            }
            if matches!(change, Change::Rename { .. }) && stage.is_some() {
                validate_claim_stage(stage.as_ref(), change.forward_path())?;
                // A Rename claim can hold an unexpected entry. Never run the
                // generic stage cleanup or infer a completed rename from it.
                return Err(rename::interrupted());
            }
            validate_stage(stage.as_ref(), change.forward_path())?;
            if restore_interrupted_source(state, id, ordinal, &change)? {
                if let Some(stage) = stage {
                    clean_stage(&stage)?;
                }
                return Ok(());
            }
            trash::recover_forward(&change)?;
            if change.committed() {
                restore_created_mode(&change, None)?;
                refresh_retained(&mut change)?;
                sync_change(&change)?;
                append_forward(state, id, ordinal, &change, stage.as_ref())?;
            } else if let Some(retained) = actual_retained(&change) {
                append_forward(
                    state,
                    id,
                    ordinal,
                    &Change::RetainedOnly {
                        path: change.forward_path().to_owned(),
                        retained,
                        replaced: None,
                    },
                    stage.as_ref(),
                )?;
            } else {
                if let (
                    Change::CreatedFolder {
                        mode: Some(_),
                        identity,
                        ..
                    },
                    Some(stage),
                ) = (&change, &stage)
                    && !identity.matches(&stage.payload())
                {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "return the original created folder before restoring its permissions",
                    ));
                }
                if let Some(stage) = &stage {
                    clean_stage(stage)?;
                }
                state.clear_intent()?;
            }
        }
        Pending::Undo {
            id,
            ordinal,
            change,
            stage,
            relocation,
            displaced,
        } => {
            undo::recover_reverse(
                state,
                id,
                ordinal,
                &change,
                stage.as_ref(),
                relocation.as_ref(),
                displaced.as_deref(),
                cancellation,
            )?;
        }
    }
    Ok(())
}

fn restore_created_mode(change: &Change, opened: Option<&fs::File>) -> io::Result<()> {
    if let Change::CreatedFolder {
        path,
        identity,
        mode: Some(mode),
        ..
    } = change
    {
        if mode & !0o777 != 0 {
            return Err(invalid("invalid copied folder mode"));
        }
        restore_directory_mode(path, *identity, *mode, opened)?;
    }
    Ok(())
}

fn restore_directory_mode(
    path: &Path,
    identity: Identity,
    mode: u32,
    opened: Option<&fs::File>,
) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if mode & !0o7777 != 0 {
        return Err(invalid("invalid returned folder mode"));
    }
    let file = match opened {
        Some(file) => file.try_clone()?,
        None => fs::OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::NOFOLLOW).bits() as i32,
            )
            .open(path)?,
    };
    let metadata = file.metadata()?;
    if !metadata.is_dir() || Identity::from_metadata(&metadata) != identity {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the copied or returned folder has been replaced",
        ));
    }
    if metadata.mode() & 0o7777 == mode | 0o700 {
        file.set_permissions(fs::Permissions::from_mode(mode))?;
        durability::sync(&file)?;
    }
    Ok(())
}

fn clean_stage(stage: &Stage) -> io::Result<()> {
    if stage.directory_claim.is_some() {
        return Err(directory_removal::interrupted());
    }
    if stage.replacement.is_some() {
        return Err(replacement::interrupted());
    }
    if stage.entry_move.is_some() {
        return Err(entry_move::interrupted());
    }
    if stage.source_claim.is_some() {
        return Err(source_removal::interrupted());
    }
    if stage.sibling_payload {
        return Err(invalid(
            "a directory Rename claim is not disposable staging",
        ));
    }
    if let Some(ownership) = &stage.ownership {
        return staging::clean(stage, ownership);
    }
    if !stage.identity.matches(&stage.directory) {
        return Ok(());
    }
    let payload = stage.payload();
    match fs::symlink_metadata(&payload) {
        Ok(metadata) => {
            if metadata.is_dir() {
                fs::remove_dir(payload)?;
            } else {
                fs::remove_file(payload)?;
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    if fs::symlink_metadata(stage.directory.join("previous")).is_ok() {
        return Ok(());
    }
    fs::remove_dir(&stage.directory)?;
    sync_parent(&stage.directory)
}

fn actual_retained(change: &Change) -> Option<Retained> {
    let mut retained = change.retained()?.clone();
    if !retained.stage.identity.matches(&retained.stage.directory) {
        return None;
    }
    let actual = Identity::read(&retained.path()).ok()?;
    retained.recovery |= retained.identity != actual;
    retained.identity = actual;
    retained.recovery |= retained
        .version
        .as_ref()
        .is_some_and(|version| !version.matches_moved(&retained.path()));
    retained.version = fingerprint::Version::read(&retained.path()).ok();
    Some(retained)
}

fn refresh_retained(change: &mut Change) -> io::Result<()> {
    let retained = match change {
        Change::Rename {
            retained: Some(retained),
            ..
        }
        | Change::Copied {
            retained: Some(retained),
            ..
        }
        | Change::RetainedOnly { retained, .. } => retained,
        _ => return Ok(()),
    };
    let version = fingerprint::Version::read(&retained.path())?;
    retained.recovery |= retained.original_identity() != version.identity
        || retained
            .version
            .as_ref()
            .is_some_and(|previous| !previous.matches_moved(&retained.path()));
    retained.identity = version.identity;
    retained.version = Some(version);
    Ok(())
}

fn restore_interrupted_source(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<bool> {
    let Change::SourceRemoved {
        original,
        quarantine,
        ..
    } = change
    else {
        return Ok(false);
    };
    if quarantine.parent_identity.is_some_and(|identity| {
        !fs::metadata(original.parent().unwrap())
            .is_ok_and(|metadata| Identity::from_metadata(&metadata) == identity)
    }) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the source filesystem is unavailable; its move must resume when it returns",
        ));
    }
    let identity = match Identity::read(&quarantine.payload()) {
        Ok(identity) => identity,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    match rename_no_replace(&quarantine.payload(), original) {
        Ok(()) => {
            sync_parent(original)?;
            state.clear_intent()?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let retained = Retained {
                replacement_facts: None,
                physical_identity: None,
                stage: quarantine.clone(),
                identity,
                recovery: true,
                version: None,
            };
            rename_no_replace(&quarantine.payload(), &retained.path())?;
            state.append(
                id,
                ordinal,
                &Change::RetainedOnly {
                    path: original.clone(),
                    retained,
                    replaced: None,
                },
            )?;
        }
        Err(error) => return Err(error),
    }
    Ok(true)
}

/// Check cleanup eligibility without changing data or catalog ownership.
/// `false` means the payload is already absent from an otherwise valid stage.
fn require_retained_expiry(retained: &Retained) -> io::Result<bool> {
    if retained.recovery {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "unexpected retained work needs explicit recovery before it can be removed",
        ));
    }
    if retained
        .version
        .as_ref()
        .is_some_and(|version| !version.matches(&retained.path()))
    {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "retained data contains newer work requiring explicit recovery",
        ));
    }
    validate_stage(Some(&retained.stage), &retained.stage.directory)?;
    if !retained.stage.identity.matches(&retained.stage.directory) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "retained data needs manual recovery",
        ));
    }
    saved_stage::require_only(&retained.stage, &["previous"])?;
    match fs::symlink_metadata(retained.path()) {
        Ok(metadata) if retained.original_identity().matches(&retained.path()) => {
            if metadata.is_file() || metadata.is_symlink() {
                Ok(true)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the retained payload needs explicit recovery",
                ))
            }
        }
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "retained data has been replaced",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn install_replacing(
    destination: &Path,
    retained: Option<&Retained>,
    install: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    if let Some(retained) = retained {
        if retained.recovery
            && retained
                .stage
                .ownership
                .as_ref()
                .is_some_and(|owner| owner.previous_origin().is_some())
        {
            return undo::displaced::install(destination, retained, install);
        }
        if retained
            .version
            .as_ref()
            .is_some_and(|version| !version.matches(destination))
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "the destination changed while the conflict was being resolved",
            ));
        }
        rename_no_replace(destination, &retained.path())?;
        if !retained.identity.matches(&retained.path())
            || retained
                .version
                .as_ref()
                .is_some_and(|version| !version.matches_moved(&retained.path()))
        {
            let _rollback = rename_no_replace(&retained.path(), destination);
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "the destination changed while the conflict was being resolved",
            ));
        }
    }
    if let Err(error) = install() {
        if let Some(retained) = retained {
            let _rollback = rename_no_replace(&retained.path(), destination);
        }
        return Err(error);
    }
    Ok(())
}
fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(test)]
    rename_boundary::before_rename(source, destination);
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        source,
        rustix::fs::CWD,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(Into::into)
}
fn sync_parent(path: &Path) -> io::Result<()> {
    fs::File::open(path.parent().ok_or_else(|| invalid("missing parent"))?)?.sync_all()
}
fn sync_change(change: &Change) -> io::Result<()> {
    if let Change::DirectoryMerge {
        change,
        defer_previous: true,
        ..
    } = change
    {
        sync_parent(change.forward_path())?;
        return sync_parent(
            change
                .reverse_path()
                .ok_or_else(|| invalid("missing Merge destination"))?,
        );
    }
    if let Some(retained) = change.retained() {
        fs::File::open(&retained.stage.directory)?.sync_all()?;
    }
    match change {
        marker @ Change::DirectoryMerge { change, .. }
            if matches!(change.as_ref(), Change::RemovedDirectory { .. }) =>
        {
            let (source, destination, _, _) = undo::directory_return::roots(marker)?;
            sync_parent(source)?;
            sync_parent(destination)
        }
        Change::DirectoryMerge { change, .. } => sync_change(change),
        // New items only; a deferred span batches this barrier (ADR0165).
        Change::CreatedFolder { path, .. } | Change::Copied { path, .. } => {
            durability::sync_parent(path)
        }
        Change::RetainedOnly { path, .. } | Change::RemovedDirectory { path, .. } => {
            sync_parent(path)
        }
        Change::Rename {
            original, current, ..
        } => {
            sync_parent(original)?;
            if original.parent() != current.parent() {
                sync_parent(current)?;
            }
            Ok(())
        }
        Change::Trashed {
            original,
            current,
            info,
            ..
        }
        | Change::Restored {
            original,
            current,
            info,
            ..
        } => {
            sync_parent(original)?;
            sync_parent(current)?;
            info.sync()
        }
        Change::SourceRemoved { original, .. } => sync_parent(original),
    }
}
fn check_cancel(cancellation: &Cancellation) -> io::Result<()> {
    if cancellation.is_cancelled() {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "operation cancelled",
        ))
    } else {
        Ok(())
    }
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn display_name(path: &Path) -> std::borrow::Cow<'_, str> {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
}
fn validate_target(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || !matches!(path.components().next_back(), Some(Component::Normal(_))) {
        return Err(invalid("operation targets must be absolute named paths"));
    }
    Ok(())
}
fn validate_change(change: &Change) -> io::Result<()> {
    if let Some(retained) = change.retained() {
        validate_stage(Some(&retained.stage), change.forward_path())?;
    }
    match change {
        Change::DirectoryMerge {
            change,
            defer_previous,
            destination_owned,
            ..
        } => {
            if *destination_owned
                && !matches!(
                    change.as_ref(),
                    Change::RemovedDirectory { moved: Some(_), .. }
                )
            {
                return Err(invalid("invalid owned directory return destination"));
            }
            if *defer_previous
                && !matches!(
                    change.as_ref(),
                    Change::Rename {
                        retained: Some(_),
                        ..
                    }
                )
            {
                return Err(invalid("invalid source-only directory return"));
            }
            if !matches!(
                change.as_ref(),
                Change::Rename { .. }
                    | Change::Trashed { .. }
                    | Change::RemovedDirectory { moved: Some(_), .. }
            ) {
                return Err(invalid("invalid directory return"));
            }
            validate_change(change)
        }
        Change::Trashed {
            original,
            current,
            info,
            ..
        } => {
            validate_target(original)?;
            validate_target(current)?;
            info.validate(current)
        }
        Change::Restored {
            original,
            current,
            info,
            ..
        } => {
            validate_target(original)?;
            validate_target(current)?;
            info.validate(original)
        }
        Change::CreatedFolder { path, .. }
        | Change::Copied { path, .. }
        | Change::RetainedOnly { path, .. } => validate_target(path),
        Change::RemovedDirectory { path, moved, .. } => {
            validate_target(path)?;
            if let Some(moved) = moved {
                validate_target(&moved.current)?;
                if moved.created.is_some_and(|created| created != moved.first) {
                    return Err(invalid("invalid moved directory creation step"));
                }
            }
            Ok(())
        }
        Change::Rename {
            original,
            current,
            saved,
            retained,
            ..
        } => {
            validate_target(original)?;
            if let Some(saved) = saved {
                if retained.is_some()
                    || ![original, current].into_iter().all(|path| {
                        path.file_name().is_some_and(|name| {
                            name.as_encoded_bytes().starts_with(b".canopa-files-stage-")
                        })
                    })
                {
                    return Err(invalid("invalid saved-container movement"));
                }
                if let Some(context) = &saved.context {
                    validate_target(&context.from)?;
                    validate_target(&context.to)?;
                    if context.from == context.to
                        || original.parent() != Some(context.from.as_path())
                        || current.parent() != Some(context.to.as_path())
                    {
                        return Err(invalid("invalid saved-container context movement"));
                    }
                }
            }
            validate_target(current)
        }
        Change::SourceRemoved {
            original,
            current,
            quarantine,
            ..
        } => {
            validate_target(original)?;
            validate_target(current)?;
            validate_stage(Some(quarantine), original)
        }
    }
}
fn is_false(value: &bool) -> bool {
    !*value
}
fn validate_stage(stage: Option<&Stage>, target: &Path) -> io::Result<()> {
    if stage.is_some_and(|stage| stage.sibling_payload) {
        return Err(invalid(
            "a sibling payload is only valid for a Rename claim",
        ));
    }
    validate_claim_stage(stage, target)
}

fn validate_claim_stage(stage: Option<&Stage>, target: &Path) -> io::Result<()> {
    if let Some(stage) = stage {
        validate_target(&stage.directory)?;
        if !stage
            .directory
            .file_name()
            .is_some_and(|name| name.as_encoded_bytes().starts_with(b".canopa-files-stage-"))
            || stage.directory.parent() != target.parent()
        {
            return Err(invalid("invalid operation stage"));
        }
    }
    Ok(())
}
fn is_legacy(path: &Path) -> io::Result<bool> {
    let mut file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !file.metadata()?.is_file() {
        return Err(invalid("history is not a regular file"));
    }
    let mut header = [0; 16];
    let length = file.read(&mut header)?;
    if length == 0 || &header == b"SQLite format 3\0" {
        return Ok(false);
    }
    if header[..length]
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        == Some(b'{')
    {
        Ok(true)
    } else {
        Err(invalid("unrecognized operation history"))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyJournal {
    version: u32,
    next_id: u64,
    entries: Vec<LegacyRecord>,
    pending: Option<LegacyPending>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyRecord {
    id: u64,
    label: String,
    change: Change,
}
#[derive(Deserialize)]
#[serde(tag = "direction", rename_all = "kebab-case", deny_unknown_fields)]
enum LegacyPending {
    Forward {
        record: Box<LegacyRecord>,
        stage: Option<Stage>,
    },
    Undo {
        id: u64,
    },
}
fn validate_legacy(legacy: &LegacyJournal) -> io::Result<()> {
    if legacy.version != 1
        || legacy.next_id == 0
        || legacy.next_id > i64::MAX as u64
        || legacy.entries.len() > HISTORY_LIMIT
    {
        return Err(invalid("unsupported legacy operation history"));
    }
    let mut previous = 0;
    for record in &legacy.entries {
        if record.id <= previous || record.id >= legacy.next_id || record.label.len() > 4096 {
            return Err(invalid("invalid legacy operation order"));
        }
        previous = record.id;
        validate_change(&record.change)?;
    }
    if let Some(LegacyPending::Forward { record, stage }) = &legacy.pending {
        if record.id == 0 || record.id >= legacy.next_id {
            return Err(invalid("invalid legacy intent"));
        }
        validate_change(&record.change)?;
        validate_stage(stage.as_ref(), record.change.forward_path())?;
    }
    Ok(())
}
