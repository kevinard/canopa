//! Inspect unresolved journal entries without releasing or replaying their data.
use super::*;

/// Preserve the original eight serialized slots; extra parts occupy slots8..15.
pub(super) fn pending_position(slot: u8) -> Option<(IntentLane, u8)> {
    let (lane, part) = match slot {
        0..=7 => (slot / 4, slot % 4),
        8..=15 => ((slot - 8) / 4, 4 + slot % 4),
        _ => return None,
    };
    Some((
        if lane == 0 {
            IntentLane::Primary
        } else {
            IntentLane::RecoveryCopy
        },
        part,
    ))
}

#[derive(Clone, Debug)]
pub struct InterruptedOperation {
    pub label: String,
    pub message: String,
    pub paths: Vec<PathBuf>,
}

/// A live owner's exact journal observation. Never serialized into a request or
/// reconstructed from the paths shown by Recovery.
#[derive(Clone)]
pub(crate) struct Reconciliation {
    journals: [(String, String); 2],
    paths: Vec<PathBuf>,
}

impl Reconciliation {
    pub(crate) fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
}

impl Pending {
    pub(super) fn id(&self) -> u64 {
        match self {
            Self::Expiry { .. } => 0,
            Self::StageMigration { migration } => migration.id,
            Self::Disposing { disposal } => disposal.id,
            Self::UndoCleanup { cleanup } => cleanup.id,
            Self::UndoCopied { removal } => removal.id,
            Self::UndoSavedReturn { returning } => returning.id(),
            Self::CopyBatch { batch } => batch.id,
            Self::Preparing { id, .. }
            | Self::Cleanup { id, .. }
            | Self::Forward { id, .. }
            | Self::Undo { id, .. }
            | Self::DirectoryDestination { id, .. } => *id,
        }
    }
    fn paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        match self {
            Self::CopyBatch { batch } => {
                paths.push(batch.stage.directory.clone());
                for item in &batch.items {
                    paths.extend([item.source.clone(), item.destination.clone()]);
                }
            }
            Self::UndoSavedReturn { returning } => {
                paths.extend(returning.origin.paths());
            }
            Self::UndoCopied { removal } => {
                paths.extend(removal.storage_paths());
            }
            Self::UndoCleanup { cleanup } => {
                paths.extend(cleanup.roots.iter().map(|s| s.directory.clone()));
            }
            Self::Disposing { disposal } => {
                paths.extend([
                    disposal.destination.clone(),
                    disposal.original.directory.clone(),
                    disposal.holding.directory.clone(),
                ]);
                if let Some(Pending::Undo { change, .. }) = disposal.undo.as_deref() {
                    change.clone().visit_paths(|path| paths.push(path.clone()));
                }
                if let Some(stage) = disposal.returned.as_ref().and_then(|r| r.holding.as_ref()) {
                    paths.push(stage.directory.clone());
                }
            }
            Self::Expiry { record, .. } => {
                record.clone().visit_paths(|path| paths.push(path.clone()));
            }
            Self::StageMigration { migration } => {
                migration
                    .change
                    .clone()
                    .visit_paths(|path| paths.push(path.clone()));
                paths.extend([
                    migration.source.directory.clone(),
                    migration.stage.directory.clone(),
                ]);
            }
            Self::DirectoryDestination { change, stage, .. } => {
                change.clone().visit_paths(|path| paths.push(path.clone()));
                paths.push(stage.directory.clone());
            }
            Self::Preparing {
                destination, stage, ..
            }
            | Self::Cleanup {
                destination, stage, ..
            } => {
                paths.extend([destination.clone(), stage.directory.clone()]);
            }
            Self::Forward { change, stage, .. } | Self::Undo { change, stage, .. } => {
                change.clone().visit_paths(|path| paths.push(path.clone()));
                if let Some(stage) = stage {
                    paths.push(stage.directory.clone());
                    if stage.sibling_payload {
                        paths.push(stage.payload());
                    }
                }
                if let Self::Undo {
                    displaced: Some(record),
                    ..
                } = self
                {
                    record.clone().visit_paths(|path| paths.push(path.clone()));
                }
            }
        }
        paths
    }
    pub(super) fn record(&self, part: u8, label: String) -> Option<recovery::Record> {
        use recovery::{Context, Record};
        if let Self::UndoSavedReturn { returning } = self {
            if part != 0 {
                return returning.origin.record(part, label);
            }
            return Some(Record {
                context_moves: Vec::new(),
                retained: Retained {
                    stage: returning.stage.clone(),
                    identity: returning.stage.identity,
                    replacement_facts: None,
                    physical_identity: None,
                    recovery: true,
                    version: None,
                },
                context: Some(Context {
                    original: returning.parts().ok()?.2.forward_path().to_owned(),
                    label,
                }),
                payload: true,
                container: true,
            });
        }
        if let Self::UndoCopied { removal } = self {
            if part != 0 {
                return None;
            }
            return Some(Record {
                context_moves: Vec::new(),
                retained: Retained {
                    stage: removal.stage.clone(),
                    identity: removal.stage.identity,
                    replacement_facts: None,
                    physical_identity: None,
                    recovery: true,
                    version: None,
                },
                context: Some(Context {
                    original: removal.change.forward_path().to_owned(),
                    label,
                }),
                payload: true,
                container: true,
            });
        }
        if let Self::UndoCleanup { cleanup } = self {
            let stage = cleanup.roots.get(usize::from(part))?;
            return Some(Record {
                context_moves: Vec::new(),
                retained: Retained {
                    stage: stage.clone(),
                    identity: stage.identity,
                    replacement_facts: None,
                    physical_identity: None,
                    recovery: true,
                    version: None,
                },
                context: Some(Context {
                    original: stage.directory.clone(),
                    label,
                }),
                payload: true,
                container: true,
            });
        }
        if matches!(self, Self::Undo { .. })
            && part == 0
            && let Some(record) = store::undo_work::direct_preparation(self)
        {
            let mut root = record.clone();
            root.retained.identity = root.retained.stage.identity;
            root.retained.version = None;
            root.payload = true;
            root.container = true;
            root.context.as_mut()?.label = label;
            return Some(root);
        }
        if let Self::Disposing { disposal } = self {
            if disposal.empty_copy && (part == 1 || part == 3 || ((part == 0 || part == 2) && disposal.undo.as_deref().is_some_and(|p| matches!(p, Pending::Undo { displaced: Some(record), .. } if record.retained.stage == disposal.original)))) { return None; }
            if part >= 4 {
                let stage = match part {
                    4 | 5 => match disposal.undo.as_deref()? {
                        Pending::Undo {
                            displaced: Some(record),
                            ..
                        } => &record.retained.stage,
                        _ => return None,
                    },
                    6 | 7 => disposal.returned.as_ref()?.holding.as_ref()?,
                    _ => return None,
                };
                let container = part == 5 || part == 7;
                let previous = part == 4;
                let path = if previous {
                    stage.directory.join("previous")
                } else {
                    stage.payload()
                };
                return Some(Record {
                    context_moves: Vec::new(),
                    retained: Retained {
                        stage: stage.clone(),
                        identity: if container {
                            stage.identity
                        } else {
                            Identity::read(&path)
                                .ok()
                                .or(stage.payload_identity)
                                .unwrap_or(stage.identity)
                        },
                        replacement_facts: None,
                        physical_identity: None,
                        recovery: true,
                        version: None,
                    },
                    context: Some(Context {
                        original: disposal.destination.clone(),
                        label,
                    }),
                    payload: !previous,
                    container,
                });
            }
            let stage = match part {
                0 | 2 => &disposal.original,
                1 | 3 => &disposal.holding,
                _ => return None,
            };
            validate_stage(Some(stage), &disposal.destination).ok()?;
            return Some(Record {
                context_moves: Vec::new(),
                retained: Retained {
                    replacement_facts: None,
                    physical_identity: None,
                    stage: stage.clone(),
                    identity: if part >= 2 {
                        stage.identity
                    } else {
                        Identity::read(&stage.payload())
                            .ok()
                            .or(stage.payload_identity)
                            .unwrap_or(stage.identity)
                    },
                    recovery: true,
                    version: None,
                },
                context: Some(Context {
                    original: disposal.destination.clone(),
                    label,
                }),
                payload: true,
                container: part >= 2,
            });
        }
        if let Self::StageMigration { migration } = self {
            return (part == 0).then(|| migration.record(label));
        }
        if let Self::CopyBatch { batch } = self {
            // The whole batch stage is one work folder in Recovery.
            if part != 0 {
                return None;
            }
            let first = batch.items.first()?;
            validate_stage(Some(&batch.stage), &first.destination).ok()?;
            return Some(Record {
                context_moves: Vec::new(),
                retained: Retained {
                    replacement_facts: None,
                    physical_identity: None,
                    stage: batch.stage.clone(),
                    identity: batch.stage.identity,
                    recovery: true,
                    version: None,
                },
                context: Some(Context {
                    original: first.destination.parent()?.to_owned(),
                    label,
                }),
                payload: true,
                container: true,
            });
        }
        let (target, change, work) = match self {
            Self::Expiry { .. } => return None,
            Self::StageMigration { .. }
            | Self::CopyBatch { .. }
            | Self::Disposing { .. }
            | Self::UndoCleanup { .. }
            | Self::UndoSavedReturn { .. }
            | Self::UndoCopied { .. } => {
                unreachable!()
            }
            Self::DirectoryDestination { change, stage, .. } => {
                (change.reverse_path()?, Some(change), Some(stage))
            }
            Self::Preparing {
                destination, stage, ..
            }
            | Self::Cleanup {
                destination, stage, ..
            } => (destination.as_path(), None, Some(stage)),
            Self::Forward { change, stage, .. } | Self::Undo { change, stage, .. } => {
                (change.forward_path(), Some(change), stage.as_ref())
            }
        };
        let (retained, payload, original) = match part {
            0 => {
                let stage = work?;
                if matches!(
                    self,
                    Self::Forward {
                        change: Change::Rename { .. },
                        ..
                    }
                ) {
                    validate_claim_stage(Some(stage), target).ok()?;
                } else {
                    validate_stage(Some(stage), target).ok()?;
                }
                (
                    Retained {
                        replacement_facts: None,
                        physical_identity: None,
                        stage: stage.clone(),
                        identity: stage
                            .payload_identity
                            .or_else(|| Identity::read(&stage.payload()).ok())
                            .unwrap_or(stage.identity),
                        recovery: true,
                        version: None,
                    },
                    true,
                    target.to_owned(),
                )
            }
            1 => {
                if matches!(
                    self,
                    Self::Undo {
                        change: Change::SourceRemoved { .. },
                        ..
                    }
                ) && let Some(stage) = work
                    && stage.ownership.is_some()
                {
                    return Some(Record {
                        context_moves: Vec::new(),
                        retained: Retained {
                            replacement_facts: None,
                            physical_identity: None,
                            stage: stage.clone(),
                            identity: stage.identity,
                            recovery: true,
                            version: None,
                        },
                        context: Some(Context {
                            original: target.to_owned(),
                            label,
                        }),
                        payload: true,
                        container: true,
                    });
                }
                (change?.retained()?.clone(), false, target.to_owned())
            }
            2 => {
                let Self::Undo {
                    displaced: Some(record),
                    ..
                } = self
                else {
                    return None;
                };
                let original = record.context.as_ref()?.original.clone();
                validate_stage(Some(&record.retained.stage), &original).ok()?;
                return Some(Record {
                    context_moves: Vec::new(),
                    retained: record.retained.clone(),
                    context: Some(Context { original, label }),
                    payload: false,
                    container: false,
                });
            }
            3 => {
                let Change::SourceRemoved {
                    original,
                    quarantine,
                    identity,
                    ..
                } = change?
                else {
                    return None;
                };
                validate_stage(Some(quarantine), original).ok()?;
                (
                    Retained {
                        replacement_facts: None,
                        physical_identity: None,
                        stage: quarantine.clone(),
                        identity: *identity,
                        recovery: true,
                        version: None,
                    },
                    true,
                    original.clone(),
                )
            }
            _ => return None,
        };
        if payload
            && matches!(
                self,
                Self::Forward {
                    change: Change::Rename { .. },
                    ..
                }
            )
        {
            validate_claim_stage(Some(&retained.stage), &original).ok()?;
        } else {
            validate_stage(Some(&retained.stage), &original).ok()?;
        }
        Some(Record {
            context_moves: Vec::new(),
            retained,
            context: Some(Context { original, label }),
            payload,
            container: false,
        })
    }
}

