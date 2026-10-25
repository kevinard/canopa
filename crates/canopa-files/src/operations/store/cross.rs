//! A cross-filesystem source-removal step immediately follows its destination
//! copy. Link only that stable owner key; never search path-wide aliases.
use super::*;

pub(super) fn paired(
    connection: &Connection,
    id: u64,
    ordinal: u64,
) -> io::Result<Option<(u64, Change)>> {
    let Change::SourceRemoved {
        current,
        copy_identity,
        ..
    } = stored_change(connection, id, ordinal)?
    else {
        return Err(invalid("expected a cross-filesystem source return"));
    };
    let Some(copy_ordinal) = ordinal.checked_sub(1) else {
        return Ok(None);
    };
    let body: Option<String> = connection
        .query_row(
            "SELECT body FROM changes WHERE operation=?1 AND ordinal=?2",
            params![id, copy_ordinal],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let Some(body) = body else { return Ok(None) };
    let change = decode(&body)?;
    if let Change::Copied { path, identity, .. } = &change
        && paths::key(path) == paths::key(&current)
        && *identity == copy_identity
    {
        Ok(Some((copy_ordinal, change)))
    } else {
        Ok(None)
    }
}

pub(super) fn returned(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    let Change::SourceRemoved {
        current,
        copy_identity,
        fingerprint,
        reviewed,
        ..
    } = change
    else {
        return Ok(());
    };
    let expected = reviewed.as_ref().map_or_else(
        || references::resolve(connection, id, ordinal, current, *copy_identity),
        |version| Ok(version.identity),
    )?;
    if let Some((copy_ordinal, mut copied)) = paired(connection, id, ordinal)? {
        let Change::Copied {
            fingerprint: cleanup_fingerprint,
            ..
        } = &mut copied
        else {
            unreachable!()
        };
        *cleanup_fingerprint = fingerprint.clone();
        connection
            .execute(
                "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                params![id, copy_ordinal, encode(&copied)?],
            )
            .map_err(db_error)?;
        references::expect_primary(connection, id, copy_ordinal, expected)?;
    }
    Ok(())
}

/// A missing incoming copy can restore its saved predecessor without returning
/// the source. Use the copy's destination owners and consume both rows only once
/// that filesystem restoration has succeeded, including during restart replay.
pub(super) fn saved_only(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<Option<(u64, Retained)>> {
    let Change::RetainedOnly {
        path,
        retained,
        replaced,
    } = change
    else {
        return Ok(None);
    };
    if !matches!(
        stored_change(connection, id, ordinal)?,
        Change::SourceRemoved { .. }
    ) {
        return Ok(None);
    }
    let Some((
        copy_ordinal,
        Change::Copied {
            path: copied_path,
            identity,
            retained: Some(saved),
            ..
        },
    )) = paired(connection, id, ordinal)?
    else {
        return Err(invalid("the source return has no paired saved destination"));
    };
    if paths::key(path) != paths::key(&copied_path)
        || *replaced != Some(identity)
        || retained.stage.identity != saved.stage.identity
        || paths::key(&retained.stage.directory) != paths::key(&saved.stage.directory)
        || retained.identity != saved.identity
    {
        return Err(invalid(
            "the saved-only return does not match its paired destination",
        ));
    }
    Ok(Some((copy_ordinal, saved)))
}

pub(super) fn restored_only(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<bool> {
    let Some((copy_ordinal, _)) = saved_only(connection, id, ordinal, change)? else {
        return Ok(false);
    };
    context::restored_destination(connection, id, copy_ordinal, change)?;
    connection
        .execute(
            "DELETE FROM changes WHERE operation=?1 AND ordinal=?2",
            params![id, copy_ordinal],
        )
        .map_err(db_error)?;
    Ok(true)
}
