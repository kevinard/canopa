//! Cross-filesystem source removal keeps the original opened leaf and claims it
//! through its held parent. A Ready claim never authorizes payload deletion.
use super::*;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags, open, openat};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Phase {
    Ready,
    Claimed,
    Returning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Claim {
    phase: Phase,
    copy_identity: Identity,
    copy: Option<staging::Published>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    return_subject: Option<original::Original>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Originals {
    source_parent: original::Original,
    source: original::Original,
    copy: Option<staging::Published>,
}
impl Originals {
    pub(super) fn matches_copied(
        &self,
        parent: original::Original,
        source: original::Original,
        copy: &staging::Published,
    ) -> bool {
        self.source_parent == parent && self.source == source && self.copy.as_ref() == Some(copy)
    }

    fn matches(&self, ownership: &staging::Ownership, copy: &Option<staging::Published>) -> bool {
        ownership.payload_origins() == Some((self.source_parent, self.source)) && self.copy == *copy
    }
}

pub(super) struct Prepared {
    source: fingerprint::source::Opened,
    opened: staging::Opened,
    stage: Stage,
    destination: PathBuf,
    journal_owned: bool,
}
impl Prepared {
    pub(super) fn originals(&self) -> Option<Box<Originals>> {
        let (source_parent, source) = self.stage.ownership.as_ref()?.payload_origins()?;
        Some(Box::new(Originals {
            source_parent,
            source,
            copy: self.stage.source_claim.as_ref()?.copy.clone(),
        }))
    }
    pub(super) fn open(
        source: fingerprint::source::Opened,
        destination: &Path,
        copy: &impl staging::VerifiedCopy,
    ) -> io::Result<Self> {
        source.require_current()?;
        let proof = copy.published(destination)?;
        let (opened, mut stage, temporary) =
            staging::Opened::prepare_in(source.path().parent().unwrap(), source.parent())?;
        let _retained = temporary.keep();
        stage.payload_identity = Some(Identity::from_metadata(&source.entry().metadata()?));
        stage.ownership = stage
            .ownership
            .take()
            .map(|proof| Box::new((*proof).with_payload(source.entry())));
        stage.source_claim = Some(Box::new(Claim {
            phase: Phase::Ready,
            copy_identity: Identity::from_metadata(&copy.metadata()?),
            copy: proof,
            return_subject: None,
        }));
        Ok(Self {
            source,
            opened,
            stage,
            destination: destination.to_owned(),
            journal_owned: false,
        })
    }
    pub(super) fn quarantine(&self) -> Stage {
        returned_stage(&self.stage)
    }
    pub(super) fn commit(
        mut self,
        state: &mut Store,
        id: u64,
        change: &Change,
        copy: &impl staging::VerifiedCopy,
        cancellation: &Cancellation,
        mut checkpoint: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        let Change::SourceRemoved {
            identity,
            copy_identity,
            fingerprint,
            source_fingerprint,
            ..
        } = change
        else {
            return Err(invalid("A source claim needs its removal step."));
        };
        if self.stage.payload_identity != Some(*identity)
            || self.stage.source_claim.as_ref().unwrap().copy_identity != *copy_identity
        {
            return Err(invalid(
                "The source claim does not match its prepared objects.",
            ));
        }
        let ordinal = state.next_ordinal(id)?;
        self.save(state, id, ordinal, change)?;
        self.journal_owned = true;
        let mut before = || -> io::Result<()> {
            checkpoint()?;
            self.source.require_current()?;
            self.require_parent()?;
            self.opened.require_current()?;
            check_cancel(cancellation)
        };
        if let Err(error) = before() {
            if transfer::placement_failure(&error).is_some() {
                return Err(protect(error));
            }
            return self.abort_empty(state, id, error);
        }
        #[cfg(test)]
        super::rename_boundary::before_rename(self.source.path(), &self.stage.payload());
        if let Err(error) = move_at(
            self.source.parent(),
            self.source.path().file_name().unwrap(),
            self.opened.directory(),
            c"folder",
        ) {
            if named(self.opened.directory(), c"folder")
                .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
                && self.source.require_current().is_ok()
            {
                return self.abort_empty(state, id, error);
            }
            return Err(protect(error));
        }
        self.opened.sync().map_err(protect)?;
        #[cfg(test)]
        super::source_removal_boundary::after_claim(self.source.path(), &self.stage.payload());
        #[cfg(test)]
        super::transfer::copy_boundary::claimed(self.source.path(), cancellation);
        let actual = named(self.opened.directory(), c"folder").map_err(protect)?;
        let mut verified = || -> io::Result<()> {
            if Identity::from_metadata(&actual.metadata()?) != *identity {
                return Err(changed());
            }
            self.opened.require_current()?;
            self.require_copy(copy)?;
            // A clone and its source are read once (ADR 0152).
            let (source_now, copy_now) =
                copy.fingerprint_with_source(self.source.entry(), cancellation)?;
            if source_now != *source_fingerprint || copy_now != *fingerprint {
                return Err(changed());
            }
            self.require_copy(copy)?;
            checkpoint()?;
            check_cancel(cancellation)
        };
        if let Err(error) = verified() {
            if transfer::placement_failure(&error).is_some() {
                return Err(protect(error));
            }
            return self.return_claim(state, id, ordinal, change, &actual, error);
        }
        self.stage.source_claim.as_mut().unwrap().phase = Phase::Claimed;
        if let Err(error) = self.save(state, id, ordinal, change) {
            return self.return_claim(state, id, ordinal, change, &actual, error);
        }
        #[cfg(test)]
        super::source_removal_boundary::before_remove(self.source.path(), &self.stage.payload());
        // The second check is after all callbacks and the durable claim save.
        // A substituted private payload is preserved, never returned or deleted.
        if Identity::from_metadata(
            &named(self.opened.directory(), c"folder")
                .map_err(protect)?
                .metadata()?,
        ) != *identity
        {
            return Err(protect(changed()));
        }
        if let Err(error) = (|| -> io::Result<()> {
            self.opened.require_current()?;
            check_cancel(cancellation)?;
            self.require_copy(copy)?;
            let (source_now, copy_now) =
                copy.fingerprint_with_source(self.source.entry(), cancellation)?;
            if source_now != *source_fingerprint || copy_now != *fingerprint {
                return Err(changed());
            }
            self.require_copy(copy)
        })() {
            return self.return_claim(state, id, ordinal, change, &actual, error);
        }
        rustix::fs::unlinkat(self.opened.directory(), c"folder", AtFlags::empty())
            .map_err(protect)?;
        self.opened.sync().map_err(protect)?;
        #[cfg(test)]
        super::source_removal_boundary::after_remove(self.source.path(), &self.stage.payload());
        #[cfg(test)]
        super::transfer::copy_boundary::boundary("unlinked", self.source.path());
        append_forward(state, id, ordinal, change, Some(&self.quarantine())).map_err(protect)?;
        #[cfg(test)]
        super::transfer::copy_boundary::boundary("removed", self.source.path());
        Ok(())
    }
    /// The copy is still the published destination in its recorded parent.
    fn require_copy(&self, copy: &impl staging::VerifiedCopy) -> io::Result<()> {
        copy.require_published(&self.destination)?;
        self.require_parent()?;
        if let Some(proof) = &self.stage.source_claim.as_ref().unwrap().copy {
            proof.require(&self.destination)?;
        }
        Ok(())
    }
    fn save(&self, state: &mut Store, id: u64, ordinal: u64, change: &Change) -> io::Result<()> {
        state.intent(&Pending::Forward {
            id,
            ordinal,
            change: change.clone(),
            stage: Some(self.stage.clone()),
        })
    }
    fn abort_empty(&self, state: &mut Store, id: u64, error: io::Error) -> io::Result<()> {
        let stage = returned_stage(&self.stage);
        state.returned_source_intent(&Pending::Preparing {
            id,
            destination: self.source.path().to_owned(),
            stage,
        })?;
        #[cfg(test)]
        super::transfer::copy_boundary::boundary("return-preparing", self.source.path());
        self.opened.clean_empty().map_err(protect)?;
        state.clear_intent()?;
        Err(error)
    }
    fn require_parent(&self) -> io::Result<()> {
        if let Some(ownership) = &self.stage.ownership {
            let parent = fs::File::from(open(
                self.source.path().parent().unwrap(),
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            ownership.require_parent(&parent)?;
        }
        Ok(())
    }
    fn return_claim(
        &mut self,
        state: &mut Store,
        id: u64,
        ordinal: u64,
        change: &Change,
        actual: &fs::File,
        error: io::Error,
    ) -> io::Result<()> {
        let identity = Identity::from_metadata(&actual.metadata()?);
        if Identity::from_metadata(&named(self.opened.directory(), c"folder")?.metadata()?)
            != identity
        {
            return Err(protect(changed()));
        }
        let claim = self.stage.source_claim.as_mut().unwrap();
        claim.phase = Phase::Returning;
        claim.return_subject = original::Original::capture(actual).ok();
        self.save(state, id, ordinal, change).map_err(protect)?;
        #[cfg(test)]
        super::source_removal_boundary::before_return(self.source.path(), &self.stage.payload());
        move_at(
            self.opened.directory(),
            c"folder",
            self.source.parent(),
            self.source.path().file_name().unwrap(),
        )
        .map_err(protect)?;
        if Identity::from_metadata(
            &named(
                self.source.parent(),
                self.source.path().file_name().unwrap(),
            )?
            .metadata()?,
        ) != identity
        {
            return Err(protect(changed()));
        }
        self.opened.sync().map_err(protect)?;
        #[cfg(test)]
        super::source_removal_boundary::after_return(self.source.path(), &self.stage.payload());
        #[cfg(test)]
        super::transfer::copy_boundary::boundary("returned", self.source.path());
        self.abort_empty(state, id, error)
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        if !self.journal_owned {
            let _empty_only = self.opened.clean_empty();
        }
    }
}

/// Only the exact normalization of an unstarted or authorized returned claim
/// can drop its uncommitted task receipt. Claimed removal is never a return.
pub(super) fn returned_transition(before: &Pending, after: &Pending) -> bool {
    let Pending::Forward {
        id,
        change: Change::SourceRemoved { original, .. },
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
        && original == destination
        && stage
            .source_claim
            .as_ref()
            .is_some_and(|claim| matches!(claim.phase, Phase::Ready | Phase::Returning))
        && returned_stage(stage) == *empty
}
fn returned_stage(stage: &Stage) -> Stage {
    let mut stage = stage.clone();
    stage.source_claim = None;
    stage.payload_identity = None;
    stage.ownership = stage
        .ownership
        .map(|proof| Box::new((*proof).without_payload()));
    stage
}
fn named(parent: &fs::File, name: impl rustix::path::Arg) -> io::Result<fs::File> {
    Ok(fs::File::from(openat(
        parent,
        name,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?))
}
fn move_at(
    source: &fs::File,
    name: impl rustix::path::Arg,
    target: &fs::File,
    destination: impl rustix::path::Arg,
) -> io::Result<()> {
    rustix::fs::renameat_with(source, name, target, destination, RenameFlags::NOREPLACE)
        .map_err(Into::into)
}
fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The original source, copy or source folder changed; its current contents are preserved.",
    )
}
pub(super) fn interrupted() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "Move stopped before source cleanup could be verified. Its files are preserved for Recovery.",
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
    let Change::SourceRemoved {
        original,
        current,
        identity,
        copy_identity,
        fingerprint,
        source_fingerprint,
        quarantine,
        originals,
        ..
    } = change
    else {
        return Err(invalid("An entry claim needs its source-removal step."));
    };
    validate_stage(Some(stage), original)?;
    if returned_stage(stage) != *quarantine {
        return Err(invalid("The source claim does not match its removal step."));
    }
    let claim = stage.source_claim.as_ref().ok_or_else(interrupted)?;
    if stage.payload_identity != Some(*identity) || claim.copy_identity != *copy_identity {
        return Err(invalid(
            "The source claim's object identities differ from its step.",
        ));
    }
    let ownership = stage.ownership.as_ref().ok_or_else(interrupted)?;
    if originals
        .as_ref()
        .is_some_and(|originals| !originals.matches(ownership, &claim.copy))
    {
        return Err(invalid(
            "The source claim's original facts differ from its recorded step.",
        ));
    }
    let parent = fs::File::from(open(
        original.parent().unwrap(),
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    ownership.require_parent(&parent)?;
    let directory = match openat(
        &parent,
        stage.directory.file_name().unwrap(),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(directory) => {
            let directory = fs::File::from(directory);
            ownership.require_directory(&directory)?;
            Some(directory)
        }
        Err(rustix::io::Errno::NOENT) => None,
        Err(error) => return Err(error.into()),
    };
    let payload = directory
        .as_ref()
        .map(|directory| named(directory, c"folder"))
        .transpose();
    let payload = match payload {
        Ok(payload) => payload,
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if claim.phase == Phase::Returning {
        let subject = claim.return_subject.as_ref().ok_or_else(interrupted)?;
        if let Some(payload) = payload {
            subject.require(&payload)?;
            let directory = directory.as_ref().unwrap();
            move_at(directory, c"folder", &parent, original.file_name().unwrap())?;
            directory.sync_all()?;
            sync_directory(&parent)?;
        } else {
            subject.require(&named(&parent, original.file_name().unwrap())?)?;
        }
        return clear_returned(state, id, original, stage);
    }
    if let Some(payload) = payload {
        ownership.require_payload(&payload)?;
        if Identity::from_metadata(&payload.metadata()?) != *identity {
            return Err(changed());
        }
        let directory = directory.as_ref().unwrap();
        if claim.phase == Phase::Ready {
            // The process died before a verified removal intent. Return only
            // the recorded original, without replacing a current occupant.
            move_at(directory, c"folder", &parent, original.file_name().unwrap())?;
            directory.sync_all()?;
            sync_directory(&parent)?;
            return clear_returned(state, id, original, stage);
        }
        let copy = claim
            .copy
            .as_ref()
            .ok_or_else(interrupted)?
            .require(current)?;
        let (payload_now, copy_now) =
            Fingerprint::read_opened_pair(&payload, &copy, &Cancellation::default())?;
        if copy_now != *fingerprint || payload_now != *source_fingerprint {
            return Err(changed());
        }
        ownership.require_payload(&named(directory, c"folder")?)?;
        rustix::fs::unlinkat(directory, c"folder", AtFlags::empty())?;
        directory.sync_all()?;
    } else if claim.phase == Phase::Ready {
        let returned = match named(&parent, original.file_name().unwrap()) {
            Ok(file) => match ownership.require_payload(&file) {
                Ok(()) => true,
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        && error.raw_os_error().is_none() =>
                {
                    false
                }
                Err(error) => return Err(error),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(error),
        };
        if returned {
            return clear_returned(state, id, original, stage);
        }
        return Err(interrupted());
    }
    // Claimed plus an absent payload has consumed this source entry. A hard
    // link can reintroduce the same inode at its old name; it is not an engine
    // return and must remain untouched. Authorized returns use Returning above.
    let normal = returned_stage(stage);
    append_forward(state, id, ordinal, change, Some(&normal))
}
fn clear_returned(state: &mut Store, id: u64, original: &Path, stage: &Stage) -> io::Result<()> {
    let returned = returned_stage(stage);
    state.returned_source_intent(&Pending::Preparing {
        id,
        destination: original.to_owned(),
        stage: returned.clone(),
    })?;
    #[cfg(test)]
    super::transfer::copy_boundary::boundary("return-preparing", original);
    clean_stage(&returned)?;
    state.clear_intent()
}
fn sync_directory(directory: &fs::File) -> io::Result<()> {
    fs::File::from(openat(
        directory,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
    .sync_all()
}
