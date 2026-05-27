use std::collections::HashSet;

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
struct ActivePanel {
    window: iced::window::Id,
    origin_window: iced::window::Id,
    content: crate::surface::panel::PanelContent,
}

#[derive(Debug, Clone)]
pub(super) enum PanelRuntimeEffect {
    OpenPanelWindow(PanelOpenRequest),
    ClosePanelWindow(iced::window::Id),
    RequestSearchFocus,
    LoadInstalledAppsThenOpenPanel(PendingPanelOpen),
    RunPanelAction(crate::config::PanelAction),
    LaunchInstalledApp(String),
    CompleteDmenu(crate::ipc::protocol::IpcResponse),
    SendDmenuResponse {
        responder: super::messages::IpcResponder,
        response: crate::ipc::protocol::IpcResponse,
    },
}

impl PartialEq for PanelRuntimeEffect {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::OpenPanelWindow(left), Self::OpenPanelWindow(right)) => left == right,
            (Self::ClosePanelWindow(left), Self::ClosePanelWindow(right)) => left == right,
            (Self::RequestSearchFocus, Self::RequestSearchFocus) => true,
            (
                Self::LoadInstalledAppsThenOpenPanel(left),
                Self::LoadInstalledAppsThenOpenPanel(right),
            ) => left == right,
            (Self::RunPanelAction(left), Self::RunPanelAction(right)) => left == right,
            (Self::LaunchInstalledApp(left), Self::LaunchInstalledApp(right)) => left == right,
            (Self::CompleteDmenu(left), Self::CompleteDmenu(right)) => left == right,
            (
                Self::SendDmenuResponse { response: left, .. },
                Self::SendDmenuResponse {
                    response: right, ..
                },
            ) => left == right,
            _ => false,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct PanelRuntime {
    active_panel: Option<ActivePanel>,
    session: Option<super::panel_menu_session::PanelMenuSession>,
    menu_scroll_y: f32,
    menu_viewport_height: f32,
    clock_month_offset: i32,
    search_focus_pending: bool,
    dmenu_responder: Option<super::messages::IpcResponder>,
    closing_windows: HashSet<iced::window::Id>,
    #[cfg(test)]
    pub(super) installed_apps_for_test: Option<Vec<crate::desktop_apps::DesktopApplication>>,
    #[cfg(test)]
    pub(super) launched_installed_app_ids_for_test: Vec<String>,
}

impl PanelRuntime {
    pub(super) fn clear(&mut self) {
        self.active_panel = None;
        self.session = None;
        self.menu_scroll_y = 0.0;
        self.menu_viewport_height = 0.0;
        self.clock_month_offset = 0;
        self.search_focus_pending = false;
        self.dmenu_responder = None;
    }

    pub(super) fn active_panel_window(&self) -> Option<iced::window::Id> {
        self.active_panel.as_ref().map(|panel| panel.window)
    }

    pub(super) fn is_clock_module_panel_open(&self) -> bool {
        self.active_panel.as_ref().is_some_and(|panel| {
            matches!(
                &panel.content,
                crate::surface::panel::PanelContent::ModulePanel(id)
                    if id.0 == "clock"
            )
        })
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

    pub(super) fn open_panel_with_menus(
        &mut self,
        request: PanelOpenRequest,
        menus: &[crate::config::MenuConfig],
    ) -> Vec<PanelRuntimeEffect> {
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
            return vec![PanelRuntimeEffect::LoadInstalledAppsThenOpenPanel(
                PendingPanelOpen { request, menu_name },
            )];
        }
        self.open_panel_after_source_ready(request)
    }

    pub(super) fn toggle_panel_with_menus(
        &mut self,
        request: PanelOpenRequest,
        menus: &[crate::config::MenuConfig],
    ) -> Vec<PanelRuntimeEffect> {
        if self.active_panel.as_ref().is_some_and(|panel| {
            panel.origin_window == request.origin_window && panel.content == request.content
        }) && let Some(window) = self.active_panel_window()
        {
            self.active_panel = None;
            self.session = None;
            self.menu_scroll_y = 0.0;
            self.menu_viewport_height = 0.0;
            self.clock_month_offset = 0;
            self.search_focus_pending = false;
            return vec![PanelRuntimeEffect::ClosePanelWindow(window)];
        }
        self.open_panel_with_menus(request, menus)
    }

    pub(super) fn open_panel_after_installed_apps_loaded(
        &mut self,
        pending: PendingPanelOpen,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
        menus: &[crate::config::MenuConfig],
    ) -> Vec<PanelRuntimeEffect> {
        let effects = self.open_panel_after_source_ready(pending.request);
        if let Some(menu) = menus.iter().find(|menu| menu.name == pending.menu_name) {
            self.open_session(super::panel_menu_session::PanelMenuSource::InstalledApps {
                prompt: menu.prompt.clone(),
                show_icons: menu.show_icons,
                apps,
            });
        }
        effects
    }

