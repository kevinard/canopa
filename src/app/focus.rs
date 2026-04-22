use iced::Task;
use iced_layershell::reexport::KeyboardInteractivity;

use super::widget_ops::FindBounds;
use super::{App, Message};
use crate::modules::{InteractionEvent, Zone};

impl App {
    pub(super) fn handle_bar_focus_gained(&mut self, window: iced::window::Id) -> Task<Message> {
        let nav = self.effective_nav_list(window);
        let first = nav
            .iter()
            .find(|(z, _)| *z == Zone::Top)
            .or_else(|| nav.iter().find(|(z, _)| *z == Zone::Center))
            .or_else(|| nav.iter().find(|(z, _)| *z == Zone::Bottom))
            .map(|(_, id)| id.clone());
        let s = self.surface_mut(window);
        s.focused_module = first;
        s.module_activated = false;
        Task::none()
    }

    pub(super) fn handle_bar_focus_lost(&mut self, window: iced::window::Id) -> Task<Message> {
        let s = self.surface_mut(window);
        s.focused_module = None;
        s.module_activated = false;
        Task::done(Message::KeyboardInteractivityChange {
            id: window,
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
        })
    }

    pub(super) fn handle_escape_pressed(&mut self, window: iced::window::Id) -> Task<Message> {
        if self.surfaces.has_popup(window) {
            return Task::done(Message::Menu(super::messages::MenuEvent::Closed));
        }
        let s = self.surface_mut(window);
        if s.module_activated {
            s.module_activated = false;
            return Task::none();
        }
        if s.focused_module.is_none() {
            return Task::none();
        }
        Task::done(Message::KeyboardInteractivityChange {
            id: window,
            keyboard_interactivity: KeyboardInteractivity::None,
        })
    }

    pub(super) fn handle_focus_move(
        &mut self,
        window: iced::window::Id,
        forward: bool,
    ) -> Task<Message> {
        if self.surfaces.has_popup(window) {
            self.popup_nav_step(window, forward);
            return Task::none();
        }

        let s = self.surface_mut(window);
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
                return self.dispatch_module_interaction(id, interaction);
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

    pub(super) fn handle_keyboard_activate(&mut self, window: iced::window::Id) -> Task<Message> {
        if self.surfaces.has_popup(window) {
            let focused = self.menu.focused_item(window).map(str::to_owned);
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
            return Task::none();
        }

        let s = self.surface_mut(window);
        if s.focused_module.is_none() {
            return Task::none();
        }
        if let Some(ref id) = s.focused_module.clone() {
            for (_, module) in &self.modules {
                if module.id() == *id {
                    if module
                        .on_interaction(InteractionEvent::ScrollDown)
                        .is_some()
                    {
                        self.surface_mut(window).module_activated = true;
                        return Task::none();
                    }
                    if let Some(msg) = module.on_interaction(InteractionEvent::Clicked) {
                        return Task::done(msg);
                    }
                    break;
                }
            }
        }
        Task::none()
    }

    pub(super) fn handle_keyboard_secondary_activate(
        &mut self,
        window: iced::window::Id,
    ) -> Task<Message> {
        let s = self.surface_mut(window);
        if s.focused_module.is_none() {
            return Task::none();
        }
        let screen_size = s.screen_size;
        if let Some(ref id) = s.focused_module.clone() {
            for (_, module) in &self.modules {
                if module.id() == *id {
                    if let Some(msg) = module.on_interaction(InteractionEvent::Activated(window)) {
                        if let Message::Tray(
                            super::messages::TrayRuntimeEvent::MenuRequestKeyboard {
                                service,
                                menu_path,
                                source_window,
                            },
                        ) = msg
                        {
                            let target =
                                crate::modules::tray::tray_icon_widget_id(window, &service);
                            let op = FindBounds {
                                target,
                                result: None,
                            };
                            let svc = service.clone();
                            let mp = menu_path.clone();
                            return iced::advanced::widget::operate(op).then(move |bounds| {
                                Task::done(Message::Tray(
                                    super::messages::TrayRuntimeEvent::MenuRequestWithBounds {
                                        service: svc.clone(),
                                        menu_path: mp.clone(),
                                        bounds,
                                        screen_size,
                                        source_window,
                                    },
                                ))
                            });
                        }
                        return Task::done(msg);
                    }
                    break;
                }
            }
        }
        Task::none()
    }
}
