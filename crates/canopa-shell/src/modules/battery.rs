use crate::theme;
pub use canopa_widgets::battery::{BatteryEvent, BatteryPowerState, BatteryState};

/// Maps percentage + threshold to a charge tier.
/// Tiers: Critical (≤threshold) / Low (threshold+1–30) / Medium (31–60) / High (61–95) / Full (≥96)
///
/// Tiers are independent of `StatesConfig`: they back the built-in icon
/// fallback (`tier_icon`) used when `IconRulesConfig` has no value rules, and
/// they drive the hard-coded `icon_color` palette (which is not user-themable
/// per state). User-supplied state names live in `StatesConfig` and are used
/// for format template selectors (`state="…"`); the two paths are deliberate.
fn charge_tier(percentage: f64, critical_threshold: u8) -> u8 {
    let pct = percentage.clamp(0.0, 100.0);
    if pct <= f64::from(critical_threshold) {
        0 // Critical
    } else if pct <= 30.0 {
        1 // Low
    } else if pct <= 60.0 {
        2 // Medium
    } else if pct <= 95.0 {
        3 // High
    } else {
        4 // Full
    }
}

/// Icon character for each charge tier (not-charging state).
fn tier_icon(tier: u8) -> &'static str {
    match tier {
        0 => "canopa:battery-0", // Critical
        1 => "canopa:battery-1", // Low
        2 => "canopa:battery-2", // Medium
        3 => "canopa:battery-3", // High
        _ => "canopa:battery-4", // Full
    }
}

/// "78% · On battery power", for links to the power provider.
pub(crate) fn panel_summary(state: &crate::modules::ModuleState) -> Option<String> {
    let crate::modules::ModuleState::Battery(battery) = state else {
        return None;
    };
    battery.present.then(|| {
        // Named, so the charge is never read as the brightness beside it.
        fl!(
            "shell-battery-summary",
            percent = canopa_i18n::format::percent(battery.percentage.round()),
            state = power_state_text(battery.power_state)
        )
    })
}

fn power_state_text(state: BatteryPowerState) -> String {
    match state {
        BatteryPowerState::Charging => fl!("shell-battery-charging"),
        BatteryPowerState::FullyCharged => fl!("shell-battery-full"),
        BatteryPowerState::PendingCharge => fl!("shell-battery-plugged-in"),
        _ => fl!("shell-battery-on-battery"),
    }
}

/// Resolve the active state name from runtime data + the merged `StatesConfig`.
/// `is_charging` short-circuits to "charging"; otherwise the rule with the
/// smallest threshold ≥ percentage wins (with fallback to the highest rule).
pub(crate) fn battery_state_key(
    battery: &BatteryState,
    states: &crate::config::StatesConfig,
) -> String {
    match battery.power_state {
        BatteryPowerState::Charging => return "charging".into(),
        BatteryPowerState::FullyCharged => return "full".into(),
        BatteryPowerState::PendingCharge => return "pending-charge".into(),
        BatteryPowerState::PendingDischarge => return "pending-discharge".into(),
        BatteryPowerState::Unknown => return "unknown".into(),
        BatteryPowerState::Discharging | BatteryPowerState::Empty => {}
    }
    states
        .resolve(battery.percentage)
        .unwrap_or("normal")
        .to_string()
}

/// Icon color based on state (AC: #2, #5, #6).
///
/// Priority: charging > critical > warning > normal
fn icon_color(percentage: f64, is_charging: bool, critical_threshold: u8) -> iced::Color {
    if is_charging {
        return theme::palette().success;
    }
    let tier = charge_tier(percentage, critical_threshold);
    match tier {
        0 => theme::palette().danger,
        1 => theme::palette().warning,
        // A healthy battery matches the other spine marks.
        _ => theme::palette().text,
    }
}

