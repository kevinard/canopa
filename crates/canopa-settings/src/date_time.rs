//! The Date & time page's service: systemd-timedated (time zone, network
//! time, the system clock) and, when GeoClue is installed, the time zone of
//! this computer's location. timedated asks polkit for each change.
use crate::live::{LiveAction, LiveService, ServiceError};
use canopa_services::{
    ServiceState,
    geoclue::{self, Place},
    timedate::{self, Clock},
};
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{path::PathBuf, sync::Arc};

const NOT_RUNNING: &str = "systemd-timedated did not answer.";
const NO_BUS: &str = "The system bus is not reachable.";
const ZONE_TABLE: &str = "/usr/share/zoneinfo/zone1970.tab";
const DESKTOP_ID: &str = "org.canopa.Settings";

/// What the page shows.
#[derive(Clone, Debug, PartialEq)]
pub struct DateTimeState {
    pub clock: Clock,
    /// Time zones timedated accepts, sorted.
    pub zones: Arc<Vec<String>>,
    /// GeoClue is installed, so the zone can follow the location.
    pub locate: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DateTimeAction {
    Timezone(String),
    /// Set the time zone of this computer's location.
    Locate,
    Ntp(bool),
    /// Microseconds since the Unix epoch.
    Time(i64),
}

impl LiveAction for DateTimeAction {
    fn control(&self) -> String {
        match self {
            Self::Timezone(_) | Self::Locate => "timezone",
            Self::Ntp(_) => "ntp",
            Self::Time(_) => "time",
        }
        .into()
    }
}

fn failure(error: canopa_services::Error) -> ServiceError {
    use canopa_services::Error;
    match error {
        Error::Unavailable => ServiceError::Unavailable(NOT_RUNNING.into()),
        Error::NotAuthorized => {
            ServiceError::Failed("The system did not allow this change.".into())
        }
        Error::TimedOut => ServiceError::Failed("The system did not answer in time.".into()),
        Error::Changed => ServiceError::Failed("The setting changed. Try again.".into()),
        Error::Invalid(message) => ServiceError::Failed(format!("Not accepted: {message}.")),
        Error::Refused(message) => ServiceError::Failed(message),
    }
}

/// A zone's principal location from `zone1970.tab`, in degrees.
fn coordinates(text: &str) -> Option<(f64, f64)> {
    // ±DDMM±DDDMM or ±DDMMSS±DDDMMSS.
    let split = text[1..].find(['+', '-'])? + 1;
    let (latitude, longitude) = text.split_at(split);
    let degrees = |part: &str, width: usize| -> Option<f64> {
        let sign = match part.as_bytes().first()? {
            b'+' => 1.0,
            b'-' => -1.0,
            _ => return None,
        };
        let digits = &part[1..];
        if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let (whole, rest) = digits.split_at_checked(width)?;
        let (minutes, seconds) = match rest.len() {
            2 => (rest, "0"),
            4 => rest.split_at(2),
            _ => return None,
        };
        let value = whole.parse::<f64>().ok()?
            + minutes.parse::<f64>().ok()? / 60.0
            + seconds.parse::<f64>().ok()? / 3600.0;
        Some(sign * value)
    };
    Some((degrees(latitude, 2)?, degrees(longitude, 3)?))
}

/// The accepted zone whose principal city is nearest to `place`.
pub fn nearest_zone(table: &str, place: Place, zones: &[String]) -> Option<String> {
    let radians = f64::to_radians;
    let distance = |(latitude, longitude): (f64, f64)| {
        // The haversine central angle; the radius does not matter here.
        let (a, b) = (radians(place.latitude), radians(latitude));
        let d_lat = b - a;
        let d_lon = radians(longitude - place.longitude);
        let h = (d_lat / 2.0).sin().powi(2) + a.cos() * b.cos() * (d_lon / 2.0).sin().powi(2);
        2.0 * h.sqrt().min(1.0).asin()
    };
    table
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let _countries = fields.next()?;
            let place = coordinates(fields.next()?)?;
            let zone = fields.next()?;
            zones
                .iter()
                .any(|known| known == zone)
                .then(|| (distance(place), zone.to_owned()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, zone)| zone)
}

/// timedated and GeoClue for the page.
#[derive(Clone)]
pub struct DateTimeService {
    system: Arc<tokio::sync::OnceCell<zbus::Connection>>,
    zones: Arc<tokio::sync::OnceCell<Arc<Vec<String>>>>,
    locate: Arc<tokio::sync::OnceCell<bool>>,
    zone_table: PathBuf,
}

impl Default for DateTimeService {
    fn default() -> Self {
        Self {
            system: Arc::default(),
            zones: Arc::default(),
            locate: Arc::default(),
            zone_table: ZONE_TABLE.into(),
        }
    }
}

impl DateTimeService {
    /// A service on an existing connection, with zone locations from `table` (tests).
    pub fn with(system: zbus::Connection, table: PathBuf) -> Self {
        Self {
            system: Arc::new(tokio::sync::OnceCell::new_with(Some(system))),
            zone_table: table,
            ..Default::default()
        }
    }

