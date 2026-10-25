//! Copy preparation retains the actual parent, private directory and payload.
//! Persisted facts constrain cleanup; they do not reconstruct an execution owner.
use super::*;
use original::Original;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags, open, openat};
use std::{ffi::OsString, os::fd::AsRawFd};
pub(super) mod disposal;
mod publication;
pub(super) use publication::recover as recover_copy;

#[cfg(test)]
mod cleanup_boundary;
#[cfg(test)]
mod publication_boundary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Ownership {
    parent: Original,
    directory: Original,
    payload: Option<Original>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous: Option<Original>,
}
impl Ownership {
    pub(super) fn publication(&self) -> Option<Published> {
        Some(Published {
            parent: self.parent,
            entry: self.payload?,
        })
    }
    pub(super) fn payload_origins(&self) -> Option<(Original, Original)> {
        Some((self.parent, self.payload?))
    }
    pub(super) fn without_payload(mut self) -> Self {
        self.payload = None;
        self
    }
    pub(super) fn with_parent(mut self, parent: Original) -> Self {
        self.parent = parent;
        self
    }
    pub(super) fn require_parent(&self, parent: &fs::File) -> io::Result<()> {
        self.parent.require(parent)
    }
    pub(super) fn require_directory(&self, directory: &fs::File) -> io::Result<()> {
        self.directory.require(directory)
    }
    pub(super) fn require_payload(&self, payload: &fs::File) -> io::Result<()> {
        self.payload
            .as_ref()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    "The original claimed source cannot be proved.",
                )
            })?
            .require(payload)
    }
    pub(super) fn capture(parent: &fs::File, directory: &fs::File) -> Option<Self> {
        Some(Self {
            parent: Original::capture(parent).ok()?,
            directory: Original::capture(directory).ok()?,
            payload: None,
            previous: None,
        })
    }
    pub(super) fn roots(&self) -> (Original, Original) {
        (self.parent, self.directory)
    }
    pub(super) fn previous(&self, file: &fs::File) -> io::Result<Option<Original>> {
        if let Some(original) = self.previous {
            original.require(file)?;
        }
        Ok(self.previous)
    }
    pub(super) fn previous_origin(&self) -> Option<Original> {
        self.previous
    }
    pub(super) fn saved(parent: Original, directory: Original, previous: Original) -> Self {
        Self {
            parent,
            directory,
            previous: Some(previous),
            payload: None,
        }
    }
    /// The created payload becomes the saved previous entry when its actual
    /// work directory is published whole. A missing created-leaf generation
    /// cannot become saved-entry proof. Never capture a current path here.
    pub(super) fn as_saved_copy(&self) -> Option<Self> {
        Some(Self::saved(self.parent, self.directory, self.payload?))
    }
    pub(super) fn with_payload(mut self, payload: &fs::File) -> Self {
        // A filesystem may support directory handles but not this leaf type.
        // Retain the proven roots and leave the leaf unknown; never downgrade
        // an already-recorded stage to the legacy cleanup interpretation.
        self.payload = Original::capture(payload).ok();
        self
    }
}

pub(super) struct Opened {
    parent: fs::File,
    directory: fs::File,
    parent_path: PathBuf,
    name: OsString,
    parent_identity: Identity,
    identity: Identity,
}

