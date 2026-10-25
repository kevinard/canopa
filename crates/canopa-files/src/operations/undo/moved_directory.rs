//! A decomposed directory move returns its recorded children, not a new scan.
use super::*;

pub(super) enum Review {
    Ready,
    Merged,
    Skipped { first: u64, path: PathBuf },
    Cancelled,
}

pub(super) fn review(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    cancellation: &Cancellation,
    resolve: Option<&mut dyn FnMut(UndoConflict) -> UndoChoice>,
) -> io::Result<Review> {
    let Change::RemovedDirectory {
        path,
        parent_identity,
        moved: Some(moved),
        ..
    } = change
    else {
        return Ok(Review::Ready);
    };
    let existing = match crate::browse::Entry::read(path) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return prepare_owned(state, id, ordinal, change, cancellation, None);
        }
        Err(error) => return Err(error),
    };
    let Some(resolve) = resolve else {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the original folder location needs an Undo choice",
        ));
    };
    require_parent(state, id, ordinal, path, Some(*parent_identity))?;
    let incoming_version = fingerprint::Version::read(&moved.current)?;
    if incoming_version.identity != moved.identity {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the moved folder has changed; its recorded files are preserved",
        ));
    }
    let existing_version = fingerprint::Version::read(path)?;
    let question = UndoConflict {
        incoming: UndoItem::Available(crate::browse::Entry::read(&moved.current)?),
        existing: Some(existing),
        return_destination: None,
        saved_previous: None,
        destination: path.clone(),
        source: UndoSource::Current,
    };
    let can_merge = question.can_merge();
    let choice = resolve(question);
    if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
        return Ok(Review::Cancelled);
    }
    match choice {
        UndoChoice::Skip => Ok(Review::Skipped {
            first: moved.first,
            path: moved.current.clone(),
        }),
        UndoChoice::Merge if can_merge => {
            state.require_directory_parents(id, ordinal)?;
            require_parent(state, id, ordinal, path, Some(*parent_identity))?;
            if !incoming_version.matches(&moved.current) || !existing_version.matches(path) {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the folders changed during directory-return review",
                ));
            }
            let marker = Change::DirectoryMerge {
                destination_mode: None,
                change: Box::new(change.clone()),
                destination_identity: existing_version.identity,
                destination_original: directory_return::root_original(
                    path,
                    existing_version.identity,
                )?,
                defer_previous: false,
                destination_owned: false,
            };
            state.begin_moved_directory_return(id, ordinal, &marker)?;
            Ok(Review::Merged)
        }
        UndoChoice::KeepBoth => {
            if !incoming_version.matches(&moved.current) || !existing_version.matches(path) {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the folders changed during directory-return review",
                ));
            }
            let target = transfer::unique_name(path)?;
            let relocation = Relocation {
                from: path.clone(),
                to: target,
                subtree: true,
            };
            prepare_owned(state, id, ordinal, change, cancellation, Some(relocation))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "choose Merge, Keep both, Skip or Cancel for this directory return",
        )),
    }
}

fn prepare_owned(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    cancellation: &Cancellation,
    relocation: Option<Relocation>,
) -> io::Result<Review> {
    use std::os::unix::fs::PermissionsExt;
    let Change::RemovedDirectory {
        path,
        parent_identity,
        mode,
        moved: Some(moved),
        ..
    } = change
    else {
        return Err(invalid("expected a moved directory"));
    };
    require_parent(state, id, ordinal, path, Some(*parent_identity))?;
    state.require_directory_parents(id, ordinal)?;
    let source = fingerprint::Version::read(&moved.current)?;
    directory_return::require_directory(&moved.current, moved.identity)?;
    let target = relocation
        .as_ref()
        .map_or(path.as_path(), |relocation| relocation.to.as_path());
    let (mut stage, mut temporary) = Stage::prepare(target.parent().unwrap())?;
    fs::create_dir(stage.payload())?;
    fs::set_permissions(stage.payload(), fs::Permissions::from_mode(*mode | 0o700))?;
    stage.payload_identity = Some(Identity::read(&stage.payload())?);
    let mut incoming = change.clone();
    if let Change::RemovedDirectory { path, .. } = &mut incoming {
        *path = target.to_owned();
    }
    let marker = Change::DirectoryMerge {
        destination_mode: Some(*mode),
        change: Box::new(incoming),
        destination_identity: stage.payload_identity.unwrap(),
        destination_original: directory_return::root_original(
            &stage.payload(),
            stage.payload_identity.unwrap(),
        )?,
        defer_previous: false,
        destination_owned: true,
    };
    if !source.matches(&moved.current) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the moved directory changed during destination preparation",
        ));
    }
    require_parent(state, id, ordinal, path, Some(*parent_identity))?;
    check_cancel(cancellation)?;
    fs::File::open(stage.payload())?.sync_all()?;
    fs::File::open(&stage.directory)?.sync_all()?;
    temporary.disable_cleanup(true);
    state.prepare_moved_directory_destination(id, ordinal, &marker, &stage, relocation.as_ref())?;
    directory_return::cross_filesystem::recover_destination(
        state,
        id,
        ordinal,
        &marker,
        &stage,
        relocation.as_ref(),
    )?;
    Ok(Review::Merged)
}
