use crate::clock::now_millis;
use crate::{
    AcceptedNotification, DeliveryDecision, DeliveryOutcome, NotificationReceipt,
    NotificationStatus, NotificationThread,
};
use std::sync::Arc;

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
    ThreadDeliveryChanged(NotificationThread),
    ThreadSilenced(NotificationThread),
    ThreadDismissed(NotificationThread),
    ThreadExpired(NotificationThread),
    ThreadClosed(NotificationThread),
    ThreadStale(NotificationThread),
    NotificationDropped {
        replaces_id: Option<i64>,
    },
    ActionInvoked {
        notification_id: i64,
        action_key: String,
    },
    SenderClosed {
        notification_id: i64,
        reason: NotificationStatus,
    },
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
}

impl<H> NotificationLifecycle<H>
where
    H: NotificationHistory,
{
    pub fn new(history: H) -> Self {
        Self::with_clock(history, now_millis)
    }

    pub fn with_clock(history: H, clock: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        Self {
            history,
            clock: Arc::new(clock),
        }
    }

    pub async fn receive(
        &self,
        notification: AcceptedNotification,
        delivery: LifecycleDelivery,
    ) -> anyhow::Result<Vec<LifecycleEffect>> {
        if delivery.outcome == DeliveryOutcome::Dropped {
            return Ok(vec![
                LifecycleEffect::NotificationDropped {
                    replaces_id: notification.replaces_id,
                },
                LifecycleEffect::SenderReceipt(NotificationReceipt {
                    notification_id: 0,
                    delivery_outcome: DeliveryOutcome::Dropped,
                }),
            ]);
        }

        if let Some(replaces_id) = notification.replaces_id
            && let Ok(existing) = self.history.thread(replaces_id).await
        {
            let expected_revision = existing.revision;
            let thread = self.revised_thread(existing, notification, delivery.clone());
            let thread = self
                .history
                .persist_thread(
                    thread,
                    PersistPrecondition::ExpectedRevision(expected_revision),
                )
                .await?;
            let mut effects = vec![LifecycleEffect::ThreadRevised(thread.clone())];
            if delivery.outcome == DeliveryOutcome::Silenced {
                effects.push(LifecycleEffect::ThreadSilenced(thread.clone()));
            }
            effects.push(LifecycleEffect::SenderReceipt(NotificationReceipt {
                notification_id: thread.id,
                delivery_outcome: delivery.outcome,
            }));
            return Ok(effects);
        }

        let thread = self
            .history
            .create_thread(self.new_thread(notification, delivery.clone()))
            .await?;
        let mut effects = vec![LifecycleEffect::ThreadCreated(thread.clone())];
        if delivery.outcome == DeliveryOutcome::Silenced {
            effects.push(LifecycleEffect::ThreadSilenced(thread.clone()));
        }
        effects.push(LifecycleEffect::SenderReceipt(NotificationReceipt {
            notification_id: thread.id,
            delivery_outcome: delivery.outcome,
        }));
        Ok(effects)
    }

    pub async fn dismiss(&self, notification_id: i64) -> anyhow::Result<Vec<LifecycleEffect>> {
        self.transition_active(notification_id, NotificationStatus::Dismissed)
            .await
    }

    pub async fn expire(&self, notification_id: i64) -> anyhow::Result<Vec<LifecycleEffect>> {
        self.transition_active(notification_id, NotificationStatus::Expired)
            .await
    }

    pub async fn close(&self, notification_id: i64) -> anyhow::Result<Vec<LifecycleEffect>> {
        self.transition_active(notification_id, NotificationStatus::Closed)
            .await
    }

    pub async fn clear_active_notifications(&self) -> anyhow::Result<Vec<LifecycleEffect>> {
        let mut effects = Vec::new();
        for thread in self.history.active_threads().await? {
            effects.extend(self.dismiss(thread.id).await?);
        }
        Ok(effects)
    }

    pub async fn invoke_action(
        &self,
        notification_id: i64,
        action_key: &str,
    ) -> anyhow::Result<Vec<LifecycleEffect>> {
        let thread = self.history.thread(notification_id).await?;
        if thread.status != NotificationStatus::Active {
            anyhow::bail!("notification {notification_id} is not active");
        }

        if !thread
            .accepted_notification
            .actions
            .iter()
            .any(|action| action.key == action_key)
        {
            anyhow::bail!("notification {notification_id} has no action {action_key}");
        }

        let mut effects = vec![LifecycleEffect::ActionInvoked {
            notification_id,
            action_key: action_key.to_string(),
        }];

        if !thread.accepted_notification.resident {
            effects.extend(self.close(notification_id).await?);
        }

        Ok(effects)
    }

    pub async fn apply_delivery_decision(
        &self,
        notification_id: i64,
        delivery: LifecycleDelivery,
    ) -> anyhow::Result<Vec<LifecycleEffect>> {
        let thread = self.history.thread(notification_id).await?;
        if thread.status != NotificationStatus::Active {
            anyhow::bail!("notification {notification_id} is not active");
        }

        let expected_revision = thread.revision;
        let updated = self.delivery_changed_thread(thread, delivery.clone());
        let updated = self
            .history
            .persist_thread(
                updated,
                PersistPrecondition::ExpectedActiveRevision(expected_revision),
            )
            .await?;
        let mut effects = vec![LifecycleEffect::ThreadDeliveryChanged(updated.clone())];

        match delivery.outcome {
            DeliveryOutcome::Shown => {}
            DeliveryOutcome::Silenced => effects.push(LifecycleEffect::ThreadSilenced(updated)),
            DeliveryOutcome::Dropped => {
                effects.push(LifecycleEffect::NotificationDropped {
                    replaces_id: Some(notification_id),
                });
                effects.extend(self.close(notification_id).await?);
            }
        }

        Ok(effects)
    }

    pub async fn mark_active_stale(&self) -> anyhow::Result<Vec<LifecycleEffect>> {
        let mut effects = Vec::new();
        for thread in self.history.active_threads().await? {
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
        }
        Ok(effects)
    }

    async fn transition_active(
        &self,
        notification_id: i64,
        status: NotificationStatus,
    ) -> anyhow::Result<Vec<LifecycleEffect>> {
        let thread = self.history.thread(notification_id).await?;
        if thread.status != NotificationStatus::Active {
            anyhow::bail!("notification {notification_id} is not active");
        }

        let expected_revision = thread.revision;
        let thread = self.transitioned_thread(thread, status);
        let thread = self
            .history
            .persist_thread(
                thread,
                PersistPrecondition::ExpectedActiveRevision(expected_revision),
            )
            .await?;
        let transition = match status {
            NotificationStatus::Dismissed => LifecycleEffect::ThreadDismissed(thread),
            NotificationStatus::Expired => LifecycleEffect::ThreadExpired(thread),
            NotificationStatus::Closed => LifecycleEffect::ThreadClosed(thread),
            NotificationStatus::Stale => LifecycleEffect::ThreadStale(thread),
            NotificationStatus::Active => unreachable!("active is not a lifecycle transition"),
        };
        Ok(vec![
            transition,
            LifecycleEffect::SenderClosed {
                notification_id,
                reason: status,
            },
        ])
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
        notification: AcceptedNotification,
        delivery: LifecycleDelivery,
    ) -> NotificationThread {
        thread.accepted_notification = notification;
        thread.status = NotificationStatus::Active;
        thread.delivery_outcome = delivery.outcome;
        thread.rule_reference = delivery.rule_reference;
        thread.revision += 1;
        thread.updated_at = self.now_millis();
        thread.closed_at = None;
        thread
    }

    fn delivery_changed_thread(
        &self,
        mut thread: NotificationThread,
        delivery: LifecycleDelivery,
    ) -> NotificationThread {
        thread.delivery_outcome = delivery.outcome;
        thread.rule_reference = delivery.rule_reference;
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
}

pub(crate) fn receipt_from_effects(
    effects: &[LifecycleEffect],
) -> anyhow::Result<NotificationReceipt> {
    effects
        .iter()
        .find_map(|effect| match effect {
            LifecycleEffect::SenderReceipt(receipt) => Some(receipt.clone()),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("notification lifecycle did not return a sender receipt"))
}