pub(super) struct BatteryModule {
    id: crate::modules::ModuleId,
    /// Percentage threshold for CriticalBadge and red icon. Set from config at construction.
    critical_threshold: u8,
    states: crate::config::StatesConfig,
    icons: crate::config::IconRulesConfig,
    format: crate::config::FormatConfig,
    content_rotation: crate::config::ContentRotation,
}

impl BatteryModule {
    #[cfg(test)]
    pub(super) fn new(
        id: crate::modules::ModuleId,
        critical_threshold: u8,
        states: crate::config::StatesConfig,
        icons: crate::config::IconRulesConfig,
        format: crate::config::FormatConfig,
    ) -> Self {
        Self::new_with_content_rotation(
            id,
            critical_threshold,
            states,
            icons,
            format,
            crate::config::ContentRotation::None,
        )
    }

    pub(super) fn new_with_content_rotation(
        id: crate::modules::ModuleId,
        critical_threshold: u8,
        states: crate::config::StatesConfig,
        icons: crate::config::IconRulesConfig,
        format: crate::config::FormatConfig,
        content_rotation: crate::config::ContentRotation,
    ) -> Self {
        BatteryModule {
            id,
            critical_threshold,
            states,
            icons,
            format,
            content_rotation,
        }
    }

    fn current_icon(&self, battery: &BatteryState) -> String {
        let state = battery_state_key(battery, &self.states);
        self.icons
            .icon_for_state_or_value(Some(&state), Some(battery.percentage))
            .unwrap_or_else(|| {
                if battery.power_state.is_charging() {
                    "canopa:battery-charging"
                } else {
                    tier_icon(charge_tier(battery.percentage, self.critical_threshold))
                }
            })
            .to_owned()
    }

    fn styled_segments_with_default(
        &self,
        battery: &BatteryState,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        use std::borrow::Cow;

        let icon = self.current_icon(battery);
        let value = format!("{}", battery.percentage.round() as u8);
        let state = battery_state_key(battery, &self.states);
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

impl crate::modules::view::IconLabel for BatteryModule {
    fn decorate<'a>(
        &'a self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
        rotation: crate::config::ResolvedContentRotation,
        content: iced::Element<'a, crate::app::Message>,
    ) -> iced::Element<'a, crate::app::Message> {
        let crate::modules::ModuleState::Battery(battery) = state else {
            return content;
        };
        if layout != crate::modules::view::IconLabelLayout::Collapsed
            || rotation != crate::config::ResolvedContentRotation::None
            || battery.percentage > self.critical_threshold as f64
        {
            return content;
        }
        let badge = iced::widget::container(
            iced::widget::text(format!("{}%", battery.percentage.round() as u8))
                .size(9)
                .color(theme::palette().on_danger),
        )
        .padding([1, 3])
        .style(|_| iced::widget::container::Style {
            background: Some(iced::Background::Color(theme::palette().danger)),
            border: iced::Border {
                radius: iced::border::Radius::from(4.0),
                ..Default::default()
            },
            ..Default::default()
        });
        iced::widget::stack![content, badge].into()
    }

    fn content_rotation(&self) -> crate::config::ContentRotation {
        self.content_rotation
    }
    fn accepts_state(&self, state: &crate::modules::ModuleState) -> bool {
        matches!(state, crate::modules::ModuleState::Battery(_))
    }

    fn icon(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Battery(s) => self.current_icon(s),
            _ => String::new(),
        }
    }

    fn label(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Battery(s) => format!("{}%", s.percentage.round() as u8),
            _ => String::new(),
        }
    }

    fn styled_segments_for_layout(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        let crate::modules::ModuleState::Battery(s) = state else {
            return None;
        };
        self.styled_segments_with_default(s, layout.into(), default_style)
    }

    fn color(&self, state: &crate::modules::ModuleState) -> iced::Color {
        match state {
            crate::modules::ModuleState::Battery(s) => icon_color(
                s.percentage,
                s.power_state.is_charging(),
                self.critical_threshold,
            ),
            _ => crate::theme::palette().text,
        }
    }
}

