use crate::InternalNotificationId;
use crate::domain::has_static_raster_extension;
use crate::{
    BannerLifecycleChange, DeliveryOutcome, NotificationAction, NotificationBodyContent,
    NotificationBodyImageSource, NotificationBodyRun, NotificationBodySegment,
    NotificationBodyStyle, NotificationImageData, NotificationStatus, NotificationThread,
    NotificationVisualReference, Urgency,
};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

const APP_ICON_VISUAL_SIZE: u16 = 34;
const NOTIFICATION_IMAGE_VISUAL_SIZE: u16 = 58;
const BODY_IMAGE_VISUAL_HEIGHT: u32 = 48;
const BODY_PREVIEW_MAX_HEIGHT: u32 = 120;
const BANNER_MAX_WIDTH: f32 = 360.0;
// One ordinary Iced wheel step: leave room to fully reveal middle stack cards.
const BANNER_SCROLL_REVEAL_SPACE: f32 = 60.0;
const BANNER_ACTIVATION_TOKEN_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);

mod display;
mod icons;
mod images;

/// Renderer-owned lifecycle, independent of notification receipt or display acknowledgements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerRendererState {
    Starting,
    Ready,
    Stopped,
}

const RENDERER_STARTUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

pub(crate) async fn await_banner_renderer(presenter: &dyn BannerPresenter) -> anyhow::Result<()> {
    let Some(mut state) = presenter.renderer_state() else {
        return Ok(());
    };
    tokio::time::timeout(RENDERER_STARTUP_TIMEOUT, async {
        loop {
            anyhow::ensure!(
                state.has_changed().is_ok(),
                "banner renderer stopped before readiness"
            );
            match *state.borrow_and_update() {
                BannerRendererState::Ready => return Ok(()),
                BannerRendererState::Stopped => {
                    anyhow::bail!("banner renderer stopped before readiness")
                }
                BannerRendererState::Starting => {}
            }
            state
                .changed()
                .await
                .context("banner renderer stopped before readiness")?;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("banner renderer readiness timed out"))?
}

#[derive(Clone)]
pub struct BannerPresentationAcknowledgement {
    // None is a legacy whole-presentation observer; Some targets are independently
    // consumable display acknowledgements. Callbacks always run outside the lock.
    callbacks: Arc<Mutex<BTreeMap<Option<BannerTarget>, DisplayCallback>>>,
}

type DisplayCallback = Box<dyn FnOnce() + Send + 'static>;

impl BannerPresentationAcknowledgement {
    pub fn new(callback: impl FnOnce() + Send + 'static) -> Self {
        Self {
            callbacks: Arc::new(Mutex::new(BTreeMap::from([(
                None,
                Box::new(callback) as DisplayCallback,
            )]))),
        }
    }

    /// Bind display to the captured revisions. Unknown and duplicate targets do nothing.
    pub fn for_banners(
        targets: impl IntoIterator<Item = BannerTarget>,
        callback: impl Fn(BannerTarget) + Send + Sync + 'static,
    ) -> Self {
        let callback = Arc::new(callback);
        let callbacks = targets
            .into_iter()
            .map(|target| {
                let callback = callback.clone();
                (
                    Some(target),
                    Box::new(move || callback(target)) as DisplayCallback,
                )
            })
            .collect();
        Self {
            callbacks: Arc::new(Mutex::new(callbacks)),
        }
    }

    /// A synchronous presenter may acknowledge all targets after displaying the full projection.
    pub fn acknowledge(self) {
        let callbacks = std::mem::take(&mut *self.callbacks.lock().unwrap());
        for callback in callbacks.into_values() {
            callback();
        }
    }

    pub fn acknowledge_banner(&self, target: BannerTarget) {
        self.acknowledge_target(Some(target));
    }

    fn acknowledge_target(&self, target: Option<BannerTarget>) {
        let callback = self.callbacks.lock().unwrap().remove(&target);
        if let Some(callback) = callback {
            callback();
        }
    }

    fn is_complete(&self) -> bool {
        self.callbacks.lock().unwrap().is_empty()
    }

    fn is_pending(&self, target: BannerTarget) -> bool {
        self.callbacks.lock().unwrap().contains_key(&Some(target))
    }
}

impl std::fmt::Debug for BannerPresentationAcknowledgement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BannerPresentationAcknowledgement")
            .finish_non_exhaustive()
    }
}

pub trait BannerPresenter: Send + Sync {
    /// None denotes a synchronous presenter without an independently running renderer.
    /// A renderer must retain its sender and report terminal state even while idle.
    fn renderer_state(&self) -> Option<tokio::sync::watch::Receiver<BannerRendererState>> {
        None
    }

    fn replace_visible_banners(&self, banners: Vec<BannerView>) -> anyhow::Result<()>;

    fn replace_banner_presentation(&self, presentation: BannerPresentation) -> anyhow::Result<()> {
        self.replace_visible_banners(presentation.banners)
    }

    fn replace_banner_presentation_with_acknowledgement(
        &self,
        presentation: BannerPresentation,
        acknowledgement: BannerPresentationAcknowledgement,
    ) -> anyhow::Result<()> {
        self.replace_banner_presentation(presentation)?;
        acknowledgement.acknowledge();
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BannerPresentation {
    pub banners: Vec<BannerView>,
    pub overflow_count: usize,
    pub icon_settings: BannerIconSettings,
}

impl BannerPresentation {
    pub fn visible_banners(banners: Vec<BannerView>) -> Self {
        Self {
            banners,
            overflow_count: 0,
            icon_settings: BannerIconSettings::default(),
        }
    }
}

/// Static icon selection. Scale selects a source asset; it does not scale the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BannerIconSettings {
    pub theme: String,
    pub scale: u16,
}

impl Default for BannerIconSettings {
    fn default() -> Self {
        Self {
            theme: "hicolor".into(),
            scale: 1,
        }
    }
}

impl BannerIconSettings {
    pub fn new(theme: String, scale: u16) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !theme.is_empty()
                && theme.len() <= 255
                && theme.is_ascii()
                && !matches!(theme.as_str(), "." | "..")
                && !theme.bytes().any(|byte| byte.is_ascii_control()
                    || byte.is_ascii_whitespace()
                    || matches!(byte, b'/' | b'\\' | b',')),
            "invalid icon theme"
        );
        anyhow::ensure!(
            (1..=8).contains(&scale),
            "icon scale must be between 1 and 8"
        );
        Ok(Self { theme, scale })
    }
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

/// The content revision captured by a banner widget, preserved through async callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BannerTarget {
    pub notification_id: InternalNotificationId,
    pub revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BannerInteraction {
    Activate {
        target: BannerTarget,
        activation_token: Option<String>,
    },
    Dismiss(BannerTarget),
    InvokeAction {
        target: BannerTarget,
        action_key: String,
        activation_token: Option<String>,
    },
}

#[derive(Clone)]
pub struct IcedLayerShellBannerPresenter {
    updates: BannerUpdateSender,
    state: tokio::sync::watch::Sender<BannerRendererState>,
    // The UI has a separate guard: either UI exit or the last owner retires the transport.
    _owner: Arc<BannerUiLifetime>,
}

impl IcedLayerShellBannerPresenter {
    pub async fn start(interactions: mpsc::Sender<BannerInteraction>) -> anyhow::Result<Self> {
        let presenter = Self::start_with(interactions, run_banner_ui)?;
        await_banner_renderer(&presenter).await?;
        Ok(presenter)
    }

    fn start_with<F>(
        interactions: mpsc::Sender<BannerInteraction>,
        run_ui: F,
    ) -> anyhow::Result<Self>
    where
        F: FnOnce(
                u64,
                mpsc::Sender<BannerInteraction>,
                tokio::sync::watch::Sender<BannerRendererState>,
            ) -> iced_layershell::Result
            + Send
            + 'static,
    {
        let runtime_id = NEXT_BANNER_RUNTIME_ID.fetch_add(1, Ordering::Relaxed);
        let executor = tokio::runtime::Handle::try_current()
            .context("notification banner UI must be started from the daemon Tokio runtime")?;
        let (updates, receiver) = BannerUpdateSender::channel(executor);
        register_banner_update_receiver(runtime_id, receiver);

        let (state, _) = tokio::sync::watch::channel(BannerRendererState::Starting);
        let renderer_state = state.clone();

        let lifetime = BannerUiLifetime {
            runtime_id,
            updates: updates.clone(),
            state: renderer_state.clone(),
        };
        let owner = Arc::new(lifetime.clone());
        spawn_banner_ui_thread(runtime_id, interactions, move |runtime_id, interactions| {
            let _lifetime = lifetime;
            run_ui(runtime_id, interactions, renderer_state)
        })?;

        Ok(Self {
            updates,
            state,
            _owner: owner,
        })
    }

    pub fn layer_shell_settings(
        spec: BannerSurfaceSpec,
    ) -> iced_layershell::settings::LayerShellSettings {
        use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
        use iced_layershell::settings::StartMode;

        iced_layershell::settings::LayerShellSettings {
            anchor: match spec.output {
                BannerOutputSelection::FocusedWithFallback => {
                    Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right
                }
            },
            layer: match spec.layer {
                BannerLayer::Overlay => Layer::Overlay,
            },
            exclusive_zone: spec.exclusive_zone,
            size: Some((0, 0)),
            margin: (12, 12, 12, 12),
            keyboard_interactivity: match spec.keyboard_interactivity {
                BannerKeyboardInteractivity::None => KeyboardInteractivity::None,
            },
            start_mode: StartMode::Active,
            events_transparent: true,
        }
    }
}

