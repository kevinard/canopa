use iced::Task;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings,
};

use super::{App, Message};

#[derive(Debug, Default)]
pub(super) struct PanelSystem {
    pub(super) search_query: String,
    pub(super) focused_index: Option<usize>,
    pub(super) menu_path: Vec<usize>,
}

impl PanelSystem {
    pub(super) fn clear(&mut self) {
        self.search_query.clear();
        self.focused_index = None;
        self.menu_path.clear();
    }
}

fn panel_window_settings(
    bar: &crate::config::BarConfig,
    _tree: &crate::surface::panel::PanelTree,
) -> NewLayerShellSettings {
    let width = bar.panel.width;
    let height = bar.panel.height.or(bar.panel.max_height).unwrap_or(600);
    let anchor = match bar.edge {
        crate::config::Edge::Top => Anchor::Top | Anchor::Left,
        crate::config::Edge::Bottom => Anchor::Bottom | Anchor::Left,
        crate::config::Edge::Left => Anchor::Left | Anchor::Top,
        crate::config::Edge::Right => Anchor::Right | Anchor::Top,
    };

    NewLayerShellSettings {
        anchor,
        exclusive_zone: Some(-1),
        size: Some((width, height)),
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        namespace: Some(App::namespace()),
        ..Default::default()
    }
}

impl App {
    pub(super) fn close_all_panels(&mut self) -> Vec<Task<Message>> {
        let ids: Vec<_> = self.surfaces.panel_popup_ids().collect();
        self.panel.clear();
        ids.into_iter()
            .map(|id| {
                self.surfaces.remove_panel_popup(id);
                Task::done(Message::RemoveWindow(id))
            })
            .collect()
    }

    pub(super) fn handle_panel_event(
        &mut self,
        event: super::messages::PanelEvent,
    ) -> Task<Message> {
        use super::messages::PanelEvent;

        match event {
            PanelEvent::Open {
                origin_window,
                origin,
                content,
                anchor,
            } => self.open_panel(origin_window, origin, content, anchor),
            PanelEvent::Toggle {
                origin_window,
                origin,
                content,
                anchor,
            } => {
                let same_panel_open = self.surfaces.panel_popup_values().any(|meta| {
                    meta.tree.origin_window == origin_window
                        && meta.tree.origin == origin
                        && meta.tree.content == content
                });
                if same_panel_open {
                    Task::batch(self.close_all_panels())
                } else {
                    self.open_panel(origin_window, origin, content, anchor)
                }
            }
            PanelEvent::Close => Task::batch(self.close_all_panels()),
            PanelEvent::SearchChanged(query) => {
                self.panel.search_query = query;
                Task::none()
            }
            PanelEvent::FocusMoveNext
            | PanelEvent::FocusMovePrev
            | PanelEvent::Activate
            | PanelEvent::Back => Task::none(),
        }
    }

    fn open_panel(
        &mut self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        anchor: Option<iced::Rectangle>,
    ) -> Task<Message> {
        let origin_surface = self.surface(origin_window).cloned().unwrap_or_default();
        let tree = crate::surface::panel::PanelTree {
            origin,
            content,
            origin_window,
            origin_bar_name: origin_surface.bar_name.clone(),
            origin_output_name: origin_surface.output_name.clone(),
            anchor,
            screen_size: origin_surface.screen_size,
        };
        let bar = Self::bar_for_surface_name(&self.config.bars, &origin_surface.bar_name);
        let settings = panel_window_settings(bar, &tree);
        let (panel_window, open_task) = Message::layershell_open(settings);

        let mut tasks = self.close_tooltip_popup_if_open(origin_window);
        tasks.extend(self.close_all_menu_popups());
        tasks.extend(self.close_all_panels());
        self.surfaces.insert_panel_popup(
            panel_window,
            super::surfaces::PanelMeta { tree },
        );
        tasks.push(open_task);
        Task::batch(tasks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_panel_closes_existing_panel() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));
        assert_eq!(app.surfaces.panel_popup_ids().count(), 1);

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            anchor: None,
        }));
        assert_eq!(app.surfaces.panel_popup_ids().count(), 1);
    }

    #[test]
    fn closing_panel_removes_panel_metadata() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Close));

        assert_eq!(app.surfaces.panel_popup_ids().count(), 0);
    }
}
