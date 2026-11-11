//! Do Not Disturb timers and schedules, break-through categories, automatic
//! suppression and per-application preferences (ADR 0055).
//!
//! Evaluation is pure over an explicit instant and time zone, so schedule edges
//! (midnight, DST gaps and repeated hours) are tested without a clock.
use chrono::{DateTime, Datelike, Days, MappedLocalTime, NaiveDate, NaiveTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use canopa_notify_client::{DoNotDisturbReason, Suppression};

/// Distinct applications with stored preferences, per layer.
pub const MAX_APP_PREFERENCES: usize = 1024;
/// "Until tomorrow" ends at this local hour.
pub const TOMORROW_MORNING_HOUR: u32 = 8;
/// Before this local hour, "tomorrow morning" is the current morning.
const NIGHT_ENDS_HOUR: u32 = 4;
const MAX_TIMER_MINUTES: u32 = 7 * 24 * 60;
const MAX_UNTIL_MILLIS: i64 = 31 * 24 * 60 * 60 * 1000;

/// A local wall-clock minute of the day, written `"HH:MM"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClockTime(u16);

impl ClockTime {
    pub fn new(hour: u32, minute: u32) -> Option<Self> {
        (hour < 24 && minute < 60).then(|| Self((hour * 60 + minute) as u16))
    }

    pub fn parse(value: &str) -> Option<Self> {
        let (hour, minute) = value.split_once(':')?;
        if hour.is_empty() || hour.len() > 2 || minute.len() != 2 {
            return None;
        }
        Self::new(hour.parse().ok()?, minute.parse().ok()?)
    }

    pub fn hour(self) -> u32 {
        u32::from(self.0 / 60)
    }

    pub fn minute(self) -> u32 {
        u32::from(self.0 % 60)
    }

    fn naive(self) -> NaiveTime {
        NaiveTime::from_hms_opt(self.hour(), self.minute(), 0).expect("validated clock time")
    }
}

impl std::fmt::Display for ClockTime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:02}:{:02}", self.hour(), self.minute())
    }
}

impl Serialize for ClockTime {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ClockTime {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| serde::de::Error::custom("expected HH:MM"))
    }
}

/// Days on which a schedule window starts. Serialized as `["mon", …]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weekdays(u8);

const WEEKDAY_NAMES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

impl Weekdays {
    pub const ALL: Self = Self(0x7f);

    pub fn contains(self, day: chrono::Weekday) -> bool {
        self.0 & (1 << day.num_days_from_monday()) != 0
    }

    pub fn is_all(&self) -> bool {
        *self == Self::ALL
    }

    pub fn is_empty_set(&self) -> bool {
        self.0 == 0
    }

    pub fn parse<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        let mut bits = 0;
        for name in names {
            let index = WEEKDAY_NAMES.iter().position(|day| *day == name)?;
            bits |= 1 << index;
        }
        (bits != 0).then_some(Self(bits))
    }

    fn names(self) -> impl Iterator<Item = &'static str> {
        WEEKDAY_NAMES
            .into_iter()
            .enumerate()
            .filter(move |(index, _)| self.0 & (1 << index) != 0)
            .map(|(_, name)| name)
    }
}

impl Default for Weekdays {
    fn default() -> Self {
        Self::ALL
    }
}

impl Serialize for Weekdays {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.names())
    }
}

impl<'de> Deserialize<'de> for Weekdays {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let names = Vec::<String>::deserialize(deserializer)?;
        Self::parse(names.iter().map(String::as_str))
            .ok_or_else(|| serde::de::Error::custom("expected non-empty weekday names mon..sun"))
    }
}

/// A daily window in local wall-clock time. `end <= start` crosses midnight;
/// `end == start` is a whole day. `days` names the day each window starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoNotDisturbSchedule {
    pub start: ClockTime,
    pub end: ClockTime,
    #[serde(default, skip_serializing_if = "Weekdays::is_all")]
    pub days: Weekdays,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Window {
    start: i64,
    end: i64,
}

