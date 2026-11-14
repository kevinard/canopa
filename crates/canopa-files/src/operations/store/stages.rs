//! Physical saved-container ownership, independent of historical namespace paths.
use super::*;

pub(super) fn create(connection: &Connection, version: u32) -> io::Result<()> {
    connection.execute_batch("CREATE TABLE IF NOT EXISTS saved_stage_changes (
        stage TEXT NOT NULL, operation INTEGER NOT NULL, ordinal INTEGER NOT NULL,
        PRIMARY KEY(stage,operation,ordinal),
        FOREIGN KEY(operation,ordinal) REFERENCES changes(operation,ordinal) ON DELETE CASCADE);
        CREATE INDEX IF NOT EXISTS saved_stage_changes_owner ON saved_stage_changes(operation,ordinal);
        CREATE TABLE IF NOT EXISTS saved_stage_records (
        stage TEXT NOT NULL, entry INTEGER NOT NULL REFERENCES garbage(id) ON DELETE CASCADE,
        PRIMARY KEY(stage,entry));
        CREATE INDEX IF NOT EXISTS saved_stage_records_owner ON saved_stage_records(entry);").map_err(db_error)?;
    if version < 14 {
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
            index_change(connection, operation, ordinal, &decode(&body)?)?;
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
    }
    Ok(())
}

pub(super) fn index_change(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    connection
        .execute_cached(
            "DELETE FROM saved_stage_changes WHERE operation=?1 AND ordinal=?2",
            params![operation, ordinal],
        )
        .map_err(db_error)?;
    if let Some(retained) = change.retained() {
        connection
            .execute_cached(
                "INSERT INTO saved_stage_changes VALUES(?1,?2,?3)",
                params![encode(&retained.stage.identity)?, operation, ordinal],
            )
            .map_err(db_error)?;
    }
    Ok(())
}

pub(super) fn index_record(
    connection: &Connection,
    entry: u64,
    record: &recovery::Record,
) -> io::Result<()> {
    connection
        .execute_cached("DELETE FROM saved_stage_records WHERE entry=?1", [entry])
        .map_err(db_error)?;
    connection
        .execute_cached(
            "INSERT INTO saved_stage_records VALUES(?1,?2)",
            params![encode(&record.retained.stage.identity)?, entry],
        )
        .map_err(db_error)?;
    Ok(())
}

pub(super) fn owners(
    connection: &Connection,
    identity: Identity,
) -> io::Result<Vec<recovery::Key>> {
    let mut owners = Vec::new();
    any_owner(connection, identity, |owner| {
        owners.push(owner);
        Ok(false)
    })?;
    Ok(owners)
}

