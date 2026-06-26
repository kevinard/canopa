use crate::domain::has_static_raster_extension;
use crate::{
    BannerLifecycleChange, DeliveryOutcome, NotificationAction, NotificationBodyMarkup,
    NotificationImageData, NotificationStatus, NotificationThread, NotificationVisualReference,
    Urgency,
};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

const MAX_FILE_BACKED_VISUAL_BYTES: u64 = 16 * 1024 * 1024;
const ICON_SEARCH_MAX_DEPTH: usize = 8;
const STATIC_ICON_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

pub trait BannerPresenter: Send + Sync {
    fn replace_visible_banners(&self, banners: Vec<BannerView>) -> anyhow::Result<()>;

    fn replace_banner_presentation(&self, presentation: BannerPresentation) -> anyhow::Result<()> {
        self.replace_visible_banners(presentation.banners)
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
    Activate(i64),
    Dismiss(i64),
    InvokeAction {
        notification_id: i64,
        action_key: String,
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
        self.updates
            .lock()
            .unwrap()
            .try_send(BannerRuntimeInput::Replace(presentation))
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
    Replace(BannerPresentation),
}

#[derive(Clone)]
struct BannerUiState {
    runtime_id: u64,
    presentation: BannerPresentation,
    surface_size: (u32, u32),
    interactions: mpsc::Sender<BannerInteraction>,
}

#[iced_layershell::to_layer_message]
#[derive(Clone, Debug)]
enum BannerUiMessage {
    Runtime(BannerRuntimeInput),
    Activate(i64),
    Dismiss(i64),
    InvokeAction(i64, String),
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
    iced::Subscription::run_with(state.runtime_id, banner_update_stream)
        .map(BannerUiMessage::Runtime)
}

fn banner_ui_update(
    state: &mut BannerUiState,
    message: BannerUiMessage,
) -> iced::Task<BannerUiMessage> {
    match message {
        BannerUiMessage::Runtime(BannerRuntimeInput::Replace(presentation)) => {
            let size = banner_surface_resize(&mut state.surface_size, &presentation);
            state.presentation = presentation;
            match size {
                Some(size) => iced::Task::done(BannerUiMessage::SizeChange(size)),
                None => iced::Task::none(),
            }
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
        BannerUiMessage::InvokeAction(notification_id, action_key) => {
            state
                .interactions
                .send(BannerInteraction::InvokeAction {
                    notification_id,
                    action_key,
                })
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
            body_markup: NotificationBodyMarkup::plain(body),
            urgency: Urgency::Normal,
            progress: None,
            image: None,
            default_action_key: None,
            actions: Vec::new(),
            placement: BannerPlacement::TopRightFocusedOutputWithFallback,
            focus_behavior: BannerFocusBehavior::DoesNotTakeFocus,
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
        .on_press(BannerUiMessage::Activate(banner.notification_id));
    let dismiss = close_button(banner.notification_id);

    let mut header = row![app_identity_visual(banner),]
        .spacing(10)
        .align_y(Alignment::Start);
    if let Some(image) = &banner.image {
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
                .on_press(BannerUiMessage::InvokeAction(
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
    use iced::widget::{rich_text, span, text};
    use iced::{Font, Length, font};

    if banner.body_markup.is_empty() {
        return text(&banner.body)
            .size(13)
            .width(Length::Fill)
            .height(34)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
            .style(subdued_text_style)
            .into();
    }

    let spans: Vec<iced::widget::text::Span<'_, (), iced::Font>> = banner
        .body_markup
        .runs
        .iter()
        .map(|run| {
            let mut body_span = span(run.text.as_str());
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
        .height(34)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
        .style(subdued_text_style)
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

fn app_identity_visual(banner: &BannerView) -> iced::Element<'_, BannerUiMessage> {
    match &banner.app_icon {
        Some(icon) => visual_image(icon, 34),
        None => app_badge(&banner.app_name, banner.urgency),
    }
}

fn notification_image_visual(image: &BannerVisual) -> iced::Element<'_, BannerUiMessage> {
    visual_image(image, 58)
}

fn visual_image(visual: &BannerVisual, size: u16) -> iced::Element<'_, BannerUiMessage> {
    use iced::widget::{container, image};

    let handle = match visual {
        BannerVisual::Rgba {
            width,
            height,
            pixels,
        } => image::Handle::from_rgba(*width, *height, pixels.clone()),
        BannerVisual::LocalPath { path } => image::Handle::from_path(path),
    };

    let size = u32::from(size);
    container(image(handle).width(size).height(size))
        .center(size)
        .clip(true)
        .style(visual_image_container_style)
        .into()
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
    pub body_markup: NotificationBodyMarkup,
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
        Self {
            notification_id: thread.id,
            app_identity: notification.app_identity,
            app_name: notification.app_name,
            app_icon: notification
                .app_icon
                .as_ref()
                .and_then(BannerVisual::from_reference),
            summary: notification.summary,
            body: notification.body,
            body_markup: notification.body_markup,
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
    find_static_icon_in_roots(icon_name, icon_search_roots())
        .map(|path| path.to_string_lossy().into_owned())
}

fn find_static_icon_in_roots(
    icon_name: &str,
    roots: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    let icon_name = icon_name.trim();
    if icon_name.is_empty() || icon_name.contains('/') || icon_name.contains('\\') {
        return None;
    }

    for root in roots {
        if let Some(path) = find_static_icon_in_root(&root, icon_name) {
            return Some(path);
        }
    }

    None
}

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
}

impl PresentationCoordinator {
    pub(crate) fn new(enabled: bool, banner_presenter: Arc<dyn BannerPresenter>) -> Self {
        Self {
            enabled,
            banner_presenter,
        }
    }

    pub(crate) fn replace_banner_presentation(
        &self,
        presentation: BannerPresentation,
    ) -> anyhow::Result<()> {
        if self.enabled {
            self.banner_presenter
                .replace_banner_presentation(presentation)?;
        }
        Ok(())
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
            BannerLifecycleChange::ReconsiderVisible(thread) => {
                if self.visible.contains_key(&thread.id) {
                    self.update_visible_banner(thread);
                } else if self.overflow.contains_key(&thread.id) {
                    self.reconsider_overflow(thread);
                }
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

    fn reconsider_overflow(&mut self, thread: NotificationThread) {
        if is_banner_eligible(&thread) {
            self.overflow.insert(thread.id, thread);
        } else {
            self.overflow.remove(&thread.id);
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
