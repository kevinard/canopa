use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

const MAX_NOTIFICATION_IMAGE_DIMENSION: u32 = 4096;
const MAX_NOTIFICATION_IMAGE_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_NOTIFICATION_SOUND_THEME: &str = "freedesktop";
pub const DEFAULT_NOTIFICATION_SOUND_NAME: &str = "message-new-instant";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotificationTimeoutDefaults {
    pub low: Option<Duration>,
    pub normal: Option<Duration>,
    pub critical: Option<Duration>,
}

impl NotificationTimeoutDefaults {
    pub fn new(
        low: Option<Duration>,
        normal: Option<Duration>,
        critical: Option<Duration>,
    ) -> Self {
        Self {
            low,
            normal,
            critical,
        }
    }

    fn for_urgency(self, urgency: Urgency) -> Option<Duration> {
        match urgency {
            Urgency::Low => self.low,
            Urgency::Normal => self.normal,
            Urgency::Critical => self.critical,
        }
    }
}

impl Default for NotificationTimeoutDefaults {
    fn default() -> Self {
        Self {
            low: Some(Duration::from_secs(5)),
            normal: Some(Duration::from_secs(5)),
            critical: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationInput {
    pub app_name: String,
    pub desktop_entry: Option<String>,
    pub replaces_id: Option<i64>,
    pub app_icon: Option<NotificationVisualReference>,
    pub summary: String,
    pub body: String,
    pub expire_timeout: NotificationTimeout,
    pub urgency: Urgency,
    pub category: Option<String>,
    pub progress: Option<i64>,
    pub image_data: Option<NotificationImageData>,
    pub image_reference: Option<NotificationVisualReference>,
    pub actions: Vec<NotificationAction>,
    pub resident: bool,
    #[serde(default)]
    pub transient: bool,
    #[serde(default)]
    pub suppress_sound: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<NotificationSound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_position: Option<NotificationPosition>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NotificationSoundRequest {
    pub suppress: bool,
    pub sound: Option<NotificationSound>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationSoundPlayback {
    pub notification_id: i64,
    pub app_identity: String,
    pub summary: String,
    pub requested_sound: Option<NotificationSound>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationSoundSettings {
    pub enabled: bool,
    pub theme: String,
    pub default_sound: String,
}

impl Default for NotificationSoundSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            theme: DEFAULT_NOTIFICATION_SOUND_THEME.to_string(),
            default_sound: DEFAULT_NOTIFICATION_SOUND_NAME.to_string(),
        }
    }
}

impl NotificationSoundSettings {
    pub fn new(
        enabled: bool,
        theme: impl Into<String>,
        default_sound: impl Into<String>,
    ) -> anyhow::Result<Self> {
        let theme = valid_sound_theme(theme)?;
        let Some(NotificationSound::Name(default_sound)) = NotificationSound::name(default_sound)
        else {
            anyhow::bail!("default notification sound must be a sound name");
        };

        Ok(Self {
            enabled,
            theme,
            default_sound,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationTimeout {
    #[default]
    ServerDefault,
    Never,
    Milliseconds(u64),
}

impl NotificationTimeout {
    pub fn from_freedesktop_expire_timeout(expire_timeout: i32) -> Self {
        match expire_timeout {
            0 => Self::Never,
            value if value > 0 => Self::Milliseconds(value as u64),
            _ => Self::ServerDefault,
        }
    }

    pub(crate) fn duration(
        self,
        urgency: Urgency,
        defaults: NotificationTimeoutDefaults,
    ) -> Option<Duration> {
        match self {
            Self::ServerDefault => defaults.for_urgency(urgency),
            Self::Never => None,
            Self::Milliseconds(milliseconds) => Some(Duration::from_millis(milliseconds)),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcceptedNotification {
    pub app_identity: String,
    pub app_name: String,
    pub replaces_id: Option<i64>,
    pub app_icon: Option<NotificationVisualReference>,
    pub summary: String,
    pub body: String,
    pub body_content: NotificationBodyContent,
    pub raw_body: String,
    pub urgency: Urgency,
    pub category: Option<String>,
    pub progress: Option<i64>,
    pub image_reference: Option<NotificationVisualReference>,
    pub has_image_payload: bool,
    pub actions: Vec<NotificationAction>,
    pub resident: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationBodyContent {
    pub segments: Vec<NotificationBodySegment>,
}

impl NotificationBodyContent {
    pub fn plain(text: impl Into<String>) -> Self {
        let text = text.into();
        if text.is_empty() {
            Self::default()
        } else {
            Self::from_segments(vec![NotificationBodySegment::plain_text(text)])
        }
    }

    pub fn plain_text(&self) -> String {
        let mut text = String::new();
        for segment in &self.segments {
            match segment {
                NotificationBodySegment::Text { text: segment, .. } => text.push_str(segment),
                NotificationBodySegment::Image {
                    image: NotificationBodyImage { alt: Some(alt), .. },
                } => text.push_str(alt),
                NotificationBodySegment::Image { .. } => {}
            }
        }
        text
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn text_runs(&self) -> Vec<NotificationBodyRun> {
        self.segments
            .iter()
            .filter_map(|segment| match segment {
                NotificationBodySegment::Text { text, style } => {
                    Some(NotificationBodyRun::styled(text.clone(), *style))
                }
                NotificationBodySegment::Image { .. } => None,
            })
            .collect()
    }

    pub fn from_text_runs(runs: Vec<NotificationBodyRun>) -> Self {
        Self::from_segments(
            runs.into_iter()
                .map(|run| NotificationBodySegment::styled_text(run.text, run.style))
                .collect(),
        )
    }

    pub fn from_segments(segments: Vec<NotificationBodySegment>) -> Self {
        let mut normalized: Vec<NotificationBodySegment> = Vec::new();
        for segment in segments {
            match segment {
                NotificationBodySegment::Text { text, .. } if text.is_empty() => {}
                NotificationBodySegment::Text { text, style } => match normalized.last_mut() {
                    Some(NotificationBodySegment::Text {
                        text: previous_text,
                        style: previous_style,
                    }) if *previous_style == style => previous_text.push_str(&text),
                    _ => normalized.push(NotificationBodySegment::Text { text, style }),
                },
                NotificationBodySegment::Image { image } => {
                    normalized.push(NotificationBodySegment::Image { image });
                }
            }
        }

        Self {
            segments: normalized,
        }
    }

    pub(crate) fn has_meaningful_content(&self) -> bool {
        !self.plain_text().trim().is_empty()
            || self
                .segments
                .iter()
                .any(|segment| matches!(segment, NotificationBodySegment::Image { .. }))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum NotificationBodySegment {
    Text {
        text: String,
        style: NotificationBodyStyle,
    },
    Image {
        image: NotificationBodyImage,
    },
}

impl NotificationBodySegment {
    pub fn plain_text(text: impl Into<String>) -> Self {
        Self::styled_text(text, NotificationBodyStyle::default())
    }

    pub fn styled_text(text: impl Into<String>, style: NotificationBodyStyle) -> Self {
        Self::Text {
            text: text.into(),
            style,
        }
    }

    pub fn body_image(image: NotificationBodyImage) -> Self {
        Self::Image { image }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationBodyImage {
    pub source: NotificationBodyImageSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

impl NotificationBodyImage {
    pub fn local_path(path: impl Into<String>, alt: Option<String>) -> Self {
        Self {
            source: NotificationBodyImageSource::local_path(path),
            alt,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum NotificationBodyImageSource {
    LocalPath { path: String },
}

impl NotificationBodyImageSource {
    pub fn local_path(path: impl Into<String>) -> Self {
        Self::LocalPath { path: path.into() }
    }

    pub(crate) fn from_body_image_src(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }

        let path = if let Some((scheme, rest)) = value.split_once(':')
            && is_uri_scheme(scheme)
        {
            scheme
                .eq_ignore_ascii_case("file")
                .then(|| local_path_string_from_file_uri(rest))
                .flatten()?
        } else {
            value.to_string()
        };

        if Path::new(&path).is_absolute() && has_static_raster_extension(&path) {
            Some(Self::local_path(path))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationBodyRun {
    pub text: String,
    pub style: NotificationBodyStyle,
}

impl NotificationBodyRun {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: NotificationBodyStyle::default(),
        }
    }

    pub fn styled(text: impl Into<String>, style: NotificationBodyStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationBodyStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum NotificationVisualReference {
    IconName(String),
    LocalPath(String),
}

impl NotificationVisualReference {
    pub fn icon_name(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        let value = value.trim();
        (!value.is_empty()).then(|| Self::IconName(value.to_string()))
    }

    pub fn local_path(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        let value = value.trim();
        (!value.is_empty()).then(|| Self::LocalPath(value.to_string()))
    }

    pub(crate) fn from_sender_reference(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }

        if let Some((scheme, rest)) = value.split_once(':')
            && is_uri_scheme(scheme)
        {
            return (scheme.eq_ignore_ascii_case("file"))
                .then(|| local_path_from_file_uri(rest))
                .flatten();
        }

        if value.starts_with('/')
            || value.starts_with("./")
            || value.starts_with("../")
            || value.starts_with('~')
            || value.contains('/')
            || has_static_raster_extension(value)
        {
            Self::local_path(value)
        } else {
            Self::icon_name(value)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationImageData {
    pub width: u32,
    pub height: u32,
    pub rowstride: u32,
    pub has_alpha: bool,
    pub channels: u8,
    pub data: Vec<u8>,
}

impl NotificationImageData {
    pub fn from_freedesktop(
        width: i32,
        height: i32,
        rowstride: i32,
        has_alpha: bool,
        bits_per_sample: i32,
        channels: i32,
        data: Vec<u8>,
    ) -> Option<Self> {
        if width <= 0 || height <= 0 || rowstride <= 0 || bits_per_sample != 8 {
            return None;
        }

        let width = width as u32;
        let height = height as u32;
        let rowstride = rowstride as u32;
        let channels = channels as u8;
        let expected_channels = if has_alpha { 4 } else { 3 };
        if channels != expected_channels {
            return None;
        }

        if width > MAX_NOTIFICATION_IMAGE_DIMENSION
            || height > MAX_NOTIFICATION_IMAGE_DIMENSION
            || data.len() > MAX_NOTIFICATION_IMAGE_BYTES
        {
            return None;
        }

        let row_bytes = width.checked_mul(channels as u32)?;
        if rowstride < row_bytes {
            return None;
        }

        let required = (height.saturating_sub(1) as usize)
            .checked_mul(rowstride as usize)?
            .checked_add(row_bytes as usize)?;
        if data.len() < required {
            return None;
        }

        Some(Self {
            width,
            height,
            rowstride,
            has_alpha,
            channels,
            data,
        })
    }

    pub fn rgba_pixels(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(self.width as usize * self.height as usize * 4);
        for y in 0..self.height as usize {
            let row_start = y * self.rowstride as usize;
            for x in 0..self.width as usize {
                let index = row_start + x * self.channels as usize;
                output.push(self.data[index]);
                output.push(self.data[index + 1]);
                output.push(self.data[index + 2]);
                output.push(if self.has_alpha {
                    self.data[index + 3]
                } else {
                    255
                });
            }
        }
        output
    }
}

pub(crate) fn has_static_raster_extension(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.ends_with(".png") || value.ends_with(".jpg") || value.ends_with(".jpeg")
}

fn is_uri_scheme(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
}

fn local_path_from_file_uri(rest: &str) -> Option<NotificationVisualReference> {
    NotificationVisualReference::local_path(local_path_string_from_file_uri(rest)?)
}

fn local_path_string_from_file_uri(rest: &str) -> Option<String> {
    let path = if let Some(path) = rest.strip_prefix("//localhost") {
        path
    } else if rest.starts_with("///") {
        rest.strip_prefix("//").unwrap_or(rest)
    } else if rest.starts_with("//") {
        return None;
    } else {
        rest
    };
    let path = percent_decode_path(path);
    (!path.trim().is_empty()).then_some(path)
}

fn percent_decode_path(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3])
            && let Ok(decoded) = u8::from_str_radix(hex, 16)
        {
            output.push(decoded);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(output).unwrap_or_else(|_| value.to_string())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationAction {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<NotificationVisualReference>,
}

impl NotificationAction {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            icon: None,
        }
    }

    pub fn with_icon(mut self, icon: NotificationVisualReference) -> Self {
        self.icon = Some(icon);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum NotificationSound {
    Name(String),
    File(String),
}

impl NotificationSound {
    pub fn name(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() || value.contains('/') {
            return None;
        }

        if let Some((scheme, _rest)) = value.split_once(':')
            && is_uri_scheme(scheme)
        {
            return None;
        }

        Some(Self::Name(value.to_string()))
    }

    pub fn file(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return None;
        }

        if let Some((scheme, rest)) = value.split_once(':')
            && is_uri_scheme(scheme)
        {
            return scheme
                .eq_ignore_ascii_case("file")
                .then(|| local_path_string_from_file_uri(rest))
                .flatten()
                .and_then(Self::file);
        }

        Some(Self::File(value.to_string()))
    }
}

pub(crate) fn valid_sound_theme(value: impl Into<String>) -> anyhow::Result<String> {
    let value = value.into();
    let value = value.trim();
    if value.is_empty() || value.contains('/') {
        anyhow::bail!("notification sound theme must be a non-empty theme name");
    }

    if let Some((scheme, _rest)) = value.split_once(':')
        && is_uri_scheme(scheme)
    {
        anyhow::bail!("notification sound theme must not be a URI");
    }

    Ok(value.to_string())
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationPosition {
    pub x: i32,
    pub y: i32,
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

    pub(crate) fn from_persisted_str(value: &str) -> Self {
        match value {
            "low" => Urgency::Low,
            "critical" => Urgency::Critical,
            _ => Urgency::Normal,
        }
    }

    pub(crate) fn parse_config_value(value: &str) -> anyhow::Result<Self> {
        match value {
            "low" => Ok(Urgency::Low),
            "normal" => Ok(Urgency::Normal),
            "critical" => Ok(Urgency::Critical),
            _ => anyhow::bail!("unknown notification urgency {value:?}"),
        }
    }
}

pub(crate) fn urgency_key(urgency: Option<Urgency>) -> &'static str {
    urgency.map(Urgency::as_str).unwrap_or("*")
}

pub(crate) fn urgency_from_key(value: &str) -> Option<Urgency> {
    if value == "*" {
        None
    } else {
        Some(Urgency::from_persisted_str(value))
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

    pub(crate) fn from_persisted_str(value: &str) -> Self {
        match value {
            "silenced" => DeliveryOutcome::Silenced,
            "dropped" => DeliveryOutcome::Dropped,
            _ => DeliveryOutcome::Shown,
        }
    }

    pub(crate) fn parse_config_value(value: &str) -> anyhow::Result<Self> {
        match value {
            "allow" | "shown" => Ok(DeliveryOutcome::Shown),
            "silence" | "silenced" => Ok(DeliveryOutcome::Silenced),
            "drop" | "dropped" => Ok(DeliveryOutcome::Dropped),
            _ => anyhow::bail!("unknown delivery outcome {value:?}"),
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

    pub(crate) fn from_persisted_str(value: &str) -> Self {
        match value {
            "dismissed" => NotificationStatus::Dismissed,
            "expired" => NotificationStatus::Expired,
            "closed" => NotificationStatus::Closed,
            "stale" => NotificationStatus::Stale,
            _ => NotificationStatus::Active,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_strict_configuration_values() {
        assert_eq!(Urgency::parse_config_value("low").unwrap(), Urgency::Low);
        assert_eq!(
            Urgency::parse_config_value("normal").unwrap(),
            Urgency::Normal
        );
        assert_eq!(
            Urgency::parse_config_value("critical").unwrap(),
            Urgency::Critical
        );

        assert_eq!(
            DeliveryOutcome::parse_config_value("allow").unwrap(),
            DeliveryOutcome::Shown
        );
        assert_eq!(
            DeliveryOutcome::parse_config_value("shown").unwrap(),
            DeliveryOutcome::Shown
        );
        assert_eq!(
            DeliveryOutcome::parse_config_value("silence").unwrap(),
            DeliveryOutcome::Silenced
        );
        assert_eq!(
            DeliveryOutcome::parse_config_value("silenced").unwrap(),
            DeliveryOutcome::Silenced
        );
        assert_eq!(
            DeliveryOutcome::parse_config_value("drop").unwrap(),
            DeliveryOutcome::Dropped
        );
        assert_eq!(
            DeliveryOutcome::parse_config_value("dropped").unwrap(),
            DeliveryOutcome::Dropped
        );

        assert!(Urgency::parse_config_value("urgent").is_err());
        assert!(DeliveryOutcome::parse_config_value("mute").is_err());
    }

    #[test]
    fn notification_sound_names_and_files_keep_distinct_reference_shapes() {
        assert_eq!(
            NotificationSound::name(" message-new-instant "),
            Some(NotificationSound::Name("message-new-instant".to_string()))
        );
        assert_eq!(
            NotificationSound::file(" file:///tmp/message%20new.oga "),
            Some(NotificationSound::File("/tmp/message new.oga".to_string()))
        );

        assert_eq!(NotificationSound::name("/tmp/message.oga"), None);
        assert_eq!(NotificationSound::name("http://example.test/sound"), None);
        assert_eq!(
            NotificationSound::file("http://example.test/sound.oga"),
            None
        );
        assert_eq!(
            NotificationSound::file("file://example.test/sound.oga"),
            None
        );
    }

    #[test]
    fn persisted_values_keep_legacy_defaulting_behavior() {
        assert_eq!(Urgency::from_persisted_str("low"), Urgency::Low);
        assert_eq!(Urgency::from_persisted_str("normal"), Urgency::Normal);
        assert_eq!(Urgency::from_persisted_str("critical"), Urgency::Critical);
        assert_eq!(Urgency::from_persisted_str("unknown"), Urgency::Normal);

        assert_eq!(
            DeliveryOutcome::from_persisted_str("shown"),
            DeliveryOutcome::Shown
        );
        assert_eq!(
            DeliveryOutcome::from_persisted_str("silenced"),
            DeliveryOutcome::Silenced
        );
        assert_eq!(
            DeliveryOutcome::from_persisted_str("dropped"),
            DeliveryOutcome::Dropped
        );
        assert_eq!(
            DeliveryOutcome::from_persisted_str("unknown"),
            DeliveryOutcome::Shown
        );

        assert_eq!(
            NotificationStatus::from_persisted_str("active"),
            NotificationStatus::Active
        );
        assert_eq!(
            NotificationStatus::from_persisted_str("dismissed"),
            NotificationStatus::Dismissed
        );
        assert_eq!(
            NotificationStatus::from_persisted_str("expired"),
            NotificationStatus::Expired
        );
        assert_eq!(
            NotificationStatus::from_persisted_str("closed"),
            NotificationStatus::Closed
        );
        assert_eq!(
            NotificationStatus::from_persisted_str("stale"),
            NotificationStatus::Stale
        );
        assert_eq!(
            NotificationStatus::from_persisted_str("unknown"),
            NotificationStatus::Active
        );
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
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SenderEvent {
    ActivationToken {
        notification_id: i64,
        activation_token: String,
    },
    ActionInvoked {
        notification_id: i64,
        action_key: String,
    },
    NotificationClosed {
        notification_id: i64,
        reason: NotificationStatus,
    },
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
