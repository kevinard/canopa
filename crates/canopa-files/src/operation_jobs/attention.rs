//! A unique host takes current attention responsibility, independently of rows.
use super::*;

const ATTENTION_LIMIT: usize = 1024;
const DELIVERY_LIMIT: usize = 17;

#[derive(Debug)]
pub struct Attention {
    pub id: u64,
    pub title: String,
    pub status: AttentionStatus,
    receipt: Arc<()>,
}

#[derive(Clone, Debug)]
pub enum AttentionStatus {
    Decision { generation: u64 },
    Outcome { phase: Phase, detail: String },
}

#[derive(Default)]
pub(super) struct Inbox {
    claimed: bool,
    pending: BTreeMap<u64, Attention>,
    handed: Vec<(u64, Arc<()>)>,
}

pub struct AttentionObserver {
    snapshot: Arc<Mutex<Snapshot>>,
    inbox: Arc<Mutex<Inbox>>,
    notices: Option<asynchronous::Receiver<()>>,
}

impl OperationJobs {
    /// Claim before exposing a new queue to callers. Restored historical results
    /// are not announced again. The host owns the queue's coalesced wakeups;
    /// dropping this observer never silently discards unhanded responsibility.
    pub fn observe_attention(&self) -> io::Result<AttentionObserver> {
        let mut notices = self.0.notices.lock().unwrap_or_else(|e| e.into_inner());
        let _state = self.0.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        let mut inbox = self.0.attention.lock().unwrap_or_else(|e| e.into_inner());
        if inbox.claimed || notices.is_none() {
            return Err(io::Error::other(
                "Operation attention already has an observer.",
            ));
        }
        inbox.claimed = true;
        Ok(AttentionObserver {
            snapshot: self.0.snapshot.clone(),
            inbox: self.0.attention.clone(),
            notices: notices.take(),
        })
    }
}

impl AttentionObserver {
    pub fn take_notices(&mut self) -> Option<asynchronous::Receiver<()>> {
        self.notices.take()
    }

    /// Hands off at most seventeen concurrent deliveries. Their queue hold stays
    /// in place until explicit acknowledgement, including if this owner drops.
    pub fn take_pending(&self) -> Vec<Attention> {
        let mut state = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        let mut inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
        let available = DELIVERY_LIMIT.saturating_sub(inbox.handed.len());
        let mut pending = Vec::new();
        for _ in 0..available {
            let Some((id, event)) = inbox.pending.pop_first() else {
                break;
            };
            inbox.handed.push((id, event.receipt.clone()));
            pending.push(event);
        }
        state.pending_attention = inbox.pending.len() + inbox.handed.len();
        pending
    }

    /// The host records actual success or visible refusal before acknowledging.
    /// Only a receipt handed to this observer can release its hold.
    pub fn acknowledge(&self, attention: Attention) {
        let mut state = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        let mut inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(index) = inbox
            .handed
            .iter()
            .position(|(_, receipt)| Arc::ptr_eq(receipt, &attention.receipt))
        {
            inbox.handed.swap_remove(index);
        }
        state.pending_attention = inbox.pending.len() + inbox.handed.len();
    }
}

impl Worker {
    pub(super) fn check_attention_capacity(&self) -> io::Result<()> {
        let state = self.state();
        let inbox = self.attention.lock().unwrap_or_else(|e| e.into_inner());
        if !inbox.claimed {
            return Ok(());
        }
        let outstanding: std::collections::BTreeSet<_> = inbox
            .pending
            .keys()
            .copied()
            .chain(inbox.handed.iter().map(|(id, _)| *id))
            .chain(
                state
                    .jobs
                    .iter()
                    .filter(|job| job.phase.holds_application())
                    .map(|job| job.id),
            )
            .collect();
        if outstanding.len() >= ATTENTION_LIMIT {
            return Err(io::Error::other(fl!("files-jobs-attention-full")));
        }
        Ok(())
    }

    pub(super) fn record_attention(&self, state: &mut Snapshot, id: u64) {
        let mut inbox = self.attention.lock().unwrap_or_else(|e| e.into_inner());
        if !inbox.claimed {
            return;
        }
        let Some(job) = state.jobs.iter().find(|job| job.id == id) else {
            return;
        };
        let status = if job.phase == Phase::Waiting {
            let Some(decision) = &job.decision else {
                return;
            };
            AttentionStatus::Decision {
                generation: decision.generation,
            }
        } else if matches!(
            job.phase,
            Phase::Finished
                | Phase::Failed
                | Phase::Stopped
                | Phase::Interrupted
                | Phase::StartSaveFailed
                | Phase::SaveFailed
        ) {
            AttentionStatus::Outcome {
                phase: job.phase,
                detail: job
                    .error
                    .clone()
                    .or(job.detail.clone())
                    .unwrap_or_else(|| fl!("files-completed-detail")),
            }
        } else {
            // A resolved question or retried save no longer needs that unread
            // notice. Already handed delivery keeps its independent receipt.
            inbox.pending.remove(&id);
            state.pending_attention = inbox.pending.len() + inbox.handed.len();
            return;
        };
        // If a question was resolved before its handoff, publish the current
        // outcome rather than obsolete attention. The row may now be trimmed.
        inbox.pending.insert(
            id,
            Attention {
                id,
                title: job.title.clone(),
                status,
                receipt: Arc::new(()),
            },
        );
        state.pending_attention = inbox.pending.len() + inbox.handed.len();
    }
}
