//! Engine-returned stopped results survive a lost application checkpoint. The
//! receipt binds exact parts; restoration must also reopen and prove originals.
use super::*;
use crate::operations::{
    requests::{RequestPhase, RequestRecord},
    transfer::{StoppedTransfer, TransferResolution},
};
use sha2::{Digest, Sha256};

pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS request_transfer_stops (
        request INTEGER PRIMARY KEY REFERENCES request_executions(request) ON DELETE CASCADE,
        body TEXT CHECK(length(CAST(body AS BLOB))<=4194304),
        body_sha256 BLOB NOT NULL CHECK(length(body_sha256)=32),
        report_sha256 BLOB NOT NULL CHECK(length(report_sha256)=32),
        retry_sha256 BLOB CHECK(length(retry_sha256)=32),
        remaining_sha256 BLOB CHECK(length(remaining_sha256)=32),
        phase INTEGER NOT NULL CHECK(phase IN (2,3,4)),
        saved_revision INTEGER CHECK(saved_revision>0),
        saved_phase INTEGER CHECK(saved_phase BETWEEN 2 AND 5),
        saved_body_sha256 BLOB CHECK(length(saved_body_sha256)=32),
        CHECK((saved_revision IS NULL)=(saved_phase IS NULL)),
        CHECK((saved_revision IS NULL)=(saved_body_sha256 IS NULL)),
        CHECK((body IS NULL)=(saved_revision IS NOT NULL)));",
        )
        .map_err(db_error)
}

fn encode(value: &impl Serialize) -> io::Result<String> {
    let body = serde_json::to_string(value).map_err(io::Error::other)?;
    if body.len() > 4 * 1024 * 1024 {
        return Err(invalid(
            "stopped transfer exceeds its request storage budget",
        ));
    }
    Ok(body)
}

fn decode(body: &str) -> io::Result<StoppedTransfer> {
    if body.len() > 4 * 1024 * 1024 {
        return Err(invalid(
            "stopped transfer exceeds its request storage budget",
        ));
    }
    serde_json::from_str(body).map_err(io::Error::other)
}

fn digest<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    Ok(Sha256::digest(encode(value)?.as_bytes()).to_vec())
}

/// The independently sealed application revision survives later process exits.
/// Generic request writes cannot advance this binding or mint part ownership.
pub(super) fn require_saved(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
    stop: &StoppedTransfer,
) -> io::Result<()> {
    if request.history != history
        || !stop.valid()
        || stop.interrupted.is_some()
        || stop.rejected_before_execution
        || request.phase != stop.phase()
    {
        return Err(invalid(
            "This saved result has no restartable transfer end.",
        ));
    }
    type Row = (u64, u8, Vec<u8>, Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>);
    let row: Option<Row> = connection.query_row(
        "SELECT s.saved_revision,s.saved_phase,s.saved_body_sha256,s.report_sha256,s.retry_sha256,s.remaining_sha256
         FROM request_transfer_stops s JOIN requests r ON r.id=s.request
         WHERE s.request=?1 AND s.body IS NULL AND r.revision=?2 AND r.phase=?3 AND r.body=?4",
        params![request.id, request.revision, request.phase as u8, request.body],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
    ).optional().map_err(db_error)?;
    let Some((revision, phase, body, report, retry, remaining)) = row else {
        return Err(invalid("The exact sealed stopped transfer is unavailable."));
    };
    let mut original_report = stop.report.clone();
    original_report.failed_resolution = TransferResolution::Available;
    original_report.remaining_resolution = TransferResolution::Available;
    let current_retry = stop.retry.as_ref().map(digest).transpose()?;
    let current_remaining = stop.remaining.as_ref().map(digest).transpose()?;
    if revision != request.revision
        || phase != request.phase as u8
        || body != Sha256::digest(&request.body).as_slice()
        || report != digest(&original_report)?
        || (current_retry.is_some() && current_retry != retry)
        || (current_remaining.is_some() && current_remaining != remaining)
    {
        return Err(invalid(
            "The saved transfer revision or exact parts changed.",
        ));
    }
    Ok(())
}