impl DoNotDisturbSchedule {
    /// The window starting on local `date`, as instants. A start or end inside a
    /// DST gap resolves to the first valid time after the gap; a repeated hour
    /// resolves to its first occurrence, so a window ends at most once.
    fn window_on<Tz: TimeZone>(&self, zone: &Tz, date: NaiveDate) -> Option<Window> {
        if !self.days.contains(date.weekday()) {
            return None;
        }
        let start = resolve_local(zone, date, self.start.naive())?;
        let end_date = if self.end > self.start {
            date
        } else {
            date.checked_add_days(Days::new(1))?
        };
        let end = resolve_local(zone, end_date, self.end.naive())?;
        (end > start).then_some(Window { start, end })
    }

    fn window_at<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> Option<Window> {
        let zone = now.timezone();
        let today = now.date_naive();
        let now = now.timestamp_millis();
        [today.pred_opt(), Some(today)]
            .into_iter()
            .flatten()
            .filter_map(|date| self.window_on(&zone, date))
            .find(|window| window.start <= now && now < window.end)
    }

    fn next_window_after<Tz: TimeZone>(
        &self,
        now: &DateTime<Tz>,
        paused_until: Option<i64>,
    ) -> Option<Window> {
        let zone = now.timezone();
        let today = now.date_naive();
        let now = now.timestamp_millis();
        (0..=8)
            .filter_map(|offset| today.checked_add_days(Days::new(offset)))
            .filter_map(|date| self.window_on(&zone, date))
            .find(|window| window.start > now && paused_until.is_none_or(|p| window.end > p))
    }
}

fn resolve_local<Tz: TimeZone>(zone: &Tz, date: NaiveDate, time: NaiveTime) -> Option<i64> {
    let mut local = date.and_time(time);
    // A DST gap is at most a few hours; probe forward minute by minute.
    for _ in 0..=(26 * 60) {
        match zone.from_local_datetime(&local) {
            MappedLocalTime::Single(instant) | MappedLocalTime::Ambiguous(instant, _) => {
                return Some(instant.timestamp_millis());
            }
            MappedLocalTime::None => local += chrono::Duration::minutes(1),
        }
    }
    None
}

/// Optional category rule: these notifications behave like critical ones for
/// Do Not Disturb and fullscreen suppression.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BreakThrough {
    /// `alarm` and `alarm.*`, including the portal `alarm.ringing` category.
    #[serde(default)]
    pub alarms: bool,
    /// `call.incoming` and `call`.
    #[serde(default)]
    pub calls: bool,
}

impl BreakThrough {
    pub fn matches(&self, category: Option<&str>) -> bool {
        let Some(category) = category else {
            return false;
        };
        (self.alarms && (category == "alarm" || category.starts_with("alarm.")))
            || (self.calls && matches!(category, "call" | "call.incoming"))
    }
}

fn enabled() -> bool {
    true
}

/// Policy around Do Not Disturb. The UI-managed copy replaces the configuration
/// copy as a whole; clearing it returns to configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoNotDisturbPreferences {
    #[serde(default)]
    pub schedule: Option<DoNotDisturbSchedule>,
    #[serde(default)]
    pub break_through: BreakThrough,
    /// Save banners quietly while an application is fullscreen on the focused output.
    #[serde(default = "enabled")]
    pub suppress_fullscreen: bool,
    /// Save banners quietly and hide content while the screen is shared or recorded.
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

/// Toggle and timer state, persisted in the daemon store.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DoNotDisturbTimer {
    #[serde(skip)]
    pub(crate) manual: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) until: Option<i64>,
    /// Turning DND off inside a scheduled window skips that window only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) schedule_paused_until: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoNotDisturbStatus {
    pub active: bool,
    #[serde(default)]
    pub reason: Option<DoNotDisturbReason>,
    /// Unix milliseconds when the current activation ends by itself.
    #[serde(default)]
    pub until: Option<i64>,
    /// The next instant when the status can change without a command.
    #[serde(default)]
    pub next_change: Option<i64>,
}

