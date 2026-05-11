use crate::theme;

/// Runtime state for the volume module.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VolumeState {
    /// Current volume percentage 0.0–150.0 (PA can exceed 100%).
    pub volume: f64,
    /// Whether the default sink is muted.
    pub muted: bool,
}

/// Events emitted by the PA subscription.
#[derive(Debug, Clone, PartialEq)]
pub enum VolumeEvent {
    StateChanged { volume: f64, muted: bool },
    Unavailable,
}

/// Map volume + mute to a display tier.
/// Tier 0=Muted/Zero, 1=Low (1–32%), 2=Medium (33–65%), 3=High (>65%)
///
/// Tiers back the built-in icon fallback (`tier_icon`) when `IconRulesConfig`
/// has no value rules, and the hard-coded `tier_color` palette. User-supplied
/// state names live in `StatesConfig` and drive format template selectors —
/// the two paths are deliberate. See `battery::charge_tier` for the same
/// rationale.
fn volume_tier(volume: f64, muted: bool) -> u8 {
    if muted || volume < 0.5 {
        0
    } else if volume <= 32.0 {
        1
    } else if volume <= 65.0 {
        2
    } else {
        3
    }
}

fn tier_icon(tier: u8) -> &'static str {
    match tier {
        0 => "✕", // Muted/Zero
        1 => "░", // Low
        2 => "▓", // Medium
        _ => "◈", // High
    }
}

fn tier_color(tier: u8) -> iced::Color {
    if tier == 0 {
        theme::OVERLAY0 // muted/dim
    } else {
        theme::ACCENT // normal
    }
}

pub(super) struct VolumeModule {
    id: crate::modules::ModuleId,
    step: u8,
    states: crate::config::StatesConfig,
    icons: crate::config::IconRulesConfig,
    format: crate::config::FormatConfig,
}

impl VolumeModule {
    pub(super) fn new(
        id: crate::modules::ModuleId,
        step: u8,
        states: crate::config::StatesConfig,
        icons: crate::config::IconRulesConfig,
        format: crate::config::FormatConfig,
    ) -> Self {
        VolumeModule {
            id,
            step,
            states,
            icons,
            format,
        }
    }

    fn state_key(&self, volume: &VolumeState) -> String {
        if volume.muted {
            "muted".to_string()
        } else {
            self.states
                .resolve(volume.volume)
                .unwrap_or("unmuted")
                .to_string()
        }
    }

    fn current_icon(&self, volume: &VolumeState) -> String {
        let tier = volume_tier(volume.volume, volume.muted);
        let state = self.state_key(volume);
        self.icons
            .icon_for_state_or_value(Some(&state), Some(volume.volume))
            .unwrap_or_else(|| tier_icon(tier))
            .to_string()
    }

    #[cfg(test)]
    fn formatted_text(
        &self,
        volume: &VolumeState,
        mode: crate::config::FormatMode,
    ) -> Option<String> {
        let icon = self.current_icon(volume);
        let value = format!("{}", volume.volume.round() as u64);
        let state = self.state_key(volume);
        self.format.template_for(mode).map(|template| {
            template.render_joined(&[
                ("icon", icon),
                ("value", value.clone()),
                ("percent", value),
                ("state", state),
            ])
        })
    }

    fn styled_segments_with_default(
        &self,
        volume: &VolumeState,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        use std::borrow::Cow;

        let icon = self.current_icon(volume);
        let value = format!("{}", volume.volume.round() as u64);
        let state = self.state_key(volume);
        crate::modules::view::render_format_segments(
            &self.format,
            mode,
            &[
                ("icon", Cow::Owned(icon)),
                ("value", Cow::Borrowed(value.as_str())),
                ("percent", Cow::Borrowed(value.as_str())),
                ("state", Cow::Borrowed(state.as_str())),
            ],
            Some(&state),
            default_style,
        )
    }
}

