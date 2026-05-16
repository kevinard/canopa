use iced::Task;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use std::collections::HashSet;
use std::time::Duration;

use super::{App, Message};

pub(super) fn panel_search_input_id(window: iced::window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("panel-search-input-{window:?}"))
}

fn request_panel_search_focus(retries: u8) -> Task<Message> {
    Task::done(Message::Panel(
        super::messages::PanelEvent::FocusSearchInput { retries },
    ))
}

#[derive(Debug, Default)]
pub(super) struct PanelSystem {
    pub(super) search_query: String,
    pub(super) focused_index: Option<usize>,
    pub(super) menu_path: Vec<usize>,
    pub(super) installed_apps_snapshot: Vec<crate::desktop_apps::DesktopApplication>,
    #[cfg(test)]
    pub(super) installed_apps_for_test: Option<Vec<crate::desktop_apps::DesktopApplication>>,
    #[cfg(test)]
    pub(super) launched_installed_app_ids_for_test: Vec<String>,
    closing_windows: HashSet<iced::window::Id>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MenuEntryTarget {
    StaticPath(Vec<usize>),
    InstalledApp(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MenuEntryView {
    pub(super) label: String,
    pub(super) path: Vec<usize>,
    pub(super) target: MenuEntryTarget,
    pub(super) is_submenu: bool,
}

impl PanelSystem {
    pub(super) fn clear(&mut self) {
        self.search_query.clear();
        self.focused_index = None;
        self.menu_path.clear();
        self.installed_apps_snapshot.clear();
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
    let anchor = match bar.edge {
        crate::config::Edge::Top => Anchor::Top | Anchor::Left,
        crate::config::Edge::Bottom => Anchor::Bottom | Anchor::Left,
        crate::config::Edge::Left => Anchor::Left | Anchor::Top,
        crate::config::Edge::Right => Anchor::Right | Anchor::Top,
    };
    let position = panel_position(
        &bar.edge,
        bar.collapsed_width,
        (width, height),
        tree.anchor,
        tree.screen_size,
        surface_origin,
    );
    let margin = panel_margin(&bar.edge, position, (width, height), tree.screen_size);

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

fn panel_position(
    edge: &crate::config::Edge,
    bar_size: u32,
    panel_size: (u32, u32),
    anchor: Option<iced::Rectangle>,
    screen_size: (u32, u32),
    surface_origin: (i32, i32),
) -> (i32, i32) {
    let bar_size = bar_size as i32;
    let (panel_w, panel_h) = (panel_size.0 as i32, panel_size.1 as i32);
    let (screen_w, screen_h) = (screen_size.0 as i32, screen_size.1 as i32);
    let anchor = anchor.unwrap_or(iced::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    });
    let anchor_center_x = surface_origin.0 + (anchor.x + anchor.width / 2.0).round() as i32;
    let anchor_center_y = surface_origin.1 + (anchor.y + anchor.height / 2.0).round() as i32;
    let min_x = surface_origin.0;
    let min_y = surface_origin.1;
    let max_x = (surface_origin.0 + screen_w - panel_w).max(min_x);
    let max_y = (surface_origin.1 + screen_h - panel_h).max(min_y);

    match edge {
        crate::config::Edge::Left => (
            bar_size,
            (anchor_center_y - panel_h / 2).clamp(min_y, max_y),
        ),
        crate::config::Edge::Right => (
            (screen_w - bar_size - panel_w).max(0),
            (anchor_center_y - panel_h / 2).clamp(min_y, max_y),
        ),
        crate::config::Edge::Top => (
            (anchor_center_x - panel_w / 2).clamp(min_x, max_x),
            bar_size,
        ),
        crate::config::Edge::Bottom => (
            (anchor_center_x - panel_w / 2).clamp(min_x, max_x),
            (screen_h - bar_size - panel_h).max(0),
        ),
    }
}

fn panel_margin(
    edge: &crate::config::Edge,
    position: (i32, i32),
    panel_size: (u32, u32),
    screen_size: (u32, u32),
) -> (i32, i32, i32, i32) {
    let (x, y) = position;
    let (panel_w, panel_h) = (panel_size.0 as i32, panel_size.1 as i32);
    let (screen_w, screen_h) = (screen_size.0 as i32, screen_size.1 as i32);

    match edge {
        crate::config::Edge::Top => (y, 0, 0, x),
        crate::config::Edge::Bottom => (0, 0, (screen_h - y - panel_h).max(0), x),
        crate::config::Edge::Left => (y, 0, 0, x),
        crate::config::Edge::Right => (y, (screen_w - x - panel_w).max(0), 0, 0),
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
    panel_position(
        &edge,
        bar_size,
        panel_size,
        anchor,
        screen_size,
        surface_origin,
    )
}

impl App {
    fn suspend_bar_keyboard_interactivity(&mut self) -> Vec<Task<Message>> {
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
        self.close_all_panels_with_restore(true)
    }

    fn close_all_panels_for_replacement(&mut self) -> Vec<Task<Message>> {
        self.close_all_panels_with_restore(false)
    }

    fn close_all_panels_with_restore(
        &mut self,
        restore_bar_interactivity: bool,
    ) -> Vec<Task<Message>> {
        let ids: Vec<_> = self.surfaces.panel_popup_ids().collect();
        self.panel.clear();
        let mut tasks: Vec<_> = ids
            .into_iter()
            .map(|id| {
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
        ) {
            self.panel.focused_index.get_or_insert(0);
            return request_panel_search_focus(2);
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
                Task::none()
            }
            PanelEvent::FocusMoveNext => {
                self.move_panel_focus(1);
                Task::none()
            }
            PanelEvent::FocusMovePrev => {
                self.move_panel_focus(-1);
                Task::none()
            }
            PanelEvent::FocusIndex(index) => {
                self.panel.focused_index = Some(index);
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

        let mut tasks = self.close_tooltip_popup_if_open(origin_window);
        tasks.extend(self.close_all_menu_popups());
        tasks.extend(self.close_all_panels_for_replacement());
        tasks.extend(self.suspend_bar_keyboard_interactivity());
        let is_menu_panel = matches!(tree.content, crate::surface::panel::PanelContent::Menu(_));
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
        }
        self.surfaces.insert_panel_popup(
            panel_window,
            super::surfaces::PanelMeta {
                tree,
                search_input_id: panel_search_input_id(panel_window),
            },
        );

        let mut open_sequence = open_task.chain(iced::window::gain_focus(panel_window));
        if is_menu_panel {
            self.panel.focused_index = Some(0);
            open_sequence = open_sequence.chain(request_panel_search_focus(2));
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
        }
    }

    pub(super) fn active_menu_entries(&self) -> Vec<MenuEntryView> {
        self.active_menu_tree_and_entries()
            .map(|(_, entries)| entries)
            .unwrap_or_default()
    }

    fn focus_panel_search_input(&mut self, retries: u8) -> Task<Message> {
        let Some(search_input_id) = self.surfaces.panel_popup_values().find_map(|meta| {
            if matches!(
                meta.tree.content,
                crate::surface::panel::PanelContent::Menu(_)
            ) {
                Some(meta.search_input_id.clone())
            } else {
                None
            }
        }) else {
            return Task::none();
        };
        self.panel.focused_index.get_or_insert(0);
        let mut tasks = vec![iced::widget::operation::focus(search_input_id)];
        if retries > 0 {
            tasks.push(Task::perform(
                async {
                    tokio::time::sleep(Duration::from_millis(8)).await;
                },
                move |_| {
                    Message::Panel(super::messages::PanelEvent::FocusSearchInput {
                        retries: retries - 1,
                    })
                },
            ));
        }
        Task::batch(tasks)
    }

    fn active_menu_tree_and_entries(
        &self,
    ) -> Option<(crate::surface::panel::PanelTree, Vec<MenuEntryView>)> {
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
        let entries = match &menu.source {
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
        };
        Some((tree, entries))
    }
}

fn menu_name_for_tree(tree: &crate::surface::panel::PanelTree) -> Option<&str> {
    match &tree.content {
        crate::surface::panel::PanelContent::Menu(name) => Some(name.as_str()),
        _ => None,
    }
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
            MenuEntryView {
                label,
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
            source: None,
            items: vec![crate::config::MenuItemConfig {
                label: "Apps".into(),
                keywords: vec![],
                action: None,
                children: vec![crate::config::MenuItemConfig {
                    label: "Browser".into(),
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
            source: None,
            items: vec![
                crate::config::MenuItemConfig {
                    label: "Terminal".into(),
                    keywords: vec![],
                    action: None,
                    children: vec![],
                },
                crate::config::MenuItemConfig {
                    label: "Browser".into(),
                    keywords: vec![],
                    action: None,
                    children: vec![],
                },
            ],
        }
    }

    #[test]
    fn menu_search_flattens_nested_matches_with_breadcrumb() {
        let menu = menu_with_apps_browser();
        let results = search_menu_entries(&menu, "browser");

        assert_eq!(results[0].label, "Apps / Browser");
        assert_eq!(results[0].path, vec![0, 0]);
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
        let mut config = crate::config::Config::default();
        config.menus = vec![crate::config::MenuConfig {
            name: "apps".into(),
            source: Some(crate::config::MenuSourceConfig {
                kind: crate::config::MenuSourceKind::InstalledApps,
            }),
            items: vec![],
        }];
        let (mut app, _) = App::boot(config, vec![]);
        app.panel.installed_apps_for_test = Some(apps);
        app
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
        let mut config = crate::config::Config::default();
        config.menus = vec![menu_with_two_entries()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.panel.focused_index = Some(0);
        app.surfaces.insert_panel_popup(
            panel,
            super::super::surfaces::PanelMeta {
                tree: crate::surface::panel::PanelTree {
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::Menu("main".into()),
                    origin_window: origin,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    anchor: None,
                    screen_size: (1920, 1080),
                },
                search_input_id: panel_search_input_id(panel),
            },
        );

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(app.panel.focused_index, Some(1));
    }

    #[test]
    fn focus_gained_on_panel_window_does_not_create_bar_surface_state() {
        let mut config = crate::config::Config::default();
        config.menus = vec![menu_with_two_entries()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.surfaces.insert_panel_popup(
            panel,
            super::super::surfaces::PanelMeta {
                tree: crate::surface::panel::PanelTree {
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::Menu("main".into()),
                    origin_window: origin,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    anchor: None,
                    screen_size: (1920, 1080),
                },
                search_input_id: panel_search_input_id(panel),
            },
        );

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::BarFocusGained(panel));

        assert!(app.surface(panel).is_none());
        assert_eq!(app.panel.focused_index, Some(0));
    }

    #[test]
    fn focus_search_input_event_initializes_menu_focus_without_bar_surface_state() {
        let mut config = crate::config::Config::default();
        config.menus = vec![menu_with_two_entries()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.surfaces.insert_panel_popup(
            panel,
            super::super::surfaces::PanelMeta {
                tree: crate::surface::panel::PanelTree {
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::Menu("main".into()),
                    origin_window: origin,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    anchor: None,
                    screen_size: (1920, 1080),
                },
                search_input_id: panel_search_input_id(panel),
            },
        );

        let _ = app.handle_panel_event(super::super::messages::PanelEvent::FocusSearchInput {
            retries: 0,
        });

        assert!(app.surface(panel).is_none());
        assert_eq!(app.panel.focused_index, Some(0));
    }

    #[test]
    fn window_sized_on_panel_window_routes_to_panel_focus_without_bar_surface_state() {
        let mut config = crate::config::Config::default();
        config.menus = vec![menu_with_two_entries()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.surfaces.insert_panel_popup(
            panel,
            super::super::surfaces::PanelMeta {
                tree: crate::surface::panel::PanelTree {
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::Menu("main".into()),
                    origin_window: origin,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    anchor: None,
                    screen_size: (1920, 1080),
                },
                search_input_id: panel_search_input_id(panel),
            },
        );

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::WindowSized(
            panel, 360, 600,
        ));

        assert!(app.surface(panel).is_none());
        assert_eq!(app.panel.focused_index, Some(0));
    }

    #[test]
    fn stale_events_from_closing_panel_window_do_not_create_bar_surface_state() {
        let mut config = crate::config::Config::default();
        config.menus = vec![menu_with_two_entries()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.surfaces.insert_panel_popup(
            panel,
            super::super::surfaces::PanelMeta {
                tree: crate::surface::panel::PanelTree {
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::Menu("main".into()),
                    origin_window: origin,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    anchor: None,
                    screen_size: (1920, 1080),
                },
                search_input_id: panel_search_input_id(panel),
            },
        );

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
