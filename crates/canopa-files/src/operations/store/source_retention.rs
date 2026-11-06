//! Retained source reservations share the scheduling transaction and survive
//! independently of Undo expiry. They do not authorize executing a saved plan.
use super::*;
use crate::operations::requests::RetainedSources;

impl Store {
    pub(in crate::operations) fn capture_retained_sources(
        &self,
        paths: &[PathBuf],
    ) -> io::Result<RetainedSources> {
        let mut ancestors = BTreeSet::new();
        let mut selections = Vec::new();
        for path in paths {
            if !path.is_absolute() {
                return Err(invalid("retained source locations must be absolute"));
            }
            for ancestor in path.ancestors() {
                if let Ok(metadata) = fs::symlink_metadata(ancestor)
                    && metadata.is_dir()
                {
                    ancestors.insert(encode(&Identity::from_metadata(&metadata))?);
                }
            }
            let tree = fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir());
            let resolved = if tree {
                fs::canonicalize(path)
            } else {
                path.parent().zip(path.file_name()).map_or_else(
                    || Ok(path.clone()),
                    |(parent, name)| fs::canonicalize(parent).map(|parent| parent.join(name)),
                )
            }
            .unwrap_or_else(|_| path.clone());
            selections.push((resolved, tree));
        }
        let mut query = self.connection.prepare_cached("SELECT stage FROM saved_stage_changes UNION SELECT stage FROM saved_stage_records ORDER BY stage").map_err(db_error)?;
        let rows = query
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(db_error)?;
        let mut stages = Vec::new();
        for row in rows {
            let key = row.map_err(db_error)?;
            let identity: Identity = decode(&key)?;
            let mut selected = ancestors.contains(&key);
            if !selected {
                for owner in stages::owners(&self.connection, identity)? {
                    if let Some(record) = self.recovery_record(&owner)?
                        && record.retained.stage.identity == identity
                        && identity.matches(&record.retained.stage.directory)
                        && record.protected_paths().iter().any(|root| {
                            interrupted::resolved_target(root).is_ok_and(|directory| {
                                selections.iter().any(|(path, tree)| {
                                    path.starts_with(&directory)
                                        || (*tree && directory.starts_with(path))
                                })
                            })
                        })
                    {
                        selected = true;
                        break;
                    }
                }
            }
            if selected {
                stages.push(identity);
            }
        }
        Ok(RetainedSources {
            history: self.identity,
            stages,
        })
    }
}

pub(super) fn held(connection: &Connection, identity: Identity) -> io::Result<bool> {
    held_except(connection, identity, None)
}

fn held_except(
    connection: &Connection,
    identity: Identity,
    owner: Option<u64>,
) -> io::Result<bool> {
    connection
        .query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM request_sources WHERE stage=?1 AND (?2 IS NULL OR request<>?2))",
            params![encode(&identity)?, owner],
            |row| row.get(0),
        )
        .map_err(db_error)
}

fn busy() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "A clipboard selection or unfinished operation still needs this retained version. Release the selection or finish or dismiss the operation before changing it.",
    )
}

fn consumed_stage(pending: &Pending) -> Option<Identity> {
    match pending {
        Pending::UndoSavedReturn { returning }
            if returning.phase == undo::saved_return::Phase::Prepared =>
        {
            Some(returning.stage.identity)
        }
        // Returning has consumed its native call. Recovery can inspect or copy
        // this root while the old return is limited to recognizing completion.
        Pending::UndoSavedReturn { .. } => None,
        Pending::UndoCopied { removal } => removal.change.retained().map(|r| r.stage.identity),
        Pending::Expiry { record, .. } => Some(record.retained.stage.identity),
        Pending::Undo { change, .. } | Pending::DirectoryDestination { change, .. } => {
            change.retained().map(|retained| retained.stage.identity)
        }
        _ => None,
    }
}

