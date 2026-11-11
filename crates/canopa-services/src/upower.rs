//! Battery state from UPower's display device.
//!
//! UPower aggregates every battery into `DisplayDevice`; that is what a bar
//! or a lock screen shows. The observer follows property changes and the
//! service owner, so a restarted UPower recovers on the same subscription.
//!
//! ```no_run
//! use canopa_services::{ServiceState, upower};
//! use futures_util::StreamExt;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let mut battery = upower::watch(system);
//! while let Some(state) = battery.next().await {
//!     match state {
//!         ServiceState::Available(device) if device.present => {
//!             println!("{:.0} % {:?}", device.percentage, device.state)
//!         }
//!         ServiceState::Available(_) => println!("No battery"),
//!         ServiceState::Unavailable => println!("UPower is not running"),
//!     }
//! }
//! # Ok(()) }
//! ```
use crate::{Error, ServiceState, bus::StateStream};
use futures_util::StreamExt;
use tokio::sync::mpsc;

pub const SERVICE: &str = "org.freedesktop.UPower";
pub const DISPLAY_DEVICE: &str = "/org/freedesktop/UPower/devices/DisplayDevice";

/// UPower's `State` property.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PowerState {
    #[default]
    Unknown,
    Charging,
    Discharging,
    Empty,
    FullyCharged,
    PendingCharge,
    PendingDischarge,
}

impl PowerState {
    pub fn from_upower(value: u32) -> Self {
        match value {
            1 => Self::Charging,
            2 => Self::Discharging,
            3 => Self::Empty,
            4 => Self::FullyCharged,
            5 => Self::PendingCharge,
            6 => Self::PendingDischarge,
            _ => Self::Unknown,
        }
    }

    pub fn is_charging(self) -> bool {
        self == Self::Charging
    }
}

/// One reading of the display device.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DisplayDevice {
    /// 0.0 to 100.0.
    pub percentage: f64,
    pub state: PowerState,
    /// False on systems without a battery.
    pub present: bool,
}

/// UPower DisplayDevice proxy.
#[zbus::proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower/devices/DisplayDevice"
)]
trait UPowerDevice {
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;

    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn is_present(&self) -> zbus::Result<bool>;
}

/// One-shot read. A missing UPower is [`ServiceState::Unavailable`].
pub async fn read(conn: &zbus::Connection) -> Result<ServiceState<DisplayDevice>, Error> {
    if crate::bus::owner(conn, SERVICE).await?.is_none() {
        return Ok(ServiceState::Unavailable);
    }
    let proxy = UPowerDeviceProxy::builder(conn)
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    Ok(ServiceState::Available(read_state(&proxy).await?))
}

/// The display device with recovery: [`ServiceState::Unavailable`] on service
/// loss, then readings again once UPower returns.
pub fn watch(conn: zbus::Connection) -> StateStream<DisplayDevice> {
    crate::bus::recovering(move |tx| {
        let conn = conn.clone();
        tokio::spawn(async move {
            let work = observe(conn.clone(), tx.clone());
            crate::bus::while_service_owned(&conn, SERVICE, &tx, work).await;
        });
    })
}