impl BannerPresenter for IcedLayerShellBannerPresenter {
    fn renderer_state(&self) -> Option<tokio::sync::watch::Receiver<BannerRendererState>> {
        Some(self.state.subscribe())
    }

    fn replace_visible_banners(&self, banners: Vec<BannerView>) -> anyhow::Result<()> {
        self.replace_banner_presentation(BannerPresentation::visible_banners(banners))
    }

    fn replace_banner_presentation(&self, presentation: BannerPresentation) -> anyhow::Result<()> {
        self.replace_banner_presentation_with_acknowledgement(
            presentation,
            BannerPresentationAcknowledgement::new(|| {}),
        )
    }

    fn replace_banner_presentation_with_acknowledgement(
        &self,
        presentation: BannerPresentation,
        acknowledgement: BannerPresentationAcknowledgement,
    ) -> anyhow::Result<()> {
        self.updates.send(BannerRuntimeInput::Replace {
            presentation,
            acknowledgement,
        })
    }
}

// Complete current-state projections replace one pending value; intermediate states are not events.
type BannerWatchSender = tokio::sync::watch::Sender<Option<BannerRuntimeInput>>;
struct BannerUpdateReceiver {
    receiver: tokio::sync::watch::Receiver<Option<BannerRuntimeInput>>,
    executor: tokio::runtime::Handle,
    icon_roots: Arc<[PathBuf]>,
}

#[derive(Clone)]
struct BannerUpdateSender {
    // None is terminal. Publishing and clearing runtime-owning callbacks share this gate.
    sender: Arc<Mutex<Option<BannerWatchSender>>>,
}

impl BannerUpdateSender {
    fn channel(executor: tokio::runtime::Handle) -> (Self, BannerUpdateReceiver) {
        let (sender, receiver) = tokio::sync::watch::channel(None);
        (
            Self {
                sender: Arc::new(Mutex::new(Some(sender))),
            },
            BannerUpdateReceiver {
                receiver,
                executor,
                icon_roots: icons::search_roots().into(),
            },
        )
    }

    fn send(&self, input: BannerRuntimeInput) -> anyhow::Result<()> {
        let sender = self.sender.lock().unwrap();
        let sender = sender
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("banner renderer disconnected"))?;
        sender
            .send(Some(input))
            .map_err(|_| anyhow::anyhow!("banner renderer disconnected"))
    }

    fn close(&self) {
        // Retire the gate before dropping callbacks. A callback may hold the last presenter
        // owner, whose destructor closes this same transport.
        let sender = self.sender.lock().unwrap().take();
        if let Some(sender) = sender {
            sender.send_replace(None);
        }
    }
}

static NEXT_BANNER_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);
static BANNER_UPDATE_RECEIVERS: OnceLock<Mutex<HashMap<u64, BannerUpdateReceiver>>> =
    OnceLock::new();

#[derive(Clone)]
struct BannerUiLifetime {
    runtime_id: u64,
    updates: BannerUpdateSender,
    state: tokio::sync::watch::Sender<BannerRendererState>,
}

impl Drop for BannerUiLifetime {
    fn drop(&mut self) {
        // A queued acknowledgement can own the runtime, which owns this presenter. Clear it
        // even when startup fails before the UI takes the receiver, breaking that ownership cycle.
        self.updates.close();
        self.state.send_replace(BannerRendererState::Stopped);
        if let Some(receivers) = BANNER_UPDATE_RECEIVERS.get() {
            receivers.lock().unwrap().remove(&self.runtime_id);
        }
    }
}

#[derive(Clone, Debug)]
enum BannerRuntimeInput {
    Replace {
        presentation: BannerPresentation,
        acknowledgement: BannerPresentationAcknowledgement,
    },
}

#[derive(Clone, Debug)]
struct PendingBannerAcknowledgement {
    generation: u64,
    acknowledgement: BannerPresentationAcknowledgement,
}

type BannerInputRegion = Arc<Mutex<Option<iced::Rectangle<i32>>>>;

#[derive(Clone)]
struct BannerUiState {
    runtime_id: u64,
    presentation: BannerPresentation,
    presentation_generation: u64,
    pending_acknowledgement: Option<PendingBannerAcknowledgement>,
    input_region: BannerInputRegion,
    interactions: mpsc::Sender<BannerInteraction>,
    renderer_state: tokio::sync::watch::Sender<BannerRendererState>,
}

#[derive(Clone, Debug)]
enum BannerUiMessage {
    RendererReady,
    Shutdown,
    Runtime(BannerRuntimeInput),
    PresentationFrame(PendingBannerAcknowledgement),
    BannerFrame {
        rendered: PendingBannerAcknowledgement,
        target: BannerTarget,
    },
    LayoutFrame {
        generation: u64,
        size: iced::Size,
        viewport: iced::Size,
        rendered: Option<PendingBannerAcknowledgement>,
    },
    ActivateRequested(BannerTarget),
    Activate {
        target: BannerTarget,
        activation_token: Option<String>,
    },
    Dismiss(BannerTarget),
    InvokeActionRequested(BannerTarget, String),
    InvokeAction {
        target: BannerTarget,
        action_key: String,
        activation_token: Option<String>,
    },
    RequestActivationToken {
        app_identity: String,
        callback: iced_layershell::actions::ActivationTokenCallback,
    },
    SetInputRegion(BannerInputRegion),
}

impl TryInto<iced_layershell::actions::LayerShellCustomActionWithId> for BannerUiMessage {
    type Error = Self;

    fn try_into(
        self,
    ) -> Result<iced_layershell::actions::LayerShellCustomActionWithId, Self::Error> {
        use iced_layershell::actions::{LayerShellCustomAction, LayerShellCustomActionWithId};

        match self {
            Self::RequestActivationToken {
                app_identity,
                callback,
            } => Ok(LayerShellCustomActionWithId::new(
                None,
                LayerShellCustomAction::RequestActivationToken {
                    app_id: app_identity,
                    callback,
                },
            )),
            Self::SetInputRegion(latest) => Ok(LayerShellCustomActionWithId::new(
                None,
                LayerShellCustomAction::SetInputRegion(
                    iced_layershell::actions::ActionCallback::new(move |region| {
                        // Layer-shell tasks may run out of order. Every queued action
                        // resolves the current measured region at execution time.
                        let bounds = latest.lock().unwrap().unwrap_or_default();
                        // The adapter reuses a region across resizes. Clear its entire
                        // old extent, including points outside the current viewport.
                        region.subtract(0, 0, i32::MAX, i32::MAX);
                        if bounds.width > 0 && bounds.height > 0 {
                            region.add(bounds.x, bounds.y, bounds.width, bounds.height);
                        }
                    }),
                ),
            )),
            message => Err(message),
        }
    }
}

fn register_banner_update_receiver(runtime_id: u64, receiver: BannerUpdateReceiver) {
    BANNER_UPDATE_RECEIVERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .insert(runtime_id, receiver);
}

fn take_banner_update_receiver(runtime_id: u64) -> Option<BannerUpdateReceiver> {
    BANNER_UPDATE_RECEIVERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .remove(&runtime_id)
}

fn banner_update_stream(
    runtime_id: &u64,
) -> impl iced::futures::Stream<Item = BannerUiMessage> + use<> {
    use iced::futures::StreamExt;
    iced::futures::stream::iter(take_banner_update_receiver(*runtime_id).map(banner_updates))
        .flatten()
        .map(BannerUiMessage::Runtime)
        .chain(iced::futures::stream::once(async {
            BannerUiMessage::Shutdown
        }))
}

fn banner_updates(
    receiver: BannerUpdateReceiver,
) -> impl iced::futures::Stream<Item = BannerRuntimeInput> {
    iced::futures::stream::unfold(receiver, |state| async move {
        let BannerUpdateReceiver {
            mut receiver,
            executor,
            icon_roots,
        } = state;
        'updates: loop {
            receiver.changed().await.ok()?;
            loop {
                let BannerRuntimeInput::Replace {
                    presentation,
                    acknowledgement,
                } = receiver.borrow_and_update().clone()?;
                let latest = receiver.clone();
                let prepared = tokio::select! {
                    biased;
                    changed = receiver.changed() => {
                        changed.ok()?;
                        continue;
                    }
                    prepared = images::prepare(presentation, &executor, icon_roots.clone(), move || latest.has_changed().unwrap_or(true)) => prepared,
                };
                match receiver.has_changed() {
                    Ok(true) => continue,
                    Err(_) => return None,
                    Ok(false) => {}
                }
                let Some(presentation) = prepared else {
                    continue 'updates;
                };
                return Some((
                    BannerRuntimeInput::Replace {
                        presentation,
                        acknowledgement,
                    },
                    BannerUpdateReceiver {
                        receiver,
                        executor,
                        icon_roots,
                    },
                ));
            }
        }
    })
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
            if run_ui(runtime_id, interactions).is_err() {
                eprintln!("level=error component=presentation event=renderer_failed");
            }
        })?;
    Ok(())
}

