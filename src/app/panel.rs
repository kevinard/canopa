use iced::Task;
use iced_layershell::reexport::{
    KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use std::time::Duration;

use super::{App, Message};

#[cfg(test)]
use super::panel_search::{self, SearchField, SearchMatch, SearchText};
#[cfg(test)]
type MenuEntryTarget = super::panel_menu_session::PanelMenuEntryTarget;
#[cfg(test)]
type MenuEntryView = super::panel_menu_session::PanelMenuEntry;

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
        keyboard_interactivity: KeyboardInteractivity::OnDemand,
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
        let mut tasks = Vec::new();
        let effects = self
            .panel
            .handle(super::panel_runtime::PanelRuntimeIntent::CancelDmenu);
        if !effects.is_empty() {
            tasks.push(self.apply_panel_runtime_effects(effects));
        }
        let effects = self.surfaces.close_panel_overlays();
        if !effects.is_empty() {
            self.panel.clear();
        }
        tasks.extend(self.apply_registry_effects(effects));
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
            let effects = self
                .panel
                .handle(super::panel_runtime::PanelRuntimeIntent::RequestSearchFocus);
            return self.apply_panel_runtime_effects(effects);
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
                let request = super::panel_runtime::PanelOpenRequest {
                    origin_window,
                    origin,
                    content,
                    anchor,
                };
                let catalog =
                    super::panel_runtime::PanelMenuCatalog::new(self.config.menus.as_slice());
                let module_detail_session = (!self.panel.would_toggle_close(&request))
                    .then(|| self.module_detail_session_for(&request.content))
                    .flatten();
                let effects = self
                    .panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::Toggle {
                        module_detail_session,
                        request,
                        catalog,
                    });
                self.apply_panel_runtime_effects(effects)
            }
            PanelEvent::Close => Task::batch(self.close_all_panels()),
            PanelEvent::SearchChanged(query) => {
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::SearchChanged(
                        query,
                    ));
                Task::none()
            }
            PanelEvent::MenuScrolled { y, viewport_height } => {
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::MenuScrolled {
                        y,
                        viewport_height,
                    });
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
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::FocusIndex(index));
                Task::none()
            }
            PanelEvent::FocusSearchInput { retries } => self.focus_panel_search_input(retries),
            PanelEvent::Activate => self.activate_panel_entry(None),
            PanelEvent::ActivateIndex(index) => self.activate_panel_entry(Some(index)),
            PanelEvent::ClockMonthPrevious => {
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::BrowseClockMonth(
                        -1,
                    ));
                Task::none()
            }
            PanelEvent::ClockMonthNext => {
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::BrowseClockMonth(
                        1,
                    ));
                Task::none()
            }
            PanelEvent::ClockMonthToday => {
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::ResetClockMonth);
                Task::none()
            }
            PanelEvent::Back => {
                self.panel
                    .handle(super::panel_runtime::PanelRuntimeIntent::Back);
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
        let request = super::panel_runtime::PanelOpenRequest {
            origin_window,
            origin,
            content,
            anchor,
        };
        let catalog = super::panel_runtime::PanelMenuCatalog::new(self.config.menus.as_slice());
        let effects = self
            .panel
            .handle(super::panel_runtime::PanelRuntimeIntent::Open {
                module_detail_session: self.module_detail_session_for(&request.content),
                request,
                catalog,
            });
        self.apply_panel_runtime_effects(effects)
    }

    fn module_detail_session_for(
        &self,
        content: &crate::surface::panel::PanelContent,
    ) -> Option<crate::modules::ModuleDetailSession> {
        let crate::surface::panel::PanelContent::ModulePanel(id) = content else {
            return None;
        };
        let module = self.modules.get(id)?;
        let state = self.modules.state(id).cloned().unwrap_or_default();
        module.create_module_detail_session(&state)
    }

    fn open_panel_window(
        &mut self,
        request: super::panel_runtime::PanelOpenRequest,
    ) -> Task<Message> {
        let origin_surface = self
            .surface(request.origin_window)
            .cloned()
            .unwrap_or_default();
        let tree = crate::surface::panel::PanelTree {
            origin: request.origin.clone(),
            content: request.content.clone(),
            origin_window: request.origin_window,
            origin_bar_name: origin_surface.bar_name.clone(),
            origin_output_name: origin_surface.output_name.clone(),
            anchor: request.anchor,
            screen_size: origin_surface.screen_size,
        };
        let bar = Self::bar_for_surface_name(&self.config.bars, &origin_surface.bar_name);
        let surface_origin = self.inferred_panel_surface_origin(&origin_surface, bar);
        let settings = panel_window_settings(bar, &tree, surface_origin);
        let (panel_window, open_task) = Message::layershell_open(settings);
        self.surface_mut(request.origin_window).tooltip_pending = None;
        self.surfaces.clear_pending_menu_popup();

        let panel_meta = super::surfaces::PanelMeta {
            tree: tree.clone(),
            search_input_id: panel_search_input_id(panel_window),
        };
        let effects =
            self.surfaces
                .open_panel_overlay(request.origin_window, panel_window, panel_meta);
        let mut tasks = self.apply_registry_effects(effects);
        let catalog = super::panel_runtime::PanelMenuCatalog::new(self.config.menus.as_slice());
        self.panel.handle(
            super::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
                window: panel_window,
                origin_window: request.origin_window,
                content: tree.content.clone(),
                catalog,
            },
        );

        let open_sequence = open_task.chain(iced::window::gain_focus(panel_window));
        tasks.push(open_sequence);
        Task::batch(tasks)
    }

    fn apply_panel_runtime_effects(
        &mut self,
        effects: Vec<super::panel_runtime::PanelRuntimeEffect>,
    ) -> Task<Message> {
        let mut tasks = Vec::new();
        for effect in effects {
            match effect {
                super::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(request) => {
                    tasks.push(self.open_panel_window(request));
                }
                super::panel_runtime::PanelRuntimeEffect::ClosePanelWindow(window) => {
                    self.panel.mark_closing_window(window);
                    self.surfaces.remove_panel_popup(window);
                    tasks.push(Task::done(Message::RemoveWindow(window)));
                }
                super::panel_runtime::PanelRuntimeEffect::RequestSearchFocus => {
                    tasks.push(request_panel_search_focus());
                }
                super::panel_runtime::PanelRuntimeEffect::LoadInstalledAppsThenOpenPanel(
                    pending,
                ) => {
                    let apps = self.load_installed_apps_for_panel();
                    let catalog =
                        super::panel_runtime::PanelMenuCatalog::new(self.config.menus.as_slice());
                    let effects = self.panel.handle(
                        super::panel_runtime::PanelRuntimeIntent::InstalledAppsLoaded {
                            pending,
                            apps,
                            catalog,
                        },
                    );
                    tasks.push(self.apply_panel_runtime_effects(effects));
                }
                super::panel_runtime::PanelRuntimeEffect::RunPanelAction(action) => {
                    let task = self.dispatch_panel_action_from_active_panel(action);
                    tasks.push(task);
                    tasks.extend(self.close_all_panels());
                }
                super::panel_runtime::PanelRuntimeEffect::LaunchInstalledApp(id) => {
                    self.launch_installed_app_by_id(&id);
                    tasks.extend(self.close_all_panels());
                }
                super::panel_runtime::PanelRuntimeEffect::SendDmenuResponse {
                    responder,
                    response,
                } => {
                    send_ipc_response(responder, response);
                }
            }
        }
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
        let effects = self
            .panel
            .handle(super::panel_runtime::PanelRuntimeIntent::Escape);
        if effects.is_empty() {
            self.handle_panel_event(super::messages::PanelEvent::Close)
        } else {
            self.apply_panel_runtime_effects(effects)
        }
    }

    fn move_panel_focus(&mut self, delta: isize) {
        self.panel
            .handle(super::panel_runtime::PanelRuntimeIntent::FocusMove(delta));
    }

    fn scroll_panel_menu_to_focus(&mut self) -> Task<Message> {
        let Some(scroll_id) = self
            .surfaces
            .panel_popup_ids()
            .next()
            .map(panel_menu_scroll_id)
        else {
            return Task::none();
        };
        let Some(y) = self
            .panel
            .scroll_focused_menu_into_view(MENU_ROW_EXTENT, DEFAULT_MENU_VIEWPORT_HEIGHT)
        else {
            return Task::none();
        };
        iced::widget::operation::scroll_to(
            scroll_id,
            iced::widget::operation::AbsoluteOffset {
                x: None,
                y: Some(y),
            },
        )
    }

    fn activate_panel_entry(&mut self, index: Option<usize>) -> Task<Message> {
        let effects = self
            .panel
            .handle(super::panel_runtime::PanelRuntimeIntent::Activate(index));
        self.apply_panel_runtime_effects(effects)
    }

    #[cfg(test)]
    fn active_menu_entries(&self) -> Vec<super::panel_menu_session::PanelMenuEntry> {
        self.active_panel_menu_entries()
    }

    #[cfg(test)]
    pub(super) fn active_panel_menu_entries(
        &self,
    ) -> Vec<super::panel_menu_session::PanelMenuEntry> {
        self.panel.active_panel_menu_entries_for_test()
    }

    fn focus_panel_search_input(&mut self, retries: u8) -> Task<Message> {
        if !self.panel.search_focus_pending() {
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
            self.panel
                .handle(super::panel_runtime::PanelRuntimeIntent::SearchFocusSettled);
            return Task::none();
        };
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
            self.panel
                .handle(super::panel_runtime::PanelRuntimeIntent::SearchFocusSettled);
        }
        Task::batch(tasks)
    }

    fn dispatch_panel_action_from_active_panel(
        &mut self,
        action: crate::config::PanelAction,
    ) -> Task<Message> {
        let Some(tree) = self
            .surfaces
            .panel_popup_values()
            .next()
            .map(|meta| meta.tree.clone())
        else {
            return Task::none();
        };
        self.dispatch_panel_action(tree.origin_window, tree.origin, action)
    }

    fn launch_installed_app_by_id(&mut self, id: &str) {
        #[cfg(test)]
        {
            self.panel
                .launched_installed_app_ids_for_test
                .push(id.to_string());
        }

        #[cfg(not(test))]
        {
            let Some(app) = self
                .load_installed_apps_for_panel()
                .into_iter()
                .find(|app| app.id == id)
            else {
                return;
            };
            let _ = crate::command::spawn_desktop_application_fire_and_forget(&app);
        }
    }
}

