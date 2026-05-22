use iced::Task;
use iced_layershell::reexport::{
    KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use std::borrow::Cow;
use std::collections::HashSet;
use std::time::Duration;

use super::{App, Message};

pub(super) fn panel_search_input_id(window: iced::window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("panel-search-input-{window:?}"))
}

pub(super) fn panel_menu_scroll_id(window: iced::window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("panel-menu-scroll-{window:?}"))
}

pub(super) const MENU_ROW_EXTENT: f32 = 45.0;
const DEFAULT_MENU_VIEWPORT_HEIGHT: f32 = MENU_ROW_EXTENT * 12.0;
const PANEL_SEARCH_FOCUS_RETRIES: u8 = 8;
const PANEL_SEARCH_FOCUS_RETRY_DELAY: Duration = Duration::from_millis(25);

fn request_panel_search_focus() -> Task<Message> {
    Task::done(Message::Panel(
        super::messages::PanelEvent::FocusSearchInput {
            retries: PANEL_SEARCH_FOCUS_RETRIES,
        },
    ))
}

#[derive(Debug, Default)]
pub(super) struct PanelSystem {
    pub(super) search_query: String,
    pub(super) focused_index: Option<usize>,
    pub(super) menu_scroll_y: f32,
    menu_viewport_height: f32,
    search_focus_pending: bool,
    pub(super) menu_path: Vec<usize>,
    pub(super) installed_apps_snapshot: Vec<crate::desktop_apps::DesktopApplication>,
    pub(super) dmenu_request: Option<crate::ipc::protocol::DmenuRequest>,
    pub(super) dmenu_responder: Option<super::messages::IpcResponder>,
    installed_apps_entry_cache: InstalledAppsEntryCache,
    #[cfg(test)]
    pub(super) installed_apps_for_test: Option<Vec<crate::desktop_apps::DesktopApplication>>,
    #[cfg(test)]
    pub(super) launched_installed_app_ids_for_test: Vec<String>,
    closing_windows: HashSet<iced::window::Id>,
}

