use std::{collections::HashMap, sync::Arc};

use super::selectable_menu_session::{
    SelectableMenuEntry, SelectableMenuSession, SelectableMenuSnapshot, SelectableMenuSource,
    SelectedMenuEntry,
};

pub(super) type PanelOpenRequest = super::foreground::PanelForegroundRequest;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PendingPanelOpen {
    pub(super) request: PanelOpenRequest,
    pub(super) menu_name: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ActivePanel {
    tree: crate::surface::panel::PanelTree,
}

#[derive(Debug, Clone)]
pub(super) enum PanelRuntimeEffect {
    PanelPrepared(crate::surface::panel::PanelTree),
    RequestSearchFocus,
    RequestPanelClose,
    LoadInstalledAppsThenPreparePanel(PendingPanelOpen),
    RunPanelAction(crate::config::PanelAction),
    LaunchInstalledApp(String),
    SendDmenuResponse {
        responder: super::messages::IpcResponder,
        response: crate::ipc::protocol::IpcResponse,
    },
}

#[derive(Debug, Clone)]
pub(super) struct PanelMenuRenderState {
    pub(super) entries: Arc<[SelectableMenuEntry]>,
    pub(super) show_icons: bool,
    pub(super) prompt: Option<String>,
    pub(super) search_query: String,
    pub(super) focused_index: Option<usize>,
    pub(super) can_go_back: bool,
    pub(super) scroll_y: f32,
    pub(super) viewport_height: f32,
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

    fn configured_menu_source(&self, name: &str) -> Option<SelectableMenuSource> {
        self.menus
            .iter()
            .find(|menu| menu.name == name && menu.source.is_none())
            .cloned()
            .map(SelectableMenuSource::Configured)
    }

    fn installed_apps_source(
        &self,
        name: &str,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
    ) -> Option<SelectableMenuSource> {
        self.menus
            .iter()
            .find(|menu| menu.name == name)
            .map(|menu| SelectableMenuSource::InstalledApps {
                prompt: menu.prompt.clone(),
                show_icons: menu.show_icons,
                apps,
            })
    }
}

pub(super) enum PanelRuntimeIntent<'a> {
    UpdateExternalContent(crate::surface::panel::ExternalPanelContent),
    PrepareOpen {
        request: PanelOpenRequest,
        catalog: PanelMenuCatalog<'a>,
        module_detail_sessions:
            HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession>,
    },
    PrepareInstalledAppsLoaded {
        pending: PendingPanelOpen,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
        catalog: PanelMenuCatalog<'a>,
    },
    Clear,
    FocusMove(isize),
    BeginDmenuSelection {
        request: crate::ipc::protocol::DmenuRequest,
        responder: super::messages::IpcResponder,
        panel_request: Option<PanelOpenRequest>,
    },
}

pub(super) enum PanelInteraction {
    SearchChanged(String),
    MenuScrolled { y: f32, viewport_height: f32 },
    FocusIndex(usize),
    RequestSearchFocus,
    SearchFocusSettled,
    Back,
    Escape,
    Activate(Option<usize>),
}

