//! Reservations for saved destinations survive independently of later occupants.
use super::*;

pub(super) fn create(connection: &Connection, version: u32) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS pending_saved_destinations (
        operation INTEGER NOT NULL, ordinal INTEGER NOT NULL,
        PRIMARY KEY(operation,ordinal),
        FOREIGN KEY(operation,ordinal) REFERENCES changes(operation,ordinal) ON DELETE CASCADE);",
        )
        .map_err(db_error)?;
    if version < 11 {
        let rows: Vec<(u64, u64, String)> = {
            let mut statement = connection
                .prepare_cached(
                    "SELECT operation,ordinal,body FROM changes ORDER BY operation,ordinal",
                )
                .map_err(db_error)?;
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .map_err(db_error)?
                .collect::<Result<_, _>>()
                .map_err(db_error)?
        };
        for (operation, ordinal, body) in rows {
            index(connection, operation, ordinal, &decode::<Change>(&body)?)?;
        }
    }
    Ok(())
}

pub(super) fn index(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    if matches!(change, Change::RetainedOnly { .. })
        || (matches!(change, Change::DirectoryMerge { .. }) && change.retained().is_some())
    {
        connection.execute_cached("INSERT INTO pending_saved_destinations(operation,ordinal) VALUES(?1,?2) ON CONFLICT DO NOTHING",params![operation,ordinal]).map_err(db_error)?;
    } else {
        connection
            .execute_cached(
                "DELETE FROM pending_saved_destinations WHERE operation=?1 AND ordinal=?2",
                params![operation, ordinal],
            )
            .map_err(db_error)?;
    }
    Ok(())
}

pub(super) fn is_reserved(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
) -> io::Result<bool> {
    connection.query_row_cached("SELECT EXISTS(SELECT 1 FROM pending_saved_destinations WHERE operation=?1 AND ordinal=?2)",params![operation,ordinal],|row|row.get(0)).map_err(db_error)
}

pub(super) fn holds_destination(
    connection: &Connection,
    movements: &directory::relocation::Movements,
    caller: (u64, u64),
    reference: (u64, u64),
    relocation: &undo::Relocation,
) -> io::Result<bool> {
    let mut statement=connection.prepare_cached("SELECT d.operation,d.ordinal,c.body FROM pending_saved_destinations d JOIN changes c ON c.operation=d.operation AND c.ordinal=d.ordinal WHERE d.operation<=?1 AND NOT(d.operation=?1 AND d.ordinal=?2)").map_err(db_error)?;
    let rows = statement
        .query_map(params![caller.0, caller.1], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(db_error)?;
    for row in rows {
        let (operation, ordinal, body) = row.map_err(db_error)?;
        let change: Change = decode(&body)?;
        if movements.saved_slot_moves(connection, operation, ordinal, &change, relocation)? {
            continue;
        }
        let owners = context::destination_owners(connection, operation, ordinal, &change)?;
        if references::contains_step(&owners, reference.0, reference.1) {
            return Ok(true);
        }
    }
    Ok(false)
}
