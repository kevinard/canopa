//! Request transactions use their own short-lived writer. Waiting for an Undo or
//! transfer question must not block durable acceptance by the desktop scheduler.
use super::*;
use crate::operations::requests::{
    RequestPhase, RequestRecord, RetainedSourceLease, RetainedSources,
};
use rusqlite::{OpenFlags, TransactionBehavior};
use sha2::{Digest, Sha256};

const PAYLOAD_LIMIT: usize = 4 * 1024 * 1024;
const TOTAL_LIMIT: usize = 64 * 1024 * 1024;
const UNFINISHED_LIMIT: usize = 1024;

enum CompletionGuard<'a> {
    None,
    Reviewed,
    ConfirmedMutation,
    ConfirmedTransfer,
    StoppedTransfer(&'a crate::operations::transfer::StoppedTransfer),
    WitnessedTransferStop(&'a crate::operations::transfer::StoppedTransfer),
    ReleasedTransfer(
        &'a crate::operations::transfer::StoppedTransfer,
        &'a crate::operations::transfer::StoppedTransfer,
    ),
}

enum Retention<'a> {
    None,
    Captured(&'a RetainedSources),
    Lease {
        history: Identity,
        id: u64,
        consume: bool,
    },
}

pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS requests (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        revision INTEGER NOT NULL CHECK(revision>0),
        phase INTEGER NOT NULL CHECK(phase BETWEEN 0 AND 5),
        body BLOB NOT NULL CHECK(length(body)<=4194304));
        CREATE INDEX IF NOT EXISTS requests_phase ON requests(phase,id);
        CREATE TABLE IF NOT EXISTS request_times (
        request INTEGER PRIMARY KEY REFERENCES requests(id) ON DELETE CASCADE,
        accepted_at INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS request_sources (
        request INTEGER NOT NULL REFERENCES requests(id) ON DELETE CASCADE,
        stage TEXT NOT NULL, PRIMARY KEY(request,stage));
        CREATE INDEX IF NOT EXISTS request_sources_stage ON request_sources(stage);
        CREATE TABLE IF NOT EXISTS source_leases (
        request INTEGER PRIMARY KEY REFERENCES requests(id) ON DELETE CASCADE);
        DELETE FROM requests WHERE id IN (SELECT request FROM source_leases);",
        )
        .map_err(db_error)
}

fn record_admission(connection: &Connection, id: u64) -> io::Result<i64> {
    let at = chrono::Utc::now().timestamp();
    connection
        .execute(
            "INSERT INTO request_times(request,accepted_at) VALUES(?1,?2)",
            params![id, at],
        )
        .map_err(db_error)?;
    Ok(at)
}

