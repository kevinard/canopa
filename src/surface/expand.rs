/// Expand/collapse state for a single bar surface.
///
/// Stored in `SurfaceState`, never in `SharedState` (architecture rule AR6).
/// Story 3.2 adds `Animating(f32)` and the tick-driven animation loop.
/// Story 3.1 delivers instant toggle only: `Collapsed ↔ Expanded`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ExpandState {
    #[default]
    Collapsed,
    Expanded,
    // Animating(f32) — added in Story 3.2; progress in [0.0, 1.0]
}

impl ExpandState {
    /// Toggle between `Collapsed` and `Expanded`.
    ///
    /// Story 3.2 will replace this with an `Animating(...)` transition instead of instant switch.
    pub fn toggle(self) -> ExpandState {
        match self {
            ExpandState::Collapsed => ExpandState::Expanded,
            ExpandState::Expanded => ExpandState::Collapsed,
        }
    }

    /// Returns `true` if the bar is fully expanded (not collapsed, not animating).
    pub fn is_expanded(&self) -> bool {
        matches!(self, ExpandState::Expanded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_state_default_is_collapsed() {
        assert_eq!(ExpandState::default(), ExpandState::Collapsed);
    }

    #[test]
    fn expand_state_toggle_collapsed_to_expanded() {
        assert_eq!(ExpandState::Collapsed.toggle(), ExpandState::Expanded);
    }

    #[test]
    fn expand_state_toggle_expanded_to_collapsed() {
        assert_eq!(ExpandState::Expanded.toggle(), ExpandState::Collapsed);
    }

    #[test]
    fn expand_state_is_expanded_true() {
        assert!(ExpandState::Expanded.is_expanded());
    }

    #[test]
    fn expand_state_is_expanded_false() {
        assert!(!ExpandState::Collapsed.is_expanded());
    }
}
