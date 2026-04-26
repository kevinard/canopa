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
    format: crate::config::FormatConfig,
}

impl NetworkModule {
    pub(super) fn new(
        id: crate::modules::ModuleId,
        icons: crate::config::IconRulesConfig,
        format: crate::config::FormatConfig,
    ) -> Self {
        NetworkModule { id, icons, format }
    }

    fn default_label(state: &NetworkState) -> String {
        state.label.clone().unwrap_or_else(|| {
            match &state.connectivity {
                NetworkConnectivity::Full => "online",
                NetworkConnectivity::Limited => "limited",
                NetworkConnectivity::None => "offline",
            }
            .to_owned()
        })
    }

    fn formatted_text(
        &self,
        network: &NetworkState,
        mode: crate::config::FormatMode,
    ) -> Option<String> {
        let icon = self
            .icons
            .icon_for_state(connectivity_state(&network.connectivity))
            .unwrap_or_else(|| icon_str(&network.connectivity))
            .to_string();
        let label = Self::default_label(network);
        let state = connectivity_state(&network.connectivity).to_string();
        self.format.template_for(mode).map(|template| {
            template.render_pairs(&[
                ("icon", icon),
                ("value", label.clone()),
                ("label", label),
                ("state", state),
            ])
        })
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
            crate::modules::ModuleState::Network(s) => Self::default_label(s),
            _ => String::new(),
        }
    }

    fn formatted_text_for_layout(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
    ) -> Option<String> {
        let crate::modules::ModuleState::Network(s) = state else {
            return None;
        };
        let mode = match layout {
            crate::modules::view::IconLabelLayout::Collapsed => {
                crate::config::FormatMode::Collapsed
            }
            crate::modules::view::IconLabelLayout::Expanded => crate::config::FormatMode::Expanded,
            crate::modules::view::IconLabelLayout::Horizontal => {
                crate::config::FormatMode::Horizontal
            }
        };
        self.formatted_text(s, mode)
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

    fn format_config(field: &str, raw: &str) -> crate::config::FormatConfig {
        let template = crate::config::TextTemplate::parse(
            raw,
            &["icon", "value", "label", "state"],
            "test.format",
        )
        .unwrap();
        let mut config = crate::config::FormatConfig::default();
        match field {
            "collapsed" => config.collapsed = Some(template),
            "expanded" => config.expanded = Some(template),
            "horizontal" => config.horizontal = Some(template),
            _ => unreachable!(),
        }
        config
    }

    #[test]
    fn network_module_id_is_network() {
        let m = NetworkModule::new(
            crate::modules::ModuleId::new("network"),
            IconRulesConfig::default(),
            crate::config::FormatConfig::default(),
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
            crate::config::FormatConfig::default(),
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
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Network(NetworkState {
            connectivity: NetworkConnectivity::Full,
            label: None,
        });

        assert_eq!(m.icon(&state), "◆");
    }

    #[test]
    fn network_expanded_format_includes_label_and_state() {
        let m = NetworkModule::new(
            crate::modules::ModuleId::new("network"),
            IconRulesConfig::default(),
            format_config("expanded", "{icon} {label} ({state})"),
        );
        let state = NetworkState {
            connectivity: NetworkConnectivity::Full,
            label: Some("wifi".to_string()),
        };

        assert_eq!(
            m.formatted_text(&state, crate::config::FormatMode::Expanded),
            Some("◆ wifi (full)".to_string())
        );
    }
}