pub(super) fn accepted_at(connection: &Connection, id: u64) -> io::Result<Option<i64>> {
    connection
        .query_row_cached(
            "SELECT accepted_at FROM request_times WHERE request=?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)
}

impl ReadSource {
    pub(in crate::operations) fn release_transfer<T: Serialize>(
        &self,
        previous: &RequestRecord,
        before: &crate::operations::transfer::StoppedTransfer,
        after: &crate::operations::transfer::StoppedTransfer,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        use crate::operations::transfer::{TransferPart, TransferResolution};
        for part in [TransferPart::Failed, TransferPart::Remaining] {
            let resolution = match part {
                TransferPart::Failed => after.report.failed_resolution,
                TransferPart::Remaining => after.report.remaining_resolution,
            };
            if before.has(part) && !after.has(part) && resolution != TransferResolution::Abandoned {
                return Err(invalid(
                    "A transfer abandonment needs its exact abandoned parts.",
                ));
            }
        }
        self.checkpoint_request_guarded(
            previous,
            after.phase(),
            payload,
            CompletionGuard::ReleasedTransfer(before, after),
        )
    }
    pub(super) fn request_writer<T>(
        &self,
        write: impl FnOnce(&rusqlite::Transaction<'_>) -> io::Result<T>,
    ) -> io::Result<T> {
        self.ensure_location()?;
        let mut connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(db_error)?;
        self.ensure_location()?;
        connection
            .busy_timeout(std::time::Duration::from_secs(2))
            .map_err(db_error)?;
        connection
            .execute_batch(
                "PRAGMA trusted_schema=OFF; PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;",
            )
            .map_err(db_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let version: u32 = transaction
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(db_error)?;
        if version != VERSION {
            return Err(invalid("unsupported operation database version"));
        }
        self.ensure_location()?;
        let result = write(&transaction)?;
        self.ensure_location()?;
        transaction.commit().map_err(db_error)?;
        Ok(result)
    }

    pub(in crate::operations) fn queue_request<T: Serialize>(
        &self,
        payload: &T,
        sources: Option<&RetainedSources>,
    ) -> io::Result<RequestRecord> {
        self.accept_request(
            payload,
            sources.map_or(Retention::None, Retention::Captured),
            false,
        )
    }

    pub(in crate::operations) fn queue_request_with_source_lease<T: Serialize>(
        &self,
        payload: &T,
        lease: &RetainedSourceLease,
    ) -> io::Result<RequestRecord> {
        let (history, id) = lease.reservation()?;
        self.accept_request(
            payload,
            Retention::Lease {
                history,
                id,
                consume: false,
            },
            false,
        )
    }

    pub(in crate::operations) fn queue_request_consuming_source_lease<T: Serialize>(
        &self,
        payload: &T,
        lease: &RetainedSourceLease,
    ) -> io::Result<RequestRecord> {
        let (history, id) = lease.reservation()?;
        self.accept_request(
            payload,
            Retention::Lease {
                history,
                id,
                consume: true,
            },
            false,
        )
    }

    pub(in crate::operations) fn lease_retained_sources(
        &self,
        sources: &RetainedSources,
    ) -> io::Result<RequestRecord> {
        self.accept_request(&(), Retention::Captured(sources), true)
    }

    fn accept_request<T: Serialize>(
        &self,
        payload: &T,
        sources: Retention<'_>,
        source_lease: bool,
    ) -> io::Result<RequestRecord> {
        let history = match &sources {
            Retention::None => None,
            Retention::Captured(sources) => Some(sources.history),
            Retention::Lease { history, .. } => Some(*history),
        };
        if history.is_some_and(|history| history != self.identity) {
            return Err(invalid(
                "the retained sources belong to another operation history",
            ));
        }
        let body = payload_bytes(payload)?;
        self.request_writer(|transaction| {
            transaction
                .execute(
                    "INSERT INTO requests(revision,phase,body) VALUES(1,0,?1)",
                    [&body],
                )
                .map_err(db_error)?;
            let id = u64::try_from(transaction.last_insert_rowid()).map_err(io::Error::other)?;
            match sources {
                Retention::None => {}
                Retention::Captured(sources) => {
                    source_retention::acquire(transaction, id, sources)?
                }
                Retention::Lease {
                    id: lease, consume, ..
                } => {
                    source_retention::acquire_lease(transaction, id, lease, self.identity)?;
                    if consume {
                        // Transfer protection inside this admission transaction.
                        // Leave the empty lease row for its normal asynchronous
                        // release; rollback preserves the original reservation.
                        transaction
                            .execute("DELETE FROM request_sources WHERE request=?1", [lease])
                            .map_err(db_error)?;
                    }
                }
            }
            if source_lease {
                transaction
                    .execute("INSERT INTO source_leases(request) VALUES(?1)", [id])
                    .map_err(db_error)?;
            }
            require_budget(transaction)?;
            let accepted_at = (!source_lease)
                .then(|| record_admission(transaction, id))
                .transpose()?;
            Ok(RequestRecord {
                history: self.identity,
                id,
                revision: 1,
                phase: RequestPhase::Queued,
                accepted_at,
                body,
            })
        })
    }

    pub(in crate::operations) fn release_source_lease(&self, id: u64) -> io::Result<()> {
        self.request_writer(|transaction| {
            let changed = transaction.execute(
                "DELETE FROM requests WHERE id=?1 AND EXISTS(SELECT 1 FROM source_leases WHERE request=?1)",
                [id],
            ).map_err(db_error)?;
            require_current(changed)
        })
    }

    pub(in crate::operations) fn checkpoint_request<T: Serialize>(
        &self,
        previous: &RequestRecord,
        phase: RequestPhase,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.checkpoint_request_guarded(previous, phase, payload, CompletionGuard::None)
    }

    pub(in crate::operations) fn finish_reviewed_request<T: Serialize>(
        &self,
        previous: &RequestRecord,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.checkpoint_request_guarded(
            previous,
            RequestPhase::Finished,
            payload,
            CompletionGuard::Reviewed,
        )
    }

    pub(in crate::operations) fn finish_confirmed_mutation<T: Serialize>(
        &self,
        previous: &RequestRecord,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.checkpoint_request_guarded(
            previous,
            RequestPhase::Finished,
            payload,
            CompletionGuard::ConfirmedMutation,
        )
    }

    pub(in crate::operations) fn finish_confirmed_transfer<T: Serialize>(
        &self,
        previous: &RequestRecord,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.checkpoint_request_guarded(
            previous,
            RequestPhase::Finished,
            payload,
            CompletionGuard::ConfirmedTransfer,
        )
    }

    pub(in crate::operations) fn save_stopped_transfer<T: Serialize>(
        &self,
        previous: &RequestRecord,
        stop: &crate::operations::transfer::StoppedTransfer,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.checkpoint_request_guarded(
            previous,
            stop.phase(),
            payload,
            CompletionGuard::StoppedTransfer(stop),
        )
    }

    pub(in crate::operations) fn save_witnessed_transfer_stop<T: Serialize>(
        &self,
        previous: &RequestRecord,
        stop: &crate::operations::transfer::StoppedTransfer,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.checkpoint_request_guarded(
            previous,
            stop.phase(),
            payload,
            CompletionGuard::WitnessedTransferStop(stop),
        )
    }

    fn checkpoint_request_guarded<T: Serialize>(
        &self,
        previous: &RequestRecord,
        phase: RequestPhase,
        payload: &T,
        guard: CompletionGuard<'_>,
    ) -> io::Result<RequestRecord> {
        if previous.history != self.identity {
            return Err(invalid("the request belongs to another operation history"));
        }
        if previous.phase == RequestPhase::Finished && phase != RequestPhase::Finished {
            return Err(invalid("a finished request cannot become executable again"));
        }
        let body = payload_bytes(payload)?;
        let revision = next_revision(previous)?;
        self.request_writer(|transaction| {
            if matches!(guard, CompletionGuard::None) && previous.phase == RequestPhase::Running {
                let active: Option<u64> = transaction.query_row("SELECT e.operation FROM request_executions e JOIN request_transfer_cursors c ON c.request=e.request WHERE e.request=?1 AND e.revision=?2 AND e.payload_sha256=?3",params![previous.id,previous.revision,&Sha256::digest(&previous.body)[..]],|r|r.get(0)).optional().map_err(db_error)?;
                if let Some(operation) = active {
                    for table in ["pending","recovery_pending"] {
                        let body: Option<String> = transaction.query_row(&format!("SELECT body FROM {table} WHERE singleton=1"),[],|r|r.get(0)).optional().map_err(db_error)?;
                        if body.as_ref().map(|body| decode::<Pending>(body)).transpose()?.is_some_and(|pending| pending.id() == operation) {
                            return Err(io::Error::new(io::ErrorKind::WouldBlock,"Reconcile the unfinished filesystem work before replacing its accepted transfer record. Its original task remains protected."));
                        }
                    }
                }
            }
            if matches!(guard, CompletionGuard::ConfirmedMutation)
                && mutation::completed(transaction, self.identity, previous)?.is_none() {
                return Err(invalid("The exact recorded mutation completion is unavailable."));
            }
            if matches!(guard, CompletionGuard::ConfirmedTransfer)
                && transfer_completion::completed(transaction, self.identity, previous)?.is_none() {
                return Err(invalid("The exact recorded transfer completion is unavailable."));
            }
            if let CompletionGuard::StoppedTransfer(stop) = guard {
                transfer_stop::confirm(transaction, self.identity, previous, stop, phase, revision, &body)?;
            }
            if let CompletionGuard::WitnessedTransferStop(stop) = guard {
                transfer_stop::confirm_witnessed(transaction, self.identity, previous, stop, revision, &body)?;
            }
            if let CompletionGuard::ReleasedTransfer(before, after) = guard {
                transfer_stop::release_saved(transaction, self.identity, previous, before, after, revision, &body)?;
            }
            if matches!(guard, CompletionGuard::Reviewed) {
                let pending: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM pending UNION ALL SELECT 1 FROM recovery_pending)",
                    [],
                    |row| row.get(0),
                ).map_err(db_error)?;
                if pending {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "Review interrupted work in Recovery before dismissing this request. Its retained files remain protected.",
                    ));
                }
                transfer_cursor::retire(transaction, previous.id)?;
            }
            let changed = transaction
                .execute(
                    "UPDATE requests SET revision=?3,phase=?4,body=?5 WHERE id=?1 AND revision=?2",
                    params![previous.id, previous.revision, revision, phase as u8, body],
                )
                .map_err(db_error)?;
            require_current(changed)?;
            transaction.execute("DELETE FROM request_transfer_interruptions WHERE request=?1", [previous.id]).map_err(db_error)?;
            if matches!(guard, CompletionGuard::ConfirmedTransfer) {
                // The finished request now owns the full typed report. Retire
                // its duplicate receipt in this same transaction so large
                // results retain the existing per-request payload allowance.
                transaction.execute("DELETE FROM request_transfer_ends WHERE request=?1", [previous.id])
                    .map_err(db_error)?;
            }
            if phase == RequestPhase::Finished {
                transaction
                    .execute(
                        "DELETE FROM request_sources WHERE request=?1",
                        [previous.id],
                    )
                    .map_err(db_error)?;
            }
            prune_finished(transaction)?;
            require_budget(transaction)?;
            Ok(RequestRecord {
                history: self.identity,
                id: previous.id,
                revision,
                phase,
                accepted_at: previous.accepted_at,
                body,
            })
        })
    }

    pub(in crate::operations) fn handoff_request<T: Serialize, U: Serialize>(
        &self,
        previous: &RequestRecord,
        remaining_phase: RequestPhase,
        remaining_payload: &T,
        successor_payload: &U,
    ) -> io::Result<(RequestRecord, RequestRecord)> {
        self.handoff_request_guarded(
            previous,
            remaining_phase,
            remaining_payload,
            successor_payload,
            None,
        )
    }

    pub(in crate::operations) fn handoff_transfer<T: Serialize, U: Serialize>(
        &self,
        previous: &RequestRecord,
        before: &crate::operations::transfer::StoppedTransfer,
        after: &crate::operations::transfer::StoppedTransfer,
        remaining_payload: &T,
        successor_payload: &U,
    ) -> io::Result<(RequestRecord, RequestRecord)> {
        self.handoff_request_guarded(
            previous,
            after.phase(),
            remaining_payload,
            successor_payload,
            Some((before, after)),
        )
    }

    fn handoff_request_guarded<T: Serialize, U: Serialize>(
        &self,
        previous: &RequestRecord,
        remaining_phase: RequestPhase,
        remaining_payload: &T,
        successor_payload: &U,
        transfer: Option<(
            &crate::operations::transfer::StoppedTransfer,
            &crate::operations::transfer::StoppedTransfer,
        )>,
    ) -> io::Result<(RequestRecord, RequestRecord)> {
        if previous.history != self.identity {
            return Err(invalid("the request belongs to another operation history"));
        }
        if matches!(
            previous.phase,
            RequestPhase::Running | RequestPhase::Finished
        ) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "only a stopped unfinished request can hand off its plan",
            ));
        }
        if matches!(
            remaining_phase,
            RequestPhase::Queued | RequestPhase::Running
        ) {
            return Err(invalid(
                "the original request must remain stopped after handing off its plan",
            ));
        }
        let remaining = payload_bytes(remaining_payload)?;
        let successor = payload_bytes(successor_payload)?;
        let revision = next_revision(previous)?;
        self.request_writer(|transaction| {
            if let Some((before, after)) = transfer {
                transfer_stop::require_handoff(before, after, &successor)?;
                transfer_stop::release_saved(transaction, self.identity, previous, before, after, revision, &remaining)?;
            }
            let changed = transaction
                .execute(
                    "UPDATE requests SET revision=?3,phase=?4,body=?5 WHERE id=?1 AND revision=?2",
                    params![
                        previous.id,
                        previous.revision,
                        revision,
                        remaining_phase as u8,
                        remaining
                    ],
                )
                .map_err(db_error)?;
            require_current(changed)?;
            transaction
                .execute(
                    "INSERT INTO requests(revision,phase,body) VALUES(1,0,?1)",
                    [&successor],
                )
                .map_err(db_error)?;
            let id = u64::try_from(transaction.last_insert_rowid()).map_err(io::Error::other)?;
            transaction.execute("INSERT INTO request_sources(request,stage) SELECT ?2,stage FROM request_sources WHERE request=?1", params![previous.id,id]).map_err(db_error)?;
            if remaining_phase == RequestPhase::Finished {
                transaction.execute("DELETE FROM request_sources WHERE request=?1", [previous.id]).map_err(db_error)?;
            }
            prune_finished(transaction)?;
            require_budget(transaction)?;
            let accepted_at = record_admission(transaction, id)?;
            Ok((
                RequestRecord {
                    history: self.identity,
                    id: previous.id,
                    revision,
                    phase: remaining_phase,
                    accepted_at: previous.accepted_at,
                    body: remaining,
                },
                RequestRecord {
                    history: self.identity,
                    id,
                    revision: 1,
                    phase: RequestPhase::Queued,
                    accepted_at: Some(accepted_at),
                    body: successor,
                },
            ))
        })
    }

    pub(in crate::operations) fn dismiss_request(
        &self,
        previous: &RequestRecord,
    ) -> io::Result<()> {
        if previous.history != self.identity {
            return Err(invalid("the request belongs to another operation history"));
        }
        if previous.phase == RequestPhase::Running {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "stop the running request before dismissing it",
            ));
        }
        self.request_writer(|transaction| {
            let changed = transaction
                .execute(
                    "DELETE FROM requests WHERE id=?1 AND revision=?2",
                    params![previous.id, previous.revision],
                )
                .map_err(db_error)?;
            require_current(changed)
        })
    }
}

