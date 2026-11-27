use iced::Task;

use super::{App, Message};
use crate::config::FormatMode;
use crate::modules::ModuleId;
use crate::surface::SurfaceState;

impl App {
    pub(super) fn module_has_content_on_surface(
        &self,
        id: &ModuleId,
        surface: &SurfaceState,
        mode: FormatMode,
    ) -> bool {
        self.module_offered(id)
            && self.modules.kind_for(id).is_some_and(|kind| {
                let state = self.module_render_state(kind, id, surface);
                self.modules.has_content(id, &state, mode)
            })
    }

    pub(super) fn reconcile_module_visibility(&mut self) -> Vec<Task<Message>> {
        let windows: Vec<_> = self.surfaces.surfaces.keys().copied().collect();
        let mut tasks = Vec::new();
        for window in windows {
            let visible = self.visible_module_layout(window);
            let hidden = |id: &ModuleId| !visible.iter().any(|(_, candidate)| candidate == id);
            let hide_tooltip = self
                .surfaces
                .tooltip_popups
                .values()
                .any(|popup| popup.target_window == window && hidden(&popup.target_module));
            let clear_focus = self
                .surface(window)
                .and_then(|surface| surface.focused_module.as_ref())
                .is_some_and(|id| hidden(id) || !self.module_is_keyboard_target(id));
            let surface = self.surface_mut(window);
            if clear_focus {
                surface.focused_module = None;
                surface.module_activated = false;
            }
            if surface.hovered_module.as_ref().is_some_and(hidden) {
                surface.hovered_module = None;
            }
            if surface
                .tooltip_pending
                .as_ref()
                .is_some_and(|(id, _)| hidden(id))
            {
                surface.tooltip_pending = None;
            }
            if hide_tooltip {
                tasks.extend(self.close_tooltip_popup_if_open(window));
            }
        }
        tasks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ModuleConfig, ModuleParams};
    use crate::modules::ModuleState;
    use crate::modules::volume::VolumeEvent;

    fn fixture() -> (App, iced::window::Id) {
        let definitions = vec![
            ("clock", ModuleParams::Clock(Default::default())),
            ("volume", ModuleParams::Volume(Default::default())),
            (
                "focused-window",
                ModuleParams::FocusedWindow(Default::default()),
            ),
            ("workspaces", ModuleParams::Workspaces(Default::default())),
            ("tray", ModuleParams::Tray(Default::default())),
            (
                "script",
                ModuleParams::Script(crate::config::ScriptConfig {
                    command: crate::config::ConfiguredCommand::argv(
                        "true".into(),
                        vec![],
                        &Default::default(),
                    ),
                    interval: 60,
                    format: Default::default(),
                    content_rotation: Default::default(),
                    tooltip: Default::default(),
                }),
            ),
        ];
        let mut config = Config::default();
        config.bars[0].modules.end = definitions
            .iter()
            .map(|(name, _)| name.to_string())
            .collect();
        config.bars[0].panel.modules.end = config.bars[0].modules.end.clone();
        config.modules = definitions
            .into_iter()
            .map(|(name, params)| ModuleConfig {
                name: name.into(),
                module_type: name.into(),
                params,
                actions: Default::default(),
            })
            .collect();
        let (mut app, _) = App::boot(config, vec![]);
        let window = iced::window::Id::unique();
        app.surfaces
            .surfaces
            .insert(window, SurfaceState::new("DP-1".into(), "main".into()));
        app.modules.set_state(
            ModuleId::new("clock"),
            ModuleState::Clock(crate::modules::clock::ClockState {
                time: "12:00".into(),
                date: "Wed 23 Sep".into(),
            }),
        );
        app.modules
            .set_state(ModuleId::new("volume"), ModuleState::Unavailable);
        app.modules.set_state(
            ModuleId::new("focused-window"),
            ModuleState::FocusedWindow(crate::modules::focused_window::FocusedWindowState {
                window: None,
            }),
        );
        app.modules.set_state(
            ModuleId::new("script"),
            ModuleState::Script(crate::modules::script::ScriptState {
                output: "  ".into(),
            }),
        );
        (app, window)
    }

