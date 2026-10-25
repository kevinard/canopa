use super::*;

pub(super) enum Resolution {
    Skipped(PathBuf),
    Kept(Vec<PathBuf>),
    Removed,
    Cancelled,
}

pub(super) fn resolve(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    label: &str,
    cancellation: &Cancellation,
    interact: &mut dyn FnMut(UndoConflict) -> UndoChoice,
) -> io::Result<Option<Resolution>> {
    let Change::Copied {
        path,
        identity,
        fingerprint,
        retained: None,
        ..
    } = change
    else {
        return Ok(None);
    };
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
        Ok(_) => {}
    }
    if state
        .resolve_for(id, ordinal, path, *identity)?
        .matches(path)
    {
        match fingerprint.matches(path, cancellation) {
            Ok(true) => return Ok(None),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return Err(error),
            _ => {}
        }
    }
    let reviewed = replacement::Reviewed::observe(path)?;
    let version = reviewed.version().clone();
    let existing = crate::browse::Entry::read(path)?;
    let conflict = UndoConflict {
        incoming: UndoItem::Available(existing),
        existing: None,
        saved_previous: None,
        return_destination: None,
        destination: path.clone(),
        source: UndoSource::ChangedCopy,
    };
    let can_remove = conflict.can_move_to_recovery();
    let choice = interact(conflict);
    if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
        return Ok(Some(Resolution::Cancelled));
    }
    let decision = match choice {
        UndoChoice::Skip => Resolution::Skipped(path.clone()),
        UndoChoice::Cancel => Resolution::Cancelled,
        UndoChoice::KeepItem => Resolution::Kept(state.keep_item(id, ordinal, path, None)?),
        UndoChoice::MoveToRecovery => {
            if !can_remove {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Move to Recovery is available for files and links; keep this item in place.",
                ));
            }
            let prepared = displaced::Prepared::for_preservation(&reviewed)?;
            let record = recovery::Record {
                context_moves: Vec::new(),
                payload: false,
                container: false,
                retained: Retained {
                    replacement_facts: None,
                    physical_identity: None,
                    stage: prepared.stage().clone(),
                    identity: version.identity,
                    recovery: true,
                    version: Some(version),
                },
                context: Some(recovery::Context {
                    original: path.clone(),
                    label: format!("Preserved by Undo {label}"),
                }),
            };
            state.register_undo_displaced(
                &Pending::Undo {
                    id,
                    ordinal,
                    change: change.clone(),
                    stage: None,
                    relocation: None,
                    displaced: Some(Box::new(record.clone())),
                },
                &prepared,
            )?;
            #[cfg(test)]
            abort_boundary::changed_copy_boundary::cut("registered", &record);
            if let Err(error) = prepared.preserve_changed_copy(&reviewed) {
                recover(state, cancellation)?;
                return Err(error);
            }
            sync_parent(path)?;
            #[cfg(test)]
            abort_boundary::changed_copy_boundary::cut("preserved", &record);
            let record = snapshot_displaced(&record)?;
            state.reversed_with_recovery(id, ordinal, change, None, Some(&record))?;
            Resolution::Removed
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "choose Keep item, Move to Recovery, Skip or Cancel for this changed copy",
            ));
        }
    };
    Ok(Some(decision))
}

pub(super) fn recover_removal(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    record: &recovery::Record,
    cancellation: &Cancellation,
) -> io::Result<()> {
    let path = change.forward_path();
    if record
        .retained
        .original_identity()
        .matches(&record.retained.path())
    {
        sync_parent(path)?;
        let record = snapshot_displaced(record)?;
        state.reversed_with_recovery(id, ordinal, change, None, Some(&record))
    } else {
        // No legacy rollback rename: empty-copy compensation records its own
        // consumed namespaces and keeps canonical Undo available for fresh review.
        staging::disposal::begin_empty_undo(state, cancellation)
    }
}
