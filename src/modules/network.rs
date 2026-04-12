use crate::modules::view::{collapsed_icon, expanded_icon_label, horizontal_icon_label};
use crate::theme;

/// Connectivity level reported by the active backend.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum NetworkConnectivity {
    Full,    // full internet access
    Limited, // local/site connectivity only or captive portal
    #[default]
    None, // no connectivity
}

/// Runtime state for the network module.
#[derive(Debug, Clone, Default)]
pub struct NetworkState {
    pub connectivity: NetworkConnectivity,
    /// Connection label: NM profile name or networkd interface name.
    pub label: Option<String>,
}

/// Events emitted by the network D-Bus subscription.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    StateChanged {
        connectivity: NetworkConnectivity,
        label: Option<String>,
    },
    Unavailable,
}

fn icon_str(connectivity: &NetworkConnectivity) -> &'static str {
    match connectivity {
        NetworkConnectivity::Full => "◆",
        NetworkConnectivity::Limited => "◈",
        NetworkConnectivity::None => "✕",
    }
}

fn icon_color(connectivity: &NetworkConnectivity) -> iced::Color {
    match connectivity {
        NetworkConnectivity::Full => theme::ACCENT,
        NetworkConnectivity::Limited => theme::WARNING,
        NetworkConnectivity::None => theme::CRITICAL,
    }
}

pub struct NetworkModule {
    id: crate::modules::ModuleId,
}

impl NetworkModule {
    pub fn new(id: crate::modules::ModuleId) -> Self {
        NetworkModule { id }
    }
}

impl crate::modules::Module for NetworkModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn module_type(&self) -> &str {
        "network"
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none() // fed via Message::Network from app.rs
    }

    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;

        let net = match state {
            ModuleState::Network(s) => s,
            _ => return iced::widget::Space::new().into(),
        };

        collapsed_icon(icon_str(&net.connectivity), icon_color(&net.connectivity))
    }

    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;

        let net = match state {
            ModuleState::Network(s) => s,
            _ => return iced::widget::Space::new().into(),
        };

        let label_text = net.label.clone().unwrap_or_else(|| {
            match &net.connectivity {
                NetworkConnectivity::Full => "online",
                NetworkConnectivity::Limited => "limited",
                NetworkConnectivity::None => "offline",
            }
            .to_owned()
        });

        expanded_icon_label(
            icon_str(&net.connectivity),
            icon_color(&net.connectivity),
            label_text,
        )
    }

    fn view_horizontal(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;

        let net = match state {
            ModuleState::Network(s) => s,
            _ => return iced::widget::Space::new().into(),
        };

        let label_text = net.label.clone().unwrap_or_else(|| {
            match &net.connectivity {
                NetworkConnectivity::Full => "online",
                NetworkConnectivity::Limited => "limited",
                NetworkConnectivity::None => "offline",
            }
            .to_owned()
        });

        horizontal_icon_label(
            icon_str(&net.connectivity),
            icon_color(&net.connectivity),
            label_text,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;

    #[test]
    fn network_module_id_is_network() {
        let m = NetworkModule::new(crate::modules::ModuleId::new("network"));
        assert_eq!(m.id().0, "network");
        assert_eq!(m.module_type(), "network");
    }

    #[test]
    fn network_state_default_connectivity_is_none() {
        assert_eq!(
            NetworkState::default().connectivity,
            NetworkConnectivity::None
        );
    }

    #[test]
    fn network_state_default_label_is_none() {
        assert!(NetworkState::default().label.is_none());
    }

    #[test]
    fn network_state_stores_label() {
        let s = NetworkState {
            connectivity: NetworkConnectivity::Full,
            label: Some("Home WiFi".to_string()),
        };
        assert_eq!(s.label.as_deref(), Some("Home WiFi"));
    }
}