impl App {
    pub(super) fn handle_dmenu_selection_request(
        &mut self,
        request: crate::ipc::protocol::DmenuRequest,
        responder: super::messages::IpcResponder,
    ) -> Task<Message> {
        let effects = self.panel.handle(
            super::panel_runtime::PanelRuntimeIntent::BeginDmenuSelection {
                request,
                responder,
                origin_window: self.default_panel_origin_window(),
            },
        );
        self.apply_panel_runtime_effects(effects)
    }

    pub(super) fn complete_dmenu_request(&mut self, response: crate::ipc::protocol::IpcResponse) {
        let effects = self
            .panel
            .handle(super::panel_runtime::PanelRuntimeIntent::CompleteDmenu(
                response,
            ));
        let _ = self.apply_panel_runtime_effects(effects);
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

#[cfg(test)]
fn dmenu_entry_views(
    request: &crate::ipc::protocol::DmenuRequest,
    query: &str,
) -> Vec<MenuEntryView> {
    let exact_match = request
        .entries
        .iter()
        .any(|entry| entry.display_label() == query || entry.selected_value() == query);

    let mut matches = Vec::new();
    for (index, entry) in request.entries.iter().enumerate() {
        let view = MenuEntryView {
            label: entry.display_label().to_string(),
            icon: entry.icon.clone(),
            path: vec![index],
            target: MenuEntryTarget::DmenuValue(entry.selected_value().to_string()),
            is_submenu: false,
        };

        if panel_search::is_blank_query(query) {
            matches.push(SearchMatch {
                item: view,
                score: panel_search::best_match(
                    &[SearchText {
                        field: SearchField::Label,
                        text: entry.display_label(),
                    }],
                    "",
                )
                .expect("blank query matches"),
                source_index: index,
            });
            continue;
        }

        let keywords = entry.keywords.join(" ");
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: entry.display_label(),
            },
            SearchText {
                field: SearchField::Secondary,
                text: keywords.as_str(),
            },
        ];

        if let Some(score) = panel_search::best_match(&fields, query) {
            matches.push(SearchMatch {
                item: view,
                score,
                source_index: index,
            });
        }
    }

