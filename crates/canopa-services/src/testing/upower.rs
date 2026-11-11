//! A fake UPower with only the display device.
use super::TestBus;
use crate::upower::{DISPLAY_DEVICE, SERVICE};

struct Device {
    percentage: f64,
    present: bool,
    state: u32,
}

#[zbus::interface(name = "org.freedesktop.UPower.Device")]
impl Device {
    #[zbus(property)]
    fn is_present(&self) -> bool {
        self.present
    }
    #[zbus(property)]
    fn percentage(&self) -> f64 {
        self.percentage
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        self.state
    }
}

/// UPower's display device on a private bus. `state` uses UPower's numbers
/// (1 charging, 2 discharging, 4 fully charged, ...).
pub struct FakeDisplayDevice {
    /// The connection owning `org.freedesktop.UPower`; close it to simulate
    /// the service leaving.
    pub connection: zbus::Connection,
}

impl FakeDisplayDevice {
    pub async fn serve(bus: &TestBus, percentage: f64, present: bool, state: u32) -> Self {
        let connection = bus.connect().await;
        connection
            .object_server()
            .at(
                DISPLAY_DEVICE,
                Device {
                    percentage,
                    present,
                    state,
                },
            )
            .await
            .unwrap();
        connection.request_name(SERVICE).await.unwrap();
        Self { connection }
    }

    /// Change the reading and emit the property changes.
    pub async fn set(&self, percentage: f64, present: bool, state: u32) {
        let device = self
            .connection
            .object_server()
            .interface::<_, Device>(DISPLAY_DEVICE)
            .await
            .unwrap();
        let mut guard = device.get_mut().await;
        let emitter = device.signal_emitter();
        if guard.percentage != percentage {
            guard.percentage = percentage;
            guard.percentage_changed(emitter).await.unwrap();
        }
        guard.present = present;
        guard.state = state;
        guard.is_present_changed(emitter).await.unwrap();
        guard.state_changed(emitter).await.unwrap();
    }
}

struct Manager {
    lid_closed: bool,
}

#[zbus::interface(name = "org.freedesktop.UPower")]
impl Manager {
    #[zbus(property)]
    fn on_battery(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn lid_is_present(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn lid_is_closed(&self) -> bool {
        self.lid_closed
    }
    fn enumerate_devices(&self) -> Vec<zbus::zvariant::OwnedObjectPath> {
        [BATTERY, MOUSE]
            .into_iter()
            .map(|path| path.try_into().unwrap())
            .collect()
    }
}

struct BatteryDevice {
    power_supply: bool,
    model: &'static str,
}

#[zbus::interface(name = "org.freedesktop.UPower.Device")]
impl BatteryDevice {
    #[zbus(property, name = "Type")]
    fn kind(&self) -> u32 {
        2
    }
    #[zbus(property)]
    fn power_supply(&self) -> bool {
        self.power_supply
    }
    #[zbus(property)]
    fn is_present(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn vendor(&self) -> &str {
        "ACME"
    }
    #[zbus(property)]
    fn model(&self) -> &str {
        self.model
    }
    #[zbus(property)]
    fn percentage(&self) -> f64 {
        64.0
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        2
    }
    #[zbus(property)]
    fn capacity(&self) -> f64 {
        87.5
    }
    #[zbus(property)]
    fn energy_full(&self) -> f64 {
        49.0
    }
    #[zbus(property)]
    fn energy_full_design(&self) -> f64 {
        56.0
    }
    #[zbus(property)]
    fn charge_cycles(&self) -> i32 {
        312
    }
    #[zbus(property)]
    fn time_to_empty(&self) -> i64 {
        5400
    }
    #[zbus(property)]
    fn time_to_full(&self) -> i64 {
        0
    }
}

/// The system battery [`FakeUPower`] lists.
pub const BATTERY: &str = "/org/freedesktop/UPower/devices/battery_BAT0";
/// A mouse battery, which is not a power supply.
pub const MOUSE: &str = "/org/freedesktop/UPower/devices/mouse_0";

/// UPower with a lid, one system battery (64 %, discharging, 87.5 % health,
/// 312 cycles, 90 minutes left) and a mouse battery.
pub struct FakeUPower {
    pub connection: zbus::Connection,
}

impl FakeUPower {
    pub async fn serve(bus: &TestBus) -> Self {
        let connection = bus.connect().await;
        let server = connection.object_server();
        server
            .at(crate::upower::MANAGER_PATH, Manager { lid_closed: false })
            .await
            .unwrap();
        for (path, power_supply, model) in [(BATTERY, true, "BAT0"), (MOUSE, false, "Mouse")] {
            server
                .at(
                    path,
                    BatteryDevice {
                        power_supply,
                        model,
                    },
                )
                .await
                .unwrap();
        }
        connection.request_name(SERVICE).await.unwrap();
        Self { connection }
    }

    /// Open or close the lid and emit the property change.
    pub async fn set_lid(&self, closed: bool) {
        let manager = self
            .connection
            .object_server()
            .interface::<_, Manager>(crate::upower::MANAGER_PATH)
            .await
            .unwrap();
        let mut guard = manager.get_mut().await;
        guard.lid_closed = closed;
        guard
            .lid_is_closed_changed(manager.signal_emitter())
            .await
            .unwrap();
    }
}
