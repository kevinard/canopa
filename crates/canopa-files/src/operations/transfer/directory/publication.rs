//! Reconcile the actual prepared folder; never infer publication from inode alone.
use super::*;

pub(in crate::operations) fn recover(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    stage: Option<&Stage>,
) -> io::Result<()> {
    let Change::CreatedFolder {
        path,
        identity,
        mode: Some(mode),
        ..
    } = change
    else {
        return Err(invalid(
            "A folder publication needs its exact creation intent.",
        ));
    };
    let stage = stage.ok_or_else(unproved)?;
    validate_stage(Some(stage), path)?;
    if stage.replacement.is_some()
        || stage.entry_move.is_some()
        || stage.source_claim.is_some()
        || stage.directory_claim.is_some()
        || stage.sibling_payload
        || stage.payload_identity != Some(*identity)
    {
        return Err(unproved());
    }
    let ownership = stage.ownership.as_ref().ok_or_else(unproved)?;
    let publication = ownership.publication().ok_or_else(unproved)?;
    state.require_directory_publication(&Pending::Forward {
        id,
        ordinal,
        change: change.clone(),
        stage: Some(stage.clone()),
    })?;
    let parent = directory(path.parent().unwrap())?;
    ownership.require_parent(&parent)?;
    if stage.parent_identity != Some(Identity::from_metadata(&parent.metadata()?)) {
        return Err(unproved());
    }
    let work = match rustix::fs::openat(
        &parent,
        stage.directory.file_name().unwrap(),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => {
            let file = fs::File::from(fd);
            ownership.require_directory(&file)?;
            if Identity::from_metadata(&file.metadata()?) != stage.identity {
                return Err(unproved());
            }
            Some(file)
        }
        Err(rustix::io::Errno::NOENT) => None,
        Err(error) => return Err(error.into()),
    };
    let payload = work
        .as_ref()
        .map(|work| optional(work, std::ffi::OsStr::new("folder")))
        .transpose()?
        .flatten();
    if let Some(payload) = &payload {
        require_payload(ownership, payload, *identity)?;
        if optional(&parent, path.file_name().unwrap())?.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Another item occupies this prepared folder's destination. Both versions remain preserved.",
            ));
        }
        let readable = fs::File::from(rustix::fs::openat(
            payload,
            c".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        readable.sync_all()?;
        ownership.require_parent(&directory(path.parent().unwrap())?)?;
        rustix::fs::renameat_with(
            work.as_ref().unwrap(),
            c"folder",
            &parent,
            path.file_name().unwrap(),
            RenameFlags::NOREPLACE,
        )?;
    }
    let output = optional(&parent, path.file_name().unwrap())?.ok_or_else(unproved)?;
    require_payload(ownership, &output, *identity)?;
    // Require the recorded parent/entry at the public name before any mode write.
    publication.require(path)?;
    let readable = fs::File::from(rustix::fs::openat(
        &output,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    let metadata = readable.metadata()?;
    if metadata.mode() & 0o7777 != *mode && metadata.mode() & 0o7777 == mode | 0o700 {
        readable.set_permissions(fs::Permissions::from_mode(*mode))?;
    }
    readable.sync_all()?;
    if let Some(work) = &work {
        work.sync_all()?;
    }
    ownership.require_parent(&directory(path.parent().unwrap())?)?;
    fs::File::from(rustix::fs::openat(
        &parent,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
    .sync_all()?;
    append_forward(state, id, ordinal, change, Some(stage))
}
fn optional(parent: &fs::File, name: &std::ffi::OsStr) -> io::Result<Option<fs::File>> {
    match rustix::fs::openat(
        parent,
        name,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => Ok(Some(fs::File::from(fd))),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
fn require_payload(
    ownership: &staging::Ownership,
    file: &fs::File,
    identity: Identity,
) -> io::Result<()> {
    ownership.require_payload(file)?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() || Identity::from_metadata(&metadata) != identity {
        return Err(unproved());
    }
    Ok(())
}
fn unproved() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The prepared folder's original objects cannot be verified. Its contents remain preserved for Recovery.",
    )
}
