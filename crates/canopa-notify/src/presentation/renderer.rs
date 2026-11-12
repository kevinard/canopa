//! Wayland/Iced adapter; the projection and presenter contract remain desktop independent.
use super::*;
use crate::NotificationBodyRun;
use canopa_assets::icons;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(test)]
use std::sync::mpsc;

const APP_ICON_VISUAL_SIZE: u16 = 34;
const NOTIFICATION_IMAGE_VISUAL_SIZE: u16 = 58;
/// Width kept clear of the floating close control on a banner's first line.
const CLOSE_CORNER_SPACE: f32 = 34.0;
const BODY_IMAGE_VISUAL_HEIGHT: u32 = 48;
const BODY_PREVIEW_MAX_HEIGHT: u32 = 120;
const BANNER_MAX_WIDTH: f32 = 360.0;
// One ordinary Iced wheel step: leave room to fully reveal middle stack cards.
const BANNER_SCROLL_REVEAL_SPACE: f32 = 60.0;
const BANNER_ACTIVATION_TOKEN_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);
const REPLY_INPUT_ID: &str = "canopa-notify-reply";

/// The one open reply composer. Its text lives only in the renderer until sent.
#[derive(Clone, Debug)]
struct ReplyDraft {
    target: BannerTarget,
    text: String,
}

mod accessibility;
mod accessibility_bounds;
mod display;
mod images;
#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod memory;
mod revision;
mod surface;

#[derive(Clone)]
pub struct IcedLayerShellBannerPresenter {
    updates: BannerUpdateSender,
    state: tokio::sync::watch::Sender<BannerRendererState>,
    // The UI has a separate guard: either UI exit or the last owner retires the transport.
    _owner: Arc<BannerUiLifetime>,
}

impl IcedLayerShellBannerPresenter {
    pub async fn start(
        interactions: tokio::sync::mpsc::Sender<BannerInteraction>,
    ) -> anyhow::Result<Self> {
        let presenter = Self::start_with(interactions, run_banner_ui)?;
        await_banner_renderer(&presenter).await?;
        Ok(presenter)
    }

    fn start_with<F>(
        interactions: tokio::sync::mpsc::Sender<BannerInteraction>,
        run_ui: F,
    ) -> anyhow::Result<Self>
    where
        F: FnOnce(
                u64,
                tokio::sync::mpsc::Sender<BannerInteraction>,
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
            // Cards carry their own side margin, so banners sit as far from
            // the bar as the Shell's panels do.
            margin: (12, 0, 12, 0),
            keyboard_interactivity: match spec.keyboard_interactivity {
                BannerKeyboardInteractivity::None => KeyboardInteractivity::None,
            },
            start_mode: StartMode::Background,
            events_transparent: true,
        }
    }
}

impl BannerPresenter for IcedLayerShellBannerPresenter {
    fn shutdown(&self) {
        self.updates.close();
    }

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

struct BannerUiState {
    theme: iced::Theme,
    surface: surface::Surface,
    runtime_id: u64,
    presentation: BannerPresentation,
    presentation_generation: u64,
    pending_acknowledgement: Option<PendingBannerAcknowledgement>,
    input_region: BannerInputRegion,
    interactions: tokio::sync::mpsc::Sender<BannerInteraction>,
    renderer_state: tokio::sync::watch::Sender<BannerRendererState>,
    accessibility_enabled: bool,
    accessibility: Option<(iced::window::Id, u64, accessibility::BannerAccessibility)>,
    idle_reclamation: Option<(iced::window::Id, iced::task::Handle)>,
    reply: Option<ReplyDraft>,
}

#[derive(Clone, Debug)]
enum BannerUiMessage {
    OutputsChanged,
    OpenLinkRequested(BannerTarget, usize),
    OpenLink {
        target: BannerTarget,
        segment_index: usize,
        activation_token: Option<String>,
    },
    WindowFrame(iced::window::Id),
    AccessibilityActivated(iced::window::Id),
    AccessibilityBounds {
        window: iced::window::Id,
        generation: u64,
        bounds: accessibility_bounds::Bounds,
    },
    WindowClosed(iced::window::Id),
    WindowTimedOut(iced::window::Id),
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    ReclaimIdleMemory(iced::window::Id),
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    IdleMemoryReclaimed,
    OpenSurface(iced::window::Id),
    Shutdown,
    Runtime(BannerRuntimeInput),
    PresentationFrame(PendingBannerAcknowledgement),
    BannerFrame {
        rendered: PendingBannerAcknowledgement,
        target: BannerTarget,
    },
    LayoutFrame {
        window: iced::window::Id,
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
    /// Open the composer; the surface takes keyboard focus while it is open.
    ReplyRequested(BannerTarget),
    ReplyChanged(String),
    ReplySubmitted,
    ReplySend {
        target: BannerTarget,
        text: String,
        activation_token: Option<String>,
    },
    ReplyCancelled,
    SetKeyboardInteractivity(iced::window::Id, bool),
}

// Banner placement remains compositor-owned; named output snapshots are for the shell.
impl From<iced_layershell::WaylandOutputSnapshot> for BannerUiMessage {
    fn from(_: iced_layershell::WaylandOutputSnapshot) -> Self {
        Self::OutputsChanged
    }
}

impl TryInto<iced_layershell::actions::LayerShellCustomActionWithId> for BannerUiMessage {
    type Error = Self;

