/// Expand/collapse state for a single bar surface.
///
/// Stored in `SurfaceState`, never in `SharedState` (architecture rule AR6).
/// Progress values are in [0.0, 1.0]; easing is applied at render/resize time.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ExpandState {
    #[default]
    Collapsed,
    /// Animating toward expanded. Progress ∈ [0.0, 1.0]; 280ms cubic-bezier(0.22,1,0.36,1).
    Expanding(f32),
    Expanded,
    /// Animating toward collapsed. Progress ∈ [0.0, 1.0]; 200ms cubic-bezier(0.4,0,1,1).
    Collapsing(f32),
}

impl ExpandState {
    /// Toggle state. Always inverts; mid-animation reverses from current position.
    pub fn toggle(self) -> ExpandState {
        match self {
            ExpandState::Collapsed => ExpandState::Expanding(0.0),
            ExpandState::Expanding(p) => ExpandState::Collapsing(1.0 - p),
            ExpandState::Expanded => ExpandState::Collapsing(0.0),
            ExpandState::Collapsing(p) => ExpandState::Expanding(1.0 - p),
        }
    }

    /// Returns `true` if the bar is fully expanded (not collapsed, not animating).
    /// Symmetric with `is_collapsed`; will be needed when expand-dependent rendering is added.
    #[allow(dead_code)]
    pub fn is_expanded(&self) -> bool {
        matches!(self, ExpandState::Expanded)
    }

    /// Returns `true` while an animation tick is running.
    pub fn is_animating(&self) -> bool {
        matches!(self, ExpandState::Expanding(_) | ExpandState::Collapsing(_))
    }

    /// `true` when expanded content (`view_expanded`) should be rendered.
    ///
    /// During `Expanding`: show expanded content (with opacity fade, UX-DR6).
    /// During `Collapsing`: show collapsed content immediately (no reverse fade).
    pub fn shows_expanded_content(&self) -> bool {
        matches!(self, ExpandState::Expanded | ExpandState::Expanding(_))
    }

    /// Interpolated surface width for the current animation state.
    pub fn current_width(&self, collapsed: u32, expanded: u32) -> u32 {
        match self {
            ExpandState::Collapsed => collapsed,
            ExpandState::Expanded => expanded,
            ExpandState::Expanding(p) => {
                let eased = cubic_bezier(0.22, 1.0, 0.36, 1.0, *p);
                collapsed + ((expanded as f32 - collapsed as f32) * eased) as u32
            }
            ExpandState::Collapsing(p) => {
                let eased = cubic_bezier(0.4, 0.0, 1.0, 1.0, *p);
                expanded.saturating_sub(((expanded as f32 - collapsed as f32) * eased) as u32)
            }
        }
    }

