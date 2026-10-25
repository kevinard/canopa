//! Publish the exact scheduling revision and filesystem operation together,
//! before its durable journal and forward filesystem mutation.
use super::*;
use crate::operations::{
    RequestEffects,
    requests::{RequestPhase, RequestRecord},
};
use sha2::{Digest, Sha256};

pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS request_executions (
        request INTEGER PRIMARY KEY REFERENCES requests(id) ON DELETE CASCADE,
        revision INTEGER NOT NULL CHECK(revision>0),
        operation INTEGER NOT NULL UNIQUE CHECK(operation>0),
        label TEXT NOT NULL CHECK(length(CAST(label AS BLOB))<=8192),
        payload_sha256 BLOB NOT NULL CHECK(length(payload_sha256)=32));",
        )
        .map_err(db_error)
}

enum Proof<'a> {
    Mutation(&'a Change),
    Transfer(
        super::super::transfer::TransferKind,
        &'a Path,
        Option<&'a super::super::transfer::RestartObservation>,
    ),
}

impl Store {
    pub(in crate::operations) fn begin_request(
        &mut self,
        label: &str,
        request: Option<&RequestRecord>,
    ) -> io::Result<u64> {
        self.begin_with_proof(label, request, None)
    }

    pub(in crate::operations) fn begin_transfer(
        &mut self,
        label: &str,
        request: Option<&RequestRecord>,
        kind: super::super::transfer::TransferKind,
        destination: &Path,
        original: Option<&super::super::transfer::RestartObservation>,
    ) -> io::Result<u64> {
        self.begin_with_proof(
            label,
            request,
            Some(Proof::Transfer(kind, destination, original)),
        )
    }

    pub(in crate::operations) fn begin_mutation(
        &mut self,
        label: &str,
        request: Option<&RequestRecord>,
        change: &Change,
    ) -> io::Result<u64> {
        self.begin_with_proof(label, request, Some(Proof::Mutation(change)))
    }

    fn begin_with_proof(
        &mut self,
        label: &str,
        request: Option<&RequestRecord>,
        proof: Option<Proof<'_>>,
    ) -> io::Result<u64> {
        self.ensure_location()?;
        if let Some(request) = request
            && (request.history != self.identity || request.phase != RequestPhase::Running)
        {
            return Err(invalid(
                "Filesystem execution needs this history's Running request",
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(request) = request {
            let current: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM requests WHERE id=?1 AND revision=?2 AND phase=1 AND body=?3)", params![request.id, request.revision, request.body], |row| row.get(0)).map_err(db_error)?;
            let used: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM request_executions WHERE request=?1)",
                    [request.id],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            if !current || used {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "This exact request revision is stale or already has a filesystem execution. Review its recorded work before starting new work.",
                ));
            }
        }
        transaction
            .execute("INSERT INTO operations(label) VALUES(?1)", [label])
            .map_err(db_error)?;
        let operation = u64::try_from(transaction.last_insert_rowid()).map_err(io::Error::other)?;
        if let Some(request) = request {
            let digest = Sha256::digest(&request.body);
            transaction
                .execute(
                    "INSERT INTO request_executions VALUES(?1,?2,?3,?4,?5)",
                    params![request.id, request.revision, operation, label, &digest[..]],
                )
                .map_err(db_error)?;
            match proof {
                Some(Proof::Mutation(change)) => mutation::reserve(&transaction, request, change)?,
                Some(Proof::Transfer(kind, destination, original)) => transfer_completion::reserve(
                    &transaction,
                    request,
                    kind,
                    destination,
                    original,
                )?,
                None => {}
            }
            requests::require_budget(&transaction)?;
        }
        transaction.commit().map_err(db_error)?;
        Ok(operation)
    }

    pub(in crate::operations) fn request_effects(
        &self,
        request: &RequestRecord,
    ) -> io::Result<Vec<RequestEffects>> {
        if request.history != self.identity {
            return Err(invalid("The request belongs to another operation history"));
        }
        let mut query = self.connection.prepare("SELECT revision,operation,label,payload_sha256 FROM request_executions WHERE request=?1 AND revision<=?2 ORDER BY revision").map_err(db_error)?;
        let mut rows = query
            .query(params![request.id, request.revision])
            .map_err(db_error)?;
        let mut effects = Vec::new();
        while let Some(row) = rows.next().map_err(db_error)? {
            let operation = row.get(1).map_err(db_error)?;
            let mut paths = BTreeSet::new();
            let confirmed_steps: usize = self
                .connection
                .query_row(
                    "SELECT count(*) FROM changes WHERE operation=?1",
                    [operation],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            let mut inspected = 0;
            let mut before = None;
            while inspected < 128
                && let Some((ordinal, change)) = self.change_before(operation, before)?
            {
                before = Some(ordinal);
                paths.insert(change.forward_path().to_owned());
                inspected += 1;
            }
            let journal_pending = [IntentLane::Primary, IntentLane::RecoveryCopy]
                .into_iter()
                .map(|lane| self.pending_in(lane))
                .collect::<io::Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .any(|pending| pending.id() == operation);
            let history_available = self
                .connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM operations WHERE id=?1)",
                    [operation],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            let digest: Vec<u8> = row.get(3).map_err(db_error)?;
            effects.push(RequestEffects {
                revision: row.get(0).map_err(db_error)?,
                operation,
                label: row.get(2).map_err(db_error)?,
                payload_sha256: digest
                    .try_into()
                    .map_err(|_| invalid("Invalid request execution digest"))?,
                confirmed_steps,
                paths: paths.into_iter().collect(),
                paths_truncated: inspected < confirmed_steps,
                history_available,
                journal_pending,
            });
        }
        Ok(effects)
    }
}