impl crate::modules::Module for BatteryModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> crate::modules::ModuleKind {
        crate::modules::ModuleKind::Battery
    }

    /// No independent subscription — fed via Message::Battery in app.rs (AR5, Story 5.1).
    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }

    fn data_source(&self) -> Option<&dyn crate::modules::data_source::ModuleDataSourceMapper> {
        Some(self)
    }
}

impl crate::modules::data_source::ModuleDataSourceMapper for BatteryModule {
    fn data_source_identity(&self) -> crate::modules::data_source::ModuleDataSourceIdentity {
        crate::modules::data_source::ModuleDataSourceIdentity::Battery
    }

    fn map_data_source_state(
        &self,
        state: &crate::modules::data_source::ModuleDataSourceState,
    ) -> crate::modules::data_source::ModuleInstanceOutcome {
        let crate::modules::data_source::ModuleDataSourceState::Battery(reading) = state else {
            return crate::modules::data_source::ModuleInstanceOutcome::Unavailable;
        };

        if !reading.present {
            return crate::modules::data_source::ModuleInstanceOutcome::Empty;
        }
        crate::modules::data_source::ModuleInstanceOutcome::Visible(
            crate::modules::ModuleState::Battery(BatteryState {
                percentage: reading.percentage,
                power_state: reading.power_state,
                present: reading.present,
            }),
        )
    }
}

impl crate::modules::render::ModuleRender for BatteryModule {
    fn presentation(&self) -> Option<&dyn crate::modules::view::IconLabel> {
        Some(self)
    }

    fn has_bar_panel_content(&self, state: &crate::modules::ModuleState) -> bool {
        matches!(state, crate::modules::ModuleState::Battery(_))
    }

    /// Quick settings shows the charge as a row like Wi-Fi and Sound.
    fn render_bar_panel_with_style(
        &self,
        state: &crate::modules::ModuleState,
        _: &crate::config::Style,
        context: crate::modules::render::PanelModuleContext,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        let crate::modules::ModuleState::Battery(battery) = state else {
            return None;
        };
        let icon = crate::modules::view::embedded_icon(&self.current_icon(battery))
            .unwrap_or(include_bytes!("../../assets/battery-3.svg").as_slice());
        let description = power_state_text(battery.power_state);
        Some(crate::modules::view::panel_control(
            icon,
            fl!("shell-battery-title"),
            description,
            crate::modules::view::PanelControlEnd::Status(canopa_i18n::format::percent(
                battery.percentage.round(),
            )),
            context,
        ))
    }

