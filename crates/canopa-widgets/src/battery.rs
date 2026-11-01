#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BatteryPowerState {
    #[default]
    Unknown,
    Charging,
    Discharging,
    Empty,
    FullyCharged,
    PendingCharge,
    PendingDischarge,
}

impl BatteryPowerState {
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

/// Toolkit-independent battery state, fed by UPower property changes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BatteryState {
    pub percentage: f64, // 0.0–100.0
    pub power_state: BatteryPowerState,
    pub present: bool,
}

/// Events emitted by the UPower D-Bus subscription.
#[derive(Debug, Clone, PartialEq)]
pub enum BatteryEvent {
    StateChanged {
        percentage: f64,
        power_state: BatteryPowerState,
        present: bool,
    },
    Unavailable,
}

use tokio::sync::mpsc;
/// UPower DisplayDevice D-Bus proxy (zbus 5.x).
///
/// - Service: org.freedesktop.UPower
/// - Path:    /org/freedesktop/UPower/devices/DisplayDevice
/// - Interface: org.freedesktop.UPower.Device
///
/// UPower State values: 1=Charging, 2=Discharging, 3=Empty, 4=FullyCharged,
/// 5=PendingCharge, 6=PendingDischarge
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

/// Drives the UPower property subscription.
///
/// Sends (percentage, power state, presence) on initial read and on each property change.
/// Sends Err(()) on any fatal error (proxy creation, property read failure).
/// The task owns the proxy for its entire lifetime, avoiding lifetime issues with the generated proxy.
pub async fn run_upower_task(
    conn: zbus::Connection,
    tx: mpsc::UnboundedSender<Result<(f64, BatteryPowerState, bool), ()>>,
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

    use futures_util::StreamExt;
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

async fn read_state(proxy: &UPowerDeviceProxy<'_>) -> zbus::Result<(f64, BatteryPowerState, bool)> {
    let pct = proxy.percentage().await?;
    let state = proxy.state().await?;
    Ok((
        pct,
        BatteryPowerState::from_upower(state),
        proxy.is_present().await?,
    ))
}