#[derive(Debug, Default)]
struct InstalledAppsEntryCache {
    query: String,
    entries: Vec<MenuEntryView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MenuEntryTarget {
    StaticPath(Vec<usize>),
    InstalledApp(String),
    DmenuValue(String),
    DmenuCustomValue(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MenuEntryView {
    pub(super) label: String,
    pub(super) icon: Option<String>,
    pub(super) path: Vec<usize>,
    pub(super) target: MenuEntryTarget,
    pub(super) is_submenu: bool,
}

impl PanelSystem {
    pub(super) fn clear(&mut self) {
        self.search_query.clear();
        self.focused_index = None;
        self.menu_scroll_y = 0.0;
        self.menu_viewport_height = 0.0;
        self.search_focus_pending = false;
        self.menu_path.clear();
        self.installed_apps_snapshot.clear();
        self.dmenu_request = None;
        self.dmenu_responder = None;
        self.installed_apps_entry_cache = InstalledAppsEntryCache::default();
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

    pub(super) fn escape_clears_search(&mut self) -> bool {
        if self.search_query.is_empty() {
            return false;
        }
        self.search_query.clear();
        self.focused_index = Some(0);
        true
    }
}

fn panel_window_settings(
    bar: &crate::config::BarConfig,
    tree: &crate::surface::panel::PanelTree,
    surface_origin: (i32, i32),
) -> NewLayerShellSettings {
    let width = bar.panel.width;
    let height = bar.panel.height.or(bar.panel.max_height).unwrap_or(600);
    let anchor = crate::surface::placement::layer_anchor(&bar.edge);
    let position = crate::surface::placement::centered_position(
        &bar.edge,
        tree.anchor,
        crate::surface::placement::Size { width, height },
        crate::surface::placement::Size {
            width: tree.screen_size.0,
            height: tree.screen_size.1,
        },
        crate::surface::placement::Point {
            x: surface_origin.0,
            y: surface_origin.1,
        },
        bar.collapsed_width,
        0,
    );
    let margin = position.layer_margin();

    NewLayerShellSettings {
        anchor,
        exclusive_zone: Some(-1),
        size: Some((width, height)),
        margin: Some(margin),
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        output_option: if tree.origin_output_name.is_empty() {
            OutputOption::None
        } else {
            OutputOption::OutputName(tree.origin_output_name.clone())
        },
        namespace: Some(App::namespace()),
        ..Default::default()
    }
}

#[cfg(test)]
fn panel_position_for_test(
    edge: crate::config::Edge,
    bar_size: u32,
    panel_size: (u32, u32),
    anchor: Option<iced::Rectangle>,
    screen_size: (u32, u32),
    surface_origin: (i32, i32),
) -> (i32, i32) {
    let placement = crate::surface::placement::centered_position(
        &edge,
        anchor,
        crate::surface::placement::Size {
            width: panel_size.0,
            height: panel_size.1,
        },
        crate::surface::placement::Size {
            width: screen_size.0,
            height: screen_size.1,
        },
        crate::surface::placement::Point {
            x: surface_origin.0,
            y: surface_origin.1,
        },
        bar_size,
        0,
    );
    (placement.position.x, placement.position.y)
}

impl App {
    pub(super) fn suspend_bar_keyboard_interactivity(&mut self) -> Vec<Task<Message>> {
        let ids: Vec<iced::window::Id> = self.surfaces.keys().copied().collect();
        ids.into_iter()
            .map(|id| {
                Task::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::None,
                })
            })
            .collect()
    }

    pub(super) fn dispatch_panel_action(
        &mut self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        action: crate::config::PanelAction,
    ) -> Task<Message> {
        match action {
            crate::config::PanelAction::Command(command) => {
                let _ = crate::command::spawn_config_command_fire_and_forget(&command);
                Task::none()
            }
            action => match panel_event_for_action(origin_window, origin, action, None) {
                Some(event) => self.handle_panel_event(event),
                None => Task::none(),
            },
        }
    }

    pub(super) fn close_all_panels(&mut self) -> Vec<Task<Message>> {
        if self.panel.dmenu_request.is_some() {
            self.complete_dmenu_request(crate::ipc::protocol::IpcResponse::Cancelled);
        }
        let effects = self.overlays.close_all_panels();
        let mut tasks = if effects.is_empty() {
            Vec::new()
        } else {
            self.panel.clear();
            self.apply_overlay_effects(effects, true)
        };
        tasks.extend(self.close_all_untracked_panels_with_restore(true));
        tasks
    }

    fn close_all_untracked_panels_with_restore(
        &mut self,
        restore_bar_interactivity: bool,
    ) -> Vec<Task<Message>> {
        let ids: Vec<_> = self
            .surfaces
            .panel_popup_ids()
            .filter(|id| self.overlays.panel(*id).is_none())
            .collect();
        if self.panel.dmenu_request.is_some() {
            self.complete_dmenu_request(crate::ipc::protocol::IpcResponse::Cancelled);
        }
        self.panel.clear();
        let mut tasks: Vec<_> = ids
            .into_iter()
            .map(|id| {
                self.overlays.forget_panel(id);
                self.panel.mark_closing_window(id);
                self.surfaces.remove_panel_popup(id);
                Task::done(Message::RemoveWindow(id))
            })
            .collect();
        if restore_bar_interactivity {
            tasks.extend(self.restore_bar_keyboard_interactivity());
        }
        tasks
    }

    pub(super) fn handle_panel_focus_gained(&mut self, window: iced::window::Id) -> Task<Message> {
        let Some(meta) = self.surfaces.panel_popup(window) else {
            return Task::none();
        };
        if matches!(
            meta.tree.content,
            crate::surface::panel::PanelContent::Menu(_)
                | crate::surface::panel::PanelContent::DmenuRequest
        ) {
            self.panel.focused_index.get_or_insert(0);
            self.panel.search_focus_pending = true;
            return request_panel_search_focus();
        }
        Task::none()
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
                self.panel.focused_index = Some(0);
                self.panel.search_focus_pending = false;
                self.panel.menu_scroll_y = 0.0;
                self.refresh_installed_apps_entry_cache();
                Task::none()
            }
            PanelEvent::MenuScrolled { y, viewport_height } => {
                self.panel.menu_scroll_y = y.max(0.0);
                self.panel.menu_viewport_height = viewport_height.max(0.0);
                Task::none()
            }
            PanelEvent::FocusMoveNext => {
                self.move_panel_focus(1);
                self.scroll_panel_menu_to_focus()
            }
            PanelEvent::FocusMovePrev => {
                self.move_panel_focus(-1);
                self.scroll_panel_menu_to_focus()
            }
            PanelEvent::FocusIndex(index) => {
                self.panel.focused_index = Some(index);
                self.panel.search_focus_pending = false;
                Task::none()
            }
            PanelEvent::FocusSearchInput { retries } => self.focus_panel_search_input(retries),
            PanelEvent::Activate => self.activate_panel_entry(None),
            PanelEvent::ActivateIndex(index) => self.activate_panel_entry(Some(index)),
            PanelEvent::Back => {
                if self.panel.search_query.is_empty() {
                    self.panel.menu_path.pop();
                    self.panel.focused_index = Some(0);
                }
                Task::none()
            }
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
        let surface_origin = self.inferred_panel_surface_origin(&origin_surface, bar);
        let settings = panel_window_settings(bar, &tree, surface_origin);
        let (panel_window, open_task) = Message::layershell_open(settings);
        self.surface_mut(origin_window).tooltip_pending = None;

        let effects = self.overlays.open_panel(origin_window, panel_window);
        let mut tasks = self.apply_overlay_effects(effects, false);
        let is_menu_panel = matches!(
            tree.content,
            crate::surface::panel::PanelContent::Menu(_)
                | crate::surface::panel::PanelContent::DmenuRequest
        );
        let is_installed_apps_menu = match &tree.content {
            crate::surface::panel::PanelContent::Menu(name) => self
                .config
                .menus
                .iter()
                .find(|menu| menu.name == *name)
                .and_then(|menu| menu.source.as_ref())
                .map(|source| matches!(source.kind, crate::config::MenuSourceKind::InstalledApps))
                .unwrap_or(false),
            _ => false,
        };
        if is_installed_apps_menu {
            self.panel.installed_apps_snapshot = self.load_installed_apps_for_panel();
            self.refresh_installed_apps_entry_cache();
        }
        self.register_panel_popup(panel_window, origin_window, tree);

        let mut open_sequence = open_task.chain(iced::window::gain_focus(panel_window));
        if is_menu_panel {
            self.panel.focused_index = Some(0);
            self.panel.search_focus_pending = true;
            open_sequence = open_sequence.chain(request_panel_search_focus());
        }
        tasks.push(open_sequence);
        Task::batch(tasks)
    }

    fn load_installed_apps_for_panel(&self) -> Vec<crate::desktop_apps::DesktopApplication> {
        #[cfg(test)]
        if let Some(apps) = &self.panel.installed_apps_for_test {
            return apps.clone();
        }
        crate::desktop_apps::discover_installed_applications().unwrap_or_default()
    }

    fn inferred_panel_surface_origin(
        &self,
        origin_surface: &crate::surface::SurfaceState,
        bar: &crate::config::BarConfig,
    ) -> (i32, i32) {
        if origin_surface.output_name.is_empty() {
            return (0, 0);
        }

        let top_inset = match bar.edge {
            crate::config::Edge::Left | crate::config::Edge::Right => {
                self.config
                    .bars_for_output(&origin_surface.output_name)
                    .into_iter()
                    .filter(|candidate| matches!(candidate.edge, crate::config::Edge::Top))
                    .map(|candidate| candidate.collapsed_width)
                    .sum::<u32>() as i32
            }
            crate::config::Edge::Top | crate::config::Edge::Bottom => 0,
        };

        (0, top_inset)
    }

    pub(super) fn handle_panel_escape_pressed(&mut self) -> Task<Message> {
        if self.panel.escape_clears_search() {
            Task::none()
        } else {
            self.handle_panel_event(super::messages::PanelEvent::Close)
        }
    }

    fn move_panel_focus(&mut self, delta: isize) {
        let entries = self.active_menu_entries();
        if entries.is_empty() {
            self.panel.focused_index = None;
            self.panel.menu_scroll_y = 0.0;
            return;
        }
        let current = self.panel.focused_index.unwrap_or(0).min(entries.len() - 1);
        let next = if delta > 0 {
            (current + 1) % entries.len()
        } else if current == 0 {
            entries.len() - 1
        } else {
            current - 1
        };
        self.panel.focused_index = Some(next);
    }

    fn scroll_panel_menu_to_focus(&mut self) -> Task<Message> {
        let Some(index) = self.panel.focused_index else {
            return Task::none();
        };
        let Some(scroll_id) = self
            .surfaces
            .panel_popup_ids()
            .next()
            .map(panel_menu_scroll_id)
        else {
            return Task::none();
        };
        let current_top = self.panel.menu_scroll_y.max(0.0);
        let viewport_height = if self.panel.menu_viewport_height > 0.0 {
            self.panel.menu_viewport_height
        } else {
            DEFAULT_MENU_VIEWPORT_HEIGHT
        };
        let current_bottom = current_top + viewport_height;
        let row_top = index as f32 * MENU_ROW_EXTENT;
        let row_bottom = row_top + MENU_ROW_EXTENT;
        let y = if row_top < current_top {
            row_top
        } else if row_bottom > current_bottom {
            row_bottom - viewport_height
        } else {
            return Task::none();
        };
        self.panel.menu_scroll_y = y.max(0.0);
        iced::widget::operation::scroll_to(
            scroll_id,
            iced::widget::operation::AbsoluteOffset {
                x: None,
                y: Some(self.panel.menu_scroll_y),
            },
        )
    }

    fn activate_panel_entry(&mut self, index: Option<usize>) -> Task<Message> {
        let Some((tree, entries)) = self.active_menu_tree_and_entries() else {
            return Task::none();
        };
        let Some(entry) = index
            .or(self.panel.focused_index)
            .and_then(|index| entries.get(index).cloned())
        else {
            return Task::none();
        };
        match &entry.target {
            MenuEntryTarget::DmenuValue(value) | MenuEntryTarget::DmenuCustomValue(value) => {
                self.complete_dmenu_request(crate::ipc::protocol::IpcResponse::Selected {
                    value: value.clone(),
                });
                return Task::batch(self.close_all_panels());
            }
            _ => {}
        }
        let Some(menu_name) = menu_name_for_tree(&tree) else {
            return Task::none();
        };
        let Some(menu) = self.config.menus.iter().find(|menu| menu.name == menu_name) else {
            return Task::none();
        };

        match entry.target {
            MenuEntryTarget::InstalledApp(app_id) => {
                let Some(app) = self
                    .panel
                    .installed_apps_snapshot
                    .iter()
                    .find(|app| app.id == app_id)
                    .cloned()
                else {
                    return Task::none();
                };
                #[cfg(test)]
                {
                    self.panel
                        .launched_installed_app_ids_for_test
                        .push(app.id.clone());
                    Task::batch(self.close_all_panels())
                }
                #[cfg(not(test))]
                {
                    let _ = crate::command::spawn_desktop_application_fire_and_forget(&app);
                    Task::batch(self.close_all_panels())
                }
            }
            MenuEntryTarget::StaticPath(path) => {
                if entry.is_submenu {
                    self.panel.menu_path = path;
                    self.panel.search_query.clear();
                    self.panel.focused_index = Some(0);
                    return Task::none();
                }

                let Some(item) = menu_item_at_path(&menu.items, &path) else {
                    return Task::none();
                };
                let Some(action) = item.action.clone() else {
                    return Task::none();
                };
                match action {
                    crate::config::PanelAction::Command(command) => {
                        let _ = crate::command::spawn_config_command_fire_and_forget(&command);
                        Task::batch(self.close_all_panels())
                    }
                    action => self.dispatch_panel_action(tree.origin_window, tree.origin, action),
                }
            }
            MenuEntryTarget::DmenuValue(_) | MenuEntryTarget::DmenuCustomValue(_) => Task::none(),
        }
    }

    pub(super) fn active_menu_entries(&self) -> Vec<MenuEntryView> {
        self.active_menu_tree_and_entries()
            .map(|(_, entries)| entries)
            .unwrap_or_default()
    }

    pub(super) fn active_menu_entries_for_render(&self) -> Cow<'_, [MenuEntryView]> {
        if let Some(request) = &self.panel.dmenu_request {
            return Cow::Owned(dmenu_entry_views(request, &self.panel.search_query));
        }
        let Some((tree, menu)) = self.active_menu_tree_and_menu() else {
            return Cow::Borrowed(&[]);
        };
        if menu_is_installed_apps(menu)
            && self.panel.installed_apps_entry_cache.query == self.panel.search_query
        {
            return Cow::Borrowed(&self.panel.installed_apps_entry_cache.entries);
        }
        Cow::Owned(self.menu_entries_for_tree_and_menu(&tree, menu))
    }

