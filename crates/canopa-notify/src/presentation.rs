use crate::InternalNotificationId;
use crate::{
    BannerLifecycleChange, DeliveryOutcome, NotificationAction, NotificationBodyContent,
    NotificationBodyImageSource, NotificationBodySegment, NotificationBodyStyle,
    NotificationImageData, NotificationStatus, NotificationThread, NotificationVisualReference,
    Urgency,
};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[cfg(feature = "gui")]
mod renderer;
#[cfg(feature = "gui")]
pub use renderer::IcedLayerShellBannerPresenter;

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

    #[cfg(feature = "gui")]
    fn is_complete(&self) -> bool {
        self.callbacks.lock().unwrap().is_empty()
    }

    #[cfg(feature = "gui")]
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
    /// Retire the adapter and its pending display callbacks.
    fn shutdown(&self) {}

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
    pub appearance: canopa_theme::Appearance,
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
            appearance: Default::default(),
        }
    }
}

/// Static icon selection. Scale selects a source asset; it does not scale the UI.
/// Without an explicitly configured theme, icons follow the desktop selection
/// (GTK `gtk-icon-theme-name`, see `canopa_file::icon_theme`), then `theme`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BannerIconSettings {
    pub theme: String,
    pub scale: u16,
    #[serde(default)]
    pub follow_desktop: bool,
}

impl Default for BannerIconSettings {
    fn default() -> Self {
        Self {
            theme: "hicolor".into(),
            scale: 1,
            follow_desktop: true,
        }
    }
}

impl BannerIconSettings {
    /// The theme to search first: the configured one, or the current desktop theme.
    pub fn effective_theme(&self) -> String {
        if self.follow_desktop
            && let Some(theme) = canopa_file::icon_theme::desktop_theme()
        {
            return theme;
        }
        self.theme.clone()
    }

