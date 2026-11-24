//! Iced adapter. Directory reads and acceptance checks run on filesystem workers.
#[cfg(all(test, feature = "accessibility-testing"))]
mod accessibility_tests;
use canopa_files::picker::{filters, folders};
mod presentation;
#[cfg(test)]
mod tests;

use crate::accessibility_ui::{self as a11y, Node, semantic};
use crate::dialog_ui;
use crate::file_chooser::{ChooserRequest, Mode, Selection, path_bytes};
use crate::worker::run as worker;
use canopa_files::{
    browse::{Browser, Directory, Entry, HistoryDirection, SelectionMode},
    browser_ui,
    interface::PlaceKind,
};
use canopa_theme::{Appearance, ColorRole, UiKind};
use iced::{
    Element, Event, Length, Subscription, Task, keyboard,
    widget::{checkbox, column, container, pick_list, row, rule, space, text},
    window,
};
const FILES_KEY: &str = "chooser-files";

use std::{
    collections::BTreeSet,
    ffi::OsString,
    io,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    sync::Arc,
};

pub(crate) fn prepare_picker(
    request: ChooserRequest,
    appearance: Appearance,
) -> anyhow::Result<PickerStart> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from("/"));
    let config_home = canopa_config::config_home_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        Some(home.as_os_str()),
    )?;
    let places = canopa_files::personal_folders::PersonalFolders::load(&home, &config_home)?;
    let shortcuts = [
        (
            PlaceKind::Home,
            fl!("portal-place-home"),
            Some(home.as_path()),
        ),
        (
            PlaceKind::Desktop,
            fl!("portal-place-desktop"),
            places.path(PlaceKind::Desktop),
        ),
        (
            PlaceKind::Documents,
            fl!("portal-place-documents"),
            places.path(PlaceKind::Documents),
        ),
        (
            PlaceKind::Downloads,
            fl!("portal-place-downloads"),
            places.path(PlaceKind::Downloads),
        ),
        (
            PlaceKind::Music,
            fl!("portal-place-music"),
            places.path(PlaceKind::Music),
        ),
        (
            PlaceKind::Pictures,
            fl!("portal-place-pictures"),
            places.path(PlaceKind::Pictures),
        ),
        (
            PlaceKind::Videos,
            fl!("portal-place-videos"),
            places.path(PlaceKind::Videos),
        ),
        (
            PlaceKind::Filesystem,
            fl!("portal-place-filesystem"),
            Some(Path::new("/")),
        ),
    ]
    .into_iter()
    .filter_map(|(kind, name, path)| Some((kind, name, path?.to_owned())))
    .collect();
    let visuals = presentation::Visuals::new(appearance, &request.app_id);
    let folder = request
        .current_file
        .as_deref()
        .and_then(path_bytes)
        .and_then(|p| p.parent().map(Path::to_owned))
        .or_else(|| request.current_folder.as_deref().and_then(path_bytes))
        .unwrap_or(home.clone());
    let original_name = request
        .current_file
        .as_deref()
        .and_then(path_bytes)
        .and_then(|p| p.file_name().map(OsStrExt::as_bytes).map(Vec::from));
    let name = original_name
        .as_ref()
        .map(|n| OsString::from_vec(n.clone()).to_string_lossy().into_owned())
        .or_else(|| request.current_name.clone())
        .unwrap_or_default();
    Ok(PickerStart {
        request,
        visuals,
        home,
        shortcuts,
        folder,
        name,
        original_name,
        mime: Arc::new(filters::MimeGlobs::load()),
    })
}

#[derive(Clone)]
pub(crate) struct PickerStart {
    request: ChooserRequest,
    visuals: presentation::Visuals,
    home: PathBuf,
    shortcuts: Vec<(PlaceKind, String, PathBuf)>,
    folder: PathBuf,
    name: String,
    original_name: Option<Vec<u8>>,
    mime: Arc<filters::MimeGlobs>,
}

#[cfg(test)]
impl PickerStart {
    pub(crate) fn fixture(request: ChooserRequest, folder: PathBuf) -> Self {
        Self {
            request,
            visuals: presentation::Visuals::new(Appearance::default(), ""),
            home: folder.clone(),
            shortcuts: Vec::new(),
            folder,
            name: String::new(),
            original_name: None,
            mime: Arc::default(),
        }
    }
}

impl std::fmt::Debug for PickerStart {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PickerStart")
            .finish_non_exhaustive()
    }
}

pub(crate) struct PickerOptions {
    pub read_only_choice: bool,
    /// Embedded Print retains the objects inspected before replacement review.
    /// Ordinary FileChooser continues to return paths for the frontend to grant.
    pub retain_file_target: bool,
}

