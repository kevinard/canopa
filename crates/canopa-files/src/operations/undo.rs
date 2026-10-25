use super::*;
#[cfg(test)]
pub(in crate::operations) mod abort_boundary;
mod changed_copy;
mod changed_move;
pub(super) mod copied;
mod created_folder;
mod cross_move;
pub(super) mod directory_return;
pub(in crate::operations) mod displaced;
mod moved_directory;
pub(super) mod saved_return;
mod saved_version;
mod unavailable_move;
mod unavailable_saved;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndoChoice {
    Merge,
    Replace,
    KeepBoth,
    KeepItem,
    MoveToRecovery,
    UseSavedVersion,
    ReturnItem,
    FinishStep,
    RestorePreviousDestination,
    Skip,
    Cancel,
}
#[derive(Clone, Debug)]
pub enum UndoItem {
    Available(crate::browse::Entry),
    Unavailable {
        path: PathBuf,
        kind: io::ErrorKind,
        message: String,
    },
}
impl UndoItem {
    pub fn path(&self) -> &Path {
        match self {
            Self::Available(entry) => &entry.path,
            Self::Unavailable { path, .. } => path,
        }
    }
    pub fn entry(&self) -> Option<&crate::browse::Entry> {
        match self {
            Self::Available(entry) => Some(entry),
            Self::Unavailable { .. } => None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct UndoConflict {
    pub incoming: UndoItem,
    pub existing: Option<crate::browse::Entry>,
    pub saved_previous: Option<crate::browse::Entry>,
    pub return_destination: Option<PathBuf>,
    pub destination: PathBuf,
    pub source: UndoSource,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndoSource {
    Current,
    Trash,
    PreviousDestination,
    NonemptyFolder,
    MergeRemainder,
    MergeSource,
    MergeDestination,
    ChangedCreatedFolder,
    UnavailableCreatedFolder,
    ChangedCopy,
    ChangedRetained,
    ChangedMove,
    ChangedCrossMove,
    UnavailableMove,
    UnavailableCrossMove,
    UnavailableRetained,
    UnavailableMergeRetained,
}
impl UndoConflict {
    pub fn can_merge(&self) -> bool {
        let directory = |entry: &crate::browse::Entry| {
            entry.kind == crate::browse::EntryKind::Directory
                // A saved version's container retains its physical identity.
                // Keep both moves it intact when its reserved name collides.
                && !entry.path.file_name().is_some_and(|name| name.as_encoded_bytes().starts_with(b".canopa-files-stage-"))
        };
        matches!(
            self.source,
            UndoSource::Current | UndoSource::Trash | UndoSource::MergeRemainder
        ) && self.incoming.entry().is_some_and(directory)
            && self.existing.as_ref().is_some_and(directory)
    }
    pub fn can_replace(&self) -> bool {
        matches!(
            self.source,
            UndoSource::Current | UndoSource::Trash | UndoSource::PreviousDestination
        ) && self.incoming.entry().is_some_and(|incoming| {
            self.existing.as_ref().is_some_and(|existing| {
                [incoming, existing].into_iter().all(|entry| {
                    matches!(
                        entry.kind,
                        crate::browse::EntryKind::File | crate::browse::EntryKind::Symlink
                    )
                })
            })
        })
    }
    pub fn can_move_to_recovery(&self) -> bool {
        self.source == UndoSource::ChangedCopy
            && self.incoming.entry().is_some_and(|incoming| {
                matches!(
                    incoming.kind,
                    crate::browse::EntryKind::File | crate::browse::EntryKind::Symlink
                )
            })
    }
    pub fn can_use_saved_version(&self) -> bool {
        self.source == UndoSource::ChangedRetained
            && self.incoming.entry().is_some_and(|incoming| {
                matches!(
                    incoming.kind,
                    crate::browse::EntryKind::File | crate::browse::EntryKind::Symlink
                )
            })
    }
    pub fn can_return_item(&self) -> bool {
        // Same-filesystem returns rename a node. Cross-filesystem returns copy
        // only regular files and links, without opening special-node streams.
        self.source == UndoSource::ChangedMove
            || (self.source == UndoSource::ChangedCrossMove
                && self.incoming.entry().is_some_and(|entry| {
                    matches!(
                        entry.kind,
                        crate::browse::EntryKind::File | crate::browse::EntryKind::Symlink
                    )
                }))
            || (matches!(
                self.source,
                UndoSource::UnavailableRetained | UndoSource::UnavailableMergeRetained
            ) && self.return_destination.is_some())
    }
    pub fn can_restore_previous_destination(&self) -> bool {
        matches!(
            self.source,
            UndoSource::UnavailableMove | UndoSource::UnavailableCrossMove
        ) && matches!(
            self.incoming,
            UndoItem::Unavailable {
                kind: io::ErrorKind::NotFound,
                ..
            }
        ) && self.saved_previous.as_ref().is_some_and(|entry| {
            matches!(
                entry.kind,
                crate::browse::EntryKind::File | crate::browse::EntryKind::Symlink
            )
        })
    }
}
#[derive(Clone, Debug)]
pub struct UndoOutcome {
    pub summary: OperationSummary,
    pub completed: usize,
    pub skipped: Vec<PathBuf>,
    pub kept: Vec<PathBuf>,
    pub dismissed: Vec<PathBuf>,
    pub unrestored_previous: Vec<PathBuf>,
    pub unremoved: Vec<PathBuf>,
    pub cancelled: bool,
    pub remaining: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Relocation {
    #[serde(with = "crate::path_serde")]
    pub from: PathBuf,
    #[serde(with = "crate::path_serde")]
    pub to: PathBuf,
    pub subtree: bool,
}
impl Relocation {
    pub fn validate(&self) -> io::Result<()> {
        validate_target(&self.from)?;
        validate_target(&self.to)?;
        if self.from == self.to || self.from.parent() != self.to.parent() {
            return Err(invalid("invalid Undo relocation"));
        }
        Ok(())
    }
    pub fn apply(&self, path: &mut PathBuf) {
        if path == &self.from {
            *path = self.to.clone();
        } else if self.subtree
            && let Ok(tail) = path.strip_prefix(&self.from)
        {
            *path = self.to.join(tail);
        }
    }
}

impl Change {
    pub(super) fn relocate(&mut self, relocation: &Relocation) {
        self.visit_paths(|path| relocation.apply(path));
    }

    // Storage indexes the same paths that restoration can rewrite.
    pub(super) fn visit_paths(&mut self, mut visit: impl FnMut(&mut PathBuf)) {
        self.visit_paths_inner(&mut visit);
    }

    fn visit_paths_inner(&mut self, mut visit: &mut dyn FnMut(&mut PathBuf)) {
        match self {
            Self::DirectoryMerge { change, .. } => change.visit_paths_inner(visit),
            Self::CreatedFolder { path, .. }
            | Self::Copied { path, .. }
            | Self::RetainedOnly { path, .. } => visit(path),
            Self::RemovedDirectory { path, moved, .. } => {
                visit(path);
                if let Some(moved) = moved {
                    visit(&mut moved.current);
                }
            }
            Self::Rename {
                original,
                current,
                saved,
                ..
            } => {
                visit(original);
                visit(current);
                if let Some(context) = saved.as_mut().and_then(|saved| saved.context.as_mut()) {
                    visit(&mut context.from);
                    visit(&mut context.to);
                }
            }
            Self::Trashed {
                original,
                current,
                info,
                ..
            }
            | Self::Restored {
                original,
                current,
                info,
                ..
            } => {
                visit(original);
                visit(current);
                info.visit_path(&mut visit);
            }
            Self::SourceRemoved {
                original,
                current,
                quarantine,
                ..
            } => {
                visit(original);
                visit(current);
                visit(&mut quarantine.directory);
            }
        }
        match self {
            Self::Rename {
                retained: Some(retained),
                ..
            }
            | Self::Copied {
                retained: Some(retained),
                ..
            }
            | Self::RetainedOnly { retained, .. } => visit(&mut retained.stage.directory),
            _ => {}
        }
    }
}

pub(super) fn run(
    engine: &OperationEngine,
    expected: Option<u64>,
    cancellation: &Cancellation,
) -> io::Result<OperationSummary> {
    run_interactive(engine, expected, cancellation, None).map(|outcome| outcome.summary)
}

pub(super) fn run_interactive(
    engine: &OperationEngine,
    expected: Option<u64>,
    cancellation: &Cancellation,
    resolve: Option<&mut dyn FnMut(UndoConflict) -> UndoChoice>,
) -> io::Result<UndoOutcome> {
    check_cancel(cancellation)?;
    let mut state = engine
        .state
        .lock()
        .map_err(|_| io::Error::other("operation history failed"))?;
    check_cancel(cancellation)?;
    recover(&mut state, cancellation)?;
    state.with_directory_access(|state| {
        run_interactive_inner(state, expected, cancellation, resolve)
    })
}

fn run_interactive_inner(
    state: &mut Store,
    expected: Option<u64>,
    cancellation: &Cancellation,
    mut resolve: Option<&mut dyn FnMut(UndoConflict) -> UndoChoice>,
) -> io::Result<UndoOutcome> {
    let (id, label) = state
        .latest()?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "nothing to undo"))?;
    if expected.is_some_and(|expected| expected != id) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "a more recent operation must be undone first",
        ));
    }
    let mut paths = Vec::new();
    let mut completed = 0;
    let mut skipped = Vec::new();
    let mut kept = Vec::new();
    let mut dismissed = Vec::new();
    let mut unrestored_previous = Vec::new();
    let mut unremoved = Vec::new();
    let mut cancelled = false;
    let mut before = None;
    let mut deferred_roots = std::collections::BTreeSet::new();
    let mut deferred_namespaces = Vec::new();
    let mut skipped_directories = Vec::<(u64, u64)>::new();
    loop {
        state.restore_directory_access()?;
        let Some((ordinal, mut change)) = state.change_before(id, before)? else {
            if before.is_some() && state.ready_moved_directory_return(id, &deferred_roots)? {
                before = None;
                continue;
            }
            break;
        };
        if cancellation.is_cancelled() && resolve.is_some() {
            cancelled = true;
            break;
        }
        check_cancel(cancellation)?;
        before = Some(ordinal);
        if skipped_directories
            .iter()
            .any(|(first, last)| ordinal >= *first && ordinal <= *last)
            || state.directory_source_cleanup(id, ordinal)?
        {
            continue;
        }
        let mut unavailable_return = None;
        let mut return_only = None;
        if let Change::CreatedFolder { path, .. } = &change
            && skipped
                .iter()
                .any(|child: &PathBuf| child != path && child.starts_with(path))
        {
            skipped.push(path.clone());
            continue;
        }
        // A skipped return still relies on its destination copy and namespace.
        // Older frames must not restore over it or move an ancestor away.
        let dependent_path = match &change {
            Change::SourceRemoved { current, .. } => current.as_path(),
            _ => change.forward_path(),
        };
        if skipped
            .iter()
            .chain(&kept)
            .chain(&dismissed)
            .chain(&unrestored_previous)
            .chain(&unremoved)
            .chain(&deferred_namespaces)
            .any(|path| path.starts_with(dependent_path))
        {
            if directory_return::moved(&change).is_some()
                && !state.directory_children_remaining(id, ordinal)?
            {
                deferred_roots.insert(ordinal);
            }
            continue;
        }
        match directory_return::roots_review::review(
            state,
            id,
            ordinal,
            &mut deferred_roots,
            cancellation,
            resolve
                .as_mut()
                .map(|interact| &mut **interact as &mut dyn FnMut(UndoConflict) -> UndoChoice),
        )? {
            directory_return::roots_review::Review::Ready => {}
            directory_return::roots_review::Review::Deferred => continue,
            directory_return::roots_review::Review::Skipped {
                source,
                destination,
            } => {
                deferred_namespaces.push(destination);
                skipped.push(source);
                continue;
            }
            directory_return::roots_review::Review::Finished {
                source,
                destination,
                previous,
            } => {
                deferred_namespaces.push(destination);
                unrestored_previous.extend(previous);
                dismissed.push(source);
                continue;
            }
            directory_return::roots_review::Review::Cancelled => {
                cancelled = true;
                break;
            }
        }
        state.require_directory_parents(id, ordinal)?;
        state.permit_undo_parent(id, ordinal, &change)?;
        match moved_directory::review(
            state,
            id,
            ordinal,
            &change,
            cancellation,
            resolve
                .as_mut()
                .map(|interact| &mut **interact as &mut dyn FnMut(UndoConflict) -> UndoChoice),
        )? {
            moved_directory::Review::Ready => {}
            moved_directory::Review::Merged => {
                before = None;
                continue;
            }
            moved_directory::Review::Skipped { first, path } => {
                skipped_directories.push((first, ordinal));
                skipped.push(path);
                continue;
            }
            moved_directory::Review::Cancelled => {
                cancelled = true;
                break;
            }
        }
        if directory_return::moved(&change).is_some()
            && (state.directory_children_remaining(id, ordinal)?
                || state.moved_directory_source_in_use(id, ordinal)?)
        {
            continue;
        }
        match directory_return::review_remainder(
            state,
            id,
            ordinal,
            &change,
            cancellation,
            resolve
                .as_mut()
                .map(|interact| &mut **interact as &mut dyn FnMut(UndoConflict) -> UndoChoice),
        )? {
            directory_return::Review::Ready => {}
            directory_return::Review::Expanded => {
                before = None;
                continue;
            }
            directory_return::Review::Kept { path, previous } => {
                paths.push(path.clone());
                if previous {
                    unrestored_previous.push(path.clone());
                }
                kept.push(path);
                continue;
            }
            directory_return::Review::Skipped(path) => {
                if directory_return::moved(&change).is_some() {
                    deferred_roots.insert(ordinal);
                }
                skipped.push(path);
                continue;
            }
            directory_return::Review::Cancelled => {
                cancelled = true;
                break;
            }
        }
        let folder_review = created_folder::review(
            state,
            id,
            ordinal,
            &change,
            cancellation,
            resolve
                .as_mut()
                .map(|interact| &mut **interact as &mut dyn FnMut(UndoConflict) -> UndoChoice),
        )?;
        match folder_review {
            created_folder::Review::Ready => {}
            created_folder::Review::Finished(path) => {
                unremoved.push(path);
                continue;
            }
            created_folder::Review::Kept(items) => {
                paths.extend(items.iter().cloned());
                kept.extend(items);
                continue;
            }
            created_folder::Review::Skipped(path) => {
                skipped.push(path);
                continue;
            }
            created_folder::Review::Cancelled => {
                cancelled = true;
                break;
            }
        }
        if let Some(interact) = resolve.as_mut() {
            match cross_move::review(
                state,
                id,
                ordinal,
                &label,
                &mut change,
                cancellation,
                *interact,
            )? {
                cross_move::Review::Ready => {}
                cross_move::Review::RestoreSaved(path) => unavailable_return = Some(path),
                cross_move::Review::Kept(items) => {
                    paths.extend(items.iter().cloned());
                    kept.extend(items);
                    continue;
                }
                cross_move::Review::Skipped(path) => {
                    skipped.push(path);
                    continue;
                }
                cross_move::Review::Dismissed {
                    path,
                    kept: containers,
                    previous,
                } => {
                    if previous {
                        unrestored_previous.push(path.clone());
                    }
                    dismissed.push(path);
                    kept.extend(containers);
                    continue;
                }
                cross_move::Review::Cancelled => {
                    cancelled = true;
                    break;
                }
            }
            match changed_move::review(
                state,
                id,
                ordinal,
                &label,
                &mut change,
                cancellation,
                *interact,
            )? {
                changed_move::Review::Ready => {}
                changed_move::Review::Skipped(path) => {
                    skipped.push(path);
                    continue;
                }
                changed_move::Review::Cancelled => {
                    cancelled = true;
                    break;
                }
                changed_move::Review::Kept(items) => {
                    paths.extend(items.iter().cloned());
                    kept.extend(items);
                    continue;
                }
                changed_move::Review::Dismissed(path) => {
                    dismissed.push(path);
                    continue;
                }
                changed_move::Review::RestoreSaved(path) => unavailable_return = Some(path),
            }
            match saved_version::review(
                state,
                id,
                ordinal,
                &label,
                &mut change,
                cancellation,
                *interact,
            )? {
                saved_version::Review::Ready => {}
                saved_version::Review::ReturnOnly(guard) => {
                    match &mut change {
                        Change::Rename { retained, .. } => *retained = None,
                        Change::DirectoryMerge { defer_previous, .. } => *defer_previous = true,
                        _ => unreachable!(),
                    }
                    return_only = Some(guard);
                }
                saved_version::Review::Finished(path) => {
                    unrestored_previous.push(path);
                    continue;
                }
                saved_version::Review::DirectoryFinished {
                    source,
                    destination,
                    previous,
                } => {
                    dismissed.push(source);
                    deferred_namespaces.push(destination);
                    unrestored_previous.extend(previous);
                    continue;
                }
                saved_version::Review::Skipped(path) => {
                    skipped.push(path);
                    continue;
                }
                saved_version::Review::Cancelled => {
                    cancelled = true;
                    break;
                }
            }
        }
        if let Some(path) = unavailable_return.as_deref() {
            unavailable_move::require_missing(path)?;
        }
        match directory_return::cross_filesystem::prepare(
            state,
            id,
            ordinal,
            &change,
            cancellation,
            None,
        )? {
            directory_return::cross_filesystem::Preparation::Unchanged => {}
            directory_return::cross_filesystem::Preparation::Expanded => {
                before = None;
                continue;
            }
            directory_return::cross_filesystem::Preparation::Returned => {
                completed += 1;
                paths.extend(change.reverse_path().map(Path::to_owned));
                before = None;
                continue;
            }
        }
        if let Some(resolve) = resolve.as_mut()
            && let Some(decision) =
                changed_copy::resolve(state, id, ordinal, &change, &label, cancellation, *resolve)?
        {
            match decision {
                changed_copy::Resolution::Skipped(path) => skipped.push(path),
                changed_copy::Resolution::Kept(items) => {
                    paths.extend(items.iter().cloned());
                    kept.extend(items);
                }
                changed_copy::Resolution::Removed => completed += 1,
                changed_copy::Resolution::Cancelled => {
                    cancelled = true;
                    break;
                }
            }
            continue;
        }
        let mut relocation = None;
        let mut displaced = None;
        let mut owned_return_displaced = None;
        if resolve.is_some()
            && let Some(target) = restoration_conflict(state, id, ordinal, &change, cancellation)?
        {
            let incoming_version = fingerprint::Version::read(&target.incoming)?;
            let incoming = crate::browse::Entry::read(&target.incoming)?;
            let observed_displaced = displaced::Observed::open(&target.original)?;
            let existing_version = observed_displaced.version().clone();
            let existing = crate::browse::Entry::read(&target.original)?;
            if let Change::Copied {
                path,
                identity,
                retained: Some(retained),
                ..
            } = &change
            {
                // A changed copy becomes newer work to retain. Undo now restores
                // its previous destination rather than deleting the changed copy.
                change = Change::RetainedOnly {
                    path: path.clone(),
                    retained: retained.clone(),
                    replaced: Some(*identity),
                };
            }
            let return_parent = match &change {
                Change::SourceRemoved { quarantine, .. } => quarantine.parent_identity,
                _ => None,
            };
            let original = match &mut change {
                Change::Rename { original, .. }
                | Change::Trashed { original, .. }
                | Change::SourceRemoved { original, .. } => original,
                Change::RetainedOnly { path, .. } => path,
                _ => unreachable!(),
            };
            let conflict = UndoConflict {
                incoming: UndoItem::Available(incoming.clone()),
                existing: Some(existing),
                saved_previous: None,
                return_destination: None,
                destination: target.original.clone(),
                source: target.source,
            };
            let can_replace = conflict.can_replace();
            let can_merge = conflict.can_merge();
            let choice = resolve.as_mut().unwrap()(conflict);
            if choice != UndoChoice::Cancel {
                check_cancel(cancellation)?;
                state.require_directory_parents(id, ordinal)?;
            }
            match choice {
                UndoChoice::Merge if can_merge => {
                    directory_return::begin(
                        state,
                        id,
                        ordinal,
                        &change,
                        (&incoming_version, &existing_version),
                        cancellation,
                        return_only.as_deref(),
                    )?;
                    before = None;
                    continue;
                }
                UndoChoice::Replace => {
                    if !can_replace {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "Undo Replace is available for files and links. Use Keep both for this item.",
                        ));
                    }
                    require_parent(state, id, ordinal, original, return_parent)?;
                    let prepared = observed_displaced.prepare()?;
                    let stage = prepared.stage().clone();
                    owned_return_displaced = Some(prepared);
                    displaced = Some(recovery::Record {
                        context_moves: Vec::new(),
                        payload: false,
                        container: false,
                        retained: Retained {
                            replacement_facts: None,
                            physical_identity: None,
                            stage,
                            identity: existing_version.identity,
                            recovery: true,
                            version: Some(existing_version),
                        },
                        context: Some(recovery::Context {
                            original: original.clone(),
                            label: format!("Preserved by Undo {label}"),
                        }),
                    });
                }
                UndoChoice::KeepBoth => {
                    let target = transfer::unique_name(original)?;
                    relocation = Some(Relocation {
                        from: original.clone(),
                        to: target.clone(),
                        // An older version can be a directory link even when
                        // the immediate version is a regular file. Its historical
                        // namespace must follow the restored name too.
                        subtree: true,
                    });
                    *original = target;
                }
                UndoChoice::Skip => {
                    skipped.push(if target.source == UndoSource::PreviousDestination {
                        target.original
                    } else {
                        target.incoming
                    });
                    continue;
                }
                UndoChoice::Cancel => {
                    cancelled = true;
                    break;
                }
                UndoChoice::Merge => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Merge requires both directories and an available saved previous destination. Use Keep both or Skip to keep this return available.",
                    ));
                }
                UndoChoice::KeepItem
                | UndoChoice::MoveToRecovery
                | UndoChoice::UseSavedVersion
                | UndoChoice::ReturnItem
                | UndoChoice::FinishStep
                | UndoChoice::RestorePreviousDestination => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "choose Replace, Keep both, Skip or Cancel for this restoration",
                    ));
                }
            }
            check_cancel(cancellation)?;
        }
        if let Some(path) = unavailable_return.as_deref() {
            unavailable_move::require_missing(path)?;
        }
        if let Some(guard) = &return_only {
            guard.require_unchanged()?;
        }
        changed_move::require_reviewed(&change)?;
        cross_move::require_reviewed(&change)?;
        state.require_directory_parents(id, ordinal)?;
        if relocation.is_some() {
            match directory_return::cross_filesystem::prepare(
                state,
                id,
                ordinal,
                &change,
                cancellation,
                relocation.as_ref(),
            )? {
                directory_return::cross_filesystem::Preparation::Unchanged => {}
                directory_return::cross_filesystem::Preparation::Expanded => {
                    before = None;
                    continue;
                }
                directory_return::cross_filesystem::Preparation::Returned => {
                    completed += 1;
                    paths.extend(change.reverse_path().map(Path::to_owned));
                    before = None;
                    continue;
                }
            }
        }
        if let Change::DirectoryMerge { .. } = &change {
            directory_return::require_ready(state, id, ordinal, &change)?;
        }
        let mut cleanup = None;
        let result = if matches!(change, Change::Copied { .. }) {
            reverse_copy(state, id, ordinal, &change, cancellation).map(|stage| {
                cleanup = stage;
            })
        } else {
            let intent = Pending::Undo {
                id,
                ordinal,
                change: change.clone(),
                stage: None,
                relocation: relocation.clone(),
                displaced: displaced.clone().map(Box::new),
            };
            if let Some(prepared) = owned_return_displaced {
                state.register_undo_displaced(&intent, &prepared)?;
                prepared.keep();
            } else {
                state.intent(&intent)?;
            }
            let mut reverse = || {
                reverse_change(
                    state,
                    id,
                    ordinal,
                    &change,
                    cancellation,
                    relocation.as_ref(),
                    displaced.as_ref(),
                )
                .map(|stage| {
                    cleanup = stage;
                })
            };
            if matches!(change, Change::SourceRemoved { .. }) {
                // Its return must be fully prepared before displacing newer
                // original-location work. The copy keeps the Undo context.
                reverse()
            } else if let Some(displaced) = &displaced {
                install_replacing(
                    change.reverse_path().unwrap(),
                    Some(&displaced.retained),
                    reverse,
                )
            } else {
                reverse()
            }
        };
        if let Err(error) = result {
            // Compensation must finish even after the user cancels the copy.
            if matches!(state.pending()?, Some(Pending::UndoSavedReturn { .. })) {
                // A consumed saved name cannot be replayed after an uncertain
                // return. Record its Recovery problem without hiding the live
                // filesystem error that caused this operation to stop.
                let _ = recover(state, &Cancellation::default());
            } else {
                recover(state, &Cancellation::default())?;
            }
            return Err(error);
        }
        sync_change(&change)?;
        if let Some(stage) = &cleanup
            && stage.identity.matches(&stage.directory)
        {
            fs::File::open(&stage.directory)?.sync_all()?;
        }
        let displaced = displaced.as_ref().map(snapshot_displaced).transpose()?;
        state.reversed_with_recovery(
            id,
            ordinal,
            &change,
            relocation.as_ref(),
            displaced.as_ref(),
        )?;
        if let Some(path) = unavailable_return {
            dismissed.push(path);
        }
        if let Some(guard) = return_only {
            unrestored_previous.push(guard.destination);
        } else if matches!(
            change,
            Change::DirectoryMerge {
                defer_previous: true,
                ..
            }
        ) {
            unrestored_previous.push(change.forward_path().to_owned());
        }
        completed += 1;
        if directory_return::moved(&change).is_some_and(|moved| moved.created.is_some()) {
            // The frame also completes its original CreatedFolder cleanup row.
            completed += 1;
        }
        if let Some(stage) = cleanup {
            clean_completed_stage(&stage)?;
        }
        if let Some(path) = change.reverse_path() {
            paths.push(path.to_owned());
        }
    }
    Ok(UndoOutcome {
        summary: OperationSummary { id, label, paths },
        completed,
        skipped,
        kept,
        dismissed,
        unrestored_previous,
        unremoved,
        cancelled,
        remaining: state.remaining_changes(id)?,
    })
}

