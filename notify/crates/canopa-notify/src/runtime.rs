use crate::InternalNotificationId;
use crate::SenderEventPublisher;
use crate::activation::ActivationTokenProvider;
use crate::control::{
    AppPermissionControl, CategoryPermissionControl, NotificationCenterAction,
    NotificationCenterGroup, NotificationCenterItem, NotificationCenterProjection,
    NotificationCenterTarget,
};
use crate::delivery_policy::{
    DeliveryDecisionLog, DeliveryPolicy, DeliveryPolicyRule, LoadedDeliveryPolicyState,
};
use crate::domain::{
    AcceptedNotification, DeliveryOutcome, NotificationImageData, NotificationReceipt,
    NotificationSnapshot, NotificationSoundPlayback, NotificationSoundRequest,
    NotificationSoundSettings, NotificationStatus, NotificationThread, NotificationTimeout,
    NotificationTimeoutDefaults, SenderEvent, Urgency, UserRule,
};
use crate::lifecycle::{
    LifecycleDelivery, LifecycleEffect, LifecycleEffects, NotificationLifecycle,
};
use crate::presentation::{
    BannerInteraction, BannerPresentation, BannerPresentationAcknowledgement, BannerPresenter,
    BannerTarget, BannerView, NotificationBannerState, PresentationCoordinator,
};
use crate::runtime_resources::RuntimeResources;
use crate::runtime_timeouts::RuntimeTimeouts;
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
    resources: Arc<Mutex<RuntimeResources>>,
    activation_token_provider: Arc<dyn ActivationTokenProvider>,
    sound_presenter: Arc<dyn SoundPresenter>,
    link_opener: Arc<dyn crate::NotificationLinkOpener>,
    link_requests: Arc<tokio::sync::Semaphore>,
    link_error: Arc<Mutex<Option<String>>>,
    links_stopping: Arc<std::sync::atomic::AtomicBool>,
    stopping: Arc<std::sync::atomic::AtomicBool>,
    headless: bool,
    activation_token_capable: bool,
    sender_events: Arc<Mutex<VecDeque<SenderEvent>>>,
    sender_event_changes: broadcast::Sender<SenderEvent>,
    sender_publisher: Arc<Mutex<Option<Arc<dyn SenderEventPublisher>>>>,
    sender_publication_error: Arc<Mutex<Option<String>>>,
    decision_logs: Arc<Mutex<VecDeque<DeliveryDecisionLog>>>,
    timeouts: Arc<Mutex<RuntimeTimeouts>>,
    loaded_policy_state: Arc<Mutex<LoadedDeliveryPolicyState>>,
    // Commit and effect order share one coordinator. External token acquisition is outside it.
    commands: Arc<AsyncMutex<()>>,
    state_version: Arc<AtomicU64>,
    state_changes: watch::Sender<u64>,
    center_instance: [u8; 16],
}