    fn try_into(
        self,
    ) -> Result<iced_layershell::actions::LayerShellCustomActionWithId, Self::Error> {
        use iced_layershell::actions::{LayerShellCustomAction, LayerShellCustomActionWithId};

        match self {
            Self::OpenSurface(id) => {
                let settings = IcedLayerShellBannerPresenter::layer_shell_settings(
                    BannerSurfaceSpec::notification_banner(),
                );
                Ok(LayerShellCustomActionWithId::new(
                    None,
                    LayerShellCustomAction::NewLayerShell {
                        id,
                        settings: iced_layershell::reexport::NewLayerShellSettings {
                            size: settings.size,
                            layer: settings.layer,
                            anchor: settings.anchor,
                            exclusive_zone: Some(settings.exclusive_zone),
                            margin: Some(settings.margin),
                            keyboard_interactivity: settings.keyboard_interactivity,
                            // Upstream Active now passes a null output to the compositor.
                            output_option: iced_layershell::reexport::OutputOption::Active,
                            events_transparent: true,
                            namespace: Some("canopa-notify-banners".into()),
                        },
                    },
                ))
            }
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
            Self::SetKeyboardInteractivity(window, exclusive) => {
                use iced_layershell::reexport::KeyboardInteractivity;
                Ok(LayerShellCustomActionWithId::new(
                    Some(window),
                    LayerShellCustomAction::KeyboardInteractivityChange(if exclusive {
                        KeyboardInteractivity::Exclusive
                    } else {
                        KeyboardInteractivity::None
                    }),
                ))
            }
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
    interactions: tokio::sync::mpsc::Sender<BannerInteraction>,
    run_ui: F,
) -> anyhow::Result<()>
where
    F: FnOnce(u64, tokio::sync::mpsc::Sender<BannerInteraction>) -> iced_layershell::Result
        + Send
        + 'static,
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
    interactions: tokio::sync::mpsc::Sender<BannerInteraction>,
    renderer_state: tokio::sync::watch::Sender<BannerRendererState>,
) -> iced_layershell::Result {
    use iced_layershell::settings::Settings;

    iced_layershell::disable_clipboard();
    iced_layershell::daemon(
        move || {
            let mut state = BannerUiState {
                theme: canopa_ui::theme(Default::default()),
                surface: surface::Surface::new(),
                runtime_id,
                presentation: BannerPresentation::default(),
                presentation_generation: 0,
                pending_acknowledgement: None,
                input_region: Default::default(),
                interactions: interactions.clone(),
                renderer_state: renderer_state.clone(),
                accessibility_enabled: true,
                accessibility: None,
                idle_reclamation: None,
                reply: None,
            };
            let window = state.surface.window().unwrap();
            let task = open_banner_surface(&mut state.surface, window);
            (state, task)
        },
        || fl!("notify-banner-window-title"),
        banner_ui_update,
        banner_ui_view,
    )
    .subscription(banner_ui_subscription)
    .theme(|state: &BannerUiState, _| state.theme.clone())
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
    let readiness = if state.surface.awaiting_frame() || state.accessibility_enabled {
        iced::event::listen_raw(|event, _, id| match event {
            iced::Event::Window(iced::window::Event::RedrawRequested(_)) => {
                Some(BannerUiMessage::WindowFrame(id))
            }
            _ => None,
        })
    } else {
        iced::Subscription::none()
    };
    let escape = if state.reply.is_some() {
        iced::keyboard::listen().filter_map(|event| match event {
            iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            } => Some(BannerUiMessage::ReplyCancelled),
            _ => None,
        })
    } else {
        iced::Subscription::none()
    };
    iced::Subscription::batch([
        readiness,
        escape,
        iced::window::close_events().map(BannerUiMessage::WindowClosed),
        iced::Subscription::run_with(state.runtime_id, banner_update_stream),
    ])
}

fn banner_ui_update(
    state: &mut BannerUiState,
    message: BannerUiMessage,
) -> iced::Task<BannerUiMessage> {
    let task = match message {
        BannerUiMessage::OutputsChanged => iced::Task::none(),
        BannerUiMessage::Shutdown => {
            state.idle_reclamation = None;
            iced::exit()
        }
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        BannerUiMessage::ReclaimIdleMemory(retired_window) => {
            if state.surface.is_idle()
                && state
                    .idle_reclamation
                    .as_ref()
                    .is_some_and(|(id, _)| *id == retired_window)
            {
                state.idle_reclamation = None;
                memory::reclaim()
            } else {
                iced::Task::none()
            }
        }
        BannerUiMessage::WindowFrame(window) => {
            let effects = state
                .surface
                .redraw(window, !state.presentation.banners.is_empty());
            let task = apply_surface_effects(state, effects);
            let accessibility = sync_banner_accessibility(state);
            let geometry = state
                .accessibility
                .as_ref()
                .filter(|(id, _, _)| *id == window)
                .map(|(_, generation, bridge)| bridge.measure(window, *generation))
                .unwrap_or_else(iced::Task::none);
            iced::Task::batch([task, geometry, accessibility])
        }
        BannerUiMessage::AccessibilityActivated(window) => {
            if let Some((current, _, bridge)) = &mut state.accessibility
                && *current == window
            {
                bridge.publish_after_activation();
            }
            iced::Task::none()
        }
        BannerUiMessage::AccessibilityBounds {
            window,
            generation,
            bounds,
        } => {
            if let Some((id, current, bridge)) = &mut state.accessibility
                && *id == window
                && *current == generation
            {
                bridge.set_bounds(bounds);
            }
            iced::Task::none()
        }
        BannerUiMessage::WindowClosed(window) => {
            let effects = state
                .surface
                .closed(window, !state.presentation.banners.is_empty());
            apply_surface_effects(state, effects)
        }
        BannerUiMessage::WindowTimedOut(window) => {
            let effects = state.surface.timed_out(window);
            apply_surface_effects(state, effects)
        }
        BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
            presentation,
            acknowledgement,
        }) => {
            if state.presentation.appearance != presentation.appearance {
                state.theme = canopa_ui::theme(presentation.appearance.clone());
            }
            state.presentation = presentation;
            state.presentation_generation = state
                .presentation_generation
                .checked_add(1)
                .expect("banner presentation generation exhausted");
            state.pending_acknowledgement = Some(PendingBannerAcknowledgement {
                generation: state.presentation_generation,
                acknowledgement,
            });
            let effects = state
                .surface
                .content_changed(!state.presentation.banners.is_empty());
            let surface = apply_surface_effects(state, effects);
            iced::Task::batch([surface, sync_reply_composer(state)])
        }
        BannerUiMessage::ReplyRequested(target) => open_reply_composer(state, target),
        BannerUiMessage::ReplyChanged(text) => {
            if let Some(draft) = &mut state.reply
                && text.len() <= crate::MAX_REPLY_BYTES
            {
                draft.text = text;
            }
            iced::Task::none()
        }
        BannerUiMessage::ReplySubmitted => {
            let Some(draft) = state
                .reply
                .clone()
                .filter(|draft| !draft.text.trim().is_empty())
            else {
                return iced::Task::none();
            };
            let Some(banner) = current_banner(state, draft.target) else {
                return iced::Task::none();
            };
            activation_token_task(banner.app_identity.clone(), move |activation_token| {
                BannerUiMessage::ReplySend {
                    target: draft.target,
                    text: draft.text,
                    activation_token,
                }
            })
        }
        BannerUiMessage::ReplySend {
            target,
            text,
            activation_token,
        } => {
            state
                .interactions
                .try_send(BannerInteraction::Reply {
                    target,
                    text,
                    activation_token,
                })
                .ok();
            close_reply_composer(state)
        }
        BannerUiMessage::ReplyCancelled => close_reply_composer(state),
        BannerUiMessage::LayoutFrame {
            window,
            generation,
            size,
            viewport,
            rendered,
        } => {
            if let Some(rendered) = rendered {
                let _ = banner_ui_update(state, BannerUiMessage::PresentationFrame(rendered));
            }
            if generation != state.presentation_generation || !state.surface.displays(window) {
                return iced::Task::none();
            }
            let width = size.width.min(viewport.width);
            let height = size.height.min(viewport.height);
            let position = state.presentation.position;
            // The stack sits in its configured corner; RTL mirrors start and end.
            let at_left = position.is_start() != canopa_ui::direction::is_rtl();
            let x = if at_left {
                0.0
            } else {
                (viewport.width - width).floor()
            };
            let y = if position.is_bottom() {
                (viewport.height - height).floor()
            } else {
                0.0
            };
            let bounds = iced::Rectangle {
                x: x as i32,
                y: y as i32,
                width: if at_left {
                    width.ceil()
                } else {
                    (viewport.width - x).ceil()
                } as i32,
                height: if position.is_bottom() {
                    (viewport.height - y).ceil()
                } else {
                    height.ceil()
                } as i32,
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
        BannerUiMessage::OpenLinkRequested(target, segment_index) => {
            let Some(banner) = current_banner(state, target) else {
                return iced::Task::none();
            };
            if !matches!(
                banner.body_content.segments.get(segment_index),
                Some(NotificationBodySegment::Link { .. })
            ) {
                return iced::Task::none();
            }
            activation_token_task(String::new(), move |activation_token| {
                BannerUiMessage::OpenLink {
                    target,
                    segment_index,
                    activation_token,
                }
            })
        }
        BannerUiMessage::OpenLink {
            target,
            segment_index,
            activation_token,
        } => {
            state
                .interactions
                .try_send(BannerInteraction::OpenLink {
                    target,
                    segment_index,
                    activation_token,
                })
                .ok();
            iced::Task::none()
        }
        BannerUiMessage::Activate {
            target,
            activation_token,
        } => {
            state
                .interactions
                .try_send(BannerInteraction::Activate {
                    target,
                    activation_token,
                })
                .ok();
            iced::Task::none()
        }
        BannerUiMessage::Dismiss(target) => {
            state
                .interactions
                .try_send(BannerInteraction::Dismiss(target))
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
                .try_send(BannerInteraction::InvokeAction {
                    target,
                    action_key,
                    activation_token,
                })
                .ok();
            iced::Task::none()
        }
        _ => iced::Task::none(),
    };
    iced::Task::batch([task, sync_banner_accessibility(state)])
}

fn open_reply_composer(
    state: &mut BannerUiState,
    target: BannerTarget,
) -> iced::Task<BannerUiMessage> {
    let Some(window) = state.surface.window() else {
        return iced::Task::none();
    };
    if current_banner(state, target).is_none_or(|banner| banner.reply.is_none())
        || state
            .reply
            .as_ref()
            .is_some_and(|draft| draft.target == target)
    {
        return iced::Task::none();
    }
    let previous = close_reply_composer(state);
    state.reply = Some(ReplyDraft {
        target,
        text: String::new(),
    });
    state
        .interactions
        .try_send(BannerInteraction::Composing {
            target,
            composing: true,
        })
        .ok();
    iced::Task::batch([
        previous,
        iced::Task::done(BannerUiMessage::SetKeyboardInteractivity(window, true)),
        iced::widget::operation::focus(REPLY_INPUT_ID),
    ])
}

/// Close the composer and return keyboard focus; the banner-hide timer resumes.
fn close_reply_composer(state: &mut BannerUiState) -> iced::Task<BannerUiMessage> {
    let Some(draft) = state.reply.take() else {
        return iced::Task::none();
    };
    state
        .interactions
        .try_send(BannerInteraction::Composing {
            target: draft.target,
            composing: false,
        })
        .ok();
    match state.surface.window() {
        Some(window) => iced::Task::done(BannerUiMessage::SetKeyboardInteractivity(window, false)),
        None => iced::Task::none(),
    }
}

/// Drop a composer whose revision disappeared, and honour a Center "Reply…" request.
fn sync_reply_composer(state: &mut BannerUiState) -> iced::Task<BannerUiMessage> {
    if let Some(draft) = &state.reply
        && current_banner(state, draft.target).is_none_or(|banner| banner.reply.is_none())
    {
        state.reply = None;
        if let Some(window) = state.surface.window() {
            return iced::Task::done(BannerUiMessage::SetKeyboardInteractivity(window, false));
        }
        return iced::Task::none();
    }
    let requested = state
        .presentation
        .banners
        .iter()
        .find(|banner| banner.reply.as_ref().is_some_and(|reply| reply.compose))
        .map(BannerView::target);
    match requested {
        Some(target) if state.reply.is_none() => open_reply_composer(state, target),
        _ => iced::Task::none(),
    }
}

fn sync_banner_accessibility(state: &mut BannerUiState) -> iced::Task<BannerUiMessage> {
    if !state.accessibility_enabled {
        return iced::Task::none();
    }
    let window = state.surface.window().filter(|window| {
        state.surface.displays(*window)
            && !state.surface.awaiting_frame()
            && !state.presentation.banners.is_empty()
    });
    let Some(window) = window else {
        state.accessibility = None;
        return iced::Task::none();
    };
    if state
        .accessibility
        .as_ref()
        .is_none_or(|(id, _, _)| *id != window)
    {
        let mut bridge = accessibility::BannerAccessibility::new(state.interactions.clone());
        bridge.update(&state.presentation.banners);
        let ready = bridge.activation();
        state.accessibility = Some((window, state.presentation_generation, bridge));
        return iced::Task::perform(
            async move {
                let _ = ready.await;
            },
            move |()| BannerUiMessage::AccessibilityActivated(window),
        );
    } else if let Some((_, generation, bridge)) = &mut state.accessibility
        && *generation != state.presentation_generation
    {
        bridge.update(&state.presentation.banners);
        *generation = state.presentation_generation;
    }
    iced::Task::none()
}

fn open_banner_surface(
    surface: &mut surface::Surface,
    window: iced::window::Id,
) -> iced::Task<BannerUiMessage> {
    // Construct the timer while entered into the daemon Tokio runtime. Iced may
    // poll this future from its separate thread-pool executor.
    let deadline = tokio::time::sleep(RENDERER_STARTUP_TIMEOUT);
    let (deadline, handle) =
        iced::Task::perform(deadline, move |_| BannerUiMessage::WindowTimedOut(window)).abortable();
    surface.watch_deadline(handle);
    iced::Task::batch([
        iced::Task::done(BannerUiMessage::OpenSurface(window)),
        deadline,
    ])
}

fn apply_surface_effects(
    state: &mut BannerUiState,
    effects: Vec<surface::Effect>,
) -> iced::Task<BannerUiMessage> {
    iced::Task::batch(effects.into_iter().map(|effect| match effect {
        surface::Effect::Open(window) => {
            state.idle_reclamation = None;
            *state.input_region.lock().unwrap() = None;
            open_banner_surface(&mut state.surface, window)
        }
        surface::Effect::Close(window) => {
            *state.input_region.lock().unwrap() = None;
            iced::window::close(window)
        }
        surface::Effect::Ready => {
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
        surface::Effect::Stop => {
            state.idle_reclamation = None;
            *state.input_region.lock().unwrap() = None;
            state
                .renderer_state
                .send_replace(BannerRendererState::Stopped);
            iced::exit()
        }
        surface::Effect::Idle(window) => schedule_idle_reclamation(state, window),
    }))
}

fn schedule_idle_reclamation(
    state: &mut BannerUiState,
    window: iced::window::Id,
) -> iced::Task<BannerUiMessage> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // Construct the timer inside the daemon runtime, as with surface deadlines.
        let delay = tokio::time::sleep(std::time::Duration::from_secs(2));
        let (task, handle) =
            iced::Task::perform(delay, move |_| BannerUiMessage::ReclaimIdleMemory(window))
                .abortable();
        state.idle_reclamation = Some((window, handle.abort_on_drop()));
        task
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        let _ = (state, window);
        iced::Task::none()
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

    mod accessibility;
    mod icons;
    mod images;
    mod layout;
    mod surfaces;

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
        let (interactions, _) = tokio::sync::mpsc::channel(32);
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
        let (interactions, _) = tokio::sync::mpsc::channel(32);
        let (exit_ui, exit_requested) = mpsc::channel();
        let presenter =
            IcedLayerShellBannerPresenter::start_with(interactions, move |_, _, renderer_state| {
                let mut ui = banner_ui_test_state();
                ui.renderer_state = renderer_state;
                let window = ui.surface.window().unwrap();
                let _task = banner_ui_update(&mut ui, BannerUiMessage::WindowFrame(window));
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
        let (interactions, _) = tokio::sync::mpsc::channel(32);
        let (release_ui, released) = mpsc::channel();
        let (late_state, observed_late_state) = tokio::sync::oneshot::channel();
        let presenter =
            IcedLayerShellBannerPresenter::start_with(interactions, move |_, _, renderer_state| {
                let _ = released.recv_timeout(Duration::from_secs(5));
                let mut ui = banner_ui_test_state();
                ui.renderer_state = renderer_state;
                let window = ui.surface.window().unwrap();
                let _task = banner_ui_update(&mut ui, BannerUiMessage::WindowFrame(window));
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
        let (interactions, _) = tokio::sync::mpsc::channel(32);
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
        let (interactions, _) = tokio::sync::mpsc::channel(32);
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
                    let window = ui.surface.window().unwrap();
                    let _task = banner_ui_update(&mut ui, BannerUiMessage::WindowFrame(window));
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
        let (interactions, _interaction_receiver) = tokio::sync::mpsc::channel(32);
        BannerUiState {
            theme: canopa_ui::theme(Default::default()),
            surface: surface::Surface::new(),
            runtime_id: 0,
            presentation: BannerPresentation::default(),
            presentation_generation: 0,
            pending_acknowledgement: None,
            input_region: Default::default(),
            interactions,
            renderer_state: tokio::sync::watch::channel(BannerRendererState::Starting).0,
            accessibility_enabled: false,
            accessibility: None,
            idle_reclamation: None,
            reply: None,
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
                // The shared fixture keeps the geometry its tests were written for.
                presentation: BannerPresentation {
                    position: crate::BannerPosition::TopEnd,
                    ..BannerPresentation::visible_banners(vec![banner_view(
                        notification_id,
                        "Queued",
                    )])
                },
                acknowledgement: BannerPresentationAcknowledgement::new(move || {
                    acknowledged.store(true, Ordering::SeqCst);
                }),
            }),
        );
    }

    #[tokio::test]
    async fn reply_composer_holds_the_timer_and_sends_the_text_once() {
        let (interactions, mut received) = tokio::sync::mpsc::channel(32);
        let mut state = BannerUiState {
            interactions,
            ..banner_ui_test_state()
        };
        let mut banner = banner_view(9, "Message");
        banner.actions = vec![BannerAction {
            key: "portal-button-0".into(),
            label: "Reply".into(),
            icon: None,
        }];
        banner.reply = Some(crate::BannerReply {
            action_key: "portal-button-0".into(),
            placeholder: None,
            compose: false,
        });
        let target = banner.target();
        let _ = banner_ui_update(
            &mut state,
            BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
                presentation: BannerPresentation::visible_banners(vec![banner.clone()]),
                acknowledgement: BannerPresentationAcknowledgement::new(|| {}),
            }),
        );
        let _ = banner_ui_update(&mut state, BannerUiMessage::ReplyRequested(target));
        assert!(state.reply.is_some());
        assert_eq!(
            received.try_recv().unwrap(),
            BannerInteraction::Composing {
                target,
                composing: true
            }
        );
        let oversized = "x".repeat(crate::MAX_REPLY_BYTES + 1);
        let _ = banner_ui_update(&mut state, BannerUiMessage::ReplyChanged(oversized));
        assert!(
            state.reply.as_ref().unwrap().text.is_empty(),
            "oversized input is refused"
        );
        let _ = banner_ui_update(&mut state, BannerUiMessage::ReplyChanged("Hi".into()));
        let _ = banner_ui_update(
            &mut state,
            BannerUiMessage::ReplySend {
                target,
                text: "Hi".into(),
                activation_token: None,
            },
        );
        assert_eq!(
            received.try_recv().unwrap(),
            BannerInteraction::Reply {
                target,
                text: "Hi".into(),
                activation_token: None
            }
        );
        assert_eq!(
            received.try_recv().unwrap(),
            BannerInteraction::Composing {
                target,
                composing: false
            }
        );
        assert!(state.reply.is_none());

        // A Center "Reply…" opens the composer; losing the banner drops it.
        banner.reply.as_mut().unwrap().compose = true;
        let _ = banner_ui_update(
            &mut state,
            BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
                presentation: BannerPresentation::visible_banners(vec![banner]),
                acknowledgement: BannerPresentationAcknowledgement::new(|| {}),
            }),
        );
        assert!(state.reply.is_some());
        let _ = banner_ui_update(
            &mut state,
            BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
                presentation: BannerPresentation::default(),
                acknowledgement: BannerPresentationAcknowledgement::new(|| {}),
            }),
        );
        assert!(state.reply.is_none());
    }

    #[tokio::test]
    async fn banner_ui_thread_enters_daemon_tokio_runtime() {
        let (interactions, _interaction_receiver) = tokio::sync::mpsc::channel(32);
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
        let (interactions, _) = tokio::sync::mpsc::channel(32);
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
        let (interactions, _) = tokio::sync::mpsc::channel(32);
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
            reply: None,
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
    window: iced::window::Id,
) -> iced::Element<'_, BannerUiMessage> {
    use iced::Length;
    use iced::widget::{container, responsive, scrollable, sensor};

    if !state.surface.displays(window) {
        return iced::widget::Space::new().into();
    }

    responsive(move |viewport| {
        // The compositor owns the viewport; natural content measurement only
        // determines the input region. Whole-presentation observers wait for the stack;
        // each card independently observes its own full visibility below.
        let frame = move |size: iced::Size| BannerUiMessage::LayoutFrame {
            window,
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
        let position = state.presentation.position;
        let at_left = position.is_start() != canopa_ui::direction::is_rtl();
        let stack = scrollable(measured)
            .width(viewport.width.min(BANNER_MAX_WIDTH))
            .height(if position.is_bottom() {
                Length::Shrink
            } else {
                Length::Fill
            });
        let column = container(stack).max_height(viewport.height);
        let column = if at_left {
            column.align_left(Length::Fill)
        } else {
            column.align_right(Length::Fill)
        };
        if position.is_bottom() {
            column.align_bottom(Length::Fill).into()
        } else {
            column.height(Length::Fill).into()
        }
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
    let start = state.presentation.position.is_start() != canopa_ui::direction::is_rtl();
    let mut stack = keyed_column![]
        .spacing(BANNER_STACK_SPACING)
        .width(Length::Fill)
        .align_items(if start {
            Alignment::Start
        } else {
            Alignment::End
        });
    for banner in &state.presentation.banners {
        stack = stack.push(
            banner_stack_item_key(banner),
            display::observe(
                // Display means the bounded card viewport is visible, not every
                // line of oversized content has been read (ADR 0046).
                container(
                    scrollable(notification_banner_widget_with_reply(
                        banner,
                        &state.presentation.appearance,
                        state
                            .reply
                            .as_ref()
                            .filter(|draft| draft.target == banner.target()),
                    ))
                    .width(Length::Fill),
                )
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
            overflow_count_banner_widget(
                state.presentation.overflow_count,
                &state.presentation.appearance,
            ),
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

#[cfg(test)]
fn notification_banner_widget<'a>(
    banner: &'a BannerView,
    appearance: &canopa_theme::Appearance,
) -> iced::Element<'a, BannerUiMessage> {
    notification_banner_widget_with_reply(banner, appearance, None)
}

fn notification_banner_widget_with_reply<'a>(
    banner: &'a BannerView,
    appearance: &canopa_theme::Appearance,
    draft: Option<&'a ReplyDraft>,
) -> iced::Element<'a, BannerUiMessage> {
    // Keep the outer card/scroll state while retiring a press captured before replacement.
    iced::widget::container(revision::own(
        notification_banner_content(banner, appearance, draft),
        banner.target(),
    ))
    .padding(canopa_ui::padding(
        appearance.style(canopa_theme::UiKind::Notification).margin,
    ))
    .into()
}

fn notification_banner_content<'a>(
    banner: &'a BannerView,
    appearance: &canopa_theme::Appearance,
    draft: Option<&'a ReplyDraft>,
) -> iced::Element<'a, BannerUiMessage> {
    use iced::widget::{button, column, container, progress_bar, row, text};
    use iced::{Alignment, Length};

    let mut title_font = canopa_ui::font(appearance);
    title_font.weight = iced::font::Weight::Semibold;
    // The first line leaves room for the close control floating at the corner;
    // the rest of the card uses the full width.
    let corner = iced::Padding {
        right: CLOSE_CORNER_SPACE,
        ..iced::Padding::ZERO
    };
    let mut content = column![].spacing(3).width(Length::Fill);
    let named = banner_title(banner) != banner.app_name;
    if named {
        content = content.push(
            container(
                text(&banner.app_name)
                    .font(canopa_ui::font(appearance))
                    .size(appearance.typography().label_size.px())
                    .width(Length::Fill)
                    .style(canopa_ui::muted),
            )
            .padding(corner),
        );
    }
    content = content
        .push(
            container(
                text(banner_title(banner))
                    .font(title_font)
                    .size(banner_title_size(appearance))
                    .width(Length::Fill)
                    .line_height(iced::Pixels(banner_title_size(appearance) * 1.3))
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            )
            .padding(if named { iced::Padding::ZERO } else { corner }),
        )
        .push(
            container(notification_body_widget(banner, appearance))
                .id(accessibility_bounds::widget_id(banner.target(), 1))
                .width(Length::Fill),
        );

    let activate = button(container(content).width(Length::Fill).clip(true))
        .padding(0)
        .width(Length::Fill)
        .style({
            let appearance = appearance.clone();
            move |theme, status| {
                let mut style = content_button_style(theme, status);
                if appearance.customization.ui[canopa_theme::UiKind::Notification as usize]
                    .foreground
                    .is_some()
                {
                    style.text_color = canopa_ui::resolve_color(
                        &appearance,
                        appearance
                            .style(canopa_theme::UiKind::Notification)
                            .foreground,
                    );
                }
                style
            }
        })
        .on_press(BannerUiMessage::ActivateRequested(banner.target()));
    let activate = container(activate)
        .id(accessibility_bounds::widget_id(banner.target(), 3))
        .width(Length::Fill);
    let dismiss = container(close_button(banner.target(), appearance))
        .id(accessibility_bounds::widget_id(banner.target(), 2));

    let visual_layout = banner_header_visual_layout(banner);
    let mut header = row![primary_banner_visual(
        banner,
        visual_layout.primary,
        appearance
    ),]
    .spacing(10)
    .align_y(Alignment::Start);
    if visual_layout.has_secondary_notification_image
        && let Some(image) = &banner.image
    {
        header = header.push(notification_image_visual(image));
    }
    header = header.push(activate);
    let header = iced::widget::stack![
        header,
        container(dismiss)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right),
    ];

    let mut card = column![header]
        .spacing(
            appearance
                .style(canopa_theme::UiKind::Notification)
                .gap
                .px(),
        )
        .width(Length::Fill);
    if !banner.actions.is_empty() {
        card = card.push(banner_actions_widget(banner, appearance));
    }
    if let Some(draft) = draft {
        card = card.push(reply_composer_widget(banner, draft, appearance));
    }

    if let Some(progress) = banner.progress {
        card = card.push(
            progress_bar(0.0..=100.0, progress as f32)
                .girth(3)
                .style(iced::widget::progress_bar::primary),
        );
    }

    container(card)
        .id(accessibility_bounds::widget_id(banner.target(), 0))
        .padding(canopa_ui::content_padding(
            appearance.style(canopa_theme::UiKind::Notification),
        ))
        .width(Length::Fill)
        .height(Length::Shrink)
        .clip(true)
        .style({
            let appearance = appearance.clone();
            move |_| {
                let mut frame = canopa_ui::frame(
                    &appearance,
                    appearance.style(canopa_theme::UiKind::Notification),
                );
                frame.shadow = banner_shadow();
                frame
            }
        })
        .into()
}