impl Default for PickerOptions {
    fn default() -> Self {
        Self {
            read_only_choice: true,
            retain_file_target: false,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Outcome {
    Selected(Selection),
    Cancelled,
}

pub(crate) struct Picker {
    request: ChooserRequest,
    visuals: presentation::Visuals,
    browser: Option<Browser>,
    parent: Option<Directory>,
    history_move: Option<HistoryDirection>,
    shortcuts: Vec<(PlaceKind, String, PathBuf)>,
    home: PathBuf,
    location: String,
    editing_location: bool,
    name: String,
    original_name: Option<Vec<u8>>,
    revision: u64,
    loading: bool,
    busy: bool,
    error: Option<String>,
    modifiers: keyboard::Modifiers,
    outcome: Option<Outcome>,
    finished: bool,
    read_only_choice: bool,
    retain_file_target: bool,
    file_target: Option<Arc<crate::print::file_target::Reviewed>>,
    mime: Arc<filters::MimeGlobs>,
    search: String,
    filtering: bool,
    overwrite: Option<Prepared>,
    new_folder: Option<FolderDraft>,
    scan: canopa_files::Cancellation,
    writable: bool,
    last_click: Option<(PathBuf, std::time::Instant)>,
    hovered: Option<PathBuf>,
    /// Keyboard input drives the file list: no field has been typed into or
    /// focused since the last arrow key or row click. The cursor row then
    /// holds accessibility focus.
    list_focus: bool,
}

struct FolderDraft {
    parent: Arc<folders::Parent>,
    name: String,
}

impl Drop for Picker {
    fn drop(&mut self) {
        self.scan.cancel();
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Message {
    Navigate(PathBuf),
    History(HistoryDirection),
    Up,
    EditLocation,
    Location(String),
    Go,
    Name(String),
    ShowNameFilter,
    CloseNameFilter,
    Search(String),
    NewFolder,
    FolderParent(u64, Result<Arc<folders::Parent>, String>),
    FolderName(String),
    CreateFolder,
    FolderCreated(u64, Result<PathBuf, String>),
    Loaded(u64, bool, Result<Directory, String>),
    ParentLoaded(u64, Option<Directory>),
    Select(PathBuf),
    Hover(PathBuf, bool),
    Activate(PathBuf),
    Event(Event),
    Hidden(bool),
    Filter(usize),
    Choice(usize, String),
    ReadOnly(bool),
    Accept,
    Checked(Result<Prepared, String>),
    Replace,
    KeepChoosing,
    Cancel,
    #[cfg(all(test, feature = "accessibility-testing"))]
    Accessibility(canopa_ui::accessibility::Event<Message>),
}

#[derive(Clone, Debug)]
pub(crate) struct Prepared {
    paths: Vec<PathBuf>,
    existing: Vec<PathBuf>,
    file_target: Option<Arc<crate::print::file_target::Reviewed>>,
}

impl Picker {
    pub(crate) fn new(start: PickerStart, options: PickerOptions) -> (Self, Task<Message>) {
        let mut picker = Self {
            request: start.request,
            visuals: start.visuals,
            browser: None,
            parent: None,
            history_move: None,
            shortcuts: start.shortcuts,
            home: start.home,
            location: start.folder.to_string_lossy().into_owned(),
            editing_location: false,
            name: start.name,
            original_name: start.original_name,
            revision: 0,
            loading: false,
            busy: false,
            error: None,
            modifiers: keyboard::Modifiers::default(),
            outcome: None,
            finished: false,
            read_only_choice: options.read_only_choice,
            retain_file_target: options.retain_file_target,
            file_target: None,
            mime: start.mime,
            search: String::new(),
            filtering: false,
            overwrite: None,
            new_folder: None,
            writable: true,
            last_click: None,
            hovered: None,
            list_focus: true,
            scan: canopa_files::Cancellation::default(),
        };
        let load = picker.navigate(start.folder, true);
        (picker, load)
    }

    pub(crate) fn take_outcome(&mut self) -> Option<Outcome> {
        self.outcome.take()
    }

    pub(crate) fn subscription(&self) -> Subscription<Message> {
        iced::event::listen_with(|event, status, _| {
            let shortcut = matches!(
                &event,
                Event::Keyboard(keyboard::Event::ModifiersChanged(_))
            ) || matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            ) || matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(key), modifiers, ..
                }) if modifiers.control() && modifiers.shift() && key.eq_ignore_ascii_case("n")
            );
            (shortcut || status == iced::event::Status::Ignored).then_some(Message::Event(event))
        })
    }

    fn navigate(&mut self, path: PathBuf, initial: bool) -> Task<Message> {
        self.scan.cancel();
        self.scan = canopa_files::Cancellation::default();
        let scan = self.scan.clone();
        self.revision += 1;
        let revision = self.revision;
        self.loading = true;
        self.history_move = None;
        self.error = None;
        self.overwrite = None;
        self.new_folder = None;
        self.search.clear();
        self.filtering = false;
        self.last_click = None;
        self.hovered = None;
        Task::perform(
            worker(move || Directory::read_cancellable(path, &scan).map_err(|e| e.to_string())),
            move |result| Message::Loaded(revision, initial, result.and_then(|r| r)),
        )
    }

    fn entries(&self) -> Vec<&canopa_files::browse::Entry> {
        self.browser.as_ref().map_or_else(Vec::new, |browser| {
            browser
                .visible_entries()
                .into_iter()
                .filter(|e| {
                    (e.is_directory()
                        || (!self.request.directory
                            && self
                                .request
                                .current_filter
                                .as_ref()
                                .is_none_or(|f| self.mime.matches(f, &e.path))))
                        && (self.search.is_empty()
                            || e.name()
                                .to_lowercase()
                                .contains(&self.search.to_lowercase()))
                })
                .collect()
        })
    }

    pub(crate) fn take_file_target(&mut self) -> Option<Arc<crate::print::file_target::Reviewed>> {
        self.file_target.take()
    }

    fn finish(&mut self, prepared: Prepared) -> Task<Message> {
        self.file_target = prepared.file_target;
        let selection = Selection {
            paths: prepared
                .paths
                .into_iter()
                .map(|p| p.into_os_string().into_vec())
                .collect(),
            choices: self
                .request
                .choices
                .iter()
                .map(|(id, _, _, value)| (id.clone(), value.clone()))
                .collect(),
            current_filter: self.request.current_filter.clone(),
            writable: self.writable,
        };
        self.scan.cancel();
        self.finished = true;
        self.outcome = Some(Outcome::Selected(selection));
        Task::none()
    }

    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        if self.finished {
            return Task::none();
        }
        if self.overwrite.is_some()
            && !matches!(
                message,
                Message::Replace | Message::KeepChoosing | Message::Cancel | Message::Event(_)
            )
        {
            return Task::none();
        }
        if self.busy
            && !matches!(
                message,
                Message::Checked(_)
                    | Message::FolderParent(..)
                    | Message::FolderCreated(..)
                    | Message::Cancel
                    | Message::Event(_)
            )
        {
            return Task::none();
        }
        if self.new_folder.is_some()
            && !matches!(
                message,
                Message::FolderName(_)
                    | Message::CreateFolder
                    | Message::FolderCreated(..)
                    | Message::KeepChoosing
                    | Message::Cancel
                    | Message::Event(_)
            )
        {
            return Task::none();
        }
        match message {
            #[cfg(all(test, feature = "accessibility-testing"))]
            Message::Accessibility(event) => {
                if matches!(event, canopa_ui::accessibility::Event::Focus(_)) {
                    self.list_focus = false;
                }
            }
            Message::Cancel => {
                self.scan.cancel();
                self.finished = true;
                self.outcome = Some(Outcome::Cancelled);
                return Task::none();
            }
            Message::Navigate(path) => return self.navigate(path, false),
            Message::History(direction) => {
                if let Some(path) = self
                    .browser
                    .as_ref()
                    .and_then(|browser| browser.history_target(direction))
                    .map(Path::to_owned)
                {
                    let load = self.navigate(path, false);
                    self.history_move = Some(direction);
                    return load;
                }
            }
            Message::EditLocation => {
                self.editing_location = true;
                self.list_focus = false;
                return iced::widget::operation::focus("location");
            }
            Message::Up => {
                if let Some(path) = self
                    .browser
                    .as_ref()
                    .and_then(|b| b.parent())
                    .map(Path::to_owned)
                {
                    return self.navigate(path, false);
                }
            }
            Message::Location(value) => {
                self.editing_location = true;
                self.location = value;
                self.list_focus = false;
            }
            Message::Go => {
                let path = self
                    .browser
                    .as_ref()
                    .filter(|b| b.path().to_string_lossy() == self.location)
                    .map_or_else(|| PathBuf::from(&self.location), |b| b.path().to_owned());
                let path = if path.is_absolute() {
                    path
                } else {
                    self.browser
                        .as_ref()
                        .map_or_else(|| self.home.join(&path), |b| b.path().join(&path))
                };
                return self.navigate(path, false);
            }
            Message::Name(value) => {
                self.list_focus = false;
                self.name = value;
                self.original_name = None;
                self.overwrite = None;
            }
            Message::ShowNameFilter => {
                self.filtering = true;
                self.editing_location = false;
                if let Some(browser) = &self.browser {
                    self.location = browser.path().to_string_lossy().into_owned();
                }
                self.list_focus = false;
                return iced::widget::operation::focus("search");
            }
            Message::CloseNameFilter => {
                self.filtering = false;
                self.search.clear();
            }
            Message::Search(value) => {
                self.list_focus = false;
                self.search = value;
                if let Some(b) = self.browser.as_mut() {
                    b.clear_selection();
                }
            }
            Message::NewFolder => {
                if self.loading || self.request.mode == Mode::Open {
                    return Task::none();
                }
                let Some(browser) = &self.browser else {
                    return Task::none();
                };
                let path = browser.path().to_owned();
                let revision = self.revision;
                self.busy = true;
                self.error = None;
                return Task::perform(
                    worker(move || {
                        folders::Parent::open(path)
                            .map(Arc::new)
                            .map_err(|e| e.to_string())
                    }),
                    move |result| Message::FolderParent(revision, result.and_then(|r| r)),
                );
            }
            Message::FolderParent(revision, result) => {
                if revision != self.revision {
                    return Task::none();
                }
                self.busy = false;
                match result {
                    Ok(parent) => {
                        self.new_folder = Some(FolderDraft {
                            parent,
                            name: String::new(),
                        });
                        return iced::widget::operation::focus("new-folder-name");
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            Message::FolderName(value) => {
                if let Some(draft) = self.new_folder.as_mut() {
                    draft.name = value;
                    self.error = None;
                }
            }
            Message::CreateFolder => {
                let Some(draft) = &self.new_folder else {
                    return Task::none();
                };
                if !folders::valid_name(&draft.name) {
                    self.error = Some(fl!("portal-chooser-folder-name-rule"));
                    return Task::none();
                }
                let (parent, name) = (draft.parent.clone(), draft.name.clone());
                let revision = self.revision;
                self.busy = true;
                self.error = None;
                return Task::perform(
                    worker(move || parent.create(&name).map_err(|e| e.to_string())),
                    move |result| Message::FolderCreated(revision, result.and_then(|r| r)),
                );
            }
            Message::FolderCreated(revision, result) => {
                if revision != self.revision {
                    return Task::none();
                }
                self.busy = false;
                match result {
                    Ok(path) => return self.navigate(path, false),
                    Err(error) => self.error = Some(error),
                }
            }
            Message::Loaded(revision, initial, result) => {
                if revision != self.revision {
                    return Task::none();
                }
                self.loading = false;
                match result {
                    Ok(directory) => {
                        self.location = directory.path.to_string_lossy().into_owned();
                        self.editing_location = false;
                        let parent = directory.path.parent().map(Path::to_owned);
                        self.parent = None;
                        if let Some(browser) = self.browser.as_mut() {
                            if let Some(direction) = self.history_move.take() {
                                browser.visit_history(directory, direction);
                            } else {
                                browser.visit(directory);
                            }
                            browser.clear_selection();
                        } else {
                            let mut browser = Browser::new(directory);
                            if initial
                                && let Some(path) =
                                    self.request.current_file.as_deref().and_then(path_bytes)
                            {
                                browser.reveal_selection(vec![path]);
                            }
                            self.browser = Some(browser);
                        }
                        if let Some(parent) = parent {
                            let cancel = self.scan.clone();
                            return Task::perform(
                                worker(move || Directory::read_cancellable(parent, &cancel).ok()),
                                move |result| {
                                    Message::ParentLoaded(revision, result.ok().flatten())
                                },
                            );
                        }
                    }
                    Err(error) => {
                        self.error = Some(error);
                        if initial {
                            return self.navigate(self.home.clone(), false);
                        }
                    }
                }
            }
            Message::ParentLoaded(revision, directory) if revision == self.revision => {
                self.parent = directory;
            }
            Message::ParentLoaded(..) => {}
            Message::Hover(path, inside) => {
                if inside {
                    self.hovered = Some(path);
                } else if self.hovered.as_ref() == Some(&path) {
                    self.hovered = None;
                }
            }
            Message::Select(path) => {
                let visible: BTreeSet<_> = self
                    .entries()
                    .into_iter()
                    .map(|entry| entry.path.clone())
                    .collect();
                let now = std::time::Instant::now();
                let double = self.last_click.as_ref().is_some_and(|(previous, at)| {
                    previous == &path && now.duration_since(*at).as_millis() < 400
                });
                self.last_click = Some((path.clone(), now));
                self.list_focus = true;
                let directory = self
                    .browser
                    .as_ref()
                    .and_then(|b| b.directory().entries.iter().find(|e| e.path == path))
                    .is_some_and(|e| e.is_directory());
                if let Some(browser) = self.browser.as_mut() {
                    let mode = if self.request.multiple && self.modifiers.control() {
                        SelectionMode::Toggle
                    } else if self.request.multiple && self.modifiers.shift() {
                        SelectionMode::Extend
                    } else {
                        SelectionMode::Replace
                    };
                    browser.select(&path, mode);
                    let selected = browser
                        .selected()
                        .intersection(&visible)
                        .cloned()
                        .collect::<Vec<_>>();
                    browser.set_selection(selected);
                }
                if self.request.mode == Mode::Save && !directory {
                    self.name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    self.original_name = path.file_name().map(|n| n.as_bytes().to_vec());
                }
                self.overwrite = None;
                if double {
                    return self.update(Message::Activate(path));
                }
            }
            Message::Activate(path) => {
                if self
                    .browser
                    .as_ref()
                    .and_then(|b| b.directory().entries.iter().find(|e| e.path == path))
                    .is_some_and(|e| e.is_directory())
                {
                    return self.navigate(path, false);
                }
                if let Some(browser) = self.browser.as_mut()
                    && !browser.selected().contains(&path)
                {
                    browser.select(&path, SelectionMode::Replace);
                }
                if self.request.mode == Mode::Save {
                    self.name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    self.original_name = path.file_name().map(|name| name.as_bytes().to_vec());
                }
                return self.update(Message::Accept);
            }
            Message::Hidden(value) => {
                if let Some(b) = self.browser.as_mut() {
                    b.set_show_hidden(value);
                }
            }
            Message::Filter(index) => {
                self.request.current_filter = self.request.filters.get(index).cloned();
                if let Some(b) = self.browser.as_mut() {
                    b.clear_selection();
                }
            }
            Message::Choice(index, value) => {
                if let Some(choice) = self.request.choices.get_mut(index) {
                    choice.3 = value;
                }
            }
            Message::ReadOnly(value) => self.writable = !value,
            Message::Accept => {
                if self.loading || self.busy {
                    return Task::none();
                }
                let Some(browser) = &self.browser else {
                    return Task::none();
                };
                let selected: Vec<_> = browser.selected().iter().cloned().collect();
                if self.request.mode != Mode::SaveMany
                    && !self.request.directory
                    && selected.len() == 1
                    && browser
                        .directory()
                        .entries
                        .iter()
                        .any(|e| e.path == selected[0] && e.is_directory())
                {
                    return self.navigate(selected[0].clone(), false);
                }
                let name = self
                    .original_name
                    .clone()
                    .map(OsString::from_vec)
                    .unwrap_or_else(|| OsString::from(&self.name));
                let request = self.request.clone();
                let folder = browser.path().to_owned();
                let retain = self.retain_file_target;
                self.busy = true;
                self.error = None;
                return Task::perform(
                    worker(move || {
                        prepare_with_target(&request, &folder, selected, name, retain)
                            .map_err(|e| e.to_string())
                    }),
                    |result| Message::Checked(result.and_then(|r| r)),
                );
            }
            Message::Checked(result) => {
                self.busy = false;
                match result {
                    Ok(prepared) if prepared.existing.is_empty() => {
                        return self.finish(prepared);
                    }
                    Ok(prepared) => self.overwrite = Some(prepared),
                    Err(error) => self.error = Some(error),
                }
            }
            Message::Replace => {
                if let Some(prepared) = self.overwrite.take() {
                    return self.finish(prepared);
                }
            }
            Message::KeepChoosing => {
                self.overwrite = None;
                self.new_folder = None;
                self.error = None;
            }
            Message::Event(Event::Window(window::Event::CloseRequested)) => {
                return self.update(Message::Cancel);
            }
            Message::Event(Event::Keyboard(keyboard::Event::ModifiersChanged(value))) => {
                self.modifiers = value
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => {
                use keyboard::{Key, key::Named};
                if key == Key::Named(Named::Escape) {
                    if self.busy && self.new_folder.is_some() {
                        return Task::none();
                    }
                    if self.editing_location
                        && self.overwrite.is_none()
                        && self.new_folder.is_none()
                    {
                        if let Some(browser) = &self.browser {
                            self.location = browser.path().to_string_lossy().into_owned();
                        }
                        self.editing_location = false;
                        return Task::none();
                    }
                    if self.filtering && self.overwrite.is_none() && self.new_folder.is_none() {
                        return self.update(Message::CloseNameFilter);
                    }
                    return self.update(if self.overwrite.is_some() || self.new_folder.is_some() {
                        Message::KeepChoosing
                    } else {
                        Message::Cancel
                    });
                }
                if self.busy {
                    return Task::none();
                }
                if self.overwrite.is_some() {
                    return Task::none();
                }
                if self.new_folder.is_some() {
                    return if key == Key::Named(Named::Enter) {
                        self.update(Message::CreateFolder)
                    } else {
                        Task::none()
                    };
                }
                if modifiers.control() {
                    match key.as_ref() {
                        Key::Character(key)
                            if modifiers.shift() && key.eq_ignore_ascii_case("n") =>
                        {
                            return self.update(Message::NewFolder);
                        }
                        Key::Character("l") => {
                            return self.update(Message::EditLocation);
                        }
                        Key::Character("f") => return self.update(Message::ShowNameFilter),
                        Key::Character("h") => {
                            return self.update(Message::Hidden(
                                !self.browser.as_ref().is_some_and(Browser::show_hidden),
                            ));
                        }
                        Key::Character("a") if self.request.multiple => {
                            let paths = self
                                .entries()
                                .into_iter()
                                .filter(|e| e.is_directory() == self.request.directory)
                                .map(|e| e.path.clone())
                                .collect::<Vec<_>>();
                            if let Some(b) = self.browser.as_mut() {
                                b.set_selection(paths);
                            }
                        }
                        _ => {}
                    }
                } else if modifiers.alt() {
                    return match key.as_ref() {
                        Key::Named(Named::ArrowUp) => self.update(Message::Up),
                        Key::Named(Named::ArrowLeft) => {
                            self.update(Message::History(HistoryDirection::Back))
                        }
                        Key::Named(Named::ArrowRight) => {
                            self.update(Message::History(HistoryDirection::Forward))
                        }
                        _ => Task::none(),
                    };
                } else if key == Key::Named(Named::Enter) {
                    if let Some(path) = self
                        .browser
                        .as_ref()
                        .and_then(|b| b.focused())
                        .map(Path::to_owned)
                    {
                        return self.update(Message::Activate(path));
                    }
                    return self.update(Message::Accept);
                } else if matches!(key, Key::Named(Named::ArrowUp | Named::ArrowDown)) {
                    let paths: Vec<_> =
                        self.entries().into_iter().map(|e| e.path.clone()).collect();
                    if !paths.is_empty() {
                        let current = self
                            .browser
                            .as_ref()
                            .and_then(|b| b.focused())
                            .and_then(|path| paths.iter().position(|p| p == path));
                        let index = match current {
                            Some(i) if key == Key::Named(Named::ArrowUp) => i.saturating_sub(1),
                            Some(i) => (i + 1).min(paths.len() - 1),
                            None => 0,
                        };
                        self.last_click = None;
                        return self.update(Message::Select(paths[index].clone()));
                    }
                }
            }
            Message::Event(_) => {}
        }
        Task::none()
    }

    /// The standalone dialog's title, also its AT-SPI window name.
    #[cfg(all(test, feature = "accessibility-testing"))]
    fn window_title(&self) -> String {
        fl!("portal-window-title", title = self.request.title.as_str())
    }

    /// The key of the row holding accessibility focus, if keyboard input
    /// drives the list.
    fn focused_row(&self, entries: &[&canopa_files::browse::Entry]) -> Option<String> {
        let cursor = self.browser.as_ref()?.focused()?;
        (self.list_focus && entries.iter().any(|entry| entry.path == cursor))
            .then(|| entry_key(cursor))
    }

    pub(crate) fn view(&self) -> Element<'_, Message> {
        self.picker_view(true)
    }

    fn replaces(&self, entry: &Entry) -> bool {
        self.request.mode == Mode::Save
            && !entry.is_directory()
            && entry.path.file_name().is_some_and(|name| {
                name.as_bytes()
                    == self
                        .original_name
                        .as_deref()
                        .unwrap_or(self.name.as_bytes())
            })
    }

    fn picker_view(&self, embedded: bool) -> Element<'_, Message> {
        if let Some(draft) = &self.new_folder {
            return self.new_folder_view(draft);
        }
        if let Some(prepared) = &self.overwrite {
            return overwrite_view(prepared);
        }
        let enabled = !self.loading && !self.busy && self.overwrite.is_none();
        let appearance = &self.visuals.appearance;
        let entries = self.entries();
        let focused_row = self.focused_row(&entries);
        let total = entries.len();
        let status = if self.loading {
            fl!("portal-chooser-loading-folder")
        } else if self.browser.is_none() {
            String::new()
        } else if entries.is_empty() {
            fl!("portal-chooser-no-matches")
        } else {
            fl!("portal-chooser-item-count", count = total)
        };
        let mut header = column![].spacing(3);
        if embedded {
            header = header.push(semantic(
                Node::heading(&self.request.title, 1),
                dialog_ui::heading(&self.request.title).font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..canopa_ui::font(appearance)
                }),
            ));
        }
        let toolbar = self.toolbar(enabled, focused_row.clone());
        let body = self.browser_panes(enabled, focused_row.clone(), status.clone());
        let mut request_options = column![].spacing(8);
        let will_replace = self.browser.as_ref().is_some_and(|browser| {
            browser
                .directory()
                .entries
                .iter()
                .any(|entry| self.replaces(entry))
        });
        if self.request.mode == Mode::Save {
            let file_name = fl!("portal-chooser-file-name");
            request_options = request_options.push(
                column![
                    text(fl!("portal-chooser-name-label")).size(14),
                    semantic(
                        Node::text_input(file_name.clone(), &self.name)
                            .key("chooser-name")
                            .placeholder(file_name.clone())
                            .required(true)
                            .active_descendant(focused_row)
                            .on_text(Message::Name),
                        self.field(&file_name, &self.name)
                            .id("name")
                            .on_input(Message::Name)
                            .on_submit(Message::Accept),
                    ),
                ]
                .spacing(5),
            );
        }
        if will_replace {
            let warning = fl!("portal-chooser-existing");
            request_options = request_options.push(semantic(
                Node::alert(&warning).key("chooser-existing-file"),
                canopa_ui::direction::row([
                    self.visuals.icon(browser_ui::Icon::Warning, 17.0),
                    text(warning)
                        .size(14)
                        .style(|theme| text::Style {
                            color: Some(canopa_ui::colors(theme).warning),
                        })
                        .into(),
                ])
                .align_y(iced::Center)
                .spacing(7),
            ));
        }
        if self.request.mode == Mode::SaveMany {
            request_options = request_options.push(text(fl!(
                "portal-chooser-save-many",
                count = self.request.files.len()
            )));
        }
        if !self.request.filters.is_empty() {
            let labels: Vec<_> = self
                .request
                .filters
                .iter()
                .enumerate()
                .map(|(i, f)| Item(i, f.0.clone()))
                .collect();
            let current = self.request.current_filter.as_ref().and_then(|f| {
                self.request
                    .filters
                    .iter()
                    .position(|candidate| candidate == f)
            });
            let selected = current.map(|i| labels[i].clone());
            let options = labels
                .iter()
                .map(|item| (item.1.clone(), Message::Filter(item.0)))
                .collect();
            request_options = request_options.push(a11y::combo_box(
                "chooser-filter",
                fl!("portal-chooser-file-type"),
                options,
                current,
                pick_list(labels, selected, |item| Message::Filter(item.0)),
            ));
        }
        for (index, (id, label, options, value)) in self.request.choices.iter().enumerate() {
            let key = format!("chooser-choice-{}", id.escape_default());
            if options.is_empty() {
                let checked = value == "true";
                request_options = request_options.push(a11y::check_box(
                    key,
                    label,
                    checked,
                    Some(Message::Choice(index, (!checked).to_string())),
                    checkbox(checked)
                        .style(canopa_ui::checkbox_style)
                        .label(label)
                        .on_toggle(move |value| Message::Choice(index, value.to_string())),
                ));
            } else {
                let items: Vec<_> = options
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Item(i, v.1.clone()))
                    .collect();
                let current = options.iter().position(|v| &v.0 == value);
                let selected = current.map(|i| items[i].clone());
                let messages = options
                    .iter()
                    .map(|(option, label)| (label.clone(), Message::Choice(index, option.clone())))
                    .collect();
                request_options = request_options.push(
                    row![
                        text(label),
                        a11y::combo_box(
                            key,
                            label,
                            messages,
                            current,
                            pick_list(items, selected, move |item| Message::Choice(
                                index,
                                options[item.0].0.clone()
                            ))
                        )
                    ]
                    .spacing(10),
                );
            }
        }
        if let Some(error) = &self.error {
            request_options = request_options.push(semantic(
                Node::alert(error).key("chooser-error"),
                text(error).style(text::danger),
            ));
        }
        let selected = self
            .browser
            .as_ref()
            .map_or(0, |browser| browser.selected().len());
        let can_accept = match self.request.mode {
            Mode::Open => self.request.directory || selected > 0,
            Mode::Save => !self.name.trim().is_empty() || selected > 0,
            Mode::SaveMany => true,
        };
        let accept = (enabled && can_accept).then_some(Message::Accept);
        let accept_label = if will_replace {
            fl!("portal-chooser-review-replacement")
        } else {
            self.request.accept_label.replace('_', "")
        };
        let read_only = if self.request.mode == Mode::Open && self.read_only_choice {
            a11y::check_box(
                "chooser-read-only",
                fl!("portal-chooser-read-only"),
                !self.writable,
                Some(Message::ReadOnly(self.writable)),
                checkbox(!self.writable)
                    .size(17)
                    .text_size(14)
                    .style(presentation::checkbox_style)
                    .label(fl!("portal-chooser-read-only"))
                    .on_toggle(Message::ReadOnly),
            )
        } else {
            space().width(0).into()
        };
        let selection_hint: Element<'_, Message> = if selected > 0 {
            text(fl!("portal-chooser-selected-count", count = selected))
                .size(14)
                .into()
        } else if self.request.mode == Mode::Open && !self.request.directory && total > 0 && enabled
        {
            dialog_ui::section(fl!("portal-chooser-select-file")).into()
        } else {
            space().width(0).into()
        };
        let start = canopa_ui::direction::row([
            read_only,
            space().width(Length::Fill).into(),
            selection_hint,
        ])
        .align_y(iced::Center)
        .spacing(10);
        let footer = canopa_ui::direction::row([
            start.into(),
            a11y::button(
                "chooser-cancel",
                fl!("portal-cancel"),
                Some(Message::Cancel),
                dialog_ui::action(
                    text(fl!("portal-cancel")),
                    Some(Message::Cancel),
                    canopa_ui::Action::Normal,
                    false,
                ),
            ),
            a11y::button(
                "chooser-accept",
                accept_label.clone(),
                accept.clone(),
                dialog_ui::action(
                    text(accept_label),
                    accept,
                    canopa_ui::Action::Primary,
                    false,
                ),
            ),
        ])
        .spacing(10)
        .align_y(iced::Center);
        let hidden = self.browser.as_ref().is_some_and(Browser::show_hidden);
        let label = fl!("portal-chooser-show-hidden");
        let toggle = enabled.then_some(Message::Hidden(!hidden));
        let hidden_files = a11y::check_box(
            "chooser-hidden",
            label.clone(),
            hidden,
            toggle,
            checkbox(hidden)
                .size(17)
                .text_size(14)
                .style(presentation::checkbox_style)
                .label(label)
                .on_toggle_maybe(enabled.then_some(Message::Hidden)),
        );
        let status_line = canopa_ui::direction::row([
            text(status.clone()).size(14).into(),
            hidden_files,
            text(fl!(
                "portal-requested-by",
                requester = self.visuals.requester.as_str()
            ))
            .size(13)
            .style(canopa_ui::muted)
            .width(Length::Fill)
            .align_x(canopa_ui::direction::end())
            .wrapping(iced::widget::text::Wrapping::None)
            .into(),
        ])
        .spacing(16)
        .align_y(iced::Center);
        let has_options = self.request.mode != Mode::Open
            || !self.request.filters.is_empty()
            || !self.request.choices.is_empty()
            || self.error.is_some();
        let mut bottom = column![].spacing(8);
        if has_options {
            bottom = bottom.push(request_options);
        }
        bottom = bottom.push(footer);
        let browser_header: Element<'_, Message> = if embedded {
            column![header, toolbar].spacing(8).into()
        } else {
            toolbar
        };
        let surface = column![
            container(browser_header)
                .padding([11, 16])
                .width(Length::Fill)
                .style(move |_| browser_ui::bar(appearance, true)),
            rule::horizontal(1).style(browser_ui::divider),
            body,
            rule::horizontal(1).style(browser_ui::divider),
            container(status_line)
                .padding([4, 16])
                .width(Length::Fill)
                .style(move |_| browser_ui::bar(appearance, false)),
            rule::horizontal(1).style(browser_ui::divider),
            container(bottom).padding([10, 16]).width(Length::Fill),
            a11y::announcer("chooser-status", status, false),
        ];
        container(surface)
            .style(move |_| browser_ui::window_surface(appearance, false))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn new_folder_view<'a>(&'a self, draft: &'a FolderDraft) -> Element<'a, Message> {
        let valid = folders::valid_name(&draft.name);
        let parent = draft.parent.path().to_string_lossy();
        let title = fl!("portal-chooser-new-folder");
        let folder_name = fl!("portal-chooser-folder-name");
        let mut prompt = column![
            semantic(
                Node::heading(title.clone(), 2),
                dialog_ui::heading(title.clone())
            ),
            text(parent.clone()).style(canopa_ui::muted),
            semantic(
                Node::text_input(folder_name.clone(), &draft.name)
                    .key("chooser-new-folder-name")
                    .placeholder(folder_name.clone())
                    .required(true)
                    .invalid(!draft.name.is_empty() && !valid)
                    .on_text(Message::FolderName),
                self.field(&folder_name, &draft.name)
                    .id("new-folder-name")
                    .on_input(Message::FolderName)
                    .on_submit(Message::CreateFolder),
            ),
        ]
        .spacing(12);
        if let Some(error) = &self.error {
            prompt = prompt.push(semantic(
                Node::alert(error).key("chooser-new-folder-error"),
                text(error).style(text::danger),
            ));
        }
        let cancel = (!self.busy).then_some(Message::KeepChoosing);
        let create = (!self.busy && valid).then_some(Message::CreateFolder);
        let create_label = if self.busy {
            fl!("portal-chooser-creating")
        } else {
            fl!("portal-chooser-create")
        };
        prompt = prompt.push(dialog_ui::footer([
            a11y::button(
                "chooser-new-folder-cancel",
                fl!("portal-cancel"),
                cancel.clone(),
                dialog_ui::action(
                    text(fl!("portal-cancel")),
                    cancel,
                    canopa_ui::Action::Normal,
                    false,
                ),
            ),
            a11y::button(
                "chooser-new-folder-create",
                create_label.clone(),
                create.clone(),
                dialog_ui::action(
                    text(create_label),
                    create,
                    canopa_ui::Action::Primary,
                    false,
                ),
            ),
        ]));
        semantic(
            Node::dialog(title)
                .key("chooser-new-folder")
                .description(parent)
                .busy(self.busy),
            container(
                container(prompt)
                    .padding(24)
                    .max_width(460)
                    .style(dialog_ui::card),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(dialog_ui::surface),
        )
    }
}

