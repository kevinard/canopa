//! Whole-entry Moves retain their selection and both parents across review.
//! A durable phase distinguishes unverified claims, publication and return.
use super::*;
use original::Original;
use rustix::fs::{Mode, OFlags, RenameFlags, open, openat};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Originals {
    entry: Original,
    source_parent: Original,
    destination_parent: Original,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Facts {
    version: fingerprint::Version,
    directory: bool,
    source_parent: Identity,
    destination_parent: Identity,
    originals: Option<Originals>,
}
impl Facts {
    pub(super) fn matches_identity(&self, identity: Identity) -> bool {
        // Live descriptors can complete a Move without restart generation facts.
        // Its exact receipt restores metadata only; recovery mutation still
        // requires the original entry and both parent generations below.
        self.version.identity == identity
    }
    pub(super) fn saved_origins(&self) -> Option<(Original, Original, Original)> {
        let origins = self.originals.as_ref()?;
        self.directory.then_some((
            origins.source_parent,
            origins.entry,
            origins.destination_parent,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Phase {
    Ready,
    Publishing,
    Returning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Claim {
    phase: Phase,
    facts: Facts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    returning: Option<Original>,
}

struct Parent {
    file: fs::File,
    path: PathBuf,
    identity: Identity,
    original: Option<Original>,
}
impl Parent {
    fn open(path: &Path) -> io::Result<Self> {
        let file = fs::File::from(open(
            path,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        Ok(Self {
            identity: Identity::from_metadata(&file.metadata()?),
            original: Original::capture(&file).ok(),
            file,
            path: path.to_owned(),
        })
    }
    fn require_current(&self) -> io::Result<()> {
        let current = fs::File::from(open(
            &self.path,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        if Identity::from_metadata(&current.metadata()?) != self.identity {
            return Err(changed());
        }
        if let Some(original) = &self.original {
            original.require(&current)?;
        }
        Ok(())
    }
    fn sync(&self) -> io::Result<()> {
        sync_parent_file(&self.file)
    }
}

/// Capture before presenting a conflict. Changing a name may change only the
/// destination leaf; it cannot replace these original opened parent folders.
pub(super) struct Selection {
    source: Parent,
    destination: Parent,
    entry: fs::File,
    original: PathBuf,
    facts: Facts,
    directory: bool,
}
impl Selection {
    pub(super) fn open(source: &Path, destination: &Path) -> io::Result<Self> {
        let source_parent = source
            .parent()
            .ok_or_else(|| invalid("A Move needs a named source entry."))?;
        let source_name = source
            .file_name()
            .ok_or_else(|| invalid("A Move needs a source name."))?;
        let destination_parent = destination
            .parent()
            .filter(|_| destination.file_name().is_some())
            .ok_or_else(|| invalid("A Move needs a named destination entry."))?;
        let source_parent = Parent::open(source_parent)?;
        let target = Parent::open(destination_parent)?;
        let entry = named(&source_parent.file, source_name)?;
        let metadata = entry.metadata()?;
        let originals = source_parent.original.zip(target.original).and_then(
            |(source_parent, destination_parent)| {
                Original::capture(&entry).ok().map(|entry| Originals {
                    entry,
                    source_parent,
                    destination_parent,
                })
            },
        );
        let result = Self {
            facts: Facts {
                version: fingerprint::Version::from_metadata(&metadata),
                directory: metadata.is_dir(),
                source_parent: source_parent.identity,
                destination_parent: target.identity,
                originals,
            },
            source: source_parent,
            destination: target,
            entry,
            original: source.to_owned(),
            directory: metadata.is_dir(),
        };
        result.require_current()?;
        Ok(result)
    }
    pub(super) fn saved_movement(
        &self,
        destination: &Path,
        record: &recovery::Record,
        access: StageParentAccess,
        context: Option<saved_stage::ContextMovement>,
        counted: bool,
    ) -> io::Result<saved_stage::Movement> {
        self.require_current()?;
        if !self.directory || destination.parent() != Some(self.destination.path.as_path()) {
            return Err(invalid(
                "Saved data needs its selected container and destination.",
            ));
        }
        saved_stage::Movement::capture_owned(saved_stage::Capture {
            source: &self.original,
            destination,
            record,
            parent: &self.source.file,
            directory: &self.entry,
            target_parent: &self.destination.file,
            access,
            context,
            counted,
        })
    }
    pub(super) fn copy_saved(
        &self,
        cancellation: &Cancellation,
        register: impl FnMut(&staging::Registration<'_>) -> io::Result<()>,
        progress: impl FnMut(u64),
        mut checkpoint: impl FnMut() -> io::Result<()>,
    ) -> io::Result<prepared_copy::PreparedCopy> {
        self.require_current()?;
        if !self.directory {
            return Err(invalid("Saved data needs its selected container."));
        }
        let source =
            fingerprint::source::Opened::open_in(&self.original.join("previous"), &self.entry)?;
        prepared_copy::prepare_pinned(
            source,
            &self.destination.path,
            &self.destination.file,
            cancellation,
            register,
            progress,
            || {
                checkpoint()?;
                self.require_current()
            },
        )
    }
    pub(super) fn same_filesystem(&self) -> io::Result<bool> {
        Ok(self.entry.metadata()?.dev() == self.destination.file.metadata()?.dev())
    }
    pub(super) fn require_current(&self) -> io::Result<()> {
        self.source.require_current()?;
        self.destination.require_current()?;
        if !self.facts.version.matches_opened(&self.entry, false)?
            || !self.facts.version.matches_opened(
                &named(&self.source.file, self.original.file_name().unwrap())?,
                false,
            )?
        {
            return Err(changed());
        }
        Ok(())
    }
    pub(super) fn prepare(self, current: &Path, access: Option<Identity>) -> io::Result<Prepared> {
        if current.parent() != Some(self.destination.path.as_path())
            || current.file_name().is_none()
        {
            return Err(invalid("A Move must use its selected destination folder."));
        }
        self.require_current()?;
        self.source.sync()?;
        let (opened, mut stage, temporary) =
            staging::Opened::prepare_in(&self.destination.path, &self.destination.file)?;
        let _retained = temporary.keep();
        stage.record_parent_access(access);
        stage.payload_identity = Some(self.facts.version.identity);
        stage.sibling_payload = self.directory;
        stage.ownership = stage
            .ownership
            .take()
            .map(|proof| Box::new((*proof).with_payload(&self.entry)));
        stage.entry_move = Some(Box::new(Claim {
            phase: Phase::Ready,
            facts: self.facts.clone(),
            returning: None,
        }));
        let change = Change::Rename {
            move_facts: Some(Box::new(self.facts.clone())),
            original: self.original.clone(),
            current: current.to_owned(),
            identity: self.facts.version.identity,
            retained: None,
            reviewed: None,
            saved: None,
        };
        Ok(Prepared {
            backup: None,
            selection: self,
            opened,
            stage,
            change,
            journal_owned: false,
        })
    }
}

pub(super) struct Prepared {
    backup: Option<replacement::Reviewed>,
    selection: Selection,
    opened: staging::Opened,
    stage: Stage,
    change: Change,
    journal_owned: bool,
}
impl Prepared {
    pub(super) fn saved(
        mut self,
        record: &recovery::Record,
        context: Option<saved_stage::ContextMovement>,
        counted: bool,
    ) -> io::Result<Self> {
        if !self.selection.directory || self.backup.is_some() {
            return Err(invalid("Saved data moves as its original whole container."));
        }
        let movement = saved_stage::Movement::capture_owned(saved_stage::Capture {
            source: &self.selection.original,
            destination: self.change.forward_path(),
            record,
            parent: &self.selection.source.file,
            directory: &self.selection.entry,
            target_parent: &self.selection.destination.file,
            access: self
                .stage
                .parent_access
                .unwrap_or(StageParentAccess::Existing),
            context,
            counted,
        })?;
        let Change::Rename { saved, .. } = &mut self.change else {
            unreachable!()
        };
        *saved = Some(movement);
        Ok(self)
    }
    pub(super) fn replacing(mut self, reviewed: replacement::Reviewed) -> io::Result<Self> {
        if self.selection.directory
            || (self.selection.source.identity == self.selection.destination.identity
                && self.selection.original.file_name() == self.change.forward_path().file_name())
        {
            return Err(invalid(
                "A replacement Move needs two distinct regular entries.",
            ));
        }
        let retained = reviewed.attach(&self.selection.destination.file, &mut self.stage)?;
        let Change::Rename { retained: slot, .. } = &mut self.change else {
            unreachable!()
        };
        *slot = Some(retained);
        self.backup = Some(reviewed);
        Ok(self)
    }
    pub(super) fn commit(
        mut self,
        state: &mut Store,
        id: u64,
        cancellation: &Cancellation,
        mut checkpoint: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        let ordinal = state.next_ordinal(id)?;
        self.save(state, id, ordinal)?;
        self.journal_owned = true;
        #[cfg(test)]
        super::entry_move_boundary::at_phase(
            "intent",
            &self.selection.original,
            &self.stage.payload(),
        );
        if let Err(error) = (|| {
            checkpoint()?;
            self.selection.require_current()?;
            self.opened.require_current()?;
            self.opened.sync()?;
            check_cancel(cancellation)
        })() {
            if transfer::placement_failure(&error).is_some() {
                return Err(protect(error));
            }
            return self.abort_empty(state, id, error);
        }
        if let Some(backup) = &self.backup {
            let change = self.change.clone();
            if let Err(error) = backup.claim(
                self.opened.directory(),
                &mut self.stage,
                |stage| {
                    state.intent(&Pending::Forward {
                        id,
                        ordinal,
                        change: change.clone(),
                        stage: Some(stage.clone()),
                    })
                },
                cancellation,
            ) {
                if transfer::placement_failure(&error).is_some() {
                    return Err(protect(error));
                }
                return self.abort_empty(state, id, error);
            }
            if let Err(error) = checkpoint() {
                if transfer::placement_failure(&error).is_some() {
                    return Err(protect(error));
                }
                return self.abort_empty(state, id, error);
            }
        }
        #[cfg(test)]
        super::rename_boundary::before_rename(&self.selection.original, &self.stage.payload());
        if let Err(error) = move_at(
            &self.selection.source.file,
            self.selection.original.file_name().unwrap(),
            self.payload_parent(),
            self.stage.payload().file_name().unwrap(),
        ) {
            if self.payload()?.is_none() && self.selection.require_current().is_ok() {
                return self.abort_empty(state, id, error);
            }
            return Err(protect(error));
        }
        self.sync().map_err(protect)?;
        #[cfg(test)]
        if self.backup.is_some() {
            super::replacement_boundary::at_phase(
                "source",
                self.change.forward_path(),
                &self.stage.payload(),
            );
        }
        #[cfg(test)]
        {
            super::transfer::move_boundary::cancel_return(&self.selection.original, cancellation);
            super::transfer::saved_boundary::cancel_return(&self.selection.original, cancellation);
        }
        #[cfg(test)]
        super::entry_move_boundary::at_phase(
            "ready",
            &self.selection.original,
            &self.stage.payload(),
        );
        let actual = self.payload()?.ok_or_else(interrupted)?;
        if let Err(error) = (|| {
            self.require_claimed(&actual)?;
            checkpoint()?;
            check_cancel(cancellation)
        })() {
            if transfer::placement_failure(&error).is_some() {
                return Err(protect(error));
            }
            return self.return_claim(state, id, ordinal, &actual, error);
        }
        self.stage.entry_move.as_mut().unwrap().phase = Phase::Publishing;
        if let Err(error) = self.save(state, id, ordinal) {
            return self.return_claim(state, id, ordinal, &actual, error);
        }
        #[cfg(test)]
        if self.backup.is_some() {
            super::replacement_boundary::at_phase(
                "publishing",
                self.change.forward_path(),
                &self.stage.payload(),
            );
        }
        #[cfg(test)]
        super::entry_move_boundary::at_phase(
            "publishing",
            &self.selection.original,
            &self.stage.payload(),
        );
        #[cfg(test)]
        super::rename_boundary::before_rename(&self.stage.payload(), self.change.forward_path());
        if let Err(error) = self
            .require_claimed(&actual)
            .and_then(|()| check_cancel(cancellation))
        {
            return self.return_claim(state, id, ordinal, &actual, error);
        }
        if let Err(error) = move_at(
            self.payload_parent(),
            self.stage.payload().file_name().unwrap(),
            &self.selection.destination.file,
            self.change.forward_path().file_name().unwrap(),
        ) {
            return self.return_claim(state, id, ordinal, &actual, error);
        }
        self.sync().map_err(protect)?;
        #[cfg(test)]
        super::entry_move_boundary::at_phase(
            "published",
            &self.selection.original,
            self.change.forward_path(),
        );
        let published = named(
            &self.selection.destination.file,
            self.change.forward_path().file_name().unwrap(),
        )
        .map_err(protect)?;
        #[cfg(test)]
        if self.backup.is_some() {
            super::replacement_boundary::at_phase(
                "published",
                self.change.forward_path(),
                &self.stage.payload(),
            );
        }
        if Identity::from_metadata(&published.metadata()?) != self.selection.facts.version.identity
        {
            return Err(protect(changed()));
        }
        if let Err(error) = self.parents_current() {
            move_at(
                &self.selection.destination.file,
                self.change.forward_path().file_name().unwrap(),
                self.payload_parent(),
                self.stage.payload().file_name().unwrap(),
            )
            .map_err(protect)?;
            self.sync().map_err(protect)?;
            return self.return_claim(state, id, ordinal, &published, error);
        }
        if self.payload()?.is_some() {
            return Err(protect(interrupted()));
        }
        // Keep even unsupported empty cleanup journalled until the original
        // process has removed it through its held descriptor.
        let normal = if let Some(backup) = &self.backup {
            let refreshed = backup
                .refreshed(self.opened.directory(), &self.change)
                .map_err(protect)?;
            state.append_replacement_with_cleanup(id, ordinal, &refreshed)?
        } else {
            state.append_with_cleanup(id, ordinal, &self.change, &empty_stage(&self.stage))?
        };
        if self.backup.is_some() {
            if normal.ownership.is_some() {
                clean_stage(&normal).map_err(protect)?;
            }
        } else {
            self.opened.clean_empty().map_err(protect)?;
        }
        state.clear_intent()?;
        #[cfg(test)]
        if self.backup.is_some() {
            super::replacement_boundary::at_phase(
                "finished",
                self.change.forward_path(),
                &self.stage.payload(),
            );
        }
        #[cfg(test)]
        super::entry_move_boundary::at_phase(
            "finished",
            &self.selection.original,
            self.change.forward_path(),
        );
        Ok(())
    }
    fn payload_parent(&self) -> &fs::File {
        if self.stage.sibling_payload {
            &self.selection.destination.file
        } else {
            self.opened.directory()
        }
    }
    fn payload(&self) -> io::Result<Option<fs::File>> {
        optional(
            self.payload_parent(),
            self.stage.payload().file_name().unwrap(),
        )
    }
    fn parents_current(&self) -> io::Result<()> {
        self.selection.source.require_current()?;
        self.selection.destination.require_current()?;
        self.opened.require_current()
    }
    fn require_claimed(&self, actual: &fs::File) -> io::Result<()> {
        self.parents_current()?;
        if let Some(backup) = &self.backup {
            backup.require_saved(self.opened.directory())?;
        }
        let named = self.payload()?.ok_or_else(interrupted)?;
        let version = &self.selection.facts.version;
        if Identity::from_metadata(&named.metadata()?) != version.identity
            || !version.matches_opened(actual, true)?
            || !version.matches_opened(&self.selection.entry, true)?
        {
            return Err(changed());
        }
        if let Change::Rename {
            saved: Some(saved), ..
        } = &self.change
        {
            saved.require_previous(actual)?;
        }
        Ok(())
    }
    fn sync(&self) -> io::Result<()> {
        self.selection.source.sync()?;
        self.opened.sync()
    }
    fn save(&self, state: &mut Store, id: u64, ordinal: u64) -> io::Result<()> {
        state.intent(&Pending::Forward {
            id,
            ordinal,
            change: self.change.clone(),
            stage: Some(self.stage.clone()),
        })
    }
    fn abort_empty(&mut self, state: &mut Store, id: u64, error: io::Error) -> io::Result<()> {
        if self.payload()?.is_some() {
            return Err(protect(error));
        }
        if let Some(backup) = &self.backup {
            let ordinal = state.next_ordinal(id)?;
            let change = self.change.clone();
            backup
                .restore(self.opened.directory(), &mut self.stage, |stage| {
                    state.intent(&Pending::Forward {
                        id,
                        ordinal,
                        change: change.clone(),
                        stage: Some(stage.clone()),
                    })
                })
                .map_err(protect)?;
        }
        let pending = Pending::Preparing {
            id,
            destination: self.change.forward_path().to_owned(),
            stage: empty_stage(&self.stage),
        };
        state.returned_source_intent(&pending)?;
        #[cfg(test)]
        if self.backup.is_some() {
            super::replacement_boundary::at_phase(
                "return-preparing",
                self.change.forward_path(),
                &self.stage.payload(),
            );
        }
        #[cfg(test)]
        {
            super::transfer::move_boundary::boundary("return-preparing", &self.selection.original);
            super::transfer::saved_boundary::boundary(
                "return-preparing",
                &self.selection.original,
                &self.stage.payload(),
            );
        }
        self.opened.clean_empty().map_err(protect)?;
        state.clear_intent()?;
        Err(error)
    }
    fn return_claim(
        &mut self,
        state: &mut Store,
        id: u64,
        ordinal: u64,
        actual: &fs::File,
        error: io::Error,
    ) -> io::Result<()> {
        let identity = Identity::from_metadata(&actual.metadata()?);
        let payload = self.payload()?.ok_or_else(interrupted)?;
        if Identity::from_metadata(&payload.metadata()?) != identity {
            return Err(protect(changed()));
        }
        let claim = self.stage.entry_move.as_mut().unwrap();
        claim.phase = Phase::Returning;
        claim.returning = Original::capture(actual).ok();
        self.save(state, id, ordinal).map_err(protect)?;
        #[cfg(test)]
        super::entry_move_boundary::at_phase(
            "returning",
            &self.selection.original,
            &self.stage.payload(),
        );
        move_at(
            self.payload_parent(),
            self.stage.payload().file_name().unwrap(),
            &self.selection.source.file,
            self.selection.original.file_name().unwrap(),
        )
        .map_err(protect)?;
        if Identity::from_metadata(
            &named(
                &self.selection.source.file,
                self.selection.original.file_name().unwrap(),
            )?
            .metadata()?,
        ) != identity
        {
            return Err(protect(changed()));
        }
        self.sync().map_err(protect)?;
        #[cfg(test)]
        super::entry_move_boundary::at_phase(
            "returned",
            &self.selection.original,
            &self.stage.payload(),
        );
        self.abort_empty(state, id, error)
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        if !self.journal_owned && self.payload().is_ok_and(|payload| payload.is_none()) {
            let _empty_only = self.opened.clean_empty();
        }
    }
}

fn empty_stage(stage: &Stage) -> Stage {
    replacement::normalized(stage)
}
fn named(parent: &fs::File, name: impl rustix::path::Arg) -> io::Result<fs::File> {
    Ok(fs::File::from(openat(
        parent,
        name,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?))
}
fn optional(parent: &fs::File, name: impl rustix::path::Arg) -> io::Result<Option<fs::File>> {
    match named(parent, name) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
fn move_at(
    source: &fs::File,
    name: impl rustix::path::Arg,
    destination: &fs::File,
    target: impl rustix::path::Arg,
) -> io::Result<()> {
    rustix::fs::renameat_with(source, name, destination, target, RenameFlags::NOREPLACE)
        .map_err(Into::into)
}
fn sync_parent_file(parent: &fs::File) -> io::Result<()> {
    fs::File::from(openat(
        parent,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
    .sync_all()
}
fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The selected Move entry or folder changed; its current contents are preserved.",
    )
}
pub(super) fn interrupted() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "Move stopped before its claimed entry could be verified. Its files are preserved for Recovery.",
    )
}
fn protect(error: impl Into<io::Error>) -> io::Error {
    transfer::destination_failure(error.into())
}

pub(super) fn recover(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    stage: &Stage,
) -> io::Result<()> {
    validate_claim_stage(Some(stage), change.forward_path())?;
    let Change::Rename {
        original,
        current,
        identity,
        move_facts: Some(facts),
        retained,
        reviewed: None,
        saved,
    } = change
    else {
        return Err(invalid(
            "A whole-entry Move claim needs its exact Move step.",
        ));
    };
    let claim = stage.entry_move.as_ref().ok_or_else(interrupted)?;
    if retained.is_some() != stage.replacement.is_some() {
        return Err(invalid("The Move replacement role differs from its step."));
    }
    let backup = if stage.replacement.is_some() {
        Some(replacement::Reopened::open(stage, change)?)
    } else {
        None
    };
    if **facts != claim.facts
        || *identity != facts.version.identity
        || stage.payload_identity != Some(*identity)
        || stage.parent_identity != Some(facts.destination_parent)
        || stage.sibling_payload != facts.directory
    {
        return Err(invalid("The Move claim differs from its recorded step."));
    }
    if saved
        .as_ref()
        .is_some_and(|saved| !saved.matches_move(facts))
    {
        return Err(invalid(
            "The saved container differs from its actual Move claim.",
        ));
    }
    let originals = facts.originals.as_ref().ok_or_else(interrupted)?;
    let ownership = stage.ownership.as_ref().ok_or_else(interrupted)?;
    if ownership.payload_origins() != Some((originals.destination_parent, originals.entry)) {
        return Err(invalid("The Move claim has inconsistent original facts."));
    }
    let source_parent = fs::File::from(open(
        original.parent().unwrap(),
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    let destination_parent = fs::File::from(open(
        current.parent().unwrap(),
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    originals.source_parent.require(&source_parent)?;
    originals.destination_parent.require(&destination_parent)?;
    if Identity::from_metadata(&source_parent.metadata()?) != facts.source_parent
        || Identity::from_metadata(&destination_parent.metadata()?) != facts.destination_parent
    {
        return Err(changed());
    }
    let directory = match openat(
        &destination_parent,
        stage.directory.file_name().unwrap(),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(file) => {
            let file = fs::File::from(file);
            ownership.require_directory(&file)?;
            if Identity::from_metadata(&file.metadata()?) != stage.identity {
                return Err(changed());
            }
            Some(file)
        }
        Err(rustix::io::Errno::NOENT) => None,
        Err(error) => return Err(error.into()),
    };
    let payload_parent = if stage.sibling_payload {
        Some(&destination_parent)
    } else {
        directory.as_ref()
    };
    let payload = payload_parent
        .map(|parent| optional(parent, stage.payload().file_name().unwrap()))
        .transpose()?
        .flatten();
    if payload.is_some() && directory.is_none() {
        return Err(interrupted());
    }
    if claim.phase == Phase::Returning {
        let subject = claim.returning.as_ref().ok_or_else(interrupted)?;
        if let Some(payload) = payload {
            subject.require(&payload)?;
            move_at(
                payload_parent.unwrap(),
                stage.payload().file_name().unwrap(),
                &source_parent,
                original.file_name().unwrap(),
            )?;
            sync_parent_file(payload_parent.unwrap())?;
            sync_parent_file(&source_parent)?;
        } else {
            subject.require(&named(&source_parent, original.file_name().unwrap())?)?;
        }
        return clear_returned(state, id, ordinal, change, stage);
    }
    if let Some(payload) = payload {
        require_selected(facts, &originals.entry, &payload)?;
        if let Some(saved) = saved {
            saved.require_previous(&payload)?;
        }
        if claim.phase == Phase::Ready || !facts.version.matches_opened(&payload, true)? {
            let mut returning = stage.clone();
            let claim = returning.entry_move.as_mut().unwrap();
            claim.phase = Phase::Returning;
            claim.returning = Some(originals.entry);
            state.intent(&Pending::Forward {
                id,
                ordinal,
                change: change.clone(),
                stage: Some(returning.clone()),
            })?;
            move_at(
                payload_parent.unwrap(),
                stage.payload().file_name().unwrap(),
                &source_parent,
                original.file_name().unwrap(),
            )?;
            sync_parent_file(payload_parent.unwrap())?;
            sync_parent_file(&source_parent)?;
            return clear_returned(state, id, ordinal, change, &returning);
        }
        if let Some(backup) = &backup {
            backup.require_saved(stage)?;
        }
        move_at(
            payload_parent.unwrap(),
            stage.payload().file_name().unwrap(),
            &destination_parent,
            current.file_name().unwrap(),
        )?;
        sync_parent_file(payload_parent.unwrap())?;
        sync_parent_file(&destination_parent)?;
    } else {
        let published = optional(&destination_parent, current.file_name().unwrap())?;
        let is_published = match &published {
            Some(file) => is_original(&originals.entry, file)?,
            None => false,
        };
        if claim.phase != Phase::Publishing || !is_published {
            if claim.phase == Phase::Publishing {
                // An absent Publishing payload is not an authorized return.
                // Never inspect a newer source name to infer that disposition.
                return Err(interrupted());
            }
            require_selected(
                facts,
                &originals.entry,
                &named(&source_parent, original.file_name().unwrap())?,
            )?;
            return clear_returned(state, id, ordinal, change, stage);
        }
    }
    require_selected(
        facts,
        &originals.entry,
        &named(&destination_parent, current.file_name().unwrap())?,
    )?;
    if let Some(saved) = saved {
        saved.require_previous(&named(&destination_parent, current.file_name().unwrap())?)?;
    }
    originals.source_parent.require(&fs::File::from(open(
        original.parent().unwrap(),
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?))?;
    originals.destination_parent.require(&fs::File::from(open(
        current.parent().unwrap(),
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?))?;
    // A completed publication is metadata only. Preserve later edits at its
    // destination; never move the name again while restoring this history row.
    if let Some(backup) = &backup {
        let refreshed = backup.refreshed(stage, change)?;
        let cleanup = state.append_replacement_with_cleanup(id, ordinal, &refreshed)?;
        clean_stage(&cleanup)?;
        return state.clear_intent();
    }
    append_forward(state, id, ordinal, change, Some(&empty_stage(stage)))
}
fn require_selected(facts: &Facts, original: &Original, file: &fs::File) -> io::Result<()> {
    original.require(file)?;
    let metadata = file.metadata()?;
    if Identity::from_metadata(&metadata) != facts.version.identity
        || metadata.is_dir() != facts.directory
    {
        return Err(changed());
    }
    Ok(())
}
fn is_original(original: &Original, file: &fs::File) -> io::Result<bool> {
    match original.require(file) {
        Ok(()) => Ok(true),
        Err(error)
            if error.kind() == io::ErrorKind::WouldBlock && error.raw_os_error().is_none() =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}
fn clear_returned(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    stage: &Stage,
) -> io::Result<()> {
    let mut returning = stage.clone();
    if returning.replacement.is_some() {
        let claim = returning.entry_move.as_mut().unwrap();
        claim.phase = Phase::Returning;
        if claim.returning.is_none() {
            claim.returning = Some(
                claim
                    .facts
                    .originals
                    .as_ref()
                    .ok_or_else(interrupted)?
                    .entry,
            );
        }
        state.intent(&Pending::Forward {
            id,
            ordinal,
            change: change.clone(),
            stage: Some(returning.clone()),
        })?;
        replacement::Reopened::open(&returning, change)?.restore(
            state,
            id,
            ordinal,
            change,
            &mut returning,
        )?;
    }
    let normal = empty_stage(&returning);
    let pending = Pending::Preparing {
        id,
        destination: change.forward_path().to_owned(),
        stage: normal.clone(),
    };
    state.returned_source_intent(&pending)?;
    #[cfg(test)]
    if change.retained().is_some() {
        super::replacement_boundary::at_phase(
            "return-preparing",
            change.forward_path(),
            &normal.payload(),
        );
    }
    #[cfg(test)]
    {
        super::transfer::move_boundary::boundary(
            "return-preparing",
            change.reverse_path().unwrap(),
        );
        super::transfer::saved_boundary::boundary(
            "return-preparing",
            change.reverse_path().unwrap(),
            &normal.payload(),
        );
    }
    clean_stage(&normal)?;
    state.clear_intent()
}

pub(super) fn returned_transition(before: &Pending, after: &Pending) -> bool {
    let Pending::Forward {
        id,
        change:
            Change::Rename {
                move_facts: Some(facts),
                current,
                retained: None,
                reviewed: None,
                saved,
                ..
            },
        stage: Some(stage),
        ..
    } = before
    else {
        return false;
    };
    let Pending::Preparing {
        id: next,
        destination,
        stage: empty,
    } = after
    else {
        return false;
    };
    id == next
        && current == destination
        && saved.as_ref().is_none_or(|saved| saved.matches_move(facts))
        && stage.entry_move.as_ref().is_some_and(|claim| {
            claim.facts == **facts && matches!(claim.phase, Phase::Ready | Phase::Returning)
        })
        && empty_stage(stage) == *empty
}

pub(super) fn returned_claim(stage: &Stage, facts: &Facts) -> bool {
    stage.entry_move.as_ref().is_some_and(|claim| {
        claim.facts == *facts && matches!(claim.phase, Phase::Ready | Phase::Returning)
    })
}