pub(crate) fn evaluate<Tz: TimeZone>(
    timer: &DoNotDisturbTimer,
    schedule: Option<&DoNotDisturbSchedule>,
    now: &DateTime<Tz>,
) -> DoNotDisturbStatus {
    let now_ms = now.timestamp_millis();
    let paused = timer.schedule_paused_until.filter(|p| *p > now_ms);
    let window = schedule
        .and_then(|schedule| schedule.window_at(now))
        .filter(|window| paused.is_none_or(|p| window.end > p));
    let next_window = schedule.and_then(|schedule| schedule.next_window_after(now, paused));
    let timer_until = timer.until.filter(|until| *until > now_ms);
    let (reason, until) = if timer.manual {
        (Some(DoNotDisturbReason::Manual), None)
    } else if let Some(timer_until) = timer_until {
        // A timer that ends inside a scheduled window continues as the schedule.
        let end = window.map_or(timer_until, |window| window.end.max(timer_until));
        (Some(DoNotDisturbReason::Timer), Some(end))
    } else if let Some(window) = window {
        (Some(DoNotDisturbReason::Schedule), Some(window.end))
    } else {
        (None, None)
    };
    let next_change = [
        timer_until,
        window.map(|window| window.end),
        next_window.map(|window| window.start),
        paused,
    ]
    .into_iter()
    .flatten()
    .filter(|instant| *instant > now_ms)
    .min();
    DoNotDisturbStatus {
        active: reason.is_some(),
        reason,
        until,
        next_change,
    }
}

/// A change requested through the control API, the bar or the Center.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DoNotDisturbRequest {
    /// Off now; inside a scheduled window, skip the rest of that window.
    Off,
    /// On until turned off.
    On,
    /// Off if currently active, otherwise on.
    Toggle,
    /// On for 1 to 10,080 minutes.
    For { minutes: u32 },
    /// On until a Unix-millisecond instant within 31 days.
    Until { until: i64 },
    /// On until 08:00 local time on the next morning.
    UntilTomorrow,
}

impl DoNotDisturbRequest {
    pub(crate) fn apply<Tz: TimeZone>(
        self,
        timer: DoNotDisturbTimer,
        schedule: Option<&DoNotDisturbSchedule>,
        now: &DateTime<Tz>,
    ) -> anyhow::Result<DoNotDisturbTimer> {
        let now_ms = now.timestamp_millis();
        let mut next = DoNotDisturbTimer {
            manual: false,
            until: None,
            schedule_paused_until: timer.schedule_paused_until.filter(|p| *p > now_ms),
        };
        match self {
            Self::Toggle => {
                let request = if evaluate(&timer, schedule, now).active {
                    Self::Off
                } else {
                    Self::On
                };
                return request.apply(timer, schedule, now);
            }
            Self::Off => {
                let window = schedule
                    .and_then(|schedule| schedule.window_at(now))
                    .filter(|window| next.schedule_paused_until.is_none_or(|p| window.end > p));
                if let Some(window) = window {
                    next.schedule_paused_until = Some(window.end);
                }
            }
            Self::On => next.manual = true,
            Self::For { minutes } => {
                anyhow::ensure!(
                    (1..=MAX_TIMER_MINUTES).contains(&minutes),
                    "Do Not Disturb duration must be 1 to {MAX_TIMER_MINUTES} minutes"
                );
                next.until = Some(now_ms + i64::from(minutes) * 60_000);
            }
            Self::Until { until } => {
                anyhow::ensure!(
                    until > now_ms && until - now_ms <= MAX_UNTIL_MILLIS,
                    "Do Not Disturb end must be in the next 31 days"
                );
                next.until = Some(until);
            }
            Self::UntilTomorrow => next.until = Some(tomorrow_morning(now)),
        }
        Ok(next)
    }
}