fn run_banner_ui(
    runtime_id: u64,
    interactions: mpsc::Sender<BannerInteraction>,
    renderer_state: tokio::sync::watch::Sender<BannerRendererState>,
) -> iced_layershell::Result {
    use iced_layershell::settings::Settings;

    iced_layershell::disable_clipboard();
    iced_layershell::daemon(
        move || BannerUiState {
            runtime_id,
            presentation: BannerPresentation::default(),
            presentation_generation: 0,
            pending_acknowledgement: None,
            input_region: Default::default(),
            interactions: interactions.clone(),
            renderer_state: renderer_state.clone(),
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
    let readiness = if *state.renderer_state.borrow() == BannerRendererState::Starting {
        iced::window::frames().map(|_| BannerUiMessage::RendererReady)
    } else {
        iced::Subscription::none()
    };
    iced::Subscription::batch([
        readiness,
        iced::Subscription::run_with(state.runtime_id, banner_update_stream),
    ])
}

fn banner_ui_update(
    state: &mut BannerUiState,
    message: BannerUiMessage,
) -> iced::Task<BannerUiMessage> {
    match message {
        BannerUiMessage::Shutdown => iced::exit(),
        BannerUiMessage::RendererReady => {
            state.renderer_state.send_if_modified(|value| {
                if *value == BannerRendererState::Starting {
                    *value = BannerRendererState::Ready;
                    true
                } else {
                    false
                }
            });
            iced::Task::none()
        }
        BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
            presentation,
            acknowledgement,
        }) => {
            state.presentation = presentation;
            state.presentation_generation = state
                .presentation_generation
                .checked_add(1)
                .expect("banner presentation generation exhausted");
            state.pending_acknowledgement = Some(PendingBannerAcknowledgement {
                generation: state.presentation_generation,
                acknowledgement,
            });
            iced::Task::none()
        }
        BannerUiMessage::LayoutFrame {
            generation,
            size,
            viewport,
            rendered,
        } => {
            if let Some(rendered) = rendered {
                let _ = banner_ui_update(state, BannerUiMessage::PresentationFrame(rendered));
            }
            if generation != state.presentation_generation {
                return iced::Task::none();
            }
            let width = size.width.min(viewport.width);
            let x = (viewport.width - width).floor();
            let bounds = iced::Rectangle {
                x: x as i32,
                y: 0,
                width: (viewport.width - x).ceil() as i32,
                height: size.height.min(viewport.height).ceil() as i32,
            };
            let mut current = state.input_region.lock().unwrap();
            if *current == Some(bounds) {
                iced::Task::none()
            } else {
                *current = Some(bounds);
                iced::Task::done(BannerUiMessage::SetInputRegion(state.input_region.clone()))
            }
        }
        BannerUiMessage::PresentationFrame(rendered) => {
            acknowledge_banner_frame(state, rendered, None);
            iced::Task::none()
        }
        BannerUiMessage::BannerFrame { rendered, target } => {
            acknowledge_banner_frame(state, rendered, Some(target));
            iced::Task::none()
        }
        BannerUiMessage::ActivateRequested(target) => {
            let Some(banner) = current_banner(state, target) else {
                return iced::Task::none();
            };
            if banner.default_action_key.is_none() {
                return iced::Task::done(BannerUiMessage::Dismiss(target));
            }
            activation_token_task(banner.app_identity.clone(), move |activation_token| {
                BannerUiMessage::Activate {
                    target,
                    activation_token,
                }
            })
        }
        BannerUiMessage::Activate {
            target,
            activation_token,
        } => {
            state
                .interactions
                .send(BannerInteraction::Activate {
                    target,
                    activation_token,
                })
                .ok();
            iced::Task::none()
        }
        BannerUiMessage::Dismiss(target) => {
            state
                .interactions
                .send(BannerInteraction::Dismiss(target))
                .ok();
            iced::Task::none()
        }
        BannerUiMessage::InvokeActionRequested(target, action_key) => {
            let Some(banner) = current_banner(state, target) else {
                return iced::Task::none();
            };
            if !banner.actions.iter().any(|action| action.key == action_key) {
                return iced::Task::none();
            }
            activation_token_task(banner.app_identity.clone(), move |activation_token| {
                BannerUiMessage::InvokeAction {
                    target,
                    action_key,
                    activation_token,
                }
            })
        }
        BannerUiMessage::InvokeAction {
            target,
            action_key,
            activation_token,
        } => {
            state
                .interactions
                .send(BannerInteraction::InvokeAction {
                    target,
                    action_key,
                    activation_token,
                })
                .ok();
            iced::Task::none()
        }
        _ => iced::Task::none(),
    }
}

fn acknowledge_banner_frame(
    state: &mut BannerUiState,
    rendered: PendingBannerAcknowledgement,
    target: Option<BannerTarget>,
) {
    // A frame owns its captured acknowledgement, including when a newer projection
    // arrives before dispatch. It must not clear that newer projection's callbacks.
    rendered.acknowledgement.acknowledge_target(target);
    if rendered.acknowledgement.is_complete()
        && state
            .pending_acknowledgement
            .as_ref()
            .is_some_and(|pending| pending.generation == rendered.generation)
    {
        state.pending_acknowledgement = None;
    }
}

fn current_banner(state: &BannerUiState, target: BannerTarget) -> Option<&BannerView> {
    state
        .presentation
        .banners
        .iter()
        .find(|banner| banner.target() == target)
}

