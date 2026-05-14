use iced::Task;

use super::{App, Message};

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
        Task::none()
    }

    pub(super) fn handle_bar_hover_enter(&mut self, window: iced::window::Id) -> Task<Message> {
        self.surface_mut(window);
        Task::none()
    }

    pub(super) fn handle_bar_hover_exit(&mut self, window: iced::window::Id) -> Task<Message> {
        self.surface_mut(window);
        Task::none()
    }

    pub(super) fn handle_tick(&mut self) -> Task<Message> {
        const TICK_MS: f32 = 16.0;

        let window_ids: Vec<_> = self.surfaces.keys().copied().collect();
        let mut tasks = vec![];

        for wid in window_ids {
            let effective_bar = self.bar_for_window(wid);
            let effective_tooltip_delay = effective_bar.tooltip_delay_ms;
            let s = self.surfaces.get_mut(&wid).unwrap();

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
