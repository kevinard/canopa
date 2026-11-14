//! One journal intent for a batch of new-file copies (ADR0180).
//!
//! A `Pending::CopyBatch` keeps the lane's single pending intent and describes
//! every file of the batch, so registration, the Forward intent and the
//! completion each take one transaction per batch instead of one per file. Its
//! binding is one digest over the encoded batch with the history's identity
//! and generation, like the single-item `stage_intents` binding.
use super::*;
use crate::operations::copy_batch::{CopyBatch, Phase};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    history: Identity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original: Option<original::Original>,
    #[serde(serialize_with = "serialized::bytes")]
    digest: [u8; 32],
}

pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS copy_batch_intents (
        lane INTEGER PRIMARY KEY CHECK(lane IN (0,1)),
        body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=4096));",
        )
        .map_err(db_error)
}

pub(super) fn clear(connection: &Connection, lane: IntentLane) -> io::Result<()> {
    connection
        .execute_cached(
            "DELETE FROM copy_batch_intents WHERE lane=?1",
            [lane.index()],
        )
        .map_err(db_error)?;
    Ok(())
}

fn digest(history: Identity, lane: IntentLane, pending: &Pending) -> io::Result<[u8; 32]> {
    Ok(serialized::tagged_digest(
        b"canopa-files-copy-batch-v1\0",
        encode(&(history, lane.index(), pending))?.as_bytes(),
    ))
}

fn pending_body(connection: &Connection, lane: IntentLane) -> io::Result<Option<String>> {
    connection
        .query_row_cached(
            match lane {
                IntentLane::Primary => "SELECT body FROM pending WHERE singleton=1",
                IntentLane::RecoveryCopy => "SELECT body FROM recovery_pending WHERE singleton=1",
            },
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)
}

fn write(connection: &Connection, lane: IntentLane, pending: &Pending) -> io::Result<()> {
    connection
        .execute_cached(
            match lane {
                IntentLane::Primary => "INSERT OR REPLACE INTO pending VALUES(1,?1)",
                IntentLane::RecoveryCopy => "INSERT OR REPLACE INTO recovery_pending VALUES(1,?1)",
            },
            [encode(pending)?],
        )
        .map_err(db_error)?;
    Ok(())
}

fn install(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
    original: Option<original::Original>,
) -> io::Result<()> {
    let binding = Binding {
        history,
        original,
        digest: digest(history, lane, pending)?,
    };
    connection
        .execute_cached(
            "INSERT INTO copy_batch_intents(lane,body) VALUES(?1,?2) ON CONFLICT(lane) DO UPDATE SET body=excluded.body",
            params![lane.index(), encode(&binding)?],
        )
        .map_err(db_error)?;
    write(connection, lane, pending)
}

