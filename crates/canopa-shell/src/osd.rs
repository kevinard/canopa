//! The on-screen display's lifetime, without a toolkit.
//!
//! One indicator is visible at a time: a new one replaces it and restarts its
//! timeout, so repeated presses never stack. Key presses show at once.
//! Outside changes (another application, a firmware hotkey) are compared with
//! the last reading of that [`Channel`]: the first reading after start or
//! after the service returns is only a baseline, readings that echo a key
//! press are absorbed, and a burst of changes shows once its value settles.
//! The caller owns the clock and schedules the [`Instant`]s this returns.
use crate::device_keys::{Channel, Indicator};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// How long an indicator stays after the last change.
    pub timeout: Duration,
    /// An outside change shows after it has been quiet this long...
    pub settle: Duration,
    /// ...or after this long, if it keeps changing.
    pub max_settle: Duration,
    /// Readings this soon after a key press on the same channel are its echo.
    pub echo: Duration,
    /// The fade-out; `None` with reduced motion.
    pub fade: Option<Duration>,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(1500),
            settle: Duration::from_millis(120),
            max_settle: Duration::from_millis(400),
            echo: Duration::from_millis(750),
            fade: Some(Duration::from_millis(180)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Visible {
    pub indicator: Indicator,
    /// Identifies this showing; timers for an older one do nothing.
    pub generation: u64,
    pub hide_at: Instant,
    /// When the fade-out started.
    pub fading: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq)]
struct Pending {
    indicator: Indicator,
    first: Instant,
    due: Instant,
}

/// What a timer or reading asks the caller to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    Nothing,
    /// Show (or keep showing) the visible indicator and call
    /// [`Osd::expire`] with this generation at the instant.
    Show {
        generation: u64,
        hide_at: Instant,
    },
    /// Call [`Osd::flush`] at this instant.
    Flush(Instant),
    /// Repaint each frame and call [`Osd::fade`] until it hides.
    Fade,
    /// Close the surface.
    Hide,
}

#[derive(Debug, Default)]
pub struct Osd {
    pub timing: Timing,
    visible: Option<Visible>,
    generation: u64,
    readings: BTreeMap<Channel, Indicator>,
    pressed: BTreeMap<Channel, Instant>,
    pending: Option<Pending>,
}

impl Osd {
    pub fn new(timing: Timing) -> Self {
        Self {
            timing,
            ..Self::default()
        }
    }

    pub fn visible(&self) -> Option<&Visible> {
        self.visible.as_ref()
    }

    /// The indicator's opacity at `now`: 1 unless fading.
    pub fn opacity(&self, now: Instant) -> f32 {
        match (
            self.visible.as_ref().and_then(|v| v.fading),
            self.timing.fade,
        ) {
            (Some(start), Some(fade)) if !fade.is_zero() => {
                1.0 - (now.saturating_duration_since(start).as_secs_f32() / fade.as_secs_f32())
                    .min(1.0)
            }
            _ => 1.0,
        }
    }

    /// The result of a key press: show it now, replacing anything visible.
    pub fn pressed(&mut self, indicator: Indicator, now: Instant) -> Next {
        let channel = indicator.channel();
        self.pressed.insert(channel, now);
        if let Indicator::Level { .. } = indicator {
            self.readings.insert(channel, indicator.clone());
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.indicator.channel() == channel)
        {
            self.pending = None;
        }
        self.show(indicator, now)
    }

    /// A reading from a service: `None` while it is unavailable.
    pub fn observed(&mut self, channel: Channel, reading: Option<Indicator>, now: Instant) -> Next {
        let Some(reading) = reading else {
            // The next reading after a return is a new baseline.
            self.readings.remove(&channel);
            return Next::Nothing;
        };
        let Some(previous) = self.readings.insert(channel, reading.clone()) else {
            return Next::Nothing;
        };
        if previous.significant() == reading.significant() {
            return Next::Nothing;
        }
        let echo = self
            .pressed
            .get(&channel)
            .is_some_and(|at| now.saturating_duration_since(*at) < self.timing.echo);
        if echo {
            // Keep the visible indicator truthful without restarting it.
            if let Some(visible) = &mut self.visible
                && visible.indicator.channel() == channel
                && visible.fading.is_none()
            {
                visible.indicator = reading;
            }
            return Next::Nothing;
        }
        let first = match &self.pending {
            Some(pending) if pending.indicator.channel() == channel => pending.first,
            _ => now,
        };
        let due = (now + self.timing.settle).min(first + self.timing.max_settle);
        self.pending = Some(Pending {
            indicator: reading,
            first,
            due,
        });
        Next::Flush(due)
    }

