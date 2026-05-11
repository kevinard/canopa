use iced::widget::{Space, column, container, mouse_area, row};
use iced::{Element, Length};

use super::render::{BarRenderModel, PopupRenderModel};
use super::{App, Message};
use crate::config::Edge;
use crate::surface::layout::zone_layout;

impl App {
    pub(crate) fn view(&self, window: iced::window::Id) -> Element<'_, Message> {
        if let Some(perf) = &self.perf {
            perf.record_view(window);
        }

        // Popup window -- full-screen transparent overlay; menu widget positioned inside.
        if let Some(model) = self.popup_render_model(window) {
            let hovered_item = self.menu.hovered_item(window);
            let focused_item = self.menu.focused_item(window);
            return render_popup_model(model, window, hovered_item, focused_item);
        }

        if let Some(popup) = self.surfaces.tooltip_popup(window) {
            return render_tooltip_popup(&popup.tree);
        }

        if self.surfaces.panel_popup(window).is_some() {
            return render_panel_popup();
        }

        render_bar_model(self.bar_render_model(window), window)
    }

    pub(crate) fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: crate::theme::TEXT,
        }
    }
}

fn render_panel_popup<'a>() -> Element<'a, Message> {
    iced::widget::container(iced::widget::text("panel"))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn render_tooltip_popup<'a>(
    tree: &'a crate::surface::tooltip_popup::TooltipTree,
) -> Element<'a, Message> {
    crate::surface::tooltip_popup::view_tooltip::<Message>(tree)
}

fn render_popup_model<'a>(
    model: PopupRenderModel<'a>,
    window: iced::window::Id,
    hovered_item: Option<&'a str>,
    focused_item: Option<&'a str>,
) -> Element<'a, Message> {
    use crate::surface::menu_popup::view_menu_popup;

    let menu = view_menu_popup(
        model.tree,
        model.service,
        model.menu_path,
        window,
        hovered_item,
        focused_item,
    );

    iced::widget::mouse_area(menu)
        .on_press(Message::Menu(super::messages::MenuEvent::Closed))
        .into()
}

fn render_bar_model<'a>(
    model: BarRenderModel<'a>,
    window: iced::window::Id,
) -> Element<'a, Message> {
    let is_horizontal = model.is_horizontal;
    let content_size = model.content_size;
    let start: Vec<Element<'_, Message>> =
        model.start.into_iter().map(|entry| entry.element).collect();
    let center: Vec<Element<'_, Message>> = model
        .center
        .into_iter()
        .map(|entry| entry.element)
        .collect();
    let end: Vec<Element<'_, Message>> = model.end.into_iter().map(|entry| entry.element).collect();

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

    let content = container(zone_layout(start, center, end, is_horizontal))
        .width(content_w)
        .height(content_h);
    let border_strip = container(Space::new())
        .width(border_w)
        .height(border_h)
        .style(border_style);

    let bar_content: Element<'_, Message> = match model.edge {
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

#[cfg(test)]
mod tests {
    use super::App;
    use crate::config::Config;
    use crate::surface::menu_popup::{MenuItem, MenuTree};

    #[test]
    fn default_surface_view_renders_without_panic() {
        let (app, _) = App::boot(Config::default(), vec![]);
        let _ = app.view(iced::window::Id::unique());
    }

    #[test]
    fn popup_surface_view_renders_without_panic() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let popup = iced::window::Id::unique();
        app.surfaces.insert_popup(
            popup,
            super::super::surfaces::PopupMeta {
                service: "org.test".into(),
                menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
                tree: MenuTree {
                    items: vec![MenuItem {
                        id: "1".into(),
                        label: "Open".into(),
                        enabled: true,
                        visible: true,
                        icon_name: None,
                        children: vec![],
                        is_separator: false,
                    }],
                    anchor: (0, 0),
                    screen_size: (0, 0),
                },
                bar_name: String::new(),
            },
        );

        let _ = app.view(popup);
    }

    #[test]
    fn panel_surface_view_renders_without_panic() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.surfaces.insert_panel_popup(
            panel,
            super::super::surfaces::PanelMeta {
                tree: crate::surface::panel::PanelTree {
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::BarPanel,
                    origin_window: origin,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    anchor: None,
                    screen_size: (1920, 1080),
                },
            },
        );

        let _ = app.view(panel);
    }
}