    fn open_panel_after_source_ready(
        &mut self,
        request: PanelOpenRequest,
    ) -> Vec<PanelRuntimeEffect> {
        let mut effects = Vec::new();
        if let Some(window) = self.active_panel_window() {
            effects.push(PanelRuntimeEffect::ClosePanelWindow(window));
        }
        self.active_panel = None;
        self.session = None;
        self.menu_scroll_y = 0.0;
        self.menu_viewport_height = 0.0;
        self.clock_month_offset = 0;
        self.search_focus_pending = false;
        effects.push(PanelRuntimeEffect::OpenPanelWindow(request.clone()));
        if matches!(
            request.content,
            crate::surface::panel::PanelContent::Menu(_)
                | crate::surface::panel::PanelContent::DmenuRequest
        ) {
            effects.push(PanelRuntimeEffect::RequestSearchFocus);
        }
        effects
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

    pub(super) fn clock_month_offset(&self) -> i32 {
        self.clock_month_offset
    }

    pub(super) fn browse_clock_month(&mut self, delta: i32) {
        if self.is_clock_module_panel_open() {
            self.clock_month_offset = self.clock_month_offset.saturating_add(delta);
        }
    }

    pub(super) fn reset_clock_month(&mut self) {
        if self.is_clock_module_panel_open() {
            self.clock_month_offset = 0;
        }
    }

    #[cfg(test)]
    pub(super) fn reset_menu_scroll(&mut self) {
        self.menu_scroll_y = 0.0;
    }

    pub(super) fn search_focus_pending(&self) -> bool {
        self.search_focus_pending
    }

    pub(super) fn set_search_focus_pending(&mut self, pending: bool) {
        self.search_focus_pending = pending;
    }

    #[cfg(test)]
    pub(super) fn has_dmenu_responder(&self) -> bool {
        self.dmenu_responder.is_some()
    }

    #[cfg(test)]
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

    pub(super) fn begin_dmenu_selection(
        &mut self,
        request: crate::ipc::protocol::DmenuRequest,
        responder: super::messages::IpcResponder,
        origin_window: Option<iced::window::Id>,
    ) -> Vec<PanelRuntimeEffect> {
        if self.dmenu_responder.is_some() {
            return vec![PanelRuntimeEffect::SendDmenuResponse {
                responder,
                response: crate::ipc::protocol::IpcResponse::Rejected {
                    reason: "another dmenu request is already active".into(),
                },
            }];
        }

        let Some(origin_window) = origin_window else {
            return vec![PanelRuntimeEffect::SendDmenuResponse {
                responder,
                response: crate::ipc::protocol::IpcResponse::Rejected {
                    reason: "no bar surface is available".into(),
                },
            }];
        };

        let mut effects = Vec::new();
        if let Some(window) = self.active_panel_window() {
            effects.push(PanelRuntimeEffect::ClosePanelWindow(window));
        }
        self.active_panel = None;
        self.session = None;
        self.menu_scroll_y = 0.0;
        self.menu_viewport_height = 0.0;
        self.clock_month_offset = 0;
        self.search_focus_pending = false;
        self.open_session(super::panel_menu_session::PanelMenuSource::Dmenu(request));
        self.dmenu_responder = Some(responder);

        effects.push(PanelRuntimeEffect::OpenPanelWindow(PanelOpenRequest {
            origin_window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::DmenuRequest,
            anchor: None,
        }));
        effects.push(PanelRuntimeEffect::RequestSearchFocus);
        effects
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

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn open_request(
        window: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) -> super::PanelOpenRequest {
        super::PanelOpenRequest {
            origin_window: window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content,
            anchor: None,
        }
    }

    fn dmenu_request() -> crate::ipc::protocol::DmenuRequest {
        crate::ipc::protocol::DmenuRequest {
            prompt: Some("Run".into()),
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
            allow_custom: true,
        }
    }

    fn ipc_responder_for_test() -> super::super::messages::IpcResponder {
        let (sender, _receiver) = tokio::sync::oneshot::channel();
        super::super::messages::ipc_responder(sender)
    }

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
    fn opening_a_panel_replaces_the_active_panel() {
        let origin = wid();
        let first_panel = wid();
        let second_panel = wid();
        let mut runtime = PanelRuntime::default();

        let first = runtime.open_panel_with_menus(
            open_request(origin, crate::surface::panel::PanelContent::BarPanel),
            &[],
        );
        assert_eq!(
            first,
            vec![PanelRuntimeEffect::OpenPanelWindow(open_request(
                origin,
                crate::surface::panel::PanelContent::BarPanel
            ))]
        );
        runtime.panel_window_opened(
            first_panel,
            origin,
            crate::surface::panel::PanelContent::BarPanel,
        );

        let second = runtime.open_panel_with_menus(
            open_request(
                origin,
                crate::surface::panel::PanelContent::Menu("main".into()),
            ),
            &[],
        );

        assert_eq!(
            second,
            vec![
                PanelRuntimeEffect::ClosePanelWindow(first_panel),
                PanelRuntimeEffect::OpenPanelWindow(open_request(
                    origin,
                    crate::surface::panel::PanelContent::Menu("main".into())
                )),
                PanelRuntimeEffect::RequestSearchFocus,
            ]
        );
        runtime.panel_window_opened(
            second_panel,
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );
        assert_eq!(runtime.active_panel_window(), Some(second_panel));
    }

    #[test]
    fn toggling_the_same_panel_closes_it() {
        let origin = wid();
        let panel = wid();
        let mut runtime = PanelRuntime::default();
        runtime.panel_window_opened(panel, origin, crate::surface::panel::PanelContent::BarPanel);

        let effects = runtime.toggle_panel_with_menus(
            open_request(origin, crate::surface::panel::PanelContent::BarPanel),
            &[],
        );

        assert_eq!(effects, vec![PanelRuntimeEffect::ClosePanelWindow(panel)]);
        assert_eq!(runtime.active_panel_window(), None);
    }

    #[test]
    fn installed_apps_menu_returns_load_effect_before_opening_panel() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();
        let menus = vec![crate::config::MenuConfig {
            name: "apps".into(),
            prompt: Some("Run".into()),
            show_icons: true,
            source: Some(crate::config::MenuSourceConfig {
                kind: crate::config::MenuSourceKind::InstalledApps,
            }),
            items: Vec::new(),
        }];

        let effects = runtime.open_panel_with_menus(
            open_request(
                origin,
                crate::surface::panel::PanelContent::Menu("apps".into()),
            ),
            &menus,
        );

        assert_eq!(
            effects,
            vec![PanelRuntimeEffect::LoadInstalledAppsThenOpenPanel(
                super::PendingPanelOpen {
                    request: open_request(
                        origin,
                        crate::surface::panel::PanelContent::Menu("apps".into())
                    ),
                    menu_name: "apps".into(),
                }
            )]
        );
        assert_eq!(runtime.active_panel_window(), None);
    }

    #[test]
    fn dmenu_selection_rejects_when_another_request_is_active() {
        let mut runtime = PanelRuntime::default();
        let first = ipc_responder_for_test();
        let second = ipc_responder_for_test();
        assert!(runtime.begin_dmenu_request(first).is_ok());

        let effects = runtime.begin_dmenu_selection(dmenu_request(), second, Some(wid()));

        assert_eq!(effects.len(), 1);
        match &effects[0] {
            PanelRuntimeEffect::SendDmenuResponse {
                response: crate::ipc::protocol::IpcResponse::Rejected { reason },
                ..
            } => assert_eq!(reason, "another dmenu request is already active"),
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[test]
    fn dmenu_selection_rejects_when_no_bar_surface_exists() {
        let mut runtime = PanelRuntime::default();

        let effects =
            runtime.begin_dmenu_selection(dmenu_request(), ipc_responder_for_test(), None);

        assert_eq!(effects.len(), 1);
        match &effects[0] {
            PanelRuntimeEffect::SendDmenuResponse {
                response: crate::ipc::protocol::IpcResponse::Rejected { reason },
                ..
            } => assert_eq!(reason, "no bar surface is available"),
            other => panic!("unexpected effect: {other:?}"),
        }
        assert!(!runtime.has_dmenu_responder());
    }

    #[test]
    fn dmenu_selection_opens_dmenu_panel_and_requests_search_focus() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();

        let effects =
            runtime.begin_dmenu_selection(dmenu_request(), ipc_responder_for_test(), Some(origin));

        assert!(runtime.has_dmenu_responder());
        assert_eq!(
            effects,
            vec![
                PanelRuntimeEffect::OpenPanelWindow(super::PanelOpenRequest {
                    origin_window: origin,
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::DmenuRequest,
                    anchor: None,
                }),
                PanelRuntimeEffect::RequestSearchFocus,
            ]
        );
        assert!(runtime.panel_menu_snapshot().is_some());
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
    fn clock_month_browsing_resets_when_clock_module_panel_opens() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();
        runtime.browse_clock_month(2);

        runtime.open_panel_with_menus(
            super::PanelOpenRequest {
                origin_window: origin,
                origin: crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new(
                    "clock",
                )),
                content: crate::surface::panel::PanelContent::ModulePanel(
                    crate::modules::ModuleId::new("clock"),
                ),
                anchor: None,
            },
            &[],
        );

        assert_eq!(runtime.clock_month_offset(), 0);
    }

    #[test]
    fn clock_month_browsing_moves_and_returns_to_current_month() {
        let mut runtime = PanelRuntime::default();
        runtime.panel_window_opened(
            wid(),
            wid(),
            crate::surface::panel::PanelContent::ModulePanel(crate::modules::ModuleId::new(
                "clock",
            )),
        );

        runtime.browse_clock_month(1);
        runtime.browse_clock_month(1);
        assert_eq!(runtime.clock_month_offset(), 2);

        runtime.browse_clock_month(-1);
        assert_eq!(runtime.clock_month_offset(), 1);

        runtime.reset_clock_month();
        assert_eq!(runtime.clock_month_offset(), 0);
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
