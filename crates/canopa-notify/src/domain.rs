use serde::{Deserialize, Serialize};
use std::time::Duration;

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
    pub summary: String,
    pub body: String,
    pub expire_timeout: NotificationTimeout,
    pub urgency: Urgency,
    pub category: Option<String>,
    pub progress: Option<i64>,
    pub image_data: Option<Vec<u8>>,
    pub actions: Vec<NotificationAction>,
    pub resident: bool,
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

    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "low" => Urgency::Low,
            "critical" => Urgency::Critical,
            _ => Urgency::Normal,
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
        Some(Urgency::from_str(value))
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

    pub(crate) fn from_str(value: &str) -> Self {
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

    pub(crate) fn from_str(value: &str) -> Self {
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
