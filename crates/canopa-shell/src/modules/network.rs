use crate::theme;

/// Connectivity level reported by the active backend.
pub use canopa_services::network::Connectivity as NetworkConnectivity;
pub use canopa_services::network::NetworkKind;

/// Runtime state for the network module.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetworkState {
    pub connectivity: NetworkConnectivity,
    /// Connection label: NM profile name or networkd's sorted usable link names.
    pub label: Option<String>,
    /// What carries the primary connection (NetworkManager only).
    pub kind: Option<NetworkKind>,
    /// Active VPN connections (NetworkManager only).
    pub vpn: Vec<String>,
}

/// Events emitted by the network D-Bus subscription.
#[derive(Debug, Clone, PartialEq)]
pub enum NetworkEvent {
    StateChanged {
        connectivity: NetworkConnectivity,
        label: Option<String>,
        kind: Option<NetworkKind>,
        vpn: Vec<String>,
    },
    Unavailable,
}

fn icon_str(connectivity: &NetworkConnectivity) -> &'static str {
    match connectivity {
        NetworkConnectivity::Full => "◆",
        NetworkConnectivity::Limited => "◈",
        NetworkConnectivity::None => "✕",
        NetworkConnectivity::Unknown => "?",
        NetworkConnectivity::Connected => "◆",
        NetworkConnectivity::Portal => "◈",
    }
}

fn connectivity_state(connectivity: &NetworkConnectivity) -> &'static str {
    match connectivity {
        NetworkConnectivity::Full => "full",
        NetworkConnectivity::Limited => "limited",
        NetworkConnectivity::None => "none",
        NetworkConnectivity::Unknown => "unknown",
        NetworkConnectivity::Connected => "connected",
        NetworkConnectivity::Portal => "portal",
    }
}

fn icon_color(connectivity: &NetworkConnectivity) -> iced::Color {
    match connectivity {
        NetworkConnectivity::Full => theme::palette().accent,
        NetworkConnectivity::Limited => theme::palette().warning,
        NetworkConnectivity::None => theme::palette().danger,
        NetworkConnectivity::Unknown => theme::palette().muted,
        NetworkConnectivity::Connected => theme::palette().accent,
        NetworkConnectivity::Portal => theme::palette().warning,
    }
}

pub(super) struct NetworkModule {
    id: crate::modules::ModuleId,
    icons: crate::config::IconRulesConfig,
    format: crate::config::FormatConfig,
    content_rotation: crate::config::ContentRotation,
}

impl NetworkModule {
    #[cfg(test)]
    pub(super) fn new(
        id: crate::modules::ModuleId,
        icons: crate::config::IconRulesConfig,
        format: crate::config::FormatConfig,
    ) -> Self {
        Self::new_with_content_rotation(id, icons, format, crate::config::ContentRotation::None)
    }

    pub(super) fn new_with_content_rotation(
        id: crate::modules::ModuleId,
        icons: crate::config::IconRulesConfig,
        format: crate::config::FormatConfig,
        content_rotation: crate::config::ContentRotation,
    ) -> Self {
        NetworkModule {
            id,
            icons,
            format,
            content_rotation,
        }
    }

    /// The Quick settings card's title: what carries the connection.
    fn card_title(state: &NetworkState) -> String {
        match state.kind {
            Some(NetworkKind::Wireless) => fl!("shell-network-wifi"),
            Some(NetworkKind::Wired) => fl!("shell-network-wired"),
            Some(NetworkKind::Vpn) => fl!("shell-network-vpn"),
            Some(NetworkKind::Other) | None => fl!("shell-network-network"),
        }
    }

    fn default_label(state: &NetworkState) -> String {
        state
            .label
            .clone()
            .unwrap_or_else(|| match &state.connectivity {
                NetworkConnectivity::Full => fl!("shell-network-online"),
                NetworkConnectivity::Limited => fl!("shell-network-limited"),
                NetworkConnectivity::None => fl!("shell-network-offline"),
                NetworkConnectivity::Unknown => fl!("shell-network-unknown"),
                NetworkConnectivity::Connected => fl!("shell-network-connected"),
                NetworkConnectivity::Portal => fl!("shell-network-portal"),
            })
    }

