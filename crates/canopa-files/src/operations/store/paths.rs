//! Derived namespace index. Keys use Path's component semantics, never display
//! text or filesystem canonicalization. Every ancestor points to its owners, so
//! a subtree relocation reads only affected records in bounded ordered pages.
use super::*;
use std::{collections::BTreeSet, os::unix::ffi::OsStringExt};

pub(super) fn create(connection: &Connection) -> io::Result<()> {
    let namespace_exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='namespace_moves')", [], |row|row.get(0)).map_err(db_error)?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS change_paths (
            path BLOB NOT NULL, operation INTEGER NOT NULL, ordinal INTEGER NOT NULL,
            PRIMARY KEY(path,operation,ordinal),
            FOREIGN KEY(operation,ordinal) REFERENCES changes(operation,ordinal) ON DELETE CASCADE);
        CREATE INDEX IF NOT EXISTS change_paths_owner ON change_paths(operation,ordinal);
        CREATE TABLE IF NOT EXISTS folder_changes (
            path BLOB NOT NULL, operation INTEGER NOT NULL, ordinal INTEGER NOT NULL,
            PRIMARY KEY(path,operation,ordinal),
            FOREIGN KEY(operation,ordinal) REFERENCES changes(operation,ordinal) ON DELETE CASCADE);
        CREATE INDEX IF NOT EXISTS folder_changes_owner ON folder_changes(operation,ordinal);
        CREATE TABLE IF NOT EXISTS garbage_paths (
            path BLOB NOT NULL, entry INTEGER NOT NULL REFERENCES garbage(id) ON DELETE CASCADE,
            PRIMARY KEY(path,entry));
        CREATE INDEX IF NOT EXISTS garbage_paths_owner ON garbage_paths(entry);
        CREATE TABLE IF NOT EXISTS binding_paths (
            namespace BLOB NOT NULL, path BLOB NOT NULL, previous TEXT NOT NULL,
            PRIMARY KEY(namespace,path,previous),
            FOREIGN KEY(path,previous) REFERENCES bindings(path,previous) ON DELETE CASCADE);
        CREATE INDEX IF NOT EXISTS binding_paths_owner ON binding_paths(path,previous);
        CREATE TABLE IF NOT EXISTS namespace_moves (
            operation INTEGER NOT NULL, ordinal INTEGER NOT NULL,
            original BLOB NOT NULL, current BLOB NOT NULL,
            PRIMARY KEY(operation,ordinal),
            FOREIGN KEY(operation,ordinal) REFERENCES changes(operation,ordinal) ON DELETE CASCADE);
        CREATE INDEX IF NOT EXISTS namespace_moves_source ON namespace_moves(original,operation,ordinal);",
        )
        .map_err(db_error)?;
    if !namespace_exists {
        let mut after: Option<(u64, u64)> = None;
        loop {
            let rows: Vec<(u64, u64, String)> = {
                let mut statement=connection.prepare("SELECT operation,ordinal,body FROM changes WHERE ?1 IS NULL OR (operation,ordinal)>(?1,?2) ORDER BY operation,ordinal LIMIT 256").map_err(db_error)?;
                statement
                    .query_map(
                        params![after.map(|key| key.0), after.map(|key| key.1)],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(db_error)?
                    .collect::<Result<_, _>>()
                    .map_err(db_error)?
            };
            if rows.is_empty() {
                break;
            }
            for (operation, ordinal, body) in rows {
                after = Some((operation, ordinal));
                let change = decode(&body)?;
                validate_change(&change)?;
                index_namespace(connection, operation, ordinal, &change)?;
            }
        }
    }
    Ok(())
}

fn index_namespace(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    connection
        .execute(
            "DELETE FROM namespace_moves WHERE operation=?1 AND ordinal=?2",
            params![operation, ordinal],
        )
        .map_err(db_error)?;
    if let Change::Rename {
        original, current, ..
    }
    | Change::Trashed {
        original, current, ..
    }
    | Change::Restored {
        original, current, ..
    } = change
    {
        connection
            .execute(
                "INSERT INTO namespace_moves VALUES(?1,?2,?3,?4)",
                params![operation, ordinal, key(original), key(current)],
            )
            .map_err(db_error)?;
    }
    Ok(())
}