    #[test]
    fn unavailable_bar_status_stays_hidden_while_panel_controls_remain_reachable() {
        let (mut app, window) = fixture();
        let panel = iced::window::Id::unique();
        app.register_panel_popup(panel, window);
        let ids = |model: super::super::render::BarRenderModel<'_>| {
            model
                .end
                .into_iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids(app.bar_render_model(window)),
            vec![ModuleId::new("clock")]
        );
        assert_eq!(
            app.effective_nav_list(window),
            vec![(crate::modules::Zone::End, ModuleId::new("clock"))]
        );
        let entries = app.panel_render_model(panel).unwrap().entries;
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.id.clone())
                .collect::<Vec<_>>(),
            vec![ModuleId::new("clock"), ModuleId::new("volume")]
        );
        assert!(
            app.resolved_tooltip_for(window, &ModuleId::new("volume"))
                .is_none()
        );
    }

    #[test]
    fn escape_releases_keyboard_after_the_focused_module_disappears() {
        for keep_clock in [false, true] {
            let (mut app, window) = fixture();
            if !keep_clock {
                app.modules
                    .set_state(ModuleId::new("clock"), ModuleState::Empty);
            }
            let _ = app.update(Message::Modules(
                super::super::messages::ModuleRuntimeEvent::Volume(VolumeEvent::StateChanged {
                    volume: 50.0,
                    muted: false,
                }),
            ));
            app.surface_mut(window).focused_module = Some(ModuleId::new("volume"));
            let _ = app.update(Message::Modules(
                super::super::messages::ModuleRuntimeEvent::Volume(VolumeEvent::Unavailable),
            ));
            assert!(app.surface(window).unwrap().focused_module.is_none());
            assert_eq!(
                app.effective_nav_list(window).len(),
                usize::from(keep_clock)
            );

            let release = app.update(Message::Surface(
                super::super::messages::SurfaceEvent::EscapePressed(window),
            ));
            assert_eq!(
                release.units(),
                1,
                "Escape must release the surface keyboard grab"
            );
        }
    }

    #[test]
    fn service_loss_retires_focus_and_tooltips_and_recovery_restores_content() {
        let (mut app, window) = fixture();
        let volume = ModuleId::new("volume");
        let _ = app.update(Message::Modules(
            super::super::messages::ModuleRuntimeEvent::Volume(VolumeEvent::StateChanged {
                volume: 50.0,
                muted: false,
            }),
        ));
        let surface = app.surface_mut(window);
        surface.focused_module = Some(volume.clone());
        surface.hovered_module = Some(volume.clone());
        surface.module_activated = true;
        surface.tooltip_pending = Some((volume.clone(), 0.0));
        let tooltip = iced::window::Id::unique();
        app.register_tooltip_popup(
            tooltip,
            window,
            volume.clone(),
            crate::surface::tooltip_popup::TooltipTree {
                segments: vec![],
                card: None,
                anchor: (0, 0),
                bar_size: (48, 800),
                screen_size: (1280, 800),
                bar_edge: crate::config::Edge::Left,
            },
        );
        let _ = app.update(Message::Modules(
            super::super::messages::ModuleRuntimeEvent::Volume(VolumeEvent::Unavailable),
        ));
        let surface = app.surface(window).unwrap();
        assert!(surface.focused_module.is_none() && surface.hovered_module.is_none());
        assert!(!surface.module_activated && surface.tooltip_pending.is_none());
        assert!(app.surfaces.tooltip_popup(tooltip).is_none());
        assert_eq!(app.effective_nav_list(window).len(), 1);

        let _ = app.update(Message::Modules(
            super::super::messages::ModuleRuntimeEvent::Volume(VolumeEvent::StateChanged {
                volume: 50.0,
                muted: true,
            }),
        ));
        assert_eq!(app.effective_nav_list(window).len(), 2);
        assert_eq!(app.bar_render_model(window).end.len(), 2);
        assert!(app.resolved_tooltip_for(window, &volume).is_some());
    }
}
