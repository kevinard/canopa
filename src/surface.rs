pub mod frame;
pub mod input;
pub mod layout;
pub mod menu_popup;
pub mod panel;
pub mod placement;
pub mod tooltip_popup;

use crate::modules::ModuleId;

/// Per-surface render state. Tracks surface-specific data (focus, hover, popup anchors).
/// Shared module data lives in shared app state (established in Story 2.1), never here.
/// Keyed by iced::window::Id in App::surfaces (Story 7.1).
///
/// Each surface corresponds to one (output_name, bar_name) pair (Story 9.1). Multiple surfaces
/// can exist on the same output when multiple bar sections target it.
///
#[derive(Clone)]
pub struct SurfaceState {
    /// Wayland output name for this surface.
    /// Set from `pending_surface_keys` in `App::surface_mut()`.
    /// Empty (`""`) only for default or legacy fallback state.
    pub output_name: String,
    /// Name of the bar section this surface renders (Story 9.1).
    /// Matches `BarConfig::name`. Empty (`""`) falls back to the first bar.
    pub bar_name: String,
    pub focused_module: Option<ModuleId>,
    pub hovered_module: Option<ModuleId>,
    pub module_activated: bool,
    /// Module currently being dwelled on for tooltip purposes plus the
    /// elapsed milliseconds since hover-enter. None means no pending dwell.
    pub tooltip_pending: Option<(crate::modules::ModuleId, f32)>,
    /// Last cursor position observed inside this surface, in logical pixels
    /// relative to the surface origin. Used as the tooltip popup anchor.
    pub last_pointer_position: (i32, i32),
    /// Logical size (width, height) of this surface as last reported by the compositor.
    /// Used to clamp tray menu popup positions near screen edges.
    pub screen_size: (u32, u32),
}

impl SurfaceState {
    pub fn new(output_name: String, bar_name: String) -> Self {
        SurfaceState {
            output_name,
            bar_name,
            focused_module: None,
            hovered_module: None,
            module_activated: false,
            tooltip_pending: None,
            last_pointer_position: (0, 0),
            screen_size: (0, 0),
        }
    }
}

impl Default for SurfaceState {
    fn default() -> Self {
        SurfaceState::new(String::new(), String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_state_constructs() {
        let state = SurfaceState::new(String::from("test"), String::from("main"));
        assert_eq!(state.output_name, "test");
    }

    #[test]
    fn surface_state_focused_module_starts_none() {
        let state = SurfaceState::new("DP-1".into(), String::new());
        assert!(state.focused_module.is_none());
    }

    #[test]
    fn surface_state_default_starts_unfocused() {
        let s = SurfaceState::default();
        assert!(s.focused_module.is_none());
        assert!(s.hovered_module.is_none());
        assert!(!s.module_activated);
        assert!(s.tooltip_pending.is_none());
    }

    #[test]
    fn surface_state_new_output_name_stored() {
        let s = SurfaceState::new("eDP-1".into(), String::new());
        assert_eq!(s.output_name, "eDP-1");
    }

    #[test]
    fn surface_state_bar_name_stored() {
        let s = SurfaceState::new("eDP-1".into(), "top-strip".into());
        assert_eq!(s.bar_name, "top-strip");
    }

    #[test]
    fn surface_state_default_bar_name_is_empty() {
        let s = SurfaceState::default();
        assert_eq!(s.bar_name, "");
    }

    #[test]
    fn surface_state_new_stores_both_output_and_bar_name() {
        let s = SurfaceState::new("DP-1".into(), "secondary".into());
        assert_eq!(s.output_name, "DP-1");
        assert_eq!(s.bar_name, "secondary");
    }

    #[test]
    fn surface_state_bar_name_independent_of_output_name() {
        let mut s1 = SurfaceState::new("eDP-1".into(), "main".into());
        let s2 = SurfaceState::new("DP-1".into(), "side".into());
        s1.bar_name = "updated".into();
        // s2 must be unaffected
        assert_eq!(s2.bar_name, "side");
    }

    #[test]
    fn surface_state_clone_preserves_bar_name() {
        let s = SurfaceState::new("eDP-1".into(), "main".into());
        let cloned = s.clone();
        assert_eq!(cloned.bar_name, "main");
        assert_eq!(cloned.output_name, "eDP-1");
    }

    #[test]
    fn logical_width_times_2x_scale_gives_physical_width() {
        // Documents the HiDPI model: physical = logical × scale
        let logical_collapsed: u32 = 48;
        let scale: f64 = 2.0;
        let physical = f64::from(logical_collapsed) * scale;
        assert_eq!(physical, 96.0);
    }

    #[test]
    fn logical_width_times_fractional_scale_gives_physical_width() {
        // 48 logical × 1.5 scale = 72 physical pixels
        let logical_collapsed: u32 = 48;
        let scale: f64 = 1.5;
        let physical = f64::from(logical_collapsed) * scale;
        assert_eq!(physical, 72.0);
    }
}
