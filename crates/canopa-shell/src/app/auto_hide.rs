//! Auto-hiding bars: a bar with `visibility "auto-hide"` reserves no space
//! and waits out of view. Touching its screen edge slides it in over the
//! windows; it leaves shortly after the pointer does, but never while one of
//! its panels, menus, tooltips or hover cards is open, while a button is held
//! on it, or while it has keyboard focus. `canopa-shell --toggle-bar` switches
//! bars between staying on screen and hiding, until the next reload.
use std::time::{Duration, Instant};

use iced::Task;
use iced_layershell::actions::{LayerShellCustomAction, LayerShellCustomActionWithId};

use super::{App, Message};
use crate::config::BarVisibility;
use crate::surface::auto_hide::{AutoHide, HIDE_DELAY, SHADOW_ALLOWANCE};

impl App {
    pub(super) fn bar_auto_hides(&self, window: iced::window::Id) -> bool {
        self.bar_for_window(window).visibility == BarVisibility::AutoHide
    }

    /// A newly opened surface of an auto-hiding bar starts out of view.
    pub(super) fn initial_auto_hide(bar: &crate::config::BarConfig) -> (AutoHide, bool) {
        if bar.visibility == BarVisibility::AutoHide {
            (AutoHide::hidden(), true)
        } else {
            (AutoHide::default(), false)
        }
    }

    /// What keeps the bar on `window` on screen besides the pointer.
    fn auto_hide_held(&self, window: iced::window::Id) -> bool {
        let Some(surface) = self.surface(window) else {
            return false;
        };
        if surface.auto_hide.pointer_inside
            || surface.auto_hide.pointer_pressed
            || surface.focused_module.is_some()
            || surface.drawer_width > 0
        {
            return true;
        }
        if self
            .surfaces
            .tooltip_popups
            .values()
            .any(|popup| popup.target_window == window)
        {
            return true;
        }
        if self.foreground.active_panel_origin_for(window).is_some() {
            return true;
        }
        let identity = self.surfaces.bar_surface_identity_for_window(window);
        self.foreground
            .active_menu_meta()
            .is_some_and(|meta| Some(&meta.origin) == identity.as_ref())
    }

    /// Bring every auto-hiding bar in line with what holds it, after any
    /// message. Starts and finishes slides, schedules departures and keeps the
    /// compositor input region in step.
    pub(super) fn sync_auto_hide(&mut self) -> Task<Message> {
        self.sync_auto_hide_at(Instant::now())
    }

    pub(super) fn sync_auto_hide_at(&mut self, now: Instant) -> Task<Message> {
        let animate = !self.config.appearance.reduced_motion();
        let windows: Vec<_> = self.surfaces.keys().copied().collect();
        let mut tasks = Vec::new();
        for window in windows {
            let auto_hides = self.bar_auto_hides(window);
            let held = auto_hides && self.auto_hide_held(window);
            let surface = self.surfaces.surface_mut(window);
            let state = &mut surface.auto_hide;
            if !auto_hides {
                // A bar switched back to staying slides in and stays.
                state.hide_at = None;
                state.settle(now);
                state.set_hidden(false, now, animate);
            } else {
                state.settle(now);
                if held {
                    state.hide_at = None;
                    state.set_hidden(false, now, animate);
                } else if !state.is_hidden() {
                    match state.hide_at {
                        None => state.hide_at = Some(now + HIDE_DELAY),
                        Some(at) if now >= at => {
                            state.set_hidden(true, now, animate);
                        }
                        Some(_) => {}
                    }
                }
            }
            let strip = auto_hides && surface.auto_hide.at_rest_hidden(now);
            if strip != surface.edge_strip_input {
                surface.edge_strip_input = strip;
                tasks.push(self.drawer_bar_input_region(window));
            }
        }
        Task::batch(tasks)
    }