    async fn connection(&self) -> Result<zbus::Connection, ServiceError> {
        self.system
            .get_or_try_init(canopa_services::bus::system)
            .await
            .cloned()
            .map_err(|_| ServiceError::Unavailable(NO_BUS.into()))
    }

    async fn state(&self, clock: Clock) -> Result<DateTimeState, ServiceError> {
        let system = self.connection().await?;
        let zones = self
            .zones
            .get_or_try_init(|| async {
                let mut zones = timedate::list_timezones(&system).await.map_err(failure)?;
                zones.sort();
                Ok::<_, ServiceError>(Arc::new(zones))
            })
            .await?
            .clone();
        let locate = *self
            .locate
            .get_or_init(|| geoclue::available(&system))
            .await;
        Ok(DateTimeState {
            clock,
            zones,
            locate,
        })
    }

    async fn read(&self) -> Result<DateTimeState, ServiceError> {
        let system = self.connection().await?;
        let clock = timedate::read(&system).await.map_err(failure)?;
        self.state(clock).await
    }

    async fn run(&self, action: DateTimeAction) -> Result<(), ServiceError> {
        let system = self.connection().await?;
        match action {
            DateTimeAction::Timezone(zone) => timedate::set_timezone(&system, &zone)
                .await
                .map_err(failure),
            DateTimeAction::Ntp(enabled) => {
                timedate::set_ntp(&system, enabled).await.map_err(failure)
            }
            DateTimeAction::Time(usec) => timedate::set_time(&system, usec).await.map_err(failure),
            DateTimeAction::Locate => {
                let place =
                    geoclue::locate(&system, DESKTOP_ID)
                        .await
                        .map_err(|error| match error {
                            canopa_services::Error::NotAuthorized => ServiceError::Failed(
                                "Location access is not allowed for Canopa Settings.".into(),
                            ),
                            canopa_services::Error::TimedOut => ServiceError::Failed(
                                "The location could not be found in time.".into(),
                            ),
                            _ => ServiceError::Failed("The location is not available.".into()),
                        })?;
                let zones = self.read().await?.zones;
                let table = std::fs::read_to_string(&self.zone_table).map_err(|_| {
                    ServiceError::Failed("The time zone table (tzdata) is not installed.".into())
                })?;
                let zone = nearest_zone(&table, place, &zones).ok_or_else(|| {
                    ServiceError::Failed("No time zone matches this location.".into())
                })?;
                timedate::set_timezone(&system, &zone)
                    .await
                    .map_err(failure)
            }
        }
    }
}

impl LiveService for DateTimeService {
    type State = DateTimeState;
    type Action = DateTimeAction;