impl Store {
    pub(in crate::operations) fn request_page(
        &self,
        before: Option<u64>,
    ) -> io::Result<Vec<RequestRecord>> {
        let mut statement = self.connection.prepare("SELECT r.id,r.revision,r.phase,length(r.body),r.body,t.accepted_at FROM requests r LEFT JOIN request_times t ON t.request=r.id WHERE (?1 IS NULL OR r.id<?1) AND NOT EXISTS(SELECT 1 FROM source_leases WHERE request=r.id) ORDER BY r.id DESC LIMIT 100").map_err(db_error)?;
        let mut rows = statement.query([before]).map_err(db_error)?;
        let mut records = Vec::new();
        while let Some(row) = rows.next().map_err(db_error)? {
            let length: usize = row.get(3).map_err(db_error)?;
            if length > PAYLOAD_LIMIT {
                return Err(invalid("saved request exceeds its storage budget"));
            }
            let phase = match row.get::<_, u8>(2).map_err(db_error)? {
                0 => RequestPhase::Queued,
                1 => RequestPhase::Running,
                2 => RequestPhase::Failed,
                3 => RequestPhase::Remaining,
                4 => RequestPhase::FailedWithRemaining,
                5 => RequestPhase::Finished,
                _ => return Err(invalid("invalid saved request phase")),
            };
            let record = RequestRecord {
                history: self.identity,
                id: row.get(0).map_err(db_error)?,
                revision: row.get(1).map_err(db_error)?,
                phase,
                body: row.get(4).map_err(db_error)?,
                accepted_at: row.get(5).map_err(db_error)?,
            };
            if record.id == 0 || record.revision == 0 {
                return Err(invalid("invalid saved request identity"));
            }
            // Keep incompatible caller payloads byte-for-byte. The caller can
            // report a decode failure without silently replacing the record.
            records.push(record);
        }
        Ok(records)
    }
}