fn clean_completed_stage(stage: &Stage) -> io::Result<()> {
    #[cfg(test)]
    abort_boundary::copy_boundary::completed(stage);
    // Modern roots belong to the receipt committed with reversal, which also
    // performs immediate cleanup. Never reopen its consumed name here.
    if stage.ownership.is_some() {
        return Ok(());
    }
    #[cfg(test)]
    abort_boundary::cleanup_boundary::point(stage);
    // A completed reversal has consumed its payload. Remove only the empty
    // owned directory; preserve any unexpected objects rather than unlinking them.
    if stage.identity.matches(&stage.directory) {
        fs::remove_dir(&stage.directory)?;
        sync_parent(&stage.directory)?;
    }
    Ok(())
}

struct RestorationTarget {
    incoming: PathBuf,
    original: PathBuf,
    source: UndoSource,
}
fn restoration_conflict(
    state: &Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    cancellation: &Cancellation,
) -> io::Result<Option<RestorationTarget>> {
    let target = match change {
        Change::Rename {
            original,
            current,
            identity,
            reviewed,
            ..
        } if changed_move::identity(state, id, ordinal, current, *identity, reviewed.as_ref())?
            .matches(current) =>
        {
            RestorationTarget {
                incoming: current.clone(),
                original: original.clone(),
                source: UndoSource::Current,
            }
        }
        Change::Trashed {
            original,
            current,
            identity,
            ..
        } if state
            .resolve_for(id, ordinal, current, *identity)?
            .matches(current) =>
        {
            RestorationTarget {
                incoming: current.clone(),
                original: original.clone(),
                source: UndoSource::Trash,
            }
        }
        Change::SourceRemoved {
            original,
            current,
            copy_identity,
            fingerprint,
            quarantine,
            reviewed,
            ..
        } if cross_move::identity(
            state,
            id,
            ordinal,
            current,
            *copy_identity,
            reviewed.as_ref(),
        )?
        .matches(current)
            && fingerprint.matches(current, cancellation)? =>
        {
            require_parent(state, id, ordinal, original, quarantine.parent_identity)?;
            RestorationTarget {
                incoming: current.clone(),
                original: original.clone(),
                source: UndoSource::Current,
            }
        }
        Change::RetainedOnly { path, retained, .. } => {
            require_backup(retained)?;
            RestorationTarget {
                incoming: retained.path(),
                original: path.clone(),
                source: UndoSource::PreviousDestination,
            }
        }
        Change::Copied {
            path,
            identity,
            fingerprint,
            retained: Some(retained),
            ..
        } => {
            require_backup(retained)?;
            if state
                .resolve_for(id, ordinal, path, *identity)?
                .matches(path)
            {
                match fingerprint.matches(path, cancellation) {
                    Ok(true) => return Ok(None),
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => return Err(error),
                    _ => {} // Unreadable or changed data must be preserved too.
                }
            }
            RestorationTarget {
                incoming: retained.path(),
                original: path.clone(),
                source: UndoSource::PreviousDestination,
            }
        }
        _ => return Ok(None),
    };
    Ok(fs::symlink_metadata(&target.original)
        .is_ok()
        .then_some(target))
}

