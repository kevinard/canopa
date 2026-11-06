//! Advance the existing Undo binding; disposal cannot invent a new Undo owner.
use super::*;

pub(in crate::operations::store) fn transition(
    connection: &Connection,
    history: Identity,
    transition: &staging::disposal::Transition,
) -> io::Result<()> {
    let current = pending(connection)?
        .ok_or_else(|| invalid("The original Undo disposal journal is missing."))?;
    if current != encode(transition.before())? {
        return Err(invalid("The original Undo disposal journal changed."));
    }
    require(connection, history, transition.before())?;
    transition.require()?;
    let before = origin(transition.before())?;
    let after = origin(transition.after())?;
    if encode(before)? != encode(after)? {
        return Err(invalid("Undo disposal changed its original return task."));
    }
    let old = body(connection)?
        .ok_or_else(|| invalid("The original Undo disposal binding is missing."))?;
    let mut binding: Binding = decode(&old)?;
    binding.pending = serialized::digest(encode(transition.after())?.as_bytes());
    let next = encode(&binding)?;
    if connection
        .execute(
            "UPDATE undo_work SET body=?1 WHERE singleton=1 AND body=?2",
            params![next, old],
        )
        .map_err(db_error)?
        != 1
        || body(connection)?.as_deref() != Some(next.as_str())
    {
        return Err(invalid("The Undo disposal binding was not saved."));
    }
    Ok(())
}

pub(in crate::operations::store) fn reproduce(
    state: &Store,
    payload: &fs::File,
    cancellation: &Cancellation,
) -> io::Result<Option<fingerprint::source::Reproduced>> {
    if !matches!(state.lane, IntentLane::Primary) {
        return Ok(None);
    }
    let Some(value) = state.pending()? else {
        return Ok(None);
    };
    if !matches!(value, Pending::Undo { stage: Some(_), .. })
        && !matches!(&value, Pending::Disposing { disposal } if disposal.undo.is_some())
    {
        return Ok(None);
    }
    require(&state.connection, state.identity, &value)?;
    let (_, _, change, _) = task(&value)?;
    let Change::SourceRemoved { current, .. } = change else {
        // Direct displacement has no copied payload and cannot acquire source
        // reproduction or unlink rights through this preparation binding.
        return Ok(None);
    };
    let binding: Binding = decode(
        &body(&state.connection)?
            .ok_or_else(|| invalid("The original Undo source binding is missing."))?,
    )?;
    binding
        .source
        .ok_or_else(|| invalid("The Undo partial has no original source proof."))?
        .reproduce(current, payload, cancellation)
        .map(Some)
        .map_err(fingerprint::source::reproduction_problem)
}
