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
    Row(RowTarget),
    Close(window::Id),
    Builtin(Arc<Source>, Command),
    Invoke(Arc<Source>, String),
    MenuItem(Arc<Source>, usize),
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
                    items.push(("Open".into(), Item::Command(Command::Open)));
                    items.push(("Quick Look".into(), Item::Command(Command::QuickLook)));
                }
                if self.file_clipboard.ready()
                    && self.file_operations.storage.is_some()
                    && self.file_operations.storage_error.is_none()
                    && !source.paths.is_empty()
                {
                    items.push(("Copy".into(), Item::Command(Command::Copy)));
                    items.push(("Cut".into(), Item::Command(Command::Cut)));
                }
                for action in &self.configuration.actions {
                    if self.action_applicable(&source, action) {
                        items.push((action.name.clone(), Item::Action(action.id.clone())));
                    }
                }
                if self.file_operations.ready() {
                    if self.file_clipboard.can_paste() {
                        items.push(("Paste".into(), Item::Command(Command::Paste)));
                    }
                    items.push(("New folder…".into(), Item::Command(Command::NewFolder)));
                    if source.paths.len() == 1 {
                        items.push(("Rename…".into(), Item::Command(Command::Rename)));
                    }
                    if !source.paths.is_empty() {
                        items.push(("Move to Trash".into(), Item::Command(Command::Trash)));
                    }
                    if !self.file_operations.has_active()
                        && let Some(undo) = &self.file_operations.snapshot.undo
                    {
                        items.push((undo.label.clone(), Item::Command(Command::Undo)));
                    }
                }
                items.push((
                    "Edit custom actions…".into(),
                    Item::Command(Command::CustomActions),
                ));
                items.push(("Operations".into(), Item::Command(Command::Operations)));
                items.push(("Open Trash".into(), Item::Command(Command::ShowTrash)));
                items.push(("Preferences…".into(), Item::Command(Command::Preferences)));
                items.push((
                    "Bookmark this folder…".into(),
                    Item::Command(Command::BookmarkFolder),
                ));
                items.push(("Locations…".into(), Item::Locations));
                items.push(("View options…".into(), Item::FolderOptions));
                self.action_menu = Some(Menu {
                    source,
                    items,
                    focused: 0,
                });
                if let Some(tab) = self.active_mut(id) {
                    tab.location = None;
                }
                use iced::advanced::widget::{operate, operation};
                if let Some(tab) = self.active(id) {
                    return operate(operation::scope(
                        view::root_id(id, tab.id),
                        operation::focusable::unfocus(),
                    ));
                }
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
                    let columns = tab.column_paths(
                        self.windows[&target.window].width,
                        self.configuration.settings.sidebar,
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
                        tab.error =
                            Some(format!("{} does not apply to this selection.", action.name));
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
                    Item::Locations => {
                        let mut items = vec![("Back".into(), Item::Back)];
                        for place in &self.configuration.settings.places {
                            if let Some(path) = self.place_path(place) {
                                items.push((place.label().to_owned(), Item::Place(path)));
                            }
                        }
                        if self.devices.catalog.status == crate::local_volumes::DeviceStatus::Ready
                        {
                            for device in &self.devices.catalog.devices {
                                if device.root.is_some() || device.can_mount {
                                    items.push((
                                        format!(
                                            "{}{}",
                                            if device.root.is_some() {
                                                "Open "
                                            } else {
                                                "Mount "
                                            },
                                            device.name
                                        ),
                                        Item::Device(device.token, false),
                                    ));
                                }
                                if let Some(kind) = device.removal {
                                    items.push((
                                        format!("{} {}", kind.label(), device.name),
                                        Item::Device(device.token, true),
                                    ));
                                }
                            }
                        }
                        self.action_menu = Some(Menu {
                            source,
                            items,
                            focused: 0,
                        });
                        Task::none()
                    }
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
                    Item::FolderOptions => {
                        self.folder_view_event(folder_views::Event::Show(source.window))
                    }
                    Item::Action(id) => self.action_event(Event::Invoke(source, id)),
                };
            }
        }
        Task::none()
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
            | Item::FolderOptions
            | Item::Locations
            | Item::Back
            | Item::Device(..)
            | Item::Place(_) => None,
        };
        items = items.push(
            canopa_ui::styled_action(
                desktop.icons.label(icon, label, size, font, true),
                Some(Message::Action(Event::MenuItem(menu.source.clone(), index))),
                canopa_ui::Action::Quiet,
                index == menu.focused,
                appearance,
            )
            .width(Length::Fill),
        );
    }
    let appearance = appearance.clone();
    let menu = container(scrollable(items).id(menu_id(id)).height(Length::Shrink))
        .padding(8)
        .max_width(320)
        .max_height(360)
        .width(Length::Fill)
        .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Menu)));
    layers
        .push(
            mouse_area(space().width(Length::Fill).height(Length::Fill))
                .on_press(Message::Action(Event::Close(id)))
                .on_right_press(Message::Action(Event::Close(id))),
        )
        .push(
            container(opaque(menu))
                .padding([74, 10])
                .width(Length::Fill)
                .height(Length::Fill)
                .align_right(Length::Fill)
                .align_top(Length::Fill),
        )
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
