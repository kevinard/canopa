//! Ordinary Rename claims its entry into private work before publishing it.
//! Opened parents survive path replacement; unexpected claimed entries are
//! returned without replacement, or retained in the durable journal for review.
use super::*;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use std::ffi::{OsStr, OsString};

struct Parent {
    file: fs::File,
    path: PathBuf,
    identity: Identity,
}

/// Immutable facts from the already-pinned application selection. Passing
/// these into execution must not resolve the paths into new authority.
pub(super) struct Bindings {
    pub source: Identity,
    pub source_parent: Identity,
    pub destination_parent: Identity,
}

impl Parent {
    fn open(path: &Path) -> io::Result<Self> {
        let file = fs::File::from(rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        Ok(Self {
            identity: Identity::from_metadata(&file.metadata()?),
            file,
            path: path.to_owned(),
        })
    }

    fn current(&self) -> bool {
        fs::metadata(&self.path)
            .is_ok_and(|metadata| Identity::from_metadata(&metadata) == self.identity)
    }
}

pub(super) struct Prepared {
    source: Parent,
    destination: Parent,
    // Pin the selected inode, including a final symlink itself, against reuse.
    _entry: fs::File,
    identity: Identity,
    original: PathBuf,
    current: PathBuf,
    stage: Stage,
    stage_file: fs::File,
    stage_name: OsString,
    payload_name: OsString,
}

impl Prepared {
    pub(super) fn open(
        original: &Path,
        current: &Path,
        identity: Identity,
        bindings: Option<&Bindings>,
    ) -> io::Result<Self> {
        let source = Parent::open(original.parent().unwrap())?;
        let destination = Parent::open(current.parent().unwrap())?;
        if bindings.is_some_and(|bindings| {
            source.identity != bindings.source_parent
                || destination.identity != bindings.destination_parent
        }) {
            return Err(changed());
        }
        if source.identity == destination.identity && original.file_name() == current.file_name() {
            return Err(invalid("source and destination are the same entry"));
        }
        let entry = fs::File::from(rustix::fs::openat(
            &source.file,
            original.file_name().unwrap(),
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let metadata = entry.metadata()?;
        if Identity::from_metadata(&metadata) != identity
            || !source.current()
            || !destination.current()
        {
            return Err(changed());
        }
        // tempfile supplies collision-resistant names; creation and opening use
        // the held directory, never a re-resolved parent pathname. Cleanup is
        // explicitly empty-only and descriptor-relative below.
        let temporary = tempfile::Builder::new()
            .prefix(".canopa-files-stage-")
            .disable_cleanup(true)
            .make_in(&destination.path, |path| -> io::Result<fs::File> {
                let name = path.file_name().unwrap();
                rustix::fs::mkdirat(
                    &destination.file,
                    name,
                    Mode::RUSR | Mode::WUSR | Mode::XUSR,
                )?;
                Ok(fs::File::from(rustix::fs::openat(
                    &destination.file,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )?))
            })?;
        let (stage_file, path) = temporary.into_parts();
        let stage = Stage {
            replacement: None,
            entry_move: None,
            source_claim: None,
            directory_claim: None,
            ownership: None,
            parent_access: Some(StageParentAccess::Existing),
            directory: path.to_path_buf(),
            identity: Identity::from_metadata(&stage_file.metadata()?),
            payload_identity: None,
            parent_identity: Some(destination.identity),
            // A readonly directory can be renamed within its parent without
            // permission to rewrite its '..' entry. Never add an extra parent
            // change merely for staging, and never broaden its permissions.
            sibling_payload: metadata.is_dir(),
        };
        Ok(Self {
            stage_name: stage.directory.file_name().unwrap().to_owned(),
            payload_name: stage.payload().file_name().unwrap().to_owned(),
            source,
            destination,
            _entry: entry,
            identity,
            original: original.to_owned(),
            current: current.to_owned(),
            stage,
            stage_file,
        })
    }

    pub(super) fn commit(self, state: &mut Store, id: u64, change: &Change) -> io::Result<()> {
        if !self.parents_current() || self.stage_at_parent()? != self.stage.identity {
            return Err(changed());
        }
        let ordinal = state.next_ordinal(id)?;
        state.intent(&Pending::Forward {
            id,
            ordinal,
            change: change.clone(),
            stage: Some(self.stage.clone()),
        })?;
        // Persist the stage directory before moving any entry into it.
        if let Err(error) = self.destination.file.sync_all() {
            return Err(protect(state, error));
        }
        #[cfg(test)]
        super::rename_boundary::before_rename(&self.original, &self.stage.payload());
        if let Err(error) = move_at(
            &self.source.file,
            self.original.file_name().unwrap(),
            self.payload_parent(),
            &self.payload_name,
        ) {
            // A failed rename does not authorize deleting anything that might
            // have appeared in the stage. Only an empty stage can be retired.
            if self.payload_absent() {
                state.clear_intent()?;
                return Err(error);
            }
            return Err(protect(state, error));
        }
        // Persist every successful claim, including an unexpected replacement,
        // before attempting a return which newer work might block.
        if let Err(error) = self
            .stage_file
            .sync_all()
            .and_then(|()| self.source.file.sync_all())
            .and_then(|()| self.destination.file.sync_all())
        {
            return Err(protect(state, error));
        }
        if at(self.payload_parent(), &self.payload_name)? != self.identity
            || !self.parents_current()
        {
            return self.refuse(state, changed());
        }
        // Only the selected object is allowed to proceed to the requested name.
        #[cfg(test)]
        super::rename_boundary::before_rename(&self.stage.payload(), &self.current);
        if let Err(error) = move_at(
            self.payload_parent(),
            &self.payload_name,
            &self.destination.file,
            self.current.file_name().unwrap(),
        ) {
            return self.refuse(state, error);
        }
        if at(&self.destination.file, self.current.file_name().unwrap())? != self.identity {
            return Err(protect(state, changed()));
        }
        if !self.parents_current() {
            // Return through the held directories even if either path moved.
            if let Err(error) = move_at(
                &self.destination.file,
                self.current.file_name().unwrap(),
                self.payload_parent(),
                &self.payload_name,
            ) {
                return Err(protect(state, error));
            }
            return self.refuse(state, changed());
        }
        if let Err(error) = self
            .source
            .file
            .sync_all()
            .and_then(|()| self.stage_file.sync_all())
            .and_then(|()| self.destination.file.sync_all())
        {
            return Err(protect(state, error));
        }
        // Only verified, synchronized publication returns to the established
        // Forward/Undo format. Before this checkpoint recovery must review the
        // claim; it must never delete an unverified staged entry.
        state.intent(&Pending::Forward {
            id,
            ordinal,
            change: change.clone(),
            stage: None,
        })?;
        state.append(id, ordinal, change)?;
        self.clean_empty()
    }

    fn refuse(&self, state: &mut Store, error: io::Error) -> io::Result<()> {
        // Pin the *actual* claimed object while returning it. It may be the
        // unexpected replacement, rather than the selected source.
        let claimed = fs::File::from(rustix::fs::openat(
            self.payload_parent(),
            &self.payload_name,
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let identity = Identity::from_metadata(&claimed.metadata()?);
        #[cfg(test)]
        super::rename_boundary::before_rename(&self.stage.payload(), &self.original);
        if let Err(rollback) = move_at(
            self.payload_parent(),
            &self.payload_name,
            &self.source.file,
            self.original.file_name().unwrap(),
        ) {
            return Err(protect(
                state,
                io::Error::new(
                    error.kind(),
                    format!("{error}; the claimed entry could not be returned: {rollback}"),
                ),
            ));
        }
        if at(&self.source.file, self.original.file_name().unwrap())? != identity {
            return Err(protect(state, changed()));
        }
        self.source.file.sync_all()?;
        self.stage_file.sync_all()?;
        self.destination.file.sync_all()?;
        state.clear_intent()?;
        Err(error)
    }

    fn parents_current(&self) -> bool {
        self.source.current() && self.destination.current()
    }

    fn payload_absent(&self) -> bool {
        at(self.payload_parent(), &self.payload_name)
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    }

    fn payload_parent(&self) -> &fs::File {
        if self.stage.sibling_payload {
            &self.destination.file
        } else {
            &self.stage_file
        }
    }

    fn stage_at_parent(&self) -> io::Result<Identity> {
        at(&self.destination.file, &self.stage_name)
    }

    fn clean_empty(&self) -> io::Result<()> {
        if !self.payload_absent() {
            return Err(interrupted());
        }
        if self.stage_at_parent().ok() == Some(self.stage.identity) {
            rustix::fs::unlinkat(&self.destination.file, &self.stage_name, AtFlags::REMOVEDIR)?;
            self.destination.file.sync_all()?;
        }
        Ok(())
    }
}

impl Drop for Prepared {
    fn drop(&mut self) {
        // rmdir cannot remove data. Incomplete claimed entries remain protected
        // by the journal, including when their parent pathname has moved.
        let _empty_only = self.clean_empty();
    }
}

fn at(parent: &fs::File, name: &OsStr) -> io::Result<Identity> {
    let stat = rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
    Ok(Identity {
        device: stat.st_dev,
        inode: stat.st_ino,
    })
}

fn move_at(
    source: &fs::File,
    name: &OsStr,
    destination: &fs::File,
    target: &OsStr,
) -> io::Result<()> {
    rustix::fs::renameat_with(source, name, destination, target, RenameFlags::NOREPLACE)
        .map_err(Into::into)
}

fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The selected item or its folder changed. Rename was stopped.",
    )
}

pub(super) fn interrupted() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "Rename stopped before its result could be verified. The affected files are preserved for Recovery review.",
    )
}

fn protect(state: &mut Store, error: io::Error) -> io::Error {
    let message = format!("{error}; {}", interrupted());
    state.recovery_result(&Err(io::Error::new(error.kind(), message.clone())));
    io::Error::new(error.kind(), message)
}
