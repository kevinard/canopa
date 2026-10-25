use super::*;

pub(super) enum Review {
    Ready,
    Kept(Vec<PathBuf>),
    Finished(PathBuf),
    Skipped(PathBuf),
    Cancelled,
}

enum Inspection {
    Empty,
    Available {
        version: fingerprint::Version,
        entry: crate::browse::Entry,
        source: UndoSource,
    },
    Unavailable {
        observed: Result<fingerprint::Version, io::ErrorKind>,
        error: io::Error,
    },
}

fn inspect(path: &Path, expected: Identity) -> io::Result<Inspection> {
    let version = match fingerprint::Version::read(path) {
        Ok(version) => version,
        Err(error) => {
            return Ok(Inspection::Unavailable {
                observed: Err(error.kind()),
                error,
            });
        }
    };
    let entry = match crate::browse::Entry::read(path) {
        Ok(entry) => entry,
        Err(error) => {
            return Ok(Inspection::Unavailable {
                observed: Ok(version),
                error,
            });
        }
    };
    let source =
        if version.identity != expected || entry.kind != crate::browse::EntryKind::Directory {
            UndoSource::ChangedCreatedFolder
        } else {
            let first = fs::read_dir(path).and_then(|mut entries| entries.next().transpose());
            match first {
                Ok(first) => {
                    if !version.matches(path) {
                        return Err(changed());
                    }
                    if first.is_none() {
                        return Ok(Inspection::Empty);
                    }
                }
                Err(error) => {
                    return Ok(Inspection::Unavailable {
                        observed: Ok(version),
                        error,
                    });
                }
            }
            UndoSource::NonemptyFolder
        };
    if !version.matches(path) {
        return Err(changed());
    }
    Ok(Inspection::Available {
        version,
        entry,
        source,
    })
}

pub(super) fn review(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    cancellation: &Cancellation,
    interact: Option<&mut dyn FnMut(UndoConflict) -> UndoChoice>,
) -> io::Result<Review> {
    let Change::CreatedFolder { path, identity, .. } = change else {
        return Ok(Review::Ready);
    };
    let expected = state.resolve_for(id, ordinal, path, *identity)?;
    let inspection = inspect(path, expected)?;
    let Some(interact) = interact else {
        return match inspection {
            Inspection::Empty => Ok(Review::Ready),
            Inspection::Unavailable { error, .. } => Err(error),
            Inspection::Available { source, .. } => Err(io::Error::new(
                if source == UndoSource::NonemptyFolder {
                    io::ErrorKind::DirectoryNotEmpty
                } else {
                    io::ErrorKind::WouldBlock
                },
                "the created-folder location needs interactive review",
            )),
        };
    };
    match inspection {
        Inspection::Empty => Ok(Review::Ready),
        Inspection::Available {
            version,
            entry,
            source,
        } => {
            let choice = interact(UndoConflict {
                incoming: UndoItem::Available(entry),
                existing: None,
                saved_previous: None,
                return_destination: None,
                destination: path.clone(),
                source,
            });
            if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
                return Ok(Review::Cancelled);
            }
            match choice {
                UndoChoice::Skip => Ok(Review::Skipped(path.clone())),
                UndoChoice::KeepItem => {
                    if !version.matches(path) {
                        return Err(changed());
                    }
                    Ok(Review::Kept(state.keep_item(id, ordinal, path, None)?))
                }
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "choose Keep item, Skip or Cancel for the created-folder review",
                )),
            }
        }
        Inspection::Unavailable { observed, error } => {
            let choice = interact(UndoConflict {
                incoming: UndoItem::Unavailable {
                    path: path.clone(),
                    kind: error.kind(),
                    message: error.to_string(),
                },
                existing: None,
                saved_previous: None,
                return_destination: None,
                destination: path.clone(),
                source: UndoSource::UnavailableCreatedFolder,
            });
            if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
                return Ok(Review::Cancelled);
            }
            match choice {
                UndoChoice::Skip => Ok(Review::Skipped(path.clone())),
                UndoChoice::FinishStep => {
                    match inspect(path, expected)? {
                        Inspection::Unavailable {
                            observed: now,
                            error: failure,
                        } if now == observed && failure.kind() == error.kind() => {}
                        _ => return Err(changed()),
                    }
                    // Only the selected folder step is dismissed. Ancestors
                    // remain in history, protected by the in-batch path barrier.
                    state.dismiss_change(id, ordinal, None)?;
                    Ok(Review::Finished(path.clone()))
                }
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "choose Finish this step, Skip or Cancel for the unavailable created folder",
                )),
            }
        }
    }
}

fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "the created-folder location changed during review; review Undo again",
    )
}
