use iced::widget::{Space, column, container, mouse_area, row};
use iced::{Element, Length};

use super::{App, Message};
use crate::config::Edge;
use crate::modules::focused_window;
use crate::modules::{ModuleState, Zone};
use crate::surface::frame::module_frame;
use crate::surface::layout::zone_layout;

impl App {
    pub fn view(&self, window: iced::window::Id) -> Element<'_, Message> {
        // Popup window -- full-screen transparent overlay; menu widget positioned inside.
        if let Some((service, menu_path, tree, bar_name)) = self.menu_popups.get(&window) {
            use crate::surface::menu_popup::view_menu_popup;
            let hovered_item = self.menu_popup_hovered.get(&window).map(|s| s.as_str());
            let focused_item = self.menu_focused_item.get(&window).map(|s| s.as_str());
            let menu = view_menu_popup(
                tree,
                service.as_str(),
                menu_path,
                window,
                hovered_item,
                focused_item,
            );

            let bar = Self::bar_for_surface_name(&self.config.bars, bar_name);
            let bar_size = bar.collapsed_width as f32;

            // Clamp the anchor so the popup stays within screen bounds.
            let (popup_w, popup_h) = crate::surface::menu_popup::popup_size(tree);
            let screen_w = tree.screen_size.0 as f32;
            let screen_h = tree.screen_size.1 as f32;
            let raw_cursor_x = tree.anchor.0 as f32;
            let raw_cursor_y = tree.anchor.1 as f32;
            let cursor_x = if screen_w > 0.0 {
                raw_cursor_x.min(screen_w - popup_w as f32).max(0.0)
            } else {
                raw_cursor_x
            };
            let cursor_y = if screen_h > 0.0 {
                raw_cursor_y.min(screen_h - popup_h as f32).max(0.0)
            } else {
                raw_cursor_y
            };

            let (h_align, v_align, padding) = match &bar.edge {
                Edge::Left => (
                    iced::alignment::Horizontal::Left,
                    iced::alignment::Vertical::Top,
                    iced::Padding {
                        top: cursor_y,
                        left: bar_size,
                        ..Default::default()
                    },
                ),
                Edge::Right => (
                    iced::alignment::Horizontal::Right,
                    iced::alignment::Vertical::Top,
                    iced::Padding {
                        top: cursor_y,
                        right: bar_size,
                        ..Default::default()
                    },
                ),
                Edge::Top => (
                    iced::alignment::Horizontal::Left,
                    iced::alignment::Vertical::Top,
                    iced::Padding {
                        top: bar_size,
                        left: cursor_x,
                        ..Default::default()
                    },
                ),
                Edge::Bottom => (
                    iced::alignment::Horizontal::Left,
                    iced::alignment::Vertical::Bottom,
                    iced::Padding {
                        bottom: bar_size,
                        left: cursor_x,
                        ..Default::default()
                    },
                ),
            };

            // Pin the menu to its computed size so Fill items inside don't expand to
            // screen width, and so the outer Fill container doesn't collapse it.
            let menu_sized = iced::widget::container(menu).width(popup_w).height(popup_h);

            return iced::widget::mouse_area(
                iced::widget::container(menu_sized)
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
                    .align_x(h_align)
                    .align_y(v_align)
                    .padding(padding),
            )
            .on_press(Message::MenuClosed)
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
            let derived: Option<ModuleState> = if module.module_type() == "workspace_item" {
                Some(ModuleState::WorkspaceOutput(surface.output_name.clone()))
            } else if let Some(ModuleState::FocusedWindow(fw)) =
                self.shared.modules.get(&module.id())
            {
                let title = if surface.output_name.is_empty() {
                    fw.title.clone()
                } else {
                    match self.shared.niri.focused_window_id {
                        None => self
                            .shared
                            .niri
                            .active_window_title_for_output(&surface.output_name),
                        Some(_) => {
                            let fo = self.shared.niri.focused_output();
                            if fo.is_none() || fo == Some(surface.output_name.as_str()) {
                                fw.title.clone()
                            } else {
                                self.shared
                                    .niri
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
                None => self
                    .shared
                    .modules
                    .get(&module.id())
                    .unwrap_or(&default_state),
            };

            let hovered = surface.hovered_module.as_ref() == Some(&id);
            let focused = surface.focused_module.as_ref() == Some(&id);

            let module_view: Element<'_, Message> =
                module.view_for_bar(state, window, is_horizontal, shows_expanded);

            let scroll_id = id.clone();
            let view: Element<'_, Message> =
                mouse_area(module_frame(module_view, hovered || focused))
                    .on_enter(Message::ModuleHoverEnter(id.clone(), window))
                    .on_exit(Message::ModuleHoverExit(id.clone(), window))
                    .on_press(Message::ModuleClick(id, window))
                    .on_scroll(move |delta| {
                        let up = match delta {
                            iced::mouse::ScrollDelta::Lines { y, .. }
                            | iced::mouse::ScrollDelta::Pixels { y, .. } => y > 0.0,
                        };
                        if up {
                            Message::ModuleScrollUp(scroll_id.clone(), window)
                        } else {
                            Message::ModuleScrollDown(scroll_id.clone(), window)
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
            .on_press(Message::BarClick(window))
            .on_enter(Message::BarHoverEnter(window))
            .on_exit(Message::BarHoverExit(window))
            .into()
    }

    pub fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: crate::theme::TEXT,
        }
    }
}
