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

/// UPower's manager object, with the lid and the device list.
pub const MANAGER_PATH: &str = "/org/freedesktop/UPower";
const DEVICE: &str = "org.freedesktop.UPower.Device";

/// One battery that powers the computer, with its health where UPower knows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Battery {
    /// The UPower object path, a stable id while the battery is present.
    pub path: String,
    pub vendor: String,
    pub model: String,
    /// 0.0 to 100.0.
    pub percentage: f64,
    pub state: PowerState,
    /// Full capacity as a percentage of the design capacity.
    pub capacity: Option<f64>,
    /// Watt-hours when full now, and when new.
    pub energy_full: Option<f64>,
    pub energy_full_design: Option<f64>,
    /// Charge cycles, when the firmware reports them.
    pub cycles: Option<u32>,
    /// Seconds until empty or full, when UPower has an estimate.
    pub time_to_empty: Option<u64>,
    pub time_to_full: Option<u64>,
}

/// The computer's power supply: AC or battery, the lid and each battery.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PowerSupply {
    pub on_battery: bool,
    pub lid_present: bool,
    pub lid_closed: bool,
    pub batteries: Vec<Battery>,
}

fn property<T: TryFrom<zbus::zvariant::OwnedValue>>(
    properties: &std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
    name: &str,
) -> Option<T> {
    properties
        .get(name)
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| T::try_from(value).ok())
}

async fn properties(
    conn: &zbus::Connection,
    path: &str,
    interface: &'static str,
) -> Result<std::collections::HashMap<String, zbus::zvariant::OwnedValue>, Error> {
    let proxy = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(SERVICE)?
        .path(path.to_owned())?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    let interface = zbus::names::InterfaceName::from_static_str(interface)
        .map_err(|_| Error::Invalid("interface name"))?;
    Ok(tokio::time::timeout(crate::bus::DEADLINE, proxy.get_all(interface)).await??)
}

/// Read the lid, the power source and every present system battery.
pub async fn read_power_supply(conn: &zbus::Connection) -> Result<PowerSupply, Error> {
    let manager = properties(conn, MANAGER_PATH, "org.freedesktop.UPower").await?;
    let proxy = zbus::Proxy::new(conn, SERVICE, MANAGER_PATH, "org.freedesktop.UPower").await?;
    let paths: Vec<zbus::zvariant::OwnedObjectPath> =
        tokio::time::timeout(crate::bus::DEADLINE, proxy.call("EnumerateDevices", &())).await??;
    let mut batteries = Vec::new();
    // A laptop has one or two batteries; bound what another process can send.
    for path in paths.into_iter().take(16) {
        let Ok(device) = properties(conn, path.as_str(), DEVICE).await else {
            continue;
        };
        // Type 2 is a battery; PowerSupply excludes mice and headsets.
        if property::<u32>(&device, "Type") != Some(2)
            || property::<bool>(&device, "PowerSupply") != Some(true)
            || property::<bool>(&device, "IsPresent") == Some(false)
        {
            continue;
        }
        let positive = |name| property::<f64>(&device, name).filter(|value| *value > 0.0);
        let seconds = |name| {
            property::<i64>(&device, name)
                .filter(|value| *value > 0)
                .map(|value| value as u64)
        };
        let text = |name| {
            crate::bus::bounded_text(
                property::<String>(&device, name).unwrap_or_default().trim(),
                80,
            )
        };
        batteries.push(Battery {
            path: path.to_string(),
            vendor: text("Vendor"),
            model: text("Model"),
            percentage: property(&device, "Percentage").unwrap_or(0.0),
            state: PowerState::from_upower(property(&device, "State").unwrap_or(0)),
            capacity: positive("Capacity").map(|value| value.min(100.0)),
            energy_full: positive("EnergyFull"),
            energy_full_design: positive("EnergyFullDesign"),
            cycles: property::<i32>(&device, "ChargeCycles")
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value > 0),
            time_to_empty: seconds("TimeToEmpty"),
            time_to_full: seconds("TimeToFull"),
        });
    }
    Ok(PowerSupply {
        on_battery: property(&manager, "OnBattery").unwrap_or(false),
        lid_present: property(&manager, "LidIsPresent").unwrap_or(false),
        lid_closed: property(&manager, "LidIsClosed").unwrap_or(false),
        batteries,
    })
}

/// [`read_power_supply`] again after each UPower signal (property changes,
/// devices added or removed), as a recovering stream.
pub fn watch_power_supply(conn: zbus::Connection) -> StateStream<PowerSupply> {
    crate::bus::watch_service(conn, SERVICE, |conn| async move {
        read_power_supply(&conn).await
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

    #[tokio::test]
    async fn batteries_lid_and_health_are_read_and_followed() {
        use crate::testing::upower::FakeUPower;
        let bus = TestBus::new();
        let client = bus.connect().await;
        let mut states = watch_power_supply(client.clone());
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(3), states.next())
                .await
                .unwrap(),
            Some(ServiceState::Unavailable)
        );
        let upower = FakeUPower::serve(&bus).await;
        let supply = read_power_supply(&client).await.unwrap();
        assert!(supply.lid_present && !supply.lid_closed && supply.on_battery);
        // The mouse battery is not a power supply.
        assert_eq!(supply.batteries.len(), 1);
        let battery = &supply.batteries[0];
        assert_eq!(
            (battery.vendor.as_str(), battery.model.as_str()),
            ("ACME", "BAT0")
        );
        assert_eq!(battery.capacity, Some(87.5));
        assert_eq!(battery.cycles, Some(312));
        assert_eq!(battery.time_to_empty, Some(5400));
        assert_eq!(battery.time_to_full, None);
        assert_eq!(battery.state, PowerState::Discharging);
        let ServiceState::Available(first) = states.next().await.unwrap() else {
            panic!("power supply expected");
        };
        assert!(!first.lid_closed);
        upower.set_lid(true).await;
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while let Some(state) = states.next().await {
                if state.available().is_some_and(|supply| supply.lid_closed) {
                    break;
                }
            }
        })
        .await
        .unwrap();
    }
}