/// Borrowed from the actual creator. A decoded Stage cannot supply these held
/// directories or turn a current pathname into new preparation ownership.
pub(super) struct Registration<'a> {
    record: &'a Stage,
    source: RegistrationSource<'a>,
    parent: &'a fs::File,
    directory: &'a fs::File,
    payload: Option<&'a fs::File>,
}
enum RegistrationSource<'a> {
    Copy(&'a fingerprint::source::Opened),
    Directory(&'a transfer::directory::Opened),
}
impl<'a> Registration<'a> {
    pub(super) fn require_reviewed(&self, version: &fingerprint::Version) -> io::Result<()> {
        let RegistrationSource::Copy(source) = self.source else {
            return Err(changed());
        };
        if !version.matches_opened(source.entry(), false)? {
            return Err(changed());
        }
        Ok(())
    }
    pub(super) fn reproduction(&self) -> Option<fingerprint::source::Reproduction> {
        match self.source {
            RegistrationSource::Copy(source) => Some(source.reproduction()),
            RegistrationSource::Directory(_) => None,
        }
    }
    pub(super) fn stage(&self) -> &Stage {
        self.record
    }
    pub(super) fn source(&self) -> &Path {
        match self.source {
            RegistrationSource::Copy(source) => source.path(),
            RegistrationSource::Directory(source) => source.path(),
        }
    }
    pub(super) fn container_identity(&self) -> io::Result<Option<Identity>> {
        match self.source {
            RegistrationSource::Copy(source) => {
                Ok(Some(Identity::from_metadata(&source.parent().metadata()?)))
            }
            RegistrationSource::Directory(_) => Ok(None),
        }
    }
    pub(super) fn saved_source(&self, record: &recovery::Record) -> io::Result<()> {
        let RegistrationSource::Copy(source) = self.source else {
            return Err(changed());
        };
        if source.path().file_name() != Some(std::ffi::OsStr::new("previous"))
            || source.path().parent() != Some(record.retained.stage.directory.as_path())
            || Identity::from_metadata(&source.parent().metadata()?)
                != record.retained.stage.identity
            || Identity::from_metadata(&source.entry().metadata()?)
                != record.retained.observed_identity()
        {
            return Err(changed());
        }
        let owned = record
            .retained
            .stage
            .ownership
            .as_ref()
            .ok_or_else(changed)?;
        owned.require_directory(source.parent())?;
        if owned.previous(source.entry())?.is_none() {
            if record.retained.physical_identity.is_some() {
                return Err(changed());
            }
            record
                .retained
                .replacement_facts
                .as_ref()
                .ok_or_else(changed)?
                .original_previous(source.entry())?;
        }
        Ok(())
    }
    pub(super) fn require(&self) -> io::Result<()> {
        match self.source {
            RegistrationSource::Copy(source) => source.require_current()?,
            RegistrationSource::Directory(source) => source.require_current()?,
        }
        let stage = self.record;
        if stage.replacement.is_some()
            || stage.entry_move.is_some()
            || stage.source_claim.is_some()
            || stage.directory_claim.is_some()
            || stage.sibling_payload
            || Identity::from_metadata(&self.directory.metadata()?) != stage.identity
            || Some(Identity::from_metadata(&self.parent.metadata()?)) != stage.parent_identity
            || self
                .payload
                .map(|file| file.metadata().map(|m| Identity::from_metadata(&m)))
                .transpose()?
                != stage.payload_identity
        {
            return Err(changed());
        }
        if let Some(owned) = &stage.ownership {
            owned.require_parent(self.parent)?;
            owned.require_directory(self.directory)?;
            if let Some(payload) = self.payload {
                // Unsupported leaf generations stay unknown, never legacy.
                if owned.payload_origins().is_some() {
                    owned.require_payload(payload)?;
                }
            }
        }
        Ok(())
    }
    pub(super) fn directory(
        source: &'a transfer::directory::Opened,
        record: &'a Stage,
        parent: &'a fs::File,
        directory: &'a fs::File,
        payload: &'a fs::File,
    ) -> Self {
        Self {
            record,
            source: RegistrationSource::Directory(source),
            parent,
            directory,
            payload: Some(payload),
        }
    }
}

impl Opened {
    /// Verify this actual creator's empty-only record without inventing a copy
    /// reader when Undo only needs to restore an already saved destination.
    pub(super) fn require_created(&self, record: &Stage) -> io::Result<()> {
        self.require_current()?;
        if record.directory != self.parent_path.join(&self.name)
            || record.identity != self.identity
            || record.parent_identity != Some(self.parent_identity)
            || completed_cleanup(record)? != *record
            || record.ownership.as_deref()
                != Ownership::capture(&self.parent, &self.directory).as_ref()
        {
            return Err(changed());
        }
        Ok(())
    }
    pub(super) fn registration<'a>(
        &'a self,
        source: &'a fingerprint::source::Opened,
        record: &'a Stage,
    ) -> io::Result<Registration<'a>> {
        self.require_current()?;
        Ok(Registration {
            record,
            source: RegistrationSource::Copy(source),
            parent: &self.parent,
            directory: &self.directory,
            payload: None,
        })
    }
    pub(super) fn prepare(parent_path: &Path) -> io::Result<(Self, Stage, tempfile::TempDir)> {
        let parent = fs::File::from(open(
            parent_path,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        Self::prepare_in(parent_path, &parent)
    }
    pub(super) fn prepare_in(
        parent_path: &Path,
        pinned: &fs::File,
    ) -> io::Result<(Self, Stage, tempfile::TempDir)> {
        let parent = pinned.try_clone()?;
        let parent_identity = Identity::from_metadata(&parent.metadata()?);
        let mut temporary = tempfile::Builder::new()
            .prefix(".canopa-files-stage-")
            .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))?;
        temporary.disable_cleanup(true);
        let name = temporary.path().file_name().unwrap().to_owned();
        let directory = fs::File::from(openat(
            &parent,
            &name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let identity = Identity::from_metadata(&directory.metadata()?);
        let stage = Stage {
            directory: parent_path.join(&name),
            identity,
            parent_identity: Some(parent_identity),
            payload_identity: None,
            parent_access: Some(StageParentAccess::Existing),
            sibling_payload: false,
            ownership: Ownership::capture(&parent, &directory).map(Box::new),
            replacement: None,
            entry_move: None,
            source_claim: None,
            directory_claim: None,
        };
        Ok((
            Self {
                parent,
                directory,
                parent_path: parent_path.to_owned(),
                name,
                parent_identity,
                identity,
            },
            stage,
            temporary,
        ))
    }

    pub(super) fn require_current(&self) -> io::Result<()> {
        if !fs::metadata(&self.parent_path).is_ok_and(|metadata| {
            metadata.is_dir() && Identity::from_metadata(&metadata) == self.parent_identity
        }) || Identity::from_metadata(&named(&self.parent, &self.name)?.metadata()?)
            != self.identity
        {
            return Err(changed());
        }
        Ok(())
    }

    pub(super) fn directory(&self) -> &fs::File {
        &self.directory
    }
    pub(super) fn sync(&self) -> io::Result<()> {
        self.directory.sync_all()?;
        fs::File::from(openat(
            &self.parent,
            c".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?)
        .sync_all()
    }
    pub(super) fn clean_empty(&self) -> io::Result<()> {
        let named = match named(&self.parent, &self.name) {
            Ok(named) => named,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        if Identity::from_metadata(&named.metadata()?) != self.identity {
            return Err(changed());
        }
        rustix::fs::unlinkat(&self.parent, &self.name, AtFlags::REMOVEDIR)?;
        self.sync()
    }

    pub(super) fn regular(&self) -> io::Result<fs::File> {
        self.require_current()?;
        Ok(fs::File::from(openat(
            &self.directory,
            c"folder",
            OFlags::RDWR
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK
                | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?))
    }

    pub(super) fn link(&self, target: &Path) -> io::Result<fs::File> {
        self.require_current()?;
        rustix::fs::symlinkat(target, &self.directory, c"folder")?;
        named(&self.directory, c"folder")
    }

    pub(super) fn payload(self, file: fs::File, record: &mut Stage) -> io::Result<Prepared> {
        let identity = Identity::from_metadata(&file.metadata()?);
        record.payload_identity = Some(identity);
        record.ownership = record
            .ownership
            .take()
            .map(|proof| Box::new((*proof).with_payload(&file)));
        let result = Prepared {
            opened: self,
            file,
            identity,
        };
        result.require_current()?;
        Ok(result)
    }
}

pub(super) struct Prepared {
    opened: Opened,
    file: fs::File,
    identity: Identity,
}
impl Prepared {
    pub(super) fn registration<'a>(
        &'a self,
        source: &'a fingerprint::source::Opened,
        record: &'a Stage,
    ) -> io::Result<Registration<'a>> {
        self.require_current()?;
        Ok(Registration {
            record,
            source: RegistrationSource::Copy(source),
            parent: &self.opened.parent,
            directory: &self.opened.directory,
            payload: Some(&self.file),
        })
    }
    pub(super) fn entry(&self) -> &fs::File {
        &self.file
    }
    pub(super) fn parent(&self) -> &fs::File {
        &self.opened.parent
    }
    pub(super) fn directory(&self) -> &fs::File {
        &self.opened.directory
    }
    pub(super) fn expected_publication(&self) -> Option<Published> {
        Original::capture(&self.opened.parent)
            .ok()
            .zip(Original::capture(&self.file).ok())
            .map(|(parent, entry)| Published { parent, entry })
    }
    pub(super) fn discard_owned(&self) -> io::Result<()> {
        if Identity::from_metadata(&named(&self.opened.directory, c"folder")?.metadata()?)
            != self.identity
        {
            return Err(changed());
        }
        rustix::fs::unlinkat(&self.opened.directory, c"folder", AtFlags::empty())?;
        self.opened.clean_empty()
    }
    pub(super) fn unpublish(&self, destination: &Path) -> io::Result<()> {
        match named(&self.opened.directory, c"folder") {
            Ok(file) => {
                return if Identity::from_metadata(&file.metadata()?) == self.identity {
                    Ok(())
                } else {
                    Err(changed())
                };
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let name = destination
            .file_name()
            .ok_or_else(|| invalid("A copied item needs its name."))?;
        if Identity::from_metadata(&named(&self.opened.parent, name)?.metadata()?) != self.identity
        {
            return Err(changed());
        }
        rustix::fs::renameat_with(
            &self.opened.parent,
            name,
            &self.opened.directory,
            c"folder",
            RenameFlags::NOREPLACE,
        )?;
        self.opened.sync()
    }
    pub(super) fn require_published(&self, path: &Path) -> io::Result<()> {
        if path.parent() != Some(self.opened.parent_path.as_path())
            || !fs::metadata(&self.opened.parent_path).is_ok_and(|metadata| {
                Identity::from_metadata(&metadata) == self.opened.parent_identity
            })
        {
            return Err(changed());
        }
        if Identity::from_metadata(
            &named(
                &self.opened.parent,
                path.file_name()
                    .ok_or_else(|| invalid("A published copy needs its name."))?,
            )?
            .metadata()?,
        ) != self.identity
        {
            return Err(changed());
        }
        Ok(())
    }
    pub(super) fn published(&self, path: &Path) -> io::Result<Option<Published>> {
        self.require_published(path)?;
        Ok(self.expected_publication())
    }
    pub(super) fn fingerprint(&self, cancellation: &Cancellation) -> io::Result<Fingerprint> {
        Fingerprint::read_opened(&self.file, cancellation)
    }
    pub(super) fn restore_metadata(&self, fingerprint: &Fingerprint) -> io::Result<()> {
        self.require_current()?;
        #[cfg(test)]
        transfer::saved_cross_boundary::before_metadata(
            &self
                .opened
                .parent_path
                .join(&self.opened.name)
                .join("folder"),
        );
        fingerprint.restore_opened(&self.file)?;
        self.require_current()
    }
    pub(super) fn file(&mut self) -> &mut fs::File {
        &mut self.file
    }
    pub(super) fn metadata(&self) -> io::Result<fs::Metadata> {
        self.file.metadata()
    }
    pub(super) fn require_current(&self) -> io::Result<()> {
        self.opened
            .require_current()
            .and_then(|()| {
                if Identity::from_metadata(&named(&self.opened.directory, c"folder")?.metadata()?)
                    == self.identity
                {
                    Ok(())
                } else {
                    Err(changed())
                }
            })
            .map_err(protect)
    }
    pub(super) fn publish(&self, destination: &Path) -> io::Result<()> {
        let publish = || -> io::Result<()> {
            if destination.parent() != Some(self.opened.parent_path.as_path()) {
                return Err(invalid(
                    "A prepared payload must use its opened destination folder.",
                ));
            }
            self.require_current()?;
            #[cfg(test)]
            publication_boundary::at_phase(
                "before",
                destination,
                &self
                    .opened
                    .parent_path
                    .join(&self.opened.name)
                    .join("folder"),
            );
            rustix::fs::renameat_with(
                &self.opened.directory,
                c"folder",
                &self.opened.parent,
                destination
                    .file_name()
                    .ok_or_else(|| invalid("A copied item needs its name."))?,
                RenameFlags::NOREPLACE,
            )?;
            if Identity::from_metadata(
                &named(&self.opened.parent, destination.file_name().unwrap())?.metadata()?,
            ) != self.identity
            {
                return Err(changed());
            }
            self.opened.require_current()?;
            #[cfg(test)]
            publication_boundary::at_phase(
                "after",
                destination,
                &self
                    .opened
                    .parent_path
                    .join(&self.opened.name)
                    .join("folder"),
            );
            Ok(())
        };
        publish().map_err(protect)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Published {
    parent: Original,
    entry: Original,
}
impl Published {
    pub(super) fn require(&self, path: &Path) -> io::Result<fs::File> {
        let parent = fs::File::from(open(
            path.parent()
                .ok_or_else(|| invalid("A published copy needs its parent."))?,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        self.parent.require(&parent)?;
        let entry = named(
            &parent,
            path.file_name()
                .ok_or_else(|| invalid("A published copy needs its name."))?,
        )?;
        self.entry.require(&entry)?;
        Ok(entry)
    }
}

pub(super) trait VerifiedCopy {
    fn published(&self, path: &Path) -> io::Result<Option<Published>>;
    fn metadata(&self) -> io::Result<fs::Metadata>;
    fn require_published(&self, path: &Path) -> io::Result<()>;
    fn fingerprint(&self, cancellation: &Cancellation) -> io::Result<Fingerprint>;
}
impl VerifiedCopy for Prepared {
    fn published(&self, path: &Path) -> io::Result<Option<Published>> {
        Prepared::published(self, path)
    }
    fn metadata(&self) -> io::Result<fs::Metadata> {
        Prepared::metadata(self)
    }
    fn require_published(&self, path: &Path) -> io::Result<()> {
        Prepared::require_published(self, path)
    }
    fn fingerprint(&self, cancellation: &Cancellation) -> io::Result<Fingerprint> {
        Prepared::fingerprint(self, cancellation)
    }
}
pub(super) struct ReopenedCopy {
    file: fs::File,
    proof: Published,
}
impl Published {
    pub(super) fn reopen(&self, path: &Path) -> io::Result<ReopenedCopy> {
        Ok(ReopenedCopy {
            file: self.require(path)?,
            proof: self.clone(),
        })
    }
}
impl VerifiedCopy for ReopenedCopy {
    fn published(&self, path: &Path) -> io::Result<Option<Published>> {
        self.require_published(path)?;
        Ok(Some(self.proof.clone()))
    }
    fn metadata(&self) -> io::Result<fs::Metadata> {
        self.file.metadata()
    }
    fn require_published(&self, path: &Path) -> io::Result<()> {
        self.proof.entry.require(&self.file)?;
        self.proof.require(path)?;
        Ok(())
    }
    fn fingerprint(&self, cancellation: &Cancellation) -> io::Result<Fingerprint> {
        Fingerprint::read_opened(&self.file, cancellation)
    }
}

fn named(parent: &fs::File, name: impl rustix::path::Arg) -> io::Result<fs::File> {
    Ok(fs::File::from(openat(
        parent,
        name,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?))
}
fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The prepared copy or its destination folder changed; existing contents are preserved.",
    )
}
fn protect(error: io::Error) -> io::Error {
    transfer::destination_failure(error)
}

pub(super) fn clean(record: &Stage, ownership: &Ownership) -> io::Result<()> {
    let parent = fs::File::from(open(
        record
            .directory
            .parent()
            .ok_or_else(|| invalid("A stage needs its parent."))?,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    ownership.parent.require(&parent)?;
    let name = record
        .directory
        .file_name()
        .ok_or_else(|| invalid("A stage needs its name."))?;
    let directory = match openat(
        &parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(directory) => fs::File::from(directory),
        Err(rustix::io::Errno::NOENT) => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    ownership.directory.require(&directory)?;
    match named(&directory, c"folder") {
        Ok(payload) => {
            ownership
                .payload
                .as_ref()
                .ok_or_else(|| invalid("An unrecorded staged payload is preserved for review."))?
                .require(&payload)?;
            let flags = if payload.metadata()?.is_dir() {
                AtFlags::REMOVEDIR
            } else {
                AtFlags::empty()
            };
            rustix::fs::unlinkat(&directory, c"folder", flags)?;
            #[cfg(test)]
            transfer::stage_boundary::disposed(record, &payload);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    match named(&directory, c"previous") {
        Ok(_) => {
            directory.sync_all()?;
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    // Only an empty, proven directory is retired. Unknown children stay intact.
    ownership.directory.require(&named(&parent, name)?)?;
    rustix::fs::unlinkat(&parent, name, AtFlags::REMOVEDIR)?;
    fs::File::from(openat(
        &parent,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
    .sync_all()
}

/// Once its change is committed, a work directory owns no disposable payload.
/// The published object may have been moved back here by someone else.
pub(super) fn completed_cleanup(stage: &Stage) -> io::Result<Stage> {
    if stage.directory_claim.is_some()
        || stage.replacement.is_some()
        || stage.entry_move.is_some()
        || stage.source_claim.is_some()
        || stage.sibling_payload
    {
        return Err(invalid(
            "An unfinished file claim cannot become completed cleanup.",
        ));
    }
    let mut stage = stage.clone();
    stage.payload_identity = None;
    stage.ownership = stage
        .ownership
        .map(|proof| Box::new((*proof).without_payload()));
    Ok(stage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_publication_refuses_new_parent_stage_and_payload_bindings() {
        for replaced in ["parent", "stage", "payload"] {
            let root = tempfile::tempdir().unwrap();
            let parent = root.path().join("destination");
            fs::create_dir(&parent).unwrap();
            let source = root.path().join("source");
            fs::write(&source, b"Original source").unwrap();
            let destination = parent.join("copy");
            let history = root.path().join("history");
            let engine = OperationEngine::open(&history).unwrap();
            let mut state = engine.state.lock().unwrap();
            let id = state.begin("publication boundary").unwrap();
            let prepared = prepared_copy::prepare_copy(
                &source,
                &parent,
                &Cancellation::default(),
                |created| state.register_preparing(id, &destination, created),
                |_| {},
                || Ok(()),
            )
            .unwrap();
            let payload = prepared.stage.payload();
            match replaced {
                "parent" => {
                    fs::rename(&parent, root.path().join("original-parent")).unwrap();
                    fs::create_dir(&parent).unwrap();
                    fs::create_dir(&prepared.stage.directory).unwrap();
                }
                "stage" => {
                    fs::rename(
                        &prepared.stage.directory,
                        root.path().join("original-stage"),
                    )
                    .unwrap();
                    fs::create_dir(&prepared.stage.directory).unwrap();
                }
                _ => fs::rename(&payload, root.path().join("original-payload")).unwrap(),
            }
            fs::write(&payload, b"Foreign staged bytes").unwrap();
            let change = Change::Copied {
                source: None,
                publication: None,
                access_parent: None,
                path: destination.clone(),
                identity: prepared.identity,
                fingerprint: prepared.fingerprint,
                retained: None,
            };
            let error = commit_forward(
                &mut state,
                id,
                change,
                Some((prepared.stage, prepared.temporary)),
                |_| prepared.opened.publish(&destination),
            )
            .unwrap_err();
            assert_eq!(
                transfer::placement_failure(&error),
                Some(transfer::TransferAccess::DestinationFolder)
            );
            assert!(matches!(
                state.pending().unwrap(),
                Some(Pending::Forward { .. })
            ));
            assert!(!destination.exists());
            drop(prepared.opened);
            drop(state);
            drop(engine);
            let reopened = OperationEngine::open(&history).unwrap();
            assert!(
                !reopened
                    .interrupted_operations(&Cancellation::default())
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(fs::read(payload).unwrap(), b"Foreign staged bytes");
            assert_eq!(fs::read(&source).unwrap(), b"Original source");
            assert!(!destination.exists());
        }
    }

    #[test]
    fn unavailable_leaf_proof_keeps_the_recorded_stage_roots() {
        let root = tempfile::tempdir().unwrap();
        let (_opened, record, _temporary) = Opened::prepare(root.path()).unwrap();
        let ownership = record
            .ownership
            .expect("private filesystem supports original stage facts");
        let file = fs::File::open("/proc/self/status").unwrap();
        assert!(Original::capture(&file).is_err());
        let updated = ownership.clone().with_payload(&file);
        assert_eq!(updated.parent, ownership.parent);
        assert_eq!(updated.directory, ownership.directory);
        assert!(updated.payload.is_none());
    }

    #[test]
    fn a_payload_created_before_its_registration_is_preserved_after_reopen() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("destination");
        let source = root.path().join("source");
        fs::write(&source, b"Original source").unwrap();
        let history = root.path().join("history");
        let engine = OperationEngine::open(&history).unwrap();
        let mut state = engine.state.lock().unwrap();
        let id = state.begin("copy registration boundary").unwrap();
        let mut registered = None;
        let result = prepared_copy::prepare_copy(
            &source,
            root.path(),
            &Cancellation::default(),
            |created| {
                let stage = created.stage();
                if stage.payload_identity.is_some() {
                    return Err(io::Error::other("payload registration refused"));
                }
                assert!(stage.ownership.is_some());
                state.register_preparing(id, &destination, created)?;
                registered = Some(stage.clone());
                Ok(())
            },
            |_| {},
            || Ok(()),
        );
        assert!(result.is_err());
        let registered = registered.unwrap();
        assert_eq!(fs::metadata(registered.payload()).unwrap().len(), 0);
        drop(state);
        drop(engine);
        for _ in 0..2 {
            let reopened = OperationEngine::open(&history).unwrap();
            assert!(
                !reopened
                    .interrupted_operations(&Cancellation::default())
                    .unwrap()
                    .is_empty()
            );
            assert!(registered.payload().is_file());
            assert_eq!(fs::read(&source).unwrap(), b"Original source");
            assert!(!destination.exists());
        }
    }
}
