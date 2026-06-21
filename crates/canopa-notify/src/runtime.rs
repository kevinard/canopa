use crate::control::{
    AppPermissionControl, NotificationCenterGroup, NotificationCenterItem,
    NotificationCenterProjection,
};
use crate::delivery_policy::{
    DeliveryDecisionLog, DeliveryPolicy, DeliveryPolicyRule, LoadedDeliveryPolicyState,
};
use crate::domain::{
    AcceptedNotification, NotificationImageData, NotificationReceipt, NotificationSnapshot,
    NotificationStatus, NotificationTimeout, NotificationTimeoutDefaults, SenderEvent, Urgency,
    UserRule,
};
use crate::lifecycle::{
    LifecycleDelivery, LifecycleEffect, LifecycleEffects, NotificationLifecycle,
    NotificationTimeoutChange,
};
use crate::presentation::{
    BannerPresentation, BannerPresenter, BannerView, NotificationBannerState,
    PresentationCoordinator,
};
use crate::sqlite_persisted_state::SqlitePersistedState;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, watch};

#[derive(Clone)]
pub(crate) struct NotificationRuntime {
    storage: SqlitePersistedState,
    lifecycle: NotificationLifecycle<SqlitePersistedState>,
    presentation: PresentationCoordinator,
    banner_state: Arc<Mutex<NotificationBannerState>>,
    runtime_images: Arc<Mutex<RuntimeImageStore>>,
    sender_events: Arc<Mutex<Vec<SenderEvent>>>,
    sender_event_changes: broadcast::Sender<SenderEvent>,
    decision_logs: Arc<Mutex<Vec<DeliveryDecisionLog>>>,
    timeout_tasks: Arc<Mutex<HashMap<i64, TimeoutTask>>>,
    config_rules: Arc<Mutex<Vec<DeliveryPolicyRule>>>,
    state_version: Arc<AtomicU64>,
    state_changes: watch::Sender<u64>,
}

#[derive(Clone, Debug)]
struct RuntimeImageEntry {
    revision: i64,
    image: NotificationImageData,
}

#[derive(Clone, Debug, Default)]
struct RuntimeImageStore {
    images: HashMap<i64, RuntimeImageEntry>,
}

impl RuntimeImageStore {
    fn replace(
        &mut self,
        notification_id: i64,
        revision: i64,
        image: Option<NotificationImageData>,
    ) {
        match image {
            Some(image) => {
                self.images
                    .insert(notification_id, RuntimeImageEntry { revision, image });
            }
            None => {
                self.images.remove(&notification_id);
            }
        }
    }

    fn remove(&mut self, notification_id: i64) {
        self.images.remove(&notification_id);
    }

    fn image_for(&self, notification_id: i64, revision: i64) -> Option<NotificationImageData> {
        self.images
            .get(&notification_id)
            .filter(|entry| entry.revision == revision)
            .map(|entry| entry.image.clone())
    }
}

struct TimeoutTask {
    revision: i64,
    handle: tokio::task::JoinHandle<()>,
}

impl NotificationRuntime {
    pub(crate) async fn new(
        storage: SqlitePersistedState,
        headless: bool,
        timeout_defaults: NotificationTimeoutDefaults,
        banner_presenter: Arc<dyn BannerPresenter>,
    ) -> anyhow::Result<Self> {
        let lifecycle =
            NotificationLifecycle::with_timeout_defaults(storage.clone(), timeout_defaults);
        lifecycle.mark_active_stale().await?;
        storage.prune_history().await?;
        let (state_changes, _state_change_receiver) = watch::channel(0);
        let (sender_event_changes, _sender_event_receiver) = broadcast::channel(128);

        Ok(Self {
            storage,
            lifecycle,
            presentation: PresentationCoordinator::new(!headless, banner_presenter),
            banner_state: Arc::new(Mutex::new(NotificationBannerState::new())),
            runtime_images: Arc::new(Mutex::new(RuntimeImageStore::default())),
            sender_events: Arc::new(Mutex::new(Vec::new())),
            sender_event_changes,
            decision_logs: Arc::new(Mutex::new(Vec::new())),
            timeout_tasks: Arc::new(Mutex::new(HashMap::new())),
            config_rules: Arc::new(Mutex::new(Vec::new())),
            state_version: Arc::new(AtomicU64::new(0)),
            state_changes,
        })
    }

    pub(crate) async fn receive_notification(
        &self,
        notification: AcceptedNotification,
        image_data: Option<NotificationImageData>,
        expire_timeout: NotificationTimeout,
    ) -> anyhow::Result<NotificationReceipt> {
        let policy = self.delivery_policy().await?;
        let decision = policy.decide(&notification);
        self.record_decision(decision.log.clone());

        let effects = self
            .lifecycle
            .receive(
                notification,
                LifecycleDelivery::from_decision(&decision),
                expire_timeout,
            )
            .await?;
        let receipt = effects.sender_receipt()?;
        self.apply_request_images(&effects, image_data);
        self.apply_lifecycle_effects_and_notify(&effects).await?;
        Ok(receipt)
    }