fn payload_bytes(payload: &impl Serialize) -> io::Result<Vec<u8>> {
    let body = serde_json::to_vec(payload).map_err(io::Error::other)?;
    if body.len() > PAYLOAD_LIMIT {
        return Err(invalid("request exceeds its storage budget"));
    }
    Ok(body)
}

fn next_revision(previous: &RequestRecord) -> io::Result<u64> {
    previous
        .revision
        .checked_add(1)
        .filter(|revision| *revision <= i64::MAX as u64)
        .ok_or_else(|| invalid("request revision exhausted"))
}

fn prune_finished(connection: &Connection) -> io::Result<()> {
    connection.execute("DELETE FROM requests WHERE phase=5 AND id IN (SELECT id FROM requests WHERE phase=5 ORDER BY id DESC LIMIT -1 OFFSET 100)", []).map_err(db_error)?;
    Ok(())
}

fn require_current(changed: usize) -> io::Result<()> {
    if changed == 1 {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the saved request changed or was dismissed; reload Operations before continuing",
        ))
    }
}

pub(super) fn require_budget(connection: &Connection) -> io::Result<()> {
    let (unfinished, bytes): (usize, usize) = connection
        .query_row(
            "SELECT coalesce(sum(phase<>5),0),coalesce(sum(length(body)),0)+(SELECT count(*)*64 FROM request_sources)+(SELECT coalesce(sum(length(CAST(label AS BLOB))+128),0) FROM request_executions)+(SELECT coalesce(sum(length(CAST(intent AS BLOB))+96),0) FROM request_mutation_ends)+(SELECT coalesce(sum(length(CAST(intent AS BLOB))+coalesce(length(CAST(result AS BLOB)),0)+128),0) FROM request_transfer_ends)+(SELECT coalesce(sum(coalesce(length(CAST(body AS BLOB)),0)+256),0) FROM request_transfer_stops)+(SELECT coalesce(sum(length(CAST(body AS BLOB))+128),0) FROM request_transfer_interruptions) FROM requests",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(db_error)?;
    let excessive: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM requests WHERE length(body)+(SELECT count(*)*64 FROM request_sources WHERE request=requests.id)+(SELECT coalesce(sum(length(CAST(label AS BLOB))+128),0) FROM request_executions WHERE request=requests.id)+(SELECT coalesce(sum(length(CAST(intent AS BLOB))+96),0) FROM request_mutation_ends WHERE request=requests.id)+(SELECT coalesce(sum(length(CAST(intent AS BLOB))+coalesce(length(CAST(result AS BLOB)),0)+128),0) FROM request_transfer_ends WHERE request=requests.id)+(SELECT coalesce(sum(coalesce(length(CAST(body AS BLOB)),0)+256),0) FROM request_transfer_stops WHERE request=requests.id)+(SELECT coalesce(sum(length(CAST(body AS BLOB))+128),0) FROM request_transfer_interruptions WHERE request=requests.id)>?1)", [PAYLOAD_LIMIT], |row| row.get(0)).map_err(db_error)?;
    if unfinished > UNFINISHED_LIMIT || bytes > TOTAL_LIMIT || excessive {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "saved operations are full; review or dismiss old requests before starting more work",
        ));
    }
    Ok(())
}
