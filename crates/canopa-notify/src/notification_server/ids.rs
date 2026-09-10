//! Session-global live mappings and bounded issued-ID memory (ADR 0029).
use std::collections::{BTreeMap, HashMap};
use std::num::{NonZeroI64, NonZeroU32};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct SenderNotificationId(NonZeroU32);

impl SenderNotificationId {
    pub(super) fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct InternalNotificationId(NonZeroI64);

impl InternalNotificationId {
    pub(super) fn get(self) -> i64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ResolvedRequest {
    pub(super) value: SenderNotificationId,
    pub(super) replacement_target: Option<InternalNotificationId>,
}

#[derive(Debug)]
pub(super) enum IdError {
    Retired,
    Exhausted,
    Fragmented,
}

impl std::fmt::Display for IdError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Retired => "notification ID is retired; use replaces_id 0 for a new notification",
            Self::Exhausted => "notification ID space exhausted",
            Self::Fragmented => "caller-supplied notification ID range limit reached",
        })
    }
}

impl std::error::Error for IdError {}

#[derive(Clone)]
pub(super) struct NotificationSenderIds {
    state: Arc<Mutex<State>>,
}

struct State {
    issued: IssuedIds,
    sender_to_internal: HashMap<SenderNotificationId, InternalNotificationId>,
    internal_to_sender: HashMap<InternalNotificationId, SenderNotificationId>,
}

impl NotificationSenderIds {
    pub(super) fn new() -> Self {
        Self::with_limits(u32::MAX, 4096)
    }

    // Smaller limits let the D-Bus fixture exercise numeric exhaustion and range pressure.
    fn with_limits(max_id: u32, max_sparse_ranges: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                issued: IssuedIds {
                    prefix: 0,
                    sparse: BTreeMap::new(),
                    max_id,
                    max_sparse_ranges,
                },
                sender_to_internal: HashMap::new(),
                internal_to_sender: HashMap::new(),
            })),
        }
    }

    #[cfg(test)]
    pub(super) fn for_test(max_id: u32, max_sparse_ranges: usize) -> Self {
        Self::with_limits(max_id, max_sparse_ranges)
    }

    /// Prepare and commit must run under the same daemon receipt guard. Preparation does
    /// not consume an ID: a failed receipt can be retried without retiring a caller token.
    pub(super) fn prepare(&self, replaces_id: u32) -> Result<ResolvedRequest, IdError> {
        let state = self.state.lock().unwrap();
        let id = if replaces_id == 0 {
            state
                .issued
                .prefix
                .checked_add(1)
                .ok_or(IdError::Exhausted)?
        } else {
            replaces_id
        };
        if id > state.issued.max_id {
            return Err(IdError::Exhausted);
        }
        let value = SenderNotificationId(NonZeroU32::new(id).unwrap());
        if let Some(&internal_id) = state.sender_to_internal.get(&value) {
            return Ok(ResolvedRequest {
                value,
                replacement_target: Some(internal_id),
            });
        }
        state.issued.admit(id)?;
        Ok(ResolvedRequest {
            value,
            replacement_target: None,
        })
    }

    pub(super) fn commit(&self, request: ResolvedRequest, internal_id: i64) {
        // Every committed receipt has a nonzero internal identity, including drops.
        let internal_id = InternalNotificationId(NonZeroI64::new(internal_id).unwrap());
        let mut state = self.state.lock().unwrap();
        state.issued.insert(request.value.get());
        if let Some(previous) = state.sender_to_internal.insert(request.value, internal_id) {
            state.internal_to_sender.remove(&previous);
        }
        if let Some(previous) = state.internal_to_sender.insert(internal_id, request.value) {
            state.sender_to_internal.remove(&previous);
        }
    }

    pub(super) fn internal_id(&self, wire_id: u32) -> Option<InternalNotificationId> {
        let id = SenderNotificationId(NonZeroU32::new(wire_id)?);
        self.state
            .lock()
            .unwrap()
            .sender_to_internal
            .get(&id)
            .copied()
    }

    pub(super) fn sender_id(&self, internal_id: i64) -> Option<SenderNotificationId> {
        let id = InternalNotificationId(NonZeroI64::new(internal_id)?);
        self.state
            .lock()
            .unwrap()
            .internal_to_sender
            .get(&id)
            .copied()
    }

    pub(super) fn retire(&self, internal_id: i64) -> Option<SenderNotificationId> {
        let id = InternalNotificationId(NonZeroI64::new(internal_id)?);
        let mut state = self.state.lock().unwrap();
        let sender_id = state.internal_to_sender.remove(&id)?;
        state.sender_to_internal.remove(&sender_id);
        Some(sender_id)
    }
}

/// All committed IDs, not just live ones. `prefix` covers 1..=prefix (initially
/// empty). Sparse intervals are disjoint, non-adjacent, and above prefix + 1.
/// Automatic allocation always extends the prefix, even at the sparse budget.
struct IssuedIds {
    prefix: u32,
    sparse: BTreeMap<u32, u32>,
    max_id: u32,
    max_sparse_ranges: usize,
}

impl IssuedIds {
    fn admit(&self, id: u32) -> Result<(), IdError> {
        let left = self.sparse.range(..=id).next_back();
        if id <= self.prefix || left.is_some_and(|(_, &end)| id <= end) {
            return Err(IdError::Retired);
        }
        let joins_prefix = self.prefix.checked_add(1) == Some(id);
        let joins_left = left.is_some_and(|(_, &end)| end.checked_add(1) == Some(id));
        let joins_right = id
            .checked_add(1)
            .is_some_and(|next| self.sparse.contains_key(&next));
        if !joins_prefix
            && !joins_left
            && !joins_right
            && self.sparse.len() >= self.max_sparse_ranges
        {
            return Err(IdError::Fragmented);
        }
        Ok(())
    }

    fn insert(&mut self, id: u32) {
        if id <= self.prefix {
            return;
        }
        let left = self
            .sparse
            .range(..=id)
            .next_back()
            .map(|(&start, &end)| (start, end));
        if left.is_some_and(|(_, end)| id <= end) {
            return; // A live replacement leaves the issued set unchanged.
        }
        if self.prefix.checked_add(1) == Some(id) {
            self.prefix = id;
            if let Some(end) = id.checked_add(1).and_then(|next| self.sparse.remove(&next)) {
                self.prefix = end;
            }
            return;
        }
        let start = match left {
            Some((start, end)) if end.checked_add(1) == Some(id) => {
                self.sparse.remove(&start);
                start
            }
            _ => id,
        };
        let end = id
            .checked_add(1)
            .and_then(|next| self.sparse.remove(&next))
            .unwrap_or(id);
        self.sparse.insert(start, end);
    }
}