    pub(crate) async fn snapshot(&self) -> anyhow::Result<NotificationSnapshot> {
        self.storage.snapshot().await
    }

    pub(crate) async fn visible_banners(&self) -> Vec<BannerView> {
        self.banner_presentation().banners
    }

    pub(crate) async fn invoke_action(
        &self,
        notification_id: i64,
        action_key: &str,
    ) -> anyhow::Result<()> {
        let effects = self
            .lifecycle
            .invoke_action(notification_id, action_key)
            .await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub(crate) async fn dismiss_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        let effects = self.lifecycle.dismiss(notification_id).await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub(crate) async fn expire_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        let effects = self.lifecycle.expire(notification_id).await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub(crate) async fn close_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        let effects = self.lifecycle.close(notification_id).await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub(crate) async fn activate_banner(&self, notification_id: i64) -> anyhow::Result<()> {
        let thread = self.storage.thread(notification_id).await?;
        if thread.status != NotificationStatus::Active {
            anyhow::bail!("notification {notification_id} is not active");
        }

        let default_action_key = thread
            .accepted_notification
            .actions
            .iter()
            .find(|action| action.key == "default")
            .map(|action| action.key.clone());

        if let Some(default_action_key) = default_action_key {
            self.invoke_action(notification_id, &default_action_key)
                .await
        } else {
            self.dismiss_notification(notification_id).await
        }
    }

    pub(crate) async fn clear_active_notifications(&self) -> anyhow::Result<()> {
        let effects = self.lifecycle.clear_active_notifications().await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub(crate) async fn delete_notification_history(&self) -> anyhow::Result<()> {
        self.storage.delete_inactive_history().await?;
        self.notify_state_changed();
        Ok(())
    }

    pub(crate) async fn set_do_not_disturb(&self, enabled: bool) -> anyhow::Result<()> {
        self.storage.set_do_not_disturb(enabled).await?;
        self.reevaluate_active_notifications().await?;
        self.notify_state_changed();
        Ok(())
    }

    pub(crate) async fn do_not_disturb(&self) -> anyhow::Result<bool> {
        self.storage.do_not_disturb().await
    }

    pub(crate) async fn upsert_user_rule(&self, rule: UserRule) -> anyhow::Result<()> {
        self.storage.upsert_user_rule(&rule).await?;
        self.reevaluate_active_notifications().await?;
        self.notify_state_changed();
        Ok(())
    }

    pub(crate) async fn user_rules(&self) -> anyhow::Result<Vec<UserRule>> {
        self.storage.user_rules().await
    }

    pub(crate) async fn remove_user_rule(
        &self,
        app_identity: &str,
        urgency: Option<Urgency>,
    ) -> anyhow::Result<()> {
        self.storage.remove_user_rule(app_identity, urgency).await?;
        self.reevaluate_active_notifications().await?;
        self.notify_state_changed();
        Ok(())
    }

    pub(crate) async fn install_config_rules(
        &self,
        rules: Vec<DeliveryPolicyRule>,
    ) -> anyhow::Result<()> {
        *self.config_rules.lock().unwrap() = rules;
        self.reevaluate_active_notifications().await?;
        self.notify_state_changed();
        Ok(())
    }

    pub(crate) async fn delivery_decision_logs(&self) -> Vec<DeliveryDecisionLog> {
        self.decision_logs.lock().unwrap().clone()
    }

    pub(crate) async fn sender_events(&self) -> Vec<SenderEvent> {
        self.sender_events.lock().unwrap().clone()
    }

    pub(crate) fn subscribe_sender_events(&self) -> broadcast::Receiver<SenderEvent> {
        self.sender_event_changes.subscribe()
    }

    pub(crate) async fn notification_center_projection(
        &self,
    ) -> anyhow::Result<NotificationCenterProjection> {
        let snapshot = self.snapshot().await?;
        let user_rules = self.user_rules().await?;
        Ok(projection_from_snapshot(snapshot, &user_rules))
    }

    pub(crate) fn subscribe_notification_state(&self) -> NotificationStateSubscription {
        NotificationStateSubscription {
            runtime: self.clone(),
            changes: self.state_changes.subscribe(),
            initial_pending: true,
        }
    }

    async fn delivery_policy(&self) -> anyhow::Result<DeliveryPolicy> {
        let config_rules = self.config_rules.lock().unwrap().clone();
        let user_rules = self.storage.user_rules().await?;
        let do_not_disturb = self.storage.do_not_disturb().await?;
        let state = LoadedDeliveryPolicyState::builder()
            .do_not_disturb(do_not_disturb)
            .user_rules(user_rules.into_iter().map(DeliveryPolicyRule::from))
            .config_rules(config_rules)
            .build();
        Ok(DeliveryPolicy::new(state))
    }

    fn record_decision(&self, log: DeliveryDecisionLog) {
        self.decision_logs.lock().unwrap().push(log);
    }

    async fn reevaluate_active_notifications(&self) -> anyhow::Result<()> {
        let active = self.snapshot().await?.active;
        let policy = self.delivery_policy().await?;
        let mut all_effects = LifecycleEffects::empty();
        for thread in active {
            let decision = policy.decide(&thread.accepted_notification);
            let effects = self
                .lifecycle
                .apply_delivery_decision(thread.id, LifecycleDelivery::from_decision(&decision))
                .await?;
            all_effects.extend(effects);
        }
        self.apply_lifecycle_effects_and_notify(&all_effects)
            .await?;
        Ok(())
    }

    async fn apply_lifecycle_effects_and_notify(
        &self,
        effects: &LifecycleEffects,
    ) -> anyhow::Result<()> {
        self.apply_lifecycle_effects(effects).await?;
        if !effects.is_empty() {
            self.notify_state_changed();
        }
        Ok(())
    }

    async fn apply_lifecycle_effects(&self, effects: &LifecycleEffects) -> anyhow::Result<()> {
        self.remove_runtime_images_for_terminal_effects(effects);
        self.apply_notification_timeout_changes(effects.timeout_changes());
        {
            let mut banner_state = self.banner_state.lock().unwrap();
            banner_state.apply_changes(effects.banner_changes());
        }
        self.publish_sender_events(effects.sender_events());
        self.sync_presentation().await?;
        Ok(())
    }

    fn publish_sender_events(&self, events: impl IntoIterator<Item = SenderEvent>) {
        let mut sender_events = self.sender_events.lock().unwrap();
        for event in events {
            sender_events.push(event.clone());
            let _ = self.sender_event_changes.send(event);
        }
    }

    async fn sync_presentation(&self) -> anyhow::Result<()> {
        let presentation = self.banner_presentation();
        self.presentation.replace_banner_presentation(presentation)
    }

    fn banner_presentation(&self) -> BannerPresentation {
        let (threads, overflow_count) = {
            let banner_state = self.banner_state.lock().unwrap();
            (
                banner_state.visible_threads(),
                banner_state.overflow_count(),
            )
        };
        let runtime_images = self.runtime_images.lock().unwrap();
        BannerPresentation {
            banners: threads
                .into_iter()
                .map(|thread| {
                    let image = runtime_images.image_for(thread.id, thread.revision);
                    BannerView::from_thread(thread, image)
                })
                .collect(),
            overflow_count,
        }
    }

    fn apply_request_images(
        &self,
        effects: &LifecycleEffects,
        image_data: Option<NotificationImageData>,
    ) {
        let mut runtime_images = self.runtime_images.lock().unwrap();
        for effect in effects {
            match effect {
                LifecycleEffect::ThreadCreated(thread) | LifecycleEffect::ThreadRevised(thread) => {
                    runtime_images.replace(thread.id, thread.revision, image_data.clone());
                }
                LifecycleEffect::NotificationDropped {
                    replaces_id: Some(notification_id),
                } => {
                    runtime_images.remove(*notification_id);
                }
                LifecycleEffect::SenderReceipt(_)
                | LifecycleEffect::ThreadDeliveryChanged(_)
                | LifecycleEffect::ThreadSilenced(_)
                | LifecycleEffect::ThreadDismissed(_)
                | LifecycleEffect::ThreadExpired(_)
                | LifecycleEffect::ThreadClosed(_)
                | LifecycleEffect::ThreadStale(_)
                | LifecycleEffect::NotificationDropped { replaces_id: None }
                | LifecycleEffect::NotificationTimeoutScheduled { .. }
                | LifecycleEffect::NotificationTimeoutCancelled { .. }
                | LifecycleEffect::ActionInvoked { .. }
                | LifecycleEffect::SenderClosed { .. } => {}
            }
        }
    }

    fn remove_runtime_images_for_terminal_effects(&self, effects: &LifecycleEffects) {
        let mut runtime_images = self.runtime_images.lock().unwrap();
        for effect in effects {
            match effect {
                LifecycleEffect::ThreadSilenced(thread)
                | LifecycleEffect::ThreadDismissed(thread)
                | LifecycleEffect::ThreadExpired(thread)
                | LifecycleEffect::ThreadClosed(thread)
                | LifecycleEffect::ThreadStale(thread) => {
                    runtime_images.remove(thread.id);
                }
                LifecycleEffect::NotificationDropped {
                    replaces_id: Some(notification_id),
                } => {
                    runtime_images.remove(*notification_id);
                }
                LifecycleEffect::SenderReceipt(_)
                | LifecycleEffect::ThreadCreated(_)
                | LifecycleEffect::ThreadRevised(_)
                | LifecycleEffect::ThreadDeliveryChanged(_)
                | LifecycleEffect::NotificationDropped { replaces_id: None }
                | LifecycleEffect::NotificationTimeoutScheduled { .. }
                | LifecycleEffect::NotificationTimeoutCancelled { .. }
                | LifecycleEffect::ActionInvoked { .. }
                | LifecycleEffect::SenderClosed { .. } => {}
            }
        }
    }

    fn notify_state_changed(&self) {
        let version = self.state_version.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.state_changes.send(version);
    }

    fn apply_notification_timeout_changes(
        &self,
        changes: impl IntoIterator<Item = NotificationTimeoutChange>,
    ) {
        for change in changes {
            match change {
                NotificationTimeoutChange::Scheduled {
                    notification_id,
                    revision,
                    duration,
                } => self.schedule_notification_timeout(notification_id, revision, duration),
                NotificationTimeoutChange::Cancelled { notification_id } => {
                    self.cancel_notification_timeout(notification_id)
                }
            }
        }
    }

    fn schedule_notification_timeout(
        &self,
        notification_id: i64,
        revision: i64,
        duration: std::time::Duration,
    ) {
        self.cancel_notification_timeout(notification_id);

        let runtime = self.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            runtime.forget_notification_timeout_if_current_revision(notification_id, revision);
            let _ = runtime
                .expire_notification_if_current_revision(notification_id, revision)
                .await;
        });

        self.timeout_tasks
            .lock()
            .unwrap()
            .insert(notification_id, TimeoutTask { revision, handle });
    }

