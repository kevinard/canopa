//! One bounded incremental cursor for the history's serial active transfer.
use super::*;
use crate::operations::{
    requests::RequestRecord,
    transfer::{
        self,
        cursor::{Edit, Recorded, Row, Update},
    },
};
use sha2::{Digest, Sha256};

const LIMIT: u64 = 64 * 1024 * 1024;

/// Created only after the complete original cursor and end evidence validates.
/// A failed write retains descriptive results, never filesystem execution rights.
pub(in crate::operations) struct Recovery {
    pub(in crate::operations) end: transfer::cursor::Recovered,
    pub(in crate::operations) error: Option<io::Error>,
}
pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS request_transfer_cursors (
        request INTEGER PRIMARY KEY REFERENCES request_executions(request) ON DELETE CASCADE,
        ordinal INTEGER NOT NULL CHECK(ordinal>=0),
        spec TEXT NOT NULL CHECK(length(CAST(spec AS BLOB))<=4194304),
        progress TEXT NOT NULL CHECK(length(CAST(progress AS BLOB))<=131072),
        spec_sha256 BLOB NOT NULL CHECK(length(spec_sha256)=32),
        progress_sha256 BLOB NOT NULL CHECK(length(progress_sha256)=32),
        bytes INTEGER NOT NULL DEFAULT 0 CHECK(bytes BETWEEN 0 AND 67108864));
        CREATE TABLE IF NOT EXISTS request_transfer_tasks (
        request INTEGER NOT NULL REFERENCES request_transfer_cursors(request) ON DELETE CASCADE,
        role INTEGER NOT NULL CHECK(role BETWEEN 0 AND 9),
        position BLOB NOT NULL CHECK(length(position)<=1048576),
        body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=131072),
        body_sha256 BLOB NOT NULL CHECK(length(body_sha256)=32),
        PRIMARY KEY(request,role,position));",
        )
        .map_err(db_error)?;
    let has_bytes: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('request_transfer_cursors') WHERE name='bytes')", [], |row| row.get(0)).map_err(db_error)?;
    if !has_bytes {
        connection.execute_batch("ALTER TABLE request_transfer_cursors ADD COLUMN bytes INTEGER NOT NULL DEFAULT 0 CHECK(bytes BETWEEN 0 AND 67108864);
        UPDATE request_transfer_cursors SET bytes=length(CAST(spec AS BLOB))+length(CAST(progress AS BLOB))+256+(SELECT coalesce(sum(length(CAST(body AS BLOB))+length(position)+128),0) FROM request_transfer_tasks WHERE request=request_transfer_cursors.request);").map_err(db_error)?;
    }
    let task_schema: String = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='request_transfer_tasks'",
            [],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if task_schema.contains("BETWEEN 0 AND 5")
        || task_schema.contains("BETWEEN 0 AND 6")
        || task_schema.contains("BETWEEN 0 AND 7")
    {
        connection
            .execute_batch(
                "DROP TRIGGER IF EXISTS transfer_task_inserted;
        DROP TRIGGER IF EXISTS transfer_task_changed;
        DROP TRIGGER IF EXISTS transfer_task_removed;
        ALTER TABLE request_transfer_tasks RENAME TO request_transfer_tasks_previous;
        CREATE TABLE request_transfer_tasks (
        request INTEGER NOT NULL REFERENCES request_transfer_cursors(request) ON DELETE CASCADE,
        role INTEGER NOT NULL CHECK(role BETWEEN 0 AND 9),
        position BLOB NOT NULL CHECK(length(position)<=1048576),
        body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=131072),
        body_sha256 BLOB NOT NULL CHECK(length(body_sha256)=32),
        PRIMARY KEY(request,role,position));
        INSERT INTO request_transfer_tasks SELECT * FROM request_transfer_tasks_previous;
        DROP TABLE request_transfer_tasks_previous;",
            )
            .map_err(db_error)?;
    }
    connection.execute_batch("CREATE TABLE IF NOT EXISTS request_transfer_cursor_steps (
        request INTEGER NOT NULL REFERENCES request_transfer_cursors(request) ON DELETE CASCADE,
        ordinal INTEGER NOT NULL CHECK(ordinal>=0),
        position BLOB NOT NULL CHECK(length(position)=8),
        task_sha256 BLOB NOT NULL CHECK(length(task_sha256)=32),
        body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=131072),
        body_sha256 BLOB NOT NULL CHECK(length(body_sha256)=32),
        PRIMARY KEY(request,ordinal));
        CREATE TRIGGER IF NOT EXISTS transfer_step_inserted AFTER INSERT ON request_transfer_cursor_steps BEGIN
        UPDATE request_transfer_cursors SET bytes=bytes+length(CAST(NEW.body AS BLOB))+160 WHERE request=NEW.request;
        END;
        CREATE TRIGGER IF NOT EXISTS transfer_step_removed AFTER DELETE ON request_transfer_cursor_steps BEGIN
        UPDATE request_transfer_cursors SET bytes=bytes-length(CAST(OLD.body AS BLOB))-160 WHERE request=OLD.request;
        END;").map_err(db_error)?;
    connection.execute_batch("CREATE TRIGGER IF NOT EXISTS transfer_task_inserted AFTER INSERT ON request_transfer_tasks BEGIN
        UPDATE request_transfer_cursors SET bytes=bytes+length(CAST(NEW.body AS BLOB))+length(NEW.position)+128 WHERE request=NEW.request;
        END;
        CREATE TRIGGER IF NOT EXISTS transfer_task_changed AFTER UPDATE OF body,position ON request_transfer_tasks BEGIN
        UPDATE request_transfer_cursors SET bytes=bytes+length(CAST(NEW.body AS BLOB))+length(NEW.position)-length(CAST(OLD.body AS BLOB))-length(OLD.position) WHERE request=NEW.request;
        END;
        CREATE TRIGGER IF NOT EXISTS transfer_task_removed AFTER DELETE ON request_transfer_tasks BEGIN
        UPDATE request_transfer_cursors SET bytes=bytes-length(CAST(OLD.body AS BLOB))-length(OLD.position)-128 WHERE request=OLD.request;
        END;").map_err(db_error)
}

