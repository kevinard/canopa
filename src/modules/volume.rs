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

pub struct VolumeModule {
    id: crate::modules::ModuleId,
    step: u8,
}

impl VolumeModule {
    pub fn new(id: crate::modules::ModuleId, step: u8) -> Self {
        VolumeModule { id, step }
    }
}

impl crate::modules::view::IconLabel for VolumeModule {
    fn icon(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Volume(s) => {
                tier_icon(volume_tier(s.volume, s.muted)).to_owned()
            }
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
    use crate::modules::Module;

    #[test]
    fn volume_module_id_is_volume() {
        let m = VolumeModule::new(crate::modules::ModuleId::new("volume"), 5);
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
}
