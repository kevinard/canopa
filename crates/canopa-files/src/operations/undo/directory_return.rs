//! A directory return expands into journalled child returns in the same entry.
use super::*;
pub(in crate::operations) mod cross_filesystem;
pub(super) mod roots_review;

pub(super) struct Guard(Vec<(Change, fingerprint::Version, fingerprint::Version)>);
impl Guard {
    pub fn read(state: &Store, id: u64, ordinal: u64) -> io::Result<Self> {
        let mut roots = Vec::new();
        for (_, marker) in state.directory_roots(id, ordinal)? {
            require_roots(&marker)?;
            let (source, destination, _, _) = self::roots(&marker)?;
            let incoming = fingerprint::Version::read(source)?;
            let existing = fingerprint::Version::read(destination)?;
            roots.push((marker, incoming, existing));
        }
        Ok(Self(roots))
    }
    pub fn require_unchanged(&self) -> io::Result<()> {
        for (marker, incoming, existing) in &self.0 {
            require_roots(marker)?;
            let (source, destination, _, _) = roots(marker)?;
            if !incoming.matches(source) || !existing.matches(destination) {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "a Merge folder changed during saved-version review; review Undo again",
                ));
            }
        }
        Ok(())
    }
}

pub(super) enum Review {
    Ready,
    Expanded,
    Kept { path: PathBuf, previous: bool },
    Skipped(PathBuf),
    Cancelled,
}

pub(in crate::operations) fn moved(change: &Change) -> Option<&MovedDirectory> {
    match change {
        Change::DirectoryMerge { change, .. } => match change.as_ref() {
            Change::RemovedDirectory { moved, .. } => moved.as_ref(),
            _ => None,
        },
        _ => None,
    }
}

fn owns_source(change: &Change) -> bool {
    moved(change).is_none_or(|moved| moved.created.is_some())
}

pub(super) fn review_remainder(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    cancellation: &Cancellation,
    resolve: Option<&mut dyn FnMut(UndoConflict) -> UndoChoice>,
) -> io::Result<Review> {
    if !matches!(change, Change::DirectoryMerge { .. }) {
        return Ok(Review::Ready);
    }
    require_roots(change)?;
    let (source, destination, _, _) = roots(change)?;
    if state.directory_children_remaining(id, ordinal)? {
        return Ok(Review::Skipped(source.to_owned()));
    }
    if !owns_source(change) {
        return Ok(Review::Ready);
    }
    if fs::read_dir(source)?.next().transpose()?.is_none() {
        return Ok(Review::Ready);
    }
    let Some(resolve) = resolve else {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the incoming folder still contains work that needs a decision",
        ));
    };
    let incoming = fingerprint::Version::read(source)?;
    let existing = fingerprint::Version::read(destination)?;
    let choice = resolve(UndoConflict {
        incoming: UndoItem::Available(crate::browse::Entry::read(source)?),
        existing: Some(crate::browse::Entry::read(destination)?),
        saved_previous: None,
        return_destination: None,
        destination: destination.to_owned(),
        source: UndoSource::MergeRemainder,
    });
    if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
        return Ok(Review::Cancelled);
    }
    match choice {
        UndoChoice::Skip => Ok(Review::Skipped(source.to_owned())),
        UndoChoice::KeepItem | UndoChoice::Merge => {
            state.require_directory_parents(id, ordinal)?;
            require_roots(change)?;
            if !incoming.matches(source) || !existing.matches(destination) {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the folders changed during remaining-items review",
                ));
            }
            check_cancel(cancellation)?;
            if choice == UndoChoice::Merge {
                let children = prepare_children(state, change, &incoming, &existing, cancellation)?;
                state.add_directory_items(id, ordinal, change, &children)?;
                return Ok(Review::Expanded);
            }
            state.keep_directory_remainder(id, ordinal, change)?;
            Ok(Review::Kept {
                path: source.to_owned(),
                previous: change.retained().is_some(),
            })
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "choose Merge remaining items, Keep remaining items, Skip or Cancel",
        )),
    }
}

