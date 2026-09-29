use iced::Task;
use iced_layershell::actions::ActionCallback;

use super::{App, Message};
use crate::config::Edge;

impl App {
    /// Grow the surface around the bar, leaving its exclusive zone unchanged.
    /// Only the visible strip receives input; the drawer has its own surface.
    pub(super) fn set_drawer_bar_width(
        &mut self,
        window: iced::window::Id,
        width: u32,
    ) -> Task<Message> {
        let Some(surface) = self.surface(window) else {
            return Task::none();
        };
        if surface.drawer_width == width {
            return Task::none();
        }
        let bar = self.bar_for_window(window);
        let size = (bar.collapsed_width.saturating_add(width), 0);
        self.surface_mut(window).drawer_width = width;
        Task::done(Message::SizeChange { id: window, size })
            .chain(self.drawer_bar_input_region(window))
    }

    pub(super) fn drawer_bar_input_region(&self, window: iced::window::Id) -> Task<Message> {
        let Some(surface) = self.surface(window) else {
            return Task::none();
        };
        let bar = self.bar_for_window(window);
        if !bar.drawer_pushes_bar() {
            return Task::none();
        }
        let x = if matches!(bar.edge, Edge::Left) {
            surface.drawer_width
        } else {
            0
        };
        let x = i32::try_from(x).unwrap_or(i32::MAX);
        let width = i32::try_from(bar.collapsed_width).unwrap_or(i32::MAX);
        Task::done(Message::SetInputRegion {
            id: window,
            callback: ActionCallback::new(move |region| {
                // The backend reuses its region object across surfaces.
                region.subtract(0, 0, i32::MAX, i32::MAX);
                region.add(x, 0, width, i32::MAX);
            }),
        })
    }

    /// Derive cleanup from the current foreground, so a stale close event cannot
    /// return a bar to the edge while a newer drawer still belongs to it.
    pub(super) fn restore_inactive_drawer_bars(&mut self) -> Vec<Task<Message>> {
        let origin = self
            .foreground
            .active_panel_tree()
            .map(|tree| tree.origin_window);
        let restore: Vec<_> = self
            .surfaces
            .keys()
            .copied()
            .filter(|window| {
                Some(*window) != origin && self.surface(*window).is_some_and(|s| s.drawer_width > 0)
            })
            .collect();
        restore
            .into_iter()
            .map(|window| self.set_drawer_bar_width(window, 0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, DrawerMode, PanelPresentation};
    use crate::surface::panel::{PanelContent, PanelOrigin};

    fn open(app: &mut App, origin: iced::window::Id, content: PanelContent) {
        let _ = app.update(Message::Panel(super::super::messages::PanelEvent::Open {
            origin_window: origin,
            origin: PanelOrigin::Bar,
            content,
            anchor: None,
        }));
    }

    #[test]
    fn pushed_bar_tracks_replacement_origin_and_current_window_close() {
        for edge in [Edge::Left, Edge::Right] {
            let mut config = Config::default();
            config.bars[0].edge = edge;
            config.bars[0].panel.presentation = PanelPresentation::Drawer;
            config.bars[0].panel.drawer_mode = DrawerMode::PushBar;
            config.bars[0].panel.width = Some(280);
            let (mut app, _) = App::boot(config, vec![]);
            let first = iced::window::Id::unique();
            let second = iced::window::Id::unique();
            for window in [first, second] {
                app.surface_mut(window).bar_name = "main".into();
                app.surface_mut(window).screen_size = (48, 800);
            }
            open(&mut app, first, PanelContent::BarPanel);
            assert_eq!(app.surface(first).unwrap().drawer_width, 280);
            assert_eq!(app.surface(second).unwrap().drawer_width, 0);
            let stale = app.foreground.active_panel_window().unwrap();

            open(
                &mut app,
                first,
                PanelContent::ModulePanel(crate::modules::ModuleId::new("clock")),
            );
            let _ =
                app.handle_surface_event(super::super::messages::SurfaceEvent::WindowClosed(stale));
            assert_eq!(app.surface(first).unwrap().drawer_width, 280);

            open(&mut app, second, PanelContent::BarPanel);
            assert_eq!(app.surface(first).unwrap().drawer_width, 0);
            assert_eq!(app.surface(second).unwrap().drawer_width, 280);
            let current = app.foreground.active_panel_window().unwrap();
            let _ = app
                .handle_surface_event(super::super::messages::SurfaceEvent::WindowClosed(current));
            assert_eq!(app.surface(second).unwrap().drawer_width, 0);

            open(&mut app, first, PanelContent::BarPanel);
            let _ = app.close_active_panel();
            assert_eq!(app.surface(first).unwrap().drawer_width, 0);
        }
    }
}
