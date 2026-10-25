use super::*;

pub(super) enum Review {
    Ready,
    ReturnOnly(Box<unavailable_saved::ReturnOnly>),
    Finished(PathBuf),
    DirectoryFinished {
        source: PathBuf,
        destination: PathBuf,
        previous: Vec<PathBuf>,
    },
    Skipped(PathBuf),
    Cancelled,
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
    if matches!(
        change,
        Change::DirectoryMerge {
            defer_previous: true,
            ..
        }
    ) {
        return Ok(Review::Ready);
    }
    let destination = change.forward_path().to_owned();
    let Some(retained) = change.retained_mut() else {
        return Ok(Review::Ready);
    };
    if require_backup(retained).is_ok() {
        return Ok(Review::Ready);
    }
    let path = retained.path();
    match Identity::read(&retained.stage.directory) {
        Ok(identity) if identity == retained.stage.identity => {}
        Ok(_) => {
            return unavailable_saved::review(
                state,
                id,
                ordinal,
                label,
                change,
                cancellation,
                interact,
            );
        }
        Err(_) => {
            return unavailable_saved::review(
                state,
                id,
                ordinal,
                label,
                change,
                cancellation,
                interact,
            );
        }
    }
    let observed = match fingerprint::Version::read(&path) {
        Ok(observed) => observed,
        Err(_) => {
            return unavailable_saved::review(
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
    let incoming = match crate::browse::Entry::read(&path) {
        Ok(incoming) => incoming,
        Err(_) => {
            return unavailable_saved::review(
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
    if !observed.matches(&path) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the saved version changed while preparing its review",
        ));
    }
    let directory_guard = directory_return::Guard::read(state, id, ordinal)?;
    let question = UndoConflict {
        incoming: UndoItem::Available(incoming),
        existing: None,
        saved_previous: None,
        return_destination: None,
        destination: destination.clone(),
        source: UndoSource::ChangedRetained,
    };
    let can_restore = question.can_use_saved_version();
    let choice = interact(question);
    if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
        return Ok(Review::Cancelled);
    }
    match choice {
        UndoChoice::Skip => Ok(Review::Skipped(destination)),
        UndoChoice::UseSavedVersion if can_restore => {
            directory_guard.require_unchanged()?;
            if !retained.stage.identity.matches(&retained.stage.directory)
                || !observed.matches(&path)
            {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the saved version changed while its review was open",
                ));
            }
            retained.version = Some(observed);
            retained.recovery = false;
            Ok(Review::Ready)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "choose Restore this version, Skip or Cancel for the changed saved version",
        )),
    }
}
