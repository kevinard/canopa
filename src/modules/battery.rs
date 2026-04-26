use crate::theme;

/// Runtime state for the battery module, fed by UPower D-Bus signals via app.rs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BatteryState {
    pub percentage: f64, // 0.0–100.0
    pub is_charging: bool,
}

/// Events emitted by the UPower D-Bus subscription.
#[derive(Debug, Clone, PartialEq)]
pub enum BatteryEvent {
    StateChanged { percentage: f64, is_charging: bool },
    Unavailable,
}

/// Maps percentage + threshold to a charge tier.
/// Tiers: Critical (≤threshold) / Low (threshold+1–30) / Medium (31–60) / High (61–95) / Full (≥96)
fn charge_tier(percentage: f64, critical_threshold: u8) -> u8 {
    let pct = percentage.clamp(0.0, 100.0) as u8;
    if pct <= critical_threshold {
        0 // Critical
    } else if pct <= 30 {
        1 // Low
    } else if pct <= 60 {
        2 // Medium
    } else if pct <= 95 {
        3 // High
    } else {
        4 // Full
    }
}

/// Icon character for each charge tier (not-charging state).
fn tier_icon(tier: u8) -> &'static str {
    match tier {
        0 => "▏", // Critical
        1 => "░", // Low
        2 => "▒", // Medium
        3 => "▓", // High
        _ => "█", // Full
    }
}

pub(crate) fn battery_state_key(battery: &BatteryState, critical_threshold: u8) -> &'static str {
    if battery.is_charging {
        "charging"
    } else {
        match charge_tier(battery.percentage, critical_threshold) {
            0 => "critical",
            1 => "low",
            _ => "normal",
        }
    }
}

/// Icon color based on state (AC: #2, #5, #6).
///
/// Priority: charging > critical > warning > normal
fn icon_color(percentage: f64, is_charging: bool, critical_threshold: u8) -> iced::Color {
    if is_charging {
        return theme::SUCCESS;
    }
    let tier = charge_tier(percentage, critical_threshold);
    match tier {
        0 => theme::CRITICAL,
        1 => theme::WARNING,
        _ => theme::ACCENT,
    }
}

pub(super) struct BatteryModule {
    id: crate::modules::ModuleId,
    /// Percentage threshold for CriticalBadge and red icon. Set from config at construction.
    critical_threshold: u8,
    icons: crate::config::IconRulesConfig,
    format: crate::config::FormatConfig,
}

impl BatteryModule {
    pub(super) fn new(
        id: crate::modules::ModuleId,
        critical_threshold: u8,
        icons: crate::config::IconRulesConfig,
        format: crate::config::FormatConfig,
    ) -> Self {
        BatteryModule {
            id,
            critical_threshold,
            icons,
            format,
        }
    }

    fn formatted_text(
        &self,
        battery: &BatteryState,
        mode: crate::config::FormatMode,
    ) -> Option<String> {
        self.formatted_segments(battery, mode)
            .map(|segments| segments.join(" "))
    }

    fn formatted_segments(
        &self,
        battery: &BatteryState,
        mode: crate::config::FormatMode,
    ) -> Option<Vec<String>> {
        let icon = self.current_icon(battery);
        let value = format!("{}", battery.percentage.round() as u8);
        let state = battery_state_key(battery, self.critical_threshold).to_string();
        self.format.template_for(mode).map(|template| {
            template.render_segments(&[
                ("icon", icon),
                ("value", value.clone()),
                ("percent", value),
                ("state", state),
            ])
        })
    }

    fn current_icon(&self, battery: &BatteryState) -> String {
        if battery.is_charging {
            self.icons
                .icon_for_state_or_value(Some("charging"), Some(battery.percentage))
                .unwrap_or("⚡")
                .to_owned()
        } else {
            self.icons
                .icon_for_value(battery.percentage)
                .unwrap_or_else(|| {
                    tier_icon(charge_tier(battery.percentage, self.critical_threshold))
                })
                .to_owned()
        }
    }

    fn styled_segments(
        &self,
        battery: &BatteryState,
        mode: crate::config::FormatMode,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        self.styled_segments_with_default(battery, mode, &crate::config::Style::default())
    }

    fn styled_segments_with_default(
        &self,
        battery: &BatteryState,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        let icon = self.current_icon(battery);
        let value = format!("{}", battery.percentage.round() as u8);
        let state = battery_state_key(battery, self.critical_threshold);
        self.format.template_for(mode).map(|template| {
            template.render_styled_segments_with_default(
                &[
                    ("icon", icon),
                    ("value", value.clone()),
                    ("percent", value),
                    ("state", state.to_string()),
                ],
                Some(state),
                default_style,
            )
        })
    }
}

