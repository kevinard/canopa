//! Short, purposeful motion for the session surfaces.
//!
//! The surfaces stay event driven: a surface only asks for further frames
//! while one of these effects is running (or while it waits on the system,
//! when a spinner turns). Each effect is a pure function of the time since it
//! started, so previews can render any moment of it.
use std::cell::Cell;
use std::time::{Duration, Instant};

/// How the content first appears.
const ENTRANCE: Duration = Duration::from_millis(520);
/// A new step (chooser, power options, details) replacing the previous one.
const STAGE: Duration = Duration::from_millis(320);
/// The field shaking after a rejected response.
const SHAKE: Duration = Duration::from_millis(560);
/// A spinner shows only when waiting lasts long enough to notice.
const SPINNER_DELAY: Duration = Duration::from_millis(0);
const SPINNER_TURN: f32 = 0.95;

fn ease_out_cubic(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

#[derive(Default)]
pub struct Motion {
    /// Hosts that present continuously enable motion; one-off renders
    /// (previews, tests, accessibility snapshots) show the settled state.
    enabled: Cell<bool>,
    /// A fixed time for previews and tests; otherwise the real clock.
    clock: Cell<Option<Instant>>,
    reduced: Cell<bool>,
    entered: Cell<Option<Instant>>,
    stage: Cell<Option<(u64, Instant)>>,
    failed: Cell<bool>,
    shake: Cell<Option<Instant>>,
    busy: Cell<Option<Instant>>,
}

/// The values a scene applies at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Opacity of the whole surface (entrance).
    pub opacity: f32,
    /// Downward offset of the whole surface, in logical pixels.
    pub rise: f32,
    /// Opacity and offset of the authentication content (a new step).
    pub stage_opacity: f32,
    pub stage_rise: f32,
    /// Horizontal offset of the field and its message.
    pub shake: f32,
    /// Spinner angle in radians, while waiting.
    pub spinner: Option<f32>,
}

impl Motion {
    pub fn now(&self) -> Instant {
        self.clock.get().unwrap_or_else(Instant::now)
    }

    /// Render a fixed moment (previews, recordings and tests).
    pub fn set_clock(&self, now: Option<Instant>) {
        self.clock.set(now);
    }

    /// Animate from now on, starting with the entrance.
    pub fn enable(&self) {
        if !self.enabled.replace(true) {
            self.entered.set(None);
        }
    }

    pub fn set_reduced(&self, reduced: bool) {
        self.reduced.set(reduced);
    }

    /// Restart the entrance, as when a surface is shown again.
    pub fn enter(&self) {
        self.entered.set(Some(self.now()));
    }

    /// Note the presented state; starts effects on its transitions.
    pub fn observe(&self, stage: u64, failed: bool, busy: bool) {
        let now = self.now();
        if self.entered.get().is_none() {
            self.entered.set(Some(now));
        }
        match self.stage.get() {
            Some((previous, _)) if previous == stage => {}
            Some(_) => self.stage.set(Some((stage, now))),
            // The first step arrives with the entrance.
            None => self.stage.set(Some((stage, now - STAGE))),
        }
        if failed && !self.failed.get() {
            self.shake.set(Some(now));
        }
        self.failed.set(failed);
        match (busy, self.busy.get()) {
            (true, None) => self.busy.set(Some(now)),
            (false, Some(_)) => self.busy.set(None),
            _ => {}
        }
    }

