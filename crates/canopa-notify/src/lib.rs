use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod clock;
mod configuration;
pub mod control;
mod delivery_policy;
mod domain;
mod intake;
mod lifecycle;
mod notification_server_intake;
mod presentation;
mod runtime;
mod sqlite_persisted_state;

pub use control::{
    AppPermissionControl, ControlCommand, ControlReply, ControlStatus, NotificationCenterGroup,
    NotificationCenterItem, NotificationCenterProjection, NotifyControl,
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
    BannerLifecycleChange, LifecycleDelivery, LifecycleEffect, LifecycleEffects,
    NewNotificationThread, NotificationHistory, NotificationLifecycle, NotificationTimeoutChange,
    PersistPrecondition,
};
pub use notification_server_intake::NotificationServerRequest;
pub use presentation::{
    BannerFocusBehavior, BannerInteraction, BannerKeyboardInteractivity, BannerLayer,
    BannerOutputSelection, BannerPlacement, BannerPresenter, BannerSurfaceSpec, BannerView,
    IcedLayerShellBannerPresenter, NoopBannerPresenter, NotificationBannerState,
    StructuredLogBannerPresenter,
};
pub use runtime::NotificationStateSubscription;

use configuration::LoadedNotificationConfiguration;
use intake::{NotificationIntakeResult, intake_notification};
use runtime::NotificationRuntime;
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
    runtime: NotificationRuntime,
    configuration: Arc<Mutex<LoadedNotificationConfiguration>>,
}

impl Daemon {
    pub async fn open(options: DaemonOptions) -> anyhow::Result<Self> {
        let storage = SqlitePersistedState::open(
            options.database_path,
            options.retention_days,
            options.history_limit,
        )
        .await?;
        let runtime = NotificationRuntime::new(
            storage,
            options.headless,
            options.notification_timeout_defaults,
            options.banner_presenter,
        )
        .await?;
        Ok(Self {
            runtime,
            configuration: Arc::new(Mutex::new(LoadedNotificationConfiguration::empty())),
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

        self.runtime
            .receive_notification(notification, expire_timeout)
            .await
    }

    pub async fn snapshot(&self) -> anyhow::Result<NotificationSnapshot> {
        self.runtime.snapshot().await
    }

    pub async fn visible_banners(&self) -> Vec<BannerView> {
        self.runtime.visible_banners().await
    }

    pub async fn invoke_action(
        &self,
        notification_id: i64,
        action_key: &str,
    ) -> anyhow::Result<()> {
        self.runtime
            .invoke_action(notification_id, action_key)
            .await
    }

    pub async fn dismiss_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        self.runtime.dismiss_notification(notification_id).await
    }

    pub async fn expire_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        self.runtime.expire_notification(notification_id).await
    }

    pub async fn close_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        self.runtime.close_notification(notification_id).await
    }

    pub async fn activate_banner(&self, notification_id: i64) -> anyhow::Result<()> {
        self.runtime.activate_banner(notification_id).await
    }

    pub async fn clear_active_notifications(&self) -> anyhow::Result<()> {
        self.runtime.clear_active_notifications().await
    }

    pub async fn delete_notification_history(&self) -> anyhow::Result<()> {
        self.runtime.delete_notification_history().await
    }

    pub async fn set_do_not_disturb(&self, enabled: bool) -> anyhow::Result<()> {
        self.runtime.set_do_not_disturb(enabled).await
    }

    pub async fn do_not_disturb(&self) -> anyhow::Result<bool> {
        self.runtime.do_not_disturb().await
    }

    pub async fn upsert_user_rule(&self, rule: UserRule) -> anyhow::Result<()> {
        self.runtime.upsert_user_rule(rule).await
    }

    pub async fn user_rules(&self) -> anyhow::Result<Vec<UserRule>> {
        self.runtime.user_rules().await
    }

    pub async fn remove_user_rule(
        &self,
        app_identity: &str,
        urgency: Option<Urgency>,
    ) -> anyhow::Result<()> {
        self.runtime.remove_user_rule(app_identity, urgency).await
    }

    pub async fn reload_configuration_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let rules = {
            let mut configuration = self.configuration.lock().unwrap();
            configuration.reload_file(path)?;
            configuration.delivery_policy_rules()
        };
        self.runtime.install_config_rules(rules).await
    }

    pub async fn configuration_error(&self) -> Option<String> {
        self.configuration.lock().unwrap().error()
    }

    pub async fn delivery_decision_logs(&self) -> Vec<DeliveryDecisionLog> {
        self.runtime.delivery_decision_logs().await
    }

    pub async fn sender_events(&self) -> Vec<SenderEvent> {
        self.runtime.sender_events().await
    }

    pub(crate) async fn notification_center_projection(
        &self,
    ) -> anyhow::Result<NotificationCenterProjection> {
        self.runtime.notification_center_projection().await
    }

    pub(crate) fn subscribe_notification_state(&self) -> NotificationStateSubscription {
        self.runtime.subscribe_notification_state()
    }
}