fn activation_token_task(
    app_identity: String,
    on_token: impl FnOnce(Option<String>) -> BannerUiMessage + Send + 'static,
) -> iced::Task<BannerUiMessage> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let sender = Arc::new(Mutex::new(Some(sender)));
    let callback =
        iced_layershell::actions::ActivationTokenCallback::new(move |activation_token| {
            if let Some(sender) = sender.lock().unwrap().take() {
                let _ = sender.send(activation_token);
            }
        });

    iced::Task::batch([
        iced::Task::done(BannerUiMessage::RequestActivationToken {
            app_identity,
            callback,
        }),
        iced::Task::perform(
            async move {
                receiver
                    .recv_timeout(BANNER_ACTIVATION_TOKEN_TIMEOUT)
                    .ok()
                    .flatten()
            },
            on_token,
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::futures::StreamExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    mod icons;
    mod images;
    mod layout;

    // Renderer-consumer fixtures already supply the running consumer themselves.
    fn test_presenter(updates: BannerUpdateSender) -> IcedLayerShellBannerPresenter {
        let state = tokio::sync::watch::channel(BannerRendererState::Ready).0;
        IcedLayerShellBannerPresenter {
            _owner: Arc::new(BannerUiLifetime {
                runtime_id: NEXT_BANNER_RUNTIME_ID.fetch_add(1, Ordering::Relaxed),
                updates: updates.clone(),
                state: state.clone(),
            }),
            updates,
            state,
        }
    }

    #[tokio::test]
    async fn renderer_exit_before_readiness_does_not_recover_history() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("history.db");
        let seed = crate::Daemon::open(crate::DaemonOptions::headless(database.clone()))
            .await
            .unwrap();
        seed.receive_notification(crate::NotificationInput {
            app_name: "Mail".into(),
            summary: "Still active".into(),
            ..Default::default()
        })
        .await
        .unwrap();
        let before = seed.snapshot().await.unwrap();
        let (interactions, _) = mpsc::channel();
        let presenter =
            IcedLayerShellBannerPresenter::start_with(interactions, |_, _, _| Ok(())).unwrap();
        let result =
            crate::Daemon::open(crate::DaemonOptions::graphical(database, presenter)).await;
        assert!(
            result.is_err(),
            "daemon accepted a renderer that exited before readiness"
        );
        assert_eq!(
            seed.snapshot().await.unwrap(),
            before,
            "failed renderer startup recovered live history"
        );
    }

    #[tokio::test]
    async fn idle_renderer_exit_degrades_status_without_disabling_notification_state() {
        let temp = tempfile::tempdir().unwrap();
        let (interactions, _) = mpsc::channel();
        let (exit_ui, exit_requested) = mpsc::channel();
        let presenter =
            IcedLayerShellBannerPresenter::start_with(interactions, move |_, _, renderer_state| {
                let mut ui = banner_ui_test_state();
                ui.renderer_state = renderer_state;
                let _task = banner_ui_update(&mut ui, BannerUiMessage::RendererReady);
                let _ = exit_requested.recv_timeout(Duration::from_secs(5));
                Ok(())
            })
            .unwrap();
        let mut renderer_state = presenter.renderer_state().unwrap();
        let daemon = crate::Daemon::open(crate::DaemonOptions::graphical(
            temp.path().join("history.db"),
            presenter,
        ))
        .await
        .unwrap();
        let control = crate::NotifyControl::new(daemon.clone());
        let crate::ControlReply::Status(status) = control
            .execute(crate::ControlCommand::Status)
            .await
            .unwrap()
        else {
            panic!("expected control status");
        };
        assert!(status.presentation_error.is_none());

        exit_ui.send(()).unwrap();
        tokio::time::timeout(
            Duration::from_secs(1),
            renderer_state.wait_for(|state| *state == BannerRendererState::Stopped),
        )
        .await
        .unwrap()
        .unwrap();
        // No receipt or presentation update occurs between renderer exit and this query.
        let crate::ControlReply::Status(status) = control
            .execute(crate::ControlCommand::Status)
            .await
            .unwrap()
        else {
            panic!("expected control status");
        };
        assert_eq!(
            status.presentation_error.as_deref(),
            Some("banner renderer stopped; restart the daemon to restore banners")
        );

        let receipt = daemon
            .receive_notification(crate::NotificationInput {
                app_name: "Mail".into(),
                summary: "Retained while banners are offline".into(),
                ..Default::default()
            })
            .await
            .expect("renderer failure must not reject committed receipts");
        let crate::ControlReply::NotificationCenterProjection(center) = control
            .execute(crate::ControlCommand::NotificationCenterProjection)
            .await
            .unwrap()
        else {
            panic!("expected center projection");
        };
        assert!(
            center
                .groups
                .iter()
                .flat_map(|group| &group.items)
                .any(|item| { item.notification_id == receipt.notification_id })
        );
        assert_eq!(daemon.presentation_error(), status.presentation_error);
        daemon
            .close_notification(receipt.notification_id)
            .await
            .unwrap();
        assert_eq!(daemon.presentation_error(), status.presentation_error);
    }

    #[tokio::test]
    async fn cancelled_renderer_startup_retires_transport_and_rejects_late_readiness() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("history.db");
        let (interactions, _) = mpsc::channel();
        let (release_ui, released) = mpsc::channel();
        let (late_state, observed_late_state) = tokio::sync::oneshot::channel();
        let presenter =
            IcedLayerShellBannerPresenter::start_with(interactions, move |_, _, renderer_state| {
                let _ = released.recv_timeout(Duration::from_secs(5));
                let mut ui = banner_ui_test_state();
                ui.renderer_state = renderer_state;
                let _task = banner_ui_update(&mut ui, BannerUiMessage::RendererReady);
                let _ = late_state.send(*ui.renderer_state.borrow());
                Ok(())
            })
            .unwrap();
        let state = presenter.renderer_state().unwrap();
        let mut startup = Box::pin(crate::Daemon::open(crate::DaemonOptions::graphical(
            database.clone(),
            presenter,
        )));
        assert!(iced::futures::poll!(&mut startup).is_pending());
        assert!(
            !database.exists(),
            "startup opened history before readiness"
        );
        drop(startup);
        assert_eq!(
            *state.borrow(),
            BannerRendererState::Stopped,
            "cancelling startup left the renderer transport alive"
        );
        release_ui.send(()).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), observed_late_state)
                .await
                .unwrap()
                .unwrap(),
            BannerRendererState::Stopped,
            "a late redraw resurrected a cancelled renderer"
        );
    }

    #[tokio::test]
    async fn last_presenter_owner_requests_renderer_shutdown() {
        let (updates, receiver) = BannerUpdateSender::channel(tokio::runtime::Handle::current());
        let presenter = test_presenter(updates);
        let retained = presenter.clone();
        let state = presenter.renderer_state().unwrap();
        let runtime_id = presenter._owner.runtime_id;
        register_banner_update_receiver(runtime_id, receiver);
        let mut messages = Box::pin(banner_update_stream(&runtime_id));
        drop(presenter);
        assert!(iced::futures::poll!(messages.next()).is_pending());
        assert_eq!(*state.borrow(), BannerRendererState::Ready);

        drop(retained);
        assert_eq!(*state.borrow(), BannerRendererState::Stopped);
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), messages.next())
                .await
                .unwrap(),
            Some(BannerUiMessage::Shutdown)
        ));
    }

    #[tokio::test]
    async fn renderer_readiness_timeout_retires_startup_without_opening_history() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("history.db");
        let (interactions, _) = mpsc::channel();
        let (release_ui, released) = mpsc::channel::<()>();
        let presenter = IcedLayerShellBannerPresenter::start_with(interactions, move |_, _, _| {
            let _ = released.recv_timeout(Duration::from_secs(15));
            Ok(())
        })
        .unwrap();
        let state = presenter.renderer_state().unwrap();
        let started = tokio::time::Instant::now();
        let result =
            crate::Daemon::open(crate::DaemonOptions::graphical(database.clone(), presenter)).await;
        assert_eq!(
            result.err().expect("unready renderer accepted").to_string(),
            "banner renderer readiness timed out"
        );
        assert!(started.elapsed() >= Duration::from_secs(10));
        assert!(!database.exists());
        assert_eq!(*state.borrow(), BannerRendererState::Stopped);
        drop(release_ui);
    }

    #[tokio::test]
    async fn delayed_renderer_readiness_gates_history_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("history.db");
        let seed = crate::Daemon::open(crate::DaemonOptions::headless(database.clone()))
            .await
            .unwrap();
        seed.receive_notification(crate::NotificationInput {
            app_name: "Mail".into(),
            summary: "Live until readiness".into(),
            ..Default::default()
        })
        .await
        .unwrap();
        let before = seed.snapshot().await.unwrap();
        let (interactions, _) = mpsc::channel();
        let (ready_ui, ready_requested) = mpsc::channel();
        let (exit_ui, exit_requested) = mpsc::channel::<()>();
        let presenter =
            IcedLayerShellBannerPresenter::start_with(interactions, move |_, _, renderer_state| {
                if ready_requested
                    .recv_timeout(Duration::from_secs(15))
                    .is_ok()
                {
                    let mut ui = banner_ui_test_state();
                    ui.renderer_state = renderer_state;
                    let _task = banner_ui_update(&mut ui, BannerUiMessage::RendererReady);
                    let _ = exit_requested.recv_timeout(Duration::from_secs(15));
                }
                Ok(())
            })
            .unwrap();
        let mut startup = Box::pin(crate::Daemon::open(crate::DaemonOptions::graphical(
            database, presenter,
        )));
        assert!(iced::futures::poll!(&mut startup).is_pending());
        assert_eq!(seed.snapshot().await.unwrap(), before);
        ready_ui.send(()).unwrap();
        let daemon = tokio::time::timeout(Duration::from_secs(3), startup)
            .await
            .unwrap()
            .unwrap();
        let recovered = daemon.snapshot().await.unwrap();
        assert!(recovered.active.is_empty());
        assert_eq!(recovered.history.len(), 1);
        assert_eq!(recovered.history[0].status, NotificationStatus::Stale);
        assert!(daemon.presentation_error().is_none());
        drop(exit_ui);
    }

    fn banner_ui_test_state() -> BannerUiState {
        let (interactions, _interaction_receiver) = mpsc::channel();
        BannerUiState {
            runtime_id: 0,
            presentation: BannerPresentation::default(),
            presentation_generation: 0,
            pending_acknowledgement: None,
            input_region: Default::default(),
            interactions,
            renderer_state: tokio::sync::watch::channel(BannerRendererState::Starting).0,
        }
    }

    fn queue_banner_ui_presentation(
        state: &mut BannerUiState,
        notification_id: i64,
        acknowledged: Arc<AtomicBool>,
    ) {
        let _task = banner_ui_update(
            state,
            BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
                presentation: BannerPresentation::visible_banners(vec![banner_view(
                    notification_id,
                    "Queued",
                )]),
                acknowledgement: BannerPresentationAcknowledgement::new(move || {
                    acknowledged.store(true, Ordering::SeqCst);
                }),
            }),
        );
    }

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

    #[tokio::test]
    async fn publication_cannot_refill_a_closing_renderers_callback_slot() {
        let (interactions, _) = mpsc::channel();
        let (exit_ui, exit_requested) = mpsc::channel();
        let presenter = IcedLayerShellBannerPresenter::start_with(interactions, move |_, _, _| {
            exit_requested.recv_timeout(Duration::from_secs(2)).unwrap();
            Ok(())
        })
        .unwrap();
        let initial = Arc::new(AtomicBool::new(false));
        let initial_lifetime = Arc::downgrade(&initial);
        presenter
            .replace_banner_presentation_with_acknowledgement(
                BannerPresentation::default(),
                BannerPresentationAcknowledgement::new(move || {
                    initial.store(true, Ordering::SeqCst);
                }),
            )
            .unwrap();

        // Pause renderer retirement between clearing pending content and removing the receiver.
        // The UI runs on a separate OS thread, so no executor work is needed while holding this lock.
        let registry = BANNER_UPDATE_RECEIVERS.get().unwrap().lock().unwrap();
        exit_ui.send(()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while initial_lifetime.upgrade().is_some() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        let cleanup_started = initial_lifetime.upgrade().is_none();
        let late = Arc::new(AtomicBool::new(false));
        let late_lifetime = Arc::downgrade(&late);
        let result = presenter.replace_banner_presentation_with_acknowledgement(
            BannerPresentation::default(),
            BannerPresentationAcknowledgement::new(move || {
                late.store(true, Ordering::SeqCst);
            }),
        );
        drop(registry);

        assert!(
            cleanup_started,
            "renderer did not begin callback retirement"
        );
        assert!(
            result.is_err(),
            "publication was admitted after renderer retirement began"
        );
        assert!(
            late_lifetime.upgrade().is_none(),
            "a late callback retained the stopped runtime"
        );
    }

    #[tokio::test]
    async fn renderer_exit_before_subscription_releases_callbacks_and_closes_transport() {
        let (interactions, _) = mpsc::channel();
        let (exit_ui, exit_requested) = mpsc::channel();
        let (finished, finished_receiver) = tokio::sync::oneshot::channel();
        let presenter = IcedLayerShellBannerPresenter::start_with(interactions, move |_, _, _| {
            // A renderer may fail before iced takes ownership of the update subscription.
            exit_requested.recv_timeout(Duration::from_secs(2)).unwrap();
            finished.send(()).unwrap();
            Ok(())
        })
        .unwrap();
        let callback = Arc::new(AtomicBool::new(false));
        let transport = presenter
            .updates
            .sender
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .clone();
        let callback_lifetime = Arc::downgrade(&callback);
        presenter
            .replace_banner_presentation_with_acknowledgement(
                BannerPresentation::default(),
                BannerPresentationAcknowledgement::new(move || {
                    callback.store(true, Ordering::SeqCst);
                }),
            )
            .unwrap();
        exit_ui.send(()).unwrap();
        finished_receiver.await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), transport.closed())
            .await
            .expect("renderer exit must close even an unclaimed subscription");
        assert!(
            callback_lifetime.upgrade().is_none(),
            "renderer exit retained a runtime callback"
        );
        assert!(presenter.replace_visible_banners(vec![]).is_err());
    }

    #[tokio::test]
    async fn paused_renderer_receives_latest_state_after_a_burst_without_lost_closures() {
        let (updates, receiver) = BannerUpdateSender::channel(tokio::runtime::Handle::current());
        let mut receiver = Box::pin(banner_updates(receiver));
        let presenter = test_presenter(updates);
        let superseded_acknowledged = Arc::new(AtomicBool::new(false));
        for id in 1..=130 {
            let acknowledged = superseded_acknowledged.clone();
            presenter
                .replace_banner_presentation_with_acknowledgement(
                    BannerPresentation::visible_banners(vec![banner_view(id, "Superseded")]),
                    BannerPresentationAcknowledgement::new(move || {
                        acknowledged.store(true, Ordering::SeqCst);
                    }),
                )
                .expect("a slow renderer must not reject current-state updates");
        }
        let final_acknowledged = Arc::new(AtomicBool::new(false));
        let acknowledged = final_acknowledged.clone();
        presenter
            .replace_banner_presentation_with_acknowledgement(
                BannerPresentation::default(),
                BannerPresentationAcknowledgement::new(move || {
                    acknowledged.store(true, Ordering::SeqCst);
                }),
            )
            .unwrap();

        let mut state = banner_ui_test_state();
        let input = receiver.next().await.unwrap();
        let _ = banner_ui_update(&mut state, BannerUiMessage::Runtime(input));
        assert!(
            state.presentation.banners.is_empty(),
            "a closed banner was resurrected by queued state"
        );
        assert!(!superseded_acknowledged.load(Ordering::SeqCst));
        assert!(!final_acknowledged.load(Ordering::SeqCst));
        let frame = state.pending_acknowledgement.clone().unwrap();
        let _ = banner_ui_update(&mut state, BannerUiMessage::PresentationFrame(frame));
        assert!(final_acknowledged.load(Ordering::SeqCst));
        assert!(!superseded_acknowledged.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn iced_presenter_acknowledges_only_after_ui_consumes_presentation() {
        let (updates, receiver) = BannerUpdateSender::channel(tokio::runtime::Handle::current());
        let mut receiver = Box::pin(banner_updates(receiver));
        let presenter = test_presenter(updates);
        let acknowledged = Arc::new(AtomicBool::new(false));
        let acknowledgement_state = acknowledged.clone();
        presenter
            .replace_banner_presentation_with_acknowledgement(
                BannerPresentation::visible_banners(vec![banner_view(42, "Displayed")]),
                BannerPresentationAcknowledgement::new(move || {
                    acknowledgement_state.store(true, Ordering::SeqCst);
                }),
            )
            .unwrap();
        assert!(!acknowledged.load(Ordering::SeqCst));

        let input = receiver.next().await.unwrap();
        assert!(!acknowledged.load(Ordering::SeqCst));
        let mut state = banner_ui_test_state();
        let _task = banner_ui_update(&mut state, BannerUiMessage::Runtime(input));

        assert!(!acknowledged.load(Ordering::SeqCst));
        let rendered = state.pending_acknowledgement.clone().unwrap();
        let _task = banner_ui_update(&mut state, BannerUiMessage::PresentationFrame(rendered));

        assert!(acknowledged.load(Ordering::SeqCst));
        assert_eq!(state.presentation.banners[0].notification_id.get(), 42);
    }

    #[test]
    fn undisplayed_banner_updates_release_superseded_acknowledgements() {
        let mut state = banner_ui_test_state();
        let mut retired = Vec::new();
        for id in 1..=130 {
            let acknowledgement = Arc::new(AtomicBool::new(false));
            retired.push(Arc::downgrade(&acknowledgement));
            queue_banner_ui_presentation(&mut state, id, acknowledgement);
        }
        assert!(
            retired[..129].iter().all(|ack| ack.upgrade().is_none()),
            "undisplayed updates must not retain old runtime callbacks indefinitely"
        );
        assert!(retired[129].upgrade().is_some());
    }

    #[test]
    fn presentation_frame_does_not_acknowledge_a_superseded_update() {
        let mut state = banner_ui_test_state();
        let first_acknowledged = Arc::new(AtomicBool::new(false));
        let latest_acknowledged = Arc::new(AtomicBool::new(false));

        for (notification_id, acknowledged) in [
            (1, first_acknowledged.clone()),
            (2, latest_acknowledged.clone()),
        ] {
            queue_banner_ui_presentation(&mut state, notification_id, acknowledged);
        }

        let rendered = state.pending_acknowledgement.clone().unwrap();
        let _task = banner_ui_update(&mut state, BannerUiMessage::PresentationFrame(rendered));

        assert!(!first_acknowledged.load(Ordering::SeqCst));
        assert!(latest_acknowledged.load(Ordering::SeqCst));
    }

    #[test]
    fn presentation_frame_acknowledges_its_generation_after_a_newer_update_arrives() {
        let mut state = banner_ui_test_state();
        let first_acknowledged = Arc::new(AtomicBool::new(false));
        let latest_acknowledged = Arc::new(AtomicBool::new(false));
        let mut queued_frames = Vec::new();

        for (notification_id, acknowledged) in [
            (1, first_acknowledged.clone()),
            (2, latest_acknowledged.clone()),
        ] {
            queue_banner_ui_presentation(&mut state, notification_id, acknowledged);
            queued_frames.push(state.pending_acknowledgement.clone().unwrap());
        }

        let _task = banner_ui_update(
            &mut state,
            BannerUiMessage::PresentationFrame(queued_frames.remove(0)),
        );

        assert!(first_acknowledged.load(Ordering::SeqCst));
        assert!(!latest_acknowledged.load(Ordering::SeqCst));
        let _task = banner_ui_update(
            &mut state,
            BannerUiMessage::PresentationFrame(queued_frames.remove(0)),
        );
        assert!(latest_acknowledged.load(Ordering::SeqCst));
    }

    #[test]
    fn banner_stack_identity_uses_notification_id() {
        let first = banner_view(42, "Starting");
        let replacement = banner_view(42, "Complete");

        assert_eq!(
            banner_stack_item_key(&replacement),
            banner_stack_item_key(&first)
        );
    }

    #[test]
    fn notification_image_without_app_icon_replaces_initials_visual() {
        let mut banner = banner_view(42, "Screenshot");
        banner.image = Some(test_banner_visual());

        assert_eq!(
            banner_header_visual_layout(&banner),
            BannerHeaderVisualLayout {
                primary: BannerPrimaryVisual::NotificationImage,
                has_secondary_notification_image: false,
            }
        );
    }

    #[test]
    fn app_icon_and_notification_image_stay_distinct() {
        let mut banner = banner_view(42, "Screenshot");
        banner.app_icon = Some(test_banner_visual());
        banner.image = Some(test_banner_visual());

        assert_eq!(
            banner_header_visual_layout(&banner),
            BannerHeaderVisualLayout {
                primary: BannerPrimaryVisual::AppIcon,
                has_secondary_notification_image: true,
            }
        );
    }

    #[test]
    fn image_only_primary_visual_uses_app_icon_size() {
        assert_eq!(
            primary_banner_visual_size(BannerPrimaryVisual::NotificationImage),
            APP_ICON_VISUAL_SIZE
        );
    }

    fn banner_view(notification_id: i64, body: &str) -> BannerView {
        BannerView {
            notification_id: InternalNotificationId::new(notification_id).unwrap(),
            revision: 1,
            app_identity: "downloader".to_string(),
            app_name: "Downloader".to_string(),
            app_icon: None,
            summary: "Download".to_string(),
            body: body.to_string(),
            body_content: NotificationBodyContent::plain(body),
            body_segments: vec![BannerBodySegment::plain_text(body)],
            urgency: Urgency::Normal,
            progress: None,
            image: None,
            default_action_key: None,
            actions: Vec::new(),
            placement: BannerPlacement::TopRightFocusedOutputWithFallback,
            focus_behavior: BannerFocusBehavior::DoesNotTakeFocus,
        }
    }

    fn test_banner_visual() -> BannerVisual {
        BannerVisual::Rgba {
            width: 1,
            height: 1,
            pixels: vec![255, 0, 0, 255],
        }
    }
}

fn banner_ui_view(
    state: &BannerUiState,
    _window: iced::window::Id,
) -> iced::Element<'_, BannerUiMessage> {
    use iced::Length;
    use iced::widget::{container, responsive, scrollable, sensor};

    responsive(move |viewport| {
        // The compositor owns the viewport; natural content measurement only
        // determines the input region. Whole-presentation observers wait for the stack;
        // each card independently observes its own full visibility below.
        let frame = move |size: iced::Size| BannerUiMessage::LayoutFrame {
            generation: state.presentation_generation,
            size,
            viewport,
            rendered: (size.width <= viewport.width && size.height <= viewport.height)
                .then(|| state.pending_acknowledgement.clone())
                .flatten(),
        };
        let measured = sensor(banner_stack_widget(state, viewport.height))
            .key((state.presentation_generation, viewport))
            // Sensor keys retrigger on_show, not an on_resize-only subscription.
            // Measure even when scrolling has moved both stack edges out of view.
            .anticipate(f32::INFINITY)
            .on_show(frame)
            .on_resize(frame);
        let stack = scrollable(measured)
            .width(viewport.width.min(BANNER_MAX_WIDTH))
            .height(Length::Fill);
        container(stack)
            .align_right(Length::Fill)
            .height(Length::Fill)
            .into()
    })
    .into()
}