impl Store {
    fn reconciliation(&self) -> io::Result<Reconciliation> {
        let mut paths = Vec::new();
        let mut capture = |lane| -> io::Result<(String, String)> {
            let pending = self.pending_in(lane)?;
            if let Some(pending) = &pending {
                paths.extend(pending.paths());
            }
            paths.extend(self.directory_access_paths(lane)?);
            Ok((
                serde_json::to_string(&pending).map_err(io::Error::other)?,
                self.directory_access_context(lane)?,
            ))
        };
        let journals = [
            capture(IntentLane::Primary)?,
            capture(IntentLane::RecoveryCopy)?,
        ];
        Ok(Reconciliation { journals, paths })
    }
    pub(super) fn interrupted(&self) -> io::Result<Vec<InterruptedOperation>> {
        let mut issues = Vec::new();
        for lane in [IntentLane::Primary, IntentLane::RecoveryCopy] {
            if self.hide_running_journals && self.recovery_problem(lane).is_none() {
                continue;
            }
            let access = self.directory_access_paths(lane)?;
            if let Some(pending) = self.pending_in(lane)? {
                let mut paths = pending.paths();
                paths.extend(access);
                issues.push(InterruptedOperation {
                    label: if matches!(pending, Pending::Expiry { .. }) {
                        "Finish expired replacement cleanup".into()
                    } else {
                        self.label(pending.id())?
                            .unwrap_or_else(|| "Interrupted operation".into())
                    },
                    message: self
                        .recovery_problem(lane)
                        .unwrap_or_else(|| "An unfinished file operation needs recovery.".into()),
                    paths,
                });
            } else if !access.is_empty() {
                issues.push(InterruptedOperation {
                    label: "Restore folder permissions".into(),
                    message: self
                        .recovery_problem(lane)
                        .unwrap_or_else(|| "Temporary folder access needs to be restored.".into()),
                    paths: access,
                });
            }
        }
        Ok(issues)
    }
    pub(super) fn pending_record(&self, slot: u8) -> io::Result<Option<recovery::Record>> {
        let Some((lane, part)) = pending_position(slot) else {
            return Ok(None);
        };
        if self.hide_running_journals && self.recovery_problem(lane).is_none() {
            return Ok(None);
        }
        let Some(pending) = self.pending_in(lane)? else {
            return Ok(None);
        };
        let label = format!(
            "Interrupted {}",
            self.label(pending.id())?
                .unwrap_or_else(|| "operation".into())
        );
        Ok(pending.record(part, label))
    }
    pub(super) fn recovery_destination_guard(&self) -> io::Result<RecoveryDestinationGuard> {
        let mut guard = RecoveryDestinationGuard { paths: Vec::new() };
        for issue in self.interrupted()? {
            for path in issue.paths {
                guard.protect(&path);
            }
        }
        Ok(guard)
    }
    pub(super) fn reconciliation_paths(&self) -> io::Result<Vec<PathBuf>> {
        if matches!(
            self.lane_for_reconciliation(),
            store::IntentLane::RecoveryCopy
        ) {
            let mut paths = self
                .pending()?
                .map(|pending| pending.paths())
                .unwrap_or_default();
            paths.extend(self.directory_access_paths(IntentLane::RecoveryCopy)?);
            Ok(paths)
        } else {
            Ok(self
                .interrupted()?
                .into_iter()
                .flat_map(|issue| issue.paths)
                .collect())
        }
    }
}

