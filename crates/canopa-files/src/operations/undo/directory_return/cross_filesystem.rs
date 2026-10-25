//! A Merge leaf uses the same verified return and cleanup as a cross-device move.
use super::*;

pub(in crate::operations::undo) enum Preparation {
    Unchanged,
    Expanded,
    Returned,
}

pub(in crate::operations::undo) fn prepare(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    cancellation: &Cancellation,
    relocation: Option<&Relocation>,
) -> io::Result<Preparation> {
    let Change::Rename {
        original,
        current,
        identity,
        retained: None,
        reviewed,
        saved,
        ..
    } = change
    else {
        return Ok(Preparation::Unchanged);
    };
    if saved.is_none() && state.directory_roots(id, ordinal)?.is_empty() {
        return Ok(Preparation::Unchanged);
    }
    if saved.is_some() {
        saved_stage::require_return_parent(state, id, ordinal, change)?;
    }
    let metadata = fs::symlink_metadata(current)?;
    if metadata.dev() == fs::metadata(original.parent().unwrap())?.dev() {
        return Ok(Preparation::Unchanged);
    }
    let expected =
        changed_move::identity(state, id, ordinal, current, *identity, reviewed.as_ref())?;
    let observed = fingerprint::Version::read(current)?;
    if observed.identity != expected {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the Merge child changed before its return was prepared",
        ));
    }
    changed_move::require_reviewed(change)?;
    if metadata.is_dir() {
        // Occupants need the ordinary conflict review before choosing a fresh
        // return name, including an owned saved-data container. Never attempt
        // to migrate into or merge the occupant before that decision.
        match fs::symlink_metadata(original) {
            Ok(_) => return Ok(Preparation::Unchanged),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if let Some(record) = state.owned_saved_stage(current, expected)? {
            saved_stage::prepare(
                state,
                id,
                ordinal,
                change,
                &record,
                cancellation,
                relocation,
            )?;
            return Ok(Preparation::Returned);
        }
        return prepare_destination(
            state,
            id,
            ordinal,
            change,
            &observed,
            cancellation,
            relocation,
        )
        .map(|()| Preparation::Expanded);
    }
    let fingerprint = Fingerprint::read(current, cancellation)?;
    state.require_directory_parents(id, ordinal)?;
    if !observed.matches(current) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the Merge child changed while its return was prepared",
        ));
    }
    // This is the parent authority used by the existing source-return step.
    // Its original quarantine is already empty: no original-location data was
    // removed by this derived step. Do not leave an unneeded staging directory.
    let (quarantine, mut temporary) = Stage::prepare(original.parent().unwrap())?;
    temporary.disable_cleanup(true);
    clean_completed_stage(&quarantine)?;
    check_cancel(cancellation)?;
    let cleanup = Change::Copied {
        source: None,
        publication: None,
        access_parent: None,
        path: current.clone(),
        identity: expected,
        fingerprint: fingerprint.clone(),
        retained: None,
    };
    let returned = Change::SourceRemoved {
        // This derived return did not remove original-location data.
        originals: None,
        original: original.clone(),
        current: current.clone(),
        identity: *identity,
        copy_identity: expected,
        fingerprint: fingerprint.clone(),
        source_fingerprint: fingerprint,
        quarantine,
        reviewed: reviewed.clone(),
    };
    state.expand_directory_file(id, ordinal, &cleanup, &returned)?;
    Ok(Preparation::Expanded)
}

