//! systemd-timedated: the time zone, network time and the system clock.
//!
//! timedated is started on demand and exits when idle, so a missing owner is
//! not an outage: reads address its well-known name, which starts it, and
//! [`watch`] follows `PropertiesChanged` from that name and re-reads every
//! few seconds. It is unavailable only when a read fails. Changes ask polkit,
//! which may show a password prompt.
//!
//! ```no_run
//! use canopa_services::timedate;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let clock = timedate::read(&system).await?;
//! if clock.can_ntp && !clock.ntp {
//!     timedate::set_ntp(&system, true).await?;
//! }
//! let zones = timedate::list_timezones(&system).await?;
//! if zones.iter().any(|zone| zone == "Europe/Paris") {
//!     timedate::set_timezone(&system, "Europe/Paris").await?;
//! }
//! # Ok(()) }
//! ```
use crate::{
    Error,
    bus::{DEADLINE, StateStream, call_interactive},
};
use std::time::Duration;

pub const SERVICE: &str = "org.freedesktop.timedate1";
pub const PATH: &str = "/org/freedesktop/timedate1";
pub const INTERFACE: &str = "org.freedesktop.timedate1";
/// How often [`watch`] re-reads without a signal.
const REREAD: Duration = Duration::from_secs(15);

/// timedated's view of the clock.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    /// An IANA zone such as `Europe/Paris`; empty when unset.
    pub timezone: String,
    /// The hardware clock keeps local time instead of UTC.
    pub local_rtc: bool,
    /// A network time service is installed.
    pub can_ntp: bool,
    /// Network time is on.
    pub ntp: bool,
    pub ntp_synchronized: bool,
}

async fn proxy(connection: &zbus::Connection) -> Result<zbus::Proxy<'static>, Error> {
    Ok(zbus::Proxy::new_owned(connection.clone(), SERVICE, PATH, INTERFACE).await?)
}

/// Read the clock settings, starting timedated if needed.
pub async fn read(connection: &zbus::Connection) -> Result<Clock, Error> {
    let all = crate::bus::properties(connection, SERVICE, PATH, INTERFACE).await?;
    let flag = |name: &str| {
        all.get(name)
            .and_then(|value| bool::try_from(value).ok())
            .unwrap_or(false)
    };
    Ok(Clock {
        timezone: all
            .get("Timezone")
            .and_then(|value| <&str>::try_from(value).ok())
            .map(|zone| crate::bus::bounded_text(zone, 128))
            .unwrap_or_default(),
        local_rtc: flag("LocalRTC"),
        can_ntp: flag("CanNTP"),
        ntp: flag("NTP"),
        ntp_synchronized: flag("NTPSynchronized"),
    })
}

/// The clock settings on each change, and every few seconds.
pub fn watch(connection: zbus::Connection) -> StateStream<Clock> {
    crate::bus::watch_on_demand(connection, SERVICE, PATH, REREAD, |connection| async move {
        read(&connection).await
    })
}

/// Every time zone timedated accepts.
pub async fn list_timezones(connection: &zbus::Connection) -> Result<Vec<String>, Error> {
    let proxy = proxy(connection).await?;
    let mut zones: Vec<String> =
        tokio::time::timeout(DEADLINE, proxy.call("ListTimezones", &())).await??;
    zones.retain(|zone| valid_zone(zone));
    zones.truncate(2048);
    Ok(zones)
}

/// A plausible IANA zone name.
pub fn valid_zone(zone: &str) -> bool {
    !zone.is_empty()
        && zone.len() <= 64
        && !zone.starts_with('/')
        && !zone.contains("..")
        && zone
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-' | b'+'))
}

pub async fn set_timezone(connection: &zbus::Connection, zone: &str) -> Result<(), Error> {
    if !valid_zone(zone) {
        return Err(Error::Invalid("time zone name"));
    }
    call_interactive(&proxy(connection).await?, "SetTimezone", &(zone, true)).await
}

/// Turn network time on or off.
pub async fn set_ntp(connection: &zbus::Connection, enabled: bool) -> Result<(), Error> {
    call_interactive(&proxy(connection).await?, "SetNTP", &(enabled, true)).await
}

/// Set the system clock to `usec` microseconds since the Unix epoch (UTC).
/// timedated refuses while network time is on.
pub async fn set_time(connection: &zbus::Connection, usec: i64) -> Result<(), Error> {
    if usec <= 0 {
        return Err(Error::Invalid("time before 1970"));
    }
    call_interactive(&proxy(connection).await?, "SetTime", &(usec, false, true)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ServiceState,
        testing::{TestBus, timedate::FakeTimedate},
    };
    use futures_util::StreamExt;

    #[tokio::test]
    async fn reads_follows_and_changes_the_clock() {
        let bus = TestBus::new();
        let client = bus.connect().await;
        assert_eq!(read(&client).await, Err(Error::Unavailable));
        let fake = FakeTimedate::serve(&bus).await;
        let clock = read(&client).await.unwrap();
        assert_eq!(clock.timezone, "Europe/Paris");
        assert!(clock.can_ntp && clock.ntp);
        let mut states = watch(client.clone());
        let next = async |states: &mut StateStream<Clock>| {
            tokio::time::timeout(Duration::from_secs(3), states.next())
                .await
                .unwrap()
                .unwrap()
        };
        assert!(matches!(
            next(&mut states).await,
            ServiceState::Available(_)
        ));

        assert!(
            list_timezones(&client)
                .await
                .unwrap()
                .contains(&"Asia/Tokyo".into())
        );
        set_timezone(&client, "Asia/Tokyo").await.unwrap();
        let ServiceState::Available(clock) = next(&mut states).await else {
            panic!("a clock expected");
        };
        assert_eq!(clock.timezone, "Asia/Tokyo");
        assert_eq!(
            set_timezone(&client, "../etc/passwd").await,
            Err(Error::Invalid("time zone name"))
        );
        assert_eq!(
            set_timezone(&client, "Mars/Olympus").await,
            Err(Error::Refused(
                "Invalid or not installed time zone 'Mars/Olympus'".into()
            ))
        );
        // Setting the time is refused while network time is on.
        assert!(matches!(
            set_time(&client, 1_700_000_000_000_000).await,
            Err(Error::Refused(_))
        ));
        set_ntp(&client, false).await.unwrap();
        set_time(&client, 1_700_000_000_000_000).await.unwrap();
        assert_eq!(*fake.time.lock().unwrap(), Some(1_700_000_000_000_000));
        assert!(*fake.interactive.lock().unwrap());
        let ServiceState::Available(clock) = next(&mut states).await else {
            panic!("a clock expected");
        };
        assert!(!clock.ntp);
        fake.connection.close().await.unwrap();
        assert_eq!(next(&mut states).await, ServiceState::Unavailable);
    }
}