pub(super) fn release_saved(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
    before: &StoppedTransfer,
    after: &StoppedTransfer,
    revision: u64,
    body: &[u8],
) -> io::Result<()> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM request_transfer_stops WHERE request=?1)",
            [request.id],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if !exists {
        // Refusal before an engine reservation has only its live process owner.
        // Do not create a seal for it, including on a later restart.
        return Ok(());
    }
    require_saved(connection, history, request, before)?;
    let mut expected = before.clone();
    for part in [
        crate::operations::transfer::TransferPart::Failed,
        crate::operations::transfer::TransferPart::Remaining,
    ] {
        if before.has(part) && !after.has(part) {
            let resolution = match part {
                crate::operations::transfer::TransferPart::Failed => after.report.failed_resolution,
                crate::operations::transfer::TransferPart::Remaining => {
                    after.report.remaining_resolution
                }
            };
            if resolution == TransferResolution::Available {
                return Err(invalid("A released transfer part needs its resolution."));
            }
            expected.release(part, resolution)?;
        }
    }
    if !after.valid() || encode(&expected)? != encode(after)? || encode(before)? == encode(after)? {
        return Err(invalid("The exact transfer release changed other work."));
    }
    connection.execute("UPDATE request_transfer_stops SET saved_revision=?2,saved_phase=?3,saved_body_sha256=?4 WHERE request=?1",
        params![request.id, revision, after.phase() as u8, &Sha256::digest(body)[..]]).map_err(db_error)?;
    Ok(())
}

pub(super) fn require_handoff(
    before: &StoppedTransfer,
    after: &StoppedTransfer,
    successor: &[u8],
) -> io::Result<()> {
    use crate::operations::transfer::{TransferContinuation, TransferPart};
    // Decode only the opaque engine checkpoint from the application envelope.
    #[derive(Deserialize)]
    struct Envelope {
        #[serde(rename = "Plan")]
        plan: Plan,
    }
    #[derive(Deserialize)]
    struct Plan {
        transfer_checkpoint: TransferContinuation,
    }
    let mut selected = None;
    for part in [TransferPart::Failed, TransferPart::Remaining] {
        if before.has(part) && !after.has(part) {
            let (plan, resolution) = match part {
                TransferPart::Failed => (&before.retry, after.report.failed_resolution),
                TransferPart::Remaining => (&before.remaining, after.report.remaining_resolution),
            };
            if selected.is_some() || resolution != TransferResolution::HandedOff {
                return Err(invalid("A handoff must consume exactly one transfer part."));
            }
            selected = plan.as_ref();
        }
    }
    let selected = selected.ok_or_else(|| invalid("A handoff needs its exact selected part."))?;
    let successor: Envelope = serde_json::from_slice(successor).map_err(io::Error::other)?;
    if digest(selected)? != digest(&successor.plan.transfer_checkpoint)? {
        return Err(invalid(
            "The successor differs from its exact handed-off part.",
        ));
    }
    Ok(())
}

fn validate(receipt: &transfer_completion::Receipt, stop: &StoppedTransfer) -> io::Result<()> {
    if !stop.valid()
        || stop.interrupted.is_some()
        || stop.rejected_before_execution
        || matches!(stop.phase(), RequestPhase::Running | RequestPhase::Finished)
        || stop.report.failed_resolution != TransferResolution::Available
        || stop.report.remaining_resolution != TransferResolution::Available
        || receipt.result.is_some()
    {
        return Err(invalid(
            "The stopped transfer result is invalid or already completed.",
        ));
    }
    receipt.validate_scope(
        stop.report.kind,
        &stop.report.destination,
        stop.report.operation,
    )
}