fn prepare_destination(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    change: &Change,
    observed: &fingerprint::Version,
    cancellation: &Cancellation,
    relocation: Option<&Relocation>,
) -> io::Result<()> {
    let Change::Rename {
        original, current, ..
    } = change
    else {
        return Err(invalid("expected a directory member"));
    };
    let (mut stage, mut temporary) = Stage::prepare(original.parent().unwrap())?;
    fs::create_dir(stage.payload())?;
    use std::os::unix::fs::PermissionsExt;
    let destination_mode = fs::symlink_metadata(current)?.permissions().mode() & 0o7777;
    fs::set_permissions(
        stage.payload(),
        fs::Permissions::from_mode(destination_mode | 0o700),
    )?;
    stage.payload_identity = Some(Identity::read(&stage.payload())?);
    let mut source = change.clone();
    if let Change::Rename {
        identity, reviewed, ..
    } = &mut source
    {
        *identity = observed.identity;
        *reviewed = None;
    }
    let marker = Change::DirectoryMerge {
        destination_mode: Some(destination_mode),
        change: Box::new(source),
        destination_identity: stage.payload_identity.unwrap(),
        destination_original: super::root_original(
            &stage.payload(),
            stage.payload_identity.unwrap(),
        )?,
        defer_previous: false,
        destination_owned: false,
    };
    let mut entries: Vec<_> = fs::read_dir(current)?.collect::<io::Result<_>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    let children = entries
        .into_iter()
        .map(|entry| {
            check_cancel(cancellation)?;
            Ok(Change::Rename {
                move_facts: None,
                saved: None,
                original: original.join(entry.file_name()),
                current: entry.path(),
                identity: Identity::read(&entry.path())?,
                retained: None,
                reviewed: None,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    state.require_directory_parents(id, ordinal)?;
    if !observed.matches(current) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the Merge folder changed while preparing its destination",
        ));
    }
    check_cancel(cancellation)?;
    fs::File::open(stage.payload())?.sync_all()?;
    fs::File::open(&stage.directory)?.sync_all()?;
    temporary.disable_cleanup(true);
    state.prepare_directory_destination(id, ordinal, &marker, &children, &stage, relocation)?;
    recover_destination(state, id, ordinal, &marker, &stage, relocation)
}

pub(in crate::operations) fn recover_destination(
    state: &mut Store,
    id: u64,
    ordinal: u64,
    marker: &Change,
    stage: &Stage,
    relocation: Option<&Relocation>,
) -> io::Result<()> {
    validate_change(marker)?;
    let (source, destination, source_identity, destination_identity) = roots(marker)?;
    validate_stage(Some(stage), destination)?;
    if stage.payload_identity != Some(destination_identity) {
        return Err(invalid("invalid new Merge destination identity"));
    }
    state.require_directory_parents(id, ordinal)?;
    require_directory(source, source_identity)?;
    if !destination_identity.matches(destination) {
        if fs::symlink_metadata(destination).is_ok() {
            // A later occupant wins. Preserve it and retry the original member
            // through the normal conflict review; only our empty stage is removed.
            clean_completed_payload(stage)?;
            state.cancel_directory_destination(id, ordinal, marker, relocation)?;
            return Ok(());
        }
        if !stage.identity.matches(&stage.directory)
            || !destination_identity.matches(&stage.payload())
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "the prepared Merge destination is unavailable",
            ));
        }
        if !stage.parent_identity.is_some_and(|expected| {
            fs::metadata(destination.parent().unwrap())
                .is_ok_and(|metadata| Identity::from_metadata(&metadata) == expected)
        }) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "the folder receiving this Merge destination has changed",
            ));
        }
        rename_no_replace(&stage.payload(), destination)?;
        sync_parent(destination)?;
    }
    if let Change::DirectoryMerge {
        destination_mode: Some(mode),
        ..
    } = marker
    {
        restore_directory_mode(destination, destination_identity, *mode, None)?;
    }
    require_roots(marker)?;
    state.clear_intent()?;
    clean_completed_stage(stage)
}

fn clean_completed_payload(stage: &Stage) -> io::Result<()> {
    if !stage.identity.matches(&stage.directory)
        || !stage
            .payload_identity
            .is_some_and(|identity| identity.matches(&stage.payload()))
    {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the prepared Merge folder changed; its data is preserved",
        ));
    }
    fs::remove_dir(stage.payload())?;
    clean_completed_stage(stage)
}
