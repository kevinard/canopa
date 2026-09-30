use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
mod dismissal;
mod motion;

pub(super) struct PanelOpenContext<'a> {
    menus: &'a [crate::config::MenuConfig],
    module_detail_sessions: HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession>,
}

impl<'a> PanelOpenContext<'a> {
    pub(super) fn new(
        menus: &'a [crate::config::MenuConfig],
        module_detail_sessions: HashMap<
            crate::modules::ModuleId,
            crate::modules::ModuleDetailSession,
        >,
    ) -> Self {
        Self {
            menus,
            module_detail_sessions,
        }
    }

    #[cfg(test)]
    fn empty() -> PanelOpenContext<'static> {
        PanelOpenContext {
            menus: &[],
            module_detail_sessions: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PanelForegroundRequest {
    pub(super) origin_window: iced::window::Id,
    pub(super) origin: crate::surface::panel::PanelOrigin,
    pub(super) content: crate::surface::panel::PanelContent,
    pub(super) anchor: Option<iced::Rectangle>,
    pub(super) origin_bar_name: String,
    pub(super) origin_output_name: String,
    pub(super) screen_size: (u32, u32),
}

impl PanelForegroundRequest {
    pub(super) fn tree(&self) -> crate::surface::panel::PanelTree {
        crate::surface::panel::PanelTree {
            origin: self.origin.clone(),
            content: self.content.clone(),
            origin_window: self.origin_window,
            origin_bar_name: self.origin_bar_name.clone(),
            origin_output_name: self.origin_output_name.clone(),
            anchor: self.anchor,
            screen_size: self.screen_size,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct MenuPopupMeta {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
    pub(super) tree: crate::surface::menu_popup::MenuTree,
    pub(super) origin: super::surfaces::BarSurfaceIdentity,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct MenuForegroundRequest {
    pub(super) meta: MenuPopupMeta,
    pub(super) search_enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ForegroundRoleSnapshot {
    roles: HashMap<iced::window::Id, super::surface_input_router::WindowRole>,
}

impl ForegroundRoleSnapshot {
    #[cfg(test)]
    pub(super) fn roles_for_test_dismissal(&self) -> iced::window::Id {
        *self
            .roles
            .iter()
            .find(|(_, role)| **role == super::surface_input_router::WindowRole::Dismissal)
            .unwrap()
            .0
    }

    pub(super) fn role_for(
        &self,
        window: Option<iced::window::Id>,
    ) -> Option<super::surface_input_router::WindowRole> {
        window.and_then(|window| self.roles.get(&window).copied())
    }
}

#[derive(Debug, Clone)]
pub(super) struct PanelMenuRenderState {
    pub(super) entries: Arc<[super::selectable_menu_session::SelectableMenuEntry]>,
    pub(super) show_icons: bool,
    pub(super) prompt: Option<String>,
    pub(super) search_query: String,
    pub(super) focused_index: Option<usize>,
    pub(super) can_go_back: bool,
    pub(super) scroll_y: f32,
    pub(super) viewport_height: f32,
}

impl From<super::panel_runtime::PanelMenuRenderState> for PanelMenuRenderState {
    fn from(value: super::panel_runtime::PanelMenuRenderState) -> Self {
        Self {
            entries: value.entries,
            show_icons: value.show_icons,
            prompt: value.prompt,
            search_query: value.search_query,
            focused_index: value.focused_index,
            can_go_back: value.can_go_back,
            scroll_y: value.scroll_y,
            viewport_height: value.viewport_height,
        }
    }
}

pub(super) struct PanelRenderSnapshot<'a> {
    pub(super) tree: &'a crate::surface::panel::PanelTree,
    pub(super) menu: PanelMenuRenderState,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PanelMenuScrollContext {
    pub(super) row_extent: f32,
    pub(super) default_viewport_height: f32,
}

#[derive(Debug, Clone)]
pub(super) enum ForegroundOverlay {
    Panel {
        window: Option<iced::window::Id>,
        tree: crate::surface::panel::PanelTree,
    },
    Menu {
        window: iced::window::Id,
        meta: MenuPopupMeta,
        session: super::selectable_menu_session::SelectableMenuSession,
    },
}

pub(super) enum ForegroundIntent<'a> {
    UpdateExternalPanelContent {
        identity: super::external_panel_session::ExternalPanelPanelIdentity,
        content: crate::surface::panel::ExternalPanelContent,
    },
    OpenPanel {
        request: PanelForegroundRequest,
        context: PanelOpenContext<'a>,
    },
    TogglePanel {
        request: PanelForegroundRequest,
        context: PanelOpenContext<'a>,
    },
    ClosePanel,
    PanelInstalledAppsLoaded {
        pending: super::panel_runtime::PendingPanelOpen,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
        menus: &'a [crate::config::MenuConfig],
    },
    PanelWindowOpened {
        window: iced::window::Id,
    },
    PanelWindowClosed {
        window: iced::window::Id,
    },
    BarSurfaceRemoved {
        identity: super::surfaces::BarSurfaceIdentity,
        window: iced::window::Id,
    },
    PanelSearchChanged(String),
    PanelMenuScrolled {
        y: f32,
        viewport_height: f32,
    },
    PanelFocusIndex(usize),
    PanelRequestSearchFocus,
    PanelSearchFocusSettled,
    PanelFocusMove {
        delta: isize,
        scroll: PanelMenuScrollContext,
    },
    PanelBack,
    PanelEscape,
    PanelActivate(Option<usize>),
    PanelActionRequested(crate::config::PanelAction),
    BeginDmenuSelection {
        request: crate::ipc::protocol::DmenuRequest,
        responder: super::messages::IpcResponder,
        panel_request: Option<PanelForegroundRequest>,
    },
    OpenMenu {
        request: MenuForegroundRequest,
    },
    CloseMenu,
    MenuWindowOpened {
        window: iced::window::Id,
        request: MenuForegroundRequest,
    },
    MenuWindowClosed {
        window: iced::window::Id,
    },
    MenuTreeUpdated {
        window: iced::window::Id,
        tree: crate::surface::menu_popup::MenuTree,
        search_enabled: bool,
    },
    MenuHoverEnter {
        window: iced::window::Id,
        item_id: String,
    },
    MenuHoverExit {
        window: iced::window::Id,
    },
    MenuSearchChanged {
        window: iced::window::Id,
        query: String,
    },
    MenuMoveFocus {
        window: iced::window::Id,
        forward: bool,
    },
}

#[derive(Debug, Clone)]
pub(super) enum ForegroundEffect {
    OpenPanelWindow {
        tree: crate::surface::panel::PanelTree,
    },
    RemoveWindow(iced::window::Id),
    RetirePanelWindow {
        window: iced::window::Id,
        duration: std::time::Duration,
    },
    SuspendBarKeyboardInteractivity,
    RestoreBarKeyboardInteractivity,
    CloseTooltipsForSurface(iced::window::Id),
    CloseAllTooltips,
    OpenMenuWindow {
        request: MenuForegroundRequest,
    },
    CloseMenu {
        window: iced::window::Id,
        meta: MenuPopupMeta,
    },
    RequestPanelSearchFocus,
    LoadInstalledAppsThenPreparePanel(super::panel_runtime::PendingPanelOpen),
    RunPanelAction {
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        action: crate::config::PanelAction,
    },
    LaunchInstalledApp(String),
    SendDmenuResponse {
        responder: super::messages::IpcResponder,
        response: crate::ipc::protocol::IpcResponse,
    },
    ScrollPanelMenuToFocused {
        window: iced::window::Id,
        y: f32,
    },
}

impl PartialEq for ForegroundEffect {
    fn eq(&self, other: &Self) -> bool {
        use ForegroundEffect::*;

        match (self, other) {
            (OpenPanelWindow { tree: left }, OpenPanelWindow { tree: right }) => left == right,
            (RemoveWindow(left), RemoveWindow(right)) => left == right,
            (
                RetirePanelWindow {
                    window: left,
                    duration: ld,
                },
                RetirePanelWindow {
                    window: right,
                    duration: rd,
                },
            ) => left == right && ld == rd,
            (SuspendBarKeyboardInteractivity, SuspendBarKeyboardInteractivity) => true,
            (RestoreBarKeyboardInteractivity, RestoreBarKeyboardInteractivity) => true,
            (CloseTooltipsForSurface(left), CloseTooltipsForSurface(right)) => left == right,
            (CloseAllTooltips, CloseAllTooltips) => true,
            (OpenMenuWindow { request: left }, OpenMenuWindow { request: right }) => left == right,
            (
                CloseMenu {
                    window: left_window,
                    meta: left_meta,
                },
                CloseMenu {
                    window: right_window,
                    meta: right_meta,
                },
            ) => left_window == right_window && left_meta == right_meta,
            (RequestPanelSearchFocus, RequestPanelSearchFocus) => true,
            (LoadInstalledAppsThenPreparePanel(left), LoadInstalledAppsThenPreparePanel(right)) => {
                left == right
            }
            (
                RunPanelAction {
                    origin_window: left_window,
                    origin: left_origin,
                    action: left_action,
                },
                RunPanelAction {
                    origin_window: right_window,
                    origin: right_origin,
                    action: right_action,
                },
            ) => {
                left_window == right_window
                    && left_origin == right_origin
                    && left_action == right_action
            }
            (LaunchInstalledApp(left), LaunchInstalledApp(right)) => left == right,
            (
                SendDmenuResponse { response: left, .. },
                SendDmenuResponse {
                    response: right, ..
                },
            ) => left == right,
            (
                ScrollPanelMenuToFocused {
                    window: left_window,
                    y: left_y,
                },
                ScrollPanelMenuToFocused {
                    window: right_window,
                    y: right_y,
                },
            ) => left_window == right_window && left_y == right_y,
            _ => false,
        }
    }
}

#[derive(Default)]
pub(super) struct Foreground {
    dismissal: dismissal::DismissalSurfaces,
    active: Option<ForegroundOverlay>,
    panel: super::panel_runtime::PanelRuntime,
    motion: Option<motion::PanelMotion>,
    retiring: Option<motion::RetiringPanel>,
    closing_panel_windows: HashSet<iced::window::Id>,
    closing_menu_windows: HashMap<iced::window::Id, MenuPopupMeta>,
    pending_menu: Option<MenuForegroundRequest>,
    pending_apps: Option<super::panel_runtime::PendingPanelOpen>,
    apps_generation: u64,
}

impl Foreground {
    pub(super) fn apps_generation(&self) -> u64 {
        self.apps_generation
    }

    pub(super) fn installed_apps_loaded(
        &mut self,
        generation: u64,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
        menus: &[crate::config::MenuConfig],
    ) -> Vec<ForegroundEffect> {
        if generation != self.apps_generation {
            return Vec::new();
        }
        let Some(pending) = self.pending_apps.take() else {
            return Vec::new();
        };
        self.handle(ForegroundIntent::PanelInstalledAppsLoaded {
            pending,
            apps,
            menus,
        })
    }

    /// Global menu commands match the menu name, independent of its bar or module origin.
    pub(super) fn close_panel_menu_if_open(&mut self, name: &str) -> Option<Vec<ForegroundEffect>> {
        if self.active_panel_tree().is_some_and(|tree| {
            matches!(&tree.content, crate::surface::panel::PanelContent::Menu(active) if active == name)
        }) {
            return Some(self.close_panel());
        }
        if self
            .pending_apps
            .as_ref()
            .is_some_and(|pending| pending.menu_name == name)
        {
            self.pending_apps = None;
            return Some(Vec::new());
        }
        None
    }

    pub(super) fn handle(&mut self, intent: ForegroundIntent<'_>) -> Vec<ForegroundEffect> {
        if let ForegroundIntent::TogglePanel { request, .. } = &intent
            && self
                .pending_apps
                .as_ref()
                .is_some_and(|pending| pending.request == *request)
        {
            self.pending_apps = None;
            return Vec::new();
        }
        match &intent {
            ForegroundIntent::OpenPanel { .. }
            | ForegroundIntent::TogglePanel { .. }
            | ForegroundIntent::ClosePanel
            | ForegroundIntent::OpenMenu { .. }
            | ForegroundIntent::BeginDmenuSelection { .. }
            | ForegroundIntent::PanelEscape => {
                self.pending_apps = None;
            }
            ForegroundIntent::BarSurfaceRemoved { window, .. }
                if self
                    .pending_apps
                    .as_ref()
                    .is_some_and(|pending| pending.request.origin_window == *window) =>
            {
                self.pending_apps = None;
            }
            _ => {}
        }
        match intent {
            ForegroundIntent::UpdateExternalPanelContent { identity, content } => {
                self.update_external_panel_content(identity, content);
                Vec::new()
            }
            ForegroundIntent::OpenPanel { request, context } => {
                self.prepare_panel(request, context)
            }
            ForegroundIntent::TogglePanel { request, context } => {
                self.toggle_panel(request, context)
            }
            ForegroundIntent::ClosePanel => self.close_panel(),
            ForegroundIntent::PanelInstalledAppsLoaded {
                pending,
                apps,
                menus,
            } => self.panel_installed_apps_loaded(pending, apps, menus),
            ForegroundIntent::PanelWindowOpened { window } => self.panel_window_opened(window),
            ForegroundIntent::PanelWindowClosed { window } => self.panel_window_closed(window),
            ForegroundIntent::BarSurfaceRemoved { identity, window } => {
                self.bar_surface_removed(identity, window)
            }
            ForegroundIntent::PanelSearchChanged(query) => {
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::SearchChanged(
                        query,
                    ));
                Vec::new()
            }
            ForegroundIntent::PanelMenuScrolled { y, viewport_height } => {
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::MenuScrolled {
                        y,
                        viewport_height,
                    });
                Vec::new()
            }
            ForegroundIntent::PanelFocusIndex(index) => {
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::FocusIndex(index));
                Vec::new()
            }
            ForegroundIntent::PanelRequestSearchFocus => {
                let effects = self
                    .panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::RequestSearchFocus);
                self.apply_panel_runtime_effects(effects)
            }
            ForegroundIntent::PanelSearchFocusSettled => {
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::SearchFocusSettled);
                Vec::new()
            }
            ForegroundIntent::PanelFocusMove { delta, scroll } => {
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::FocusMove(delta));
                self.scroll_focused_panel_menu(scroll)
            }
            ForegroundIntent::PanelBack => {
                let effects = self
                    .panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::Back);
                self.apply_panel_runtime_effects(effects)
            }
            ForegroundIntent::PanelEscape => {
                let effects = self
                    .panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::Escape);
                self.apply_panel_runtime_effects(effects)
            }
            ForegroundIntent::PanelActivate(index) => {
                let effects = self
                    .panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::Activate(index));
                self.apply_panel_runtime_effects(effects)
            }
            ForegroundIntent::PanelActionRequested(action) => self.panel_action_requested(action),
            ForegroundIntent::BeginDmenuSelection {
                request,
                responder,
                panel_request,
            } => {
                let effects = self.panel.handle(
                    super::panel_runtime::PanelRuntimeIntent::BeginDmenuSelection {
                        request,
                        responder,
                        panel_request,
                    },
                );
                self.apply_panel_runtime_effects(effects)
            }
            ForegroundIntent::OpenMenu { request } => self.open_menu(request),
            ForegroundIntent::CloseMenu => self.close_menu(),
            ForegroundIntent::MenuWindowOpened { window, request } => {
                self.track_menu_window_opened(window, request);
                Vec::new()
            }
            ForegroundIntent::MenuWindowClosed { window } => self.menu_window_closed(window),
            ForegroundIntent::MenuTreeUpdated {
                window,
                tree,
                search_enabled,
            } => {
                self.update_menu_tree(window, tree, search_enabled);
                Vec::new()
            }
            ForegroundIntent::MenuHoverEnter { window, item_id } => {
                if let Some(session) = self.active_menu_session_mut(window) {
                    session.hover_sni_item(&item_id);
                }
                Vec::new()
            }
            ForegroundIntent::MenuHoverExit { window } => {
                if let Some(session) = self.active_menu_session_mut(window) {
                    session.clear_hover();
                }
                Vec::new()
            }
            ForegroundIntent::MenuSearchChanged { window, query } => {
                if let Some(session) = self.active_menu_session_mut(window) {
                    session.set_search_query(query);
                }
                Vec::new()
            }
            ForegroundIntent::MenuMoveFocus { window, forward } => {
                if let Some(session) = self.active_menu_session_mut(window) {
                    session.move_focus(if forward { 1 } else { -1 });
                }
                Vec::new()
            }
        }
    }

    #[cfg(test)]
    pub(super) fn active_overlay(&self) -> Option<&ForegroundOverlay> {
        self.active.as_ref()
    }

    pub(super) fn has_active_panel(&self) -> bool {
        matches!(self.active, Some(ForegroundOverlay::Panel { .. }))
    }

    #[cfg(test)]
    pub(super) fn is_closing_panel_window(&self, window: iced::window::Id) -> bool {
        self.closing_panel_windows.contains(&window)
    }

    pub(super) fn active_panel_window(&self) -> Option<iced::window::Id> {
        match self.active.as_ref()? {
            ForegroundOverlay::Panel { window, .. } => *window,
            ForegroundOverlay::Menu { .. } => None,
        }
    }

    pub(super) fn active_panel_tree(&self) -> Option<&crate::surface::panel::PanelTree> {
        match self.active.as_ref()? {
            ForegroundOverlay::Panel { tree, .. } => Some(tree),
            ForegroundOverlay::Menu { .. } => None,
        }
    }

    pub(super) fn active_panel_menu_render_state(&self) -> PanelMenuRenderState {
        self.panel.panel_menu_render_state().into()
    }

    pub(super) fn active_panel_origin_for(
        &self,
        origin_window: iced::window::Id,
    ) -> Option<&crate::surface::panel::PanelOrigin> {
        match self.active.as_ref()? {
            ForegroundOverlay::Panel { tree, .. } if tree.origin_window == origin_window => {
                Some(&tree.origin)
            }
            ForegroundOverlay::Panel { .. } | ForegroundOverlay::Menu { .. } => None,
        }
    }

    pub(super) fn module_action_targets_active_panel(
        &self,
        origin_window: iced::window::Id,
        module: &crate::modules::ModuleId,
        action: &crate::config::PanelAction,
    ) -> bool {
        use crate::config::PanelAction;
        use crate::surface::panel::{PanelContent, PanelOrigin};

        let Some(tree) = self.active_panel_tree() else {
            return false;
        };
        if tree.origin_window != origin_window || tree.origin != PanelOrigin::Module(module.clone())
        {
            return false;
        }
        match (action, &tree.content) {
            (PanelAction::OpenPanel | PanelAction::TogglePanel, PanelContent::ModulePanel(id)) => {
                id == module
            }
            (
                PanelAction::OpenMenu(name) | PanelAction::ToggleMenu(name),
                PanelContent::Menu(active),
            ) => name == active,
            (PanelAction::OpenProvider(name), PanelContent::ExternalProvider(active)) => {
                name == &active.provider_name
            }
            _ => false,
        }
    }

    pub(super) fn panel_tree_for_window(
        &self,
        window: iced::window::Id,
    ) -> Option<&crate::surface::panel::PanelTree> {
        match self.active.as_ref()? {
            ForegroundOverlay::Panel {
                window: Some(current),
                tree,
            } if *current == window => Some(tree),
            ForegroundOverlay::Panel { .. } | ForegroundOverlay::Menu { .. } => None,
        }
    }

    pub(super) fn is_menu_panel_window(&self, window: iced::window::Id) -> bool {
        self.panel_tree_for_window(window).is_some_and(|tree| {
            matches!(
                tree.content,
                crate::surface::panel::PanelContent::Menu(_)
                    | crate::surface::panel::PanelContent::DmenuRequest
            )
        })
    }

    pub(super) fn active_menu_window(&self) -> Option<iced::window::Id> {
        match self.active.as_ref()? {
            ForegroundOverlay::Menu { window, .. } => Some(*window),
            ForegroundOverlay::Panel { .. } => None,
        }
    }

    pub(super) fn active_menu_meta_for_window(
        &self,
        window: iced::window::Id,
    ) -> Option<&MenuPopupMeta> {
        match self.active.as_ref()? {
            ForegroundOverlay::Menu {
                window: current,
                meta,
                ..
            } if *current == window => Some(meta),
            ForegroundOverlay::Menu { .. } | ForegroundOverlay::Panel { .. } => None,
        }
    }

    pub(super) fn active_menu_meta(&self) -> Option<&MenuPopupMeta> {
        match self.active.as_ref()? {
            ForegroundOverlay::Menu { meta, .. } => Some(meta),
            ForegroundOverlay::Panel { .. } => None,
        }
    }

    pub(super) fn active_menu_for(
        &self,
        service: &str,
        menu_path: &zbus::zvariant::OwnedObjectPath,
    ) -> Option<(iced::window::Id, &MenuPopupMeta)> {
        match self.active.as_ref()? {
            ForegroundOverlay::Menu { window, meta, .. }
                if meta.service == service && meta.menu_path == *menu_path =>
            {
                Some((*window, meta))
            }
            ForegroundOverlay::Menu { .. } | ForegroundOverlay::Panel { .. } => None,
        }
    }

    pub(super) fn menu_session(
        &self,
        window: iced::window::Id,
    ) -> Option<&super::selectable_menu_session::SelectableMenuSession> {
        match self.active.as_ref()? {
            ForegroundOverlay::Menu {
                window: current,
                session,
                ..
            } if *current == window => Some(session),
            ForegroundOverlay::Menu { .. } | ForegroundOverlay::Panel { .. } => None,
        }
    }

    #[cfg(test)]
    pub(super) fn menu_snapshot(
        &self,
        window: iced::window::Id,
    ) -> Option<super::selectable_menu_session::SelectableMenuSnapshot> {
        self.menu_session(window).map(|session| session.snapshot())
    }

    pub(super) fn hovered_menu_item(&self, window: iced::window::Id) -> Option<String> {
        self.menu_session(window)
            .and_then(|session| session.hovered_sni_item())
    }

    pub(super) fn focused_menu_item(&self, window: iced::window::Id) -> Option<String> {
        self.menu_session(window)
            .and_then(|session| session.focused_sni_item())
    }

    #[cfg(test)]
    pub(super) fn is_panel_window(&self, window: iced::window::Id) -> bool {
        self.active_panel_window() == Some(window)
    }

    #[cfg(test)]
    pub(super) fn is_menu_window(&self, window: iced::window::Id) -> bool {
        self.active_menu_window() == Some(window) || self.closing_menu_windows.contains_key(&window)
    }

    pub(super) fn panel_render_snapshot(
        &self,
        window: iced::window::Id,
    ) -> Option<PanelRenderSnapshot<'_>> {
        if let Some(retiring) = &self.retiring
            && retiring.window == window
        {
            return Some(PanelRenderSnapshot {
                tree: &retiring.tree,
                menu: retiring.runtime.panel_menu_render_state().into(),
            });
        }
        let tree = self.panel_tree_for_window(window)?;
        Some(PanelRenderSnapshot {
            tree,
            menu: self.panel.panel_menu_render_state().into(),
        })
    }

    pub(super) fn panel_detail_for_render(
        &self,
        window: iced::window::Id,
        id: &crate::modules::ModuleId,
    ) -> Option<&crate::modules::ModuleDetailSession> {
        if let Some(retiring) = &self.retiring
            && retiring.window == window
        {
            return retiring.runtime.module_detail_session_for(id);
        }
        self.active_panel_module_detail_session(id)
    }

    pub(super) fn active_panel_module_detail_session(
        &self,
        id: &crate::modules::ModuleId,
    ) -> Option<&crate::modules::ModuleDetailSession> {
        if !self.panel.active_module_panel_contains(id) {
            return None;
        }
        self.panel.module_detail_session_for(id)
    }

    pub(super) fn active_panel_module_detail_session_mut(
        &mut self,
        id: &crate::modules::ModuleId,
    ) -> Option<&mut crate::modules::ModuleDetailSession> {
        if !self.panel.active_module_panel_contains(id) {
            return None;
        }
        self.panel.module_detail_session_for_mut(id)
    }

    pub(super) fn panel_search_focus_pending(&self) -> bool {
        self.panel.search_focus_pending()
    }

    #[cfg(test)]
    pub(super) fn active_panel_menu_entries_for_test(
        &self,
    ) -> Vec<super::selectable_menu_session::SelectableMenuEntry> {
        self.panel.active_panel_menu_entries_for_test()
    }

    #[cfg(test)]
    pub(super) fn panel_menu_snapshot_for_test(
        &self,
    ) -> Option<super::selectable_menu_session::SelectableMenuSnapshot> {
        self.panel.panel_menu_snapshot_for_test()
    }

    #[cfg(test)]
    pub(super) fn focused_panel_index_for_test(&self) -> Option<usize> {
        self.panel.focused_index_for_test()
    }

    #[cfg(test)]
    pub(super) fn has_dmenu_responder_for_test(&self) -> bool {
        self.panel.has_dmenu_responder()
    }

    #[cfg(test)]
    pub(super) fn reset_panel_menu_scroll_for_test(&mut self) {
        self.panel.reset_menu_scroll();
    }

    #[cfg(test)]
    pub(super) fn panel_menu_scroll_y_for_test(&self) -> f32 {
        self.panel.menu_scroll_y()
    }

    #[cfg(test)]
    pub(super) fn installed_apps_for_test(
        &self,
    ) -> Option<&Vec<crate::desktop_apps::DesktopApplication>> {
        self.panel.installed_apps_for_test.as_ref()
    }

    #[cfg(test)]
    pub(super) fn set_installed_apps_for_test(
        &mut self,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
    ) {
        self.panel.installed_apps_for_test = Some(apps);
    }

    #[cfg(test)]
    pub(super) fn record_launched_installed_app_for_test(&mut self, id: &str) {
        self.panel
            .launched_installed_app_ids_for_test
            .push(id.to_string());
    }

    #[cfg(test)]
    pub(super) fn launched_installed_app_ids_for_test(&self) -> &[String] {
        &self.panel.launched_installed_app_ids_for_test
    }

    pub(super) fn role_snapshot(&self) -> ForegroundRoleSnapshot {
        let mut roles = HashMap::new();

        for window in self.dismissal.windows() {
            roles.insert(window, super::surface_input_router::WindowRole::Dismissal);
        }

        if let Some(window) = self.active_panel_window() {
            roles.insert(window, super::surface_input_router::WindowRole::Panel);
        }
        if let Some(window) = self.active_menu_window() {
            roles.insert(window, super::surface_input_router::WindowRole::Menu);
        }
        for window in &self.closing_panel_windows {
            roles.insert(
                *window,
                super::surface_input_router::WindowRole::ClosingPanel,
            );
        }
        for window in self.closing_menu_windows.keys() {
            roles.insert(*window, super::surface_input_router::WindowRole::Menu);
        }

        ForegroundRoleSnapshot { roles }
    }

    fn update_external_panel_content(
        &mut self,
        identity: super::external_panel_session::ExternalPanelPanelIdentity,
        content: crate::surface::panel::ExternalPanelContent,
    ) {
        use crate::surface::panel::PanelContent;
        let Some(ForegroundOverlay::Panel { tree, .. }) = self.active.as_mut() else {
            return;
        };
        let PanelContent::ExternalProvider(current) = &tree.content else {
            return;
        };
        if tree.origin_window != identity.origin_window
            || tree.origin != identity.origin
            || current.provider_name != identity.provider_name
            || content.provider_name != identity.provider_name
        {
            return;
        }
        // Updating an existing document must not retire its focused window.
        // External content has a fixed physical size; only its view changes.
        tree.content = PanelContent::ExternalProvider(content.clone());
        self.panel
            .handle(super::panel_runtime::PanelRuntimeIntent::UpdateExternalContent(content));
        // A captured entry frame cannot represent newly arranged provider controls.
        // Keep their window/focus, but reveal the live document before accepting input.
        if let Some(window) = self.active_panel_window()
            && matches!(
                self.panel_presentation(window),
                crate::surface::panel_motion::Presentation::Animated(_)
            )
        {
            self.finish_panel_motion(window, false);
        }
    }

    fn prepare_panel(
        &mut self,
        request: PanelForegroundRequest,
        context: PanelOpenContext<'_>,
    ) -> Vec<ForegroundEffect> {
        self.pending_apps = None;
        let catalog = super::panel_runtime::PanelMenuCatalog::new(context.menus);
        let effects = self
            .panel
            .handle(super::panel_runtime::PanelRuntimeIntent::PrepareOpen {
                request,
                catalog,
                module_detail_sessions: context.module_detail_sessions,
            });
        self.apply_panel_runtime_effects(effects)
    }

    fn toggle_panel(
        &mut self,
        request: PanelForegroundRequest,
        context: PanelOpenContext<'_>,
    ) -> Vec<ForegroundEffect> {
        if self.active_panel_matches(&request) {
            return self.close_panel();
        }
        self.prepare_panel(request, context)
    }

    fn panel_installed_apps_loaded(
        &mut self,
        pending: super::panel_runtime::PendingPanelOpen,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
        menus: &[crate::config::MenuConfig],
    ) -> Vec<ForegroundEffect> {
        let catalog = super::panel_runtime::PanelMenuCatalog::new(menus);
        let effects = self.panel.handle(
            super::panel_runtime::PanelRuntimeIntent::PrepareInstalledAppsLoaded {
                pending,
                apps,
                catalog,
            },
        );
        self.apply_panel_runtime_effects(effects)
    }

    fn panel_runtime_prepared(
        &mut self,
        tree: crate::surface::panel::PanelTree,
    ) -> Vec<ForegroundEffect> {
        let mut effects = self.close_for_replacement();
        effects.push(ForegroundEffect::CloseTooltipsForSurface(
            tree.origin_window,
        ));
        effects.push(ForegroundEffect::SuspendBarKeyboardInteractivity);
        self.active = Some(ForegroundOverlay::Panel {
            window: None,
            tree: tree.clone(),
        });
        effects.push(ForegroundEffect::OpenPanelWindow { tree });
        effects
    }

    fn close_panel(&mut self) -> Vec<ForegroundEffect> {
        self.pending_apps = None;
        if matches!(self.active, Some(ForegroundOverlay::Panel { .. })) {
            self.close_active_panel()
        } else {
            Vec::new()
        }
    }

    fn open_menu(&mut self, request: MenuForegroundRequest) -> Vec<ForegroundEffect> {
        self.pending_apps = None;
        self.motion = None;
        let mut effects = self.remove_retiring_panel();
        effects.push(ForegroundEffect::CloseAllTooltips);
        effects.extend(self.close_for_menu_open(request));
        effects
    }

    fn close_for_replacement(&mut self) -> Vec<ForegroundEffect> {
        self.motion = None;
        let mut effects = self.remove_retiring_panel();
        let active = self.active.take();
        effects.extend(match active {
            Some(ForegroundOverlay::Panel {
                window: Some(window),
                ..
            }) => {
                self.closing_panel_windows.insert(window);
                vec![ForegroundEffect::RemoveWindow(window)]
            }
            Some(ForegroundOverlay::Panel { window: None, .. }) | None => Vec::new(),
            Some(ForegroundOverlay::Menu { window, meta, .. }) => {
                self.closing_menu_windows.insert(window, meta.clone());
                vec![ForegroundEffect::CloseMenu { window, meta }]
            }
        });
        effects
    }

    fn close_for_menu_open(&mut self, request: MenuForegroundRequest) -> Vec<ForegroundEffect> {
        let active = self.active.take();
        match active {
            Some(ForegroundOverlay::Panel {
                window: Some(window),
                ..
            }) => {
                self.closing_panel_windows.insert(window);
                let mut effects = vec![ForegroundEffect::RemoveWindow(window)];
                effects.extend(self.clear_panel_runtime());
                effects.push(ForegroundEffect::OpenMenuWindow { request });
                effects
            }
            Some(ForegroundOverlay::Panel { window: None, .. }) => {
                let mut effects = self.clear_panel_runtime();
                effects.push(ForegroundEffect::OpenMenuWindow { request });
                effects
            }
            Some(ForegroundOverlay::Menu {
                window, meta: old, ..
            }) => {
                self.closing_menu_windows.insert(window, old.clone());
                self.pending_menu = Some(request);
                vec![ForegroundEffect::CloseMenu { window, meta: old }]
            }
            None => {
                if self.closing_menu_windows.is_empty() {
                    vec![ForegroundEffect::OpenMenuWindow { request }]
                } else {
                    self.pending_menu = Some(request);
                    Vec::new()
                }
            }
        }
    }

    fn close_active_panel(&mut self) -> Vec<ForegroundEffect> {
        let active = self.active.take();
        let mut effects = match active {
            Some(ForegroundOverlay::Panel {
                window: Some(window),
                tree,
            }) => {
                self.closing_panel_windows.insert(window);
                if let Some(mut effects) = self.retire_panel(window, tree) {
                    effects.push(ForegroundEffect::RestoreBarKeyboardInteractivity);
                    return effects;
                }
                vec![ForegroundEffect::RemoveWindow(window)]
            }
            Some(ForegroundOverlay::Panel { window: None, .. }) => Vec::new(),
            Some(ForegroundOverlay::Menu {
                window,
                meta,
                session,
            }) => {
                self.active = Some(ForegroundOverlay::Menu {
                    window,
                    meta,
                    session,
                });
                return Vec::new();
            }
            None => return Vec::new(),
        };
        effects.extend(self.clear_panel_runtime());
        effects.push(ForegroundEffect::RestoreBarKeyboardInteractivity);
        effects
    }

    fn clear_panel_runtime(&mut self) -> Vec<ForegroundEffect> {
        let effects = self
            .panel
            .handle(super::panel_runtime::PanelRuntimeIntent::Clear);
        self.apply_panel_runtime_effects(effects)
    }

    fn panel_action_requested(&self, action: crate::config::PanelAction) -> Vec<ForegroundEffect> {
        let Some(tree) = self.active_panel_tree() else {
            return Vec::new();
        };
        vec![ForegroundEffect::RunPanelAction {
            origin_window: tree.origin_window,
            origin: tree.origin.clone(),
            action,
        }]
    }

    fn scroll_focused_panel_menu(
        &mut self,
        context: PanelMenuScrollContext,
    ) -> Vec<ForegroundEffect> {
        let Some(window) = self.active_panel_window() else {
            return Vec::new();
        };
        self.panel
            .scroll_focused_menu_into_view(context.row_extent, context.default_viewport_height)
            .map(|y| ForegroundEffect::ScrollPanelMenuToFocused { window, y })
            .into_iter()
            .collect()
    }

    fn apply_panel_runtime_effects(
        &mut self,
        effects: Vec<super::panel_runtime::PanelRuntimeEffect>,
    ) -> Vec<ForegroundEffect> {
        let mut foreground_effects = Vec::new();
        for effect in effects {
            match effect {
                super::panel_runtime::PanelRuntimeEffect::PanelPrepared(tree) => {
                    foreground_effects.extend(self.panel_runtime_prepared(tree));
                }
                super::panel_runtime::PanelRuntimeEffect::RequestSearchFocus => {
                    foreground_effects.push(ForegroundEffect::RequestPanelSearchFocus);
                }
                super::panel_runtime::PanelRuntimeEffect::RequestPanelClose => {
                    foreground_effects.extend(self.close_panel());
                }
                super::panel_runtime::PanelRuntimeEffect::LoadInstalledAppsThenPreparePanel(
                    pending,
                ) => {
                    self.apps_generation = self.apps_generation.wrapping_add(1);
                    self.pending_apps = Some(pending.clone());
                    foreground_effects
                        .push(ForegroundEffect::LoadInstalledAppsThenPreparePanel(pending));
                }
                super::panel_runtime::PanelRuntimeEffect::RunPanelAction(action) => {
                    foreground_effects.extend(self.panel_action_requested(action));
                }
                super::panel_runtime::PanelRuntimeEffect::LaunchInstalledApp(id) => {
                    foreground_effects.push(ForegroundEffect::LaunchInstalledApp(id));
                }
                super::panel_runtime::PanelRuntimeEffect::SendDmenuResponse {
                    responder,
                    response,
                } => {
                    foreground_effects.push(ForegroundEffect::SendDmenuResponse {
                        responder,
                        response,
                    });
                }
            }
        }
        foreground_effects
    }

    fn panel_window_opened(&mut self, window: iced::window::Id) -> Vec<ForegroundEffect> {
        if let Some(ForegroundOverlay::Panel {
            window: current_window,
            ..
        }) = self.active.as_mut()
            && current_window.is_none()
        {
            *current_window = Some(window);
        }
        Vec::new()
    }

    fn panel_window_closed(&mut self, window: iced::window::Id) -> Vec<ForegroundEffect> {
        if self
            .retiring
            .as_ref()
            .is_some_and(|panel| panel.window == window)
        {
            self.retiring = None;
        }
        if self.active_panel_window() == Some(window) {
            self.motion = None;
        }
        if self.closing_panel_windows.remove(&window) {
            return Vec::new();
        }
        let is_current = matches!(
            self.active.as_ref(),
            Some(ForegroundOverlay::Panel {
                window: Some(current_window),
                ..
            }) if *current_window == window
        );
        if is_current {
            self.active = None;
            let mut effects = self.clear_panel_runtime();
            effects.push(ForegroundEffect::RestoreBarKeyboardInteractivity);
            return effects;
        }
        Vec::new()
    }

    fn menu_window_closed(&mut self, window: iced::window::Id) -> Vec<ForegroundEffect> {
        if self.closing_menu_windows.remove(&window).is_some() {
            return self
                .pending_menu
                .take()
                .map(|request| vec![ForegroundEffect::OpenMenuWindow { request }])
                .unwrap_or_default();
        }
        if let Some(ForegroundOverlay::Menu {
            window: current,
            meta,
            session,
        }) = self.active.take()
        {
            if current == window {
                return vec![ForegroundEffect::CloseMenu { window, meta }];
            }
            self.active = Some(ForegroundOverlay::Menu {
                window: current,
                meta,
                session,
            });
        }
        Vec::new()
    }

    fn track_menu_window_opened(
        &mut self,
        window: iced::window::Id,
        request: MenuForegroundRequest,
    ) {
        let session = Self::menu_session_for(&request.meta.tree, request.search_enabled);
        self.pending_menu = None;
        self.active = Some(ForegroundOverlay::Menu {
            window,
            meta: request.meta,
            session,
        });
    }

    fn update_menu_tree(
        &mut self,
        window: iced::window::Id,
        tree: crate::surface::menu_popup::MenuTree,
        search_enabled: bool,
    ) {
        if let Some(ForegroundOverlay::Menu {
            window: current,
            meta,
            session,
        }) = self.active.as_mut()
            && *current == window
        {
            meta.tree = tree.clone();
            *session = Self::menu_session_for(&tree, search_enabled);
        }
    }

    fn active_menu_session_mut(
        &mut self,
        window: iced::window::Id,
    ) -> Option<&mut super::selectable_menu_session::SelectableMenuSession> {
        match self.active.as_mut()? {
            ForegroundOverlay::Menu {
                window: current,
                session,
                ..
            } if *current == window => Some(session),
            ForegroundOverlay::Menu { .. } | ForegroundOverlay::Panel { .. } => None,
        }
    }

    fn menu_session_for(
        tree: &crate::surface::menu_popup::MenuTree,
        search_enabled: bool,
    ) -> super::selectable_menu_session::SelectableMenuSession {
        super::selectable_menu_session::SelectableMenuSession::open(
            super::selectable_menu_session::SelectableMenuSource::SniMenu {
                tree: tree.clone(),
                search_enabled,
            },
        )
    }

    fn close_menu(&mut self) -> Vec<ForegroundEffect> {
        self.pending_menu = None;
        match self.active.take() {
            Some(ForegroundOverlay::Menu { window, meta, .. }) => {
                self.closing_menu_windows.insert(window, meta.clone());
                vec![ForegroundEffect::CloseMenu { window, meta }]
            }
            Some(panel @ ForegroundOverlay::Panel { .. }) => {
                self.active = Some(panel);
                Vec::new()
            }
            None => Vec::new(),
        }
    }

    fn bar_surface_removed(
        &mut self,
        identity: super::surfaces::BarSurfaceIdentity,
        window: iced::window::Id,
    ) -> Vec<ForegroundEffect> {
        let mut retired_effects = if self
            .retiring
            .as_ref()
            .is_some_and(|panel| panel.tree.origin_window == window)
        {
            self.remove_retiring_panel()
        } else {
            Vec::new()
        };
        if self
            .pending_menu
            .as_ref()
            .is_some_and(|request| request.meta.origin == identity)
        {
            self.pending_menu = None;
        }

        let Some(active) = self.active.take() else {
            return retired_effects;
        };

        retired_effects.extend(match active {
            ForegroundOverlay::Panel {
                window: panel_window,
                tree,
            } if tree.origin_window == window => {
                self.motion = None;
                let mut effects = match panel_window {
                    Some(panel_window) => {
                        self.closing_panel_windows.insert(panel_window);
                        vec![ForegroundEffect::RemoveWindow(panel_window)]
                    }
                    None => Vec::new(),
                };
                effects.extend(self.clear_panel_runtime());
                effects.push(ForegroundEffect::RestoreBarKeyboardInteractivity);
                effects
            }
            ForegroundOverlay::Menu {
                window: menu_window,
                meta,
                ..
            } if meta.origin == identity => {
                self.closing_menu_windows.insert(menu_window, meta.clone());
                vec![ForegroundEffect::CloseMenu {
                    window: menu_window,
                    meta,
                }]
            }
            other => {
                self.active = Some(other);
                Vec::new()
            }
        });
        retired_effects
    }

    fn active_panel_matches(&self, request: &PanelForegroundRequest) -> bool {
        matches!(
            self.active.as_ref(),
            Some(ForegroundOverlay::Panel { tree, .. })
                if tree.origin_window == request.origin_window
                    && tree.origin == request.origin
                    && tree.content == request.content
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::surfaces;

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn panel_request(
        origin_window: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) -> super::PanelForegroundRequest {
        super::PanelForegroundRequest {
            origin_window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content,
            anchor: None,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            screen_size: (1920, 1080),
        }
    }

    fn bar_identity(output: &str) -> surfaces::BarSurfaceIdentity {
        surfaces::BarSurfaceIdentity::new("main", Some(output.into()))
    }

    fn menu_meta_for_origin(
        service: &str,
        origin: surfaces::BarSurfaceIdentity,
    ) -> super::MenuPopupMeta {
        super::MenuPopupMeta {
            service: service.into(),
            menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
            tree: crate::surface::menu_popup::MenuTree {
                items: Vec::new(),
                anchor: (0, 0),
                screen_size: (1920, 1080),
            },
            origin,
        }
    }

    fn menu_meta(service: &str) -> super::MenuPopupMeta {
        menu_meta_for_origin(service, surfaces::BarSurfaceIdentity::all_screens())
    }

    fn menu_tree_with_items(items: &[(&str, &str)]) -> crate::surface::menu_popup::MenuTree {
        crate::surface::menu_popup::MenuTree {
            items: items
                .iter()
                .map(|(id, label)| crate::surface::menu_popup::MenuItem {
                    id: (*id).into(),
                    label: (*label).into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: Vec::new(),
                    is_separator: false,
                })
                .collect(),
            anchor: (0, 0),
            screen_size: (1920, 1080),
        }
    }

    #[test]
    fn toggle_menu_cancels_pending_discovery_and_preserves_a_different_menu() {
        use crate::surface::panel::PanelContent;
        let menus = vec![crate::config::MenuConfig {
            name: "apps".into(),
            prompt: None,
            show_icons: true,
            source: Some(crate::config::MenuSourceConfig {
                kind: crate::config::MenuSourceKind::InstalledApps,
            }),
            items: Vec::new(),
        }];
        let mut foreground = Foreground::default();
        let main = panel_request(wid(), PanelContent::Menu("main".into()));
        open_panel(&mut foreground, main.clone());
        let apps = panel_request(wid(), PanelContent::Menu("apps".into()));
        let open_apps = |foreground: &mut Foreground| {
            foreground.handle(ForegroundIntent::OpenPanel {
                request: apps.clone(),
                context: PanelOpenContext::new(&menus, HashMap::new()),
            });
            foreground.apps_generation()
        };

        let cancelled = open_apps(&mut foreground);
        assert_eq!(foreground.close_panel_menu_if_open("apps"), Some(vec![]));
        assert!(
            foreground
                .installed_apps_loaded(cancelled, vec![], &menus)
                .is_empty()
        );
        assert_eq!(foreground.active_panel_tree(), Some(&main.tree()));

        let current = open_apps(&mut foreground);
        assert!(foreground.close_panel_menu_if_open("missing").is_none());
        assert!(
            !foreground
                .installed_apps_loaded(current, vec![], &menus)
                .is_empty()
        );
        assert_eq!(foreground.active_panel_tree(), Some(&apps.tree()));
        assert!(foreground.close_panel_menu_if_open("apps").is_some());
        assert!(!foreground.has_active_panel());
    }

    #[test]
    fn stale_launcher_discovery_cannot_reopen_closed_or_replaced_panel() {
        let menus = vec![crate::config::MenuConfig {
            name: "apps".into(),
            prompt: None,
            show_icons: true,
            source: Some(crate::config::MenuSourceConfig {
                kind: crate::config::MenuSourceKind::InstalledApps,
            }),
            items: Vec::new(),
        }];
        let request = panel_request(
            wid(),
            crate::surface::panel::PanelContent::Menu("apps".into()),
        );
        let mut foreground = Foreground::default();
        let open = |foreground: &mut Foreground| {
            foreground.handle(ForegroundIntent::OpenPanel {
                request: request.clone(),
                context: PanelOpenContext::new(&menus, HashMap::new()),
            });
            foreground.apps_generation()
        };
        let closed = open(&mut foreground);
        foreground.handle(ForegroundIntent::ClosePanel);
        assert!(
            foreground
                .installed_apps_loaded(closed, vec![], &menus)
                .is_empty()
        );
        let stale = open(&mut foreground);
        let current = open(&mut foreground);
        assert!(
            foreground
                .installed_apps_loaded(stale, vec![], &menus)
                .is_empty()
        );
        assert!(
            !foreground
                .installed_apps_loaded(current, vec![], &menus)
                .is_empty()
        );
    }

    fn menu_request(service: &str, search_enabled: bool) -> super::MenuForegroundRequest {
        let mut meta = menu_meta(service);
        meta.tree = menu_tree_with_items(&[("1", "Open"), ("2", "Close")]);
        super::MenuForegroundRequest {
            meta,
            search_enabled,
        }
    }

    fn menu_request_from_meta(meta: super::MenuPopupMeta) -> super::MenuForegroundRequest {
        super::MenuForegroundRequest {
            meta,
            search_enabled: false,
        }
    }

    fn open_panel(
        foreground: &mut Foreground,
        request: super::PanelForegroundRequest,
    ) -> Vec<ForegroundEffect> {
        foreground.handle(ForegroundIntent::OpenPanel {
            request,
            context: PanelOpenContext::empty(),
        })
    }

    fn toggle_panel(
        foreground: &mut Foreground,
        request: super::PanelForegroundRequest,
    ) -> Vec<ForegroundEffect> {
        foreground.handle(ForegroundIntent::TogglePanel {
            request,
            context: PanelOpenContext::empty(),
        })
    }

    fn panel_open_effects(
        origin: iced::window::Id,
        tree: crate::surface::panel::PanelTree,
    ) -> Vec<ForegroundEffect> {
        let requests_search_focus = matches!(
            tree.content,
            crate::surface::panel::PanelContent::Menu(_)
                | crate::surface::panel::PanelContent::DmenuRequest
        );
        let mut effects = vec![
            ForegroundEffect::CloseTooltipsForSurface(origin),
            ForegroundEffect::SuspendBarKeyboardInteractivity,
            ForegroundEffect::OpenPanelWindow { tree },
        ];
        if requests_search_focus {
            effects.push(ForegroundEffect::RequestPanelSearchFocus);
        }
        effects
    }

    fn menu_open_effect(meta: super::MenuPopupMeta) -> ForegroundEffect {
        ForegroundEffect::OpenMenuWindow {
            request: menu_request_from_meta(meta),
        }
    }

    fn menu_open_request_effect(request: super::MenuForegroundRequest) -> ForegroundEffect {
        ForegroundEffect::OpenMenuWindow { request }
    }

    #[test]
    fn opening_panel_prepares_panel_runtime_before_opening_window() {
        let origin = wid();
        let mut foreground = Foreground::default();
        let request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);
        let tree = request.tree();

        let effects = open_panel(&mut foreground, request);

        assert_eq!(effects, panel_open_effects(origin, tree.clone()));
        assert_eq!(foreground.active_panel_tree(), Some(&tree));
        assert_eq!(foreground.active_panel_window(), None);
    }

    #[test]
    fn prepared_panel_is_foreground_before_window_opens() {
        let origin = wid();
        let mut foreground = Foreground::default();
        let request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);
        let tree = request.tree();

        let effects = open_panel(&mut foreground, request);

        assert_eq!(effects, panel_open_effects(origin, tree.clone()));
        assert_eq!(foreground.active_panel_tree(), Some(&tree));
        assert_eq!(foreground.active_panel_window(), None);
    }

    #[test]
    fn replacing_panel_closes_old_window_and_opens_new_panel() {
        let origin = wid();
        let old_window = wid();
        let old_request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);
        let new_request = panel_request(
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );
        let new_tree = new_request.tree();
        let mut foreground = Foreground::default();

        open_panel(&mut foreground, old_request);
        foreground.handle(ForegroundIntent::PanelWindowOpened { window: old_window });

        let effects = open_panel(&mut foreground, new_request);

        let mut expected = vec![ForegroundEffect::RemoveWindow(old_window)];
        expected.extend(panel_open_effects(origin, new_tree.clone()));
        assert_eq!(effects, expected);
        assert_eq!(foreground.active_panel_tree(), Some(&new_tree));
        assert_eq!(foreground.active_panel_window(), None);
        assert!(foreground.is_closing_panel_window(old_window));
    }

    #[test]
    fn stale_panel_window_close_does_not_clear_current_panel() {
        let origin = wid();
        let old_window = wid();
        let current_window = wid();
        let old_request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);
        let current_request = panel_request(
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );
        let current_tree = current_request.tree();
        let mut foreground = Foreground::default();

        open_panel(&mut foreground, old_request);
        foreground.handle(ForegroundIntent::PanelWindowOpened { window: old_window });
        open_panel(&mut foreground, current_request);
        foreground.handle(ForegroundIntent::PanelWindowOpened {
            window: current_window,
        });

        let effects = foreground.handle(ForegroundIntent::PanelWindowClosed { window: old_window });

        assert!(effects.is_empty());
        assert_eq!(foreground.active_panel_window(), Some(current_window));
        assert_eq!(foreground.active_panel_tree(), Some(&current_tree));
        assert!(!foreground.is_closing_panel_window(old_window));
    }

    #[test]
    fn current_panel_window_close_clears_foreground_and_panel_runtime() {
        let origin = wid();
        let panel_window = wid();
        let request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);
        let mut foreground = Foreground::default();

        open_panel(&mut foreground, request);
        foreground.handle(ForegroundIntent::PanelWindowOpened {
            window: panel_window,
        });

        let effects = foreground.handle(ForegroundIntent::PanelWindowClosed {
            window: panel_window,
        });

        assert_eq!(
            effects,
            vec![ForegroundEffect::RestoreBarKeyboardInteractivity,]
        );
        assert!(foreground.active_overlay().is_none());
    }

    #[test]
    fn bar_surface_removed_closes_panel_from_that_origin_window() {
        let origin = wid();
        let panel_window = wid();
        let identity = bar_identity("DP-1");
        let request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);
        let mut foreground = Foreground::default();

        open_panel(&mut foreground, request);
        foreground.handle(ForegroundIntent::PanelWindowOpened {
            window: panel_window,
        });

        let effects = foreground.handle(ForegroundIntent::BarSurfaceRemoved {
            identity,
            window: origin,
        });

        assert_eq!(
            effects,
            vec![
                ForegroundEffect::RemoveWindow(panel_window),
                ForegroundEffect::RestoreBarKeyboardInteractivity,
            ]
        );
        assert!(foreground.active_overlay().is_none());
        assert!(foreground.is_closing_panel_window(panel_window));
    }

    #[test]
    fn bar_surface_removed_closes_menu_from_that_identity() {
        let menu_window = wid();
        let identity = bar_identity("DP-1");
        let removed_window = wid();
        let meta = menu_meta_for_origin("org.menu", identity.clone());
        let request = menu_request_from_meta(meta.clone());
        let mut foreground = Foreground::default();

        foreground.handle(ForegroundIntent::OpenMenu {
            request: request.clone(),
        });
        foreground.handle(ForegroundIntent::MenuWindowOpened {
            window: menu_window,
            request,
        });

        let effects = foreground.handle(ForegroundIntent::BarSurfaceRemoved {
            identity,
            window: removed_window,
        });

        assert_eq!(
            effects,
            vec![ForegroundEffect::CloseMenu {
                window: menu_window,
                meta,
            }]
        );
        assert!(foreground.active_overlay().is_none());
        assert!(foreground.is_menu_window(menu_window));
    }

    #[test]
    fn bar_surface_removed_clears_pending_menu_replacement_for_that_identity() {
        let old_window = wid();
        let removed_window = wid();
        let identity = bar_identity("DP-1");
        let old_meta = menu_meta_for_origin("org.old", identity.clone());
        let replacement_meta = menu_meta_for_origin("org.replacement", identity.clone());
        let old_request = menu_request_from_meta(old_meta.clone());
        let replacement_request = menu_request_from_meta(replacement_meta);
        let mut foreground = Foreground::default();

        foreground.handle(ForegroundIntent::OpenMenu {
            request: old_request.clone(),
        });
        foreground.handle(ForegroundIntent::MenuWindowOpened {
            window: old_window,
            request: old_request,
        });
        foreground.handle(ForegroundIntent::OpenMenu {
            request: replacement_request,
        });

        let removal_effects = foreground.handle(ForegroundIntent::BarSurfaceRemoved {
            identity,
            window: removed_window,
        });
        let closed_effects =
            foreground.handle(ForegroundIntent::MenuWindowClosed { window: old_window });

        assert!(removal_effects.is_empty());
        assert!(closed_effects.is_empty());
        assert!(foreground.active_overlay().is_none());
    }

    #[test]
    fn toggle_same_panel_closes_through_foreground() {
        let origin = wid();
        let window = wid();
        let mut foreground = Foreground::default();
        let request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);

        open_panel(&mut foreground, request.clone());
        foreground.handle(ForegroundIntent::PanelWindowOpened { window });

        let effects = toggle_panel(&mut foreground, request);

        assert_eq!(
            effects,
            vec![
                ForegroundEffect::RemoveWindow(window),
                ForegroundEffect::RestoreBarKeyboardInteractivity,
            ]
        );
        assert!(foreground.active_overlay().is_none());
    }

    #[test]
    fn toggle_different_panel_prepares_replacement() {
        let origin = wid();
        let mut foreground = Foreground::default();
        let active = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);
        let replacement = panel_request(
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );
        let replacement_tree = replacement.tree();

        open_panel(&mut foreground, active);

        let effects = toggle_panel(&mut foreground, replacement);

        assert_eq!(effects, panel_open_effects(origin, replacement_tree));
    }

    #[test]
    fn panel_opening_while_menu_is_active_prepares_before_closing_menu() {
        let origin = wid();
        let menu_window = wid();
        let meta = menu_meta("org.menu");
        let menu_request = menu_request_from_meta(meta.clone());
        let request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);
        let tree = request.tree();
        let mut foreground = Foreground::default();

        assert_eq!(
            foreground.handle(ForegroundIntent::OpenMenu {
                request: menu_request.clone(),
            }),
            vec![
                ForegroundEffect::CloseAllTooltips,
                menu_open_effect(meta.clone()),
            ]
        );
        foreground.handle(ForegroundIntent::MenuWindowOpened {
            window: menu_window,
            request: menu_request,
        });

        let mut expected = vec![ForegroundEffect::CloseMenu {
            window: menu_window,
            meta,
        }];
        expected.extend(panel_open_effects(origin, tree));
        assert_eq!(open_panel(&mut foreground, request), expected);
        assert_eq!(foreground.active_menu_window(), None);
    }

    #[test]
    fn menu_open_replace_current_close_and_stale_close_are_owned_by_foreground() {
        let old_window = wid();
        let current_window = wid();
        let old_meta = menu_meta("org.old");
        let current_meta = menu_meta("org.current");
        let old_request = menu_request_from_meta(old_meta.clone());
        let current_request = menu_request_from_meta(current_meta.clone());
        let mut foreground = Foreground::default();

        let first = foreground.handle(ForegroundIntent::OpenMenu {
            request: old_request.clone(),
        });
        assert_eq!(
            first,
            vec![
                ForegroundEffect::CloseAllTooltips,
                menu_open_effect(old_meta.clone()),
            ]
        );
        foreground.handle(ForegroundIntent::MenuWindowOpened {
            window: old_window,
            request: old_request,
        });
        assert_eq!(foreground.active_menu_window(), Some(old_window));
        assert_eq!(foreground.active_menu_meta(), Some(&old_meta));

        let replacement = foreground.handle(ForegroundIntent::OpenMenu {
            request: current_request.clone(),
        });
        assert_eq!(
            replacement,
            vec![
                ForegroundEffect::CloseAllTooltips,
                ForegroundEffect::CloseMenu {
                    window: old_window,
                    meta: old_meta,
                },
            ]
        );
        assert_eq!(foreground.active_menu_window(), None);
        assert!(foreground.is_menu_window(old_window));

        assert_eq!(
            foreground.handle(ForegroundIntent::MenuWindowClosed { window: old_window }),
            vec![menu_open_effect(current_meta.clone())]
        );
        foreground.handle(ForegroundIntent::MenuWindowOpened {
            window: current_window,
            request: current_request,
        });
        assert_eq!(foreground.active_menu_window(), Some(current_window));
        assert_eq!(foreground.active_menu_meta(), Some(&current_meta));

        assert_eq!(
            foreground.handle(ForegroundIntent::MenuWindowClosed { window: old_window }),
            Vec::<ForegroundEffect>::new()
        );
        assert_eq!(foreground.active_menu_window(), Some(current_window));

        let current_close = foreground.handle(ForegroundIntent::MenuWindowClosed {
            window: current_window,
        });
        assert_eq!(
            current_close,
            vec![ForegroundEffect::CloseMenu {
                window: current_window,
                meta: current_meta,
            }]
        );
        assert!(foreground.active_overlay().is_none());
    }

    #[test]
    fn opening_menu_replaces_panel() {
        let origin = wid();
        let panel_window = wid();
        let menu_window = wid();
        let panel_request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);
        let meta = menu_meta("org.menu");
        let request = menu_request_from_meta(meta.clone());
        let mut foreground = Foreground::default();

        open_panel(&mut foreground, panel_request);
        foreground.handle(ForegroundIntent::PanelWindowOpened {
            window: panel_window,
        });

        let menu_effects = foreground.handle(ForegroundIntent::OpenMenu {
            request: request.clone(),
        });
        assert_eq!(
            menu_effects,
            vec![
                ForegroundEffect::CloseAllTooltips,
                ForegroundEffect::RemoveWindow(panel_window),
                menu_open_effect(meta.clone()),
            ]
        );
        foreground.handle(ForegroundIntent::MenuWindowOpened {
            window: menu_window,
            request,
        });
        assert_eq!(foreground.active_menu_window(), Some(menu_window));
        assert_eq!(foreground.active_menu_meta(), Some(&meta));
    }

    #[test]
    fn opened_menu_creates_active_selectable_session() {
        let window = wid();
        let request = menu_request("org.menu", true);
        let mut foreground = Foreground::default();

        assert_eq!(
            foreground.handle(ForegroundIntent::OpenMenu {
                request: request.clone(),
            }),
            vec![
                ForegroundEffect::CloseAllTooltips,
                menu_open_request_effect(request.clone()),
            ]
        );
        foreground.handle(ForegroundIntent::MenuWindowOpened { window, request });

        let snapshot = foreground
            .menu_snapshot(window)
            .expect("active menu session");
        assert!(snapshot.search_enabled);
        assert_eq!(
            snapshot
                .entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Open", "Close"]
        );
    }

    #[test]
    fn active_menu_interactions_update_current_session_only() {
        let active_window = wid();
        let stale_window = wid();
        let request = menu_request("org.menu", true);
        let mut foreground = Foreground::default();

        foreground.handle(ForegroundIntent::MenuWindowOpened {
            window: active_window,
            request,
        });
        foreground.handle(ForegroundIntent::MenuHoverEnter {
            window: stale_window,
            item_id: "1".into(),
        });
        foreground.handle(ForegroundIntent::MenuMoveFocus {
            window: stale_window,
            forward: true,
        });
        foreground.handle(ForegroundIntent::MenuSearchChanged {
            window: stale_window,
            query: "open".into(),
        });

        let unchanged = foreground
            .menu_snapshot(active_window)
            .expect("active menu session");
        assert_eq!(unchanged.search_query, "");
        assert_eq!(unchanged.focused_index, None);
        assert_eq!(unchanged.hovered_index, None);

        foreground.handle(ForegroundIntent::MenuHoverEnter {
            window: active_window,
            item_id: "2".into(),
        });
        assert_eq!(
            foreground.hovered_menu_item(active_window).as_deref(),
            Some("2")
        );
        foreground.handle(ForegroundIntent::MenuSearchChanged {
            window: active_window,
            query: "close".into(),
        });

        let snapshot = foreground
            .menu_snapshot(active_window)
            .expect("active menu session");
        assert_eq!(snapshot.search_query, "close");
        assert_eq!(snapshot.focused_index, Some(0));
        assert_eq!(
            foreground.focused_menu_item(active_window).as_deref(),
            Some("2")
        );
    }

    #[test]
    fn menu_tree_update_replaces_active_session_and_resets_search() {
        let window = wid();
        let request = menu_request("org.menu", true);
        let replacement_tree = menu_tree_with_items(&[("7", "Reload")]);
        let mut foreground = Foreground::default();

        foreground.handle(ForegroundIntent::MenuWindowOpened { window, request });
        foreground.handle(ForegroundIntent::MenuSearchChanged {
            window,
            query: "close".into(),
        });

        foreground.handle(ForegroundIntent::MenuTreeUpdated {
            window,
            tree: replacement_tree,
            search_enabled: true,
        });

        let snapshot = foreground
            .menu_snapshot(window)
            .expect("active menu session");
        assert_eq!(snapshot.search_query, "");
        assert_eq!(snapshot.focused_index, None);
        assert_eq!(
            snapshot
                .entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Reload"]
        );
    }

    #[test]
    fn role_snapshot_reports_active_and_closing_panel_windows() {
        let origin = wid();
        let old_window = wid();
        let current_window = wid();
        let mut foreground = Foreground::default();

        open_panel(
            &mut foreground,
            panel_request(origin, crate::surface::panel::PanelContent::BarPanel),
        );
        foreground.handle(ForegroundIntent::PanelWindowOpened { window: old_window });
        open_panel(
            &mut foreground,
            panel_request(
                origin,
                crate::surface::panel::PanelContent::Menu("main".into()),
            ),
        );
        foreground.handle(ForegroundIntent::PanelWindowOpened {
            window: current_window,
        });

        let snapshot = foreground.role_snapshot();

        assert_eq!(
            snapshot.role_for(Some(old_window)),
            Some(super::super::surface_input_router::WindowRole::ClosingPanel)
        );
        assert_eq!(
            snapshot.role_for(Some(current_window)),
            Some(super::super::surface_input_router::WindowRole::Panel)
        );
    }

    #[test]
    fn role_snapshot_reports_active_and_closing_menu_windows() {
        let old_window = wid();
        let current_window = wid();
        let old_request = menu_request("org.old", false);
        let current_request = menu_request("org.current", false);
        let mut foreground = Foreground::default();

        foreground.handle(ForegroundIntent::MenuWindowOpened {
            window: old_window,
            request: old_request.clone(),
        });
        foreground.handle(ForegroundIntent::OpenMenu {
            request: current_request.clone(),
        });

        let closing_snapshot = foreground.role_snapshot();
        assert_eq!(
            closing_snapshot.role_for(Some(old_window)),
            Some(super::super::surface_input_router::WindowRole::Menu)
        );
        assert_eq!(foreground.active_menu_window(), None);

        foreground.handle(ForegroundIntent::MenuWindowClosed { window: old_window });
        foreground.handle(ForegroundIntent::MenuWindowOpened {
            window: current_window,
            request: current_request,
        });

        let snapshot = foreground.role_snapshot();

        assert_eq!(snapshot.role_for(Some(old_window)), None);
        assert_eq!(
            snapshot.role_for(Some(current_window)),
            Some(super::super::surface_input_router::WindowRole::Menu)
        );
        assert_eq!(snapshot.role_for(None), None);
    }
}
