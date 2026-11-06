//! Exact accepted execution plus a typed successful end; no pathname replay.
use super::*;
use crate::operations::{
    execution::CompletedTransfer,
    requests::{RequestPhase, RequestRecord},
    transfer::{RestartObservation, TransferKind, TransferRequest},
};
use sha2::{Digest, Sha256};

// Completion records include the complete selection, rather than one filesystem
// step. Match this table's existing bound without widening the step codec.
const COMPLETION_LIMIT: usize = 4 * 1024 * 1024;

fn encode(value: &impl Serialize) -> io::Result<String> {
    let body = serde_json::to_string(value).map_err(io::Error::other)?;
    if body.len() > COMPLETION_LIMIT {
        return Err(invalid("transfer completion exceeds its storage budget"));
    }
    Ok(body)
}

fn decode<T: serde::de::DeserializeOwned>(body: &str) -> io::Result<T> {
    if body.len() > COMPLETION_LIMIT {
        return Err(invalid("transfer completion exceeds its storage budget"));
    }
    serde_json::from_str(body).map_err(io::Error::other)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    history_original: Option<crate::operations::original::Original>,
    kind: TransferKind,
    #[serde(with = "crate::path_serde")]
    destination: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original: Option<RestartObservation>,
}

thread_local! {
    static INTENT: std::cell::RefCell<serialized::Last<Intent>> =
        const { std::cell::RefCell::new(serialized::Last::new()) };
}
fn intent(body: &str) -> io::Result<std::sync::Arc<Intent>> {
    INTENT.with(|cached| cached.borrow_mut().read(body, decode))
}

pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS request_transfer_ends (
        request INTEGER PRIMARY KEY REFERENCES request_executions(request) ON DELETE CASCADE,
        intent TEXT NOT NULL CHECK(length(CAST(intent AS BLOB))<=4194304),
        intent_sha256 BLOB NOT NULL CHECK(length(intent_sha256)=32),
        result TEXT CHECK(length(CAST(result AS BLOB))<=4194304),
        result_sha256 BLOB CHECK(length(result_sha256)=32),
        CHECK((result IS NULL)=(result_sha256 IS NULL)));",
        )
        .map_err(db_error)
}

pub(super) fn reserve(
    connection: &Connection,
    request: &RequestRecord,
    kind: TransferKind,
    destination: &Path,
    original: Option<&RestartObservation>,
) -> io::Result<()> {
    if original.is_some_and(|original| {
        !original.covers(&TransferRequest {
            kind,
            destination: destination.to_owned(),
            sources: Vec::new(),
        })
    }) {
        return Err(invalid(
            "The original selection does not match its transfer execution.",
        ));
    }
    let intent = encode(&Intent {
        history_original: history_file(connection, request.history)
            .ok()
            .and_then(|file| crate::operations::original::Original::capture(&file).ok()),
        kind,
        destination: destination.to_owned(),
        original: original.cloned(),
    })?;
    connection
        .execute_cached(
            "INSERT INTO request_transfer_ends(request,intent,intent_sha256) VALUES(?1,?2,?3)",
            params![request.id, intent, &Sha256::digest(intent.as_bytes())[..]],
        )
        .map_err(db_error)?;
    Ok(())
}

pub(super) struct Receipt {
    operation: u64,
    intent: String,
    intent_digest: Vec<u8>,
    pub(super) result: Option<String>,
    digest: Option<Vec<u8>>,
}

pub(super) fn read(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
) -> io::Result<Option<Receipt>> {
    if request.history != history {
        return Err(invalid("The request belongs to another operation history"));
    }
    if request.phase != RequestPhase::Running {
        return Ok(None);
    }
    connection.query_row_cached("SELECT e.operation,t.intent,t.intent_sha256,t.result,t.result_sha256
        FROM request_executions e JOIN request_transfer_ends t ON t.request=e.request JOIN requests r ON r.id=e.request
        WHERE e.request=?1 AND e.revision=?2 AND e.payload_sha256=?3 AND r.revision=?2 AND r.phase=1 AND r.body=?4",
        params![request.id,request.revision,&serialized::digest(&request.body)[..],request.body],
        |row| Ok(Receipt { operation:row.get(0)?,intent:row.get(1)?,intent_digest:row.get(2)?,result:row.get(3)?,digest:row.get(4)? }))
        .optional().map_err(db_error)
}

pub(super) fn active_operation(
    connection: &Connection,
    request: &RequestRecord,
) -> io::Result<u64> {
    read(connection, request.history, request)?
        .map(|receipt| receipt.operation)
        .ok_or_else(|| invalid("The exact active transfer execution is unavailable."))
}
pub(super) fn require_active_operation(
    connection: &Connection,
    request: &RequestRecord,
    operation: u64,
) -> io::Result<()> {
    if active_operation(connection, request)? != operation {
        return Err(invalid("The transfer cursor belongs to another operation."));
    }
    Ok(())
}

impl Receipt {
    pub(super) fn history_original(&self) -> io::Result<Option<original::Original>> {
        Ok(intent(&self.intent)?.history_original)
    }
    pub(super) fn require_restart_history(
        &self,
        connection: &Connection,
        history: Identity,
    ) -> io::Result<()> {
        let intent = intent(&self.intent)?;
        intent
            .history_original
            .ok_or_else(|| invalid("This older transfer has no original history-file proof."))?
            .require(&history_file(connection, history)?)
    }
    fn validate(&self, result: &CompletedTransfer) -> io::Result<()> {
        if !result.valid() {
            return Err(invalid("The transfer completion is invalid."));
        }
        self.validate_scope(result.kind, &result.destination, result.operation)
    }

