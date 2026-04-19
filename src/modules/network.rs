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
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetworkState {
    pub connectivity: NetworkConnectivity,
    /// Connection label: NM profile name or networkd interface name.
    pub label: Option<String>,
}

/// Events emitted by the network D-Bus subscription.
#[derive(Debug, Clone, PartialEq)]
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

impl crate::modules::view::IconLabel for NetworkModule {
    fn icon(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Network(s) => icon_str(&s.connectivity).to_owned(),
            _ => String::new(),
        }
    }

    fn label(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Network(s) => s.label.clone().unwrap_or_else(|| {
                match &s.connectivity {
                    NetworkConnectivity::Full => "online",
                    NetworkConnectivity::Limited => "limited",
                    NetworkConnectivity::None => "offline",
                }
                .to_owned()
            }),
            _ => String::new(),
        }
    }

    fn color(&self, state: &crate::modules::ModuleState) -> iced::Color {
        match state {
            crate::modules::ModuleState::Network(s) => icon_color(&s.connectivity),
            _ => crate::theme::TEXT,
        }
    }
}

impl crate::modules::Module for NetworkModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> crate::modules::ModuleKind {
        crate::modules::ModuleKind::Network
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none() // fed via Message::Network from app.rs
    }

    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            crate::modules::ModuleState::Network(_) => crate::modules::view::icon_label_view(
                self,
                state,
                crate::modules::view::IconLabelLayout::Collapsed,
            ),
            _ => iced::widget::Space::new().into(),
        }
    }

    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            crate::modules::ModuleState::Network(_) => crate::modules::view::icon_label_view(
                self,
                state,
                crate::modules::view::IconLabelLayout::Expanded,
            ),
            _ => iced::widget::Space::new().into(),
        }
    }

    fn view_horizontal(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            crate::modules::ModuleState::Network(_) => crate::modules::view::icon_label_view(
                self,
                state,
                crate::modules::view::IconLabelLayout::Horizontal,
            ),
            _ => iced::widget::Space::new().into(),
        }
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
