//! Persistent child membership keeps partial returns in their original Undo entry.
use super::*;
use undo::directory_return;
pub(super) mod relocation;

pub(super) fn create(connection: &Connection, version: u32) -> io::Result<()> {
    let indexed: bool = connection.query_row_cached(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='moved_directory_roots')",
        [], |row| row.get(0),
    ).map_err(db_error)?;
    connection.execute_batch("CREATE TABLE IF NOT EXISTS directory_returns (
        operation INTEGER NOT NULL, ordinal INTEGER NOT NULL,
        PRIMARY KEY(operation,ordinal),
        FOREIGN KEY(operation,ordinal) REFERENCES changes(operation,ordinal) ON DELETE CASCADE);
        CREATE TABLE IF NOT EXISTS directory_members (
        operation INTEGER NOT NULL, ordinal INTEGER NOT NULL, parent INTEGER NOT NULL,
        PRIMARY KEY(operation,ordinal),
        FOREIGN KEY(operation,ordinal) REFERENCES changes(operation,ordinal) ON DELETE CASCADE,
        FOREIGN KEY(operation,parent) REFERENCES directory_returns(operation,ordinal) ON DELETE CASCADE);
        CREATE INDEX IF NOT EXISTS directory_members_parent ON directory_members(operation,parent);
        CREATE TABLE IF NOT EXISTS directory_archives (
        operation INTEGER NOT NULL, parent INTEGER NOT NULL, entry INTEGER NOT NULL REFERENCES garbage(id) ON DELETE CASCADE,
        PRIMARY KEY(operation,parent,entry),
            FOREIGN KEY(operation,parent) REFERENCES directory_returns(operation,ordinal) ON DELETE CASCADE);").map_err(db_error)?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS directory_scopes (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        operation INTEGER NOT NULL REFERENCES operations(id) ON DELETE CASCADE,
        historical_before INTEGER);
        CREATE TABLE IF NOT EXISTS directory_sequences (
        operation INTEGER PRIMARY KEY REFERENCES operations(id) ON DELETE CASCADE,
        next_ordinal INTEGER NOT NULL);",
        )
        .map_err(db_error)?;
    if version < 10 {
        connection.execute_batch("ALTER TABLE directory_returns ADD COLUMN scope INTEGER REFERENCES directory_scopes(id);
            ALTER TABLE directory_members ADD COLUMN scope INTEGER REFERENCES directory_scopes(id);
            ALTER TABLE directory_archives ADD COLUMN scope INTEGER REFERENCES directory_scopes(id);").map_err(db_error)?;
        let roots: Vec<(u64, u64)> = {
            let mut statement = connection
                .prepare_cached(
                    "SELECT operation,ordinal FROM directory_returns ORDER BY operation,ordinal",
                )
                .map_err(db_error)?;
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(db_error)?
                .collect::<Result<_, _>>()
                .map_err(db_error)?
        };
        for (operation, ordinal) in roots {
            let inherited = member_scope(connection, operation, ordinal)?;
            let scope = match inherited {
                Some(scope) => scope,
                None => new_scope(connection, operation, Some(ordinal))?,
            };
            connection
                .execute_cached(
                    "UPDATE directory_returns SET scope=?3 WHERE operation=?1 AND ordinal=?2",
                    params![operation, ordinal, scope.id],
                )
                .map_err(db_error)?;
            connection
                .execute_cached(
                    "UPDATE directory_members SET scope=?3 WHERE operation=?1 AND parent=?2",
                    params![operation, ordinal, scope.id],
                )
                .map_err(db_error)?;
            connection
                .execute_cached(
                    "UPDATE directory_archives SET scope=?3 WHERE operation=?1 AND parent=?2",
                    params![operation, ordinal, scope.id],
                )
                .map_err(db_error)?;
        }
        // A completed nested marker must not erase the archive's scope. Later
        // passes can encounter the same retained stage again under its old name.
        connection.execute_batch("CREATE TABLE directory_archives_v10 (
            operation INTEGER NOT NULL REFERENCES operations(id) ON DELETE CASCADE,
            parent INTEGER NOT NULL,
            entry INTEGER NOT NULL REFERENCES garbage(id) ON DELETE CASCADE,
            scope INTEGER REFERENCES directory_scopes(id),
            PRIMARY KEY(operation,parent,entry));
            INSERT INTO directory_archives_v10 SELECT operation,parent,entry,scope FROM directory_archives;
            DROP TABLE directory_archives;
            ALTER TABLE directory_archives_v10 RENAME TO directory_archives;").map_err(db_error)?;
    }
    if version < 13 {
        connection.execute_batch("CREATE TABLE directory_members_v13 (
            operation INTEGER NOT NULL, ordinal INTEGER NOT NULL, parent INTEGER NOT NULL,
            scope INTEGER REFERENCES directory_scopes(id), PRIMARY KEY(operation,ordinal),
            FOREIGN KEY(operation,ordinal) REFERENCES changes(operation,ordinal) ON DELETE CASCADE,
            FOREIGN KEY(operation,parent) REFERENCES directory_returns(operation,ordinal) ON DELETE CASCADE);
            INSERT INTO directory_members_v13 SELECT operation,ordinal,parent,scope FROM directory_members;
            DROP TABLE directory_members; ALTER TABLE directory_members_v13 RENAME TO directory_members;
            CREATE INDEX directory_members_parent ON directory_members(operation,parent);").map_err(db_error)?;
    }
    connection.execute_batch("CREATE INDEX IF NOT EXISTS directory_archives_scope ON directory_archives(operation,scope,entry);
        CREATE TABLE IF NOT EXISTS moved_directory_roots (
            operation INTEGER NOT NULL, ordinal INTEGER NOT NULL,
            current BLOB NOT NULL, identity TEXT NOT NULL,
            PRIMARY KEY(operation,ordinal),
            FOREIGN KEY(operation,ordinal) REFERENCES changes(operation,ordinal) ON DELETE CASCADE);
        CREATE INDEX IF NOT EXISTS moved_directory_roots_source ON moved_directory_roots(operation,current,identity);").map_err(db_error)?;
    if !indexed {
        let mut statement = connection
            .prepare_cached("SELECT operation,ordinal,body FROM changes")
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
            index_moved(connection, operation, ordinal, &decode::<Change>(&body)?)?;
        }
    }
    Ok(())
}

