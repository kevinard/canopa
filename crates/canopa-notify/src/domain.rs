use serde::{Deserialize, Serialize};
use std::time::Duration;

const MAX_NOTIFICATION_IMAGE_DIMENSION: u32 = 4096;
const MAX_NOTIFICATION_IMAGE_BYTES: usize = 16 * 1024 * 1024;

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
    pub body_markup: NotificationBodyMarkup,
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
pub struct NotificationBodyMarkup {
    pub runs: Vec<NotificationBodyRun>,
}

impl NotificationBodyMarkup {
    pub fn plain(text: impl Into<String>) -> Self {
        let text = text.into();
        if text.is_empty() {
            Self::default()
        } else {
            Self {
                runs: vec![NotificationBodyRun::plain(text)],
            }
        }
    }

    pub fn plain_text(&self) -> String {
        self.runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>()
    }

    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|run| run.text.is_empty())
    }

    pub(crate) fn from_runs(runs: Vec<NotificationBodyRun>) -> Self {
        let mut normalized: Vec<NotificationBodyRun> = Vec::new();
        for run in runs {
            if run.text.is_empty() {
                continue;
            }

            match normalized.last_mut() {
                Some(previous) if previous.style == run.style => previous.text.push_str(&run.text),
                _ => normalized.push(run),
            }
        }

        Self { runs: normalized }
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

        if let Some((scheme, rest)) = value.split_once(':') {
            if is_uri_scheme(scheme) {
                return (scheme.eq_ignore_ascii_case("file"))
                    .then(|| local_path_from_file_uri(rest))
                    .flatten();
            }
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
    let path = rest
        .strip_prefix("//localhost")
        .or_else(|| rest.strip_prefix("//"))
        .unwrap_or(rest);
    NotificationVisualReference::local_path(percent_decode_path(path))
}

fn percent_decode_path(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(decoded) = u8::from_str_radix(hex, 16) {
                    output.push(decoded);
                    index += 3;
                    continue;
                }
            }
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
        (!value.is_empty()).then(|| Self::Name(value.to_string()))
    }

    pub fn file(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        let value = value.trim();
        (!value.is_empty()).then(|| Self::File(value.to_string()))
    }
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
