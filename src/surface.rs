pub mod expand;
pub mod frame;
pub mod input;
pub mod layout;

use crate::modules::ModuleId;
use crate::surface::expand::ExpandState;

/// Per-output render state. Tracks surface-specific data (expand/collapse, focus, hover, scale).
/// Shared module data lives in SharedState (established in Story 2.1), never here.
/// Keyed by iced::window::Id in App::surfaces (Story 7.1).
///
/// ## HiDPI model (Story 7.2)
///
/// All dimensions are in **logical pixels**. The compositor multiplies logical pixels by
/// `scale_factor` to produce physical pixels. `iced_layershell` + `wgpu` handle physical
/// rendering automatically — this crate never computes physical pixel values directly.
#[derive(Clone)]
pub struct SurfaceState {
    /// Wayland output name for this surface. Used by Story 7.4 to resolve per-output config overrides.
    /// Set from `pending_output_names` in `App::surface_mut()`. Empty (`""`) in AllScreens mode.
    pub output_name: String,
    /// Scale factor reported by the compositor for this output.
    /// 1.0 for standard displays, 2.0 for 2× HiDPI, 1.5 for fractional HiDPI.
    /// Set when the compositor reports scale via wlr-layer-shell (Story 7.2).
    /// Used for observability and per-output dimension overrides (Story 7.4).
    // Intentionally stored; read by tests and will be used in Stories 7.3/7.4.
    #[allow(dead_code)]
    pub scale_factor: f64,
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
            scale_factor: 1.0, // updated if compositor reports scale > 1
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
            scale_factor: 1.0,
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

    // --- Story 7.2: HiDPI / scale factor tests ---

    #[test]
    fn surface_state_scale_factor_defaults_to_1() {
        let s = SurfaceState::new("eDP-1".into());
        assert_eq!(s.scale_factor, 1.0);
    }

    #[test]
    fn surface_state_default_scale_factor_is_1() {
        let s = SurfaceState::default();
        assert_eq!(s.scale_factor, 1.0);
    }

    #[test]
    fn surface_state_scale_factor_can_be_set_to_2x() {
        let mut s = SurfaceState::new("DP-1".into());
        s.scale_factor = 2.0;
        assert_eq!(s.scale_factor, 2.0);
    }

    #[test]
    fn surface_state_scale_factor_can_be_set_to_fractional() {
        let mut s = SurfaceState::new("HDMI-1".into());
        s.scale_factor = 1.5;
        assert_eq!(s.scale_factor, 1.5);
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

    #[test]
    fn logical_expanded_width_times_2x_scale_gives_physical_width() {
        let logical_expanded: u32 = 200;
        let scale: f64 = 2.0;
        let physical = f64::from(logical_expanded) * scale;
        assert_eq!(physical, 400.0);
    }

    #[test]
    fn surface_state_two_outputs_have_independent_scale_factors() {
        let mut s1 = SurfaceState::new("eDP-1".into());
        let s2 = SurfaceState::new("HDMI-1".into());
        s1.scale_factor = 2.0;
        // s2 is unaffected — scale factors are independent per surface
        assert_eq!(s2.scale_factor, 1.0);
    }

    #[test]
    fn scale_factor_1_means_logical_equals_physical() {
        let s = SurfaceState::new("DP-1".into());
        // At 1.0 scale, logical and physical pixels are identical
        let logical: u32 = 48;
        let physical = f64::from(logical) * s.scale_factor;
        assert_eq!(physical, f64::from(logical));
    }

    #[test]
    fn scale_factor_field_does_not_affect_expand_state() {
        // Modifying scale_factor does not change other per-surface state
        let mut s = SurfaceState::new("eDP-1".into());
        s.scale_factor = 2.0;
        assert_eq!(s.expand, ExpandState::Collapsed);
        assert!(s.focused_module.is_none());
    }
}
