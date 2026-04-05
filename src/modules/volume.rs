use crate::theme;
use crate::modules::view::{collapsed_icon, expanded_icon_label};

/// Runtime state for the volume module.
#[derive(Debug, Clone, Default)]
pub struct VolumeState {
    /// Current volume percentage 0.0–150.0 (PA can exceed 100%).
    pub volume: f64,
    /// Whether the default sink is muted.
    pub muted: bool,
}

/// Events emitted by the PA subscription.
#[derive(Debug, Clone)]
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

pub struct VolumeModule {
    id: crate::modules::ModuleId,
    step: u8,
}

impl VolumeModule {
    pub fn new(step: u8) -> Self {
        VolumeModule {
            id: crate::modules::ModuleId::new("volume"),
            step,
        }
    }
}

impl crate::modules::Module for VolumeModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none() // fed via Message::Volume from app.rs
    }

    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;

        let vol = match state {
            ModuleState::Volume(s) => s,
            _ => return iced::widget::Space::new().into(),
        };
        let tier = volume_tier(vol.volume, vol.muted);

        collapsed_icon(tier_icon(tier), tier_color(tier))
    }

    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;

        let vol = match state {
            ModuleState::Volume(s) => s,
            _ => return iced::widget::Space::new().into(),
        };
        let tier = volume_tier(vol.volume, vol.muted);
        let pct_label = if vol.muted {
            "muted".to_owned()
        } else {
            format!("{}%", vol.volume.round() as u64)
        };

        expanded_icon_label(tier_icon(tier), tier_color(tier), pct_label)
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
    use crate::modules::Module;

    #[test]
    fn volume_module_id_is_volume() {
        assert_eq!(VolumeModule::new(5).id().0, "volume");
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
}