fn changed_paths(pending: &Pending) -> [Option<&Path>; 2] {
    match pending {
        Pending::UndoSavedReturn { returning } => returning
            .parts()
            .map_or([None, None], |parts| reverse_paths(parts.2)),
        Pending::UndoCopied { removal } => reverse_paths(&removal.change),
        Pending::Undo { change, .. } | Pending::DirectoryDestination { change, .. } => {
            reverse_paths(change)
        }
        Pending::StageMigration { migration } => [
            Some(&migration.source.directory),
            Some(migration.destination()),
        ],
        Pending::Preparing { destination, .. } => [Some(destination), None],
        Pending::Disposing { disposal } => match disposal.undo.as_deref() {
            Some(Pending::Undo { change, .. }) => reverse_paths(change),
            _ => [Some(&disposal.destination), None],
        },
        Pending::Forward {
            change: Change::Copied { path, .. } | Change::RemovedDirectory { path, .. },
            ..
        } => [Some(path), None],
        Pending::Forward {
            change: Change::SourceRemoved { original, .. },
            ..
        } => [Some(original), None],
        Pending::Forward {
            change:
                Change::Rename {
                    original, current, ..
                }
                | Change::Trashed {
                    original, current, ..
                }
                | Change::Restored {
                    original, current, ..
                },
            ..
        } => [Some(original), Some(current)],
        _ => [None, None],
    }
}

fn reverse_paths(change: &Change) -> [Option<&Path>; 2] {
    match change {
        Change::DirectoryMerge { change, .. } => reverse_paths(change),
        Change::SourceRemoved {
            original, current, ..
        } => [Some(original), Some(current)],
        Change::RemovedDirectory {
            path,
            moved: Some(moved),
            ..
        } => [Some(path), Some(&moved.current)],
        _ => [Some(change.forward_path()), change.reverse_path()],
    }
}

/// Inspect parents, preserving the final link's semantics. Moving an entire
/// saved container does not consume its payload; existing catalog relocation
/// keeps that container and its reservations together.
fn changes_inside_stage(
    connection: &Connection,
    pending: &Pending,
    mut protects: impl FnMut(Identity) -> io::Result<bool>,
) -> io::Result<bool> {
    for path in changed_paths(pending).into_iter().flatten() {
        if let Some(parent) = path.parent() {
            let parent = fs::canonicalize(parent)?;
            for ancestor in parent.ancestors() {
                let metadata = fs::metadata(ancestor)?;
                if metadata.is_dir() && protects(Identity::from_metadata(&metadata))? {
                    return Ok(true);
                }
            }
        }
    }
    // A readonly directory claim lives beside its receipt, so its descendants
    // have no receipt-directory ancestor. Keep the same stage reservation for
    // its actual payload tree, including changing the payload root itself.
    let mut query = connection
        .prepare_cached(
            "SELECT g.body FROM saved_stage_records s JOIN garbage g ON g.id=s.entry ORDER BY g.id",
        )
        .map_err(db_error)?;
    let rows = query
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(db_error)?;
    for body in rows {
        let record = recovery::Record::decode(&body.map_err(db_error)?)?;
        if !record.payload
            || !record.retained.stage.sibling_payload
            || !protects(record.retained.stage.identity)?
        {
            continue;
        }
        // An unexpected claim may be a final symlink, including a dangling one.
        // Its referent is separate data and grants no ownership of this entry.
        let root = interrupted::resolved_target(&record.path())?;
        for path in changed_paths(pending).into_iter().flatten() {
            if interrupted::resolved_target(path)?.starts_with(&root) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(super) fn check_intent(connection: &Connection, pending: &Pending) -> io::Result<()> {
    if let Some(identity) = consumed_stage(pending)
        && held(connection, identity)?
    {
        return Err(busy());
    }
    let any_held: bool = connection
        .query_row_cached("SELECT EXISTS(SELECT 1 FROM request_sources)", [], |row| {
            row.get(0)
        })
        .map_err(db_error)?;
    // Only the exact accepted forward Rename may use its own reservation.
    // Reverse work can carry that same operation ID: it must never inherit this
    // exception. Keep the durable association for interrupted forward journals,
    // even after the scheduling row has advanced to a stopped revision.
    let owner = match pending {
        Pending::Forward {
            id,
            change: Change::Rename { .. },
            ..
        } => connection
            .query_row_cached(
                "SELECT request FROM request_executions WHERE operation=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?,
        _ => None,
    };
    if any_held
        && changes_inside_stage(connection, pending, |identity| {
            held_except(connection, identity, owner)
        })?
    {
        return Err(busy());
    }
    Ok(())
}

pub(super) fn acquire(
    connection: &Connection,
    request: u64,
    sources: &RetainedSources,
) -> io::Result<()> {
    for identity in &sources.stages {
        if stages::owners(connection, *identity)?.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "The retained source changed before the request was accepted. Select it again.",
            ));
        }
        for table in ["pending", "recovery_pending"] {
            let body: Option<String> = connection
                .query_row_cached(
                    &format!("SELECT body FROM {table} WHERE singleton=1"),
                    [],
                    |row| row.get(0),
                )
                .optional()
                .map_err(db_error)?;
            if let Some(body) = body {
                let pending: Pending = decode(&body)?;
                let changing =
                    consumed_stage(&pending) == Some(*identity)
                        || changes_inside_stage(connection, &pending, |parent| {
                            Ok(parent == *identity)
                        })?
                        || matches!(&pending, Pending::StageMigration { migration }
                        if migration.source.identity == *identity
                            || migration.stage.identity == *identity);
                if changing {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "The retained source is changing. Wait for the current operation before selecting it again.",
                    ));
                }
            }
        }
        connection
            .execute_cached(
                "INSERT INTO request_sources VALUES(?1,?2)",
                params![request, encode(identity)?],
            )
            .map_err(db_error)?;
    }
    Ok(())
}

