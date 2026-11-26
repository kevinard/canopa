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
        if self.bar_has_action(window, &crate::config::ActionTrigger::Click) {
            return self.dispatch_bar_action(window, crate::config::ActionTrigger::Click);
        }
        if self.foreground.active_menu_window().is_some() {
            return Task::done(Message::Menu(super::messages::MenuEvent::Closed));
        }
        Task::none()
    }

    pub(super) fn handle_bar_right_click(&mut self, window: iced::window::Id) -> Task<Message> {
        let close_tt = self.close_tooltip_popup_if_open(window);
        if !close_tt.is_empty() {
            return Task::batch(close_tt);
        }
        if self.bar_has_action(window, &crate::config::ActionTrigger::RightClick) {
            return self.dispatch_bar_action(window, crate::config::ActionTrigger::RightClick);
        }
        if self.foreground.active_menu_window().is_some() {
            return Task::done(Message::Menu(super::messages::MenuEvent::Closed));
        }
        Task::none()
    }

    pub(super) fn handle_bar_action_key(
        &mut self,
        window: iced::window::Id,
        modifiers: iced::keyboard::Modifiers,
        key: iced::keyboard::Key,
    ) -> Task<Message> {
        let Some(trigger) = self.bar_action_key_trigger(window, modifiers, &key) else {
            return Task::none();
        };
        self.dispatch_bar_action(window, trigger)
    }

    fn dispatch_bar_action(
        &mut self,
        window: iced::window::Id,
        trigger: crate::config::ActionTrigger,
    ) -> Task<Message> {
        if let Some(action) = self
            .bar_for_window(window)
            .actions
            .action_for(&trigger)
            .cloned()
        {
            return self.dispatch_panel_action(
                window,
                crate::surface::panel::PanelOrigin::Bar,
                action,
            );
        }
        Task::none()
    }

    fn bar_has_action(
        &self,
        window: iced::window::Id,
        trigger: &crate::config::ActionTrigger,
    ) -> bool {
        self.bar_for_window(window)
            .actions
            .action_for(trigger)
            .is_some()
    }

    fn bar_action_key_trigger(
        &self,
        window: iced::window::Id,
        modifiers: iced::keyboard::Modifiers,
        key: &iced::keyboard::Key,
    ) -> Option<crate::config::ActionTrigger> {
        self.bar_for_window(window)
            .actions
            .bindings
            .iter()
            .filter_map(|binding| match &binding.trigger {
                crate::config::ActionTrigger::Key(keybind) => Some(keybind),
                _ => None,
            })
            .find(|keybind| {
                crate::surface::input::parse_keybind(keybind)
                    .map(|(configured_modifiers, configured_key)| {
                        configured_modifiers == modifiers && action_keys_match(&configured_key, key)
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .map(crate::config::ActionTrigger::Key)
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
        let mut tasks = vec![self.hover_card_tick()];

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

fn action_keys_match(configured: &iced::keyboard::Key, actual: &iced::keyboard::Key) -> bool {
    match (configured, actual) {
        (iced::keyboard::Key::Character(configured), iced::keyboard::Key::Character(actual)) => {
            configured.eq_ignore_ascii_case(actual)
        }
        _ => configured == actual,
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

    #[test]
    fn bar_click_with_configured_open_panel_opens_bar_panel() {
        let mut config = crate::config::Config::default();
        config.bars[0]
            .actions
            .bindings
            .push(crate::config::ActionBinding {
                trigger: crate::config::ActionTrigger::Click,
                action: crate::config::PanelAction::OpenPanel,
            });
        let (mut app, _) = App::boot(config, vec![]);
        let wid = iced::window::Id::unique();
        app.surface_mut(wid).bar_name = "main".into();

        let _ = app.handle_bar_click(wid);

        let panel = app.foreground.active_panel_tree().expect("active panel");
        assert_eq!(panel.origin, crate::surface::panel::PanelOrigin::Bar);
        assert_eq!(panel.content, crate::surface::panel::PanelContent::BarPanel);
        assert_eq!(panel.origin_window, wid);
    }

    #[test]
    fn bar_click_with_configured_toggle_panel_closes_open_bar_panel() {
        let mut config = crate::config::Config::default();
        config.bars[0]
            .actions
            .bindings
            .push(crate::config::ActionBinding {
                trigger: crate::config::ActionTrigger::Click,
                action: crate::config::PanelAction::TogglePanel,
            });
        let (mut app, _) = App::boot(config, vec![]);
        let wid = iced::window::Id::unique();
        app.surface_mut(wid).bar_name = "main".into();

        let _ = app.handle_bar_click(wid);
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );

        let _ = app.handle_bar_click(wid);

        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            0
        );
    }

    #[test]
    fn bar_right_click_with_configured_open_panel_opens_bar_panel() {
        let mut config = crate::config::Config::default();
        config.bars[0]
            .actions
            .bindings
            .push(crate::config::ActionBinding {
                trigger: crate::config::ActionTrigger::RightClick,
                action: crate::config::PanelAction::OpenPanel,
            });
        let (mut app, _) = App::boot(config, vec![]);
        let wid = iced::window::Id::unique();
        app.surface_mut(wid).bar_name = "main".into();

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::BarRightClick(wid));

        let panel = app.foreground.active_panel_tree().expect("active panel");
        assert_eq!(panel.origin, crate::surface::panel::PanelOrigin::Bar);
        assert_eq!(panel.content, crate::surface::panel::PanelContent::BarPanel);
        assert_eq!(panel.origin_window, wid);
    }

    #[test]
    fn bar_keybind_with_configured_open_panel_opens_bar_panel() {
        let mut config = crate::config::Config::default();
        config.bars[0]
            .actions
            .bindings
            .push(crate::config::ActionBinding {
                trigger: crate::config::ActionTrigger::Key("Ctrl+E".into()),
                action: crate::config::PanelAction::OpenPanel,
            });
        let (mut app, _) = App::boot(config, vec![]);
        let wid = iced::window::Id::unique();
        app.surface_mut(wid).bar_name = "main".into();

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::ActionKeyPressed {
            window: wid,
            modifiers: iced::keyboard::Modifiers::CTRL,
            key: iced::keyboard::Key::Character("e".into()),
        });

        let panel = app.foreground.active_panel_tree().expect("active panel");
        assert_eq!(panel.origin, crate::surface::panel::PanelOrigin::Bar);
        assert_eq!(panel.content, crate::surface::panel::PanelContent::BarPanel);
        assert_eq!(panel.origin_window, wid);
    }
}