    /// Opacity for expanded content while animating.
    ///
    /// UX-DR6: 60ms delay after expand begins, then fade over remaining 220ms.
    /// Returns 0.0 for `Collapsed` and `Collapsing` (instant switch to collapsed content).
    /// Deferred: `iced::widget::opacity` wrapper unavailable in iced 0.14.0; see deferred-work.md.
    #[allow(dead_code)]
    pub fn label_opacity(&self) -> f32 {
        match self {
            ExpandState::Expanded => 1.0,
            ExpandState::Expanding(p) => {
                let elapsed_ms = p * 280.0;
                ((elapsed_ms - 60.0) / 220.0).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }
}

/// Evaluate a CSS cubic-bezier at time fraction `t` ∈ [0, 1].
///
/// P1=(p1x, p1y), P2=(p2x, p2y) are the inner control points; P0=(0,0), P3=(1,1).
/// Uses Newton-Raphson to solve for parametric `s` where x(s) == t, then returns y(s).
fn cubic_bezier(p1x: f32, p1y: f32, p2x: f32, p2y: f32, t: f32) -> f32 {
    let cx = 3.0 * p1x;
    let bx = 3.0 * (p2x - p1x) - cx;
    let ax = 1.0 - cx - bx;
    let cy = 3.0 * p1y;
    let by = 3.0 * (p2y - p1y) - cy;
    let ay = 1.0 - cy - by;

    let sample_x = |s: f32| ((ax * s + bx) * s + cx) * s;
    let sample_dx = |s: f32| (3.0 * ax * s + 2.0 * bx) * s + cx;
    let sample_y = |s: f32| ((ay * s + by) * s + cy) * s;

    let mut s = t;
    for _ in 0..8 {
        let dx = sample_dx(s);
        if dx.abs() < 1e-6 {
            break;
        }
        s -= (sample_x(s) - t) / dx;
    }
    sample_y(s.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Core invariants ---

    #[test]
    fn expand_state_default_is_collapsed() {
        assert_eq!(ExpandState::default(), ExpandState::Collapsed);
    }

    #[test]
    fn expand_state_is_expanded_true() {
        assert!(ExpandState::Expanded.is_expanded());
    }

    #[test]
    fn expand_state_is_expanded_false() {
        assert!(!ExpandState::Collapsed.is_expanded());
    }

    // --- toggle() ---

    #[test]
    fn toggle_collapsed_starts_expanding() {
        assert_eq!(ExpandState::Collapsed.toggle(), ExpandState::Expanding(0.0));
    }

    #[test]
    fn toggle_expanded_starts_collapsing() {
        assert_eq!(ExpandState::Expanded.toggle(), ExpandState::Collapsing(0.0));
    }

    #[test]
    fn toggle_expanding_reverses_to_collapsing() {
        let result = ExpandState::Expanding(0.3).toggle();
        assert!(
            matches!(result, ExpandState::Collapsing(p) if (p - 0.7).abs() < 1e-5),
            "expected Collapsing(~0.7), got {result:?}"
        );
    }

    #[test]
    fn toggle_collapsing_reverses_to_expanding() {
        let result = ExpandState::Collapsing(0.6).toggle();
        assert!(
            matches!(result, ExpandState::Expanding(p) if (p - 0.4).abs() < 1e-5),
            "expected Expanding(~0.4), got {result:?}"
        );
    }

    // --- is_animating() ---

    #[test]
    fn is_animating_expanding() {
        assert!(ExpandState::Expanding(0.5).is_animating());
    }

    #[test]
    fn is_animating_collapsing() {
        assert!(ExpandState::Collapsing(0.5).is_animating());
    }

    #[test]
    fn is_animating_collapsed_false() {
        assert!(!ExpandState::Collapsed.is_animating());
    }

    #[test]
    fn is_animating_expanded_false() {
        assert!(!ExpandState::Expanded.is_animating());
    }

    // --- shows_expanded_content() ---

    #[test]
    fn shows_expanded_content_expanding() {
        assert!(ExpandState::Expanding(0.0).shows_expanded_content());
    }

    #[test]
    fn shows_expanded_content_expanded() {
        assert!(ExpandState::Expanded.shows_expanded_content());
    }

    #[test]
    fn shows_expanded_content_collapsing_false() {
        assert!(!ExpandState::Collapsing(0.0).shows_expanded_content());
    }

    #[test]
    fn shows_expanded_content_collapsed_false() {
        assert!(!ExpandState::Collapsed.shows_expanded_content());
    }

    // --- current_width() ---

    #[test]
    fn current_width_collapsed() {
        assert_eq!(ExpandState::Collapsed.current_width(48, 200), 48);
    }

    #[test]
    fn current_width_expanded() {
        assert_eq!(ExpandState::Expanded.current_width(48, 200), 200);
    }

    #[test]
    fn current_width_expanding_at_zero_is_collapsed_width() {
        assert_eq!(ExpandState::Expanding(0.0).current_width(48, 200), 48);
    }

    #[test]
    fn current_width_collapsing_at_zero_is_expanded_width() {
        assert_eq!(ExpandState::Collapsing(0.0).current_width(48, 200), 200);
    }

    #[test]
    fn current_width_expanding_mid_is_between() {
        let w = ExpandState::Expanding(0.5).current_width(48, 200);
        assert!(
            w > 48 && w < 200,
            "expected width between 48 and 200, got {w}"
        );
    }

    // --- label_opacity() ---

    #[test]
    fn label_opacity_expanded_is_one() {
        assert_eq!(ExpandState::Expanded.label_opacity(), 1.0);
    }

    #[test]
    fn label_opacity_collapsed_is_zero() {
        assert_eq!(ExpandState::Collapsed.label_opacity(), 0.0);
    }

    #[test]
    fn label_opacity_collapsing_is_zero() {
        assert_eq!(ExpandState::Collapsing(0.5).label_opacity(), 0.0);
    }

    #[test]
    fn label_opacity_expanding_at_zero_progress_is_zero() {
        // No time has elapsed — still within the 60ms delay.
        assert_eq!(ExpandState::Expanding(0.0).label_opacity(), 0.0);
    }

    #[test]
    fn label_opacity_expanding_at_full_progress_is_one() {
        assert_eq!(ExpandState::Expanding(1.0).label_opacity(), 1.0);
    }

    // --- cubic_bezier() ---

    #[test]
    fn cubic_bezier_expand_at_zero() {
        let result = cubic_bezier(0.22, 1.0, 0.36, 1.0, 0.0);
        assert!(result.abs() < 1e-4, "expected ~0.0, got {result}");
    }

    #[test]
    fn cubic_bezier_expand_at_one() {
        let result = cubic_bezier(0.22, 1.0, 0.36, 1.0, 1.0);
        assert!((result - 1.0).abs() < 1e-4, "expected ~1.0, got {result}");
    }

    #[test]
    fn cubic_bezier_collapse_at_zero() {
        let result = cubic_bezier(0.4, 0.0, 1.0, 1.0, 0.0);
        assert!(result.abs() < 1e-4, "expected ~0.0, got {result}");
    }

    #[test]
    fn cubic_bezier_collapse_at_one() {
        let result = cubic_bezier(0.4, 0.0, 1.0, 1.0, 1.0);
        assert!((result - 1.0).abs() < 1e-4, "expected ~1.0, got {result}");
    }
}
