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
        NetworkConnectivity::Full => iced::Color::from_rgb8(0x89, 0xb4, 0xfa), // accent
        NetworkConnectivity::Limited => iced::Color::from_rgb8(0xf9, 0xe2, 0xaf), // warning
        NetworkConnectivity::None => iced::Color::from_rgb8(0xf3, 0x8b, 0xa8), // critical
    }
}

pub struct NetworkModule {
    id: crate::modules::ModuleId,
}

impl NetworkModule {
    pub fn new() -> Self {
        NetworkModule {
            id: crate::modules::ModuleId::new("network"),
        }
    }
}

impl crate::modules::Module for NetworkModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none() // fed via Message::Network from app.rs
    }

    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{container, text};
        use iced::{Alignment, Length};

        let net = match state {
            ModuleState::Network(s) => s,
            _ => return iced::widget::Space::new().into(),
        };

        container(
            text(icon_str(&net.connectivity))
                .size(16)
                .color(icon_color(&net.connectivity)),
        )
        .width(Length::Fill)
        .height(20)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    }

    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{container, row, text};
        use iced::{Alignment, Length};

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

        container(
            row![
                text(icon_str(&net.connectivity))
                    .size(16)
                    .color(icon_color(&net.connectivity)),
                text(label_text)
                    .size(12)
                    .color(iced::Color::from_rgb8(0xa6, 0xad, 0xc8)) // color.subtext
                    .width(Length::Fill),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(20)
        .align_y(Alignment::Center)
        .padding([0, 6])
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;

    #[test]
    fn network_module_id_is_network() {
        assert_eq!(NetworkModule::new().id().0, "network");
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
