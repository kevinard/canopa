//! Physical expectations are scoped to a history reference, not a path-wide
//! alias. Roles remain stable when an entry's paths are relocated.
use super::*;

const PRIMARY: u8 = 0;
const PARENT: u8 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Owner {
    operation: u64,
    ordinal: u64,
    role: u8,
}
pub(super) fn contains_step(owners: &[Owner], operation: u64, ordinal: u64) -> bool {
    owners
        .iter()
        .any(|owner| owner.operation == operation && owner.ordinal == ordinal)
}

pub(super) fn contains_primary(owners: &[Owner], operation: u64, ordinal: u64) -> bool {
    owners.iter().any(|owner| {
        owner.operation == operation && owner.ordinal == ordinal && owner.role == PRIMARY
    })
}

pub(super) fn primary(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
) -> io::Result<Option<Identity>> {
    let body: Option<String> = connection
        .query_row_cached(
            "SELECT expected FROM identity_refs WHERE operation=?1 AND ordinal=?2 AND role=0",
            params![operation, ordinal],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    body.map(|body| decode(&body)).transpose()
}

pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS identity_refs (
        operation INTEGER NOT NULL, ordinal INTEGER NOT NULL,
        role INTEGER NOT NULL CHECK(role IN (0,1)), path BLOB NOT NULL,
        logical TEXT NOT NULL, expected TEXT NOT NULL,
        PRIMARY KEY(operation,ordinal,role),
        FOREIGN KEY(operation,ordinal) REFERENCES changes(operation,ordinal) ON DELETE CASCADE);
        CREATE INDEX IF NOT EXISTS identity_refs_location ON identity_refs(path,expected);
        CREATE INDEX IF NOT EXISTS identity_refs_expected ON identity_refs(expected,operation,ordinal,role);
        CREATE TABLE IF NOT EXISTS identity_ref_origins (
        operation INTEGER NOT NULL, ordinal INTEGER NOT NULL, role INTEGER NOT NULL,
        expected TEXT NOT NULL, body TEXT NOT NULL,
        PRIMARY KEY(operation,ordinal,role),
        FOREIGN KEY(operation,ordinal,role) REFERENCES identity_refs(operation,ordinal,role) ON DELETE CASCADE);",
        )
        .map_err(db_error)
}

fn fields(change: &Change) -> Vec<(u8, PathBuf, Identity)> {
    let mut result = Vec::new();
    match change {
        Change::DirectoryMerge { change, .. } => {
            if let Change::RemovedDirectory {
                path,
                parent_identity,
                moved: Some(moved),
                ..
            } = change.as_ref()
            {
                return vec![
                    (PRIMARY, moved.current.clone(), moved.identity),
                    (PARENT, path.parent().unwrap().to_owned(), *parent_identity),
                ];
            }
            return fields(change);
        }
        Change::CreatedFolder { path, identity, .. }
        | Change::Copied { path, identity, .. }
        | Change::RemovedDirectory { path, identity, .. } => {
            result.push((PRIMARY, path.clone(), *identity))
        }
        Change::Rename {
            current, identity, ..
        }
        | Change::Trashed {
            current, identity, ..
        }
        | Change::Restored {
            current, identity, ..
        } => result.push((PRIMARY, current.clone(), *identity)),
        Change::SourceRemoved {
            current,
            copy_identity,
            ..
        } => result.push((PRIMARY, current.clone(), *copy_identity)),
        Change::RetainedOnly { .. } => {}
    }
    match change {
        Change::CreatedFolder {
            path,
            access_parent: Some(identity),
            ..
        }
        | Change::Copied {
            path,
            access_parent: Some(identity),
            ..
        } => {
            result.push((PARENT, path.parent().unwrap().to_owned(), *identity));
        }
        Change::Rename {
            original,
            saved: Some(saved),
            ..
        } => result.push((
            PARENT,
            original.parent().unwrap().to_owned(),
            saved.parent_identity,
        )),
        Change::SourceRemoved {
            original,
            quarantine,
            ..
        } => {
            if let Some(identity) = quarantine.parent_identity {
                result.push((PARENT, original.parent().unwrap().to_owned(), identity));
            }
        }
        Change::RemovedDirectory {
            path,
            parent_identity,
            ..
        } => result.push((PARENT, path.parent().unwrap().to_owned(), *parent_identity)),
        _ => {}
    }
    result
}