/// Whether a later namespace move can affect this path or one of its ancestors.
pub(super) fn has_later_namespace_move(
    connection: &Connection,
    path: &Path,
    after: (u64, u64),
) -> io::Result<bool> {
    let mut statement = connection.prepare_cached("SELECT EXISTS(SELECT 1 FROM namespace_moves WHERE original=?1 AND (operation,ordinal)>(?2,?3))").map_err(db_error)?;
    for ancestor in path.ancestors() {
        let found = statement
            .query_row(params![key(ancestor), after.0, after.1], |row| {
                row.get::<_, bool>(0)
            })
            .map_err(db_error)?;
        if found {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Follow only committed moves of this path or one of its ancestors, in order.
pub(super) fn follow_namespace(
    connection: &Connection,
    historical: &[u8],
    mut after: (u64, u64),
    before: (u64, u64),
) -> io::Result<Vec<u8>> {
    let mut path = PathBuf::from(std::ffi::OsString::from_vec(historical.to_vec()));
    let mut statement=connection.prepare_cached("SELECT operation,ordinal,current FROM namespace_moves WHERE original=?1 AND (operation,ordinal)>(?2,?3) AND (operation,ordinal)<(?4,?5) ORDER BY operation,ordinal LIMIT 1").map_err(db_error)?;
    loop {
        let mut next: Option<((u64, u64), PathBuf, Vec<u8>)> = None;
        for ancestor in path.ancestors() {
            let row: Option<(u64, u64, Vec<u8>)> = statement
                .query_row(
                    params![key(ancestor), after.0, after.1, before.0, before.1],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(db_error)?;
            if let Some((operation, ordinal, current)) = row
                && next
                    .as_ref()
                    .is_none_or(|next| (operation, ordinal) < next.0)
            {
                next = Some(((operation, ordinal), ancestor.to_owned(), current));
            }
        }
        let Some((step, original, current)) = next else {
            return Ok(key(&path));
        };
        let current = PathBuf::from(std::ffi::OsString::from_vec(current));
        path = current.join(path.strip_prefix(original).unwrap());
        after = step;
    }
}

pub(super) fn populate(connection: &Connection) -> io::Result<()> {
    let mut statement = connection
        .prepare("SELECT operation,ordinal,body FROM changes ORDER BY operation,ordinal")
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(db_error)?;
    for row in rows {
        let (operation, ordinal, body) = row.map_err(db_error)?;
        let change = decode(&body)?;
        validate_change(&change)?;
        index_change(connection, operation, ordinal, &change)?;
    }
    let mut statement = connection
        .prepare("SELECT id,body FROM garbage ORDER BY id")
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(db_error)?;
    for row in rows {
        let (entry, body) = row.map_err(db_error)?;
        index_record(connection, entry, &recovery::Record::decode(&body)?)?;
    }
    let mut statement = connection
        .prepare("SELECT path,previous FROM bindings ORDER BY path,previous")
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(db_error)?;
    for row in rows {
        let (path, previous) = row.map_err(db_error)?;
        index_binding(connection, &path, &previous)?;
    }
    Ok(())
}

pub(super) fn index_change(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    index_namespace(connection, operation, ordinal, change)?;
    connection
        .execute(
            "DELETE FROM folder_changes WHERE operation=?1 AND ordinal=?2",
            params![operation, ordinal],
        )
        .map_err(db_error)?;
    if let Change::CreatedFolder { path, .. } = change {
        connection
            .execute(
                "INSERT INTO folder_changes VALUES(?1,?2,?3)",
                params![key(path), operation, ordinal],
            )
            .map_err(db_error)?;
    }
    let mut keys = BTreeSet::new();
    change
        .clone()
        .visit_paths(|path| ancestors(path, &mut keys));
    connection
        .execute(
            "DELETE FROM change_paths WHERE operation=?1 AND ordinal=?2",
            params![operation, ordinal],
        )
        .map_err(db_error)?;
    let mut statement = connection
        .prepare_cached("INSERT INTO change_paths VALUES(?1,?2,?3)")
        .map_err(db_error)?;
    for key in keys {
        statement
            .execute(params![key, operation, ordinal])
            .map_err(db_error)?;
    }
    Ok(())
}

pub(super) fn index_record(
    connection: &Connection,
    entry: u64,
    record: &recovery::Record,
) -> io::Result<()> {
    stages::index_record(connection, entry, record)?;
    origins::index_record(connection, entry, record)?;
    let keys = record_keys(record);
    connection
        .execute("DELETE FROM garbage_paths WHERE entry=?1", [entry])
        .map_err(db_error)?;
    let mut statement = connection
        .prepare_cached("INSERT INTO garbage_paths VALUES(?1,?2)")
        .map_err(db_error)?;
    for key in keys {
        statement.execute(params![key, entry]).map_err(db_error)?;
    }
    Ok(())
}

fn record_keys(record: &recovery::Record) -> BTreeSet<Vec<u8>> {
    let mut keys = BTreeSet::new();
    record
        .clone()
        .visit_paths(|path| ancestors(path, &mut keys));
    for path in record.protected_paths() {
        ancestors(&path, &mut keys);
    }
    keys
}

/// A retained owner must be discoverable by both namespace and physical root
/// before an interrupted journal relinquishes its protection.
pub(super) fn require_retained_record(
    connection: &Connection,
    entry: u64,
    record: &recovery::Record,
) -> io::Result<()> {
    let mut statement = connection
        .prepare("SELECT path FROM garbage_paths WHERE entry=?1")
        .map_err(db_error)?;
    let actual = statement
        .query_map([entry], |row| row.get::<_, Vec<u8>>(0))
        .map_err(db_error)?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(db_error)?;
    let root: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM saved_stage_records WHERE entry=?1 AND stage=?2)",
            params![entry, encode(&record.retained.stage.identity)?],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if !root || actual != record_keys(record) {
        return Err(invalid(
            "The retained work ownership index was not recorded.",
        ));
    }
    Ok(())
}

pub(super) fn index_binding(
    connection: &Connection,
    path: &[u8],
    previous: &str,
) -> io::Result<()> {
    let mut keys = BTreeSet::new();
    ancestors(
        &PathBuf::from(std::ffi::OsString::from_vec(path.to_vec())),
        &mut keys,
    );
    connection
        .execute(
            "DELETE FROM binding_paths WHERE path=?1 AND previous=?2",
            params![path, previous],
        )
        .map_err(db_error)?;
    let mut statement = connection
        .prepare_cached("INSERT INTO binding_paths VALUES(?1,?2,?3)")
        .map_err(db_error)?;
    for key in keys {
        statement
            .execute(params![key, path, previous])
            .map_err(db_error)?;
    }
    Ok(())
}

pub(super) fn key(path: &Path) -> Vec<u8> {
    path.components()
        .collect::<PathBuf>()
        .into_os_string()
        .into_vec()
}

fn ancestors(path: &Path, keys: &mut BTreeSet<Vec<u8>>) {
    let normalized = path.components().collect::<PathBuf>();
    for ancestor in normalized.ancestors() {
        keys.insert(ancestor.as_os_str().as_encoded_bytes().to_vec());
    }
}