fn banner_actions_widget<'a>(
    banner: &'a BannerView,
    appearance: &canopa_theme::Appearance,
) -> iced::Element<'a, BannerUiMessage> {
    use iced::widget::{button, column, container, row};
    use iced::{Alignment, Length};

    // Configured button styles keep the shared action; by default actions are
    // soft, borderless buttons, two to a row, like the Center's chips.
    let style = appearance.style(canopa_theme::UiKind::Button);
    let custom = style.custom_padding || style.custom_border || style.custom_radius;
    let mut actions = column![].spacing(6).width(Length::Fill);

    for (row_index, pair) in banner.actions.chunks(2).enumerate() {
        let mut action_row = row![]
            .spacing(6)
            .align_y(Alignment::Center)
            .width(Length::Fill);
        for (column_index, action) in pair.iter().enumerate() {
            let control = canopa_ui::styled_action(
                banner_action_content(action, appearance),
                // The reply action opens the composer; sending invokes it.
                Some(
                    if banner
                        .reply
                        .as_ref()
                        .is_some_and(|reply| reply.action_key == action.key)
                    {
                        BannerUiMessage::ReplyRequested(banner.target())
                    } else {
                        BannerUiMessage::InvokeActionRequested(banner.target(), action.key.clone())
                    },
                ),
                canopa_ui::Action::Normal,
                false,
                appearance,
            )
            .height(Length::Shrink)
            .width(Length::Fill);
            let control = if custom {
                control
            } else {
                control.padding([6, 10]).style(|theme, status| {
                    let colors = canopa_ui::colors(theme);
                    let engaged =
                        matches!(status, button::Status::Hovered | button::Status::Pressed);
                    button::Style {
                        background: Some(
                            colors
                                .text
                                .scale_alpha(if engaged { 0.16 } else { 0.08 })
                                .into(),
                        ),
                        text_color: colors.text,
                        border: iced::Border::default().rounded(12),
                        ..Default::default()
                    }
                })
            };
            action_row = action_row.push(
                container(control)
                    .id(accessibility_bounds::widget_id(
                        banner.target(),
                        row_index * 2 + column_index + 4,
                    ))
                    .width(Length::Fill),
            );
        }
        actions = actions.push(action_row);
    }

    container(actions).width(Length::Fill).into()
}

