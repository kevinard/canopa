use iced::Task;

use super::{App, Message};
use crate::config::ExpandTrigger;
use crate::surface::expand::ExpandState;

impl App {
    pub(super) fn bar_for_window(&self, window: iced::window::Id) -> &crate::config::BarConfig {
        let bar_name = self
            .surface(window)
            .map(|s| s.bar_name.clone())
            .unwrap_or_default();
        Self::bar_for_surface_name(&self.config.bars, &bar_name)
    }

    pub(super) fn handle_bar_click(&mut self, window: iced::window::Id) -> Task<Message> {
        let close_tt = self.close_tooltip_popup_if_open(window);
        if !close_tt.is_empty() {
            return Task::batch(close_tt);
        }
        if self.surfaces.has_any_popup() {
            return Task::done(Message::Menu(super::messages::MenuEvent::Closed));
        }
        let bar = self.bar_for_window(window);
        if !bar.expand || bar.expand_trigger == ExpandTrigger::Hover {
            return Task::none();
        }
        let s = self.surface_mut(window);
        s.expand = s.expand.toggle();
        self.overlay_size_change_for(window)
    }

    pub(super) fn handle_bar_hover_enter(&mut self, window: iced::window::Id) -> Task<Message> {
        let bar = self.bar_for_window(window);
        let expand_enabled = bar.expand;
        let expand_trigger = bar.expand_trigger;
        let hover_delay = bar.hover_delay;
        let s = self.surface_mut(window);
        s.bar_hovered = true;
        if !expand_enabled || expand_trigger != ExpandTrigger::Hover {
            return Task::none();
        }

        match s.expand {
            ExpandState::Collapsed => {
                if hover_delay == 0 {
                    let s = self.surface_mut(window);
                    s.expand = s.expand.toggle();
                    self.overlay_size_change_for(window)
                } else {
                    self.surface_mut(window).hover_pending_ms = Some(0.0);
                    Task::none()
                }
            }
            ExpandState::Collapsing(_) => {
                let s = self.surface_mut(window);
                s.expand = s.expand.toggle();
                Task::none()
            }
            _ => Task::none(),
        }
    }

    pub(super) fn handle_bar_hover_exit(&mut self, window: iced::window::Id) -> Task<Message> {
        let bar = self.bar_for_window(window);
        let expand_enabled = bar.expand;
        let expand_trigger = bar.expand_trigger;
        let s = self.surface_mut(window);
        s.bar_hovered = false;
        if !expand_enabled || expand_trigger != ExpandTrigger::Hover {
            return Task::none();
        }

        s.hover_pending_ms = None;
        match s.expand {
            ExpandState::Expanded | ExpandState::Expanding(_) => {
                let s = self.surface_mut(window);
                s.expand = s.expand.toggle();
                self.overlay_size_change_for(window)
            }
            _ => Task::none(),
        }
    }