    fn styled_segments_with_default(
        &self,
        network: &NetworkState,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        use std::borrow::Cow;

        let icon = self
            .icons
            .icon_for_state(connectivity_state(&network.connectivity))
            .unwrap_or_else(|| icon_str(&network.connectivity))
            .to_string();
        let label = Self::default_label(network);
        let state = connectivity_state(&network.connectivity);
        crate::modules::view::render_format_segments(
            &self.format,
            mode,
            &[
                ("icon", Cow::Owned(icon)),
                ("value", Cow::Borrowed(label.as_str())),
                ("label", Cow::Borrowed(label.as_str())),
                ("state", Cow::Borrowed(state)),
            ],
            Some(state),
            default_style,
        )
    }
}

impl crate::modules::view::IconLabel for NetworkModule {
    fn content_rotation(&self) -> crate::config::ContentRotation {
        self.content_rotation
    }
    fn accepts_state(&self, state: &crate::modules::ModuleState) -> bool {
        matches!(state, crate::modules::ModuleState::Network(_))
    }

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

    fn styled_segments_for_layout(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        let crate::modules::ModuleState::Network(s) = state else {
            return None;
        };
        self.styled_segments_with_default(s, layout.into(), default_style)
    }

    fn color(&self, state: &crate::modules::ModuleState) -> iced::Color {
        match state {
            crate::modules::ModuleState::Network(s) => icon_color(&s.connectivity),
            _ => crate::theme::palette().text,
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

    fn data_source(&self) -> Option<&dyn crate::modules::data_source::ModuleDataSourceMapper> {
        Some(self)
    }
}

impl crate::modules::data_source::ModuleDataSourceMapper for NetworkModule {
    fn data_source_identity(&self) -> crate::modules::data_source::ModuleDataSourceIdentity {
        crate::modules::data_source::ModuleDataSourceIdentity::Network
    }

    fn map_data_source_state(
        &self,
        state: &crate::modules::data_source::ModuleDataSourceState,
    ) -> crate::modules::data_source::ModuleInstanceOutcome {
        let crate::modules::data_source::ModuleDataSourceState::Network(reading) = state else {
            return crate::modules::data_source::ModuleInstanceOutcome::Unavailable;
        };

        crate::modules::data_source::ModuleInstanceOutcome::Visible(
            crate::modules::ModuleState::Network(NetworkState {
                connectivity: reading.connectivity.clone(),
                label: reading.label.clone(),
                kind: reading.kind,
                vpn: reading.vpn.clone(),
            }),
        )
    }
}

impl crate::modules::render::ModuleRender for NetworkModule {
    fn has_bar_panel_content(&self, _: &crate::modules::ModuleState) -> bool {
        true
    }

    fn render_bar_panel_with_style(
        &self,
        state: &crate::modules::ModuleState,
        _: &crate::config::Style,
        context: crate::modules::render::PanelModuleContext,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        let (title, description) = match state {
            crate::modules::ModuleState::Network(network) => {
                let mut description = Self::default_label(network);
                if !network.vpn.is_empty() {
                    description = fl!(
                        "shell-network-with-vpn",
                        description = description,
                        vpn = network.vpn.join(", ")
                    );
                }
                (Self::card_title(network), description)
            }
            _ => (
                fl!("shell-network-network"),
                fl!("shell-network-status-unavailable"),
            ),
        };
        Some(crate::modules::view::panel_control(
            include_bytes!("../../assets/wifi.svg"),
            title,
            description,
            super::view::PanelControlEnd::Navigate,
            context,
        ))
    }

    fn presentation(&self) -> Option<&dyn crate::modules::view::IconLabel> {
        Some(self)
    }

    fn tooltip_values(&self, state: &crate::modules::ModuleState) -> Vec<(&'static str, String)> {
        if let crate::modules::ModuleState::Network(n) = state {
            let icon = self
                .icons
                .icon_for_state(connectivity_state(&n.connectivity))
                .unwrap_or_else(|| icon_str(&n.connectivity))
                .to_string();
            let label = Self::default_label(n);
            let s = connectivity_state(&n.connectivity).to_string();
            vec![
                ("icon", icon),
                ("value", label.clone()),
                ("label", label),
                ("state", s),
            ]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ColorValue, IconRulesConfig, SegmentStyles, Style, ThemeToken};
    use crate::modules::Module;
    use crate::modules::data_source::ModuleDataSourceMapper;
    use crate::modules::view::IconLabel;
    use std::collections::HashMap;

    fn format_config(field: &str, raw: &str) -> crate::config::FormatConfig {
        let template = crate::config::FormatTemplate::parse(
            &[raw],
            &["icon", "value", "label", "state"],
            "test.format",
        )
        .unwrap();
        match field {
            "collapsed" => crate::config::FormatConfig {
                collapsed: Some(template),
                ..Default::default()
            },
            "expanded" => crate::config::FormatConfig {
                expanded: Some(template),
                ..Default::default()
            },
            "horizontal" => crate::config::FormatConfig {
                horizontal: Some(template),
                ..Default::default()
            },
            _ => unreachable!(),
        }
    }

    fn styled_format_config() -> crate::config::FormatConfig {
        let mut template = crate::config::FormatTemplate::parse(
            &["{icon}"],
            &["icon", "value", "label", "state"],
            "test.format",
        )
        .unwrap();
        template.set_segment_styles(
            0,
            SegmentStyles::Stateful {
                default: None,
                by_state: vec![
                    (
                        "limited".to_string(),
                        Style {
                            color: Some(ColorValue::Token(ThemeToken::Warning)),
                            ..Default::default()
                        },
                    ),
                    (
                        "none".to_string(),
                        Style {
                            color: Some(ColorValue::Token(ThemeToken::Critical)),
                            ..Default::default()
                        },
                    ),
                ],
            },
        );
        crate::config::FormatConfig {
            collapsed: Some(template),
            ..Default::default()
        }
    }

    #[test]
    fn network_module_id_is_network() {
        let m = NetworkModule::new(
            crate::modules::ModuleId::new("network"),
            IconRulesConfig::default(),
            crate::config::FormatConfig::default(),
        );
        assert_eq!(m.id().display_name(), "network");
        assert_eq!(m.module_type(), "network");
    }

    #[test]
    fn network_maps_source_reading_to_visible_module_state() {
        let module = NetworkModule::new(
            crate::modules::ModuleId::new("network"),
            crate::config::IconRulesConfig::default(),
            crate::config::FormatConfig::default(),
        );

        let outcome = module.map_data_source_state(
            &crate::modules::data_source::ModuleDataSourceState::Network(
                crate::modules::data_source::NetworkReading {
                    connectivity: NetworkConnectivity::Limited,
                    label: Some("captive".into()),
                    ..Default::default()
                },
            ),
        );

        assert_eq!(
            outcome,
            crate::modules::data_source::ModuleInstanceOutcome::Visible(
                crate::modules::ModuleState::Network(NetworkState {
                    connectivity: NetworkConnectivity::Limited,
                    label: Some("captive".into()),
                    ..Default::default()
                })
            )
        );
    }

    #[test]
    fn network_state_default_connectivity_is_unknown() {
        assert_eq!(
            NetworkState::default().connectivity,
            NetworkConnectivity::Unknown
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
        };

        let rendered = m
            .styled_segments_with_default(
                &state,
                crate::config::FormatMode::Expanded,
                &crate::config::Style::default(),
            )
            .unwrap();
        assert_eq!(rendered[0].0, "◆ wifi (full)");
    }

    #[test]
    fn network_styled_segments_resolve_connectivity_state() {
        let m = NetworkModule::new(
            crate::modules::ModuleId::new("network"),
            IconRulesConfig::default(),
            styled_format_config(),
        );
        let limited = NetworkState {
            connectivity: NetworkConnectivity::Limited,
            label: None,
            ..Default::default()
        };
        let none = NetworkState {
            connectivity: NetworkConnectivity::None,
            label: None,
            ..Default::default()
        };

        assert_eq!(
            m.styled_segments_with_default(
                &limited,
                crate::config::FormatMode::Collapsed,
                &crate::config::Style::default()
            )
            .unwrap()[0]
                .1
                .color,
            Some(crate::theme::palette().warning)
        );
        assert_eq!(
            m.styled_segments_with_default(
                &none,
                crate::config::FormatMode::Collapsed,
                &crate::config::Style::default()
            )
            .unwrap()[0]
                .1
                .color,
            Some(crate::theme::palette().danger)
        );
    }
}

#[cfg(test)]
mod card_tests {
    use super::*;

    #[test]
    fn the_card_names_what_carries_the_connection() {
        let state = |kind| NetworkState {
            kind,
            ..Default::default()
        };
        assert_eq!(
            NetworkModule::card_title(&state(Some(NetworkKind::Wired))),
            "Wired"
        );
        assert_eq!(
            NetworkModule::card_title(&state(Some(NetworkKind::Wireless))),
            "Wi-Fi"
        );
        assert_eq!(
            NetworkModule::card_title(&state(Some(NetworkKind::Vpn))),
            "VPN"
        );
        assert_eq!(NetworkModule::card_title(&state(None)), "Network");
    }
}
