use super::*;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{collections::BTreeSet, sync::Arc};
mod claim;
mod context;
pub(super) mod copy_undo;
mod cross;
mod directory;
mod directory_access;
pub(in crate::operations) mod directory_reads;
pub(super) mod disposal_review;
mod execution;
mod expiry;
mod mutation;
mod origins;
mod paths;
mod references;
mod requests;
pub(in crate::operations) mod reverse_cleanup;
mod saved;
pub(super) mod saved_return;
mod source_retention;
mod sql;
use sql::CachedSql;
mod stage_intent;
#[cfg(test)]
pub(in crate::operations) use stage_intent::legacy_reproduction_fixture;
mod stages;
mod transfer_completion;
mod transfer_cursor;
mod transfer_interruption;
pub(in crate::operations) use transfer_cursor::Scope as ActiveCursorScope;
mod transfer_stop;
pub(in crate::operations) mod undo_work;
// v24 reserves Forward/Rename with a stage for an unresolved opened-parent
// claim. Older executables would treat that stage as disposable work.
// v25 persists protected Recovery payloads after explicit claim review.
// v26 reserves and records typed ordinary mutation completion before result save.
// v27 records typed successful Copy/Move ends without granting restart execution.
// v28 persists descriptor-derived original selection facts in checkpoint v2.
// v29 records exact stopped transfer results independently of application saves.
// v30 persists opened traversal/created-folder facts in checkpoint v3.
// v31 persists opened private-copy stage facts and post-append cleanup intents.
// v32 reserves verified source-removal claim phases from generic cleanup.
// v33 reserves whole-entry Move claim/publication/return phases.
// v34 reserves reviewed replacement claims and their limited return direction.
// v35 binds exact new Forward intents before recovery filesystem mutations.
// Completed Cleanup has empty-directory rights, even for its original payload.
// v36 verifies and resumes ordinary prepared-copy publication using original facts.
// v37 reserves empty-source-directory claims and their limited return direction.
// v38 reserves stopped-part restart ownership and its original history-file proof.
// v39 persists normalized active traversal cursors with scoped per-row digests.
// v40 binds copied-leaf filesystem steps to the durable active traversal task.
// v41 persists confirmed source-folder remainders alongside active traversal.
// v42 persists exact directory expansions before publication and child traversal.
// v43 records original saved-container generations and exact whole-Move tasks.
// v44 binds copied saved-container recipes and source cleanup claims.
// v45 independently binds created preparations and committed empty cleanup.
// v46 reserves durable preparation disposal and consumed-name rights.
// v53 reserves one-shot displaced Undo returns and additional pending work slots.
// v54 retains empty-only Undo cleanup atomically with completed reversals.
// Original returned-parent generations follow surviving history references.
// v55 binds ordinary Copy Undo namespace consumption and its original work root.
// v56 binds saved-destination Copy Undo returns independently of copy disposal.
// v57 binds changed-copy preservation and its empty-preparation compensation.
// v58 reserves selected saved-return consumption and completion.
// v59 binds ordinary Undo displacement and its consumed compensation.
// v60 reserves proved saved-return refusal and exact paired saved-only history.
const VERSION: u32 = 60;

type Problems = Arc<Mutex<[Option<String>; 2]>>;

thread_local! {
    static CHANGES: std::cell::RefCell<serialized::Last<Change>> =
        const { std::cell::RefCell::new(serialized::Last::new()) };
    static JOURNAL: std::cell::RefCell<serialized::Last<Pending>> =
        const { std::cell::RefCell::new(serialized::Last::new()) };
}

fn decode_change(body: &str) -> io::Result<Change> {
    CHANGES.with(|cached| Ok(cached.borrow_mut().read(body, decode)?.as_ref().clone()))
}

fn decode_pending(body: &str) -> io::Result<Pending> {
    JOURNAL.with(|cached| Ok(cached.borrow_mut().read(body, decode)?.as_ref().clone()))
}

#[derive(Clone)]
pub(super) struct ReadSource {
    path: PathBuf,
    identity: Identity,
    problems: Problems,
    directory_reads: directory_reads::Shared,
}

impl ReadSource {
    fn ensure_location(&self) -> io::Result<()> {
        if self.identity.matches(&self.path) {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "operation history has been moved or replaced",
            ))
        }
    }

    pub fn snapshot(&self, hide_running_journals: bool) -> io::Result<Store> {
        use rusqlite::OpenFlags;
        self.ensure_location()?;
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(db_error)?;
        // WAL readers keep one committed view for the page without taking the
        // mutation mutex. Dropping this connection releases its read snapshot.
        connection
            .execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON; BEGIN DEFERRED;")
            .map_err(db_error)?;
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(db_error)?;
        if version != VERSION {
            return Err(invalid("unsupported operation database version"));
        }
        self.ensure_location()?;
        Ok(Store {
            connection,
            path: self.path.clone(),
            identity: self.identity,
            lane: IntentLane::Primary,
            problems: Arc::new(Mutex::new(
                self.problems
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone(),
            )),
            hide_running_journals,
            opened_directories: Default::default(),
            directory_reads: self.directory_reads.clone(),
        })
    }
}