fn reply_composer_widget<'a>(
    banner: &'a BannerView,
    draft: &'a ReplyDraft,
    appearance: &canopa_theme::Appearance,
) -> iced::Element<'a, BannerUiMessage> {
    use iced::widget::{row, text, text_input};
    use iced::{Alignment, Length};
    let placeholder = banner
        .reply
        .as_ref()
        .and_then(|reply| reply.placeholder.as_deref())
        .map(str::to_owned)
        .unwrap_or_else(|| fl!("notify-reply-placeholder"));
    let label = |value: String| {
        text(value)
            .font(canopa_ui::font(appearance))
            .size(appearance.typography().body_size.px())
    };
    let can_send = !draft.text.trim().is_empty();
    row![
        text_input(&placeholder, &draft.text)
            .id(REPLY_INPUT_ID)
            .on_input(BannerUiMessage::ReplyChanged)
            .on_submit(BannerUiMessage::ReplySubmitted)
            .size(appearance.typography().body_size.px())
            .width(Length::Fill),
        canopa_ui::styled_action(
            label(fl!("notify-reply-send")),
            can_send.then_some(BannerUiMessage::ReplySubmitted),
            canopa_ui::Action::Normal,
            false,
            appearance,
        ),
        canopa_ui::styled_action(
            label(fl!("notify-reply-cancel")),
            Some(BannerUiMessage::ReplyCancelled),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

fn banner_action_content<'a>(
    action: &'a BannerAction,
    appearance: &canopa_theme::Appearance,
) -> iced::Element<'a, BannerUiMessage> {
    use iced::widget::{row, text};
    use iced::{Alignment, Length};

    let mut font = canopa_ui::font(appearance);
    font.weight = iced::font::Weight::Semibold;
    let label = text(action.label.as_str())
        .font(font)
        .size(appearance.typography().body_size.px())
        .line_height(iced::Pixels(
            appearance.typography().body_size.px() * 1.4286,
        ))
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
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

fn notification_body_widget<'a>(
    banner: &'a BannerView,
    appearance: &canopa_theme::Appearance,
) -> iced::Element<'a, BannerUiMessage> {
    use iced::Length;
    use iced::widget::{column, text};

    if banner.body_segments.is_empty() {
        return body_preview(
            text(&banner.body)
                .font(canopa_ui::font(appearance))
                .size(appearance.typography().body_size.px())
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .style(canopa_ui::muted)
                .into(),
        );
    }

    if !banner
        .body_segments
        .iter()
        .any(|segment| !matches!(segment, BannerBodySegment::Text { .. }))
    {
        return body_preview(banner_body_text_widget(
            banner_body_text_runs(&banner.body_segments),
            appearance,
        ));
    }

    let mut body = column![].spacing(4).width(Length::Fill);
    let mut pending_text = Vec::new();

    for segment in &banner.body_segments {
        match segment {
            BannerBodySegment::Text { text, style } => {
                pending_text.push(NotificationBodyRun::styled(text.clone(), *style));
            }
            BannerBodySegment::Link {
                text,
                style,
                segment_index,
            } => {
                if !pending_text.is_empty() {
                    body = body.push(banner_body_text_widget(
                        std::mem::take(&mut pending_text),
                        appearance,
                    ));
                }
                let label = styled_body_widget(
                    vec![NotificationBodyRun::styled(
                        text.clone(),
                        NotificationBodyStyle {
                            underline: true,
                            ..*style
                        },
                    )],
                    true,
                    appearance,
                );
                body = body.push(
                    iced::widget::container(
                        iced::widget::button(label)
                            .padding(2)
                            .width(Length::Fill)
                            .style({
                                let appearance = appearance.clone();
                                move |theme, status| {
                                    let mut style = content_button_style(theme, status);
                                    if appearance.customization.ui
                                        [canopa_theme::UiKind::Notification as usize]
                                        .foreground
                                        .is_some()
                                    {
                                        style.text_color = canopa_ui::resolve_color(
                                            &appearance,
                                            appearance
                                                .style(canopa_theme::UiKind::Notification)
                                                .foreground,
                                        );
                                    }
                                    style
                                }
                            })
                            .on_press(BannerUiMessage::OpenLinkRequested(
                                banner.target(),
                                *segment_index,
                            )),
                    )
                    .width(Length::Fill)
                    .id(accessibility_bounds::widget_id(
                        banner.target(),
                        accessibility_bounds::link_slot(*segment_index),
                    )),
                );
            }
            BannerBodySegment::Image { image } => {
                if !pending_text.is_empty() {
                    body = body.push(banner_body_text_widget(
                        std::mem::take(&mut pending_text),
                        appearance,
                    ));
                }
                body = body.push(body_image_visual(image));
            }
        }
    }

    if !pending_text.is_empty() {
        body = body.push(banner_body_text_widget(pending_text, appearance));
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
            BannerBodySegment::Text { text, style }
            | BannerBodySegment::Link { text, style, .. } => {
                Some(NotificationBodyRun::styled(text.clone(), *style))
            }
            BannerBodySegment::Image { .. } => None,
        })
        .collect()
}

