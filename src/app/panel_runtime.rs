use std::{collections::HashMap, sync::Arc};

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
    origin: crate::surface::panel::PanelOrigin,
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
    SendDmenuResponse {
        responder: super::messages::IpcResponder,
        response: crate::ipc::protocol::IpcResponse,
    },
}

#[derive(Debug, Clone)]
pub(super) struct PanelMenuRenderState {
    pub(super) entries: Arc<[super::panel_menu_session::PanelMenuEntry]>,
    pub(super) show_icons: bool,
    pub(super) prompt: Option<String>,
    pub(super) search_query: String,
    pub(super) focused_index: Option<usize>,
    pub(super) scroll_y: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PanelMenuCatalog<'a> {
    menus: &'a [crate::config::MenuConfig],
}

impl<'a> PanelMenuCatalog<'a> {
    pub(super) fn new(menus: &'a [crate::config::MenuConfig]) -> Self {
        Self { menus }
    }

    fn source_kind_for(&self, name: &str) -> Option<&crate::config::MenuSourceKind> {
        self.menus
            .iter()
            .find(|menu| menu.name == name)
            .and_then(|menu| menu.source.as_ref())
            .map(|source| &source.kind)
    }

    fn is_installed_apps_menu(&self, name: &str) -> bool {
        self.source_kind_for(name)
            .is_some_and(|kind| matches!(kind, crate::config::MenuSourceKind::InstalledApps))
    }

    fn configured_menu_source(
        &self,
        name: &str,
    ) -> Option<super::panel_menu_session::PanelMenuSource> {
        self.menus
            .iter()
            .find(|menu| menu.name == name && menu.source.is_none())
            .cloned()
            .map(super::panel_menu_session::PanelMenuSource::Configured)
    }

    fn installed_apps_source(
        &self,
        name: &str,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
    ) -> Option<super::panel_menu_session::PanelMenuSource> {
        self.menus
            .iter()
            .find(|menu| menu.name == name)
            .map(
                |menu| super::panel_menu_session::PanelMenuSource::InstalledApps {
                    prompt: menu.prompt.clone(),
                    show_icons: menu.show_icons,
                    apps,
                },
            )
    }
}

