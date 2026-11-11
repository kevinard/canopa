//! The preferences control API for Settings' Notifications page
//! (`docs/notify/settings-control-api.md`): Do Not Disturb, its schedule,
//! break-through and suppression, and per-application rules. No notification
//! content crosses this interface.
//!
//! Each [`request`] opens the control socket, sends one `control <json>` line
//! and reads one reply line. Unknown reply fields are ignored, so an older
//! client keeps working with a newer Notify.
use crate::{DoNotDisturbReason, Suppression};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// Replies larger than this are refused.
pub const MAX_REPLY_BYTES: usize = 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Effective Do Not Disturb state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoNotDisturbStatus {
    pub active: bool,
    #[serde(default)]
    pub reason: Option<DoNotDisturbReason>,
    /// Unix milliseconds when the current Do Not Disturb ends by itself.
    #[serde(default)]
    pub until: Option<i64>,
    /// Unix milliseconds of the next change, such as a schedule starting.
    #[serde(default)]
    pub next_change: Option<i64>,
}

/// A daily window, `HH:MM` local time. `end <= start` crosses midnight.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub start: String,
    pub end: String,
    /// `mon`…`sun`; `None` is every day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<Vec<String>>,
}

/// The weekday names Notify accepts, Monday first.
pub const WEEKDAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakThrough {
    #[serde(default)]
    pub alarms: bool,
    #[serde(default)]
    pub calls: bool,
}

fn enabled() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoNotDisturbPreferences {
    #[serde(default)]
    pub schedule: Option<Schedule>,
    #[serde(default)]
    pub break_through: BreakThrough,
    #[serde(default = "enabled")]
    pub suppress_fullscreen: bool,
    #[serde(default = "enabled")]
    pub suppress_screen_sharing: bool,
}

impl Default for DoNotDisturbPreferences {
    fn default() -> Self {
        Self {
            schedule: None,
            break_through: BreakThrough::default(),
            suppress_fullscreen: true,
            suppress_screen_sharing: true,
        }
    }
}

/// Where the effective Do Not Disturb preferences come from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceSource {
    #[default]
    Default,
    /// `config.kdl`.
    Configuration,
    /// Set through this API; overrides configuration.
    User,
}

/// The compositor inputs for automatic suppression.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuppressionInputs {
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub screen_sharing: bool,
}

/// An application-wide delivery rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// Banners.
    Allow,
    /// No banners; the Notification Center only.
    Silence,
    /// Mute: discard.
    Drop,
}

/// What the lock screen shows of an application's critical notifications.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LockScreen {
    Hidden,
    #[default]
    NameOnly,
    Content,
}

/// One layer of an application's preferences; `None` falls back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPreferences {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_screen: Option<LockScreen>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppEntry {
    /// The desktop-entry ID (without `.desktop`), else the application name.
    pub app_identity: String,
    /// The rule set through this API, if any.
    #[serde(default)]
    pub delivery: Option<Delivery>,
    #[serde(default)]
    pub configured: AppPreferences,
    /// The values set through this API.
    #[serde(default)]
    pub user: AppPreferences,
    /// Effective values.
    #[serde(default = "enabled")]
    pub sound: bool,
    #[serde(default)]
    pub lock_screen: LockScreen,
}

/// Reply to [`Command::NotificationPreferences`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationPreferences {
    pub do_not_disturb: DoNotDisturbStatus,
    #[serde(default)]
    pub do_not_disturb_preferences: DoNotDisturbPreferences,
    #[serde(default)]
    pub do_not_disturb_source: PreferenceSource,
    #[serde(default)]
    pub suppression_inputs: SuppressionInputs,
    #[serde(default)]
    pub suppression: Option<Suppression>,
    #[serde(default)]
    pub apps: Vec<AppEntry>,
}

impl NotificationPreferences {
    pub fn app(&self, identity: &str) -> Option<&AppEntry> {
        self.apps.iter().find(|app| app.app_identity == identity)
    }
}

