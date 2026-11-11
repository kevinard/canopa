use crate::InternalNotificationId;
use crate::SenderEventPublisher;
use crate::activation::ActivationTokenProvider;
use crate::center::{CenterProjection, NotificationCenterReader};
use crate::control::{
    NotificationCenterAction, NotificationCenterProjection, NotificationCenterTarget,
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
    LifecycleDelivery, LifecycleEffects, NotificationHistory, NotificationLifecycle,
};
use crate::presentation::{
    BannerInteraction, BannerPresentation, BannerPresentationAcknowledgement, BannerPresenter,
    BannerTarget, BannerView, NotificationBannerState, PresentationCoordinator,
};
use crate::runtime_resources::RuntimeResources;
use crate::runtime_timeouts::RuntimeTimeouts;
use crate::sound::SoundPresenter;
use crate::sqlite_persisted_state::SqlitePersistedState;
use std::collections::{BTreeSet, VecDeque};
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

    /// The coordinator admits at most one completion task. Waiting callers may
    /// cancel before admission; after admission they own only the reply waiter.
    /// The owned guard keeps subsequent mutations and shutdown behind all effects.
    pub(crate) async fn complete_mutation<T: Send + 'static>(
        command: tokio::sync::OwnedMutexGuard<()>,
        operation: impl std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
    ) -> anyhow::Result<T> {
        tokio::spawn(async move {
            let _command = command;
            operation.await
        })
        .await?
    }

    async fn mutate<T, F, Fut>(&self, operation: F) -> anyhow::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(Self) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
    {
        let command = self.begin_mutation().await?;
        Self::complete_mutation(command, operation(self.clone())).await
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
        notification: AcceptedNotification,
        image_data: Option<NotificationImageData>,
        expire_timeout: NotificationTimeout,
        sound_request: NotificationSoundRequest,
        on_receipt: impl FnOnce(&NotificationReceipt),
    ) -> anyhow::Result<NotificationReceipt> {
        let policy = self.delivery_policy();
        let decision = policy.decide(&notification);
        let evicted = self.resources.lock().unwrap().admit(
            &notification,
            decision.outcome,
            image_data.as_ref(),
        )?;
        if let Some(evicted) = evicted {
            // A full live set retires its oldest non-critical thread the way its
            // timeout would, instead of refusing every application. Retained
            // content keeps its attention state in the Center.
            let effects = self.lifecycle.expire(evicted).await?;
            self.apply_lifecycle_effects(&effects, None, None).await?;
            eprintln!("level=warn component=runtime event=live_limit_retired_oldest");
        }
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
        self.apply_lifecycle_effects(&effects, None, sound).await?;
        Ok(receipt)
    }

    pub(crate) async fn drop_notification(
        &self,
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
        self.apply_lifecycle_effects(&effects, None, None).await?;
        Ok(receipt)
    }

    pub(crate) async fn snapshot(&self) -> anyhow::Result<NotificationSnapshot> {
        self.storage.snapshot().await
    }

    pub(crate) async fn retained_active_threads(&self) -> anyhow::Result<Vec<NotificationThread>> {
        self.storage.active_threads().await
    }

    pub(crate) async fn retained_active_count(&self) -> anyhow::Result<u64> {
        self.storage.active_thread_count().await
    }

    pub(crate) fn timeout_record_count(&self) -> usize {
        self.timeouts.lock().unwrap().record_count()
    }

    pub(crate) fn timeout_failure_counts(&self) -> (usize, usize) {
        self.timeouts.lock().unwrap().failure_counts()
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
        let action_key = action_key.to_owned();
        self.mutate(move |runtime| async move {
            let effects = runtime
                .lifecycle
                .invoke_action_if_current_revision(
                    notification_id,
                    &action_key,
                    Some(thread.revision),
                )
                .await?;
            runtime
                .apply_lifecycle_effects(&effects, activation_token, None)
                .await
        })
        .await
    }

    pub(crate) async fn dismiss_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        self.mutate(move |runtime| async move {
            let effects = runtime.lifecycle.dismiss(notification_id).await?;
            runtime.apply_lifecycle_effects(&effects, None, None).await
        })
        .await
    }

    pub(crate) async fn expire_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        self.mutate(move |runtime| async move {
            let effects = runtime.lifecycle.expire(notification_id).await?;
            runtime.apply_lifecycle_effects(&effects, None, None).await
        })
        .await
    }

    pub(crate) async fn close_notification(
        &self,
        notification_id: InternalNotificationId,
    ) -> anyhow::Result<()> {
        self.mutate(move |runtime| async move {
            let effects = runtime.lifecycle.close(notification_id).await?;
            runtime.apply_lifecycle_effects(&effects, None, None).await
        })
        .await
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
        self.mutate(move |runtime| async move {
            let effects = runtime
                .lifecycle
                .dismiss_if_current_revision(notification_id, revision)
                .await?;
            runtime.apply_lifecycle_effects(&effects, None, None).await
        })
        .await
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
        activation_token: Option<String>,
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
                    activation_token,
                    false,
                )
                .await
            }
            NotificationCenterAction::Acknowledge => {
                self.mutate(move |runtime| async move {
                    let effects = runtime
                        .lifecycle
                        .acknowledge_if_current_revision(target.notification_id, target.revision)
                        .await?;
                    runtime
                        .apply_lifecycle_effects(&effects, None, None)
                        .await?;
                    Ok(())
                })
                .await
            }
            NotificationCenterAction::Activate => {
                self.activate_notification_for_revision(
                    target.notification_id,
                    activation_token,
                    Some(target.revision),
                )
                .await
            }
            NotificationCenterAction::InvokeAction { action_key } => {
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
        self.mutate(|runtime| async move {
            let batch = runtime.lifecycle.clear_active_notifications().await?;
            runtime
                .apply_lifecycle_effects(&batch.effects, None, None)
                .await?;
            batch.result
        })
        .await
    }

    pub(crate) async fn delete_notification_history(&self) -> anyhow::Result<()> {
        self.mutate(move |runtime| async move {
            runtime.storage.delete_inactive_history().await?;
            runtime.notify_state_changed();
            Ok(())
        })
        .await
    }

    pub(crate) async fn set_do_not_disturb(&self, enabled: bool) -> anyhow::Result<()> {
        self.change_do_not_disturb(Some(enabled)).await.map(|_| ())
    }

    pub(crate) async fn change_do_not_disturb(
        &self,
        enabled: Option<bool>,
    ) -> anyhow::Result<bool> {
        self.mutate(move |runtime| async move {
            let enabled = enabled
                .unwrap_or_else(|| !runtime.loaded_policy_state.lock().unwrap().do_not_disturb());
            runtime.storage.set_do_not_disturb(enabled).await?;
            runtime.replace_loaded_policy_state(|state| state.with_do_not_disturb(enabled));
            runtime.notify_state_changed();
            Ok(enabled)
        })
        .await
    }

    pub(crate) async fn do_not_disturb(&self) -> anyhow::Result<bool> {
        Ok(self.loaded_policy_state.lock().unwrap().do_not_disturb())
    }

    pub(crate) async fn upsert_user_rule(&self, rule: UserRule) -> anyhow::Result<()> {
        self.mutate(move |runtime| async move {
            runtime.storage.upsert_user_rule(&rule).await?;
            runtime.replace_loaded_policy_state(|state| {
                state.with_user_rule(DeliveryPolicyRule::from(rule))
            });
            runtime.notify_state_changed();
            Ok(())
        })
        .await
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
        let app_identity = app_identity.to_owned();
        self.mutate(move |runtime| async move {
            runtime
                .storage
                .remove_user_rule(&app_identity, None, urgency)
                .await?;
            runtime.replace_loaded_policy_state(|state| {
                state.without_user_rule(&app_identity, None, urgency)
            });
            runtime.notify_state_changed();
            Ok(())
        })
        .await
    }

    pub(crate) async fn remove_category_user_rule(
        &self,
        app_identity: &str,
        category: &str,
        urgency: Option<Urgency>,
    ) -> anyhow::Result<()> {
        let app_identity = app_identity.to_owned();
        let category = category.to_owned();
        self.mutate(move |runtime| async move {
            runtime
                .storage
                .remove_user_rule(&app_identity, Some(&category), urgency)
                .await?;
            runtime.replace_loaded_policy_state(|state| {
                state.without_user_rule(&app_identity, Some(&category), urgency)
            });
            runtime.notify_state_changed();
            Ok(())
        })
        .await
    }

    pub(crate) async fn install_configuration(
        &self,
        rules: Vec<DeliveryPolicyRule>,
        sound_settings: NotificationSoundSettings,
        icon_settings: crate::BannerIconSettings,
        appearance: canopa_theme::Appearance,
    ) -> anyhow::Result<()> {
        self.mutate(move |runtime| async move {
            if !runtime.headless {
                let disable = !sound_settings.enabled;
                runtime
                    .sound_presenter
                    .apply_sound_settings(sound_settings)?;
                if disable {
                    runtime.sound_presenter.stop_all_sounds();
                }
            }
            runtime.replace_loaded_policy_state(|state| state.with_config_rules(rules));
            runtime.set_icon_settings(icon_settings);
            runtime.set_appearance(appearance);
            // A renderer outage is presentation status, not rejection of a valid local snapshot.
            // Publication is synchronous and coalesced; it does not await redraw or restart timeouts.
            let _ = runtime.sync_presentation();
            runtime.notify_state_changed();
            Ok(())
        })
        .await
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

    pub(crate) async fn notification_summary(
        &self,
    ) -> anyhow::Result<canopa_notify_client::Summary> {
        // Read the count and policy under the same coordinator as commits. The
        // indexed aggregate never decodes content; transient items require no attention.
        let _read = self.begin_mutation().await?;
        Ok(canopa_notify_client::Summary {
            instance: self.center_instance,
            requires_attention: self.storage.requires_attention_count().await?,
            do_not_disturb: self.loaded_policy_state.lock().unwrap().do_not_disturb(),
        })
    }

    pub(crate) async fn notification_center_projection(
        &self,
    ) -> anyhow::Result<NotificationCenterProjection> {
        let mut threads = self.snapshot().await?.history;
        threads.extend(self.lifecycle.transient_threads());
        // Match retained ordering and keep runtime-only HashMap iteration out of the UI.
        threads.sort_by_key(|thread| {
            std::cmp::Reverse(crate::NotificationCenterCursor::for_thread(thread))
        });
        let user_rules = self.user_rules().await?;
        Ok(CenterProjection::new(
            &user_rules,
            self.center_instance,
            self.presentation.icon_settings(),
        )
        .project(threads))
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
        self.notification_center_reader()
            .await?
            .batch(after, limit)
            .await
    }

    pub(crate) async fn notification_center_reader(
        &self,
    ) -> anyhow::Result<NotificationCenterReader> {
        let rules = self.user_rules().await?;
        Ok(NotificationCenterReader::new(
            self.storage.clone(),
            self.lifecycle.clone(),
            CenterProjection::new(
                &rules,
                self.center_instance,
                self.presentation.icon_settings(),
            ),
        ))
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

    async fn apply_lifecycle_effects(
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
        if self.presentation.presents_banners() {
            // Silenced revisions never get a banner, so their timeout begins at
            // acceptance instead of display. Headless Mode displays nothing and
            // therefore expires nothing, silenced or not.
            self.start_timeouts_for_presented_revisions(effects.silenced_revisions());
        }
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
        if !effects.is_empty() {
            self.notify_state_changed();
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
        let mut activation_token = activation_token.filter(|token| !token.is_empty());
        // Decorate the canonical Lifecycle projection. Only the first action
        // consumes the token, immediately before its action event.
        let events = effects.sender_events().flat_map(move |event| {
            let token = match &event {
                SenderEvent::ActionInvoked {
                    notification_id, ..
                } => activation_token
                    .take()
                    .map(|activation_token| SenderEvent::ActivationToken {
                        notification_id: *notification_id,
                        activation_token,
                    }),
                _ => None,
            };
            token.into_iter().chain(std::iter::once(event))
        });
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
        if self.presentation.presents_banners() {
            // Overflowed revisions are not promoted when a slot frees, so they
            // would never be displayed. Their timeout begins when they overflow.
            let overflow = self.banner_state.lock().unwrap().overflow_revisions();
            self.start_timeouts_for_presented_revisions(overflow);
        }
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
                Some(self.schedule_expiry(notification_id, revision, duration))
            });
        }
    }

    fn schedule_expiry(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
        delay: Duration,
    ) -> tokio::task::JoinHandle<()> {
        let runtime = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            if !runtime
                .timeouts
                .lock()
                .unwrap()
                .begin_expiry(notification_id, revision)
            {
                return;
            }
            match runtime
                .expire_notification_if_current_revision(notification_id, revision)
                .await
            {
                Ok(()) => runtime
                    .timeouts
                    .lock()
                    .unwrap()
                    .complete(notification_id, revision),
                Err(_) => {
                    let terminal = runtime.timeouts.lock().unwrap().failed(
                        notification_id,
                        revision,
                        |delay| runtime.schedule_expiry(notification_id, revision, delay),
                    );
                    if let Some(terminal) = terminal {
                        // Storage error strings may contain notification data.
                        eprintln!(
                            "level=warn component=timeout event=expiry_failed terminal={terminal}"
                        );
                    }
                }
            }
        })
    }

    async fn expire_notification_if_current_revision(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
    ) -> anyhow::Result<()> {
        self.mutate(move |runtime| async move {
            let effects = runtime
                .lifecycle
                .expire_timeout_if_current_revision(notification_id, revision)
                .await?;
            runtime.apply_lifecycle_effects(&effects, None, None).await
        })
        .await
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

    pub async fn next_summary(&mut self) -> anyhow::Result<Option<canopa_notify_client::Summary>> {
        if self.changed().await {
            Ok(Some(self.runtime.notification_summary().await?))
        } else {
            Ok(None)
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
