//! A fake systemd-timedated with three time zones and network time on.
use super::TestBus;
use crate::timedate::{PATH, SERVICE};
use std::sync::{Arc, Mutex};
use zbus::{fdo, object_server::SignalEmitter};

#[derive(Clone, Default)]
struct State {
    zone: Arc<Mutex<String>>,
    ntp: Arc<Mutex<bool>>,
    time: Arc<Mutex<Option<i64>>>,
    interactive: Arc<Mutex<bool>>,
}

struct Timedate(State);

const ZONES: [&str; 3] = ["America/New_York", "Asia/Tokyo", "Europe/Paris"];

#[zbus::interface(name = "org.freedesktop.timedate1")]
impl Timedate {
    async fn set_timezone(
        &self,
        zone: &str,
        interactive: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        if !ZONES.contains(&zone) {
            return Err(fdo::Error::InvalidArgs(format!(
                "Invalid or not installed time zone '{zone}'"
            )));
        }
        *self.0.interactive.lock().unwrap() = interactive;
        *self.0.zone.lock().unwrap() = zone.into();
        self.timezone_changed(&emitter).await?;
        Ok(())
    }
    #[zbus(name = "SetNTP")]
    async fn set_ntp(
        &self,
        enabled: bool,
        interactive: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        *self.0.interactive.lock().unwrap() = interactive;
        *self.0.ntp.lock().unwrap() = enabled;
        self.n_t_p_changed(&emitter).await?;
        Ok(())
    }
    fn set_time(&self, usec: i64, relative: bool, interactive: bool) -> fdo::Result<()> {
        if *self.0.ntp.lock().unwrap() {
            return Err(fdo::Error::Failed(
                "Automatic time synchronization is enabled".into(),
            ));
        }
        *self.0.interactive.lock().unwrap() = interactive;
        *self.0.time.lock().unwrap() = Some(if relative { 0 } else { usec });
        Ok(())
    }
    fn list_timezones(&self) -> Vec<String> {
        ZONES.map(str::to_owned).to_vec()
    }
    #[zbus(property)]
    fn timezone(&self) -> String {
        self.0.zone.lock().unwrap().clone()
    }
    #[zbus(property, name = "LocalRTC")]
    fn local_rtc(&self) -> bool {
        false
    }
    #[zbus(property, name = "CanNTP")]
    fn can_ntp(&self) -> bool {
        true
    }
    #[zbus(property, name = "NTP")]
    fn ntp(&self) -> bool {
        *self.0.ntp.lock().unwrap()
    }
    #[zbus(property, name = "NTPSynchronized")]
    fn ntp_synchronized(&self) -> bool {
        *self.0.ntp.lock().unwrap()
    }
}

/// timedated on a private bus. Time zone `Europe/Paris`, network time on.
pub struct FakeTimedate {
    /// Close it to simulate timedated leaving.
    pub connection: zbus::Connection,
    /// The last absolute time set, in microseconds.
    pub time: Arc<Mutex<Option<i64>>>,
    /// Whether the last change allowed a polkit prompt.
    pub interactive: Arc<Mutex<bool>>,
}

impl FakeTimedate {
    pub async fn serve(bus: &TestBus) -> Self {
        let state = State::default();
        *state.zone.lock().unwrap() = "Europe/Paris".into();
        *state.ntp.lock().unwrap() = true;
        let connection = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at(PATH, Timedate(state.clone()))
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .build()
            .await
            .unwrap();
        Self {
            connection,
            time: state.time,
            interactive: state.interactive,
        }
    }
}