impl PartialEq for PanelRuntimeEffect {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PanelPrepared(left), Self::PanelPrepared(right)) => left == right,
            (Self::RequestSearchFocus, Self::RequestSearchFocus) => true,
            (Self::RequestPanelClose, Self::RequestPanelClose) => true,
            (
                Self::LoadInstalledAppsThenPreparePanel(left),
                Self::LoadInstalledAppsThenPreparePanel(right),
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
    session: Option<SelectableMenuSession>,
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
            PanelRuntimeIntent::UpdateExternalContent(content) => {
                if let Some(panel) = &mut self.active_panel
                    && matches!(
                        panel.tree.content,
                        crate::surface::panel::PanelContent::ExternalProvider(_)
                    )
                {
                    panel.tree.content =
                        crate::surface::panel::PanelContent::ExternalProvider(content);
                }
                Vec::new()
            }
            PanelRuntimeIntent::PrepareOpen {
                request,
                catalog,
                module_detail_sessions,
            } => self.open_panel_with_catalog(request, catalog, module_detail_sessions),
            PanelRuntimeIntent::PrepareInstalledAppsLoaded {
                pending,
                apps,
                catalog,
            } => self.open_panel_after_installed_apps_loaded(pending, apps, catalog),
            PanelRuntimeIntent::Clear => self.clear_with_cancelled_dmenu_response(),
            PanelRuntimeIntent::FocusMove(delta) => {
                self.move_focus(delta);
                Vec::new()
            }
            PanelRuntimeIntent::BeginDmenuSelection {
                request,
                responder,
                panel_request,
            } => self.begin_dmenu_selection(request, responder, panel_request),
        }
    }

    pub(super) fn interact(&mut self, interaction: PanelInteraction) -> Vec<PanelRuntimeEffect> {
        match interaction {
            PanelInteraction::SearchChanged(query) => {
                self.search_changed(query);
                Vec::new()
            }
            PanelInteraction::MenuScrolled { y, viewport_height } => {
                self.set_menu_scrolled(y, viewport_height);
                Vec::new()
            }
            PanelInteraction::FocusIndex(index) => {
                self.focus_index(index);
                Vec::new()
            }
            PanelInteraction::RequestSearchFocus => vec![self.request_search_focus()],
            PanelInteraction::SearchFocusSettled => {
                self.search_focus_pending = false;
                Vec::new()
            }
            PanelInteraction::Back => self.back(),
            PanelInteraction::Escape => self.escape(),
            PanelInteraction::Activate(index) => self.activate(index),
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

    pub(super) fn active_panel_tree(&self) -> Option<&crate::surface::panel::PanelTree> {
        self.active_panel.as_ref().map(|panel| &panel.tree)
    }

    #[cfg(test)]
    pub(super) fn active_panel_tree_for_test(&self) -> Option<&crate::surface::panel::PanelTree> {
        self.active_panel_tree()
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
            return vec![PanelRuntimeEffect::LoadInstalledAppsThenPreparePanel(
                PendingPanelOpen { request, menu_name },
            )];
        }
        let menu_source = if let crate::surface::panel::PanelContent::Menu(name) = &request.content
        {
            catalog.configured_menu_source(name)
        } else {
            None
        };
        self.open_panel_after_source_ready(request, module_detail_sessions, menu_source)
    }

    fn open_panel_after_installed_apps_loaded(
        &mut self,
        pending: PendingPanelOpen,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
        catalog: PanelMenuCatalog<'_>,
    ) -> Vec<PanelRuntimeEffect> {
        let source = catalog.installed_apps_source(&pending.menu_name, apps);
        self.open_panel_after_source_ready(pending.request, HashMap::new(), source)
    }

    fn open_panel_after_source_ready(
        &mut self,
        request: PanelOpenRequest,
        module_detail_sessions: HashMap<
            crate::modules::ModuleId,
            crate::modules::ModuleDetailSession,
        >,
        menu_source: Option<super::panel_menu_session::PanelMenuSource>,
    ) -> Vec<PanelRuntimeEffect> {
        let mut effects = self.finish_interaction();

        self.session = None;
        if let Some(source) = menu_source {
            self.open_session(source);
        }

        self.module_detail_sessions = module_detail_sessions;
        self.menu_scroll_y = 0.0;
        self.menu_viewport_height = 0.0;
        self.search_focus_pending = false;

        let tree = request.tree();
        self.active_panel = Some(ActivePanel { tree: tree.clone() });

        effects.push(PanelRuntimeEffect::PanelPrepared(tree));
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
        panel_request: Option<PanelOpenRequest>,
    ) -> Vec<PanelRuntimeEffect> {
        if self.dmenu_responder.is_some() {
            return vec![PanelRuntimeEffect::SendDmenuResponse {
                responder,
                response: crate::ipc::protocol::IpcResponse::Rejected {
                    reason: "another dmenu request is already active".into(),
                },
            }];
        }

        let Some(panel_request) = panel_request else {
            return vec![PanelRuntimeEffect::SendDmenuResponse {
                responder,
                response: crate::ipc::protocol::IpcResponse::Rejected {
                    reason: "no bar surface is available".into(),
                },
            }];
        };

        let effects = self.open_panel_after_source_ready(
            panel_request,
            HashMap::new(),
            Some(SelectableMenuSource::Dmenu(request)),
        );
        self.dmenu_responder = Some(responder);
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
        if !effects.is_empty() {
            effects.push(PanelRuntimeEffect::RequestPanelClose);
        }
        // Foreground applies the close synchronously. Keep the last presentation
        // available for its inert exit frame; the responder is already consumed.
        effects
    }

    fn clear_with_cancelled_dmenu_response(&mut self) -> Vec<PanelRuntimeEffect> {
        let effects = self.finish_interaction();
        self.clear();
        effects
    }

    /// Retain only presentation data; IPC interaction completes at logical close.
    pub(super) fn finish_interaction(&mut self) -> Vec<PanelRuntimeEffect> {
        self.search_focus_pending = false;
        self.dmenu_responder
            .take()
            .map(|responder| PanelRuntimeEffect::SendDmenuResponse {
                responder,
                response: crate::ipc::protocol::IpcResponse::Cancelled,
            })
            .into_iter()
            .collect()
    }

    fn open_session(&mut self, source: SelectableMenuSource) {
        let session = SelectableMenuSession::open(source);
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
                tree:
                    crate::surface::panel::PanelTree {
                        content: crate::surface::panel::PanelContent::ModulePanel(content_id),
                        ..
                    },
                ..
            }) => content_id == id && self.module_detail_sessions.contains_key(id),
            Some(ActivePanel {
                tree:
                    crate::surface::panel::PanelTree {
                        content: crate::surface::panel::PanelContent::BarPanel,
                        ..
                    },
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

    fn back(&mut self) -> Vec<PanelRuntimeEffect> {
        if self
            .session
            .as_mut()
            .map(super::panel_menu_session::PanelMenuSession::back)
            .unwrap_or(false)
        {
            vec![self.request_search_focus()]
        } else {
            Vec::new()
        }
    }

    fn escape(&mut self) -> Vec<PanelRuntimeEffect> {
        let cleared_search = self
            .session
            .as_mut()
            .map(super::panel_menu_session::PanelMenuSession::clear_search)
            .unwrap_or(false);
        if cleared_search {
            return vec![self.request_search_focus()];
        }

        let back_effects = self.back();
        if !back_effects.is_empty() {
            return back_effects;
        }

        vec![PanelRuntimeEffect::RequestPanelClose]
    }

    fn panel_menu_snapshot(&self) -> Option<SelectableMenuSnapshot> {
        self.session.as_ref().map(|session| session.snapshot())
    }

    #[cfg(test)]
    pub(super) fn panel_menu_snapshot_for_test(&self) -> Option<SelectableMenuSnapshot> {
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
            can_go_back: snapshot
                .as_ref()
                .map(|snapshot| snapshot.can_go_back)
                .unwrap_or(false),
            scroll_y: self.menu_scroll_y,
            viewport_height: self.menu_viewport_height,
        }
    }

    fn active_panel_menu_entries(&self) -> Vec<SelectableMenuEntry> {
        self.panel_menu_snapshot()
            .map(|snapshot| snapshot.entries.to_vec())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(super) fn active_panel_menu_entries_for_test(&self) -> Vec<SelectableMenuEntry> {
        self.active_panel_menu_entries()
    }

    fn activate(&mut self, index: Option<usize>) -> Vec<PanelRuntimeEffect> {
        let activation = match (self.session.as_mut(), index) {
            (Some(session), Some(index)) => session.activate_index(index),
            (Some(session), None) => session.activate_focused(),
            (None, _) => SelectedMenuEntry::None,
        };
        match activation {
            SelectedMenuEntry::ConfiguredPath(path) => self
                .session
                .as_ref()
                .and_then(|session| session.configured_action_for_path(&path))
                .map(|action| {
                    vec![
                        PanelRuntimeEffect::RunPanelAction(action),
                        PanelRuntimeEffect::RequestPanelClose,
                    ]
                })
                .unwrap_or_default(),
            SelectedMenuEntry::InstalledApp(id) => {
                vec![
                    PanelRuntimeEffect::LaunchInstalledApp(id),
                    PanelRuntimeEffect::RequestPanelClose,
                ]
            }
            SelectedMenuEntry::DmenuValue(value) | SelectedMenuEntry::CustomValue(value) => {
                self.complete_dmenu(crate::ipc::protocol::IpcResponse::Selected { value })
            }
            SelectedMenuEntry::SniMenuItem(_) => Vec::new(),
            SelectedMenuEntry::None => Vec::new(),
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
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            screen_size: (1920, 1080),
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

    fn ipc_responder_with_receiver_for_test() -> (
        super::super::messages::IpcResponder,
        tokio::sync::oneshot::Receiver<crate::ipc::protocol::IpcResponse>,
    ) {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        (super::super::messages::ipc_responder(sender), receiver)
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
                        crate::config::ConfiguredCommand::shell_for_test("kitty"),
                    )),
                    children: vec![],
                },
                crate::config::MenuItemConfig {
                    label: "Browser".into(),
                    icon: None,
                    keywords: vec!["web".into()],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::ConfiguredCommand::shell_for_test("firefox"),
                    )),
                    children: vec![],
                },
            ],
        }
    }

    fn menu_with_apps_submenu() -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "apps".into(),
            prompt: Some("Run".into()),
            show_icons: false,
            source: None,
            items: vec![crate::config::MenuItemConfig {
                label: "Apps".into(),
                icon: None,
                keywords: vec![],
                action: None,
                children: vec![crate::config::MenuItemConfig {
                    label: "Browser".into(),
                    icon: None,
                    keywords: vec![],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::ConfiguredCommand::shell_for_test("firefox"),
                    )),
                    children: vec![],
                }],
            }],
        }
    }

    fn empty_catalog() -> PanelMenuCatalog<'static> {
        PanelMenuCatalog::new(&[])
    }

    fn prepare_panel(
        runtime: &mut PanelRuntime,
        request: PanelOpenRequest,
    ) -> Vec<PanelRuntimeEffect> {
        runtime.handle(PanelRuntimeIntent::PrepareOpen {
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

    fn module_panel_request(origin: iced::window::Id, name: &str) -> PanelOpenRequest {
        let id = crate::modules::ModuleId::new(name);
        PanelOpenRequest {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Module(id.clone()),
            content: crate::surface::panel::PanelContent::ModulePanel(id),
            anchor: None,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            screen_size: (1920, 1080),
        }
    }

    #[test]
    fn opening_module_detail_content_stores_module_detail_session() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();

        let request = module_panel_request(origin, "custom");
        let effects = runtime.handle(PanelRuntimeIntent::PrepareOpen {
            request: request.clone(),
            catalog: empty_catalog(),
            module_detail_sessions: detail_sessions("custom", 11),
        });

        assert_eq!(
            effects,
            vec![PanelRuntimeEffect::PanelPrepared(request.tree())]
        );
        assert_eq!(
            runtime.module_detail_session_value::<u32>(&crate::modules::ModuleId::new("custom")),
            Some(&11)
        );
    }

    #[test]
    fn preparing_replacement_panel_replaces_module_detail_session() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();
        prepare_panel(&mut runtime, module_panel_request(origin, "first"));

        let request = module_panel_request(origin, "second");
        let effects = runtime.handle(PanelRuntimeIntent::PrepareOpen {
            request: request.clone(),
            catalog: empty_catalog(),
            module_detail_sessions: detail_sessions("second", 22),
        });

        assert_eq!(
            effects,
            vec![PanelRuntimeEffect::PanelPrepared(request.tree())]
        );
        assert_eq!(
            runtime.module_detail_session_value::<u32>(&crate::modules::ModuleId::new("second")),
            Some(&22)
        );
    }

    #[test]
    fn clearing_open_module_detail_content_discards_module_detail_session() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();
        runtime.handle(PanelRuntimeIntent::PrepareOpen {
            request: module_panel_request(origin, "custom"),
            catalog: empty_catalog(),
            module_detail_sessions: detail_sessions("custom", 33),
        });

        let effects = runtime.handle(PanelRuntimeIntent::Clear);

        assert!(effects.is_empty());
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
        runtime.handle(PanelRuntimeIntent::PrepareOpen {
            request: module_panel_request(origin, "custom"),
            catalog: empty_catalog(),
            module_detail_sessions: detail_sessions("custom", 55),
        });
        runtime.clear();

        runtime.handle(PanelRuntimeIntent::PrepareOpen {
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

        let request = module_panel_request(origin, "simple");
        let effects = runtime.handle(PanelRuntimeIntent::PrepareOpen {
            request: request.clone(),
            catalog: empty_catalog(),
            module_detail_sessions: HashMap::new(),
        });

        assert_eq!(
            effects,
            vec![PanelRuntimeEffect::PanelPrepared(request.tree())]
        );
        assert!(
            runtime
                .module_detail_session_for(&crate::modules::ModuleId::new("simple"))
                .is_none()
        );
    }

    #[test]
    fn preparing_menu_panel_requests_search_focus() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();
        let request = open_request(
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );

        assert_eq!(
            prepare_panel(&mut runtime, request.clone()),
            vec![
                PanelRuntimeEffect::PanelPrepared(request.tree()),
                PanelRuntimeEffect::RequestSearchFocus,
            ]
        );
    }

    #[test]
    fn preparing_panel_stores_active_panel_tree_without_window_identity() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();
        let request = open_request(origin, crate::surface::panel::PanelContent::BarPanel);

        let effects = prepare_panel(&mut runtime, request.clone());

        assert_eq!(
            effects,
            vec![PanelRuntimeEffect::PanelPrepared(request.tree())]
        );
        assert_eq!(runtime.active_panel_tree_for_test(), Some(&request.tree()));
    }

    #[test]
    fn installed_apps_menu_returns_load_effect_before_preparing_panel() {
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

        let effects = runtime.handle(PanelRuntimeIntent::PrepareOpen {
            request: open_request(
                origin,
                crate::surface::panel::PanelContent::Menu("apps".into()),
            ),
            catalog: PanelMenuCatalog::new(&menus),
            module_detail_sessions: HashMap::new(),
        });

        assert_eq!(
            effects,
            vec![PanelRuntimeEffect::LoadInstalledAppsThenPreparePanel(
                super::PendingPanelOpen {
                    request: open_request(
                        origin,
                        crate::surface::panel::PanelContent::Menu("apps".into())
                    ),
                    menu_name: "apps".into(),
                }
            )]
        );
        assert!(runtime.active_panel_tree_for_test().is_none());
    }

    #[test]
    fn dmenu_selection_rejects_when_another_request_is_active() {
        let mut runtime = PanelRuntime::default();
        let first = ipc_responder_for_test();
        let second = ipc_responder_for_test();
        assert!(runtime.begin_dmenu_request(first).is_ok());

        let effects = runtime.begin_dmenu_selection(
            dmenu_request(),
            second,
            Some(open_request(
                wid(),
                crate::surface::panel::PanelContent::DmenuRequest,
            )),
        );

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
    fn dmenu_selection_prepares_dmenu_panel_and_requests_search_focus() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();
        let request = open_request(origin, crate::surface::panel::PanelContent::DmenuRequest);

        let effects = runtime.begin_dmenu_selection(
            dmenu_request(),
            ipc_responder_for_test(),
            Some(request.clone()),
        );

        assert!(runtime.has_dmenu_responder());
        assert_eq!(
            effects,
            vec![
                PanelRuntimeEffect::PanelPrepared(request.tree()),
                PanelRuntimeEffect::RequestSearchFocus,
            ]
        );
        assert!(runtime.panel_menu_snapshot().is_some());
    }

    #[test]
    fn clearing_runtime_cancels_active_dmenu_request() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();
        let request = open_request(origin, crate::surface::panel::PanelContent::DmenuRequest);
        let (responder, _receiver) = ipc_responder_with_receiver_for_test();
        runtime.begin_dmenu_selection(dmenu_request(), responder, Some(request));

        let effects = runtime.handle(PanelRuntimeIntent::Clear);

        assert!(!runtime.has_dmenu_responder());
        assert!(matches!(
            effects.as_slice(),
            [PanelRuntimeEffect::SendDmenuResponse {
                response: crate::ipc::protocol::IpcResponse::Cancelled,
                ..
            }]
        ));
    }

    #[test]
    fn clear_resets_session_scroll_focus_and_dmenu_state() {
        let mut runtime = PanelRuntime::default();
        runtime.set_menu_scrolled(120.0, 300.0);
        runtime.interact(PanelInteraction::RequestSearchFocus);
        runtime.clear();

        assert_eq!(runtime.menu_scroll_y(), 0.0);
        assert_eq!(runtime.menu_viewport_height(), 0.0);
        assert!(!runtime.search_focus_pending());
        assert!(!runtime.has_dmenu_responder());
    }

    #[test]
    fn escape_without_first_escape_behavior_requests_panel_close() {
        let origin = wid();
        let mut runtime = PanelRuntime::default();
        prepare_panel(
            &mut runtime,
            open_request(origin, crate::surface::panel::PanelContent::BarPanel),
        );

        assert_eq!(
            runtime.interact(PanelInteraction::Escape),
            vec![PanelRuntimeEffect::RequestPanelClose]
        );
    }

    #[test]
    fn escape_in_configured_submenu_returns_to_parent_menu() {
        let mut runtime = PanelRuntime::default();
        runtime.open_session(
            super::super::panel_menu_session::PanelMenuSource::Configured(menu_with_apps_submenu()),
        );
        runtime.interact(PanelInteraction::Activate(Some(0)));

        let effects = runtime.interact(PanelInteraction::Escape);

        assert_eq!(effects, vec![PanelRuntimeEffect::RequestSearchFocus]);
        assert!(runtime.search_focus_pending());
        let snapshot = runtime.panel_menu_snapshot().expect("snapshot");
        assert!(!snapshot.can_go_back);
        assert_eq!(snapshot.entries[0].label, "Apps");
    }

    #[test]
    fn back_in_configured_submenu_requests_search_refocus() {
        let mut runtime = PanelRuntime::default();
        runtime.open_session(
            super::super::panel_menu_session::PanelMenuSource::Configured(menu_with_apps_submenu()),
        );
        runtime.interact(PanelInteraction::Activate(Some(0)));

        let effects = runtime.interact(PanelInteraction::Back);

        assert_eq!(effects, vec![PanelRuntimeEffect::RequestSearchFocus]);
        assert!(runtime.search_focus_pending());
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
    fn activate_focused_returns_panel_action_and_close_request_effects() {
        let mut runtime = PanelRuntime::default();
        runtime.open_session(
            super::super::panel_menu_session::PanelMenuSource::Configured(menu_with_two_entries()),
        );

        let effects = runtime.activate(None);

        assert!(matches!(
            effects.as_slice(),
            [
                PanelRuntimeEffect::RunPanelAction(crate::config::PanelAction::Command(_)),
                PanelRuntimeEffect::RequestPanelClose
            ]
        ));
    }

    #[test]
    fn configured_application_launch_requests_session_launch_and_closes_panel() {
        let mut menu = menu_with_two_entries();
        menu.items[0].action = Some(crate::config::PanelAction::Launch(vec!["kitty".into()]));
        let mut runtime = PanelRuntime::default();
        runtime.open_session(super::super::panel_menu_session::PanelMenuSource::Configured(menu));
        assert_eq!(
            runtime.activate(None),
            vec![
                PanelRuntimeEffect::RunPanelAction(crate::config::PanelAction::Launch(vec![
                    "kitty".into()
                ])),
                PanelRuntimeEffect::RequestPanelClose,
            ]
        );
    }

    #[test]
    fn activating_installed_app_returns_launch_and_close_request_effects() {
        let mut runtime = PanelRuntime::default();
        runtime.open_session(
            super::super::panel_menu_session::PanelMenuSource::InstalledApps {
                prompt: Some("Run".into()),
                show_icons: false,
                apps: vec![crate::desktop_apps::DesktopApplication {
                    id: "alpha.desktop".into(),
                    name: "Alpha".into(),
                    generic_name: None,
                    comment: None,
                    keywords: Vec::new(),
                    icon: None,
                    desktop_file: "fixture.desktop".into(),
                    dbus_activatable: false,
                }],
            },
        );

        let effects = runtime.activate(Some(0));

        assert_eq!(
            effects,
            vec![
                PanelRuntimeEffect::LaunchInstalledApp("alpha.desktop".into()),
                PanelRuntimeEffect::RequestPanelClose,
            ]
        );
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
            [
                PanelRuntimeEffect::SendDmenuResponse { .. },
                PanelRuntimeEffect::RequestPanelClose
            ]
        ));
        assert!(second.is_empty());
    }
}
