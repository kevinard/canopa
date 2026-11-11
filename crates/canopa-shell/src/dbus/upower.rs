//! Battery module subscription over the shared UPower observer.
use crate::modules::battery::BatteryEvent;
use canopa_services::{ServiceState, upower};
use iced::futures::StreamExt;

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
    Box::pin(upower::watch(data.0.clone()).map(|state| {
        let event = match state {
            ServiceState::Available(device) => BatteryEvent::StateChanged {
                percentage: device.percentage,
                power_state: device.state,
                present: device.present,
            },
            ServiceState::Unavailable => BatteryEvent::Unavailable,
        };
        crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Battery(event))
    }))
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