    /// Follow the desktop theme with an explicit source scale.
    pub fn desktop(scale: u16) -> anyhow::Result<Self> {
        Ok(Self {
            follow_desktop: true,
            ..Self::new("hicolor".into(), scale)?
        })
    }

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
        Ok(Self {
            theme,
            scale,
            follow_desktop: false,
        })
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
    OpenLink {
        target: BannerTarget,
        segment_index: usize,
        activation_token: Option<String>,
    },
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
    /// Send composed reply text with the revision's reply action.
    Reply {
        target: BannerTarget,
        text: String,
        activation_token: Option<String>,
    },
    /// The composer opened or closed; the banner-hide timer waits while open.
    Composing {
        target: BannerTarget,
        composing: bool,
    },
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

/// The summary shown instead of content while content is hidden.
pub const HIDDEN_CONTENT_SUMMARY: &str = "New notification";

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
    /// A reply composer for this revision (portal `im.reply-with-text`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<BannerReply>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BannerReply {
    /// The action whose button opens the composer and which the reply invokes.
    pub action_key: String,
    #[serde(default)]
    pub placeholder: Option<String>,
    /// Open the composer immediately (the Center's "Reply…").
    #[serde(default)]
    pub compose: bool,
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
    /// Unresolved local theme references, in preference order (at most eight).
    IconNames {
        names: Vec<String>,
    },
    /// An unresolved application identity without a sender icon: the matching
    /// desktop entry's `Icon=` when installed, else the identities as icon names.
    DesktopEntry {
        ids: Vec<String>,
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
    Link {
        text: String,
        style: NotificationBodyStyle,
        segment_index: usize,
    },
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
            NotificationVisualReference::IconNames(names) => Some(Self::IconNames {
                names: names.clone(),
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

    for (segment_index, segment) in body_content.segments.iter().enumerate() {
        match segment {
            NotificationBodySegment::Link { text, style, .. } => {
                banner_segments.push(BannerBodySegment::Link {
                    text: text.clone(),
                    style: *style,
                    segment_index,
                });
            }
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

    /// Screen-sharing presentation: application identity and actions only.
    pub fn with_hidden_content(mut self) -> Self {
        self.summary = HIDDEN_CONTENT_SUMMARY.into();
        self.body.clear();
        self.body_content = NotificationBodyContent::default();
        self.body_segments.clear();
        self.image = None;
        self.progress = None;
        // Typing a reply would reveal the conversation while sharing.
        self.reply = None;
        self
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
            reply: None,
        }
    }
}

fn banner_app_icon_visual(
    app_icon: Option<&NotificationVisualReference>,
    app_identity: &str,
) -> Option<BannerVisual> {
    match app_icon {
        Some(reference) => BannerVisual::from_reference(reference),
        None => Some(BannerVisual::DesktopEntry {
            ids: app_identity_icon_name_candidates(app_identity),
        }),
    }
}

/// Ordered source-icon fallback shared by banners and the Notification Center.
pub fn app_identity_icon_name_candidates(app_identity: &str) -> Vec<String> {
    let app_identity = app_identity.trim();
    if !valid_icon_name(app_identity) {
        return Vec::new();
    }
    let mut candidates = vec![app_identity.to_owned()];
    let suffix = ".desktop";
    if app_identity.to_ascii_lowercase().ends_with(suffix) {
        let stripped = app_identity[..app_identity.len() - suffix.len()].trim();
        if valid_icon_name(stripped) && stripped != app_identity {
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

/// Any absolute path: the renderer admits PNG, JPEG and SVG by content, so
/// extensionless temporary files (Chromium, Electron) are kept.
fn supported_local_image_path(path: &str) -> bool {
    Path::new(path).is_absolute()
}

#[derive(Clone)]
pub(crate) struct PresentationCoordinator {
    enabled: bool,
    banner_presenter: Arc<dyn BannerPresenter>,
    renderer_state: Option<tokio::sync::watch::Receiver<BannerRendererState>>,
    last_error: Arc<Mutex<Option<String>>>,
    icon_settings: Arc<Mutex<BannerIconSettings>>,
    appearance: Arc<Mutex<canopa_theme::Appearance>>,
}

impl PresentationCoordinator {
    pub(crate) async fn shutdown(&self) -> anyhow::Result<()> {
        self.banner_presenter.shutdown();
        let Some(mut state) = self.renderer_state.clone() else {
            return Ok(());
        };
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while *state.borrow_and_update() != BannerRendererState::Stopped {
                if state.changed().await.is_err() {
                    break;
                }
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("banner renderer shutdown timed out"))
    }

    pub(crate) fn new(enabled: bool, banner_presenter: Arc<dyn BannerPresenter>) -> Self {
        Self {
            enabled,
            renderer_state: enabled.then(|| banner_presenter.renderer_state()).flatten(),
            banner_presenter,
            last_error: Arc::new(Mutex::new(None)),
            appearance: Default::default(),
            icon_settings: Arc::new(Mutex::new(BannerIconSettings::default())),
        }
    }

    /// Headless Mode never displays banners, so no display-based timeout starts.
    pub(crate) fn presents_banners(&self) -> bool {
        self.enabled
    }

    pub(crate) fn set_appearance(&self, appearance: canopa_theme::Appearance) {
        *self.appearance.lock().unwrap() = appearance;
    }

    pub(crate) fn appearance(&self) -> canopa_theme::Appearance {
        self.appearance.lock().unwrap().clone()
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

    /// Whether a banner or overflow slot currently presents this notification.
    pub(crate) fn shows(&self, notification_id: InternalNotificationId) -> bool {
        self.visible.contains_key(&notification_id) || self.overflow.contains_key(&notification_id)
    }

    pub(crate) fn overflow_count(&self) -> usize {
        self.overflow.len()
    }

    pub(crate) fn overflow_revisions(&self) -> Vec<(InternalNotificationId, i64)> {
        self.overflow
            .values()
            .map(|thread| (thread.id, thread.revision))
            .collect()
    }

    pub fn banner_presentation(&self) -> BannerPresentation {
        BannerPresentation {
            banners: self.visible_banners(),
            overflow_count: self.overflow_count(),
            icon_settings: BannerIconSettings::default(),
            appearance: Default::default(),
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

fn valid_icon_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !matches!(name, "." | "..")
        && !name
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
}
