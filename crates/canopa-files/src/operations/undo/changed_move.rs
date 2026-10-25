use super::*;

pub(super) enum Review {
    Ready,
    Skipped(PathBuf),
    Kept(Vec<PathBuf>),
    Dismissed(PathBuf),
    RestoreSaved(PathBuf),
    Cancelled,
}

pub(super) fn identity(
    state: &Store,
    id: u64,
    ordinal: u64,
    path: &Path,
    logical: Identity,
    reviewed: Option<&fingerprint::Version>,
) -> io::Result<Identity> {
    reviewed.map_or_else(
        || state.resolve_for(id, ordinal, path, logical),
        |version| Ok(version.identity),
    )
}

pub(super) fn review(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    label: &str,
    change: &mut Change,
    cancellation: &Cancellation,
    interact: &mut dyn FnMut(UndoConflict) -> UndoChoice,
) -> io::Result<Review> {
    let Change::Rename {
        original,
        current,
        identity: logical,
        reviewed,
        ..
    } = change
    else {
        return Ok(Review::Ready);
    };
    if identity(state, id, ordinal, current, *logical, reviewed.as_ref())?.matches(current) {
        return Ok(Review::Ready);
    }
    let observed = match fingerprint::Version::read(current) {
        Ok(observed) => observed,
        Err(_) => {
            return unavailable_move::review(
                state,
                id,
                ordinal,
                label,
                change,
                cancellation,
                interact,
            );
        }
    };
    let incoming = crate::browse::Entry::read(current.as_path())?;
    if !observed.matches(current) {
        return Err(changed());
    }
    let question = UndoConflict {
        incoming: UndoItem::Available(incoming),
        existing: None,
        saved_previous: None,
        return_destination: None,
        destination: original.clone(),
        source: UndoSource::ChangedMove,
    };
    let can_return = question.can_return_item();
    let choice = interact(question);
    if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
        return Ok(Review::Cancelled);
    }
    match choice {
        UndoChoice::Skip => Ok(Review::Skipped(current.clone())),
        UndoChoice::ReturnItem if can_return => {
            if !observed.matches(current) {
                return Err(changed());
            }
            *reviewed = Some(observed);
            Ok(Review::Ready)
        }
        UndoChoice::KeepItem => {
            if !observed.matches(current) {
                return Err(changed());
            }
            let current = current.clone();
            let record = change.retained().cloned().map(|mut retained| {
                retained.recovery = true;
                recovery::Record {
                    context_moves: Vec::new(),
                    retained,
                    payload: false,
                    container: false,
                    context: Some(recovery::Context {
                        original: current.clone(),
                        label: format!("Preserved by keeping an item during Undo {label}"),
                    }),
                }
            });
            Ok(Review::Kept(state.keep_item(
                id,
                ordinal,
                &current,
                record.as_ref(),
            )?))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "choose Return this item, Keep item, Skip or Cancel for this changed move",
        )),
    }
}

pub(super) fn require_reviewed(change: &Change) -> io::Result<()> {
    if let Change::Rename {
        current,
        reviewed: Some(version),
        ..
    } = change
        && !version.matches(current)
    {
        return Err(changed());
    }
    Ok(())
}

pub(super) fn return_reviewed(
    current: &Path,
    original: &Path,
    version: &fingerprint::Version,
) -> io::Result<()> {
    // A displaced hard link can change ctime during our own extraction. The
    // full captured version was checked before that journaled displacement.
    if !version.matches_moved(current) {
        return Err(changed());
    }
    rename_no_replace(current, original)?;
    if !version.matches_moved(original) {
        let _rollback = rename_no_replace(original, current);
        return Err(changed());
    }
    Ok(())
}

fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "the reviewed item changed before it could be returned",
    )
}
