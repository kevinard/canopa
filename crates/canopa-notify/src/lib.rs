use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

mod clock;
mod configuration;
pub mod control;
mod delivery_policy;
mod domain;
mod intake;
mod lifecycle;
mod presentation;
mod sqlite_persisted_state;

pub use control::{
    AppPermissionControl, ControlCommand, ControlReply, ControlStatus, NotificationCenterGroup,
    NotificationCenterItem, NotificationCenterProjection, NotificationStateSubscription,
    NotifyControl,
};
pub use delivery_policy::{
    DeliveryDecision, DeliveryDecisionLog, DeliveryDecisionReason, DeliveryPolicy,
    DeliveryPolicyRule, LoadedDeliveryPolicyState,
};
pub use domain::{
    AcceptedNotification, DeliveryOutcome, NotificationAction, NotificationInput,
    NotificationReceipt, NotificationSnapshot, NotificationStatus, NotificationThread,
    NotificationTimeout, NotificationTimeoutDefaults, SenderEvent, Urgency, UserRule,
};
pub use lifecycle::{
    LifecycleDelivery, LifecycleEffect, NewNotificationThread, NotificationHistory,
    NotificationLifecycle, PersistPrecondition,
};
pub use presentation::{
    BannerFocusBehavior, BannerInteraction, BannerKeyboardInteractivity, BannerLayer,
    BannerOutputSelection, BannerPlacement, BannerPresenter, BannerSurfaceSpec, BannerView,
    IcedLayerShellBannerPresenter, NoopBannerPresenter, NotificationBannerState,
    StructuredLogBannerPresenter,
};

use configuration::LoadedNotificationConfiguration;
use intake::{NotificationIntakeResult, intake_notification};
use lifecycle::receipt_from_effects;
use presentation::PresentationCoordinator;
use sqlite_persisted_state::SqlitePersistedState;

#[derive(Clone)]
pub struct DaemonOptions {
    pub database_path: PathBuf,
    pub headless: bool,
    pub retention_days: i64,
    pub history_limit: i64,
    pub notification_timeout_defaults: NotificationTimeoutDefaults,
    banner_presenter: Arc<dyn BannerPresenter>,
}

impl DaemonOptions {
    pub fn headless(database_path: PathBuf) -> Self {
        Self {
            database_path,
            headless: true,
            retention_days: 30,
            history_limit: 1_000,
            notification_timeout_defaults: NotificationTimeoutDefaults::default(),
            banner_presenter: Arc::new(NoopBannerPresenter),
        }
    }

    pub fn graphical(
        database_path: PathBuf,
        banner_presenter: impl BannerPresenter + 'static,
    ) -> Self {
        Self {
            database_path,
            headless: false,
            retention_days: 30,
            history_limit: 1_000,
            notification_timeout_defaults: NotificationTimeoutDefaults::default(),
            banner_presenter: Arc::new(banner_presenter),
        }
    }

    pub fn with_history_limit(mut self, history_limit: i64) -> Self {
        self.history_limit = history_limit;
        self
    }

    pub fn with_notification_timeout_defaults(
        mut self,
        notification_timeout_defaults: NotificationTimeoutDefaults,
    ) -> Self {
        self.notification_timeout_defaults = notification_timeout_defaults;
        self
    }

    pub fn with_banner_presenter(
        mut self,
        banner_presenter: impl BannerPresenter + 'static,
    ) -> Self {
        self.banner_presenter = Arc::new(banner_presenter);
        self
    }
}

#[derive(Clone)]
pub struct Daemon {
    storage: SqlitePersistedState,
    lifecycle: NotificationLifecycle<SqlitePersistedState>,
    presentation: PresentationCoordinator,
    banner_state: Arc<Mutex<NotificationBannerState>>,
    sender_events: Arc<Mutex<Vec<SenderEvent>>>,
    configuration: Arc<Mutex<LoadedNotificationConfiguration>>,
    decision_logs: Arc<Mutex<Vec<DeliveryDecisionLog>>>,
    timeout_tasks: Arc<Mutex<HashMap<i64, TimeoutTask>>>,
    state_version: Arc<AtomicU64>,
    state_changes: watch::Sender<u64>,
}

struct TimeoutTask {
    revision: i64,
    handle: tokio::task::JoinHandle<()>,
}

impl Daemon {
    pub async fn open(options: DaemonOptions) -> anyhow::Result<Self> {
        let storage = SqlitePersistedState::open(
            options.database_path,
            options.retention_days,
            options.history_limit,
        )
        .await?;
        let lifecycle = NotificationLifecycle::with_timeout_defaults(
            storage.clone(),
            options.notification_timeout_defaults,
        );
        lifecycle.mark_active_stale().await?;
        storage.prune_history().await?;
        let (state_changes, _state_change_receiver) = watch::channel(0);
        Ok(Self {
            storage,
            lifecycle,
            presentation: PresentationCoordinator::new(!options.headless, options.banner_presenter),
            banner_state: Arc::new(Mutex::new(NotificationBannerState::new())),
            sender_events: Arc::new(Mutex::new(Vec::new())),
            configuration: Arc::new(Mutex::new(LoadedNotificationConfiguration::empty())),
            decision_logs: Arc::new(Mutex::new(Vec::new())),
            timeout_tasks: Arc::new(Mutex::new(HashMap::new())),
            state_version: Arc::new(AtomicU64::new(0)),
            state_changes,
        })
    }

    pub async fn receive_notification(
        &self,
        input: NotificationInput,
    ) -> anyhow::Result<NotificationReceipt> {
        let expire_timeout = input.expire_timeout;
        let notification = match intake_notification(input) {
            NotificationIntakeResult::Accepted(notification) => notification,
            NotificationIntakeResult::NoOpDropped(receipt) => return Ok(receipt),
        };

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
        let receipt = receipt_from_effects(&effects)?;
        self.apply_lifecycle_effects_and_notify(&effects).await?;
        Ok(receipt)
    }

