//! Finish only the prepared object recorded by this ordinary Copy step.
use super::*;

pub(in crate::operations) fn recover(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    stage: Option<&Stage>,
) -> io::Result<()> {
    let Change::Copied {
        path,
        identity,
        fingerprint,
        publication,
        retained: None,
        ..
    } = change
    else {
        return Err(invalid("Ordinary copy recovery needs its exact Copy step."));
    };
    let stage = stage.ok_or_else(unproved)?;
    validate_stage(Some(stage), path)?;
    if stage.replacement.is_some()
        || stage.entry_move.is_some()
        || stage.source_claim.is_some()
        || stage.sibling_payload
    {
        return Err(unproved());
    }
    let ownership = stage.ownership.as_ref().ok_or_else(unproved)?;
    let expected = ownership.publication().ok_or_else(unproved)?;
    if stage.payload_identity != Some(*identity)
        || publication
            .as_deref()
            .is_some_and(|recorded| recorded != &expected)
    {
        return Err(invalid(
            "The prepared copy differs from its original publication record.",
        ));
    }
    let parent = fs::File::from(open(
        path.parent().unwrap(),
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    ownership.require_parent(&parent)?;
    let parent_identity = Identity::from_metadata(&parent.metadata()?);
    if stage.parent_identity != Some(parent_identity) {
        return Err(unproved());
    }
    let name = stage.directory.file_name().unwrap();
    let directory = match openat(
        &parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(file) => {
            let file = fs::File::from(file);
            ownership.require_directory(&file)?;
            if Identity::from_metadata(&file.metadata()?) != stage.identity {
                return Err(unproved());
            }
            Some(file)
        }
        Err(rustix::io::Errno::NOENT) => None,
        Err(error) => return Err(error.into()),
    };
    let payload = directory
        .as_ref()
        .map(|directory| optional(directory, c"folder"))
        .transpose()?
        .flatten();
    if let Some(payload) = payload {
        require_payload(ownership, &payload, *identity)?;
        if Fingerprint::read_opened(&payload, &Cancellation::default())? != *fingerprint {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "The prepared copy has newer contents. Its files are preserved for Recovery.",
            ));
        }
        if optional(&parent, path.file_name().unwrap())?.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Another entry occupies this copy's destination. Both versions are preserved for Recovery.",
            ));
        }
        // These are newly opened pins verified against the original saved facts,
        // not a new observation of whichever file currently has this pathname.
        let prepared = Prepared {
            opened: Opened {
                parent: parent.try_clone()?,
                directory: directory.as_ref().unwrap().try_clone()?,
                parent_path: path.parent().unwrap().to_owned(),
                name: name.to_owned(),
                parent_identity,
                identity: stage.identity,
            },
            file: payload,
            identity: *identity,
        };
        prepared.publish(path)?;
        prepared.opened.sync()?;
    }
    let output = optional(&parent, path.file_name().unwrap())?.ok_or_else(unproved)?;
    require_payload(ownership, &output, *identity)?;
    // A completed publication is metadata only. Its later contents and modes
    // belong to the user and must not be restored from the old fingerprint.
    let current_parent = fs::File::from(open(
        path.parent().unwrap(),
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    ownership.require_parent(&current_parent)?;
    if let Some(directory) = &directory {
        directory.sync_all()?;
    }
    fs::File::from(openat(
        &parent,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
    .sync_all()?;
    let mut finished = change.clone();
    let Change::Copied { publication, .. } = &mut finished else {
        unreachable!();
    };
    // Older opened-copy intents already contain these exact original facts in
    // their stage. Retain those facts in the completed step before cleanup.
    *publication = Some(Box::new(expected));
    append_forward(state, id, ordinal, &finished, Some(stage))
}

fn require_payload(ownership: &Ownership, file: &fs::File, identity: Identity) -> io::Result<()> {
    ownership.require_payload(file)?;
    if Identity::from_metadata(&file.metadata()?) != identity {
        return Err(unproved());
    }
    Ok(())
}
fn optional(parent: &fs::File, name: impl rustix::path::Arg) -> io::Result<Option<fs::File>> {
    match named(parent, name) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
fn unproved() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "Files could not verify this copy's original file and destination. Its files are preserved for Recovery.",
    )
}
