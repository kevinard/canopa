//! Inspect retained versions and recover copies without consuming Undo data.
use super::*;
use crate::browse::Entry;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) enum Key {
    Pending(u8),
    Active { operation: u64, ordinal: u64 },
    Archived(u64),
}
#[derive(Clone, Debug)]
pub struct RecoveryCursor(pub(super) Key);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStatus {
    UndoBackup,
    NeedsRecovery,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryItem {
    #[serde(with = "crate::path_serde::optional")]
    pub original: Option<PathBuf>,
    pub label: String,
    pub entry: Option<Entry>,
    pub status: RecoveryStatus,
    pub message: String,
    pub held_for_undo: bool,
    key: Key,
    saved: SavedIdentity,
    observed: Option<fingerprint::Version>,
    recorded_location: bool,
    /// Fresh review of the exact journal, never restored as execution authority.
    #[serde(skip)]
    claim_review: Option<String>,
    #[serde(skip)]
    disposal_review: Option<std::sync::Arc<store::disposal_review::Review>>,
    #[serde(skip)]
    directory_reads: Vec<std::sync::Arc<store::directory_reads::Lease>>,
    #[serde(skip)]
    undo_review: Option<std::sync::Arc<store::undo_work::Review>>,
    #[serde(skip)]
    undo_cleanup_review: Option<std::sync::Arc<store::reverse_cleanup::Review>>,
    #[serde(skip)]
    copy_undo_review: Option<std::sync::Arc<store::copy_undo::Review>>,
    #[serde(skip)]
    saved_return_review: Option<std::sync::Arc<store::saved_return::Review>>,
}

/// A catalog row can expire while its protected bytes remain owned elsewhere.
/// Selection follows the same saved container, logical item and payload role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedIdentity {
    stage: Identity,
    logical: Identity,
    payload: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    container: bool,
}
/// Presentation identity across owner expiry and identified container movement.
/// It neither resolves a source nor grants execution authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RecoveryIdentity(SavedIdentity);

/// Prepared-content identity. A changed payload must invalidate its preview even
/// when its display path and original filename stay the same.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryVersion {
    item: SavedIdentity,
    observed: fingerprint::Version,
}
/// Exact in-memory remaining Recovery work. It cannot be resumed as an ordinary
/// transfer or authorize replacing work created in its destination meanwhile.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryCopyContinuation {
    item: RecoveryItem,
    #[serde(with = "crate::path_serde")]
    source: PathBuf,
    #[serde(with = "crate::path_serde")]
    destination: PathBuf,
    parent_identity: Identity,
    transfer: transfer::TransferContinuation,
}
pub type RecoveryCopyOutcome = transfer::TransferOutcome<RecoveryCopyContinuation>;
impl RecoveryCopyContinuation {
    /// Keep the same exact output and source ownership through a preflight
    /// refusal; a descriptive path list is never used to rebuild the plan.
    pub(crate) fn paused(&self, message: String) -> RecoveryCopyOutcome {
        self.transfer
            .clone()
            .paused(message)
            .map_continuations(|transfer| Self {
                transfer,
                ..self.clone()
            })
    }
    pub(crate) fn destination(&self) -> &Path {
        &self.destination
    }
    pub(crate) fn item(&self) -> &RecoveryItem {
        &self.item
    }
    pub(crate) fn storage_destinations(&self) -> impl Iterator<Item = PathBuf> + '_ {
        std::iter::once(self.destination.clone()).chain(self.transfer.storage_destinations())
    }
}