/// 08:00 local on the next day; between midnight and 04:00 the coming morning.
pub(crate) fn tomorrow_morning<Tz: TimeZone>(now: &DateTime<Tz>) -> i64 {
    let zone = now.timezone();
    let today = now.date_naive();
    let date = if now.hour() < NIGHT_ENDS_HOUR {
        today
    } else {
        today.succ_opt().unwrap_or(today)
    };
    let morning = NaiveTime::from_hms_opt(TOMORROW_MORNING_HOUR, 0, 0).expect("valid hour");
    resolve_local(&zone, date, morning).unwrap_or(now.timestamp_millis() + 12 * 3_600_000)
}

/// What the lock screen may show for an application's critical notifications.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LockScreenVisibility {
    /// Nothing, not even critical notifications.
    Hidden,
    /// "App — New notification" for critical notifications (the default).
    #[default]
    NameOnly,
    /// The application name and summary for critical notifications.
    Content,
}

impl LockScreenVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::NameOnly => "name-only",
            Self::Content => "content",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "hidden" | "hide" => Some(Self::Hidden),
            "name-only" | "hide-content" => Some(Self::NameOnly),
            "content" | "show-content" => Some(Self::Content),
            _ => None,
        }
    }

    /// The stricter of two visibilities.
    pub fn min(self, other: Self) -> Self {
        let rank = |value| match value {
            Self::Hidden => 0,
            Self::NameOnly => 1,
            Self::Content => 2,
        };
        if rank(self) <= rank(other) {
            self
        } else {
            other
        }
    }
}

/// Per-application overrides keyed by App Identity (desktop entry first, then
/// application name). Unset fields fall back to the next layer, then defaults.
/// Delivery (banners, Center only, mute) remains a Notification Rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppPreferences {
    /// `false` never plays sounds for this application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_screen: Option<LockScreenVisibility>,
}

impl AppPreferences {
    pub fn is_empty(&self) -> bool {
        self.sound.is_none() && self.lock_screen.is_none()
    }

    fn or(self, fallback: Self) -> Self {
        Self {
            sound: self.sound.or(fallback.sound),
            lock_screen: self.lock_screen.or(fallback.lock_screen),
        }
    }
}

/// The current automatic suppression inputs, from Niri's compositor state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuppressionState {
    /// The focused output's active window covers the whole output.
    pub fullscreen: bool,
    /// A screencast (portal PipeWire or wlr-screencopy) is streaming.
    pub screen_sharing: bool,
}

/// Configuration and UI-managed layers. The UI layer wins (ADR 0011).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PreferenceLayers {
    pub(crate) config_dnd: Option<DoNotDisturbPreferences>,
    pub(crate) user_dnd: Option<DoNotDisturbPreferences>,
    pub(crate) config_apps: BTreeMap<String, AppPreferences>,
    pub(crate) user_apps: BTreeMap<String, AppPreferences>,
}

impl PreferenceLayers {
    pub(crate) fn dnd(&self) -> DoNotDisturbPreferences {
        self.user_dnd.or(self.config_dnd).unwrap_or_default()
    }

    pub(crate) fn app(&self, app_identity: &str) -> AppPreferences {
        let user = self
            .user_apps
            .get(app_identity)
            .copied()
            .unwrap_or_default();
        let config = self
            .config_apps
            .get(app_identity)
            .copied()
            .unwrap_or_default();
        user.or(config)
    }

    pub(crate) fn sound_enabled(&self, app_identity: &str) -> bool {
        self.app(app_identity).sound.unwrap_or(true)
    }

    pub(crate) fn lock_screen(&self, app_identity: &str) -> LockScreenVisibility {
        self.app(app_identity).lock_screen.unwrap_or_default()
    }