impl NotificationRuntime {
    #[allow(
        clippy::too_many_arguments,
        reason = "One constructor wires the runtime adapters; callers use DaemonOptions"
    )]
    pub(crate) async fn new(
        storage: SqlitePersistedState,
        headless: bool,
        timeout_defaults: NotificationTimeoutDefaults,
        banner_presenter: Arc<dyn BannerPresenter>,
        activation_token_provider: Arc<dyn ActivationTokenProvider>,
        sound_presenter: Arc<dyn SoundPresenter>,
        link_opener: Arc<dyn crate::NotificationLinkOpener>,
        configuration_rules: Vec<DeliveryPolicyRule>,
    ) -> anyhow::Result<Self> {
        // External center events can outlive this runtime and its transient ID allocator.
        let mut center_instance = [0u8; 16];
        let mut remaining = center_instance.as_mut_slice();
        while !remaining.is_empty() {
            match rustix::rand::getrandom(&mut *remaining, rustix::rand::GetRandomFlags::empty()) {
                Ok(0) => anyhow::bail!("could not allocate notification center instance"),
                Ok(count) => remaining = &mut remaining[count..],
                Err(rustix::io::Errno::INTR) => continue,
                Err(_) => anyhow::bail!("could not allocate notification center instance"),
            }
        }
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
            resources: Arc::new(Mutex::new(RuntimeResources::default())),
            activation_token_provider,
            sound_presenter,
            link_opener,
            link_requests: Arc::new(tokio::sync::Semaphore::new(8)),
            link_error: Arc::default(),
            links_stopping: Arc::default(),
            stopping: Arc::default(),
            headless,
            activation_token_capable,
            sender_events: Arc::new(Mutex::new(VecDeque::new())),
            sender_event_changes,
            sender_publisher: Arc::new(Mutex::new(None)),
            sender_publication_error: Arc::new(Mutex::new(None)),
            decision_logs: Arc::new(Mutex::new(VecDeque::new())),
            timeouts: Arc::new(Mutex::new(RuntimeTimeouts::default())),
            loaded_policy_state: Arc::new(Mutex::new(loaded_policy_state)),
            commands: Arc::new(AsyncMutex::new(())),
            state_version: Arc::new(AtomicU64::new(0)),
            state_changes,
            center_instance,
        })
    }

    async fn begin_mutation(&self) -> anyhow::Result<tokio::sync::OwnedMutexGuard<()>> {
        let command = self.commands.clone().lock_owned().await;
        anyhow::ensure!(
            !self.stopping.load(Ordering::SeqCst),
            "notification daemon is stopping"
        );
        Ok(command)
    }

    pub(crate) async fn begin_receipt(&self) -> anyhow::Result<tokio::sync::OwnedMutexGuard<()>> {
        let command = self.begin_mutation().await?;
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
        self.resources.lock().unwrap().admit(
            &notification,
            decision.outcome,
            image_data.as_ref(),
        )?;
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
        self.resources
            .lock()
            .unwrap()
            .record_receipt(&effects, image_data);
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

    pub(crate) fn timeout_record_count(&self) -> usize {
        self.timeouts.lock().unwrap().record_count()
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
        let _command = self.begin_mutation().await?;
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
        let _command = self.begin_mutation().await?;
        let effects = self.lifecycle.dismiss(notification_id).await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub(crate) async fn expire_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        let _command = self.begin_mutation().await?;
        let effects = self.lifecycle.expire(notification_id).await?;
        self.apply_lifecycle_effects_and_notify(&effects).await
    }

    pub(crate) async fn close_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        let _command = self.begin_mutation().await?;
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
        let _command = self.begin_mutation().await?;
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
            BannerInteraction::OpenLink {
                target,
                segment_index,
                activation_token,
            } => {
                self.open_link_for_revision(
                    target.notification_id,
                    target.revision,
                    segment_index,
                    activation_token,
                    true,
                )
                .await
            }
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

    pub(crate) async fn handle_notification_center_interaction(
        &self,
        target: NotificationCenterTarget,
        action: NotificationCenterAction,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            target.instance == self.center_instance,
            "notification center target is obsolete"
        );
        match action {
            NotificationCenterAction::OpenLink { segment_index } => {
                self.open_link_for_revision(
                    target.notification_id,
                    target.revision,
                    segment_index,
                    None,
                    false,
                )
                .await
            }
            NotificationCenterAction::Acknowledge => {
                let _command = self.begin_mutation().await?;
                let effects = self
                    .lifecycle
                    .acknowledge_if_current_revision(target.notification_id, target.revision)
                    .await?;
                self.apply_lifecycle_effects_and_notify(&effects).await?;
                self.notify_state_changed();
                Ok(())
            }
            NotificationCenterAction::Activate => {
                self.activate_notification_for_revision(
                    target.notification_id,
                    None,
                    Some(target.revision),
                )
                .await
            }
            NotificationCenterAction::InvokeAction { action_key } => {
                self.invoke_action_for_revision(
                    target.notification_id,
                    &action_key,
                    None,
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

    async fn link_for_revision(
        &self,
        id: InternalNotificationId,
        revision: i64,
        segment_index: usize,
        active_only: bool,
    ) -> anyhow::Result<crate::NotificationLink> {
        let thread = self.lifecycle.thread(id).await?;
        anyhow::ensure!(
            thread.revision == revision
                && (!active_only || thread.status == NotificationStatus::Active),
            "notification link is obsolete"
        );
        match thread
            .accepted_notification
            .body_content
            .segments
            .get(segment_index)
        {
            Some(crate::NotificationBodySegment::Link { uri, .. }) => Ok(uri.clone()),
            _ => anyhow::bail!("notification link is unavailable"),
        }
    }

    async fn open_link_for_revision(
        &self,
        id: InternalNotificationId,
        revision: i64,
        segment_index: usize,
        token: Option<String>,
        active_only: bool,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.link_opener.available() && !self.links_stopping.load(Ordering::SeqCst),
            "notification link opener unavailable"
        );
        // Capacity includes token waits and portal requests. It is admission, not
        // a waiting queue that can grow with repeated interactions.
        let permit = self
            .link_requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| anyhow::anyhow!("too many pending notification link requests"))?;
        self.link_for_revision(id, revision, segment_index, active_only)
            .await?;
        let token = match token.filter(|token| !token.is_empty()) {
            Some(token) => Some(token),
            None => self.activation_token(String::new()).await,
        };
        {
            let _command = self.begin_mutation().await?;
            anyhow::ensure!(
                !self.links_stopping.load(Ordering::SeqCst),
                "notification link opener is shutting down"
            );
            let uri = self
                .link_for_revision(id, revision, segment_index, active_only)
                .await?;
            let opener = self.link_opener.clone();
            let error = self.link_error.clone();
            *error.lock().unwrap() = None;
            // Commit the selected URI under the mutation coordinator. After
            // admission replacements cannot change the user's opening intent.
            // The task also finishes cleanup if its control caller disconnects.
            tokio::spawn(async move {
                let _permit = permit;
                use futures_util::FutureExt;
                let result = std::panic::AssertUnwindSafe(async { opener.open(uri, token).await })
                    .catch_unwind()
                    .await
                    .unwrap_or(crate::LinkOpeningResult::Failed);
                let failure = match result {
                    crate::LinkOpeningResult::Opened | crate::LinkOpeningResult::Cancelled => None,
                    crate::LinkOpeningResult::Unavailable => {
                        Some("notification link opener unavailable")
                    }
                    crate::LinkOpeningResult::TimedOut => {
                        Some("notification link opening timed out")
                    }
                    crate::LinkOpeningResult::Failed => Some("notification link opening failed"),
                };
                if let Some(failure) = failure {
                    *error.lock().unwrap() = Some(failure.into());
                    eprintln!(
                        "level=warn component=links event=opening_failed category={result:?}"
                    );
                }
            });
        }
        // Success means the selected request was admitted. A chooser may remain
        // open for minutes; it must not occupy a control socket or serial input
        // forwarder. Completion/failure is observable in daemon status.
        Ok(())
    }

    pub(crate) fn link_opening_status(&self) -> crate::LinkOpeningStatus {
        crate::LinkOpeningStatus {
            available: self.link_opener.available() && !self.links_stopping.load(Ordering::SeqCst),
            pending: 8 - self.link_requests.available_permits(),
            last_error: self.link_error.lock().unwrap().clone(),
        }
    }

    pub(crate) async fn shutdown_link_opening(&self) -> anyhow::Result<()> {
        {
            let _command = self.commands.lock().await;
            self.links_stopping.store(true, Ordering::SeqCst);
            self.link_opener.cancel_pending();
        }
        let _drained =
            tokio::time::timeout(Duration::from_secs(8), self.link_requests.acquire_many(8))
                .await
                .map_err(|_| anyhow::anyhow!("notification link cleanup timed out"))?
                .map_err(|_| anyhow::anyhow!("notification link cleanup failed"))?;
        Ok(())
    }

    pub(crate) async fn shutdown(&self) -> anyhow::Result<()> {
        {
            // Wait for the current commitment before rejecting queued and future mutations.
            let _command = self.commands.lock().await;
            self.stopping.store(true, Ordering::SeqCst);
            self.timeouts.lock().unwrap().cancel_all();
            self.sound_presenter.stop_all_sounds();
        }
        let publisher = self.sender_publisher.lock().unwrap().clone();
        let publication = async {
            if let Some(publisher) = publisher {
                tokio::time::timeout(Duration::from_secs(5), publisher.flush())
                    .await
                    .map_err(|_| anyhow::anyhow!("sender publication drain timed out"))??;
            }
            Ok::<_, anyhow::Error>(())
        };
        // Always attempt every adapter's cleanup even if another one fails.
        let (links, presentation, publication) = tokio::join!(
            self.shutdown_link_opening(),
            self.presentation.shutdown(),
            publication
        );
        links?;
        presentation?;
        publication?;
        Ok(())
    }

    pub(crate) async fn clear_active_notifications(&self) -> anyhow::Result<()> {
        let _command = self.begin_mutation().await?;
        let batch = self.lifecycle.clear_active_notifications().await?;
        self.apply_lifecycle_effects_and_notify(&batch.effects)
            .await?;
        batch.result
    }

    pub(crate) async fn delete_notification_history(&self) -> anyhow::Result<()> {
        let _command = self.begin_mutation().await?;
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
        let _change = self.begin_mutation().await?;
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
        let _change = self.begin_mutation().await?;
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
        let _change = self.begin_mutation().await?;
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
        let _change = self.begin_mutation().await?;
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
        icon_settings: crate::BannerIconSettings,
        appearance: canopa_theme::Appearance,
    ) -> anyhow::Result<()> {
        let _change = self.begin_mutation().await?;
        if !self.headless {
            let disable = !sound_settings.enabled;
            self.sound_presenter.apply_sound_settings(sound_settings)?;
            if disable {
                self.sound_presenter.stop_all_sounds();
            }
        }
        self.replace_loaded_policy_state(|state| state.with_config_rules(rules));
        self.set_icon_settings(icon_settings);
        self.set_appearance(appearance);
        // A renderer outage is presentation status, not rejection of a valid local snapshot.
        // Publication is synchronous and coalesced; it does not await redraw or restart timeouts.
        let _ = self.sync_presentation();
        self.notify_state_changed();
        Ok(())
    }

    pub(crate) fn set_appearance(&self, appearance: canopa_theme::Appearance) {
        self.presentation.set_appearance(appearance);
    }

    pub(crate) fn set_icon_settings(&self, settings: crate::BannerIconSettings) {
        self.presentation.set_icon_settings(settings);
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
        let mut threads = self.snapshot().await?.history;
        threads.extend(self.lifecycle.transient_threads());
        // Match retained ordering and keep runtime-only HashMap iteration out of the UI.
        threads.sort_by_key(|thread| {
            std::cmp::Reverse((thread.requires_attention, thread.created_at, thread.id))
        });
        let user_rules = self.user_rules().await?;
        Ok(projection_from_threads(
            threads,
            &user_rules,
            self.center_instance,
        ))
    }

    pub(crate) fn subscribe_notification_state(&self) -> NotificationStateSubscription {
        NotificationStateSubscription {
            runtime: self.clone(),
            changes: self.state_changes.subscribe(),
            initial_pending: true,
        }
    }

    pub(crate) async fn notification_center_batch(
        &self,
        after: Option<crate::NotificationCenterCursor>,
        limit: u8,
    ) -> anyhow::Result<crate::NotificationCenterBatch> {
        let mut threads = self.storage.center_threads_after(after, limit).await?;
        threads.extend(self.lifecycle.transient_center_threads_after(after, limit));
        threads.sort_by_key(|thread| {
            std::cmp::Reverse(crate::NotificationCenterCursor::for_thread(thread))
        });
        threads.truncate(usize::from(limit));
        let last = threads
            .last()
            .map(crate::NotificationCenterCursor::for_thread);
        let rules = self.user_rules().await?;
        Ok(crate::NotificationCenterBatch {
            last,
            projection: projection_from_threads(threads, &rules, self.center_instance),
        })
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
        self.resources.lock().unwrap().release(effects);
        self.timeouts
            .lock()
            .unwrap()
            .apply(effects.timeout_changes());
        {
            let mut banner_state = self.banner_state.lock().unwrap();
            banner_state.apply_changes(effects.banner_changes());
        }
        self.publish_sender_events_with_activation_token(effects, activation_token)
            .await;
        if let Some(sound) = sound {
            self.play_notification_sound(sound);
        }
        if let Err(error) = self.sync_presentation() {
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

    fn sync_presentation(&self) -> anyhow::Result<()> {
        let (presentation, visible_revisions) = self.banner_presentation_with_visible_revisions();
        let presented_timeout_revisions: Vec<_> =
            self.timeouts.lock().unwrap().pending_in(&visible_revisions);
        let runtime = self.clone();
        let acknowledgement = BannerPresentationAcknowledgement::for_banners(
            presented_timeout_revisions
                .into_iter()
                .map(|(notification_id, revision)| BannerTarget {
                    notification_id,
                    revision,
                }),
            move |target| {
                runtime.start_timeouts_for_presented_revisions([(
                    target.notification_id,
                    target.revision,
                )])
            },
        );
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
        let resources = self.resources.lock().unwrap();
        (
            BannerPresentation {
                banners: threads
                    .into_iter()
                    .map(|thread| {
                        let image = resources.image_for(thread.id, thread.revision);
                        BannerView::from_thread(thread, image)
                    })
                    .collect(),
                overflow_count,
                icon_settings: self.presentation.icon_settings(),
                appearance: self.presentation.appearance(),
            },
            visible_revisions,
        )
    }

    fn notify_state_changed(&self) {
        let version = self.state_version.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.state_changes.send(version);
    }

    fn start_timeouts_for_presented_revisions(
        &self,
        revisions: impl IntoIterator<Item = (InternalNotificationId, i64)>,
    ) {
        let mut timeouts = self.timeouts.lock().unwrap();
        for (notification_id, revision) in revisions {
            timeouts.start(notification_id, revision, |timeout, urgency| {
                let duration = self
                    .lifecycle
                    .timeout_duration_on_display(timeout, urgency)?;
                let runtime = self.clone();
                Some(tokio::spawn(async move {
                    tokio::time::sleep(duration).await;
                    runtime
                        .timeouts
                        .lock()
                        .unwrap()
                        .finish(notification_id, revision);
                    let _ = runtime
                        .expire_notification_if_current_revision(notification_id, revision)
                        .await;
                }))
            });
        }
    }

    async fn expire_notification_if_current_revision(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
    ) -> anyhow::Result<()> {
        let _command = self.begin_mutation().await?;
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
    /// Coalesced change notification without constructing an unbounded full projection.
    pub async fn changed(&mut self) -> bool {
        if self.initial_pending {
            self.initial_pending = false;
            true
        } else {
            self.changes.changed().await.is_ok()
        }
    }

    pub async fn next_projection(
        &mut self,
    ) -> anyhow::Result<Option<NotificationCenterProjection>> {
        if self.changed().await {
            Ok(Some(self.runtime.notification_center_projection().await?))
        } else {
            Ok(None)
        }
    }
}

fn projection_from_threads(
    threads: Vec<NotificationThread>,
    user_rules: &[UserRule],
    instance: [u8; 16],
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

    for thread in threads {
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
                requires_attention: thread.requires_attention,
                notification_id: thread.id,
                target: NotificationCenterTarget {
                    instance,
                    notification_id: thread.id,
                    revision: thread.revision,
                },
                transient: notification.transient,
                has_default_action: thread.status == NotificationStatus::Active
                    && notification
                        .actions
                        .iter()
                        .any(|action| action.key == "default"),
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