    let mut entries = if panel_search::is_blank_query(query) {
        matches.into_iter().map(|matched| matched.item).collect()
    } else {
        panel_search::rank_matches(matches)
    };

    if request.allow_custom && !panel_search::is_blank_query(query) && !exact_match {
        entries.push(MenuEntryView {
            label: query.to_string(),
            icon: None,
            path: Vec::new(),
            target: MenuEntryTarget::DmenuCustomValue(query.to_string()),
            is_submenu: false,
        });
    }

    entries
}

#[cfg(test)]
fn current_menu_entries(menu: &crate::config::MenuConfig, path: &[usize]) -> Vec<MenuEntryView> {
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

#[cfg(test)]
fn search_menu_entries(menu: &crate::config::MenuConfig, query: &str) -> Vec<MenuEntryView> {
    if panel_search::is_blank_query(query) {
        return current_menu_entries(menu, &[]);
    }

    let mut results = Vec::new();
    collect_search_menu_entries(
        &menu.items,
        query,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut 0,
        &mut results,
    );
    panel_search::rank_matches(results)
}

#[cfg(test)]
fn collect_search_menu_entries(
    items: &[crate::config::MenuItemConfig],
    query: &str,
    labels: &mut Vec<String>,
    path: &mut Vec<usize>,
    source_index: &mut usize,
    results: &mut Vec<SearchMatch<MenuEntryView>>,
) {
    for (index, item) in items.iter().enumerate() {
        labels.push(item.label.clone());
        path.push(index);
        let current_source_index = *source_index;
        *source_index += 1;

        let breadcrumb = labels
            .iter()
            .take(labels.len().saturating_sub(1))
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" / ");
        let keyword_text = item.keywords.join(" ");
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: item.label.as_str(),
            },
            SearchText {
                field: SearchField::Secondary,
                text: keyword_text.as_str(),
            },
            SearchText {
                field: SearchField::Breadcrumb,
                text: breadcrumb.as_str(),
            },
        ];

        if let Some(mut score) = panel_search::best_match(&fields, query) {
            if !item.children.is_empty() {
                score.score -= 1;
            }
            results.push(SearchMatch {
                item: MenuEntryView {
                    label: labels.join(" / "),
                    icon: item.icon.clone(),
                    path: path.clone(),
                    target: MenuEntryTarget::StaticPath(path.clone()),
                    is_submenu: !item.children.is_empty(),
                },
                score,
                source_index: current_source_index,
            });
        }

        collect_search_menu_entries(&item.children, query, labels, path, source_index, results);
        path.pop();
        labels.pop();
    }
}