fn banner_stack_widget(
    state: &BannerUiState,
    viewport_height: f32,
) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{container, keyed_column, scrollable};
    use iced::{Alignment, Length};

    let has_siblings =
        state.presentation.banners.len() > 1 || state.presentation.overflow_count > 0;
    let card_height = if has_siblings {
        (viewport_height - BANNER_SCROLL_REVEAL_SPACE).max(viewport_height / 2.0)
    } else {
        viewport_height
    };
    let mut stack = keyed_column![]
        .spacing(BANNER_STACK_SPACING)
        .width(Length::Fill)
        .align_items(Alignment::End);
    for banner in &state.presentation.banners {
        stack = stack.push(
            banner_stack_item_key(banner),
            display::observe(
                // Display means the bounded card viewport is visible, not every
                // line of oversized content has been read (ADR 0046).
                container(scrollable(notification_banner_widget(banner)).width(Length::Fill))
                    .width(Length::Fill)
                    .max_height(card_height)
                    .into(),
                banner.target(),
                state.pending_acknowledgement.as_ref(),
            ),
        );
    }
    if state.presentation.overflow_count > 0 {
        stack = stack.push(
            BannerStackItemKey::Overflow,
            overflow_count_banner_widget(state.presentation.overflow_count),
        );
    }

    container(stack)
        // A multi-item stack can overflow independently of an individual card.
        // Keep its 10px rail clear of the card's own rail and action hit regions.
        .padding(iced::Padding {
            right: if has_siblings { 12.0 } else { 0.0 },
            ..iced::Padding::ZERO
        })
        .width(Length::Fill)
        .height(Length::Shrink)
        .into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BannerStackItemKey {
    Notification(InternalNotificationId),
    Overflow,
}

