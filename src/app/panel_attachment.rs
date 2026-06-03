use super::panel_runtime::{PanelAttachmentId, PanelRuntime, PanelRuntimeIntent};
use super::surfaces::SurfaceManager;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum PanelAttachmentEffect {
    RemoveWindow(iced::window::Id),
}

pub(super) fn attached_panel_tree_for_window<'a>(
    panel: &'a PanelRuntime,
    surfaces: &SurfaceManager,
    window: iced::window::Id,
) -> Option<&'a crate::surface::panel::PanelTree> {
    let meta = surfaces.panel_popup(window)?;
    panel.panel_tree_for_attachment(meta.attachment_id)
}

pub(super) fn close_attached_panel_window(
    panel: &mut PanelRuntime,
    surfaces: &mut SurfaceManager,
    window: iced::window::Id,
    attachment_id: PanelAttachmentId,
) -> Vec<PanelAttachmentEffect> {
    panel.handle(PanelRuntimeIntent::PanelWindowClosed {
        attachment_id,
        window,
    });
    surfaces.remove_panel_popup(window);
    vec![PanelAttachmentEffect::RemoveWindow(window)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn open_and_attach_panel(
        app: &mut super::super::App,
        origin: iced::window::Id,
        window: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) -> PanelAttachmentId {
        let request = app.panel_open_request(
            origin,
            crate::surface::panel::PanelOrigin::Bar,
            content,
            None,
        );
        let effects = app.panel.handle(super::PanelRuntimeIntent::Open {
            request,
            catalog: super::super::panel_runtime::PanelMenuCatalog::new(
                app.config.menus.as_slice(),
            ),
            module_detail_sessions: Default::default(),
        });
        let attachment_id = effects
            .into_iter()
            .find_map(|effect| match effect {
                super::super::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(request) => {
                    Some(request.attachment_id)
                }
                _ => None,
            })
            .expect("open panel window effect");
        app.panel
            .handle(super::PanelRuntimeIntent::PanelWindowOpened {
                attachment_id,
                window,
            });
        app.surfaces.insert_panel_popup(
            window,
            super::super::surfaces::PanelMeta {
                attachment_id,
                origin_window: origin,
            },
        );
        attachment_id
    }

    #[test]
    fn attached_panel_tree_is_none_for_stale_window() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let origin = wid();
        let stale_window = wid();
        let current_window = wid();

        let _stale_attachment = open_and_attach_panel(
            &mut app,
            origin,
            stale_window,
            crate::surface::panel::PanelContent::BarPanel,
        );
        let _current_attachment = open_and_attach_panel(
            &mut app,
            origin,
            current_window,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );

        assert!(attached_panel_tree_for_window(&app.panel, &app.surfaces, stale_window).is_none());
        assert!(
            attached_panel_tree_for_window(&app.panel, &app.surfaces, current_window).is_some()
        );
    }

    #[test]
    fn closing_stale_panel_detaches_window_without_clearing_current_panel() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let origin = wid();
        let stale_window = wid();
        let current_window = wid();

        let stale_attachment = open_and_attach_panel(
            &mut app,
            origin,
            stale_window,
            crate::surface::panel::PanelContent::BarPanel,
        );
        let _current_attachment = open_and_attach_panel(
            &mut app,
            origin,
            current_window,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );

        let effects = close_attached_panel_window(
            &mut app.panel,
            &mut app.surfaces,
            stale_window,
            stale_attachment,
        );

        assert_eq!(
            effects,
            vec![PanelAttachmentEffect::RemoveWindow(stale_window)]
        );
        assert_eq!(app.panel.active_panel_window(), Some(current_window));
        assert!(app.surfaces.panel_popup(stale_window).is_none());
        assert!(app.surfaces.panel_popup(current_window).is_some());
    }

    #[test]
    fn closing_current_panel_detaches_window_and_clears_panel_runtime() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let origin = wid();
        let window = wid();
        let attachment_id = open_and_attach_panel(
            &mut app,
            origin,
            window,
            crate::surface::panel::PanelContent::BarPanel,
        );

        let effects =
            close_attached_panel_window(&mut app.panel, &mut app.surfaces, window, attachment_id);

        assert_eq!(effects, vec![PanelAttachmentEffect::RemoveWindow(window)]);
        assert_eq!(app.panel.active_panel_window(), None);
        assert!(app.panel.active_panel_tree().is_none());
        assert!(app.surfaces.panel_popup(window).is_none());
    }
}
