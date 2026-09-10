use crate::InternalNotificationId;
use crate::SenderEventPublisher;
use crate::activation::ActivationTokenProvider;
use crate::control::{
    AppPermissionControl, CategoryPermissionControl, NotificationCenterGroup,
    NotificationCenterItem, NotificationCenterProjection,
};
use crate::delivery_policy::{
    DeliveryDecisionLog, DeliveryPolicy, DeliveryPolicyRule, LoadedDeliveryPolicyState,
};
use crate::domain::{
    AcceptedNotification, DeliveryOutcome, NotificationImageData, NotificationReceipt,
    NotificationSnapshot, NotificationSoundPlayback, NotificationSoundRequest,
    NotificationSoundSettings, NotificationStatus, NotificationTimeout,
    NotificationTimeoutDefaults, SenderEvent, Urgency, UserRule,
};
use crate::lifecycle::{
    LifecycleDelivery, LifecycleEffect, LifecycleEffects, NotificationLifecycle,
    NotificationTimeoutChange,
};
use crate::presentation::{
    BannerInteraction, BannerPresentation, BannerPresentationAcknowledgement, BannerPresenter,
    BannerView, NotificationBannerState, PresentationCoordinator,
};
use crate::sound::SoundPresenter;
use crate::sqlite_persisted_state::SqlitePersistedState;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{Mutex as AsyncMutex, broadcast, watch};

const ACTIVATION_TOKEN_TIMEOUT: Duration = Duration::from_millis(100);
const DIAGNOSTIC_RECORD_LIMIT: usize = 256;

fn record_recent<T>(records: &Mutex<VecDeque<T>>, value: T) {
    let mut records = records.lock().unwrap();
    if records.len() == DIAGNOSTIC_RECORD_LIMIT {
        records.pop_front();
    }
    records.push_back(value);
}

#[derive(Clone)]
pub(crate) struct NotificationRuntime {
    storage: SqlitePersistedState,
    lifecycle: NotificationLifecycle<SqlitePersistedState>,
    presentation: PresentationCoordinator,
    banner_state: Arc<Mutex<NotificationBannerState>>,
    runtime_images: Arc<Mutex<RuntimeImageStore>>,
    activation_token_provider: Arc<dyn ActivationTokenProvider>,
    sound_presenter: Arc<dyn SoundPresenter>,
    headless: bool,
    activation_token_capable: bool,
    sender_events: Arc<Mutex<VecDeque<SenderEvent>>>,
    sender_event_changes: broadcast::Sender<SenderEvent>,
    sender_publisher: Arc<Mutex<Option<Arc<dyn SenderEventPublisher>>>>,
    sender_publication_error: Arc<Mutex<Option<String>>>,
    decision_logs: Arc<Mutex<VecDeque<DeliveryDecisionLog>>>,
    timeout_tasks: Arc<Mutex<HashMap<InternalNotificationId, TimeoutTask>>>,
    pending_timeout_starts: Arc<Mutex<PendingTimeoutStarts>>,
    loaded_policy_state: Arc<Mutex<LoadedDeliveryPolicyState>>,
    // Commit and effect order share one coordinator. External token acquisition is outside it.
    commands: Arc<AsyncMutex<()>>,
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
    images: HashMap<InternalNotificationId, RuntimeImageEntry>,
}

