use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::watch;

pub mod control;
mod delivery_policy;
mod intake;
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

use intake::{NotificationIntakeResult, intake_notification};
use sqlite_persisted_state::SqlitePersistedState;

#[derive(Clone)]
pub struct DaemonOptions {
    pub database_path: PathBuf,
    pub headless: bool,
    pub retention_days: i64,
    pub history_limit: i64,
    banner_presenter: Arc<dyn BannerPresenter>,
}

impl DaemonOptions {
    pub fn headless(database_path: PathBuf) -> Self {
        Self {
            database_path,
            headless: true,
            retention_days: 30,
            history_limit: 1_000,
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
            banner_presenter: Arc::new(banner_presenter),
        }
    }

    pub fn with_history_limit(mut self, history_limit: i64) -> Self {
        self.history_limit = history_limit;
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

pub trait BannerPresenter: Send + Sync {
    fn replace_visible_banners(&self, banners: Vec<BannerView>) -> anyhow::Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerOutputSelection {
    FocusedWithFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerLayer {
    Overlay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerKeyboardInteractivity {
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BannerSurfaceSpec {
    pub output: BannerOutputSelection,
    pub layer: BannerLayer,
    pub keyboard_interactivity: BannerKeyboardInteractivity,
    pub exclusive_zone: i32,
}

impl BannerSurfaceSpec {
    pub fn notification_banner() -> Self {
        Self {
            output: BannerOutputSelection::FocusedWithFallback,
            layer: BannerLayer::Overlay,
            keyboard_interactivity: BannerKeyboardInteractivity::None,
            exclusive_zone: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BannerInteraction {
    Activate(i64),
    Dismiss(i64),
}

#[derive(Clone)]
pub struct IcedLayerShellBannerPresenter {
    updates: Arc<Mutex<BannerUpdateSender>>,
}

impl IcedLayerShellBannerPresenter {
    pub fn start(interactions: mpsc::Sender<BannerInteraction>) -> anyhow::Result<Self> {
        let runtime_id = NEXT_BANNER_RUNTIME_ID.fetch_add(1, Ordering::Relaxed);
        let (updates, receiver) = iced::futures::channel::mpsc::channel(32);
        register_banner_update_receiver(runtime_id, receiver);

        spawn_banner_ui_thread(runtime_id, interactions, run_banner_ui)?;

        Ok(Self {
            updates: Arc::new(Mutex::new(updates)),
        })
    }

    pub fn layer_shell_settings(
        spec: BannerSurfaceSpec,
    ) -> iced_layershell::settings::LayerShellSettings {
        use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
        use iced_layershell::settings::StartMode;

        iced_layershell::settings::LayerShellSettings {
            anchor: match spec.output {
                BannerOutputSelection::FocusedWithFallback => Anchor::Top | Anchor::Right,
            },
            layer: match spec.layer {
                BannerLayer::Overlay => Layer::Overlay,
            },
            exclusive_zone: spec.exclusive_zone,
            size: Some((360, 1)),
            margin: (12, 12, 0, 0),
            keyboard_interactivity: match spec.keyboard_interactivity {
                BannerKeyboardInteractivity::None => KeyboardInteractivity::None,
            },
            start_mode: StartMode::Active,
            events_transparent: false,
        }
    }
}

impl BannerPresenter for IcedLayerShellBannerPresenter {
    fn replace_visible_banners(&self, banners: Vec<BannerView>) -> anyhow::Result<()> {
        self.updates
            .lock()
            .unwrap()
            .try_send(BannerRuntimeInput::Replace(banners))
            .map_err(|error| anyhow::anyhow!("failed to send banner update to renderer: {error}"))
    }
}

type BannerUpdateSender = iced::futures::channel::mpsc::Sender<BannerRuntimeInput>;
type BannerUpdateReceiver = iced::futures::channel::mpsc::Receiver<BannerRuntimeInput>;

static NEXT_BANNER_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);
static BANNER_UPDATE_RECEIVERS: OnceLock<Mutex<HashMap<u64, BannerUpdateReceiver>>> =
    OnceLock::new();

#[derive(Clone, Debug)]
enum BannerRuntimeInput {
    Replace(Vec<BannerView>),
}

#[derive(Clone)]
struct BannerUiState {
    runtime_id: u64,
    banners: Vec<BannerView>,
    interactions: mpsc::Sender<BannerInteraction>,
}

#[iced_layershell::to_layer_message]
#[derive(Clone, Debug)]
enum BannerUiMessage {
    Runtime(BannerRuntimeInput),
    Activate(i64),
    Dismiss(i64),
}

fn register_banner_update_receiver(runtime_id: u64, receiver: BannerUpdateReceiver) {
    BANNER_UPDATE_RECEIVERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .insert(runtime_id, receiver);
}

fn take_banner_update_receiver(runtime_id: u64) -> BannerUpdateReceiver {
    BANNER_UPDATE_RECEIVERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .remove(&runtime_id)
        .unwrap_or_else(|| {
            let (_sender, receiver) = iced::futures::channel::mpsc::channel(1);
            receiver
        })
}

fn banner_update_stream(runtime_id: &u64) -> BannerUpdateReceiver {
    take_banner_update_receiver(*runtime_id)
}

fn spawn_banner_ui_thread<F>(
    runtime_id: u64,
    interactions: mpsc::Sender<BannerInteraction>,
    run_ui: F,
) -> anyhow::Result<()>
where
    F: FnOnce(u64, mpsc::Sender<BannerInteraction>) -> iced_layershell::Result + Send + 'static,
{
    let runtime_handle = tokio::runtime::Handle::try_current()
        .context("notification banner UI must be started from the daemon Tokio runtime")?;
    std::thread::Builder::new()
        .name("canopa-notify-banner-ui".to_string())
        .spawn(move || {
            let _runtime_guard = runtime_handle.enter();
            if let Err(error) = run_ui(runtime_id, interactions) {
                eprintln!(
                    "level=error component=presentation event=renderer_failed error={error:#}"
                );
            }
        })?;
    Ok(())
}

fn run_banner_ui(
    runtime_id: u64,
    interactions: mpsc::Sender<BannerInteraction>,
) -> iced_layershell::Result {
    use iced_layershell::settings::Settings;

    iced_layershell::disable_clipboard();
    iced_layershell::daemon(
        move || BannerUiState {
            runtime_id,
            banners: Vec::new(),
            interactions: interactions.clone(),
        },
        || "Canopa Notify Notification Banners".to_string(),
        banner_ui_update,
        banner_ui_view,
    )
    .subscription(banner_ui_subscription)
    .style(banner_ui_style)
    .settings(Settings {
        id: Some("canopa-notify-banners".to_string()),
        layer_settings: IcedLayerShellBannerPresenter::layer_shell_settings(
            BannerSurfaceSpec::notification_banner(),
        ),
        ..Default::default()
    })
    .run()
}

fn banner_ui_subscription(state: &BannerUiState) -> iced::Subscription<BannerUiMessage> {
    iced::Subscription::run_with(state.runtime_id, banner_update_stream)
        .map(BannerUiMessage::Runtime)
}

fn banner_ui_update(
    state: &mut BannerUiState,
    message: BannerUiMessage,
) -> iced::Task<BannerUiMessage> {
    match message {
        BannerUiMessage::Runtime(BannerRuntimeInput::Replace(banners)) => {
            let size = banner_surface_size(banners.len());
            state.banners = banners;
            iced::Task::done(BannerUiMessage::SizeChange(size))
        }
        BannerUiMessage::Activate(notification_id) => {
            state
                .interactions
                .send(BannerInteraction::Activate(notification_id))
                .ok();
            iced::Task::none()
        }
        BannerUiMessage::Dismiss(notification_id) => {
            state
                .interactions
                .send(BannerInteraction::Dismiss(notification_id))
                .ok();
            iced::Task::none()
        }
        _ => iced::Task::none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn banner_ui_thread_enters_daemon_tokio_runtime() {
        let (interactions, _interaction_receiver) = mpsc::channel();
        let (observed, observed_receiver) = mpsc::channel();

        spawn_banner_ui_thread(0, interactions, move |_, _| {
            let can_spawn_tokio_blocking_task = std::panic::catch_unwind(|| {
                tokio::task::spawn_blocking(|| ());
            })
            .is_ok();
            observed.send(can_spawn_tokio_blocking_task).unwrap();
            Ok(())
        })
        .unwrap();

        assert!(
            observed_receiver
                .recv_timeout(Duration::from_secs(1))
                .unwrap(),
            "banner UI thread must enter the daemon Tokio runtime before invoking iced/layer-shell"
        );
    }
}

fn banner_ui_view(
    state: &BannerUiState,
    _window: iced::window::Id,
) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{column, container};
    use iced::{Alignment, Length};

    let mut stack = column![]
        .spacing(8)
        .width(Length::Fill)
        .align_x(Alignment::End);
    for banner in &state.banners {
        stack = stack.push(notification_banner_widget(banner));
    }

    container(stack)
        .width(Length::Fill)
        .height(Length::Shrink)
        .into()
}

fn notification_banner_widget(banner: &BannerView) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{button, column, container, row, text};
    use iced::{Alignment, Length};

    let title = if banner.summary.trim().is_empty() {
        banner.app_name.as_str()
    } else {
        banner.summary.as_str()
    };

    let content = column![
        text(title).size(16),
        text(&banner.body).size(14),
        text(&banner.app_name).size(12),
    ]
    .spacing(4)
    .width(Length::Fill);

    let activate = button(container(content).width(Length::Fill))
        .width(Length::Fill)
        .on_press(BannerUiMessage::Activate(banner.notification_id));
    let dismiss = button(text("x"))
        .width(32)
        .on_press(BannerUiMessage::Dismiss(banner.notification_id));

    container(row![activate, dismiss].spacing(8).align_y(Alignment::Start))
        .padding(10)
        .width(Length::Fill)
        .style(container::rounded_box)
        .into()
}

fn banner_ui_style(_state: &BannerUiState, theme: &iced::Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: iced::Color::TRANSPARENT,
        text_color: theme.palette().text,
    }
}

fn banner_surface_size(count: usize) -> (u32, u32) {
    if count == 0 {
        (1, 1)
    } else {
        (360, (count.min(3) as u32 * 108).max(108))
    }
}

#[derive(Clone, Default)]
pub struct NoopBannerPresenter;

impl BannerPresenter for NoopBannerPresenter {
    fn replace_visible_banners(&self, _banners: Vec<BannerView>) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct StructuredLogBannerPresenter;

impl BannerPresenter for StructuredLogBannerPresenter {
    fn replace_visible_banners(&self, banners: Vec<BannerView>) -> anyhow::Result<()> {
        eprintln!(
            "level=info component=presentation event=replace_visible_banners count={}",
            banners.len()
        );
        for banner in banners {
            eprintln!(
                "level=info component=presentation event=visible_banner notification_id={} app_identity={} urgency={} summary={:?}",
                banner.notification_id,
                banner.app_identity,
                banner.urgency.as_str(),
                banner.summary
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BannerPlacement {
    TopRightFocusedOutputWithFallback,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BannerFocusBehavior {
    DoesNotTakeFocus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BannerView {
    pub notification_id: i64,
    pub app_identity: String,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    pub progress: Option<i64>,
    pub default_action_key: Option<String>,
    pub placement: BannerPlacement,
    pub focus_behavior: BannerFocusBehavior,
}

impl BannerView {
    fn from_thread(thread: NotificationThread) -> Self {
        let notification = thread.accepted_notification;
        let default_action_key = notification
            .actions
            .iter()
            .find(|action| action.key == "default")
            .map(|action| action.key.clone());
        Self {
            notification_id: thread.id,
            app_identity: notification.app_identity,
            app_name: notification.app_name,
            summary: notification.summary,
            body: notification.body,
            urgency: notification.urgency,
            progress: notification.progress,
            default_action_key,
            placement: BannerPlacement::TopRightFocusedOutputWithFallback,
            focus_behavior: BannerFocusBehavior::DoesNotTakeFocus,
        }
    }
}

#[derive(Clone)]
struct PresentationCoordinator {
    enabled: bool,
    banner_presenter: Arc<dyn BannerPresenter>,
}

impl PresentationCoordinator {
    fn new(enabled: bool, banner_presenter: Arc<dyn BannerPresenter>) -> Self {
        Self {
            enabled,
            banner_presenter,
        }
    }

    fn replace_visible_banners(&self, banners: Vec<BannerView>) -> anyhow::Result<()> {
        if self.enabled {
            self.banner_presenter.replace_visible_banners(banners)?;
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct NotificationBannerState {
    visible: HashMap<i64, NotificationThread>,
}

impl NotificationBannerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_effects(&mut self, effects: &[LifecycleEffect]) {
        for effect in effects {
            self.apply_effect(effect);
        }
    }

    pub fn visible_banners(&self) -> Vec<BannerView> {
        let mut threads: Vec<_> = self.visible.values().cloned().collect();
        sort_banner_candidates(&mut threads);
        threads.into_iter().map(BannerView::from_thread).collect()
    }

    fn apply_effect(&mut self, effect: &LifecycleEffect) {
        match effect {
            LifecycleEffect::ThreadCreated(thread) | LifecycleEffect::ThreadRevised(thread) => {
                self.consider_banner_opportunity(thread.clone());
            }
            LifecycleEffect::ThreadDeliveryChanged(thread) => {
                if self.visible.contains_key(&thread.id) {
                    self.consider_banner_opportunity(thread.clone());
                }
            }
            LifecycleEffect::ThreadSilenced(thread)
            | LifecycleEffect::ThreadDismissed(thread)
            | LifecycleEffect::ThreadExpired(thread)
            | LifecycleEffect::ThreadClosed(thread)
            | LifecycleEffect::ThreadStale(thread) => {
                self.visible.remove(&thread.id);
            }
            LifecycleEffect::NotificationDropped {
                replaces_id: Some(notification_id),
            } => {
                self.visible.remove(notification_id);
            }
            LifecycleEffect::SenderReceipt(_)
            | LifecycleEffect::NotificationDropped { replaces_id: None }
            | LifecycleEffect::ActionInvoked { .. }
            | LifecycleEffect::SenderClosed { .. } => {}
        }
    }

    fn consider_banner_opportunity(&mut self, thread: NotificationThread) {
        if thread.status != NotificationStatus::Active
            || thread.delivery_outcome != DeliveryOutcome::Shown
        {
            self.visible.remove(&thread.id);
            return;
        }

        self.visible.insert(thread.id, thread);
        let mut threads: Vec<_> = self.visible.values().cloned().collect();
        sort_banner_candidates(&mut threads);
        self.visible.clear();
        for thread in threads.into_iter().take(3) {
            self.visible.insert(thread.id, thread);
        }
    }
}

fn sort_banner_candidates(threads: &mut [NotificationThread]) {
    threads.sort_by(|left, right| {
        banner_rank(right)
            .cmp(&banner_rank(left))
            .then_with(|| right.id.cmp(&left.id))
    });
}

fn banner_rank(thread: &NotificationThread) -> (u8, i64) {
    (
        if thread.accepted_notification.urgency == Urgency::Critical {
            1
        } else {
            0
        },
        thread.updated_at,
    )
}

#[derive(Clone)]
pub struct Daemon {
    storage: SqlitePersistedState,
    lifecycle: NotificationLifecycle<SqlitePersistedState>,
    presentation: PresentationCoordinator,
    banner_state: Arc<Mutex<NotificationBannerState>>,
    sender_events: Arc<Mutex<Vec<SenderEvent>>>,
    config_rules: Arc<Mutex<Vec<ConfigRule>>>,
    config_error: Arc<Mutex<Option<String>>>,
    decision_logs: Arc<Mutex<Vec<DeliveryDecisionLog>>>,
    state_version: Arc<AtomicU64>,
    state_changes: watch::Sender<u64>,
}

impl Daemon {
    pub async fn open(options: DaemonOptions) -> anyhow::Result<Self> {
        let storage = SqlitePersistedState::open(
            options.database_path,
            options.retention_days,
            options.history_limit,
        )
        .await?;
        let lifecycle = NotificationLifecycle::new(storage.clone());
        lifecycle.mark_active_stale().await?;
        let (state_changes, _state_change_receiver) = watch::channel(0);
        Ok(Self {
            storage,
            lifecycle,
            presentation: PresentationCoordinator::new(!options.headless, options.banner_presenter),
            banner_state: Arc::new(Mutex::new(NotificationBannerState::new())),
            sender_events: Arc::new(Mutex::new(Vec::new())),
            config_rules: Arc::new(Mutex::new(Vec::new())),
            config_error: Arc::new(Mutex::new(None)),
            decision_logs: Arc::new(Mutex::new(Vec::new())),
            state_version: Arc::new(AtomicU64::new(0)),
            state_changes,
        })
    }

    pub async fn receive_notification(
        &self,
        input: NotificationInput,
    ) -> anyhow::Result<NotificationReceipt> {
        let notification = match intake_notification(input) {
            NotificationIntakeResult::Accepted(notification) => notification,
            NotificationIntakeResult::NoOpDropped(receipt) => return Ok(receipt),
        };

        let policy = self.delivery_policy().await?;
        let decision = policy.decide(&notification);
        self.record_decision(decision.log.clone());

        let effects = self
            .lifecycle
            .receive(notification, LifecycleDelivery::from_decision(&decision))
            .await?;
        self.apply_lifecycle_effects_and_notify(&effects).await?;
        receipt_from_effects(&effects)
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
        let source = std::fs::read_to_string(path).with_context(|| {
            format!(
                "failed to read Notification Configuration {}",
                path.display()
            )
        })?;
        match parse_config_rules(&source) {
            Ok(rules) => {
                *self.config_rules.lock().unwrap() = rules;
                *self.config_error.lock().unwrap() = None;
                self.reevaluate_active_notifications().await?;
                self.notify_state_changed();
                Ok(())
            }
            Err(error) => {
                *self.config_error.lock().unwrap() = Some(error.to_string());
                Err(error)
            }
        }
    }

    pub async fn configuration_error(&self) -> Option<String> {
        self.config_error.lock().unwrap().clone()
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
        let config_rules = self.config_rules.lock().unwrap().clone();
        let user_rules = self.storage.user_rules().await?;
        let do_not_disturb = self.storage.do_not_disturb().await?;
        let state = LoadedDeliveryPolicyState::builder()
            .do_not_disturb(do_not_disturb)
            .user_rules(user_rules.into_iter().map(delivery_policy_rule_from_user))
            .config_rules(
                config_rules
                    .into_iter()
                    .map(delivery_policy_rule_from_config),
            )
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
}

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

    fn from_decision(decision: &DeliveryDecision) -> Self {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadRevision {
    pub accepted_notification: AcceptedNotification,
    pub delivery_outcome: DeliveryOutcome,
    pub rule_reference: Option<String>,
    pub updated_at: i64,
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
    async fn revise_thread(
        &self,
        id: i64,
        revision: ThreadRevision,
    ) -> anyhow::Result<NotificationThread>;
    async fn transition_thread(
        &self,
        id: i64,
        status: NotificationStatus,
        updated_at: i64,
    ) -> anyhow::Result<NotificationThread>;
    async fn active_threads(&self) -> anyhow::Result<Vec<NotificationThread>>;
    async fn update_delivery(
        &self,
        id: i64,
        delivery: LifecycleDelivery,
    ) -> anyhow::Result<NotificationThread>;
    async fn mark_active_threads_stale(
        &self,
        updated_at: i64,
    ) -> anyhow::Result<Vec<NotificationThread>>;
}

#[derive(Clone)]
pub struct NotificationLifecycle<H> {
    history: H,
}

impl<H> NotificationLifecycle<H>
where
    H: NotificationHistory,
{
    pub fn new(history: H) -> Self {
        Self { history }
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
            && self.history.thread(replaces_id).await.is_ok()
        {
            let thread = self
                .history
                .revise_thread(
                    replaces_id,
                    ThreadRevision::from_notification(notification, delivery.clone()),
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
            .create_thread(NewNotificationThread::from_notification(
                notification,
                delivery.clone(),
            ))
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

        let updated = self
            .history
            .update_delivery(notification_id, delivery.clone())
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
        let stale = self.history.mark_active_threads_stale(now_millis()).await?;
        Ok(stale
            .into_iter()
            .map(LifecycleEffect::ThreadStale)
            .collect())
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

        let thread = self
            .history
            .transition_thread(notification_id, status, now_millis())
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
}

impl NewNotificationThread {
    fn from_notification(notification: AcceptedNotification, delivery: LifecycleDelivery) -> Self {
        let now = now_millis();
        Self {
            accepted_notification: notification,
            status: NotificationStatus::Active,
            delivery_outcome: delivery.outcome,
            rule_reference: delivery.rule_reference,
            created_at: now,
            updated_at: now,
        }
    }
}

impl ThreadRevision {
    fn from_notification(notification: AcceptedNotification, delivery: LifecycleDelivery) -> Self {
        Self {
            accepted_notification: notification,
            delivery_outcome: delivery.outcome,
            rule_reference: delivery.rule_reference,
            updated_at: now_millis(),
        }
    }
}

fn receipt_from_effects(effects: &[LifecycleEffect]) -> anyhow::Result<NotificationReceipt> {
    effects
        .iter()
        .find_map(|effect| match effect {
            LifecycleEffect::SenderReceipt(receipt) => Some(receipt.clone()),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("notification lifecycle did not return a sender receipt"))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationInput {
    pub app_name: String,
    pub desktop_entry: Option<String>,
    pub replaces_id: Option<i64>,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    pub category: Option<String>,
    pub progress: Option<i64>,
    pub image_data: Option<Vec<u8>>,
    pub actions: Vec<NotificationAction>,
    pub resident: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcceptedNotification {
    pub app_identity: String,
    pub app_name: String,
    pub replaces_id: Option<i64>,
    pub summary: String,
    pub body: String,
    pub raw_body: String,
    pub urgency: Urgency,
    pub category: Option<String>,
    pub progress: Option<i64>,
    pub has_image_payload: bool,
    pub actions: Vec<NotificationAction>,
    pub resident: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationAction {
    pub key: String,
    pub label: String,
}

impl NotificationAction {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

impl Urgency {
    pub fn as_str(self) -> &'static str {
        match self {
            Urgency::Low => "low",
            Urgency::Normal => "normal",
            Urgency::Critical => "critical",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "low" => Urgency::Low,
            "critical" => Urgency::Critical,
            _ => Urgency::Normal,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryOutcome {
    Shown,
    Silenced,
    Dropped,
}

impl DeliveryOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            DeliveryOutcome::Shown => "shown",
            DeliveryOutcome::Silenced => "silenced",
            DeliveryOutcome::Dropped => "dropped",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "silenced" => DeliveryOutcome::Silenced,
            "dropped" => DeliveryOutcome::Dropped,
            _ => DeliveryOutcome::Shown,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStatus {
    Active,
    Dismissed,
    Expired,
    Closed,
    Stale,
}

impl NotificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            NotificationStatus::Active => "active",
            NotificationStatus::Dismissed => "dismissed",
            NotificationStatus::Expired => "expired",
            NotificationStatus::Closed => "closed",
            NotificationStatus::Stale => "stale",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "dismissed" => NotificationStatus::Dismissed,
            "expired" => NotificationStatus::Expired,
            "closed" => NotificationStatus::Closed,
            "stale" => NotificationStatus::Stale,
            _ => NotificationStatus::Active,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationReceipt {
    pub notification_id: i64,
    pub delivery_outcome: DeliveryOutcome,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationThread {
    pub id: i64,
    pub accepted_notification: AcceptedNotification,
    pub status: NotificationStatus,
    pub delivery_outcome: DeliveryOutcome,
    pub rule_reference: Option<String>,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationSnapshot {
    pub active: Vec<NotificationThread>,
    pub history: Vec<NotificationThread>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SenderEvent {
    pub notification_id: i64,
    pub action_key: Option<String>,
    pub closed_id: Option<i64>,
    pub close_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserRule {
    pub app_identity: String,
    pub urgency: Option<Urgency>,
    pub outcome: DeliveryOutcome,
    pub reference: String,
}

impl UserRule {
    pub fn for_app(app_identity: impl Into<String>, outcome: DeliveryOutcome) -> Self {
        let app_identity = app_identity.into();
        Self {
            reference: format!("user:{app_identity}:*"),
            app_identity,
            urgency: None,
            outcome,
        }
    }

    pub fn for_app_urgency(
        app_identity: impl Into<String>,
        urgency: Urgency,
        outcome: DeliveryOutcome,
    ) -> Self {
        let app_identity = app_identity.into();
        Self {
            reference: format!("user:{app_identity}:{}", urgency.as_str()),
            app_identity,
            urgency: Some(urgency),
            outcome,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConfigRule {
    app_identity: String,
    urgency: Option<Urgency>,
    outcome: DeliveryOutcome,
    reference: String,
}

fn urgency_key(urgency: Option<Urgency>) -> &'static str {
    urgency.map(Urgency::as_str).unwrap_or("*")
}

fn delivery_policy_rule_from_user(rule: UserRule) -> DeliveryPolicyRule {
    DeliveryPolicyRule::new(
        rule.app_identity,
        rule.urgency,
        rule.outcome,
        rule.reference,
    )
}

fn delivery_policy_rule_from_config(rule: ConfigRule) -> DeliveryPolicyRule {
    DeliveryPolicyRule::new(
        rule.app_identity,
        rule.urgency,
        rule.outcome,
        rule.reference,
    )
}

fn urgency_from_key(value: &str) -> Option<Urgency> {
    if value == "*" {
        None
    } else {
        Some(Urgency::from_str(value))
    }
}

fn parse_config_rules(source: &str) -> anyhow::Result<Vec<ConfigRule>> {
    let mut rules = Vec::new();

    for (index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }

        if !line.starts_with("rule") {
            anyhow::bail!(
                "invalid Notification Configuration line {}: expected rule",
                index + 1
            );
        }

        let id = first_quoted_value(line);
        let app_identity = attribute_value(line, "app")
            .ok_or_else(|| anyhow::anyhow!("rule on line {} is missing app", index + 1))?;
        let outcome = attribute_value(line, "outcome")
            .ok_or_else(|| anyhow::anyhow!("rule on line {} is missing outcome", index + 1))
            .and_then(|value| parse_outcome(&value))?;
        let urgency = attribute_value(line, "urgency")
            .map(|value| Ok::<_, anyhow::Error>(Urgency::from_str(&value)))
            .transpose()?;

        rules.push(ConfigRule {
            app_identity,
            urgency,
            outcome,
            reference: id.unwrap_or_else(|| format!("config-rule-{}", rules.len() + 1)),
        });
    }

    Ok(rules)
}

fn parse_outcome(value: &str) -> anyhow::Result<DeliveryOutcome> {
    match value {
        "allow" | "shown" => Ok(DeliveryOutcome::Shown),
        "silence" | "silenced" => Ok(DeliveryOutcome::Silenced),
        "drop" | "dropped" => Ok(DeliveryOutcome::Dropped),
        _ => anyhow::bail!("unknown delivery outcome {value}"),
    }
}

fn first_quoted_value(line: &str) -> Option<String> {
    let mut parts = line.split('"');
    parts.next()?;
    parts.next().map(ToString::to_string)
}

fn attribute_value(line: &str, name: &str) -> Option<String> {
    let pattern = format!("{name}=\"");
    let start = line.find(&pattern)? + pattern.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}