/// A Do Not Disturb change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum DoNotDisturbRequest {
    /// Off; inside a scheduled window, skip the rest of it.
    Off,
    /// On until turned off.
    On,
    Toggle,
    /// 1–10,080 minutes.
    For {
        minutes: u32,
    },
    /// Unix milliseconds, within 31 days.
    Until {
        until: i64,
    },
    /// Until 08:00 local time.
    UntilTomorrow,
}

/// A preferences request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum Command {
    NotificationPreferences,
    RequestDoNotDisturb {
        request: DoNotDisturbRequest,
    },
    /// Replace the schedule, break-through and suppression as a whole;
    /// `None` returns to configuration.
    SetDoNotDisturbPreferences {
        preferences: Option<DoNotDisturbPreferences>,
    },
    /// Replace one application's values set through this API. `None`
    /// delivery removes its rule; empty preferences remove its overrides.
    SetAppPreferences {
        app_identity: String,
        delivery: Option<Delivery>,
        preferences: AppPreferences,
    },
}

/// A reply to a [`Command`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Preferences(Box<NotificationPreferences>),
    DoNotDisturb(DoNotDisturbStatus),
    Applied,
}

/// Why a request failed.
#[derive(Debug)]
pub enum RequestError {
    /// Nothing listens on the socket: Notify is not running.
    Unavailable,
    /// Notify refused the request (an `error …` line).
    Refused(String),
    /// The reply was not understood.
    Invalid,
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("Canopa Notify is not running"),
            Self::Refused(reason) => write!(formatter, "Canopa Notify refused: {reason}"),
            Self::Invalid => formatter.write_str("Canopa Notify sent an unexpected reply"),
        }
    }
}

impl std::error::Error for RequestError {}

/// Send one command and read its reply.
pub async fn request(socket: &Path, command: &Command) -> Result<Reply, RequestError> {
    let line = format!(
        "control {}\n",
        serde_json::to_string(command).map_err(|_| RequestError::Invalid)?
    );
    let mut stream = tokio::time::timeout(
        crate::CONNECT_TIMEOUT,
        tokio::net::UnixStream::connect(socket),
    )
    .await
    .map_err(|_| RequestError::Unavailable)?
    .map_err(|_| RequestError::Unavailable)?;
    let reply = tokio::time::timeout(REQUEST_TIMEOUT, async {
        stream
            .write_all(line.as_bytes())
            .await
            .map_err(|_| RequestError::Unavailable)?;
        let mut reader = BufReader::new(stream).take(MAX_REPLY_BYTES as u64 + 1);
        let mut bytes = Vec::new();
        reader
            .read_until(b'\n', &mut bytes)
            .await
            .map_err(|_| RequestError::Unavailable)?;
        Ok::<_, RequestError>(bytes)
    })
    .await
    .map_err(|_| RequestError::Unavailable)??;
    parse_reply(&reply)
}

fn parse_reply(bytes: &[u8]) -> Result<Reply, RequestError> {
    if bytes.is_empty() {
        return Err(RequestError::Unavailable);
    }
    if bytes.len() > MAX_REPLY_BYTES || bytes.last() != Some(&b'\n') {
        return Err(RequestError::Invalid);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| RequestError::Invalid)?;
    if let Some(reason) = text.strip_prefix("error ") {
        let reason: String = reason
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(200)
            .collect();
        return Err(RequestError::Refused(reason));
    }
    #[derive(Deserialize)]
    struct Envelope {
        reply: String,
        #[serde(default)]
        value: serde_json::Value,
    }
    let envelope: Envelope = serde_json::from_str(text).map_err(|_| RequestError::Invalid)?;
    match envelope.reply.as_str() {
        "notification-preferences" => serde_json::from_value(envelope.value)
            .map(|value| Reply::Preferences(Box::new(value)))
            .map_err(|_| RequestError::Invalid),
        "do-not-disturb-status" => serde_json::from_value(envelope.value)
            .map(Reply::DoNotDisturb)
            .map_err(|_| RequestError::Invalid),
        "command-applied" => Ok(Reply::Applied),
        _ => Err(RequestError::Invalid),
    }
}