    pub(super) fn auto_hide_pointer(&mut self, window: iced::window::Id, inside: bool) {
        if let Some(surface) = self.surfaces.get_mut(&window) {
            surface.auto_hide.pointer_inside = inside;
            if !inside {
                // A press that ends outside the surface never reports a release.
                surface.auto_hide.pointer_pressed = false;
            }
        }
    }

    pub(super) fn auto_hide_pressed(&mut self, window: iced::window::Id, pressed: bool) {
        if let Some(surface) = self.surfaces.get_mut(&window) {
            surface.auto_hide.pointer_pressed = pressed;
        }
    }

    /// How far the bar on `window` is drawn from its resting place.
    pub(super) fn bar_slide(&self, window: iced::window::Id) -> iced::Vector {
        let Some(surface) = self.surface(window) else {
            return iced::Vector::ZERO;
        };
        let fraction = surface.auto_hide.fraction(Instant::now());
        if fraction <= 0.0 {
            return iced::Vector::ZERO;
        }
        let bar = self.bar_for_window(window);
        let (near, _, _, _) = bar.outer_margins();
        let travel = (near + bar.collapsed_width) as f32 + SHADOW_ALLOWANCE;
        crate::surface::auto_hide::slide_offset(&bar.edge, travel, fraction)
    }

    /// `canopa-shell --toggle-bar`: bars that stay on screen start hiding
    /// and auto-hiding bars stay. The switch lasts until the configuration is
    /// reloaded.
    pub(super) fn toggle_bar_visibility(&mut self) -> Task<Message> {
        let now = Instant::now();
        for bar in &mut self.config.bars {
            bar.visibility = bar.visibility.toggled();
        }
        let windows: Vec<_> = self.surfaces.keys().copied().collect();
        let mut tasks = Vec::new();
        for window in windows {
            let bar = self.bar_for_window(window);
            let zone = crate::surface::bar_geometry::BarGeometry::reservation(bar);
            let hide_now = bar.visibility == BarVisibility::AutoHide;
            if hide_now {
                // Leave at once rather than after the usual pause.
                self.surfaces.surface_mut(window).auto_hide.hide_at = Some(now);
            }
            tasks.push(Task::done(Message::LayerShell(
                LayerShellCustomActionWithId::new(
                    Some(window),
                    LayerShellCustomAction::ExclusiveZoneChange(zone),
                ),
            )));
        }
        tasks.push(self.sync_auto_hide_at(now));
        Task::batch(tasks)
    }

