use crate::domain::has_static_raster_extension;
use crate::{
    BannerLifecycleChange, DeliveryOutcome, NotificationAction, NotificationBodyContent,
    NotificationBodyImageSource, NotificationBodyRun, NotificationBodySegment,
    NotificationBodyStyle, NotificationImageData, NotificationStatus, NotificationThread,
    NotificationVisualReference, Urgency,
};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(test)]
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

const MAX_FILE_BACKED_VISUAL_BYTES: u64 = 16 * 1024 * 1024;
const ICON_SEARCH_MAX_DEPTH: usize = 8;
const STATIC_ICON_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];
const APP_ICON_VISUAL_SIZE: u16 = 34;
const NOTIFICATION_IMAGE_VISUAL_SIZE: u16 = 58;
const BODY_IMAGE_VISUAL_HEIGHT: u32 = 48;
const BODY_CONTENT_TEXT_HEIGHT: u32 = 34;
const BODY_CONTENT_RICH_HEIGHT: u32 = 70;
const BANNER_ACTIVATION_TOKEN_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);

#[derive(Clone)]
pub struct BannerPresentationAcknowledgement {
    callback: Arc<Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>>,
}

impl BannerPresentationAcknowledgement {
    pub fn new(callback: impl FnOnce() + Send + 'static) -> Self {
        Self {
            callback: Arc::new(Mutex::new(Some(Box::new(callback)))),
        }
    }