pub(in crate::operations) fn roots(
    change: &Change,
) -> io::Result<(&Path, &Path, Identity, Identity)> {
    let Change::DirectoryMerge {
        change,
        destination_identity,
        ..
    } = change
    else {
        return Err(invalid("expected a directory return"));
    };
    let (original, current, identity) = match change.as_ref() {
        Change::Rename {
            original,
            current,
            identity,
            ..
        }
        | Change::Trashed {
            original,
            current,
            identity,
            ..
        } => (original, current, identity),
        Change::RemovedDirectory {
            path,
            moved: Some(moved),
            ..
        } => (path, &moved.current, &moved.identity),
        _ => return Err(invalid("invalid directory return source")),
    };
    Ok((current, original, *identity, *destination_identity))
}

pub(in crate::operations) fn require_roots(change: &Change) -> io::Result<()> {
    if let Change::DirectoryMerge { change, .. } = change
        && let Change::Trashed { info, .. } = change.as_ref()
    {
        info.require()?;
    }
    let (source, destination, source_identity, destination_identity) = roots(change)?;
    require_directory(source, source_identity)?;
    require_directory(destination, destination_identity)?;
    if let Change::DirectoryMerge {
        destination_original: Some(original),
        ..
    } = change
    {
        let file = fs::File::from(rustix::fs::open(
            destination,
            rustix::fs::OFlags::PATH
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?);
        original.require(&file)?;
    }
    Ok(())
}

pub(super) fn root_original(
    path: &Path,
    identity: Identity,
) -> io::Result<Option<original::Original>> {
    let file = fs::File::from(rustix::fs::open(
        path,
        rustix::fs::OFlags::PATH
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    if Identity::from_metadata(&file.metadata()?) != identity {
        return Err(invalid(
            "The selected return folder changed before its original proof was saved.",
        ));
    }
    Ok(original::Original::capture(&file).ok())
}

pub(in crate::operations) fn require_directory(path: &Path, identity: Identity) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || Identity::from_metadata(&metadata) != identity {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "a directory participating in this return has moved or changed",
        ));
    }
    Ok(())
}

pub(super) fn begin(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    versions: (&fingerprint::Version, &fingerprint::Version),
    cancellation: &Cancellation,
    return_only: Option<&unavailable_saved::ReturnOnly>,
) -> io::Result<()> {
    let (incoming, existing) = versions;
    if let Some(guard) = return_only {
        guard.require_unchanged()?;
    }
    let mut parent = change.clone();
    match &mut parent {
        Change::Rename {
            identity,
            reviewed,
            retained,
            ..
        } => {
            *identity = incoming.identity;
            *reviewed = None;
            if let Some(guard) = return_only {
                *retained = Some(guard.retained().clone());
            }
        }
        Change::Trashed { identity, .. } => *identity = incoming.identity,
        _ => return Err(invalid("Merge is available for returning directories")),
    }
    let marker = Change::DirectoryMerge {
        destination_mode: None,
        change: Box::new(parent),
        destination_identity: existing.identity,
        destination_original: root_original(
            change
                .reverse_path()
                .ok_or_else(|| invalid("Missing Merge destination."))?,
            existing.identity,
        )?,
        defer_previous: return_only.is_some(),
        destination_owned: false,
    };
    let children = prepare_children(state, &marker, incoming, existing, cancellation)?;
    state.require_directory_parents(id, ordinal)?;
    if let Some(guard) = return_only {
        guard.require_unchanged()?;
    }
    state.begin_directory_return(id, ordinal, &marker, &children)
}