#[derive(Clone, Debug)]
pub struct RecoveryPage {
    pub items: Vec<RecoveryItem>,
    pub next: Option<RecoveryCursor>,
    pub interrupted: Vec<InterruptedOperation>,
    #[cfg_attr(not(feature = "local-runtime"), allow(dead_code))]
    pub(crate) observation_paths: Vec<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Context {
    #[serde(with = "crate::path_serde")]
    pub original: PathBuf,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ContextStep {
    pub operation: u64,
    pub ordinal: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ContextMove {
    #[serde(with = "crate::path_serde")]
    pub from: PathBuf,
    #[serde(with = "crate::path_serde")]
    pub to: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_step: Option<ContextStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_step: Option<ContextStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_identity: Option<Identity>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub retained: Retained,
    pub context: Option<Context>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_moves: Vec<ContextMove>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub payload: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub container: bool,
}
impl Record {
    pub(super) fn protected_paths(&self) -> Vec<PathBuf> {
        let mut paths = vec![self.retained.stage.directory.clone()];
        if self.payload && self.retained.stage.sibling_payload {
            paths.push(self.path());
        }
        paths
    }
    fn saved_identity(&self) -> SavedIdentity {
        SavedIdentity {
            stage: self.retained.stage.identity,
            logical: self.retained.identity,
            payload: self.payload,
            container: self.container,
        }
    }
    pub fn from_change(change: &Change, label: String) -> Option<Self> {
        let mut owner = change;
        while let Change::DirectoryMerge { change, .. } = owner {
            owner = change;
        }
        let cleanup_parent = match owner {
            Change::Copied { access_parent, .. } => *access_parent,
            _ => None,
        };
        let mut retained = change.retained()?.clone();
        // Legacy metadata grants only its explicitly captured original physical
        // copied parent. A new Existing marker deliberately suppresses fallback.
        if retained.stage.parent_access.is_none()
            && cleanup_parent == retained.stage.parent_identity
            && let Some(parent) = cleanup_parent
        {
            retained.stage.record_parent_access(Some(parent));
        }
        Some(Self {
            context_moves: Vec::new(),
            payload: false,
            container: false,
            retained,
            context: Some(Context {
                original: change.forward_path().to_owned(),
                label,
            }),
        })
    }
    pub fn decode(body: &str) -> io::Result<Self> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Stored {
            Current(Record),
            Legacy(Retained),
        }
        match store::decode(body)? {
            Stored::Current(record)
                if (!record.payload || record.retained.recovery)
                    && (!record.container
                        || (record.payload
                            && !record.retained.stage.sibling_payload
                            && record.retained.identity == record.retained.stage.identity
                            && record.retained.stage.ownership.is_some())) =>
            {
                Ok(record)
            }
            Stored::Current(_) => Err(invalid("a Recovery payload must remain protected")),
            Stored::Legacy(retained) => Ok(Self {
                context_moves: Vec::new(),
                payload: false,
                container: false,
                retained,
                context: None,
            }),
        }
    }
    pub fn path(&self) -> PathBuf {
        if self.container {
            self.retained.stage.directory.clone()
        } else if self.payload {
            self.retained.stage.payload()
        } else {
            self.retained.path()
        }
    }
    pub fn relocate(&mut self, relocation: &undo::Relocation) {
        self.visit_paths(|path| relocation.apply(path));
    }
    pub fn visit_paths(&mut self, mut visit: impl FnMut(&mut PathBuf)) {
        visit(&mut self.retained.stage.directory);
        if let Some(context) = &mut self.context {
            visit(&mut context.original);
        }
        for movement in &mut self.context_moves {
            visit(&mut movement.from);
            visit(&mut movement.to);
        }
    }
    /// Follow a committed namespace rename without changing historical Undo paths.
    pub fn follow(
        &mut self,
        step: ContextStep,
        change: &Change,
        reverse: bool,
        file_owner: bool,
        mut parent: impl FnMut(&ContextMove) -> io::Result<Option<Identity>>,
    ) -> io::Result<()> {
        // A container can move before its associated user file. Remember only
        // the contexts whose identified container followed that movement; a
        // later committed user-file/folder step changes their displayed origin.
        let owner_change = match change {
            Change::DirectoryMerge { change, .. } => change.as_ref(),
            change => change,
        };
        let owner = match owner_change {
            Change::SourceRemoved {
                original, current, ..
            }
            | Change::Rename {
                original,
                current,
                saved: None,
                ..
            } => Some((original.as_path(), current.as_path(), None)),
            Change::RemovedDirectory {
                path,
                moved: Some(moved),
                identity,
                ..
            } => Some((path.as_path(), moved.current.as_path(), Some(*identity))),
            _ => None,
        };
        if let Some((original, current, directory_identity)) = owner
            && let Some(context) = &mut self.context
        {
            for movement in self.context_moves.iter_mut().rev() {
                if movement.container_step.is_none()
                    || (reverse && movement.owner_step != Some(step))
                {
                    continue;
                }
                let directory = directory_identity.is_some();
                let matches = if directory {
                    original == movement.from && current == movement.to
                } else {
                    original.parent() == Some(movement.from.as_path())
                        && current.parent() == Some(movement.to.as_path())
                };
                let (from, to) = if reverse {
                    (current, original)
                } else {
                    (original, current)
                };
                if matches
                    && (context.original == from
                        || (directory && context.original.starts_with(from)))
                {
                    if !reverse {
                        if let Some(identity) = directory_identity {
                            if parent(movement)? != Some(identity) {
                                continue;
                            }
                        } else if !file_owner {
                            continue;
                        }
                    }
                    context.original = if directory {
                        to.join(context.original.strip_prefix(from).unwrap())
                    } else {
                        to.to_owned()
                    };
                    if !reverse {
                        movement.owner_step = Some(step);
                    }
                    break;
                }
            }
        }
        let (original, current) = match change {
            Change::Rename {
                original, current, ..
            }
            | Change::Trashed {
                original, current, ..
            }
            | Change::Restored {
                original, current, ..
            } => (original, current),
            _ => return Ok(()),
        };
        let (from, to) = if reverse {
            (current, original)
        } else {
            (original, current)
        };
        // A later rename/Trash of the replacement itself leaves its sibling
        // backup in place. Keep that backup's filename, including its format
        // hint, instead of adopting the replacement's new/storage basename.
        if self.retained.stage.directory.starts_with(from) {
            if let Change::Rename {
                saved: Some(saved), ..
            } = change
                && let Some(context) = &saved.context
                && let Some(record_context) = &mut self.context
            {
                if reverse {
                    if self
                        .context_moves
                        .last()
                        .is_some_and(|movement| movement.container_step == Some(step))
                    {
                        self.context_moves.pop();
                        if let Ok(tail) = record_context.original.strip_prefix(&context.to) {
                            record_context.original = context.from.join(tail);
                        }
                    }
                } else if record_context.original.starts_with(&context.from) {
                    self.context_moves.push(ContextMove {
                        from: context.from.clone(),
                        to: context.to.clone(),
                        container_step: Some(step),
                        owner_step: None,
                        parent_identity: Some(saved.parent_identity),
                    });
                }
            }
            self.relocate(&undo::Relocation {
                from: from.clone(),
                to: to.clone(),
                subtree: true,
            });
        }
        Ok(())
    }
    pub(super) fn available(&self) -> io::Result<fingerprint::Version> {
        if self.container {
            return staging::disposal::container_version(&self.retained.stage);
        }
        if self.payload {
            validate_claim_stage(Some(&self.retained.stage), &self.retained.stage.directory)?;
        } else {
            validate_stage(Some(&self.retained.stage), &self.retained.stage.directory)?;
        }
        if !self
            .retained
            .stage
            .identity
            .matches(&self.retained.stage.directory)
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "the retained folder is unavailable or has been replaced",
            ));
        }
        fingerprint::Version::read(&self.path())
    }
}
impl RecoveryItem {
    /// Only a fresh review of available interrupted work can retire its journal.
    pub fn can_keep_in_recovery(&self) -> bool {
        (self.claim_review.is_some()
            || self.disposal_review.is_some()
            || self.undo_cleanup_review.is_some()
            || self.copy_undo_review.is_some()
            || self.saved_return_review.is_some()
            || self.undo_review.is_some())
            && self.entry.is_some()
    }
    pub fn identity(&self) -> RecoveryIdentity {
        RecoveryIdentity(self.saved)
    }
    pub fn version(&self) -> Option<RecoveryVersion> {
        self.entry.as_ref()?;
        Some(RecoveryVersion {
            item: self.saved,
            observed: self.observed.clone()?,
        })
    }
    pub(crate) fn cancelled_copy(
        &self,
        continuation: Option<&RecoveryCopyContinuation>,
    ) -> RecoveryCopyOutcome {
        let mut outcome = match continuation {
            Some(plan) => plan.paused("Recovery copy cancelled before more work started.".into()),
            None => RecoveryCopyOutcome {
                remaining: self.entry.iter().map(|entry| entry.path.clone()).collect(),
                ..Default::default()
            },
        };
        outcome.cancelled = true;
        outcome.needs_attention = false;
        outcome.attention = None;
        outcome
    }
    fn resolve(&self, state: &Store) -> io::Result<Record> {
        if let Some(record) = state.recovery_record(&self.key)? {
            require_observed(self, &record)?;
            return Ok(record);
        }
        let mut unavailable = None;
        let record = state.find_saved_recovery(self.saved.stage, |record| {
            if record.saved_identity() != self.saved {
                return Ok(false);
            }
            match require_observed(self, record) {
                Ok(()) => Ok(true),
                Err(error) => {
                    unavailable = Some(error);
                    Ok(false)
                }
            }
        })?;
        record.ok_or_else(|| {
            unavailable.unwrap_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "the selected retained version is no longer available; refresh Recovery",
                )
            })
        })
    }
    /// Whether `original` is a historical hint without current movement proof.
    pub fn original_is_recorded(&self) -> bool {
        self.recorded_location
    }
    pub fn name(&self) -> &std::ffi::OsStr {
        if self.saved.container {
            return std::ffi::OsStr::new("Working files");
        }
        self.original
            .as_deref()
            .and_then(Path::file_name)
            .unwrap_or_else(|| std::ffi::OsStr::new("Retained file"))
    }
    fn inspect(key: Key, record: Record) -> Self {
        let saved = record.saved_identity();
        let recorded_location = record.context_moves.iter().any(|movement| {
            movement.container_step.is_none() || movement.parent_identity.is_none()
        });
        let held_for_undo = !matches!(key, Key::Archived(_));
        let observed = record.available();
        let (status, message) = match &observed {
            Err(error) => (RecoveryStatus::Unavailable, error.to_string()),
            Ok(_) if matches!(key, Key::Pending(_)) => (RecoveryStatus::NeedsRecovery, "Kept by an interrupted operation. Recovering a copy leaves the saved version intact.".into()),
            Ok(version) if record.retained.version.as_ref().is_some_and(|expected| expected != version) || record.retained.original_identity() != version.identity =>
                (RecoveryStatus::NeedsRecovery, "This retained version contains newer or unexpected work. It is protected from automatic cleanup.".into()),
            Ok(_) if record.retained.recovery =>
                (RecoveryStatus::NeedsRecovery, "This version is protected from automatic cleanup. Recovering a copy leaves it intact.".into()),
            Ok(_) if held_for_undo => (RecoveryStatus::UndoBackup, "Still needed by an Undo entry. Recovering a copy leaves it available to Undo.".into()),
            Ok(_) => (RecoveryStatus::NeedsRecovery, "Kept after its Undo entry expired. Recovering a copy leaves the retained version intact.".into()),
        };
        let entry = observed
            .as_ref()
            .ok()
            .and_then(|_| Entry::read(record.path()).ok());
        let (original, label) = record
            .context
            .map_or((None, "Retained file".into()), |context| {
                (Some(context.original), context.label)
            });
        Self {
            saved,
            recorded_location,
            original,
            label,
            entry,
            status,
            message,
            held_for_undo,
            key,
            observed: observed.ok(),
            claim_review: None,
            disposal_review: None,
            directory_reads: Vec::new(),
            undo_review: None,
            undo_cleanup_review: None,
            copy_undo_review: None,
            saved_return_review: None,
        }
    }
}