fn banner_stack_item_key(banner: &BannerView) -> BannerStackItemKey {
    BannerStackItemKey::Notification(banner.notification_id)
}

fn notification_banner_widget(banner: &BannerView) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{button, column, container, progress_bar, row, text};
    use iced::{Alignment, Length};

    let content = column![
        text(banner_title(banner))
            .size(15)
            .width(Length::Fill)
            .line_height(iced::Pixels(22.0))
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        notification_body_widget(banner),
        text(&banner.app_name)
            .size(11)
            .width(Length::Fill)
            .style(metadata_text_style),
    ]
    .spacing(1)
    .width(Length::Fill);

    let activate = button(container(content).width(Length::Fill).clip(true))
        .padding(0)
        .width(Length::Fill)
        .style(content_button_style)
        .on_press(BannerUiMessage::ActivateRequested(banner.target()));
    let dismiss = close_button(banner.target());

    let visual_layout = banner_header_visual_layout(banner);
    let mut header = row![primary_banner_visual(banner, visual_layout.primary),]
        .spacing(10)
        .align_y(Alignment::Start);
    if visual_layout.has_secondary_notification_image
        && let Some(image) = &banner.image
    {
        header = header.push(notification_image_visual(image));
    }
    header = header.push(activate).push(dismiss);

    let mut card = column![header, banner_actions_widget(banner)]
        .spacing(8)
        .width(Length::Fill);

    if let Some(progress) = banner.progress {
        card = card.push(
            progress_bar(0.0..=100.0, progress as f32)
                .girth(3)
                .style(iced::widget::progress_bar::primary),
        );
    }

    container(card)
        .padding(10)
        .width(Length::Fill)
        .height(Length::Shrink)
        .clip(true)
        .style(card_container_style)
        .into()
}

fn banner_actions_widget(banner: &BannerView) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{button, container, row};
    use iced::{Alignment, Length};

    let mut actions = row![]
        .spacing(6)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .height(if banner.actions.is_empty() {
            Length::Fixed(28.0)
        } else {
            Length::Shrink
        });

    for action in &banner.actions {
        actions = actions.push(
            button(banner_action_content(action))
                .padding([4, 8])
                .height(Length::Shrink)
                .width(Length::Fill)
                .style(content_button_style)
                .on_press(BannerUiMessage::InvokeActionRequested(
                    banner.target(),
                    action.key.clone(),
                )),
        );
    }

    container(actions).width(Length::Fill).into()
}

fn banner_action_content(action: &BannerAction) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{row, text};
    use iced::{Alignment, Length};

    let label = text(action.label.as_str())
        .size(12)
        .line_height(iced::Pixels(20.0))
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph);

    match &action.icon {
        Some(icon) => row![visual_image(icon, 16), label]
            .spacing(5)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into(),
        None => label.into(),
    }
}

fn notification_body_widget(banner: &BannerView) -> iced::Element<'_, BannerUiMessage> {
    use iced::Length;
    use iced::widget::{column, text};

    if banner.body_segments.is_empty() {
        return body_preview(
            text(&banner.body)
                .size(13)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .style(subdued_text_style)
                .into(),
        );
    }

    if !banner
        .body_segments
        .iter()
        .any(|segment| matches!(segment, BannerBodySegment::Image { .. }))
    {
        return body_preview(banner_body_text_widget(banner_body_text_runs(
            &banner.body_segments,
        )));
    }

    let mut body = column![].spacing(4).width(Length::Fill);
    let mut pending_text = Vec::new();

    for segment in &banner.body_segments {
        match segment {
            BannerBodySegment::Text { text, style } => {
                pending_text.push(NotificationBodyRun::styled(text.clone(), *style));
            }
            BannerBodySegment::Image { image } => {
                if !pending_text.is_empty() {
                    body = body.push(banner_body_text_widget(std::mem::take(&mut pending_text)));
                }
                body = body.push(body_image_visual(image));
            }
        }
    }

    if !pending_text.is_empty() {
        body = body.push(banner_body_text_widget(pending_text));
    }

    body_preview(body.into())
}

fn body_preview(body: iced::Element<'_, BannerUiMessage>) -> iced::Element<'_, BannerUiMessage> {
    use iced::Length;
    use iced::widget::{container, scrollable};

    // Only body content scrolls. The app name, actions and progress keep their space.
    container(scrollable(body).width(Length::Fill))
        .width(Length::Fill)
        .max_height(BODY_PREVIEW_MAX_HEIGHT)
        .into()
}

fn banner_body_text_runs(segments: &[BannerBodySegment]) -> Vec<NotificationBodyRun> {
    segments
        .iter()
        .filter_map(|segment| match segment {
            BannerBodySegment::Text { text, style } => {
                Some(NotificationBodyRun::styled(text.clone(), *style))
            }
            BannerBodySegment::Image { .. } => None,
        })
        .collect()
}

fn banner_body_text_widget(
    runs: Vec<NotificationBodyRun>,
) -> iced::Element<'static, BannerUiMessage> {
    use iced::widget::{rich_text, span};
    use iced::{Font, Length, font};

    let spans: Vec<iced::widget::text::Span<'static, (), iced::Font>> = runs
        .into_iter()
        .map(|run| {
            let mut body_span = span(run.text);
            if run.style.bold || run.style.italic {
                body_span = body_span.font(Font {
                    weight: if run.style.bold {
                        font::Weight::Bold
                    } else {
                        font::Weight::Normal
                    },
                    style: if run.style.italic {
                        font::Style::Italic
                    } else {
                        font::Style::Normal
                    },
                    ..Font::default()
                });
            }
            if run.style.underline {
                body_span = body_span.underline(true);
            }
            body_span
        })
        .collect();

    rich_text(spans)
        .size(13)
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
        .style(subdued_text_style)
        .into()
}

fn body_image_visual(visual: &BannerVisual) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{container, image};
    use iced::{ContentFit, Length};

    let height = BODY_IMAGE_VISUAL_HEIGHT;
    container(
        image(banner_visual_handle(visual))
            .width(Length::Fill)
            .height(height)
            .content_fit(ContentFit::Contain),
    )
    .width(Length::Fill)
    .height(height)
    .clip(true)
    .style(visual_image_container_style)
    .into()
}