    fn refresh_installed_apps_entry_cache(&mut self) {
        self.panel.installed_apps_entry_cache = InstalledAppsEntryCache {
            query: self.panel.search_query.clone(),
            entries: installed_app_entries(
                &self.panel.installed_apps_snapshot,
                &self.panel.search_query,
            ),
        };
    }

    fn focus_panel_search_input(&mut self, retries: u8) -> Task<Message> {
        if !self.panel.search_focus_pending {
            return Task::none();
        }
        let Some(search_input_id) = self.surfaces.panel_popup_values().find_map(|meta| {
            if matches!(
                meta.tree.content,
                crate::surface::panel::PanelContent::Menu(_)
                    | crate::surface::panel::PanelContent::DmenuRequest
            ) {
                Some(meta.search_input_id.clone())
            } else {
                None
            }
        }) else {
            self.panel.search_focus_pending = false;
            return Task::none();
        };
        self.panel.focused_index.get_or_insert(0);
        let mut tasks = vec![iced::widget::operation::focus(search_input_id)];
        if retries > 0 {
            tasks.push(Task::perform(
                async {
                    tokio::time::sleep(PANEL_SEARCH_FOCUS_RETRY_DELAY).await;
                },
                move |_| {
                    Message::Panel(super::messages::PanelEvent::FocusSearchInput {
                        retries: retries - 1,
                    })
                },
            ));
        } else {
            self.panel.search_focus_pending = false;
        }
        Task::batch(tasks)
    }