// Mutating transactions acquire the writer before reading ownership/journal
// state. Deferred read snapshots cannot safely upgrade after the independent
// admission writer commits, even when SQLite's busy timeout is nonzero.
pub(super) struct Store {
    connection: Connection,
    path: PathBuf,
    identity: Identity,
    lane: IntentLane,
    problems: Problems,
    pub(super) hide_running_journals: bool,
    opened_directories: std::collections::BTreeMap<i64, fs::File>,
    directory_reads: directory_reads::Shared,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum IntentLane {
    Primary,
    RecoveryCopy,
}
impl IntentLane {
    fn index(self) -> usize {
        match self {
            Self::Primary => 0,
            Self::RecoveryCopy => 1,
        }
    }
}

impl Store {
    pub(super) fn history_observation_paths(&self) -> Vec<PathBuf> {
        let mut wal = self.path.as_os_str().to_owned();
        wal.push("-wal");
        vec![self.path.clone(), wal.into()]
    }
    pub(super) fn lane_for_reconciliation(&self) -> IntentLane {
        self.lane
    }
    pub fn open(path: &Path) -> io::Result<Self> {
        use rusqlite::OpenFlags;
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(db_error)?;
        connection.set_prepared_statement_cache_capacity(128);
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(db_error)?;
        if version > VERSION {
            return Err(invalid("unsupported operation database version"));
        }
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;").map_err(db_error)?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(db_error)?;
        transaction.execute_batch("
            CREATE TABLE IF NOT EXISTS operations (id INTEGER PRIMARY KEY AUTOINCREMENT, label TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS changes (operation INTEGER NOT NULL REFERENCES operations(id) ON DELETE CASCADE, ordinal INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(operation, ordinal));
            CREATE TABLE IF NOT EXISTS pending (singleton INTEGER PRIMARY KEY CHECK(singleton=1), body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS recovery_pending (singleton INTEGER PRIMARY KEY CHECK(singleton=1), body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS garbage (id INTEGER PRIMARY KEY, body TEXT NOT NULL, error TEXT);
            CREATE TABLE IF NOT EXISTS bindings (path BLOB NOT NULL, previous TEXT NOT NULL, replacement TEXT NOT NULL, PRIMARY KEY(path,previous));").map_err(db_error)?;
        paths::create(&transaction)?;
        directory::create(&transaction, version)?;
        directory_access::create(&transaction)?;
        directory_reads::create(&transaction)?;
        saved::create(&transaction, version)?;
        stages::create(&transaction, version)?;
        origins::create(&transaction, version)?;
        requests::create(&transaction)?;
        execution::create(&transaction)?;
        mutation::create(&transaction)?;
        transfer_completion::create(&transaction)?;
        transfer_stop::create(&transaction)?;
        transfer_cursor::create(&transaction)?;
        transfer_interruption::create(&transaction)?;
        stage_intent::create(&transaction)?;
        undo_work::create(&transaction)?;
        reverse_cleanup::create(&transaction)?;
        copy_undo::create(&transaction)?;
        saved_return::create(&transaction)?;
        if version < 6 {
            transaction
                .execute_batch(
                    "ALTER TABLE changes ADD COLUMN binding_revision INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE bindings ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
                INSERT INTO metadata(key,value) VALUES('binding-revision','1');",
                )
                .map_err(db_error)?;
        }
        if version < 7 {
            transaction.execute_batch("ALTER TABLE changes ADD COLUMN bindings TEXT;
                CREATE TABLE pending_bindings (lane INTEGER PRIMARY KEY CHECK(lane IN (0,1)), operation INTEGER NOT NULL, ordinal INTEGER NOT NULL, body TEXT NOT NULL);").map_err(db_error)?;
        }
        if version < 4 {
            paths::populate(&transaction)?;
        }
        references::create(&transaction)?;
        if version < 7 {
            references::populate(&transaction)?;
            transaction
                .execute_cached("DELETE FROM bindings", [])
                .map_err(db_error)?;
        }
        if version < VERSION {
            transaction
                .pragma_update(None, "user_version", VERSION)
                .map_err(db_error)?;
        }
        transaction.commit().map_err(db_error)?;
        let mut store = Self {
            connection,
            path: path.to_owned(),
            identity: Identity::read(path)?,
            lane: IntentLane::Primary,
            problems: Arc::new(Mutex::new([None, None])),
            hide_running_journals: false,
            opened_directories: Default::default(),
            directory_reads: Default::default(),
        };
        store.restore_abandoned_directory_reads()?;
        Ok(store)
    }

    pub fn read_source(&self) -> ReadSource {
        ReadSource {
            path: self.path.clone(),
            identity: self.identity,
            problems: self.problems.clone(),
            directory_reads: self.directory_reads.clone(),
        }
    }

    pub fn imported(&self) -> io::Result<bool> {
        self.connection
            .query_row_cached(
                "SELECT value FROM metadata WHERE key='legacy-imported'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|value| value.is_some())
            .map_err(db_error)
    }
    pub fn import(&mut self, legacy: Option<LegacyJournal>) -> io::Result<()> {
        if self.imported()? {
            return Ok(());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(legacy) = legacy {
            for record in legacy.entries {
                validate_change(&record.change)?;
                transaction
                    .execute_cached(
                        "INSERT INTO operations(id,label) VALUES(?1,?2)",
                        params![record.id, record.label],
                    )
                    .map_err(db_error)?;
                transaction
                    .execute_cached(
                        "INSERT INTO changes(operation,ordinal,body) VALUES(?1,0,?2)",
                        params![record.id, encode(&record.change)?],
                    )
                    .map_err(db_error)?;
                paths::index_change(&transaction, record.id, 0, &record.change)?;
                references::index(&transaction, record.id, 0, &record.change, false)?;
            }
            let pending = match legacy.pending {
                Some(LegacyPending::Forward { record, stage }) => {
                    validate_change(&record.change)?;
                    transaction
                        .execute_cached(
                            "INSERT OR IGNORE INTO operations(id,label) VALUES(?1,?2)",
                            params![record.id, record.label],
                        )
                        .map_err(db_error)?;
                    Some(Pending::Forward {
                        id: record.id,
                        ordinal: 0,
                        change: record.change,
                        stage,
                    })
                }
                Some(LegacyPending::Undo { id }) => {
                    let body: String = transaction
                        .query_row_cached(
                            "SELECT body FROM changes WHERE operation=?1 AND ordinal=0",
                            [id],
                            |row| row.get(0),
                        )
                        .map_err(db_error)?;
                    Some(Pending::Undo {
                        id,
                        ordinal: 0,
                        change: decode(&body)?,
                        stage: None,
                        relocation: None,
                        displaced: None,
                    })
                }
                None => None,
            };
            if let Some(pending) = pending {
                transaction
                    .execute_cached("INSERT INTO pending VALUES(1,?1)", [encode(&pending)?])
                    .map_err(db_error)?;
            }
            let sequence: u64 = transaction
                .query_row_cached(
                    "SELECT COALESCE(MAX(seq),0) FROM sqlite_sequence WHERE name='operations'",
                    [],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            if legacy.next_id.saturating_sub(1) > sequence {
                transaction
                    .execute_cached(
                        "INSERT INTO operations(id,label) VALUES(?1,'migration sequence')",
                        [legacy.next_id - 1],
                    )
                    .map_err(db_error)?;
                transaction
                    .execute_cached("DELETE FROM operations WHERE id=?1", [legacy.next_id - 1])
                    .map_err(db_error)?;
            }
        }
        transaction
            .execute_cached("INSERT INTO metadata VALUES('legacy-imported','1')", [])
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)
    }

    pub fn begin(&mut self, label: &str) -> io::Result<u64> {
        self.begin_request(label, None)
    }
    pub fn latest(&self) -> io::Result<Option<(u64, String)>> {
        self.connection.query_row_cached("SELECT id,label FROM operations WHERE EXISTS(SELECT 1 FROM changes WHERE operation=operations.id) ORDER BY id DESC LIMIT 1", [], |row| Ok((row.get(0)?, row.get(1)?))).optional().map_err(db_error)
    }
    pub fn last_change(&self, id: u64) -> io::Result<Option<(u64, Change)>> {
        self.change_before(id, None)
    }
    pub fn change_before(&self, id: u64, before: Option<u64>) -> io::Result<Option<(u64, Change)>> {
        let result: Option<(u64, String)> = self
            .connection
            .query_row_cached(
                "SELECT ordinal,body FROM changes WHERE operation=?1 AND (?2 IS NULL OR ordinal<?2) ORDER BY ordinal DESC LIMIT 1",
                params![id, before],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        result
            .map(|(ordinal, body)| {
                decode_change(&body).and_then(|change| {
                    validate_change(&change)?;
                    Ok((ordinal, change))
                })
            })
            .transpose()
    }
    pub fn next_ordinal(&self, id: u64) -> io::Result<u64> {
        self.connection
            .query_row_cached(
                "SELECT COALESCE(MAX(ordinal)+1,0) FROM changes WHERE operation=?1",
                [id],
                |row| row.get(0),
            )
            .map_err(db_error)
    }
    pub fn pending(&self) -> io::Result<Option<Pending>> {
        self.pending_in(self.lane)
    }
    pub fn pending_in(&self, lane: IntentLane) -> io::Result<Option<Pending>> {
        let result: Option<String> = self
            .connection
            .query_row_cached(
                match lane {
                    IntentLane::Primary => "SELECT body FROM pending WHERE singleton=1",
                    IntentLane::RecoveryCopy => {
                        "SELECT body FROM recovery_pending WHERE singleton=1"
                    }
                },
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        let pending = result.map(|body| decode_pending(&body)).transpose()?;
        if matches!(lane, IntentLane::RecoveryCopy)
            && pending.as_ref().is_some_and(|pending| {
                !matches!(
                    pending,
                    Pending::Preparing { .. }
                        | Pending::Disposing { .. }
                        | Pending::Cleanup { .. }
                        | Pending::UndoCleanup { .. }
                        | Pending::UndoCopied { .. }
                        | Pending::Forward {
                            change: Change::Copied { retained: None, .. }
                                | Change::CreatedFolder { .. },
                            ..
                        }
                )
            })
        {
            return Err(invalid("invalid recovery-copy journal entry"));
        }
        Ok(pending)
    }
    pub fn intent(&mut self, pending: &Pending) -> io::Result<()> {
        self.ensure_location()?;
        let preserve_undo = if matches!(self.lane, IntentLane::Primary) {
            undo_work::preserve(&self.connection, self.identity, pending)?
        } else {
            true
        };
        if matches!(
            pending,
            Pending::Preparing { .. }
                | Pending::Disposing { .. }
                | Pending::Cleanup { .. }
                | Pending::UndoCleanup { .. }
                | Pending::UndoCopied { .. }
                | Pending::UndoSavedReturn { .. }
        ) {
            return Err(invalid(
                "Staged work needs its actual creator or verified completion transition.",
            ));
        }
        if let Some(previous) = self.pending()? {
            if matches!(
                previous,
                Pending::Disposing { .. }
                    | Pending::UndoCleanup { .. }
                    | Pending::UndoCopied { .. }
                    | Pending::UndoSavedReturn { .. }
            ) {
                return Err(invalid(
                    "Disposal needs its original private phase transition.",
                ));
            }
            context::require_forward(&self.connection, self.identity, self.lane, &previous)?;
            transfer_cursor::require_forward(
                &self.connection,
                self.identity,
                self.lane,
                &previous,
            )?;
            stage_intent::require(
                &self.connection,
                self.identity,
                self.lane,
                &previous,
                self.problems.clone(),
            )?;
            stage_intent::leaving(&previous, pending)?;
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        source_retention::check_intent(&transaction, pending)?;
        context::prepare(&transaction, self.identity, self.lane, pending)?;
        transfer_cursor::bind_forward(&transaction, self.identity, self.lane, pending)?;
        stage_intent::clear(&transaction, self.lane)?;
        if !preserve_undo {
            undo_work::clear(&transaction)?;
        }
        transaction
            .execute_cached(
                match self.lane {
                    IntentLane::Primary => "INSERT OR REPLACE INTO pending VALUES(1,?1)",
                    IntentLane::RecoveryCopy => {
                        "INSERT OR REPLACE INTO recovery_pending VALUES(1,?1)"
                    }
                },
                [encode(pending)?],
            )
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)
    }
    pub(in crate::operations) fn disposal_transition(
        &mut self,
        transition: &staging::disposal::Transition,
    ) -> io::Result<()> {
        self.ensure_location()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if matches!(transition.after(), Pending::Disposing { disposal } if disposal.undo.is_some())
        {
            if !matches!(self.lane, IntentLane::Primary) {
                return Err(invalid("Undo disposal belongs to its primary journal."));
            }
            undo_work::disposal_transition(&transaction, self.identity, transition)?;
        } else {
            stage_intent::disposal_transition(
                &transaction,
                self.identity,
                self.lane,
                transition,
                self.problems.clone(),
            )?;
        }
        source_retention::check_intent(&transaction, transition.after())?;
        let changed = transaction
            .execute_cached(
                match self.lane {
                    IntentLane::Primary => "UPDATE pending SET body=?1 WHERE singleton=1",
                    IntentLane::RecoveryCopy => {
                        "UPDATE recovery_pending SET body=?1 WHERE singleton=1"
                    }
                },
                [encode(transition.after())?],
            )
            .map_err(db_error)?;
        if changed != 1 {
            return Err(invalid("The disposal phase was not recorded."));
        }
        let body: String = transaction
            .query_row_cached(
                match self.lane {
                    IntentLane::Primary => "SELECT body FROM pending WHERE singleton=1",
                    IntentLane::RecoveryCopy => {
                        "SELECT body FROM recovery_pending WHERE singleton=1"
                    }
                },
                [],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if body != encode(transition.after())? {
            return Err(invalid("The recorded disposal phase changed."));
        }
        stage_intent::require(
            &transaction,
            self.identity,
            self.lane,
            transition.after(),
            self.problems.clone(),
        )?;
        transaction.commit().map_err(db_error)
    }
    pub(super) fn register_preparing(
        &mut self,
        id: u64,
        destination: &Path,
        created: &staging::Registration<'_>,
    ) -> io::Result<()> {
        self.ensure_location()?;
        created.require()?;
        let stage = created.stage();
        if destination.parent() != stage.directory.parent() {
            return Err(invalid(
                "New work must stay in its selected destination folder.",
            ));
        }
        let task = transfer_cursor::stage_task(&self.connection, self.identity, self.lane, id)?;
        if let Some(task) = &task {
            if task.destination.parent() != destination.parent() {
                return Err(invalid(
                    "The preparation differs from its exact destination task.",
                ));
            }
            if task.source != created.source() {
                let identity = created.container_identity()?.ok_or_else(|| {
                    invalid("The preparation differs from its exact source task.")
                })?;
                let record = self
                    .owned_saved_stage(&task.source, identity)?
                    .ok_or_else(|| {
                        invalid("The preparation has no original saved-container task.")
                    })?;
                created.saved_source(&record)?;
            }
        }
        let pending = Pending::Preparing {
            id,
            destination: destination.to_owned(),
            stage: stage.clone(),
        };
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        stage_intent::prepare(
            &transaction,
            self.identity,
            self.lane,
            &pending,
            created,
            task,
            self.problems.clone(),
        )?;
        source_retention::check_intent(&transaction, &pending)?;
        context::prepare(&transaction, self.identity, self.lane, &pending)?;
        transaction
            .execute_cached(
                match self.lane {
                    IntentLane::Primary => "INSERT OR REPLACE INTO pending VALUES(1,?1)",
                    IntentLane::RecoveryCopy => {
                        "INSERT OR REPLACE INTO recovery_pending VALUES(1,?1)"
                    }
                },
                [encode(&pending)?],
            )
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)
    }
    pub(super) fn returned_source_intent(&mut self, pending: &Pending) -> io::Result<()> {
        if !matches!(self.lane, IntentLane::Primary) {
            return Err(invalid("A source return belongs to the primary journal."));
        }
        self.ensure_location()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let body: String = transaction
            .query_row_cached("SELECT body FROM pending WHERE singleton=1", [], |r| {
                r.get(0)
            })
            .map_err(db_error)?;
        let current: Pending = decode(&body)?;
        if !source_removal::returned_transition(&current, pending)
            && !directory_removal::returned_transition(&current, pending)
            && !entry_move::returned_transition(&current, pending)
            && !replacement::returned_transition(&current, pending)
        {
            return Err(invalid(
                "The returned source does not match its exact empty-stage transition.",
            ));
        }
        context::require_forward(&transaction, self.identity, self.lane, &current)?;
        let history_original =
            context::history_original(&transaction, self.identity, self.lane, &current)?;
        let Pending::Forward {
            id,
            ordinal,
            change,
            ..
        } = &current
        else {
            unreachable!()
        };
        transfer_cursor::retire_returned(&transaction, self.identity, *id, *ordinal, change)?;
        source_retention::check_intent(&transaction, pending)?;
        context::prepare(&transaction, self.identity, self.lane, pending)?;
        stage_intent::returned(
            &transaction,
            self.identity,
            self.lane,
            pending,
            history_original,
        )?;
        transaction
            .execute_cached(
                "INSERT OR REPLACE INTO pending VALUES(1,?1)",
                [encode(pending)?],
            )
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)
    }
    pub fn clear_intent(&mut self) -> io::Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        clear_intent(&transaction, self.lane)?;
        transaction.commit().map_err(db_error)
    }
    pub(super) fn require_bound_forward(&self) -> io::Result<()> {
        if let Some(pending) = self.pending()? {
            copy_undo::require(&self.connection, self.identity, self.lane, &pending)?;
            saved_return::require(&self.connection, self.identity, self.lane, &pending)?;
            reverse_cleanup::require(&self.connection, self.identity, self.lane, &pending)?;
            if matches!(self.lane, IntentLane::Primary) {
                undo_work::require(&self.connection, self.identity, &pending)?;
            }
            stage_intent::require(
                &self.connection,
                self.identity,
                self.lane,
                &pending,
                self.problems.clone(),
            )?;
            context::require_forward(&self.connection, self.identity, self.lane, &pending)?;
            transfer_cursor::require_forward(&self.connection, self.identity, self.lane, &pending)?;
        }
        Ok(())
    }
    pub fn in_recovery_copy<T>(
        &mut self,
        run: impl FnOnce(&mut Self) -> io::Result<T>,
    ) -> io::Result<T> {
        let previous = self.lane;
        self.lane = IntentLane::RecoveryCopy;
        let result = run(self);
        self.lane = previous;
        result
    }
    pub fn recovery_result(&mut self, result: &io::Result<()>) {
        let message = result.as_ref().err().map(ToString::to_string);
        self.problems
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[self.lane.index()] = message;
    }
    pub fn recovery_problem(&self, lane: IntentLane) -> Option<String> {
        self.problems
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[lane.index()]
        .clone()
    }
    pub fn has_pending(&self) -> io::Result<bool> {
        Ok(self.pending_in(IntentLane::Primary)?.is_some()
            || self.pending_in(IntentLane::RecoveryCopy)?.is_some()
            || !self.directory_access_paths(IntentLane::Primary)?.is_empty()
            || !self
                .directory_access_paths(IntentLane::RecoveryCopy)?
                .is_empty())
    }
    pub fn label(&self, id: u64) -> io::Result<Option<String>> {
        self.connection
            .query_row_cached("SELECT label FROM operations WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .optional()
            .map_err(db_error)
    }
    pub fn append(&mut self, id: u64, ordinal: u64, change: &Change) -> io::Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        append_transaction(&transaction, self.lane, id, ordinal, change)?;
        transaction.commit().map_err(db_error)
    }
    pub fn append_with_cleanup(
        &mut self,
        id: u64,
        ordinal: u64,
        change: &Change,
        stage: &Stage,
    ) -> io::Result<Stage> {
        let stage = staging::completed_cleanup(stage)?;
        let cleanup = Pending::Cleanup {
            id,
            ordinal,
            destination: change.forward_path().to_owned(),
            stage: stage.clone(),
        };
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let completion = stage_intent::completion(
            &transaction,
            self.identity,
            self.lane,
            id,
            ordinal,
            change,
            &stage,
        )?;
        append_transaction(&transaction, self.lane, id, ordinal, change)?;
        transaction
            .execute_cached(
                match self.lane {
                    IntentLane::Primary => "INSERT INTO pending VALUES(1,?1)",
                    IntentLane::RecoveryCopy => "INSERT INTO recovery_pending VALUES(1,?1)",
                },
                [encode(&cleanup)?],
            )
            .map_err(db_error)?;
        completion.install(&transaction, change)?;
        transaction.commit().map_err(db_error)?;
        #[cfg(test)]
        transfer::stage_boundary::cleanup(&cleanup);
        Ok(stage)
    }
    pub fn reversed(&mut self, id: u64, ordinal: u64) -> io::Result<()> {
        self.reversed_with_relocation(id, ordinal, None)
    }
    pub(super) fn append_replacement_with_cleanup(
        &mut self,
        id: u64,
        ordinal: u64,
        refreshed: &replacement::Refreshed,
    ) -> io::Result<Stage> {
        self.ensure_location()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let body: String = transaction
            .query_row_cached(
                match self.lane {
                    IntentLane::Primary => "SELECT body FROM pending WHERE singleton=1",
                    IntentLane::RecoveryCopy => {
                        "SELECT body FROM recovery_pending WHERE singleton=1"
                    }
                },
                [],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let pending: Pending = decode(&body)?;
        context::require_forward(&transaction, self.identity, self.lane, &pending)?;
        let (before, after) = refreshed.changes();
        let Pending::Forward {
            id: owner,
            ordinal: step,
            change,
            stage: Some(stage),
        } = &pending
        else {
            return Err(invalid(
                "A replacement refresh needs its exact Forward intent.",
            ));
        };
        if *owner != id
            || *step != ordinal
            || encode(change)? != encode(before)?
            || stage.replacement.is_none()
        {
            return Err(invalid(
                "The replacement refresh differs from its original intent.",
            ));
        }
        let cleanup_stage = staging::completed_cleanup(&replacement::normalized(stage))?;
        let completion = stage_intent::completion(
            &transaction,
            self.identity,
            self.lane,
            id,
            ordinal,
            before,
            &cleanup_stage,
        )?;
        if matches!(self.lane, IntentLane::Primary) {
            transfer_cursor::refresh_replacement(
                &transaction,
                self.identity,
                id,
                ordinal,
                before,
                after,
            )?;
        }
        let stage = staging::completed_cleanup(&replacement::normalized(stage))?;
        append_transaction(&transaction, self.lane, id, ordinal, after)?;
        let cleanup = Pending::Cleanup {
            id,
            ordinal,
            destination: after.forward_path().to_owned(),
            stage: stage.clone(),
        };
        transaction
            .execute_cached(
                match self.lane {
                    IntentLane::Primary => "INSERT INTO pending VALUES(1,?1)",
                    IntentLane::RecoveryCopy => "INSERT INTO recovery_pending VALUES(1,?1)",
                },
                [encode(&cleanup)?],
            )
            .map_err(db_error)?;
        completion.install(&transaction, after)?;
        transaction.commit().map_err(db_error)?;
        Ok(stage)
    }
    pub fn reversed_with_relocation(
        &mut self,
        id: u64,
        ordinal: u64,
        relocation: Option<&undo::Relocation>,
    ) -> io::Result<()> {
        let change = stored_change(&self.connection, id, ordinal)?;
        self.reversed_with_recovery(id, ordinal, &change, relocation, None)
    }
    pub fn reversed_with_recovery(
        &mut self,
        id: u64,
        ordinal: u64,
        change: &Change,
        relocation: Option<&undo::Relocation>,
        displaced: Option<&recovery::Record>,
    ) -> io::Result<()> {
        // The durable intent omits restoration of the saved destination only
        // for an explicitly chosen source-only return. Keep its original saved
        // record as a retryable remainder, including after process interruption.
        if matches!(
            change,
            Change::DirectoryMerge {
                defer_previous: true,
                ..
            }
        ) {
            return self.defer_directory_saved(id, ordinal, change);
        }
        if matches!(change, Change::Rename { retained: None, .. })
            && let Change::Rename {
                current,
                identity,
                retained: Some(retained),
                ..
            } = stored_change(&self.connection, id, ordinal)?
        {
            return self.replace_change_with_recovery(
                id,
                ordinal,
                &Change::RetainedOnly {
                    path: current,
                    retained,
                    replaced: Some(identity),
                },
                relocation,
                displaced,
            );
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let cleanup =
            reverse_cleanup::prepare(&transaction, self.identity, self.lane, id, ordinal, change)?;
        reverse_transaction(
            &transaction,
            self.lane,
            id,
            ordinal,
            change,
            relocation,
            displaced,
        )?;
        if let Some(cleanup) = cleanup {
            cleanup.install(&transaction, self.lane)?;
        }
        transaction.commit().map_err(db_error)?;
        reverse_cleanup::recover(self)
    }
    pub fn dismiss_change(
        &mut self,
        id: u64,
        ordinal: u64,
        protected: Option<&recovery::Record>,
    ) -> io::Result<()> {
        self.ensure_location()?;
        self.require_directory_parents(id, ordinal)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        archive(&transaction, protected)?;
        complete_reversal(&transaction, id, ordinal, self.lane)?;
        transaction.commit().map_err(db_error)
    }
    pub fn cross_copy(&self, id: u64, ordinal: u64) -> io::Result<Option<Change>> {
        cross::paired(&self.connection, id, ordinal).map(|paired| paired.map(|(_, change)| change))
    }
    pub fn keep_cross_item(
        &mut self,
        id: u64,
        ordinal: u64,
        path: &Path,
        label: &str,
    ) -> io::Result<Vec<PathBuf>> {
        let paired = cross::paired(&self.connection, id, ordinal)?;
        let protected = paired
            .as_ref()
            .and_then(|(_, change)| {
                recovery::Record::from_change(
                    change,
                    format!(
                        "Preserved after keeping a cross-filesystem destination during Undo {label}"
                    ),
                )
            })
            .map(|mut record| {
                record.retained.recovery = true;
                record
            });
        self.keep_item_with_copy(
            id,
            ordinal,
            path,
            protected.as_ref(),
            paired.as_ref().map(|(ordinal, _)| *ordinal),
        )
    }
    pub fn keep_item(
        &mut self,
        id: u64,
        ordinal: u64,
        path: &Path,
        protected: Option<&recovery::Record>,
    ) -> io::Result<Vec<PathBuf>> {
        self.keep_item_with_copy(id, ordinal, path, protected, None)
    }
    fn keep_item_with_copy(
        &mut self,
        id: u64,
        ordinal: u64,
        path: &Path,
        protected: Option<&recovery::Record>,
        copy: Option<u64>,
    ) -> io::Result<Vec<PathBuf>> {
        self.ensure_location()?;
        self.require_directory_parents(id, ordinal)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        archive(&transaction, protected)?;
        let mut kept = vec![path.to_owned()];
        for ancestor in path.ancestors().skip(1) {
            let key = paths::key(ancestor);
            let mut cursor = None::<u64>;
            loop {
                let rows: Vec<(u64, String)> = {
                    let mut statement = transaction.prepare_cached("SELECT f.ordinal,c.body FROM folder_changes f JOIN changes c ON c.operation=f.operation AND c.ordinal=f.ordinal WHERE f.path=?1 AND f.operation=?2 AND f.ordinal<?3 AND (?4 IS NULL OR f.ordinal>?4) ORDER BY f.ordinal LIMIT 256").map_err(db_error)?;
                    statement
                        .query_map(params![key, id, ordinal, cursor], |row| {
                            Ok((row.get(0)?, row.get(1)?))
                        })
                        .map_err(db_error)?
                        .collect::<Result<_, _>>()
                        .map_err(db_error)?
                };
                if rows.is_empty() {
                    break;
                }
                for (folder_ordinal, body) in rows {
                    cursor = Some(folder_ordinal);
                    let Change::CreatedFolder { path: folder, .. } = decode(&body)? else {
                        return Err(invalid("invalid container index"));
                    };
                    if folder != ancestor {
                        return Err(invalid("invalid container path"));
                    }
                    if !kept.contains(&folder) {
                        kept.push(folder);
                    }
                    transaction
                        .execute_cached(
                            "DELETE FROM changes WHERE operation=?1 AND ordinal=?2",
                            params![id, folder_ordinal],
                        )
                        .map_err(db_error)?;
                }
            }
        }
        if let Some(copy) = copy {
            complete_reversal(&transaction, id, copy, self.lane)?;
        }
        complete_reversal(&transaction, id, ordinal, self.lane)?;
        transaction.commit().map_err(db_error)?;
        Ok(kept)
    }
    pub fn remaining_changes(&self, id: u64) -> io::Result<usize> {
        self.connection
            .query_row_cached(
                "SELECT COUNT(*) FROM changes WHERE operation=?1",
                [id],
                |row| row.get(0),
            )
            .map_err(db_error)
    }
    pub fn discard_empty(&mut self, id: u64) -> io::Result<()> {
        // A syscall can commit before its first confirmed-change insert fails.
        // Its journal still needs this parent row to record reconciliation.
        for lane in [IntentLane::Primary, IntentLane::RecoveryCopy] {
            if self
                .pending_in(lane)?
                .is_some_and(|pending| pending.id() == id)
            {
                return Ok(());
            }
        }
        self.connection.execute_cached("DELETE FROM operations WHERE id=?1 AND NOT EXISTS(SELECT 1 FROM changes WHERE operation=?1) AND NOT EXISTS(SELECT 1 FROM request_executions e JOIN request_transfer_cursors c ON c.request=e.request WHERE e.operation=?1)", [id]).map(|_| ()).map_err(db_error)
    }
    pub(in crate::operations) fn require_original_change_history(
        &self,
        id: u64,
        ordinal: u64,
    ) -> io::Result<()> {
        self.ensure_location()?;
        context::require_change_history(&self.connection, self.identity, id, ordinal)
    }
    pub fn replace_change_with_recovery(
        &mut self,
        id: u64,
        ordinal: u64,
        change: &Change,
        relocation: Option<&undo::Relocation>,
        displaced: Option<&recovery::Record>,
    ) -> io::Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let followed = if matches!(change, Change::RetainedOnly { .. }) {
            let mut previous = stored_change(&transaction, id, ordinal)?;
            if let Some(relocation) = relocation {
                previous.relocate(relocation);
            }
            if !undo_work::returned_context(&transaction, self.lane, id, ordinal)? {
                context::returned(&transaction, id, ordinal, &previous)?;
            }
            follow_archived(&transaction, id, ordinal, &previous, true)?
        } else {
            BTreeSet::new()
        };
        if let Some(relocation) = relocation {
            relocate(&transaction, id, ordinal, relocation, &followed)?;
        }
        archive(&transaction, displaced)?;
        transaction
            .execute_cached(
                "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                params![id, ordinal, encode(change)?],
            )
            .map_err(db_error)?;
        paths::index_change(&transaction, id, ordinal, change)?;
        references::index(&transaction, id, ordinal, change, false)?;
        clear_intent(&transaction, self.lane)?;
        transaction.commit().map_err(db_error)
    }
    pub fn clear_undo_with_recovery(
        &mut self,
        displaced: Option<&recovery::Record>,
    ) -> io::Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        archive(&transaction, displaced)?;
        clear_intent(&transaction, self.lane)?;
        transaction.commit().map_err(db_error)
    }
    pub fn resolve_for(
        &self,
        operation: u64,
        ordinal: u64,
        path: &Path,
        identity: Identity,
    ) -> io::Result<Identity> {
        references::resolve(&self.connection, operation, ordinal, path, identity)
    }

    pub fn trim(&mut self) -> io::Result<()> {
        // Expiry belongs to the primary journal even when Recovery Copy's last
        // step makes old history eligible. Both lanes are idle before trimming.
        if self.has_pending()? {
            return Ok(());
        }
        let previous = self.lane;
        self.lane = IntentLane::Primary;
        let result = self.trim_idle();
        self.lane = previous;
        result
    }
    fn trim_idle(&mut self) -> io::Result<()> {
        if self.has_pending()? {
            return Ok(());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        {
            let mut statement = transaction.prepare_cached("SELECT changes.operation,changes.ordinal,changes.body,operations.label FROM changes JOIN operations ON operations.id=changes.operation WHERE operation IN (SELECT id FROM operations WHERE EXISTS(SELECT 1 FROM changes WHERE operation=operations.id) AND NOT EXISTS(SELECT 1 FROM directory_returns WHERE operation=operations.id) ORDER BY id DESC LIMIT -1 OFFSET ?1)").map_err(db_error)?;
            let rows = statement
                .query_map([HISTORY_LIMIT], |row| {
                    Ok((
                        row.get::<_, u64>(0)?,
                        row.get::<_, u64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })
                .map_err(db_error)?;
            for body in rows {
                let (operation, ordinal, body, label) = body.map_err(db_error)?;
                let change: Change = decode(&body)?;
                if let Some(mut record) = recovery::Record::from_change(&change, label) {
                    // A partial operation has left this version as its only
                    // unfinished result. Expiry must preserve it even when a
                    // previous Undo review accepted its current contents.
                    record.retained.recovery |= matches!(change, Change::RetainedOnly { .. });
                    follow_later(&transaction, operation, ordinal, &mut record)?;
                    archive(&transaction, Some(&record))?;
                }
            }
        }
        transaction.execute_cached("DELETE FROM operations WHERE NOT EXISTS(SELECT 1 FROM changes WHERE operation=operations.id) AND NOT EXISTS(SELECT 1 FROM request_executions e JOIN request_transfer_cursors c ON c.request=e.request WHERE e.operation=operations.id)", []).map_err(db_error)?;
        transaction
            .execute_cached(
                "DELETE FROM bindings WHERE NOT EXISTS(SELECT 1 FROM changes)",
                [],
            )
            .map_err(db_error)?;
        transaction.execute_cached("DELETE FROM operations WHERE id IN (SELECT id FROM operations WHERE EXISTS(SELECT 1 FROM changes WHERE operation=operations.id) AND NOT EXISTS(SELECT 1 FROM directory_returns WHERE operation=operations.id) ORDER BY id DESC LIMIT -1 OFFSET ?1)", [HISTORY_LIMIT]).map_err(db_error)?;
        origins::prune(&transaction)?;
        transaction.commit().map_err(db_error)?;
        self.collect_garbage()
    }
    fn collect_garbage(&mut self) -> io::Result<()> {
        let mut cursor = 0i64;
        loop {
            let rows: Vec<(i64, String)> = {
                let mut statement = self
                    .connection
                    .prepare_cached("SELECT id,body FROM garbage WHERE id>?1 ORDER BY id LIMIT 256")
                    .map_err(db_error)?;
                statement
                    .query_map([cursor], |row| Ok((row.get(0)?, row.get(1)?)))
                    .map_err(db_error)?
                    .collect::<Result<_, _>>()
                    .map_err(db_error)?
            };
            if rows.is_empty() {
                break;
            }
            for (id, body) in rows {
                cursor = id;
                let result = recovery::Record::decode(&body).and_then(|record| {
                    self.expire_archived_retained(
                        u64::try_from(id).map_err(io::Error::other)?,
                        &record,
                    )
                });
                match result {
                    Ok(()) => {
                        self.connection
                            .execute_cached("DELETE FROM garbage WHERE id=?1", [id])
                            .map_err(db_error)?;
                    }
                    Err(error) => {
                        self.connection
                            .execute_cached(
                                "UPDATE garbage SET error=?2 WHERE id=?1",
                                params![id, error.to_string()],
                            )
                            .map_err(db_error)?;
                        if self.has_pending()? {
                            self.recovery_result(&Err(io::Error::other(error.to_string())));
                            return Ok(());
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn next_recovery(
        &self,
        cursor: Option<&recovery::RecoveryCursor>,
    ) -> io::Result<Option<(recovery::RecoveryCursor, Option<recovery::Record>)>> {
        use recovery::{Key, Record, RecoveryCursor};
        if cursor.is_none() || matches!(cursor, Some(RecoveryCursor(Key::Pending(_)))) {
            let start = match cursor {
                Some(RecoveryCursor(Key::Pending(slot))) => slot + 1,
                _ => 0,
            };
            for slot in start..16 {
                if let Some(record) = self.pending_record(slot)? {
                    return Ok(Some((RecoveryCursor(Key::Pending(slot)), Some(record))));
                }
            }
        }
        if !matches!(cursor, Some(RecoveryCursor(Key::Archived(_)))) {
            let (operation, ordinal) = match cursor {
                Some(RecoveryCursor(Key::Active { operation, ordinal })) => (*operation, *ordinal),
                _ => (0, 0),
            };
            let row: Option<(u64, u64, String, String)> = self.connection.query_row_cached(
                "SELECT changes.operation,changes.ordinal,changes.body,operations.label FROM changes JOIN operations ON operations.id=changes.operation WHERE changes.operation>?1 OR(changes.operation=?1 AND changes.ordinal>?2) ORDER BY changes.operation,changes.ordinal LIMIT 1",
                params![operation, ordinal], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).optional().map_err(db_error)?;
            if let Some((operation, ordinal, body, label)) = row {
                let change: Change = decode(&body)?;
                validate_change(&change)?;
                let mut record = Record::from_change(&change, label);
                if let Some(record) = &mut record {
                    follow_later(&self.connection, operation, ordinal, record)?;
                }
                return Ok(Some((
                    RecoveryCursor(Key::Active { operation, ordinal }),
                    record,
                )));
            }
        }
        let after = match cursor {
            Some(RecoveryCursor(Key::Archived(id))) => *id,
            _ => 0,
        };
        let row: Option<(u64, String)> = self
            .connection
            .query_row_cached(
                "SELECT id,body FROM garbage WHERE id>?1 ORDER BY id LIMIT 1",
                [after],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        row.map(|(id, body)| {
            Ok((
                RecoveryCursor(Key::Archived(id)),
                Some(Record::decode(&body)?),
            ))
        })
        .transpose()
    }
    pub fn recovery_record(&self, key: &recovery::Key) -> io::Result<Option<recovery::Record>> {
        match key {
            recovery::Key::Pending(slot) => self.pending_record(*slot),
            recovery::Key::Active { operation, ordinal } => {
                let row: Option<(String, String)> = self.connection.query_row_cached("SELECT changes.body,operations.label FROM changes JOIN operations ON operations.id=changes.operation WHERE changes.operation=?1 AND changes.ordinal=?2", params![operation, ordinal], |row| Ok((row.get(0)?, row.get(1)?))).optional().map_err(db_error)?;
                row.map(|(body, label)| {
                    let change: Change = decode(&body)?;
                    validate_change(&change)?;
                    let mut record = recovery::Record::from_change(&change, label);
                    if let Some(record) = &mut record {
                        follow_later(&self.connection, *operation, *ordinal, record)?;
                    }
                    Ok(record)
                })
                .transpose()
                .map(Option::flatten)
            }
            recovery::Key::Archived(id) => {
                let body: Option<String> = self
                    .connection
                    .query_row_cached("SELECT body FROM garbage WHERE id=?1", [id], |row| {
                        row.get(0)
                    })
                    .optional()
                    .map_err(db_error)?;
                body.map(|body| recovery::Record::decode(&body)).transpose()
            }
        }
    }
    fn ensure_location(&self) -> io::Result<()> {
        if !self.identity.matches(&self.path) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "operation history has been moved or replaced",
            ));
        }
        Ok(())
    }
}

fn append_transaction(
    transaction: &rusqlite::Transaction<'_>,
    lane: IntentLane,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<()> {
    let body = encode(change)?;
    let bindings = context::for_append(transaction, lane, id, ordinal, change)?;
    transaction
            .execute_cached(
                "INSERT OR IGNORE INTO changes(operation,ordinal,body,binding_revision,bindings) VALUES(?1,?2,?3,?4,?5)",
                params![id, ordinal, body, 0, bindings],
            )
            .map_err(db_error)?;
    let existing: String = transaction
        .query_row_cached(
            "SELECT body FROM changes WHERE operation=?1 AND ordinal=?2",
            params![id, ordinal],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if existing != body {
        return Err(invalid("conflicting operation history step"));
    }
    paths::index_change(transaction, id, ordinal, change)?;
    references::index(transaction, id, ordinal, change, false)?;
    stages::move_parent(transaction, id, ordinal, change, false)?;
    let _followed = follow_archived(transaction, id, ordinal, change, false)?;
    clear_intent(transaction, lane)?;
    Ok(())
}

fn reverse_transaction(
    transaction: &rusqlite::Transaction<'_>,
    lane: IntentLane,
    id: u64,
    ordinal: u64,
    change: &Change,
    relocation: Option<&undo::Relocation>,
    displaced: Option<&recovery::Record>,
) -> io::Result<()> {
    if !cross::restored_only(transaction, id, ordinal, change)? {
        cross::returned(transaction, id, ordinal, change)?;
        if (!matches!(
            change,
            Change::Rename {
                retained: Some(_),
                ..
            } | Change::RetainedOnly { .. }
        ) || !saved_return::returned_context(transaction, lane, id, ordinal, change)?)
            && (!matches!(change, Change::Rename { .. } | Change::RetainedOnly { .. })
                || !undo_work::returned_context(transaction, lane, id, ordinal)?)
        {
            context::returned(transaction, id, ordinal, change)?;
        }
        context::restored_destination(transaction, id, ordinal, change)?;
    }
    stages::move_parent(transaction, id, ordinal, change, true)?;
    let followed = follow_archived(transaction, id, ordinal, change, true)?;
    if let Some(relocation) = relocation {
        relocate(transaction, id, ordinal, relocation, &followed)?;
    }
    archive(transaction, displaced)?;
    if let Some(owner) = undo::directory_return::moved(change).and_then(|moved| moved.created) {
        complete_reversal(transaction, id, owner, lane)?;
    }
    complete_reversal(transaction, id, ordinal, lane)
}

fn complete_reversal(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    lane: IntentLane,
) -> io::Result<()> {
    connection
        .execute_cached(
            "DELETE FROM changes WHERE operation=?1 AND ordinal=?2",
            params![id, ordinal],
        )
        .map_err(db_error)?;
    let remains: bool = connection
        .query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM changes WHERE operation=?1 AND ordinal=?2)",
            params![id, ordinal],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    if remains {
        return Err(invalid("The original Undo history step was not retired."));
    }
    clear_intent(connection, lane)?;
    connection.execute_cached("DELETE FROM operations WHERE id=?1 AND NOT EXISTS(SELECT 1 FROM changes WHERE operation=?1) AND NOT EXISTS(SELECT 1 FROM request_executions e JOIN request_transfer_cursors c ON c.request=e.request WHERE e.operation=?1)", [id]).map_err(db_error)?;
    connection
        .execute_cached(
            "DELETE FROM bindings WHERE NOT EXISTS(SELECT 1 FROM changes)",
            [],
        )
        .map_err(db_error)?;
    Ok(())
}

fn clear_intent(connection: &Connection, lane: IntentLane) -> io::Result<()> {
    copy_undo::clear(connection, lane)?;
    saved_return::clear(connection, lane)?;
    reverse_cleanup::clear(connection, lane)?;
    if matches!(lane, IntentLane::Primary) {
        undo_work::clear(connection)?;
    }
    stage_intent::clear(connection, lane)?;
    context::clear(connection, lane)?;
    connection
        .execute_cached(
            match lane {
                IntentLane::Primary => "DELETE FROM pending",
                IntentLane::RecoveryCopy => "DELETE FROM recovery_pending",
            },
            [],
        )
        .map_err(db_error)?;
    let remains: bool = connection
        .query_row_cached(
            match lane {
                IntentLane::Primary => "SELECT EXISTS(SELECT 1 FROM pending) OR EXISTS(SELECT 1 FROM undo_work) OR EXISTS(SELECT 1 FROM stage_intents WHERE lane=0) OR EXISTS(SELECT 1 FROM pending_bindings WHERE lane=0)",
                IntentLane::RecoveryCopy => "SELECT EXISTS(SELECT 1 FROM recovery_pending) OR EXISTS(SELECT 1 FROM stage_intents WHERE lane=1) OR EXISTS(SELECT 1 FROM pending_bindings WHERE lane=1)",
            },
            [],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if remains {
        return Err(invalid(
            "The interrupted journal and its bindings were not retired.",
        ));
    }
    Ok(())
}

fn archive(connection: &Connection, record: Option<&recovery::Record>) -> io::Result<()> {
    if let Some(record) = record {
        let encoded = encode(record)?;
        if connection
            .execute_cached("INSERT INTO garbage(body) VALUES(?1)", [&encoded])
            .map_err(db_error)?
            != 1
        {
            return Err(invalid("The Recovery version was not archived."));
        }
        let entry = u64::try_from(connection.last_insert_rowid()).map_err(io::Error::other)?;
        let saved: Option<String> = connection
            .query_row_cached("SELECT body FROM garbage WHERE id=?1", [entry], |r| {
                r.get(0)
            })
            .optional()
            .map_err(db_error)?;
        if saved.as_deref() != Some(encoded.as_str()) {
            return Err(invalid("The Recovery version was not saved exactly."));
        }
        paths::index_record(connection, entry, record)?;
        paths::require_retained_record(connection, entry, record)?;
    }
    Ok(())
}

fn follow_later(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    record: &mut recovery::Record,
) -> io::Result<()> {
    // Without an existing container movement, only a later namespace move of
    // this container or an ancestor can start a context chain. The derived
    // namespace index proves that absence without decoding every unrelated
    // later copy/folder step for every Recovery row. Once a chain exists (or a
    // relevant move is present), keep the complete identity-aware traversal.
    if record.context_moves.is_empty()
        && !paths::has_later_namespace_move(
            connection,
            &record.retained.stage.directory,
            (operation, ordinal),
        )?
    {
        return Ok(());
    }
    let base_operation = operation;
    let base_ordinal = ordinal;
    let expected_owner = references::primary(connection, base_operation, base_ordinal)?;
    let mut statement = connection.prepare_cached("SELECT operation,ordinal,body FROM changes WHERE operation>?1 OR(operation=?1 AND ordinal>?2) ORDER BY operation,ordinal").map_err(db_error)?;
    let rows = statement
        .query_map(params![operation, ordinal], |row| {
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
        validate_change(&change)?;
        let step = recovery::ContextStep { operation, ordinal };
        let owner = recovery::ContextStep {
            operation: base_operation,
            ordinal: base_ordinal,
        };
        let source_identity = match &change {
            Change::SourceRemoved { identity, .. }
            | Change::Rename {
                saved: None,
                identity,
                ..
            } => Some(*identity),
            _ => None,
        };
        let file_owner = if let Some(identity) = source_identity {
            expected_owner == Some(identity) || context::source_owned_by(connection, step, owner)?
        } else {
            false
        };
        record.follow(step, &change, false, file_owner, |movement| {
            origins::parent(connection, movement)
        })?;
    }
    Ok(())
}

fn stored_change(connection: &Connection, operation: u64, ordinal: u64) -> io::Result<Change> {
    let body: String = connection
        .query_row_cached(
            "SELECT body FROM changes WHERE operation=?1 AND ordinal=?2",
            params![operation, ordinal],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    decode_change(&body)
}

fn follow_archived(
    connection: &Connection,
    operation: u64,
    ordinal: u64,
    change: &Change,
    reverse: bool,
) -> io::Result<BTreeSet<u64>> {
    let owner_change = match change {
        Change::DirectoryMerge { change, .. } => change.as_ref(),
        change => change,
    };
    if !matches!(
        owner_change,
        Change::Rename { .. }
            | Change::Trashed { .. }
            | Change::Restored { .. }
            | Change::SourceRemoved { .. }
            | Change::RemovedDirectory { .. }
    ) {
        return Ok(BTreeSet::new());
    }
    let namespace = match owner_change {
        Change::SourceRemoved { original, .. } => Some(paths::key(original.parent().unwrap())),
        Change::RemovedDirectory { path, .. } => Some(paths::key(path)),
        _ => None,
    };
    let mut statement = connection
        .prepare_cached(if namespace.is_some() {
            "SELECT g.id,g.body FROM garbage_paths p JOIN garbage g ON g.id=p.entry WHERE p.path=?1 ORDER BY g.id"
        } else { "SELECT id,body FROM garbage ORDER BY id" })
        .map_err(db_error)?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(namespace.iter()), |row| {
            Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(db_error)?;
    let mut followed = BTreeSet::new();
    for row in rows {
        let (id, body) = row.map_err(db_error)?;
        let mut record = recovery::Record::decode(&body)?;
        record.follow(
            recovery::ContextStep { operation, ordinal },
            change,
            reverse,
            false,
            |movement| origins::parent(connection, movement),
        )?;
        let updated = encode(&record)?;
        if updated != body {
            followed.insert(id);
            connection
                .execute_cached(
                    "UPDATE garbage SET body=?2 WHERE id=?1",
                    params![id, updated],
                )
                .map_err(db_error)?;
            paths::index_record(connection, id, &record)?;
        }
    }
    Ok(followed)
}

fn relocate(
    transaction: &rusqlite::Transaction<'_>,
    id: u64,
    ordinal: u64,
    relocation: &undo::Relocation,
    followed_archives: &BTreeSet<u64>,
) -> io::Result<()> {
    relocate_return(
        transaction,
        id,
        ordinal,
        relocation,
        followed_archives,
        None,
    )
}

fn relocate_return(
    transaction: &rusqlite::Transaction<'_>,
    id: u64,
    ordinal: u64,
    relocation: &undo::Relocation,
    followed_archives: &BTreeSet<u64>,
    planned: Option<u64>,
) -> io::Result<()> {
    relocation.validate()?;
    let mut movements = directory::relocation::Movements::read(transaction, id, relocation)?;
    if let Some(root) = planned {
        movements.planned_destination(id, root);
    }
    let history_before = if planned.is_some() {
        Some(ordinal)
    } else {
        directory::history_before(transaction, id, ordinal)?
    };
    let namespace = paths::key(&relocation.from);
    // Materialize each bounded page before rewriting its index entries.
    let mut cursor = (0u64, 0u64);
    while let Some(history_before) = history_before {
        let rows: Vec<(u64, u64, String)> = {
            let mut statement = transaction.prepare_cached("SELECT p.operation,p.ordinal,c.body FROM change_paths p JOIN changes c ON c.operation=p.operation AND c.ordinal=p.ordinal WHERE p.path=?1 AND (p.operation<?2 OR(p.operation=?2 AND p.ordinal<?3)) AND (p.operation,p.ordinal)>(?4,?5) ORDER BY p.operation,p.ordinal LIMIT 256").map_err(db_error)?;
            statement
                .query_map(
                    params![namespace, id, history_before, cursor.0, cursor.1],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(db_error)?
                .collect::<Result<_, _>>()
                .map_err(db_error)?
        };
        if rows.is_empty() {
            break;
        }
        for (operation, step, body) in rows {
            cursor = (operation, step);
            if saved::holds_destination(
                transaction,
                &movements,
                (id, ordinal),
                (operation, step),
                relocation,
            )? {
                continue;
            }
            let mut change: Change = decode(&body)?;
            movements.relocate_member(transaction, operation, step, &mut change, relocation)?;
            validate_change(&change)?;
            let updated = encode(&change)?;
            if updated != body {
                transaction
                    .execute_cached(
                        "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                        params![operation, step, updated],
                    )
                    .map_err(db_error)?;
                paths::index_change(transaction, operation, step, &change)?;
                references::index(transaction, operation, step, &change, false)?;
            }
        }
    }
    let mut after = 0u64;
    loop {
        let rows: Vec<(u64, String)> = {
            let mut statement = transaction.prepare_cached("SELECT p.entry,g.body FROM garbage_paths p JOIN garbage g ON g.id=p.entry WHERE p.path=?1 AND p.entry>?2 ORDER BY p.entry LIMIT 256").map_err(db_error)?;
            statement
                .query_map(params![namespace, after], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .map_err(db_error)?
                .collect::<Result<_, _>>()
                .map_err(db_error)?
        };
        if rows.is_empty() {
            break;
        }
        for (entry, body) in rows {
            after = entry;
            // Archived stages are live locations. Only records that followed
            // this actual return may adopt its logical Keep both namespace.
            // Newer data remaining in the destination must keep its own path.
            if !followed_archives.contains(&entry)
                && !directory::owns_archive(transaction, id, ordinal, entry)?
                && !movements.owns_archive_destination(transaction, entry)?
            {
                continue;
            }
            let mut record = recovery::Record::decode(&body)?;
            record.relocate(relocation);
            let updated = encode(&record)?;
            if updated != body {
                transaction
                    .execute_cached(
                        "UPDATE garbage SET body=?2 WHERE id=?1",
                        params![entry, updated],
                    )
                    .map_err(db_error)?;
                paths::index_record(transaction, entry, &record)?;
            }
        }
    }
    Ok(())
}

fn resolve_binding(
    connection: &Connection,
    path: &Path,
    mut identity: Identity,
    after: u64,
) -> io::Result<Identity> {
    let mut visited = std::collections::HashSet::new();
    while visited.insert((identity.device, identity.inode)) {
        let next: Option<String> = connection
            .query_row_cached(
                "SELECT replacement FROM bindings WHERE path=?1 AND previous=?2 AND revision>?3",
                params![
                    path.as_os_str().as_encoded_bytes(),
                    encode(&identity)?,
                    after
                ],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        if let Some(next) = next {
            let next = decode(&next)?;
            if identity == next {
                return Ok(identity);
            }
            identity = next;
        } else {
            return Ok(identity);
        }
    }
    Err(invalid("operation identity history contains a cycle"))
}

pub(super) fn encode(value: &impl Serialize) -> io::Result<String> {
    let mut body = Vec::new();
    value
        .serialize(&mut serde_json::Serializer::with_formatter(
            &mut body,
            serialized::CompactJournal,
        ))
        .map_err(io::Error::other)?;
    if body.len() > 128 * 1024 {
        return Err(invalid("operation step exceeds its storage budget"));
    }
    String::from_utf8(body).map_err(io::Error::other)
}
pub(super) fn decode<T: serde::de::DeserializeOwned>(body: &str) -> io::Result<T> {
    if body.len() > 128 * 1024 {
        return Err(invalid("operation step exceeds its storage budget"));
    }
    serde_json::from_str(body).map_err(io::Error::other)
}
fn db_error(error: rusqlite::Error) -> io::Error {
    io::Error::other(format!("operation history: {error}"))
}
