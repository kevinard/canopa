//! A reviewed replacement retains the actual old entry and its parent. The
//! previous version is claimed and returned through those held directories.
use super::*;
use original::Original;
use rustix::fs::{Mode, OFlags, RenameFlags, open, openat};
mod copy;
mod recovery;
pub(super) use copy::{PreparedCopy, commit as commit_copy};
pub(super) use recovery::{Reopened, recover_copy};

#[derive(Clone, Copy)]
struct Step<'a> {
    id: u64,
    ordinal: u64,
    change: &'a Change,
}

/// Produced only by a verifier holding the actual reviewed saved entry.
/// It authorizes one canonical metadata refresh, not a new object selection.
pub(super) struct Refreshed {
    before: Change,
    after: Change,
}
impl Refreshed {
    pub(super) fn changes(&self) -> (&Change, &Change) {
        (&self.before, &self.after)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Originals {
    parent: Original,
    entry: Original,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Facts {
    version: fingerprint::Version,
    parent: Identity,
    originals: Option<Originals>,
}
impl Facts {
    pub(super) fn original_previous(&self, file: &fs::File) -> io::Result<Original> {
        let original = self.originals.as_ref().ok_or_else(interrupted)?.entry;
        original.require(file)?;
        Ok(original)
    }
    pub(super) fn matches_retained(&self, retained: &Retained, destination: &Path) -> bool {
        self.version.identity == retained.original_identity()
            && retained.physical_identity.is_none()
            && retained.stage.parent_identity == Some(self.parent)
            && retained.stage.directory.parent() == destination.parent()
            && retained
                .version
                .as_ref()
                .is_some_and(|version| version.identity == retained.identity)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Phase {
    Unclaimed,
    Ready,
    Saved,
    Returning,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Subject {
    identity: Identity,
    original: Option<Original>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CopyPhase {
    Ready,
    Publishing,
    Returning,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Copy {
    phase: CopyPhase,
    identity: Identity,
    fingerprint: Fingerprint,
    publication: Option<staging::Published>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Claim {
    facts: Facts,
    phase: Phase,
    returning: Option<Subject>,
    copy: Option<Copy>,
}

pub(super) struct Reviewed {
    parent: fs::File,
    entry: fs::File,
    path: PathBuf,
    facts: Facts,
}
impl Reviewed {
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        let result = Self::observe(path)?;
        result.require_leaf()?;
        Ok(result)
    }
    /// A changed copy may now be a folder. Holding its observation permits
    /// Keep/Skip review without granting permission to displace that folder.
    pub(super) fn observe(path: &Path) -> io::Result<Self> {
        let parent = fs::File::from(open(
            path.parent()
                .ok_or_else(|| invalid("A replacement needs its parent."))?,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let entry = named(
            &parent,
            path.file_name()
                .ok_or_else(|| invalid("A replacement needs its name."))?,
        )?;
        let metadata = entry.metadata()?;
        let originals = Original::capture(&parent)
            .ok()
            .zip(Original::capture(&entry).ok())
            .map(|(parent, entry)| Originals { parent, entry });
        let result = Self {
            facts: Facts {
                version: fingerprint::Version::from_metadata(&metadata),
                parent: Identity::from_metadata(&parent.metadata()?),
                originals,
            },
            parent,
            entry,
            path: path.to_owned(),
        };
        result.require_current()?;
        Ok(result)
    }
    fn require_leaf(&self) -> io::Result<()> {
        let metadata = self.entry.metadata()?;
        if metadata.is_file() || metadata.is_symlink() {
            Ok(())
        } else {
            Err(invalid("Only a regular file or link can be replaced."))
        }
    }
    pub(super) fn version(&self) -> &fingerprint::Version {
        &self.facts.version
    }
    pub(super) fn preservation_selection(&self) -> io::Result<(PathBuf, fs::File)> {
        self.require_leaf()?;
        self.require_current()?;
        Ok((self.path.clone(), self.entry.try_clone()?))
    }
    /// Retain the reviewed entry's original identity when Undo creates a new
    /// archive. These facts precede its displacement; recovery cannot mint them
    /// by inspecting whatever later occupies the saved name.
    pub(super) fn prepare_preservation(
        &self,
    ) -> io::Result<(staging::Opened, Stage, tempfile::TempDir)> {
        self.require_leaf()?;
        self.require_current()?;
        let (opened, mut stage, temporary) =
            staging::Opened::prepare_in(self.path.parent().unwrap(), &self.parent)?;
        if let (Some(roots), Some(originals)) = (&stage.ownership, &self.facts.originals) {
            let (parent, directory) = roots.roots();
            if parent != originals.parent {
                return Err(changed());
            }
            stage.ownership = Some(Box::new(staging::Ownership::saved(
                parent,
                directory,
                originals.entry,
            )));
        }
        self.require_current()?;
        Ok((opened, stage, temporary))
    }
    pub(super) fn preserve(&self, opened: &staging::Opened) -> io::Result<()> {
        self.require_current()?;
        opened.require_current()?;
        move_at(
            &self.parent,
            self.path.file_name().unwrap(),
            opened.directory(),
            c"previous",
        )?;
        opened.sync()?;
        let actual = named(opened.directory(), c"previous")?;
        if !self.facts.version.matches_opened(&actual, true)? {
            return Err(changed());
        }
        if let Some(originals) = &self.facts.originals {
            originals.entry.require(&actual)?;
        }
        self.require_parent()?;
        opened.require_current()
    }
    fn require_parent(&self) -> io::Result<()> {
        let current = fs::File::from(open(
            self.path.parent().unwrap(),
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        self.require_parent_file(&current)
    }
    fn require_parent_file(&self, parent: &fs::File) -> io::Result<()> {
        if Identity::from_metadata(&parent.metadata()?) != self.facts.parent {
            return Err(changed());
        }
        if let Some(originals) = &self.facts.originals {
            originals.parent.require(parent)?;
        }
        Ok(())
    }
    fn require_current(&self) -> io::Result<()> {
        self.require_parent()?;
        if !self.facts.version.matches_opened(&self.entry, false)?
            || !self
                .facts
                .version
                .matches_opened(&named(&self.parent, self.path.file_name().unwrap())?, false)?
        {
            return Err(changed());
        }
        Ok(())
    }
    pub(super) fn attach(&self, parent: &fs::File, stage: &mut Stage) -> io::Result<Retained> {
        self.require_parent_file(parent)?;
        if stage.directory.parent() != self.path.parent() || stage.replacement.is_some() {
            return Err(invalid(
                "A replacement needs its own work in the reviewed folder.",
            ));
        }
        stage.replacement = Some(Box::new(Claim {
            facts: self.facts.clone(),
            phase: Phase::Unclaimed,
            returning: None,
            copy: None,
        }));
        Ok(Retained {
            replacement_facts: Some(Box::new(self.facts.clone())),
            stage: normalized(stage),
            identity: self.facts.version.identity,
            physical_identity: None,
            recovery: false,
            version: Some(self.facts.version.clone()),
        })
    }
    pub(super) fn claim(
        &self,
        directory: &fs::File,
        stage: &mut Stage,
        mut save: impl FnMut(&Stage) -> io::Result<()>,
        cancellation: &Cancellation,
    ) -> io::Result<()> {
        #[cfg(test)]
        super::replacement_boundary::at_phase(
            "unclaimed",
            &self.path,
            &stage.directory.join("previous"),
        );
        self.require_current()?;
        check_cancel(cancellation)?;
        stage.replacement.as_mut().ok_or_else(interrupted)?.phase = Phase::Ready;
        save(stage)?;
        #[cfg(test)]
        super::replacement_boundary::at_phase(
            "ready",
            &self.path,
            &stage.directory.join("previous"),
        );
        #[cfg(test)]
        super::rename_boundary::before_rename(&self.path, &stage.directory.join("previous"));
        move_at(
            &self.parent,
            self.path.file_name().unwrap(),
            directory,
            c"previous",
        )?;
        sync(directory, &self.parent)?;
        #[cfg(test)]
        super::transfer::replacement_boundary::cancel_return(&self.path, cancellation);
        #[cfg(test)]
        super::replacement_boundary::at_phase(
            "backup",
            &self.path,
            &stage.directory.join("previous"),
        );
        let actual = named(directory, c"previous")?;
        let verified = self.require_parent().and_then(|()| {
            if !self.facts.version.matches_opened(&actual, true)?
                || Identity::from_metadata(&actual.metadata()?)
                    != Identity::from_metadata(&self.entry.metadata()?)
            {
                return Err(changed());
            }
            check_cancel(cancellation)
        });
        if let Err(error) = verified {
            self.restore_actual(directory, stage, &actual, &mut save)?;
            return Err(error);
        }
        stage.replacement.as_mut().unwrap().phase = Phase::Saved;
        save(stage)?;
        Ok(())
    }
    fn restore_actual(
        &self,
        directory: &fs::File,
        stage: &mut Stage,
        actual: &fs::File,
        save: &mut impl FnMut(&Stage) -> io::Result<()>,
    ) -> io::Result<()> {
        let identity = Identity::from_metadata(&actual.metadata()?);
        if Identity::from_metadata(&named(directory, c"previous")?.metadata()?) != identity {
            return Err(changed());
        }
        let claim = stage.replacement.as_mut().unwrap();
        claim.phase = Phase::Returning;
        claim.returning = Some(Subject {
            identity,
            original: Original::capture(actual).ok(),
        });
        save(stage)?;
        #[cfg(test)]
        super::replacement_boundary::at_phase(
            "returning",
            &self.path,
            &stage.directory.join("previous"),
        );
        move_at(
            directory,
            c"previous",
            &self.parent,
            self.path.file_name().unwrap(),
        )?;
        if Identity::from_metadata(
            &named(&self.parent, self.path.file_name().unwrap())?.metadata()?,
        ) != identity
        {
            return Err(changed());
        }
        sync(directory, &self.parent)?;
        #[cfg(test)]
        super::replacement_boundary::at_phase(
            "returned",
            &self.path,
            &stage.directory.join("previous"),
        );
        Ok(())
    }
    pub(super) fn restore(
        &self,
        directory: &fs::File,
        stage: &mut Stage,
        mut save: impl FnMut(&Stage) -> io::Result<()>,
    ) -> io::Result<()> {
        if stage.replacement.as_ref().ok_or_else(interrupted)?.phase == Phase::Unclaimed {
            return if optional(directory, c"previous")?.is_none() {
                Ok(())
            } else {
                Err(changed())
            };
        }
        if let Some(actual) = optional(directory, c"previous")? {
            let claim = stage.replacement.as_ref().ok_or_else(interrupted)?;
            if claim.phase == Phase::Returning {
                let subject = claim.returning.as_ref().ok_or_else(interrupted)?;
                if Identity::from_metadata(&actual.metadata()?) != subject.identity {
                    return Err(changed());
                }
                if let Some(original) = &subject.original {
                    original.require(&actual)?;
                }
            } else {
                if Identity::from_metadata(&actual.metadata()?) != self.facts.version.identity {
                    return Err(changed());
                }
                if let Some(originals) = &self.facts.originals {
                    originals.entry.require(&actual)?;
                }
            }
            self.restore_actual(directory, stage, &actual, &mut save)
        } else {
            let actual = named(&self.parent, self.path.file_name().unwrap())?;
            let subject = stage
                .replacement
                .as_ref()
                .and_then(|claim| claim.returning.as_ref());
            let identity = subject.map_or(self.facts.version.identity, |subject| subject.identity);
            if Identity::from_metadata(&actual.metadata()?) != identity {
                return Err(changed());
            }
            if let Some(subject) = subject {
                if let Some(original) = &subject.original {
                    original.require(&actual)?;
                }
            } else if let Some(originals) = &self.facts.originals {
                originals.entry.require(&actual)?;
            }
            // Ready can precede the backup rename. After proving that its old
            // entry is still at the target, record that exact return subject
            // before disposing the unused claim and its task receipt.
            let claim = stage.replacement.as_mut().unwrap();
            if claim.phase != Phase::Returning || claim.returning.is_none() {
                claim.phase = Phase::Returning;
                claim.returning = Some(Subject {
                    identity,
                    original: Original::capture(&actual).ok(),
                });
                save(stage)?;
            }
            Ok(())
        }
    }
    pub(super) fn require_saved(&self, directory: &fs::File) -> io::Result<()> {
        self.require_parent()?;
        let actual = named(directory, c"previous")?;
        if Identity::from_metadata(&actual.metadata()?) != self.facts.version.identity {
            return Err(changed());
        }
        if let Some(originals) = &self.facts.originals {
            originals.entry.require(&actual)?;
        }
        Ok(())
    }
    pub(super) fn refresh(&self, directory: &fs::File, retained: &mut Retained) -> io::Result<()> {
        self.require_saved(directory)?;
        if retained.replacement_facts.as_deref() != Some(&self.facts) {
            return Err(invalid("The retained replacement differs from its review."));
        }
        let actual = named(directory, c"previous")?;
        retained.recovery |= !self.facts.version.matches_opened(&actual, true)?;
        retained.identity = Identity::from_metadata(&actual.metadata()?);
        retained.version = Some(fingerprint::Version::from_metadata(&actual.metadata()?));
        Ok(())
    }
    pub(super) fn refreshed(&self, directory: &fs::File, change: &Change) -> io::Result<Refreshed> {
        let mut after = change.clone();
        let retained = after.retained_mut().ok_or_else(interrupted)?;
        if Identity::from_metadata(&directory.metadata()?) != retained.stage.identity {
            return Err(changed());
        }
        if let Some(ownership) = &retained.stage.ownership {
            ownership.require_directory(directory)?;
        }
        self.refresh(directory, retained)?;
        Ok(Refreshed {
            before: change.clone(),
            after,
        })
    }
}

pub(super) fn normalized(stage: &Stage) -> Stage {
    let mut stage = stage.clone();
    stage.replacement = None;
    stage.entry_move = None;
    stage.source_claim = None;
    stage.payload_identity = None;
    stage.sibling_payload = false;
    stage.ownership = stage
        .ownership
        .map(|proof| Box::new((*proof).without_payload()));
    stage
}
pub(super) fn returned_transition(before: &Pending, after: &Pending) -> bool {
    let Pending::Forward {
        id,
        change,
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
    let Some(claim) = &stage.replacement else {
        return false;
    };
    let Some(retained) = change.retained() else {
        return false;
    };
    let role = match change {
        Change::Copied {
            identity,
            fingerprint,
            publication,
            retained: Some(_),
            ..
        } => claim.copy.as_ref().is_some_and(|copy| {
            copy.phase == CopyPhase::Returning
                && copy.identity == *identity
                && copy.fingerprint == *fingerprint
                && copy.publication.as_ref() == publication.as_deref()
        }),
        Change::Rename {
            move_facts: Some(facts),
            retained: Some(_),
            reviewed: None,
            saved: None,
            ..
        } => claim.copy.is_none() && entry_move::returned_claim(stage, facts),
        _ => false,
    };
    id == next
        && destination == change.forward_path()
        && role
        && matches!(claim.phase, Phase::Unclaimed | Phase::Returning)
        && retained.replacement_facts.as_deref() == Some(&claim.facts)
        && retained.stage == normalized(stage)
        && claim.facts.matches_retained(retained, destination)
        && normalized(stage) == *empty
}
pub(super) fn interrupted() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "Replacement stopped before its saved version could be verified. Its files are preserved for Recovery.",
    )
}
fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The reviewed destination or its folder changed; its files are preserved.",
    )
}
fn protect(error: impl Into<io::Error>) -> io::Error {
    transfer::destination_failure(error.into())
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
    target: &fs::File,
    to: impl rustix::path::Arg,
) -> io::Result<()> {
    rustix::fs::renameat_with(source, name, target, to, RenameFlags::NOREPLACE).map_err(Into::into)
}
fn sync(directory: &fs::File, parent: &fs::File) -> io::Result<()> {
    directory.sync_all()?;
    fs::File::from(openat(
        parent,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
    .sync_all()
}