impl RuntimeImageStore {
    fn replace(
        &mut self,
        notification_id: InternalNotificationId,
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

    fn remove(&mut self, notification_id: InternalNotificationId) {
        self.images.remove(&notification_id);
    }

    fn image_for(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
    ) -> Option<NotificationImageData> {
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

#[derive(Default)]
struct PendingTimeoutStarts {
    starts: BTreeMap<(InternalNotificationId, i64), NotificationTimeoutChange>,
    cancelled_through: HashMap<InternalNotificationId, i64>,
}

impl PendingTimeoutStarts {
    fn allows(&self, notification_id: InternalNotificationId, revision: i64) -> bool {
        self.cancelled_through
            .get(&notification_id)
            .is_none_or(|cancelled_revision| revision > *cancelled_revision)
    }

    fn apply(&mut self, changes: impl IntoIterator<Item = NotificationTimeoutChange>) {
        for change in changes {
            match change {
                NotificationTimeoutChange::StartsOnDisplay {
                    notification_id,
                    revision,
                    ..
                } => {
                    if self.allows(notification_id, revision) {
                        self.starts.insert((notification_id, revision), change);
                    }
                }
                NotificationTimeoutChange::CancelThroughRevision {
                    notification_id,
                    revision,
                } => {
                    self.cancelled_through
                        .entry(notification_id)
                        .and_modify(|cancelled_revision| {
                            *cancelled_revision = (*cancelled_revision).max(revision);
                        })
                        .or_insert(revision);
                    self.starts.retain(|(pending_id, pending_revision), _| {
                        *pending_id != notification_id || *pending_revision > revision
                    });
                }
            }
        }
    }

    fn revisions_in(
        &self,
        visible_revisions: &BTreeSet<(InternalNotificationId, i64)>,
    ) -> Vec<(InternalNotificationId, i64)> {
        self.starts
            .keys()
            .filter(|revision| visible_revisions.contains(revision))
            .copied()
            .collect()
    }

    fn take(
        &mut self,
        revisions: impl IntoIterator<Item = (InternalNotificationId, i64)>,
    ) -> Vec<NotificationTimeoutChange> {
        revisions
            .into_iter()
            .filter_map(|revision| self.starts.remove(&revision))
            .collect()
    }
}

impl NotificationRuntime {
    pub(crate) async fn new(
        storage: SqlitePersistedState,
        headless: bool,
        timeout_defaults: NotificationTimeoutDefaults,
        banner_presenter: Arc<dyn BannerPresenter>,
        activation_token_provider: Arc<dyn ActivationTokenProvider>,
        sound_presenter: Arc<dyn SoundPresenter>,
        configuration_rules: Vec<DeliveryPolicyRule>,
    ) -> anyhow::Result<Self> {
        let lifecycle =
            NotificationLifecycle::with_timeout_defaults(storage.clone(), timeout_defaults);
        lifecycle.mark_active_stale().await?;
        storage.prune_history().await?;
        let loaded_policy_state = LoadedDeliveryPolicyState::builder()
            .config_rules(configuration_rules)
            .do_not_disturb(storage.do_not_disturb().await?)
            .user_rules(
                storage
                    .user_rules()
                    .await?
                    .into_iter()
                    .map(DeliveryPolicyRule::from),
            )
            .build();
        let (state_changes, _state_change_receiver) = watch::channel(0);
        let (sender_event_changes, _sender_event_receiver) = broadcast::channel(128);
        let activation_token_capable = activation_token_provider.available();

        Ok(Self {
            storage,
            lifecycle,
            presentation: PresentationCoordinator::new(!headless, banner_presenter),
            banner_state: Arc::new(Mutex::new(NotificationBannerState::new())),
            runtime_images: Arc::new(Mutex::new(RuntimeImageStore::default())),
            activation_token_provider,
            sound_presenter,
            headless,
            activation_token_capable,
            sender_events: Arc::new(Mutex::new(VecDeque::new())),
            sender_event_changes,
            sender_publisher: Arc::new(Mutex::new(None)),
            sender_publication_error: Arc::new(Mutex::new(None)),
            decision_logs: Arc::new(Mutex::new(VecDeque::new())),
            timeout_tasks: Arc::new(Mutex::new(HashMap::new())),
            pending_timeout_starts: Arc::new(Mutex::new(PendingTimeoutStarts::default())),
            loaded_policy_state: Arc::new(Mutex::new(loaded_policy_state)),
            commands: Arc::new(AsyncMutex::new(())),
            state_version: Arc::new(AtomicU64::new(0)),
            state_changes,
        })
    }

    pub(crate) async fn begin_receipt(&self) -> anyhow::Result<tokio::sync::OwnedMutexGuard<()>> {
        let command = self.commands.clone().lock_owned().await;
        anyhow::ensure!(
            self.sender_publication_error().is_none(),
            "notification transport is unavailable"
        );
        Ok(command)
    }

    pub(crate) async fn receive_notification_with_receipt_hook(
        &self,
        _command: &tokio::sync::OwnedMutexGuard<()>,
        notification: AcceptedNotification,
        image_data: Option<NotificationImageData>,
        expire_timeout: NotificationTimeout,
        sound_request: NotificationSoundRequest,
        on_receipt: impl FnOnce(&NotificationReceipt),
    ) -> anyhow::Result<NotificationReceipt> {
        let policy = self.delivery_policy();
        let decision = policy.decide(&notification);
        self.record_decision(decision.log.clone());
        let sound_app_identity = notification.app_identity.clone();
        let sound_summary = notification.summary.clone();

        let effects = self
            .lifecycle
            .receive(
                notification,
                LifecycleDelivery::from_decision(&decision),
                expire_timeout,
            )
            .await?;
        let receipt = effects.sender_receipt()?;
        on_receipt(&receipt);
        self.apply_request_images(&effects, image_data);
        let sound = (decision.outcome == DeliveryOutcome::Shown && !sound_request.suppress)
            .then_some(NotificationSoundPlayback {
                notification_id: receipt.notification_id,
                app_identity: sound_app_identity,
                summary: sound_summary,
                requested_sound: sound_request.sound,
            });
        self.apply_lifecycle_effects_and_notify_with_sound(&effects, sound)
            .await?;
        Ok(receipt)
    }

    pub(crate) async fn drop_notification(
        &self,
        _command: &tokio::sync::OwnedMutexGuard<()>,
        app_identity: String,
        replaces_id: Option<InternalNotificationId>,
        on_receipt: impl FnOnce(&NotificationReceipt),
    ) -> anyhow::Result<NotificationReceipt> {
        let effects = self
            .lifecycle
            .drop_notification(&app_identity, replaces_id)
            .await?;
        let receipt = effects.sender_receipt()?;
        on_receipt(&receipt);
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
        notification_id: InternalNotificationId,
        action_key: &str,
    ) -> anyhow::Result<()> {
        self.invoke_action_with_activation_token(notification_id, action_key, None)
            .await
    }

    pub(crate) async fn invoke_action_with_activation_token(
        &self,
        notification_id: InternalNotificationId,
        action_key: &str,
        activation_token: Option<String>,
    ) -> anyhow::Result<()> {
        self.invoke_action_for_revision(notification_id, action_key, activation_token, None)
            .await
    }

    async fn invoke_action_for_revision(
        &self,
        notification_id: InternalNotificationId,
        action_key: &str,
        activation_token: Option<String>,
        expected_revision: Option<i64>,
    ) -> anyhow::Result<()> {
        let thread = self
            .lifecycle
            .prepare_action(notification_id, action_key)
            .await?;
        anyhow::ensure!(
            expected_revision.is_none_or(|revision| revision == thread.revision),
            "notification was replaced before action invocation"
        );
        let activation_token = match activation_token.filter(|token| !token.is_empty()) {
            Some(token) => Some(token),
            None => {
                self.activation_token(thread.accepted_notification.app_identity.clone())
                    .await
            }
        };
        let _command = self.commands.lock().await;
        let effects = self
            .lifecycle
            .invoke_action_if_current_revision(notification_id, action_key, Some(thread.revision))
            .await?;
        self.apply_lifecycle_effects_and_notify_with_activation_token(&effects, activation_token)
            .await
    }

    pub(crate) async fn dismiss_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        let _command = self.commands.lock().await;
        let effects = self.lifecycle.dismiss(notification_id).await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub(crate) async fn expire_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        let _command = self.commands.lock().await;
        let effects = self.lifecycle.expire(notification_id).await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub(crate) async fn close_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        let _command = self.commands.lock().await;
        let effects = self.lifecycle.close(notification_id).await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub(crate) async fn activate_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        self.activate_notification_for_revision(notification_id, None, None)
            .await
    }

    async fn activate_notification_for_revision(
        &self,
        notification_id: InternalNotificationId,
        activation_token: Option<String>,
        expected_revision: Option<i64>,
    ) -> anyhow::Result<()> {
        let thread = self.lifecycle.thread(notification_id).await?;
        if thread.status != NotificationStatus::Active {
            anyhow::bail!("notification {notification_id} is not active");
        }
        anyhow::ensure!(
            expected_revision.is_none_or(|revision| revision == thread.revision),
            "notification was replaced before activation"
        );

        let default_action_key = thread
            .accepted_notification
            .actions
            .iter()
            .find(|action| action.key == "default")
            .map(|action| action.key.clone());

        if let Some(default_action_key) = default_action_key {
            self.invoke_action_for_revision(
                notification_id,
                &default_action_key,
                activation_token,
                Some(thread.revision),
            )
            .await
        } else {
            self.dismiss_notification_for_revision(notification_id, thread.revision)
                .await
        }
    }

    async fn dismiss_notification_for_revision(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
    ) -> anyhow::Result<()> {
        let _command = self.commands.lock().await;
        let effects = self
            .lifecycle
            .dismiss_if_current_revision(notification_id, revision)
            .await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub(crate) async fn activate_banner(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        self.activate_banner_with_activation_token(notification_id, None)
            .await
    }

    pub(crate) async fn handle_banner_interaction(
        &self,
        interaction: BannerInteraction,
    ) -> anyhow::Result<()> {
        match interaction {
            BannerInteraction::Activate {
                target,
                activation_token,
            } => {
                self.activate_notification_for_revision(
                    target.notification_id,
                    activation_token,
                    Some(target.revision),
                )
                .await
            }
            BannerInteraction::Dismiss(target) => {
                self.dismiss_notification_for_revision(target.notification_id, target.revision)
                    .await
            }
            BannerInteraction::InvokeAction {
                target,
                action_key,
                activation_token,
            } => {
                self.invoke_action_for_revision(
                    target.notification_id,
                    &action_key,
                    activation_token,
                    Some(target.revision),
                )
                .await
            }
        }
    }

    pub(crate) async fn activate_banner_with_activation_token(
        &self,
        notification_id: InternalNotificationId,
        activation_token: Option<String>,
    ) -> anyhow::Result<()> {
        self.activate_notification_for_revision(notification_id, activation_token, None)
            .await
    }

    pub(crate) async fn clear_active_notifications(&self) -> anyhow::Result<()> {
        let _command = self.commands.lock().await;
        let batch = self.lifecycle.clear_active_notifications().await?;
        self.apply_lifecycle_effects_and_notify(&batch.effects)
            .await?;
        batch.result
    }

    pub(crate) async fn delete_notification_history(&self) -> anyhow::Result<()> {
        let _command = self.commands.lock().await;
        self.storage.delete_inactive_history().await?;
        self.notify_state_changed();
        Ok(())
    }

    pub(crate) async fn set_do_not_disturb(&self, enabled: bool) -> anyhow::Result<()> {
        self.change_do_not_disturb(Some(enabled)).await.map(|_| ())
    }

    pub(crate) async fn change_do_not_disturb(
        &self,
        enabled: Option<bool>,
    ) -> anyhow::Result<bool> {
        let _change = self.commands.lock().await;
        let enabled =
            enabled.unwrap_or_else(|| !self.loaded_policy_state.lock().unwrap().do_not_disturb());
        self.storage.set_do_not_disturb(enabled).await?;
        self.replace_loaded_policy_state(|state| state.with_do_not_disturb(enabled));
        self.notify_state_changed();
        Ok(enabled)
    }

    pub(crate) async fn do_not_disturb(&self) -> anyhow::Result<bool> {
        Ok(self.loaded_policy_state.lock().unwrap().do_not_disturb())
    }

    pub(crate) async fn upsert_user_rule(&self, rule: UserRule) -> anyhow::Result<()> {
        let _change = self.commands.lock().await;
        self.storage.upsert_user_rule(&rule).await?;
        self.replace_loaded_policy_state(|state| {
            state.with_user_rule(DeliveryPolicyRule::from(rule))
        });
        self.notify_state_changed();
        Ok(())
    }

    pub(crate) async fn user_rules(&self) -> anyhow::Result<Vec<UserRule>> {
        Ok(self
            .loaded_policy_state
            .lock()
            .unwrap()
            .user_rules()
            .iter()
            .cloned()
            .map(|rule| UserRule {
                app_identity: rule.app_identity,
                category: rule.category,
                urgency: rule.urgency,
                outcome: rule.outcome,
                reference: rule.reference,
            })
            .collect())
    }

    pub(crate) async fn remove_user_rule(
        &self,
        app_identity: &str,
        urgency: Option<Urgency>,
    ) -> anyhow::Result<()> {
        let _change = self.commands.lock().await;
        self.storage
            .remove_user_rule(app_identity, None, urgency)
            .await?;
        self.replace_loaded_policy_state(|state| {
            state.without_user_rule(app_identity, None, urgency)
        });
        self.notify_state_changed();
        Ok(())
    }

    pub(crate) async fn remove_category_user_rule(
        &self,
        app_identity: &str,
        category: &str,
        urgency: Option<Urgency>,
    ) -> anyhow::Result<()> {
        let _change = self.commands.lock().await;
        self.storage
            .remove_user_rule(app_identity, Some(category), urgency)
            .await?;
        self.replace_loaded_policy_state(|state| {
            state.without_user_rule(app_identity, Some(category), urgency)
        });
        self.notify_state_changed();
        Ok(())
    }

    pub(crate) async fn install_configuration(
        &self,
        rules: Vec<DeliveryPolicyRule>,
        sound_settings: NotificationSoundSettings,
    ) -> anyhow::Result<()> {
        let _change = self.commands.lock().await;
        if !self.headless {
            let disable = !sound_settings.enabled;
            self.sound_presenter.apply_sound_settings(sound_settings)?;
            if disable {
                self.sound_presenter.stop_all_sounds();
            }
        }
        self.replace_loaded_policy_state(|state| state.with_config_rules(rules));
        self.notify_state_changed();
        Ok(())
    }

    pub(crate) async fn delivery_decision_logs(&self) -> Vec<DeliveryDecisionLog> {
        self.decision_logs.lock().unwrap().iter().cloned().collect()
    }

    pub(crate) async fn sender_events(&self) -> Vec<SenderEvent> {
        self.sender_events.lock().unwrap().iter().cloned().collect()
    }

    pub(crate) fn subscribe_sender_events(&self) -> broadcast::Receiver<SenderEvent> {
        self.sender_event_changes.subscribe()
    }

    pub(crate) fn install_sender_event_publisher(
        &self,
        publisher: Arc<dyn SenderEventPublisher>,
    ) -> anyhow::Result<()> {
        let mut installed = self.sender_publisher.lock().unwrap();
        anyhow::ensure!(
            installed.is_none(),
            "sender event publisher already installed"
        );
        *installed = Some(publisher);
        Ok(())
    }

    pub(crate) fn sender_publication_error(&self) -> Option<String> {
        self.sender_publication_error
            .lock()
            .unwrap()
            .clone()
            .or_else(|| {
                self.sender_publisher
                    .lock()
                    .unwrap()
                    .as_ref()
                    .and_then(|publisher| publisher.error())
            })
    }

    pub(crate) fn supports_activation_tokens(&self) -> bool {
        self.activation_token_capable
    }

    pub(crate) fn supports_sound(&self) -> bool {
        !self.headless && self.sound_presenter.supports_sound()
    }

    pub(crate) fn presentation_error(&self) -> Option<String> {
        self.presentation.error()
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

    fn delivery_policy(&self) -> DeliveryPolicy {
        DeliveryPolicy::new(self.loaded_policy_state.lock().unwrap().clone())
    }

    fn record_decision(&self, log: DeliveryDecisionLog) {
        record_recent(&self.decision_logs, log);
    }

    fn replace_loaded_policy_state(
        &self,
        replace: impl FnOnce(LoadedDeliveryPolicyState) -> LoadedDeliveryPolicyState,
    ) {
        let mut state = self.loaded_policy_state.lock().unwrap();
        *state = replace(state.clone());
    }

    async fn apply_lifecycle_effects_and_notify(
        &self,
        effects: &LifecycleEffects,
    ) -> anyhow::Result<()> {
        self.apply_lifecycle_effects_with_activation_token(effects, None)
            .await?;
        if !effects.is_empty() {
            self.notify_state_changed();
        }
        Ok(())
    }

    async fn apply_lifecycle_effects_and_notify_with_activation_token(
        &self,
        effects: &LifecycleEffects,
        activation_token: Option<String>,
    ) -> anyhow::Result<()> {
        self.apply_lifecycle_effects_with_activation_token_and_sound(
            effects,
            activation_token,
            None,
        )
        .await?;
        if !effects.is_empty() {
            self.notify_state_changed();
        }
        Ok(())
    }

    async fn apply_lifecycle_effects_and_notify_with_sound(
        &self,
        effects: &LifecycleEffects,
        sound: Option<NotificationSoundPlayback>,
    ) -> anyhow::Result<()> {
        self.apply_lifecycle_effects_with_activation_token_and_sound(effects, None, sound)
            .await?;
        if !effects.is_empty() {
            self.notify_state_changed();
        }
        Ok(())
    }

    async fn apply_lifecycle_effects_with_activation_token(
        &self,
        effects: &LifecycleEffects,
        activation_token: Option<String>,
    ) -> anyhow::Result<()> {
        self.apply_lifecycle_effects_with_activation_token_and_sound(
            effects,
            activation_token,
            None,
        )
        .await
    }

    async fn apply_lifecycle_effects_with_activation_token_and_sound(
        &self,
        effects: &LifecycleEffects,
        activation_token: Option<String>,
        sound: Option<NotificationSoundPlayback>,
    ) -> anyhow::Result<()> {
        self.remove_runtime_images_for_terminal_effects(effects);
        let timeout_changes: Vec<_> = effects.timeout_changes().collect();
        self.pending_timeout_starts
            .lock()
            .unwrap()
            .apply(timeout_changes.iter().copied());
        self.cancel_notification_timeouts(timeout_changes.iter().copied());
        {
            let mut banner_state = self.banner_state.lock().unwrap();
            banner_state.apply_changes(effects.banner_changes());
        }
        self.publish_sender_events_with_activation_token(effects, activation_token)
            .await;
        if let Some(sound) = sound {
            self.play_notification_sound(sound);
        }
        if let Err(error) = self.sync_presentation().await {
            // Lifecycle commitment is independent of the replaceable UI projection.
            eprintln!("level=warn component=presentation event=update_failed error={error:#}");
        }
        Ok(())
    }

    fn play_notification_sound(&self, sound: NotificationSoundPlayback) {
        if self.headless {
            return;
        }

        if !self.sound_presenter.supports_sound() {
            return;
        }

        if let Err(error) = self.sound_presenter.play_notification_sound(sound) {
            eprintln!("level=warn component=sound event=request_failed error={error:#}");
        }
    }

    async fn publish_sender_events_with_activation_token(
        &self,
        effects: &LifecycleEffects,
        activation_token: Option<String>,
    ) {
        let events = self
            .sender_events_for_effects(effects, activation_token)
            .await;
        let publisher = self.sender_publisher.lock().unwrap().clone();
        for event in events {
            if let Some(publisher) = &publisher
                && let Err(error) = publisher.publish(event.clone()).await
            {
                *self.sender_publication_error.lock().unwrap() = Some(format!("{error:#}"));
                eprintln!("level=error component=transport event=sender_publication_failed");
            }
            record_recent(&self.sender_events, event.clone());
            let _ = self.sender_event_changes.send(event);
        }
    }

    async fn sender_events_for_effects(
        &self,
        effects: &LifecycleEffects,
        activation_token: Option<String>,
    ) -> Vec<SenderEvent> {
        let mut events = Vec::new();
        let mut activation_token = activation_token.filter(|token| !token.is_empty());
        for effect in effects {
            match effect {
                LifecycleEffect::ActionInvoked {
                    notification_id,
                    action_key,
                    ..
                } => {
                    let activation_token = activation_token.take();
                    if let Some(activation_token) = activation_token {
                        events.push(SenderEvent::ActivationToken {
                            notification_id: *notification_id,
                            activation_token,
                        });
                    }
                    events.push(SenderEvent::ActionInvoked {
                        notification_id: *notification_id,
                        action_key: action_key.clone(),
                    });
                }
                LifecycleEffect::SenderClosed {
                    notification_id,
                    reason,
                } => events.push(SenderEvent::NotificationClosed {
                    notification_id: *notification_id,
                    reason: *reason,
                }),
                LifecycleEffect::SenderReceipt(_)
                | LifecycleEffect::ThreadCreated(_)
                | LifecycleEffect::ThreadRevised(_)
                | LifecycleEffect::ThreadSilenced(_)
                | LifecycleEffect::ThreadDismissed(_)
                | LifecycleEffect::ThreadExpired(_)
                | LifecycleEffect::ThreadClosed(_)
                | LifecycleEffect::ThreadStale(_)
                | LifecycleEffect::NotificationDropped { .. }
                | LifecycleEffect::NotificationTimeoutStartsOnDisplay { .. }
                | LifecycleEffect::NotificationTimeoutCancelledThroughRevision { .. } => {}
            }
        }
        events
    }

    async fn activation_token(&self, app_identity: String) -> Option<String> {
        if !self.activation_token_capable {
            return None;
        }

        tokio::time::timeout(
            ACTIVATION_TOKEN_TIMEOUT,
            self.activation_token_provider
                .activation_token(app_identity),
        )
        .await
        .ok()
        .flatten()
        .filter(|token| !token.is_empty())
    }

    async fn sync_presentation(&self) -> anyhow::Result<()> {
        let (presentation, visible_revisions) = self.banner_presentation_with_visible_revisions();
        let presented_timeout_revisions: Vec<_> = self
            .pending_timeout_starts
            .lock()
            .unwrap()
            .revisions_in(&visible_revisions);
        let runtime = self.clone();
        let acknowledgement = BannerPresentationAcknowledgement::new(move || {
            runtime.start_timeouts_for_presented_revisions(presented_timeout_revisions);
        });
        self.presentation
            .replace_banner_presentation_with_acknowledgement(presentation, acknowledgement)
    }

    fn banner_presentation(&self) -> BannerPresentation {
        self.banner_presentation_with_visible_revisions().0
    }

    fn banner_presentation_with_visible_revisions(
        &self,
    ) -> (BannerPresentation, BTreeSet<(InternalNotificationId, i64)>) {
        let (threads, overflow_count) = {
            let banner_state = self.banner_state.lock().unwrap();
            (
                banner_state.visible_threads(),
                banner_state.overflow_count(),
            )
        };
        let visible_revisions = threads
            .iter()
            .map(|thread| (thread.id, thread.revision))
            .collect();
        let runtime_images = self.runtime_images.lock().unwrap();
        (
            BannerPresentation {
                banners: threads
                    .into_iter()
                    .map(|thread| {
                        let image = runtime_images.image_for(thread.id, thread.revision);
                        BannerView::from_thread(thread, image)
                    })
                    .collect(),
                overflow_count,
            },
            visible_revisions,
        )
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
                LifecycleEffect::SenderReceipt(_)
                | LifecycleEffect::ThreadSilenced(_)
                | LifecycleEffect::ThreadDismissed(_)
                | LifecycleEffect::ThreadExpired(_)
                | LifecycleEffect::ThreadClosed(_)
                | LifecycleEffect::ThreadStale(_)
                | LifecycleEffect::NotificationDropped { .. }
                | LifecycleEffect::NotificationTimeoutStartsOnDisplay { .. }
                | LifecycleEffect::NotificationTimeoutCancelledThroughRevision { .. }
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
                LifecycleEffect::SenderReceipt(_)
                | LifecycleEffect::ThreadCreated(_)
                | LifecycleEffect::ThreadRevised(_)
                | LifecycleEffect::NotificationDropped { .. }
                | LifecycleEffect::NotificationTimeoutStartsOnDisplay { .. }
                | LifecycleEffect::NotificationTimeoutCancelledThroughRevision { .. }
                | LifecycleEffect::ActionInvoked { .. }
                | LifecycleEffect::SenderClosed { .. } => {}
            }
        }
    }

    fn notify_state_changed(&self) {
        let version = self.state_version.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.state_changes.send(version);
    }

    fn cancel_notification_timeouts(
        &self,
        changes: impl IntoIterator<Item = NotificationTimeoutChange>,
    ) {
        for change in changes {
            if let NotificationTimeoutChange::CancelThroughRevision {
                notification_id,
                revision,
            } = change
            {
                self.cancel_notification_timeout_through_revision(notification_id, revision);
            }
        }
    }

    fn start_timeouts_for_presented_revisions(
        &self,
        revisions: impl IntoIterator<Item = (InternalNotificationId, i64)>,
    ) {
        let changes = self.pending_timeout_starts.lock().unwrap().take(revisions);
        for change in changes {
            if let NotificationTimeoutChange::StartsOnDisplay {
                notification_id,
                revision,
                timeout,
                urgency,
            } = change
            {
                if let Some(duration) = self.lifecycle.timeout_duration_on_display(timeout, urgency)
                {
                    self.schedule_notification_timeout(notification_id, revision, duration);
                }
            }
        }
    }

    fn schedule_notification_timeout(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
        duration: Duration,
    ) {
        let pending_timeout_starts = self.pending_timeout_starts.lock().unwrap();
        if !pending_timeout_starts.allows(notification_id, revision) {
            return;
        }
        let mut timeout_tasks = self.timeout_tasks.lock().unwrap();
        if timeout_tasks
            .get(&notification_id)
            .is_some_and(|task| task.revision >= revision)
        {
            return;
        }
        if let Some(task) = timeout_tasks.remove(&notification_id) {
            task.handle.abort();
        }

        let runtime = self.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            runtime.forget_notification_timeout_if_current_revision(notification_id, revision);
            let _ = runtime
                .expire_notification_if_current_revision(notification_id, revision)
                .await;
        });

        timeout_tasks.insert(notification_id, TimeoutTask { revision, handle });
    }