    fn tooltip_values(&self, state: &crate::modules::ModuleState) -> Vec<(&'static str, String)> {
        if let crate::modules::ModuleState::Battery(b) = state {
            let icon = self.current_icon(b);
            let value = format!("{}", b.percentage.round() as u8);
            let s = battery_state_key(b, &self.states);
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
            &["{icon}", "{value}%"],
            &["icon", "value", "percent", "state"],
            "test.format",
        )
        .unwrap();
        template.set_segment_styles(
            0,
            SegmentStyles::Stateful {
                default: Some(Style {
                    color: Some(ColorValue::Token(ThemeToken::Accent)),
                    ..Default::default()
                }),
                by_state: vec![(
                    "critical".to_string(),
                    Style {
                        color: Some(ColorValue::Token(ThemeToken::Critical)),
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
    fn battery_module_id_is_battery() {
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            crate::config::StatesConfig::battery_default(),
            IconRulesConfig::default(),
            crate::config::FormatConfig::default(),
        );
        assert_eq!(m.id().display_name(), "battery");
        assert_eq!(m.module_type(), "battery");
    }

    #[test]
    fn battery_maps_source_reading_to_visible_module_state() {
        let module = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            crate::config::StatesConfig::battery_default(),
            crate::config::IconRulesConfig::default(),
            crate::config::FormatConfig::default(),
        );

        let absent = module.map_data_source_state(
            &crate::modules::data_source::ModuleDataSourceState::Battery(
                crate::modules::data_source::BatteryReading {
                    percentage: 0.0,
                    power_state: BatteryPowerState::Unknown,
                    present: false,
                },
            ),
        );
        assert_eq!(
            absent,
            crate::modules::data_source::ModuleInstanceOutcome::Empty
        );

        let outcome = module.map_data_source_state(
            &crate::modules::data_source::ModuleDataSourceState::Battery(
                crate::modules::data_source::BatteryReading {
                    percentage: 66.0,
                    power_state: crate::modules::battery::BatteryPowerState::Charging,
                    present: true,
                },
            ),
        );

        assert_eq!(
            outcome,
            crate::modules::data_source::ModuleInstanceOutcome::Visible(
                crate::modules::ModuleState::Battery(BatteryState {
                    percentage: 66.0,
                    power_state: crate::modules::battery::BatteryPowerState::Charging,
                    present: true,
                })
            )
        );
    }

    #[test]
    fn battery_state_default_percentage_zero() {
        assert_eq!(BatteryState::default().percentage, 0.0);
    }

    #[test]
    fn battery_state_default_not_charging() {
        assert!(!BatteryState::default().power_state.is_charging());
    }

    #[test]
    fn charge_tier_critical_at_threshold() {
        assert_eq!(charge_tier(15.0, 15), 0); // exactly at threshold → Critical
    }

    #[test]
    fn charge_tier_low_above_threshold() {
        assert_eq!(charge_tier(16.0, 15), 1); // just above → Low
    }

    #[test]
    fn charge_tier_uses_exact_percentage_above_threshold() {
        assert_eq!(charge_tier(15.6, 15), 1);
    }

    #[test]
    fn charge_tier_full() {
        assert_eq!(charge_tier(100.0, 15), 4);
    }

    #[test]
    fn icon_color_charging_returns_success() {
        assert_eq!(icon_color(10.0, true, 15), theme::palette().success);
    }

    #[test]
    fn icon_color_critical_not_charging_returns_critical() {
        assert_eq!(icon_color(10.0, false, 15), theme::palette().danger);
    }

    #[test]
    fn icon_color_uses_exact_percentage_above_threshold() {
        assert_eq!(icon_color(15.6, false, 15), theme::palette().warning);
    }

    #[test]
    fn icon_color_warning_not_charging_returns_warning() {
        assert_eq!(icon_color(25.0, false, 15), theme::palette().warning);
    }

    #[test]
    fn icon_color_normal_matches_the_spine_text() {
        assert_eq!(icon_color(80.0, false, 15), theme::palette().text);
    }

    #[test]
    fn battery_custom_charging_icon_overrides_threshold() {
        let mut states = HashMap::new();
        states.insert("charging".to_string(), "plug".to_string());
        let icons = IconRulesConfig {
            states,
            thresholds: vec![IconThreshold {
                at: 10.0,
                icon: "low".to_string(),
            }],
        };
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            crate::config::StatesConfig::battery_default(),
            icons,
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Battery(BatteryState {
            percentage: 20.0,
            power_state: crate::modules::battery::BatteryPowerState::Charging,
            present: true,
        });

        assert_eq!(m.icon(&state), "plug");
    }

    #[test]
    fn battery_custom_state_icon_overrides_thresholds() {
        let mut states = HashMap::new();
        states.insert("critical".to_string(), "crit".to_string());
        let icons = IconRulesConfig {
            states,
            thresholds: vec![IconThreshold {
                at: 0.0,
                icon: "threshold".to_string(),
            }],
        };
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            crate::config::StatesConfig::battery_default(),
            icons,
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Battery(BatteryState {
            percentage: 10.0,
            power_state: crate::modules::battery::BatteryPowerState::Discharging,
            present: true,
        });

        assert_eq!(m.icon(&state), "crit");
    }

    #[test]
    fn battery_custom_threshold_icon_overrides_builtin_tier() {
        let icons = IconRulesConfig {
            states: HashMap::new(),
            thresholds: vec![IconThreshold {
                at: 50.0,
                icon: "half".to_string(),
            }],
        };
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            crate::config::StatesConfig::battery_default(),
            icons,
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Battery(BatteryState {
            percentage: 75.0,
            power_state: crate::modules::battery::BatteryPowerState::Discharging,
            present: true,
        });

        assert_eq!(m.icon(&state), "half");
    }

    #[test]
    fn battery_falls_back_to_builtin_icon_without_matching_rule() {
        let icons = IconRulesConfig {
            states: HashMap::new(),
            thresholds: vec![IconThreshold {
                at: 90.0,
                icon: "full".to_string(),
            }],
        };
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            crate::config::StatesConfig::battery_default(),
            icons,
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Battery(BatteryState {
            percentage: 75.0,
            power_state: crate::modules::battery::BatteryPowerState::Discharging,
            present: true,
        });

        assert_eq!(m.icon(&state), "canopa:battery-3");
    }

    #[test]
    fn battery_format_includes_icon_value_and_state() {
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            crate::config::StatesConfig::battery_default(),
            IconRulesConfig::default(),
            format_config("expanded", "{icon} {value}% {state}"),
        );
        let state = BatteryState {
            percentage: 42.0,
            power_state: crate::modules::battery::BatteryPowerState::Discharging,
            present: true,
        };

        assert_eq!(
            m.styled_segments_with_default(
                &state,
                crate::config::FormatMode::Expanded,
                &Style::default()
            )
            .map(|segments| segments
                .into_iter()
                .map(|(text, _)| text)
                .collect::<Vec<_>>()
                .join(" ")),
            Some("canopa:battery-2 42% normal".to_string())
        );
    }

    #[test]
    fn battery_state_placeholder_uses_widened_vocabulary() {
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            crate::config::StatesConfig::battery_default(),
            IconRulesConfig::default(),
            format_config("collapsed", "{state}"),
        );
        let state = BatteryState {
            percentage: 5.0,
            power_state: crate::modules::battery::BatteryPowerState::Discharging,
            present: true,
        };

        assert_eq!(
            m.styled_segments_with_default(
                &state,
                crate::config::FormatMode::Collapsed,
                &Style::default()
            )
            .map(|segments| segments
                .into_iter()
                .map(|(text, _)| text)
                .collect::<Vec<_>>()
                .join(" ")),
            Some("critical".to_string())
        );
    }

    #[test]
    fn battery_styled_segments_resolve_critical_state() {
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            crate::config::StatesConfig::battery_default(),
            IconRulesConfig::default(),
            styled_format_config(),
        );
        let state = BatteryState {
            percentage: 5.0,
            power_state: crate::modules::battery::BatteryPowerState::Discharging,
            present: true,
        };

        let segments = m
            .styled_segments_with_default(
                &state,
                crate::config::FormatMode::Collapsed,
                &crate::config::Style::default(),
            )
            .unwrap();
        assert_eq!(segments[0].1.color, Some(crate::theme::palette().danger));
        assert_eq!(
            segments[1].1,
            crate::config::ResolvedStyle {
                size: Some((crate::theme::label_size() as u32) as f32),
                ..Default::default()
            }
        );
    }

    #[test]
    fn battery_collapsed_styled_segments_resolve_critical_state() {
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            crate::config::StatesConfig::battery_default(),
            IconRulesConfig::default(),
            styled_format_config(),
        );
        let state = BatteryState {
            percentage: 5.0,
            power_state: crate::modules::battery::BatteryPowerState::Discharging,
            present: true,
        };

        let segments = m
            .styled_segments_with_default(
                &state,
                crate::config::FormatMode::Collapsed,
                &crate::config::Style::default(),
            )
            .unwrap();

        assert_eq!(segments[0].1.color, Some(crate::theme::palette().danger));
        assert_eq!(
            segments[1].1,
            crate::config::ResolvedStyle {
                size: Some((crate::theme::label_size() as u32) as f32),
                ..Default::default()
            }
        );
    }
}
