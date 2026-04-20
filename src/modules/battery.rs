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
}

impl BatteryModule {
    pub(super) fn new(id: crate::modules::ModuleId, critical_threshold: u8) -> Self {
        BatteryModule {
            id,
            critical_threshold,
        }
    }
}

impl crate::modules::view::IconLabel for BatteryModule {
    fn icon(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Battery(s) if s.is_charging => "⚡".to_owned(),
            crate::modules::ModuleState::Battery(s) => {
                tier_icon(charge_tier(s.percentage, self.critical_threshold)).to_owned()
            }
            _ => String::new(),
        }
    }

    fn label(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Battery(s) => format!("{}%", s.percentage.round() as u8),
            _ => String::new(),
        }
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
        use crate::modules::ModuleState;
        use iced::widget::{container, stack, text};
        use iced::{Alignment, Length};

        let battery = match state {
            ModuleState::Battery(s) => s,
            _ => return iced::widget::Space::new().into(),
        };

        let icon_str = if battery.is_charging {
            "⚡"
        } else {
            tier_icon(charge_tier(battery.percentage, self.critical_threshold))
        };
        let color = icon_color(
            battery.percentage,
            battery.is_charging,
            self.critical_threshold,
        );

        // Battery needs the raw container (not the helper) because it may be stacked
        // with the CriticalBadge overlay below.
        let icon = container(text(icon_str).size(16).color(color))
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
        match state {
            crate::modules::ModuleState::Battery(_) => crate::modules::view::icon_label_view(
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
            crate::modules::ModuleState::Battery(_) => crate::modules::view::icon_label_view(
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
    fn battery_module_id_is_battery() {
        let m = BatteryModule::new(crate::modules::ModuleId::new("battery"), 15);
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
}
