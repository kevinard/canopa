//! Descriptive interrupted results never replace the accepted execution record.
use super::*;
use crate::operations::{requests::RequestRecord, transfer::StoppedTransfer};
use sha2::{Digest, Sha256};

pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS request_transfer_interruptions (
        request INTEGER PRIMARY KEY REFERENCES request_executions(request) ON DELETE CASCADE,
        revision INTEGER NOT NULL CHECK(revision>0),
        accepted_sha256 BLOB NOT NULL CHECK(length(accepted_sha256)=32),
        body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=4194304),
        body_sha256 BLOB NOT NULL CHECK(length(body_sha256)=32));",
        )
        .map_err(db_error)
}

fn validate(receipt: &transfer_completion::Receipt, stop: &StoppedTransfer) -> io::Result<()> {
    if !stop.valid() || stop.interrupted.is_none() || stop.rejected_before_execution {
        return Err(invalid("The interrupted transfer report is invalid."));
    }
    receipt.validate_scope(
        stop.report.kind,
        &stop.report.destination,
        stop.report.operation,
    )
}

impl ReadSource {
    pub(in crate::operations) fn save_interrupted_transfer(
        &self,
        request: &RequestRecord,
        stop: &StoppedTransfer,
    ) -> io::Result<Option<RequestRecord>> {
        self.request_writer(|transaction| {
            let Some(receipt) = transfer_completion::read(transaction, self.identity, request)?
            else {
                let used: bool = transaction
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM request_executions WHERE request=?1)",
                        [request.id],
                        |row| row.get(0),
                    )
                    .map_err(db_error)?;
                return if used {
                    Err(invalid(
                        "The interrupted transfer lost its accepted execution record.",
                    ))
                } else {
                    Ok(None)
                };
            };
            validate(&receipt, stop)?;
            let body = encode(stop)?;
            transaction
                .execute(
                    "INSERT INTO request_transfer_interruptions VALUES(?1,?2,?3,?4,?5)
                ON CONFLICT(request) DO UPDATE SET revision=excluded.revision,
                accepted_sha256=excluded.accepted_sha256,body=excluded.body,
                body_sha256=excluded.body_sha256",
                    params![
                        request.id,
                        request.revision,
                        &Sha256::digest(&request.body)[..],
                        body,
                        &Sha256::digest(body.as_bytes())[..]
                    ],
                )
                .map_err(db_error)?;
            requests::require_budget(transaction)?;
            Ok(Some(request.clone()))
        })
    }
}

impl Store {
    pub(in crate::operations) fn interrupted_transfer(
        &self,
        request: &RequestRecord,
    ) -> io::Result<Option<StoppedTransfer>> {
        type Row = (u64, Vec<u8>, String, Vec<u8>);
        let row: Option<Row> = self
            .connection
            .query_row(
                "SELECT revision,accepted_sha256,body,body_sha256
            FROM request_transfer_interruptions WHERE request=?1",
                [request.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(db_error)?;
        let Some((revision, accepted, body, digest)) = row else {
            return Ok(None);
        };
        if revision != request.revision
            || accepted != Sha256::digest(&request.body).as_slice()
            || digest != Sha256::digest(body.as_bytes()).as_slice()
        {
            return Err(invalid("The interrupted transfer report changed."));
        }
        let receipt = transfer_completion::read(&self.connection, self.identity, request)?
            .ok_or_else(|| invalid("The interrupted report lost its accepted execution record."))?;
        let stop = decode(&body)?;
        validate(&receipt, &stop)?;
        Ok(Some(stop))
    }
}