/// Read every preference.
pub async fn read(socket: &Path) -> Result<NotificationPreferences, RequestError> {
    match request(socket, &Command::NotificationPreferences).await? {
        Reply::Preferences(preferences) => Ok(*preferences),
        _ => Err(RequestError::Invalid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_and_replies_follow_the_documented_shapes() {
        let line = serde_json::to_string(&Command::RequestDoNotDisturb {
            request: DoNotDisturbRequest::For { minutes: 60 },
        })
        .unwrap();
        assert_eq!(
            line,
            r#"{"command":"request-do-not-disturb","request":{"mode":"for","minutes":60}}"#
        );
        let line = serde_json::to_string(&Command::SetAppPreferences {
            app_identity: "org.gnome.Fractal".into(),
            delivery: Some(Delivery::Silence),
            preferences: AppPreferences {
                sound: Some(false),
                lock_screen: Some(LockScreen::Content),
            },
        })
        .unwrap();
        assert_eq!(
            line,
            r#"{"command":"set-app-preferences","app_identity":"org.gnome.Fractal","delivery":"silence","preferences":{"sound":false,"lock_screen":"content"}}"#
        );
        assert_eq!(
            serde_json::to_string(&Command::SetDoNotDisturbPreferences { preferences: None })
                .unwrap(),
            r#"{"command":"set-do-not-disturb-preferences","preferences":null}"#
        );
        let documented = br#"{"reply": "notification-preferences", "value": {
  "do_not_disturb": {"active": true, "reason": "schedule", "until": 1762930800000, "next_change": 1762930800000},
  "do_not_disturb_preferences": {
    "schedule": {"start": "22:00", "end": "07:00", "days": ["mon", "tue", "wed", "thu", "fri"]},
    "break_through": {"alarms": true, "calls": false},
    "suppress_fullscreen": true,
    "suppress_screen_sharing": true
  },
  "do_not_disturb_source": "user",
  "suppression_inputs": {"fullscreen": false, "screen_sharing": false},
  "suppression": null,
  "future_field": 1,
  "apps": [
    {"app_identity": "org.gnome.Fractal", "delivery": "silence",
     "configured": {}, "user": {"sound": false, "lock_screen": "content"},
     "sound": false, "lock_screen": "content"}
  ]
}}
"#
        .iter()
        .copied()
        .filter(|byte| *byte != b'\n')
        .chain([b'\n'])
        .collect::<Vec<_>>();
        let Reply::Preferences(preferences) = parse_reply(&documented).unwrap() else {
            panic!("preferences expected");
        };
        assert_eq!(
            preferences.do_not_disturb.reason,
            Some(DoNotDisturbReason::Schedule)
        );
        assert_eq!(
            preferences
                .do_not_disturb_preferences
                .schedule
                .as_ref()
                .unwrap()
                .days
                .as_ref()
                .unwrap()
                .len(),
            5
        );
        assert_eq!(preferences.do_not_disturb_source, PreferenceSource::User);
        let app = preferences.app("org.gnome.Fractal").unwrap();
        assert_eq!(app.delivery, Some(Delivery::Silence));
        assert_eq!(app.user.lock_screen, Some(LockScreen::Content));
        assert!(matches!(
            parse_reply(b"error schedule needs a day\n"),
            Err(RequestError::Refused(reason)) if reason == "schedule needs a day"
        ));
        assert!(matches!(parse_reply(b""), Err(RequestError::Unavailable)));
        assert!(matches!(parse_reply(b"{}"), Err(RequestError::Invalid)));
        assert_eq!(
            parse_reply(b"{\"reply\":\"command-applied\"}\n").unwrap(),
            Reply::Applied
        );
    }

    #[tokio::test]
    async fn a_missing_daemon_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            read(&dir.path().join("control.sock")).await,
            Err(RequestError::Unavailable)
        ));
    }
}
