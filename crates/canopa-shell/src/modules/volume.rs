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
        0 => "canopa:sound-muted", // Muted/Zero
        1 => "canopa:sound-low",   // Low
        _ => "canopa:sound",       // Medium and high
    }
}

fn tier_color(tier: u8) -> iced::Color {
    if tier == 0 {
        theme::palette().muted // muted remains readable on bar and panel surfaces
    } else {
        theme::palette().text // normal, like the other spine marks
    }
}

pub(super) struct VolumeModule {
    id: crate::modules::ModuleId,
    step: u8,
    states: crate::config::StatesConfig,
    icons: crate::config::IconRulesConfig,
    format: crate::config::FormatConfig,
    content_rotation: crate::config::ContentRotation,
}

impl VolumeModule {
    #[cfg(test)]
    pub(super) fn new(
        id: crate::modules::ModuleId,
        step: u8,
        states: crate::config::StatesConfig,
        icons: crate::config::IconRulesConfig,
        format: crate::config::FormatConfig,
    ) -> Self {
        Self::new_with_content_rotation(
            id,
            step,
            states,
            icons,
            format,
            crate::config::ContentRotation::None,
        )
    }

    pub(super) fn new_with_content_rotation(
        id: crate::modules::ModuleId,
        step: u8,
        states: crate::config::StatesConfig,
        icons: crate::config::IconRulesConfig,
        format: crate::config::FormatConfig,
        content_rotation: crate::config::ContentRotation,
    ) -> Self {
        VolumeModule {
            id,
            step,
            states,
            icons,
            format,
            content_rotation,
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
    fn content_rotation(&self) -> crate::config::ContentRotation {
        self.content_rotation
    }
    fn accepts_state(&self, state: &crate::modules::ModuleState) -> bool {
        matches!(state, crate::modules::ModuleState::Volume(_))
    }

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
            _ => crate::theme::palette().text,
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

    fn data_source(&self) -> Option<&dyn crate::modules::data_source::ModuleDataSourceMapper> {
        Some(self)
    }

    fn interaction_handler(
        &self,
    ) -> Option<&dyn crate::modules::interaction::ModuleInteractionHandler> {
        Some(self)
    }
}

impl crate::modules::data_source::ModuleDataSourceMapper for VolumeModule {
    fn data_source_identity(&self) -> crate::modules::data_source::ModuleDataSourceIdentity {
        crate::modules::data_source::ModuleDataSourceIdentity::Volume
    }

    fn map_data_source_state(
        &self,
        state: &crate::modules::data_source::ModuleDataSourceState,
    ) -> crate::modules::data_source::ModuleInstanceOutcome {
        let crate::modules::data_source::ModuleDataSourceState::Volume(reading) = state else {
            return crate::modules::data_source::ModuleInstanceOutcome::Unavailable;
        };

        crate::modules::data_source::ModuleInstanceOutcome::Visible(
            crate::modules::ModuleState::Volume(VolumeState {
                volume: reading.volume,
                muted: reading.muted,
            }),
        )
    }
}

impl crate::modules::interaction::ModuleInteractionHandler for VolumeModule {
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

impl crate::modules::render::ModuleRender for VolumeModule {
    fn has_bar_panel_content(&self, _: &crate::modules::ModuleState) -> bool {
        true
    }

    fn render_bar_panel_with_style(
        &self,
        state: &crate::modules::ModuleState,
        _: &crate::config::Style,
        context: crate::modules::render::PanelModuleContext,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        use iced::widget::column;
        let volume = if let crate::modules::ModuleState::Volume(volume) = state {
            Some(volume)
        } else {
            None
        };
        let description = match volume {
            Some(volume) if volume.muted => fl!("shell-volume-muted"),
            Some(_) => context
                .provider_summary
                .clone()
                .unwrap_or_else(|| fl!("shell-volume-devices")),
            None => fl!("shell-volume-unavailable"),
        };
        // The heading names the devices; the speaker mark belongs to mute.
        let heading = crate::modules::view::panel_control(
            include_bytes!("../../assets/audio-devices.svg"),
            fl!("shell-volume-sound"),
            description,
            super::view::PanelControlEnd::Navigate,
            crate::modules::render::PanelModuleContext {
                primary: context.primary,
                focused: context.focused,
                scroll_up: None,
                scroll_down: None,
                footer: false,
                paired_toggle: false,
                opens_provider: None,
                provider_summary: None,
                provider_switch: None,
                provider_level: None,
                opens_menu: false,
                key: context.key.clone(),
            },
        );
        use canopa_ui::accessibility::{Node, Range, semantic};
        let level = volume.map_or_else(
            || "—".to_owned(),
            |v| canopa_i18n::format::percent(v.volume.round()),
        );
        let percent = volume.map_or(0.0, |v| v.volume.clamp(0.0, 100.0).round());
        let muted = volume.is_some_and(|v| v.muted);
        let slider_node = Node::slider(
            fl!("shell-volume-level"),
            Range {
                value: percent,
                min: 0.0,
                max: 100.0,
                step: Some(f64::from(self.step)),
            },
        )
        .key(format!("{}/level", context.key))
        .value(fl!("shell-volume-level-value", level = level.clone()))
        .disabled(volume.is_none())
        .on_increment(volume.and(context.scroll_up.clone()))
        .on_decrement(volume.and(context.scroll_down.clone()));
        let mute_node = Node::switch(fl!("shell-volume-mute"), muted)
            .key(format!("{}/mute", context.key))
            .disabled(volume.is_none())
            .on_click(volume.map(|_| crate::app::Message::VolumeToggleMute));
        let mute = crate::theme::icon_button(
            if muted || percent < 0.5 {
                include_bytes!("../../assets/sound-muted.svg").as_slice()
            } else if percent <= 32.0 {
                include_bytes!("../../assets/sound-low.svg").as_slice()
            } else {
                include_bytes!("../../assets/sound.svg").as_slice()
            },
            volume.map(|_| crate::app::Message::VolumeToggleMute),
            false,
        );
        let slider = crate::theme::level_slider(
            percent as f32,
            volume.map(|_| |value: f32| crate::app::Message::VolumeSet(value.round() as u8)),
            muted,
        );
        Some(
            column![
                heading,
                canopa_ui::direction::row([
                    semantic(mute_node, mute),
                    semantic(slider_node, slider),
                    iced::widget::container(
                        crate::theme::strong_text(level)
                            .size(crate::theme::label_size())
                            .style(canopa_ui::muted)
                    )
                    .width(40)
                    .align_x(canopa_ui::direction::end())
                    .into(),
                ])
                .spacing(12)
                .padding([0, 6])
                .align_y(iced::Alignment::Center),
            ]
            .spacing(4)
            .into(),
        )
    }

    fn presentation(&self) -> Option<&dyn crate::modules::view::IconLabel> {
        Some(self)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ColorValue, IconRulesConfig, IconThreshold, SegmentStyles, Style, ThemeToken,
    };
    use crate::modules::Module;
    use crate::modules::data_source::ModuleDataSourceMapper;
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
        assert_eq!(m.id().display_name(), "volume");
        assert_eq!(m.module_type(), "volume");
    }

    #[test]
    fn volume_maps_source_reading_to_visible_module_state() {
        let module = VolumeModule::new(
            crate::modules::ModuleId::new("volume"),
            5,
            crate::config::StatesConfig::volume_default(),
            crate::config::IconRulesConfig::default(),
            crate::config::FormatConfig::default(),
        );

        let outcome = module.map_data_source_state(
            &crate::modules::data_source::ModuleDataSourceState::Volume(
                crate::modules::data_source::VolumeReading {
                    volume: 22.0,
                    muted: false,
                },
            ),
        );

        assert_eq!(
            outcome,
            crate::modules::data_source::ModuleInstanceOutcome::Visible(
                crate::modules::ModuleState::Volume(VolumeState {
                    volume: 22.0,
                    muted: false,
                })
            )
        );
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

        assert_eq!(m.icon(&state), "canopa:sound");
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

        let rendered = m
            .styled_segments_with_default(
                &state,
                crate::config::FormatMode::Horizontal,
                &crate::config::Style::default(),
            )
            .unwrap();
        assert_eq!(rendered[0].0, "canopa:sound 80%");
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
            Some(crate::theme::palette().disabled)
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
