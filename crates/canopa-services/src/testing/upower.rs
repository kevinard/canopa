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
