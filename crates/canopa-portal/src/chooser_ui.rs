//! Iced adapter. Directory reads and acceptance checks run on filesystem workers.
mod filters;
mod folders;
#[cfg(test)]
mod tests;

use crate::file_chooser::{ChooserRequest, Mode, Selection, file_name, path_bytes};
use crate::worker::run as worker;
use canopa_files::browse::{Browser, Directory, SelectionMode};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{
        button, checkbox, column, container, pick_list, row, rule, scrollable, text, text_input,
    },
    window,
};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    io,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub fn run(
    request: ChooserRequest,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<Option<Selection>> {
    let start = prepare_picker(request)?;
    let output = Arc::new(Mutex::new(None));
    let result = output.clone();
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    // Iced's event loop must remain on the helper's main thread.
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(820.0, 560.0),
                min_size: Some(Size::new(480.0, 360.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let open = if let Some(handle) = start
                .request
                .parent_window
                .strip_prefix("wayland:")
                .filter(|handle| !handle.is_empty())
            {
                window::open_dialog(handle.to_owned(), start.request.modal, settings)
                    .1
                    .map(|_| Message::Opened)
            } else {
                window::open(settings).1.map(|_| Message::Opened)
            };
            let (picker, load) = Picker::new(start.clone(), PickerOptions::default());
            (
                Dialog {
                    picker,
                    theme: theme.clone(),
                    output: output.clone(),
                },
                Task::batch([open, load]),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|app: &Dialog, _| format!("{} — Canopa", app.picker.request.title))
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(|app: &Dialog| app.picker.subscription())
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("chooser output").take())
}

struct Dialog {
    picker: Picker,
    theme: Theme,
    output: Arc<Mutex<Option<Selection>>>,
}

impl Dialog {
    fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.picker.update(message);
        match self.picker.take_outcome() {
            Some(Outcome::Selected(selection)) => {
                *self.output.lock().expect("chooser output") = Some(selection);
                iced::exit()
            }
            Some(Outcome::Cancelled) => iced::exit(),
            None => task,
        }
    }

    fn view(&self, _: window::Id) -> Element<'_, Message> {
        self.picker.view()
    }
}

