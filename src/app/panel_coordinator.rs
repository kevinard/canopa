#[derive(Debug, Clone, PartialEq)]
pub(super) struct PanelOpenRequest {
    pub(super) origin_window: iced::window::Id,
    pub(super) origin: crate::surface::panel::PanelOrigin,
    pub(super) content: crate::surface::panel::PanelContent,
    pub(super) anchor: Option<iced::Rectangle>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PendingPanelOpen {
    pub(super) request: PanelOpenRequest,
    pub(super) menu_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum PanelEffect {
    OpenPanelWindow(PanelOpenRequest),
    ClosePanelWindow(iced::window::Id),
    RequestSearchFocus,
    LoadInstalledAppsThenOpenPanel(PendingPanelOpen),
}

#[derive(Debug, Clone, PartialEq)]
struct ActivePanel {
    window: iced::window::Id,
    origin_window: iced::window::Id,
    content: crate::surface::panel::PanelContent,
}

#[derive(Debug, Default)]
pub(super) struct PanelCoordinator {
    runtime: super::panel_runtime::PanelRuntime,
    active_panel: Option<ActivePanel>,
    #[cfg(test)]
    pub(super) installed_apps_for_test: Option<Vec<crate::desktop_apps::DesktopApplication>>,
    #[cfg(test)]
    pub(super) launched_installed_app_ids_for_test: Vec<String>,
}

impl PanelCoordinator {
    pub(super) fn clear(&mut self) {
        self.active_panel = None;
        self.runtime.clear();
    }

    pub(super) fn mark_closing_window(&mut self, window: iced::window::Id) {
        self.runtime.mark_closing_window(window);
    }

    pub(super) fn is_closing_window(&self, window: iced::window::Id) -> bool {
        self.runtime.is_closing_window(window)
    }

    pub(super) fn forget_closing_window(&mut self, window: iced::window::Id) {
        self.runtime.forget_closing_window(window);
    }

    pub(super) fn menu_scroll_y(&self) -> f32 {
        self.runtime.menu_scroll_y()
    }

    pub(super) fn menu_viewport_height(&self) -> f32 {
        self.runtime.menu_viewport_height()
    }

    pub(super) fn set_menu_scrolled(&mut self, y: f32, viewport_height: f32) {
        self.runtime.set_menu_scrolled(y, viewport_height);
    }

    pub(super) fn reset_menu_scroll(&mut self) {
        self.runtime.reset_menu_scroll();
    }

    pub(super) fn search_focus_pending(&self) -> bool {
        self.runtime.search_focus_pending()
    }

    pub(super) fn set_search_focus_pending(&mut self, pending: bool) {
        self.runtime.set_search_focus_pending(pending);
    }

    pub(super) fn has_dmenu_responder(&self) -> bool {
        self.runtime.has_dmenu_responder()
    }

    pub(super) fn begin_dmenu_request(
        &mut self,
        responder: super::messages::IpcResponder,
    ) -> Result<(), super::messages::IpcResponder> {
        self.runtime.begin_dmenu_request(responder)
    }

    pub(super) fn take_dmenu_responder(&mut self) -> Option<super::messages::IpcResponder> {
        self.runtime.take_dmenu_responder()
    }

    pub(super) fn cancel_dmenu_if_active(&mut self) -> Option<super::messages::IpcResponder> {
        self.runtime.cancel_dmenu_if_active()
    }

    pub(super) fn open_session(&mut self, source: super::panel_menu_session::PanelMenuSource) {
        self.runtime.open_session(source);
    }

    pub(super) fn search_changed(&mut self, query: String) {
        self.runtime.search_changed(query);
    }

    pub(super) fn focus_index(&mut self, index: usize) {
        self.runtime.focus_index(index);
    }

    pub(super) fn move_focus(&mut self, delta: isize) -> Option<usize> {
        self.runtime.move_focus(delta)
    }

    pub(super) fn focused_index(&self) -> Option<usize> {
        self.runtime.focused_index()
    }

    pub(super) fn back(&mut self) {
        self.runtime.back();
    }

    pub(super) fn escape(&mut self) -> Vec<super::panel_runtime::PanelRuntimeEffect> {
        self.runtime.escape()
    }

    pub(super) fn panel_menu_snapshot(
        &self,
    ) -> Option<super::panel_menu_session::PanelMenuSnapshot> {
        self.runtime.panel_menu_snapshot()
    }

    pub(super) fn active_panel_menu_entries(
        &self,
    ) -> Vec<super::panel_menu_session::PanelMenuEntry> {
        self.runtime.active_panel_menu_entries()
    }

    pub(super) fn activate(
        &mut self,
        index: Option<usize>,
    ) -> Vec<super::panel_runtime::PanelRuntimeEffect> {
        self.runtime.activate(index)
    }

    pub(super) fn active_panel_window(&self) -> Option<iced::window::Id> {
        self.active_panel.as_ref().map(|panel| panel.window)
    }

    pub(super) fn panel_window_opened(
        &mut self,
        window: iced::window::Id,
        origin_window: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) {
        self.active_panel = Some(ActivePanel {
            window,
            origin_window,
            content,
        });
    }

    pub(super) fn open_panel(&mut self, request: PanelOpenRequest) -> Vec<PanelEffect> {
        self.open_panel_after_source_ready(request)
    }

    pub(super) fn open_panel_after_installed_apps_loaded(
        &mut self,
        pending: PendingPanelOpen,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
        menus: &[crate::config::MenuConfig],
    ) -> Vec<PanelEffect> {
        let effects = self.open_panel_after_source_ready(pending.request);
        if let Some(menu) = menus.iter().find(|menu| menu.name == pending.menu_name) {
            self.runtime
                .open_session(super::panel_menu_session::PanelMenuSource::InstalledApps {
                    prompt: menu.prompt.clone(),
                    show_icons: menu.show_icons,
                    apps,
                });
        }
        effects
    }

    pub(super) fn open_panel_with_menus(
        &mut self,
        request: PanelOpenRequest,
        menus: &[crate::config::MenuConfig],
    ) -> Vec<PanelEffect> {
        if let crate::surface::panel::PanelContent::Menu(name) = &request.content
            && menus
                .iter()
                .find(|menu| menu.name == *name)
                .and_then(|menu| menu.source.as_ref())
                .is_some_and(|source| {
                    matches!(source.kind, crate::config::MenuSourceKind::InstalledApps)
                })
        {
            let menu_name = name.clone();
            return vec![PanelEffect::LoadInstalledAppsThenOpenPanel(
                PendingPanelOpen { request, menu_name },
            )];
        }
        self.open_panel_after_source_ready(request)
    }

    pub(super) fn toggle_panel(&mut self, request: PanelOpenRequest) -> Vec<PanelEffect> {
        if self.active_panel.as_ref().is_some_and(|panel| {
            panel.origin_window == request.origin_window && panel.content == request.content
        }) {
            if let Some(window) = self.active_panel_window() {
                self.active_panel = None;
                self.runtime.clear();
                return vec![PanelEffect::ClosePanelWindow(window)];
            }
        }
        self.open_panel(request)
    }

    pub(super) fn toggle_panel_with_menus(
        &mut self,
        request: PanelOpenRequest,
        menus: &[crate::config::MenuConfig],
    ) -> Vec<PanelEffect> {
        if self.active_panel.as_ref().is_some_and(|panel| {
            panel.origin_window == request.origin_window && panel.content == request.content
        }) {
            if let Some(window) = self.active_panel_window() {
                self.active_panel = None;
                self.runtime.clear();
                return vec![PanelEffect::ClosePanelWindow(window)];
            }
        }
        self.open_panel_with_menus(request, menus)
    }

    fn open_panel_after_source_ready(&mut self, request: PanelOpenRequest) -> Vec<PanelEffect> {
        let mut effects = Vec::new();
        if let Some(window) = self.active_panel_window() {
            effects.push(PanelEffect::ClosePanelWindow(window));
        }
        self.active_panel = None;
        self.runtime.clear();
        effects.push(PanelEffect::OpenPanelWindow(request.clone()));
        if matches!(
            request.content,
            crate::surface::panel::PanelContent::Menu(_)
                | crate::surface::panel::PanelContent::DmenuRequest
        ) {
            effects.push(PanelEffect::RequestSearchFocus);
        }
        effects
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{MenuConfig, MenuSourceConfig, MenuSourceKind};
    use crate::surface::panel::{PanelContent, PanelOrigin};

    use super::*;

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn open_request(window: iced::window::Id, content: PanelContent) -> PanelOpenRequest {
        PanelOpenRequest {
            origin_window: window,
            origin: PanelOrigin::Bar,
            content,
            anchor: None,
        }
    }

    #[test]
    fn opening_a_panel_replaces_the_active_panel() {
        let origin = wid();
        let first_panel = wid();
        let second_panel = wid();
        let mut coordinator = PanelCoordinator::default();

        let first = coordinator.open_panel(open_request(origin, PanelContent::BarPanel));
        assert_eq!(
            first,
            vec![PanelEffect::OpenPanelWindow(open_request(
                origin,
                PanelContent::BarPanel
            ))]
        );
        coordinator.panel_window_opened(first_panel, origin, PanelContent::BarPanel);

        let second =
            coordinator.open_panel(open_request(origin, PanelContent::Menu("main".into())));

        assert_eq!(
            second,
            vec![
                PanelEffect::ClosePanelWindow(first_panel),
                PanelEffect::OpenPanelWindow(open_request(
                    origin,
                    PanelContent::Menu("main".into())
                )),
                PanelEffect::RequestSearchFocus,
            ]
        );
        coordinator.panel_window_opened(second_panel, origin, PanelContent::Menu("main".into()));
        assert_eq!(coordinator.active_panel_window(), Some(second_panel));
    }

    #[test]
    fn toggling_the_same_panel_closes_it() {
        let origin = wid();
        let panel = wid();
        let mut coordinator = PanelCoordinator::default();
        coordinator.panel_window_opened(panel, origin, PanelContent::BarPanel);

        let effects = coordinator.toggle_panel(open_request(origin, PanelContent::BarPanel));

        assert_eq!(effects, vec![PanelEffect::ClosePanelWindow(panel)]);
        assert_eq!(coordinator.active_panel_window(), None);
    }

    #[test]
    fn installed_apps_menu_returns_load_effect_before_opening_panel() {
        let origin = wid();
        let mut coordinator = PanelCoordinator::default();
        let menus = vec![MenuConfig {
            name: "apps".into(),
            prompt: Some("Run".into()),
            show_icons: true,
            source: Some(MenuSourceConfig {
                kind: MenuSourceKind::InstalledApps,
            }),
            items: Vec::new(),
        }];

        let effects = coordinator.open_panel_with_menus(
            open_request(origin, PanelContent::Menu("apps".into())),
            &menus,
        );

        assert_eq!(
            effects,
            vec![PanelEffect::LoadInstalledAppsThenOpenPanel(
                PendingPanelOpen {
                    request: open_request(origin, PanelContent::Menu("apps".into())),
                    menu_name: "apps".into(),
                }
            )]
        );
        assert_eq!(coordinator.active_panel_window(), None);
    }
}