fn next(connection: &Connection, id: u64) -> io::Result<u64> {
    connection
        .query_row_cached(
            "SELECT COALESCE(MAX(ordinal)+1,0) FROM changes WHERE operation=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(db_error)
}

/// The saved batch must be exactly `batch`, bound to this history and its
/// original generation. Returns the binding's history generation.
pub(super) fn require(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    batch: &CopyBatch,
) -> io::Result<Option<original::Original>> {
    let refused = || {
        invalid(
            "The interrupted copies differ from their recorded batch. Their files are preserved for Recovery.",
        )
    };
    let pending = Pending::CopyBatch {
        batch: Box::new(batch.clone()),
    };
    if pending_body(connection, lane)?.as_deref() != Some(encode(&pending)?.as_str()) {
        return Err(refused());
    }
    let body: String = connection
        .query_row_cached(
            "SELECT body FROM copy_batch_intents WHERE lane=?1",
            [lane.index()],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?
        .ok_or_else(refused)?;
    let binding: Binding = decode(&body)?;
    if binding.history != history || binding.digest != digest(history, lane, &pending)? {
        return Err(refused());
    }
    if let Some(original) = binding.original {
        original.require(&transfer_completion::history_file(connection, history)?)?;
    }
    let exists: bool = connection
        .query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM operations WHERE id=?1)",
            [batch.id],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if !exists {
        return Err(refused());
    }
    Ok(binding.original)
}

impl Store {
    /// Register a new batch: its empty stage and the files it will copy.
    pub(in crate::operations) fn begin_copy_batch(&mut self, batch: &CopyBatch) -> io::Result<()> {
        self.ensure_location()?;
        batch.validate()?;
        if batch.phase != Phase::Preparing
            || batch
                .items
                .iter()
                .any(|item| item.payload.is_some() || item.change.is_some())
        {
            return Err(invalid("A new copy batch starts empty."));
        }
        let pending = Pending::CopyBatch {
            batch: Box::new(batch.clone()),
        };
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if pending_body(&transaction, self.lane)?.is_some() {
            return Err(invalid("Other interrupted work needs review first."));
        }
        let exists: bool = transaction
            .query_row_cached(
                "SELECT EXISTS(SELECT 1 FROM operations WHERE id=?1)",
                [batch.id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if !exists {
            return Err(invalid("A copy batch needs its operation."));
        }
        let original = transfer_completion::history_file(&transaction, self.identity)
            .ok()
            .and_then(|file| original::Original::capture(&file).ok());
        install(&transaction, self.identity, self.lane, &pending, original)?;
        transaction.commit().map_err(db_error)
    }

    /// Save the next phase of the batch the journal holds now.
    pub(in crate::operations) fn advance_copy_batch(
        &mut self,
        before: &CopyBatch,
        after: &CopyBatch,
    ) -> io::Result<()> {
        self.ensure_location()?;
        after.validate()?;
        before.transition(after)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let original = require(&transaction, self.identity, self.lane, before)?;
        if let Phase::Forward { first } = after.phase
            && before.phase == Phase::Preparing
        {
            if next(&transaction, after.id)? != first {
                return Err(invalid("The copy batch crossed another filesystem step."));
            }
            let changes: Vec<&Change> = after
                .items
                .iter()
                .filter_map(|item| item.change.as_ref())
                .collect();
            transfer_cursor::bind_copy_batch(
                &transaction,
                self.identity,
                after.id,
                first,
                &changes,
            )?;
        }
        let pending = Pending::CopyBatch {
            batch: Box::new(after.clone()),
        };
        install(&transaction, self.identity, self.lane, &pending, original)?;
        transaction.commit().map_err(db_error)
    }

    /// Append the published prefix of a Forward batch and keep its cleanup,
    /// in one transaction. Returns the saved Cleanup phase.
    pub(in crate::operations) fn complete_copy_batch(
        &mut self,
        before: &CopyBatch,
        published: usize,
    ) -> io::Result<CopyBatch> {
        self.ensure_location()?;
        let Phase::Forward { first } = before.phase else {
            return Err(invalid("Only a Forward copy batch can complete."));
        };
        let changes: Vec<&Change> = before
            .items
            .iter()
            .filter_map(|item| item.change.as_ref())
            .collect();
        if published > changes.len() {
            return Err(invalid("A copy batch cannot publish more than it copied."));
        }
        let mut after = before.clone();
        after.phase = Phase::Cleanup { first, published };
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let original = require(&transaction, self.identity, self.lane, before)?;
        if next(&transaction, before.id)? != first {
            return Err(invalid("The copy batch crossed another filesystem step."));
        }
        transfer_cursor::retire_copy_batch_steps(
            &transaction,
            self.identity,
            before.id,
            first + published as u64,
        )?;
        for (offset, change) in changes.into_iter().take(published).enumerate() {
            append_rows(
                &transaction,
                self.lane,
                before.id,
                first + offset as u64,
                change,
            )?;
        }
        let pending = Pending::CopyBatch {
            batch: Box::new(after.clone()),
        };
        install(&transaction, self.identity, self.lane, &pending, original)?;
        transaction.commit().map_err(db_error)?;
        Ok(after)
    }

    /// Check the journal still holds exactly `batch` before recovery touches
    /// any of its files.
    pub(in crate::operations) fn require_copy_batch(&self, batch: &CopyBatch) -> io::Result<()> {
        require(&self.connection, self.identity, self.lane, batch).map(|_| ())
    }
}

/// A stuck batch reviewed in Recovery: the exact journal, its binding and the
/// stage version shown to the person who chose Keep.
#[derive(Clone, Debug)]
pub(in crate::operations) struct Review {
    lane: IntentLane,
    pending: String,
    binding: String,
    version: fingerprint::Version,
}

fn bodies(connection: &Connection, lane: IntentLane) -> io::Result<Option<(String, String)>> {
    let Some(pending) = pending_body(connection, lane)? else {
        return Ok(None);
    };
    let binding: Option<String> = connection
        .query_row_cached(
            "SELECT body FROM copy_batch_intents WHERE lane=?1",
            [lane.index()],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    Ok(binding.map(|binding| (pending, binding)))
}

impl Store {
    pub(in crate::operations) fn copy_batch_review(&self, slot: u8) -> io::Result<Option<Review>> {
        let Some((lane, 0)) = interrupted::pending_position(slot) else {
            return Ok(None);
        };
        if self.hide_running_journals && self.recovery_problem(lane).is_none() {
            return Ok(None);
        }
        let Some((pending, binding)) = bodies(&self.connection, lane)? else {
            return Ok(None);
        };
        let decoded: Pending = decode(&pending)?;
        let Pending::CopyBatch { batch } = &decoded else {
            return Ok(None);
        };
        require(&self.connection, self.identity, lane, batch)?;
        let record = decoded
            .record(0, String::new())
            .ok_or_else(|| invalid("Invalid interrupted copies."))?;
        Ok(Some(Review {
            lane,
            pending,
            binding,
            version: record.available()?,
        }))
    }

    /// Keep a stuck batch's whole stage as one Recovery work folder and retire
    /// its journal. Nothing is moved, removed or appended to history.
    pub(in crate::operations) fn keep_copy_batch(&mut self, reviewed: &Review) -> io::Result<()> {
        self.ensure_location()?;
        let pending: Pending = decode(&reviewed.pending)?;
        let Pending::CopyBatch { batch } = &pending else {
            return Err(invalid("Expected interrupted copies."));
        };
        let mut record = pending
            .record(0, "Kept interrupted working files".into())
            .ok_or_else(|| invalid("Invalid interrupted copies."))?;
        let version = record.available()?;
        if version != reviewed.version {
            return Err(invalid(
                "Interrupted work changed; refresh Recovery before keeping it.",
            ));
        }
        record.retained.version = Some(version);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if bodies(&transaction, reviewed.lane)?
            != Some((reviewed.pending.clone(), reviewed.binding.clone()))
        {
            return Err(invalid(
                "Interrupted work changed; refresh Recovery before keeping it.",
            ));
        }
        require(&transaction, self.identity, reviewed.lane, batch)?;
        let body = encode(&record)?;
        if transaction
            .execute("INSERT INTO garbage(body) VALUES(?1)", [&body])
            .map_err(db_error)?
            != 1
        {
            return Err(invalid("The retained work was not recorded."));
        }
        let entry = u64::try_from(transaction.last_insert_rowid()).map_err(io::Error::other)?;
        paths::index_record(&transaction, entry, &record)?;
        paths::require_retained_record(&transaction, entry, &record)?;
        clear_intent(&transaction, reviewed.lane)?;
        transaction.commit().map_err(db_error)?;
        self.problems
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[reviewed.lane.index()] = None;
        Ok(())
    }
}