/// The replacement question: an alert dialog whose question is announced
/// assertively when it appears.
fn overwrite_view(prepared: &Prepared) -> Element<'_, Message> {
    let question = fl!("portal-chooser-overwrite-question");
    let names: Vec<String> = prepared
        .existing
        .iter()
        .map(|path| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let more = names.len().saturating_sub(3);
    let description = fl!(
        "portal-chooser-overwrite-names",
        names = names.iter().take(3).cloned().collect::<Vec<_>>().join(", "),
        more = more
    );
    let mut block = column![semantic(
        Node::alert(question.clone()).key("chooser-overwrite-question"),
        dialog_ui::heading(question)
    )]
    .spacing(10);
    for name in names.iter().take(3) {
        block = block.push(text(name.clone()));
    }
    if names.len() > 3 {
        block = block.push(text(fl!("portal-chooser-overwrite-more", count = more)));
    }
    block = block.push(dialog_ui::footer([
        a11y::button(
            "chooser-keep-choosing",
            fl!("portal-chooser-keep-choosing"),
            Some(Message::KeepChoosing),
            dialog_ui::action(
                text(fl!("portal-chooser-keep-choosing")),
                Some(Message::KeepChoosing),
                canopa_ui::Action::Normal,
                false,
            ),
        ),
        a11y::button(
            "chooser-replace",
            fl!("portal-chooser-replace"),
            Some(Message::Replace),
            dialog_ui::action(
                text(fl!("portal-chooser-replace")),
                Some(Message::Replace),
                canopa_ui::Action::Destructive,
                false,
            ),
        ),
    ]));
    semantic(
        Node::alert_dialog(fl!("portal-chooser-overwrite-title"))
            .key("chooser-overwrite")
            .description(description),
        container(
            container(block)
                .padding(24)
                .width(Length::Fill)
                .max_width(460)
                .style(dialog_ui::card),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(dialog_ui::surface),
    )
}

/// A file row's identity: its path, never its (lossy) display name.
fn entry_key(path: &Path) -> String {
    format!("chooser-entry-{}", a11y::path_key(path))
}

/// What a row's type and state sound like: "PDF file, hidden".
fn describe(entry: &canopa_files::browse::Entry) -> String {
    use canopa_files::browse::EntryKind;
    let kind = match entry.kind {
        EntryKind::Symlink if entry.is_directory() => fl!("portal-chooser-kind-folder-link"),
        EntryKind::Symlink => fl!("portal-chooser-kind-link"),
        _ if entry.is_directory() => fl!("portal-chooser-kind-folder"),
        _ => entry.path.extension().map_or_else(
            || fl!("portal-chooser-kind-file"),
            |extension| {
                fl!(
                    "portal-chooser-kind-typed-file",
                    extension = extension.to_string_lossy().to_uppercase()
                )
            },
        ),
    };
    if entry.is_hidden() {
        fl!("portal-chooser-kind-hidden", kind = kind)
    } else {
        kind
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Item(usize, String);
impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.1.fmt(f)
    }
}

fn prepare(
    request: &ChooserRequest,
    folder: &Path,
    selected: Vec<PathBuf>,
    name: OsString,
) -> io::Result<Prepared> {
    let target = canopa_files::picker::prepare(request, folder, selected, name)?;
    Ok(Prepared {
        paths: target.paths,
        existing: target.existing,
        file_target: None,
    })
}

fn prepare_with_target(
    request: &ChooserRequest,
    folder: &Path,
    selected: Vec<PathBuf>,
    name: OsString,
    retain: bool,
) -> io::Result<Prepared> {
    let mut prepared = prepare(request, folder, selected, name)?;
    if retain {
        if request.mode != Mode::Save || prepared.paths.len() != 1 {
            return Err(io::Error::other("Invalid retained save selection"));
        }
        let reviewed = Arc::new(crate::print::file_target::Reviewed::open(
            &prepared.paths[0],
        )?);
        if reviewed.selection().path() != prepared.paths[0].as_os_str().as_bytes() {
            return Err(io::Error::other(fl!("portal-chooser-destination-changed")));
        }
        prepared.existing = if reviewed.selection().replaces() {
            prepared.paths.clone()
        } else {
            Vec::new()
        };
        prepared.file_target = Some(reviewed);
    }
    Ok(prepared)
}