pub(super) enum PanelRuntimeIntent<'a> {
    Open {
        request: PanelOpenRequest,
        catalog: PanelMenuCatalog<'a>,
        module_detail_sessions:
            HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession>,
    },
    Toggle {
        request: PanelOpenRequest,
        catalog: PanelMenuCatalog<'a>,
        module_detail_sessions:
            HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession>,
    },
    PanelWindowOpened {
        window: iced::window::Id,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        catalog: PanelMenuCatalog<'a>,
    },
    InstalledAppsLoaded {
        pending: PendingPanelOpen,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
        catalog: PanelMenuCatalog<'a>,
    },
    SearchChanged(String),
    MenuScrolled {
        y: f32,
        viewport_height: f32,
    },
    FocusIndex(usize),
    RequestSearchFocus,
    SearchFocusSettled,
    FocusMove(isize),
    Back,
    Escape,
    Activate(Option<usize>),
    CancelDmenu,
    BeginDmenuSelection {
        request: crate::ipc::protocol::DmenuRequest,
        responder: super::messages::IpcResponder,
        origin_window: Option<iced::window::Id>,
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

#[derive(Default)]
pub(super) struct PanelRuntime {
    active_panel: Option<ActivePanel>,
    session: Option<super::panel_menu_session::PanelMenuSession>,
    module_detail_sessions: HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession>,
    menu_scroll_y: f32,
    menu_viewport_height: f32,
    search_focus_pending: bool,
    dmenu_responder: Option<super::messages::IpcResponder>,
    #[cfg(test)]
    pub(super) installed_apps_for_test: Option<Vec<crate::desktop_apps::DesktopApplication>>,
    #[cfg(test)]
    pub(super) launched_installed_app_ids_for_test: Vec<String>,
}

impl PanelRuntime {
    pub(super) fn handle(&mut self, intent: PanelRuntimeIntent<'_>) -> Vec<PanelRuntimeEffect> {
        match intent {
            PanelRuntimeIntent::Open {
                request,
                catalog,
                module_detail_sessions,
            } => self.open_panel_with_catalog(request, catalog, module_detail_sessions),
            PanelRuntimeIntent::Toggle {
                request,
                catalog,
                module_detail_sessions,
            } => self.toggle_panel_with_catalog(request, catalog, module_detail_sessions),
            PanelRuntimeIntent::PanelWindowOpened {
                window,
                origin_window,
                origin,
                content,
                catalog,
            } => {
                self.panel_window_opened(window, origin_window, origin, content, catalog);
                Vec::new()
            }
            PanelRuntimeIntent::InstalledAppsLoaded {
                pending,
                apps,
                catalog,
            } => self.open_panel_after_installed_apps_loaded(pending, apps, catalog),
            PanelRuntimeIntent::SearchChanged(query) => {
                self.search_changed(query);
                Vec::new()
            }
            PanelRuntimeIntent::MenuScrolled { y, viewport_height } => {
                self.set_menu_scrolled(y, viewport_height);
                Vec::new()
            }
            PanelRuntimeIntent::FocusIndex(index) => {
                self.focus_index(index);
                Vec::new()
            }
            PanelRuntimeIntent::RequestSearchFocus => vec![self.request_search_focus()],
            PanelRuntimeIntent::SearchFocusSettled => {
                self.search_focus_pending = false;
                Vec::new()
            }
            PanelRuntimeIntent::FocusMove(delta) => {
                self.move_focus(delta);
                Vec::new()
            }
            PanelRuntimeIntent::Back => {
                self.back();
                Vec::new()
            }
            PanelRuntimeIntent::Escape => self.escape(),
            PanelRuntimeIntent::Activate(index) => self.activate(index),
            PanelRuntimeIntent::CancelDmenu => {
                self.cancel_dmenu(crate::ipc::protocol::IpcResponse::Cancelled)
            }
            PanelRuntimeIntent::BeginDmenuSelection {
                request,
                responder,
                origin_window,
            } => self.begin_dmenu_selection(request, responder, origin_window),
        }
    }

    pub(super) fn clear(&mut self) {
        self.active_panel = None;
        self.session = None;
        self.module_detail_sessions.clear();
        self.menu_scroll_y = 0.0;
        self.menu_viewport_height = 0.0;
        self.search_focus_pending = false;
        self.dmenu_responder = None;
    }

    fn active_panel_window(&self) -> Option<iced::window::Id> {
        self.active_panel.as_ref().map(|panel| panel.window)
    }

    pub(super) fn would_toggle_close(&self, request: &PanelOpenRequest) -> bool {
        self.active_panel.as_ref().is_some_and(|panel| {
            panel.origin_window == request.origin_window && panel.content == request.content
        }) && self.active_panel_window().is_some()
    }

    fn panel_window_opened(
        &mut self,
        window: iced::window::Id,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        catalog: PanelMenuCatalog<'_>,
    ) {
        if let crate::surface::panel::PanelContent::Menu(name) = &content
            && let Some(source) = catalog.configured_menu_source(name)
        {
            self.open_session(source);
        }
        self.active_panel = Some(ActivePanel {
            window,
            origin_window,
            origin,
            content,
        });
    }

    fn open_panel_with_catalog(
        &mut self,
        request: PanelOpenRequest,
        catalog: PanelMenuCatalog<'_>,
        module_detail_sessions: HashMap<
            crate::modules::ModuleId,
            crate::modules::ModuleDetailSession,
        >,
    ) -> Vec<PanelRuntimeEffect> {
        if let crate::surface::panel::PanelContent::Menu(name) = &request.content
            && catalog.is_installed_apps_menu(name)
        {
            let menu_name = name.clone();
            return vec![PanelRuntimeEffect::LoadInstalledAppsThenOpenPanel(
                PendingPanelOpen { request, menu_name },
            )];
        }
        self.open_panel_after_source_ready(request, module_detail_sessions)
    }

    fn toggle_panel_with_catalog(
        &mut self,
        request: PanelOpenRequest,
        catalog: PanelMenuCatalog<'_>,
        module_detail_sessions: HashMap<
            crate::modules::ModuleId,
            crate::modules::ModuleDetailSession,
        >,
    ) -> Vec<PanelRuntimeEffect> {
        if self.would_toggle_close(&request)
            && let Some(window) = self.active_panel_window()
        {
            self.active_panel = None;
            self.session = None;
            self.module_detail_sessions.clear();
            self.menu_scroll_y = 0.0;
            self.menu_viewport_height = 0.0;
            self.search_focus_pending = false;
            return vec![PanelRuntimeEffect::ClosePanelWindow(window)];
        }
        self.open_panel_with_catalog(request, catalog, module_detail_sessions)
    }

    fn open_panel_after_installed_apps_loaded(
        &mut self,
        pending: PendingPanelOpen,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
        catalog: PanelMenuCatalog<'_>,
    ) -> Vec<PanelRuntimeEffect> {
        let effects = self.open_panel_after_source_ready(pending.request, HashMap::new());
        if let Some(source) = catalog.installed_apps_source(&pending.menu_name, apps) {
            self.open_session(source);
        }
        effects
    }

    fn open_panel_after_source_ready(
        &mut self,
        request: PanelOpenRequest,
        module_detail_sessions: HashMap<
            crate::modules::ModuleId,
            crate::modules::ModuleDetailSession,
        >,
    ) -> Vec<PanelRuntimeEffect> {
        let mut effects = Vec::new();
        if let Some(window) = self.active_panel_window() {
            effects.push(PanelRuntimeEffect::ClosePanelWindow(window));
        }
        self.active_panel = None;
        self.session = None;
        self.module_detail_sessions = module_detail_sessions;
        self.menu_scroll_y = 0.0;
        self.menu_viewport_height = 0.0;
        self.search_focus_pending = false;
        effects.push(PanelRuntimeEffect::OpenPanelWindow(request.clone()));
        if matches!(
            request.content,
            crate::surface::panel::PanelContent::Menu(_)
                | crate::surface::panel::PanelContent::DmenuRequest
        ) {
            effects.push(self.request_search_focus());
        }
        effects
    }

    #[cfg(test)]
    pub(super) fn menu_scroll_y(&self) -> f32 {
        self.menu_scroll_y
    }

    #[cfg(test)]
    fn menu_viewport_height(&self) -> f32 {
        self.menu_viewport_height
    }

    pub(super) fn scroll_focused_menu_into_view(
        &mut self,
        row_extent: f32,
        default_viewport_height: f32,
    ) -> Option<f32> {
        let y = self.menu_scroll_target_for_focused(row_extent, default_viewport_height)?;
        self.menu_scroll_y = y.max(0.0);
        Some(self.menu_scroll_y)
    }

    fn menu_scroll_target_for_focused(
        &self,
        row_extent: f32,
        default_viewport_height: f32,
    ) -> Option<f32> {
        let index = self.focused_index()?;
        let current_top = self.menu_scroll_y.max(0.0);
        let viewport_height = if self.menu_viewport_height > 0.0 {
            self.menu_viewport_height
        } else {
            default_viewport_height
        };
        let current_bottom = current_top + viewport_height;
        let row_top = index as f32 * row_extent;
        let row_bottom = row_top + row_extent;
        if row_top < current_top {
            Some(row_top)
        } else if row_bottom > current_bottom {
            Some(row_bottom - viewport_height)
        } else {
            None
        }
    }

    fn set_menu_scrolled(&mut self, y: f32, viewport_height: f32) {
        self.menu_scroll_y = y.max(0.0);
        self.menu_viewport_height = viewport_height.max(0.0);
    }

    #[cfg(test)]
    pub(super) fn reset_menu_scroll(&mut self) {
        self.menu_scroll_y = 0.0;
    }

    pub(super) fn search_focus_pending(&self) -> bool {
        self.search_focus_pending
    }

    fn request_search_focus(&mut self) -> PanelRuntimeEffect {
        self.search_focus_pending = true;
        PanelRuntimeEffect::RequestSearchFocus
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

    fn begin_dmenu_selection(
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
        self.module_detail_sessions.clear();
        self.menu_scroll_y = 0.0;
        self.menu_viewport_height = 0.0;
        self.search_focus_pending = false;
        self.open_session(super::panel_menu_session::PanelMenuSource::Dmenu(request));
        self.dmenu_responder = Some(responder);

        effects.push(PanelRuntimeEffect::OpenPanelWindow(PanelOpenRequest {
            origin_window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::DmenuRequest,
            anchor: None,
        }));
        effects.push(self.request_search_focus());
        effects
    }

    fn complete_dmenu(
        &mut self,
        response: crate::ipc::protocol::IpcResponse,
    ) -> Vec<PanelRuntimeEffect> {
        let mut effects: Vec<_> = self
            .dmenu_responder
            .take()
            .map(|responder| PanelRuntimeEffect::SendDmenuResponse {
                responder,
                response,
            })
            .into_iter()
            .collect();
        if let Some(window) = self.active_panel_window() {
            effects.push(PanelRuntimeEffect::ClosePanelWindow(window));
        }
        self.clear();
        effects
    }

    fn cancel_dmenu(
        &mut self,
        response: crate::ipc::protocol::IpcResponse,
    ) -> Vec<PanelRuntimeEffect> {
        self.dmenu_responder
            .take()
            .map(|responder| PanelRuntimeEffect::SendDmenuResponse {
                responder,
                response,
            })
            .into_iter()
            .collect()
    }

    fn open_session(&mut self, source: super::panel_menu_session::PanelMenuSource) {
        let session = super::panel_menu_session::PanelMenuSession::open(source);
        let _ = session.snapshot();
        self.session = Some(session);
    }

    pub(super) fn module_detail_session_for(
        &self,
        id: &crate::modules::ModuleId,
    ) -> Option<&crate::modules::ModuleDetailSession> {
        self.module_detail_sessions.get(id)
    }

    pub(super) fn module_detail_session_for_mut(
        &mut self,
        id: &crate::modules::ModuleId,
    ) -> Option<&mut crate::modules::ModuleDetailSession> {
        self.module_detail_sessions.get_mut(id)
    }

    pub(super) fn active_module_panel_contains(&self, id: &crate::modules::ModuleId) -> bool {
        match self.active_panel.as_ref() {
            Some(ActivePanel {
                content: crate::surface::panel::PanelContent::ModulePanel(content_id),
                ..
            }) => content_id == id && self.module_detail_sessions.contains_key(id),
            Some(ActivePanel {
                content: crate::surface::panel::PanelContent::BarPanel,
                ..
            }) => self.module_detail_sessions.contains_key(id),
            _ => false,
        }
    }

    #[cfg(test)]
    fn module_detail_session_value<T: 'static>(&self, id: &crate::modules::ModuleId) -> Option<&T> {
        self.module_detail_sessions
            .get(id)
            .and_then(|session| session.downcast_ref::<T>())
    }

    fn search_changed(&mut self, query: String) {
        if let Some(session) = self.session.as_mut() {
            session.set_search_query(query);
        }
        self.search_focus_pending = false;
        self.menu_scroll_y = 0.0;
    }

    fn focus_index(&mut self, index: usize) {
        if let Some(session) = self.session.as_mut() {
            session.focus_index(index);
        }
        self.search_focus_pending = false;
    }

    fn move_focus(&mut self, delta: isize) -> Option<usize> {
        if self.active_panel_menu_entries().is_empty() {
            self.menu_scroll_y = 0.0;
            return None;
        }
        let session = self.session.as_mut()?;
        session.move_focus(delta);
        session.focused_index()
    }

    fn focused_index(&self) -> Option<usize> {
        self.session
            .as_ref()
            .and_then(|session| session.focused_index())
    }

    #[cfg(test)]
    pub(super) fn focused_index_for_test(&self) -> Option<usize> {
        self.focused_index()
    }

    fn back(&mut self) {
        if let Some(session) = self.session.as_mut() {
            session.back();
        }
    }

    fn escape(&mut self) -> Vec<PanelRuntimeEffect> {
        let cleared_search = self
            .session
            .as_mut()
            .map(super::panel_menu_session::PanelMenuSession::clear_search)
            .unwrap_or(false);
        if cleared_search {
            vec![self.request_search_focus()]
        } else {
            Vec::new()
        }
    }

    fn panel_menu_snapshot(&self) -> Option<super::panel_menu_session::PanelMenuSnapshot> {
        self.session.as_ref().map(|session| session.snapshot())
    }

    #[cfg(test)]
    pub(super) fn panel_menu_snapshot_for_test(
        &self,
    ) -> Option<super::panel_menu_session::PanelMenuSnapshot> {
        self.panel_menu_snapshot()
    }

    pub(super) fn panel_menu_render_state(&self) -> PanelMenuRenderState {
        let snapshot = self.panel_menu_snapshot();
        PanelMenuRenderState {
            entries: snapshot
                .as_ref()
                .map(|snapshot| snapshot.entries.clone())
                .unwrap_or_else(|| Arc::from([])),
            show_icons: snapshot
                .as_ref()
                .map(|snapshot| snapshot.show_icons)
                .unwrap_or(false),
            prompt: snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.prompt.clone()),
            search_query: snapshot
                .as_ref()
                .map(|snapshot| snapshot.search_query.clone())
                .unwrap_or_default(),
            focused_index: snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.focused_index),
            scroll_y: self.menu_scroll_y,
        }
    }

    fn active_panel_menu_entries(&self) -> Vec<super::panel_menu_session::PanelMenuEntry> {
        self.panel_menu_snapshot()
            .map(|snapshot| snapshot.entries.to_vec())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(super) fn active_panel_menu_entries_for_test(
        &self,
    ) -> Vec<super::panel_menu_session::PanelMenuEntry> {
        self.active_panel_menu_entries()
    }

    fn activate(&mut self, index: Option<usize>) -> Vec<PanelRuntimeEffect> {
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
                self.complete_dmenu(response)
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

    fn empty_catalog() -> PanelMenuCatalog<'static> {
        PanelMenuCatalog::new(&[])
    }

    fn open_panel(
        runtime: &mut PanelRuntime,
        request: PanelOpenRequest,
    ) -> Vec<PanelRuntimeEffect> {
        runtime.handle(PanelRuntimeIntent::Open {
            request,
            catalog: empty_catalog(),
            module_detail_sessions: HashMap::new(),
        })
    }

    fn detail_sessions(
        id: &str,
        value: u32,
    ) -> HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession> {
        [(
            crate::modules::ModuleId::new(id),
            Box::new(value) as crate::modules::ModuleDetailSession,
        )]
        .into_iter()
        .collect()
    }

    fn record_panel_window_opened(
        runtime: &mut PanelRuntime,
        window: iced::window::Id,
        origin_window: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) {
        runtime.handle(PanelRuntimeIntent::PanelWindowOpened {
            window,
            origin_window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content,
            catalog: empty_catalog(),
        });
    }

    fn module_panel_request(origin: iced::window::Id, name: &str) -> PanelOpenRequest {
        let id = crate::modules::ModuleId::new(name);
        PanelOpenRequest {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Module(id.clone()),
            content: crate::surface::panel::PanelContent::ModulePanel(id),
            anchor: None,
        }
    }

    #[test]
    fn opening_module_detail_content_stores_module_detail_session() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();

        let effects = runtime.handle(PanelRuntimeIntent::Open {
            request: module_panel_request(origin, "custom"),
            catalog: empty_catalog(),
            module_detail_sessions: detail_sessions("custom", 11),
        });

        assert_eq!(
            effects,
            vec![PanelRuntimeEffect::OpenPanelWindow(module_panel_request(
                origin, "custom"
            ))]
        );
        assert_eq!(
            runtime.module_detail_session_value::<u32>(&crate::modules::ModuleId::new("custom")),
            Some(&11)
        );
    }

    #[test]
    fn replacing_panel_replaces_module_detail_session() {
        let origin = wid();
        let first_panel = wid();
        let mut runtime = PanelRuntime::default();
        open_panel(&mut runtime, module_panel_request(origin, "first"));
        record_panel_window_opened(
            &mut runtime,
            first_panel,
            origin,
            crate::surface::panel::PanelContent::ModulePanel(crate::modules::ModuleId::new(
                "first",
            )),
        );

        let effects = runtime.handle(PanelRuntimeIntent::Open {
            request: module_panel_request(origin, "second"),
            catalog: empty_catalog(),
            module_detail_sessions: detail_sessions("second", 22),
        });

        assert_eq!(
            effects,
            vec![
                PanelRuntimeEffect::ClosePanelWindow(first_panel),
                PanelRuntimeEffect::OpenPanelWindow(module_panel_request(origin, "second")),
            ]
        );
        assert_eq!(
            runtime.module_detail_session_value::<u32>(&crate::modules::ModuleId::new("second")),
            Some(&22)
        );
    }

    #[test]
    fn toggling_open_module_detail_content_discards_module_detail_session() {
        let origin = wid();
        let panel = wid();
        let mut runtime = PanelRuntime::default();
        runtime.handle(PanelRuntimeIntent::Open {
            request: module_panel_request(origin, "custom"),
            catalog: empty_catalog(),
            module_detail_sessions: detail_sessions("custom", 33),
        });
        record_panel_window_opened(
            &mut runtime,
            panel,
            origin,
            crate::surface::panel::PanelContent::ModulePanel(crate::modules::ModuleId::new(
                "custom",
            )),
        );

        let effects = runtime.handle(PanelRuntimeIntent::Toggle {
            request: module_panel_request(origin, "custom"),
            catalog: empty_catalog(),
            module_detail_sessions: detail_sessions("custom", 44),
        });

        assert_eq!(effects, vec![PanelRuntimeEffect::ClosePanelWindow(panel)]);
        assert!(
            runtime
                .module_detail_session_for(&crate::modules::ModuleId::new("custom"))
                .is_none()
        );
    }

    #[test]
    fn reopening_same_module_detail_content_uses_fresh_module_detail_session() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();
        runtime.handle(PanelRuntimeIntent::Open {
            request: module_panel_request(origin, "custom"),
            catalog: empty_catalog(),
            module_detail_sessions: detail_sessions("custom", 55),
        });
        runtime.clear();

        runtime.handle(PanelRuntimeIntent::Open {
            request: module_panel_request(origin, "custom"),
            catalog: empty_catalog(),
            module_detail_sessions: detail_sessions("custom", 66),
        });

        assert_eq!(
            runtime.module_detail_session_value::<u32>(&crate::modules::ModuleId::new("custom")),
            Some(&66)
        );
    }

    #[test]
    fn modules_without_custom_detail_session_open_with_existing_fallback() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();

        let effects = runtime.handle(PanelRuntimeIntent::Open {
            request: module_panel_request(origin, "simple"),
            catalog: empty_catalog(),
            module_detail_sessions: HashMap::new(),
        });

        assert_eq!(
            effects,
            vec![PanelRuntimeEffect::OpenPanelWindow(module_panel_request(
                origin, "simple"
            ))]
        );
        assert!(
            runtime
                .module_detail_session_for(&crate::modules::ModuleId::new("simple"))
                .is_none()
        );
    }

    #[test]
    fn opening_a_panel_replaces_the_active_panel() {
        let origin = wid();
        let first_panel = wid();
        let second_panel = wid();
        let mut runtime = PanelRuntime::default();

        let first = open_panel(
            &mut runtime,
            open_request(origin, crate::surface::panel::PanelContent::BarPanel),
        );
        assert_eq!(
            first,
            vec![PanelRuntimeEffect::OpenPanelWindow(open_request(
                origin,
                crate::surface::panel::PanelContent::BarPanel
            ))]
        );
        record_panel_window_opened(
            &mut runtime,
            first_panel,
            origin,
            crate::surface::panel::PanelContent::BarPanel,
        );

        let second = open_panel(
            &mut runtime,
            open_request(
                origin,
                crate::surface::panel::PanelContent::Menu("main".into()),
            ),
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
        record_panel_window_opened(
            &mut runtime,
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
        record_panel_window_opened(
            &mut runtime,
            panel,
            origin,
            crate::surface::panel::PanelContent::BarPanel,
        );

        let effects = runtime.handle(PanelRuntimeIntent::Toggle {
            request: open_request(origin, crate::surface::panel::PanelContent::BarPanel),
            catalog: empty_catalog(),
            module_detail_sessions: HashMap::new(),
        });

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

        let effects = runtime.handle(PanelRuntimeIntent::Open {
            request: open_request(
                origin,
                crate::surface::panel::PanelContent::Menu("apps".into()),
            ),
            catalog: PanelMenuCatalog::new(&menus),
            module_detail_sessions: HashMap::new(),
        });

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
        runtime.handle(PanelRuntimeIntent::RequestSearchFocus);
        runtime.clear();

        assert_eq!(runtime.menu_scroll_y(), 0.0);
        assert_eq!(runtime.menu_viewport_height(), 0.0);
        assert!(!runtime.search_focus_pending());
        assert!(!runtime.has_dmenu_responder());
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
    fn focused_menu_scroll_target_is_none_when_entry_is_visible() {
        let mut runtime = PanelRuntime::default();
        runtime.open_session(
            super::super::panel_menu_session::PanelMenuSource::Configured(menu_with_two_entries()),
        );
        runtime.focus_index(1);
        runtime.set_menu_scrolled(0.0, 100.0);

        assert_eq!(runtime.menu_scroll_target_for_focused(40.0, 100.0), None);
    }

    #[test]
    fn focused_menu_scroll_target_reveals_entry_below_view() {
        let mut runtime = PanelRuntime::default();
        runtime.open_session(
            super::super::panel_menu_session::PanelMenuSource::Configured(menu_with_two_entries()),
        );
        runtime.focus_index(1);
        runtime.set_menu_scrolled(0.0, 50.0);

        assert_eq!(
            runtime.menu_scroll_target_for_focused(40.0, 100.0),
            Some(30.0)
        );
    }

    #[test]
    fn focused_menu_scroll_target_reveals_entry_above_view() {
        let mut runtime = PanelRuntime::default();
        runtime.open_session(
            super::super::panel_menu_session::PanelMenuSource::Configured(menu_with_two_entries()),
        );
        runtime.focus_index(0);
        runtime.set_menu_scrolled(40.0, 50.0);

        assert_eq!(
            runtime.menu_scroll_target_for_focused(40.0, 100.0),
            Some(0.0)
        );
    }

    #[test]
    fn scroll_focused_menu_into_view_updates_runtime_scroll() {
        let mut runtime = PanelRuntime::default();
        runtime.open_session(
            super::super::panel_menu_session::PanelMenuSource::Configured(menu_with_two_entries()),
        );
        runtime.focus_index(1);
        runtime.set_menu_scrolled(0.0, 50.0);

        assert_eq!(
            runtime.scroll_focused_menu_into_view(40.0, 100.0),
            Some(30.0)
        );
        assert_eq!(runtime.menu_scroll_y(), 30.0);
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
        let first = runtime.complete_dmenu(crate::ipc::protocol::IpcResponse::Selected {
            value: "firefox".into(),
        });
        let second = runtime.complete_dmenu(crate::ipc::protocol::IpcResponse::Selected {
            value: "firefox".into(),
        });

        assert!(matches!(
            first.as_slice(),
            [PanelRuntimeEffect::SendDmenuResponse { .. }]
        ));
        assert!(second.is_empty());
    }
}
