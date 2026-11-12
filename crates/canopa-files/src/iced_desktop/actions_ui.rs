//! Every visible command control carries the selection and catalog it described.
//! Accepted requests keep their captured paths even when browsing moves on.
use super::*;
use crate::browse::Entry;
use canopa_theme::UiKind;
use iced::{
    Length,
    widget::{column, container, mouse_area, opaque, scrollable, space, stack},
};

#[derive(Clone, Debug)]
pub(super) struct Source {
    pub window: window::Id,
    tab: u64,
    navigation: u64,
    epoch: u64,
    catalog: u64,
    paths: Arc<Vec<PathBuf>>,
    entries: Arc<Vec<Entry>>,
    focused: Option<PathBuf>,
    directory: PathBuf,
    resolved: PathBuf,
}

#[derive(Clone, Debug)]
pub(super) struct RowTarget {
    pub window: window::Id,
    pub tab: u64,
    pub navigation: u64,
    pub epoch: u64,
    pub column: PathBuf,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Menu(window::Id),
    Locations(window::Id),
    /// The status bar's view menu.
    ViewMenu(window::Id),
    Row(RowTarget),
    Close(window::Id),
    Builtin(Arc<Source>, Command),
    Invoke(Arc<Source>, String),
    MenuItem(Arc<Source>, usize),
    /// Applications for the selection, resolved on a worker.
    OpenWithReady(Arc<Source>, Arc<Vec<Arc<canopa_apps::DesktopApplication>>>),
}

#[derive(Clone)]
enum Item {
    Command(Command),
    Action(String),
    FolderOptions,
    Locations,
    Back,
    Place(PathBuf),
    Device(crate::local_volumes::DeviceToken, bool),
    OpenWith,
    Application(Arc<canopa_apps::DesktopApplication>),
    OtherApplication,
    View(crate::preferences::ViewMode),
}
pub(super) struct Menu {
    source: Arc<Source>,
    items: Vec<(String, Item)>,
    focused: usize,
}
impl Menu {
    pub fn window(&self) -> window::Id {
        self.source.window
    }
    /// Whether this is the status bar's view menu, which opens above it.
    pub fn is_view_menu(&self) -> bool {
        self.items
            .first()
            .is_some_and(|(_, item)| matches!(item, Item::View(_)))
    }
}

impl Desktop {
    pub(super) fn action_source(&self, id: window::Id) -> Option<Arc<Source>> {
        let tab = self.active(id).filter(|tab| !tab.loading)?;
        let browser = tab.browser.as_ref()?;
        Some(Arc::new(Source {
            window: id,
            tab: tab.id,
            navigation: tab.revision,
            epoch: tab.list_epoch,
            catalog: self.configuration_revision,
            paths: Arc::new(browser.selected().iter().cloned().collect()),
            entries: Arc::new(
                browser
                    .visible_entries()
                    .into_iter()
                    .filter(|entry| browser.selected().contains(&entry.path))
                    .cloned()
                    .collect(),
            ),
            focused: browser.focused().map(Path::to_owned),
            directory: browser.path().to_owned(),
            resolved: browser.directory().resolved_path.clone(),
        }))
    }

    fn matches_source(&self, source: &Source) -> bool {
        source.catalog == self.configuration_revision
            && self.active(source.window).is_some_and(|tab| {
                tab.id == source.tab
                    && tab.revision == source.navigation
                    && tab.list_epoch == source.epoch
                    && !tab.loading
                    && tab.browser.as_ref().is_some_and(|browser| {
                        browser.path() == source.directory
                            && browser.focused() == source.focused.as_deref()
                            && browser.selected().iter().eq(source.paths.iter())
                    })
            })
    }

    pub(super) fn action_applicable(
        &self,
        source: &Source,
        action: &crate::actions::CustomAction,
    ) -> bool {
        source.entries.len() == source.paths.len() && action.applies_to(source.entries.iter())
    }

    pub(super) fn sync_action_menu(&mut self) {
        if self
            .action_menu
            .as_ref()
            .is_some_and(|menu| !self.matches_source(&menu.source))
        {
            self.action_menu = None;
        }
    }