fn prepare_children(
    state: &Store,
    marker: &Change,
    incoming: &fingerprint::Version,
    existing: &fingerprint::Version,
    cancellation: &Cancellation,
) -> io::Result<Vec<Change>> {
    require_roots(marker)?;
    let (source, destination, _, _) = roots(marker)?;
    if !incoming.matches(source) || !existing.matches(destination) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the folders changed during Merge review",
        ));
    }
    let mut entries: Vec<_> = fs::read_dir(source)?.collect::<io::Result<_>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut children = Vec::with_capacity(entries.len());
    for entry in entries {
        check_cancel(cancellation)?;
        let current = entry.path();
        let original = destination.join(entry.file_name());
        let identity = Identity::read(&current)?;
        // Only a direct rename preserves this container's generation. A
        // cross-filesystem return uses its created-copy proof and rebinds its
        // owners during migration; it must not apply the old root's proof to
        // that new container when completing the return.
        let saved = if identity.device == existing.identity.device {
            state
                .owned_saved_stage(&current, identity)?
                .map(|record| {
                    entry_move::Selection::open(&current, &original)?
                        .saved_movement(
                            &original,
                            &record,
                            if matches!(
                                marker,
                                Change::DirectoryMerge {
                                    destination_mode: Some(_),
                                    ..
                                }
                            ) {
                                StageParentAccess::Created(existing.identity)
                            } else {
                                StageParentAccess::Existing
                            },
                            None,
                            false,
                        )
                        .map(saved_stage::Movement::into_return)
                })
                .transpose()?
        } else {
            None
        };
        children.push(Change::Rename {
            move_facts: None,
            saved,
            original,
            current,
            identity,
            retained: None,
            reviewed: None,
        });
    }
    check_cancel(cancellation)?;
    if !incoming.matches(source) || !existing.matches(destination) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the folders changed while preparing Merge",
        ));
    }
    Ok(children)
}

pub(in crate::operations) fn require_ready(
    state: &Store,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    require_roots(change)?;
    if state.directory_children_remaining(id, ordinal)?
        || state.moved_directory_source_in_use(id, ordinal)?
        || (owns_source(change)
            && fs::read_dir(change.forward_path())?
                .next()
                .transpose()?
                .is_some())
    {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "this directory return still has pending or newly arrived items",
        ));
    }
    if !matches!(
        change,
        Change::DirectoryMerge {
            defer_previous: true,
            ..
        }
    ) && let Some(retained) = change.retained()
    {
        require_backup(retained)?;
    }
    Ok(())
}

pub(in crate::operations) fn finish(change: &Change) -> io::Result<Option<Stage>> {
    let (source, destination, source_identity, destination_identity) = roots(change)?;
    require_directory(destination, destination_identity)?;
    let inner = match change {
        Change::DirectoryMerge { change, .. } => change,
        _ => unreachable!(),
    };
    let deferred = matches!(
        change,
        Change::DirectoryMerge {
            defer_previous: true,
            ..
        }
    );
    if !owns_source(change) {
        return Ok(None);
    }
    if !deferred && let Some(retained) = inner.retained() {
        if retained.restored_at(source) {
            return Ok(Some(retained.stage.clone()));
        }
        require_backup(retained)?;
    }
    match fs::symlink_metadata(source) {
        Ok(_) => {
            require_directory(source, source_identity)?;
            fs::remove_dir(source)?;
            sync_parent(source)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    if deferred {
        return Ok(None);
    }
    if let Some(retained) = inner.retained() {
        rename_no_replace(&retained.path(), source)?;
    }
    if let Change::Trashed { info, .. } = inner.as_ref() {
        info.remove()?;
    }
    Ok(inner.retained().map(|retained| retained.stage.clone()))
}

pub(in crate::operations) fn recover(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    let (source, _, identity, _) = roots(change)?;
    if identity.matches(source) && require_ready(state, id, ordinal, change).is_err() {
        // No root removal happened. Keep the persistent session for fresh review.
        return state.clear_intent();
    }
    let cleanup = finish(change)?;
    sync_change(change)?;
    state.reversed_with_recovery(id, ordinal, change, None, None)?;
    if let Some(stage) = cleanup {
        clean_completed_stage(&stage)?;
    }
    Ok(())
}