    fn active_menu_tree_and_entries(
        &self,
    ) -> Option<(crate::surface::panel::PanelTree, Vec<MenuEntryView>)> {
        if let Some(tree) = self
            .surfaces
            .panel_popup_values()
            .find(|meta| {
                matches!(
                    meta.tree.content,
                    crate::surface::panel::PanelContent::DmenuRequest
                )
            })
            .map(|meta| meta.tree.clone())
        {
            let entries = self
                .panel
                .dmenu_request
                .as_ref()
                .map(|request| dmenu_entry_views(request, &self.panel.search_query))
                .unwrap_or_default();
            return Some((tree, entries));
        }
        let (tree, menu) = self.active_menu_tree_and_menu()?;
        let entries = self.menu_entries_for_tree_and_menu(&tree, menu);
        Some((tree, entries))
    }

    fn active_menu_tree_and_menu(
        &self,
    ) -> Option<(crate::surface::panel::PanelTree, &crate::config::MenuConfig)> {
        let tree = self
            .surfaces
            .panel_popup_values()
            .find(|meta| {
                matches!(
                    meta.tree.content,
                    crate::surface::panel::PanelContent::Menu(_)
                )
            })?
            .tree
            .clone();
        let menu_name = menu_name_for_tree(&tree)?;
        let menu = self
            .config
            .menus
            .iter()
            .find(|menu| menu.name == menu_name)?;
        Some((tree, menu))
    }

    fn menu_entries_for_tree_and_menu(
        &self,
        _tree: &crate::surface::panel::PanelTree,
        menu: &crate::config::MenuConfig,
    ) -> Vec<MenuEntryView> {
        match &menu.source {
            Some(source) if matches!(source.kind, crate::config::MenuSourceKind::InstalledApps) => {
                installed_app_entries(
                    &self.panel.installed_apps_snapshot,
                    &self.panel.search_query,
                )
            }
            _ if self.panel.search_query.is_empty() => {
                current_menu_entries(menu, &self.panel.menu_path)
            }
            _ => search_menu_entries(menu, &self.panel.search_query),
        }
    }
}

impl App {
    pub(super) fn handle_dmenu_selection_request(
        &mut self,
        request: crate::ipc::protocol::DmenuRequest,
        responder: super::messages::IpcResponder,
    ) -> Task<Message> {
        if self.panel.dmenu_request.is_some() {
            send_ipc_response(
                responder,
                crate::ipc::protocol::IpcResponse::Rejected {
                    reason: "another dmenu request is already active".into(),
                },
            );
            return Task::none();
        }

        let Some(origin_window) = self.default_panel_origin_window() else {
            send_ipc_response(
                responder,
                crate::ipc::protocol::IpcResponse::Rejected {
                    reason: "no bar surface is available".into(),
                },
            );
            return Task::none();
        };

        let tasks = self.close_all_panels();
        self.panel.clear();
        self.panel.dmenu_request = Some(request);
        self.panel.dmenu_responder = Some(responder);
        self.panel.focused_index = Some(0);

        Task::batch(
            tasks
                .into_iter()
                .chain(std::iter::once(self.handle_panel_event(
                    super::messages::PanelEvent::Open {
                        origin_window,
                        origin: crate::surface::panel::PanelOrigin::Bar,
                        content: crate::surface::panel::PanelContent::DmenuRequest,
                        anchor: None,
                    },
                ))),
        )
    }

    pub(super) fn complete_dmenu_request(&mut self, response: crate::ipc::protocol::IpcResponse) {
        if let Some(responder) = self.panel.dmenu_responder.take() {
            send_ipc_response(responder, response);
        }
        self.panel.dmenu_request = None;
    }

    fn default_panel_origin_window(&self) -> Option<iced::window::Id> {
        self.surfaces.keys().copied().next()
    }
}

fn send_ipc_response(
    responder: super::messages::IpcResponder,
    response: crate::ipc::protocol::IpcResponse,
) {
    if let Some(sender) = responder
        .lock()
        .expect("IPC responder mutex poisoned")
        .take()
    {
        let _ = sender.send(response);
    }
}

fn menu_is_installed_apps(menu: &crate::config::MenuConfig) -> bool {
    menu.source
        .as_ref()
        .map(|source| matches!(source.kind, crate::config::MenuSourceKind::InstalledApps))
        .unwrap_or(false)
}

fn menu_name_for_tree(tree: &crate::surface::panel::PanelTree) -> Option<&str> {
    match &tree.content {
        crate::surface::panel::PanelContent::Menu(name) => Some(name.as_str()),
        _ => None,
    }
}

pub(super) fn dmenu_entry_views(
    request: &crate::ipc::protocol::DmenuRequest,
    query: &str,
) -> Vec<MenuEntryView> {
    let mut entries = Vec::new();
    let exact_match = request.entries.iter().any(|entry| entry == query);
    if request.allow_custom && !query.is_empty() && !exact_match {
        entries.push(MenuEntryView {
            label: query.to_string(),
            icon: None,
            path: Vec::new(),
            target: MenuEntryTarget::DmenuCustomValue(query.to_string()),
            is_submenu: false,
        });
    }

    let query = query.to_ascii_lowercase();
    entries.extend(
        request
            .entries
            .iter()
            .enumerate()
            .filter(|(_, value)| query.is_empty() || value.to_ascii_lowercase().contains(&query))
            .map(|(index, value)| MenuEntryView {
                label: value.clone(),
                icon: None,
                path: vec![index],
                target: MenuEntryTarget::DmenuValue(value.clone()),
                is_submenu: false,
            }),
    );
    entries
}

pub(super) fn current_menu_entries(
    menu: &crate::config::MenuConfig,
    path: &[usize],
) -> Vec<MenuEntryView> {
    let items = if path.is_empty() {
        &menu.items
    } else {
        menu_item_at_path(&menu.items, path)
            .map(|item| item.children.as_slice())
            .unwrap_or(&[])
    };
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let mut item_path = path.to_vec();
            item_path.push(index);
            MenuEntryView {
                label: item.label.clone(),
                icon: item.icon.clone(),
                target: MenuEntryTarget::StaticPath(item_path.clone()),
                path: item_path,
                is_submenu: !item.children.is_empty(),
            }
        })
        .collect()
}

pub(super) fn search_menu_entries(
    menu: &crate::config::MenuConfig,
    query: &str,
) -> Vec<MenuEntryView> {
    let query = query.to_ascii_lowercase();
    let mut results = Vec::new();
    collect_search_menu_entries(
        &menu.items,
        &query,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut results,
    );
    results
}