pub(super) struct RecoveryDestinationGuard {
    paths: Vec<PathBuf>,
}
impl RecoveryDestinationGuard {
    pub(super) fn protect(&mut self, path: &Path) {
        self.paths.push(path.to_owned());
        if let Ok(resolved) = resolved_target(path) {
            self.paths.push(resolved);
        }
    }
    pub(super) fn check(&self, destination: &Path) -> io::Result<()> {
        if self.paths.iter().any(|path| destination.starts_with(path)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "choose a recovery location outside saved data and the interrupted operation's paths",
            ));
        }
        Ok(())
    }
    pub(super) fn check_folder(&self, folder: &Path) -> io::Result<()> {
        self.check(&folder.canonicalize()?)
    }
}

pub(super) fn resolved_target(path: &Path) -> io::Result<PathBuf> {
    validate_target(path)?;
    Ok(path
        .parent()
        .unwrap()
        .canonicalize()?
        .join(path.file_name().unwrap()))
}

impl OperationEngine {
    pub(crate) fn capture_reconciliation(&self) -> io::Result<Reconciliation> {
        self.read_history()?.reconciliation()
    }

    pub(crate) fn reconcile_with_check(
        &self,
        observed: &Reconciliation,
        cancellation: &Cancellation,
        check: impl FnOnce(&[PathBuf]) -> io::Result<()>,
    ) -> io::Result<()> {
        check_cancel(cancellation)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?;
        check_cancel(cancellation)?;
        if state.reconciliation()?.journals != observed.journals {
            return Err(io::Error::other(
                "Interrupted work changed after this review was requested. Refresh Recovery before retrying.",
            ));
        }
        check(&observed.paths)?;
        recover(&mut state, cancellation)?;
        state.trim()
    }

