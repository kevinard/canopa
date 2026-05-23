use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PanelRuntimeEffect {
    RequestSearchFocus,
    RunPanelAction(crate::config::PanelAction),
    LaunchInstalledApp(String),
    CompleteDmenu(crate::ipc::protocol::IpcResponse),
    None,
}

#[derive(Debug, Default)]
pub(super) struct PanelRuntime {
    session: Option<super::panel_menu_session::PanelMenuSession>,
    menu_scroll_y: f32,
    menu_viewport_height: f32,
    search_focus_pending: bool,
    dmenu_responder: Option<super::messages::IpcResponder>,
    #[cfg(test)]
    pub(super) installed_apps_for_test: Option<Vec<crate::desktop_apps::DesktopApplication>>,
    #[cfg(test)]
    pub(super) launched_installed_app_ids_for_test: Vec<String>,
    closing_windows: HashSet<iced::window::Id>,
}

impl PanelRuntime {
    pub(super) fn clear(&mut self) {
        self.session = None;
        self.menu_scroll_y = 0.0;
        self.menu_viewport_height = 0.0;
        self.search_focus_pending = false;
        self.dmenu_responder = None;
    }

    pub(super) fn mark_closing_window(&mut self, window: iced::window::Id) {
        self.closing_windows.insert(window);
    }

    pub(super) fn is_closing_window(&self, window: iced::window::Id) -> bool {
        self.closing_windows.contains(&window)
    }

    pub(super) fn forget_closing_window(&mut self, window: iced::window::Id) {
        self.closing_windows.remove(&window);
    }

    pub(super) fn menu_scroll_y(&self) -> f32 {
        self.menu_scroll_y
    }

    pub(super) fn menu_viewport_height(&self) -> f32 {
        self.menu_viewport_height
    }

    pub(super) fn set_menu_scrolled(&mut self, y: f32, viewport_height: f32) {
        self.menu_scroll_y = y.max(0.0);
        self.menu_viewport_height = viewport_height.max(0.0);
    }

    pub(super) fn reset_menu_scroll(&mut self) {
        self.menu_scroll_y = 0.0;
    }

    pub(super) fn search_focus_pending(&self) -> bool {
        self.search_focus_pending
    }

    pub(super) fn set_search_focus_pending(&mut self, pending: bool) {
        self.search_focus_pending = pending;
    }

    pub(super) fn has_dmenu_responder(&self) -> bool {
        self.dmenu_responder.is_some()
    }

    pub(super) fn begin_dmenu_request(
        &mut self,
        responder: super::messages::IpcResponder,
    ) -> Result<(), super::messages::IpcResponder> {
        if self.dmenu_responder.is_some() {
            return Err(responder);
        }
        self.dmenu_responder = Some(responder);
        Ok(())
    }

    pub(super) fn take_dmenu_responder(&mut self) -> Option<super::messages::IpcResponder> {
        self.dmenu_responder.take()
    }

    pub(super) fn cancel_dmenu_if_active(&mut self) -> Option<super::messages::IpcResponder> {
        self.dmenu_responder.take()
    }

    pub(super) fn open_session(&mut self, source: super::panel_menu_session::PanelMenuSource) {
        let session = super::panel_menu_session::PanelMenuSession::open(source);
        let _ = session.snapshot();
        self.session = Some(session);
    }

    pub(super) fn clear_session(&mut self) {
        self.session = None;
    }

    pub(super) fn search_changed(&mut self, query: String) {
        if let Some(session) = self.session.as_mut() {
            session.set_search_query(query);
        }
        self.search_focus_pending = false;
        self.menu_scroll_y = 0.0;
    }

    pub(super) fn focus_index(&mut self, index: usize) {
        if let Some(session) = self.session.as_mut() {
            session.focus_index(index);
        }
        self.search_focus_pending = false;
    }

    pub(super) fn move_focus(&mut self, delta: isize) -> Option<usize> {
        if self.active_panel_menu_entries().is_empty() {
            self.menu_scroll_y = 0.0;
            return None;
        }
        let session = self.session.as_mut()?;
        session.move_focus(delta);
        session.focused_index()
    }

    pub(super) fn focused_index(&self) -> Option<usize> {
        self.session
            .as_ref()
            .and_then(|session| session.focused_index())
    }

    pub(super) fn back(&mut self) {
        if let Some(session) = self.session.as_mut() {
            session.back();
        }
    }

    pub(super) fn escape(&mut self) -> Vec<PanelRuntimeEffect> {
        let cleared_search = self
            .session
            .as_mut()
            .map(super::panel_menu_session::PanelMenuSession::clear_search)
            .unwrap_or(false);
        if cleared_search {
            self.search_focus_pending = true;
            vec![PanelRuntimeEffect::RequestSearchFocus]
        } else {
            Vec::new()
        }
    }

