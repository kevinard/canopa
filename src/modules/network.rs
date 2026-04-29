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

    #[cfg(test)]
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
            template.render_joined(&[
                ("icon", icon),
                ("value", label.clone()),
                ("label", label),
                ("state", state),
            ])
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

    fn view_collapsed_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            crate::modules::ModuleState::Network(_) => {
                crate::modules::view::icon_label_view_with_default(
                    self,
                    state,
                    crate::modules::view::IconLabelLayout::Collapsed,
                    bar_style,
                )
            }
            _ => iced::widget::Space::new().into(),
        }
    }

    fn view_expanded_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            crate::modules::ModuleState::Network(_) => {
                crate::modules::view::icon_label_view_with_default(
                    self,
                    state,
                    crate::modules::view::IconLabelLayout::Expanded,
                    bar_style,
                )
            }
            _ => iced::widget::Space::new().into(),
        }
    }

    fn view_horizontal_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            crate::modules::ModuleState::Network(_) => {
                crate::modules::view::icon_label_view_with_default(
                    self,
                    state,
                    crate::modules::view::IconLabelLayout::Horizontal,
                    bar_style,
                )
            }
            _ => iced::widget::Space::new().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ColorValue, IconRulesConfig, SegmentStyles, Style, ThemeToken};
    use crate::modules::Module;
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
        };
        let none = NetworkState {
            connectivity: NetworkConnectivity::None,
            label: None,
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
            Some(crate::theme::WARNING)
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
            Some(crate::theme::CRITICAL)
        );
    }
}