#[cfg(test)]
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
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
                crate::ipc::protocol::DmenuEntry::plain("kitty"),
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
            ],
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
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("Firefox"),
                crate::ipc::protocol::DmenuEntry::plain("Kitty"),
            ],
            allow_custom: true,
        };
        let entries = dmenu_entry_views(&request, "fire");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Firefox");
        assert_eq!(entries[1].label, "fire");
    }

    #[test]
    fn dmenu_custom_value_entry_appears_when_query_has_no_exact_match() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
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
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
            allow_custom: false,
        };
        let entries = dmenu_entry_views(&request, "kitty");

        assert!(entries.is_empty());
    }

    #[test]
    fn dmenu_rich_entries_use_label_icon_keywords_and_value() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: Some("Run".into()),
            entries: vec![
                crate::ipc::protocol::DmenuEntry {
                    label: Some("Firefox".into()),
                    value: Some("org.mozilla.firefox".into()),
                    icon: Some("firefox".into()),
                    keywords: vec!["browser".into(), "web".into()],
                },
                crate::ipc::protocol::DmenuEntry {
                    label: Some("Kitty".into()),
                    value: Some("kitty".into()),
                    icon: None,
                    keywords: Vec::new(),
                },
            ],
            allow_custom: true,
        };

        let entries = dmenu_entry_views(&request, "browser");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Firefox");
        assert_eq!(entries[0].icon.as_deref(), Some("firefox"));
        assert_eq!(
            entries[0].target,
            MenuEntryTarget::DmenuValue("org.mozilla.firefox".into())
        );
        assert_eq!(entries[1].label, "browser");
        assert!(matches!(
            entries[1].target,
            MenuEntryTarget::DmenuCustomValue(_)
        ));
    }

    #[test]
    fn dmenu_search_uses_fuzzy_ranking() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("Telemetry Renderer Module"),
                crate::ipc::protocol::DmenuEntry::plain("Terminal"),
            ],
            allow_custom: false,
        };

        let entries = dmenu_entry_views(&request, "term");

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Terminal", "Telemetry Renderer Module"]
        );
    }

    #[test]
    fn dmenu_custom_value_appears_after_fuzzy_matches() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("Firefox")],
            allow_custom: true,
        };

        let entries = dmenu_entry_views(&request, "firfox");

        assert_eq!(entries[0].label, "Firefox");
        assert_eq!(entries[1].label, "firfox");
        assert!(matches!(
            entries[1].target,
            MenuEntryTarget::DmenuCustomValue(_)
        ));
    }

    #[test]
    fn dmenu_blank_query_preserves_duplicate_source_order() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
                crate::ipc::protocol::DmenuEntry::plain("kitty"),
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
            ],
            allow_custom: true,
        };

        let entries = dmenu_entry_views(&request, " ");

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["firefox", "kitty", "firefox"]
        );
    }

    #[test]
    fn dmenu_rich_search_does_not_match_opaque_value_when_label_exists() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry {
                label: Some("Firefox".into()),
                value: Some("org.mozilla.firefox".into()),
                icon: None,
                keywords: Vec::new(),
            }],
            allow_custom: false,
        };

        let entries = dmenu_entry_views(&request, "mozilla");

        assert!(entries.is_empty());
    }

    #[test]
    fn dmenu_value_only_rich_entry_matches_and_displays_value() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry {
                label: None,
                value: Some("org.mozilla.firefox".into()),
                icon: None,
                keywords: Vec::new(),
            }],
            allow_custom: false,
        };

        let entries = dmenu_entry_views(&request, "mozilla");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "org.mozilla.firefox");
    }

    #[test]
    fn dmenu_request_rejects_when_another_request_is_pending() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let (active_tx, _active_rx) = tokio::sync::oneshot::channel();
        app.panel
            .begin_dmenu_request(crate::app::messages::ipc_responder(active_tx))
            .expect("stored");
        let (tx, mut rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("x")],
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
    fn dmenu_request_keeps_responder_for_open_selection() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let (tx, _rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(tx),
        });

        assert!(app.panel.has_dmenu_responder());
        assert_eq!(app.active_menu_entries()[0].label, "firefox");
    }

    #[test]
    fn completing_dmenu_selection_sends_selected_response() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        app.panel
            .begin_dmenu_request(crate::app::messages::ipc_responder(tx))
            .expect("stored");

        app.complete_dmenu_request(crate::ipc::protocol::IpcResponse::Selected {
            value: "firefox".into(),
        });

        assert_eq!(
            rx.try_recv().unwrap(),
            crate::ipc::protocol::IpcResponse::Selected {
                value: "firefox".into()
            }
        );
        assert!(!app.panel.has_dmenu_responder());
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
    fn panel_window_uses_on_demand_keyboard_interactivity() {
        let bar = crate::config::BarConfig {
            edge: crate::config::Edge::Top,
            ..crate::config::BarConfig::default()
        };
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            origin_window: iced::window::Id::unique(),
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: Some(iced::Rectangle {
                x: 100.0,
                y: 0.0,
                width: 48.0,
                height: 32.0,
            }),
            screen_size: (1920, 48),
        };

        assert_eq!(
            panel_window_settings(&bar, &tree, (0, 0)).keyboard_interactivity,
            KeyboardInteractivity::OnDemand
        );
    }

    #[test]
    fn panel_window_settings_keep_layer_shell_focusable_for_search_input() {
        let bar = crate::config::BarConfig {
            edge: crate::config::Edge::Top,
            ..crate::config::BarConfig::default()
        };
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            origin_window: iced::window::Id::unique(),
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (1920, 48),
        };

        let settings = panel_window_settings(&bar, &tree, (0, 0));

        assert_eq!(settings.size, Some((bar.panel.width, 600)));
        assert_eq!(
            settings.keyboard_interactivity,
            KeyboardInteractivity::OnDemand
        );
    }

    #[test]
    fn bottom_panel_window_uses_layer_margin_above_bar() {
        let bar = crate::config::BarConfig {
            edge: crate::config::Edge::Bottom,
            ..crate::config::BarConfig::default()
        };
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            origin_window: iced::window::Id::unique(),
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: Some(iced::Rectangle {
                x: 100.0,
                y: 0.0,
                width: 48.0,
                height: 48.0,
            }),
            screen_size: (1920, 48),
        };

        let settings = panel_window_settings(&bar, &tree, (0, 0));

        assert_eq!(settings.size, Some((bar.panel.width, 600)));
        assert_eq!(settings.margin, Some((0, 0, 48, 0)));
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
        if app.panel.panel_menu_snapshot_for_test().is_none() {
            app.panel.handle(
                super::super::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
                    window: panel,
                    origin_window: origin,
                    content: crate::surface::panel::PanelContent::Menu("main".into()),
                    catalog: super::super::panel_runtime::PanelMenuCatalog::new(
                        app.config.menus.as_slice(),
                    ),
                },
            );
        }
    }

    fn set_panel_search(app: &mut App, query: &str) {
        app.panel
            .handle(super::super::panel_runtime::PanelRuntimeIntent::SearchChanged(query.into()));
    }

    fn set_panel_focus(app: &mut App, index: usize) {
        if app.panel.panel_menu_snapshot_for_test().is_none() {
            app.panel.handle(
                super::super::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
                    window: iced::window::Id::unique(),
                    origin_window: iced::window::Id::unique(),
                    content: crate::surface::panel::PanelContent::Menu("main".into()),
                    catalog: super::super::panel_runtime::PanelMenuCatalog::new(
                        app.config.menus.as_slice(),
                    ),
                },
            );
        }
        app.panel
            .handle(super::super::panel_runtime::PanelRuntimeIntent::FocusIndex(
                index,
            ));
    }

    fn panel_focus(app: &App) -> Option<usize> {
        app.panel.focused_index_for_test()
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

    #[test]
    fn menu_search_ranks_label_matches_before_keyword_matches() {
        let mut menu = menu_with_apps_browser();
        menu.items.push(crate::config::MenuItemConfig {
            label: "Settings".into(),
            icon: None,
            keywords: vec!["browser".into()],
            action: Some(crate::config::PanelAction::Command(
                crate::config::CommandConfig::Shell("settings".into()),
            )),
            children: vec![],
        });

        let results = search_menu_entries(&menu, "browser");

        assert_eq!(results[0].label, "Apps / Browser");
        assert_eq!(results[1].label, "Settings");
    }

    #[test]
    fn menu_search_uses_ordered_fuzzy_matching() {
        let menu = menu_with_apps_browser();

        let results = search_menu_entries(&menu, "brsr");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "Apps / Browser");
        assert!(search_menu_entries(&menu, "rb").is_empty());
    }

    #[test]
    fn menu_search_matches_breadcrumb_as_lowest_priority() {
        let menu = crate::config::MenuConfig {
            name: "main".into(),
            show_icons: false,
            prompt: None,
            source: None,
            items: vec![
                crate::config::MenuItemConfig {
                    label: "Apps".into(),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![crate::config::MenuItemConfig {
                        label: "Firefox".into(),
                        icon: None,
                        keywords: vec![],
                        action: Some(crate::config::PanelAction::Command(
                            crate::config::CommandConfig::Shell("firefox".into()),
                        )),
                        children: vec![],
                    }],
                },
                crate::config::MenuItemConfig {
                    label: "App Settings".into(),
                    icon: None,
                    keywords: vec![],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::CommandConfig::Shell("settings".into()),
                    )),
                    children: vec![],
                },
            ],
        };

        let results = search_menu_entries(&menu, "app");

        assert_eq!(
            results
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["App Settings", "Apps", "Apps / Firefox"]
        );
    }

    #[test]
    fn menu_search_requires_every_query_token() {
        let menu = menu_with_apps_browser();

        assert_eq!(search_menu_entries(&menu, "apps browser").len(), 1);
        assert!(search_menu_entries(&menu, "apps terminal").is_empty());
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
    fn toggle_installed_apps_menu_loads_entries() {
        let mut app = app_with_installed_apps_menu(vec![installed_app("Alpha", "alpha.desktop")]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Toggle {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("apps".into()),
            anchor: None,
        }));

        assert_eq!(
            app.active_menu_entries()
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha"]
        );
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

        set_panel_search(&mut app, "internet");

        let entries = app.active_menu_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Browser");
    }

    #[test]
    fn installed_apps_search_ranks_visible_label_before_metadata() {
        let mut settings = installed_app("Settings", "settings.desktop");
        settings.keywords = vec!["network".into()];
        let mut network = installed_app("Network Connections", "network.desktop");
        network.comment = Some("Configure links".into());
        let mut app = app_with_installed_apps_menu(vec![settings, network]);
        open_apps_menu_panel(&mut app);

        set_panel_search(&mut app, "net");

        let entries = app.active_menu_entries();
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Network Connections", "Settings"]
        );
    }

    #[test]
    fn installed_apps_search_matches_tokens_across_fields() {
        let mut code = installed_app("Visual Studio Code", "code.desktop");
        code.keywords = vec!["editor".into()];
        let mut app = app_with_installed_apps_menu(vec![code]);
        open_apps_menu_panel(&mut app);

        set_panel_search(&mut app, "code editor");

        let entries = app.active_menu_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Visual Studio Code");
    }

    #[test]
    fn installed_apps_search_preserves_source_order_for_blank_query() {
        let mut app = app_with_installed_apps_menu(vec![
            installed_app("Zed", "zed.desktop"),
            installed_app("Alpha", "alpha.desktop"),
        ]);
        open_apps_menu_panel(&mut app);

        set_panel_search(&mut app, "   ");

        let entries = app.active_menu_entries();
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Zed", "Alpha"]
        );
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
        set_panel_focus(&mut app, 0);
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(panel_focus(&app), Some(1));
    }

    #[test]
    fn arrow_down_on_panel_window_does_not_scroll_when_focused_entry_is_visible() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        set_panel_focus(&mut app, 0);
        app.panel.reset_menu_scroll();
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(panel_focus(&app), Some(1));
        assert_eq!(app.panel.menu_scroll_y(), 0.0);
    }

    #[test]
    fn arrow_down_on_panel_window_scrolls_only_enough_to_reveal_focused_entry() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        set_panel_focus(&mut app, 40);
        app.panel.reset_menu_scroll();
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(panel_focus(&app), Some(41));
        assert_eq!(
            app.panel.menu_scroll_y(),
            42.0 * MENU_ROW_EXTENT - DEFAULT_MENU_VIEWPORT_HEIGHT
        );
    }

    #[test]
    fn arrow_up_on_panel_window_scrolls_only_when_focused_entry_is_above_view() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        set_panel_focus(&mut app, 10);
        app.panel.handle(
            super::super::panel_runtime::PanelRuntimeIntent::MenuScrolled {
                y: 10.0 * MENU_ROW_EXTENT,
                viewport_height: DEFAULT_MENU_VIEWPORT_HEIGHT,
            },
        );
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMovePrev(panel));

        assert_eq!(panel_focus(&app), Some(9));
        assert_eq!(app.panel.menu_scroll_y(), 9.0 * MENU_ROW_EXTENT);
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
        assert_eq!(panel_focus(&app), Some(0));
    }

    #[test]
    fn focus_search_input_event_initializes_menu_focus_without_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);
        app.panel
            .handle(super::super::panel_runtime::PanelRuntimeIntent::RequestSearchFocus);

        let _ = app.handle_panel_event(super::super::messages::PanelEvent::FocusSearchInput {
            retries: 0,
        });

        assert!(app.surface(panel).is_none());
        assert_eq!(panel_focus(&app), Some(0));
        assert!(!app.panel.search_focus_pending());
    }

    #[test]
    fn hover_focus_cancels_pending_search_focus_retries() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);
        app.panel
            .handle(super::super::panel_runtime::PanelRuntimeIntent::RequestSearchFocus);

        let _ = app.handle_panel_event(super::super::messages::PanelEvent::FocusIndex(1));

        assert_eq!(panel_focus(&app), Some(1));
        assert!(!app.panel.search_focus_pending());
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
        assert_eq!(panel_focus(&app), Some(0));
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
        let mut panel = super::super::panel_runtime::PanelRuntime::default();
        let menus = [menu_with_two_entries()];
        panel.handle(
            super::super::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
                window: iced::window::Id::unique(),
                origin_window: iced::window::Id::unique(),
                content: crate::surface::panel::PanelContent::Menu("main".into()),
                catalog: super::super::panel_runtime::PanelMenuCatalog::new(&menus),
            },
        );
        panel.handle(super::super::panel_runtime::PanelRuntimeIntent::SearchChanged("term".into()));

        assert!(matches!(
            panel
                .handle(super::super::panel_runtime::PanelRuntimeIntent::Escape)
                .as_slice(),
            [super::super::panel_runtime::PanelRuntimeEffect::RequestSearchFocus]
        ));
        assert_eq!(
            panel.panel_menu_snapshot_for_test().unwrap().search_query,
            ""
        );
    }

    #[test]
    fn escape_that_clears_search_requests_search_refocus() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.surface_mut(origin);
        register_menu_panel(&mut app, panel, origin);
        set_panel_search(&mut app, "term");
        app.panel
            .handle(super::super::panel_runtime::PanelRuntimeIntent::SearchFocusSettled);

        let _ = app.handle_panel_escape_pressed();

        assert_eq!(
            app.panel
                .panel_menu_snapshot_for_test()
                .unwrap()
                .search_query,
            ""
        );
        assert!(app.panel.search_focus_pending());
        assert!(app.surfaces.has_panel_popup(panel));
    }

    #[test]
    fn panel_search_input_id_is_stable_for_focus_task() {
        let first = iced::window::Id::unique();
        let second = iced::window::Id::unique();

        assert_eq!(panel_search_input_id(first), panel_search_input_id(first));
        assert_ne!(panel_search_input_id(first), panel_search_input_id(second));
    }
}
