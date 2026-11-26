//! Auto-hide motion of one Bar Surface: where the bar is heading, how far it
//! has slid, and when it should leave. Time is passed in, so the policy is
//! tested without a clock; the app decides what holds a bar on screen.
use std::time::{Duration, Instant};

/// Width of the strip at the screen edge that still takes the pointer while
/// the bar is hidden, in logical pixels.
pub const EDGE_STRIP: i32 = 2;
/// How long the bar stays after the pointer, a panel or a popup left it.
pub const HIDE_DELAY: Duration = Duration::from_millis(500);
/// A full slide in, and a full slide out. Partial slides take their share.
pub const SHOW_DURATION: Duration = Duration::from_millis(170);
pub const HIDE_DURATION: Duration = Duration::from_millis(230);
/// Extra travel past the bar's outer edge, so its shadow leaves the screen too.
pub const SHADOW_ALLOWANCE: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Motion {
    from: f32,
    started: Instant,
    duration: Duration,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AutoHide {
    /// The bar is hidden, or sliding out.
    hidden: bool,
    motion: Option<Motion>,
    /// The pointer is over the bar surface (the bar or its edge strip).
    pub pointer_inside: bool,
    /// A pointer button went down on the bar and has not come up.
    pub pointer_pressed: bool,
    /// When the bar leaves, unless something holds it first.
    pub hide_at: Option<Instant>,
}

impl AutoHide {
    /// A bar that starts out of view.
    pub fn hidden() -> Self {
        Self {
            hidden: true,
            ..Self::default()
        }
    }

    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// How far the bar has slid out at `now`: 0 on screen, 1 out of view.
    pub fn fraction(&self, now: Instant) -> f32 {
        let to = if self.hidden { 1.0 } else { 0.0 };
        let Some(motion) = self.motion else {
            return to;
        };
        let t = (now.saturating_duration_since(motion.started).as_secs_f32()
            / motion.duration.as_secs_f32().max(f32::EPSILON))
        .clamp(0.0, 1.0);
        let eased = if self.hidden {
            // Leaves gently, then speeds away.
            t * t * (3.0 - 2.0 * t)
        } else {
            // Arrives quickly and settles.
            1.0 - (1.0 - t).powi(3)
        };
        motion.from + (to - motion.from) * eased
    }

    /// Send the bar out of view or back. Returns whether the direction changed.
    /// With `animate` false (reduced motion) it jumps.
    pub fn set_hidden(&mut self, hidden: bool, now: Instant, animate: bool) -> bool {
        if self.hidden == hidden {
            return false;
        }
        let from = self.fraction(now);
        self.hidden = hidden;
        self.hide_at = None;
        let full = if hidden { HIDE_DURATION } else { SHOW_DURATION };
        let distance = if hidden { 1.0 - from } else { from };
        self.motion = (animate && distance > 0.0).then(|| Motion {
            from,
            started: now,
            duration: if distance >= 1.0 {
                full
            } else {
                full.mul_f32(distance.max(0.05))
            },
        });
        true
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.motion
            .is_some_and(|motion| now.saturating_duration_since(motion.started) < motion.duration)
    }

    /// Forget a finished slide. Returns whether one just finished.
    pub fn settle(&mut self, now: Instant) -> bool {
        if self.motion.is_some() && !self.animating(now) {
            self.motion = None;
            return true;
        }
        false
    }

    /// Hidden and at rest: only the edge strip takes the pointer.
    pub fn at_rest_hidden(&self, now: Instant) -> bool {
        self.hidden && !self.animating(now)
    }

    /// A slide is under way, or finished and not yet settled.
    pub fn moving(&self) -> bool {
        self.motion.is_some()
    }

    /// Whether something should still happen without new input.
    #[cfg(test)]
    pub fn pending(&self, now: Instant) -> bool {
        self.animating(now) || self.motion.is_some() || self.hide_at.is_some()
    }
}

/// The offset that slides a bar on `edge` out of view by `fraction`, when it
/// needs `travel` logical pixels to leave entirely.
pub fn slide_offset(edge: &crate::config::Edge, travel: f32, fraction: f32) -> iced::Vector {
    let distance = travel * fraction.clamp(0.0, 1.0);
    match edge {
        crate::config::Edge::Left => iced::Vector::new(-distance, 0.0),
        crate::config::Edge::Right => iced::Vector::new(distance, 0.0),
        crate::config::Edge::Top => iced::Vector::new(0.0, -distance),
        crate::config::Edge::Bottom => iced::Vector::new(0.0, distance),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Edge;

    #[test]
    fn slides_out_and_back_with_eased_progress() {
        let start = Instant::now();
        let mut bar = AutoHide::default();
        assert_eq!(bar.fraction(start), 0.0);
        assert!(bar.set_hidden(true, start, true));
        assert!(!bar.set_hidden(true, start, true), "already leaving");
        let mid = bar.fraction(start + HIDE_DURATION / 2);
        assert!(mid > 0.3 && mid < 0.7, "{mid}");
        assert!(bar.animating(start + HIDE_DURATION / 2));
        assert_eq!(bar.fraction(start + HIDE_DURATION), 1.0);
        assert!(bar.settle(start + HIDE_DURATION));
        assert!(bar.at_rest_hidden(start + HIDE_DURATION));
        assert!(!bar.pending(start + HIDE_DURATION));
    }

    #[test]
    fn reversing_mid_slide_starts_where_the_bar_is_and_takes_its_share() {
        let start = Instant::now();
        let mut bar = AutoHide::default();
        bar.set_hidden(true, start, true);
        let turn = start + HIDE_DURATION / 2;
        let at_turn = bar.fraction(turn);
        bar.set_hidden(false, turn, true);
        assert!((bar.fraction(turn) - at_turn).abs() < 1e-4, "no jump");
        // The way back is shorter than a full slide in.
        let back = turn + SHOW_DURATION.mul_f32(at_turn);
        assert_eq!(bar.fraction(back), 0.0);
        assert!(!bar.animating(back));
    }

    #[test]
    fn reduced_motion_jumps_and_scheduled_hides_are_cancelled() {
        let now = Instant::now();
        let mut bar = AutoHide::hidden();
        bar.hide_at = Some(now);
        assert!(bar.set_hidden(false, now, false));
        assert_eq!(bar.fraction(now), 0.0);
        assert!(bar.hide_at.is_none());
        assert!(!bar.pending(now));
    }

    #[test]
    fn offsets_point_toward_the_bar_edge() {
        assert_eq!(
            slide_offset(&Edge::Left, 80.0, 0.5),
            iced::Vector::new(-40.0, 0.0)
        );
        assert_eq!(
            slide_offset(&Edge::Right, 80.0, 1.0),
            iced::Vector::new(80.0, 0.0)
        );
        assert_eq!(
            slide_offset(&Edge::Top, 80.0, 2.0),
            iced::Vector::new(0.0, -80.0)
        );
        assert_eq!(
            slide_offset(&Edge::Bottom, 80.0, 0.0),
            iced::Vector::new(0.0, 0.0)
        );
    }
}
