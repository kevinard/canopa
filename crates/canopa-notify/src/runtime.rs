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
use crate::preferences::{
    AppPreferences, DoNotDisturbPreferences, DoNotDisturbRequest, DoNotDisturbStatus,
    DoNotDisturbTimer, MAX_APP_PREFERENCES, PreferenceLayers, SuppressionState,
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
const DND_TIMER_KEY: &str = "do_not_disturb_timer";
const DND_PREFERENCES_KEY: &str = "do_not_disturb_preferences";
const APP_PREFERENCES_KEY: &str = "app_preferences";
const CENTER_CLEARED_KEY: &str = "center_cleared_through";
/// How long "Clear all" can be undone.
pub(crate) const CLEAR_UNDO_WINDOW: Duration = Duration::from_secs(30);
const MAX_UNDO_ATTENTION: u32 = 4096;

/// Runtime-only record of the latest "Clear all"; never persisted.
struct ClearUndo {
    token: u64,
    expires: std::time::Instant,
    previous: Option<crate::NotificationCenterCursor>,
    unread: Vec<(InternalNotificationId, i64)>,
}

/// Runtime-only presentation state for one revision (ADR 0055).
#[derive(Clone, Debug)]
pub(crate) struct ThreadHints {
    pub(crate) revision: i64,
    pub(crate) keep_after_banner: bool,
    pub(crate) reply: Option<crate::ReplyRequest>,
    /// The Center asked to reopen the banner with its reply composer.
    pub(crate) compose: bool,
    timeout: NotificationTimeout,
    urgency: Urgency,
}

const MAX_THREAD_HINTS: usize = 4096;

/// An undo offer for the Center: its opaque token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClearUndoOffer {
    pub(crate) token: u64,
}

