use anyhow::Context;
use canopa_panel_protocol::{AppPermission, InteractionEvent};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};

mod delivery_policy;

pub use delivery_policy::{
    DeliveryDecision, DeliveryDecisionLog, DeliveryDecisionReason, DeliveryPolicy,
    DeliveryPolicyFacts, DeliveryPolicyRule, LoadedDeliveryPolicyState,
};

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
        Self {
            notification_id: thread.id,
            app_identity: thread.app_identity,
            app_name: thread.app_name,
            summary: thread.summary,
            body: thread.body,
            urgency: thread.urgency,
            progress: thread.progress,
            default_action_key: thread
                .actions
                .iter()
                .find(|action| action.key == "default")
                .map(|action| action.key.clone()),
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
        if thread.urgency == Urgency::Critical {
            1
        } else {
            0
        },
        thread.updated_at,
    )
}

#[derive(Clone)]
pub struct Daemon {
    storage: Storage,
    lifecycle: NotificationLifecycle<Storage>,
    presentation: PresentationCoordinator,
    banner_state: Arc<Mutex<NotificationBannerState>>,
    sender_events: Arc<Mutex<Vec<SenderEvent>>>,
    config_rules: Arc<Mutex<Vec<ConfigRule>>>,
    config_error: Arc<Mutex<Option<String>>>,
    decision_logs: Arc<Mutex<Vec<DeliveryDecisionLog>>>,
}