fn banner_body_text_widget(
    runs: Vec<NotificationBodyRun>,
    appearance: &canopa_theme::Appearance,
) -> iced::Element<'static, BannerUiMessage> {
    styled_body_widget(runs, false, appearance)
}

fn styled_body_widget(
    runs: Vec<NotificationBodyRun>,
    link: bool,
    appearance: &canopa_theme::Appearance,
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
                    ..canopa_ui::font(appearance)
                });
            }
            if run.style.underline {
                body_span = body_span.underline(true);
            }
            body_span
        })
        .collect();

    rich_text(spans)
        .font(canopa_ui::font(appearance))
        .size(appearance.typography().body_size.px())
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
        .style(move |theme: &iced::Theme| {
            if link {
                canopa_ui::link(theme)
            } else {
                canopa_ui::muted(theme)
            }
        })
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

fn overflow_count_banner_widget(
    count: usize,
    appearance: &canopa_theme::Appearance,
) -> iced::Element<'static, BannerUiMessage> {
    use iced::widget::{container, row, text};
    use iced::{Alignment, Length};

    let label = if count == 1 {
        "+1 more notification".to_string()
    } else {
        format!("+{count} more notifications")
    };
    let badge = container(
        text("+")
            .font(canopa_ui::font(appearance))
            .size(appearance.typography().body_size.px()),
    )
    .center(26)
    .style(|theme| app_badge_style(theme, Urgency::Normal));

    container(
        row![
            badge,
            text(label)
                .font(canopa_ui::font(appearance))
                .size(appearance.typography().body_size.px())
                .width(Length::Fill)
                .style(canopa_ui::muted)
        ]
        .spacing(
            appearance
                .style(canopa_theme::UiKind::Notification)
                .gap
                .px(),
        )
        .align_y(Alignment::Center),
    )
    .padding(8)
    .width(Length::Fill)
    .height((appearance.typography().body_size.px() * 1.3 + 16.0).max(DENSE_BANNER_HEIGHT as f32))
    .clip(true)
    .style({
        let appearance = appearance.clone();
        move |_| {
            canopa_ui::frame(
                &appearance,
                appearance.style(canopa_theme::UiKind::Notification),
            )
        }
    })
    .into()
}