pub(in crate::operations) struct Scope {
    request: u64,
    revision: u64,
    operation: u64,
    history: Identity,
    prefix: [u8; 32],
    owner: Problems,
}
fn scope(request: &RequestRecord, operation: u64, spec_digest: &[u8], owner: Problems) -> Scope {
    Scope {
        request: request.id,
        revision: request.revision,
        operation,
        history: request.history,
        prefix: prefix(
            request.history,
            request.id,
            request.revision,
            operation,
            &Sha256::digest(&request.body),
            spec_digest,
        ),
        owner,
    }
}
fn prefix(
    history: Identity,
    request: u64,
    revision: u64,
    operation: u64,
    payload_digest: &[u8],
    spec_digest: &[u8],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"canopa-files-active-transfer-v1\0");
    hash.update(history.device.to_le_bytes());
    hash.update(history.inode.to_le_bytes());
    hash.update(request.to_le_bytes());
    hash.update(revision.to_le_bytes());
    hash.update(operation.to_le_bytes());
    hash.update(payload_digest);
    hash.update(spec_digest);
    hash.finalize().into()
}

fn progress_digest(prefix: &[u8; 32], ordinal: u64, body: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(prefix);
    hash.update(b"progress\0");
    hash.update(ordinal.to_le_bytes());
    hash.update(body.as_bytes());
    hash.finalize().into()
}
fn row_digest(prefix: &[u8; 32], role: u8, key: &[u8], body: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(prefix);
    hash.update(b"row\0");
    hash.update([role]);
    hash.update((key.len() as u64).to_le_bytes());
    hash.update(key);
    hash.update(body.as_bytes());
    hash.finalize().into()
}
fn require_idle(connection: &Connection) -> io::Result<()> {
    let pending: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM pending UNION ALL SELECT 1 FROM recovery_pending UNION ALL SELECT 1 FROM directory_access)", [], |row| row.get(0)).map_err(db_error)?;
    if pending {
        return Err(invalid(
            "Reconcile interrupted filesystem work before continuing this transfer.",
        ));
    }
    Ok(())
}
fn ordinal(connection: &Connection, operation: u64) -> io::Result<u64> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM operations WHERE id=?1)",
            [operation],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if !exists {
        return Err(invalid("The active transfer's history is unavailable."));
    }
    connection
        .query_row(
            "SELECT COALESCE(MAX(ordinal)+1,0) FROM changes WHERE operation=?1",
            [operation],
            |row| row.get(0),
        )
        .map_err(db_error)
}
fn budget(connection: &Connection) -> io::Result<()> {
    let (count, bytes): (u64, u64) = connection
        .query_row(
            "SELECT count(*),coalesce(sum(bytes),0) FROM request_transfer_cursors",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(db_error)?;
    if count > 1 {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "Review the earlier interrupted transfer before starting more work.",
        ));
    }
    if bytes > LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "Files cannot save more transfer progress. Its completed work remains protected.",
        ));
    }
    Ok(())
}
fn edits(connection: &Connection, scope: &Scope, edits: Vec<Edit>) -> io::Result<()> {
    for edit in edits {
        match edit {
            Edit::Put(Row { role, key, body }) => {
                let hash = row_digest(&scope.prefix, role, &key, &body);
                connection.execute("INSERT INTO request_transfer_tasks VALUES(?1,?2,?3,?4,?5) ON CONFLICT(request,role,position) DO UPDATE SET body=excluded.body,body_sha256=excluded.body_sha256", params![scope.request, role, key, body, &hash[..]]).map_err(db_error)?;
            }
            Edit::Remove { role, key } => {
                let changed = connection.execute("DELETE FROM request_transfer_tasks WHERE request=?1 AND role=?2 AND position=?3", params![scope.request,role,key]).map_err(db_error)?;
                if changed != 1 {
                    return Err(invalid("The active transfer task is missing."));
                }
            }
        }
    }
    Ok(())
}
impl Store {
    pub(in crate::operations) fn begin_active_transfer(
        &mut self,
        request: &RequestRecord,
        operation: u64,
        spec: &str,
        update: Update,
    ) -> io::Result<Scope> {
        self.ensure_location()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        require_idle(&transaction)?;
        let receipt = transfer_completion::read(&transaction, self.identity, request)?
            .ok_or_else(|| invalid("The accepted transfer execution is unavailable."))?;
        let (kind, destination) = transfer::cursor::scope(spec)?;
        receipt.validate_scope(kind, &destination, None)?;
        transfer_completion::require_active_operation(&transaction, request, operation)?;
        let digest = Sha256::digest(spec.as_bytes());
        let scope = scope(request, operation, &digest, self.problems.clone());
        let next = ordinal(&transaction, operation)?;
        let hash = progress_digest(&scope.prefix, next, &update.progress);
        transaction
            .execute(
                "INSERT INTO request_transfer_cursors VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    request.id,
                    next,
                    spec,
                    update.progress,
                    &digest[..],
                    &hash[..],
                    (spec.len() + update.progress.len() + 256) as u64
                ],
            )
            .map_err(db_error)?;
        edits(&transaction, &scope, update.edits)?;
        budget(&transaction)?;
        transaction.commit().map_err(db_error)?;
        Ok(scope)
    }
    pub(in crate::operations) fn update_active_transfer(
        &mut self,
        scope: &Scope,
        update: Update,
    ) -> io::Result<()> {
        self.update_transfer_cursor(scope, update, None)
    }
    pub(in crate::operations) fn prepare_active_directory(
        &mut self,
        scope: &Scope,
        update: Update,
        preparing: Option<&Pending>,
    ) -> io::Result<()> {
        self.update_transfer_cursor(scope, update, Some(preparing))
    }
    fn update_transfer_cursor(
        &mut self,
        scope: &Scope,
        update: Update,
        preparation: Option<Option<&Pending>>,
    ) -> io::Result<()> {
        if self.identity != scope.history || !Arc::ptr_eq(&self.problems, &scope.owner) {
            return Err(invalid(
                "The live transfer cursor belongs to another engine.",
            ));
        }
        self.ensure_location()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if preparation.is_some() {
            let pending: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pending UNION ALL SELECT 1 FROM recovery_pending)",
                    [],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            if pending {
                return Err(invalid(
                    "Reconcile the existing journal before preparing a directory expansion.",
                ));
            }
        } else {
            require_idle(&transaction)?;
        }
        let current: bool=transaction.query_row("SELECT EXISTS(SELECT 1 FROM requests r JOIN request_executions e ON e.request=r.id WHERE r.id=?1 AND r.revision=?2 AND r.phase=1 AND e.revision=?2 AND e.operation=?3)",params![scope.request,scope.revision,scope.operation],|row|row.get(0)).map_err(db_error)?;
        if !current {
            return Err(invalid(
                "The active transfer's scheduling revision changed.",
            ));
        }
        let next = ordinal(&transaction, scope.operation)?;
        if let Some(preparing) = preparation {
            let head = active_head(&transaction, self.identity, scope.operation)?
                .ok_or_else(|| invalid("The directory has no exact active transfer head."))?;
            if head.request != scope.request || head.first != next {
                return Err(invalid(
                    "The directory expansion crossed another filesystem step.",
                ));
            }
            let expansion = update
                .edits
                .iter()
                .find_map(|edit| match edit {
                    Edit::Put(row) if row.role == transfer::cursor::EXPANSION => Some(row),
                    _ => None,
                })
                .ok_or_else(|| invalid("The directory expansion descriptor is unavailable."))?;
            transfer::cursor::expansion::validate_prepared(
                &head.row,
                expansion,
                scope.operation,
                next,
                preparing,
            )?;
            if let Some(preparing) = preparing {
                source_retention::check_intent(&transaction, preparing)?;
                context::prepare(&transaction, self.identity, IntentLane::Primary, preparing)?;
                transaction
                    .execute(
                        "INSERT OR REPLACE INTO pending VALUES(1,?1)",
                        [encode(preparing)?],
                    )
                    .map_err(db_error)?;
            }
        }
        let hash = progress_digest(&scope.prefix, next, &update.progress);
        let changed=transaction.execute("UPDATE request_transfer_cursors SET ordinal=?2,progress=?3,progress_sha256=?4,bytes=bytes+length(CAST(?3 AS BLOB))-length(CAST(progress AS BLOB)) WHERE request=?1",params![scope.request,next,update.progress,&hash[..]]).map_err(db_error)?;
        if changed != 1 {
            return Err(invalid("The active transfer cursor is unavailable."));
        }
        transaction
            .execute(
                "DELETE FROM request_transfer_cursor_steps WHERE request=?1",
                [scope.request],
            )
            .map_err(db_error)?;
        edits(&transaction, scope, update.edits)?;
        budget(&transaction)?;
        transaction.commit().map_err(db_error)
    }
    pub(in crate::operations) fn recover_active_transfer(
        &mut self,
        request: &RequestRecord,
    ) -> io::Result<Option<Recovery>> {
        self.ensure_location()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let Some(Active {
            mut record,
            scope,
            next,
            current,
        }) = read_active(&transaction, self.identity, request, self.problems.clone())?
        else {
            return Ok(None);
        };
        require_idle(&transaction)?;
        let operation = scope.operation;
        if next != current {
            let steps = verified_steps(&transaction, &scope, next, current, &record.rows)?;
            transfer::cursor::advance(&mut record, operation, &steps)?;
        } else {
            transfer::cursor::expansion::advance_merge(&mut record, operation)?;
        }
        let receipt = transfer_completion::read(&transaction, self.identity, request)?
            .ok_or_else(|| invalid("The exact recovered transfer reservation changed."))?;
        let end = transfer::cursor::recover(record)?;
        let digest = match &end {
            transfer::cursor::Recovered::Completed(completion) => {
                let digest =
                    Sha256::digest(serde_json::to_vec(completion).map_err(io::Error::other)?)
                        .into();
                transfer_completion::validate_record(
                    &transaction,
                    self.identity,
                    request,
                    completion,
                    &digest,
                )?;
                digest
            }
            transfer::cursor::Recovered::Stopped(stop) => {
                receipt.validate_scope(
                    stop.report.kind,
                    &stop.report.destination,
                    stop.report.operation,
                )?;
                let digest =
                    Sha256::digest(serde_json::to_vec(stop).map_err(io::Error::other)?).into();
                transfer_stop::validate_record(
                    &transaction,
                    self.identity,
                    request,
                    stop,
                    &digest,
                )?;
                digest
            }
        };
        // Validation errors above remain Interrupted. Only this proved end can
        // survive metadata refusal as a process-local Retry Save witness.
        let result = (|| {
            match &end {
                transfer::cursor::Recovered::Completed(completion) => transfer_completion::record(
                    &transaction,
                    self.identity,
                    request,
                    completion,
                    &digest,
                )?,
                transfer::cursor::Recovered::Stopped(stop) => {
                    transfer_stop::record(&transaction, self.identity, request, stop, &digest)?
                }
            }
            requests::require_budget(&transaction)?;
            transaction.commit().map_err(db_error)
        })();
        Ok(Some(Recovery {
            end,
            error: result.err(),
        }))
    }
}
pub(super) fn retire(connection: &Connection, request: u64) -> io::Result<()> {
    connection
        .execute(
            "DELETE FROM request_transfer_cursors WHERE request=?1",
            [request],
        )
        .map_err(db_error)?;
    Ok(())
}

