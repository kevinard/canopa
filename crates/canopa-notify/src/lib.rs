use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod activation;
mod clock;
mod configuration;
pub mod control;
mod delivery_policy;
mod domain;
mod input_limits;
mod intake;
mod lifecycle;
mod local_file;
mod notification_id;
mod notification_server_intake;
mod presentation;
mod runtime;
mod runtime_resources;
mod runtime_timeouts;
mod sender_events;
mod sound;
mod sqlite_persisted_state;

pub use activation::{
    ActivationTokenFuture, ActivationTokenProvider, NoopActivationTokenProvider,
    WaylandActivationTokenProvider,
};
pub use configuration::ConfigurationStatus;
pub use control::{
    AppPermissionControl, CategoryPermissionControl, ControlCommand, ControlReply, ControlStatus,
    NotificationCenterGroup, NotificationCenterItem, NotificationCenterProjection, NotifyControl,
};
pub use delivery_policy::{
    DeliveryDecision, DeliveryDecisionLog, DeliveryDecisionReason, DeliveryPolicy,
    DeliveryPolicyRule, LoadedDeliveryPolicyState,
};
pub use domain::{
    AcceptedNotification, DEFAULT_NOTIFICATION_SOUND_NAME, DEFAULT_NOTIFICATION_SOUND_THEME,
    DeliveryOutcome, NotificationAction, NotificationBodyContent, NotificationBodyImage,
    NotificationBodyImageSource, NotificationBodyRun, NotificationBodySegment,
    NotificationBodyStyle, NotificationImageData, NotificationInput, NotificationPosition,
    NotificationReceipt, NotificationSnapshot, NotificationSound, NotificationSoundPlayback,
    NotificationSoundRequest, NotificationSoundSettings, NotificationStatus, NotificationThread,
    NotificationTimeout, NotificationTimeoutDefaults, NotificationVisualReference,
    SenderCloseReason, SenderEvent, Urgency, UserRule, normalize_notification_category,
};
pub use input_limits::NotificationLimitExceeded;
pub use lifecycle::{
    BannerLifecycleChange, LifecycleBatch, LifecycleDelivery, LifecycleEffect, LifecycleEffects,
    NewNotificationThread, NotificationHistory, NotificationLifecycle, NotificationTimeoutChange,
    PartialClearError, PersistPrecondition,
};
pub use notification_id::InternalNotificationId;
pub use notification_server_intake::NotificationServerRequest;
pub use presentation::{
    BannerAction, BannerBodySegment, BannerFocusBehavior, BannerIconSettings, BannerInteraction,
    BannerKeyboardInteractivity, BannerLayer, BannerOutputSelection, BannerPlacement,
    BannerPresentation, BannerPresentationAcknowledgement, BannerPresenter, BannerSurfaceSpec,
    BannerTarget, BannerView, BannerVisual, IcedLayerShellBannerPresenter, NoopBannerPresenter,
    NotificationBannerState, StructuredLogBannerPresenter,
};
pub use runtime::NotificationStateSubscription;
pub use sender_events::{SenderEventPublication, SenderEventPublisher};
pub use sound::{NoopSoundPresenter, RodioSoundPresenter, SoundPresenter};

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
    activation_token_provider: Arc<dyn ActivationTokenProvider>,
    sound_presenter: Arc<dyn SoundPresenter>,
    configuration_path: Option<PathBuf>,
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
            activation_token_provider: Arc::new(NoopActivationTokenProvider),
            sound_presenter: Arc::new(NoopSoundPresenter),
            configuration_path: None,
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
            activation_token_provider: Arc::new(NoopActivationTokenProvider),
            sound_presenter: Arc::new(NoopSoundPresenter),
            configuration_path: None,
        }
    }

    pub fn with_history_limit(mut self, history_limit: i64) -> Self {
        self.history_limit = history_limit;
        self
    }

    pub fn with_configuration_file(mut self, path: PathBuf) -> Self {
        self.configuration_path = Some(path);
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

    pub fn with_activation_token_provider(
        mut self,
        activation_token_provider: impl ActivationTokenProvider + 'static,
    ) -> Self {
        self.activation_token_provider = Arc::new(activation_token_provider);
        self
    }

    pub fn with_sound_presenter(mut self, sound_presenter: impl SoundPresenter + 'static) -> Self {
        self.sound_presenter = Arc::new(sound_presenter);
        self
    }
}

#[derive(Clone)]
pub struct Daemon {
    runtime: NotificationRuntime,
    configuration: Arc<Mutex<LoadedNotificationConfiguration>>,
    configuration_reloads: Arc<tokio::sync::Mutex<()>>,
}