    pub(super) fn panel_menu_snapshot(
        &self,
    ) -> Option<super::panel_menu_session::PanelMenuSnapshot> {
        self.session.as_ref().map(|session| session.snapshot())
    }

    pub(super) fn active_panel_menu_entries(
        &self,
    ) -> Vec<super::panel_menu_session::PanelMenuEntry> {
        self.panel_menu_snapshot()
            .map(|snapshot| snapshot.entries.to_vec())
            .unwrap_or_default()
    }

    pub(super) fn activate(&mut self, index: Option<usize>) -> Vec<PanelRuntimeEffect> {
        let activation = match (self.session.as_mut(), index) {
            (Some(session), Some(index)) => session.activate_index(index),
            (Some(session), None) => session.activate_focused(),
            (None, _) => super::panel_menu_session::PanelMenuActivation::None,
        };
        match activation {
            super::panel_menu_session::PanelMenuActivation::RunPanelAction(action) => {
                vec![PanelRuntimeEffect::RunPanelAction(action)]
            }
            super::panel_menu_session::PanelMenuActivation::LaunchInstalledApp(id) => {
                vec![PanelRuntimeEffect::LaunchInstalledApp(id)]
            }
            super::panel_menu_session::PanelMenuActivation::CompleteDmenu(response) => {
                vec![PanelRuntimeEffect::CompleteDmenu(response)]
            }
            super::panel_menu_session::PanelMenuActivation::None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu_with_two_entries() -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "apps".into(),
            prompt: Some("Run".into()),
            show_icons: false,
            source: None,
            items: vec![
                crate::config::MenuItemConfig {
                    label: "Terminal".into(),
                    icon: None,
                    keywords: vec!["shell".into()],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::CommandConfig::Shell("kitty".into()),
                    )),
                    children: vec![],
                },
                crate::config::MenuItemConfig {
                    label: "Browser".into(),
                    icon: None,
                    keywords: vec!["web".into()],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::CommandConfig::Shell("firefox".into()),
                    )),
                    children: vec![],
                },
            ],
        }
    }

    #[test]
    fn clear_resets_session_scroll_focus_and_dmenu_state() {
        let mut runtime = PanelRuntime::default();
        runtime.set_menu_scrolled(120.0, 300.0);
        runtime.set_search_focus_pending(true);
        runtime.clear();

        assert_eq!(runtime.menu_scroll_y(), 0.0);
        assert_eq!(runtime.menu_viewport_height(), 0.0);
        assert!(!runtime.search_focus_pending());
        assert!(!runtime.has_dmenu_responder());
    }

    #[test]
    fn closing_windows_are_tracked_by_window_id() {
        let window = iced::window::Id::unique();
        let mut runtime = PanelRuntime::default();

        runtime.mark_closing_window(window);
        assert!(runtime.is_closing_window(window));

        runtime.forget_closing_window(window);
        assert!(!runtime.is_closing_window(window));
    }

    #[test]
    fn search_changed_updates_session_and_resets_scroll() {
        let mut runtime = PanelRuntime::default();
        runtime.open_session(
            super::super::panel_menu_session::PanelMenuSource::Configured(menu_with_two_entries()),
        );
        runtime.set_menu_scrolled(90.0, 200.0);

        runtime.search_changed("term".into());

        let snapshot = runtime.panel_menu_snapshot().expect("snapshot");
        assert_eq!(snapshot.search_query, "term");
        assert_eq!(runtime.menu_scroll_y(), 0.0);
    }

    #[test]
    fn activate_focused_returns_panel_action_effect() {
        let mut runtime = PanelRuntime::default();
        runtime.open_session(
            super::super::panel_menu_session::PanelMenuSource::Configured(menu_with_two_entries()),
        );

        let effects = runtime.activate(None);

        assert!(matches!(
            effects.as_slice(),
            [PanelRuntimeEffect::RunPanelAction(
                crate::config::PanelAction::Command(_)
            )]
        ));
    }

    #[test]
    fn dmenu_responder_rejects_second_active_request() {
        let (sender, _receiver) = tokio::sync::oneshot::channel();
        let responder = super::super::messages::ipc_responder(sender);
        let mut runtime = PanelRuntime::default();

        assert!(runtime.begin_dmenu_request(responder).is_ok());

        let (sender2, _receiver2) = tokio::sync::oneshot::channel();
        let responder2 = super::super::messages::ipc_responder(sender2);
        assert!(runtime.begin_dmenu_request(responder2).is_err());
    }

    #[test]
    fn complete_dmenu_request_takes_responder_once() {
        let (sender, _receiver) = tokio::sync::oneshot::channel();
        let responder = super::super::messages::ipc_responder(sender);
        let mut runtime = PanelRuntime::default();

        runtime.begin_dmenu_request(responder).expect("stored");
        let first = runtime.take_dmenu_responder();
        let second = runtime.take_dmenu_responder();

        assert!(first.is_some());
        assert!(second.is_none());
    }
}