    /// Show a settled outside change.
    pub fn flush(&mut self, now: Instant) -> Next {
        match &self.pending {
            Some(pending) if now >= pending.due => {
                let indicator = self.pending.take().unwrap().indicator;
                self.show(indicator, now)
            }
            Some(pending) => Next::Flush(pending.due),
            None => Next::Nothing,
        }
    }

    /// The timeout for `generation` elapsed.
    pub fn expire(&mut self, generation: u64, now: Instant) -> Next {
        let fade = self.timing.fade.filter(|fade| !fade.is_zero());
        let Some(visible) = &mut self.visible else {
            return Next::Nothing;
        };
        if visible.generation != generation || visible.fading.is_some() {
            return Next::Nothing;
        }
        if now < visible.hide_at {
            return Next::Show {
                generation,
                hide_at: visible.hide_at,
            };
        }
        if fade.is_some() {
            visible.fading = Some(now);
            Next::Fade
        } else {
            self.visible = None;
            Next::Hide
        }
    }

    /// A frame during the fade-out.
    pub fn fade(&mut self, now: Instant) -> Next {
        match &self.visible {
            Some(Visible {
                fading: Some(_), ..
            }) if self.opacity(now) <= 0.0 => {
                self.visible = None;
                Next::Hide
            }
            Some(Visible {
                fading: Some(_), ..
            }) => Next::Fade,
            _ => Next::Nothing,
        }
    }

    /// Forget the visible indicator, for example when its surface closed.
    pub fn hidden(&mut self) {
        self.visible = None;
    }

