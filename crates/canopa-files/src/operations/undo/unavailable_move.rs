use super::*;

pub(super) fn review(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    label: &str,
    change: &mut Change,
    cancellation: &Cancellation,
    interact: &mut dyn FnMut(UndoConflict) -> UndoChoice,
) -> io::Result<changed_move::Review> {
    let Change::Rename {
        current,
        original,
        identity,
        retained,
        ..
    } = change
    else {
        unreachable!()
    };
    let error = fingerprint::Version::read(current)
        .err()
        .ok_or_else(changed)?;
    let kind = error.kind();
    let saved = retained
        .as_ref()
        .filter(|retained| retained.stage.identity.matches(&retained.stage.directory))
        .and_then(|retained| {
            let path = retained.path();
            let version = fingerprint::Version::read(&path).ok()?;
            let entry = crate::browse::Entry::read(&path).ok()?;
            version.matches(&path).then_some((version, entry))
        });
    let question = UndoConflict {
        incoming: UndoItem::Unavailable {
            path: current.to_owned(),
            kind,
            message: error.to_string(),
        },
        existing: None,
        saved_previous: saved.as_ref().map(|(_, entry)| entry.clone()),
        return_destination: None,
        destination: original.to_owned(),
        source: UndoSource::UnavailableMove,
    };
    let can_restore = question.can_restore_previous_destination();
    let choice = interact(question);
    if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
        return Ok(changed_move::Review::Cancelled);
    }
    match choice {
        UndoChoice::Skip => Ok(changed_move::Review::Skipped(current.to_owned())),
        UndoChoice::RestorePreviousDestination if can_restore => {
            require_missing(current)?;
            let path = current.clone();
            let mut retained = retained.as_ref().unwrap().clone();
            let version = saved.unwrap().0;
            if !retained.stage.identity.matches(&retained.stage.directory)
                || !version.matches(&retained.path())
            {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the saved previous destination changed during review; review Undo again",
                ));
            }
            retained.version = Some(version);
            retained.recovery = false;
            *change = Change::RetainedOnly {
                path: path.clone(),
                retained,
                replaced: Some(*identity),
            };
            Ok(changed_move::Review::RestoreSaved(path))
        }
        UndoChoice::FinishStep => {
            require_unavailable(current, kind)?;
            let current = current.clone();
            let protected = change.retained().cloned().map(|mut retained| {
                retained.recovery = true;
                recovery::Record {
                    context_moves: Vec::new(),
                    retained,
                    payload: false,
                    container: false,
                    context: Some(recovery::Context {
                        original: current.clone(),
                        label: format!(
                            "Preserved after finishing an unavailable item during Undo {label}"
                        ),
                    }),
                }
            });
            state.dismiss_change(id, ordinal, protected.as_ref())?;
            Ok(changed_move::Review::Dismissed(current))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "choose Restore previous destination when available, Finish this step, Skip or Cancel for this unavailable moved item",
        )),
    }
}

pub(super) fn require_missing(path: &Path) -> io::Result<()> {
    require_unavailable(path, io::ErrorKind::NotFound)
}

fn require_unavailable(path: &Path, kind: io::ErrorKind) -> io::Result<()> {
    match fingerprint::Version::read(path) {
        Err(error) if error.kind() == kind => Ok(()),
        _ => Err(changed()),
    }
}

fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "the moved item became available or its lookup error changed; review Undo again",
    )
}