pub(super) fn stopped(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
) -> io::Result<Option<StoppedTransfer>> {
    let Some(receipt) = transfer_completion::read(connection, history, request)? else {
        return Ok(None);
    };
    struct Recorded {
        body: String,
        sha: Vec<u8>,
        report: Vec<u8>,
        retry: Option<Vec<u8>>,
        remaining: Option<Vec<u8>>,
        phase: u8,
    }
    let row = connection.query_row(
        "SELECT body,body_sha256,report_sha256,retry_sha256,remaining_sha256,phase
        FROM request_transfer_stops WHERE request=?1 AND body IS NOT NULL AND saved_revision IS NULL",
        [request.id], |row| Ok(Recorded { body: row.get(0)?, sha: row.get(1)?, report: row.get(2)?, retry: row.get(3)?, remaining: row.get(4)?, phase: row.get(5)? }),
    ).optional().map_err(db_error)?;
    let Some(Recorded {
        body,
        sha,
        report,
        retry,
        remaining,
        phase,
    }) = row
    else {
        return Ok(None);
    };
    if sha != Sha256::digest(body.as_bytes()).as_slice() {
        return Err(invalid("The recorded stopped transfer changed."));
    }
    // Hash the original wire fragments. Re-encoding a decoded older checkpoint
    // uses the current codec and must not invalidate a valid historical end.
    #[derive(Deserialize)]
    struct Parts<'a> {
        #[serde(borrow)]
        report: &'a serde_json::value::RawValue,
        #[serde(borrow)]
        retry: Option<&'a serde_json::value::RawValue>,
        #[serde(borrow)]
        remaining: Option<&'a serde_json::value::RawValue>,
    }
    if body.len() > 4 * 1024 * 1024 {
        return Err(invalid(
            "stopped transfer exceeds its request storage budget",
        ));
    }
    let parts: Parts<'_> = serde_json::from_str(&body).map_err(io::Error::other)?;
    let raw_digest =
        |part: &serde_json::value::RawValue| Sha256::digest(part.get().as_bytes()).to_vec();
    if report != raw_digest(parts.report)
        || retry != parts.retry.map(raw_digest)
        || remaining != parts.remaining.map(raw_digest)
    {
        return Err(invalid("The exact stopped transfer parts changed."));
    }
    let stop: StoppedTransfer = decode(&body)?;
    validate(&receipt, &stop)?;
    if phase != stop.phase() as u8 {
        return Err(invalid("The stopped transfer phase changed."));
    }
    Ok(Some(stop))
}

pub(super) fn confirm(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
    stop: &StoppedTransfer,
    phase: RequestPhase,
    revision: u64,
    body: &[u8],
) -> io::Result<()> {
    let recorded = stopped(connection, history, request)?
        .ok_or_else(|| invalid("The exact recorded stopped transfer is unavailable."))?;
    if phase != stop.phase() || encode(&recorded)? != encode(stop)? {
        return Err(invalid(
            "The application result differs from the recorded stopped transfer.",
        ));
    }
    // Retire the duplicate full plans in the same transaction as the saved
    // application result. Retain exact part hashes and the accepted execution.
    // After verifying the old bytes, bind current part hashes to the exact
    // saved codec in the same transaction. The original whole-end hash stays
    // immutable; no traversal facts or execution ownership are invented.
    connection.execute("UPDATE request_transfer_stops SET body=NULL,saved_revision=?2,saved_phase=?3,saved_body_sha256=?4,report_sha256=?5,retry_sha256=?6,remaining_sha256=?7 WHERE request=?1",
        params![request.id,revision,phase as u8,&Sha256::digest(body)[..],digest(&stop.report)?,stop.retry.as_ref().map(digest).transpose()?,stop.remaining.as_ref().map(digest).transpose()?])
        .map_err(db_error)?;
    Ok(())
}