fn banner_title(banner: &BannerView) -> &str {
    if banner.summary.trim().is_empty() {
        banner.app_name.as_str()
    } else {
        banner.summary.as_str()
    }
}

/// Titles sit between body and panel-title sizes, as in the Center's cards.
fn banner_title_size(appearance: &canopa_theme::Appearance) -> f32 {
    let typography = appearance.typography();
    ((typography.body_size.px() + typography.title_size.px()) / 2.0).round()
}

fn app_initial(app_name: &str) -> String {
    app_name
        .trim()
        .chars()
        .next()
        .map(|character| character.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

fn app_badge<'a>(
    app_name: &'a str,
    urgency: Urgency,
    appearance: &canopa_theme::Appearance,
) -> iced::Element<'a, BannerUiMessage> {
    use iced::widget::{container, text};

    container(
        text(app_initial(app_name))
            .font(canopa_ui::font(appearance))
            .size(appearance.typography().body_size.px() + 2.0),
    )
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

fn primary_banner_visual<'a>(
    banner: &'a BannerView,
    primary: BannerPrimaryVisual,
    appearance: &canopa_theme::Appearance,
) -> iced::Element<'a, BannerUiMessage> {
    let size = primary_banner_visual_size(primary);
    match primary {
        BannerPrimaryVisual::AppBadge => app_badge(&banner.app_name, banner.urgency, appearance),
        BannerPrimaryVisual::AppIcon => banner
            .app_icon
            .as_ref()
            .map(|icon| app_icon_image(icon, size))
            .unwrap_or_else(|| app_badge(&banner.app_name, banner.urgency, appearance)),
        BannerPrimaryVisual::NotificationImage => banner
            .image
            .as_ref()
            .map(|image| visual_image(image, size))
            .unwrap_or_else(|| app_badge(&banner.app_name, banner.urgency, appearance)),
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

/// Application icons carry their own shape; only notification images are framed.
fn app_icon_image(visual: &BannerVisual, size: u16) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{container, image};

    let size = u32::from(size);
    container(image(banner_visual_handle(visual)).width(size).height(size))
        .center(size)
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
        BannerVisual::LocalPath { .. }
        | BannerVisual::IconNames { .. }
        | BannerVisual::DesktopEntry { .. } => {
            iced::widget::image::Handle::from_rgba(1, 1, vec![0; 4])
        }
    }
}