fn reverse_change(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    cancellation: &Cancellation,
    relocation: Option<&Relocation>,
    displaced: Option<&recovery::Record>,
) -> io::Result<Option<Stage>> {
    match change {
        Change::DirectoryMerge { .. } => directory_return::finish(change),
        Change::Restored { .. } => trash::undo_restore(change, state, id, ordinal).map(|()| None),
        Change::Trashed {
            original,
            current,
            identity,
            info,
        } => {
            info.require()?;
            rename_verified(state, id, ordinal, current, original, *identity)?;
            info.remove()?;
            Ok(None)
        }
        Change::CreatedFolder { path, identity, .. } => {
            if !state
                .resolve_for(id, ordinal, path, *identity)?
                .matches(path)
            {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the created folder has been replaced or moved",
                ));
            }
            fs::remove_dir(path)?;
            Ok(None)
        }
        Change::Rename {
            original,
            current,
            identity,
            retained,
            reviewed,
            ..
        } => {
            if let Some(retained) = retained {
                require_backup(retained)?;
            }
            let returned_identity =
                changed_move::identity(state, id, ordinal, current, *identity, reviewed.as_ref())?;
            if let Some(reviewed) = reviewed {
                changed_move::return_reviewed(current, original, reviewed)?;
            } else {
                rename_verified(state, id, ordinal, current, original, *identity)?;
            }
            #[cfg(test)]
            abort_boundary::direct_displacement_boundary::cut("incoming-returned");
            if retained.is_some() {
                saved_return::restore(state, id, ordinal, change, Some(returned_identity))?;
            }
            Ok(retained.as_ref().map(|retained| retained.stage.clone()))
        }
        Change::RetainedOnly { retained, .. } => {
            require_backup(retained)?;
            saved_return::restore(state, id, ordinal, change, None)?;
            Ok(Some(retained.stage.clone()))
        }
        Change::SourceRemoved { .. } => restore_source(
            state,
            id,
            ordinal,
            change,
            cancellation,
            relocation,
            displaced,
        )
        .map(Some),
        Change::RemovedDirectory { .. } => restore_directory(state, id, ordinal, change).map(Some),
        Change::Copied { .. } => unreachable!(),
    }
}

