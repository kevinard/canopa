use tokio::sync::mpsc;

use crate::modules::battery::{BatteryEvent, BatteryPowerState};

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
    crate::dbus::subscription::build::<(f64, BatteryPowerState, bool), crate::app::Message, _, _, _>(
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
        |(pct, power_state, present)| {
            crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Battery(
                BatteryEvent::StateChanged {
                    percentage: pct,
                    power_state,
                    present,
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
/// Emits `BatteryEvent::Unavailable` on service loss and retries with bounded backoff.
pub(crate) fn battery_subscription(
    conn: zbus::Connection,
) -> iced::Subscription<crate::app::Message> {
    iced::Subscription::run_with(BatteryConnData(conn), build_battery_stream)
}

/// Drives the UPower property subscription.
///
/// Sends (percentage, power state, presence) on initial read and on each property change.
/// Sends Err(()) on any fatal error (proxy creation, property read failure).
/// The task owns the proxy for its entire lifetime, avoiding lifetime issues with the generated proxy.
async fn run_upower_task(
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

    use iced::futures::StreamExt;
    loop {
        let live = tokio::select! {
            next = pct_stream.next() => next.is_some(),
            next = state_stream.next() => next.is_some(),
            next = presence_stream.next() => next.is_some(),
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

#[cfg(test)]
mod tests {
    use super::*;
    struct Device {
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
            73.0
        }
        #[zbus(property)]
        fn state(&self) -> u32 {
            self.state
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
                .at(
                    "/org/freedesktop/UPower/devices/DisplayDevice",
                    Device {
                        present: true,
                        state: 2,
                    },
                )
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

    #[tokio::test]
    async fn presence_and_power_state_changes_are_observed_without_percentage_changes() {
        let bus = crate::test_bus::TestBus::new();
        let service = bus.connect().await;
        let path = "/org/freedesktop/UPower/devices/DisplayDevice";
        service
            .object_server()
            .at(
                path,
                Device {
                    present: false,
                    state: 0,
                },
            )
            .await
            .unwrap();
        service
            .request_name("org.freedesktop.UPower")
            .await
            .unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let worker = tokio::spawn(run_upower_task(bus.connect().await, tx));
        async fn expect(
            rx: &mut mpsc::UnboundedReceiver<Result<(f64, BatteryPowerState, bool), ()>>,
            state: BatteryPowerState,
            present: bool,
        ) {
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                loop {
                    let reading = rx.recv().await.unwrap().unwrap();
                    if reading == (73.0, state, present) {
                        break;
                    }
                }
            })
            .await
            .unwrap();
        }
        expect(&mut rx, BatteryPowerState::Unknown, false).await;
        let device = service
            .object_server()
            .interface::<_, Device>(path)
            .await
            .unwrap();
        for (present, state) in [
            (true, 1),
            (true, 4),
            (true, 5),
            (true, 6),
            (true, 2),
            (false, 0),
            (true, 2),
        ] {
            let mut guard = device.get_mut().await;
            guard.present = present;
            guard.state = state;
            guard
                .is_present_changed(device.signal_emitter())
                .await
                .unwrap();
            guard.state_changed(device.signal_emitter()).await.unwrap();
            drop(guard);
            expect(&mut rx, BatteryPowerState::from_upower(state), present).await;
        }
        worker.abort();
        let _ = worker.await;
    }

    #[test]
    fn upower_preserves_full_and_pending_states() {
        assert_eq!(
            BatteryPowerState::from_upower(4),
            BatteryPowerState::FullyCharged
        );
        assert_eq!(
            BatteryPowerState::from_upower(5),
            BatteryPowerState::PendingCharge
        );
        assert!(!BatteryPowerState::from_upower(4).is_charging());
        assert!(!BatteryPowerState::from_upower(5).is_charging());
        assert!(BatteryPowerState::from_upower(1).is_charging());
    }
}