    pub async fn snapshot(&self) -> anyhow::Result<NotificationSnapshot> {
        self.storage.snapshot().await
    }

    pub async fn visible_banners(&self) -> Vec<BannerView> {
        self.banner_state.lock().unwrap().visible_banners()
    }

    pub async fn invoke_action(
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

    pub async fn dismiss_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        let effects = self.lifecycle.dismiss(notification_id).await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub async fn expire_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        let effects = self.lifecycle.expire(notification_id).await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub async fn close_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        let effects = self.lifecycle.close(notification_id).await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub async fn activate_banner(&self, notification_id: i64) -> anyhow::Result<()> {
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

    pub async fn clear_active_notifications(&self) -> anyhow::Result<()> {
        let effects = self.lifecycle.clear_active_notifications().await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub async fn delete_notification_history(&self) -> anyhow::Result<()> {
        self.storage.delete_inactive_history().await?;
        self.notify_state_changed();
        Ok(())
    }

    pub async fn set_do_not_disturb(&self, enabled: bool) -> anyhow::Result<()> {
        self.storage.set_do_not_disturb(enabled).await?;
        self.reevaluate_active_notifications().await?;
        self.notify_state_changed();
        Ok(())
    }

    pub async fn do_not_disturb(&self) -> anyhow::Result<bool> {
        self.storage.do_not_disturb().await
    }

    pub async fn upsert_user_rule(&self, rule: UserRule) -> anyhow::Result<()> {
        self.storage.upsert_user_rule(&rule).await?;
        self.reevaluate_active_notifications().await?;
        self.notify_state_changed();
        Ok(())
    }

    pub async fn user_rules(&self) -> anyhow::Result<Vec<UserRule>> {
        self.storage.user_rules().await
    }

    pub async fn remove_user_rule(
        &self,
        app_identity: &str,
        urgency: Option<Urgency>,
    ) -> anyhow::Result<()> {
        self.storage.remove_user_rule(app_identity, urgency).await?;
        self.reevaluate_active_notifications().await?;
        self.notify_state_changed();
        Ok(())
    }

    pub async fn reload_configuration_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        {
            let mut configuration = self.configuration.lock().unwrap();
            configuration.reload_file(path)?;
        }
        self.reevaluate_active_notifications().await?;
        self.notify_state_changed();
        Ok(())
    }

    pub async fn configuration_error(&self) -> Option<String> {
        self.configuration.lock().unwrap().error()
    }

    pub async fn delivery_decision_logs(&self) -> Vec<DeliveryDecisionLog> {
        self.decision_logs.lock().unwrap().clone()
    }

    pub async fn sender_events(&self) -> Vec<SenderEvent> {
        self.sender_events.lock().unwrap().clone()
    }

    pub(crate) fn subscribe_state_changes(&self) -> watch::Receiver<u64> {
        self.state_changes.subscribe()
    }

    async fn delivery_policy(&self) -> anyhow::Result<DeliveryPolicy> {
        let config_rules = self.configuration.lock().unwrap().delivery_policy_rules();
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
        let mut all_effects = Vec::new();
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
        effects: &[LifecycleEffect],
    ) -> anyhow::Result<()> {
        self.apply_lifecycle_effects(effects).await?;
        if !effects.is_empty() {
            self.notify_state_changed();
        }
        Ok(())
    }

    async fn apply_lifecycle_effects(&self, effects: &[LifecycleEffect]) -> anyhow::Result<()> {
        self.apply_notification_timeout_effects(effects);
        {
            let mut banner_state = self.banner_state.lock().unwrap();
            banner_state.apply_effects(effects);
        }
        self.publish_sender_events(effects);
        self.sync_presentation().await?;
        Ok(())
    }

    fn publish_sender_events(&self, effects: &[LifecycleEffect]) {
        let mut sender_events = self.sender_events.lock().unwrap();
        for effect in effects {
            match effect {
                LifecycleEffect::ActionInvoked {
                    notification_id,
                    action_key,
                } => sender_events.push(SenderEvent {
                    notification_id: *notification_id,
                    action_key: Some(action_key.clone()),
                    closed_id: None,
                    close_reason: None,
                }),
                LifecycleEffect::SenderClosed {
                    notification_id,
                    reason,
                } => sender_events.push(SenderEvent {
                    notification_id: *notification_id,
                    action_key: None,
                    closed_id: Some(*notification_id),
                    close_reason: Some(reason.as_str().to_string()),
                }),
                _ => {}
            }
        }
    }

    async fn sync_presentation(&self) -> anyhow::Result<()> {
        let banners = self.banner_state.lock().unwrap().visible_banners();
        self.presentation.replace_visible_banners(banners)
    }

    fn notify_state_changed(&self) {
        let version = self.state_version.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.state_changes.send(version);
    }

    fn apply_notification_timeout_effects(&self, effects: &[LifecycleEffect]) {
        for effect in effects {
            match effect {
                LifecycleEffect::NotificationTimeoutScheduled {
                    notification_id,
                    revision,
                    duration,
                } => self.schedule_notification_timeout(*notification_id, *revision, *duration),
                LifecycleEffect::NotificationTimeoutCancelled { notification_id } => {
                    self.cancel_notification_timeout(*notification_id)
                }
                _ => {}
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

        let daemon = self.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            daemon.forget_notification_timeout_if_current_revision(notification_id, revision);
            let _ = daemon
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
        let thread = self.storage.thread(notification_id).await?;
        if thread.status != NotificationStatus::Active || thread.revision != revision {
            return Ok(());
        }

        self.expire_notification(notification_id).await
    }
}
