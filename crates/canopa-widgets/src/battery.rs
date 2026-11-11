pub use canopa_services::upower::PowerState as BatteryPowerState;

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

/// Drives the UPower display-device subscription ([`canopa_services::upower`]).
///
/// Sends (percentage, power state, presence) on initial read and on each property change.
/// Sends Err(()) on any fatal error (proxy creation, property read failure).
pub async fn run_upower_task(
    conn: zbus::Connection,
    tx: mpsc::UnboundedSender<Result<(f64, BatteryPowerState, bool), ()>>,
) -> zbus::Error {
    let (readings, mut rx) = mpsc::unbounded_channel();
    let observe = canopa_services::upower::observe(conn, readings);
    tokio::pin!(observe);
    loop {
        tokio::select! {
            error = &mut observe => {
                // Deliver what the observer sent before it ended.
                while let Ok(reading) = rx.try_recv() {
                    let _ = tx.send(reading.map(|d| (d.percentage, d.state, d.present)));
                }
                return error;
            }
            _ = tx.closed() => return zbus::Error::Failure("channel closed".into()),
            Some(reading) = rx.recv() => {
                if tx.send(reading.map(|d| (d.percentage, d.state, d.present))).is_err() {
                    return zbus::Error::Failure("channel closed".into());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_services::testing::{TestBus, upower::FakeDisplayDevice};

    #[tokio::test]
    async fn readings_keep_their_tuple_shape_and_service_loss_ends_the_task() {
        let bus = TestBus::new();
        let service = FakeDisplayDevice::serve(&bus, 40.0, true, 1).await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let worker = tokio::spawn(run_upower_task(bus.connect().await, tx));
        // Property streams may repeat a reading; wait for each expected one.
        let until = async |rx: &mut mpsc::UnboundedReceiver<_>, expected| {
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while rx.recv().await.unwrap() != expected {}
            })
            .await
            .unwrap()
        };
        until(&mut rx, Ok((40.0, BatteryPowerState::Charging, true))).await;
        service.set(41.0, true, 1).await;
        until(&mut rx, Ok((41.0, BatteryPowerState::Charging, true))).await;
        service.connection.close().await.unwrap();
        until(&mut rx, Err(())).await;
        tokio::time::timeout(std::time::Duration::from_secs(3), worker)
            .await
            .unwrap()
            .unwrap();
    }
}