fn collect_search_menu_entries(
    items: &[crate::config::MenuItemConfig],
    query: &str,
    labels: &mut Vec<String>,
    path: &mut Vec<usize>,
    results: &mut Vec<MenuEntryView>,
) {
    for (index, item) in items.iter().enumerate() {
        labels.push(item.label.clone());
        path.push(index);
        let label_matches = item.label.to_ascii_lowercase().contains(query);
        let keyword_matches = item
            .keywords
            .iter()
            .any(|keyword| keyword.to_ascii_lowercase().contains(query));
        if label_matches || keyword_matches {
            results.push(MenuEntryView {
                label: labels.join(" / "),
                icon: item.icon.clone(),
                path: path.clone(),
                target: MenuEntryTarget::StaticPath(path.clone()),
                is_submenu: !item.children.is_empty(),
            });
        }
        collect_search_menu_entries(&item.children, query, labels, path, results);
        path.pop();
        labels.pop();
    }
}

fn menu_item_at_path<'a>(
    items: &'a [crate::config::MenuItemConfig],
    path: &[usize],
) -> Option<&'a crate::config::MenuItemConfig> {
    let (first, rest) = path.split_first()?;
    let item = items.get(*first)?;
    if rest.is_empty() {
        Some(item)
    } else {
        menu_item_at_path(&item.children, rest)
    }
}

fn installed_app_entries(
    apps: &[crate::desktop_apps::DesktopApplication],
    query: &str,
) -> Vec<MenuEntryView> {
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for app in apps {
        *counts.entry(app.name.to_ascii_lowercase()).or_default() += 1;
    }
    let query = query.to_ascii_lowercase();
    apps.iter()
        .filter(|app| query.is_empty() || installed_app_matches(app, &query))
        .enumerate()
        .map(|(index, app)| {
            let label = if counts
                .get(&app.name.to_ascii_lowercase())
                .copied()
                .unwrap_or(0)
                > 1
            {
                app.generic_name
                    .as_ref()
                    .filter(|value| !value.is_empty())
                    .map(|generic| format!("{} ({generic})", app.name))
                    .unwrap_or_else(|| format!("{} ({})", app.name, app.id))
            } else {
                app.name.clone()
            };
            let icon = app
                .icon
                .as_ref()
                .filter(|icon| crate::icon_theme::is_valid_icon_name(icon))
                .cloned();
            MenuEntryView {
                label,
                icon,
                path: vec![index],
                target: MenuEntryTarget::InstalledApp(app.id.clone()),
                is_submenu: false,
            }
        })
        .collect()
}

fn installed_app_matches(app: &crate::desktop_apps::DesktopApplication, query: &str) -> bool {
    app.name.to_ascii_lowercase().contains(query)
        || app
            .generic_name
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase()
            .contains(query)
        || app
            .comment
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase()
            .contains(query)
        || app
            .keywords
            .iter()
            .any(|keyword| keyword.to_ascii_lowercase().contains(query))
}

fn content_for_origin(
    origin: &crate::surface::panel::PanelOrigin,
) -> crate::surface::panel::PanelContent {
    match origin {
        crate::surface::panel::PanelOrigin::Bar => crate::surface::panel::PanelContent::BarPanel,
        crate::surface::panel::PanelOrigin::Module(id) => {
            crate::surface::panel::PanelContent::ModulePanel(id.clone())
        }
    }
}

pub(super) fn panel_action_needs_anchor(action: &crate::config::PanelAction) -> bool {
    matches!(
        action,
        crate::config::PanelAction::OpenPanel
            | crate::config::PanelAction::TogglePanel
            | crate::config::PanelAction::OpenMenu(_)
            | crate::config::PanelAction::ToggleMenu(_)
    )
}