    fn name(&self) -> &str {
        "systemd-timedated"
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<DateTimeState, ServiceError>> {
        let service = self.clone();
        async move { service.read().await }.boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<DateTimeState, ServiceError>> {
        let service = self.clone();
        futures_util::stream::once(async move {
            let Ok(system) = service.connection().await else {
                return futures_util::stream::iter([Err(ServiceError::Unavailable(NO_BUS.into()))])
                    .boxed();
            };
            timedate::watch(system)
                .then(move |state| {
                    let service = service.clone();
                    async move {
                        match state {
                            ServiceState::Available(clock) => service.state(clock).await,
                            ServiceState::Unavailable => {
                                Err(ServiceError::Unavailable(NOT_RUNNING.into()))
                            }
                        }
                    }
                })
                .boxed()
        })
        .flatten()
        .boxed()
    }

    fn apply(
        &self,
        action: DateTimeAction,
    ) -> BoxFuture<'static, Result<Option<DateTimeState>, ServiceError>> {
        let service = self.clone();
        async move {
            service.run(action).await?;
            Ok(service.read().await.ok())
        }
        .boxed()
    }
}

/// Local broken-down time.
fn local(seconds: libc::time_t) -> Option<libc::tm> {
    unsafe extern "C" {
        fn tzset();
    }
    // SAFETY: tzset reloads the process's time zone; localtime_r writes
    // only the zeroed struct it is given.
    unsafe {
        tzset();
        let mut tm: libc::tm = std::mem::zeroed();
        (!libc::localtime_r(&seconds, &mut tm).is_null()).then_some(tm)
    }
}

/// The current local date (`YYYY-MM-DD`) and time (`HH:MM`).
pub fn now_fields() -> (String, String) {
    // SAFETY: time with a null pointer only returns the time.
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    local(now).map_or_else(Default::default, |tm| {
        (
            format!(
                "{:04}-{:02}-{:02}",
                tm.tm_year + 1900,
                tm.tm_mon + 1,
                tm.tm_mday
            ),
            format!("{:02}:{:02}", tm.tm_hour, tm.tm_min),
        )
    })
}

const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// "Wednesday 11 November 2026, 14:32:05" (or "2:32:05 PM"), now.
pub fn now_label(twelve_hour: bool) -> String {
    // SAFETY: time with a null pointer only returns the time.
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    let Some(tm) = local(now) else {
        return String::new();
    };
    let clock = if twelve_hour {
        let hour = match tm.tm_hour % 12 {
            0 => 12,
            hour => hour,
        };
        let half = if tm.tm_hour < 12 { "AM" } else { "PM" };
        format!("{hour}:{:02}:{:02} {half}", tm.tm_min, tm.tm_sec)
    } else {
        format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
    };
    format!(
        "{} {} {} {}, {clock}",
        WEEKDAYS.get(tm.tm_wday as usize).unwrap_or(&""),
        tm.tm_mday,
        MONTHS.get(tm.tm_mon as usize).unwrap_or(&""),
        tm.tm_year + 1900
    )
}

/// Microseconds since the epoch for a local `YYYY-MM-DD` date and `HH:MM`
/// or `HH:MM:SS` time, in this computer's time zone.
pub fn local_usec(date: &str, time: &str) -> Option<i64> {
    let mut date = date.trim().split('-').map(|part| part.parse::<i32>().ok());
    let (year, month, day) = (date.next()??, date.next()??, date.next()??);
    let mut time = time.trim().split(':').map(|part| part.parse::<i32>().ok());
    let (hour, minute) = (time.next()??, time.next()??);
    let second = time.next().flatten().unwrap_or(0);
    if date.next().is_some()
        || !(1970..=2200).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || !(0..24).contains(&hour)
        || !(0..60).contains(&minute)
        || !(0..60).contains(&second)
    {
        return None;
    }
    // SAFETY: mktime reads and normalizes the struct it is given.
    let seconds = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_year = year - 1900;
        tm.tm_mon = month - 1;
        tm.tm_mday = day;
        tm.tm_hour = hour;
        tm.tm_min = minute;
        tm.tm_sec = second;
        tm.tm_isdst = -1;
        let seconds = libc::mktime(&mut tm);
        // mktime normalizes 31 February into March; refuse that.
        (tm.tm_mday == day && tm.tm_mon == month - 1).then_some(seconds)
    }?;
    (seconds > 0).then_some(seconds * 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_services::testing::{TestBus, timedate::FakeTimedate};

    const TABLE: &str = "# comment\nFR,MC\t+4852+00220\tEurope/Paris\nJP\t+353916+1394441\tAsia/Tokyo\nUS\t+404251-0740023\tAmerica/New_York\tEastern\n";

    #[test]
    fn locations_map_to_the_nearest_zone() {
        assert_eq!(
            coordinates("+4852+00220"),
            Some((48.0 + 52.0 / 60.0, 2.0 + 20.0 / 60.0))
        );
        let (lat, lon) = coordinates("+404251-0740023").unwrap();
        assert!((lat - 40.714).abs() < 0.01 && (lon + 74.006).abs() < 0.01);
        assert_eq!(coordinates("4852+00220"), None);
        let zones: Vec<String> = ["Europe/Paris", "Asia/Tokyo", "America/New_York"]
            .map(str::to_owned)
            .to_vec();
        let near = |latitude, longitude| {
            nearest_zone(
                TABLE,
                Place {
                    latitude,
                    longitude,
                    accuracy: 0.0,
                },
                &zones,
            )
        };
        assert_eq!(near(50.85, 4.35).as_deref(), Some("Europe/Paris"));
        assert_eq!(near(34.69, 135.5).as_deref(), Some("Asia/Tokyo"));
        assert_eq!(near(42.36, -71.06).as_deref(), Some("America/New_York"));
        assert_eq!(
            nearest_zone(
                TABLE,
                Place {
                    latitude: 0.0,
                    longitude: 0.0,
                    accuracy: 0.0
                },
                &[]
            ),
            None
        );
    }

    #[test]
    fn manual_times_are_validated() {
        assert!(local_usec("2026-11-11", "14:30").is_some());
        assert!(local_usec("2026-11-11", "14:30:15").is_some());
        assert_eq!(local_usec("2026-02-31", "10:00"), None);
        assert_eq!(local_usec("2026-13-01", "10:00"), None);
        assert_eq!(local_usec("1960-01-01", "10:00"), None);
        assert_eq!(local_usec("2026-11-11", "25:00"), None);
        assert_eq!(local_usec("yesterday", "10:00"), None);
        let (date, time) = now_fields();
        assert_eq!((date.len(), time.len()), (10, 5));
        assert!(now_label(true).contains('M'));
    }

    #[tokio::test]
    async fn the_page_changes_zone_network_time_and_clock() {
        let bus = TestBus::new();
        let dir = tempfile::tempdir().unwrap();
        let table = dir.path().join("zone1970.tab");
        std::fs::write(&table, TABLE).unwrap();
        let service = DateTimeService::with(bus.connect().await, table);
        assert!(matches!(
            service.snapshot().await,
            Err(ServiceError::Unavailable(_))
        ));
        let fake = FakeTimedate::serve(&bus).await;
        let state = service.snapshot().await.unwrap();
        assert_eq!(state.clock.timezone, "Europe/Paris");
        assert_eq!(state.zones.len(), 3);
        assert!(!state.locate, "no GeoClue on this bus");

        let state = service
            .apply(DateTimeAction::Timezone("Asia/Tokyo".into()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.clock.timezone, "Asia/Tokyo");
        assert!(matches!(
            service.apply(DateTimeAction::Time(1_700_000_000_000_000)).await,
            Err(ServiceError::Failed(message)) if message.contains("synchronization")
        ));
        service.apply(DateTimeAction::Ntp(false)).await.unwrap();
        service
            .apply(DateTimeAction::Time(1_700_000_000_000_000))
            .await
            .unwrap();
        assert_eq!(*fake.time.lock().unwrap(), Some(1_700_000_000_000_000));

        let _geoclue = canopa_services::testing::geoclue::FakeGeoclue::serve(&bus).await;
        let service = DateTimeService::with(bus.connect().await, dir.path().join("zone1970.tab"));
        assert!(service.snapshot().await.unwrap().locate);
        let state = service
            .apply(DateTimeAction::Locate)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.clock.timezone, "Europe/Paris");

        let mut updates = service.updates();
        assert!(updates.next().await.unwrap().is_ok());
        fake.connection.close().await.unwrap();
        assert!(matches!(
            updates.next().await,
            Some(Err(ServiceError::Unavailable(_)))
        ));
    }
}
