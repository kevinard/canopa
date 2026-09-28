//! A short frame-driven response to input packets, with no added travel.
use iced::{Vector, time::Instant};

#[derive(Default)]
pub(super) struct Motion {
    pending: Vector,
    last_frame: Option<Instant>,
}

impl Motion {
    pub(super) fn pending(&self) -> Vector {
        self.pending
    }

    pub(super) fn is_active(&self) -> bool {
        self.pending != Vector::ZERO
    }

    pub(super) fn push(&mut self, delta: Vector) {
        // Reversals should respond to the fingers immediately, without first
        // finishing movement queued in the previous direction.
        for (pending, input) in [
            (&mut self.pending.x, delta.x),
            (&mut self.pending.y, delta.y),
        ] {
            if *pending * input < 0.0 {
                *pending = 0.0;
            }
            *pending += input;
        }
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn advance(&mut self, now: Instant) -> Vector {
        if !self.is_active() {
            return Vector::ZERO;
        }
        let elapsed = self.last_frame.map_or(1.0 / 60.0, |last| {
            now.saturating_duration_since(last).as_secs_f32()
        });
        self.last_frame = Some(now);
        // An 18 ms response smooths packet boundaries with little finger lag.
        // Use elapsed time so the response does not depend on display refresh.
        let fraction = 1.0 - (-elapsed / 0.018).exp();
        let movement = self.pending * fraction;
        self.pending -= movement;
        if self.pending.x.abs().max(self.pending.y.abs()) < 0.02 {
            let rest = self.pending;
            self.clear();
            movement + rest
        } else {
            movement
        }
    }
}
