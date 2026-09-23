use tokio::sync::mpsc;

use crate::modules::battery::BatteryEvent;

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
}

/// Returns `true` for states that indicate "not discharging":
/// Charging (1), FullyCharged (4), PendingCharge (5).
fn upower_state_is_charging(state: u32) -> bool {
    matches!(state, 1 | 4 | 5)
}

/// Wrapper around `zbus::Connection` that implements `Hash` for use as subscription data.
/// There is at most one battery subscription per app, so a constant hash is correct.
struct BatteryConnData(zbus::Connection);

impl std::hash::Hash for BatteryConnData {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "battery-subscription".hash(h);
        self.0.unique_name().hash(h);
        self.0.server_guid().hash(h);
    }
}

fn build_battery_stream(
    data: &BatteryConnData,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>> {
    let conn = data.0.clone();
    crate::dbus::subscription::build::<(f64, bool), crate::app::Message, _, _, _>(
        move |tx| {
            let conn = conn.clone();
            tokio::spawn(async move {
                let work = run_upower_task(conn.clone(), tx.clone());
                crate::dbus::subscription::while_service_owned(
                    &conn,
                    "org.freedesktop.UPower",
                    &tx,
                    work,
                )
                .await;
            });
        },
        |(pct, charging)| {
            crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Battery(
                BatteryEvent::StateChanged {
                    percentage: pct,
                    is_charging: charging,
                },
            ))
        },
        || {
            crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Battery(
                BatteryEvent::Unavailable,
            ))
        },
    )
}

/// Returns an Iced subscription that connects to the UPower DisplayDevice on `conn`
/// and emits wrapped module battery events on each state change.
///
/// Emits `BatteryEvent::Unavailable` (once) if UPower is unreachable, then terminates.
pub(crate) fn battery_subscription(
    conn: zbus::Connection,
) -> iced::Subscription<crate::app::Message> {
    iced::Subscription::run_with(BatteryConnData(conn), build_battery_stream)
}

/// Drives the UPower property subscription.
///
/// Sends (percentage, is_charging) on initial read and on each property change.
/// Sends Err(()) on any fatal error (proxy creation, property read failure).
/// The task owns the proxy for its entire lifetime, avoiding lifetime issues with the generated proxy.
async fn run_upower_task(
    conn: zbus::Connection,
    tx: mpsc::UnboundedSender<Result<(f64, bool), ()>>,
) -> zbus::Error {
    // Build proxy — error if UPower not on bus.
    let proxy = match UPowerDeviceProxy::new(&conn).await {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(Err(()));
            return e;
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

    // Subscribe to property changes.
    // zbus 5.x: `receive_percentage_changed()` and `receive_state_changed()` return
    // `impl Stream` (awaitable). Use tokio::select! to merge both streams.
    let mut pct_stream = proxy.receive_percentage_changed().await;
    let mut state_stream = proxy.receive_state_changed().await;

    use iced::futures::StreamExt;
    loop {
        tokio::select! {
            _ = pct_stream.next() => {
                match read_state(&proxy).await {
                    Ok(data) => { if tx.send(Ok(data)).is_err() { break; } }
                    Err(_) => { let _ = tx.send(Err(())); break; }
                }
            }
            _ = state_stream.next() => {
                match read_state(&proxy).await {
                    Ok(data) => { if tx.send(Ok(data)).is_err() { break; } }
                    Err(_) => { let _ = tx.send(Err(())); break; }
                }
            }
        }
    }
    zbus::Error::Failure("stream ended".into())
}

async fn read_state(proxy: &UPowerDeviceProxy<'_>) -> zbus::Result<(f64, bool)> {
    let pct = proxy.percentage().await?;
    let state = proxy.state().await?;
    Ok((pct, upower_state_is_charging(state)))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Device;
    #[zbus::interface(name = "org.freedesktop.UPower.Device")]
    impl Device {
        #[zbus(property)]
        fn percentage(&self) -> f64 {
            73.0
        }
        #[zbus(property)]
        fn state(&self) -> u32 {
            2
        }
    }

    #[tokio::test]
    async fn absent_service_and_owner_restart_recover_on_same_bus() {
        use iced::futures::StreamExt;
        let bus = crate::test_bus::TestBus::new();
        let data = BatteryConnData(bus.connect().await);
        let mut stream = build_battery_stream(&data);
        let next = async |stream: &mut std::pin::Pin<
            Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send>,
        >| {
            tokio::time::timeout(std::time::Duration::from_secs(3), stream.next())
                .await
                .unwrap()
                .unwrap()
        };
        fn unavailable(message: crate::app::Message) -> bool {
            matches!(
                message,
                crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Battery(
                    BatteryEvent::Unavailable
                ))
            )
        }
        assert!(unavailable(next(&mut stream).await));
        for _ in 0..2 {
            let service = bus.connect().await;
            service
                .object_server()
                .at("/org/freedesktop/UPower/devices/DisplayDevice", Device)
                .await
                .unwrap();
            service
                .request_name("org.freedesktop.UPower")
                .await
                .unwrap();
            assert!(matches!(
                next(&mut stream).await,
                crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Battery(
                    BatteryEvent::StateChanged {
                        percentage: 73.0,
                        ..
                    }
                ))
            ));
            service.close().await.unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while !unavailable(next(&mut stream).await) {}
            })
            .await
            .unwrap();
        }
    }

    #[test]
    fn charging_state_1_is_charging() {
        assert!(upower_state_is_charging(1));
    }

    #[test]
    fn fully_charged_state_4_is_charging() {
        assert!(upower_state_is_charging(4));
    }

    #[test]
    fn discharging_state_2_is_not_charging() {
        assert!(!upower_state_is_charging(2));
    }
}