    pub fn acknowledge(self) {
        let callback = self.callback.lock().unwrap().take();
        if let Some(callback) = callback {
            callback();
        }
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
}

impl BannerPresentation {
    pub fn visible_banners(banners: Vec<BannerView>) -> Self {
        Self {
            banners,
            overflow_count: 0,
        }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BannerInteraction {
    Activate {
        notification_id: i64,
        activation_token: Option<String>,
    },
    Dismiss(i64),
    InvokeAction {
        notification_id: i64,
        action_key: String,
        activation_token: Option<String>,
    },
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
        self.updates
            .lock()
            .unwrap()
            .try_send(BannerRuntimeInput::Replace {
                presentation,
                acknowledgement,
            })
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
    Replace {
        presentation: BannerPresentation,
        acknowledgement: BannerPresentationAcknowledgement,
    },
}

#[derive(Clone)]
struct PendingBannerAcknowledgement {
    generation: u64,
    acknowledgement: BannerPresentationAcknowledgement,
}

#[derive(Clone)]
struct BannerUiState {
    runtime_id: u64,
    presentation: BannerPresentation,
    presentation_generation: u64,
    pending_acknowledgements: Vec<PendingBannerAcknowledgement>,
    surface_size: (u32, u32),
    interactions: mpsc::Sender<BannerInteraction>,
}

#[derive(Clone, Debug)]
enum BannerUiMessage {
    Runtime(BannerRuntimeInput),
    PresentationFrame(u64),
    ActivateRequested(i64),
    Activate {
        notification_id: i64,
        activation_token: Option<String>,
    },
    Dismiss(i64),
    InvokeActionRequested(i64, String),
    InvokeAction {
        notification_id: i64,
        action_key: String,
        activation_token: Option<String>,
    },
    RequestActivationToken {
        app_identity: String,
        callback: iced_layershell::actions::ActivationTokenCallback,
    },
    SizeChange((u32, u32)),
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
            Self::SizeChange(size) => Ok(LayerShellCustomActionWithId::new(
                None,
                LayerShellCustomAction::SizeChange(size),
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
            presentation: BannerPresentation::default(),
            presentation_generation: 0,
            pending_acknowledgements: Vec::new(),
            surface_size: banner_surface_size(0, 0),
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
    let presentation_frames = if state
        .pending_acknowledgements
        .iter()
        .any(|pending| pending.generation == state.presentation_generation)
    {
        iced::window::frames()
            .with(state.presentation_generation)
            .map(|(generation, _)| BannerUiMessage::PresentationFrame(generation))
    } else {
        iced::Subscription::none()
    };
    iced::Subscription::batch([
        iced::Subscription::run_with(state.runtime_id, banner_update_stream)
            .map(BannerUiMessage::Runtime),
        presentation_frames,
    ])
}

fn banner_ui_update(
    state: &mut BannerUiState,
    message: BannerUiMessage,
) -> iced::Task<BannerUiMessage> {
    match message {
        BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
            presentation,
            acknowledgement,
        }) => {
            let size = banner_surface_resize(&mut state.surface_size, &presentation);
            state.presentation = presentation;
            state.presentation_generation = state
                .presentation_generation
                .checked_add(1)
                .expect("banner presentation generation exhausted");
            state
                .pending_acknowledgements
                .push(PendingBannerAcknowledgement {
                    generation: state.presentation_generation,
                    acknowledgement,
                });
            match size {
                Some(size) => iced::Task::done(BannerUiMessage::SizeChange(size)),
                None => iced::Task::none(),
            }
        }
        BannerUiMessage::PresentationFrame(rendered_generation) => {
            let mut later_acknowledgements = Vec::new();
            for pending in std::mem::take(&mut state.pending_acknowledgements) {
                if pending.generation == rendered_generation {
                    pending.acknowledgement.acknowledge();
                } else if pending.generation > rendered_generation {
                    later_acknowledgements.push(pending);
                }
            }
            state.pending_acknowledgements = later_acknowledgements;
            iced::Task::none()
        }
        BannerUiMessage::ActivateRequested(notification_id) => {
            let Some(app_identity) = banner_app_identity(state, notification_id) else {
                state
                    .interactions
                    .send(BannerInteraction::Activate {
                        notification_id,
                        activation_token: None,
                    })
                    .ok();
                return iced::Task::none();
            };
            activation_token_task(app_identity, move |activation_token| {
                BannerUiMessage::Activate {
                    notification_id,
                    activation_token,
                }
            })
        }
        BannerUiMessage::Activate {
            notification_id,
            activation_token,
        } => {
            state
                .interactions
                .send(BannerInteraction::Activate {
                    notification_id,
                    activation_token,
                })
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
        BannerUiMessage::InvokeActionRequested(notification_id, action_key) => {
            let Some(app_identity) = banner_app_identity(state, notification_id) else {
                state
                    .interactions
                    .send(BannerInteraction::InvokeAction {
                        notification_id,
                        action_key,
                        activation_token: None,
                    })
                    .ok();
                return iced::Task::none();
            };
            activation_token_task(app_identity, move |activation_token| {
                BannerUiMessage::InvokeAction {
                    notification_id,
                    action_key,
                    activation_token,
                }
            })
        }
        BannerUiMessage::InvokeAction {
            notification_id,
            action_key,
            activation_token,
        } => {
            state
                .interactions
                .send(BannerInteraction::InvokeAction {
                    notification_id,
                    action_key,
                    activation_token,
                })
                .ok();
            iced::Task::none()
        }
        _ => iced::Task::none(),
    }
}

fn banner_app_identity(state: &BannerUiState, notification_id: i64) -> Option<String> {
    state
        .presentation
        .banners
        .iter()
        .find(|banner| banner.notification_id == notification_id)
        .map(|banner| banner.app_identity.clone())
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

    fn banner_ui_test_state() -> BannerUiState {
        let (interactions, _interaction_receiver) = mpsc::channel();
        BannerUiState {
            runtime_id: 0,
            presentation: BannerPresentation::default(),
            presentation_generation: 0,
            pending_acknowledgements: Vec::new(),
            surface_size: banner_surface_size(0, 0),
            interactions,
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
    async fn iced_presenter_acknowledges_only_after_ui_consumes_presentation() {
        let (updates, mut receiver) = iced::futures::channel::mpsc::channel(1);
        let presenter = IcedLayerShellBannerPresenter {
            updates: Arc::new(Mutex::new(updates)),
        };
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
        let rendered_generation = state.presentation_generation;
        let _task = banner_ui_update(
            &mut state,
            BannerUiMessage::PresentationFrame(rendered_generation),
        );

        assert!(acknowledged.load(Ordering::SeqCst));
        assert_eq!(state.presentation.banners[0].notification_id, 42);
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

        let rendered_generation = state.presentation_generation;
        let _task = banner_ui_update(
            &mut state,
            BannerUiMessage::PresentationFrame(rendered_generation),
        );

        assert!(!first_acknowledged.load(Ordering::SeqCst));
        assert!(latest_acknowledged.load(Ordering::SeqCst));
    }

    #[test]
    fn presentation_frame_acknowledges_its_generation_after_a_newer_update_arrives() {
        let mut state = banner_ui_test_state();
        let first_acknowledged = Arc::new(AtomicBool::new(false));
        let latest_acknowledged = Arc::new(AtomicBool::new(false));

        for (notification_id, acknowledged) in [
            (1, first_acknowledged.clone()),
            (2, latest_acknowledged.clone()),
        ] {
            queue_banner_ui_presentation(&mut state, notification_id, acknowledged);
        }

        let _task = banner_ui_update(&mut state, BannerUiMessage::PresentationFrame(1));

        assert!(first_acknowledged.load(Ordering::SeqCst));
        assert!(!latest_acknowledged.load(Ordering::SeqCst));
        let _task = banner_ui_update(&mut state, BannerUiMessage::PresentationFrame(2));
        assert!(latest_acknowledged.load(Ordering::SeqCst));
    }

    #[test]
    fn same_size_banner_replacement_does_not_request_layer_shell_resize() {
        let mut current_size = banner_surface_size(1, 0);
        let replacement = BannerPresentation::visible_banners(vec![banner_view(42, "Complete")]);

        assert_eq!(banner_surface_resize(&mut current_size, &replacement), None);
        assert_eq!(current_size, banner_surface_size(1, 0));
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

    #[test]
    fn static_icon_names_resolve_from_icon_search_roots() {
        let temp = tempfile::tempdir().unwrap();
        let icon_root = temp.path().join("icons");
        let icon_path = icon_root
            .join("hicolor")
            .join("48x48")
            .join("actions")
            .join("mail-reply-sender.png");
        std::fs::create_dir_all(icon_path.parent().unwrap()).unwrap();
        std::fs::write(&icon_path, [0_u8]).unwrap();

        assert_eq!(
            find_static_icon_in_roots("mail-reply-sender", vec![icon_root]),
            Some(icon_path)
        );
        assert_eq!(
            find_static_icon_in_roots("../mail-reply-sender", vec![temp.path().join("icons")]),
            None
        );
    }

    #[test]
    fn static_icon_name_resolution_reuses_cached_index_for_roots() {
        clear_static_icon_index_cache_for_tests();

        let temp = tempfile::tempdir().unwrap();
        let icon_root = temp.path().join("icons");
        let icon_path = icon_root
            .join("hicolor")
            .join("48x48")
            .join("apps")
            .join("mail-client.png");
        std::fs::create_dir_all(icon_path.parent().unwrap()).unwrap();
        std::fs::write(&icon_path, [0_u8]).unwrap();

        let icon_root_set = vec![icon_root.clone()];
        assert_eq!(
            resolve_static_icon_name_in_roots("mail-client", icon_root_set.clone()),
            Some(icon_path.to_string_lossy().into_owned())
        );

        std::fs::remove_file(&icon_path).unwrap();
        assert_eq!(
            resolve_static_icon_name_in_roots("mail-client", icon_root_set),
            Some(icon_path.to_string_lossy().into_owned())
        );

        let missing_icon_root = temp.path().join("missing-icons");
        let missing_icon_path = missing_icon_root.join("hicolor/48x48/apps/calendar.png");
        std::fs::create_dir_all(missing_icon_path.parent().unwrap()).unwrap();
        let missing_icon_root_set = vec![missing_icon_root];
        assert_eq!(
            resolve_static_icon_name_in_roots("calendar", missing_icon_root_set.clone()),
            None
        );

        std::fs::write(&missing_icon_path, [0_u8]).unwrap();
        assert_eq!(
            resolve_static_icon_name_in_roots("calendar", missing_icon_root_set),
            None
        );
    }

    #[test]
    fn app_identity_icon_is_used_when_notification_has_no_app_icon() {
        clear_static_icon_index_cache_for_tests();

        let temp = tempfile::tempdir().unwrap();
        let icon_root = temp.path().join("icons");
        let icon_path = icon_root
            .join("hicolor")
            .join("48x48")
            .join("apps")
            .join("org.example.Mail.png");
        std::fs::create_dir_all(icon_path.parent().unwrap()).unwrap();
        std::fs::write(&icon_path, [0_u8]).unwrap();

        assert!(matches!(
            banner_app_icon_visual_in_roots(
                None,
                "org.example.Mail.desktop",
                vec![icon_root],
            ),
            Some(BannerVisual::LocalPath { path }) if path == icon_path.to_string_lossy()
        ));
    }

    #[test]
    fn specified_app_icon_takes_priority_over_app_identity_icon() {
        clear_static_icon_index_cache_for_tests();

        let temp = tempfile::tempdir().unwrap();
        let explicit_icon_path = temp.path().join("explicit.png");
        std::fs::write(&explicit_icon_path, [0_u8]).unwrap();
        let icon_root = temp.path().join("icons");
        let identity_icon_path = icon_root
            .join("hicolor")
            .join("48x48")
            .join("apps")
            .join("org.example.Mail.png");
        std::fs::create_dir_all(identity_icon_path.parent().unwrap()).unwrap();
        std::fs::write(identity_icon_path, [0_u8]).unwrap();
        let explicit_icon_path = explicit_icon_path.to_string_lossy().into_owned();
        let explicit_icon = NotificationVisualReference::local_path(explicit_icon_path.clone());

        assert!(matches!(
            banner_app_icon_visual_in_roots(
                explicit_icon.as_ref(),
                "org.example.Mail.desktop",
                vec![icon_root],
            ),
            Some(BannerVisual::LocalPath { path }) if path == explicit_icon_path
        ));
    }

    #[test]
    fn banner_actions_resolve_sender_action_icons() {
        let temp = tempfile::tempdir().unwrap();
        let icon_path = temp.path().join("reply.png");
        std::fs::write(&icon_path, [0_u8]).unwrap();
        let icon_path_string = icon_path.to_string_lossy().into_owned();
        let action = NotificationAction::new("reply", "Reply")
            .with_icon(NotificationVisualReference::local_path(icon_path_string.clone()).unwrap());

        let action = banner_action_from_notification_action(&action);

        assert_eq!(action.key, "reply");
        assert_eq!(action.label, "Reply");
        assert!(matches!(
            action.icon,
            Some(BannerVisual::LocalPath { path }) if path == icon_path_string
        ));
    }

    fn banner_view(notification_id: i64, body: &str) -> BannerView {
        BannerView {
            notification_id,
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
    use iced::widget::{container, keyed_column};
    use iced::{Alignment, Length};

    let mut stack = keyed_column![]
        .spacing(8)
        .width(Length::Fill)
        .align_items(Alignment::End);
    for banner in &state.presentation.banners {
        stack = stack.push(
            banner_stack_item_key(banner),
            notification_banner_widget(banner),
        );
    }
    if state.presentation.overflow_count > 0 {
        stack = stack.push(
            BannerStackItemKey::Overflow,
            overflow_count_banner_widget(state.presentation.overflow_count),
        );
    }

    container(stack)
        .width(Length::Fill)
        .height(Length::Shrink)
        .into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BannerStackItemKey {
    Notification(i64),
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
            .height(22)
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
        .on_press(BannerUiMessage::ActivateRequested(banner.notification_id));
    let dismiss = close_button(banner.notification_id);

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
        .height(FULL_BANNER_HEIGHT)
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
        .height(28);

    for action in &banner.actions {
        actions = actions.push(
            button(banner_action_content(action))
                .padding([4, 8])
                .height(28)
                .width(Length::Fill)
                .style(content_button_style)
                .on_press(BannerUiMessage::InvokeActionRequested(
                    banner.notification_id,
                    action.key.clone(),
                )),
        );
    }

    container(actions).height(28).width(Length::Fill).into()
}

fn banner_action_content(action: &BannerAction) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{row, text};
    use iced::{Alignment, Length};

    let label = text(action.label.as_str())
        .size(12)
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
    use iced::widget::{column, container, text};

    if banner.body_segments.is_empty() {
        return text(&banner.body)
            .size(13)
            .width(Length::Fill)
            .height(BODY_CONTENT_TEXT_HEIGHT)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
            .style(subdued_text_style)
            .into();
    }

    if !banner
        .body_segments
        .iter()
        .any(|segment| matches!(segment, BannerBodySegment::Image { .. }))
    {
        return banner_body_text_widget(
            banner_body_text_runs(&banner.body_segments),
            BODY_CONTENT_TEXT_HEIGHT,
        );
    }

    let mut body = column![]
        .spacing(4)
        .width(Length::Fill)
        .height(BODY_CONTENT_RICH_HEIGHT);
    let mut pending_text = Vec::new();

    for segment in &banner.body_segments {
        match segment {
            BannerBodySegment::Text { text, style } => {
                pending_text.push(NotificationBodyRun::styled(text.clone(), *style));
            }
            BannerBodySegment::Image { image } => {
                if !pending_text.is_empty() {
                    body = body.push(banner_body_text_widget(
                        std::mem::take(&mut pending_text),
                        BODY_CONTENT_TEXT_HEIGHT,
                    ));
                }
                body = body.push(body_image_visual(image));
            }
        }
    }

    if !pending_text.is_empty() {
        body = body.push(banner_body_text_widget(
            pending_text,
            BODY_CONTENT_TEXT_HEIGHT,
        ));
    }

    container(body)
        .width(Length::Fill)
        .height(BODY_CONTENT_RICH_HEIGHT)
        .clip(true)
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
    height: u32,
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
        .height(height)
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
        BannerVisual::LocalPath { path } => iced::widget::image::Handle::from_path(path),
    }
}

fn close_button<'a>(notification_id: i64) -> iced::widget::Button<'a, BannerUiMessage> {
    use iced::widget::{button, text};

    button(text("x").size(14))
        .padding(0)
        .width(28)
        .height(28)
        .style(close_button_style)
        .on_press(BannerUiMessage::Dismiss(notification_id))
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

const FULL_BANNER_HEIGHT: u32 = 140;
const DENSE_BANNER_HEIGHT: u32 = 60;
const BANNER_STACK_SPACING: u32 = 8;

fn banner_surface_size(count: usize, overflow_count: usize) -> (u32, u32) {
    if count == 0 && overflow_count == 0 {
        (1, 1)
    } else {
        let count = count.min(3) as u32;
        let mut height =
            count * FULL_BANNER_HEIGHT + count.saturating_sub(1) * BANNER_STACK_SPACING;
        if overflow_count > 0 {
            let spacing = if count > 0 { BANNER_STACK_SPACING } else { 0 };
            height += spacing + DENSE_BANNER_HEIGHT;
        }

        (360, height.max(FULL_BANNER_HEIGHT))
    }
}

fn banner_surface_resize(
    current_size: &mut (u32, u32),
    presentation: &BannerPresentation,
) -> Option<(u32, u32)> {
    let next_size = banner_surface_size(presentation.banners.len(), presentation.overflow_count);
    if *current_size == next_size {
        None
    } else {
        *current_size = next_size;
        Some(next_size)
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
    pub notification_id: i64,
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
            NotificationVisualReference::IconName(icon_name) => {
                resolve_static_icon_name(icon_name).map(|path| Self::LocalPath { path })
            }
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
    banner_app_icon_visual_in_roots(app_icon, app_identity, icon_search_roots())
}

fn banner_app_icon_visual_in_roots(
    app_icon: Option<&NotificationVisualReference>,
    app_identity: &str,
    roots: Vec<PathBuf>,
) -> Option<BannerVisual> {
    match app_icon {
        Some(app_icon) => BannerVisual::from_reference(app_icon),
        None => app_identity_icon_visual_in_roots(app_identity, roots),
    }
}

fn app_identity_icon_visual_in_roots(
    app_identity: &str,
    roots: Vec<PathBuf>,
) -> Option<BannerVisual> {
    let index = cached_static_icon_index(roots);
    app_identity_icon_name_candidates(app_identity)
        .into_iter()
        .find_map(|icon_name| {
            index
                .resolve(&icon_name)
                .map(|path| BannerVisual::LocalPath {
                    path: path.to_string_lossy().into_owned(),
                })
        })
}

fn app_identity_icon_name_candidates(app_identity: &str) -> Vec<String> {
    let Some(app_identity) = normalize_static_icon_name(app_identity) else {
        return Vec::new();
    };
    let mut candidates = vec![app_identity.clone()];
    let suffix = ".desktop";
    if app_identity.to_ascii_lowercase().ends_with(suffix) {
        let stripped = app_identity[..app_identity.len() - suffix.len()].trim();
        if let Some(stripped) = normalize_static_icon_name(stripped)
            && stripped != app_identity
        {
            candidates.push(stripped);
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
    has_static_raster_extension(path)
        && std::fs::metadata(Path::new(path))
            .map(|metadata| metadata.is_file() && metadata.len() <= MAX_FILE_BACKED_VISUAL_BYTES)
            .unwrap_or(false)
}

fn resolve_static_icon_name(icon_name: &str) -> Option<String> {
    resolve_static_icon_name_in_roots(icon_name, icon_search_roots())
}

fn resolve_static_icon_name_in_roots(icon_name: &str, roots: Vec<PathBuf>) -> Option<String> {
    let icon_name = normalize_static_icon_name(icon_name)?;
    cached_static_icon_index(roots)
        .resolve(&icon_name)
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
fn find_static_icon_in_roots(
    icon_name: &str,
    roots: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    let icon_name = normalize_static_icon_name(icon_name)?;

    for root in roots {
        if let Some(path) = find_static_icon_in_root(&root, &icon_name) {
            return Some(path);
        }
    }

    None
}

fn normalize_static_icon_name(icon_name: &str) -> Option<String> {
    let icon_name = icon_name.trim();
    if icon_name.is_empty() || icon_name.contains('/') || icon_name.contains('\\') {
        None
    } else {
        Some(icon_name.to_string())
    }
}

#[derive(Debug, Default)]
struct StaticIconIndex {
    paths_by_name: HashMap<String, PathBuf>,
}

impl StaticIconIndex {
    fn from_roots(roots: &[PathBuf]) -> Self {
        let mut index = Self::default();
        for root in roots {
            index.add_root(root);
        }
        index
    }

    fn resolve(&self, icon_name: &str) -> Option<PathBuf> {
        self.paths_by_name.get(icon_name).cloned()
    }

    fn add_root(&mut self, root: &Path) {
        let mut stack = vec![(root.to_path_buf(), 0_usize)];

        while let Some((directory, depth)) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name());

            let mut child_directories = Vec::new();
            for entry in entries {
                let path = entry.path();
                if let Some(icon_name) = supported_static_icon_file_name(&path)
                    && supported_local_image_path(path.to_string_lossy().as_ref())
                {
                    self.paths_by_name
                        .entry(icon_name)
                        .or_insert_with(|| path.clone());
                }

                if depth < ICON_SEARCH_MAX_DEPTH
                    && entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
                {
                    child_directories.push(path);
                }
            }

            child_directories.sort();
            for child in child_directories.into_iter().rev() {
                stack.push((child, depth + 1));
            }
        }
    }
}

fn supported_static_icon_file_name(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?;
    if !STATIC_ICON_EXTENSIONS
        .iter()
        .any(|supported| extension.eq_ignore_ascii_case(supported))
    {
        return None;
    }

    path.file_stem()?.to_str().map(str::to_string)
}

fn cached_static_icon_index(roots: Vec<PathBuf>) -> Arc<StaticIconIndex> {
    let cache = static_icon_index_cache();
    if let Some(index) = cache.lock().unwrap().get(&roots).cloned() {
        return index;
    }

    let index = Arc::new(StaticIconIndex::from_roots(&roots));
    let mut cache = cache.lock().unwrap();
    Arc::clone(cache.entry(roots).or_insert(index))
}

fn static_icon_index_cache() -> &'static Mutex<HashMap<Vec<PathBuf>, Arc<StaticIconIndex>>> {
    static CACHE: OnceLock<Mutex<HashMap<Vec<PathBuf>, Arc<StaticIconIndex>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn clear_static_icon_index_cache_for_tests() {
    static_icon_index_cache().lock().unwrap().clear();
}

#[cfg(test)]
fn find_static_icon_in_root(root: &Path, icon_name: &str) -> Option<PathBuf> {
    let file_names = STATIC_ICON_EXTENSIONS
        .iter()
        .map(|extension| format!("{icon_name}.{extension}"))
        .collect::<Vec<_>>();
    let mut stack = vec![(root.to_path_buf(), 0_usize)];

    while let Some((directory, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());

        let mut child_directories = Vec::new();
        for entry in entries {
            let path = entry.path();
            let file_name = entry.file_name();
            if file_names
                .iter()
                .any(|expected| file_name == OsStr::new(expected))
                && supported_local_image_path(path.to_string_lossy().as_ref())
            {
                return Some(path);
            }

            if depth < ICON_SEARCH_MAX_DEPTH
                && entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
            {
                child_directories.push(path);
            }
        }

        child_directories.sort();
        for child in child_directories.into_iter().rev() {
            stack.push((child, depth + 1));
        }
    }

    None
}

fn icon_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        push_data_dir_icon_roots(&mut roots, data_home);
    } else if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        push_data_dir_icon_roots(&mut roots, home.join(".local/share"));
    }

    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        push_unique_path(&mut roots, home.join(".icons"));
    }

    let data_dirs = std::env::var_os("XDG_DATA_DIRS")
        .map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_else(|| {
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        });
    for data_dir in data_dirs {
        push_data_dir_icon_roots(&mut roots, data_dir);
    }

    roots
}

fn push_data_dir_icon_roots(roots: &mut Vec<PathBuf>, data_dir: PathBuf) {
    push_unique_path(roots, data_dir.join("icons"));
    push_unique_path(roots, data_dir.join("pixmaps"));
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

#[derive(Clone)]
pub(crate) struct PresentationCoordinator {
    enabled: bool,
    banner_presenter: Arc<dyn BannerPresenter>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl PresentationCoordinator {
    pub(crate) fn new(enabled: bool, banner_presenter: Arc<dyn BannerPresenter>) -> Self {
        Self {
            enabled,
            banner_presenter,
            last_error: Arc::new(Mutex::new(None)),
        }
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
        self.last_error.lock().unwrap().clone()
    }
}

#[derive(Clone, Default)]
pub struct NotificationBannerState {
    visible: HashMap<i64, NotificationThread>,
    visible_order: Vec<i64>,
    overflow: HashMap<i64, NotificationThread>,
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

    fn remove_banner(&mut self, notification_id: i64) {
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