/// Configuration-authored preference layer (ADR 0055).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ConfiguredPreferences {
    pub(crate) do_not_disturb: Option<DoNotDisturbPreferences>,
    pub(crate) apps: std::collections::BTreeMap<String, AppPreferences>,
}
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
    state_changes: Arc<watch::Sender<u64>>,
    center_instance: [u8; 16],
    /// Wakes the Do Not Disturb clock after policy changes.
    dnd_wake: Arc<tokio::sync::Notify>,
    cleared_through: Arc<Mutex<Option<crate::NotificationCenterCursor>>>,
    clear_undo: Arc<Mutex<Option<ClearUndo>>>,
    /// Sender lock-screen limits (portal display hints), runtime-only and bounded.
    lock_hints:
        Arc<Mutex<std::collections::BTreeMap<InternalNotificationId, crate::LockScreenVisibility>>>,
    thread_hints: Arc<Mutex<std::collections::BTreeMap<InternalNotificationId, ThreadHints>>>,
    /// A quiet replacement whose banner must not reappear in the current commit.
    quiet_banner: Arc<Mutex<Option<InternalNotificationId>>>,
    /// Reply text for the action committed under the current mutation.
    pending_reply: Arc<Mutex<Option<String>>>,
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
        configured: ConfiguredPreferences,
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
        let mut timer: DoNotDisturbTimer = storage
            .metadata_json(DND_TIMER_KEY)
            .await
            .unwrap_or_default()
            .unwrap_or_default();
        timer.manual = storage.do_not_disturb().await?;
        let mut user_apps: std::collections::BTreeMap<String, AppPreferences> = storage
            .metadata_json(APP_PREFERENCES_KEY)
            .await
            .unwrap_or_default()
            .unwrap_or_default();
        while user_apps.len() > MAX_APP_PREFERENCES {
            user_apps.pop_last();
        }
        let loaded_policy_state =
            loaded_policy_state
                .with_dnd_timer(timer)
                .with_preferences(PreferenceLayers {
                    config_dnd: configured.do_not_disturb,
                    user_dnd: storage
                        .metadata_json(DND_PREFERENCES_KEY)
                        .await
                        .unwrap_or_default(),
                    config_apps: configured.apps,
                    user_apps,
                });
        let cleared_through = storage
            .metadata_json(CENTER_CLEARED_KEY)
            .await
            .unwrap_or_default();
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
            state_changes: Arc::new(state_changes),
            center_instance,
            dnd_wake: Arc::new(tokio::sync::Notify::new()),
            cleared_through: Arc::new(Mutex::new(cleared_through)),
            clear_undo: Arc::default(),
            lock_hints: Arc::default(),
            thread_hints: Arc::default(),
            quiet_banner: Arc::default(),
            pending_reply: Arc::default(),
        }
        .with_dnd_clock())
    }

    /// Publish a state change when a timer or scheduled window begins or ends,
    /// so the bar and Center follow without polling. Holds no strong runtime state.
    fn with_dnd_clock(self) -> Self {
        let state = Arc::downgrade(&self.loaded_policy_state);
        let changes = Arc::downgrade(&self.state_changes);
        let version = Arc::downgrade(&self.state_version);
        let wake = self.dnd_wake.clone();
        let stopping = self.stopping.clone();
        tokio::spawn(async move {
            while let Some(next) = state.upgrade().map(|state| {
                state
                    .lock()
                    .unwrap()
                    .do_not_disturb_status(&chrono::Local::now())
                    .next_change
            }) {
                let delay = next
                    .map(|next| (next - crate::clock::now_millis()).clamp(0, 3_600_000))
                    .unwrap_or(3_600_000);
                let elapsed = tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(delay as u64 + 20)) => true,
                    _ = wake.notified() => false,
                };
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                if elapsed
                    && next.is_some_and(|next| next <= crate::clock::now_millis())
                    && let (Some(changes), Some(version)) = (changes.upgrade(), version.upgrade())
                {
                    let value = version.fetch_add(1, Ordering::Relaxed) + 1;
                    let _ = changes.send(value);
                }
            }
        });
        self
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
        hints: crate::PresentationHints,
        on_receipt: impl FnOnce(&NotificationReceipt),
    ) -> anyhow::Result<NotificationReceipt> {
        let policy = self.delivery_policy();
        let mut decision = policy.decide(&notification);
        if hints.center_only && decision.outcome == DeliveryOutcome::Shown {
            decision.outcome = DeliveryOutcome::Silenced;
            decision.rule_reference = Some("display-hint".into());
            decision.log.delivery_outcome = DeliveryOutcome::Silenced;
            decision.log.rule_reference = decision.rule_reference.clone();
            decision.log.reason = crate::DeliveryDecisionReason::DisplayHint;
        }
        // Without `show-as-new`, a replacement whose banner already hid updates quietly.
        let quiet = hints.quiet_update
            && notification
                .replaces_id
                .is_some_and(|id| !self.banner_state.lock().unwrap().shows(id));
        let urgency = notification.urgency;
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
        let revision = effects.raw().iter().find_map(|effect| match effect {
            crate::LifecycleEffect::ThreadCreated(thread)
            | crate::LifecycleEffect::ThreadRevised(thread)
                if thread.id == receipt.notification_id =>
            {
                Some(thread.revision)
            }
            _ => None,
        });
        if let Some(revision) = revision {
            self.record_thread_hints(
                receipt.notification_id,
                revision,
                &hints,
                expire_timeout,
                urgency,
            );
        }
        let app_sound = self
            .loaded_policy_state
            .lock()
            .unwrap()
            .preferences()
            .sound_enabled(&sound_app_identity);
        let sound = (decision.outcome == DeliveryOutcome::Shown
            && !sound_request.suppress
            && app_sound
            && !quiet)
            .then_some(NotificationSoundPlayback {
                notification_id: receipt.notification_id,
                app_identity: sound_app_identity,
                summary: sound_summary,
                requested_sound: sound_request.sound,
            });
        if quiet {
            *self.quiet_banner.lock().unwrap() = Some(receipt.notification_id);
        }
        let applied = self.apply_lifecycle_effects(&effects, None, sound).await;
        if quiet {
            *self.quiet_banner.lock().unwrap() = None;
            // The hidden revision is never displayed, so it has no display timeout.
            if let Some(revision) = revision {
                self.timeouts.lock().unwrap().apply([
                    crate::NotificationTimeoutChange::CancelThroughRevision {
                        notification_id: receipt.notification_id,
                        revision,
                    },
                ]);
            }
        }
        applied?;
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
        self.invoke_action_with_reply(
            notification_id,
            action_key,
            activation_token,
            expected_revision,
            None,
        )
        .await
    }

    async fn invoke_action_with_reply(
        &self,
        notification_id: InternalNotificationId,
        action_key: &str,
        activation_token: Option<String>,
        expected_revision: Option<i64>,
        reply: Option<String>,
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
            *runtime.pending_reply.lock().unwrap() = reply;
            let applied = runtime
                .apply_lifecycle_effects(&effects, activation_token, None)
                .await;
            runtime.pending_reply.lock().unwrap().take();
            applied
        })
        .await
    }

    fn record_thread_hints(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
        hints: &crate::PresentationHints,
        timeout: NotificationTimeout,
        urgency: Urgency,
    ) {
        if let Some(visibility) = hints.lock_screen {
            self.record_lock_hint(notification_id, visibility);
        } else {
            self.lock_hints.lock().unwrap().remove(&notification_id);
        }
        let mut thread_hints = self.thread_hints.lock().unwrap();
        if hints.keep_after_banner || hints.reply.is_some() {
            thread_hints.insert(
                notification_id,
                ThreadHints {
                    revision,
                    keep_after_banner: hints.keep_after_banner,
                    reply: hints.reply.clone(),
                    compose: false,
                    timeout,
                    urgency,
                },
            );
            while thread_hints.len() > MAX_THREAD_HINTS {
                thread_hints.pop_first();
            }
        } else {
            thread_hints.remove(&notification_id);
        }
    }

    pub(crate) fn thread_hints(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
    ) -> Option<ThreadHints> {
        self.thread_hints
            .lock()
            .unwrap()
            .get(&notification_id)
            .filter(|hints| hints.revision == revision)
            .cloned()
    }

    pub(crate) fn reply_revisions(
        &self,
    ) -> std::collections::HashSet<(InternalNotificationId, i64)> {
        self.thread_hints
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, hints)| hints.reply.is_some())
            .map(|(id, hints)| (*id, hints.revision))
            .collect()
    }

    /// Send typed reply text with the revision's reply action. The text is never
    /// stored; it travels with the committed action to the sender transport.
    pub(crate) async fn reply_to_notification(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
        text: String,
        activation_token: Option<String>,
    ) -> anyhow::Result<()> {
        let text = text.trim().to_owned();
        anyhow::ensure!(
            !text.is_empty() && text.len() <= crate::MAX_REPLY_BYTES && !text.contains('\0'),
            "invalid reply text"
        );
        let action_key = self
            .thread_hints(notification_id, revision)
            .and_then(|hints| hints.reply)
            .map(|reply| reply.action_key)
            .ok_or_else(|| anyhow::anyhow!("notification does not accept replies"))?;
        self.invoke_action_with_reply(
            notification_id,
            &action_key,
            activation_token,
            Some(revision),
            Some(text),
        )
        .await
    }

    /// Hold or release the banner-hide timer while a reply is being composed.
    pub(crate) fn set_composing(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
        composing: bool,
    ) {
        let Some(hints) = self.thread_hints(notification_id, revision) else {
            return;
        };
        if let Some(stored) = self.thread_hints.lock().unwrap().get_mut(&notification_id) {
            stored.compose = false;
        }
        if composing {
            self.timeouts.lock().unwrap().apply([
                crate::NotificationTimeoutChange::CancelThroughRevision {
                    notification_id,
                    revision,
                },
            ]);
        } else {
            self.timeouts.lock().unwrap().apply([
                crate::NotificationTimeoutChange::StartsOnDisplay {
                    notification_id,
                    revision,
                    timeout: hints.timeout,
                    urgency: hints.urgency,
                },
            ]);
            if self.banner_state.lock().unwrap().shows(notification_id) {
                self.start_timeouts_for_presented_revisions([(notification_id, revision)]);
            }
        }
        let _ = self.sync_presentation();
    }

    /// Reopen an active notification's banner with its reply composer (Center "Reply…").
    pub(crate) async fn open_reply_composer(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
    ) -> anyhow::Result<()> {
        self.mutate(move |runtime| async move {
            let thread = runtime.lifecycle.thread(notification_id).await?;
            anyhow::ensure!(
                thread.revision == revision && thread.status == NotificationStatus::Active,
                "notification was replaced or closed before replying"
            );
            {
                let mut hints = runtime.thread_hints.lock().unwrap();
                let hints = hints
                    .get_mut(&notification_id)
                    .filter(|hints| hints.revision == revision && hints.reply.is_some())
                    .ok_or_else(|| anyhow::anyhow!("notification does not accept replies"))?;
                hints.compose = true;
            }
            runtime
                .banner_state
                .lock()
                .unwrap()
                .apply_changes([crate::BannerLifecycleChange::ConsiderOpportunity(thread)]);
            let _ = runtime.sync_presentation();
            runtime.notify_state_changed();
            Ok(())
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
            BannerInteraction::Reply {
                target,
                text,
                activation_token,
            } => {
                self.reply_to_notification(
                    target.notification_id,
                    target.revision,
                    text,
                    activation_token,
                )
                .await
            }
            BannerInteraction::Composing { target, composing } => {
                self.set_composing(target.notification_id, target.revision, composing);
                Ok(())
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
            NotificationCenterAction::Reply => {
                self.open_reply_composer(target.notification_id, target.revision)
                    .await
            }
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
            self.stopping_dnd_clock();
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

    fn stopping_dnd_clock(&self) {
        // The mutation coordinator sets `stopping`; wake the clock to observe it.
        self.dnd_wake.notify_one();
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

    /// Acknowledge every unread item without ending live sender lifecycles.
    pub(crate) async fn mark_all_read(&self) -> anyhow::Result<usize> {
        self.mutate(|runtime| async move {
            let unread = runtime
                .storage
                .attention_revisions(MAX_UNDO_ATTENTION)
                .await?;
            let mut changed = 0;
            for (id, revision) in unread {
                let effects = runtime
                    .lifecycle
                    .set_attention_if_current_revision(
                        InternalNotificationId::try_from(id)?,
                        revision,
                        false,
                    )
                    .await?;
                changed += usize::from(!effects.is_empty());
                runtime
                    .apply_lifecycle_effects(&effects, None, None)
                    .await?;
            }
            Ok(changed)
        })
        .await
    }

    /// Dismiss live notifications, mark everything read and hide the cleared
    /// items from the Center. Undo restores visibility and unread state for
    /// `CLEAR_UNDO_WINDOW`; dismissals already reached their senders.
    pub(crate) async fn clear_all(&self) -> anyhow::Result<u64> {
        self.mutate(|runtime| async move {
            let unread: Vec<_> = runtime
                .storage
                .attention_revisions(MAX_UNDO_ATTENTION)
                .await?
                .into_iter()
                .filter_map(|(id, revision)| {
                    Some((InternalNotificationId::try_from(id).ok()?, revision))
                })
                .collect();
            let batch = runtime.lifecycle.clear_active_notifications().await?;
            runtime
                .apply_lifecycle_effects(&batch.effects, None, None)
                .await?;
            batch.result?;
            for (id, revision) in &unread {
                let effects = runtime
                    .lifecycle
                    .set_attention_if_current_revision(*id, *revision, false)
                    .await?;
                runtime
                    .apply_lifecycle_effects(&effects, None, None)
                    .await?;
            }
            let newest = runtime
                .storage
                .newest_center_cursor()
                .await?
                .into_iter()
                .chain(
                    runtime
                        .lifecycle
                        .transient_center_cursors()
                        .first()
                        .copied(),
                )
                .max();
            let previous = *runtime.cleared_through.lock().unwrap();
            let through = newest.max(previous);
            runtime
                .storage
                .set_metadata(&[(
                    CENTER_CLEARED_KEY,
                    through.map(|c| serde_json::to_string(&c)).transpose()?,
                )])
                .await?;
            *runtime.cleared_through.lock().unwrap() = through;
            let token = runtime.state_version.fetch_add(1, Ordering::Relaxed) + 1;
            *runtime.clear_undo.lock().unwrap() = Some(ClearUndo {
                token,
                expires: std::time::Instant::now() + CLEAR_UNDO_WINDOW,
                previous,
                unread,
            });
            // Re-render once the offer lapses so the Center drops its Undo action.
            let changes = Arc::downgrade(&runtime.state_changes);
            let version = Arc::downgrade(&runtime.state_version);
            tokio::spawn(async move {
                tokio::time::sleep(CLEAR_UNDO_WINDOW + Duration::from_millis(50)).await;
                if let (Some(changes), Some(version)) = (changes.upgrade(), version.upgrade()) {
                    let _ = changes.send(version.fetch_add(1, Ordering::Relaxed) + 1);
                }
            });
            runtime.notify_state_changed();
            Ok(token)
        })
        .await
    }

    pub(crate) fn clear_undo_offer(&self) -> Option<ClearUndoOffer> {
        let undo = self.clear_undo.lock().unwrap();
        undo.as_ref()
            .filter(|undo| undo.expires > std::time::Instant::now())
            .map(|undo| ClearUndoOffer { token: undo.token })
    }

    pub(crate) async fn undo_clear(&self, token: u64) -> anyhow::Result<()> {
        self.mutate(move |runtime| async move {
            let undo = {
                let mut slot = runtime.clear_undo.lock().unwrap();
                anyhow::ensure!(
                    slot.as_ref().is_some_and(
                        |undo| undo.token == token && undo.expires > std::time::Instant::now()
                    ),
                    "nothing to undo"
                );
                slot.take().expect("checked undo")
            };
            runtime
                .storage
                .set_metadata(&[(
                    CENTER_CLEARED_KEY,
                    undo.previous
                        .map(|c| serde_json::to_string(&c))
                        .transpose()?,
                )])
                .await?;
            *runtime.cleared_through.lock().unwrap() = undo.previous;
            for (id, revision) in undo.unread {
                // A replaced or deleted item keeps its current state.
                if let Ok(effects) = runtime
                    .lifecycle
                    .set_attention_if_current_revision(id, revision, true)
                    .await
                {
                    runtime
                        .apply_lifecycle_effects(&effects, None, None)
                        .await?;
                }
            }
            runtime.notify_state_changed();
            Ok(())
        })
        .await
    }

    /// Remember a sender's lock-screen limit for a thread (runtime-only).
    pub(crate) fn record_lock_hint(
        &self,
        notification_id: InternalNotificationId,
        visibility: crate::LockScreenVisibility,
    ) {
        let mut hints = self.lock_hints.lock().unwrap();
        hints.insert(notification_id, visibility);
        while hints.len() > crate::runtime_resources_lock_hint_limit() {
            hints.pop_first();
        }
    }

    /// The lock-screen feed (ADR 0055). Only critical notifications, and alarm
    /// or call notifications when they break through Do Not Disturb, that are
    /// live or unread. Each shows "App — New notification" unless the app's rule
    /// (and the sender) allows its summary. Nothing interactive crosses.
    pub(crate) async fn lock_screen_snapshot(
        &self,
    ) -> anyhow::Result<canopa_notify_client::LockScreenSnapshot> {
        use canopa_notify_client::{LockScreenEntry, LockScreenSnapshot, MAX_LOCK_SCREEN_ENTRIES};
        let mut threads = self.storage.lock_screen_candidates(64).await?;
        threads.extend(self.lifecycle.transient_threads());
        threads.sort_by_key(|thread| {
            std::cmp::Reverse(crate::NotificationCenterCursor::for_thread(thread))
        });
        let (layers, break_through) = {
            let state = self.loaded_policy_state.lock().unwrap();
            (
                state.preferences().clone(),
                state.preferences().dnd().break_through,
            )
        };
        let hints = self.lock_hints.lock().unwrap().clone();
        let mut snapshot = LockScreenSnapshot::default();
        for thread in threads {
            let notification = &thread.accepted_notification;
            if !(thread.status == NotificationStatus::Active || thread.requires_attention)
                || thread.delivery_outcome == DeliveryOutcome::Dropped
            {
                continue;
            }
            let critical = notification.urgency == Urgency::Critical;
            if !critical && !break_through.matches(notification.category.as_deref()) {
                continue;
            }
            let mut visibility = layers.lock_screen(&notification.app_identity);
            if let Some(hint) = hints.get(&thread.id) {
                visibility = visibility.min(*hint);
            }
            if visibility == crate::LockScreenVisibility::Hidden {
                continue;
            }
            if snapshot.entries.len() == MAX_LOCK_SCREEN_ENTRIES {
                snapshot.more = snapshot.more.saturating_add(1);
                continue;
            }
            let app = [
                notification.app_name.trim(),
                notification.app_identity.trim(),
            ]
            .into_iter()
            .find(|name| !name.is_empty())
            .unwrap_or("Application");
            let text = if visibility == crate::LockScreenVisibility::Content
                && !notification.summary.trim().is_empty()
            {
                notification.summary.trim()
            } else {
                crate::HIDDEN_CONTENT_SUMMARY
            };
            let bounded = |value: &str, limit: usize| {
                value
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(limit)
                    .collect()
            };
            snapshot.entries.push(LockScreenEntry {
                app: bounded(app, 64),
                text: bounded(text, 120),
                critical,
            });
        }
        Ok(snapshot)
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
        let request = match enabled {
            Some(true) => DoNotDisturbRequest::On,
            Some(false) => DoNotDisturbRequest::Off,
            None => DoNotDisturbRequest::Toggle,
        };
        Ok(self.request_do_not_disturb(request).await?.active)
    }

    pub(crate) async fn request_do_not_disturb(
        &self,
        request: DoNotDisturbRequest,
    ) -> anyhow::Result<DoNotDisturbStatus> {
        self.mutate(move |runtime| async move {
            let now = chrono::Local::now();
            let (timer, schedule) = {
                let state = runtime.loaded_policy_state.lock().unwrap();
                (state.dnd_timer(), state.preferences().dnd().schedule)
            };
            let timer = request.apply(timer, schedule.as_ref(), &now)?;
            runtime
                .storage
                .set_metadata(&[(
                    DND_TIMER_KEY,
                    (timer.until.is_some() || timer.schedule_paused_until.is_some())
                        .then(|| serde_json::to_string(&timer))
                        .transpose()?,
                )])
                .await?;
            runtime.storage.set_do_not_disturb(timer.manual).await?;
            runtime.replace_loaded_policy_state(|state| state.with_dnd_timer(timer));
            runtime.dnd_wake.notify_one();
            runtime.notify_state_changed();
            Ok(runtime.do_not_disturb_status())
        })
        .await
    }

    pub(crate) fn do_not_disturb_status(&self) -> DoNotDisturbStatus {
        self.loaded_policy_state
            .lock()
            .unwrap()
            .do_not_disturb_status(&chrono::Local::now())
    }

    pub(crate) async fn do_not_disturb(&self) -> anyhow::Result<bool> {
        Ok(self.do_not_disturb_status().active)
    }

    /// Replace the UI-managed Do Not Disturb preferences; None returns to configuration.
    pub(crate) async fn set_do_not_disturb_preferences(
        &self,
        preferences: Option<DoNotDisturbPreferences>,
    ) -> anyhow::Result<()> {
        self.mutate(move |runtime| async move {
            runtime
                .storage
                .set_metadata(&[(
                    DND_PREFERENCES_KEY,
                    preferences.map(|p| serde_json::to_string(&p)).transpose()?,
                )])
                .await?;
            runtime.replace_loaded_policy_state(|state| {
                let mut layers = state.preferences().clone();
                layers.user_dnd = preferences;
                state.with_preferences(layers)
            });
            runtime.dnd_wake.notify_one();
            let _ = runtime.sync_presentation();
            runtime.notify_state_changed();
            Ok(())
        })
        .await
    }

    /// Replace one application's UI-managed preferences; empty removes them.
    pub(crate) async fn set_app_preferences(
        &self,
        app_identity: String,
        preferences: AppPreferences,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !app_identity.trim().is_empty() && app_identity.len() <= 512,
            "invalid App Identity"
        );
        self.mutate(move |runtime| async move {
            let mut layers = runtime
                .loaded_policy_state
                .lock()
                .unwrap()
                .preferences()
                .clone();
            if preferences.is_empty() {
                layers.user_apps.remove(&app_identity);
            } else {
                anyhow::ensure!(
                    layers.user_apps.contains_key(&app_identity)
                        || layers.user_apps.len() < MAX_APP_PREFERENCES,
                    "too many application preferences"
                );
                layers.user_apps.insert(app_identity, preferences);
            }
            runtime
                .storage
                .set_metadata(&[(
                    APP_PREFERENCES_KEY,
                    (!layers.user_apps.is_empty())
                        .then(|| serde_json::to_string(&layers.user_apps))
                        .transpose()?,
                )])
                .await?;
            runtime.replace_loaded_policy_state(|state| state.with_preferences(layers));
            runtime.notify_state_changed();
            Ok(())
        })
        .await
    }

    pub(crate) fn preference_layers(&self) -> PreferenceLayers {
        self.loaded_policy_state
            .lock()
            .unwrap()
            .preferences()
            .clone()
    }

    /// Compositor-reported fullscreen and screencast state (Niri IPC).
    pub(crate) fn set_suppression(&self, suppression: SuppressionState) {
        let changed = {
            let mut state = self.loaded_policy_state.lock().unwrap();
            let changed = state.suppression() != suppression;
            *state = state.clone().with_suppression(suppression);
            changed
        };
        if changed {
            // Visible banners are redacted at presentation time while sharing.
            let _ = self.sync_presentation();
            self.notify_state_changed();
        }
    }

    pub(crate) fn suppression_inputs(&self) -> SuppressionState {
        self.loaded_policy_state.lock().unwrap().suppression()
    }

    pub(crate) fn active_suppression(&self) -> Option<crate::Suppression> {
        self.loaded_policy_state
            .lock()
            .unwrap()
            .active_suppression()
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
        configured: ConfiguredPreferences,
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
            runtime.replace_loaded_policy_state(|state| {
                let mut layers = state.preferences().clone();
                layers.config_dnd = configured.do_not_disturb;
                layers.config_apps = configured.apps;
                state.with_config_rules(rules).with_preferences(layers)
            });
            runtime.dnd_wake.notify_one();
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
        let status = self.do_not_disturb_status();
        Ok(canopa_notify_client::Summary {
            instance: self.center_instance,
            requires_attention: self.storage.requires_attention_count().await?,
            do_not_disturb: status.active,
            do_not_disturb_reason: status.reason,
            do_not_disturb_until: status.until,
            suppression: self.active_suppression(),
        })
    }

    pub(crate) async fn notification_center_projection(
        &self,
    ) -> anyhow::Result<NotificationCenterProjection> {
        let mut threads = self.snapshot().await?.history;
        threads.extend(self.lifecycle.transient_threads());
        let cleared = *self.cleared_through.lock().unwrap();
        threads.retain(|thread| {
            thread.status == NotificationStatus::Active
                || thread.requires_attention
                || cleared.is_none_or(|c| crate::NotificationCenterCursor::for_thread(thread) > c)
        });
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
        .with_replies(self.reply_revisions())
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
            )
            .with_replies(self.reply_revisions()),
            *self.cleared_through.lock().unwrap(),
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
            let quiet = *self.quiet_banner.lock().unwrap();
            let mut banner_state = self.banner_state.lock().unwrap();
            banner_state.apply_changes(effects.banner_changes().filter(|change| {
                !matches!(change, crate::BannerLifecycleChange::ConsiderOpportunity(thread)
                    if Some(thread.id) == quiet)
            }));
        }
        for event in effects.sender_events() {
            if let SenderEvent::NotificationClosed {
                notification_id, ..
            } = event
            {
                self.thread_hints.lock().unwrap().remove(&notification_id);
            }
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
        let reply = self.pending_reply.lock().unwrap().take();
        self.publish_sender_events(effects, activation_token, reply)
            .await
    }

    async fn publish_sender_events(
        &self,
        effects: &LifecycleEffects,
        activation_token: Option<String>,
        mut reply: Option<String>,
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
            // The reply, like the token, belongs only to the first action.
            let text = match &event {
                SenderEvent::ActionInvoked {
                    notification_id, ..
                } => reply.take().map(|text| SenderEvent::ReplyText {
                    notification_id: *notification_id,
                    text,
                }),
                _ => None,
            };
            token.into_iter().chain(text).chain(std::iter::once(event))
        });
        let publisher = self.sender_publisher.lock().unwrap().clone();
        for event in events {
            if let Some(publisher) = &publisher
                && let Err(error) = publisher.publish(event.clone()).await
            {
                *self.sender_publication_error.lock().unwrap() = Some(format!("{error:#}"));
                eprintln!("level=error component=transport event=sender_publication_failed");
            }
            // Reply text reaches only the transport, never diagnostics or broadcasts.
            let event = match event {
                SenderEvent::ReplyText {
                    notification_id, ..
                } => SenderEvent::ReplyText {
                    notification_id,
                    text: String::new(),
                },
                event => event,
            };
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
        let hide_content = self.active_suppression() == Some(crate::Suppression::ScreenSharing);
        let resources = self.resources.lock().unwrap();
        (
            BannerPresentation {
                banners: threads
                    .into_iter()
                    .map(|thread| {
                        let image = resources.image_for(thread.id, thread.revision);
                        let hints = self.thread_hints(thread.id, thread.revision);
                        let mut view = BannerView::from_thread(thread, image);
                        view.reply = hints.and_then(|hints| {
                            hints.reply.map(|reply| crate::BannerReply {
                                action_key: reply.action_key,
                                placeholder: reply.placeholder,
                                compose: hints.compose,
                            })
                        });
                        if hide_content {
                            view.with_hidden_content()
                        } else {
                            view
                        }
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
            if runtime
                .thread_hints(notification_id, revision)
                .is_some_and(|hints| hints.keep_after_banner)
            {
                // Banner-hide timer: the sender lifecycle and its actions stay
                // live in the Center until the user or the application ends them.
                runtime
                    .banner_state
                    .lock()
                    .unwrap()
                    .apply_changes([crate::BannerLifecycleChange::Remove { notification_id }]);
                let _ = runtime.sync_presentation();
                runtime.notify_state_changed();
                return Ok(());
            }
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

    pub async fn next_lock_screen(
        &mut self,
    ) -> anyhow::Result<Option<canopa_notify_client::LockScreenSnapshot>> {
        if self.changed().await {
            Ok(Some(self.runtime.lock_screen_snapshot().await?))
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
