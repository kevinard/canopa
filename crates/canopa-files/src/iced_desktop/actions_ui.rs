//! Every visible command control carries the selection and catalog it described.
//! Accepted requests keep their captured paths even when browsing moves on.
use super::chrome::Icon;
use super::*;
use crate::browse::Entry;
use canopa_theme::{ColorRole, UiKind};
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
    /// The toolbar's menu: the window's, folder's and application's commands.
    Menu(window::Id),
    /// A context menu: the selected items', or the folder's when none is selected.
    Context(window::Id),
    Locations(window::Id),
    /// The status bar's view menu.
    ViewMenu(window::Id),
    /// The toolbar's view menu, opening below its button.
    ToolbarViewMenu(window::Id),
    Row(RowTarget),
    /// A secondary click on a folder's empty space: its menu, without a selection.
    Background(window::Id, u64, u64),
    /// A secondary click on a tab: it becomes current and shows its menu.
    Tab(window::Id, usize),
    Close(window::Id),
    Builtin(Arc<Source>, Command),
    Invoke(Arc<Source>, String),
    MenuItem(Arc<Source>, usize),
    /// A row of the menu a submenu opened from: back to that menu, then the row.
    ParentItem(Arc<Source>, usize),
    /// Applications for the selection, resolved on a worker.
    OpenWithReady(Arc<Source>, Arc<Vec<Arc<canopa_apps::DesktopApplication>>>),
    /// The Templates folder's documents and the folder, if it exists, read
    /// on a worker.
    TemplatesReady(
        Arc<Source>,
        Arc<(Vec<crate::templates::Template>, Option<PathBuf>)>,
    ),
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
    /// Open this folder in a new tab.
    OpenTab(PathBuf),
    /// Close every tab of the window but the current one.
    CloseOtherTabs,
    /// Open the current tab's folder again in a new tab.
    DuplicateTab(PathBuf),
    /// Compress the selection, after choosing a name and format.
    Compress,
    /// Extract the selected archives beside them.
    ExtractHere,
    /// Extract the selected archives to a chosen folder.
    ExtractTo,
    /// Open the user's terminal in this folder.
    Terminal(PathBuf),
    /// The New document menu, listing the templates.
    NewDocument,
    /// A new document from this template, or an empty one.
    Template(Option<Arc<crate::templates::Template>>),
}
pub(super) struct Menu {
    source: Arc<Source>,
    items: Vec<(String, Item)>,
    focused: usize,
    /// Where a secondary click opened this menu; others open from the toolbar.
    anchor: Option<iced::Point>,
    /// Whether the focused item is shown: a menu opened from the keyboard or
    /// navigated with it highlights it, one opened by the pointer follows hover.
    keyboard: bool,
    /// The menu this one opened from, still drawn behind it with the opening
    /// row (its `focused` item) highlighted. Only the newest menu is used.
    parent: Option<Box<Menu>>,
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
        if self.picker.is_some()
            && matches!(
                event,
                Event::Context(_) | Event::Row(_) | Event::Background(..) | Event::Invoke(..)
            )
        {
            return Task::none();
        }
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
                let items = if let Some(picker) = &self.picker {
                    [
                        Command::Location,
                        Command::Search,
                        Command::ShowHidden,
                        Command::Refresh,
                        Command::NewFolder,
                    ]
                    .into_iter()
                    .filter(|command| picker.allows(*command))
                    .map(|command| (command.label(), Item::Command(command)))
                    .collect()
                } else {
                    self.application_items(id)
                };
                return self.present_menu(source, items);
            }
            Event::Context(id) => {
                let Some(source) = self.action_source(id) else {
                    return Task::none();
                };
                let items = if source.paths.is_empty() {
                    self.folder_items()
                } else {
                    self.selection_items(&source)
                };
                return self.present_menu(source, items);
            }
            Event::Locations(id) => {
                let Some(source) = self.action_source(id) else {
                    return Task::none();
                };
                return self.present_menu(source, self.location_items(false));
            }
            Event::ToolbarViewMenu(id) => {
                let below = self.windows.get(&id).and_then(|window| window.pointer);
                let task = self.action_event(Event::ViewMenu(id));
                if let Some(pointer) = below
                    && self
                        .action_menu
                        .as_ref()
                        .is_some_and(|menu| menu.window() == id)
                {
                    // Under the toolbar, where the button was pressed.
                    self.anchor_action_menu(
                        id,
                        Some(iced::Point::new(pointer.x, TOOLBAR_MENU_TOP)),
                    );
                    if let Some(menu) = &mut self.action_menu {
                        menu.keyboard = false;
                    }
                }
                return task;
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
            Event::Tab(window, index) => {
                // The menu opens below the bar, so the tabs stay in view.
                let anchor = self.windows.get(&window).and_then(|window| {
                    let pointer = window.pointer?;
                    let bottom = window.tab_geometry.lock().ok()?.bottom();
                    Some(iced::Point::new(pointer.x, bottom + 2.0))
                });
                let switched = self.update(Message::SwitchTab(window, index));
                let Some(source) = self.action_source(window) else {
                    return switched;
                };
                let several = self
                    .windows
                    .get(&window)
                    .is_some_and(|window| window.tabs.len() > 1);
                let mut items = vec![
                    (Command::NewTab.label(), Item::Command(Command::NewTab)),
                    (
                        fl!("files-menu-duplicate-tab"),
                        Item::DuplicateTab(source.directory.clone()),
                    ),
                ];
                if several {
                    items.push((
                        Command::MoveTabToWindow.label(),
                        Item::Command(Command::MoveTabToWindow),
                    ));
                }
                items.push((Command::CloseTab.label(), Item::Command(Command::CloseTab)));
                if several {
                    items.push((fl!("files-menu-close-other-tabs"), Item::CloseOtherTabs));
                }
                let menu = self.present_menu(source, items);
                self.anchor_action_menu(window, anchor);
                return Task::batch([switched, menu]);
            }
            Event::Background(window, tab, navigation) => {
                let anchor = self.windows.get(&window).and_then(|window| window.pointer);
                let Some(current) = self.active_mut(window).filter(|current| {
                    current.id == tab && current.revision == navigation && !current.loading
                }) else {
                    return Task::none();
                };
                current.location = None;
                if let Some(browser) = current.browser.as_mut() {
                    browser.clear_selection();
                }
                let task = self.action_event(Event::Context(window));
                self.anchor_action_menu(window, anchor);
                return task;
            }
            Event::Row(target) => {
                let anchor = self
                    .windows
                    .get(&target.window)
                    .and_then(|window| window.pointer);
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
                        (
                            self.configuration.settings.sidebar,
                            self.sidebar_width(target.window),
                        ),
                        window.panel_width(self.properties_width(target.window)),
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
                        tab.menu_anchor = anchor;
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
                let task = self.action_event(Event::Context(target.window));
                self.anchor_action_menu(target.window, anchor);
                return task;
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
                // It replaces its menu where it is, unless that was dismissed.
                if !self.matches_source(&source)
                    || !self
                        .action_menu
                        .as_ref()
                        .is_some_and(|menu| Arc::ptr_eq(&menu.source, &source))
                {
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
            Event::TemplatesReady(source, found) => {
                // Only while the menu that asked is still open at its row.
                if !self.matches_source(&source)
                    || !self.action_menu.as_ref().is_some_and(|menu| {
                        Arc::ptr_eq(&menu.source, &source)
                            && menu.parent.is_none()
                            && matches!(menu.items.get(menu.focused), Some((_, Item::NewDocument)))
                    })
                {
                    return Task::none();
                }
                let (templates, folder) = &*found;
                let mut items = vec![(fl!("files-menu-new-document-empty"), Item::Template(None))];
                items.extend(templates.iter().map(|template| {
                    (
                        template.label.clone(),
                        Item::Template(Some(Arc::new(template.clone()))),
                    )
                }));
                if let Some(folder) = folder {
                    items.push((
                        fl!("files-menu-templates-folder"),
                        Item::Place(folder.clone()),
                    ));
                }
                return self.present_submenu(items);
            }
            Event::ParentItem(source, index) => {
                if let Some(menu) = self
                    .action_menu
                    .as_mut()
                    .filter(|menu| Arc::ptr_eq(&menu.source, &source) && menu.parent.is_some())
                {
                    self.action_menu = menu.parent.take().map(|parent| *parent);
                    return self.action_event(Event::MenuItem(source, index));
                }
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
                if !self.item_enabled(&item) {
                    return Task::none();
                }
                // A menu whose next menu is read on a worker stays open, so
                // that one can open beside, or in place of, it.
                if matches!(item, Item::NewDocument | Item::OpenWith) {
                    if let Some(menu) = self.action_menu.as_mut() {
                        menu.focused = index;
                    }
                } else {
                    self.action_menu = None;
                }
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
                    Item::OpenTab(path) | Item::DuplicateTab(path) => {
                        self.update(Message::OpenTab(source.window, path))
                    }
                    Item::CloseOtherTabs => {
                        let others: Vec<u64> = self
                            .windows
                            .get(&source.window)
                            .map(|window| {
                                let current = window.tabs.get(window.active).map(|tab| tab.id);
                                window
                                    .tabs
                                    .iter()
                                    .map(|tab| tab.id)
                                    .filter(|tab| Some(*tab) != current)
                                    .collect()
                            })
                            .unwrap_or_default();
                        let mut tasks = Vec::new();
                        for tab in others {
                            tasks.push(self.close_tab(source.window, tab));
                        }
                        Task::batch(tasks)
                    }
                    Item::Action(id) => self.action_event(Event::Invoke(source, id)),
                    Item::Compress => self.show_compress_dialog(source.window),
                    Item::ExtractHere => self.extract_here(source.window),
                    Item::ExtractTo => self.show_extract_dialog(source.window),
                    Item::Terminal(folder) => self.launch(
                        source.window,
                        super::open_with_ui::Launch::TerminalAt(folder),
                    ),
                    Item::NewDocument => {
                        let folder = self.templates_folder();
                        let home = self.home.clone();
                        Task::perform(
                            async move {
                                let (send, receive) = iced::futures::channel::oneshot::channel();
                                let _ = std::thread::Builder::new()
                                    .name("files-templates".into())
                                    .spawn(move || {
                                        let folder = folder.filter(|folder| folder.is_dir());
                                        let templates =
                                            folder.as_ref().map_or_else(Vec::new, |folder| {
                                                crate::templates::list(folder, &home)
                                            });
                                        let _ = send.send((templates, folder));
                                    });
                                Arc::new(receive.await.unwrap_or_default())
                            },
                            move |templates| {
                                Message::Action(Event::TemplatesReady(source.clone(), templates))
                            },
                        )
                    }
                    Item::Template(template) => self.show_document_dialog(
                        source.window,
                        template.map(|template| (*template).clone()),
                    ),
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

    /// Whether a shown item can be chosen now; others are drawn unavailable.
    fn item_enabled(&self, item: &Item) -> bool {
        match item {
            Item::Command(Command::Paste) => self.file_clipboard.can_paste(),
            _ => true,
        }
    }

    /// Open the menu just shown in `window` at `anchor`, where it was asked for.
    pub(super) fn anchor_action_menu(&mut self, window: window::Id, anchor: Option<iced::Point>) {
        if let Some(menu) = self
            .action_menu
            .as_mut()
            .filter(|menu| menu.source.window == window)
        {
            menu.keyboard = anchor.is_none();
            menu.anchor = anchor;
        }
    }

    fn present_menu(&mut self, source: Arc<Source>, items: Vec<(String, Item)>) -> Task<Message> {
        let id = source.window;
        // A menu that replaces another in the same window, such as Open With
        // after its applications resolve, opens where the first one was.
        let (anchor, keyboard) = self
            .action_menu
            .as_ref()
            .filter(|menu| menu.source.window == id)
            .map_or((None, true), |menu| (menu.anchor, menu.keyboard));
        self.action_menu = Some(Menu {
            source,
            items,
            focused: 0,
            anchor,
            keyboard,
            parent: None,
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

    /// Open `items` beside the open menu's focused row, keeping that menu.
    fn present_submenu(&mut self, items: Vec<(String, Item)>) -> Task<Message> {
        let Some(parent) = self.action_menu.take() else {
            return Task::none();
        };
        self.action_menu = Some(Menu {
            source: parent.source.clone(),
            items,
            focused: 0,
            anchor: None,
            keyboard: parent.keyboard,
            parent: Some(Box::new(parent)),
        });
        Task::none()
    }

    /// What can be done with the selected items, and nothing else.
    fn selection_items(&self, source: &Source) -> Vec<(String, Item)> {
        let mut items = vec![(fl!("files-menu-open"), Item::Command(Command::Open))];
        if let [folder] = source.entries.as_slice()
            && folder.is_directory()
        {
            items.push((
                fl!("files-menu-open-in-new-tab"),
                Item::OpenTab(folder.path.clone()),
            ));
            items.push((
                Command::OpenTerminal.label(),
                Item::Terminal(folder.path.clone()),
            ));
        }
        if source.entries.len() == source.paths.len()
            && source.entries.iter().all(|entry| !entry.is_directory())
        {
            items.push((fl!("files-menu-open-with"), Item::OpenWith));
        }
        items.push((
            fl!("files-command-quick-look"),
            Item::Command(Command::QuickLook),
        ));
        for action in &self.configuration.actions {
            if self.action_applicable(source, action) {
                items.push((action.name.clone(), Item::Action(action.id.clone())));
            }
        }
        if self.file_clipboard.ready()
            && self.file_operations.storage.is_some()
            && self.file_operations.storage_error.is_none()
        {
            items.push((fl!("files-command-copy"), Item::Command(Command::Copy)));
            items.push((fl!("files-command-cut"), Item::Command(Command::Cut)));
        }
        if self.file_operations.ready() {
            // Archives can be extracted only when every selected item is one.
            if source.entries.len() == source.paths.len()
                && source.entries.iter().all(|entry| {
                    !entry.is_directory()
                        && entry
                            .path
                            .file_name()
                            .and_then(crate::archives::Format::detect)
                            .is_some()
                })
            {
                items.push((fl!("files-menu-extract-here"), Item::ExtractHere));
                items.push((fl!("files-menu-extract-to"), Item::ExtractTo));
            }
            items.push((fl!("files-menu-compress"), Item::Compress));
            // Several items of one folder open the bulk Rename dialog.
            if source.paths.len() == 1 || super::name_dialog::batch_folder(&source.paths).is_some()
            {
                items.push((fl!("files-menu-rename"), Item::Command(Command::Rename)));
            }
            items.push((fl!("files-command-trash"), Item::Command(Command::Trash)));
            items.push((
                fl!("files-menu-delete-permanently"),
                Item::Command(Command::DeletePermanently),
            ));
        }
        items.push((
            fl!("files-command-properties"),
            Item::Command(Command::Properties),
        ));
        items
    }

    /// What can be done in the open folder itself.
    fn folder_items(&self) -> Vec<(String, Item)> {
        let mut items = Vec::new();
        if self.file_operations.ready() {
            items.push((
                fl!("files-menu-new-folder"),
                Item::Command(Command::NewFolder),
            ));
            items.push((fl!("files-menu-new-document"), Item::NewDocument));
            // Paste keeps its place, unavailable while there is nothing to paste.
            if self.file_clipboard.ready() {
                items.push((fl!("files-command-paste"), Item::Command(Command::Paste)));
            }
            items.extend(self.undo_item());
        }
        items.push((
            fl!("files-command-select-all"),
            Item::Command(Command::SelectAll),
        ));
        items.push((
            Command::OpenTerminal.label(),
            Item::Command(Command::OpenTerminal),
        ));
        items.push((
            fl!("files-menu-bookmark-folder"),
            Item::Command(Command::BookmarkFolder),
        ));
        items.push((fl!("files-menu-view-options"), Item::FolderOptions));
        items.push((
            fl!("files-command-properties"),
            Item::Command(Command::Properties),
        ));
        items
    }

    /// The toolbar's menu: windows, the folder and the application, without
    /// anything that acts on the selection.
    fn application_items(&self, id: window::Id) -> Vec<(String, Item)> {
        let mut items = vec![
            (
                fl!("files-command-new-window"),
                Item::Command(Command::NewWindow),
            ),
            (fl!("files-command-new-tab"), Item::Command(Command::NewTab)),
        ];
        if self
            .windows
            .get(&id)
            .is_some_and(|window| window.tabs.len() > 1)
        {
            items.push((
                fl!("files-command-move-tab-to-window"),
                Item::Command(Command::MoveTabToWindow),
            ));
        }
        if self.file_operations.ready() {
            items.push((
                fl!("files-menu-new-folder"),
                Item::Command(Command::NewFolder),
            ));
            items.push((fl!("files-menu-new-document"), Item::NewDocument));
            items.extend(self.undo_item());
        }
        items.push((
            Command::OpenTerminal.label(),
            Item::Command(Command::OpenTerminal),
        ));
        items.push((
            fl!("files-menu-bookmark-folder"),
            Item::Command(Command::BookmarkFolder),
        ));
        items.push((fl!("files-menu-locations"), Item::Locations));
        items.push((fl!("files-menu-view-options"), Item::FolderOptions));
        items.push((
            fl!("files-command-show-hidden"),
            Item::Command(Command::ShowHidden),
        ));
        items.push((
            fl!("files-command-show-trash"),
            Item::Command(Command::ShowTrash),
        ));
        items.push((
            fl!("files-command-operations"),
            Item::Command(Command::Operations),
        ));
        items.push((
            fl!("files-menu-edit-custom-actions"),
            Item::Command(Command::CustomActions),
        ));
        items.push((
            fl!("files-menu-preferences"),
            Item::Command(Command::Preferences),
        ));
        items
    }

    /// The XDG Templates folder, unless it is Home, which means none.
    pub(super) fn templates_folder(&self) -> Option<PathBuf> {
        self.personal_folders
            .path(crate::interface::PlaceKind::Templates)
            .filter(|folder| *folder != self.home)
            .map(Path::to_owned)
    }

    fn undo_item(&self) -> Option<(String, Item)> {
        let undo = self.file_operations.snapshot.undo.as_ref()?;
        (!self.file_operations.has_active()).then(|| {
            (
                crate::history_labels::display(&undo.label),
                Item::Command(Command::Undo),
            )
        })
    }

    fn location_items(&self, back: bool) -> Vec<(String, Item)> {
        use crate::interface::{Place, PlaceKind};

        let mut items = Vec::new();
        if back {
            items.push((fl!("files-command-back"), Item::Back));
        }
        for place in &self.configuration.settings.places {
            if self.picker.as_ref().is_some_and(|picker| match place {
                Place::Builtin(PlaceKind::Trash | PlaceKind::Recovery) => true,
                Place::Builtin(PlaceKind::Recent) => !picker.allows(Command::ShowRecent),
                _ => false,
            }) {
                continue;
            }
            let item = match place {
                Place::Builtin(PlaceKind::Recent) => Some(Item::Command(Command::ShowRecent)),
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
                if self.picker.is_some() {
                    if device.root.is_some() {
                        items.push((device.name.clone(), Item::Device(device.token, false)));
                    }
                    continue;
                }
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
        if self.picker.is_some() {
            return None;
        }
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
        if !matches!(
            key.as_ref(),
            keyboard::Key::Named(keyboard::key::Named::Enter | keyboard::key::Named::Space)
                | keyboard::Key::Character(" ")
        ) {
            menu.keyboard = true;
        }
        let rtl = canopa_ui::direction::is_rtl();
        let (back, forward) = if rtl {
            (
                keyboard::key::Named::ArrowRight,
                keyboard::key::Named::ArrowLeft,
            )
        } else {
            (
                keyboard::key::Named::ArrowLeft,
                keyboard::key::Named::ArrowRight,
            )
        };
        match key.as_ref() {
            keyboard::Key::Named(named)
                if (named == keyboard::key::Named::Escape || named == back)
                    && menu.parent.is_some() =>
            {
                self.action_menu = menu.parent.take().map(|parent| *parent);
                return Task::none();
            }
            keyboard::Key::Named(named)
                if named == forward
                    && menu
                        .items
                        .get(menu.focused)
                        .is_some_and(|(_, item)| opens_menu(item)) =>
            {
                let event = Event::MenuItem(menu.source.clone(), menu.focused);
                return self.action_event(event);
            }
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

/// Items that open another menu, drawn with a chevron.
fn opens_menu(item: &Item) -> bool {
    matches!(item, Item::OpenWith | Item::Locations | Item::NewDocument)
}

fn menu_id(id: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-actions-menu-{id:?}"))
}

fn parent_menu_id(id: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-actions-parent-menu-{id:?}"))
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
    // A submenu is drawn beside the menu it opened from, which stays in view
    // without taking input.
    let root = menu.parent.as_deref().unwrap_or(menu);
    let root_width = menu_width(desktop, &root.items, root.is_view_menu());
    let placed = place_root(
        desktop,
        id,
        root,
        root_width,
        panel(desktop, id, root, menu.parent.is_none()),
    );
    let mut layers = layers
        .push(
            mouse_area(space().width(Length::Fill).height(Length::Fill))
                .on_press(Message::Action(Event::Close(id)))
                .on_right_press(Message::Action(Event::Close(id))),
        )
        .push(placed);
    if menu.parent.is_some() {
        let (left, top) = beside(desktop, id, root, root_width, menu);
        layers = layers.push(
            container(panel(desktop, id, menu, true))
                .padding(iced::Padding {
                    left,
                    top,
                    ..iced::Padding::ZERO
                })
                .width(Length::Fill)
                .height(Length::Fill)
                .align_left(Length::Fill)
                .align_top(Length::Fill),
        );
    }
    layers.into()
}

/// A menu's surface and rows. Only the `active` menu takes input; a parent
/// shows the row its submenu opened from.
fn panel<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    menu: &'a Menu,
    active: bool,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let size = appearance.typography().body_size.px();
    let font = canopa_ui::font(appearance);
    let view_menu = menu.is_view_menu();
    let current = desktop
        .active(id)
        .and_then(|tab| tab.browser.as_ref())
        .map(|browser| browser.view().mode);
    let width = menu_width(desktop, &menu.items, view_menu);
    let prefix = if active {
        "files-menu"
    } else {
        "files-parent-menu"
    };
    let mut items = column![].spacing(1);
    let mut previous_group = None;
    for (index, (label, item)) in menu.items.iter().enumerate() {
        let group = item_group(item);
        if previous_group.is_some_and(|previous| previous != group) {
            items = items.push(menu_separator(appearance));
        }
        previous_group = Some(group);
        let destructive = matches!(item, Item::Command(Command::DeletePermanently));
        let enabled = desktop.item_enabled(item);
        let message = Message::Action(if active {
            Event::MenuItem(menu.source.clone(), index)
        } else {
            Event::ParentItem(menu.source.clone(), index)
        });
        let content = match item {
            Item::View(mode) => view_item(desktop, *mode, current, label, size, font),
            _ => menu_row(desktop, item, label, size, font, destructive, enabled),
        };
        let focused = index == menu.focused && (menu.keyboard || !active);
        let row_appearance = appearance.clone();
        items = items.push(super::accessibility_ui::semantic(
            super::accessibility_ui::Node::menu_item(label.clone())
                .key(format!("{prefix}-item-{index}"))
                .position(index + 1, menu.items.len())
                .focused(active && index == menu.focused)
                .on_click(Some(message.clone())),
            iced::widget::button(content)
                .padding(MENU_ROW_PADDING)
                .on_press_maybe(enabled.then_some(message))
                .width(Length::Fill)
                .style(move |_, status| {
                    menu_row_style(&row_appearance, status, focused, destructive)
                }),
        ));
    }
    let appearance = appearance.clone();
    let surface = container(
        scrollable(items)
            .style(canopa_ui::scrollbar)
            .id(if active {
                menu_id(id)
            } else {
                parent_menu_id(id)
            })
            .height(Length::Shrink),
    )
    .padding(MENU_PADDING)
    .width(width)
    .max_height(MENU_MAX_HEIGHT)
    .style(move |_| menu_surface(&appearance));
    opaque(super::accessibility_ui::semantic(
        super::accessibility_ui::Node::menu(if view_menu {
            fl!("files-view")
        } else {
            fl!("files-toolbar-actions")
        })
        .key(prefix),
        surface,
    ))
}

const EDGE: f32 = 8.0;

/// The first menu of a window: at a secondary click's pointer, turned back from
/// the window's right and bottom edges so it stays whole, or below the toolbar.
fn place_root<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    menu: &Menu,
    width: f32,
    panel: Element<'a, Message>,
) -> Element<'a, Message> {
    let placed = container(panel).width(Length::Fill).height(Length::Fill);
    let window = desktop.windows.get(&id);
    let placed = if let (Some(anchor), Some(window)) = (menu.anchor, window) {
        let height = menu_height(desktop, &menu.items);
        let mut padding = iced::Padding::ZERO;
        let placed = if anchor.x + width + EDGE <= window.width {
            padding.left = anchor.x.max(EDGE);
            placed.align_left(Length::Fill)
        } else {
            padding.right = (window.width - anchor.x).max(EDGE);
            placed.align_right(Length::Fill)
        };
        let placed = if anchor.y + height + EDGE <= window.height {
            padding.top = anchor.y.max(EDGE);
            placed.align_top(Length::Fill)
        } else if anchor.y - height >= EDGE {
            padding.bottom = (window.height - anchor.y).max(EDGE);
            placed.align_bottom(Length::Fill)
        } else {
            padding.top = (window.height - height - EDGE).max(EDGE);
            placed.align_top(Length::Fill)
        };
        placed.padding(padding)
    } else {
        placed.align_right(Length::Fill)
    };
    // Opened from the keyboard, the view menu drops from the toolbar's end,
    // where its button sits.
    if menu.anchor.is_some() {
        placed
    } else if menu.is_view_menu() {
        placed
            .padding(iced::Padding {
                top: TOOLBAR_MENU_TOP,
                right: VIEW_MENU_RIGHT,
                ..Default::default()
            })
            .align_top(Length::Fill)
    } else {
        placed
            .padding([TOOLBAR_MENU_BELOW, TOOLBAR_MENU_RIGHT])
            .align_top(Length::Fill)
    }
    .into()
}

/// Where `root` is drawn, as `place_root` places it: its left and top edges.
fn root_origin(desktop: &Desktop, id: window::Id, root: &Menu, width: f32) -> (f32, f32) {
    let Some(window) = desktop.windows.get(&id) else {
        return (EDGE, TOOLBAR_MENU_BELOW);
    };
    let height = menu_height(desktop, &root.items);
    match root.anchor {
        Some(anchor) => {
            let left = if anchor.x + width + EDGE <= window.width {
                anchor.x.max(EDGE)
            } else {
                window.width - (window.width - anchor.x).max(EDGE) - width
            };
            let top = if anchor.y + height + EDGE <= window.height {
                anchor.y.max(EDGE)
            } else if anchor.y - height >= EDGE {
                window.height - (window.height - anchor.y).max(EDGE) - height
            } else {
                (window.height - height - EDGE).max(EDGE)
            };
            (left, top)
        }
        None => (
            window.width - TOOLBAR_MENU_RIGHT - width,
            TOOLBAR_MENU_BELOW,
        ),
    }
}

/// A submenu's left and top edges: its first row level with the row that
/// opened it, after the menu's trailing edge, or before its leading edge when
/// it would leave the window.
fn beside(
    desktop: &Desktop,
    id: window::Id,
    root: &Menu,
    root_width: f32,
    menu: &Menu,
) -> (f32, f32) {
    let (root_left, root_top) = root_origin(desktop, id, root, root_width);
    let width = menu_width(desktop, &menu.items, false);
    let height = menu_height(desktop, &menu.items);
    let (window_width, window_height) = desktop
        .windows
        .get(&id)
        .map_or((f32::MAX, f32::MAX), |window| (window.width, window.height));
    let size = desktop.configuration.appearance.typography().body_size.px();
    let row = (size * 1.3).ceil() + MENU_ROW_PADDING[0] * 2.0 + 1.0;
    let opener = root.focused.min(root.items.len().saturating_sub(1));
    let separators = root.items[..=opener]
        .windows(2)
        .filter(|pair| item_group(&pair[0].1) != item_group(&pair[1].1))
        .count();
    let row_top = root_top + opener as f32 * row + separators as f32 * MENU_SEPARATOR;
    // Overlap the edges slightly, so the two surfaces read as one gesture.
    const OVERLAP: f32 = 4.0;
    let after = root_left + root_width - OVERLAP;
    let before = root_left - width + OVERLAP;
    let fits_after = after + width + EDGE <= window_width;
    let fits_before = before >= EDGE;
    let left = match (canopa_ui::direction::is_rtl(), fits_after, fits_before) {
        (false, true, _) | (true, true, false) => after,
        (true, _, true) | (false, false, true) => before,
        _ => (window_width - width - EDGE).max(EDGE),
    };
    let top = row_top.min(window_height - height - EDGE).max(EDGE);
    (left, top)
}

/// Where a menu opened from the toolbar starts: just below its bar.
const TOOLBAR_MENU_TOP: f32 = 62.0;
/// The toolbar menu's top and trailing edge when opened from the keyboard.
const TOOLBAR_MENU_BELOW: f32 = 74.0;
const TOOLBAR_MENU_RIGHT: f32 = 10.0;
/// The view menu's trailing edge, under its button.
const VIEW_MENU_RIGHT: f32 = 52.0;
const MENU_MIN_WIDTH: f32 = 220.0;
const MENU_MAX_WIDTH: f32 = 380.0;
const VIEW_MENU_WIDTH: f32 = 200.0;
const MENU_MAX_HEIGHT: f32 = 520.0;
const MENU_PADDING: f32 = 6.0;
const MENU_ROW_PADDING: [f32; 2] = [6.0, 10.0];
/// A separator's line and the space around it.
const MENU_SEPARATOR: f32 = 9.0;
const MENU_ICON: f32 = 16.0;

/// Items that belong together; a line separates neighbouring groups.
fn item_group(item: &Item) -> u8 {
    match item {
        Item::Command(command) => match command {
            Command::Open | Command::QuickLook => 0,
            Command::NewWindow | Command::NewTab | Command::MoveTabToWindow => 2,
            Command::NewFolder
            | Command::Copy
            | Command::Cut
            | Command::Paste
            | Command::Undo
            | Command::SelectAll => 4,
            Command::Rename | Command::Trash | Command::DeletePermanently => 5,
            Command::BookmarkFolder | Command::ShowHidden => 6,
            Command::OpenTerminal => 17,
            Command::ShowRecent | Command::ShowTrash | Command::Operations | Command::Recovery => 7,
            Command::CustomActions | Command::Preferences => 8,
            Command::Properties => 9,
            _ => 10,
        },
        Item::OpenTab(_) | Item::OpenWith => 0,
        Item::Compress | Item::ExtractHere | Item::ExtractTo => 16,
        Item::NewDocument => 4,
        Item::Terminal(_) => 17,
        // The New document menu: the empty document and templates, then their folder.
        Item::Template(_) => 18,
        Item::CloseOtherTabs => 10,
        Item::DuplicateTab(_) => 2,
        Item::Action(_) => 1,
        Item::Back => 11,
        Item::Application(_) => 12,
        Item::OtherApplication => 13,
        Item::Locations | Item::FolderOptions => 6,
        Item::Place(_) | Item::Device(..) => 14,
        Item::View(_) => 15,
    }
}

fn item_icon(item: &Item) -> Option<Icon> {
    Some(match item {
        Item::Command(command) => match command {
            Command::Open => Icon::Open,
            Command::QuickLook => Icon::Eye,
            Command::NewWindow | Command::MoveTabToWindow => Icon::Window,
            Command::NewTab => Icon::Plus,
            Command::NewFolder => Icon::NewFolder,
            Command::Copy => Icon::Copy,
            Command::Cut => Icon::Cut,
            Command::Paste => Icon::Paste,
            Command::Undo => Icon::Undo,
            Command::SelectAll => Icon::SelectAll,
            Command::Rename => Icon::Edit,
            Command::Trash | Command::ShowTrash => Icon::Trash,
            Command::DeletePermanently => Icon::DangerTrash,
            Command::BookmarkFolder => Icon::Bookmark,
            Command::ShowHidden => Icon::Hidden,
            Command::Operations => Icon::Activity,
            Command::Recovery => Icon::Recovery,
            Command::CustomActions | Command::OpenTerminal => Icon::Terminal,
            Command::Preferences => Icon::Settings,
            Command::Properties => Icon::Info,
            Command::CloseTab => Icon::Close,
            _ => return None,
        },
        Item::OpenTab(_) | Item::DuplicateTab(_) => Icon::Tab,
        Item::CloseOtherTabs => Icon::CloseOthers,
        Item::OpenWith => Icon::OpenWith,
        Item::Compress => Icon::Compress,
        Item::Terminal(_) => Icon::Terminal,
        Item::NewDocument | Item::Template(None) => Icon::Plus,
        Item::Template(Some(_)) => Icon::Templates,
        Item::ExtractHere | Item::ExtractTo => Icon::Extract,
        Item::Back => Icon::Back,
        Item::Locations => Icon::Places,
        Item::FolderOptions => Icon::Grid,
        Item::Place(_) => Icon::Folder,
        Item::Device(_, true) => Icon::Eject,
        Item::Device(_, false) => Icon::Drive,
        Item::OtherApplication => Icon::More,
        Item::Action(_) | Item::Application(_) | Item::View(_) => return None,
    })
}

fn item_shortcut(desktop: &Desktop, item: &Item) -> Option<String> {
    let Item::Command(command) = item else {
        return None;
    };
    desktop
        .configuration
        .settings
        .bindings(*command)
        .iter()
        .find_map(|binding| crate::interface::KeyChord::parse(binding).ok())
        .map(|chord| chord.display())
}

/// The width that shows every item's name and shortcut whole, within bounds.
fn menu_width(desktop: &Desktop, items: &[(String, Item)], view_menu: bool) -> f32 {
    if view_menu {
        return VIEW_MENU_WIDTH;
    }
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let body = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let widest = items
        .iter()
        .map(|(label, item)| {
            chrome::text_width(label, font, body)
                + item_shortcut(desktop, item).map_or(0.0, |shortcut| {
                    24.0 + chrome::text_width(&shortcut, font, small)
                })
        })
        .fold(0.0, f32::max);
    (widest + MENU_ICON + 10.0 + MENU_ROW_PADDING[1] * 2.0 + MENU_PADDING * 2.0 + 6.0)
        .ceil()
        .clamp(MENU_MIN_WIDTH, MENU_MAX_WIDTH)
}

/// An item's symbol, its name and, at the end, its first shortcut.
fn menu_row<'a>(
    desktop: &'a Desktop,
    item: &Item,
    label: &str,
    size: f32,
    font: iced::Font,
    destructive: bool,
    enabled: bool,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let custom = match item {
        Item::Action(id) => desktop
            .configuration
            .actions
            .iter()
            .find(|action| &action.id == id)
            .and_then(|action| action.icon.as_deref()),
        _ => None,
    };
    let icon: Element<'a, Message> = match (custom, item_icon(item)) {
        (Some(name), _) => desktop.icons.named(name, MENU_ICON),
        (None, Some(icon)) => desktop.icons.chrome(
            icon,
            MENU_ICON,
            if !enabled {
                0.35
            } else if destructive {
                1.0
            } else {
                0.78
            },
        ),
        (None, None) => space().width(MENU_ICON).height(MENU_ICON).into(),
    };
    let shortcut = item_shortcut(desktop, item);
    let mut label = iced::widget::text(label.to_owned())
        .font(font)
        .size(size)
        .wrapping(iced::widget::text::Wrapping::None);
    if !enabled {
        label = label.color(chrome::ink(appearance, ColorRole::Disabled));
    } else if destructive {
        label = label.color(chrome::ink(appearance, ColorRole::Danger));
    }
    let mut content = iced::widget::row![icon, container(label).width(Length::Fill).clip(true),]
        .spacing(10)
        .align_y(iced::Alignment::Center);
    // Items that open another menu end with a chevron instead of a shortcut.
    if opens_menu(item) {
        content = content.push(desktop.icons.chrome(Icon::Forward, 14.0, 0.6));
    } else if let Some(shortcut) = shortcut {
        content = content.push(
            iced::widget::text(shortcut)
                .font(font)
                .size(appearance.typography().label_size.px())
                .color(chrome::ink(
                    appearance,
                    if enabled {
                        ColorRole::Muted
                    } else {
                        ColorRole::Disabled
                    },
                ))
                .wrapping(iced::widget::text::Wrapping::None),
        );
    }
    content.into()
}

/// Hover and keyboard focus fill the row with the selection tint; a
/// destructive row takes a wash of its own colour instead.
fn menu_row_style(
    appearance: &canopa_theme::Appearance,
    status: iced::widget::button::Status,
    focused: bool,
    destructive: bool,
) -> iced::widget::button::Style {
    use iced::widget::button::Status;
    let engaged = focused || matches!(status, Status::Hovered | Status::Pressed);
    let background = engaged.then(|| {
        if destructive {
            chrome::ink(appearance, ColorRole::Danger).scale_alpha(
                if appearance.color_scheme == canopa_theme::ColorScheme::Dark {
                    0.18
                } else {
                    0.1
                },
            )
        } else if status == Status::Pressed {
            chrome::mix(
                chrome::ink(appearance, ColorRole::AccentSurface),
                chrome::ink(appearance, ColorRole::Accent),
                0.12,
            )
        } else {
            chrome::ink(appearance, ColorRole::AccentSurface)
        }
    });
    iced::widget::button::Style {
        background: background.map(Into::into),
        text_color: chrome::ink(appearance, ColorRole::Text),
        border: iced::Border {
            radius: 8.0.into(),
            // High contrast keeps an outline on the focused row.
            width: if focused && appearance.high_contrast() {
                2.0
            } else {
                0.0
            },
            color: chrome::ink(appearance, ColorRole::Focus),
        },
        ..Default::default()
    }
}

fn menu_separator<'a>(appearance: &canopa_theme::Appearance) -> Element<'a, Message> {
    let mut line = chrome::ink(appearance, ColorRole::Border);
    line.a = 0.32;
    container(
        container(space().height(1))
            .width(Length::Fill)
            .height(1)
            .style(move |_| container::Style::default().background(line)),
    )
    .padding([((MENU_SEPARATOR - 1.0) / 2.0), 8.0])
    .into()
}

/// The menu's raised surface: the shared menu frame on a soft shadow.
pub(super) fn menu_surface(appearance: &canopa_theme::Appearance) -> container::Style {
    let style = appearance.style(UiKind::Menu);
    let mut frame = canopa_ui::frame(appearance, style);
    let dark = appearance.color_scheme == canopa_theme::ColorScheme::Dark;
    if !style.custom_radius {
        frame.border.radius = 12.0.into();
    }
    if !style.custom_border && !appearance.high_contrast() {
        frame.border.color.a = if dark { 0.55 } else { 0.38 };
    }
    if dark
        && appearance.customization.ui[UiKind::Menu as usize]
            .background
            .is_none()
    {
        // A menu over a dark browser is lifted above the raised tone so it
        // separates from lists and dialogs alike.
        frame.background = Some(
            chrome::mix(
                chrome::ink(appearance, ColorRole::Raised),
                chrome::ink(appearance, ColorRole::Text),
                0.05,
            )
            .into(),
        );
    }
    frame.shadow = iced::Shadow {
        color: iced::Color::BLACK.scale_alpha(if dark { 0.5 } else { 0.16 }),
        offset: iced::Vector::new(0.0, 8.0),
        blur_radius: 24.0,
    };
    frame
}

/// The height the menu will take, to turn it back from the window's edges.
fn menu_height(desktop: &Desktop, items: &[(String, Item)]) -> f32 {
    let size = desktop.configuration.appearance.typography().body_size.px();
    let row = (size * 1.3).ceil() + MENU_ROW_PADDING[0] * 2.0 + 1.0;
    let separators = items
        .windows(2)
        .filter(|pair| item_group(&pair[0].1) != item_group(&pair[1].1))
        .count();
    (items.len() as f32 * row + separators as f32 * MENU_SEPARATOR + MENU_PADDING * 2.0 + 2.0)
        .min(MENU_MAX_HEIGHT)
}

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
    fn archive_items_appear_only_where_they_apply() {
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let archive = root.path().join("photos.tar.gz");
        let text = root.path().join("notes.txt");
        let folder = root.path().join("folder.zip");
        std::fs::write(&archive, b"x").unwrap();
        std::fs::write(&text, b"x").unwrap();
        std::fs::create_dir(&folder).unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let tab = desktop.active(window).unwrap().id;
        super::super::tests::loaded(&mut desktop, window, tab, 1, root.path());
        let labels = |desktop: &mut Desktop, selection: &[&PathBuf]| {
            desktop
                .active_mut(window)
                .unwrap()
                .browser
                .as_mut()
                .unwrap()
                .set_selection(selection.iter().map(|path| path.to_path_buf()));
            let source = desktop.action_source(window).unwrap();
            desktop
                .selection_items(&source)
                .into_iter()
                .map(|(label, _)| label)
                .collect::<Vec<_>>()
        };
        // Nothing that writes files is offered before the operation queue.
        assert!(!labels(&mut desktop, &[&archive]).contains(&"Compress…".to_owned()));
        let service = crate::operation_jobs::OperationJobs::start(
            state.path().join("history.sqlite3"),
            state.path().join("volumes.json"),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while service.snapshot().initializing || service.storage().is_none() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        desktop.file_operations.storage = service.storage();
        desktop.file_operations.snapshot = service.snapshot();
        desktop.file_operations.service = Some(service.clone());

        let items = labels(&mut desktop, &[&archive]);
        let position = |label: &str| items.iter().position(|item| item == label);
        // Their own group, just before Rename.
        let rename = position("Rename…").expect("Rename");
        assert_eq!(
            (
                position("Extract here"),
                position("Extract to…"),
                position("Compress…")
            ),
            (Some(rename - 3), Some(rename - 2), Some(rename - 1)),
            "{items:?}"
        );
        // A folder named like an archive, or a mixed selection, only compresses.
        for selection in [&[&folder][..], &[&archive, &text][..]] {
            let items = labels(&mut desktop, selection);
            assert!(items.contains(&"Compress…".to_owned()), "{items:?}");
            assert!(!items.contains(&"Extract here".to_owned()), "{items:?}");
        }
        drop(desktop);
        iced::futures::executor::block_on(service.shutdown().unwrap())
            .unwrap()
            .unwrap();
    }

    #[test]
    fn terminal_new_document_and_bulk_rename_items_appear_where_they_apply() {
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let folder = root.path().join("folder");
        let first = root.path().join("first.txt");
        let second = root.path().join("second.txt");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(&first, b"x").unwrap();
        std::fs::write(&second, b"x").unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let tab = desktop.active(window).unwrap().id;
        super::super::tests::loaded(&mut desktop, window, tab, 1, root.path());
        let service = crate::operation_jobs::OperationJobs::start(
            state.path().join("history.sqlite3"),
            state.path().join("volumes.json"),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while service.snapshot().initializing || service.storage().is_none() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        desktop.file_operations.storage = service.storage();
        desktop.file_operations.snapshot = service.snapshot();
        desktop.file_operations.service = Some(service.clone());
        let labels = |desktop: &mut Desktop, selection: &[&PathBuf]| {
            desktop
                .active_mut(window)
                .unwrap()
                .browser
                .as_mut()
                .unwrap()
                .set_selection(selection.iter().map(|path| path.to_path_buf()));
            let source = desktop.action_source(window).unwrap();
            desktop
                .selection_items(&source)
                .into_iter()
                .map(|(label, _)| label)
                .collect::<Vec<_>>()
        };
        let folder_menu = labels(&mut desktop, &[&folder]);
        assert!(
            folder_menu.contains(&"Open in terminal".to_owned()),
            "{folder_menu:?}"
        );
        let file_menu = labels(&mut desktop, &[&first]);
        assert!(
            !file_menu.contains(&"Open in terminal".to_owned()),
            "{file_menu:?}"
        );
        let several = labels(&mut desktop, &[&first, &second]);
        assert!(several.contains(&"Rename…".to_owned()), "{several:?}");
        let background: Vec<_> = desktop
            .folder_items()
            .into_iter()
            .map(|(label, _)| label)
            .collect();
        for label in ["New folder…", "New document", "Open in terminal"] {
            assert!(background.contains(&label.to_owned()), "{background:?}");
        }
        let application: Vec<_> = desktop
            .application_items(window)
            .into_iter()
            .map(|(label, _)| label)
            .collect();
        assert!(
            application.contains(&"New document".to_owned()),
            "{application:?}"
        );
        assert!(
            application.contains(&"Open in terminal".to_owned()),
            "{application:?}"
        );
        drop(desktop);
        iced::futures::executor::block_on(service.shutdown().unwrap())
            .unwrap()
            .unwrap();
    }

    #[test]
    fn new_document_opens_beside_its_row_and_returns_to_its_menu() {
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let tab = desktop.active(window).unwrap().id;
        super::super::tests::loaded(&mut desktop, window, tab, 1, root.path());
        let service = crate::operation_jobs::OperationJobs::start(
            state.path().join("history.sqlite3"),
            state.path().join("volumes.json"),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while service.snapshot().initializing || service.storage().is_none() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        desktop.file_operations.storage = service.storage();
        desktop.file_operations.snapshot = service.snapshot();
        desktop.file_operations.service = Some(service.clone());
        if let Some(browser) = desktop.windows.get_mut(&window) {
            browser.width = 1200.0;
            browser.height = 800.0;
        }
        // A secondary click on the folder's background.
        let _ = desktop.action_event(Event::Context(window));
        desktop.anchor_action_menu(window, Some(iced::Point::new(300.0, 200.0)));
        let menu = desktop.action_menu.as_ref().unwrap();
        let source = menu.source.clone();
        let row = menu
            .items
            .iter()
            .position(|(_, item)| matches!(item, Item::NewDocument))
            .expect("New document");
        let _ = desktop.action_event(Event::MenuItem(source.clone(), row));
        // The menu waits, at its place, while the templates are read.
        let menu = desktop.action_menu.as_ref().expect("the menu stays open");
        assert_eq!((menu.focused, menu.parent.is_none()), (row, true));
        let _ = desktop.action_event(Event::TemplatesReady(
            source.clone(),
            Arc::new((Vec::new(), None)),
        ));
        let submenu = desktop.action_menu.as_ref().expect("a submenu");
        let parent = submenu.parent.as_deref().expect("its menu stays behind it");
        assert_eq!(submenu.items[0].0, "Empty document");
        let width = menu_width(&desktop, &parent.items, false);
        let (left, top) = beside(&desktop, window, parent, width, submenu);
        // After the menu's trailing edge, level with the New document row.
        assert_eq!(left, 300.0 + width - 4.0);
        let size = desktop.configuration.appearance.typography().body_size.px();
        let row_height = (size * 1.3).ceil() + MENU_ROW_PADDING[0] * 2.0 + 1.0;
        assert!(top >= 200.0 + row as f32 * row_height, "{top}");
        assert!(top < 200.0 + (row + 1) as f32 * row_height + 2.0 * MENU_SEPARATOR);
        // Near the right edge it opens before the menu instead.
        desktop.anchor_action_menu(window, None);
        let submenu = desktop.action_menu.as_mut().unwrap();
        submenu.parent.as_mut().unwrap().anchor =
            Some(iced::Point::new(1200.0 - width - 10.0, 200.0));
        let submenu = desktop.action_menu.as_ref().unwrap();
        let parent = submenu.parent.as_deref().unwrap();
        let (left, _) = beside(&desktop, window, parent, width, submenu);
        assert!(
            left + menu_width(&desktop, &submenu.items, false) <= 1200.0 - width,
            "{left}"
        );
        // Escape returns to the first menu, at its row.
        let _ = desktop.menu_key(
            window,
            keyboard::Key::Named(keyboard::key::Named::Escape),
            keyboard::Modifiers::default(),
        );
        let menu = desktop.action_menu.as_ref().expect("the first menu");
        assert!(menu.parent.is_none());
        assert!(matches!(menu.items[menu.focused].1, Item::NewDocument));
        // A result for a dismissed menu opens nothing.
        desktop.action_menu = None;
        let _ = desktop.action_event(Event::TemplatesReady(source, Arc::new((Vec::new(), None))));
        assert!(desktop.action_menu.is_none());
        drop(desktop);
        iced::futures::executor::block_on(service.shutdown().unwrap())
            .unwrap()
            .unwrap();
    }

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
