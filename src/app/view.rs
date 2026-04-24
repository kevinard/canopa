use iced::widget::{Space, column, container, mouse_area, row};
use iced::{Element, Length};

use super::{App, Message};
use crate::config::Edge;
use crate::modules::focused_window;
use crate::modules::{ModuleKind, ModuleState, Zone};
use crate::surface::frame::module_frame;
use crate::surface::layout::zone_layout;

impl App {
    pub(crate) fn view(&self, window: iced::window::Id) -> Element<'_, Message> {
        if let Some(perf) = &self.perf {
            perf.record_view(window);
        }

        // Popup window -- full-screen transparent overlay; menu widget positioned inside.
        if let Some(model) = self.popup_render_model(window) {
            use crate::surface::menu_popup::view_menu_popup;
            let hovered_item = self.menu.hovered_item(window);
            let focused_item = self.menu.focused_item(window);
            let menu = view_menu_popup(
                model.tree,
                model.service,
                model.menu_path,
                window,
                hovered_item,
                focused_item,
            );

            // Pin the menu to its computed size so Fill items inside don't expand to
            // screen width, and so the outer Fill container doesn't collapse it.
            let menu_sized = iced::widget::container(menu)
                .width(model.placement.popup_width)
                .height(model.placement.popup_height);

            return iced::widget::mouse_area(
                iced::widget::container(menu_sized)
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
                    .align_x(model.placement.h_align)
                    .align_y(model.placement.v_align)
                    .padding(model.placement.padding),
            )
            .on_press(Message::Menu(super::messages::MenuEvent::Closed))
            .into();
        }

        let surface = self.surface(window).cloned().unwrap_or_default();
        let bar = Self::bar_for_surface_name(&self.config.bars, &surface.bar_name);
        let is_horizontal = bar.edge.is_horizontal();

        let surface_size = surface
            .expand
            .current_width(bar.collapsed_width, bar.expanded_width);
        let content_size = surface_size.saturating_sub(1);
        let shows_expanded = surface.expand.shows_expanded_content();

        let mut top: Vec<Element<'_, Message>> = vec![];
        let mut center: Vec<Element<'_, Message>> = vec![];
        let mut bottom: Vec<Element<'_, Message>> = vec![];

        let effective_zone = self.effective_module_zones_for_bar(bar);
        let default_state = ModuleState::default();

        for (_boot_zone, module) in &self.modules {
            let id = module.id();
            let Some(zone) = effective_zone.get(&id) else {
                continue;
            };

            // Compute a derived state only when module logic requires a runtime-calculated value
            // (workspace_item, FocusedWindow title derivation). All other modules borrow directly
            // from the shared map so no clone is needed.
            let derived: Option<ModuleState> = if module.kind() == ModuleKind::WorkspaceItem {
                Some(ModuleState::WorkspaceOutput(surface.output_name.clone()))
            } else if let Some(ModuleState::FocusedWindow(fw)) = self.modules.state(&module.id()) {
                let title = if surface.output_name.is_empty() {
                    fw.title.clone()
                } else {
                    match self.niri.state().focused_window_id {
                        None => self
                            .niri
                            .state()
                            .active_window_title_for_output(&surface.output_name),
                        Some(_) => {
                            let fo = self.niri.state().focused_output();
                            if fo.is_none() || fo == Some(surface.output_name.as_str()) {
                                fw.title.clone()
                            } else {
                                self.niri
                                    .state()
                                    .active_window_title_for_output(&surface.output_name)
                            }
                        }
                    }
                };
                Some(ModuleState::FocusedWindow(
                    focused_window::FocusedWindowState { title },
                ))
            } else {
                None
            };

            let state: &ModuleState = match &derived {
                Some(s) => s,
                None => self.modules.state(&module.id()).unwrap_or(&default_state),
            };

            let hovered = surface.hovered_module.as_ref() == Some(&id);
            let focused = surface.focused_module.as_ref() == Some(&id);

            let module_view: Element<'_, Message> =
                module.view_for_bar(state, window, is_horizontal, shows_expanded);

            let scroll_id = id.clone();
            let view: Element<'_, Message> =
                mouse_area(module_frame(module_view, hovered || focused))
                    .on_enter(Message::Surface(
                        super::messages::SurfaceEvent::ModuleHoverEnter(id.clone(), window),
                    ))
                    .on_exit(Message::Surface(
                        super::messages::SurfaceEvent::ModuleHoverExit(id.clone(), window),
                    ))
                    .on_press(Message::Modules(
                        super::messages::ModuleRuntimeEvent::ModuleClick(id, window),
                    ))
                    .on_scroll(move |delta| {
                        let up = match delta {
                            iced::mouse::ScrollDelta::Lines { y, .. }
                            | iced::mouse::ScrollDelta::Pixels { y, .. } => y > 0.0,
                        };
                        if up {
                            Message::Modules(super::messages::ModuleRuntimeEvent::ModuleScrollUp(
                                scroll_id.clone(),
                                window,
                            ))
                        } else {
                            Message::Modules(super::messages::ModuleRuntimeEvent::ModuleScrollDown(
                                scroll_id.clone(),
                                window,
                            ))
                        }
                    })
                    .into();

            match zone {
                Zone::Top => top.push(view),
                Zone::Center => center.push(view),
                Zone::Bottom => bottom.push(view),
            }
        }

        let border_style = |_theme: &_| container::Style {
            background: Some(iced::Background::Color(crate::theme::SURFACE1)),
            ..Default::default()
        };

        let (content_w, content_h, border_w, border_h) = if is_horizontal {
            (
                Length::Fill,
                Length::from(content_size),
                Length::Fill,
                Length::from(1),
            )
        } else {
            (
                Length::from(content_size),
                Length::Fill,
                Length::from(1),
                Length::Fill,
            )
        };

        let content = container(zone_layout(top, center, bottom, is_horizontal))
            .width(content_w)
            .height(content_h);
        let border_strip = container(Space::new())
            .width(border_w)
            .height(border_h)
            .style(border_style);

        let bar_content: Element<'_, Message> = match bar.edge {
            Edge::Left => row![content, border_strip]
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            Edge::Right => row![border_strip, content]
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            Edge::Top => column![content, border_strip]
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            Edge::Bottom => column![border_strip, content]
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
        };

        let bar_with_bg = iced::widget::container(bar_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(iced::Background::Color(crate::theme::BASE)),
                ..Default::default()
            });

        mouse_area(bar_with_bg)
            .on_press(Message::Surface(super::messages::SurfaceEvent::BarClick(
                window,
            )))
            .on_enter(Message::Surface(
                super::messages::SurfaceEvent::BarHoverEnter(window),
            ))
            .on_exit(Message::Surface(
                super::messages::SurfaceEvent::BarHoverExit(window),
            ))
            .into()
    }

    pub(crate) fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: crate::theme::TEXT,
        }
    }
}