    /// Applications named by either layer, in identity order.
    pub(crate) fn app_identities(&self) -> impl Iterator<Item = &String> {
        let mut names: Vec<_> = self
            .config_apps
            .keys()
            .chain(self.user_apps.keys())
            .collect();
        names.sort();
        names.dedup();
        names.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, NaiveDateTime, Utc};

    /// Europe/Paris-like rules for 2026: +02:00 from 29 March 01:00 UTC to
    /// 25 October 01:00 UTC, otherwise +01:00.
    #[derive(Clone, Copy, Debug)]
    struct Paris;

    fn utc(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").unwrap()
    }

    impl TimeZone for Paris {
        type Offset = FixedOffset;
        fn from_offset(_: &FixedOffset) -> Self {
            Paris
        }
        fn offset_from_utc_datetime(&self, instant: &NaiveDateTime) -> FixedOffset {
            let summer = utc("2026-03-29 01:00") <= *instant && *instant < utc("2026-10-25 01:00");
            FixedOffset::east_opt(if summer { 7200 } else { 3600 }).unwrap()
        }
        fn offset_from_utc_date(&self, date: &NaiveDate) -> FixedOffset {
            self.offset_from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        }
        fn offset_from_local_datetime(
            &self,
            local: &NaiveDateTime,
        ) -> MappedLocalTime<FixedOffset> {
            let valid: Vec<_> = [7200, 3600]
                .into_iter()
                .map(|seconds| FixedOffset::east_opt(seconds).unwrap())
                .filter(|offset| {
                    self.offset_from_utc_datetime(
                        &(*local - chrono::Duration::seconds(offset.local_minus_utc().into())),
                    ) == *offset
                })
                .collect();
            match valid.as_slice() {
                [] => MappedLocalTime::None,
                [one] => MappedLocalTime::Single(*one),
                [earliest, latest] => MappedLocalTime::Ambiguous(*earliest, *latest),
                _ => unreachable!(),
            }
        }
        fn offset_from_local_date(&self, date: &NaiveDate) -> MappedLocalTime<FixedOffset> {
            self.offset_from_local_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
        }
    }

    fn paris(local: &str) -> DateTime<Paris> {
        Paris.from_local_datetime(&utc(local)).earliest().unwrap()
    }

    fn local_text(millis: i64) -> String {
        Paris
            .timestamp_millis_opt(millis)
            .unwrap()
            .format("%Y-%m-%d %H:%M %:z")
            .to_string()
    }

    fn night() -> DoNotDisturbSchedule {
        DoNotDisturbSchedule {
            start: ClockTime::parse("22:00").unwrap(),
            end: ClockTime::parse("07:00").unwrap(),
            days: Weekdays::ALL,
        }
    }

    fn status(schedule: &DoNotDisturbSchedule, local: &str) -> DoNotDisturbStatus {
        evaluate(&DoNotDisturbTimer::default(), Some(schedule), &paris(local))
    }