pub(super) fn index(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    change: &Change,
    legacy: bool,
) -> io::Result<()> {
    stages::index_change(connection, operation, ordinal, change)?;
    saved::index(connection, operation, ordinal, change)?;
    directory::index_moved(connection, operation, ordinal, change)?;
    let fields = fields(change);
    for role in [PRIMARY, PARENT] {
        if !fields.iter().any(|field| field.0 == role) {
            connection
                .execute_cached(
                    "DELETE FROM identity_refs WHERE operation=?1 AND ordinal=?2 AND role=?3",
                    params![operation, ordinal, role],
                )
                .map_err(db_error)?;
        }
    }
    for (role, path, logical) in fields {
        let previous: Option<(String,String)> = connection.query_row_cached("SELECT logical,expected FROM identity_refs WHERE operation=?1 AND ordinal=?2 AND role=?3", params![operation,ordinal,role], |row| Ok((row.get(0)?,row.get(1)?))).optional().map_err(db_error)?;
        let encoded = encode(&logical)?;
        let expected = if let Some((_, expected)) =
            previous.filter(|previous| previous.0 == encoded)
        {
            expected
        } else if legacy {
            let after: u64 = connection
                .query_row_cached(
                    "SELECT binding_revision FROM changes WHERE operation=?1 AND ordinal=?2",
                    params![operation, ordinal],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            let resolved = resolve_binding(connection, &path, logical, after)?;
            // Old path aliases do not prove that an independently restored copy
            // was explicitly adopted. Require its normal changed-copy review.
            if role == PRIMARY && matches!(change, Change::Copied { .. }) && resolved != logical {
                encoded.clone()
            } else {
                encode(&resolved)?
            }
        } else {
            encoded.clone()
        };
        connection.execute_cached("INSERT INTO identity_refs(operation,ordinal,role,path,logical,expected) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(operation,ordinal,role) DO UPDATE SET path=excluded.path,logical=excluded.logical,expected=excluded.expected", params![operation,ordinal,role,paths::key(&path),encoded,expected]).map_err(db_error)?;
    }
    Ok(())
}

pub(super) fn populate(connection: &Connection) -> io::Result<()> {
    let mut statement = connection
        .prepare_cached("SELECT operation,ordinal,body FROM changes ORDER BY operation,ordinal")
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
        index(connection, operation, ordinal, &change, true)?;
    }
    Ok(())
}

pub(super) fn resolve(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    path: &Path,
    logical: Identity,
) -> io::Result<Identity> {
    let body: String = connection.query_row_cached("SELECT expected FROM identity_refs WHERE operation=?1 AND ordinal=?2 AND path=?3 AND logical=?4", params![operation,ordinal,paths::key(path),encode(&logical)?], |row| row.get(0)).map_err(db_error)?;
    decode(&body)
}

/// Generation evidence follows this exact surviving history reference. A later
/// return may retire its own frame while an older saved-container move still
/// needs the parent that frame created. Never capture a current path here.
pub(super) fn resolved_original(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    path: &Path,
    logical: Identity,
) -> io::Result<Option<original::Original>> {
    let body: Option<String> = connection.query_row_cached("SELECT o.body FROM identity_ref_origins o JOIN identity_refs r ON r.operation=o.operation AND r.ordinal=o.ordinal AND r.role=o.role AND r.expected=o.expected WHERE r.operation=?1 AND r.ordinal=?2 AND r.path=?3 AND r.logical=?4", params![operation,ordinal,paths::key(path),encode(&logical)?], |r| r.get(0)).optional().map_err(db_error)?;
    body.map(|body| decode(&body)).transpose()
}

pub(super) fn owned(
    connection: &Connection,
    path: &Path,
    expected: Identity,
    operation: u64,
    ordinal: u64,
) -> io::Result<Vec<Owner>> {
    // Historical paths intentionally stay with their Undo step. A committed
    // enclosing Rename can nevertheless put that same physical file at this
    // source. Resolve that namespace chain before capturing ownership; matching
    // an inode elsewhere in history alone does not grant ownership at this path.
    let mut statement = connection.prepare_cached("SELECT operation,ordinal,role,path FROM identity_refs WHERE expected=?1 AND (operation,ordinal)<(?2,?3) ORDER BY operation,ordinal,role").map_err(db_error)?;
    let rows: Vec<(Owner, Vec<u8>)> = statement
        .query_map(params![encode(&expected)?, operation, ordinal], |row| {
            Ok((
                Owner {
                    operation: row.get(0)?,
                    ordinal: row.get(1)?,
                    role: row.get(2)?,
                },
                row.get::<_, Vec<u8>>(3)?,
            ))
        })
        .map_err(db_error)?
        .collect::<Result<_, _>>()
        .map_err(db_error)?;
    let path = paths::key(path);
    let mut owners = Vec::new();
    for (owner, historical) in rows {
        if paths::follow_namespace(
            connection,
            &historical,
            (owner.operation, owner.ordinal),
            (operation, ordinal),
        )? == path
        {
            owners.push(owner);
        }
    }
    Ok(owners)
}