impl OperationEngine {
    /// Retire exactly reviewed interrupted work after its data has a durable,
    /// nonexpiring Recovery owner. No filesystem work is replayed.
    pub fn keep_in_recovery(
        &self,
        item: &RecoveryItem,
        cancellation: &Cancellation,
    ) -> io::Result<()> {
        check_cancel(cancellation)?;
        if let Some(reviewed) = &item.saved_return_review {
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("operation history failed"))?;
            check_cancel(cancellation)?;
            let record = state
                .recovery_record(&item.key)?
                .ok_or_else(|| invalid("The saved return work is unavailable."))?;
            require_observed(item, &record)?;
            return state.keep_saved_return(reviewed);
        }
        if let Some(reviewed) = &item.copy_undo_review {
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("operation history failed"))?;
            check_cancel(cancellation)?;
            let record = state
                .recovery_record(&item.key)?
                .ok_or_else(|| invalid("The interrupted Copy Undo work is unavailable."))?;
            require_observed(item, &record)?;
            return state.keep_copy_undo(reviewed);
        }
        if let Some(reviewed) = &item.undo_cleanup_review {
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("operation history failed"))?;
            check_cancel(cancellation)?;
            let record = state
                .recovery_record(&item.key)?
                .ok_or_else(|| invalid("The completed Undo work is unavailable."))?;
            require_observed(item, &record)?;
            return state.keep_undo_cleanup(reviewed);
        }
        if let Some(reviewed) = &item.undo_review {
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("operation history failed"))?;
            check_cancel(cancellation)?;
            let record = state
                .recovery_record(&item.key)?
                .ok_or_else(|| invalid("The interrupted Undo work is unavailable."))?;
            require_observed(item, &record)?;
            return state.keep_undo_work(reviewed);
        }
        if let Some(reviewed) = &item.disposal_review {
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("operation history failed"))?;
            check_cancel(cancellation)?;
            let record = state
                .recovery_record(&item.key)?
                .ok_or_else(|| invalid("The interrupted work is unavailable."))?;
            require_observed(item, &record)?;
            return state.keep_disposal(reviewed);
        }
        let reviewed = item
            .claim_review
            .as_deref()
            .filter(|_| matches!(item.key, Key::Pending(0)))
            .ok_or_else(|| invalid("Refresh Recovery and select an interrupted Rename entry."))?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?;
        check_cancel(cancellation)?;
        if state.rename_claim_review()?.as_deref() != Some(reviewed) {
            return Err(invalid(
                "Interrupted work changed; refresh Recovery before keeping it.",
            ));
        }
        let mut record = state
            .recovery_record(&item.key)?
            .ok_or_else(|| invalid("The interrupted Rename entry is unavailable."))?;
        require_observed(item, &record)?;
        record.retained.recovery = true;
        record.retained.version = item.observed.clone();
        state.keep_rename_claim(reviewed, &record)
    }

    /// Resolve the selected record again after an identified ancestor moves.
    /// The display path captured by Recovery is not placement authority.
    pub(crate) fn recovery_copy_source_path(&self, item: &RecoveryItem) -> io::Result<PathBuf> {
        let state = self.read_history()?;
        let record = item.resolve(&state)?;
        Ok(record.path())
    }
    /// Returns at most 256 versions; use `next` to inspect the remainder.
    pub fn recovery_items(
        &self,
        cursor: Option<RecoveryCursor>,
        cancellation: &Cancellation,
    ) -> io::Result<RecoveryPage> {
        check_cancel(cancellation)?;
        let state = self.read_history()?;
        let mut cursor = cursor;
        let mut items = Vec::new();
        let mut observation_paths = state.history_observation_paths();
        loop {
            check_cancel(cancellation)?;
            let Some((next, record)) = state.next_recovery(cursor.as_ref())? else {
                return Ok(RecoveryPage {
                    items,
                    next: None,
                    interrupted: state.interrupted()?,
                    observation_paths,
                });
            };
            if let Some(record) = record {
                observation_paths.extend([record.path(), record.retained.stage.directory.clone()]);
                let directory_reads = state
                    .recovery_directory_reads(&record, &self._lock)
                    .unwrap_or_default();
                let mut item = RecoveryItem::inspect(next.0.clone(), record);
                item.directory_reads = directory_reads;
                if matches!(next.0, Key::Pending(0)) && item.entry.is_some() {
                    item.claim_review = state.rename_claim_review()?;
                }
                if let Key::Pending(slot) = next.0
                    && item.entry.is_some()
                {
                    item.saved_return_review = state
                        .saved_return_review(slot)
                        .ok()
                        .flatten()
                        .map(std::sync::Arc::new);
                    if item.saved_return_review.is_some() {
                        item.message="Interrupted saved destination return. Keep in Recovery preserves its original work folder and commits only the completed incoming return.".into();
                    }
                    item.copy_undo_review = state
                        .copy_undo_review(slot)
                        .ok()
                        .flatten()
                        .map(std::sync::Arc::new);
                    if let Some(review) = &item.copy_undo_review {
                        item.message = if review.finishes_saved_return() {
                            "The saved destination has already returned. Keep in Recovery preserves this work folder and finishes the recorded Undo without replaying its return."
                        } else {
                            "Interrupted Copy Undo work folder. Keep in Recovery preserves its contents and leaves the original Undo history available."
                        }.into();
                    }
                    item.undo_cleanup_review = state
                        .undo_cleanup_review(slot)
                        .ok()
                        .flatten()
                        .map(std::sync::Arc::new);
                    if item.undo_cleanup_review.is_some() {
                        item.message = "Completed Undo work folder. Keep in Recovery preserves its contents without replaying the completed Undo step.".into();
                    }
                    item.undo_review = state
                        .undo_work_review(slot)
                        .ok()
                        .flatten()
                        .map(std::sync::Arc::new);
                    if item.undo_review.is_some() {
                        item.message="Interrupted Undo return. Keep in Recovery preserves this entire work folder and leaves the original Undo history unchanged.".into();
                    }
                    item.disposal_review = state
                        .disposal_review(slot)
                        .ok()
                        .flatten()
                        .map(std::sync::Arc::new);
                    if let Some(review) = &item.disposal_review {
                        item.message = if review.is_undo() {
                            "Interrupted Undo recovery. Keep in Recovery preserves every work folder and leaves the original Undo history available."
                        } else if item.saved.container {
                            "Contains all files in this work folder. Keep in Recovery preserves all work folders and lets you continue the original transfer."
                        } else {
                            "Interrupted file. Keep in Recovery preserves all work folders and lets you continue the original transfer."
                        }.into();
                    }
                }
                items.push(item);
            }
            cursor = Some(next);
            if items.len() == 256 {
                return Ok(RecoveryPage {
                    items,
                    next: cursor,
                    interrupted: state.interrupted()?,
                    observation_paths,
                });
            }
        }
    }

    /// Convenience copy-out. Interactive callers should use the richer outcome
    /// to present completed work and exact continuation after partial directory copies.
    pub fn recover_copy(
        &self,
        item: &RecoveryItem,
        destination: &Path,
        cancellation: &Cancellation,
    ) -> io::Result<OperationSummary> {
        let outcome = self.recover_copy_with_check(
            item,
            destination,
            cancellation,
            |_, _| Ok(()),
            |_| transfer::TransferReply::Continue,
        )?;
        if outcome.cancelled {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Recovery copy cancelled; completed items remain available to Undo",
            ));
        }
        if let Some(failure) = outcome.failures.first() {
            return Err(io::Error::new(failure.kind, failure.message.clone()));
        }
        if outcome.needs_attention {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                outcome
                    .attention
                    .unwrap_or_else(|| "Recovery copy needs attention".into()),
            ));
        }
        if !outcome.skipped.is_empty() || !outcome.remaining.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "some Recovery items were not copied; completed items remain available to Undo",
            ));
        }
        let id = outcome
            .id
            .ok_or_else(|| invalid("Recovery copy completed no changes"))?;
        Ok(OperationSummary {
            id,
            label: format!("recover {}", display_name(destination)),
            paths: outcome.destinations,
        })
    }

    pub fn recover_copy_with_check(
        &self,
        item: &RecoveryItem,
        destination: &Path,
        cancellation: &Cancellation,
        check: impl FnMut(&Path, transfer::TransferAccess) -> io::Result<()>,
        interact: impl FnMut(transfer::TransferEvent) -> transfer::TransferReply,
    ) -> io::Result<RecoveryCopyOutcome> {
        self.run_recovery_copy(item, destination, None, cancellation, check, interact)
    }

    pub fn resume_recovery_copy_with_check(
        &self,
        continuation: &RecoveryCopyContinuation,
        cancellation: &Cancellation,
        check: impl FnMut(&Path, transfer::TransferAccess) -> io::Result<()>,
        interact: impl FnMut(transfer::TransferEvent) -> transfer::TransferReply,
    ) -> io::Result<RecoveryCopyOutcome> {
        self.run_recovery_copy(
            &continuation.item,
            &continuation.destination,
            Some(continuation),
            cancellation,
            check,
            interact,
        )
    }

    fn run_recovery_copy(
        &self,
        item: &RecoveryItem,
        destination: &Path,
        continuation: Option<&RecoveryCopyContinuation>,
        cancellation: &Cancellation,
        mut check: impl FnMut(&Path, transfer::TransferAccess) -> io::Result<()>,
        interact: impl FnMut(transfer::TransferEvent) -> transfer::TransferReply,
    ) -> io::Result<RecoveryCopyOutcome> {
        if cancellation.is_cancelled() {
            return Ok(item.cancelled_copy(continuation));
        }
        validate_target(destination)?;
        let destination = interrupted::resolved_target(destination)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?;
        // Salvage never replays the primary journal as a side effect. Only its
        // independent output lane must reconcile before more output is created.
        state.in_recovery_copy(|state| {
            transfer::check_pending(state, &mut check)?;
            recover_current(state, cancellation)
        })?;
        let record = item.resolve(&state)?;
        let source = record.path();
        let mut guard = state.recovery_destination_guard()?;
        guard.protect(&record.retained.stage.directory);
        guard.protect(&source);
        guard.check(&destination)?;
        let parent = destination.parent().unwrap();
        check(parent, transfer::TransferAccess::DestinationFolder)?;
        let parent_identity = if let Some(continuation) = continuation {
            continuation.parent_identity
        } else {
            Identity::from_metadata(&fs::metadata(parent)?)
        };
        let mut plan = continuation
            .map(|continuation| continuation.transfer.clone())
            .unwrap_or_else(|| {
                transfer::TransferContinuation::recovery(source.clone(), destination.clone())
            });
        if let Some(continuation) = continuation
            && continuation.source != source
        {
            plan.relocate_recovery_source(&continuation.source, &source);
        }
        let wrap = |transfer| RecoveryCopyContinuation {
            item: item.clone(),
            source: source.clone(),
            destination: destination.clone(),
            parent_identity,
            transfer,
        };
        let owned = plan.recovery_root_identity(&source, &destination);
        let output_error = match fs::symlink_metadata(&destination) {
            Ok(metadata)
                if metadata.is_dir() && owned == Some(Identity::from_metadata(&metadata)) =>
            {
                None
            }
            Ok(_) => Some(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "the recovery destination already exists; choose a new name",
            )),
            Err(error) if error.kind() == io::ErrorKind::NotFound && owned.is_none() => None,
            Err(error) => Some(error),
        };
        if let Some(error) = output_error {
            if continuation.is_some() {
                return Ok(plan.paused(format!(
                    "the original Recovery output folder changed or is unavailable; restore it before continuing or choose a new copy location. {error}"
                )).map_continuations(wrap));
            }
            return Err(error);
        }
        let label = format!("recover {}", display_name(&destination));
        let outcome = state.in_recovery_copy(|state| {
            let protected_check = |path: &Path, access| {
                check(path, access)?;
                let original_parent = fs::metadata(parent).is_ok_and(|metadata| {
                    metadata.is_dir() && Identity::from_metadata(&metadata) == parent_identity
                });
                if !original_parent {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "the original Recovery destination folder changed; restore it before continuing",
                    ));
                }
                match access {
                    transfer::TransferAccess::SourceEntry => require_observed(item, &record),
                    transfer::TransferAccess::DestinationFolder => guard.check_folder(path),
                }
            };
            transfer::run_transfer(state, &plan, cancellation, protected_check, interact, transfer::RunContext::Recovery(&label))
        })?;
        Ok(outcome.map_continuations(wrap))
    }
}

fn require_observed(item: &RecoveryItem, record: &Record) -> io::Result<()> {
    if item.saved != record.saved_identity() || item.observed.as_ref() != Some(&record.available()?)
    {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the retained file or folder changed; refresh Recovery before copying it",
        ));
    }
    Ok(())
}