    /// Frames while a bar slides, and a slower check while one waits to leave.
    pub(super) fn auto_hide_subscription(&self) -> iced::Subscription<Message> {
        let states = || self.surfaces.values().map(|surface| &surface.auto_hide);
        if states().any(AutoHide::moving) {
            iced::time::every(Duration::from_millis(16)).map(|_| Message::BarAutoHideFrame)
        } else if states().any(|state| state.hide_at.is_some()) {
            iced::time::every(Duration::from_millis(50)).map(|_| Message::BarAutoHideFrame)
        } else {
            iced::Subscription::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn app(visibility: BarVisibility) -> (App, iced::window::Id) {
        let mut config = Config::default();
        config.bars[0].visibility = visibility;
        let (mut app, _) = App::boot(config, vec![]);
        let window = iced::window::Id::unique();
        app.surface_mut(window).bar_name = "main".into();
        app.surface_mut(window).screen_size = (76, 800);
        let (state, _) = App::initial_auto_hide(&app.config.bars[0]);
        app.surface_mut(window).auto_hide = state;
        (app, window)
    }

    fn fraction(app: &App, window: iced::window::Id, now: Instant) -> f32 {
        app.surface(window).unwrap().auto_hide.fraction(now)
    }

    #[test]
    fn the_edge_reveals_the_bar_and_it_leaves_after_the_pointer() {
        let (mut app, window) = app(BarVisibility::AutoHide);
        let start = Instant::now();
        let _ = app.sync_auto_hide_at(start);
        assert_eq!(fraction(&app, window, start), 1.0);
        assert!(app.surface(window).unwrap().edge_strip_input);

        app.auto_hide_pointer(window, true);
        let _ = app.sync_auto_hide_at(start);
        assert!(!app.surface(window).unwrap().edge_strip_input);
        let shown = start + Duration::from_secs(1);
        assert_eq!(fraction(&app, window, shown), 0.0);

        app.auto_hide_pointer(window, false);
        let _ = app.sync_auto_hide_at(shown);
        assert_eq!(fraction(&app, window, shown), 0.0, "waits before leaving");
        let _ = app.sync_auto_hide_at(shown + HIDE_DELAY);
        let gone = shown + HIDE_DELAY + Duration::from_secs(1);
        let _ = app.sync_auto_hide_at(gone);
        assert_eq!(fraction(&app, window, gone), 1.0);
        assert!(app.surface(window).unwrap().edge_strip_input);
    }

    #[test]
    fn open_panels_tooltips_and_presses_hold_the_bar() {
        let (mut app, window) = app(BarVisibility::AutoHide);
        let start = Instant::now();
        app.auto_hide_pointer(window, true);
        app.auto_hide_pressed(window, true);
        let _ = app.sync_auto_hide_at(start);
        app.auto_hide_pointer(window, false);
        app.auto_hide_pressed(window, true);
        let later = start + HIDE_DELAY * 4;
        let _ = app.sync_auto_hide_at(later);
        assert_eq!(fraction(&app, window, later), 0.0, "a drag holds it");
        app.auto_hide_pressed(window, false);

        let _ = app.update(Message::Panel(super::super::messages::PanelEvent::Open {
            origin_window: window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));
        let _ = app.sync_auto_hide_at(later + HIDE_DELAY * 4);
        assert!(!app.surface(window).unwrap().auto_hide.is_hidden());

        let _ = app.close_active_panel();
        let after = later + HIDE_DELAY * 5;
        let _ = app.sync_auto_hide_at(after);
        let _ = app.sync_auto_hide_at(after + HIDE_DELAY);
        assert!(app.surface(window).unwrap().auto_hide.is_hidden());
    }

    #[test]
    fn a_panel_opened_by_shortcut_brings_its_hidden_bar_along() {
        let (mut app, window) = app(BarVisibility::AutoHide);
        let _ = app.sync_auto_hide_at(Instant::now());
        assert!(app.surface(window).unwrap().auto_hide.is_hidden());
        let _ = app.update(Message::Panel(super::super::messages::PanelEvent::Open {
            origin_window: window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));
        assert!(!app.surface(window).unwrap().auto_hide.is_hidden());
    }

    #[test]
    fn toggling_switches_between_staying_and_hiding() {
        let (mut app, window) = app(BarVisibility::Always);
        let now = Instant::now();
        let _ = app.sync_auto_hide_at(now);
        assert!(!app.surface(window).unwrap().auto_hide.is_hidden());

        let _ = app.toggle_bar_visibility();
        assert_eq!(app.config.bars[0].visibility, BarVisibility::AutoHide);
        assert!(app.surface(window).unwrap().auto_hide.is_hidden());
        assert_eq!(
            crate::surface::bar_geometry::BarGeometry::reservation(&app.config.bars[0]),
            0
        );

        let _ = app.toggle_bar_visibility();
        assert_eq!(app.config.bars[0].visibility, BarVisibility::Always);
        let later = Instant::now() + Duration::from_secs(1);
        let _ = app.sync_auto_hide_at(later);
        assert_eq!(fraction(&app, window, later), 0.0);
        assert!(!app.surface(window).unwrap().edge_strip_input);
    }

    #[test]
    fn hidden_bars_slide_toward_their_edge() {
        let (app, window) = app(BarVisibility::AutoHide);
        let slide = app.bar_slide(window);
        // The default bar is 48 pixels thick, flush with the edge.
        assert!(slide.x <= -48.0 && slide.y == 0.0, "{slide:?}");
    }
}