impl Daemon {
    pub async fn open(options: DaemonOptions) -> anyhow::Result<Self> {
        let mut configuration = LoadedNotificationConfiguration::empty();
        if let Some(path) = &options.configuration_path {
            configuration.reload_file(path)?;
            if !options.headless {
                options
                    .sound_presenter
                    .apply_sound_settings(configuration.sound_settings())?;
            }
        }
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
            options.activation_token_provider,
            options.sound_presenter,
            configuration.delivery_policy_rules(),
        )
        .await?;
        runtime.set_icon_settings(configuration.icon_settings());
        Ok(Self {
            runtime,
            configuration: Arc::new(Mutex::new(configuration)),
            configuration_reloads: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub async fn receive_notification(
        &self,
        input: NotificationInput,
    ) -> anyhow::Result<NotificationReceipt> {
        self.receive_notification_with_receipt_hook(input, |_| {})
            .await
    }

    pub async fn receive_notification_with_receipt_hook(
        &self,
        input: NotificationInput,
        on_receipt: impl FnOnce(&NotificationReceipt),
    ) -> anyhow::Result<NotificationReceipt> {
        self.receive_notification_with_preparation(|| Ok(input), on_receipt)
            .await
    }

    /// Resolves transport-local identity while holding the receipt mutation boundary.
    /// Hooks must be synchronous and nonblocking; receipt registration precedes effects.
    pub async fn receive_notification_with_preparation(
        &self,
        prepare: impl FnOnce() -> anyhow::Result<NotificationInput>,
        on_receipt: impl FnOnce(&NotificationReceipt),
    ) -> anyhow::Result<NotificationReceipt> {
        let command = self.runtime.begin_receipt().await?;
        let input = prepare()?;
        input_limits::validate_input(&input)?;
        let expire_timeout = input.expire_timeout;
        let accepted = match intake_notification(input) {
            NotificationIntakeResult::Accepted(accepted) => *accepted,
            NotificationIntakeResult::NoOpDropped {
                app_identity,
                replaces_id,
            } => {
                return self
                    .runtime
                    .drop_notification(&command, app_identity, replaces_id, on_receipt)
                    .await;
            }
        };

        self.runtime
            .receive_notification_with_receipt_hook(
                &command,
                accepted.notification,
                accepted.image_data,
                expire_timeout,
                accepted.sound,
                on_receipt,
            )
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
        notification_id: InternalNotificationId,
        action_key: &str,
    ) -> anyhow::Result<()> {
        self.runtime
            .invoke_action(notification_id, action_key)
            .await
    }

    pub async fn invoke_action_with_activation_token(
        &self,
        notification_id: InternalNotificationId,
        action_key: &str,
        activation_token: Option<String>,
    ) -> anyhow::Result<()> {
        self.runtime
            .invoke_action_with_activation_token(notification_id, action_key, activation_token)
            .await
    }

    pub async fn dismiss_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        self.runtime.dismiss_notification(notification_id).await
    }

    pub async fn expire_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        self.runtime.expire_notification(notification_id).await
    }

    pub async fn close_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        self.runtime.close_notification(notification_id).await
    }

    /// Activate current content immediately. Queued UI events must use `handle_banner_interaction`.
    pub async fn activate_banner(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        self.runtime.activate_banner(notification_id).await
    }

    /// Apply a queued banner interaction only to the content revision it originated from.
    pub async fn handle_banner_interaction(
        &self,
        interaction: BannerInteraction,
    ) -> anyhow::Result<()> {
        self.runtime.handle_banner_interaction(interaction).await
    }

    /// Current-state convenience; a captured UI callback must retain its `BannerTarget` instead.
    pub async fn activate_banner_with_activation_token(
        &self,
        notification_id: InternalNotificationId,
        activation_token: Option<String>,
    ) -> anyhow::Result<()> {
        self.runtime
            .activate_banner_with_activation_token(notification_id, activation_token)
            .await
    }

    pub async fn activate_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        self.runtime.activate_notification(notification_id).await
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

    pub async fn change_do_not_disturb(&self, enabled: Option<bool>) -> anyhow::Result<bool> {
        self.runtime.change_do_not_disturb(enabled).await
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

    pub async fn remove_category_user_rule(
        &self,
        app_identity: &str,
        category: &str,
        urgency: Option<Urgency>,
    ) -> anyhow::Result<()> {
        let category = normalize_notification_category(category)
            .ok_or_else(|| anyhow::anyhow!("invalid Notification Category"))?;
        self.runtime
            .remove_category_user_rule(app_identity, &category, urgency)
            .await
    }

    pub async fn reload_configuration_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let _reload = self.configuration_reloads.lock().await;
        let mut candidate = self.configuration.lock().unwrap().clone();
        if let Err(error) = candidate.reload_file(path) {
            self.configuration.lock().unwrap().record_error(&error);
            return Err(error);
        }
        if let Err(error) = self
            .runtime
            .install_configuration(
                candidate.delivery_policy_rules(),
                candidate.sound_settings(),
                candidate.icon_settings(),
            )
            .await
        {
            self.configuration.lock().unwrap().record_error(&error);
            return Err(error);
        }
        *self.configuration.lock().unwrap() = candidate;
        Ok(())
    }

    pub fn configuration_status(&self) -> ConfigurationStatus {
        self.configuration.lock().unwrap().status()
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

    pub fn subscribe_sender_events(&self) -> tokio::sync::broadcast::Receiver<SenderEvent> {
        self.runtime.subscribe_sender_events()
    }

    /// Installs the single authoritative transport, before exposing its request interface.
    pub fn install_sender_event_publisher(
        &self,
        publisher: impl SenderEventPublisher + 'static,
    ) -> anyhow::Result<()> {
        self.runtime
            .install_sender_event_publisher(Arc::new(publisher))
    }

    pub fn sender_publication_error(&self) -> Option<String> {
        self.runtime.sender_publication_error()
    }

    pub fn supports_activation_tokens(&self) -> bool {
        self.runtime.supports_activation_tokens()
    }

    pub fn supports_sound(&self) -> bool {
        self.runtime.supports_sound()
    }

    pub fn presentation_error(&self) -> Option<String> {
        self.runtime.presentation_error()
    }

    pub(crate) fn timeout_record_count(&self) -> usize {
        self.runtime.timeout_record_count()
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