pub(super) fn index_moved(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    let inner = match change {
        Change::DirectoryMerge { change, .. } => change.as_ref(),
        change => change,
    };
    if let Change::RemovedDirectory {
        moved: Some(moved), ..
    } = inner
    {
        connection.execute_cached("INSERT INTO moved_directory_roots(operation,ordinal,current,identity) VALUES(?1,?2,?3,?4)
            ON CONFLICT(operation,ordinal) DO UPDATE SET current=excluded.current,identity=excluded.identity",
            params![operation,ordinal,paths::key(&moved.current),encode(&moved.identity)?]).map_err(db_error)?;
    } else {
        connection
            .execute_cached(
                "DELETE FROM moved_directory_roots WHERE operation=?1 AND ordinal=?2",
                params![operation, ordinal],
            )
            .map_err(db_error)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Scope {
    id: u64,
    historical_before: Option<u64>,
}

enum Expansion<'a> {
    Existing,
    Additional,
    Destination(&'a Stage, Option<&'a undo::Relocation>),
}

fn new_scope(
    connection: &Connection,
    operation: u64,
    historical_before: Option<u64>,
) -> io::Result<Scope> {
    connection
        .execute_cached(
            "INSERT INTO directory_scopes(operation,historical_before) VALUES(?1,?2)",
            params![operation, historical_before],
        )
        .map_err(db_error)?;
    Ok(Scope {
        id: u64::try_from(connection.last_insert_rowid()).map_err(io::Error::other)?,
        historical_before,
    })
}

fn member_scope(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
) -> io::Result<Option<Scope>> {
    let row: Option<(Option<u64>,Option<u64>)> = connection.query_row_cached("SELECT m.scope,s.historical_before FROM directory_members m LEFT JOIN directory_scopes s ON s.id=m.scope WHERE m.operation=?1 AND m.ordinal=?2",params![operation,ordinal],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(db_error)?;
    row.map(|(id, historical_before)| {
        Ok(Scope {
            id: id.ok_or_else(|| invalid("directory member has no scope"))?,
            historical_before,
        })
    })
    .transpose()
}

pub(super) fn history_before(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
) -> io::Result<Option<u64>> {
    Ok(member_scope(connection, operation, ordinal)?
        .map_or(Some(ordinal), |scope| scope.historical_before))
}

fn parents(connection: &Connection, id: u64, ordinal: u64) -> io::Result<Vec<u64>> {
    let mut statement = connection.prepare_cached("WITH RECURSIVE parents(ordinal) AS (
        SELECT parent FROM directory_members WHERE operation=?1 AND ordinal=?2
        UNION ALL SELECT m.parent FROM directory_members m JOIN parents p ON m.ordinal=p.ordinal WHERE m.operation=?1)
        SELECT ordinal FROM parents").map_err(db_error)?;
    statement
        .query_map(params![id, ordinal], |row| row.get(0))
        .map_err(db_error)?
        .collect::<Result<_, _>>()
        .map_err(db_error)
}

pub(super) fn owns_archive(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    entry: u64,
) -> io::Result<bool> {
    let Some(scope) = member_scope(connection, id, ordinal)? else {
        return Ok(false);
    };
    connection.query_row_cached("SELECT EXISTS(SELECT 1 FROM directory_archives WHERE operation=?1 AND scope=?2 AND entry=?3)",params![id,scope.id,entry],|row|row.get(0)).map_err(db_error)
}

impl Store {
    pub fn begin_moved_directory_return(
        &mut self,
        id: u64,
        ordinal: u64,
        marker: &Change,
    ) -> io::Result<()> {
        self.register_moved_directory_return(id, ordinal, marker, None, None)
    }

    pub fn prepare_moved_directory_destination(
        &mut self,
        id: u64,
        ordinal: u64,
        marker: &Change,
        stage: &Stage,
        relocation: Option<&undo::Relocation>,
    ) -> io::Result<()> {
        self.register_moved_directory_return(id, ordinal, marker, Some(stage), relocation)
    }

    fn register_moved_directory_return(
        &mut self,
        id: u64,
        ordinal: u64,
        marker: &Change,
        stage: Option<&Stage>,
        relocation: Option<&undo::Relocation>,
    ) -> io::Result<()> {
        self.ensure_location()?;
        self.require_directory_parents(id, ordinal)?;
        if let Some(stage) = stage {
            let (source, destination, source_identity, destination_identity) =
                directory_return::roots(marker)?;
            directory_return::require_directory(source, source_identity)?;
            validate_stage(Some(stage), destination)?;
            if stage.payload_identity != Some(destination_identity)
                || !stage.identity.matches(&stage.directory)
                || !destination_identity.matches(&stage.payload())
            {
                return Err(invalid("invalid prepared moved-folder destination"));
            }
        } else {
            directory_return::require_roots(marker)?;
        }
        let moved = directory_return::moved(marker)
            .ok_or_else(|| invalid("expected a moved directory frame"))?;
        if moved.first > ordinal
            || moved
                .created
                .is_some_and(|created| created < moved.first || created >= ordinal)
        {
            return Err(invalid("invalid moved directory scope"));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(owner) = moved.created {
            match stored_change(&transaction, id, owner)? {
                Change::CreatedFolder { path, identity, .. }
                    if path == moved.current && identity == moved.identity => {}
                _ => {
                    return Err(invalid(
                        "the moved directory has no matching owned destination",
                    ));
                }
            }
        }
        let scope = match member_scope(&transaction, id, ordinal)? {
            Some(scope) => scope,
            None => new_scope(&transaction, id, Some(moved.first))?,
        };
        let mut staged = marker.clone();
        if let Some(relocation) = relocation
            && let Change::DirectoryMerge { change, .. } = &mut staged
            && let Change::RemovedDirectory { path, .. } = change.as_mut()
        {
            *path = relocation.from.clone();
        }
        transaction
            .execute_cached(
                "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal, encode(&staged)?],
            )
            .map_err(db_error)?;
        paths::index_change(&transaction, id, ordinal, &staged)?;
        references::index(&transaction, id, ordinal, &staged, false)?;
        transaction
            .execute_cached(
                "INSERT INTO directory_returns(operation,ordinal,scope) VALUES(?1,?2,?3)",
                params![id, ordinal, scope.id],
            )
            .map_err(db_error)?;
        transaction.execute_cached("INSERT INTO directory_members(operation,ordinal,parent,scope)
            SELECT operation,ordinal,?3,?4 FROM changes WHERE operation=?1 AND ordinal>=?2 AND ordinal<?3
            ON CONFLICT(operation,ordinal) DO UPDATE SET parent=excluded.parent,scope=excluded.scope",params![id,moved.first,ordinal,scope.id]).map_err(db_error)?;
        context::returned(&transaction, id, ordinal, marker)?;
        if let Some(relocation) = relocation {
            relocate_return(
                &transaction,
                id,
                ordinal,
                relocation,
                &BTreeSet::new(),
                Some(ordinal),
            )?;
            transaction
                .execute_cached(
                    "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                    params![id, ordinal, encode(marker)?],
                )
                .map_err(db_error)?;
            paths::index_change(&transaction, id, ordinal, marker)?;
            references::index(&transaction, id, ordinal, marker, false)?;
        }
        if let Some(stage) = stage {
            let pending = Pending::DirectoryDestination {
                id,
                ordinal,
                change: marker.clone(),
                stage: stage.clone(),
                relocation: relocation.cloned(),
            };
            context::prepare(&transaction, self.identity, self.lane, &pending)?;
            transaction
                .execute_cached(
                    "INSERT OR REPLACE INTO pending VALUES(1,?1)",
                    [encode(&pending)?],
                )
                .map_err(db_error)?;
        }
        transaction.commit().map_err(db_error)
    }

    pub fn directory_source_cleanup(&self, id: u64, ordinal: u64) -> io::Result<bool> {
        let parent: Option<u64> = self
            .connection
            .query_row_cached(
                "SELECT parent FROM directory_members WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        Ok(parent
            .map(|parent| stored_change(&self.connection, id, parent))
            .transpose()?
            .as_ref()
            .and_then(directory_return::moved)
            .and_then(|moved| moved.created)
            == Some(ordinal))
    }

    pub fn ready_moved_directory_return(
        &self,
        id: u64,
        deferred: &std::collections::BTreeSet<u64>,
    ) -> io::Result<bool> {
        let mut statement=self.connection.prepare_cached("SELECT r.ordinal,c.body FROM directory_returns r JOIN changes c ON c.operation=r.operation AND c.ordinal=r.ordinal WHERE r.operation=?1 ORDER BY r.ordinal DESC").map_err(db_error)?;
        let rows = statement
            .query_map([id], |row| {
                Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(db_error)?;
        for row in rows {
            let (ordinal, body) = row.map_err(db_error)?;
            if !deferred.contains(&ordinal)
                && directory_return::moved(&decode::<Change>(&body)?).is_some()
                && !self.directory_children_remaining(id, ordinal)?
                && !self.moved_directory_source_in_use(id, ordinal)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn moved_directory_source_in_use(&self, id: u64, ordinal: u64) -> io::Result<bool> {
        if directory_return::moved(&stored_change(&self.connection, id, ordinal)?)
            .is_none_or(|moved| moved.created.is_none())
        {
            return Ok(false);
        }
        self.connection.query_row_cached("SELECT EXISTS(SELECT 1 FROM moved_directory_roots own
            JOIN moved_directory_roots other ON other.operation=own.operation AND other.current=own.current AND other.identity=own.identity
            WHERE own.operation=?1 AND own.ordinal=?2 AND other.ordinal!=own.ordinal)",
            params![id,ordinal], |row| row.get(0)).map_err(db_error)
    }

    pub fn expand_directory_file(
        &mut self,
        id: u64,
        ordinal: u64,
        cleanup: &Change,
        returned: &Change,
    ) -> io::Result<()> {
        self.ensure_location()?;
        self.require_directory_parents(id, ordinal)?;
        validate_change(cleanup)?;
        validate_change(returned)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let (parent,scope,bindings):(u64,u64,Option<String>)=transaction.query_row_cached("SELECT m.parent,m.scope,c.bindings FROM directory_members m JOIN changes c ON c.operation=m.operation AND c.ordinal=m.ordinal WHERE m.operation=?1 AND m.ordinal=?2",params![id,ordinal],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(db_error)?;
        let available: u64 = transaction
            .query_row_cached(
                "SELECT COALESCE(MAX(ordinal)+1,0) FROM changes WHERE operation=?1",
                [id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        let next: u64 = transaction
            .query_row_cached(
                "SELECT next_ordinal FROM directory_sequences WHERE operation=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?
            .unwrap_or(available)
            .max(available);
        let returned_ordinal = next
            .checked_add(1)
            .ok_or_else(|| invalid("directory return is too large"))?;
        let after = next
            .checked_add(2)
            .ok_or_else(|| invalid("directory return is too large"))?;
        for (step, change, bindings) in [
            (next, cleanup, Some(encode(&context::Bindings::default())?)),
            (returned_ordinal, returned, bindings),
        ] {
            transaction
                .execute_cached(
                    "INSERT INTO changes(operation,ordinal,body,bindings) VALUES(?1,?2,?3,?4)",
                    params![id, step, encode(change)?, bindings],
                )
                .map_err(db_error)?;
            paths::index_change(&transaction, id, step, change)?;
            references::index(&transaction, id, step, change, false)?;
            transaction.execute_cached("INSERT INTO directory_members(operation,ordinal,parent,scope) VALUES(?1,?2,?3,?4)",params![id,step,parent,scope]).map_err(db_error)?;
        }
        transaction
            .execute_cached(
                "DELETE FROM changes WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal],
            )
            .map_err(db_error)?;
        transaction.execute_cached("INSERT INTO directory_sequences(operation,next_ordinal) VALUES(?1,?2) ON CONFLICT(operation) DO UPDATE SET next_ordinal=excluded.next_ordinal",params![id,after]).map_err(db_error)?;
        transaction.commit().map_err(db_error)
    }

    pub fn defer_directory_saved(
        &mut self,
        id: u64,
        ordinal: u64,
        marker: &Change,
    ) -> io::Result<()> {
        self.ensure_location()?;
        self.require_directory_parents(id, ordinal)?;
        if self.directory_children_remaining(id, ordinal)? {
            return Err(invalid("directory return still has pending children"));
        }
        let (source, _, identity, _) = directory_return::roots(marker)?;
        let retained = marker
            .retained()
            .ok_or_else(|| invalid("missing deferred destination"))?;
        let remainder = Change::RetainedOnly {
            path: source.to_owned(),
            retained: retained.clone(),
            replaced: Some(identity),
        };
        validate_change(&remainder)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        context::returned(&transaction, id, ordinal, marker)?;
        transaction
            .execute_cached(
                "DELETE FROM directory_returns WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal],
            )
            .map_err(db_error)?;
        transaction
            .execute_cached(
                "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal, encode(&remainder)?],
            )
            .map_err(db_error)?;
        paths::index_change(&transaction, id, ordinal, &remainder)?;
        references::index(&transaction, id, ordinal, &remainder, false)?;
        clear_intent(&transaction, self.lane)?;
        transaction.commit().map_err(db_error)
    }

    pub fn finish_directory_return(&mut self, id: u64, ordinal: u64) -> io::Result<Vec<PathBuf>> {
        self.ensure_location()?;
        self.require_directory_parents(id, ordinal)?;
        let label = self
            .label(id)?
            .ok_or_else(|| invalid("directory return has no operation"))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let rows: Vec<(u64, String)> = {
            let mut statement = transaction.prepare_cached("WITH RECURSIVE branch(ordinal) AS (
                SELECT ?2 UNION ALL SELECT m.ordinal FROM directory_members m JOIN branch b ON m.parent=b.ordinal WHERE m.operation=?1)
                SELECT c.ordinal,c.body FROM changes c JOIN branch b ON b.ordinal=c.ordinal WHERE c.operation=?1 ORDER BY c.ordinal DESC").map_err(db_error)?;
            statement
                .query_map(params![id, ordinal], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(db_error)?
                .collect::<Result<_, _>>()
                .map_err(db_error)?
        };
        let mut previous = Vec::new();
        for (step, body) in &rows {
            let change: Change = decode(body)?;
            if let Some(mut record) = recovery::Record::from_change(
                &change,
                format!("Preserved after finishing a partial Merge during Undo {label}"),
            ) {
                record.retained.recovery = true;
                previous.push(change.forward_path().to_owned());
                archive(&transaction, Some(&record))?;
            }
            if matches!(change, Change::DirectoryMerge { .. }) {
                let mut chain = parents(&transaction, id, *step)?;
                chain.push(*step);
                let owned = chain.into_iter().try_fold(true, |owned, root| {
                    let marker = stored_change(&transaction, id, root)?;
                    let (_, destination, _, identity) = directory_return::roots(&marker)?;
                    Ok::<_, io::Error>(
                        owned
                            && fs::symlink_metadata(destination).is_ok_and(|metadata| {
                                metadata.is_dir() && Identity::from_metadata(&metadata) == identity
                            }),
                    )
                })?;
                if owned {
                    context::returned(&transaction, id, *step, &change)?;
                }
            }
        }
        for (step, _) in rows {
            complete_reversal(&transaction, id, step, self.lane)?;
        }
        transaction.commit().map_err(db_error)?;
        Ok(previous)
    }

    pub fn directory_roots(&self, id: u64, ordinal: u64) -> io::Result<Vec<(u64, Change)>> {
        let mut roots = parents(&self.connection, id, ordinal)?;
        let marker: bool = self
            .connection
            .query_row_cached(
                "SELECT EXISTS(SELECT 1 FROM directory_returns WHERE operation=?1 AND ordinal=?2)",
                params![id, ordinal],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if marker {
            roots.push(ordinal);
        }
        roots.sort_unstable();
        roots
            .into_iter()
            .map(|root| Ok((root, stored_change(&self.connection, id, root)?)))
            .collect()
    }

    pub fn keep_directory_remainder(
        &mut self,
        id: u64,
        ordinal: u64,
        marker: &Change,
    ) -> io::Result<()> {
        self.ensure_location()?;
        self.require_directory_parents(id, ordinal)?;
        directory_return::require_roots(marker)?;
        if self.directory_children_remaining(id, ordinal)? {
            return Err(invalid("directory return still has pending children"));
        }
        let label = self
            .label(id)?
            .ok_or_else(|| invalid("directory return has no operation"))?;
        let protected = recovery::Record::from_change(
            marker,
            format!("Preserved after keeping remaining items during Undo {label}"),
        )
        .map(|mut record| {
            record.retained.recovery = true;
            record
        });
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        context::returned(&transaction, id, ordinal, marker)?;
        archive(&transaction, protected.as_ref())?;
        if let Some(owner) = directory_return::moved(marker).and_then(|moved| moved.created) {
            complete_reversal(&transaction, id, owner, self.lane)?;
        }
        complete_reversal(&transaction, id, ordinal, self.lane)?;
        transaction.commit().map_err(db_error)
    }

    pub fn require_directory_parents(&self, id: u64, ordinal: u64) -> io::Result<()> {
        for parent in parents(&self.connection, id, ordinal)? {
            directory_return::require_roots(&stored_change(&self.connection, id, parent)?)?;
        }
        Ok(())
    }

    pub(in crate::operations) fn owned_return_parent(
        &self,
        id: u64,
        ordinal: u64,
        path: &Path,
    ) -> io::Result<Option<Identity>> {
        Ok(
            return_parent(&self.connection, id, ordinal, path)?.and_then(|parent| {
                match parent.access {
                    StageParentAccess::Created(identity) => Some(identity),
                    StageParentAccess::Existing => None,
                }
            }),
        )
    }

    pub(in crate::operations) fn permit_owned_return_parents(
        &mut self,
        id: u64,
        ordinal: u64,
    ) -> io::Result<()> {
        for parent in parents(&self.connection, id, ordinal)? {
            let marker = stored_change(&self.connection, id, parent)?;
            if matches!(
                marker,
                Change::DirectoryMerge {
                    destination_mode: Some(_),
                    ..
                }
            ) {
                let (_, destination, _, identity) = directory_return::roots(&marker)?;
                self.permit_directory(destination, identity)?;
            }
        }
        Ok(())
    }

    pub fn directory_children_remaining(&self, id: u64, ordinal: u64) -> io::Result<bool> {
        let cleanup = directory_return::moved(&stored_change(&self.connection, id, ordinal)?)
            .and_then(|moved| moved.created);
        self.connection
            .query_row_cached(
                "SELECT EXISTS(SELECT 1 FROM directory_members WHERE operation=?1 AND parent=?2 AND (?3 IS NULL OR ordinal!=?3))",
                params![id, ordinal,cleanup],
                |row| row.get(0),
            )
            .map_err(db_error)
    }

    pub fn begin_directory_return(
        &mut self,
        id: u64,
        ordinal: u64,
        marker: &Change,
        children: &[Change],
    ) -> io::Result<()> {
        self.expand_directory_return(id, ordinal, marker, children, Expansion::Existing)
    }

    pub fn prepare_directory_destination(
        &mut self,
        id: u64,
        ordinal: u64,
        marker: &Change,
        children: &[Change],
        stage: &Stage,
        relocation: Option<&undo::Relocation>,
    ) -> io::Result<()> {
        if !matches!(self.lane, IntentLane::Primary) {
            return Err(invalid(
                "directory preparation requires the primary journal",
            ));
        }
        self.expand_directory_return(
            id,
            ordinal,
            marker,
            children,
            Expansion::Destination(stage, relocation),
        )
    }

    pub fn add_directory_items(
        &mut self,
        id: u64,
        ordinal: u64,
        marker: &Change,
        children: &[Change],
    ) -> io::Result<()> {
        self.expand_directory_return(id, ordinal, marker, children, Expansion::Additional)
    }

    fn expand_directory_return(
        &mut self,
        id: u64,
        ordinal: u64,
        marker: &Change,
        children: &[Change],
        expansion: Expansion<'_>,
    ) -> io::Result<()> {
        self.ensure_location()?;
        validate_change(marker)?;
        let (source, destination, _, _) = directory_return::roots(marker)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Expansion::Destination(_, Some(relocation)) = expansion {
            relocate(&transaction, id, ordinal, relocation, &BTreeSet::new())?;
        }
        let scope = if matches!(expansion, Expansion::Additional) {
            new_scope(&transaction, id, None)?
        } else {
            match member_scope(&transaction, id, ordinal)? {
                Some(scope) => scope,
                None => new_scope(&transaction, id, Some(ordinal))?,
            }
        };
        transaction
            .execute_cached(
                "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal, encode(marker)?],
            )
            .map_err(db_error)?;
        paths::index_change(&transaction, id, ordinal, marker)?;
        references::index(&transaction, id, ordinal, marker, false)?;
        transaction
            .execute_cached(
                "INSERT INTO directory_returns(operation,ordinal,scope) VALUES(?1,?2,?3) ON CONFLICT(operation,ordinal) DO NOTHING",
                params![id, ordinal,scope.id],
            )
            .map_err(db_error)?;
        let next_available: u64 = transaction
            .query_row_cached(
                "SELECT COALESCE(MAX(ordinal)+1,0) FROM changes WHERE operation=?1",
                [id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        let mut next = transaction
            .query_row_cached(
                "SELECT next_ordinal FROM directory_sequences WHERE operation=?1",
                [id],
                |row| row.get::<_, u64>(0),
            )
            .optional()
            .map_err(db_error)?
            .unwrap_or(next_available)
            .max(next_available);
        // Descending Undo order returns the lexically first child first.
        for child in children.iter().rev() {
            validate_change(child)?;
            let bindings = encode(&match scope.historical_before {
                Some(before) => context::capture(&transaction, id, before, child)?,
                None => context::Bindings::default(),
            })?;
            transaction
                .execute_cached(
                    "INSERT INTO changes(operation,ordinal,body,bindings) VALUES(?1,?2,?3,?4)",
                    params![id, next, encode(child)?, bindings],
                )
                .map_err(db_error)?;
            paths::index_change(&transaction, id, next, child)?;
            references::index(&transaction, id, next, child, false)?;
            transaction
                .execute_cached(
                    "INSERT INTO directory_members(operation,ordinal,parent,scope) VALUES(?1,?2,?3,?4)",
                    params![id, next, ordinal,scope.id],
                )
                .map_err(db_error)?;
            next = next
                .checked_add(1)
                .ok_or_else(|| invalid("directory return is too large"))?;
        }
        transaction.execute_cached("INSERT INTO directory_sequences(operation,next_ordinal) VALUES(?1,?2) ON CONFLICT(operation) DO UPDATE SET next_ordinal=excluded.next_ordinal",params![id,next]).map_err(db_error)?;
        let records: Vec<(u64, String)> = {
            let mut statement = transaction
                .prepare_cached("SELECT id,body FROM garbage ORDER BY id")
                .map_err(db_error)?;
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(db_error)?
                .collect::<Result<_, _>>()
                .map_err(db_error)?
        };
        for (entry, body) in records {
            let mut record = recovery::Record::decode(&body)?;
            if record.retained.stage.directory.starts_with(source) {
                let other_scope: bool = transaction.query_row_cached("SELECT EXISTS(SELECT 1 FROM directory_archives WHERE operation=?1 AND entry=?2 AND scope!=?3)",params![id,entry,scope.id],|row|row.get(0)).map_err(db_error)?;
                if other_scope {
                    continue;
                }
                if let Some(context) = &mut record.context {
                    undo::Relocation {
                        from: source.to_owned(),
                        to: destination.to_owned(),
                        subtree: true,
                    }
                    .apply(&mut context.original);
                }
                transaction
                    .execute_cached(
                        "UPDATE garbage SET body=?2 WHERE id=?1",
                        params![entry, encode(&record)?],
                    )
                    .map_err(db_error)?;
                paths::index_record(&transaction, entry, &record)?;
                // Bind the logical destination to the most specific active
                // container, preserving the scope after that container finishes.
                transaction.execute_cached("DELETE FROM directory_archives WHERE operation=?1 AND entry=?2 AND scope=?3",params![id,entry,scope.id]).map_err(db_error)?;
                transaction
                    .execute_cached(
                        "INSERT INTO directory_archives(operation,parent,entry,scope) VALUES(?1,?2,?3,?4)",
                        params![id, ordinal, entry,scope.id],
                    )
                    .map_err(db_error)?;
            }
        }
        if let Expansion::Destination(stage, relocation) = expansion {
            let pending = Pending::DirectoryDestination {
                id,
                ordinal,
                change: marker.clone(),
                stage: stage.clone(),
                relocation: relocation.cloned(),
            };
            context::prepare(&transaction, self.identity, self.lane, &pending)?;
            transaction
                .execute_cached(
                    "INSERT OR REPLACE INTO pending VALUES(1,?1)",
                    [encode(&pending)?],
                )
                .map_err(db_error)?;
        } else {
            clear_intent(&transaction, self.lane)?;
        }
        transaction.commit().map_err(db_error)
    }

    pub fn cancel_directory_destination(
        &mut self,
        id: u64,
        ordinal: u64,
        marker: &Change,
        relocation: Option<&undo::Relocation>,
    ) -> io::Result<()> {
        if directory_return::moved(marker).is_some() {
            return self.cancel_moved_directory_destination(id, ordinal, marker, relocation);
        }
        let Change::DirectoryMerge { change, .. } = marker else {
            return Err(invalid("invalid prepared directory return"));
        };
        self.ensure_location()?;
        let mut original = change.as_ref().clone();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(relocation) = relocation {
            let reversed = undo::Relocation {
                from: relocation.to.clone(),
                to: relocation.from.clone(),
                subtree: relocation.subtree,
            };
            original.relocate(&reversed);
            relocate(&transaction, id, ordinal, &reversed, &BTreeSet::new())?;
        }
        transaction.execute_cached("DELETE FROM changes WHERE operation=?1 AND ordinal IN(SELECT ordinal FROM directory_members WHERE operation=?1 AND parent=?2)",params![id,ordinal]).map_err(db_error)?;
        transaction
            .execute_cached(
                "DELETE FROM directory_returns WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal],
            )
            .map_err(db_error)?;
        transaction
            .execute_cached(
                "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal, encode(&original)?],
            )
            .map_err(db_error)?;
        paths::index_change(&transaction, id, ordinal, &original)?;
        references::index(&transaction, id, ordinal, &original, false)?;
        clear_intent(&transaction, self.lane)?;
        transaction.commit().map_err(db_error)
    }

    fn cancel_moved_directory_destination(
        &mut self,
        id: u64,
        ordinal: u64,
        marker: &Change,
        relocation: Option<&undo::Relocation>,
    ) -> io::Result<()> {
        let Change::DirectoryMerge { change, .. } = marker else {
            return Err(invalid("invalid directory preparation"));
        };
        self.ensure_location()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut original = change.as_ref().clone();
        if let Some(relocation) = relocation {
            let reversed = undo::Relocation {
                from: relocation.to.clone(),
                to: relocation.from.clone(),
                subtree: true,
            };
            original.relocate(&reversed);
            relocate_return(
                &transaction,
                id,
                ordinal,
                &reversed,
                &BTreeSet::new(),
                Some(ordinal),
            )?;
        }
        context::restore_source_context(&transaction, id, ordinal, &original)?;
        let parent: Option<(u64, u64)> = transaction
            .query_row_cached(
                "SELECT parent,scope FROM directory_members WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        if let Some((parent, scope)) = parent {
            transaction.execute_cached("UPDATE directory_members SET parent=?3,scope=?4 WHERE operation=?1 AND parent=?2",params![id,ordinal,parent,scope]).map_err(db_error)?;
        } else {
            transaction
                .execute_cached(
                    "DELETE FROM directory_members WHERE operation=?1 AND parent=?2",
                    params![id, ordinal],
                )
                .map_err(db_error)?;
        }
        transaction
            .execute_cached(
                "DELETE FROM directory_returns WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal],
            )
            .map_err(db_error)?;
        transaction
            .execute_cached(
                "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal, encode(&original)?],
            )
            .map_err(db_error)?;
        paths::index_change(&transaction, id, ordinal, &original)?;
        references::index(&transaction, id, ordinal, &original, false)?;
        clear_intent(&transaction, self.lane)?;
        transaction.commit().map_err(db_error)
    }
}

// These roots were captured by the directory-return plan before child mutations.
pub(super) fn return_parent(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    path: &Path,
) -> io::Result<Option<saved_stage::Parent>> {
    for parent in parents(connection, id, ordinal)? {
        let marker = stored_change(connection, id, parent)?;
        if let Change::DirectoryMerge {
            destination_mode,
            destination_original,
            ..
        } = &marker
        {
            let (_, destination, _, identity) = directory_return::roots(&marker)?;
            if destination == path {
                return Ok(Some(saved_stage::Parent {
                    original: *destination_original,
                    identity,
                    access: if destination_mode.is_some() {
                        StageParentAccess::Created(identity)
                    } else {
                        StageParentAccess::Existing
                    },
                }));
            }
        }
    }
    Ok(None)
}