/// The observation worker behind [`watch`]. Sends a reading initially and on
/// each property change, and `Err(())` once on failure or owner change.
pub async fn observe(
    conn: zbus::Connection,
    tx: mpsc::UnboundedSender<Result<DisplayDevice, ()>>,
) -> zbus::Error {
    // Build proxy — error if UPower not on bus.
    let proxy = match UPowerDeviceProxy::new(&conn).await {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(Err(()));
            return e;
        }
    };

    let mut pct_stream = proxy.receive_percentage_changed().await;
    let mut state_stream = proxy.receive_state_changed().await;
    let mut presence_stream = proxy.receive_is_present_changed().await;
    let mut owners = match proxy.inner().receive_owner_changed().await {
        Ok(stream) => stream,
        Err(error) => {
            let _ = tx.send(Err(()));
            return error;
        }
    };

    // Send initial state.
    match read_state(&proxy).await {
        Ok(data) => {
            if tx.send(Ok(data)).is_err() {
                return zbus::Error::Failure("channel closed".into());
            }
        }
        Err(e) => {
            let _ = tx.send(Err(()));
            return e;
        }
    }

    loop {
        let live = tokio::select! {
            next = pct_stream.next() => next.is_some(),
            next = state_stream.next() => next.is_some(),
            next = presence_stream.next() => next.is_some(),
            _ = owners.next() => { let _ = tx.send(Err(())); false },
        };
        if !live {
            break;
        }
        match read_state(&proxy).await {
            Ok(data) => {
                if tx.send(Ok(data)).is_err() {
                    break;
                }
            }
            Err(_) => {
                let _ = tx.send(Err(()));
                break;
            }
        }
    }
    zbus::Error::Failure("stream ended".into())
}

async fn read_state(proxy: &UPowerDeviceProxy<'_>) -> zbus::Result<DisplayDevice> {
    let percentage = proxy.percentage().await?;
    let state = proxy.state().await?;
    Ok(DisplayDevice {
        percentage,
        state: PowerState::from_upower(state),
        present: proxy.is_present().await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestBus, upower::FakeDisplayDevice};

    async fn next(stream: &mut StateStream<DisplayDevice>) -> ServiceState<DisplayDevice> {
        tokio::time::timeout(std::time::Duration::from_secs(3), stream.next())
            .await
            .unwrap()
            .unwrap()
    }

    #[tokio::test]
    async fn absent_service_and_owner_restart_recover_on_same_bus() {
        let bus = TestBus::new();
        let client = bus.connect().await;
        assert_eq!(read(&client).await, Ok(ServiceState::Unavailable));
        let mut stream = watch(client.clone());
        assert_eq!(next(&mut stream).await, ServiceState::Unavailable);
        for _ in 0..2 {
            let service = FakeDisplayDevice::serve(&bus, 73.0, true, 2).await;
            assert!(matches!(
                next(&mut stream).await,
                ServiceState::Available(DisplayDevice {
                    percentage: 73.0,
                    ..
                })
            ));
            assert!(read(&client).await.unwrap().is_available());
            service.connection.close().await.unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while next(&mut stream).await.is_available() {}
            })
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn presence_and_power_state_changes_are_observed_without_percentage_changes() {
        let bus = TestBus::new();
        let service = FakeDisplayDevice::serve(&bus, 73.0, false, 0).await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let worker = tokio::spawn(observe(bus.connect().await, tx));
        async fn expect(
            rx: &mut mpsc::UnboundedReceiver<Result<DisplayDevice, ()>>,
            state: PowerState,
            present: bool,
        ) {
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                loop {
                    let reading = rx.recv().await.unwrap().unwrap();
                    if reading
                        == (DisplayDevice {
                            percentage: 73.0,
                            state,
                            present,
                        })
                    {
                        break;
                    }
                }
            })
            .await
            .unwrap();
        }
        expect(&mut rx, PowerState::Unknown, false).await;
        for (present, state) in [
            (true, 1),
            (true, 4),
            (true, 5),
            (true, 6),
            (true, 2),
            (false, 0),
            (true, 2),
        ] {
            service.set(73.0, present, state).await;
            expect(&mut rx, PowerState::from_upower(state), present).await;
        }
        worker.abort();
        let _ = worker.await;
    }

    #[test]
    fn upower_preserves_full_and_pending_states() {
        assert_eq!(PowerState::from_upower(4), PowerState::FullyCharged);
        assert_eq!(PowerState::from_upower(5), PowerState::PendingCharge);
        assert!(!PowerState::from_upower(4).is_charging());
        assert!(!PowerState::from_upower(5).is_charging());
        assert!(PowerState::from_upower(1).is_charging());
    }
}
