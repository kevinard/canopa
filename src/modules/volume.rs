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
    icons: crate::config::IconRulesConfig,
    format: crate::config::FormatConfig,
}

impl VolumeModule {
    pub(super) fn new(
        id: crate::modules::ModuleId,
        step: u8,
        icons: crate::config::IconRulesConfig,
        format: crate::config::FormatConfig,
    ) -> Self {
        VolumeModule {
            id,
            step,
            icons,
            format,
        }
    }

    fn formatted_text(
        &self,
        volume: &VolumeState,
        mode: crate::config::FormatMode,
    ) -> Option<String> {
        let tier = volume_tier(volume.volume, volume.muted);
        let icon = self
            .icons
            .icon_for_state_or_value((tier == 0).then_some("muted"), Some(volume.volume))
            .unwrap_or_else(|| tier_icon(tier))
            .to_string();
        let value = format!("{}", volume.volume.round() as u64);
        let state = if tier == 0 { "muted" } else { "unmuted" }.to_string();
        self.format.template_for(mode).map(|template| {
            template.render_pairs(&[
                ("icon", icon),
                ("value", value.clone()),
                ("percent", value),
                ("state", state),
            ])
        })
    }
}

impl crate::modules::view::IconLabel for VolumeModule {
    fn icon(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Volume(s) => self
                .icons
                .icon_for_state_or_value(
                    (volume_tier(s.volume, s.muted) == 0).then_some("muted"),
                    Some(s.volume),
                )
                .unwrap_or_else(|| tier_icon(volume_tier(s.volume, s.muted)))
                .to_owned(),
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

    fn formatted_text_for_layout(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
    ) -> Option<String> {
        let crate::modules::ModuleState::Volume(s) = state else {
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

    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            crate::modules::ModuleState::Volume(_) => crate::modules::view::icon_label_view(
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
            crate::modules::ModuleState::Volume(_) => crate::modules::view::icon_label_view(
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
            crate::modules::ModuleState::Volume(_) => crate::modules::view::icon_label_view(
                self,
                state,
                crate::modules::view::IconLabelLayout::Horizontal,
            ),
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
    use crate::config::{IconRulesConfig, IconThreshold};
    use crate::modules::Module;
    use crate::modules::view::IconLabel;
    use std::collections::HashMap;

    fn format_config(field: &str, raw: &str) -> crate::config::FormatConfig {
        let template = crate::config::TextTemplate::parse(
            raw,
            &["icon", "value", "percent", "state"],
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
    fn volume_module_id_is_volume() {
        let m = VolumeModule::new(
            crate::modules::ModuleId::new("volume"),
            5,
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
}