    fn cancel_notification_timeout(&self, notification_id: i64) {
        if let Some(task) = self.timeout_tasks.lock().unwrap().remove(&notification_id) {
            task.handle.abort();
        }
    }

    fn forget_notification_timeout_if_current_revision(&self, notification_id: i64, revision: i64) {
        let mut timeout_tasks = self.timeout_tasks.lock().unwrap();
        if timeout_tasks
            .get(&notification_id)
            .map(|task| task.revision == revision)
            .unwrap_or(false)
        {
            timeout_tasks.remove(&notification_id);
        }
    }

    async fn expire_notification_if_current_revision(
        &self,
        notification_id: i64,
        revision: i64,
    ) -> anyhow::Result<()> {
        let effects = self
            .lifecycle
            .expire_timeout_if_current_revision(notification_id, revision)
            .await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }
}

pub struct NotificationStateSubscription {
    runtime: NotificationRuntime,
    changes: watch::Receiver<u64>,
    initial_pending: bool,
}

impl NotificationStateSubscription {
    pub async fn next_projection(
        &mut self,
    ) -> anyhow::Result<Option<NotificationCenterProjection>> {
        if self.initial_pending {
            self.initial_pending = false;
            return Ok(Some(self.runtime.notification_center_projection().await?));
        }

        match self.changes.changed().await {
            Ok(()) => Ok(Some(self.runtime.notification_center_projection().await?)),
            Err(_) => Ok(None),
        }
    }
}

fn projection_from_snapshot(
    snapshot: NotificationSnapshot,
    user_rules: &[UserRule],
) -> NotificationCenterProjection {
    let mut grouped: BTreeMap<String, Vec<NotificationCenterItem>> = BTreeMap::new();
    let app_permissions: HashMap<_, _> = user_rules
        .iter()
        .filter(|rule| rule.urgency.is_none())
        .map(|rule| {
            (
                rule.app_identity.as_str(),
                AppPermissionControl::from_delivery_outcome(rule.outcome),
            )
        })
        .collect();

    for thread in snapshot.history {
        let notification = thread.accepted_notification;
        grouped
            .entry(notification.app_identity.clone())
            .or_default()
            .push(NotificationCenterItem {
                notification_id: thread.id,
                summary: notification.summary,
                body: notification.body,
                status: thread.status,
                delivery_outcome: thread.delivery_outcome,
                actions: notification.actions,
            });
    }

    NotificationCenterProjection {
        groups: grouped
            .into_iter()
            .map(|(app_identity, items)| NotificationCenterGroup {
                permission: app_permissions
                    .get(app_identity.as_str())
                    .copied()
                    .unwrap_or(AppPermissionControl::Allow),
                app_identity,
                items,
            })
            .collect(),
    }
}