pub(super) fn apply(
    connection: &Connection,
    owners: &[Owner],
    expected: Identity,
) -> io::Result<()> {
    let mut statement = connection
        .prepare_cached(
            "UPDATE identity_refs SET expected=?4 WHERE operation=?1 AND ordinal=?2 AND role=?3",
        )
        .map_err(db_error)?;
    let expected = encode(&expected)?;
    for owner in owners {
        statement
            .execute(params![
                owner.operation,
                owner.ordinal,
                owner.role,
                expected
            ])
            .map_err(db_error)?;
    }
    Ok(())
}

pub(super) fn adopt_primary(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    expected: Identity,
) -> io::Result<()> {
    apply(
        connection,
        &[Owner {
            operation,
            ordinal,
            role: PRIMARY,
        }],
        expected,
    )
}

pub(super) fn apply_parents(
    connection: &Connection,
    owners: &[Owner],
    expected: Identity,
) -> io::Result<()> {
    let parents: Vec<_> = owners
        .iter()
        .filter(|owner| owner.role == PARENT)
        .cloned()
        .collect();
    apply(connection, &parents, expected)
}

pub(super) fn returned_directory(
    connection: &Connection,
    owners: &[Owner],
    expected: Identity,
    original: Option<original::Original>,
    parents_only: bool,
) -> io::Result<()> {
    if parents_only {
        apply_parents(connection, owners, expected)?;
    } else {
        apply(connection, owners, expected)?;
    }
    let Some(original) = original else {
        return Ok(());
    };
    for owner in owners
        .iter()
        .filter(|owner| !parents_only || owner.role == PARENT)
    {
        let exists: bool = connection.query_row_cached("SELECT EXISTS(SELECT 1 FROM identity_refs WHERE operation=?1 AND ordinal=?2 AND role=?3)", params![owner.operation,owner.ordinal,owner.role], |r| r.get(0)).map_err(db_error)?;
        if !exists {
            continue;
        }
        if connection.execute_cached("INSERT INTO identity_ref_origins(operation,ordinal,role,expected,body) SELECT operation,ordinal,role,expected,?4 FROM identity_refs WHERE operation=?1 AND ordinal=?2 AND role=?3 AND expected=?5 ON CONFLICT(operation,ordinal,role) DO UPDATE SET expected=excluded.expected,body=excluded.body", params![owner.operation,owner.ordinal,owner.role,encode(&original)?,encode(&expected)?]).map_err(db_error)? != 1 { return Err(invalid("The returned parent generation was not saved.")); }
        let saved: String = connection.query_row_cached("SELECT o.body FROM identity_ref_origins o JOIN identity_refs r ON r.operation=o.operation AND r.ordinal=o.ordinal AND r.role=o.role AND r.expected=o.expected WHERE o.operation=?1 AND o.ordinal=?2 AND o.role=?3",params![owner.operation,owner.ordinal,owner.role],|r|r.get(0)).map_err(db_error)?;
        if saved != encode(&original)? {
            return Err(invalid(
                "The returned parent generation was not recorded exactly.",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate(owners: &[Owner], operation: u64, ordinal: u64) -> io::Result<()> {
    if owners.iter().any(|owner| {
        owner.role > PARENT || (owner.operation, owner.ordinal) >= (operation, ordinal)
    }) {
        return Err(invalid("invalid history identity ownership"));
    }
    Ok(())
}

/// Change one frame's expected physical copy after a verified source return.
pub(super) fn expect_primary(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    identity: Identity,
) -> io::Result<()> {
    let count = connection
        .execute_cached(
            "UPDATE identity_refs SET expected=?3 WHERE operation=?1 AND ordinal=?2 AND role=0",
            params![operation, ordinal, encode(&identity)?],
        )
        .map_err(db_error)?;
    if count != 1 {
        return Err(invalid("missing destination-copy identity reference"));
    }
    Ok(())
}