    pub(super) fn action_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Builtin(source, command) => {
                if self.matches_source(&source) {
                    return self.command(source.window, command);
                }
            }
            Event::Close(id) => {
                if self
                    .action_menu
                    .as_ref()
                    .is_some_and(|menu| menu.source.window == id)
                {
                    self.action_menu = None;
                }
            }
            Event::Menu(id) => {
                let Some(source) = self.action_source(id) else {
                    return Task::none();
                };
                let mut items = Vec::new();
                if !source.paths.is_empty() {
                    items.push((fl!("files-menu-open"), Item::Command(Command::Open)));
                    if source.entries.len() == source.paths.len()
                        && source.entries.iter().all(|entry| !entry.is_directory())
                    {
                        items.push((fl!("files-menu-open-with"), Item::OpenWith));
                    }
                    items.push((
                        fl!("files-command-quick-look"),
                        Item::Command(Command::QuickLook),
                    ));
                }
                if self.file_clipboard.ready()
                    && self.file_operations.storage.is_some()
                    && self.file_operations.storage_error.is_none()
                    && !source.paths.is_empty()
                {
                    items.push((fl!("files-command-copy"), Item::Command(Command::Copy)));
                    items.push((fl!("files-command-cut"), Item::Command(Command::Cut)));
                }
                for action in &self.configuration.actions {
                    if self.action_applicable(&source, action) {
                        items.push((action.name.clone(), Item::Action(action.id.clone())));
                    }
                }
                if self.file_operations.ready() {
                    if self.file_clipboard.can_paste() {
                        items.push((fl!("files-command-paste"), Item::Command(Command::Paste)));
                    }
                    items.push((
                        fl!("files-menu-new-folder"),
                        Item::Command(Command::NewFolder),
                    ));
                    if source.paths.len() == 1 {
                        items.push((fl!("files-menu-rename"), Item::Command(Command::Rename)));
                    }
                    if !source.paths.is_empty() {
                        items.push((fl!("files-command-trash"), Item::Command(Command::Trash)));
                        items.push((
                            fl!("files-menu-delete-permanently"),
                            Item::Command(Command::DeletePermanently),
                        ));
                    }
                    if !self.file_operations.has_active()
                        && let Some(undo) = &self.file_operations.snapshot.undo
                    {
                        items.push((
                            crate::history_labels::display(&undo.label),
                            Item::Command(Command::Undo),
                        ));
                    }
                }
                items.push((
                    fl!("files-command-properties"),
                    Item::Command(Command::Properties),
                ));
                items.push((
                    fl!("files-menu-edit-custom-actions"),
                    Item::Command(Command::CustomActions),
                ));
                items.push((
                    fl!("files-command-operations"),
                    Item::Command(Command::Operations),
                ));
                items.push((
                    fl!("files-command-show-trash"),
                    Item::Command(Command::ShowTrash),
                ));
                items.push((
                    fl!("files-menu-preferences"),
                    Item::Command(Command::Preferences),
                ));
                items.push((
                    fl!("files-menu-bookmark-folder"),
                    Item::Command(Command::BookmarkFolder),
                ));
                items.push((fl!("files-menu-locations"), Item::Locations));
                items.push((fl!("files-menu-view-options"), Item::FolderOptions));
                return self.present_menu(source, items);
            }
            Event::Locations(id) => {
                let Some(source) = self.action_source(id) else {
                    return Task::none();
                };
                return self.present_menu(source, self.location_items(false));
            }
            Event::ViewMenu(id) => {
                // The button toggles its menu.
                if self
                    .action_menu
                    .as_ref()
                    .is_some_and(|menu| menu.window() == id && menu.is_view_menu())
                {
                    self.action_menu = None;
                    return Task::none();
                }
                let Some(source) = self.action_source(id) else {
                    return Task::none();
                };
                let current = self
                    .active(id)
                    .and_then(|tab| tab.browser.as_ref())
                    .map(|browser| browser.view().mode)
                    .unwrap_or_default();
                use crate::preferences::ViewMode;
                let items: Vec<_> = [ViewMode::Columns, ViewMode::List, ViewMode::Icons]
                    .into_iter()
                    .map(|mode| (super::view::view_label(mode), Item::View(mode)))
                    .collect();
                let focused = items
                    .iter()
                    .position(|(_, item)| matches!(item, Item::View(mode) if *mode == current))
                    .unwrap_or(0);
                let task = self.present_menu(source, items);
                if let Some(menu) = &mut self.action_menu {
                    menu.focused = focused;
                }
                return task;
            }
            Event::Row(target) => {
                let Some(tab) = self.active(target.window).filter(|tab| {
                    tab.id == target.tab
                        && tab.revision == target.navigation
                        && tab.list_epoch == target.epoch
                        && !tab.loading
                }) else {
                    return Task::none();
                };
                let Some(browser) = &tab.browser else {
                    return Task::none();
                };
                let visible = if browser.path() == target.column {
                    browser
                        .visible_entries()
                        .into_iter()
                        .any(|entry| entry.path == target.path)
                } else {
                    let window = &self.windows[&target.window];
                    let columns = tab.column_paths(
                        window.width,
                        self.configuration.settings.sidebar,
                        window.panel_width(),
                        &self.home,
                    );
                    let child = columns
                        .iter()
                        .position(|path| path == &target.column)
                        .and_then(|index| columns.get(index + 1))
                        .map(PathBuf::as_path);
                    tab.columns
                        .get(&target.column)
                        .and_then(|column| column.result.as_ref())
                        .and_then(|result| result.as_ref().ok())
                        .is_some_and(|directory| {
                            directory
                                .visible_entries(browser.show_hidden(), child)
                                .iter()
                                .any(|entry| entry.path == target.path)
                        })
                };
                if !visible {
                    return Task::none();
                }
                if target.column != browser.path() {
                    let task = self.navigate(
                        target.window,
                        target.column,
                        Navigation::Visit,
                        Some(target.path),
                    );
                    if let Some(tab) = self.active_mut(target.window) {
                        tab.menu_after_load = Some(tab.revision);
                    }
                    return task;
                }
                let tab = self.active_mut(target.window).unwrap();
                tab.location = None;
                let browser = tab.browser.as_mut().unwrap();
                if !browser.selected().contains(&target.path) {
                    browser.select(&target.path, SelectionMode::Replace);
                } else {
                    browser.focus_entry(&target.path, false);
                }
                return self.action_event(Event::Menu(target.window));
            }
            Event::Invoke(source, action_id) => {
                if !self.matches_source(&source) {
                    return Task::none();
                }
                let Some(action) = self
                    .configuration
                    .actions
                    .iter()
                    .find(|action| action.id == action_id)
                    .cloned()
                else {
                    return Task::none();
                };
                if !self.action_applicable(&source, &action) {
                    if let Some(tab) = self.active_mut(source.window) {
                        tab.error = Some(fl!(
                            "files-action-not-applicable",
                            action = action.name.clone()
                        ));
                    }
                    return Task::none();
                }
                self.action_menu = None;
                return self.enqueue_command(jobs_ui::CommandRequest {
                    action,
                    paths: source.paths.clone(),
                    directory: source.directory.clone(),
                    resolved_directory: source.resolved.clone(),
                });
            }
            Event::OpenWithReady(source, apps) => {
                if !self.matches_source(&source) {
                    return Task::none();
                }
                let mut items = vec![(fl!("files-command-back"), Item::Back)];
                for (index, app) in apps.iter().enumerate() {
                    let label = if index == 0 {
                        fl!(
                            "files-menu-default-application",
                            application = app.name.clone()
                        )
                    } else {
                        app.name.clone()
                    };
                    items.push((label, Item::Application(app.clone())));
                }
                items.push((fl!("files-menu-other-application"), Item::OtherApplication));
                return self.present_menu(source, items);
            }
            Event::MenuItem(source, index) => {
                if !self.matches_source(&source) {
                    self.action_menu = None;
                    return Task::none();
                }
                let Some(menu) = self
                    .action_menu
                    .as_ref()
                    .filter(|menu| Arc::ptr_eq(&menu.source, &source))
                else {
                    return Task::none();
                };
                let Some((_, item)) = menu.items.get(index).cloned() else {
                    return Task::none();
                };
                self.action_menu = None;
                return match item {
                    Item::Locations => self.present_menu(source, self.location_items(true)),
                    Item::Back => self.action_event(Event::Menu(source.window)),
                    Item::Place(path) => {
                        self.navigate(source.window, path, Navigation::Visit, None)
                    }
                    Item::Device(token, remove) => {
                        if remove {
                            self.device_event(super::devices_ui::Event::Remove(
                                source.window,
                                token,
                            ))
                        } else {
                            super::devices_ui::Origin::capture(self, source.window).map_or_else(
                                Task::none,
                                |origin| {
                                    self.device_event(super::devices_ui::Event::Open(origin, token))
                                },
                            )
                        }
                    }
                    Item::Command(command) => self.command(source.window, command),
                    Item::View(mode) => self.command(
                        source.window,
                        match mode {
                            crate::preferences::ViewMode::Columns => Command::Columns,
                            crate::preferences::ViewMode::List => Command::List,
                            crate::preferences::ViewMode::Icons => Command::Icons,
                        },
                    ),
                    Item::FolderOptions => {
                        self.folder_view_event(folder_views::Event::Show(source.window))
                    }
                    Item::Action(id) => self.action_event(Event::Invoke(source, id)),
                    Item::OpenWith => {
                        let paths = source.paths.to_vec();
                        Task::perform(
                            async move {
                                let (send, receive) = iced::futures::channel::oneshot::channel();
                                let _ = std::thread::Builder::new()
                                    .name("files-open-with-menu".into())
                                    .spawn(move || {
                                        let _ = send
                                            .send(super::open_with_ui::applications_for(&paths));
                                    });
                                Arc::new(receive.await.unwrap_or_default())
                            },
                            move |apps| Message::Action(Event::OpenWithReady(source.clone(), apps)),
                        )
                    }
                    Item::Application(app) => self.launch(
                        source.window,
                        super::open_with_ui::Launch::Application {
                            app,
                            files: source.paths.to_vec(),
                            default_for: None,
                        },
                    ),
                    Item::OtherApplication => {
                        self.show_open_with_chooser(source.window, source.paths.to_vec())
                    }
                };
            }
        }
        Task::none()
    }

    fn present_menu(&mut self, source: Arc<Source>, items: Vec<(String, Item)>) -> Task<Message> {
        let id = source.window;
        self.action_menu = Some(Menu {
            source,
            items,
            focused: 0,
        });
        let Some(tab) = self.active_mut(id) else {
            return Task::none();
        };
        tab.location = None;
        use iced::advanced::widget::{operate, operation};
        operate(operation::scope(
            view::root_id(id, tab.id),
            operation::focusable::unfocus(),
        ))
    }

    fn location_items(&self, back: bool) -> Vec<(String, Item)> {
        use crate::interface::{Place, PlaceKind};

        let mut items = Vec::new();
        if back {
            items.push((fl!("files-command-back"), Item::Back));
        }
        for place in &self.configuration.settings.places {
            let item = match place {
                Place::Builtin(PlaceKind::Trash) => Some(Item::Command(Command::ShowTrash)),
                Place::Builtin(PlaceKind::Recovery) => Some(Item::Command(Command::Recovery)),
                _ => self.place_path(place).map(Item::Place),
            };
            if let Some(item) = item {
                items.push((place.label(), item));
            }
        }
        if self.devices.catalog.status == crate::local_volumes::DeviceStatus::Ready {
            for device in &self.devices.catalog.devices {
                if device.root.is_some() || device.can_mount {
                    let device_name = device.name.clone();
                    let label = if device.root.is_some() {
                        fl!("files-device-open-named", device = device_name)
                    } else {
                        fl!("files-device-mount-named", device = device_name)
                    };
                    items.push((label, Item::Device(device.token, false)));
                }
                if let Some(kind) = device.removal {
                    items.push((
                        kind.action_label(&device.name),
                        Item::Device(device.token, true),
                    ));
                }
            }
        }
        items
    }

    pub(super) fn action_shortcut(
        &mut self,
        id: window::Id,
        chord: &KeyChord,
    ) -> Option<Task<Message>> {
        let action = self
            .configuration
            .actions
            .iter()
            .find(|action| {
                action
                    .shortcut
                    .as_ref()
                    .and_then(|binding| KeyChord::parse(binding).ok())
                    .as_ref()
                    == Some(chord)
            })?
            .id
            .clone();
        Some(self.action_source(id).map_or_else(Task::none, |source| {
            self.action_event(Event::Invoke(source, action))
        }))
    }

    pub(super) fn menu_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        let Some(menu) = self
            .action_menu
            .as_mut()
            .filter(|menu| menu.source.window == id)
        else {
            return Task::none();
        };
        match key.as_ref() {
            keyboard::Key::Named(
                keyboard::key::Named::Escape | keyboard::key::Named::ContextMenu,
            ) => self.action_menu = None,
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                menu.focused = (menu.focused + 1) % menu.items.len()
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                menu.focused = (menu.focused + menu.items.len() - 1) % menu.items.len()
            }
            keyboard::Key::Named(keyboard::key::Named::Home) => menu.focused = 0,
            keyboard::Key::Named(keyboard::key::Named::End) => menu.focused = menu.items.len() - 1,
            keyboard::Key::Named(keyboard::key::Named::Tab) => {
                menu.focused = (menu.focused
                    + if modifiers.shift() {
                        menu.items.len() - 1
                    } else {
                        1
                    })
                    % menu.items.len()
            }
            keyboard::Key::Named(keyboard::key::Named::Enter | keyboard::key::Named::Space)
            | keyboard::Key::Character(" ") => {
                let event = Event::MenuItem(menu.source.clone(), menu.focused);
                return self.action_event(event);
            }
            _ => {}
        }
        if let Some(menu) = &self.action_menu {
            let height = super::view::row_height(&self.configuration.appearance);
            return iced::widget::operation::scroll_to(
                menu_id(id),
                iced::widget::scrollable::AbsoluteOffset {
                    x: None,
                    y: Some((menu.focused.saturating_sub(4)) as f32 * height),
                },
            );
        }
        Task::none()
    }
}