fn step_digest(prefix: &[u8; 32], ordinal: u64, key: &[u8], task: &[u8], body: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(prefix);
    hash.update(b"step\0");
    hash.update(ordinal.to_le_bytes());
    hash.update((key.len() as u64).to_le_bytes());
    hash.update(key);
    hash.update(task);
    hash.update(body.as_bytes());
    hash.finalize().into()
}

struct Head {
    request: u64,
    prefix: [u8; 32],
    first: u64,
    row: Row,
    task_sha: Vec<u8>,
}
fn active_head(connection: &Connection, history: Identity, id: u64) -> io::Result<Option<Head>> {
    type Binding = (u64, u64, Vec<u8>, u64, Vec<u8>, String, Vec<u8>);
    let binding: Option<Binding> = connection.query_row("SELECT c.request,e.revision,e.payload_sha256,c.ordinal,c.spec_sha256,c.progress,c.progress_sha256
        FROM request_transfer_cursors c JOIN request_executions e ON e.request=c.request JOIN requests r ON r.id=e.request
        WHERE e.operation=?1 AND e.revision=r.revision AND r.phase=1", [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?))).optional().map_err(db_error)?;
    let Some((request, revision, payload, first, spec, progress, progress_sha)) = binding else {
        return Ok(None);
    };
    let prefix = prefix(history, request, revision, id, &payload, &spec);
    if progress_sha != progress_digest(&prefix, first, &progress) {
        return Err(invalid("The active task cannot bind this filesystem step."));
    }
    let head: Option<(Vec<u8>,String,Vec<u8>)> = connection.query_row("SELECT position,body,body_sha256 FROM request_transfer_tasks WHERE request=?1 AND role=0 ORDER BY position LIMIT 1",[request],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional().map_err(db_error)?;
    let (key, task, task_sha) =
        head.ok_or_else(|| invalid("The active transfer has no durable head."))?;
    let row = Row {
        role: transfer::cursor::QUEUE,
        key,
        body: task,
    };
    if task_sha != row_digest(&prefix, row.role, &row.key, &row.body) {
        return Err(invalid(
            "The active task changed before its filesystem step.",
        ));
    }
    Ok(Some(Head {
        request,
        prefix,
        first,
        row,
        task_sha,
    }))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StageTask {
    pub(super) history_original: Option<original::Original>,
    request: u64,
    first: u64,
    prefix: [u8; 32],
    position: Vec<u8>,
    task: Vec<u8>,
    progress: Vec<u8>,
    #[serde(with = "crate::path_serde")]
    pub(super) source: PathBuf,
    #[serde(with = "crate::path_serde")]
    pub(super) destination: PathBuf,
}
/// Bounded verification of the original execution, header and head. Unrelated
/// tail rows are neither execution authority for cleanup nor traversed here.
pub(super) fn stage_task(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    id: u64,
) -> io::Result<Option<StageTask>> {
    if !matches!(lane, IntentLane::Primary) {
        return Ok(None);
    }
    type RequestRow = (u64, u64, Vec<u8>, i64);
    let row: Option<RequestRow> = connection.query_row("SELECT r.id,r.revision,r.body,r.phase FROM requests r JOIN request_executions e ON e.request=r.id WHERE e.operation=?1", [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(db_error)?;
    let Some((request, revision, body, phase)) = row else {
        return Ok(None);
    };
    if phase != 1 {
        return Err(invalid("The original staged-work request is not running."));
    }
    let request = RequestRecord {
        history,
        id: request,
        revision,
        body,
        phase: crate::operations::requests::RequestPhase::Running,
    };
    let receipt = transfer_completion::read(connection, history, &request)?
        .ok_or_else(|| invalid("The staged work lost its original execution receipt."))?;
    receipt.require_restart_history(connection, history)?;
    let head = active_head(connection, history, id)?
        .ok_or_else(|| invalid("The staged work lost its original active cursor."))?;
    if head.request != request.id {
        return Err(invalid("The staged-work request changed."));
    }
    let (spec, digest, progress): (String, Vec<u8>, Vec<u8>) = connection.query_row("SELECT spec,spec_sha256,progress_sha256 FROM request_transfer_cursors WHERE request=?1", [request.id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(db_error)?;
    if digest != Sha256::digest(spec.as_bytes()).as_slice() {
        return Err(invalid("The original staged-work specification changed."));
    }
    let (kind, destination) = transfer::cursor::scope(&spec)?;
    receipt.validate_scope(kind, &destination, None)?;
    let (source, destination) = transfer::cursor::stage_scope(&head.row)?;
    Ok(Some(StageTask {
        history_original: receipt.history_original()?,
        request: head.request,
        first: head.first,
        prefix: head.prefix,
        position: head.row.key,
        task: head.task_sha,
        progress,
        source,
        destination,
    }))
}
pub(super) fn stage_step(
    connection: &Connection,
    task: &StageTask,
    ordinal: u64,
    body: &str,
) -> io::Result<Vec<u8>> {
    type StepRow = (Vec<u8>, Vec<u8>, String, Vec<u8>);
    let row: Option<StepRow> = connection.query_row("SELECT position,task_sha256,body,body_sha256 FROM request_transfer_cursor_steps WHERE request=?1 AND ordinal=?2", params![task.request,ordinal], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(db_error)?;
    let (position, sha, recorded, digest) =
        row.ok_or_else(|| invalid("The cleanup lost its original filesystem-step receipt."))?;
    if ordinal < task.first
        || ordinal - task.first > 1
        || position != task.position
        || recorded != body
        || digest != step_digest(&task.prefix, ordinal, &position, &sha, body)
    {
        return Err(invalid(
            "The cleanup differs from its original filesystem-step receipt.",
        ));
    }
    Ok(digest)
}

fn bound_forward(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
) -> io::Result<Option<Head>> {
    if let Pending::StageMigration { migration } = pending
        && migration.has_claim()
    {
        saved_stage::forward::validate(migration)?;
        return bound_forward(
            connection,
            history,
            lane,
            &saved_stage::forward::virtual_forward(migration),
        );
    }
    if !matches!(lane, IntentLane::Primary) {
        return Ok(None);
    }
    let Pending::Forward {
        id,
        ordinal,
        change,
        stage,
    } = pending
    else {
        return Ok(None);
    };
    if !matches!(
        change,
        Change::Copied {
            source: Some(_),
            publication: Some(_),
            ..
        } | Change::CreatedFolder { .. }
            | Change::Rename {
                move_facts: Some(_),
                reviewed: None,
                ..
            }
            | Change::RemovedDirectory {
                removal: Some(_),
                moved: Some(_),
                ..
            }
            | Change::SourceRemoved {
                originals: Some(_),
                ..
            }
    ) && !matches!(change,Change::Rename {saved:Some(saved),..} if saved.copied_bytes().is_some())
    {
        return Ok(None);
    }
    let Some(Head {
        request,
        prefix,
        first,
        row,
        mut task_sha,
    }) = active_head(connection, history, *id)?
    else {
        return Ok(None);
    };
    if *ordinal < first {
        return Err(invalid("The active task cannot bind this filesystem step."));
    }
    // Other task shapes require their own restart phase.
    if *ordinal - first > 1 {
        return Ok(None);
    }
    if matches!(change, Change::CreatedFolder { .. }) {
        if *ordinal != first {
            return Err(invalid(
                "The directory publication crossed another filesystem step.",
            ));
        }
        let expansion = read_expansion(connection, request, &prefix, &row.key)?;
        if !transfer::cursor::expansion::binds(
            &row,
            &expansion,
            *id,
            first,
            change,
            stage.as_ref(),
        )? {
            return Err(invalid(
                "The directory Forward intent differs from its original expansion.",
            ));
        }
        task_sha = directory_digest(&prefix, &row, &expansion).to_vec();
    } else if matches!(change, Change::RemovedDirectory { .. }) {
        if *ordinal != first
            || !transfer::cursor::binds_directory_finish(&row, *id, *ordinal, change)?
        {
            return Err(invalid(
                "The source-directory removal differs from its exact postorder task.",
            ));
        }
    } else if change
        .retained()
        .is_some_and(|r| r.replacement_facts.is_some())
    {
        if *ordinal != first || !transfer::cursor::binds_reviewed_replacement(&row, change)? {
            return Err(invalid(
                "The reviewed replacement differs from its exact active task.",
            ));
        }
    } else if matches!(change, Change::Rename { .. }) {
        if *ordinal != first
            || !(transfer::cursor::binds_entry_move(&row, change)?
                || transfer::cursor::binds_saved_move(&row, change)?
                || transfer::cursor::binds_saved_copy(&row, change)?)
        {
            return Err(invalid(
                "The whole-entry Move differs from its exact active task.",
            ));
        }
    } else if !transfer::cursor::binds(&row, change)? {
        return Ok(None);
    }
    Ok(Some(Head {
        request,
        prefix,
        first,
        row,
        task_sha,
    }))
}

/// Check the existing receipt before recovery opens mutation targets. A valid
/// journal context alone cannot replace the active task's independent receipt.
pub(super) fn require_forward(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
) -> io::Result<()> {
    if let Pending::StageMigration { migration } = pending
        && migration.has_claim()
    {
        saved_stage::forward::validate(migration)?;
        return require_forward(
            connection,
            history,
            lane,
            &saved_stage::forward::virtual_forward(migration),
        );
    }
    let Some(head) = bound_forward(connection, history, lane, pending)? else {
        return Ok(());
    };
    let Pending::Forward {
        ordinal, change, ..
    } = pending
    else {
        unreachable!()
    };
    let body = encode(change)?;
    let sha = step_digest(&head.prefix, *ordinal, &head.row.key, &head.task_sha, &body);
    let exact: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM request_transfer_cursor_steps WHERE request=?1 AND ordinal=?2 AND position=?3 AND task_sha256=?4 AND body=?5 AND body_sha256=?6)", params![head.request, ordinal, head.row.key, head.task_sha, body, &sha[..]], |r| r.get(0)).map_err(db_error)?;
    if !exact {
        return Err(invalid(
            "The interrupted filesystem step lost its original active-task receipt.",
        ));
    }
    Ok(())
}

pub(super) fn bind_forward(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
) -> io::Result<()> {
    if let Pending::StageMigration { migration } = pending
        && migration.has_claim()
    {
        saved_stage::forward::validate(migration)?;
        return bind_forward(
            connection,
            history,
            lane,
            &saved_stage::forward::virtual_forward(migration),
        );
    }
    let Some(Head {
        request,
        prefix,
        row,
        task_sha,
        ..
    }) = bound_forward(connection, history, lane, pending)?
    else {
        return Ok(());
    };
    let Pending::Forward {
        id,
        ordinal,
        change,
        ..
    } = pending
    else {
        unreachable!()
    };
    let body = encode(change)?;
    if matches!(change, Change::Rename { .. })
        || change
            .retained()
            .is_some_and(|r| r.replacement_facts.is_some())
    {
        let previous: Option<String> = connection
            .query_row("SELECT body FROM pending WHERE singleton=1", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(db_error)?;
        if let Some(previous) = previous
            && let Pending::Forward {
                id: old_id,
                ordinal: old_ordinal,
                change: old_change,
                ..
            } = decode::<Pending>(&previous)?
        {
            if old_id != *id || old_ordinal != *ordinal || encode(&old_change)? != body {
                return Err(invalid(
                    "The transfer phase update differs from its original intent.",
                ));
            }
            let exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM request_transfer_cursor_steps WHERE request=?1 AND ordinal=?2)",params![request,ordinal],|r|r.get(0)).map_err(db_error)?;
            if !exists {
                return Err(invalid(
                    "The transfer phase update lost its original task receipt.",
                ));
            }
        }
    }
    let sha = step_digest(&prefix, *ordinal, &row.key, &task_sha, &body);
    connection
        .execute(
            "INSERT OR IGNORE INTO request_transfer_cursor_steps VALUES(?1,?2,?3,?4,?5,?6)",
            params![request, ordinal, row.key, task_sha, body, &sha[..]],
        )
        .map_err(db_error)?;
    let exact: bool = connection.query_row("SELECT position=?3 AND task_sha256=?4 AND body=?5 AND body_sha256=?6 FROM request_transfer_cursor_steps WHERE request=?1 AND ordinal=?2",params![request,ordinal,row.key,task_sha,body,&sha[..]],|row|row.get(0)).map_err(db_error)?;
    if !exact {
        return Err(invalid(
            "The filesystem step differs from its original task binding.",
        ));
    }
    budget(connection)
}

/// Only a descriptor-verified replacement refresh can rewrite its existing
/// canonical step receipt, in the same transaction as its committed history.
pub(super) fn refresh_replacement(
    connection: &Connection,
    history: Identity,
    id: u64,
    step: u64,
    before: &Change,
    after: &Change,
) -> io::Result<()> {
    let Some(head) = active_head(connection, history, id)? else {
        return Ok(());
    };
    if !transfer::cursor::binds_reviewed_replacement(&head.row, before)? {
        return if matches!(before, Change::Copied { source: None, .. }) {
            Ok(())
        } else {
            Err(invalid("The replacement refresh has no exact active task."))
        };
    }
    if step != head.first || ordinal(connection, id)? != step {
        return Err(invalid(
            "The replacement refresh crossed another filesystem step.",
        ));
    }
    let body = encode(before)?;
    let sha = step_digest(&head.prefix, step, &head.row.key, &head.task_sha, &body);
    let exact: bool = connection.query_row("SELECT count(*)=1 AND coalesce(min(ordinal=?2 AND position=?3 AND task_sha256=?4 AND body=?5 AND body_sha256=?6),0) FROM request_transfer_cursor_steps WHERE request=?1", params![head.request,step,head.row.key,head.task_sha,body,&sha[..]], |r| r.get(0)).map_err(db_error)?;
    if !exact {
        return Err(invalid("The replacement lost its original task receipt."));
    }
    let updated = encode(after)?;
    let digest = step_digest(&head.prefix, step, &head.row.key, &head.task_sha, &updated);
    connection.execute("UPDATE request_transfer_cursor_steps SET body=?3,body_sha256=?4 WHERE request=?1 AND ordinal=?2", params![head.request,step,updated,&digest[..]]).map_err(db_error)?;
    // Current supported refreshes change scalar version data only, not size;
    // maintain the bounded cursor budget even if future encodings differ.
    connection
        .execute(
            "UPDATE request_transfer_cursors SET bytes=bytes+?2 WHERE request=?1",
            params![head.request, updated.len() as i64 - body.len() as i64],
        )
        .map_err(db_error)?;
    budget(connection)
}

/// Called only with the independently verified Ready/Returning Forward context.
/// Retire its uncommitted removal, never the preceding committed Copy.
pub(super) fn retire_returned(
    connection: &Connection,
    history: Identity,
    id: u64,
    step: u64,
    change: &Change,
) -> io::Result<()> {
    if ordinal(connection, id)? != step {
        return Err(invalid(
            "A committed or out-of-order removal cannot be returned.",
        ));
    }
    let Some(head) = active_head(connection, history, id)? else {
        return Ok(());
    };
    type Receipt = (Vec<u8>, Vec<u8>, String, Vec<u8>);
    let receipt:Option<Receipt>=connection.query_row("SELECT position,task_sha256,body,body_sha256 FROM request_transfer_cursor_steps WHERE request=?1 AND ordinal=?2",params![head.request,step],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(db_error)?;
    let Some((position, task, body, sha)) = receipt else {
        if transfer::cursor::binds_entry_move(&head.row, change)?
            || transfer::cursor::binds_saved_move(&head.row, change)?
            || (change
                .retained()
                .is_some_and(|r| r.replacement_facts.is_some())
                && !matches!(change, Change::Copied { source: None, .. }))
        {
            return Err(invalid("The returned Move lost its original task receipt."));
        }
        return Ok(());
    };
    let bound = if change
        .retained()
        .is_some_and(|r| r.replacement_facts.is_some())
    {
        step == head.first && transfer::cursor::binds_reviewed_replacement(&head.row, change)?
    } else {
        transfer::cursor::binds(&head.row, change)?
            || (step == head.first && transfer::cursor::binds_entry_move(&head.row, change)?)
            || (step == head.first && transfer::cursor::binds_saved_move(&head.row, change)?)
            || transfer::cursor::binds_directory_finish(&head.row, id, step, change)?
    };
    if step < head.first
        || step - head.first > 1
        || position != head.row.key
        || task != head.task_sha
        || body != encode(change)?
        || sha != step_digest(&head.prefix, step, &position, &task, &body)
        || !bound
    {
        return Err(invalid(
            "The returned source differs from its exact active task proof.",
        ));
    }
    connection
        .execute(
            "DELETE FROM request_transfer_cursor_steps WHERE request=?1 AND ordinal=?2",
            params![head.request, step],
        )
        .map_err(db_error)?;
    Ok(())
}

fn verified_steps(
    connection: &Connection,
    scope: &Scope,
    first: u64,
    next: u64,
    rows: &[Row],
) -> io::Result<Vec<Change>> {
    if next < first || next - first > 2 {
        return Err(invalid(
            "The active cursor cannot prove its completed filesystem steps.",
        ));
    }
    let head = rows
        .iter()
        .find(|row| row.role == transfer::cursor::QUEUE)
        .ok_or_else(|| invalid("The completed filesystem step has no active task."))?;
    let task_sha = row_digest(&scope.prefix, head.role, &head.key, &head.body);
    let mut statement = connection.prepare("SELECT s.ordinal,s.position,s.task_sha256,s.body,s.body_sha256,c.body
        FROM request_transfer_cursor_steps s LEFT JOIN changes c ON c.operation=?2 AND c.ordinal=s.ordinal
        WHERE s.request=?1 ORDER BY s.ordinal").map_err(db_error)?;
    let mut query = statement
        .query(params![scope.request, scope.operation])
        .map_err(db_error)?;
    let mut steps = Vec::new();
    while let Some(row) = query.next().map_err(db_error)? {
        let ordinal: u64 = row.get(0).map_err(db_error)?;
        let key: Vec<u8> = row.get(1).map_err(db_error)?;
        let task: Vec<u8> = row.get(2).map_err(db_error)?;
        let body: String = row.get(3).map_err(db_error)?;
        let sha: Vec<u8> = row.get(4).map_err(db_error)?;
        let committed: Option<String> = row.get(5).map_err(db_error)?;
        let change: Change = decode(&body)?;
        let expected_task = if matches!(change, Change::CreatedFolder { .. }) {
            let expansion = rows
                .iter()
                .find(|row| row.role == transfer::cursor::EXPANSION && row.key == head.key)
                .ok_or_else(|| invalid("The original directory expansion is missing."))?;
            if !transfer::cursor::expansion::committed(
                head,
                expansion,
                scope.operation,
                first,
                &change,
            )? {
                return Err(invalid(
                    "The directory step differs from its original expansion.",
                ));
            }
            directory_digest(&scope.prefix, head, expansion).to_vec()
        } else {
            if change
                .retained()
                .is_some_and(|r| r.replacement_facts.is_some())
            {
                if !transfer::cursor::binds_reviewed_replacement(head, &change)? {
                    return Err(invalid(
                        "The completed replacement differs from its exact task.",
                    ));
                }
            } else if !(transfer::cursor::binds(head, &change)?
                || transfer::cursor::binds_entry_move(head, &change)?
                || transfer::cursor::binds_saved_move(head, &change)?
                || transfer::cursor::binds_saved_copy(head, &change)?
                || transfer::cursor::binds_directory_finish(
                    head,
                    scope.operation,
                    ordinal,
                    &change,
                )?)
            {
                return Err(invalid("The completed filesystem step changed."));
            }
            task_sha.to_vec()
        };
        if ordinal != first + steps.len() as u64
            || ordinal >= next
            || key != head.key
            || task != expected_task
            || sha != step_digest(&scope.prefix, ordinal, &key, &task, &body)
            || committed.as_deref() != Some(&body)
        {
            return Err(invalid(
                "A completed filesystem step lost its exact active-task proof.",
            ));
        }
        if encode(&change)? != body {
            return Err(invalid("The completed filesystem step changed."));
        }
        steps.push(change);
    }
    if steps.len() as u64 != next - first {
        return Err(invalid(
            "The saved progress does not cover a completed filesystem step. Review this interrupted transfer before continuing.",
        ));
    }
    Ok(steps)
}

fn read_expansion(
    connection: &Connection,
    request: u64,
    prefix: &[u8; 32],
    key: &[u8],
) -> io::Result<Row> {
    let (body, sha): (String, Vec<u8>) = connection.query_row("SELECT body,body_sha256 FROM request_transfer_tasks WHERE request=?1 AND role=?2 AND position=?3", params![request,transfer::cursor::EXPANSION,key], |r| Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    if sha != row_digest(prefix, transfer::cursor::EXPANSION, key, &body) {
        return Err(invalid(
            "The directory expansion changed before publication.",
        ));
    }
    Ok(Row {
        role: transfer::cursor::EXPANSION,
        key: key.to_vec(),
        body,
    })
}
fn directory_digest(prefix: &[u8; 32], head: &Row, expansion: &Row) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"directory-expansion\0");
    hash.update(row_digest(prefix, head.role, &head.key, &head.body));
    hash.update(row_digest(
        prefix,
        expansion.role,
        &expansion.key,
        &expansion.body,
    ));
    hash.finalize().into()
}

struct Active {
    record: Recorded,
    scope: Scope,
    next: u64,
    current: u64,
}
fn read_active(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
    problems: Problems,
) -> io::Result<Option<Active>> {
    type Header = (u64, String, String, Vec<u8>, Vec<u8>);
    let header:Option<Header>=connection.query_row("SELECT ordinal,spec,progress,spec_sha256,progress_sha256 FROM request_transfer_cursors WHERE request=?1",[request.id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).optional().map_err(db_error)?;
    let Some((next, spec, progress, spec_hash, progress_hash)) = header else {
        return Ok(None);
    };
    let receipt = transfer_completion::read(connection, history, request)?
        .ok_or_else(|| invalid("The original active transfer execution changed."))?;
    let (kind, destination) = transfer::cursor::scope(&spec)?;
    receipt.validate_scope(kind, &destination, None)?;
    receipt.require_restart_history(connection, history)?;
    let operation = transfer_completion::active_operation(connection, request)?;
    let scope = scope(request, operation, &spec_hash, problems);
    if spec_hash != Sha256::digest(spec.as_bytes()).as_slice()
        || progress_hash != progress_digest(&scope.prefix, next, &progress)
    {
        return Err(invalid(
            "The saved progress does not cover a completed filesystem step. Review this interrupted transfer before continuing.",
        ));
    }
    let rows = {
        let mut statement=connection.prepare("SELECT role,position,body,body_sha256 FROM request_transfer_tasks WHERE request=?1 ORDER BY role,position").map_err(db_error)?;
        let mut query = statement.query([request.id]).map_err(db_error)?;
        let mut rows = Vec::new();
        let mut bytes = spec.len() + progress.len();
        while let Some(row) = query.next().map_err(db_error)? {
            let role = row.get(0).map_err(db_error)?;
            let key: Vec<u8> = row.get(1).map_err(db_error)?;
            let body: String = row.get(2).map_err(db_error)?;
            let sha: Vec<u8> = row.get(3).map_err(db_error)?;
            bytes += key.len() + body.len() + 128;
            if bytes as u64 > LIMIT || sha != row_digest(&scope.prefix, role, &key, &body) {
                return Err(invalid(
                    "Saved transfer progress changed. Its files remain preserved for review.",
                ));
            }
            rows.push(Row { role, key, body });
        }
        rows
    };
    let record = Recorded {
        spec,
        progress,
        rows,
    };
    let current = ordinal(connection, operation)?;
    Ok(Some(Active {
        record,
        scope,
        next,
        current,
    }))
}

pub(super) fn prepared_expansion(
    connection: &Connection,
    history: Identity,
    problems: Problems,
    pending: &Pending,
) -> io::Result<Option<Pending>> {
    let Pending::Preparing { id, .. } = pending else {
        return Ok(None);
    };
    let Some(request) = directory_request(connection, history, *id)? else {
        return Ok(None);
    };
    let current: String = connection
        .query_row("SELECT body FROM pending WHERE singleton=1", [], |row| {
            row.get(0)
        })
        .map_err(db_error)?;
    if current != encode(pending)? {
        return Err(invalid("The captured directory preparation changed."));
    }
    let active = read_active(connection, history, &request, problems)?
        .ok_or_else(|| invalid("The exact active directory cursor is unavailable."))?;
    if !transfer::cursor::expansion::present(&active.record)? {
        return Ok(None);
    }
    if active.next != active.current {
        return Err(invalid(
            "The directory preparation crossed a filesystem step.",
        ));
    }
    let count: u64 = connection
        .query_row(
            "SELECT count(*) FROM request_transfer_cursor_steps WHERE request=?1",
            [request.id],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if count != 0 {
        return Err(invalid(
            "The unstarted directory already has a filesystem-step receipt.",
        ));
    }
    let forward =
        transfer::cursor::expansion::publication_intent(&active.record, *id, active.next, pending)?;
    Ok(Some(forward))
}

impl Store {
    /// Promote only a captured Preparing record whose independent exact active
    /// expansion proves the original request, history, objects and child graph.
    pub(in crate::operations) fn promote_directory_preparing(
        &mut self,
        pending: &Pending,
    ) -> io::Result<bool> {
        if !matches!(self.lane, IntentLane::Primary) {
            return Ok(false);
        }
        if !matches!(pending, Pending::Preparing { .. }) {
            return Ok(false);
        }
        self.ensure_location()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let Some(forward) =
            prepared_expansion(&transaction, self.identity, self.problems.clone(), pending)?
        else {
            return Ok(false);
        };
        source_retention::check_intent(&transaction, &forward)?;
        context::prepare(&transaction, self.identity, IntentLane::Primary, &forward)?;
        bind_forward(&transaction, self.identity, IntentLane::Primary, &forward)?;
        transaction
            .execute(
                "INSERT OR REPLACE INTO pending VALUES(1,?1)",
                [encode(&forward)?],
            )
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)?;
        Ok(true)
    }
    pub(in crate::operations) fn require_directory_publication(
        &self,
        pending: &Pending,
    ) -> io::Result<()> {
        self.ensure_location()?;
        if !matches!(self.lane, IntentLane::Primary) {
            return Ok(());
        }
        let Pending::Forward {
            id,
            ordinal: step,
            change,
            ..
        } = pending
        else {
            return Err(invalid("A directory publication needs its Forward intent."));
        };
        let Some(request) = directory_request(&self.connection, self.identity, *id)? else {
            return Ok(());
        };
        let active = read_active(
            &self.connection,
            self.identity,
            &request,
            self.problems.clone(),
        )?
        .ok_or_else(|| invalid("The exact directory cursor is unavailable."))?;
        if !active
            .record
            .rows
            .iter()
            .any(|row| row.role == transfer::cursor::EXPANSION)
        {
            return Err(invalid("The original directory expansion is unavailable."));
        }
        if active.next != active.current || active.next != *step {
            return Err(invalid(
                "The original directory publication crossed another step.",
            ));
        }
        transfer::cursor::expansion::publication_intent(&active.record, *id, *step, pending)?;
        // Validate the existing pre-mutation receipt; never recreate a missing one.
        let head = active
            .record
            .rows
            .iter()
            .find(|row| row.role == transfer::cursor::QUEUE)
            .unwrap();
        let expansion = active
            .record
            .rows
            .iter()
            .find(|row| row.role == transfer::cursor::EXPANSION)
            .unwrap();
        let task = directory_digest(&active.scope.prefix, head, expansion);
        let body = encode(change)?;
        let sha = step_digest(&active.scope.prefix, *step, &head.key, &task, &body);
        let exact: bool = self.connection.query_row("SELECT count(*)=1 AND coalesce(min(ordinal=?2 AND position=?3 AND task_sha256=?4 AND body=?5 AND body_sha256=?6),0) FROM request_transfer_cursor_steps WHERE request=?1", params![request.id,step,head.key,&task[..],body,&sha[..]], |row| row.get(0)).map_err(db_error)?;
        if !exact {
            return Err(invalid(
                "The directory lost its original pre-mutation receipt.",
            ));
        }
        Ok(())
    }
}
fn directory_request(
    connection: &Connection,
    history: Identity,
    operation: u64,
) -> io::Result<Option<RequestRecord>> {
    let row: Option<(u64,u64,Vec<u8>)> = connection.query_row("SELECT r.id,r.revision,r.body FROM requests r JOIN request_executions e ON e.request=r.id JOIN request_transfer_cursors c ON c.request=r.id WHERE e.operation=?1 AND r.phase=1", [operation], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional().map_err(db_error)?;
    Ok(row.map(|(id, revision, body)| RequestRecord {
        history,
        id,
        revision,
        phase: crate::operations::requests::RequestPhase::Running,
        body,
    }))
}
