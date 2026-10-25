//! A forward step captures which older references owned its physical source and
//! saved destination. Returning one version updates only those references.
use super::*;
use references::Owner;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ForwardProof {
    history: Identity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original: Option<original::Original>,
    digest: [u8; 32],
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Bindings {
    source: Vec<Owner>,
    destination: Vec<Owner>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    forward: Option<ForwardProof>,
}

fn source(change: &Change) -> Option<(&Path, Identity)> {
    match change {
        Change::Rename {
            original, identity, ..
        }
        | Change::SourceRemoved {
            original, identity, ..
        }
        | Change::Trashed {
            original, identity, ..
        }
        | Change::Restored {
            original, identity, ..
        } => Some((original, *identity)),
        Change::RemovedDirectory { path, identity, .. } => Some((path, *identity)),
        _ => None,
    }
}

pub(super) fn capture(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<Bindings> {
    let source = source(change)
        .map(|(path, identity)| references::owned(connection, path, identity, id, ordinal))
        .transpose()?
        .unwrap_or_default();
    let destination = destination(connection, id, ordinal, change)?;
    Ok(Bindings {
        source,
        destination,
        forward: None,
    })
}

fn destination(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<Vec<Owner>> {
    change
        .retained()
        .map(|retained| {
            references::owned(
                connection,
                change.forward_path(),
                retained.identity,
                id,
                ordinal,
            )
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn validate(bindings: &Bindings, id: u64, ordinal: u64) -> io::Result<()> {
    references::validate(&bindings.source, id, ordinal)?;
    references::validate(&bindings.destination, id, ordinal)
}

pub(super) fn prepare(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
) -> io::Result<()> {
    if let Pending::StageMigration { migration } = pending
        && migration.has_claim()
    {
        saved_stage::forward::validate(migration)?;
        if migration.catalogued {
            return Err(invalid(
                "A committed saved claim needs its original phase transition.",
            ));
        }
        return prepare(
            connection,
            history,
            lane,
            &saved_stage::forward::virtual_forward(migration),
        );
    }
    if let Pending::StageMigration { migration } = pending
        && migration.forward
    {
        return prepare(
            connection,
            history,
            lane,
            &Pending::Forward {
                id: migration.id,
                ordinal: migration.ordinal,
                change: migration.change.clone(),
                stage: None,
            },
        );
    }
    if let Pending::Forward {
        id,
        ordinal,
        change,
        ..
    } = pending
    {
        let mut bindings = capture(connection, *id, *ordinal, change)?;
        let original = transfer_completion::history_file(connection, history)
            .ok()
            .and_then(|file| original::Original::capture(&file).ok());
        bindings.forward = Some(ForwardProof {
            history,
            original,
            digest: forward_digest(history, lane, pending, original)?,
        });
        let body = encode(&bindings)?;
        connection.execute("INSERT INTO pending_bindings(lane,operation,ordinal,body) VALUES(?1,?2,?3,?4) ON CONFLICT(lane) DO UPDATE SET operation=excluded.operation,ordinal=excluded.ordinal,body=excluded.body", params![lane.index(), id, ordinal, body]).map_err(db_error)?;
    } else {
        clear(connection, lane)?;
    }
    Ok(())
}

fn forward_digest(
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
    original: Option<original::Original>,
) -> io::Result<[u8; 32]> {
    let mut hash = Sha256::new();
    if let Some(original) = original {
        hash.update(b"canopa-files-forward-v2\0");
        hash.update(encode(&original)?.as_bytes());
    } else {
        hash.update(b"canopa-files-forward-v1\0");
    }
    hash.update(encode(&history)?.as_bytes());
    hash.update([lane.index() as u8]);
    hash.update(encode(pending)?.as_bytes());
    Ok(hash.finalize().into())
}

fn require_history(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    id: u64,
    proof: &ForwardProof,
    required: bool,
) -> io::Result<()> {
    if let Some(original) = proof.original {
        return original.require(&transfer_completion::history_file(connection, history)?);
    }
    // Older job intents can use their independently captured original history;
    // an unproved direct intent cannot adopt the file currently at this path.
    if required && transfer_cursor::stage_task(connection, history, lane, id)?.is_none() {
        return Err(invalid(
            "The interrupted work has no original history-file generation proof.",
        ));
    }
    Ok(())
}
pub(super) fn history_original(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
) -> io::Result<Option<original::Original>> {
    let id = match pending {
        Pending::Forward { id, .. } => *id,
        _ => return Err(invalid("Expected the original Forward history.")),
    };
    let body: String = connection
        .query_row(
            "SELECT body FROM pending_bindings WHERE lane=?1 AND operation=?2",
            params![lane.index(), id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    let bindings: Bindings = decode(&body)?;
    if let Some(original) = bindings.forward.and_then(|proof| proof.original) {
        return Ok(Some(original));
    }
    Ok(transfer_cursor::stage_task(connection, history, lane, id)?
        .and_then(|task| task.history_original))
}

/// Check independently saved intent facts before preparing directory access or
/// moving any claimed object. Older unproved transfer claims remain in Recovery.
pub(super) fn require_forward(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
) -> io::Result<()> {
    if let Pending::StageMigration { migration } = pending
        && migration.has_claim()
    {
        return require_saved_migration(connection, history, lane, migration);
    }
    let Pending::Forward {
        id, ordinal, stage, ..
    } = pending
    else {
        return Ok(());
    };
    let required = stage.as_ref().is_some_and(|stage| {
        stage.directory_claim.is_some()
            || stage.ownership.is_some()
            || stage.replacement.is_some()
            || stage.entry_move.is_some()
            || stage.source_claim.is_some()
    });
    let prepared: Option<(u64, u64, String)> = connection
        .query_row(
            "SELECT operation,ordinal,body FROM pending_bindings WHERE lane=?1",
            [lane.index()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(db_error)?;
    let Some((operation, step, body)) = prepared else {
        return if required {
            Err(invalid(
                "Files could not verify which operation owns this interrupted work. Its files are preserved for Recovery.",
            ))
        } else {
            Ok(())
        };
    };
    let bindings: Bindings = decode(&body)?;
    let Some(proof) = &bindings.forward else {
        return if required {
            Err(invalid(
                "This interrupted transfer predates verified operation records. Its files are preserved for Recovery.",
            ))
        } else {
            Ok(())
        };
    };
    if operation != *id
        || step != *ordinal
        || proof.history != history
        || proof.digest != forward_digest(history, lane, pending, proof.original)?
    {
        return Err(invalid(
            "The interrupted work differs from its recorded operation. Its files are preserved for Recovery.",
        ));
    }
    validate(&bindings, *id, *ordinal)?;
    require_history(connection, history, lane, *id, proof, required)?;
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM operations WHERE id=?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    let next: u64 = connection
        .query_row(
            "SELECT COALESCE(MAX(ordinal)+1,0) FROM changes WHERE operation=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if !exists || next != *ordinal {
        return Err(invalid(
            "Recorded file work no longer matches this operation's completed steps. Its files are preserved for Recovery.",
        ));
    }
    Ok(())
}

pub(super) fn require_saved_migration(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    migration: &saved_stage::Migration,
) -> io::Result<()> {
    saved_stage::forward::validate(migration)?;
    if !matches!(lane, IntentLane::Primary) {
        return Err(invalid("A saved move belongs to the primary journal."));
    }
    let forward = saved_stage::forward::virtual_forward(migration);
    if !migration.catalogued {
        return require_forward(connection, history, lane, &forward);
    }
    let expected = saved_phase_bindings(connection, history, lane, migration)?;
    let actual: Option<(u64, u64, String)> = connection
        .query_row(
            "SELECT operation,ordinal,body FROM pending_bindings WHERE lane=?1",
            [lane.index()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(db_error)?;
    if actual != Some((migration.id, migration.ordinal, encode(&expected)?)) {
        return Err(invalid(
            "The saved cleanup lost its independently recorded phase.",
        ));
    }
    Ok(())
}

fn committed_saved_bindings(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    migration: &saved_stage::Migration,
) -> io::Result<Bindings> {
    let forward = saved_stage::forward::virtual_forward(migration);
    let row: Option<(String, String)> = connection
        .query_row(
            "SELECT body,bindings FROM changes WHERE operation=?1 AND ordinal=?2",
            params![migration.id, migration.ordinal],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(db_error)?;
    let Some((body, bindings)) = row else {
        return Err(invalid(
            "The saved cleanup has no committed namespace step.",
        ));
    };
    let bindings: Bindings = decode(&bindings)?;
    if body != encode(&migration.change)?
        || bindings
            .forward
            .as_ref()
            .is_none_or(|p| p.history != history)
        || bindings.forward.as_ref().map(|p| p.digest)
            != Some(forward_digest(
                history,
                lane,
                &forward,
                bindings.forward.as_ref().and_then(|p| p.original),
            )?)
        || connection
            .query_row(
                "SELECT coalesce(max(ordinal)+1,0) FROM changes WHERE operation=?1",
                [migration.id],
                |r| r.get::<_, u64>(0),
            )
            .map_err(db_error)?
            != migration
                .ordinal
                .checked_add(1)
                .ok_or_else(|| invalid("Saved ordinal overflow."))?
    {
        return Err(invalid(
            "The saved cleanup differs from its original committed context.",
        ));
    }
    validate(&bindings, migration.id, migration.ordinal)?;
    require_history(
        connection,
        history,
        lane,
        migration.id,
        bindings.forward.as_ref().unwrap(),
        true,
    )?;
    Ok(bindings)
}
fn saved_phase_bindings(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    migration: &saved_stage::Migration,
) -> io::Result<Bindings> {
    saved_stage::forward::validate(migration)?;
    let mut bindings = committed_saved_bindings(connection, history, lane, migration)?;
    let mut hash = Sha256::new();
    hash.update(b"canopa-files-saved-cleanup-phase-v1\0");
    hash.update(encode(&history)?.as_bytes());
    hash.update([lane.index() as u8]);
    hash.update(
        encode(&Pending::StageMigration {
            migration: Box::new(migration.clone()),
        })?
        .as_bytes(),
    );
    bindings.forward = Some(ForwardProof {
        history,
        original: bindings.forward.as_ref().and_then(|proof| proof.original),
        digest: hash.finalize().into(),
    });
    Ok(bindings)
}
pub(super) fn install_saved_phase(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    migration: &saved_stage::Migration,
) -> io::Result<()> {
    let bindings = saved_phase_bindings(connection, history, lane, migration)?;
    connection.execute("INSERT INTO pending_bindings(lane,operation,ordinal,body) VALUES(?1,?2,?3,?4) ON CONFLICT(lane) DO UPDATE SET operation=excluded.operation,ordinal=excluded.ordinal,body=excluded.body", params![lane.index(),migration.id,migration.ordinal,encode(&bindings)?]).map_err(db_error)?;
    Ok(())
}

pub(super) fn for_append(
    connection: &Connection,
    lane: IntentLane,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<String> {
    let prepared: Option<(u64, u64, String)> = connection
        .query_row(
            "SELECT operation,ordinal,body FROM pending_bindings WHERE lane=?1",
            [lane.index()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(db_error)?;
    match prepared {
        Some((operation, step, body)) if operation == id && step == ordinal => {
            validate(&decode(&body)?, id, ordinal)?;
            Ok(body)
        }
        Some(_) => Err(invalid(
            "forward identity context does not match its journal",
        )),
        // Older forward intents have no context. No Undo can have changed these
        // references while their filesystem mutation was pending.
        None => encode(&capture(connection, id, ordinal, change)?),
    }
}

fn read(connection: &Connection, id: u64, ordinal: u64) -> io::Result<Option<Bindings>> {
    let body: Option<String> = connection
        .query_row(
            "SELECT bindings FROM changes WHERE operation=?1 AND ordinal=?2",
            params![id, ordinal],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    body.map(|body| {
        let bindings = decode(&body)?;
        validate(&bindings, id, ordinal)?;
        Ok(bindings)
    })
    .transpose()
}

pub(super) fn require_change_history(
    connection: &Connection,
    history: Identity,
    id: u64,
    ordinal: u64,
) -> io::Result<()> {
    let proof=read(connection,id,ordinal)?.and_then(|b|b.forward).ok_or_else(||invalid("This legacy Copy Undo has no original history-file proof. Its work stays protected in Recovery."))?;
    if proof.history != history {
        return Err(invalid(
            "The legacy Copy Undo belongs to a different history file.",
        ));
    }
    proof
        .original
        .ok_or_else(|| invalid("The original legacy history generation is unavailable."))?
        .require(&transfer_completion::history_file(connection, history)?)
}

pub(super) fn source_owned_by(
    connection: &Connection,
    step: recovery::ContextStep,
    owner: recovery::ContextStep,
) -> io::Result<bool> {
    Ok(
        read(connection, step.operation, step.ordinal)?.is_some_and(|bindings| {
            references::contains_primary(&bindings.source, owner.operation, owner.ordinal)
        }),
    )
}

pub(super) fn destination_owners(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<Vec<Owner>> {
    match read(connection, operation, ordinal)? {
        Some(bindings) => Ok(bindings.destination),
        None => destination(connection, operation, ordinal, change),
    }
}

pub(super) fn returned(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    if let Change::DirectoryMerge {
        destination_identity,
        destination_original,
        destination_owned,
        change: inner,
        ..
    } = change
    {
        let owners = if let Some(bindings) = read(connection, id, ordinal)? {
            bindings.source
        } else {
            let (path, identity) =
                source(inner).ok_or_else(|| invalid("directory return has no source context"))?;
            references::owned(connection, path, identity, id, ordinal)?
        };
        references::returned_directory(
            connection,
            &owners,
            *destination_identity,
            *destination_original,
            !destination_owned,
        )?;
        return Ok(());
    }
    let Some((path, logical)) = source(change) else {
        return Ok(());
    };
    let owners = if let Some(context) = read(connection, id, ordinal)? {
        context.source
    } else {
        // A legacy step lacks causal context. Its own expectation remains stable
        // until this transaction consumes it, including during restart recovery.
        let expected = match change {
            Change::Rename { current, .. }
            | Change::Trashed { current, .. }
            | Change::Restored { current, .. } => {
                references::resolve(connection, id, ordinal, current, logical)?
            }
            _ => logical,
        };
        references::owned(connection, path, expected, id, ordinal)?
    };
    references::apply(connection, &owners, Identity::read(path)?)
}

pub(super) fn restored_destination(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    let Some(retained) = change.retained() else {
        return Ok(());
    };
    let owners = match read(connection, id, ordinal)? {
        Some(context) => context.destination,
        None => destination(connection, id, ordinal, change)?,
    };
    references::apply(connection, &owners, retained.observed_identity())
}

pub(super) fn restore_source_context(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    let (_, identity) = source(change)
        .ok_or_else(|| invalid("the directory preparation has no original source"))?;
    let owners = read(connection, id, ordinal)?
        .ok_or_else(|| invalid("the directory preparation has no source bindings"))?
        .source;
    references::apply(connection, &owners, identity)
}

pub(super) fn clear(connection: &Connection, lane: IntentLane) -> io::Result<()> {
    connection
        .execute("DELETE FROM pending_bindings WHERE lane=?1", [lane.index()])
        .map(|_| ())
        .map_err(db_error)
}

/// Complete an already recorded source return without reopening its consumed name.
pub(super) fn returned_recorded(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    identity: Identity,
) -> io::Result<()> {
    let owners = read(connection, id, ordinal)?
        .ok_or_else(|| invalid("The recorded source return has no original identity context."))?
        .source;
    references::apply(connection, &owners, identity)
}