fn menu_id(id: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-actions-menu-{id:?}"))
}

pub(super) fn overlay<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    // Keep the Browser subtree in the same Stack slot while opening/closing
    // menus; changing its root widget kind resets native scroll/input state.
    if let Some(target) = desktop
        .folder_options
        .as_ref()
        .filter(|target| target.window == id)
    {
        return folder_views::overlay(desktop, target, content);
    }
    let layers = stack![content];
    let Some(menu) = desktop
        .action_menu
        .as_ref()
        .filter(|menu| menu.source.window == id)
    else {
        return layers.into();
    };
    let appearance = &desktop.configuration.appearance;
    let size = appearance.typography().body_size.px();
    let font = canopa_ui::font(appearance);
    let view_menu = menu.is_view_menu();
    let current = desktop
        .active(id)
        .and_then(|tab| tab.browser.as_ref())
        .map(|browser| browser.view().mode);
    let mut items = column![].spacing(4);
    for (index, (label, item)) in menu.items.iter().enumerate() {
        let icon = match item {
            Item::Action(id) => desktop
                .configuration
                .actions
                .iter()
                .find(|action| &action.id == id)
                .and_then(|action| action.icon.as_deref()),
            Item::Command(_)
            | Item::OpenWith
            | Item::Application(_)
            | Item::OtherApplication
            | Item::FolderOptions
            | Item::Locations
            | Item::Back
            | Item::Device(..)
            | Item::View(_)
            | Item::Place(_) => None,
        };
        let message = Message::Action(Event::MenuItem(menu.source.clone(), index));
        items = items.push(super::accessibility_ui::semantic(
            super::accessibility_ui::Node::menu_item(label.clone())
                .key(format!("files-menu-item-{index}"))
                .position(index + 1, menu.items.len())
                .focused(index == menu.focused)
                .on_click(Some(message.clone())),
            canopa_ui::styled_action(
                match item {
                    Item::View(mode) => view_item(desktop, *mode, current, label, size, font),
                    _ => desktop.icons.label(icon, label, size, font, true),
                },
                Some(message),
                canopa_ui::Action::Quiet,
                index == menu.focused,
                appearance,
            )
            .width(Length::Fill),
        ));
    }
    let appearance = appearance.clone();
    let menu = container(scrollable(items).id(menu_id(id)).height(Length::Shrink))
        .padding(if view_menu { 6 } else { 8 })
        .max_width(if view_menu { 200 } else { 320 })
        .max_height(360)
        .width(Length::Fill)
        .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Menu)));
    let placed = container(opaque(super::accessibility_ui::semantic(
        super::accessibility_ui::Node::menu(if view_menu {
            fl!("files-view")
        } else {
            fl!("files-toolbar-actions")
        })
        .key("files-menu"),
        menu,
    )))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_right(Length::Fill);
    // The view menu opens above its status bar button, at the window's end.
    let placed = if view_menu {
        placed
            .padding(iced::Padding {
                bottom: VIEW_MENU_BOTTOM,
                right: 8.0,
                ..Default::default()
            })
            .align_bottom(Length::Fill)
    } else {
        placed.padding([74, 10]).align_top(Length::Fill)
    };
    layers
        .push(
            mouse_area(space().width(Length::Fill).height(Length::Fill))
                .on_press(Message::Action(Event::Close(id)))
                .on_right_press(Message::Action(Event::Close(id))),
        )
        .push(placed)
        .into()
}

