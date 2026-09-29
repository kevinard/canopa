//! Presentation lifetime belongs to Foreground. Retired content is never interactive.
use std::time::{Duration, Instant};

use super::{Foreground, ForegroundEffect};
use crate::{
    config::{Edge, PanelAnimationConfig},
    surface::panel_motion::{Animation, Presentation},
};

pub(super) struct PanelMotion {
    window: iced::window::Id,
    direction: iced::Vector,
    config: PanelAnimationConfig,
    presentation: Presentation,
    scale: Option<f32>,
}

pub(super) struct RetiringPanel {
    pub(super) window: iced::window::Id,
    pub(super) tree: crate::surface::panel::PanelTree,
    pub(super) runtime: crate::app::panel_runtime::PanelRuntime,
    animation: Animation,
}

impl Foreground {
    pub(in crate::app) fn configure_panel_motion(
        &mut self,
        window: iced::window::Id,
        edge: &Edge,
        config: PanelAnimationConfig,
    ) {
        if self.active_panel_window() != Some(window) {
            return;
        }
        self.motion = Some(PanelMotion {
            window,
            direction: match edge {
                Edge::Left => iced::Vector::new(-1.0, 0.0),
                Edge::Right => iced::Vector::new(1.0, 0.0),
                Edge::Top => iced::Vector::new(0.0, -1.0),
                Edge::Bottom => iced::Vector::new(0.0, 1.0),
            },
            presentation: if config.open_ms > 0.0 || config.close_ms > 0.0 {
                Presentation::Preparing
            } else {
                Presentation::Still
            },
            config,
            scale: None,
        });
    }

    pub(in crate::app) fn panel_motion_ready(&mut self, window: iced::window::Id, scale: f32) {
        if self.active_panel_window() != Some(window) {
            return;
        }
        let Some(motion) = &mut self.motion else {
            return;
        };
        if motion.window != window || motion.presentation != Presentation::Preparing {
            return;
        }
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        motion.scale = Some(scale);
        motion.presentation = if motion.config.open_ms > 0.0 {
            Presentation::Animated(Animation {
                started: Instant::now(),
                duration: Duration::from_secs_f32(motion.config.open_ms / 1000.0),
                from: 0.0,
                closing: false,
                direction: motion.direction,
                scale,
            })
        } else {
            Presentation::Still
        };
    }

    pub(in crate::app) fn panel_presentation(&self, window: iced::window::Id) -> Presentation {
        if let Some(retiring) = &self.retiring
            && retiring.window == window
        {
            return Presentation::Animated(retiring.animation);
        }
        self.motion
            .as_ref()
            .filter(|motion| motion.window == window)
            .map_or(Presentation::Still, |motion| motion.presentation)
    }

    pub(in crate::app) fn finish_panel_motion(
        &mut self,
        window: iced::window::Id,
        closing: bool,
    ) -> Vec<ForegroundEffect> {
        if closing {
            if self
                .retiring
                .as_ref()
                .is_some_and(|retiring| retiring.window == window)
            {
                return self.remove_retiring_panel();
            }
        } else if let Some(motion) = &mut self.motion
            && motion.window == window
        {
            motion.presentation = Presentation::Still;
        }
        Vec::new()
    }

    pub(super) fn retire_panel(
        &mut self,
        window: iced::window::Id,
        tree: crate::surface::panel::PanelTree,
    ) -> Option<Vec<ForegroundEffect>> {
        let motion = self.motion.take()?;
        let scale = motion.scale?;
        if motion.window != window || motion.config.close_ms <= 0.0 {
            return None;
        }
        let now = Instant::now();
        let from = match motion.presentation {
            Presentation::Preparing => return None,
            Presentation::Animated(animation) => animation.progress(now),
            Presentation::Still => 1.0,
        };
        let duration = Duration::from_secs_f32(motion.config.close_ms / 1000.0);
        let mut runtime = std::mem::take(&mut self.panel);
        let cleanup = runtime.finish_interaction();
        let mut effects = self.apply_panel_runtime_effects(cleanup);
        self.retiring = Some(RetiringPanel {
            window,
            tree,
            runtime,
            animation: Animation {
                started: now,
                duration,
                from,
                closing: true,
                direction: motion.direction,
                scale,
            },
        });
        effects.push(ForegroundEffect::RetirePanelWindow { window, duration });
        Some(effects)
    }