    pub(super) fn handle_tick(&mut self) -> Task<Message> {
        const TICK_MS: f32 = 16.0;

        let window_ids: Vec<_> = self.surfaces.keys().copied().collect();
        let mut tasks = vec![];

        for wid in window_ids {
            let effective_bar = self.bar_for_window(wid);
            let effective_hover_delay = effective_bar.hover_delay;
            let effective_expand_ms = effective_bar.animation.expand_ms;
            let effective_collapse_ms = effective_bar.animation.collapse_ms;
            let effective_tooltip_delay = effective_bar.tooltip_delay_ms;
            let s = self.surfaces.get_mut(&wid).unwrap();

            // Tooltip dwell advances independently of hover-pending and expand.
            if let Some((module, elapsed)) = s.tooltip_pending.clone() {
                let new_elapsed = elapsed + TICK_MS;
                if new_elapsed >= effective_tooltip_delay as f32 {
                    s.tooltip_pending = None;
                    tasks.push(Task::done(Message::Surface(
                        super::messages::SurfaceEvent::TooltipDwellElapsed {
                            window: wid,
                            module,
                        },
                    )));
                } else {
                    s.tooltip_pending = Some((module, new_elapsed));
                }
            }

            let s = self.surfaces.get_mut(&wid).unwrap();
            if let Some(elapsed) = s.hover_pending_ms {
                let new_elapsed = elapsed + TICK_MS;
                if new_elapsed >= effective_hover_delay as f32 {
                    s.hover_pending_ms = None;
                    s.expand = ExpandState::Expanding(0.0);
                    tasks.push(self.overlay_size_change_for(wid));
                } else {
                    s.hover_pending_ms = Some(new_elapsed);
                }
                continue;
            }

            match s.expand {
                ExpandState::Expanding(progress) => {
                    let new_progress = (progress + TICK_MS / effective_expand_ms).min(1.0);
                    s.expand = if new_progress >= 1.0 {
                        ExpandState::Expanded
                    } else {
                        ExpandState::Expanding(new_progress)
                    };
                    tasks.push(self.overlay_size_change_for(wid));
                }
                ExpandState::Collapsing(progress) => {
                    let new_progress = (progress + TICK_MS / effective_collapse_ms).min(1.0);
                    s.expand = if new_progress >= 1.0 {
                        ExpandState::Collapsed
                    } else {
                        ExpandState::Collapsing(new_progress)
                    };
                    tasks.push(self.overlay_size_change_for(wid));
                }
                _ => {}
            }
        }

        Task::batch(tasks)
    }

    pub(super) fn handle_toggle_expand(&mut self) -> Task<Message> {
        let window_ids: Vec<_> = self.surfaces.keys().copied().collect();
        let mut tasks = vec![];
        for wid in window_ids {
            let expand_enabled = self.bar_for_window(wid).expand;
            if expand_enabled {
                let s = self.surfaces.get_mut(&wid).unwrap();
                s.expand = s.expand.toggle();
                tasks.push(self.overlay_size_change_for(wid));
            }
        }
        Task::batch(tasks)
    }
}

#[cfg(test)]
mod tooltip_dwell_tests {
    use super::*;
    use crate::modules::ModuleId;

    fn boot_with_tooltip_delay(delay_ms: u32) -> (App, iced::window::Id) {
        let mut config = crate::config::Config::default();
        config.bars[0].tooltip_delay_ms = delay_ms;
        let (mut app, _task) = App::boot(config, vec![]);
        let wid = iced::window::Id::unique();
        let _ = app.surface_mut(wid);
        (app, wid)
    }

    #[test]
    fn dwell_advances_under_threshold_does_not_fire() {
        let (mut app, wid) = boot_with_tooltip_delay(200);
        let id = ModuleId::new("clock");
        app.surface_mut(wid).tooltip_pending = Some((id.clone(), 0.0));

        for _ in 0..6 {
            let _ = app.handle_tick();
        }
        let s = app.surface(wid).unwrap();
        assert!(matches!(&s.tooltip_pending, Some((m, _)) if m == &id));
    }

    #[test]
    fn dwell_crosses_threshold_clears_pending() {
        let (mut app, wid) = boot_with_tooltip_delay(32);
        let id = ModuleId::new("clock");
        app.surface_mut(wid).tooltip_pending = Some((id, 0.0));

        let _ = app.handle_tick();
        let _ = app.handle_tick();
        assert!(app.surface(wid).unwrap().tooltip_pending.is_none());
    }

    #[test]
    fn hover_exit_during_dwell_cancels() {
        let (mut app, wid) = boot_with_tooltip_delay(500);
        let id = ModuleId::new("clock");
        app.surface_mut(wid).tooltip_pending = Some((id.clone(), 50.0));

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::ModuleHoverExit(
            id, wid,
        ));
        assert!(app.surface(wid).unwrap().tooltip_pending.is_none());
    }

    #[test]
    fn hover_enter_on_other_module_replaces_pending() {
        let (mut app, wid) = boot_with_tooltip_delay(500);
        let a = ModuleId::new("clock");
        let b = ModuleId::new("battery");
        app.surface_mut(wid).tooltip_pending = Some((a, 50.0));

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::ModuleHoverEnter(
            b, wid,
        ));
        assert!(app.surface(wid).unwrap().tooltip_pending.is_none());
    }
}
