//! Review a lost Merge container before inspecting any of its pending children.
use super::*;
use std::collections::BTreeSet;

pub(in crate::operations::undo) enum Review {
    Ready,
    Deferred,
    Skipped {
        source: PathBuf,
        destination: PathBuf,
    },
    Finished {
        source: PathBuf,
        destination: PathBuf,
        previous: Vec<PathBuf>,
    },
    Cancelled,
}

#[derive(Clone, PartialEq, Eq)]
struct Observation {
    version: Result<fingerprint::Version, io::ErrorKind>,
    failure: Option<io::ErrorKind>,
}

struct Inspection {
    observed: Observation,
    problem: Option<UndoItem>,
}

fn inspect(path: &Path, expected: Identity) -> io::Result<Inspection> {
    let unavailable = |version, error: io::Error| Inspection {
        observed: Observation {
            version,
            failure: Some(error.kind()),
        },
        problem: Some(UndoItem::Unavailable {
            path: path.to_owned(),
            kind: error.kind(),
            message: error.to_string(),
        }),
    };
    let version = match fingerprint::Version::read(path) {
        Ok(version) => version,
        Err(error) => return Ok(unavailable(Err(error.kind()), error)),
    };
    let entry = match crate::browse::Entry::read(path) {
        Ok(entry) => entry,
        Err(error) => return Ok(unavailable(Ok(version), error)),
    };
    let problem = if version.identity != expected
        || entry.kind != crate::browse::EntryKind::Directory
    {
        Some(UndoItem::Available(entry))
    } else {
        if let Err(error) = fs::read_dir(path).and_then(|mut entries| entries.next().transpose()) {
            return Ok(unavailable(Ok(version), error));
        }
        None
    };
    if !version.matches(path) {
        return Err(changed());
    }
    Ok(Inspection {
        observed: Observation {
            version: Ok(version),
            failure: None,
        },
        problem,
    })
}

#[derive(PartialEq, Eq)]
struct TrashObservation {
    version: Result<fingerprint::Version, io::ErrorKind>,
    failure: Option<io::ErrorKind>,
}

fn trash_observation(marker: &Change) -> io::Result<Option<TrashObservation>> {
    if let Change::DirectoryMerge { change, .. } = marker
        && let Change::Trashed { info, .. } = change.as_ref()
    {
        let version = fingerprint::Version::read(info.path()).map_err(|error| error.kind());
        let failure = info.require().err().map(|error| error.kind());
        if let Ok(version) = &version
            && !version.matches(info.path())
        {
            return Err(changed());
        }
        return Ok(Some(TrashObservation { version, failure }));
    }
    Ok(None)
}

fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "a Merge folder or its Trash information changed during review; review Undo again",
    )
}

pub(in crate::operations::undo) fn review(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    deferred: &mut BTreeSet<u64>,
    cancellation: &Cancellation,
    mut interact: Option<&mut dyn FnMut(UndoConflict) -> UndoChoice>,
) -> io::Result<Review> {
    for (root, marker) in state.directory_roots(id, ordinal)? {
        if deferred.contains(&root) {
            return Ok(Review::Deferred);
        }
        let (source, destination, incoming, existing) = roots(&marker)?;
        let source_inspection = inspect(source, incoming)?;
        let destination_inspection = inspect(destination, existing)?;
        let trash = trash_observation(&marker)?;
        let problem = if let Some(item) = source_inspection.problem {
            Some((item, destination, UndoSource::MergeSource))
        } else if let Some(item) = destination_inspection.problem {
            Some((item, source, UndoSource::MergeDestination))
        } else if let Change::DirectoryMerge { change, .. } = &marker
            && let Change::Trashed { info, .. } = change.as_ref()
            && let Err(error) = info.require()
        {
            Some((
                UndoItem::Unavailable {
                    path: source.to_owned(),
                    kind: error.kind(),
                    message: format!("Trash information cannot be verified: {error}"),
                },
                destination,
                UndoSource::MergeSource,
            ))
        } else {
            None
        };
        let Some((item, other, role)) = problem else {
            continue;
        };
        let Some(interact) = interact.as_mut() else {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "a folder participating in this Merge needs interactive review",
            ));
        };
        let choice = interact(UndoConflict {
            incoming: item,
            existing: crate::browse::Entry::read(other).ok(),
            saved_previous: None,
            return_destination: None,
            destination: other.to_owned(),
            source: role,
        });
        if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
            return Ok(Review::Cancelled);
        }
        match choice {
            UndoChoice::Skip => {
                deferred.insert(root);
                return Ok(Review::Skipped {
                    source: source.to_owned(),
                    destination: destination.to_owned(),
                });
            }
            UndoChoice::FinishStep => {
                if inspect(source, incoming)?.observed != source_inspection.observed
                    || inspect(destination, existing)?.observed != destination_inspection.observed
                    || trash_observation(&marker)? != trash
                {
                    return Err(changed());
                }
                check_cancel(cancellation)?;
                let previous = state.finish_directory_return(id, root)?;
                return Ok(Review::Finished {
                    source: source.to_owned(),
                    destination: destination.to_owned(),
                    previous,
                });
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "choose Finish this Merge, Skip or Cancel for this changed Merge folder",
                ));
            }
        }
    }
    Ok(Review::Ready)
}