/// Space below the view menu: the status bar and a small gap.
const VIEW_MENU_BOTTOM: f32 = 40.0;

/// A view menu item: the view's symbol, its name, and a check on the current one.
fn view_item<'a>(
    desktop: &'a Desktop,
    mode: crate::preferences::ViewMode,
    current: Option<crate::preferences::ViewMode>,
    label: &str,
    size: f32,
    font: iced::Font,
) -> Element<'a, Message> {
    let check: Element<'a, Message> = if current == Some(mode) {
        desktop.icons.chrome(super::chrome::Icon::Check, 16.0, 0.9)
    } else {
        space().width(16).into()
    };
    iced::widget::row![
        desktop
            .icons
            .chrome(super::view::view_icon(mode), 18.0, 0.92),
        iced::widget::text(label.to_owned())
            .font(font)
            .size(size)
            .width(Length::Fill),
        check,
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_action_controls_cannot_follow_selection_tabs_or_configuration() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first.txt");
        let second = root.path().join("second.txt");
        std::fs::write(&first, b"first").unwrap();
        std::fs::write(&second, b"second").unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned(); 2]);
        let tab = desktop.active(window).unwrap().id;
        super::super::tests::loaded(&mut desktop, window, tab, 1, root.path());
        desktop
            .active_mut(window)
            .unwrap()
            .browser
            .as_mut()
            .unwrap()
            .select(&first, SelectionMode::Replace);
        let source = desktop.action_source(window).unwrap();
        let action = crate::actions::CustomAction::new("inspect", "Inspect", "/bin/true %F");
        desktop.configuration.actions.push(action);
        assert!(desktop.matches_source(&source));
        desktop
            .active_mut(window)
            .unwrap()
            .browser
            .as_mut()
            .unwrap()
            .select(&second, SelectionMode::Replace);
        let _ = desktop.action_event(Event::Invoke(source.clone(), "inspect".into()));
        assert!(desktop.jobs.summary().is_none());
        let _ = desktop.action_event(Event::Builtin(source.clone(), Command::CustomActions));
        assert!(desktop.action_editor.is_none());
        desktop
            .active_mut(window)
            .unwrap()
            .browser
            .as_mut()
            .unwrap()
            .select(&first, SelectionMode::Replace);
        desktop.configuration_revision += 1;
        assert!(!desktop.matches_source(&source));
        let current = desktop.action_source(window).unwrap();
        let _ = desktop.update(Message::SwitchTab(window, 1));
        assert!(!desktop.matches_source(&current));
        let _ = desktop.action_event(Event::Invoke(current, "inspect".into()));
        assert!(desktop.jobs.summary().is_none());
    }

    #[test]
    fn context_click_preserves_a_selection_and_replaces_it_for_an_unselected_file() {
        let root = tempfile::tempdir().unwrap();
        let paths: Vec<_> = ["a", "b", "c"].map(|name| root.path().join(name)).into();
        for path in &paths {
            std::fs::write(path, b"fixture").unwrap();
        }
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let id = desktop.active(window).unwrap().id;
        super::super::tests::loaded(&mut desktop, window, id, 1, root.path());
        let browser = desktop
            .active_mut(window)
            .unwrap()
            .browser
            .as_mut()
            .unwrap();
        browser.select(&paths[0], SelectionMode::Replace);
        browser.select(&paths[1], SelectionMode::Toggle);
        let target = |desktop: &Desktop, index: usize| {
            let tab = desktop.active(window).unwrap();
            RowTarget {
                window,
                tab: tab.id,
                navigation: tab.revision,
                epoch: tab.list_epoch,
                column: root.path().to_owned(),
                path: paths[index].clone(),
            }
        };
        let _ = desktop.action_event(Event::Row(target(&desktop, 0)));
        let browser = desktop.active(window).unwrap().browser.as_ref().unwrap();
        assert_eq!(browser.selected().len(), 2);
        assert_eq!(browser.focused(), Some(paths[0].as_path()));
        assert_eq!(desktop.action_menu.as_ref().unwrap().source.paths.len(), 2);
        let _ = desktop.action_event(Event::Row(target(&desktop, 2)));
        let browser = desktop.active(window).unwrap().browser.as_ref().unwrap();
        assert_eq!(browser.selected().iter().collect::<Vec<_>>(), [&paths[2]]);
        let old = target(&desktop, 1);
        desktop.active_mut(window).unwrap().list_epoch += 1;
        let _ = desktop.action_event(Event::Row(old));
        assert_eq!(
            desktop
                .active(window)
                .unwrap()
                .browser
                .as_ref()
                .unwrap()
                .selected()
                .iter()
                .collect::<Vec<_>>(),
            [&paths[2]]
        );
    }
}