impl crate::modules::view::IconLabel for BatteryModule {
    fn icon(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Battery(s) if s.is_charging => self
                .icons
                .icon_for_state_or_value(Some("charging"), Some(s.percentage))
                .unwrap_or("⚡")
                .to_owned(),
            crate::modules::ModuleState::Battery(s) => self
                .icons
                .icon_for_value(s.percentage)
                .unwrap_or_else(|| tier_icon(charge_tier(s.percentage, self.critical_threshold)))
                .to_owned(),
            _ => String::new(),
        }
    }

    fn label(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Battery(s) => format!("{}%", s.percentage.round() as u8),
            _ => String::new(),
        }
    }

    fn formatted_text_for_layout(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
    ) -> Option<String> {
        let crate::modules::ModuleState::Battery(s) = state else {
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

    fn formatted_segments_for_layout(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
    ) -> Option<Vec<String>> {
        let crate::modules::ModuleState::Battery(s) = state else {
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
        self.formatted_segments(s, mode)
    }

    fn styled_segments_for_layout(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        self.styled_segments_for_layout_with_default(
            state,
            layout,
            &crate::config::Style::default(),
        )
    }

    fn styled_segments_for_layout_with_default(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        let crate::modules::ModuleState::Battery(s) = state else {
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
        self.styled_segments_with_default(s, mode, default_style)
    }

    fn color(&self, state: &crate::modules::ModuleState) -> iced::Color {
        match state {
            crate::modules::ModuleState::Battery(s) => {
                icon_color(s.percentage, s.is_charging, self.critical_threshold)
            }
            _ => crate::theme::TEXT,
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

    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        self.view_collapsed_with_style(state, &crate::config::Style::default())
    }

    fn view_collapsed_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{container, stack, text};
        use iced::{Alignment, Length};

        let battery = match state {
            ModuleState::Battery(s) => s,
            _ => return iced::widget::Space::new().into(),
        };

        let icon_str = self
            .formatted_text(battery, crate::config::FormatMode::Collapsed)
            .unwrap_or_else(|| <Self as crate::modules::view::IconLabel>::icon(self, state));
        let color = icon_color(
            battery.percentage,
            battery.is_charging,
            self.critical_threshold,
        );

        // Battery needs the raw container (not the helper) because it may be stacked
        // with the CriticalBadge overlay below.
        let icon = container(crate::modules::view::styled_segment_text(
            icon_str,
            16,
            color,
            crate::modules::view::fallback_resolved_style(bar_style),
        ))
        .width(Length::Fill)
        .height(20)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

        // CriticalBadge: shown when collapsed AND at/below threshold (AC: #3, UX-DR17).
        if battery.percentage <= self.critical_threshold as f64 {
            let badge_label = format!("{}%", battery.percentage.round() as u8);
            let badge = container(text(badge_label).size(9).color(theme::BASE))
                .padding([1, 3])
                .style(|_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(theme::CRITICAL)),
                    border: iced::Border {
                        radius: iced::border::Radius::from(4.0),
                        ..Default::default()
                    },
                    ..Default::default()
                });
            stack![icon, badge].into()
        } else {
            icon.into()
        }
    }

    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        self.view_expanded_with_style(state, &crate::config::Style::default())
    }

    fn view_expanded_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            crate::modules::ModuleState::Battery(_) => {
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

    fn view_horizontal(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        self.view_horizontal_with_style(state, &crate::config::Style::default())
    }

    fn view_horizontal_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            crate::modules::ModuleState::Battery(_) => {
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
            IconRulesConfig::default(),
            crate::config::FormatConfig::default(),
        );
        assert_eq!(m.id().0, "battery");
        assert_eq!(m.module_type(), "battery");
    }

    #[test]
    fn battery_state_default_percentage_zero() {
        assert_eq!(BatteryState::default().percentage, 0.0);
    }

    #[test]
    fn battery_state_default_not_charging() {
        assert!(!BatteryState::default().is_charging);
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
    fn charge_tier_full() {
        assert_eq!(charge_tier(100.0, 15), 4);
    }

    #[test]
    fn icon_color_charging_returns_success() {
        assert_eq!(icon_color(10.0, true, 15), theme::SUCCESS);
    }

    #[test]
    fn icon_color_critical_not_charging_returns_critical() {
        assert_eq!(icon_color(10.0, false, 15), theme::CRITICAL);
    }

    #[test]
    fn icon_color_warning_not_charging_returns_warning() {
        assert_eq!(icon_color(25.0, false, 15), theme::WARNING);
    }

    #[test]
    fn icon_color_normal_returns_accent() {
        assert_eq!(icon_color(80.0, false, 15), theme::ACCENT);
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
            icons,
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Battery(BatteryState {
            percentage: 20.0,
            is_charging: true,
        });

        assert_eq!(m.icon(&state), "plug");
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
            icons,
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Battery(BatteryState {
            percentage: 75.0,
            is_charging: false,
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
            icons,
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Battery(BatteryState {
            percentage: 75.0,
            is_charging: false,
        });

        assert_eq!(m.icon(&state), "▓");
    }

    #[test]
    fn battery_format_includes_icon_value_and_state() {
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            IconRulesConfig::default(),
            format_config("expanded", "{icon} {value}% {state}"),
        );
        let state = BatteryState {
            percentage: 42.0,
            is_charging: false,
        };

        assert_eq!(
            m.formatted_text(&state, crate::config::FormatMode::Expanded),
            Some("▒ 42% normal".to_string())
        );
    }

    #[test]
    fn battery_state_placeholder_uses_widened_vocabulary() {
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            IconRulesConfig::default(),
            format_config("collapsed", "{state}"),
        );
        let state = BatteryState {
            percentage: 5.0,
            is_charging: false,
        };

        assert_eq!(
            m.formatted_segments(&state, crate::config::FormatMode::Collapsed),
            Some(vec!["critical".to_string()])
        );
    }

    #[test]
    fn battery_styled_segments_resolve_critical_state() {
        let m = BatteryModule::new(
            crate::modules::ModuleId::new("battery"),
            15,
            IconRulesConfig::default(),
            styled_format_config(),
        );
        let state = BatteryState {
            percentage: 5.0,
            is_charging: false,
        };

        let segments = m
            .styled_segments(&state, crate::config::FormatMode::Collapsed)
            .unwrap();
        assert_eq!(segments[0].1.color, Some(crate::theme::CRITICAL));
        assert_eq!(segments[1].1, crate::config::ResolvedStyle::default());
    }
}