fn rename_verified(
    state: &Store,
    id: u64,
    ordinal: u64,
    current: &Path,
    original: &Path,
    expected: Identity,
) -> io::Result<()> {
    let expected = state.resolve_for(id, ordinal, current, expected)?;
    if !expected.matches(current) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the item to restore has been replaced or moved",
        ));
    }
    rename_no_replace(current, original)?;
    if !expected.matches(original) {
        let _rollback = rename_no_replace(original, current);
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the item changed while being restored; its contents are preserved",
        ));
    }
    Ok(())
}

fn snapshot_displaced(record: &recovery::Record) -> io::Result<recovery::Record> {
    if let Some(ownership) = &record.retained.stage.ownership {
        // A modern archive already names its original roots and previous entry.
        // Verify those facts before refreshing its descriptive version. Never
        // turn the currently observed path into replacement ownership.
        use rustix::fs::{Mode, OFlags, open, openat};
        let stage = &record.retained.stage;
        let parent = fs::File::from(open(
            stage
                .directory
                .parent()
                .ok_or_else(|| invalid("Missing archive parent."))?,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        ownership.require_parent(&parent)?;
        let directory = fs::File::from(openat(
            &parent,
            stage
                .directory
                .file_name()
                .ok_or_else(|| invalid("Missing archive name."))?,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        ownership.require_directory(&directory)?;
        let previous = match openat(
            &directory,
            c"previous",
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(previous) => fs::File::from(previous),
            Err(rustix::io::Errno::NOENT) => return Ok(record.clone()),
            Err(error) => return Err(error.into()),
        };
        ownership.previous(&previous)?;
        let mut record = record.clone();
        let version = fingerprint::Version::from_metadata(&previous.metadata()?);
        record.retained.identity = version.identity;
        record.retained.physical_identity = None;
        record.retained.version = Some(version);
        directory.sync_all()?;
        return Ok(record);
    }
    if !record
        .retained
        .stage
        .identity
        .matches(&record.retained.stage.directory)
    {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the folder holding displaced work is unavailable; Undo needs recovery",
        ));
    }
    let mut record = record.clone();
    match fingerprint::Version::read(&record.retained.path()) {
        Ok(version) => {
            record.retained.identity = version.identity;
            record.retained.physical_identity = None;
            record.retained.version = Some(version);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::File::open(&record.retained.stage.directory)?.sync_all()?;
    Ok(record)
}

fn reverse_copy(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    cancellation: &Cancellation,
) -> io::Result<Option<Stage>> {
    let Change::Copied {
        path,
        identity,
        fingerprint,
        retained,
        ..
    } = change
    else {
        return Err(invalid("expected a copied item"));
    };
    let identity = state.resolve_for(id, ordinal, path, *identity)?;
    if let Some(retained) = retained.as_ref() {
        require_backup(retained)?;
        return copied::begin(state, id, ordinal, change, cancellation).map(Some);
    }
    let absent =
        matches!(fs::symlink_metadata(path), Err(e) if e.kind() == io::ErrorKind::NotFound);
    if absent {
        state.intent(&Pending::Undo {
            id,
            ordinal,
            change: change.clone(),
            stage: None,
            relocation: None,
            displaced: None,
        })?;
        return Ok(None);
    }
    if !identity.matches(path) || !fingerprint.matches(path, cancellation)? {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the copied item has changed; choose how to keep newer work",
        ));
    }
    copied::begin(state, id, ordinal, change, cancellation).map(Some)
}

pub(super) fn rollback_copy(state: &mut Store, id: u64, path: &Path) -> io::Result<()> {
    let Some((ordinal, change)) = state.last_change(id)? else {
        return Ok(());
    };
    if matches!(&change, Change::Copied { path: copied, .. } if copied == path) {
        let cleanup = reverse_copy(state, id, ordinal, &change, &Cancellation::default())?;
        sync_change(&change)?;
        state.reversed(id, ordinal)?;
        if let Some(stage) = cleanup {
            clean_completed_stage(&stage)?;
        }
    }
    Ok(())
}

fn require_backup(retained: &Retained) -> io::Result<()> {
    if retained.recovery
        || retained
            .version
            .as_ref()
            .is_some_and(|version| !version.matches(&retained.path()))
    {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the retained destination contains newer work requiring a recovery choice",
        ));
    }
    if !retained.stage.identity.matches(&retained.stage.directory)
        || !retained.observed_identity().matches(&retained.path())
    {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the retained destination is unavailable or has been replaced",
        ));
    }
    Ok(())
}