    pub(super) fn remove_retiring_panel(&mut self) -> Vec<ForegroundEffect> {
        self.retiring
            .take()
            .map(|panel| vec![ForegroundEffect::RemoveWindow(panel.window)])
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::foreground::{ForegroundIntent, PanelForegroundRequest, PanelOpenContext};
    use crate::surface::panel::{PanelContent, PanelOrigin};

    fn open(
        foreground: &mut Foreground,
        origin: iced::window::Id,
        edge: Edge,
        config: PanelAnimationConfig,
    ) -> iced::window::Id {
        foreground.handle(ForegroundIntent::OpenPanel {
            request: PanelForegroundRequest {
                origin_window: origin,
                origin: PanelOrigin::Bar,
                content: PanelContent::BarPanel,
                anchor: None,
                origin_bar_name: "main".into(),
                origin_output_name: "DP-1".into(),
                screen_size: (1280, 800),
            },
            context: PanelOpenContext::empty(),
        });
        let window = iced::window::Id::unique();
        foreground.handle(ForegroundIntent::PanelWindowOpened { window });
        foreground.configure_panel_motion(window, &edge, config);
        foreground.panel_motion_ready(window, 1.5);
        foreground.finish_panel_motion(window, false);
        window
    }

    #[test]
    fn close_ends_interaction_but_keeps_one_bounded_presentation_on_every_edge() {
        for (edge, direction) in [
            (Edge::Left, (-1.0, 0.0)),
            (Edge::Right, (1.0, 0.0)),
            (Edge::Top, (0.0, -1.0)),
            (Edge::Bottom, (0.0, 1.0)),
        ] {
            let mut foreground = Foreground::default();
            let window = open(
                &mut foreground,
                iced::window::Id::unique(),
                edge,
                PanelAnimationConfig::default(),
            );
            let effects = foreground.handle(ForegroundIntent::ClosePanel);
            assert!(!foreground.has_active_panel());
            assert!(foreground.panel_render_snapshot(window).is_some());
            assert!(effects.iter().any(|effect| matches!(effect, ForegroundEffect::RetirePanelWindow { window: id, .. } if *id == window)));
            let Presentation::Animated(animation) = foreground.panel_presentation(window) else {
                panic!("exit animation")
            };
            assert_eq!(
                animation.direction,
                iced::Vector::new(direction.0, direction.1)
            );
            assert_eq!(animation.scale, 1.5);
            assert_eq!(
                foreground.finish_panel_motion(window, true),
                vec![ForegroundEffect::RemoveWindow(window)]
            );
            assert!(foreground.panel_render_snapshot(window).is_none());
            assert!(foreground.finish_panel_motion(window, true).is_empty());
        }
    }

    #[test]
    fn stale_exit_completion_cannot_remove_a_reopened_panel() {
        let mut foreground = Foreground::default();
        let origin = iced::window::Id::unique();
        let old = open(
            &mut foreground,
            origin,
            Edge::Left,
            PanelAnimationConfig::default(),
        );
        foreground.handle(ForegroundIntent::ClosePanel);
        let current = open(
            &mut foreground,
            origin,
            Edge::Left,
            PanelAnimationConfig::default(),
        );
        assert!(foreground.retiring.is_none());
        assert!(foreground.finish_panel_motion(old, true).is_empty());
        foreground.handle(ForegroundIntent::PanelWindowClosed { window: old });
        assert_eq!(foreground.active_panel_window(), Some(current));
    }

    #[test]
    fn zero_duration_and_unprepared_surfaces_close_immediately() {
        let mut foreground = Foreground::default();
        let window = open(
            &mut foreground,
            iced::window::Id::unique(),
            Edge::Left,
            PanelAnimationConfig {
                open_ms: 0.0,
                close_ms: 0.0,
            },
        );
        let effects = foreground.handle(ForegroundIntent::ClosePanel);
        assert!(effects.contains(&ForegroundEffect::RemoveWindow(window)));
        assert!(foreground.retiring.is_none());
        let window = open(
            &mut foreground,
            iced::window::Id::unique(),
            Edge::Left,
            PanelAnimationConfig::default(),
        );
        foreground.configure_panel_motion(window, &Edge::Left, PanelAnimationConfig::default());
        assert_eq!(
            foreground.panel_presentation(window),
            Presentation::Preparing
        );
        let effects = foreground.handle(ForegroundIntent::ClosePanel);
        assert!(effects.contains(&ForegroundEffect::RemoveWindow(window)));
        assert!(foreground.retiring.is_none());
        foreground.panel_motion_ready(window, 2.0);
        assert_eq!(foreground.panel_presentation(window), Presentation::Still);
    }

    #[test]
    fn removing_origin_also_removes_its_retired_presentation() {
        let mut foreground = Foreground::default();
        let origin = iced::window::Id::unique();
        let window = open(
            &mut foreground,
            origin,
            Edge::Left,
            PanelAnimationConfig::default(),
        );
        foreground.handle(ForegroundIntent::ClosePanel);
        let effects = foreground.handle(ForegroundIntent::BarSurfaceRemoved {
            identity: crate::app::surfaces::BarSurfaceIdentity::all_screens(),
            window: origin,
        });
        assert!(effects.contains(&ForegroundEffect::RemoveWindow(window)));
        assert!(foreground.retiring.is_none());
    }
}