fn any_owner(
    connection: &Connection,
    identity: Identity,
    mut matches: impl FnMut(recovery::Key) -> io::Result<bool>,
) -> io::Result<bool> {
    let key = encode(&identity)?;
    let mut statement = connection.prepare("SELECT operation,ordinal FROM saved_stage_changes WHERE stage=?1 ORDER BY operation,ordinal").map_err(db_error)?;
    let rows = statement
        .query_map([&key], |row| {
            Ok(recovery::Key::Active {
                operation: row.get(0)?,
                ordinal: row.get(1)?,
            })
        })
        .map_err(db_error)?;
    for row in rows {
        if matches(row.map_err(db_error)?)? {
            return Ok(true);
        }
    }
    let mut statement = connection
        .prepare("SELECT entry FROM saved_stage_records WHERE stage=?1 ORDER BY entry")
        .map_err(db_error)?;
    let rows = statement
        .query_map([&key], |row| Ok(recovery::Key::Archived(row.get(0)?)))
        .map_err(db_error)?;
    for row in rows {
        if matches(row.map_err(db_error)?)? {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Store {
    /// Stream only catalog owners of this physical saved container. A caller's
    /// selected-version check decides whether one is the same recoverable data.
    pub(in crate::operations) fn find_saved_recovery(
        &self,
        identity: Identity,
        mut accepts: impl FnMut(&recovery::Record) -> io::Result<bool>,
    ) -> io::Result<Option<recovery::Record>> {
        let mut found = None;
        any_owner(&self.connection, identity, |owner| {
            if let Some(record) = self.recovery_record(&owner)?
                && accepts(&record)?
            {
                found = Some(record);
                return Ok(true);
            }
            Ok(false)
        })?;
        Ok(found)
    }

    /// Expire only this archive's ownership when identified data is still shared.
    /// Eligibility is checked before discarding even redundant catalog metadata.
    pub(in crate::operations) fn expire_archived_retained(
        &mut self,
        entry: u64,
        record: &recovery::Record,
    ) -> io::Result<()> {
        source_retention::require_released(&self.connection, record.retained.stage.identity)?;
        let present = require_retained_expiry(&record.retained)?;
        if self.expiry_is_shared(entry, record, present)? {
            Ok(())
        } else {
            self.begin_expiry(entry, record)
        }
    }

    pub(in crate::operations) fn expiry_is_shared(
        &self,
        entry: u64,
        record: &recovery::Record,
        present: bool,
    ) -> io::Result<bool> {
        any_owner(&self.connection, record.retained.stage.identity, |owner| {
            if matches!(owner, recovery::Key::Archived(other) if other==entry) {
                return Ok(false);
            }
            if let Some(other) = self.recovery_record(&owner)?
                && other.retained.stage.identity == record.retained.stage.identity
            {
                if !present {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "shared retained data is unavailable; its recovery records are preserved",
                    ));
                }
                // Lexical parent aliases can name the same identified stage.
                // Retire this metadata only when the remaining owner really
                // reaches that container. An unavailable owner's identity can
                // protect bytes but must not discard their available record.
                if other
                    .retained
                    .stage
                    .identity
                    .matches(&other.retained.stage.directory)
                {
                    return Ok(true);
                }
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "another saved-data owner is unavailable; its available recovery record is preserved",
                ));
            }
            Ok(false)
        })
    }

    pub(in crate::operations) fn saved_move_completed(
        &self,
        receipt: &saved_stage::Receipt,
        source: &Path,
        destination: &Path,
    ) -> io::Result<bool> {
        if self.has_pending()? {
            return Ok(false);
        }
        let body: Option<String> = self
            .connection
            .query_row_cached(
                "SELECT body FROM changes WHERE operation=?1 AND ordinal=?2",
                params![receipt.id, receipt.ordinal],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        let Some(body) = body else {
            return Ok(false);
        };
        let change: Change = decode(&body)?;
        validate_change(&change)?;
        let Change::Rename {
            original,
            current,
            identity,
            retained: None,
            saved: Some(_),
            ..
        } = change
        else {
            return Ok(false);
        };
        if original != source || current != destination || identity != receipt.identity {
            return Ok(false);
        }
        let expected = self.resolve_for(receipt.id, receipt.ordinal, destination, identity)?;
        Ok(fs::symlink_metadata(destination).is_ok_and(|metadata| {
            metadata.is_dir() && Identity::from_metadata(&metadata) == expected
        }) && self.owned_saved_stage(destination, expected)?.is_some())
    }

    pub(in crate::operations) fn owned_saved_stage(
        &self,
        path: &Path,
        identity: Identity,
    ) -> io::Result<Option<recovery::Record>> {
        for owner in owners(&self.connection, identity)? {
            if let Some(record) = self.recovery_record(&owner)?
                && record.retained.stage.directory == path
                && record.retained.stage.identity == identity
            {
                if record.payload {
                    return Err(invalid(
                        "Keep claimed Recovery data with its container; recover an explicit copy first.",
                    ));
                }
                return Ok(Some(record));
            }
        }
        Ok(None)
    }

    /// Only explicit Recovery Copy may traverse a protected work container as a
    /// read-only source. Ordinary saved-container moves retain their own rules.
    pub(in crate::operations) fn is_recovery_container(
        &self,
        path: &Path,
        identity: Identity,
    ) -> io::Result<bool> {
        for owner in owners(&self.connection, identity)? {
            if let Some(record) = self.recovery_record(&owner)?
                && record.container
                && record.retained.stage.directory == path
                && record.retained.stage.identity == identity
            {
                record.available()?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl Store {
    pub(in crate::operations) fn publish_saved_stage(
        &mut self,
        migration: &saved_stage::Migration,
        version: &fingerprint::Version,
    ) -> io::Result<()> {
        self.ensure_location()?;
        if !migration.forward {
            self.require_directory_parents(migration.id, migration.ordinal)?;
        }
        if migration.has_claim() {
            self.require_saved_migration(migration)?;
        }
        let mut owned = Vec::new();
        for key in owners(&self.connection, migration.source.identity)? {
            if self.recovery_record(&key)?.is_some_and(|record| {
                record.retained.stage.identity == migration.source.identity
                    && record.retained.stage.directory == migration.source.directory
            }) {
                owned.push(key);
            }
        }
        if owned.is_empty() {
            return Err(invalid(
                "the saved-data container no longer has its original catalog owner",
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        source_retention::relocate(
            &transaction,
            migration.source.identity,
            migration.stage.identity,
        )?;
        for key in owned {
            match key {
                recovery::Key::Active { operation, ordinal } => {
                    let mut change = stored_change(&transaction, operation, ordinal)?;
                    let retained = change
                        .retained_mut()
                        .ok_or_else(|| invalid("saved-stage owner has no retained version"))?;
                    rebind(retained, migration, version);
                    transaction
                        .execute_cached(
                            "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                            params![operation, ordinal, encode(&change)?],
                        )
                        .map_err(db_error)?;
                    paths::index_change(&transaction, operation, ordinal, &change)?;
                    references::index(&transaction, operation, ordinal, &change, false)?;
                }
                recovery::Key::Archived(entry) => {
                    let body: String = transaction
                        .query_row_cached("SELECT body FROM garbage WHERE id=?1", [entry], |row| {
                            row.get(0)
                        })
                        .map_err(db_error)?;
                    let mut record = recovery::Record::decode(&body)?;
                    rebind(&mut record.retained, migration, version);
                    transaction
                        .execute_cached(
                            "UPDATE garbage SET body=?2 WHERE id=?1",
                            params![entry, encode(&record)?],
                        )
                        .map_err(db_error)?;
                    paths::index_record(&transaction, entry, &record)?;
                }
                recovery::Key::Pending(_) => {
                    return Err(invalid("an interrupted saved stage cannot migrate"));
                }
            }
        }
        if migration.forward {
            append_transaction(
                &transaction,
                self.lane,
                migration.id,
                migration.ordinal,
                &migration.change,
            )?;
            references::adopt_primary(
                &transaction,
                migration.id,
                migration.ordinal,
                migration.stage.identity,
            )?;
        } else {
            reverse_transaction(
                &transaction,
                self.lane,
                migration.id,
                migration.ordinal,
                &migration.change,
                migration.relocation.as_ref(),
                None,
            )?;
        }
        let mut pending = migration.clone();
        pending.catalogued = true;
        if pending.has_claim() {
            context::install_saved_phase(&transaction, self.identity, self.lane, &pending)?;
        }
        transaction
            .execute_cached(
                "INSERT INTO pending(singleton,body) VALUES(1,?1)",
                [encode(&Pending::StageMigration {
                    migration: Box::new(pending),
                })?],
            )
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)
    }
}

impl Store {
    pub(in crate::operations) fn require_saved_migration(
        &self,
        migration: &saved_stage::Migration,
    ) -> io::Result<()> {
        context::require_saved_migration(&self.connection, self.identity, self.lane, migration)?;
        transfer_cursor::require_forward(
            &self.connection,
            self.identity,
            self.lane,
            &saved_stage::forward::virtual_forward(migration),
        )
    }
    pub(in crate::operations) fn saved_migration_phase(
        &mut self,
        transition: &saved_stage::forward::Transition,
    ) -> io::Result<()> {
        self.ensure_location()?;
        let before = transition.before();
        let after = transition.after();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let body: String = transaction
            .query_row_cached("SELECT body FROM pending WHERE singleton=1", [], |r| {
                r.get(0)
            })
            .map_err(db_error)?;
        if body
            != encode(&Pending::StageMigration {
                migration: Box::new(before.clone()),
            })?
        {
            return Err(invalid(
                "The saved cleanup phase no longer owns this journal.",
            ));
        }
        context::require_saved_migration(&transaction, self.identity, self.lane, before)?;
        transfer_cursor::require_forward(
            &transaction,
            self.identity,
            self.lane,
            &saved_stage::forward::virtual_forward(before),
        )?;
        saved_stage::forward::validate(after)?;
        if !before.catalogued
            || !after.catalogued
            || encode(&before.change)? != encode(&after.change)?
        {
            return Err(invalid(
                "The saved cleanup phase changed its committed step.",
            ));
        }
        context::install_saved_phase(&transaction, self.identity, self.lane, after)?;
        transaction
            .execute_cached(
                "UPDATE pending SET body=?1 WHERE singleton=1",
                [encode(&Pending::StageMigration {
                    migration: Box::new(after.clone()),
                })?],
            )
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)
    }
}

fn rebind(
    retained: &mut Retained,
    migration: &saved_stage::Migration,
    version: &fingerprint::Version,
) {
    let owned_identity = retained.original_identity() == migration.source_version.identity;
    let unchanged = retained
        .version
        .as_ref()
        .map_or(owned_identity, |expected| {
            expected == &migration.source_version
        });
    if owned_identity {
        retained.physical_identity = Some(migration.copy_identity);
    }
    if unchanged {
        retained.version = Some(version.clone());
    } else if let Some(expected) = &mut retained.version
        && expected.identity == migration.source_version.identity
    {
        // Recreating a changed original object does not authorize its edits.
        expected.identity = migration.copy_identity;
    }
    retained.stage.identity = migration.stage.identity;
    retained.stage.payload_identity = None;
    retained.stage.parent_identity = migration.stage.parent_identity;
    retained.stage.parent_access = migration.stage.parent_access;
    if let Some(ownership) = migration.destination_stage().ownership {
        retained.stage.ownership = Some(ownership);
    }
    // An older migration without target proof cannot downgrade an existing
    // strong owner to scalar cleanup rights. Its stale proof stays reviewable.
}

/// A direct saved-container rename keeps its physical identity but changes its
/// parent. Update every indexed owner in the same history transaction.
pub(super) fn move_parent(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    change: &Change,
    reverse: bool,
) -> io::Result<()> {
    let change = match change {
        Change::DirectoryMerge { change, .. } => change.as_ref(),
        change => change,
    };
    let Change::Rename {
        original,
        current,
        identity,
        saved: Some(saved),
        ..
    } = change
    else {
        return Ok(());
    };
    // A recreated container was already rebound from its created-copy proof by
    // publish_saved_stage. Its history identity still names the old source;
    // direct-rename parent updates must not borrow that source's root facts.
    if saved.copied_bytes().is_some() {
        return Ok(());
    }
    let parent = if reverse {
        if let Some(parent) =
            directory::return_parent(connection, operation, ordinal, original.parent().unwrap())?
        {
            parent
        } else {
            let expected = references::resolve(
                connection,
                operation,
                ordinal,
                original.parent().unwrap(),
                saved.parent_identity,
            )?;
            let access = match saved.source_access {
                Some(StageParentAccess::Created(created)) if created == expected => {
                    StageParentAccess::Created(expected)
                }
                _ => StageParentAccess::Existing,
            };
            saved_stage::Parent {
                original: references::resolved_original(
                    connection,
                    operation,
                    ordinal,
                    original.parent().unwrap(),
                    saved.parent_identity,
                )?,
                identity: expected,
                access,
            }
        }
    } else if let Some(parent) = saved.destination_parent.as_deref() {
        *parent
    } else {
        return Ok(());
    };
    let ownership = if let Some(original) = parent.original {
        saved
            .ownership(reverse)
            .map(|owner| owner.with_parent(original))
    } else if (reverse && parent.identity == saved.parent_identity)
        || (!reverse
            && saved
                .destination_parent
                .as_deref()
                .is_some_and(|p| p.identity == parent.identity))
    {
        saved.ownership(reverse)
    } else {
        None
    };
    if let Some(ownership) = &ownership {
        let target = if reverse { original } else { current };
        let parent = fs::File::from(rustix::fs::open(
            target.parent().unwrap(),
            rustix::fs::OFlags::PATH | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?);
        ownership.require_parent(&parent)?;
        let directory = fs::File::from(rustix::fs::open(
            target,
            rustix::fs::OFlags::PATH
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?);
        ownership.require_directory(&directory)?;
    }
    // Only parent metadata changes. Stream the stable owner index while updating
    // bodies; path and stage-index keys remain unchanged.
    any_owner(connection, *identity, |owner| {
        match owner {
            recovery::Key::Active { operation, ordinal } => {
                let mut change = stored_change(connection, operation, ordinal)?;
                let retained = change
                    .retained_mut()
                    .ok_or_else(|| invalid("saved owner lost its retained data"))?;
                if retained.stage.identity != *identity {
                    return Ok(false);
                }
                retained.stage.parent_identity = Some(parent.identity);
                retained.stage.parent_access = Some(parent.access);
                if let Some(ownership) = &ownership {
                    retained.stage.ownership = Some(Box::new(ownership.clone()));
                }
                connection
                    .execute_cached(
                        "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                        params![operation, ordinal, encode(&change)?],
                    )
                    .map_err(db_error)?;
            }
            recovery::Key::Archived(entry) => {
                let body: String = connection
                    .query_row_cached("SELECT body FROM garbage WHERE id=?1", [entry], |row| {
                        row.get(0)
                    })
                    .map_err(db_error)?;
                let mut record = recovery::Record::decode(&body)?;
                if record.retained.stage.identity != *identity {
                    return Ok(false);
                }
                record.retained.stage.parent_identity = Some(parent.identity);
                record.retained.stage.parent_access = Some(parent.access);
                if let Some(ownership) = &ownership {
                    record.retained.stage.ownership = Some(Box::new(ownership.clone()));
                }
                connection
                    .execute_cached(
                        "UPDATE garbage SET body=?2 WHERE id=?1",
                        params![entry, encode(&record)?],
                    )
                    .map_err(db_error)?;
            }
            recovery::Key::Pending(_) => {
                return Err(invalid("an interrupted saved container cannot move"));
            }
        }
        Ok(false)
    })?;
    Ok(())
}