fn close_button<'a>(
    target: BannerTarget,
    appearance: &canopa_theme::Appearance,
) -> iced::widget::Button<'a, BannerUiMessage> {
    let label = iced::widget::container(
        iced::widget::text("×")
            .font(canopa_ui::font(appearance))
            .size(appearance.typography().body_size.px() + 2.0)
            .line_height(1.0),
    )
    .center(iced::Length::Fill);
    let style = appearance.style(canopa_theme::UiKind::Button);
    let custom = style.custom_padding || style.custom_border;
    let padding = if custom {
        canopa_ui::content_padding(style)
    } else {
        iced::Padding::ZERO
    };
    let button = canopa_ui::styled_action(
        label,
        Some(BannerUiMessage::Dismiss(target)),
        canopa_ui::Action::Quiet,
        false,
        appearance,
    );
    // The default close control is a soft round button, like the Shell's panels.
    let button = if custom {
        button
    } else {
        button.style(|theme, status| {
            let colors = canopa_ui::colors(theme);
            let engaged = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(
                    colors
                        .text
                        .scale_alpha(if engaged { 0.18 } else { 0.10 })
                        .into(),
                ),
                text_color: colors.text,
                border: iced::Border::default().rounded(14),
                ..Default::default()
            }
        })
    };
    button
        .padding(padding)
        .width((appearance.typography().body_size.px() + padding.left + padding.right).max(28.0))
        .height(
            (appearance.typography().body_size.px() * 1.3 + padding.top + padding.bottom).max(28.0),
        )
}

fn app_badge_style(theme: &iced::Theme, urgency: Urgency) -> iced::widget::container::Style {
    let accent = urgency_accent_color(theme, urgency);

    iced::widget::container::Style {
        background: Some(accent.scale_alpha(0.18).into()),
        text_color: Some(accent),
        border: iced::Border::default().rounded(17),
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
    canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false)
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
