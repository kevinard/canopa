use iced::widget::{Space, column, container, mouse_area, row, scrollable, text, text_input};
use iced::{Element, Length};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use super::render::{BarRenderModel, PanelRenderModel, PopupRenderModel};
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

        if let Some(model) = self.panel_render_model(window) {
            return render_panel_model(model);
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

fn render_panel_model<'a>(model: PanelRenderModel<'a>) -> Element<'a, Message> {
    let text_color = model.bar_style.resolve().color;
    if matches!(
        model.tree.content,
        crate::surface::panel::PanelContent::Menu(_)
            | crate::surface::panel::PanelContent::DmenuRequest
    ) {
        return render_panel_menu_model(model, text_color);
    }

    let entries: Vec<Element<'_, Message>> = model
        .entries
        .into_iter()
        .map(|entry| entry.element)
        .collect();

    container(scrollable(column(entries).spacing(8).padding(8)))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(crate::theme::BASE)),
            text_color,
            border: iced::Border {
                color: crate::theme::BORDER,
                width: 1.0,
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn render_panel_menu_model<'a>(
    model: PanelRenderModel<'a>,
    text_color: Option<iced::Color>,
) -> Element<'a, Message> {
    const MENU_RENDERED_ROWS: usize = 40;
    const MENU_ROW_BUFFER: usize = 8;

    let placeholder = model.menu_prompt.as_deref().unwrap_or("Search");
    let search = text_input(placeholder, &model.search_query)
        .id(model.search_input_id.clone())
        .on_input(|query| Message::Panel(super::messages::PanelEvent::SearchChanged(query)))
        .padding([8, 10])
        .width(Length::Fill);

    let mut list = column![].spacing(8).padding(8);
    let entry_count = model.menu_entries.len();
    let first_visible = (model.menu_scroll_y / super::panel::MENU_ROW_EXTENT)
        .floor()
        .max(0.0) as usize;
    let start = first_visible.saturating_sub(MENU_ROW_BUFFER);
    let end = (start + MENU_RENDERED_ROWS).min(entry_count);
    let top_spacer = start as f32 * super::panel::MENU_ROW_EXTENT;
    let bottom_spacer = entry_count.saturating_sub(end) as f32 * super::panel::MENU_ROW_EXTENT;

    if top_spacer > 0.0 {
        list = list.push(Space::new().width(Length::Fill).height(top_spacer));
    }

    for (index, entry) in model
        .menu_entries
        .iter()
        .enumerate()
        .skip(start)
        .take(end.saturating_sub(start))
    {
        let focused = model.focused_index == Some(index);
        let mut row_content = row![].spacing(8).width(Length::Fill);
        if model.show_icons {
            row_content = row_content.push(panel_menu_icon_element(entry.icon.as_deref()));
        }
        row_content = row_content.push(text(entry.label.clone()).width(Length::Fill));
        if entry.is_submenu {
            row_content = row_content.push(text(">"));
        }
        let row = container(row_content)
            .padding([8, 10])
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: if focused {
                    Some(iced::Background::Color(crate::theme::SURFACE0))
                } else {
                    Some(iced::Background::Color(crate::theme::SURFACE1))
                },
                border: iced::Border {
                    color: crate::theme::BORDER,
                    width: 1.0,
                    radius: iced::border::Radius::from(4.0),
                },
                ..Default::default()
            });
        list = list.push(
            mouse_area(row)
                .on_enter(Message::Panel(super::messages::PanelEvent::FocusIndex(
                    index,
                )))
                .on_press(Message::Panel(super::messages::PanelEvent::ActivateIndex(
                    index,
                ))),
        );
    }

    if bottom_spacer > 0.0 {
        list = list.push(Space::new().width(Length::Fill).height(bottom_spacer));
    }

    let menu_scroll = scrollable(list)
        .id(model.menu_scroll_id)
        .on_scroll(|viewport| {
            Message::Panel(super::messages::PanelEvent::MenuScrolled {
                y: viewport.absolute_offset().y,
                viewport_height: viewport.bounds().height,
            })
        });

    container(column![search, menu_scroll])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(crate::theme::BASE)),
            text_color,
            border: iced::Border {
                color: crate::theme::BORDER,
                width: 1.0,
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn panel_menu_icon_element<'a>(icon: Option<&str>) -> Element<'a, Message> {
    const ICON_SIZE: u16 = 18;

    let Some(handle) = icon.and_then(cached_panel_menu_icon_handle) else {
        return Space::new()
            .width(ICON_SIZE as u32)
            .height(ICON_SIZE as u32)
            .into();
    };

    match handle {
        PanelMenuIconHandle::Svg(handle) => iced::widget::svg(handle)
            .width(ICON_SIZE as u32)
            .height(ICON_SIZE as u32)
            .into(),
        PanelMenuIconHandle::Image(handle) => iced::widget::image(handle)
            .width(ICON_SIZE as u32)
            .height(ICON_SIZE as u32)
            .into(),
    }
}

#[derive(Clone)]
enum PanelMenuIconHandle {
    Svg(iced::widget::svg::Handle),
    Image(iced::widget::image::Handle),
}

fn cached_panel_menu_icon_handle(icon: &str) -> Option<PanelMenuIconHandle> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<PanelMenuIconHandle>>>> = OnceLock::new();

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(handle) = cache
        .lock()
        .expect("panel menu icon cache poisoned")
        .get(icon)
        .cloned()
    {
        return handle;
    }

    let handle = crate::icon_theme::resolve_icon_path(icon).map(panel_menu_icon_handle_from_path);
    cache
        .lock()
        .expect("panel menu icon cache poisoned")
        .insert(icon.to_string(), handle.clone());
    handle
}