pub(super) fn panel_event_for_action(
    origin_window: iced::window::Id,
    origin: crate::surface::panel::PanelOrigin,
    action: crate::config::PanelAction,
    anchor: Option<iced::Rectangle>,
) -> Option<super::messages::PanelEvent> {
    match action {
        crate::config::PanelAction::OpenPanel => {
            let content = content_for_origin(&origin);
            Some(super::messages::PanelEvent::Open {
                origin_window,
                origin,
                content,
                anchor,
            })
        }
        crate::config::PanelAction::TogglePanel => {
            let content = content_for_origin(&origin);
            Some(super::messages::PanelEvent::Toggle {
                origin_window,
                origin,
                content,
                anchor,
            })
        }
        crate::config::PanelAction::ClosePanel => Some(super::messages::PanelEvent::Close),
        crate::config::PanelAction::OpenMenu(name) => Some(super::messages::PanelEvent::Open {
            origin_window,
            origin,
            content: crate::surface::panel::PanelContent::Menu(name),
            anchor,
        }),
        crate::config::PanelAction::ToggleMenu(name) => Some(super::messages::PanelEvent::Toggle {
            origin_window,
            origin,
            content: crate::surface::panel::PanelContent::Menu(name),
            anchor,
        }),
        crate::config::PanelAction::Command(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dmenu_entries_use_input_as_label_and_value() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: Some("Run".into()),
            entries: vec!["firefox".into(), "kitty".into(), "firefox".into()],
            allow_custom: true,
        };
        let entries = dmenu_entry_views(&request, "");

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["firefox", "kitty", "firefox"]
        );
        assert!(matches!(entries[0].target, MenuEntryTarget::DmenuValue(_)));
    }

    #[test]
    fn dmenu_search_filters_entries_case_insensitively() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec!["Firefox".into(), "Kitty".into()],
            allow_custom: true,
        };
        let entries = dmenu_entry_views(&request, "fire");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].label, "Firefox");
    }

    #[test]
    fn dmenu_custom_value_entry_appears_when_query_has_no_exact_match() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec!["firefox".into()],
            allow_custom: true,
        };
        let entries = dmenu_entry_views(&request, "kitty");

        assert_eq!(entries[0].label, "kitty");
        assert!(matches!(
            entries[0].target,
            MenuEntryTarget::DmenuCustomValue(_)
        ));
    }

    #[test]
    fn dmenu_no_custom_does_not_create_custom_entry() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec!["firefox".into()],
            allow_custom: false,
        };
        let entries = dmenu_entry_views(&request, "kitty");

        assert!(entries.is_empty());
    }

    #[test]
    fn dmenu_request_rejects_when_another_request_is_pending() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        app.panel.dmenu_request = Some(crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![],
            allow_custom: true,
        });
        let (tx, mut rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec!["x".into()],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(tx),
        });

        let response = rx.try_recv().unwrap();
        assert!(matches!(
            response,
            crate::ipc::protocol::IpcResponse::Rejected { .. }
        ));
    }

    #[test]
    fn completing_dmenu_selection_sends_selected_response() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        app.panel.dmenu_request = Some(crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec!["firefox".into()],
            allow_custom: true,
        });
        app.panel.dmenu_responder = Some(crate::app::messages::ipc_responder(tx));

        app.complete_dmenu_request(crate::ipc::protocol::IpcResponse::Selected {
            value: "firefox".into(),
        });

        assert_eq!(
            rx.try_recv().unwrap(),
            crate::ipc::protocol::IpcResponse::Selected {
                value: "firefox".into()
            }
        );
        assert!(app.panel.dmenu_request.is_none());
    }

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
    fn opening_panel_clears_origin_tooltip_pending() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin).tooltip_pending =
            Some((crate::modules::ModuleId::new("clock"), 0.0));

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new(
                "clock",
            )),
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            anchor: None,
        }));

        assert!(app.surface(origin).unwrap().tooltip_pending.is_none());
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

    #[test]
    fn left_edge_panel_opens_right_of_bar_and_aligns_to_anchor_y() {
        let position = panel_position_for_test(
            crate::config::Edge::Left,
            48,
            (360, 600),
            Some(iced::Rectangle {
                x: 0.0,
                y: 100.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
            (0, 0),
        );

        assert_eq!(position.0, 48);
        assert!(position.1 >= 0);
    }

    #[test]
    fn right_edge_panel_opens_left_of_bar_and_aligns_to_anchor_y() {
        let position = panel_position_for_test(
            crate::config::Edge::Right,
            48,
            (360, 600),
            Some(iced::Rectangle {
                x: 1872.0,
                y: 100.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
            (1872, 0),
        );

        assert_eq!(position.0, 1512);
        assert!(position.1 >= 0);
    }

    #[test]
    fn top_edge_panel_opens_below_bar_and_aligns_to_anchor_x() {
        let position = panel_position_for_test(
            crate::config::Edge::Top,
            32,
            (360, 600),
            Some(iced::Rectangle {
                x: 100.0,
                y: 0.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
            (0, 0),
        );

        assert!(position.0 >= 0);
        assert_eq!(position.1, 32);
    }

    #[test]
    fn bottom_edge_panel_opens_above_bar_and_aligns_to_anchor_x() {
        let position = panel_position_for_test(
            crate::config::Edge::Bottom,
            32,
            (360, 600),
            Some(iced::Rectangle {
                x: 100.0,
                y: 1048.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
            (0, 0),
        );

        assert!(position.0 >= 0);
        assert_eq!(position.1, 448);
    }

    #[test]
    fn panel_position_top_centers_on_anchor_and_clamps() {
        let position = super::panel_position_for_test(
            crate::config::Edge::Top,
            48,
            (300, 600),
            Some(iced::Rectangle {
                x: 480.0,
                y: 4.0,
                width: 40.0,
                height: 40.0,
            }),
            (1920, 1080),
            (0, 0),
        );

        assert_eq!(position, (350, 48));
    }

    #[test]
    fn left_edge_bar_panel_starts_at_vertical_bar_top_when_top_bar_reserves_space() {
        let position = panel_position_for_test(
            crate::config::Edge::Left,
            48,
            (360, 600),
            None,
            (48, 1048),
            (0, 32),
        );

        assert_eq!(position, (48, 32));
    }

    #[test]
    fn side_panel_origin_includes_top_bars_on_same_output() {
        let mut config = crate::config::Config::default();
        config.bars[0].name = "top".into();
        config.bars[0].edge = crate::config::Edge::Top;
        config.bars[0].collapsed_width = 32;
        let side = crate::config::BarConfig {
            name: "side".into(),
            edge: crate::config::Edge::Left,
            collapsed_width: 48,
            ..Default::default()
        };
        config.bars.push(side);
        let (app, _) = App::boot(config, vec!["DP-1".into()]);
        let origin_surface = crate::surface::SurfaceState::new("DP-1".into(), "side".into());
        let bar = App::bar_for_surface_name(&app.config.bars, "side");

        assert_eq!(
            app.inferred_panel_surface_origin(&origin_surface, bar),
            (0, 32)
        );
    }

    #[test]
    fn configured_module_click_open_panel_takes_precedence_over_builtin_click() {
        let legacy_path = format!("/tmp/niribar-panel-legacy-click-{}", std::process::id());
        let _ = std::fs::remove_file(&legacy_path);

        let mut config = crate::config::Config {
            modules: vec![crate::config::ModuleConfig {
                name: "button".into(),
                module_type: "button".into(),
                params: crate::config::ModuleParams::Button(crate::config::ButtonConfig {
                    label: "Launch".into(),
                    icon: ">".into(),
                    on_click: crate::config::CommandConfig::Shell(format!(
                        "printf legacy >{legacy_path}"
                    )),
                    format: crate::config::FormatConfig::default(),
                    tooltip: crate::config::TooltipSetting::Default,
                }),
                actions: crate::config::ModuleActionsConfig {
                    bindings: vec![crate::config::ActionBinding {
                        trigger: crate::config::ActionTrigger::Click,
                        action: crate::config::PanelAction::OpenPanel,
                    }],
                },
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["button".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.update(Message::Modules(
            crate::app::messages::ModuleRuntimeEvent::ModuleClick(
                crate::modules::ModuleId::new("button"),
                origin,
            ),
        ));

        assert!(
            !std::path::Path::new(&legacy_path).exists(),
            "legacy button command should not run when open-panel action is configured"
        );
    }

    #[test]
    fn panel_event_for_module_open_panel_carries_anchor() {
        let anchor = iced::Rectangle {
            x: 8.0,
            y: 16.0,
            width: 24.0,
            height: 32.0,
        };
        let event = panel_event_for_action(
            iced::window::Id::unique(),
            crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new("button")),
            crate::config::PanelAction::OpenPanel,
            Some(anchor),
        )
        .expect("panel event");

        match event {
            crate::app::messages::PanelEvent::Open {
                origin,
                content,
                anchor: event_anchor,
                ..
            } => {
                assert_eq!(
                    origin,
                    crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new(
                        "button"
                    ))
                );
                assert_eq!(
                    content,
                    crate::surface::panel::PanelContent::ModulePanel(
                        crate::modules::ModuleId::new("button")
                    )
                );
                assert_eq!(event_anchor, Some(anchor));
            }
            other => panic!("expected open event, got {other:?}"),
        }
    }

    fn menu_with_apps_browser() -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "main".into(),
            show_icons: false,
            prompt: None,
            source: None,
            items: vec![crate::config::MenuItemConfig {
                label: "Apps".into(),
                icon: Some("applications-other".into()),
                keywords: vec![],
                action: None,
                children: vec![crate::config::MenuItemConfig {
                    label: "Browser".into(),
                    icon: Some("firefox".into()),
                    keywords: vec!["web".into()],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::CommandConfig::Shell("firefox".into()),
                    )),
                    children: vec![],
                }],
            }],
        }
    }

    fn menu_with_two_entries() -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "main".into(),
            show_icons: false,
            prompt: None,
            source: None,
            items: vec![
                crate::config::MenuItemConfig {
                    label: "Terminal".into(),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![],
                },
                crate::config::MenuItemConfig {
                    label: "Browser".into(),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![],
                },
            ],
        }
    }

    fn menu_with_entries(count: usize) -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "main".into(),
            show_icons: false,
            prompt: None,
            source: None,
            items: (0..count)
                .map(|index| crate::config::MenuItemConfig {
                    label: format!("Entry {index}"),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![],
                })
                .collect(),
        }
    }

    fn config_with_menu(menu: crate::config::MenuConfig) -> crate::config::Config {
        crate::config::Config {
            menus: vec![menu],
            ..crate::config::Config::default()
        }
    }

    fn register_menu_panel(app: &mut App, panel: iced::window::Id, origin: iced::window::Id) {
        app.register_panel_popup(
            panel,
            origin,
            crate::surface::panel::PanelTree {
                origin: crate::surface::panel::PanelOrigin::Bar,
                content: crate::surface::panel::PanelContent::Menu("main".into()),
                origin_window: origin,
                origin_bar_name: "main".into(),
                origin_output_name: "DP-1".into(),
                anchor: None,
                screen_size: (1920, 1080),
            },
        );
    }

    #[test]
    fn menu_search_flattens_nested_matches_with_breadcrumb() {
        let menu = menu_with_apps_browser();
        let results = search_menu_entries(&menu, "browser");

        assert_eq!(results[0].label, "Apps / Browser");
        assert_eq!(results[0].path, vec![0, 0]);
        assert_eq!(results[0].icon, Some("firefox".into()));
    }

    #[test]
    fn menu_search_matches_keywords_case_insensitively() {
        let menu = menu_with_apps_browser();
        let results = search_menu_entries(&menu, "WEB");

        assert_eq!(results[0].label, "Apps / Browser");
    }

    fn installed_app(name: &str, id: &str) -> crate::desktop_apps::DesktopApplication {
        crate::desktop_apps::DesktopApplication {
            id: id.into(),
            name: name.into(),
            generic_name: None,
            comment: None,
            keywords: vec![],
            icon: None,
            exec: name.to_ascii_lowercase(),
        }
    }

    fn app_with_installed_apps_menu(apps: Vec<crate::desktop_apps::DesktopApplication>) -> App {
        let config = config_with_menu(crate::config::MenuConfig {
            name: "apps".into(),
            show_icons: false,
            prompt: None,
            source: Some(crate::config::MenuSourceConfig {
                kind: crate::config::MenuSourceKind::InstalledApps,
            }),
            items: vec![],
        });
        let (mut app, _) = App::boot(config, vec![]);
        app.panel.installed_apps_for_test = Some(apps);
        app
    }

    #[test]
    fn installed_apps_hover_render_path_does_not_rebuild_large_entry_list() {
        let apps = (0..5_000)
            .map(|index| installed_app(&format!("App {index:04}"), &format!("app-{index}.desktop")))
            .collect::<Vec<_>>();
        let mut app = app_with_installed_apps_menu(apps);
        open_apps_menu_panel(&mut app);
        let panel_window = app.surfaces.panel_popup_ids().next().expect("panel window");

        let started = std::time::Instant::now();
        for index in 0..100 {
            let _ = app.update(Message::Panel(
                crate::app::messages::PanelEvent::FocusIndex(index),
            ));
            let model = app
                .panel_render_model(panel_window)
                .expect("panel render model");
            assert_eq!(model.menu_entries.len(), 5_000);
            assert_eq!(model.focused_index, Some(index));
        }

        assert!(
            started.elapsed() < std::time::Duration::from_millis(25),
            "hover render path rebuilt too much installed-app menu state: {:?}",
            started.elapsed()
        );
    }

    fn open_apps_menu_panel(app: &mut App) {
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("apps".into()),
            anchor: None,
        }));
    }

    #[test]
    fn installed_apps_menu_entries_use_open_snapshot() {
        let mut app = app_with_installed_apps_menu(vec![
            installed_app("Alpha", "alpha.desktop"),
            installed_app("Zed", "zed.desktop"),
        ]);

        open_apps_menu_panel(&mut app);

        let entries = app.active_menu_entries();
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha", "Zed"]
        );
    }

    #[test]
    fn installed_apps_search_matches_keywords_and_comment() {
        let mut browser = installed_app("Browser", "browser.desktop");
        browser.comment = Some("Browse the web".into());
        browser.keywords = vec!["internet".into()];
        let mut app =
            app_with_installed_apps_menu(vec![browser, installed_app("Editor", "editor.desktop")]);
        open_apps_menu_panel(&mut app);

        app.panel.search_query = "internet".into();

        let entries = app.active_menu_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Browser");
    }

    #[test]
    fn installed_apps_entries_carry_valid_icons_only() {
        let mut alpha = installed_app("Alpha", "alpha.desktop");
        alpha.icon = Some("alpha-icon".into());
        let mut beta = installed_app("Beta", "beta.desktop");
        beta.icon = Some("/usr/share/icons/beta.png".into());
        let mut app = app_with_installed_apps_menu(vec![alpha, beta]);
        open_apps_menu_panel(&mut app);

        let entries = app.active_menu_entries();
        assert_eq!(entries[0].icon, Some("alpha-icon".into()));
        assert_eq!(entries[1].icon, None);
    }

    #[test]
    fn activating_installed_app_closes_panel() {
        let mut app = app_with_installed_apps_menu(vec![installed_app("Alpha", "alpha.desktop")]);
        open_apps_menu_panel(&mut app);
        assert_eq!(app.surfaces.panel_popup_ids().count(), 1);

        let _ = app.update(Message::Panel(
            crate::app::messages::PanelEvent::ActivateIndex(0),
        ));

        assert_eq!(
            app.panel.launched_installed_app_ids_for_test,
            vec!["alpha.desktop"]
        );
        assert_eq!(app.surfaces.panel_popup_ids().count(), 0);
    }

    #[test]
    fn duplicate_installed_app_names_are_disambiguated() {
        let mut app = app_with_installed_apps_menu(vec![
            installed_app("Terminal", "org.gnome.Terminal.desktop"),
            installed_app("Terminal", "com.raggesilver.BlackBox.desktop"),
        ]);
        open_apps_menu_panel(&mut app);

        let entries = app.active_menu_entries();

        assert_eq!(entries.len(), 2);
        assert!(entries[0].label.contains("Terminal"));
        assert!(entries[1].label.contains("Terminal"));
        assert_ne!(entries[0].label, entries[1].label);
    }

    #[test]
    fn installed_apps_menu_can_be_empty() {
        let mut app = app_with_installed_apps_menu(vec![]);
        open_apps_menu_panel(&mut app);

        assert_eq!(app.surfaces.panel_popup_ids().count(), 1);
        assert!(app.active_menu_entries().is_empty());
    }

    #[test]
    fn arrow_down_on_panel_window_moves_menu_focus_to_next_entry() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.panel.focused_index = Some(0);
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(app.panel.focused_index, Some(1));
    }

    #[test]
    fn arrow_down_on_panel_window_does_not_scroll_when_focused_entry_is_visible() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.panel.focused_index = Some(0);
        app.panel.menu_scroll_y = 0.0;
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(app.panel.focused_index, Some(1));
        assert_eq!(app.panel.menu_scroll_y, 0.0);
    }

    #[test]
    fn arrow_down_on_panel_window_scrolls_only_enough_to_reveal_focused_entry() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.panel.focused_index = Some(40);
        app.panel.menu_scroll_y = 0.0;
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(app.panel.focused_index, Some(41));
        assert_eq!(
            app.panel.menu_scroll_y,
            42.0 * MENU_ROW_EXTENT - DEFAULT_MENU_VIEWPORT_HEIGHT
        );
    }

    #[test]
    fn arrow_up_on_panel_window_scrolls_only_when_focused_entry_is_above_view() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.panel.focused_index = Some(10);
        app.panel.menu_scroll_y = 10.0 * MENU_ROW_EXTENT;
        app.panel.menu_viewport_height = DEFAULT_MENU_VIEWPORT_HEIGHT;
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMovePrev(panel));

        assert_eq!(app.panel.focused_index, Some(9));
        assert_eq!(app.panel.menu_scroll_y, 9.0 * MENU_ROW_EXTENT);
    }

    #[test]
    fn focus_gained_on_panel_window_does_not_create_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::BarFocusGained(panel));

        assert!(app.surface(panel).is_none());
        assert_eq!(app.panel.focused_index, Some(0));
    }

    #[test]
    fn focus_search_input_event_initializes_menu_focus_without_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);
        app.panel.search_focus_pending = true;

        let _ = app.handle_panel_event(super::super::messages::PanelEvent::FocusSearchInput {
            retries: 0,
        });

        assert!(app.surface(panel).is_none());
        assert_eq!(app.panel.focused_index, Some(0));
        assert!(!app.panel.search_focus_pending);
    }

    #[test]
    fn hover_focus_cancels_pending_search_focus_retries() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);
        app.panel.search_focus_pending = true;

        let _ = app.handle_panel_event(super::super::messages::PanelEvent::FocusIndex(1));

        assert_eq!(app.panel.focused_index, Some(1));
        assert!(!app.panel.search_focus_pending);
    }

    #[test]
    fn window_sized_on_panel_window_routes_to_panel_focus_without_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::WindowSized(
            panel, 360, 600,
        ));

        assert!(app.surface(panel).is_none());
        assert_eq!(app.panel.focused_index, Some(0));
    }

    #[test]
    fn close_all_panels_closes_tracked_and_untracked_panel_popups() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let tracked_panel = iced::window::Id::unique();
        let untracked_panel = iced::window::Id::unique();
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            origin_window: origin,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (1920, 1080),
        };
        app.overlays.panel_opened(tracked_panel, origin);
        app.surfaces.insert_panel_popup(
            tracked_panel,
            super::super::surfaces::PanelMeta {
                tree: tree.clone(),
                search_input_id: panel_search_input_id(tracked_panel),
            },
        );
        app.surfaces.insert_panel_popup(
            untracked_panel,
            super::super::surfaces::PanelMeta {
                tree,
                search_input_id: panel_search_input_id(untracked_panel),
            },
        );

        let tasks = app.close_all_panels();

        assert!(tasks.len() >= 2);
        assert!(app.surfaces.panel_popup(tracked_panel).is_none());
        assert!(app.surfaces.panel_popup(untracked_panel).is_none());
        assert!(app.overlays.panel(tracked_panel).is_none());
    }

    #[test]
    fn stale_events_from_closing_panel_window_do_not_create_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);

        let _ = app.close_all_panels();
        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::BarFocusLost(panel));
        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::WindowSized(
            panel, 360, 600,
        ));

        assert!(app.surface(panel).is_none());
    }

    #[test]
    fn escape_clears_search_before_closing_panel() {
        let mut panel = PanelSystem {
            search_query: "term".into(),
            ..PanelSystem::default()
        };

        assert!(panel.escape_clears_search());
        assert_eq!(panel.search_query, "");
    }

    #[test]
    fn panel_search_input_id_is_stable_for_focus_task() {
        let first = iced::window::Id::unique();
        let second = iced::window::Id::unique();

        assert_eq!(panel_search_input_id(first), panel_search_input_id(first));
        assert_ne!(panel_search_input_id(first), panel_search_input_id(second));
    }
}