    pub(super) fn validate_scope(
        &self,
        kind: TransferKind,
        destination: &Path,
        operation: Option<u64>,
    ) -> io::Result<()> {
        if self.intent_digest.as_slice() != serialized::digest(self.intent.as_bytes()).as_slice() {
            return Err(invalid("The transfer completion intent changed."));
        }
        let intent = intent(&self.intent)?;
        if intent.original.as_ref().is_some_and(|original| {
            !original.covers(&TransferRequest {
                kind: intent.kind,
                destination: intent.destination.clone(),
                sources: Vec::new(),
            })
        }) {
            return Err(invalid("The original transfer selection changed."));
        }
        if kind != intent.kind
            || destination != intent.destination
            || operation.is_some_and(|operation| operation != self.operation)
        {
            return Err(invalid(
                "The transfer result does not match its accepted execution.",
            ));
        }
        Ok(())
    }
}

pub(super) fn history_file(connection: &Connection, history: Identity) -> io::Result<fs::File> {
    use rustix::fs::{Mode, OFlags, open};
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let bytes: Vec<u8> = connection
        .query_row_cached(
            "SELECT CAST(file AS BLOB) FROM pragma_database_list WHERE name='main'",
            [],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    let path = PathBuf::from(OsString::from_vec(bytes));
    let file = fs::File::from(open(
        &path,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    if Identity::from_metadata(&file.metadata()?) != history {
        return Err(invalid("The original operation history changed."));
    }
    Ok(file)
}

pub(super) fn require_saved_scope(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
    stop: &crate::operations::transfer::StoppedTransfer,
) -> io::Result<()> {
    let receipt = connection.query_row_cached("SELECT e.operation,t.intent,t.intent_sha256,t.result,t.result_sha256 FROM request_executions e JOIN request_transfer_ends t ON t.request=e.request WHERE e.request=?1", [request.id],
        |row| Ok(Receipt { operation: row.get(0)?, intent: row.get(1)?, intent_digest: row.get(2)?, result: row.get(3)?, digest: row.get(4)? })).optional().map_err(db_error)?
        .ok_or_else(|| invalid("The original transfer execution is unavailable."))?;
    receipt.validate_scope(
        stop.report.kind,
        &stop.report.destination,
        stop.report.operation,
    )?;
    if receipt.result.is_some() {
        return Err(invalid("This transfer already recorded a completed end."));
    }
    receipt.require_restart_history(connection, history)
}

pub(super) fn completed(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
) -> io::Result<Option<CompletedTransfer>> {
    let Some(receipt) = read(connection, history, request)? else {
        return Ok(None);
    };
    let Some(body) = &receipt.result else {
        return Ok(None);
    };
    if receipt.digest.as_deref() != Some(Sha256::digest(body.as_bytes()).as_slice()) {
        return Err(invalid("The recorded transfer completion changed."));
    }
    require_no_stop(connection, request)?;
    let result = decode(body)?;
    receipt.validate(&result)?;
    Ok(Some(result))
}

fn require_no_stop(connection: &Connection, request: &RequestRecord) -> io::Result<()> {
    let stopped: bool = connection
        .query_row_cached(
            "SELECT EXISTS(SELECT 1 FROM request_transfer_stops WHERE request=?1)",
            [request.id],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if stopped {
        return Err(invalid(
            "This transfer has a recorded stopped result, not a completed end.",
        ));
    }
    Ok(())
}

impl Store {
    pub(in crate::operations) fn completed_transfer(
        &self,
        request: &RequestRecord,
    ) -> io::Result<Option<CompletedTransfer>> {
        completed(&self.connection, self.identity, request)
    }

    pub(in crate::operations) fn has_transfer_reservation(
        &self,
        request: &RequestRecord,
    ) -> io::Result<bool> {
        Ok(read(&self.connection, self.identity, request)?.is_some())
    }

    pub(in crate::operations) fn record_transfer_end(
        &mut self,
        request: &RequestRecord,
        completion: &CompletedTransfer,
        digest: &[u8; 32],
    ) -> io::Result<()> {
        self.ensure_location()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        record(&transaction, self.identity, request, completion, digest)?;
        requests::require_budget(&transaction)?;
        transaction.commit().map_err(db_error)
    }
}

/// Both ordinary execution and proved cursor completion use this transaction
/// body. The end and cursor retirement must commit or roll back together.
pub(super) fn record(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
    completion: &CompletedTransfer,
    digest: &[u8; 32],
) -> io::Result<()> {
    let body = validate_record(connection, history, request, completion, digest)?;
    connection
        .execute_cached(
            "UPDATE request_transfer_ends SET result=?2,result_sha256=?3 WHERE request=?1 AND result IS NULL",
            params![request.id, body, &digest[..]],
        )
        .map_err(db_error)?;
    transfer_cursor::retire(connection, request.id)?;
    Ok(())
}

/// All evidence checks precede the first metadata write, including on recovery.
pub(super) fn validate_record(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
    completion: &CompletedTransfer,
    digest: &[u8; 32],
) -> io::Result<String> {
    let receipt = read(connection, history, request)?
        .ok_or_else(|| invalid("The exact transfer completion reservation is unavailable."))?;
    require_no_stop(connection, request)?;
    receipt.validate(completion)?;
    let body = encode(completion)?;
    if digest.as_slice() != Sha256::digest(body.as_bytes()).as_slice() {
        return Err(invalid("The live transfer completion changed."));
    }
    if let Some(previous) = receipt.result
        && (previous != body || receipt.digest.as_deref() != Some(digest.as_slice()))
    {
        return Err(invalid(
            "The original recorded transfer completion changed.",
        ));
    }
    Ok(body)
}