    fn show(&mut self, indicator: Indicator, now: Instant) -> Next {
        self.generation += 1;
        let hide_at = now + self.timing.timeout;
        self.visible = Some(Visible {
            indicator,
            generation: self.generation,
            hide_at,
            fading: None,
        });
        Next::Show {
            generation: self.generation,
            hide_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(percent: u16, muted: bool) -> Indicator {
        Indicator::Level {
            channel: Channel::Volume,
            percent,
            muted,
            device: Some("Speakers".into()),
        }
    }

    fn microphone(percent: u16, muted: bool) -> Indicator {
        Indicator::Level {
            channel: Channel::Microphone,
            percent,
            muted,
            device: Some("Microphone".into()),
        }
    }

    fn ms(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn presses_replace_the_visible_indicator_and_restart_its_timeout() {
        let start = Instant::now();
        let mut osd = Osd::new(Timing {
            fade: None,
            ..Timing::default()
        });
        let Next::Show {
            generation: first, ..
        } = osd.pressed(volume(50, false), start)
        else {
            panic!("a press shows")
        };
        let Next::Show {
            generation: second,
            hide_at,
        } = osd.pressed(
            Indicator::Level {
                channel: Channel::Brightness,
                percent: 40,
                muted: false,
                device: None,
            },
            ms(start, 1000),
        )
        else {
            panic!("a second press shows")
        };
        assert_ne!(first, second);
        assert_eq!(hide_at, ms(start, 2500));
        assert_eq!(
            osd.visible().unwrap().indicator.channel(),
            Channel::Brightness
        );
        // The first showing's timer is stale.
        assert_eq!(osd.expire(first, ms(start, 1500)), Next::Nothing);
        assert!(osd.visible().is_some());
        // Early timers ask to wait; the right one hides.
        assert_eq!(
            osd.expire(second, ms(start, 2000)),
            Next::Show {
                generation: second,
                hide_at
            }
        );
        assert_eq!(osd.expire(second, hide_at), Next::Hide);
        assert!(osd.visible().is_none());
    }

    #[test]
    fn fade_out_follows_motion_and_a_press_during_it_restores_the_indicator() {
        let start = Instant::now();
        let mut osd = Osd::default();
        let Next::Show {
            generation,
            hide_at,
        } = osd.pressed(volume(50, false), start)
        else {
            panic!()
        };
        assert_eq!(osd.expire(generation, hide_at), Next::Fade);
        assert!(osd.opacity(hide_at + Duration::from_millis(90)) < 0.6);
        assert_eq!(osd.fade(hide_at + Duration::from_millis(90)), Next::Fade);
        assert!(matches!(
            osd.pressed(volume(55, false), hide_at + Duration::from_millis(100)),
            Next::Show { .. }
        ));
        assert_eq!(osd.opacity(hide_at + Duration::from_millis(100)), 1.0);
        let Next::Show {
            generation,
            hide_at,
        } = osd.pressed(volume(60, false), start)
        else {
            panic!()
        };
        assert_eq!(osd.expire(generation, hide_at), Next::Fade);
        assert_eq!(osd.fade(hide_at + Duration::from_millis(180)), Next::Hide);
        assert!(osd.visible().is_none());
    }

    #[test]
    fn the_first_reading_and_readings_after_an_outage_are_baselines() {
        let start = Instant::now();
        let mut osd = Osd::default();
        assert_eq!(
            osd.observed(Channel::Volume, Some(volume(40, false)), start),
            Next::Nothing
        );
        assert!(osd.visible().is_none(), "nothing shows at startup");
        assert_eq!(
            osd.observed(Channel::Volume, Some(volume(40, false)), ms(start, 10)),
            Next::Nothing
        );
        assert_eq!(
            osd.observed(Channel::Volume, None, ms(start, 20)),
            Next::Nothing
        );
        assert_eq!(
            osd.observed(Channel::Volume, Some(volume(70, false)), ms(start, 30)),
            Next::Nothing,
            "a returning server is not a change"
        );
    }

    #[test]
    fn outside_changes_settle_before_showing_and_bursts_show_once() {
        let start = Instant::now();
        let mut osd = Osd::default();
        osd.observed(Channel::Volume, Some(volume(40, false)), start);
        assert_eq!(
            osd.observed(Channel::Volume, Some(volume(45, false)), ms(start, 1000)),
            Next::Flush(ms(start, 1120))
        );
        // A slider drag keeps changing; the deadline moves but is capped.
        let mut due = ms(start, 1120);
        for (step, at) in [(50, 1050), (55, 1100), (60, 1150), (65, 1300), (70, 1390)] {
            match osd.observed(Channel::Volume, Some(volume(step, false)), ms(start, at)) {
                Next::Flush(next) => due = next,
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(due, ms(start, 1400));
        assert_eq!(osd.flush(ms(start, 1200)), Next::Flush(due));
        assert!(osd.visible().is_none());
        assert!(matches!(osd.flush(due), Next::Show { .. }));
        assert_eq!(osd.visible().unwrap().indicator, volume(70, false));
        assert_eq!(osd.flush(ms(start, 1500)), Next::Nothing);
    }

    #[test]
    fn key_press_echoes_are_absorbed_but_later_outside_changes_show() {
        let start = Instant::now();
        let mut osd = Osd::default();
        osd.observed(Channel::Volume, Some(volume(40, false)), start);
        osd.pressed(volume(45, false), ms(start, 1000));
        // The server reports the press, slightly different after rounding.
        assert_eq!(
            osd.observed(Channel::Volume, Some(volume(44, false)), ms(start, 1050)),
            Next::Nothing
        );
        assert_eq!(osd.visible().unwrap().indicator, volume(44, false));
        assert!(matches!(
            osd.observed(Channel::Volume, Some(volume(20, false)), ms(start, 3000)),
            Next::Flush(_)
        ));
        // Another channel's press does not hide this one's outside change.
        osd.pressed(
            Indicator::Level {
                channel: Channel::Brightness,
                percent: 10,
                muted: false,
                device: None,
            },
            ms(start, 3010),
        );
        assert!(matches!(osd.flush(ms(start, 3200)), Next::Show { .. }));
        assert_eq!(osd.visible().unwrap().indicator, volume(20, false));
    }

    #[test]
    fn microphone_levels_alone_never_show_but_mute_and_device_changes_do() {
        let start = Instant::now();
        let mut osd = Osd::default();
        osd.observed(Channel::Microphone, Some(microphone(40, false)), start);
        assert_eq!(
            osd.observed(
                Channel::Microphone,
                Some(microphone(80, false)),
                ms(start, 10)
            ),
            Next::Nothing,
            "automatic gain control is not a change"
        );
        assert!(matches!(
            osd.observed(
                Channel::Microphone,
                Some(microphone(80, true)),
                ms(start, 20)
            ),
            Next::Flush(_)
        ));
        osd.flush(ms(start, 500));
        assert!(osd.visible().unwrap().indicator.muted());
        let mut headset = volume(40, false);
        if let Indicator::Level { device, .. } = &mut headset {
            *device = Some("Headset".into());
        }
        osd.observed(Channel::Volume, Some(volume(40, false)), ms(start, 600));
        assert!(matches!(
            osd.observed(Channel::Volume, Some(headset), ms(start, 700)),
            Next::Flush(_)
        ));
    }

    #[test]
    fn unavailable_presses_show_briefly_without_becoming_a_baseline() {
        let start = Instant::now();
        let mut osd = Osd::default();
        osd.observed(Channel::Volume, Some(volume(40, false)), start);
        osd.pressed(Indicator::Unavailable(Channel::Brightness), ms(start, 10));
        assert_eq!(
            osd.visible().unwrap().indicator.value(),
            "Unavailable".to_string()
        );
        assert_eq!(
            osd.observed(
                Channel::Brightness,
                Some(Indicator::Level {
                    channel: Channel::Brightness,
                    percent: 30,
                    muted: false,
                    device: None
                }),
                ms(start, 20)
            ),
            Next::Nothing
        );
    }
}