fn overflow_count_banner_widget(count: usize) -> iced::Element<'static, BannerUiMessage> {
    use iced::widget::{container, row, text};
    use iced::{Alignment, Length};

    let label = if count == 1 {
        "+1 more notification".to_string()
    } else {
        format!("+{count} more notifications")
    };
    let badge = container(text("+").size(13))
        .center(26)
        .style(|theme| app_badge_style(theme, Urgency::Normal));

    container(
        row![
            badge,
            text(label)
                .size(14)
                .width(Length::Fill)
                .style(metadata_text_style)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(8)
    .width(Length::Fill)
    .height(DENSE_BANNER_HEIGHT)
    .clip(true)
    .style(stacked_card_container_style)
    .into()
}

fn banner_title(banner: &BannerView) -> &str {
    if banner.summary.trim().is_empty() {
        banner.app_name.as_str()
    } else {
        banner.summary.as_str()
    }
}

fn app_initial(app_name: &str) -> String {
    app_name
        .trim()
        .chars()
        .next()
        .map(|character| character.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

fn app_badge(app_name: &str, urgency: Urgency) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{container, text};

    container(text(app_initial(app_name)).size(15))
        .center(34)
        .style(move |theme| app_badge_style(theme, urgency))
        .into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BannerPrimaryVisual {
    AppBadge,
    AppIcon,
    NotificationImage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BannerHeaderVisualLayout {
    primary: BannerPrimaryVisual,
    has_secondary_notification_image: bool,
}

fn banner_header_visual_layout(banner: &BannerView) -> BannerHeaderVisualLayout {
    match (&banner.app_icon, &banner.image) {
        (Some(_), Some(_)) => BannerHeaderVisualLayout {
            primary: BannerPrimaryVisual::AppIcon,
            has_secondary_notification_image: true,
        },
        (Some(_), None) => BannerHeaderVisualLayout {
            primary: BannerPrimaryVisual::AppIcon,
            has_secondary_notification_image: false,
        },
        (None, Some(_)) => BannerHeaderVisualLayout {
            primary: BannerPrimaryVisual::NotificationImage,
            has_secondary_notification_image: false,
        },
        (None, None) => BannerHeaderVisualLayout {
            primary: BannerPrimaryVisual::AppBadge,
            has_secondary_notification_image: false,
        },
    }
}

fn primary_banner_visual(
    banner: &BannerView,
    primary: BannerPrimaryVisual,
) -> iced::Element<'_, BannerUiMessage> {
    let size = primary_banner_visual_size(primary);
    match primary {
        BannerPrimaryVisual::AppBadge => app_badge(&banner.app_name, banner.urgency),
        BannerPrimaryVisual::AppIcon => banner
            .app_icon
            .as_ref()
            .map(|icon| visual_image(icon, size))
            .unwrap_or_else(|| app_badge(&banner.app_name, banner.urgency)),
        BannerPrimaryVisual::NotificationImage => banner
            .image
            .as_ref()
            .map(|image| visual_image(image, size))
            .unwrap_or_else(|| app_badge(&banner.app_name, banner.urgency)),
    }
}

fn primary_banner_visual_size(primary: BannerPrimaryVisual) -> u16 {
    match primary {
        BannerPrimaryVisual::AppBadge
        | BannerPrimaryVisual::AppIcon
        | BannerPrimaryVisual::NotificationImage => APP_ICON_VISUAL_SIZE,
    }
}

fn notification_image_visual(image: &BannerVisual) -> iced::Element<'_, BannerUiMessage> {
    visual_image(image, NOTIFICATION_IMAGE_VISUAL_SIZE)
}

fn visual_image(visual: &BannerVisual, size: u16) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{container, image};

    let size = u32::from(size);
    container(image(banner_visual_handle(visual)).width(size).height(size))
        .center(size)
        .clip(true)
        .style(visual_image_container_style)
        .into()
}

fn banner_visual_handle(visual: &BannerVisual) -> iced::widget::image::Handle {
    match visual {
        BannerVisual::Rgba {
            width,
            height,
            pixels,
        } => iced::widget::image::Handle::from_rgba(*width, *height, pixels.clone()),
        // Renderer inputs are prepared pixels. Never reopen a sender path here,
        // even if an internal caller accidentally bypasses preparation.
        BannerVisual::LocalPath { .. } | BannerVisual::IconNames { .. } => {
            iced::widget::image::Handle::from_rgba(1, 1, vec![0; 4])
        }
    }
}

fn close_button<'a>(target: BannerTarget) -> iced::widget::Button<'a, BannerUiMessage> {
    use iced::widget::{button, text};

    button(text("x").size(14))
        .padding(0)
        .width(28)
        .height(28)
        .style(close_button_style)
        .on_press(BannerUiMessage::Dismiss(target))
}

fn card_container_style(theme: &iced::Theme) -> iced::widget::container::Style {
    let palette = theme.extended_palette();

    iced::widget::container::Style {
        background: Some(palette.background.base.color.into()),
        text_color: Some(palette.background.base.text),
        border: iced::Border {
            width: 1.0,
            radius: 8.0.into(),
            color: palette.background.weak.color,
        },
        shadow: banner_shadow(),
        ..Default::default()
    }
}

fn stacked_card_container_style(theme: &iced::Theme) -> iced::widget::container::Style {
    let palette = theme.extended_palette();

    iced::widget::container::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: Some(palette.background.weakest.text),
        border: iced::Border {
            width: 1.0,
            radius: 8.0.into(),
            color: palette.background.weak.color.scale_alpha(0.8),
        },
        shadow: iced::Shadow {
            color: iced::Color::from_rgba8(0, 0, 0, 0.12),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 10.0,
        },
        ..Default::default()
    }
}

fn app_badge_style(theme: &iced::Theme, urgency: Urgency) -> iced::widget::container::Style {
    let accent = urgency_accent_color(theme, urgency);

    iced::widget::container::Style {
        background: Some(accent.scale_alpha(0.18).into()),
        text_color: Some(accent),
        border: iced::Border {
            width: 1.0,
            radius: 7.0.into(),
            color: accent.scale_alpha(0.55),
        },
        ..Default::default()
    }
}

fn visual_image_container_style(theme: &iced::Theme) -> iced::widget::container::Style {
    let palette = theme.extended_palette();

    iced::widget::container::Style {
        background: Some(palette.background.weakest.color.into()),
        border: iced::Border {
            width: 1.0,
            radius: 7.0.into(),
            color: palette.background.weak.color,
        },
        ..Default::default()
    }
}

