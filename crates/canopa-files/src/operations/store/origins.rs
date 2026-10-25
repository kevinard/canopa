//! Context authority follows history keys; retired keys queue only their owners.
use super::*;

pub(super) fn create(connection: &Connection, version: u32) -> io::Result<()> {
    connection.execute_batch("CREATE TABLE IF NOT EXISTS recovery_context_steps (
        entry INTEGER NOT NULL REFERENCES garbage(id) ON DELETE CASCADE,
        movement INTEGER NOT NULL, role INTEGER NOT NULL CHECK(role IN (0,1)),
        operation INTEGER NOT NULL, ordinal INTEGER NOT NULL,
        PRIMARY KEY(entry,movement,role));
        CREATE INDEX IF NOT EXISTS recovery_context_steps_step ON recovery_context_steps(operation,ordinal,entry);
        CREATE TABLE IF NOT EXISTS recovery_context_cleanup (
        entry INTEGER PRIMARY KEY REFERENCES garbage(id) ON DELETE CASCADE);
        CREATE TRIGGER IF NOT EXISTS recovery_context_deleted AFTER DELETE ON changes BEGIN
        INSERT OR IGNORE INTO recovery_context_cleanup(entry)
        SELECT entry FROM recovery_context_steps WHERE operation=OLD.operation AND ordinal=OLD.ordinal;
        END;").map_err(db_error)?;
    if version < 16 {
        let mut after = 0;
        loop {
            let rows = records(connection, after)?;
            if rows.is_empty() {
                break;
            }
            for (entry, body) in rows {
                after = entry;
                let mut record = recovery::Record::decode(&body)?;
                compact_legacy(&mut record);
                write(connection, entry, &body, &record)?;
            }
        }
    }
    prune(connection)
}

fn records(connection: &Connection, after: u64) -> io::Result<Vec<(u64, String)>> {
    let mut statement = connection
        .prepare("SELECT id,body FROM garbage WHERE id>?1 ORDER BY id LIMIT 256")
        .map_err(db_error)?;
    statement
        .query_map([after], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(db_error)?
        .collect::<Result<_, _>>()
        .map_err(db_error)
}

pub(super) fn index_record(
    connection: &Connection,
    entry: u64,
    record: &recovery::Record,
) -> io::Result<()> {
    connection
        .execute("DELETE FROM recovery_context_steps WHERE entry=?1", [entry])
        .map_err(db_error)?;
    let mut statement = connection
        .prepare_cached("INSERT INTO recovery_context_steps VALUES(?1,?2,?3,?4,?5)")
        .map_err(db_error)?;
    for (index, movement) in record.context_moves.iter().enumerate() {
        let index = u64::try_from(index).map_err(io::Error::other)?;
        for (role, step) in [(0, movement.container_step), (1, movement.owner_step)] {
            if let Some(step) = step {
                statement
                    .execute(params![entry, index, role, step.operation, step.ordinal])
                    .map_err(db_error)?;
            }
        }
    }
    Ok(())
}

fn compact_legacy(record: &mut recovery::Record) {
    let mut retained = false;
    record.context_moves.retain_mut(|movement| {
        if movement.container_step.is_some() {
            return true;
        }
        movement.owner_step = None;
        if retained {
            false
        } else {
            retained = true;
            true
        }
    });
}

fn exists(connection: &Connection, step: recovery::ContextStep) -> io::Result<bool> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM changes WHERE operation=?1 AND ordinal=?2)",
            params![step.operation, step.ordinal],
            |row| row.get(0),
        )
        .map_err(db_error)
}

pub(super) fn parent(
    connection: &Connection,
    movement: &recovery::ContextMove,
) -> io::Result<Option<Identity>> {
    let (Some(step), Some(logical)) = (movement.container_step, movement.parent_identity) else {
        return Ok(None);
    };
    let body:Option<String>=connection.query_row("SELECT expected FROM identity_refs WHERE operation=?1 AND ordinal=?2 AND role=1 AND logical=?3",
        params![step.operation,step.ordinal,encode(&logical)?],|row|row.get(0)).optional().map_err(db_error)?;
    body.map(|body| decode(&body)).transpose()
}

fn write(
    connection: &Connection,
    entry: u64,
    previous: &str,
    record: &recovery::Record,
) -> io::Result<()> {
    let body = encode(record)?;
    if body != previous {
        connection
            .execute(
                "UPDATE garbage SET body=?2 WHERE id=?1",
                params![entry, body],
            )
            .map_err(db_error)?;
    }
    paths::index_record(connection, entry, record)
}

pub(super) fn prune(connection: &Connection) -> io::Result<()> {
    let mut after = 0;
    loop {
        let rows: Vec<(u64, String)> = {
            let mut statement=connection.prepare("SELECT q.entry,g.body FROM recovery_context_cleanup q JOIN garbage g ON g.id=q.entry WHERE q.entry>?1 ORDER BY q.entry LIMIT 256").map_err(db_error)?;
            statement
                .query_map([after], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(db_error)?
                .collect::<Result<_, _>>()
                .map_err(db_error)?
        };
        if rows.is_empty() {
            break;
        }
        for (entry, body) in rows {
            after = entry;
            let mut record = recovery::Record::decode(&body)?;
            let mut retained = Vec::new();
            for movement in std::mem::take(&mut record.context_moves) {
                if movement
                    .container_step
                    .map(|step| exists(connection, step))
                    .transpose()?
                    .unwrap_or(true)
                    || movement
                        .owner_step
                        .map(|step| exists(connection, step))
                        .transpose()?
                        .unwrap_or(false)
                {
                    retained.push(movement);
                }
            }
            record.context_moves = retained;
            compact_legacy(&mut record);
            write(connection, entry, &body, &record)?;
            connection
                .execute(
                    "DELETE FROM recovery_context_cleanup WHERE entry=?1",
                    [entry],
                )
                .map_err(db_error)?;
        }
    }
    Ok(())
}
