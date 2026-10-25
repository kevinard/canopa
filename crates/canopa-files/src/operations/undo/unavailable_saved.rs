use super::*;

#[derive(PartialEq, Eq)]
struct Observation {
    stage: Result<fingerprint::Version, io::ErrorKind>,
    saved: Option<Result<fingerprint::Version, io::ErrorKind>>,
}
impl Observation {
    fn read(retained: &Retained) -> io::Result<(Self, io::Error)> {
        let stage = fingerprint::Version::read(&retained.stage.directory);
        let saved = match &stage {
            Ok(version) if version.identity == retained.stage.identity => {
                Some(fingerprint::Version::read(&retained.path()))
            }
            _ => None,
        };
        let error = match (&stage, &saved) {
            (_, Some(Ok(_))) => return Err(changed()),
            (_, Some(Err(error))) | (Err(error), _) => {
                io::Error::new(error.kind(), error.to_string())
            }
            _ => io::Error::new(
                io::ErrorKind::WouldBlock,
                "the folder holding the saved version has been replaced",
            ),
        };
        Ok((
            Self {
                stage: stage.map_err(|error| error.kind()),
                saved: saved.map(|saved| saved.map_err(|error| error.kind())),
            },
            error,
        ))
    }
    fn require_unchanged(&self, retained: &Retained) -> io::Result<()> {
        if Self::read(retained).is_ok_and(|(now, _)| &now == self) {
            Ok(())
        } else {
            Err(changed())
        }
    }
}

pub(super) struct ReturnOnly {
    observed: Observation,
    retained: Retained,
    directory_guard: directory_return::Guard,
    pub destination: PathBuf,
}
impl ReturnOnly {
    pub fn retained(&self) -> &Retained {
        &self.retained
    }
    pub fn require_unchanged(&self) -> io::Result<()> {
        self.observed.require_unchanged(&self.retained)?;
        self.directory_guard.require_unchanged()
    }
}

pub(super) fn review(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    label: &str,
    change: &Change,
    cancellation: &Cancellation,
    interact: &mut dyn FnMut(UndoConflict) -> UndoChoice,
) -> io::Result<saved_version::Review> {
    let retained = change
        .retained()
        .ok_or_else(|| invalid("missing saved version"))?;
    let destination = change.forward_path();
    let (observed, error) = Observation::read(retained)?;
    let directory_guard = directory_return::Guard::read(state, id, ordinal)?;
    let directory = matches!(change, Change::DirectoryMerge { .. });
    let existing = crate::browse::Entry::read(destination).ok();
    let return_destination = match change {
        Change::Rename { original, .. } if existing.is_some() => Some(original.clone()),
        Change::DirectoryMerge { .. } if existing.is_some() => {
            Some(directory_return::roots(change)?.1.to_owned())
        }
        _ => None,
    };
    let can_return = return_destination.is_some();
    let choice = interact(UndoConflict {
        incoming: UndoItem::Unavailable {
            path: retained.path(),
            kind: error.kind(),
            message: error.to_string(),
        },
        existing,
        return_destination,
        saved_previous: None,
        destination: destination.to_owned(),
        source: if directory {
            UndoSource::UnavailableMergeRetained
        } else {
            UndoSource::UnavailableRetained
        },
    });
    if choice == UndoChoice::Cancel || cancellation.is_cancelled() {
        return Ok(saved_version::Review::Cancelled);
    }
    match choice {
        UndoChoice::Skip => Ok(saved_version::Review::Skipped(destination.to_owned())),
        UndoChoice::ReturnItem if can_return => {
            observed.require_unchanged(retained)?;
            directory_guard.require_unchanged()?;
            Ok(saved_version::Review::ReturnOnly(Box::new(ReturnOnly {
                observed,
                retained: retained.clone(),
                directory_guard,
                destination: destination.to_owned(),
            })))
        }
        UndoChoice::FinishStep => {
            observed.require_unchanged(retained)?;
            directory_guard.require_unchanged()?;
            if directory {
                let (source, destination, _, _) = directory_return::roots(change)?;
                let source = source.to_owned();
                let destination = destination.to_owned();
                let previous = state.finish_directory_return(id, ordinal)?;
                return Ok(saved_version::Review::DirectoryFinished {
                    source,
                    destination,
                    previous,
                });
            }
            let mut retained = retained.clone();
            retained.recovery = true;
            let protected = recovery::Record {
                context_moves: Vec::new(),
                retained,
                payload: false,
                container: false,
                context: Some(recovery::Context {
                    original: destination.to_owned(),
                    label: format!(
                        "Preserved after finishing an unavailable saved version during Undo {label}"
                    ),
                }),
            };
            state.dismiss_change(id, ordinal, Some(&protected))?;
            Ok(saved_version::Review::Finished(destination.to_owned()))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "choose Finish this step, Skip or Cancel for the unavailable saved version",
        )),
    }
}

fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "the saved version or its containing folder changed during review; review Undo again",
    )
}
