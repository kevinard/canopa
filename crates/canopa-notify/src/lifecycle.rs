use crate::clock::now_millis;
use crate::{
    AcceptedNotification, DeliveryDecision, DeliveryOutcome, NotificationReceipt,
    NotificationStatus, NotificationThread, NotificationTimeout, NotificationTimeoutDefaults,
    SenderCloseReason, SenderEvent, Urgency,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifecycleDelivery {
    pub outcome: DeliveryOutcome,
    pub rule_reference: Option<String>,
}

impl LifecycleDelivery {
    pub fn shown() -> Self {
        Self {
            outcome: DeliveryOutcome::Shown,
            rule_reference: None,
        }
    }

    pub fn silenced(rule_reference: impl Into<String>) -> Self {
        Self {
            outcome: DeliveryOutcome::Silenced,
            rule_reference: Some(rule_reference.into()),
        }
    }

    pub fn dropped(rule_reference: impl Into<String>) -> Self {
        Self {
            outcome: DeliveryOutcome::Dropped,
            rule_reference: Some(rule_reference.into()),
        }
    }

    pub(crate) fn from_decision(decision: &DeliveryDecision) -> Self {
        Self {
            outcome: decision.outcome,
            rule_reference: decision.rule_reference.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewNotificationThread {
    pub accepted_notification: AcceptedNotification,
    pub status: NotificationStatus,
    pub delivery_outcome: DeliveryOutcome,
    pub rule_reference: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistPrecondition {
    ExpectedRevision(i64),
    ExpectedActiveRevision(i64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecycleEffect {
    SenderReceipt(NotificationReceipt),
    ThreadCreated(NotificationThread),
    ThreadRevised(NotificationThread),
    ThreadSilenced(NotificationThread),
    ThreadDismissed(NotificationThread),
    ThreadExpired(NotificationThread),
    ThreadClosed(NotificationThread),
    ThreadStale(NotificationThread),
    NotificationDropped {
        replaces_id: Option<i64>,
    },
    NotificationTimeoutStartsOnDisplay {
        notification_id: i64,
        revision: i64,
        timeout: NotificationTimeout,
        urgency: Urgency,
    },
    NotificationTimeoutCancelledThroughRevision {
        notification_id: i64,
        revision: i64,
    },
    ActionInvoked {
        notification_id: i64,
        action_key: String,
        app_identity: String,
    },
    SenderClosed {
        notification_id: i64,
        reason: SenderCloseReason,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LifecycleEffects {
    effects: Vec<LifecycleEffect>,
}

/// A bulk operation may stop on a storage error after earlier transitions committed.
/// Consumers must apply `effects` even when `result` reports partial failure.
#[derive(Debug)]
pub struct LifecycleBatch {
    pub effects: LifecycleEffects,
    pub result: anyhow::Result<()>,
}

/// Safe operation outcome; the underlying storage error stays out of wire replies.
#[derive(Debug)]
pub struct PartialClearError {
    pub dismissed: usize,
    source: anyhow::Error,
}

impl std::fmt::Display for PartialClearError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "clear stopped after {} committed dismissals",
            self.dismissed
        )
    }
}

impl std::error::Error for PartialClearError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl LifecycleEffects {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn new(effects: Vec<LifecycleEffect>) -> Self {
        Self { effects }
    }

    pub fn raw(&self) -> &[LifecycleEffect] {
        &self.effects
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub(crate) fn extend(&mut self, effects: LifecycleEffects) {
        self.effects.extend(effects.effects);
    }

    pub fn sender_receipt(&self) -> anyhow::Result<NotificationReceipt> {
        self.effects
            .iter()
            .find_map(|effect| match effect {
                LifecycleEffect::SenderReceipt(receipt) => Some(receipt.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                anyhow::anyhow!("notification lifecycle did not return a sender receipt")
            })
    }

    pub fn sender_events(&self) -> impl Iterator<Item = SenderEvent> + '_ {
        self.effects.iter().filter_map(|effect| match effect {
            LifecycleEffect::ActionInvoked {
                notification_id,
                action_key,
                ..
            } => Some(SenderEvent::ActionInvoked {
                notification_id: *notification_id,
                action_key: action_key.clone(),
            }),
            LifecycleEffect::SenderClosed {
                notification_id,
                reason,
            } => Some(SenderEvent::NotificationClosed {
                notification_id: *notification_id,
                reason: *reason,
            }),
            _ => None,
        })
    }

    pub fn timeout_changes(&self) -> impl Iterator<Item = NotificationTimeoutChange> + '_ {
        self.effects.iter().filter_map(|effect| match effect {
            LifecycleEffect::NotificationTimeoutStartsOnDisplay {
                notification_id,
                revision,
                timeout,
                urgency,
            } => Some(NotificationTimeoutChange::StartsOnDisplay {
                notification_id: *notification_id,
                revision: *revision,
                timeout: *timeout,
                urgency: *urgency,
            }),
            LifecycleEffect::NotificationTimeoutCancelledThroughRevision {
                notification_id,
                revision,
            } => Some(NotificationTimeoutChange::CancelThroughRevision {
                notification_id: *notification_id,
                revision: *revision,
            }),
            _ => None,
        })
    }

    pub fn banner_changes(&self) -> impl Iterator<Item = BannerLifecycleChange> + '_ {
        self.effects.iter().filter_map(|effect| match effect {
            LifecycleEffect::ThreadCreated(thread) | LifecycleEffect::ThreadRevised(thread) => {
                Some(BannerLifecycleChange::ConsiderOpportunity(thread.clone()))
            }
            LifecycleEffect::ThreadSilenced(thread)
            | LifecycleEffect::ThreadDismissed(thread)
            | LifecycleEffect::ThreadExpired(thread)
            | LifecycleEffect::ThreadClosed(thread)
            | LifecycleEffect::ThreadStale(thread) => Some(BannerLifecycleChange::Remove {
                notification_id: thread.id,
            }),
            LifecycleEffect::SenderReceipt(_)
            | LifecycleEffect::NotificationDropped { .. }
            | LifecycleEffect::NotificationTimeoutStartsOnDisplay { .. }
            | LifecycleEffect::NotificationTimeoutCancelledThroughRevision { .. }
            | LifecycleEffect::ActionInvoked { .. }
            | LifecycleEffect::SenderClosed { .. } => None,
        })
    }
}

impl IntoIterator for LifecycleEffects {
    type Item = LifecycleEffect;
    type IntoIter = std::vec::IntoIter<LifecycleEffect>;

    fn into_iter(self) -> Self::IntoIter {
        self.effects.into_iter()
    }
}

impl<'a> IntoIterator for &'a LifecycleEffects {
    type Item = &'a LifecycleEffect;
    type IntoIter = std::slice::Iter<'a, LifecycleEffect>;

    fn into_iter(self) -> Self::IntoIter {
        self.effects.iter()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationTimeoutChange {
    StartsOnDisplay {
        notification_id: i64,
        revision: i64,
        timeout: NotificationTimeout,
        urgency: Urgency,
    },
    CancelThroughRevision {
        notification_id: i64,
        revision: i64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BannerLifecycleChange {
    ConsiderOpportunity(NotificationThread),
    Remove { notification_id: i64 },
}

#[allow(async_fn_in_trait)]
pub trait NotificationHistory: Clone + Send + Sync + 'static {
    async fn create_thread(
        &self,
        thread: NewNotificationThread,
    ) -> anyhow::Result<NotificationThread>;
    async fn thread(&self, id: i64) -> anyhow::Result<NotificationThread>;
    async fn persist_thread(
        &self,
        thread: NotificationThread,
        precondition: PersistPrecondition,
    ) -> anyhow::Result<NotificationThread>;
    async fn active_threads(&self) -> anyhow::Result<Vec<NotificationThread>>;
}

#[derive(Clone)]
pub struct NotificationLifecycle<H> {
    history: H,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    timeout_defaults: NotificationTimeoutDefaults,
    transient_threads: Arc<Mutex<HashMap<i64, NotificationThread>>>,
    next_ephemeral_notification_id: Arc<AtomicI64>,
}

impl<H> NotificationLifecycle<H>
where
    H: NotificationHistory,
{
    pub fn new(history: H) -> Self {
        Self::with_clock(history, now_millis)
    }

    pub fn with_timeout_defaults(
        history: H,
        timeout_defaults: NotificationTimeoutDefaults,
    ) -> Self {
        Self::with_clock_and_timeout_defaults(history, now_millis, timeout_defaults)
    }

    pub fn with_clock(history: H, clock: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        Self::with_clock_and_timeout_defaults(
            history,
            clock,
            NotificationTimeoutDefaults::default(),
        )
    }

    pub fn with_clock_and_timeout_defaults(
        history: H,
        clock: impl Fn() -> i64 + Send + Sync + 'static,
        timeout_defaults: NotificationTimeoutDefaults,
    ) -> Self {
        Self {
            history,
            clock: Arc::new(clock),
            timeout_defaults,
            transient_threads: Arc::new(Mutex::new(HashMap::new())),
            next_ephemeral_notification_id: Arc::new(AtomicI64::new(-1)),
        }
    }

    pub async fn receive(
        &self,
        notification: AcceptedNotification,
        delivery: LifecycleDelivery,
        timeout: NotificationTimeout,
    ) -> anyhow::Result<LifecycleEffects> {
        if delivery.outcome == DeliveryOutcome::Dropped {
            return self
                .drop_notification(&notification.app_identity, notification.replaces_id)
                .await;
        }

        if let Some(existing) = self.replacement_target_thread(&notification).await? {
            let expected_revision = existing.revision;
            let thread = self.revised_thread(existing, notification, delivery.clone());
            let thread = self
                .persist_thread(
                    thread,
                    PersistPrecondition::ExpectedActiveRevision(expected_revision),
                )
                .await?;
            let mut effects = vec![LifecycleEffect::ThreadRevised(thread.clone())];
            if delivery.outcome == DeliveryOutcome::Silenced {
                effects.push(LifecycleEffect::ThreadSilenced(thread.clone()));
            }
            self.push_timeout_effect(&mut effects, &thread, timeout, true);
            effects.push(LifecycleEffect::SenderReceipt(NotificationReceipt {
                notification_id: thread.id,
                delivery_outcome: delivery.outcome,
            }));
            return Ok(LifecycleEffects::new(effects));
        }

        let new_thread = self.new_thread(notification, delivery.clone());
        let thread = if new_thread.accepted_notification.transient {
            self.create_transient_thread(new_thread)
        } else {
            self.history.create_thread(new_thread).await?
        };
        let mut effects = vec![LifecycleEffect::ThreadCreated(thread.clone())];
        if delivery.outcome == DeliveryOutcome::Silenced {
            effects.push(LifecycleEffect::ThreadSilenced(thread.clone()));
        }
        self.push_timeout_effect(&mut effects, &thread, timeout, false);
        effects.push(LifecycleEffect::SenderReceipt(NotificationReceipt {
            notification_id: thread.id,
            delivery_outcome: delivery.outcome,
        }));
        Ok(LifecycleEffects::new(effects))
    }

    pub(crate) fn timeout_duration_on_display(
        &self,
        timeout: NotificationTimeout,
        urgency: Urgency,
    ) -> Option<Duration> {
        timeout.duration(urgency, self.timeout_defaults)
    }

    pub async fn drop_notification(
        &self,
        app_identity: &str,
        replaces_id: Option<i64>,
    ) -> anyhow::Result<LifecycleEffects> {
        let replacement_target = self.replacement_target(app_identity, replaces_id).await?;
        if let Some(existing) = replacement_target {
            return Ok(LifecycleEffects::new(vec![
                LifecycleEffect::NotificationDropped {
                    replaces_id: Some(existing.id),
                },
                LifecycleEffect::SenderReceipt(NotificationReceipt {
                    notification_id: existing.id,
                    delivery_outcome: DeliveryOutcome::Dropped,
                }),
            ]));
        }

        let notification_id = self
            .next_ephemeral_notification_id
            .fetch_sub(1, Ordering::Relaxed);
        Ok(LifecycleEffects::new(vec![
            LifecycleEffect::NotificationDropped { replaces_id: None },
            LifecycleEffect::SenderReceipt(NotificationReceipt {
                notification_id,
                delivery_outcome: DeliveryOutcome::Dropped,
            }),
            LifecycleEffect::SenderClosed {
                notification_id,
                reason: SenderCloseReason::Undefined,
            },
        ]))
    }

    async fn replacement_target_thread(
        &self,
        notification: &AcceptedNotification,
    ) -> anyhow::Result<Option<NotificationThread>> {
        self.replacement_target(&notification.app_identity, notification.replaces_id)
            .await
    }

    async fn replacement_target(
        &self,
        _app_identity: &str,
        replaces_id: Option<i64>,
    ) -> anyhow::Result<Option<NotificationThread>> {
        let Some(replaces_id) = replaces_id else {
            return Ok(None);
        };

        Ok(self
            .active_threads()
            .await?
            .into_iter()
            .find(|thread| thread.id == replaces_id))
    }

    pub async fn dismiss(&self, notification_id: i64) -> anyhow::Result<LifecycleEffects> {
        self.transition_active(notification_id, NotificationStatus::Dismissed)
            .await
    }

    pub async fn expire(&self, notification_id: i64) -> anyhow::Result<LifecycleEffects> {
        self.transition_active(notification_id, NotificationStatus::Expired)
            .await
    }

    pub async fn expire_timeout_if_current_revision(
        &self,
        notification_id: i64,
        revision: i64,
    ) -> anyhow::Result<LifecycleEffects> {
        let thread = self.thread(notification_id).await?;
        if thread.status != NotificationStatus::Active || thread.revision != revision {
            return Ok(LifecycleEffects::empty());
        }

        self.transition_thread(thread, NotificationStatus::Expired)
            .await
    }

    pub async fn close(&self, notification_id: i64) -> anyhow::Result<LifecycleEffects> {
        self.transition_active(notification_id, NotificationStatus::Closed)
            .await
    }

    pub async fn clear_active_notifications(&self) -> anyhow::Result<LifecycleBatch> {
        let mut effects = LifecycleEffects::empty();
        for (completed, thread) in self.active_threads().await?.into_iter().enumerate() {
            match self.dismiss(thread.id).await {
                Ok(committed) => effects.extend(committed),
                Err(error) => {
                    return Ok(LifecycleBatch {
                        effects,
                        result: Err(PartialClearError {
                            dismissed: completed,
                            source: error,
                        }
                        .into()),
                    });
                }
            }
        }
        Ok(LifecycleBatch {
            effects,
            result: Ok(()),
        })
    }

    /// Validate an action without committing it, before bounded external token acquisition.
    pub(crate) async fn prepare_action(
        &self,
        notification_id: i64,
        action_key: &str,
    ) -> anyhow::Result<NotificationThread> {
        let thread = self.thread(notification_id).await?;
        anyhow::ensure!(
            thread.status == NotificationStatus::Active,
            "notification {notification_id} is not active"
        );
        anyhow::ensure!(
            thread
                .accepted_notification
                .actions
                .iter()
                .any(|action| action.key == action_key),
            "notification {notification_id} has no action {action_key}"
        );
        Ok(thread)
    }

    pub async fn invoke_action(
        &self,
        notification_id: i64,
        action_key: &str,
    ) -> anyhow::Result<LifecycleEffects> {
        self.invoke_action_if_current_revision(notification_id, action_key, None)
            .await
    }

    pub(crate) async fn invoke_action_if_current_revision(
        &self,
        notification_id: i64,
        action_key: &str,
        revision: Option<i64>,
    ) -> anyhow::Result<LifecycleEffects> {
        let thread = self.prepare_action(notification_id, action_key).await?;
        anyhow::ensure!(
            revision.is_none_or(|revision| thread.revision == revision),
            "notification was replaced before action invocation"
        );
        let mut effects = LifecycleEffects::new(vec![LifecycleEffect::ActionInvoked {
            notification_id,
            action_key: action_key.to_string(),
            app_identity: thread.accepted_notification.app_identity.clone(),
        }]);

        if !thread.accepted_notification.resident {
            effects.extend(
                self.transition_thread(thread, NotificationStatus::Dismissed)
                    .await?,
            );
        } else {
            let revision = thread.revision;
            self.persist_thread(
                thread,
                PersistPrecondition::ExpectedActiveRevision(revision),
            )
            .await?;
        }

        Ok(effects)
    }

    pub async fn mark_active_stale(&self) -> anyhow::Result<LifecycleEffects> {
        let mut effects = Vec::new();
        for thread in self.history.active_threads().await? {
            let notification_id = thread.id;
            let expected_revision = thread.revision;
            let stale = self.transitioned_thread(thread, NotificationStatus::Stale);
            let stale = self
                .history
                .persist_thread(
                    stale,
                    PersistPrecondition::ExpectedActiveRevision(expected_revision),
                )
                .await?;
            effects.push(LifecycleEffect::ThreadStale(stale));
            effects.push(
                LifecycleEffect::NotificationTimeoutCancelledThroughRevision {
                    notification_id,
                    revision: expected_revision,
                },
            );
        }
        Ok(LifecycleEffects::new(effects))
    }

    async fn transition_active(
        &self,
        notification_id: i64,
        status: NotificationStatus,
    ) -> anyhow::Result<LifecycleEffects> {
        let thread = self.thread(notification_id).await?;
        self.transition_thread(thread, status).await
    }

    async fn transition_thread(
        &self,
        thread: NotificationThread,
        status: NotificationStatus,
    ) -> anyhow::Result<LifecycleEffects> {
        let notification_id = thread.id;
        if thread.status != NotificationStatus::Active {
            anyhow::bail!("notification {notification_id} is not active");
        }

        let expected_revision = thread.revision;
        let thread = self.transitioned_thread(thread, status);
        let thread = self
            .persist_thread(
                thread,
                PersistPrecondition::ExpectedActiveRevision(expected_revision),
            )
            .await?;
        let revision = thread.revision;
        let transition = match status {
            NotificationStatus::Dismissed => LifecycleEffect::ThreadDismissed(thread),
            NotificationStatus::Expired => LifecycleEffect::ThreadExpired(thread),
            NotificationStatus::Closed => LifecycleEffect::ThreadClosed(thread),
            NotificationStatus::Stale => LifecycleEffect::ThreadStale(thread),
            NotificationStatus::Active => unreachable!("active is not a lifecycle transition"),
        };
        Ok(LifecycleEffects::new(vec![
            transition,
            LifecycleEffect::NotificationTimeoutCancelledThroughRevision {
                notification_id,
                revision,
            },
            LifecycleEffect::SenderClosed {
                notification_id,
                reason: status.into(),
            },
        ]))
    }

    fn new_thread(
        &self,
        notification: AcceptedNotification,
        delivery: LifecycleDelivery,
    ) -> NewNotificationThread {
        let now = self.now_millis();
        NewNotificationThread {
            accepted_notification: notification,
            status: NotificationStatus::Active,
            delivery_outcome: delivery.outcome,
            rule_reference: delivery.rule_reference,
            created_at: now,
            updated_at: now,
        }
    }

    fn revised_thread(
        &self,
        mut thread: NotificationThread,
        mut notification: AcceptedNotification,
        delivery: LifecycleDelivery,
    ) -> NotificationThread {
        notification.transient = thread.accepted_notification.transient;
        thread.accepted_notification = notification;
        thread.status = NotificationStatus::Active;
        thread.delivery_outcome = delivery.outcome;
        thread.rule_reference = delivery.rule_reference;
        thread.revision += 1;
        thread.updated_at = self.now_millis();
        thread.closed_at = None;
        thread
    }

    fn transitioned_thread(
        &self,
        mut thread: NotificationThread,
        status: NotificationStatus,
    ) -> NotificationThread {
        let now = self.now_millis();
        thread.status = status;
        thread.updated_at = now;
        thread.closed_at = Some(now);
        thread
    }

    fn now_millis(&self) -> i64 {
        (self.clock)()
    }

    pub async fn thread(&self, notification_id: i64) -> anyhow::Result<NotificationThread> {
        if let Some(thread) = self
            .transient_threads
            .lock()
            .unwrap()
            .get(&notification_id)
            .cloned()
        {
            return Ok(thread);
        }

        self.history.thread(notification_id).await
    }

    async fn active_threads(&self) -> anyhow::Result<Vec<NotificationThread>> {
        let mut threads = self.history.active_threads().await?;
        threads.extend(
            self.transient_threads
                .lock()
                .unwrap()
                .values()
                .filter(|thread| thread.status == NotificationStatus::Active)
                .cloned(),
        );
        Ok(threads)
    }

    fn create_transient_thread(&self, thread: NewNotificationThread) -> NotificationThread {
        let id = self
            .next_ephemeral_notification_id
            .fetch_sub(1, Ordering::Relaxed);
        let thread = NotificationThread {
            id,
            accepted_notification: thread.accepted_notification,
            status: thread.status,
            delivery_outcome: thread.delivery_outcome,
            rule_reference: thread.rule_reference,
            revision: 1,
            created_at: thread.created_at,
            updated_at: thread.updated_at,
            closed_at: None,
        };
        self.transient_threads
            .lock()
            .unwrap()
            .insert(id, thread.clone());
        thread
    }

    async fn persist_thread(
        &self,
        thread: NotificationThread,
        precondition: PersistPrecondition,
    ) -> anyhow::Result<NotificationThread> {
        if !thread.accepted_notification.transient {
            return self.history.persist_thread(thread, precondition).await;
        }

        let mut transient_threads = self.transient_threads.lock().unwrap();
        let current = transient_threads
            .get(&thread.id)
            .ok_or_else(|| anyhow::anyhow!("transient notification {} is not active", thread.id))?;
        let expected_revision = match precondition {
            PersistPrecondition::ExpectedRevision(revision)
            | PersistPrecondition::ExpectedActiveRevision(revision) => revision,
        };
        let requires_active =
            matches!(precondition, PersistPrecondition::ExpectedActiveRevision(_));
        if current.revision != expected_revision
            || (requires_active && current.status != NotificationStatus::Active)
        {
            anyhow::bail!(
                "failed to update transient notification {}; lifecycle precondition was not met",
                thread.id
            );
        }

        if thread.status == NotificationStatus::Active {
            transient_threads.insert(thread.id, thread.clone());
        } else {
            transient_threads.remove(&thread.id);
        }
        Ok(thread)
    }

    fn push_timeout_effect(
        &self,
        effects: &mut Vec<LifecycleEffect>,
        thread: &NotificationThread,
        timeout: NotificationTimeout,
        replace_existing: bool,
    ) {
        if replace_existing {
            effects.push(
                LifecycleEffect::NotificationTimeoutCancelledThroughRevision {
                    notification_id: thread.id,
                    revision: thread.revision.saturating_sub(1),
                },
            );
        }

        if thread.status != NotificationStatus::Active
            || thread.delivery_outcome != DeliveryOutcome::Shown
        {
            return;
        }

        match timeout {
            NotificationTimeout::ServerDefault | NotificationTimeout::Milliseconds(_) => {
                effects.push(LifecycleEffect::NotificationTimeoutStartsOnDisplay {
                    notification_id: thread.id,
                    revision: thread.revision,
                    timeout,
                    urgency: thread.accepted_notification.urgency,
                });
            }
            NotificationTimeout::Never => {}
        }
    }
}