fn restore_source(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    cancellation: &Cancellation,
    relocation: Option<&Relocation>,
    displaced: Option<&recovery::Record>,
) -> io::Result<Stage> {
    let Change::SourceRemoved {
        original,
        current,
        copy_identity,
        fingerprint,
        source_fingerprint,
        quarantine,
        reviewed,
        ..
    } = change
    else {
        return Err(invalid("expected a moved source"));
    };
    require_parent(state, id, ordinal, original, quarantine.parent_identity)?;
    if displaced.is_some_and(|record| {
        record
            .retained
            .version
            .as_ref()
            .is_some_and(|version| !version.matches(original))
    }) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the destination changed while the conflict was being resolved",
        ));
    }
    let expected = cross_move::identity(
        state,
        id,
        ordinal,
        current,
        *copy_identity,
        reviewed.as_ref(),
    )?;
    if !expected.matches(current) || !fingerprint.matches(current, cancellation)? {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the moved file has changed; choose how to keep newer work",
        ));
    }
    let prepared = prepared_copy::prepare_copy(
        current,
        original.parent().unwrap(),
        cancellation,
        |created| {
            let stage = created.stage();
            state.register_undo_work(
                &Pending::Undo {
                    id,
                    ordinal,
                    change: change.clone(),
                    stage: Some(stage.clone()),
                    relocation: relocation.cloned(),
                    displaced: displaced.cloned().map(Box::new),
                },
                created,
            )
        },
        |_| {},
        || Ok(()),
    )?;
    if prepared.source_identity != expected || prepared.source_fingerprint != *fingerprint {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the moved file changed while its return was being prepared",
        ));
    }
    cross_move::require_reviewed(change)?;
    source_fingerprint.restore_metadata(&prepared.stage.payload())?;
    require_parent(state, id, ordinal, original, quarantine.parent_identity)?;
    state.intent(&Pending::Undo {
        id,
        ordinal,
        change: change.clone(),
        stage: Some(prepared.stage.clone()),
        relocation: relocation.cloned(),
        displaced: displaced.cloned().map(Box::new),
    })?;
    let _retained = prepared.temporary.keep();
    install_replacing(original, displaced.map(|record| &record.retained), || {
        rename_no_replace(&prepared.stage.payload(), original)
    })?;
    Ok(prepared.stage)
}

