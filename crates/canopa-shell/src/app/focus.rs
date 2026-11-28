use iced::Task;
use iced_layershell::reexport::KeyboardInteractivity;

use super::{App, Message};
use crate::modules::{InteractionEvent, Zone};

impl App {
    pub(super) fn handle_bar_focus_gained(&mut self, window: iced::window::Id) -> Task<Message> {
        let nav = self.effective_nav_list(window);
        let first = nav
            .iter()
            .find(|(z, _)| *z == Zone::Start)
            .or_else(|| nav.iter().find(|(z, _)| *z == Zone::Center))
            .or_else(|| nav.iter().find(|(z, _)| *z == Zone::End))
            .map(|(_, id)| id.clone());
        let s = self.surface_mut(window);
        s.focused_module = first;
        s.module_activated = false;
        Task::none()
    }

    pub(super) fn handle_bar_focus_lost(&mut self, window: iced::window::Id) -> Task<Message> {
        let mut tasks = self.close_tooltip_popup_if_open(window);
        let s = self.surface_mut(window);
        s.focused_module = None;
        s.focus_visible = false;
        s.module_activated = false;
        tasks.push(Task::done(Message::KeyboardInteractivityChange {
            id: window,
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
        }));
        Task::batch(tasks)
    }

    pub(super) fn handle_bar_escape_pressed(&mut self, window: iced::window::Id) -> Task<Message> {
        let close_tt = self.close_tooltip_popup_if_open(window);
        if !close_tt.is_empty() {
            return Task::batch(close_tt);
        }
        let s = self.surface_mut(window);
        if s.module_activated {
            s.module_activated = false;
            return Task::none();
        }
        // The bar may still own keyboard focus after its last visible module
        // disappears. Escape must release the surface even without a target.
        Task::done(Message::KeyboardInteractivityChange {
            id: window,
            keyboard_interactivity: KeyboardInteractivity::None,
        })
    }

    pub(super) fn handle_panel_focus_move(&mut self, forward: bool) -> Task<Message> {
        let active_panel = self.active_external_panel_identity();
        if let Some(effects) = self
            .external_panel_session
            .canopa_focus_move(forward, active_panel.as_ref())
        {
            return self.apply_external_panel_session_effects(effects);
        }
        self.handle_panel_event(if forward {
            super::messages::PanelEvent::FocusMoveNext
        } else {
            super::messages::PanelEvent::FocusMovePrev
        })
    }

    pub(super) fn handle_menu_focus_move(
        &mut self,
        window: iced::window::Id,
        forward: bool,
    ) -> Task<Message> {
        self.popup_nav_step(window, forward);
        Task::none()
    }

    pub(super) fn handle_bar_focus_move(
        &mut self,
        window: iced::window::Id,
        forward: bool,
    ) -> Task<Message> {
        let s = self.surface_mut(window);
        s.focus_visible = true;
        if s.focused_module.is_none() {
            return Task::done(Message::Surface(
                super::messages::SurfaceEvent::BarFocusGained(window),
            ));
        }

        if s.module_activated {
            let interaction = if forward {
                InteractionEvent::ScrollDown
            } else {
                InteractionEvent::ScrollUp
            };
            if let Some(ref id) = s.focused_module.clone() {
                return self.dispatch_module_interaction(id, interaction, window);
            }
            return Task::none();
        }

        if let Some(ref current) = s.focused_module.clone() {
            let next = if forward {
                self.nav_next(current, window)
            } else {
                self.nav_prev(current, window)
            };
            if let Some(next) = next {
                self.surface_mut(window).focused_module = Some(next);
            }
        }
        Task::none()
    }

    pub(super) fn handle_focus_zone(
        &mut self,
        window: iced::window::Id,
        forward: bool,
    ) -> Task<Message> {
        let s = self.surface_mut(window);
        s.focus_visible = true;
        if s.focused_module.is_none() {
            return Task::done(Message::Surface(
                super::messages::SurfaceEvent::BarFocusGained(window),
            ));
        }

        s.module_activated = false;
        if let Some(ref current) = s.focused_module.clone() {
            let next = if forward {
                self.nav_next_zone(current, window)
            } else {
                self.nav_prev_zone(current, window)
            };
            if let Some(next) = next {
                self.surface_mut(window).focused_module = Some(next);
            }
        }
        Task::none()
    }

    pub(super) fn handle_panel_keyboard_activate(&mut self) -> Task<Message> {
        let active_panel = self.active_external_panel_identity();
        if let Some(effects) = self
            .external_panel_session
            .canopa_keyboard_activate(active_panel.as_ref())
        {
            return self.apply_external_panel_session_effects(effects);
        }
        self.handle_panel_event(super::messages::PanelEvent::Activate)
    }

    pub(super) fn handle_menu_keyboard_activate(
        &mut self,
        window: iced::window::Id,
    ) -> Task<Message> {
        let focused = self.foreground.focused_menu_item(window);
        if let Some(item_id) = focused {
            let menu_item_id: i32 = item_id.parse().unwrap_or(0);
            if let Some(model) = self.menu_model(window) {
                let activation = model.activation_for(menu_item_id);
                return Task::done(Message::Menu(super::messages::MenuEvent::ItemSelected {
                    service: activation.service,
                    menu_item_id: activation.item_id,
                    menu_path: activation.menu_path,
                }));
            }
        }
        Task::none()
    }

    pub(super) fn handle_bar_keyboard_activate(
        &mut self,
        window: iced::window::Id,
    ) -> Task<Message> {
        let s = self.surface_mut(window);
        if s.focused_module.is_none() {
            return Task::none();
        }
        if let Some(ref id) = s.focused_module.clone() {
            if self
                .modules
                .action_for(id, &crate::config::ActionTrigger::Click)
                .is_some()
            {
                return self.dispatch_module_interaction(id, InteractionEvent::Clicked, window);
            }
            if self.modules.supports_scroll_interaction(id) {
                self.surface_mut(window).module_activated = true;
                return Task::none();
            }
            if self.modules.supports_primary_activation(id) {
                return self.dispatch_module_interaction(id, InteractionEvent::Clicked, window);
            }
        }
        Task::none()
    }

    pub(super) fn handle_bar_keyboard_secondary_activate(
        &mut self,
        window: iced::window::Id,
    ) -> Task<Message> {
        let s = self.surface_mut(window);
        if s.focused_module.is_none() {
            return Task::none();
        }
        let screen_size = s.screen_size;
        if let Some(ref id) = s.focused_module.clone() {
            if self
                .modules
                .action_for(id, &crate::config::ActionTrigger::RightClick)
                .is_some()
            {
                return self.dispatch_module_interaction(
                    id,
                    InteractionEvent::RightClicked(window),
                    window,
                );
            }
            if self.modules.supports_secondary_activation(id) {
                return self.dispatch_keyboard_secondary_module_interaction(
                    id,
                    window,
                    screen_size,
                );
            }
        }
        Task::none()
    }
}