pub(super) fn confirm_witnessed(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
    stop: &StoppedTransfer,
    revision: u64,
    body: &[u8],
) -> io::Result<()> {
    if stopped(connection, history, request)?.is_some() {
        return confirm(
            connection,
            history,
            request,
            stop,
            stop.phase(),
            revision,
            body,
        );
    }
    let receipt = transfer_completion::read(connection, history, request)?
        .ok_or_else(|| invalid("The exact transfer stop reservation is unavailable."))?;
    transfer_cursor::retire(connection, request.id)?;
    validate(&receipt, stop)?;
    // The engine-scoped live witness permits one atomic end/result write if
    // retaining two full reports would exceed the existing request budget.
    connection.execute("INSERT INTO request_transfer_stops(request,body_sha256,report_sha256,retry_sha256,remaining_sha256,phase,saved_revision,saved_phase,saved_body_sha256) VALUES(?1,?2,?3,?4,?5,?6,?7,?6,?8)",
        params![request.id,digest(stop)?,digest(&stop.report)?,stop.retry.as_ref().map(digest).transpose()?,stop.remaining.as_ref().map(digest).transpose()?,stop.phase() as u8,revision,&Sha256::digest(body)[..]])
        .map_err(db_error)?;
    Ok(())
}

impl Store {
    pub(in crate::operations) fn require_saved_transfer(
        &self,
        request: &RequestRecord,
        stop: &StoppedTransfer,
    ) -> io::Result<()> {
        require_saved(&self.connection, self.identity, request, stop)?;
        transfer_completion::require_saved_scope(&self.connection, self.identity, request, stop)?;
        if self.pending_in(IntentLane::Primary)?.is_some()
            || self.pending_in(IntentLane::RecoveryCopy)?.is_some()
        {
            return Err(invalid(
                "Reconcile interrupted filesystem work before restoring transfer owners.",
            ));
        }
        Ok(())
    }
    pub(in crate::operations) fn stopped_transfer(
        &self,
        request: &RequestRecord,
    ) -> io::Result<Option<StoppedTransfer>> {
        stopped(&self.connection, self.identity, request)
    }

    pub(in crate::operations) fn record_transfer_stop(
        &mut self,
        request: &RequestRecord,
        stop: &StoppedTransfer,
        sha: &[u8; 32],
    ) -> io::Result<()> {
        self.ensure_location()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        record(&transaction, self.identity, request, stop, sha)?;
        requests::require_budget(&transaction)?;
        transaction.commit().map_err(db_error)
    }
}

pub(super) fn record(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
    stop: &StoppedTransfer,
    sha: &[u8; 32],
) -> io::Result<()> {
    let body = validate_record(connection, history, request, stop, sha)?;
    transfer_cursor::retire(connection, request.id)?;
    if stopped(connection, history, request)?.is_none() {
        connection.execute("INSERT INTO request_transfer_stops(request,body,body_sha256,report_sha256,retry_sha256,remaining_sha256,phase) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![request.id,body,&sha[..],digest(&stop.report)?,stop.retry.as_ref().map(digest).transpose()?,stop.remaining.as_ref().map(digest).transpose()?,stop.phase() as u8])
                .map_err(db_error)?;
    }
    Ok(())
}

pub(super) fn validate_record(
    connection: &Connection,
    history: Identity,
    request: &RequestRecord,
    stop: &StoppedTransfer,
    sha: &[u8; 32],
) -> io::Result<String> {
    let receipt = transfer_completion::read(connection, history, request)?
        .ok_or_else(|| invalid("The exact transfer stop reservation is unavailable."))?;
    validate(&receipt, stop)?;
    let body = encode(stop)?;
    if sha.as_slice() != Sha256::digest(body.as_bytes()).as_slice() {
        return Err(invalid("The live stopped transfer changed."));
    }
    if let Some(previous) = stopped(connection, history, request)?
        && encode(&previous)? != body
    {
        return Err(invalid("The original recorded stopped transfer changed."));
    }
    Ok(body)
}