impl crate::modules::view::IconLabel for VolumeModule {
    fn icon(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Volume(s) => self.current_icon(s),
            _ => String::new(),
        }
    }

    fn label(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Volume(s) if s.muted => "muted".to_owned(),
            crate::modules::ModuleState::Volume(s) => format!("{}%", s.volume.round() as u64),
            _ => String::new(),
        }
    }

    fn styled_segments_for_layout(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        let crate::modules::ModuleState::Volume(s) = state else {
            return None;
        };
        self.styled_segments_with_default(s, layout.into(), default_style)
    }

    fn color(&self, state: &crate::modules::ModuleState) -> iced::Color {
        match state {
            crate::modules::ModuleState::Volume(s) => tier_color(volume_tier(s.volume, s.muted)),
            _ => crate::theme::TEXT,
        }
    }
}

impl crate::modules::Module for VolumeModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> crate::modules::ModuleKind {
        crate::modules::ModuleKind::Volume
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none() // fed via Message::Volume from app.rs
    }

    fn tooltip_values(&self, state: &crate::modules::ModuleState) -> Vec<(&'static str, String)> {
        if let crate::modules::ModuleState::Volume(v) = state {
            let icon = self.current_icon(v);
            let value = format!("{}", v.volume.round() as u64);
            let s = self.state_key(v);
            vec![
                ("icon", icon),
                ("value", value.clone()),
                ("percent", value),
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
            crate::modules::ModuleState::Volume(_) => {
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

    fn view_panel_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            crate::modules::ModuleState::Volume(_) => {
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
            crate::modules::ModuleState::Volume(_) => {
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

    fn on_interaction(
        &self,
        event: crate::modules::InteractionEvent,
    ) -> Option<crate::app::Message> {
        use crate::modules::InteractionEvent;
        match event {
            InteractionEvent::ScrollUp => Some(crate::app::Message::VolumeAdjust {
                step: self.step,
                up: true,
            }),
            InteractionEvent::ScrollDown => Some(crate::app::Message::VolumeAdjust {
                step: self.step,
                up: false,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ColorValue, IconRulesConfig, IconThreshold, SegmentStyles, Style, ThemeToken,
    };
    use crate::modules::Module;
    use crate::modules::view::IconLabel;
    use std::collections::HashMap;

    fn format_config(field: &str, raw: &str) -> crate::config::FormatConfig {
        let template = crate::config::FormatTemplate::parse(
            &[raw],
            &["icon", "value", "percent", "state"],
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
            &["icon", "value", "percent", "state"],
            "test.format",
        )
        .unwrap();
        template.set_segment_styles(
            0,
            SegmentStyles::Stateful {
                default: None,
                by_state: vec![(
                    "muted".to_string(),
                    Style {
                        color: Some(ColorValue::Token(ThemeToken::Overlay0)),
                        ..Default::default()
                    },
                )],
            },
        );
        crate::config::FormatConfig {
            collapsed: Some(template),
            ..Default::default()
        }
    }

    #[test]
    fn volume_module_id_is_volume() {
        let m = VolumeModule::new(
            crate::modules::ModuleId::new("volume"),
            5,
            crate::config::StatesConfig::volume_default(),
            IconRulesConfig::default(),
            crate::config::FormatConfig::default(),
        );
        assert_eq!(m.id().0, "volume");
        assert_eq!(m.module_type(), "volume");
    }

    #[test]
    fn volume_state_default_volume_is_zero() {
        assert_eq!(VolumeState::default().volume, 0.0);
    }

    #[test]
    fn volume_state_default_not_muted() {
        assert!(!VolumeState::default().muted);
    }

    #[test]
    fn volume_tier_muted_returns_zero() {
        assert_eq!(volume_tier(80.0, true), 0);
    }

    #[test]
    fn volume_tier_zero_returns_zero() {
        assert_eq!(volume_tier(0.0, false), 0);
    }

    #[test]
    fn volume_tier_low() {
        assert_eq!(volume_tier(20.0, false), 1);
    }

    #[test]
    fn volume_tier_medium() {
        assert_eq!(volume_tier(50.0, false), 2);
    }

    #[test]
    fn volume_tier_high() {
        assert_eq!(volume_tier(80.0, false), 3);
    }

    #[test]
    fn volume_custom_muted_icon_overrides_threshold() {
        let mut states = HashMap::new();
        states.insert("muted".to_string(), "mute".to_string());
        let icons = IconRulesConfig {
            states,
            thresholds: vec![IconThreshold {
                at: 10.0,
                icon: "low".to_string(),
            }],
        };
        let m = VolumeModule::new(
            crate::modules::ModuleId::new("volume"),
            5,
            crate::config::StatesConfig::volume_default(),
            icons,
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Volume(VolumeState {
            volume: 40.0,
            muted: true,
        });

        assert_eq!(m.icon(&state), "mute");
    }

    #[test]
    fn volume_custom_state_icon_overrides_thresholds() {
        let mut states_config = crate::config::StatesConfig::volume_default();
        states_config.upsert("quiet".to_string(), 33);
        let mut states = HashMap::new();
        states.insert("quiet".to_string(), "quiet-icon".to_string());
        let icons = IconRulesConfig {
            states,
            thresholds: vec![IconThreshold {
                at: 1.0,
                icon: "threshold".to_string(),
            }],
        };
        let m = VolumeModule::new(
            crate::modules::ModuleId::new("volume"),
            5,
            states_config,
            icons,
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Volume(VolumeState {
            volume: 20.0,
            muted: false,
        });

        assert_eq!(m.icon(&state), "quiet-icon");
    }

    #[test]
    fn volume_custom_threshold_icon_overrides_builtin_tier() {
        let icons = IconRulesConfig {
            states: HashMap::new(),
            thresholds: vec![IconThreshold {
                at: 66.0,
                icon: "loud".to_string(),
            }],
        };
        let m = VolumeModule::new(
            crate::modules::ModuleId::new("volume"),
            5,
            crate::config::StatesConfig::volume_default(),
            icons,
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Volume(VolumeState {
            volume: 80.0,
            muted: false,
        });

        assert_eq!(m.icon(&state), "loud");
    }

    #[test]
    fn volume_falls_back_to_builtin_icon_without_matching_rule() {
        let icons = IconRulesConfig {
            states: HashMap::new(),
            thresholds: vec![IconThreshold {
                at: 90.0,
                icon: "max".to_string(),
            }],
        };
        let m = VolumeModule::new(
            crate::modules::ModuleId::new("volume"),
            5,
            crate::config::StatesConfig::volume_default(),
            icons,
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Volume(VolumeState {
            volume: 80.0,
            muted: false,
        });

        assert_eq!(m.icon(&state), "◈");
    }

    #[test]
    fn volume_horizontal_format_includes_icon_and_value() {
        let m = VolumeModule::new(
            crate::modules::ModuleId::new("volume"),
            5,
            crate::config::StatesConfig::volume_default(),
            IconRulesConfig::default(),
            format_config("horizontal", "{icon} {value}%"),
        );
        let state = VolumeState {
            volume: 80.0,
            muted: false,
        };

        assert_eq!(
            m.formatted_text(&state, crate::config::FormatMode::Horizontal),
            Some("◈ 80%".to_string())
        );
    }

    #[test]
    fn volume_styled_segments_resolve_muted_state() {
        let m = VolumeModule::new(
            crate::modules::ModuleId::new("volume"),
            5,
            crate::config::StatesConfig::volume_default(),
            IconRulesConfig::default(),
            styled_format_config(),
        );

        let muted = VolumeState {
            volume: 80.0,
            muted: true,
        };
        let unmuted = VolumeState {
            volume: 80.0,
            muted: false,
        };

        assert_eq!(
            m.styled_segments_with_default(
                &muted,
                crate::config::FormatMode::Collapsed,
                &crate::config::Style::default()
            )
            .unwrap()[0]
                .1
                .color,
            Some(crate::theme::OVERLAY0)
        );
        assert_eq!(
            m.styled_segments_with_default(
                &unmuted,
                crate::config::FormatMode::Collapsed,
                &crate::config::Style::default()
            )
            .unwrap()[0]
                .1
                .color,
            None
        );
    }
}