fn content_button_style(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    let background = match status {
        iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed => {
            Some(palette.background.weakest.color.scale_alpha(0.55).into())
        }
        _ => None,
    };

    iced::widget::button::Style {
        background,
        text_color: palette.background.base.text,
        border: iced::Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn close_button_style(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    let hovered = matches!(
        status,
        iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
    );

    iced::widget::button::Style {
        background: hovered.then(|| palette.danger.weak.color.scale_alpha(0.4).into()),
        text_color: if hovered {
            palette.danger.base.color
        } else {
            palette.background.base.text.scale_alpha(0.58)
        },
        border: iced::Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn subdued_text_style(theme: &iced::Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(
            theme
                .extended_palette()
                .background
                .base
                .text
                .scale_alpha(0.72),
        ),
    }
}

fn metadata_text_style(theme: &iced::Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(
            theme
                .extended_palette()
                .background
                .base
                .text
                .scale_alpha(0.54),
        ),
    }
}

fn urgency_accent_color(theme: &iced::Theme, urgency: Urgency) -> iced::Color {
    let palette = theme.extended_palette();

    match urgency {
        Urgency::Low => palette.secondary.base.color,
        Urgency::Normal => palette.primary.base.color,
        Urgency::Critical => palette.danger.base.color,
    }
}

fn banner_shadow() -> iced::Shadow {
    iced::Shadow {
        color: iced::Color::from_rgba8(0, 0, 0, 0.18),
        offset: iced::Vector::new(0.0, 4.0),
        blur_radius: 16.0,
    }
}

fn banner_ui_style(_state: &BannerUiState, theme: &iced::Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: iced::Color::TRANSPARENT,
        text_color: theme.palette().text,
    }
}

const DENSE_BANNER_HEIGHT: u32 = 60;
const BANNER_STACK_SPACING: u32 = 8;

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
        self.replace_banner_presentation(BannerPresentation::visible_banners(banners))
    }

    fn replace_banner_presentation(&self, presentation: BannerPresentation) -> anyhow::Result<()> {
        eprintln!(
            "level=info component=presentation event=replace_visible_banners count={} overflow_count={}",
            presentation.banners.len(),
            presentation.overflow_count
        );
        for banner in presentation.banners {
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
    pub notification_id: InternalNotificationId,
    pub revision: i64,
    pub app_identity: String,
    pub app_name: String,
    pub app_icon: Option<BannerVisual>,
    pub summary: String,
    pub body: String,
    pub body_content: NotificationBodyContent,
    pub body_segments: Vec<BannerBodySegment>,
    pub urgency: Urgency,
    pub progress: Option<i64>,
    pub image: Option<BannerVisual>,
    pub default_action_key: Option<String>,
    pub actions: Vec<BannerAction>,
    pub placement: BannerPlacement,
    pub focus_behavior: BannerFocusBehavior,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BannerAction {
    pub key: String,
    pub label: String,
    pub icon: Option<BannerVisual>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum BannerVisual {
    /// Unresolved local theme references, in preference order (at most two).
    IconNames {
        names: Vec<String>,
    },
    Rgba {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
    LocalPath {
        path: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum BannerBodySegment {
    Text {
        text: String,
        style: NotificationBodyStyle,
    },
    Image {
        image: BannerVisual,
    },
}

impl BannerBodySegment {
    pub fn plain_text(text: impl Into<String>) -> Self {
        Self::styled_text(text, NotificationBodyStyle::default())
    }

    pub fn styled_text(text: impl Into<String>, style: NotificationBodyStyle) -> Self {
        Self::Text {
            text: text.into(),
            style,
        }
    }

    pub fn image(image: BannerVisual) -> Self {
        Self::Image { image }
    }
}

impl BannerVisual {
    pub(crate) fn from_image_data(image: &NotificationImageData) -> Self {
        Self::Rgba {
            width: image.width,
            height: image.height,
            pixels: image.rgba_pixels(),
        }
    }

    fn from_reference(reference: &NotificationVisualReference) -> Option<Self> {
        match reference {
            NotificationVisualReference::LocalPath(path) if supported_local_image_path(path) => {
                Some(Self::LocalPath { path: path.clone() })
            }
            NotificationVisualReference::IconName(icon_name) => Some(Self::IconNames {
                names: vec![icon_name.clone()],
            }),
            NotificationVisualReference::LocalPath(_) => None,
        }
    }

    fn from_body_image_source(source: &NotificationBodyImageSource) -> Option<Self> {
        match source {
            NotificationBodyImageSource::LocalPath { path } if supported_local_image_path(path) => {
                Some(Self::LocalPath { path: path.clone() })
            }
            NotificationBodyImageSource::LocalPath { .. } => None,
        }
    }
}

fn banner_body_segments_from_body_content(
    body_content: &NotificationBodyContent,
) -> Vec<BannerBodySegment> {
    let mut banner_segments = Vec::new();

    for segment in &body_content.segments {
        match segment {
            NotificationBodySegment::Text { text, style } => {
                push_banner_body_text_segment(&mut banner_segments, text, *style);
            }
            NotificationBodySegment::Image { image } => {
                if let Some(visual) = BannerVisual::from_body_image_source(&image.source) {
                    banner_segments.push(BannerBodySegment::image(visual));
                } else if let Some(alt) = &image.alt {
                    push_banner_body_text_segment(
                        &mut banner_segments,
                        alt,
                        NotificationBodyStyle::default(),
                    );
                }
            }
        }
    }

    banner_segments
}

fn push_banner_body_text_segment(
    segments: &mut Vec<BannerBodySegment>,
    text: &str,
    style: NotificationBodyStyle,
) {
    if text.is_empty() {
        return;
    }

    match segments.last_mut() {
        Some(BannerBodySegment::Text {
            text: previous_text,
            style: previous_style,
        }) if *previous_style == style => previous_text.push_str(text),
        _ => segments.push(BannerBodySegment::styled_text(text, style)),
    }
}

impl BannerView {
    pub fn target(&self) -> BannerTarget {
        BannerTarget {
            notification_id: self.notification_id,
            revision: self.revision,
        }
    }

    pub(crate) fn from_thread(
        thread: NotificationThread,
        image_data: Option<NotificationImageData>,
    ) -> Self {
        let notification = thread.accepted_notification;
        let default_action_key = notification
            .actions
            .iter()
            .find(|action| action.key == "default")
            .map(|action| action.key.clone());
        let actions = notification
            .actions
            .iter()
            .filter(|action| action.key != "default")
            .take(2)
            .map(banner_action_from_notification_action)
            .collect();
        let app_icon =
            banner_app_icon_visual(notification.app_icon.as_ref(), &notification.app_identity);
        Self {
            notification_id: thread.id,
            revision: thread.revision,
            app_identity: notification.app_identity,
            app_name: notification.app_name,
            app_icon,
            summary: notification.summary,
            body: notification.body,
            body_segments: banner_body_segments_from_body_content(&notification.body_content),
            body_content: notification.body_content,
            urgency: notification.urgency,
            progress: notification.progress,
            image: image_data
                .as_ref()
                .map(BannerVisual::from_image_data)
                .or_else(|| {
                    notification
                        .image_reference
                        .as_ref()
                        .and_then(BannerVisual::from_reference)
                }),
            default_action_key,
            actions,
            placement: BannerPlacement::TopRightFocusedOutputWithFallback,
            focus_behavior: BannerFocusBehavior::DoesNotTakeFocus,
        }
    }
}

fn banner_app_icon_visual(
    app_icon: Option<&NotificationVisualReference>,
    app_identity: &str,
) -> Option<BannerVisual> {
    match app_icon {
        Some(reference) => BannerVisual::from_reference(reference),
        None => Some(BannerVisual::IconNames {
            names: app_identity_icon_name_candidates(app_identity),
        }),
    }
}

fn app_identity_icon_name_candidates(app_identity: &str) -> Vec<String> {
    let app_identity = app_identity.trim();
    if !icons::valid_name(app_identity) {
        return Vec::new();
    }
    let mut candidates = vec![app_identity.to_owned()];
    let suffix = ".desktop";
    if app_identity.to_ascii_lowercase().ends_with(suffix) {
        let stripped = app_identity[..app_identity.len() - suffix.len()].trim();
        if icons::valid_name(stripped) && stripped != app_identity {
            candidates.push(stripped.to_owned());
        }
    }
    candidates
}

fn banner_action_from_notification_action(action: &NotificationAction) -> BannerAction {
    BannerAction {
        key: action.key.clone(),
        label: action.label.clone(),
        icon: action.icon.as_ref().and_then(BannerVisual::from_reference),
    }
}

fn supported_local_image_path(path: &str) -> bool {
    Path::new(path).is_absolute() && has_static_raster_extension(path)
}

#[derive(Clone)]
pub(crate) struct PresentationCoordinator {
    enabled: bool,
    banner_presenter: Arc<dyn BannerPresenter>,
    renderer_state: Option<tokio::sync::watch::Receiver<BannerRendererState>>,
    last_error: Arc<Mutex<Option<String>>>,
    icon_settings: Arc<Mutex<BannerIconSettings>>,
}

impl PresentationCoordinator {
    pub(crate) fn new(enabled: bool, banner_presenter: Arc<dyn BannerPresenter>) -> Self {
        Self {
            enabled,
            renderer_state: enabled.then(|| banner_presenter.renderer_state()).flatten(),
            banner_presenter,
            last_error: Arc::new(Mutex::new(None)),
            icon_settings: Arc::new(Mutex::new(BannerIconSettings::default())),
        }
    }

    pub(crate) fn set_icon_settings(&self, settings: BannerIconSettings) {
        *self.icon_settings.lock().unwrap() = settings;
    }

    pub(crate) fn icon_settings(&self) -> BannerIconSettings {
        self.icon_settings.lock().unwrap().clone()
    }

    pub(crate) fn replace_banner_presentation_with_acknowledgement(
        &self,
        presentation: BannerPresentation,
        acknowledgement: BannerPresentationAcknowledgement,
    ) -> anyhow::Result<()> {
        if self.enabled {
            let result = self
                .banner_presenter
                .replace_banner_presentation_with_acknowledgement(presentation, acknowledgement);
            *self.last_error.lock().unwrap() =
                result.as_ref().err().map(|error| format!("{error:#}"));
            result?;
        }
        Ok(())
    }

    pub(crate) fn error(&self) -> Option<String> {
        if self.renderer_state.as_ref().is_some_and(|state| {
            state.has_changed().is_err() || *state.borrow() == BannerRendererState::Stopped
        }) {
            return Some("banner renderer stopped; restart the daemon to restore banners".into());
        }
        self.last_error.lock().unwrap().clone()
    }
}

#[derive(Clone, Default)]
pub struct NotificationBannerState {
    visible: HashMap<InternalNotificationId, NotificationThread>,
    visible_order: Vec<InternalNotificationId>,
    overflow: HashMap<InternalNotificationId, NotificationThread>,
}

impl NotificationBannerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_changes(&mut self, changes: impl IntoIterator<Item = BannerLifecycleChange>) {
        for change in changes {
            self.apply_change(change);
        }
    }

    pub fn visible_banners(&self) -> Vec<BannerView> {
        self.visible_threads()
            .into_iter()
            .map(|thread| BannerView::from_thread(thread, None))
            .collect()
    }

    pub(crate) fn visible_threads(&self) -> Vec<NotificationThread> {
        self.visible_order
            .iter()
            .filter_map(|id| self.visible.get(id).cloned())
            .collect()
    }

    pub(crate) fn overflow_count(&self) -> usize {
        self.overflow.len()
    }

    pub fn banner_presentation(&self) -> BannerPresentation {
        BannerPresentation {
            banners: self.visible_banners(),
            overflow_count: self.overflow_count(),
            icon_settings: BannerIconSettings::default(),
        }
    }

    fn apply_change(&mut self, change: BannerLifecycleChange) {
        match change {
            BannerLifecycleChange::ConsiderOpportunity(thread) => {
                self.consider_banner_opportunity(thread);
            }
            BannerLifecycleChange::Remove { notification_id } => {
                self.remove_banner(notification_id);
            }
        }
    }

    fn consider_banner_opportunity(&mut self, thread: NotificationThread) {
        if !is_banner_eligible(&thread) {
            self.remove_banner(thread.id);
            return;
        }

        if self.visible.contains_key(&thread.id) {
            self.update_visible_banner(thread);
            return;
        }

        self.overflow.remove(&thread.id);
        self.visible.insert(thread.id, thread);
        let mut threads: Vec<_> = self.visible.values().cloned().collect();
        sort_banner_candidates(&mut threads);
        self.visible.clear();
        self.visible_order.clear();
        for (index, thread) in threads.into_iter().enumerate() {
            if index < 3 {
                self.visible_order.push(thread.id);
                self.visible.insert(thread.id, thread);
            } else {
                self.overflow.insert(thread.id, thread);
            }
        }
    }

    fn update_visible_banner(&mut self, thread: NotificationThread) {
        if is_banner_eligible(&thread) {
            if !self.visible_order.contains(&thread.id) {
                self.visible_order.push(thread.id);
            }
            self.visible.insert(thread.id, thread);
        } else {
            self.remove_banner(thread.id);
        }
    }

    fn remove_banner(&mut self, notification_id: InternalNotificationId) {
        self.visible.remove(&notification_id);
        self.overflow.remove(&notification_id);
        self.visible_order.retain(|id| *id != notification_id);
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

fn is_banner_eligible(thread: &NotificationThread) -> bool {
    thread.status == NotificationStatus::Active && thread.delivery_outcome == DeliveryOutcome::Shown
}
