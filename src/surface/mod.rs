pub mod expand;
pub mod frame;
pub mod layout;

use crate::surface::expand::ExpandState;

/// Per-output render state. Tracks surface-specific data (expand/collapse, focus).
/// Shared module data lives in SharedState (established in Story 2.1), never here.
/// Multi-surface support (HashMap<SurfaceId, SurfaceState>) deferred to Story 7.1.
#[allow(dead_code)]
pub struct SurfaceState {
    pub output_name: String,
    pub expand: ExpandState,
    // focused_module: Option<ModuleId> — added in Story 6.1
}

impl SurfaceState {
    #[allow(dead_code)]
    pub fn new(output_name: String) -> Self {
        SurfaceState {
            output_name,
            expand: ExpandState::default(), // starts Collapsed
        }
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
        };
        assert!(state.expand.is_expanded());
    }
}