    pub fn frame(&self) -> Frame {
        if !self.enabled.get() {
            return Frame {
                opacity: 1.,
                rise: 0.,
                stage_opacity: 1.,
                stage_rise: 0.,
                shake: 0.,
                // A still frame still shows that the system is working.
                spinner: self.busy.get().map(|_| -std::f32::consts::FRAC_PI_4),
            };
        }
        let now = self.now();
        let reduced = self.reduced.get();
        let since = |start: Option<Instant>| start.map(|start| now.saturating_duration_since(start));
        let progress = |start: Option<Instant>, length: Duration| {
            since(start).map_or(1., |elapsed| {
                (elapsed.as_secs_f32() / length.as_secs_f32()).clamp(0., 1.)
            })
        };
        let entrance = ease_out_cubic(progress(self.entered.get(), ENTRANCE));
        let stage = ease_out_cubic(progress(self.stage.get().map(|(_, at)| at), STAGE));
        let shake = progress(self.shake.get(), SHAKE);
        Frame {
            opacity: if reduced { 1. } else { entrance },
            rise: if reduced { 0. } else { (1. - entrance) * 22. },
            // A new step starts partly visible, so a change never blinks.
            stage_opacity: 0.35 + 0.65 * stage,
            stage_rise: if reduced { 0. } else { (1. - stage) * 12. },
            shake: if reduced || shake >= 1. {
                0.
            } else {
                13. * (shake * std::f32::consts::TAU * 3.).sin() * (1. - shake).powi(2)
            },
            spinner: since(self.busy.get())
                .filter(|elapsed| *elapsed >= SPINNER_DELAY)
                .map(|elapsed| elapsed.as_secs_f32() / SPINNER_TURN * std::f32::consts::TAU),
        }
    }

    /// Whether another frame would differ from this one.
    pub fn animating(&self) -> bool {
        if !self.enabled.get() {
            return false;
        }
        let now = self.now();
        let running = |start: Option<Instant>, length: Duration| {
            start.is_some_and(|start| now.saturating_duration_since(start) < length)
        };
        running(self.entered.get(), ENTRANCE)
            || running(self.stage.get().map(|(_, at)| at), STAGE)
            || running(self.shake.get(), SHAKE)
            || self.busy.get().is_some()
    }

    /// A key that changes with every distinct animated frame.
    pub fn revision(&self) -> u64 {
        if self.animating() {
            self.now()
                .saturating_duration_since(self.entered.get().unwrap_or_else(|| self.now()))
                .as_millis() as u64
                | 1 << 63
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_start_on_transitions_and_settle() {
        let motion = Motion::default();
        let start = Instant::now();
        motion.set_clock(Some(start));
        motion.observe(1, false, false);
        assert_eq!(motion.frame().opacity, 1., "disabled motion is settled");
        motion.enable();
        motion.observe(1, false, false);
        let first = motion.frame();
        assert_eq!(first.opacity, 0.);
        assert!(first.rise > 20.);
        assert_eq!(first.stage_opacity, 1., "the first step arrives with the entrance");
        motion.set_clock(Some(start + Duration::from_secs(1)));
        assert!(!motion.animating());
        assert_eq!(motion.frame().opacity, 1.);
        // A failure shakes once; staying failed does not shake again.
        motion.observe(1, true, false);
        motion.set_clock(Some(start + Duration::from_millis(1060)));
        assert!(motion.frame().shake.abs() > 1.);
        motion.observe(1, true, false);
        motion.set_clock(Some(start + Duration::from_secs(2)));
        assert_eq!(motion.frame().shake, 0.);
        // A new step fades in.
        motion.observe(2, true, false);
        assert!(motion.frame().stage_opacity < 0.4);
        // Waiting turns a spinner until the answer arrives.
        motion.observe(2, false, true);
        motion.set_clock(Some(start + Duration::from_millis(2500)));
        assert!(motion.frame().spinner.is_some());
        assert!(motion.animating());
        motion.observe(2, false, false);
        motion.set_clock(Some(start + Duration::from_secs(3)));
        assert!(!motion.animating());
    }

    #[test]
    fn reduced_motion_keeps_state_feedback_without_movement() {
        let motion = Motion::default();
        let start = Instant::now();
        motion.enable();
        motion.set_reduced(true);
        motion.set_clock(Some(start));
        motion.observe(1, false, false);
        motion.observe(1, true, false);
        motion.set_clock(Some(start + Duration::from_millis(100)));
        let frame = motion.frame();
        assert_eq!((frame.opacity, frame.rise, frame.shake), (1., 0., 0.));
    }
}
