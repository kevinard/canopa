//! Captured before the conflict question; recovery never adopts current roots.
use super::*;
use rustix::fs::{Mode, OFlags, RenameFlags, open, openat, renameat_with};

pub(in crate::operations) struct Observed {
    path: PathBuf,
    file: fs::File,
    parent: fs::File,
    parent_original: Option<original::Original>,
    version: fingerprint::Version,
}
pub(in crate::operations) struct Prepared {
    observed: Observed,
    opened: staging::Opened,
    stage: Stage,
    registered: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::operations) struct Review {
    root: fingerprint::Version,
    previous: Option<fingerprint::Version>,
    unmoved: Option<fingerprint::Version>,
}
fn attention() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The original displaced Undo item or work folder cannot be proved. Its files are preserved for Recovery.",
    )
}
impl Observed {
    pub fn open(path: &Path) -> io::Result<Self> {
        let parent = fs::File::from(open(
            path.parent().ok_or_else(attention)?,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let file = fs::File::from(openat(
            &parent,
            path.file_name().ok_or_else(attention)?,
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let parent_original = original::Original::capture(&parent).ok();
        Ok(Self {
            path: path.to_owned(),
            parent,
            parent_original,
            version: fingerprint::Version::from_metadata(&file.metadata()?),
            file,
        })
    }
    pub fn version(&self) -> &fingerprint::Version {
        &self.version
    }
    fn require(&self) -> io::Result<()> {
        let parent = fs::File::from(open(
            self.path.parent().ok_or_else(attention)?,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        if let Some(original) = self.parent_original {
            original.require(&self.parent)?;
            original.require(&parent)?;
        } else if Identity::from_metadata(&self.parent.metadata()?)
            != Identity::from_metadata(&parent.metadata()?)
        {
            return Err(attention());
        }
        if !self.version.matches_opened(&self.file, false)? || !self.version.matches(&self.path) {
            return Err(attention());
        }
        Ok(())
    }
    pub fn prepare(self) -> io::Result<Prepared> {
        self.require()?;
        self.parent_original
            .ok_or_else(attention)?
            .require(&self.parent)?;
        let previous = original::Original::capture(&self.file)?;
        let (opened, mut stage, temporary) =
            staging::Opened::prepare_in(self.path.parent().ok_or_else(attention)?, &self.parent)?;
        let _retained = temporary.keep();
        let Some(owned) = stage.ownership.as_ref() else {
            let _cleanup = opened.clean_empty();
            return Err(attention());
        };
        let (parent, directory) = owned.roots();
        stage.ownership = Some(Box::new(staging::Ownership::saved(
            parent, directory, previous,
        )));
        let prepared = Prepared {
            observed: self,
            opened,
            stage,
            registered: false,
        };
        prepared.observed.require()?;
        prepared.opened.sync()?;
        Ok(prepared)
    }
}
impl Prepared {
    pub fn for_preservation(reviewed: &replacement::Reviewed) -> io::Result<Self> {
        let (path, file) = reviewed.preservation_selection()?;
        let parent = fs::File::from(open(
            path.parent().ok_or_else(attention)?,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let parent_original = original::Original::capture(&parent).ok();
        let observed = Observed {
            path,
            file,
            parent,
            parent_original,
            version: reviewed.version().clone(),
        };
        let (opened, stage, temporary) = reviewed.prepare_preservation()?;
        let _retained = temporary.keep();
        let prepared = Self {
            observed,
            opened,
            stage,
            registered: false,
        };
        prepared.observed.require()?;
        Ok(prepared)
    }
    pub fn preserve_changed_copy(mut self, reviewed: &replacement::Reviewed) -> io::Result<()> {
        // Registration owns this root even if the reviewed rename now refuses.
        self.registered = true;
        reviewed.preserve(&self.opened)
    }
    pub fn stage(&self) -> &Stage {
        &self.stage
    }
    pub fn require(&self, record: &recovery::Record) -> io::Result<()> {
        if record.retained.stage != self.stage
            || record.context.as_ref().map(|c| &c.original) != Some(&self.observed.path)
            || record.retained.identity != self.observed.version.identity
            || record.retained.version.as_ref() != Some(&self.observed.version)
        {
            return Err(attention());
        }
        self.observed.require()?;
        self.opened.require_current()?;
        let owned = self.stage.ownership.as_ref().ok_or_else(attention)?;
        owned.require_parent(&self.observed.parent)?;
        owned.require_directory(self.opened.directory())?;
        if owned.previous(&self.observed.file)?.is_none() {
            return Err(attention());
        }
        Ok(())
    }
    pub fn keep(mut self) {
        self.registered = true;
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        if !self.registered {
            let _cleanup = self.opened.clean_empty();
        }
    }
}
pub(in crate::operations) fn validate(
    record: &recovery::Record,
    original: &Path,
) -> io::Result<()> {
    if record.payload
        || record.container
        || !record.retained.recovery
        || record.context.as_ref().map(|c| c.original.as_path()) != Some(original)
        || record
            .retained
            .stage
            .ownership
            .as_ref()
            .and_then(|owned| owned.previous_origin())
            .is_none()
    {
        return Err(attention());
    }
    validate_stage(Some(&record.retained.stage), original)
}
pub(in crate::operations) fn observe(record: &recovery::Record) -> io::Result<Review> {
    let stage = &record.retained.stage;
    let original = &record.context.as_ref().ok_or_else(attention)?.original;
    validate(record, original)?;
    let owned = stage.ownership.as_ref().ok_or_else(attention)?;
    let parent = fs::File::from(open(
        stage.directory.parent().ok_or_else(attention)?,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    owned.require_parent(&parent)?;
    let directory = fs::File::from(openat(
        &parent,
        stage.directory.file_name().ok_or_else(attention)?,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    owned.require_directory(&directory)?;
    let (previous, unmoved) = match openat(
        &directory,
        c"previous",
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(file) => {
            let file = fs::File::from(file);
            owned.previous(&file)?;
            (
                Some(fingerprint::Version::from_metadata(&file.metadata()?)),
                None,
            )
        }
        Err(rustix::io::Errno::NOENT) => {
            let file = fs::File::from(open(
                original,
                OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            owned.previous(&file)?;
            (
                None,
                Some(fingerprint::Version::from_metadata(&file.metadata()?)),
            )
        }
        Err(error) => return Err(error.into()),
    };
    Ok(Review {
        root: fingerprint::Version::from_metadata(&directory.metadata()?),
        previous,
        unmoved,
    })
}

/// A fresh bound preparation moves its selected entry through the actual parent
/// and created directory descriptors. Errors leave compensation to the durable
/// consumed-return protocol; never perform a raw rollback rename here.
pub(in crate::operations) fn install(
    destination: &Path,
    retained: &Retained,
    install: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let stage = &retained.stage;
    let owner = stage.ownership.as_ref().ok_or_else(attention)?;
    let original = owner.previous_origin().ok_or_else(attention)?;
    let parent = fs::File::from(open(
        stage.directory.parent().ok_or_else(attention)?,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    owner.require_parent(&parent)?;
    let root = fs::File::from(openat(
        &parent,
        stage.directory.file_name().ok_or_else(attention)?,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    owner.require_directory(&root)?;
    let target_parent = fs::File::from(open(
        destination.parent().ok_or_else(attention)?,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    owner.require_parent(&target_parent)?;
    let file = fs::File::from(openat(
        &target_parent,
        destination.file_name().ok_or_else(attention)?,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    original.require(&file)?;
    if !retained
        .version
        .as_ref()
        .ok_or_else(attention)?
        .matches_opened(&file, false)?
    {
        return Err(attention());
    }
    renameat_with(
        &target_parent,
        destination.file_name().ok_or_else(attention)?,
        &root,
        c"previous",
        RenameFlags::NOREPLACE,
    )?;
    let moved = fs::File::from(openat(
        &root,
        c"previous",
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    original.require(&moved)?;
    if !retained
        .version
        .as_ref()
        .unwrap()
        .matches_moved(&retained.path())
    {
        return Err(attention());
    }
    root.sync_all()?;
    fs::File::from(openat(
        &target_parent,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
    .sync_all()?;
    #[cfg(test)]
    abort_boundary::direct_displacement_boundary::cut("displaced");
    install()
}