impl Daemon {
    pub async fn open(options: DaemonOptions) -> anyhow::Result<Self> {
        let storage = Storage::open(
            options.database_path,
            RetentionPolicy {
                days: options.retention_days,
                history_limit: options.history_limit,
            },
        )
        .await?;
        let lifecycle = NotificationLifecycle::new(storage.clone());
        lifecycle.mark_active_stale().await?;
        storage.prune_history().await?;
        Ok(Self {
            storage,
            lifecycle,
            presentation: PresentationCoordinator::new(!options.headless, options.banner_presenter),
            banner_state: Arc::new(Mutex::new(NotificationBannerState::new())),
            sender_events: Arc::new(Mutex::new(Vec::new())),
            config_rules: Arc::new(Mutex::new(Vec::new())),
            config_error: Arc::new(Mutex::new(None)),
            decision_logs: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub async fn receive_notification(
        &self,
        input: NotificationInput,
    ) -> anyhow::Result<NotificationReceipt> {
        let app_identity = derive_app_identity(&input);
        let body = plain_text_body(&input.body);

        if input.summary.trim().is_empty()
            && body.trim().is_empty()
            && input.actions.is_empty()
            && input.category.is_none()
            && input.progress.is_none()
        {
            return Ok(NotificationReceipt {
                notification_id: 0,
                delivery_outcome: DeliveryOutcome::Dropped,
            });
        }

        let policy = self.delivery_policy().await?;
        let decision = policy.decide(DeliveryPolicyFacts::new(&app_identity, input.urgency));
        self.record_decision(decision.log.clone());

        let effects = self
            .lifecycle
            .receive(
                LifecycleNotification {
                    app_identity,
                    app_name: input.app_name,
                    replaces_id: input.replaces_id,
                    summary: input.summary,
                    body,
                    raw_body: input.body,
                    urgency: input.urgency,
                    category: input.category,
                    progress: input.progress,
                    actions: input.actions,
                    resident: input.resident,
                },
                LifecycleDelivery::from_decision(&decision),
            )
            .await?;
        self.apply_lifecycle_effects(&effects).await?;
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
        self.apply_lifecycle_effects(&effects).await
    }

    pub async fn dismiss_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        let effects = self.lifecycle.dismiss(notification_id).await?;
        self.apply_lifecycle_effects(&effects).await
    }

    pub async fn expire_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        let effects = self.lifecycle.expire(notification_id).await?;
        self.apply_lifecycle_effects(&effects).await
    }

    pub async fn close_notification(&self, notification_id: i64) -> anyhow::Result<()> {
        let effects = self.lifecycle.close(notification_id).await?;
        self.apply_lifecycle_effects(&effects).await
    }

    pub async fn activate_banner(&self, notification_id: i64) -> anyhow::Result<()> {
        let thread = self.storage.thread(notification_id).await?;
        if thread.status != NotificationStatus::Active {
            anyhow::bail!("notification {notification_id} is not active");
        }

        let default_action_key = thread
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
        self.apply_lifecycle_effects(&effects).await
    }

    pub async fn delete_notification_history(&self) -> anyhow::Result<()> {
        self.storage.delete_inactive_history().await
    }

    pub async fn set_do_not_disturb(&self, enabled: bool) -> anyhow::Result<()> {
        self.storage.set_do_not_disturb(enabled).await?;
        self.reevaluate_active_notifications().await
    }

    pub async fn do_not_disturb(&self) -> anyhow::Result<bool> {
        self.storage.do_not_disturb().await
    }

    pub async fn upsert_user_rule(&self, rule: UserRule) -> anyhow::Result<()> {
        self.storage.upsert_user_rule(&rule).await?;
        self.reevaluate_active_notifications().await
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
        self.reevaluate_active_notifications().await
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
                self.reevaluate_active_notifications().await
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

    pub async fn handle_panel_event(&self, event: InteractionEvent) -> anyhow::Result<()> {
        match event {
            InteractionEvent::SetAppPermission {
                app_identity,
                permission,
            } => {
                self.upsert_user_rule(UserRule::for_app(
                    app_identity,
                    match permission {
                        AppPermission::Allow => DeliveryOutcome::Shown,
                        AppPermission::Silence => DeliveryOutcome::Silenced,
                        AppPermission::Drop => DeliveryOutcome::Dropped,
                    },
                ))
                .await
            }
            InteractionEvent::ClearActiveNotifications => self.clear_active_notifications().await,
            InteractionEvent::DeleteNotificationHistory => self.delete_notification_history().await,
            InteractionEvent::InvokeNotificationAction {
                notification_id,
                action_key,
            } => self.invoke_action(notification_id, &action_key).await,
        }
    }

    pub async fn sender_events(&self) -> Vec<SenderEvent> {
        self.sender_events.lock().unwrap().clone()
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
        for thread in active {
            let decision = policy.decide(DeliveryPolicyFacts::new(
                &thread.app_identity,
                thread.urgency,
            ));
            let effects = self
                .lifecycle
                .apply_delivery_decision(thread.id, LifecycleDelivery::from_decision(&decision))
                .await?;
            self.apply_lifecycle_effects(&effects).await?;
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
}

#[derive(Clone)]
struct Storage {
    pool: SqlitePool,
    retention: RetentionPolicy,
}

#[derive(Clone, Copy)]
struct RetentionPolicy {
    days: i64,
    history_limit: i64,
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
pub struct LifecycleNotification {
    pub app_identity: String,
    pub app_name: String,
    pub replaces_id: Option<i64>,
    pub summary: String,
    pub body: String,
    pub raw_body: String,
    pub urgency: Urgency,
    pub category: Option<String>,
    pub progress: Option<i64>,
    pub actions: Vec<NotificationAction>,
    pub resident: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewNotificationThread {
    pub app_identity: String,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub raw_body: String,
    pub urgency: Urgency,
    pub status: NotificationStatus,
    pub delivery_outcome: DeliveryOutcome,
    pub rule_reference: Option<String>,
    pub category: Option<String>,
    pub progress: Option<i64>,
    pub actions: Vec<NotificationAction>,
    pub resident: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadRevision {
    pub app_identity: String,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub raw_body: String,
    pub urgency: Urgency,
    pub delivery_outcome: DeliveryOutcome,
    pub rule_reference: Option<String>,
    pub category: Option<String>,
    pub progress: Option<i64>,
    pub actions: Vec<NotificationAction>,
    pub resident: bool,
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
        notification: LifecycleNotification,
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

        if !thread.actions.iter().any(|action| action.key == action_key) {
            anyhow::bail!("notification {notification_id} has no action {action_key}");
        }

        let mut effects = vec![LifecycleEffect::ActionInvoked {
            notification_id,
            action_key: action_key.to_string(),
        }];

        if !thread.resident {
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
    fn from_notification(notification: LifecycleNotification, delivery: LifecycleDelivery) -> Self {
        let now = now_millis();
        Self {
            app_identity: notification.app_identity,
            app_name: notification.app_name,
            summary: notification.summary,
            body: notification.body,
            raw_body: notification.raw_body,
            urgency: notification.urgency,
            status: NotificationStatus::Active,
            delivery_outcome: delivery.outcome,
            rule_reference: delivery.rule_reference,
            category: notification.category,
            progress: notification.progress,
            actions: notification.actions,
            resident: notification.resident,
            created_at: now,
            updated_at: now,
        }
    }
}

impl ThreadRevision {
    fn from_notification(notification: LifecycleNotification, delivery: LifecycleDelivery) -> Self {
        Self {
            app_identity: notification.app_identity,
            app_name: notification.app_name,
            summary: notification.summary,
            body: notification.body,
            raw_body: notification.raw_body,
            urgency: notification.urgency,
            delivery_outcome: delivery.outcome,
            rule_reference: delivery.rule_reference,
            category: notification.category,
            progress: notification.progress,
            actions: notification.actions,
            resident: notification.resident,
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

impl Storage {
    async fn open(path: PathBuf, retention: RetentionPolicy) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .with_context(|| format!("invalid SQLite database path {}", path.display()))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS daemon_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_rules (
                app_identity TEXT NOT NULL,
                urgency TEXT NOT NULL,
                outcome TEXT NOT NULL,
                reference TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (app_identity, urgency)
            )
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS notification_threads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                app_identity TEXT NOT NULL,
                app_name TEXT NOT NULL,
                summary TEXT NOT NULL,
                body TEXT NOT NULL,
                raw_body TEXT NOT NULL,
                urgency TEXT NOT NULL,
                status TEXT NOT NULL,
                delivery_outcome TEXT NOT NULL,
                rule_reference TEXT,
                category TEXT,
                progress INTEGER,
                resident INTEGER NOT NULL DEFAULT 0,
                actions_json TEXT NOT NULL DEFAULT '[]',
                revision INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                closed_at INTEGER
            )
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool, retention })
    }

    async fn snapshot(&self) -> anyhow::Result<NotificationSnapshot> {
        let rows = sqlx::query(
            r#"
            SELECT id, app_identity, app_name, summary, body, raw_body, urgency, status,
                   delivery_outcome, rule_reference, category, progress, resident,
                   actions_json, revision, created_at, updated_at, closed_at
            FROM notification_threads
            ORDER BY created_at DESC, id DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut active = Vec::new();
        let mut history = Vec::new();

        for row in rows {
            let thread = NotificationThread::from_row(&row)?;
            if thread.status == NotificationStatus::Active {
                active.push(thread.clone());
            }
            history.push(thread);
        }

        Ok(NotificationSnapshot { active, history })
    }

    async fn thread(&self, id: i64) -> anyhow::Result<NotificationThread> {
        let row = sqlx::query(
            r#"
            SELECT id, app_identity, app_name, summary, body, raw_body, urgency, status,
                   delivery_outcome, rule_reference, category, progress, resident,
                   actions_json, revision, created_at, updated_at, closed_at
            FROM notification_threads
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        NotificationThread::from_row(&row)
    }

    async fn active_ids(&self) -> anyhow::Result<Vec<i64>> {
        let ids = sqlx::query_scalar(
            "SELECT id FROM notification_threads WHERE status = 'active' ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(ids)
    }

    async fn delete_inactive_history(&self) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM notification_threads WHERE status != 'active'")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn set_do_not_disturb(&self, enabled: bool) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO daemon_metadata (key, value)
            VALUES ('do_not_disturb', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
        )
        .bind(if enabled { "true" } else { "false" })
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn do_not_disturb(&self) -> anyhow::Result<bool> {
        let value: Option<String> =
            sqlx::query_scalar("SELECT value FROM daemon_metadata WHERE key = 'do_not_disturb'")
                .fetch_optional(&self.pool)
                .await?;
        Ok(value.as_deref() == Some("true"))
    }

    async fn upsert_user_rule(&self, rule: &UserRule) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO user_rules (app_identity, urgency, outcome, reference, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(app_identity, urgency) DO UPDATE SET
                outcome = excluded.outcome,
                reference = excluded.reference,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&rule.app_identity)
        .bind(rule.urgency_key())
        .bind(rule.outcome.as_str())
        .bind(&rule.reference)
        .bind(now_millis())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn user_rules(&self) -> anyhow::Result<Vec<UserRule>> {
        let rows = sqlx::query(
            "SELECT app_identity, urgency, outcome, reference FROM user_rules ORDER BY app_identity, urgency",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(UserRule {
                    app_identity: row.try_get("app_identity")?,
                    urgency: urgency_from_key(&row.try_get::<String, _>("urgency")?),
                    outcome: DeliveryOutcome::from_str(&row.try_get::<String, _>("outcome")?),
                    reference: row.try_get("reference")?,
                })
            })
            .collect()
    }

    async fn remove_user_rule(
        &self,
        app_identity: &str,
        urgency: Option<Urgency>,
    ) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM user_rules WHERE app_identity = ?1 AND urgency = ?2")
            .bind(app_identity)
            .bind(urgency_key(urgency))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn prune_history(&self) -> anyhow::Result<()> {
        let cutoff = now_millis() - self.retention.days.max(0) * 24 * 60 * 60 * 1_000;

        sqlx::query(
            "DELETE FROM notification_threads WHERE status != 'active' AND updated_at < ?1",
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM notification_threads
            WHERE status != 'active'
              AND id IN (
                  SELECT id
                  FROM notification_threads
                  WHERE status != 'active'
                  ORDER BY updated_at DESC, id DESC
                  LIMIT -1 OFFSET ?1
              )
            "#,
        )
        .bind(self.retention.history_limit.max(0))
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

impl NotificationHistory for Storage {
    async fn create_thread(
        &self,
        thread: NewNotificationThread,
    ) -> anyhow::Result<NotificationThread> {
        let actions_json = serde_json::to_string(&thread.actions)?;
        let id = sqlx::query(
            r#"
            INSERT INTO notification_threads (
                app_identity, app_name, summary, body, raw_body, urgency, status,
                delivery_outcome, rule_reference, category, progress, resident, created_at,
                updated_at, actions_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
        )
        .bind(&thread.app_identity)
        .bind(&thread.app_name)
        .bind(&thread.summary)
        .bind(&thread.body)
        .bind(&thread.raw_body)
        .bind(thread.urgency.as_str())
        .bind(thread.status.as_str())
        .bind(thread.delivery_outcome.as_str())
        .bind(&thread.rule_reference)
        .bind(&thread.category)
        .bind(thread.progress)
        .bind(thread.resident)
        .bind(thread.created_at)
        .bind(thread.updated_at)
        .bind(actions_json)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        self.thread(id).await
    }

    async fn thread(&self, id: i64) -> anyhow::Result<NotificationThread> {
        Storage::thread(self, id).await
    }

    async fn revise_thread(
        &self,
        id: i64,
        revision: ThreadRevision,
    ) -> anyhow::Result<NotificationThread> {
        let actions_json = serde_json::to_string(&revision.actions)?;
        sqlx::query(
            r#"
            UPDATE notification_threads
            SET app_identity = ?2,
                app_name = ?3,
                summary = ?4,
                body = ?5,
                raw_body = ?6,
                urgency = ?7,
                category = ?8,
                progress = ?9,
                resident = ?10,
                actions_json = ?11,
                delivery_outcome = ?12,
                rule_reference = ?13,
                status = 'active',
                revision = revision + 1,
                updated_at = ?14,
                closed_at = NULL
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(revision.app_identity)
        .bind(revision.app_name)
        .bind(revision.summary)
        .bind(revision.body)
        .bind(revision.raw_body)
        .bind(revision.urgency.as_str())
        .bind(revision.category)
        .bind(revision.progress)
        .bind(revision.resident)
        .bind(actions_json)
        .bind(revision.delivery_outcome.as_str())
        .bind(revision.rule_reference)
        .bind(revision.updated_at)
        .execute(&self.pool)
        .await?;
        self.thread(id).await
    }

    async fn transition_thread(
        &self,
        id: i64,
        status: NotificationStatus,
        updated_at: i64,
    ) -> anyhow::Result<NotificationThread> {
        sqlx::query(
            r#"
            UPDATE notification_threads
            SET status = ?2, updated_at = ?3, closed_at = ?3
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(status.as_str())
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        self.prune_history().await?;
        self.thread(id).await
    }

    async fn active_threads(&self) -> anyhow::Result<Vec<NotificationThread>> {
        Ok(self.snapshot().await?.active)
    }

    async fn update_delivery(
        &self,
        id: i64,
        delivery: LifecycleDelivery,
    ) -> anyhow::Result<NotificationThread> {
        sqlx::query(
            r#"
            UPDATE notification_threads
            SET delivery_outcome = ?2,
                rule_reference = ?3
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(delivery.outcome.as_str())
        .bind(delivery.rule_reference)
        .execute(&self.pool)
        .await?;
        self.thread(id).await
    }

    async fn mark_active_threads_stale(
        &self,
        updated_at: i64,
    ) -> anyhow::Result<Vec<NotificationThread>> {
        let ids = self.active_ids().await?;
        sqlx::query(
            r#"
            UPDATE notification_threads
            SET status = 'stale', updated_at = ?1, closed_at = ?1
            WHERE status = 'active'
            "#,
        )
        .bind(updated_at)
        .execute(&self.pool)
        .await?;

        let mut stale = Vec::new();
        for id in ids {
            stale.push(self.thread(id).await?);
        }
        Ok(stale)
    }
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
    pub app_identity: String,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub raw_body: String,
    pub urgency: Urgency,
    pub status: NotificationStatus,
    pub delivery_outcome: DeliveryOutcome,
    pub rule_reference: Option<String>,
    pub category: Option<String>,
    pub progress: Option<i64>,
    pub has_image_payload: bool,
    pub actions: Vec<NotificationAction>,
    pub resident: bool,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
}

impl NotificationThread {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            app_identity: row.try_get("app_identity")?,
            app_name: row.try_get("app_name")?,
            summary: row.try_get("summary")?,
            body: row.try_get("body")?,
            raw_body: row.try_get("raw_body")?,
            urgency: Urgency::from_str(&row.try_get::<String, _>("urgency")?),
            status: NotificationStatus::from_str(&row.try_get::<String, _>("status")?),
            delivery_outcome: DeliveryOutcome::from_str(
                &row.try_get::<String, _>("delivery_outcome")?,
            ),
            rule_reference: row.try_get("rule_reference")?,
            category: row.try_get("category")?,
            progress: row.try_get("progress")?,
            has_image_payload: false,
            actions: serde_json::from_str(&row.try_get::<String, _>("actions_json")?)?,
            resident: row.try_get::<i64, _>("resident")? != 0,
            revision: row.try_get("revision")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            closed_at: row.try_get("closed_at")?,
        })
    }
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

    fn urgency_key(&self) -> &'static str {
        urgency_key(self.urgency)
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

fn derive_app_identity(input: &NotificationInput) -> String {
    input
        .desktop_entry
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&input.app_name)
        .to_string()
}

fn plain_text_body(body: &str) -> String {
    let mut output = String::new();
    let mut chars = body.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut tag = String::new();
            for tag_ch in chars.by_ref() {
                if tag_ch == '>' {
                    break;
                }
                tag.push(tag_ch);
            }

            let tag_name = tag
                .trim()
                .trim_start_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_end_matches('/');

            if matches!(tag_name.to_ascii_lowercase().as_str(), "br" | "p" | "div") {
                push_newline(&mut output);
            }
            continue;
        }

        output.push(ch);
    }

    decode_basic_entities(&output)
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn push_newline(output: &mut String) {
    if !output.ends_with('\n') {
        output.push('\n');
    }
}

fn decode_basic_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}