/// Preparation may read local places and MIME metadata. Embedded callers run it
/// on a worker; the standalone adapter runs it before starting the event loop.
pub(crate) fn prepare_picker(request: ChooserRequest) -> anyhow::Result<PickerStart> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from("/"));
    let config_home = canopa_config::config_home_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        Some(home.as_os_str()),
    )?;
    let places = canopa_files::personal_folders::PersonalFolders::load(&home, &config_home)?;
    let mut shortcuts = vec![("Home".to_owned(), home.clone())];
    for (label, kind) in [
        ("Documents", canopa_files::interface::PlaceKind::Documents),
        ("Downloads", canopa_files::interface::PlaceKind::Downloads),
        ("Pictures", canopa_files::interface::PlaceKind::Pictures),
    ] {
        if let Some(path) = places.path(kind) {
            shortcuts.push((label.into(), path.to_owned()));
        }
    }
    shortcuts.push(("Filesystem".into(), PathBuf::from("/")));
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
    home: PathBuf,
    shortcuts: Vec<(String, PathBuf)>,
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
    browser: Option<Browser>,
    shortcuts: Vec<(String, PathBuf)>,
    home: PathBuf,
    location: String,
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
    overwrite: Option<Prepared>,
    new_folder: Option<FolderDraft>,
    scan: canopa_files::Cancellation,
    writable: bool,
    last_click: Option<(PathBuf, std::time::Instant)>,
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
    Opened,
    Navigate(PathBuf),
    Up,
    Home,
    Location(String),
    Go,
    Name(String),
    Search(String),
    NewFolder,
    FolderParent(u64, Result<Arc<folders::Parent>, String>),
    FolderName(String),
    CreateFolder,
    FolderCreated(u64, Result<PathBuf, String>),
    Loaded(u64, bool, Result<Directory, String>),
    Select(PathBuf),
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
            browser: None,
            shortcuts: start.shortcuts,
            home: start.home,
            location: start.folder.to_string_lossy().into_owned(),
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
            overwrite: None,
            new_folder: None,
            writable: true,
            last_click: None,
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
        self.error = None;
        self.overwrite = None;
        self.new_folder = None;
        self.search.clear();
        self.last_click = None;
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
            Message::Opened => {}
            Message::Cancel => {
                self.scan.cancel();
                self.finished = true;
                self.outcome = Some(Outcome::Cancelled);
                return Task::none();
            }
            Message::Navigate(path) => return self.navigate(path, false),
            Message::Home => return self.navigate(self.home.clone(), false),
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
            Message::Location(value) => self.location = value,
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
                self.name = value;
                self.original_name = None;
                self.overwrite = None;
            }
            Message::Search(value) => {
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
                    self.error =
                        Some("Enter one folder name, without a slash, up to 255 bytes".into());
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
                        let hidden = self.browser.as_ref().is_some_and(Browser::show_hidden);
                        let mut browser = Browser::new(directory);
                        browser.set_show_hidden(hidden);
                        if initial
                            && let Some(path) =
                                self.request.current_file.as_deref().and_then(path_bytes)
                        {
                            browser.reveal_selection(vec![path]);
                        }
                        self.browser = Some(browser);
                    }
                    Err(error) => {
                        self.error = Some(error);
                        if initial {
                            return self.navigate(self.home.clone(), false);
                        }
                    }
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
                        Key::Character("l") => return iced::widget::operation::focus("location"),
                        Key::Character("f") => return iced::widget::operation::focus("search"),
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
                } else if modifiers.alt() && key == Key::Named(Named::ArrowUp) {
                    return self.update(Message::Up);
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

    pub(crate) fn view(&self) -> Element<'_, Message> {
        if let Some(draft) = &self.new_folder {
            let mut prompt = column![
                text("New folder").size(22),
                text(draft.parent.path().to_string_lossy()).style(canopa_ui::muted),
                text_input("Folder name", &draft.name)
                    .id("new-folder-name")
                    .on_input(Message::FolderName)
                    .on_submit(Message::CreateFolder),
            ]
            .spacing(12);
            if let Some(error) = &self.error {
                prompt = prompt.push(text(error).style(text::danger));
            }
            prompt = prompt.push(
                row![
                    canopa_ui::action(
                        text("Cancel"),
                        (!self.busy).then_some(Message::KeepChoosing),
                        canopa_ui::Action::Normal,
                        false
                    ),
                    canopa_ui::action(
                        text(if self.busy { "Creating…" } else { "Create" }),
                        (!self.busy && folders::valid_name(&draft.name))
                            .then_some(Message::CreateFolder),
                        canopa_ui::Action::Primary,
                        false
                    ),
                ]
                .spacing(10),
            );
            return container(container(prompt).padding(24).max_width(460))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(canopa_ui::surface)
                .into();
        }
        let enabled = !self.loading && !self.busy && self.overwrite.is_none();
        let action = |label: &'static str, message| {
            canopa_ui::action(
                text(label),
                enabled.then_some(message),
                canopa_ui::Action::Normal,
                false,
            )
        };
        let mut toolbar = row![
            action("Home", Message::Home),
            action("Up", Message::Up),
            text_input("Location", &self.location)
                .id("location")
                .on_input(Message::Location)
                .on_submit(Message::Go)
                .width(Length::Fill),
            action("Go", Message::Go)
        ]
        .spacing(6);
        if self.request.mode != Mode::Open {
            toolbar = toolbar.push(action("New folder", Message::NewFolder));
        }
        let mut shortcuts = column![chooser_heading("Places", 14),].spacing(5);
        for (label, path) in &self.shortcuts {
            let current = self
                .browser
                .as_ref()
                .is_some_and(|browser| browser.path() == path);
            shortcuts = shortcuts.push(
                canopa_ui::action(
                    text(label),
                    enabled.then_some(Message::Navigate(path.clone())),
                    if current {
                        canopa_ui::Action::Normal
                    } else {
                        canopa_ui::Action::Quiet
                    },
                    false,
                )
                .width(Length::Fill),
            );
        }
        let places = container(scrollable(shortcuts).height(Length::Fill))
            .width(145)
            .height(Length::Fill)
            .padding(10)
            .style(|theme| {
                container::Style::default()
                    .background(canopa_ui::colors(theme).surface)
                    .border(iced::Border::default().rounded(10))
            });
        let entries = self.entries();
        let empty = entries.is_empty();
        let mut listing = column![].spacing(2);
        for entry in entries {
            let selected = self
                .browser
                .as_ref()
                .is_some_and(|b| b.selected().contains(&entry.path));
            let label = format!(
                "{}  {}",
                if entry.is_directory() { "▸" } else { " " },
                entry.name()
            );
            let select = button(text(label))
                .width(Length::Fill)
                .padding([5, 8])
                .on_press_maybe(enabled.then_some(Message::Select(entry.path.clone())))
                .style(move |theme: &Theme, status| {
                    if selected {
                        button::primary(theme, status)
                    } else {
                        button::text(theme, status)
                    }
                });
            listing = listing.push(
                row![
                    select,
                    canopa_ui::action(
                        text("Open").size(13),
                        enabled.then_some(Message::Activate(entry.path.clone())),
                        canopa_ui::Action::Quiet,
                        false
                    )
                ]
                .spacing(4),
            );
        }
        if self.loading {
            listing = listing.push(text("Loading…"));
        } else if empty {
            listing = listing
                .push(container(text("No matching files").style(canopa_ui::muted)).padding(12));
        }
        let body = row![
            places,
            column![
                container(chooser_heading("Name", 14)).padding([0, 8]),
                rule::horizontal(1).style(chooser_rule),
                scrollable(listing).height(Length::Fill).width(Length::Fill),
            ]
            .spacing(6)
        ]
        .spacing(12)
        .height(Length::Fill);
        let mut content = column![
            column![
                chooser_heading(&self.request.title, 23),
                text(if self.request.app_id.is_empty() {
                    "Choose from your local files and folders"
                } else {
                    &self.request.app_id
                })
                .size(13)
                .style(canopa_ui::muted)
            ]
            .spacing(3),
            toolbar,
            text_input("Filter names in this folder", &self.search)
                .id("search")
                .on_input(Message::Search),
            body
        ]
        .spacing(10);
        if self.request.mode == Mode::Save {
            content = content.push(
                row![
                    text("Name"),
                    text_input("File name", &self.name)
                        .id("name")
                        .on_input(Message::Name)
                        .on_submit(Message::Accept)
                ]
                .spacing(10)
                .align_y(iced::Center),
            );
        }
        if self.request.mode == Mode::SaveMany {
            content = content.push(text(format!(
                "Choose a folder for {} files",
                self.request.files.len()
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
            let selected = self
                .request
                .current_filter
                .as_ref()
                .and_then(|f| {
                    self.request
                        .filters
                        .iter()
                        .position(|candidate| candidate == f)
                })
                .map(|i| labels[i].clone());
            content = content.push(pick_list(labels, selected, |item| Message::Filter(item.0)));
        }
        for (index, (_, label, options, value)) in self.request.choices.iter().enumerate() {
            if options.is_empty() {
                content = content.push(
                    checkbox(value == "true")
                        .style(canopa_ui::checkbox_style)
                        .label(label)
                        .on_toggle(move |value| Message::Choice(index, value.to_string())),
                );
            } else {
                let items: Vec<_> = options
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Item(i, v.1.clone()))
                    .collect();
                let selected = options
                    .iter()
                    .position(|v| &v.0 == value)
                    .map(|i| items[i].clone());
                content = content.push(
                    row![
                        text(label),
                        pick_list(items, selected, move |item| Message::Choice(
                            index,
                            options[item.0].0.clone()
                        ))
                    ]
                    .spacing(10),
                );
            }
        }
        if self.request.mode == Mode::Open && self.read_only_choice {
            content = content.push(
                checkbox(!self.writable)
                    .style(canopa_ui::checkbox_style)
                    .label("Open read-only")
                    .on_toggle(Message::ReadOnly),
            );
        }
        if let Some(error) = &self.error {
            content = content.push(text(error).style(text::danger));
        }
        if let Some(prepared) = &self.overwrite {
            content = content.push(text(
                "The destination already exists. Allow the application to replace it?",
            ));
            for path in prepared.existing.iter().take(3) {
                content = content.push(text(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                ));
            }
            if prepared.existing.len() > 3 {
                content = content.push(text(format!("and {} more", prepared.existing.len() - 3)));
            }
            content = content.push(
                row![
                    canopa_ui::action(
                        text("Keep choosing"),
                        Some(Message::KeepChoosing),
                        canopa_ui::Action::Normal,
                        false
                    ),
                    canopa_ui::action(
                        text("Replace"),
                        Some(Message::Replace),
                        canopa_ui::Action::Destructive,
                        false
                    )
                ]
                .spacing(10),
            );
        }
        let footer = row![
            checkbox(self.browser.as_ref().is_some_and(Browser::show_hidden))
                .style(canopa_ui::checkbox_style)
                .label("Show hidden files")
                .on_toggle(Message::Hidden),
            iced::widget::space().width(Length::Fill),
            canopa_ui::action(
                text("Cancel"),
                Some(Message::Cancel),
                canopa_ui::Action::Normal,
                false
            ),
            canopa_ui::action(
                text(self.request.accept_label.replace('_', "")),
                enabled.then_some(Message::Accept),
                canopa_ui::Action::Primary,
                false
            )
        ]
        .spacing(10)
        .align_y(iced::Center);
        container(
            content
                .push(rule::horizontal(1).style(chooser_rule))
                .push(footer),
        )
        .padding(16)
        .style(canopa_ui::surface)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

fn chooser_heading(label: &str, size: u32) -> iced::widget::Text<'_> {
    text(label).size(size)
}

fn chooser_rule(theme: &Theme) -> rule::Style {
    rule::Style {
        color: canopa_ui::colors(theme).border,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Item(usize, String);
impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.1.fmt(f)
    }
}

/// Read-only acceptance: the requesting application owns all actual writes.
fn prepare(
    request: &ChooserRequest,
    folder: &Path,
    selected: Vec<PathBuf>,
    name: OsString,
) -> io::Result<Prepared> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidInput, message);
    let folder = std::fs::canonicalize(folder)?;
    if !folder.is_dir() {
        return Err(invalid("Choose an existing folder"));
    }
    let paths = match request.mode {
        Mode::Open => {
            let paths = if selected.is_empty() && request.directory {
                vec![folder]
            } else {
                selected
            };
            if paths.is_empty() || (!request.multiple && paths.len() > 1) {
                return Err(invalid("Select a file"));
            }
            for path in &paths {
                let metadata = std::fs::metadata(path)?;
                if if request.directory {
                    !metadata.is_dir()
                } else {
                    !metadata.is_file()
                } {
                    return Err(invalid("Select the requested file type"));
                }
            }
            paths
        }
        Mode::Save => {
            if name.as_bytes().contains(&0) || !file_name(Path::new(&name)) {
                return Err(invalid("Enter one file name without a folder path"));
            }
            vec![folder.join(name)]
        }
        Mode::SaveMany => {
            let folder = if selected.len() == 1 {
                std::fs::canonicalize(&selected[0])?
            } else {
                folder
            };
            if !folder.is_dir() {
                return Err(invalid("Choose a folder"));
            }
            request
                .files
                .iter()
                .map(|bytes| {
                    path_bytes(bytes)
                        .filter(|p| file_name(p))
                        .map(|p| folder.join(p))
                        .ok_or_else(|| invalid("Invalid file name"))
                })
                .collect::<io::Result<Vec<_>>>()?
        }
    };
    let mut existing = Vec::new();
    if request.mode != Mode::Open {
        let mut unique = BTreeSet::new();
        for path in &paths {
            if !unique.insert(path) {
                return Err(invalid("Save file names must be distinct"));
            }
            match std::fs::symlink_metadata(path) {
                Ok(metadata) => {
                    if !metadata.is_file() {
                        return Err(invalid("The destination is a folder or special file"));
                    }
                    existing.push(path.clone());
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(Prepared {
        paths,
        existing,
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
            return Err(io::Error::other(
                "The destination folder changed. Choose it again.",
            ));
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
