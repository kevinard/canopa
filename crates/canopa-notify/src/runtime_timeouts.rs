//! Registered timers, not a history of cancelled IDs. The runtime serializes
//! committed changes; its single timer lock also covers acknowledgement/start.
use crate::{InternalNotificationId, NotificationTimeout, NotificationTimeoutChange, Urgency};
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;
use tokio::task::JoinHandle;

const MAX_EXPIRY_ATTEMPTS: u32 = 6;

struct Entry {
    revision: i64,
    state: State,
    failures: u32,
}

enum State {
    PendingDisplay {
        timeout: NotificationTimeout,
        urgency: Urgency,
    },
    Scheduled(JoinHandle<()>),
    Firing,
    Failed,
}

#[derive(Default)]
pub(crate) struct RuntimeTimeouts {
    entries: HashMap<InternalNotificationId, Entry>,
}

impl RuntimeTimeouts {
    pub(crate) fn cancel_all(&mut self) {
        for (_, entry) in self.entries.drain() {
            if let State::Scheduled(handle) = entry.state {
                handle.abort();
            }
        }
    }

    /// Only ordered, committed lifecycle effects register timers. In particular,
    /// an asynchronous display callback must use `start`, never `apply`.
    pub(crate) fn apply(&mut self, changes: impl IntoIterator<Item = NotificationTimeoutChange>) {
        for change in changes {
            match change {
                NotificationTimeoutChange::StartsOnDisplay {
                    notification_id,
                    revision,
                    timeout,
                    urgency,
                } => {
                    if self
                        .entries
                        .get(&notification_id)
                        .is_some_and(|entry| entry.revision >= revision)
                    {
                        continue;
                    }
                    self.cancel_through(notification_id, revision);
                    self.entries.insert(
                        notification_id,
                        Entry {
                            revision,
                            state: State::PendingDisplay { timeout, urgency },
                            failures: 0,
                        },
                    );
                }
                NotificationTimeoutChange::CancelThroughRevision {
                    notification_id,
                    revision,
                } => {
                    self.cancel_through(notification_id, revision);
                }
            }
        }
    }

    pub(crate) fn pending_in(
        &self,
        visible: &BTreeSet<(InternalNotificationId, i64)>,
    ) -> Vec<(InternalNotificationId, i64)> {
        visible
            .iter()
            .copied()
            .filter(|(id, revision)| {
                self.entries.get(id).is_some_and(|entry| {
                    entry.revision == *revision
                        && matches!(entry.state, State::PendingDisplay { .. })
                })
            })
            .collect()
    }

    /// Check and schedule under the same lock as cancellation. Absent or already
    /// consumed registrations cannot be recreated by delayed/duplicate callbacks.
    pub(crate) fn start(
        &mut self,
        notification_id: InternalNotificationId,
        revision: i64,
        schedule: impl FnOnce(NotificationTimeout, Urgency) -> Option<JoinHandle<()>>,
    ) {
        let Some(entry) = self
            .entries
            .get_mut(&notification_id)
            .filter(|entry| entry.revision == revision)
        else {
            return;
        };
        let (timeout, urgency) = match &entry.state {
            State::PendingDisplay { timeout, urgency } => (*timeout, *urgency),
            State::Scheduled(_) | State::Firing | State::Failed => return,
        };
        if let Some(handle) = schedule(timeout, urgency) {
            entry.state = State::Scheduled(handle);
        } else {
            self.entries.remove(&notification_id);
        }
    }

    /// Detach the firing task's handle before commitment: applying its own
    /// cancellation effect must not abort committed sender publication. Keep the
    /// registration so an unsuccessful storage transition can be retried.
    pub(crate) fn begin_expiry(
        &mut self,
        notification_id: InternalNotificationId,
        revision: i64,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(&notification_id).filter(|entry| {
            entry.revision == revision && matches!(entry.state, State::Scheduled(_))
        }) else {
            return false;
        };
        entry.state = State::Firing;
        true
    }

    pub(crate) fn complete(&mut self, notification_id: InternalNotificationId, revision: i64) {
        if self
            .entries
            .get(&notification_id)
            .is_some_and(|entry| entry.revision == revision)
        {
            self.entries.remove(&notification_id);
        }
    }

    /// Retry only the still-current firing registration. Attempts are bounded;
    /// terminal failure stays visible until explicit completion or replacement.
    /// Returns whether the failure is terminal, or None for obsolete work.
    pub(crate) fn failed(
        &mut self,
        notification_id: InternalNotificationId,
        revision: i64,
        schedule: impl FnOnce(Duration) -> JoinHandle<()>,
    ) -> Option<bool> {
        let entry = self
            .entries
            .get_mut(&notification_id)
            .filter(|entry| entry.revision == revision && matches!(entry.state, State::Firing))?;
        entry.failures += 1;
        let terminal = entry.failures >= MAX_EXPIRY_ATTEMPTS;
        entry.state = if terminal {
            State::Failed
        } else {
            State::Scheduled(schedule(Duration::from_millis(100 << (entry.failures - 1))))
        };
        Some(terminal)
    }

    pub(crate) fn failure_counts(&self) -> (usize, usize) {
        let mut retrying = 0;
        let mut failed = 0;
        for entry in self.entries.values().filter(|entry| entry.failures > 0) {
            if matches!(entry.state, State::Failed) {
                failed += 1;
            } else {
                retrying += 1;
            }
        }
        (retrying, failed)
    }

    pub(crate) fn record_count(&self) -> usize {
        self.entries.len()
    }

    fn cancel_through(&mut self, notification_id: InternalNotificationId, revision: i64) {
        if self
            .entries
            .get(&notification_id)
            .is_some_and(|entry| entry.revision <= revision)
            && let Some(Entry {
                state: State::Scheduled(handle),
                ..
            }) = self.entries.remove(&notification_id)
        {
            handle.abort();
        }
    }
}