    #[test]
    fn clock_times_and_weekdays_round_trip() {
        assert_eq!(ClockTime::parse("7:05").unwrap().to_string(), "07:05");
        for invalid in ["24:00", "12:60", "12", "1:2", "aa:bb", ""] {
            assert!(ClockTime::parse(invalid).is_none(), "{invalid}");
        }
        let schedule: DoNotDisturbSchedule =
            serde_json::from_str(r#"{"start":"22:30","end":"06:45","days":["fri","mon"]}"#)
                .unwrap();
        assert_eq!(
            serde_json::to_string(&schedule).unwrap(),
            r#"{"start":"22:30","end":"06:45","days":["mon","fri"]}"#
        );
        assert!(
            serde_json::from_str::<DoNotDisturbSchedule>(
                r#"{"start":"1:00","end":"2:00","days":[]}"#
            )
            .is_err()
        );
        assert_eq!(
            serde_json::to_string(&night()).unwrap(),
            r#"{"start":"22:00","end":"07:00"}"#
        );
    }

    #[test]
    fn schedule_crosses_midnight_with_exclusive_end() {
        let schedule = night();
        assert!(!status(&schedule, "2026-01-14 21:59").active);
        let start = status(&schedule, "2026-01-14 22:00");
        assert_eq!(start.reason, Some(DoNotDisturbReason::Schedule));
        assert_eq!(local_text(start.until.unwrap()), "2026-01-15 07:00 +01:00");
        // Midnight belongs to the window that started the previous evening.
        let midnight = status(&schedule, "2026-01-15 00:00");
        assert!(midnight.active);
        assert_eq!(midnight.until, start.until);
        assert!(status(&schedule, "2026-01-15 06:59").active);
        let end = status(&schedule, "2026-01-15 07:00");
        assert!(!end.active);
        assert_eq!(
            local_text(end.next_change.unwrap()),
            "2026-01-15 22:00 +01:00"
        );
    }

    #[test]
    fn same_day_window_and_whole_day_window() {
        let lunch = DoNotDisturbSchedule {
            start: ClockTime::parse("12:00").unwrap(),
            end: ClockTime::parse("13:00").unwrap(),
            days: Weekdays::ALL,
        };
        assert!(!status(&lunch, "2026-01-14 11:59").active);
        assert!(status(&lunch, "2026-01-14 12:30").active);
        assert!(!status(&lunch, "2026-01-14 13:00").active);
        let all_day = DoNotDisturbSchedule {
            start: ClockTime::parse("00:00").unwrap(),
            end: ClockTime::parse("00:00").unwrap(),
            days: Weekdays::parse(["sat", "sun"]).unwrap(),
        };
        // 2026-01-17 is a Saturday.
        assert!(!status(&all_day, "2026-01-16 23:59").active);
        assert!(status(&all_day, "2026-01-17 00:00").active);
        assert!(status(&all_day, "2026-01-18 23:59").active);
        assert!(!status(&all_day, "2026-01-19 00:00").active);
    }

    #[test]
    fn weekdays_name_the_starting_evening() {
        let weeknights = DoNotDisturbSchedule {
            days: Weekdays::parse(["mon", "tue", "wed", "thu", "fri"]).unwrap(),
            ..night()
        };
        // Friday 23:00 started on Friday; Saturday 23:00 did not start.
        assert!(status(&weeknights, "2026-01-16 23:00").active);
        assert!(status(&weeknights, "2026-01-17 06:00").active);
        assert!(!status(&weeknights, "2026-01-17 23:00").active);
        let sunday = status(&weeknights, "2026-01-18 12:00");
        assert_eq!(
            local_text(sunday.next_change.unwrap()),
            "2026-01-19 22:00 +01:00"
        );
    }

    #[test]
    fn spring_forward_gap_starts_at_the_first_valid_time() {
        // 2026-03-29 02:00 jumps to 03:00 in Paris.
        let schedule = DoNotDisturbSchedule {
            start: ClockTime::parse("02:30").unwrap(),
            end: ClockTime::parse("06:00").unwrap(),
            days: Weekdays::ALL,
        };
        let before = status(&schedule, "2026-03-29 01:59");
        assert!(!before.active);
        assert_eq!(
            local_text(before.next_change.unwrap()),
            "2026-03-29 03:00 +02:00"
        );
        let after = status(&schedule, "2026-03-29 03:00");
        assert!(after.active);
        assert_eq!(local_text(after.until.unwrap()), "2026-03-29 06:00 +02:00");
        // The night window through the gap keeps a wall-clock 07:00 end.
        let night = status(&night(), "2026-03-29 01:30");
        assert_eq!(local_text(night.until.unwrap()), "2026-03-29 07:00 +02:00");
        assert_eq!(
            night.until.unwrap() - paris("2026-03-28 22:00").timestamp_millis(),
            8 * 3_600_000
        );
    }

    #[test]
    fn fall_back_repeated_hour_ends_once() {
        // 2026-10-25 03:00 +02:00 falls back to 02:00 +01:00; 02:30 occurs twice.
        let schedule = DoNotDisturbSchedule {
            start: ClockTime::parse("22:00").unwrap(),
            end: ClockTime::parse("02:30").unwrap(),
            days: Weekdays::ALL,
        };
        let first = Paris
            .from_local_datetime(&utc("2026-10-25 02:15"))
            .earliest()
            .unwrap();
        let during = evaluate(&DoNotDisturbTimer::default(), Some(&schedule), &first);
        assert!(during.active);
        assert_eq!(local_text(during.until.unwrap()), "2026-10-25 02:30 +02:00");
        // The repeated 02:15 (+01:00) is after the first 02:30: the window has ended.
        let repeated = Paris
            .from_local_datetime(&utc("2026-10-25 02:15"))
            .latest()
            .unwrap();
        assert!(!evaluate(&DoNotDisturbTimer::default(), Some(&schedule), &repeated).active);
        // The night window is one hour longer across the fall-back.
        let night = status(&night(), "2026-10-24 23:00");
        assert_eq!(
            night.until.unwrap() - paris("2026-10-24 22:00").timestamp_millis(),
            10 * 3_600_000
        );
    }

    #[test]
    fn manual_timer_and_schedule_precedence() {
        let schedule = night();
        let now = paris("2026-01-14 20:00");
        let manual = DoNotDisturbRequest::On
            .apply(Default::default(), Some(&schedule), &now)
            .unwrap();
        let on = evaluate(&manual, Some(&schedule), &now);
        assert_eq!(
            (on.reason, on.until),
            (Some(DoNotDisturbReason::Manual), None)
        );

        let hour = DoNotDisturbRequest::For { minutes: 60 }
            .apply(manual, Some(&schedule), &now)
            .unwrap();
        let timed = evaluate(&hour, Some(&schedule), &now);
        assert_eq!(timed.reason, Some(DoNotDisturbReason::Timer));
        assert_eq!(local_text(timed.until.unwrap()), "2026-01-14 21:00 +01:00");
        assert_eq!(timed.next_change, timed.until);
        assert!(!evaluate(&hour, Some(&schedule), &paris("2026-01-14 21:00")).active);

        // A timer that runs into the schedule reports the schedule's end.
        let long = DoNotDisturbRequest::For { minutes: 150 }
            .apply(hour, Some(&schedule), &now)
            .unwrap();
        let joined = evaluate(&long, Some(&schedule), &paris("2026-01-14 22:15"));
        assert_eq!(joined.reason, Some(DoNotDisturbReason::Timer));
        assert_eq!(local_text(joined.until.unwrap()), "2026-01-15 07:00 +01:00");

        for invalid in [
            DoNotDisturbRequest::For { minutes: 0 },
            DoNotDisturbRequest::For {
                minutes: MAX_TIMER_MINUTES + 1,
            },
            DoNotDisturbRequest::Until {
                until: now.timestamp_millis(),
            },
            DoNotDisturbRequest::Until {
                until: now.timestamp_millis() + MAX_UNTIL_MILLIS + 1,
            },
        ] {
            assert!(invalid.apply(hour, None, &now).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn off_inside_a_window_skips_only_that_window() {
        let schedule = night();
        let inside = paris("2026-01-14 23:00");
        let off = DoNotDisturbRequest::Off
            .apply(Default::default(), Some(&schedule), &inside)
            .unwrap();
        let skipped = evaluate(&off, Some(&schedule), &inside);
        assert!(!skipped.active);
        assert_eq!(
            local_text(skipped.next_change.unwrap()),
            "2026-01-15 07:00 +01:00"
        );
        assert!(!evaluate(&off, Some(&schedule), &paris("2026-01-15 06:00")).active);
        assert!(evaluate(&off, Some(&schedule), &paris("2026-01-15 22:00")).active);
        // Toggle inside the skipped window turns it back on manually.
        let toggled = DoNotDisturbRequest::Toggle
            .apply(off, Some(&schedule), &inside)
            .unwrap();
        assert_eq!(
            evaluate(&toggled, Some(&schedule), &inside).reason,
            Some(DoNotDisturbReason::Manual)
        );
        // Toggle inside an active window turns it off for that window.
        let toggled_off = DoNotDisturbRequest::Toggle
            .apply(Default::default(), Some(&schedule), &inside)
            .unwrap();
        assert!(!evaluate(&toggled_off, Some(&schedule), &inside).active);
    }

    #[test]
    fn until_tomorrow_ends_next_morning() {
        let evening = DoNotDisturbRequest::UntilTomorrow
            .apply(Default::default(), None, &paris("2026-01-14 21:00"))
            .unwrap();
        assert_eq!(
            local_text(evening.until.unwrap()),
            "2026-01-15 08:00 +01:00"
        );
        let late_night = DoNotDisturbRequest::UntilTomorrow
            .apply(Default::default(), None, &paris("2026-01-15 01:30"))
            .unwrap();
        assert_eq!(
            local_text(late_night.until.unwrap()),
            "2026-01-15 08:00 +01:00"
        );
        // Across the spring-forward night, 08:00 is still the wall-clock morning.
        let spring = DoNotDisturbRequest::UntilTomorrow
            .apply(Default::default(), None, &paris("2026-03-28 23:00"))
            .unwrap();
        assert_eq!(local_text(spring.until.unwrap()), "2026-03-29 08:00 +02:00");
        let utc_now = Utc::now();
        assert!(tomorrow_morning(&utc_now) > utc_now.timestamp_millis());
    }

    #[test]
    fn break_through_matches_alarm_and_call_categories_only_when_enabled() {
        let none = BreakThrough::default();
        assert!(!none.matches(Some("alarm.ringing")));
        let both = BreakThrough {
            alarms: true,
            calls: true,
        };
        for category in ["alarm", "alarm.ringing", "call.incoming", "call"] {
            assert!(both.matches(Some(category)), "{category}");
        }
        for category in [
            None,
            Some("call.missed"),
            Some("call.ongoing"),
            Some("alarms"),
            Some("im.received"),
        ] {
            assert!(!both.matches(category), "{category:?}");
        }
        let calls = BreakThrough {
            alarms: false,
            calls: true,
        };
        assert!(!calls.matches(Some("alarm.ringing")));
    }

    #[test]
    fn preference_layers_prefer_ui_values_field_by_field() {
        let mut layers = PreferenceLayers::default();
        assert!(layers.sound_enabled("Mail"));
        assert_eq!(layers.lock_screen("Mail"), LockScreenVisibility::NameOnly);
        layers.config_apps.insert(
            "Mail".into(),
            AppPreferences {
                sound: Some(false),
                lock_screen: Some(LockScreenVisibility::Content),
            },
        );
        layers.user_apps.insert(
            "Mail".into(),
            AppPreferences {
                sound: None,
                lock_screen: Some(LockScreenVisibility::Hidden),
            },
        );
        assert!(!layers.sound_enabled("Mail"));
        assert_eq!(layers.lock_screen("Mail"), LockScreenVisibility::Hidden);
        layers.config_dnd = Some(DoNotDisturbPreferences {
            suppress_fullscreen: false,
            ..Default::default()
        });
        assert!(!layers.dnd().suppress_fullscreen);
        layers.user_dnd = Some(DoNotDisturbPreferences::default());
        assert!(layers.dnd().suppress_fullscreen);
        assert_eq!(
            LockScreenVisibility::Content.min(LockScreenVisibility::NameOnly),
            LockScreenVisibility::NameOnly
        );
    }
}
