pub mod expand;
pub mod frame;
pub mod input;
pub mod layout;

use crate::modules::ModuleId;
use crate::surface::expand::ExpandState;

/// Per-output render state. Tracks surface-specific data (expand/collapse, focus, hover).
/// Shared module data lives in SharedState (established in Story 2.1), never here.
/// Keyed by iced::window::Id in App::surfaces (Story 7.1).
#[derive(Clone)]
pub struct SurfaceState {
    pub output_name: String,
    pub expand: ExpandState,
    pub focused_module: Option<ModuleId>,
    pub hovered_module: Option<ModuleId>,
    pub module_activated: bool,
    pub bar_hovered: bool,
    pub hover_pending_ms: Option<f32>,
}

impl SurfaceState {
    pub fn new(output_name: String) -> Self {
        SurfaceState {
            output_name,
            expand: ExpandState::default(), // starts Collapsed
            focused_module: None,
            hovered_module: None,
            module_activated: false,
            bar_hovered: false,
            hover_pending_ms: None,
        }
    }
}

impl Default for SurfaceState {
    fn default() -> Self {
        SurfaceState::new(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_state_constructs() {
        let state = SurfaceState::new(String::from("test"));
        assert_eq!(state.output_name, "test");
    }

    #[test]
    fn surface_state_new_starts_collapsed() {
        let state = SurfaceState::new("HDMI-1".into());
        assert_eq!(state.expand, ExpandState::Collapsed);
    }

    #[test]
    fn surface_state_expand_field_is_set() {
        let state = SurfaceState {
            output_name: "DP-1".into(),
            expand: ExpandState::Expanded,
            focused_module: None,
            hovered_module: None,
            module_activated: false,
            bar_hovered: false,
            hover_pending_ms: None,
        };
        assert!(state.expand.is_expanded());
    }

    #[test]
    fn surface_state_focused_module_starts_none() {
        let state = SurfaceState::new("DP-1".into());
        assert!(state.focused_module.is_none());
    }

    #[test]
    fn surface_state_default_starts_collapsed_and_unfocused() {
        let s = SurfaceState::default();
        assert_eq!(s.expand, ExpandState::Collapsed);
        assert!(s.focused_module.is_none());
        assert!(s.hovered_module.is_none());
        assert!(!s.module_activated);
        assert!(!s.bar_hovered);
        assert!(s.hover_pending_ms.is_none());
    }

    #[test]
    fn surface_state_new_output_name_stored() {
        let s = SurfaceState::new("eDP-1".into());
        assert_eq!(s.output_name, "eDP-1");
    }
}
