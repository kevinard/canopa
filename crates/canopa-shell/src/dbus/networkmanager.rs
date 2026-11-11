//! Network module subscription over the shared status observer.
use crate::modules::network::NetworkEvent;
use canopa_services::{ServiceState, network::status};
use iced::futures::StreamExt;

struct NetworkConnData(zbus::Connection);

impl std::hash::Hash for NetworkConnData {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "network-subscription".hash(h);
        self.0.unique_name().hash(h);
        self.0.server_guid().hash(h);
    }
}

fn build_network_stream(
    data: &NetworkConnData,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>> {
    Box::pin(status::watch(data.0.clone()).map(|state| {
        let event = match state {
            ServiceState::Available(status) => NetworkEvent::StateChanged {
                connectivity: status.connectivity,
                label: status.label,
            },
            ServiceState::Unavailable => NetworkEvent::Unavailable,
        };
        crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Network(event))
    }))
}

/// Returns an Iced subscription for network state.
/// Tries NetworkManager first; falls back to systemd-networkd; emits Unavailable if both absent.
pub(crate) fn network_subscription(
    conn: zbus::Connection,
) -> iced::Subscription<crate::app::Message> {
    iced::Subscription::run_with(NetworkConnData(conn), build_network_stream)
}
