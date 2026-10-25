//! The original saved entry has one return claim, independent of copy disposal.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Saved {
    pub stage: Stage,
    entry: original::Original,
    version: fingerprint::Version,
}
pub(super) struct Opened {
    pub root: Root,
    entry: fs::File,
}
impl Opened {
    pub fn capture(retained: &Retained, reviewed: bool) -> io::Result<(Self, Box<Saved>)> {
        require_backup(retained)?;
        let stage = staging::completed_cleanup(&retained.stage)?;
        let root = Root::stage(&stage)?;
        let entry = fs::File::from(openat(
            &root.directory,
            c"previous",
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        if Identity::from_metadata(&entry.metadata()?) != retained.observed_identity() {
            return Err(attention());
        }
        let owner = stage.ownership.as_ref().ok_or_else(attention)?;
        let origin = if reviewed {
            // Only a fresh live saved-version approval may select a different
            // entry in the same original container. Restart uses this recorded
            // generation; it never recaptures a current path as approval.
            if retained.version.is_none() {
                return Err(attention());
            }
            original::Original::capture(&entry)?
        } else {
            match owner.previous(&entry)? {
                Some(origin) => origin,
                None if retained.physical_identity.is_none() => retained
                    .replacement_facts
                    .as_ref()
                    .ok_or_else(attention)?
                    .original_previous(&entry)?,
                None => return Err(attention()),
            }
        };
        let version = fingerprint::Version::from_metadata(&entry.metadata()?);
        if retained.version.as_ref().is_some_and(|v| *v != version) {
            return Err(attention());
        }
        Ok((
            Self { root, entry },
            Box::new(Saved {
                stage,
                entry: origin,
                version,
            }),
        ))
    }
    pub fn require(&self, saved: &Saved, removal: &Removal) -> io::Result<()> {
        saved.validate(removal)?;
        let fresh = Root::stage(&saved.stage)?;
        let owner = saved.stage.ownership.as_ref().ok_or_else(attention)?;
        owner.require_parent(&self.root.parent)?;
        owner.require_directory(&self.root.directory)?;
        saved.entry.require(&self.entry)?;
        let named = fs::File::from(openat(
            &fresh.directory,
            c"previous",
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        saved.entry.require(&named)?;
        if !saved.version.matches_opened(&self.entry, false)?
            || !saved.version.matches_opened(&named, false)?
        {
            return Err(attention());
        }
        Ok(())
    }
}
impl Saved {
    pub fn require_original(&self, removal: &Removal) -> io::Result<()> {
        self.open(removal).map(|_| ())
    }
    pub fn validate(&self, removal: &Removal) -> io::Result<()> {
        let retained = removal.change.retained().ok_or_else(attention)?;
        if self.stage != staging::completed_cleanup(&retained.stage)?
            || self.stage.ownership.is_none()
            || self.stage.directory == removal.stage.directory
            || self.stage.identity == removal.stage.identity
            || retained
                .version
                .as_ref()
                .is_some_and(|v| *v != self.version)
        {
            return Err(attention());
        }
        Ok(())
    }
    fn open(&self, removal: &Removal) -> io::Result<Opened> {
        let root = Root::stage(&self.stage)?;
        let entry = fs::File::from(openat(
            &root.directory,
            c"previous",
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let opened = Opened { root, entry };
        opened.require(self, removal)?;
        Ok(opened)
    }
}
pub(super) fn require_start(removal: &Removal, root: &Root) -> io::Result<()> {
    let saved = removal.previous.as_ref().ok_or_else(attention)?;
    let previous = saved.open(removal)?;
    if removal.previous_returned || !root.source_absent(removal)? {
        return Err(attention());
    }
    match removal.phase {
        Phase::Prepared if removal.selected.is_none() && root.payload()?.is_none() => {}
        Phase::Detached if removal.selected.is_some() => {
            require_payload_snapshot(root, removal, &root.payload()?.ok_or_else(attention)?)?;
        }
        _ => return Err(attention()),
    }
    previous.require(saved, removal)
}
pub(super) fn returned(removal: &Removal, root: &Root) -> io::Result<bool> {
    let saved = removal.previous.as_ref().ok_or_else(attention)?;
    let file = match openat(
        &root.parent,
        removal.change.forward_path().file_name().unwrap(),
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(file) => fs::File::from(file),
        Err(rustix::io::Errno::NOENT) => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    Ok(saved.entry.require(&file).is_ok())
}
pub(super) fn restore(
    state: &mut Store,
    removal: &mut Removal,
    root: &Root,
    cancellation: &Cancellation,
) -> io::Result<()> {
    if removal.selected.is_some() {
        require_payload(
            root,
            removal,
            &root.payload()?.ok_or_else(attention)?,
            cancellation,
        )?;
    }
    let saved = removal.previous.as_ref().ok_or_else(attention)?.clone();
    let opened = saved.open(removal)?;
    advance(state, removal, Phase::Restoring, root, None)?;
    #[cfg(test)]
    abort_boundary::saved_copy_boundary::cut("restoring", &removal.change, &removal.stage);
    opened.require(&saved, removal)?;
    root.require(removal)?;
    if !root.source_absent(removal)? {
        return Err(attention());
    }
    renameat_with(
        &opened.root.directory,
        c"previous",
        &root.parent,
        removal.change.forward_path().file_name().unwrap(),
        RenameFlags::NOREPLACE,
    )?;
    opened.root.directory.sync_all()?;
    root.sync()?;
    #[cfg(test)]
    abort_boundary::saved_copy_boundary::cut("previous-renamed", &removal.change, &removal.stage);
    advance(state, removal, Phase::Restored, root, None)?;
    #[cfg(test)]
    abort_boundary::saved_copy_boundary::cut("restored", &removal.change, &removal.stage);
    Ok(())
}
