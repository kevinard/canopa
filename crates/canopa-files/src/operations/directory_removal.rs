//! Claim an empty source directory before removing it. Ready is never deletion authority.
use super::*;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags, open, openat};
use transfer::directory::{DirectoryOriginal, Opened as Folder};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Facts {
    original: DirectoryOriginal,
    source: Identity,
    parent: Identity,
    destination: Identity,
}
impl Facts {
    pub(super) fn matches(
        &self,
        original: Option<DirectoryOriginal>,
        source: Identity,
        parent: Identity,
        destination: Identity,
    ) -> bool {
        original == Some(self.original)
            && self.source == source
            && self.parent == parent
            && self.destination == destination
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Phase {
    Ready,
    Claimed,
    Returning,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Subject {
    identity: Identity,
    original: Option<original::Original>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Claim {
    facts: Facts,
    phase: Phase,
    returning: Option<Subject>,
}
#[derive(Clone, Copy)]
struct Step<'a> {
    id: u64,
    ordinal: u64,
    change: &'a Change,
}

pub(super) struct Prepared {
    source: Folder,
    source_path: PathBuf,
    destination: PathBuf,
    opened: staging::Opened,
    stage: Stage,
    facts: Facts,
    journal_owned: bool,
}
impl Prepared {
    pub(super) fn open(
        source_path: &Path,
        destination: &Path,
        original: Option<DirectoryOriginal>,
        source_id: Identity,
        parent_id: Identity,
        destination_id: Identity,
    ) -> io::Result<Self> {
        let original = original.ok_or_else(interrupted)?;
        let source = Folder::open(source_path)?;
        let target = Folder::open(destination)?;
        original.require_opened(&source, &target)?;
        if Identity::from_metadata(&source.metadata()?) != source_id
            || Identity::from_metadata(&source.parent().metadata()?) != parent_id
            || Identity::from_metadata(&target.metadata()?) != destination_id
        {
            return Err(interrupted());
        }
        require_empty(source.entry())?;
        let (opened, mut stage, temporary) =
            staging::Opened::prepare_in(source_path.parent().unwrap(), source.parent())?;
        let _retained = temporary.keep();
        stage.payload_identity = Some(source_id);
        stage.sibling_payload = true;
        stage.ownership = stage
            .ownership
            .take()
            .map(|proof| Box::new((*proof).with_payload(source.entry())));
        let facts = Facts {
            original,
            source: source_id,
            parent: parent_id,
            destination: destination_id,
        };
        stage.directory_claim = Some(Box::new(Claim {
            facts: facts.clone(),
            phase: Phase::Ready,
            returning: None,
        }));
        Ok(Self {
            source,
            source_path: source_path.to_owned(),
            destination: destination.to_owned(),
            opened,
            stage,
            facts,
            journal_owned: false,
        })
    }
    pub(super) fn facts(&self) -> &Facts {
        &self.facts
    }
    pub(super) fn commit(
        mut self,
        state: &mut Store,
        id: u64,
        change: &Change,
        cancellation: &Cancellation,
        mut checkpoint: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        let Change::RemovedDirectory {
            path,
            identity,
            parent_identity,
            moved: Some(moved),
            removal: Some(facts),
            ..
        } = change
        else {
            return Err(interrupted());
        };
        if path != &self.source_path
            || moved.current != self.destination
            || **facts != self.facts
            || *identity != self.facts.source
            || *parent_identity != self.facts.parent
            || moved.identity != self.facts.destination
        {
            return Err(interrupted());
        }
        let step = Step {
            id,
            ordinal: state.next_ordinal(id)?,
            change,
        };
        save(state, step, &self.stage)?;
        self.journal_owned = true;
        if let Err(error) = (|| -> io::Result<()> {
            checkpoint()?;
            self.facts
                .original
                .require(&self.source_path, &self.destination, false)?;
            self.source.require_current()?;
            self.opened.require_current()?;
            require_empty(self.source.entry())?;
            check_cancel(cancellation)
        })() {
            return self.abort_empty(state, step, error);
        }
        #[cfg(test)]
        super::directory_removal_boundary::before_remove(&self.source_path, &self.destination);
        if let Err(error) = move_at(
            self.source.parent(),
            self.source_path.file_name().unwrap(),
            self.stage.payload().file_name().unwrap(),
        ) {
            return self.abort_empty(state, step, error);
        }
        self.opened.sync().map_err(protect)?;
        #[cfg(test)]
        super::transfer::directory_boundary::cancel_cleanup_return(&self.source_path, cancellation);
        #[cfg(test)]
        super::directory_removal_boundary::at_phase(
            "claimed",
            &self.source_path,
            &self.stage.payload(),
        );
        let actual = named(
            self.source.parent(),
            self.stage.payload().file_name().unwrap(),
        )
        .map_err(protect)?;
        if let Err(error) = self
            .verify_claim(&actual)
            .and_then(|()| check_cancel(cancellation))
        {
            return self.return_claim(state, step, &actual, error);
        }
        self.stage.directory_claim.as_mut().unwrap().phase = Phase::Claimed;
        if let Err(error) = save(state, step, &self.stage) {
            return self.return_claim(state, step, &actual, error);
        }
        #[cfg(test)]
        super::directory_removal_boundary::at_phase(
            "verified",
            &self.source_path,
            &self.stage.payload(),
        );
        if let Err(error) = self
            .verify_claim(&actual)
            .and_then(|()| check_cancel(cancellation))
        {
            return self.return_claim(state, step, &actual, error);
        }
        if let Err(error) = rustix::fs::unlinkat(
            self.source.parent(),
            self.stage.payload().file_name().unwrap(),
            AtFlags::REMOVEDIR,
        ) {
            return self.return_claim(state, step, &actual, error.into());
        }
        self.opened.sync().map_err(protect)?;
        #[cfg(test)]
        super::directory_removal_boundary::at_phase(
            "removed",
            &self.source_path,
            &self.stage.payload(),
        );
        finish_removed(state, step, &self.stage).map_err(protect)
    }
    fn verify_claim(&self, actual: &fs::File) -> io::Result<()> {
        self.opened.require_current()?;
        let ownership = self.stage.ownership.as_ref().ok_or_else(interrupted)?;
        require_parent(ownership, &self.source_path)?;
        require_original(
            ownership,
            &named(
                self.source.parent(),
                self.stage.payload().file_name().unwrap(),
            )?,
            self.facts.source,
        )?;
        if Identity::from_metadata(&actual.metadata()?) != self.facts.source {
            return Err(interrupted());
        }
        require_empty(actual)?;
        self.facts
            .original
            .require(&self.source_path, &self.destination, true)
    }
    fn abort_empty(&self, state: &mut Store, step: Step<'_>, error: io::Error) -> io::Result<()> {
        if optional(
            self.source.parent(),
            self.stage.payload().file_name().unwrap(),
        )?
        .is_some()
        {
            return Err(protect(error));
        }
        let normal = normalized(&self.stage);
        state.returned_source_intent(&Pending::Preparing {
            id: step.id,
            destination: self.source_path.clone(),
            stage: normal,
        })?;
        self.opened.clean_empty().map_err(protect)?;
        state.clear_intent()?;
        Err(error)
    }
    fn return_claim(
        &mut self,
        state: &mut Store,
        step: Step<'_>,
        actual: &fs::File,
        error: io::Error,
    ) -> io::Result<()> {
        return_subject(
            state,
            step,
            &mut self.stage,
            self.source.parent(),
            &self.source_path,
            actual,
        )
        .map_err(protect)?;
        finish_returned(state, step, &self.source_path, &self.stage).map_err(protect)?;
        Err(error)
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        if !self.journal_owned
            && optional(
                self.source.parent(),
                self.stage.payload().file_name().unwrap(),
            )
            .is_ok_and(|p| p.is_none())
        {
            let _empty_only = self.opened.clean_empty();
        }
    }
}

pub(super) fn recover(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    stage: &Stage,
) -> io::Result<()> {
    let step = Step {
        id,
        ordinal,
        change,
    };
    let Change::RemovedDirectory {
        path,
        identity,
        parent_identity,
        moved: Some(moved),
        removal: Some(facts),
        ..
    } = change
    else {
        return Err(invalid(
            "A directory claim needs its exact source-removal step.",
        ));
    };
    validate_claim_stage(Some(stage), path)?;
    if stage.replacement.is_some()
        || stage.entry_move.is_some()
        || stage.source_claim.is_some()
        || !stage.sibling_payload
    {
        return Err(interrupted());
    }
    let claim = stage.directory_claim.as_ref().ok_or_else(interrupted)?;
    let ownership = stage.ownership.as_ref().ok_or_else(interrupted)?;
    if claim.facts != **facts
        || *identity != facts.source
        || *parent_identity != facts.parent
        || moved.identity != facts.destination
        || stage.payload_identity != Some(*identity)
        || stage.parent_identity != Some(*parent_identity)
        || ownership.payload_origins() != Some(facts.original.source_origins())
    {
        return Err(interrupted());
    }
    let parent = require_parent(ownership, path)?;
    if Identity::from_metadata(&parent.metadata()?) != *parent_identity {
        return Err(interrupted());
    }
    let directory = match openat(
        &parent,
        stage.directory.file_name().unwrap(),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(file) => {
            let file = fs::File::from(file);
            ownership.require_directory(&file)?;
            if Identity::from_metadata(&file.metadata()?) != stage.identity {
                return Err(interrupted());
            }
            Some(file)
        }
        Err(rustix::io::Errno::NOENT) => None,
        Err(error) => return Err(error.into()),
    };
    let payload = optional(&parent, stage.payload().file_name().unwrap())?;
    if payload.is_some() && directory.is_none() {
        return Err(interrupted());
    }
    if claim.phase == Phase::Returning {
        let subject = claim.returning.as_ref().ok_or_else(interrupted)?;
        let original = subject.original.as_ref().ok_or_else(interrupted)?;
        if let Some(payload) = payload {
            original.require(&payload)?;
            if Identity::from_metadata(&payload.metadata()?) != subject.identity {
                return Err(interrupted());
            }
            move_at(
                &parent,
                stage.payload().file_name().unwrap(),
                path.file_name().unwrap(),
            )?;
            sync_parent(&parent)?;
        } else {
            let returned = named(&parent, path.file_name().unwrap())?;
            original.require(&returned)?;
            if Identity::from_metadata(&returned.metadata()?) != subject.identity {
                return Err(interrupted());
            }
        }
        return finish_returned(state, step, path, stage);
    }
    if let Some(payload) = payload {
        require_original(ownership, &payload, *identity)?;
        let verified = require_empty(&payload)
            .and_then(|()| facts.original.require(path, &moved.current, true));
        if claim.phase == Phase::Ready || verified.is_err() {
            let mut returning = stage.clone();
            return_subject(state, step, &mut returning, &parent, path, &payload)?;
            return finish_returned(state, step, path, &returning);
        }
        require_original(
            ownership,
            &named(&parent, stage.payload().file_name().unwrap())?,
            *identity,
        )?;
        rustix::fs::unlinkat(
            &parent,
            stage.payload().file_name().unwrap(),
            AtFlags::REMOVEDIR,
        )?;
        sync_parent(&parent)?;
    } else {
        if claim.phase == Phase::Ready
            && let Some(source) = optional(&parent, path.file_name().unwrap())?
        {
            match require_original(ownership, &source, *identity) {
                Ok(()) => return finish_returned(state, step, path, stage),
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        && error.raw_os_error().is_none() => {}
                Err(error) => return Err(error),
            }
        }
        if claim.phase == Phase::Ready {
            return Err(interrupted());
        }
    }
    // Completed unlink is metadata only; never touch a newer source occupant.
    finish_removed(state, step, stage)
}

fn return_subject(
    state: &mut Store,
    step: Step<'_>,
    stage: &mut Stage,
    parent: &fs::File,
    source: &Path,
    actual: &fs::File,
) -> io::Result<()> {
    let identity = Identity::from_metadata(&actual.metadata()?);
    if Identity::from_metadata(&named(parent, stage.payload().file_name().unwrap())?.metadata()?)
        != identity
    {
        return Err(interrupted());
    }
    let claim = stage.directory_claim.as_mut().ok_or_else(interrupted)?;
    claim.phase = Phase::Returning;
    claim.returning = Some(Subject {
        identity,
        original: original::Original::capture(actual).ok(),
    });
    save(state, step, stage)?;
    #[cfg(test)]
    super::directory_removal_boundary::at_phase("returning", source, &stage.payload());
    move_at(
        parent,
        stage.payload().file_name().unwrap(),
        source.file_name().unwrap(),
    )?;
    if Identity::from_metadata(&named(parent, source.file_name().unwrap())?.metadata()?) != identity
    {
        return Err(interrupted());
    }
    sync_parent(parent)?;
    #[cfg(test)]
    super::directory_removal_boundary::at_phase("returned", source, &stage.payload());
    Ok(())
}
fn finish_returned(
    state: &mut Store,
    step: Step<'_>,
    source: &Path,
    stage: &Stage,
) -> io::Result<()> {
    let ownership = stage.ownership.as_ref().ok_or_else(interrupted)?;
    let parent = require_parent(ownership, source)?;
    if optional(&parent, stage.payload().file_name().unwrap())?.is_some() {
        return Err(interrupted());
    }
    let normal = normalized(stage);
    state.returned_source_intent(&Pending::Preparing {
        id: step.id,
        destination: source.to_owned(),
        stage: normal.clone(),
    })?;
    #[cfg(test)]
    super::transfer::directory_boundary::removal_boundary("return-preparing", source);
    clean_stage(&normal)?;
    state.clear_intent()
}
fn finish_removed(state: &mut Store, step: Step<'_>, stage: &Stage) -> io::Result<()> {
    append_forward(
        state,
        step.id,
        step.ordinal,
        step.change,
        Some(&normalized(stage)),
    )?;
    #[cfg(test)]
    super::transfer::directory_boundary::removal_boundary("finished", step.change.forward_path());
    Ok(())
}
fn normalized(stage: &Stage) -> Stage {
    let mut stage = stage.clone();
    stage.directory_claim = None;
    stage.sibling_payload = false;
    stage.payload_identity = None;
    stage.ownership = stage
        .ownership
        .map(|proof| Box::new((*proof).without_payload()));
    stage
}
pub(super) fn returned_transition(before: &Pending, after: &Pending) -> bool {
    let Pending::Forward {
        id,
        change:
            Change::RemovedDirectory {
                path,
                removal: Some(_),
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
        && path == destination
        && stage
            .directory_claim
            .as_ref()
            .is_some_and(|claim| matches!(claim.phase, Phase::Ready | Phase::Returning))
        && normalized(stage) == *empty
}
fn save(state: &mut Store, step: Step<'_>, stage: &Stage) -> io::Result<()> {
    state.intent(&Pending::Forward {
        id: step.id,
        ordinal: step.ordinal,
        change: step.change.clone(),
        stage: Some(stage.clone()),
    })
}
fn require_parent(ownership: &staging::Ownership, source: &Path) -> io::Result<fs::File> {
    let parent = fs::File::from(open(
        source.parent().unwrap(),
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    ownership.require_parent(&parent)?;
    Ok(parent)
}
fn require_original(
    ownership: &staging::Ownership,
    file: &fs::File,
    identity: Identity,
) -> io::Result<()> {
    ownership.require_payload(file)?;
    if Identity::from_metadata(&file.metadata()?) != identity {
        return Err(interrupted());
    }
    Ok(())
}
fn require_empty(file: &fs::File) -> io::Result<()> {
    let readable = fs::File::from(openat(
        file,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    for entry in rustix::fs::Dir::new(readable)? {
        let entry = entry?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            return Err(io::Error::new(
                io::ErrorKind::DirectoryNotEmpty,
                "The source folder has newer entries; they are preserved.",
            ));
        }
    }
    Ok(())
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
    parent: &fs::File,
    from: impl rustix::path::Arg,
    to: impl rustix::path::Arg,
) -> io::Result<()> {
    rustix::fs::renameat_with(parent, from, parent, to, RenameFlags::NOREPLACE).map_err(Into::into)
}
fn sync_parent(parent: &fs::File) -> io::Result<()> {
    fs::File::from(openat(
        parent,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
    .sync_all()
}
fn protect(error: io::Error) -> io::Error {
    transfer::destination_failure(error)
}
pub(super) fn interrupted() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "Files could not verify this source folder's cleanup. Its folders are preserved for Recovery.",
    )
}