fn panel_menu_icon_handle_from_path(path: PathBuf) -> PanelMenuIconHandle {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext == "svg" {
        PanelMenuIconHandle::Svg(iced::widget::svg::Handle::from_path(path))
    } else {
        PanelMenuIconHandle::Image(iced::widget::image::Handle::from_path(path))
    }
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
        .on_right_press(Message::Surface(
            super::messages::SurfaceEvent::BarRightClick(window),
        ))
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
    use crate::config::{Config, MenuConfig, MenuSourceConfig, MenuSourceKind};
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
        app.register_menu_popup(
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
                origin: super::super::surfaces::BarSurfaceIdentity::all_screens(),
            },
        );

        let _ = app.view(popup);
    }

    #[test]
    fn panel_surface_view_renders_without_panic() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.register_panel_popup(
            panel,
            origin,
            crate::surface::panel::PanelTree {
                origin: crate::surface::panel::PanelOrigin::Bar,
                content: crate::surface::panel::PanelContent::BarPanel,
                origin_window: origin,
                origin_bar_name: "main".into(),
                origin_output_name: "DP-1".into(),
                anchor: None,
                screen_size: (1920, 1080),
            },
        );

        let _ = app.view(panel);
    }

    #[test]
    fn installed_apps_hover_view_path_does_not_rebuild_every_entry_widget() {
        let mut config = Config {
            menus: vec![MenuConfig {
                name: "apps".into(),
                show_icons: false,
                prompt: None,
                source: Some(MenuSourceConfig {
                    kind: MenuSourceKind::InstalledApps,
                }),
                items: vec![],
            }],
            ..Default::default()
        };
        config.bars[0].actions.bindings.clear();

        let (mut app, _) = App::boot(config, vec![]);
        app.panel.installed_apps_for_test = Some(
            (0..5_000)
                .map(|index| crate::desktop_apps::DesktopApplication {
                    id: format!("app-{index}.desktop"),
                    name: format!("App {index:04}"),
                    generic_name: None,
                    comment: None,
                    keywords: vec![],
                    icon: None,
                    exec: format!("app-{index}"),
                })
                .collect(),
        );
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let _ = app.update(crate::app::Message::Panel(
            crate::app::messages::PanelEvent::Open {
                origin_window: origin,
                origin: crate::surface::panel::PanelOrigin::Bar,
                content: crate::surface::panel::PanelContent::Menu("apps".into()),
                anchor: None,
            },
        ));
        let panel = app.surfaces.panel_popup_ids().next().expect("panel");
        let _ = app.view(panel);

        let started = std::time::Instant::now();
        for index in 0..100 {
            let _ = app.update(crate::app::Message::Panel(
                crate::app::messages::PanelEvent::FocusIndex(index),
            ));
            let _ = app.view(panel);
        }

        assert!(
            started.elapsed() < std::time::Duration::from_millis(50),
            "hover view path rebuilt too much installed-app widget state: {:?}",
            started.elapsed()
        );
    }
}