fn restore_directory(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<Stage> {
    use std::os::unix::fs::PermissionsExt;
    let Change::RemovedDirectory {
        path,
        mode,
        parent_identity,
        ..
    } = change
    else {
        return Err(invalid("expected a removed directory"));
    };
    require_parent(state, id, ordinal, path, Some(*parent_identity))?;
    let (mut stage, temporary) = Stage::prepare(path.parent().unwrap())?;
    fs::create_dir(stage.payload())?;
    fs::set_permissions(stage.payload(), fs::Permissions::from_mode(*mode | 0o700))?;
    let opened = fs::File::open(stage.payload())?;
    stage.payload_identity = Some(Identity::read(&stage.payload())?);
    state.intent(&Pending::Undo {
        id,
        ordinal,
        change: change.clone(),
        stage: Some(stage.clone()),
        relocation: None,
        displaced: None,
    })?;
    let _retained = temporary.keep();
    rename_no_replace(&stage.payload(), path)?;
    restore_directory_mode(path, stage.payload_identity.unwrap(), *mode, Some(&opened))?;
    Ok(stage)
}

fn require_parent(
    state: &Store,
    id: u64,
    ordinal: u64,
    path: &Path,
    expected: Option<Identity>,
) -> io::Result<()> {
    let expected = expected
        .map(|identity| state.resolve_for(id, ordinal, path.parent().unwrap(), identity))
        .transpose()?;
    if expected.is_some_and(|identity| {
        !fs::metadata(path.parent().unwrap())
            .is_ok_and(|metadata| Identity::from_metadata(&metadata) == identity)
    }) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the original folder is unavailable or has moved",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn recover_reverse(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    stage: Option<&Stage>,
    relocation: Option<&Relocation>,
    displaced: Option<&recovery::Record>,
    cancellation: &Cancellation,
) -> io::Result<()> {
    validate_change(change)?;
    state.require_directory_parents(id, ordinal)?;
    if let Change::DirectoryMerge { .. } = change {
        return directory_return::recover(state, id, ordinal, change);
    }
    // Older intents scoped file relocations to an exact path. Historical
    // descendants can become reachable again after another version is restored.
    let relocation = relocation.map(|record| Relocation {
        subtree: true,
        ..record.clone()
    });
    let relocation = relocation.as_ref();
    if let Some(relocation) = relocation {
        relocation.validate()?;
    }
    if let Some(record) = displaced {
        let changed_copy = matches!(change, Change::Copied { retained: None, .. });
        let original = record
            .context
            .as_ref()
            .map(|context| context.original.as_path());
        let supported = changed_copy
            || matches!(
                change,
                Change::Rename { .. }
                    | Change::Trashed { .. }
                    | Change::RetainedOnly { .. }
                    | Change::SourceRemoved { .. }
            );
        if !supported
            || original
                != change
                    .reverse_path()
                    .or_else(|| changed_copy.then(|| change.forward_path()))
            || !record.retained.recovery
        {
            return Err(invalid("invalid displaced Undo data"));
        }
        validate_stage(Some(&record.retained.stage), original.unwrap())?;
        if changed_copy {
            if stage.is_some() || relocation.is_some() {
                return Err(invalid("invalid changed-copy Undo intent"));
            }
            return changed_copy::recover_removal(state, id, ordinal, change, record, cancellation);
        }
    }
    // Destination bindings remain unchanged until the transaction that clears
    // this intent. Resolve the same physical item that the live reversal used;
    // its logical identity may have been adopted by an earlier Undo.
    let mut physical = change.clone();
    if let Change::Trashed {
        current, identity, ..
    }
    | Change::Restored {
        current, identity, ..
    } = &mut physical
    {
        *identity = state.resolve_for(id, ordinal, current, *identity)?;
    }
    trash::recover_reverse(&physical)?;
    if let Some(stage) = stage {
        validate_stage(Some(stage), change.forward_path())?;
    }
    if let Change::SourceRemoved { original, .. }
    | Change::RemovedDirectory { path: original, .. } = change
    {
        let returned = stage
            .and_then(|stage| stage.payload_identity)
            .is_some_and(|identity| identity.matches(original));
        if !returned && matches!(change, Change::SourceRemoved { .. }) && displaced.is_some() {
            return if stage.is_some_and(|stage| fs::symlink_metadata(stage.payload()).is_ok()) {
                staging::disposal::begin_undo(state, cancellation)
            } else {
                staging::disposal::begin_empty_undo(state, cancellation)
            };
        }
        if !returned
            && let Some(stage) = stage
            && fs::symlink_metadata(stage.payload()).is_ok()
        {
            if matches!(change, Change::SourceRemoved { .. }) {
                return staging::disposal::begin_undo(state, cancellation);
            }
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Interrupted Undo return data is preserved. Copy it or keep its work folder in Recovery before retrying Undo.",
            ));
        }
        if returned {
            if let Change::SourceRemoved {
                source_fingerprint, ..
            } = change
            {
                if let Some(ownership) = stage.and_then(|stage| stage.ownership.as_ref()) {
                    let file = fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(
                            (rustix::fs::OFlags::PATH | rustix::fs::OFlags::NOFOLLOW).bits() as i32,
                        )
                        .open(original)?;
                    ownership.require_payload(&file)?;
                }
                if !source_fingerprint.matches(original, &Cancellation::default())? {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "The returned Undo file is incomplete or changed. Its contents and original history remain protected for Recovery.",
                    ));
                }
            }
            if let Change::RemovedDirectory { mode, .. } = change {
                restore_directory_mode(
                    original,
                    stage.and_then(|stage| stage.payload_identity).unwrap(),
                    *mode,
                    None,
                )?;
            }
            sync_parent(original)?;
            let displaced = displaced.map(snapshot_displaced).transpose()?;
            state.reversed_with_recovery(id, ordinal, change, relocation, displaced.as_ref())?;
        } else if let Some(record) = displaced {
            rollback_displaced(state, record)?;
        } else {
            if let Some(stage) = stage {
                clean_stage(stage)?;
            }
            state.clear_intent()?;
            return Ok(());
        }
        if let Some(stage) = stage {
            if returned {
                clean_completed_stage(stage)?;
            } else {
                clean_stage(stage)?;
            }
        }
        return Ok(());
    }
    if matches!(change, Change::Copied { .. }) && (stage.is_some() || change.retained().is_some()) {
        if relocation.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Legacy Copy Undo relocation needs Recovery review.",
            ));
        }
        return copied::recover_legacy(state, id, ordinal, change, stage);
    }
    let reversed = if let Change::Rename {
        original,
        current,
        identity,
        retained,
        reviewed,
        ..
    } = change
    {
        let resolved =
            changed_move::identity(state, id, ordinal, current, *identity, reviewed.as_ref())?;
        resolved.matches(original)
            && retained.as_ref().map_or_else(
                || !resolved.matches(current),
                |retained| retained.restored_at(current),
            )
    } else {
        physical.reversed()
    };
    if reversed {
        if let Change::Trashed { info, .. } = change {
            info.remove()?;
        }
        sync_change(change)?;
        let displaced = displaced.map(snapshot_displaced).transpose()?;
        state.reversed_with_recovery(id, ordinal, change, relocation, displaced.as_ref())?;
        if let Some(stage) = stage {
            clean_completed_stage(stage)?;
        }
        if let Some(retained) = change.retained()
            && stage.is_none_or(|stage| stage.directory != retained.stage.directory)
        {
            clean_completed_stage(&retained.stage)?;
        }
        return Ok(());
    }
    if let Change::Rename {
        original,
        current,
        identity,
        retained: Some(retained),
        reviewed,
        ..
    } = change
    {
        let resolved =
            changed_move::identity(state, id, ordinal, current, *identity, reviewed.as_ref())?;
        if resolved.matches(original) && !resolved.matches(current) {
            sync_change(change)?;
            let displaced = displaced.map(snapshot_displaced).transpose()?;
            state.replace_change_with_recovery(
                id,
                ordinal,
                &Change::RetainedOnly {
                    path: current.clone(),
                    retained: retained.clone(),
                    replaced: Some(*identity),
                },
                relocation,
                displaced.as_ref(),
            )?;
            return Ok(());
        }
    }
    if let Some(record) = displaced {
        if state
            .pending()?
            .as_ref()
            .is_some_and(|value| store::undo_work::direct_preparation(value).is_some())
        {
            return staging::disposal::begin_empty_undo(state, cancellation);
        }
        rollback_displaced(state, record)?;
    } else {
        state.clear_intent()?;
    }
    if let Some(stage) = stage {
        clean_stage(stage)?;
    }
    Ok(())
}

fn rollback_displaced(state: &mut Store, record: &recovery::Record) -> io::Result<()> {
    let record = snapshot_displaced(record)?;
    let original = &record.context.as_ref().unwrap().original;
    let archive = match fs::symlink_metadata(record.retained.path()) {
        Ok(_) => match rename_no_replace(&record.retained.path(), original) {
            Ok(()) => {
                sync_parent(original)?;
                fs::File::open(&record.retained.stage.directory)?.sync_all()?;
                #[cfg(test)]
                abort_boundary::disposal_boundary::after_raw_return();
                false
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => true,
            Err(error) => return Err(error),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    state.clear_undo_with_recovery(archive.then_some(&record))?;
    clean_stage(&record.retained.stage)
}
