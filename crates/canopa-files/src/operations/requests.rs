//! Durable scheduling records. Payloads belong to the caller; filesystem journals
//! and saved-data ownership remain the operation engine's responsibility.
pub use super::source_leases::RetainedSourceLease;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
pub enum RequestPhase {
    Queued = 0,
    Running = 1,
    Failed = 2,
    Remaining = 3,
    FailedWithRemaining = 4,
    Finished = 5,
}

#[derive(Clone)]
pub struct RequestRecord {
    pub(super) history: Identity,
    pub(super) id: u64,
    pub(super) revision: u64,
    pub(super) phase: RequestPhase,
    pub(super) body: Vec<u8>,
}

/// Core-owned retained-data dependencies captured by a worker. This is retention
/// authority for this history, not permission to execute or replay a transfer.
#[derive(Clone, Debug)]
pub struct RetainedSources {
    pub(super) history: Identity,
    pub(super) stages: Vec<Identity>,
}

impl RequestRecord {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn phase(&self) -> RequestPhase {
        self.phase
    }

    pub fn payload<T: serde::de::DeserializeOwned>(&self) -> io::Result<T> {
        serde_json::from_slice(&self.body)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}

impl OperationEngine {
    /// Worker-only. Persist acceptance before dispatching any filesystem work.
    /// This short transaction does not wait on a running operation's UI question.
    pub fn queue_request<T: Serialize>(&self, payload: &T) -> io::Result<RequestRecord> {
        self.reads.queue_request(payload, None)
    }

    /// Prepare dependencies without taking the mutation worker's lock. Paths
    /// preserve their final links; selecting a directory includes owned data
    /// inside that directory. No filesystem mutation occurs during capture.
    pub fn capture_retained_sources(&self, paths: &[PathBuf]) -> io::Result<RetainedSources> {
        self.reads.snapshot(false)?.capture_retained_sources(paths)
    }

    /// Accept the request and its retained-data protection atomically. A stale
    /// capture cannot resurrect data already consumed by another operation.
    pub fn queue_request_with_sources<T: Serialize>(
        &self,
        payload: &T,
        sources: &RetainedSources,
    ) -> io::Result<RequestRecord> {
        self.reads.queue_request(payload, Some(sources))
    }

    /// Retention follows this live lease's catalog relocations. It neither
    /// reselects current paths nor supplies execution authority; the application
    /// must still carry its original selected-entry and storage observations.
    pub(crate) fn queue_request_with_source_lease<T: Serialize>(
        &self,
        payload: &T,
        lease: &RetainedSourceLease,
    ) -> io::Result<RequestRecord> {
        self.reads.queue_request_with_source_lease(payload, lease)
    }

    /// Admission for a single-use prepared intent. Protection moves to the new
    /// request in the same transaction; a refusal leaves the lease unchanged.
    pub(crate) fn queue_request_consuming_source_lease<T: Serialize>(
        &self,
        payload: &T,
        lease: &RetainedSourceLease,
    ) -> io::Result<RequestRecord> {
        self.reads
            .queue_request_consuming_source_lease(payload, lease)
    }

    /// Save against the exact previously read revision. A stale writer cannot
    /// resurrect dismissed work or replace a newer continuation.
    pub fn checkpoint_request<T: Serialize>(
        &self,
        previous: &RequestRecord,
        phase: RequestPhase,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.reads.checkpoint_request(previous, phase, payload)
    }

    /// Finish a reviewed, nonexecuting application request without losing its
    /// execution evidence. Journal checks and reservation release are atomic.
    pub(crate) fn finish_reviewed_request<T: Serialize>(
        &self,
        previous: &RequestRecord,
        payload: &T,
    ) -> io::Result<RequestRecord> {
        self.reads.finish_reviewed_request(previous, payload)
    }

    /// Atomically consume a stopped request's selected plan and accept its
    /// successor. Keep any other failed or remaining plan in the original row.
    /// Returns the updated original record and the new Queued record.
    pub fn handoff_request<T: Serialize, U: Serialize>(
        &self,
        previous: &RequestRecord,
        remaining_phase: RequestPhase,
        remaining_payload: &T,
        successor_payload: &U,
    ) -> io::Result<(RequestRecord, RequestRecord)> {
        self.reads.handoff_request(
            previous,
            remaining_phase,
            remaining_payload,
            successor_payload,
        )
    }

    /// Dismiss scheduling metadata, never completed changes or their Undo data.
    /// Running work must first reach a durable stopped or finished checkpoint.
    pub fn dismiss_request(&self, previous: &RequestRecord) -> io::Result<()> {
        self.reads.dismiss_request(previous)
    }

    /// Newest first, at most 100 rows. Pass the last ID to inspect older rows.
    /// Loading records never executes them, including an interrupted Running row.
    pub fn request_page(&self, before: Option<u64>) -> io::Result<Vec<RequestRecord>> {
        self.reads.snapshot(false)?.request_page(before)
    }
}
