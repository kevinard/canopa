use super::*;

pub(super) enum Review {
    Ready,
    RestoreSaved(PathBuf),
    Kept(Vec<PathBuf>),
    Skipped(PathBuf),
    Dismissed {
        path: PathBuf,
        kept: Vec<PathBuf>,
        previous: bool,
    },
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

struct Available {
    version: fingerprint::Version,
    entry: crate::browse::Entry,
    fingerprint: Option<Fingerprint>,
}
struct Unavailable {
    observed: Result<fingerprint::Version, io::ErrorKind>,
    error: io::Error,
}
enum Current {
    Available(Available),
    Unavailable(Unavailable),
}

fn inspect(path: &Path, cancellation: &Cancellation) -> io::Result<Current> {
    let version = match fingerprint::Version::read(path) {
        Ok(version) => version,
        Err(error) => {
            return Ok(Current::Unavailable(Unavailable {
                observed: Err(error.kind()),
                error,
            }));
        }
    };
    let entry = match crate::browse::Entry::read(path) {
        Ok(entry) => entry,
        Err(error) => {
            return Ok(Current::Unavailable(Unavailable {
                observed: Ok(version),
                error,
            }));
        }
    };
    let fingerprint = if matches!(
        entry.kind,
        crate::browse::EntryKind::File | crate::browse::EntryKind::Symlink
    ) {
        match Fingerprint::read(path, cancellation) {
            Ok(fingerprint) => Some(fingerprint),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return Err(error),
            Err(error) => {
                if !version.matches(path) {
                    return Err(changed());
                }
                return Ok(Current::Unavailable(Unavailable {
                    observed: Ok(version),
                    error,
                }));
            }
        }
    } else {
        None
    };
    if !version.matches(path) {
        return Err(changed());
    }
    Ok(Current::Available(Available {
        version,
        entry,
        fingerprint,
    }))
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
    let Change::SourceRemoved {
        original,
        current,
        copy_identity,
        fingerprint,
        source_fingerprint,
        reviewed,
        ..
    } = change
    else {
        return Ok(Review::Ready);
    };
    let observed = match inspect(current, cancellation)? {
        Current::Available(observed) => observed,
        Current::Unavailable(unavailable) => {
            let retained = state
                .cross_copy(id, ordinal)?
                .and_then(|copy| copy.retained().cloned());
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
                    path: current.clone(),
                    kind: unavailable.error.kind(),
                    message: unavailable.error.to_string(),
                },
                existing: None,
                saved_previous: saved.as_ref().map(|(_, entry)| entry.clone()),
                return_destination: None,
                destination: original.clone(),
                source: UndoSource::UnavailableCrossMove,
            };
            let can_restore = question.can_restore_previous_destination();
            let choice = interact(question);
            if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
                return Ok(Review::Cancelled);
            }
            return match choice {
                UndoChoice::Skip => Ok(Review::Skipped(current.clone())),
                UndoChoice::RestorePreviousDestination if can_restore => {
                    unavailable_move::require_missing(current)?;
                    let mut retained = retained.unwrap();
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
                    let path = current.clone();
                    *change = Change::RetainedOnly {
                        path: path.clone(),
                        retained,
                        replaced: Some(*copy_identity),
                    };
                    Ok(Review::RestoreSaved(path))
                }
                UndoChoice::FinishStep => {
                    match inspect(current, cancellation)? {
                        Current::Unavailable(now)
                            if now.observed == unavailable.observed
                                && now.error.kind() == unavailable.error.kind() => {}
                        _ => return Err(changed()),
                    }
                    let previous = state
                        .cross_copy(id, ordinal)?
                        .is_some_and(|copy| copy.retained().is_some());
                    let kept = state
                        .keep_cross_item(id, ordinal, current, label)?
                        .into_iter()
                        .skip(1)
                        .collect();
                    Ok(Review::Dismissed {
                        path: current.clone(),
                        kept,
                        previous,
                    })
                }
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "choose Restore previous destination when available, Finish this step, Skip or Cancel for the unavailable cross-filesystem destination",
                )),
            };
        }
    };
    if identity(
        state,
        id,
        ordinal,
        current,
        *copy_identity,
        reviewed.as_ref(),
    )?
    .matches(current)
        && observed.fingerprint.as_ref() == Some(&*fingerprint)
    {
        return Ok(Review::Ready);
    }
    let question = UndoConflict {
        incoming: UndoItem::Available(observed.entry),
        existing: None,
        saved_previous: None,
        return_destination: None,
        destination: original.clone(),
        source: UndoSource::ChangedCrossMove,
    };
    let can_return = question.can_return_item();
    let choice = interact(question);
    if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
        return Ok(Review::Cancelled);
    }
    match choice {
        UndoChoice::Skip => Ok(Review::Skipped(current.clone())),
        UndoChoice::KeepItem => {
            if !observed.version.matches(current) {
                return Err(changed());
            }
            Ok(Review::Kept(
                state.keep_cross_item(id, ordinal, current, label)?,
            ))
        }
        UndoChoice::ReturnItem if can_return => {
            if !observed.version.matches(current) {
                return Err(changed());
            }
            let selected = observed.fingerprint.unwrap();
            *fingerprint = selected.clone();
            *source_fingerprint = selected;
            *reviewed = Some(observed.version);
            Ok(Review::Ready)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "choose Return this version, Keep item, Skip or Cancel for the changed cross-filesystem destination",
        )),
    }
}

pub(super) fn require_reviewed(change: &Change) -> io::Result<()> {
    if let Change::SourceRemoved {
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
fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "the reviewed destination version changed before it could be returned",
    )
}