/// Resolve the original live owner's current stage identities in the admission
/// transaction. Physical migration updates these reservations with the catalog;
/// looking at source pathnames again could adopt an unrelated retained version.
pub(super) fn acquire_lease(
    connection: &Connection,
    request: u64,
    lease: u64,
    history: Identity,
) -> io::Result<()> {
    let exists: bool = connection
        .query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM source_leases WHERE request=?1)",
            [lease],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if !exists {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "The original clipboard protection is unavailable. Copy or Cut the files again.",
        ));
    }
    let mut query = connection
        .prepare_cached("SELECT stage FROM request_sources WHERE request=?1 ORDER BY stage")
        .map_err(db_error)?;
    let rows = query
        .query_map([lease], |row| row.get::<_, String>(0))
        .map_err(db_error)?;
    let mut stages = Vec::new();
    for row in rows {
        stages.push(decode(&row.map_err(db_error)?)?);
    }
    acquire(connection, request, &RetainedSources { history, stages })
}

pub(super) fn require_released(connection: &Connection, identity: Identity) -> io::Result<()> {
    if held(connection, identity)? {
        Err(busy())
    } else {
        Ok(())
    }
}

/// Catalog cutover and source reservations must name the same physical stage.
/// A request may already own both versions; coalesce rather than dropping either.
pub(super) fn relocate(connection: &Connection, from: Identity, to: Identity) -> io::Result<()> {
    if from == to {
        return Ok(());
    }
    let from = encode(&from)?;
    let to = encode(&to)?;
    connection.execute_cached("INSERT OR IGNORE INTO request_sources(request,stage) SELECT request,?2 FROM request_sources WHERE stage=?1", params![from,to]).map_err(db_error)?;
    connection
        .execute_cached("DELETE FROM request_sources WHERE stage=?1", [from])
        .map_err(db_error)?;
    Ok(())
}