    fn cancel_notification_timeout_through_revision(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
    ) {
        let mut pending_timeout_starts = self.pending_timeout_starts.lock().unwrap();
        pending_timeout_starts.apply([NotificationTimeoutChange::CancelThroughRevision {
            notification_id,
            revision,
        }]);
        let mut timeout_tasks = self.timeout_tasks.lock().unwrap();
        let should_cancel = timeout_tasks
            .get(&notification_id)
            .is_some_and(|task| task.revision <= revision);
        if should_cancel {
            timeout_tasks
                .remove(&notification_id)
                .unwrap()
                .handle
                .abort();
        }
    }

    fn forget_notification_timeout_if_current_revision(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
    ) {
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
        notification_id: InternalNotificationId,
        revision: i64,
    ) -> anyhow::Result<()> {
        let _command = self.commands.lock().await;
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
        .filter(|rule| rule.category.is_none() && rule.urgency.is_none())
        .map(|rule| {
            (
                rule.app_identity.as_str(),
                AppPermissionControl::from_delivery_outcome(rule.outcome),
            )
        })
        .collect();
    let category_permissions: HashMap<_, _> = user_rules
        .iter()
        .filter_map(|rule| {
            if rule.urgency.is_some() {
                return None;
            }
            let category = rule.category.as_ref()?;
            Some((
                (rule.app_identity.clone(), category.clone()),
                AppPermissionControl::from_delivery_outcome(rule.outcome),
            ))
        })
        .collect();
    let mut categories: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for thread in snapshot.history {
        let notification = thread.accepted_notification;
        if let Some(category) = &notification.category {
            categories
                .entry(notification.app_identity.clone())
                .or_default()
                .insert(category.clone());
        }
        let actions = if thread.status == NotificationStatus::Active {
            notification
                .actions
                .iter()
                .filter(|action| action.key != "default")
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        grouped
            .entry(notification.app_identity.clone())
            .or_default()
            .push(NotificationCenterItem {
                notification_id: thread.id,
                summary: notification.summary,
                body: notification.body,
                body_content: notification.body_content,
                status: thread.status,
                delivery_outcome: thread.delivery_outcome,
                actions,
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
                category_permissions: categories
                    .remove(&app_identity)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|category| CategoryPermissionControl {
                        permission: category_permissions
                            .get(&(app_identity.clone(), category.clone()))
                            .copied()
                            .or_else(|| app_permissions.get(app_identity.as_str()).copied())
                            .unwrap_or(AppPermissionControl::Allow),
                        category,
                    })
                    .collect(),
                app_identity,
                items,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEST_ID: InternalNotificationId = InternalNotificationId::new(7).unwrap();
    use crate::{NoopActivationTokenProvider, NoopBannerPresenter, NoopSoundPresenter};

    #[test]
    fn pending_timeout_starts_reject_changes_older_than_a_cancellation() {
        let mut pending = PendingTimeoutStarts::default();
        pending.apply([
            NotificationTimeoutChange::CancelThroughRevision {
                notification_id: TEST_ID,
                revision: 2,
            },
            NotificationTimeoutChange::StartsOnDisplay {
                notification_id: TEST_ID,
                revision: 1,
                timeout: NotificationTimeout::Milliseconds(10),
                urgency: Urgency::Normal,
            },
            NotificationTimeoutChange::StartsOnDisplay {
                notification_id: TEST_ID,
                revision: 3,
                timeout: NotificationTimeout::Milliseconds(10),
                urgency: Urgency::Normal,
            },
        ]);

        assert!(!pending.starts.contains_key(&(TEST_ID, 1)));
        assert!(pending.starts.contains_key(&(TEST_ID, 3)));
    }

    #[tokio::test]
    async fn stale_timeout_changes_cannot_replace_or_cancel_newer_revision_task() {
        let temp = tempfile::tempdir().unwrap();
        let storage = SqlitePersistedState::open(temp.path().join("history.db"), 30, 1_000)
            .await
            .unwrap();
        let runtime = NotificationRuntime::new(
            storage,
            false,
            NotificationTimeoutDefaults::default(),
            Arc::new(NoopBannerPresenter),
            Arc::new(NoopActivationTokenProvider),
            Arc::new(NoopSoundPresenter),
            Vec::new(),
        )
        .await
        .unwrap();

        runtime.schedule_notification_timeout(TEST_ID, 3, Duration::from_secs(60));
        runtime.schedule_notification_timeout(TEST_ID, 2, Duration::from_secs(60));

        assert_eq!(
            runtime
                .timeout_tasks
                .lock()
                .unwrap()
                .get(&TEST_ID)
                .map(|task| task.revision),
            Some(3)
        );
        runtime.cancel_notification_timeout_through_revision(TEST_ID, 2);
        assert_eq!(
            runtime
                .timeout_tasks
                .lock()
                .unwrap()
                .get(&TEST_ID)
                .map(|task| task.revision),
            Some(3)
        );
        runtime.cancel_notification_timeout_through_revision(TEST_ID, 3);
        assert!(!runtime.timeout_tasks.lock().unwrap().contains_key(&TEST_ID));

        runtime.schedule_notification_timeout(TEST_ID, 2, Duration::from_secs(60));
        assert!(!runtime.timeout_tasks.lock().unwrap().contains_key(&TEST_ID));
        runtime.schedule_notification_timeout(TEST_ID, 4, Duration::from_secs(60));
        assert_eq!(
            runtime
                .timeout_tasks
                .lock()
                .unwrap()
                .get(&TEST_ID)
                .map(|task| task.revision),
            Some(4)
        );
    }
}
