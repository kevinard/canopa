//! A typed end is a historical completion fact, never authority to replay files.
use super::*;
use crate::operations::requests::{RequestPhase, RequestRecord};
use sha2::{Digest, Sha256};

pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS request_mutation_ends (
        request INTEGER PRIMARY KEY REFERENCES request_executions(request) ON DELETE CASCADE,
        intent TEXT NOT NULL CHECK(length(CAST(intent AS BLOB))<=4194304),
        intent_sha256 BLOB NOT NULL CHECK(length(intent_sha256)=32),
        ended INTEGER NOT NULL DEFAULT 0 CHECK(ended IN (0,1)));",
        )
        .map_err(db_error)
}

pub(super) fn reserve(
    connection: &Connection,
    request: &RequestRecord,
    change: &Change,
) -> io::Result<()> {
    require_kind(change)?;
    let intent = encode(change)?;
    connection
        .execute(
            "INSERT INTO request_mutation_ends(request,intent,intent_sha256) VALUES(?1,?2,?3)",
            params![request.id, intent, &Sha256::digest(intent.as_bytes())[..]],
        )
        .map_err(db_error)?;
    Ok(())
}

struct Receipt {
    revision: u64,
    operation: u64,
    label: String,
    payload: Vec<u8>,
    intent: String,
    digest: Vec<u8>,
    ended: bool,
    current_revision: u64,
    current_phase: u8,
    current_body: Vec<u8>,
}

fn read(connection: &Connection, request: u64) -> io::Result<Option<Receipt>> {
    connection.query_row(
        "SELECT e.revision,e.operation,e.label,e.payload_sha256,m.intent,m.intent_sha256,m.ended,
         r.revision,r.phase,r.body FROM request_executions e
         JOIN request_mutation_ends m ON m.request=e.request JOIN requests r ON r.id=e.request
         WHERE e.request=?1",
        [request], |row| Ok(Receipt {
            revision: row.get(0)?, operation: row.get(1)?, label: row.get(2)?,
            payload: row.get(3)?, intent: row.get(4)?, digest: row.get(5)?, ended: row.get(6)?,
            current_revision: row.get(7)?, current_phase: row.get(8)?, current_body: row.get(9)?,
        }),
    ).optional().map_err(db_error)
}

impl Receipt {
    fn matches(&self, request: &RequestRecord) -> bool {
        request.phase == RequestPhase::Running
            && self.revision == request.revision
            && self.current_revision == request.revision
            && self.current_phase == RequestPhase::Running as u8
            && self.current_body == request.body
            && self.payload.as_slice() == Sha256::digest(&request.body).as_slice()
    }
    fn change(&self) -> io::Result<Change> {
        if self.digest.as_slice() != Sha256::digest(self.intent.as_bytes()).as_slice() {
            return Err(invalid("The mutation completion record changed."));
        }
        let change: Change = decode(&self.intent)?;
        require_kind(&change)?;
        validate_change(&change)?;
        Ok(change)
    }
}

fn require_kind(change: &Change) -> io::Result<()> {
    if matches!(change, Change::CreatedFolder { .. } | Change::Rename { .. }) {
        Ok(())
    } else {
        Err(invalid("Unsupported ordinary mutation completion record"))
    }
}

pub(super) fn completed(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
) -> io::Result<Option<OperationSummary>> {
    if request.history != history {
        return Err(invalid("The request belongs to another operation history"));
    }
    let Some(receipt) = read(connection, request.id)? else {
        return Ok(None);
    };
    if !receipt.ended || !receipt.matches(request) {
        return Ok(None);
    }
    let change = receipt.change()?;
    Ok(Some(OperationSummary {
        id: receipt.operation,
        label: receipt.label,
        paths: vec![change.forward_path().to_owned()],
    }))
}

impl Store {
    pub(in crate::operations) fn completed_mutation(
        &self,
        request: &RequestRecord,
    ) -> io::Result<Option<OperationSummary>> {
        completed(&self.connection, self.identity, request)
    }

    /// Called only with the live engine's post-success witness. Its fixed end
    /// bit adds no payload bytes beyond the reservation made before mutation.
    pub(in crate::operations) fn record_mutation_end(
        &mut self,
        request: &RequestRecord,
        operation: u64,
        expected: &[u8; 32],
    ) -> io::Result<()> {
        self.ensure_location()?;
        if request.history != self.identity {
            return Err(invalid("The request belongs to another operation history"));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let receipt = read(&transaction, request.id)?.ok_or_else(|| {
            invalid("The original mutation completion reservation is unavailable.")
        })?;
        if !receipt.matches(request)
            || receipt.operation != operation
            || receipt.digest.as_slice() != expected
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "The exact mutation completion changed; review it before saving.",
            ));
        }
        receipt.change()?;
        if !receipt.ended {
            // A retained step must agree. History may already have expired
            // while this same live witness still protects the metadata retry.
            let step: Option<String> = transaction
                .query_row(
                    "SELECT body FROM changes WHERE operation=?1 AND ordinal=0",
                    [operation],
                    |row| row.get(0),
                )
                .optional()
                .map_err(db_error)?;
            if step.is_some_and(|body| body != receipt.intent) {
                return Err(invalid("The confirmed mutation step changed."));
            }
            transaction
                .execute(
                    "UPDATE request_mutation_ends SET ended=1 WHERE request=?1",
                    [request.id],
                )
                .map_err(db_error)?;
        }
        transaction.commit().map_err(db_error)
    }
}