    pub fn interrupted_operations(
        &self,
        cancellation: &Cancellation,
    ) -> io::Result<Vec<InterruptedOperation>> {
        check_cancel(cancellation)?;
        let state = self.read_history()?;
        state.interrupted()
    }
    /// Explicitly retry automatic reconciliation after resolving an unavailable
    /// location or occupied path, preserving journal-owned data on conflicts.
    pub fn retry_recovery(&self, cancellation: &Cancellation) -> io::Result<()> {
        check_cancel(cancellation)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?;
        check_cancel(cancellation)?;
        recover(&mut state, cancellation)?;
        state.trim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_sibling_claim_paths_can_be_inspected_without_panicking() {
        // Pure journal inspection: the invalid root path is never used for a
        // filesystem operation. Startup must be able to show corrupt metadata.
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("source");
        let current = root.path().join("renamed");
        let stage = Stage {
            replacement: None,
            entry_move: None,
            source_claim: None,
            directory_claim: None,
            ownership: None,
            directory: PathBuf::from("/"),
            identity: Identity {
                device: 0,
                inode: 0,
            },
            parent_access: None,
            payload_identity: None,
            parent_identity: None,
            sibling_payload: true,
        };
        assert!(validate_claim_stage(Some(&stage), &current).is_err());
        let pending = Pending::Forward {
            id: 1,
            ordinal: 0,
            change: Change::Rename {
                move_facts: None,
                original: original.clone(),
                current: current.clone(),
                identity: Identity {
                    device: 0,
                    inode: 0,
                },
                saved: None,
                retained: None,
                reviewed: None,
            },
            stage: Some(stage),
        };
        let decoded: Pending =
            serde_json::from_slice(&serde_json::to_vec(&pending).unwrap()).unwrap();
        let paths = decoded.paths();
        assert!(paths.contains(&original) && paths.contains(&current));
    }
}
