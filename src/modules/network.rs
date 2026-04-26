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

fn connectivity_state(connectivity: &NetworkConnectivity) -> &'static str {
    match connectivity {
        NetworkConnectivity::Full => "full",
        NetworkConnectivity::Limited => "limited",
        NetworkConnectivity::None => "none",
    }
}

fn icon_color(connectivity: &NetworkConnectivity) -> iced::Color {
    match connectivity {
        NetworkConnectivity::Full => theme::ACCENT,
        NetworkConnectivity::Limited => theme::WARNING,
        NetworkConnectivity::None => theme::CRITICAL,
    }
}

pub(super) struct NetworkModule {
    id: crate::modules::ModuleId,
    icons: crate::config::IconRulesConfig,
}

impl NetworkModule {
    pub(super) fn new(id: crate::modules::ModuleId, icons: crate::config::IconRulesConfig) -> Self {
        NetworkModule { id, icons }
    }
}

impl crate::modules::view::IconLabel for NetworkModule {
    fn icon(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Network(s) => self
                .icons
                .icon_for_state(connectivity_state(&s.connectivity))
                .unwrap_or_else(|| icon_str(&s.connectivity))
                .to_owned(),
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
    use crate::config::IconRulesConfig;
    use crate::modules::Module;
    use crate::modules::view::IconLabel;
    use std::collections::HashMap;

    #[test]
    fn network_module_id_is_network() {
        let m = NetworkModule::new(
            crate::modules::ModuleId::new("network"),
            IconRulesConfig::default(),
        );
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

    #[test]
    fn network_custom_state_icon_overrides_builtin_icon() {
        let mut states = HashMap::new();
        states.insert("full".to_string(), "ok".to_string());
        let m = NetworkModule::new(
            crate::modules::ModuleId::new("network"),
            IconRulesConfig {
                states,
                thresholds: vec![],
            },
        );
        let state = crate::modules::ModuleState::Network(NetworkState {
            connectivity: NetworkConnectivity::Full,
            label: None,
        });

        assert_eq!(m.icon(&state), "ok");
    }

    #[test]
    fn network_falls_back_to_builtin_icon_without_matching_rule() {
        let mut states = HashMap::new();
        states.insert("none".to_string(), "off".to_string());
        let m = NetworkModule::new(
            crate::modules::ModuleId::new("network"),
            IconRulesConfig {
                states,
                thresholds: vec![],
            },
        );
        let state = crate::modules::ModuleState::Network(NetworkState {
            connectivity: NetworkConnectivity::Full,
            label: None,
        });

        assert_eq!(m.icon(&state), "◆");
    }
}
