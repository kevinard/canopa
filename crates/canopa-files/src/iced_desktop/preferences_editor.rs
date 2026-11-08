//! One transactional interface draft. Rendered controls capture its page/selection
//! revision; publications update the application but never replace an open draft.
use super::*;
use crate::{
    interface::{InterfaceSettings, Place, PlaceKind},
    preferences::{Detail, SortField, ViewMode},
};
use action_editor::Operation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Page {
    Views,
    Toolbar,
    Locations,
    Shortcuts,
}
impl Page {
    pub const ALL: [Self; 4] = [Self::Views, Self::Toolbar, Self::Locations, Self::Shortcuts];
    pub fn label(self) -> &'static str {
        match self {
            Self::Views => "Views",
            Self::Toolbar => "Toolbar",
            Self::Locations => "Locations",
            Self::Shortcuts => "Shortcuts",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Target {
    pub window: window::Id,
    pub revision: u64,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Field {
    Width,
    Filter,
    BookmarkName,
    BookmarkPath,
    Shortcut,
}
#[derive(Clone, Debug)]
pub(super) enum Change {
    Mode(ViewMode),
    Sort(SortField),
    Descending(bool),
    Detail(Detail, bool),
    Sidebar(SidebarMode),
    Field(Field, String),
    SelectToolbar(usize),
    MoveToolbar(isize),
    RemoveToolbar,
    AddToolbar(String),
    SelectPlace(usize),
    MovePlace(isize),
    RemovePlace,
    AddPlace(PlaceKind),
    AddBookmark,
    SelectCommand(Command),
    ResetShortcut,
    Record(bool),
}
#[derive(Clone, Debug)]
pub(super) enum Event {
    Page(Target, Page),
    Change(Target, Change),
    Save(window::Id),
    Reload(window::Id),
    Confirm(window::Id, bool),
    Finished(u64, Option<Arc<configuration_ui::Reply>>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Confirmation {
    Close,
    Reload,
}

pub(super) enum Location {
    Builtin(PlaceKind),
    Bookmark { name: String, path: String },
}
impl Location {
    pub fn label(&self) -> &str {
        match self {
            Self::Builtin(kind) => kind.label(),
            Self::Bookmark { name, .. } => name,
        }
    }
}

pub(super) struct Editor {
    pub window: window::Id,
    pub base: ConfigurationSnapshot,
    pub draft: InterfaceSettings,
    pub page: Page,
    pub width: String,
    pub filter: String,
    pub locations: Vec<Location>,
    pub selected_toolbar: Option<usize>,
    pub selected_place: Option<usize>,
    pub selected_command: Command,
    pub shortcuts: BTreeMap<Command, String>,
    pub busy: bool,
    pub dirty: bool,
    pub recording: bool,
    recording_activation: bool,
    pub confirmation: Option<Confirmation>,
    pub status: String,
    revision: u64,
    folder: PathBuf,
}
impl Editor {
    fn new(window: window::Id, base: ConfigurationSnapshot, folder: PathBuf) -> Self {
        let locations = base
            .settings
            .places
            .iter()
            .map(|place| match place {
                Place::Builtin(kind) => Location::Builtin(*kind),
                Place::Bookmark { name, path } => Location::Bookmark {
                    name: name.clone(),
                    path: display_location(path),
                },
            })
            .collect();
        let shortcuts = Command::ALL
            .iter()
            .map(|command| (*command, base.settings.bindings(*command).join("; ")))
            .collect();
        Self {
            window,
            width: base.settings.view.column_width.to_string(),
            draft: base.settings.clone(),
            base,
            page: Page::Views,
            filter: String::new(),
            locations,
            selected_toolbar: Some(0),
            selected_place: None,
            selected_command: Command::Search,
            shortcuts,
            busy: false,
            dirty: false,
            recording: false,
            recording_activation: false,
            confirmation: None,
            status: "Changes apply to all Files windows when saved.".into(),
            revision: 0,
            folder,
        }
    }
    pub fn target(&self) -> Target {
        Target {
            window: self.window,
            revision: self.revision,
        }
    }
    pub fn enabled(&self) -> bool {
        !self.busy && !self.recording && self.confirmation.is_none()
    }
    fn accepts(&self, target: Target) -> bool {
        self.enabled() && target.window == self.window && target.revision == self.revision
    }
    fn select_page(&mut self, page: Page) {
        self.page = page;
        self.filter.clear();
        self.revision += 1;
    }
    fn add_bookmark(&mut self, folder: PathBuf) {
        if self.locations.len() >= 256 {
            self.status = "At most 256 locations are supported.".into();
            return;
        }
        self.select_page(Page::Locations);
        let name = folder
            .file_name()
            .unwrap_or(folder.as_os_str())
            .to_string_lossy()
            .into_owned();
        self.locations.push(Location::Bookmark {
            name,
            path: display_location(&folder),
        });
        self.selected_place = Some(self.locations.len() - 1);
        self.dirty = true;
    }
    pub fn focus_id(&self, name: &str) -> iced::widget::Id {
        iced::widget::Id::from(format!(
            "files-preferences-{:?}-{}-{name}",
            self.window, self.revision
        ))
    }
    pub fn settings(&self, home: &Path) -> Result<InterfaceSettings, String> {
        let mut settings = self.draft.clone();
        settings.view.column_width = self
            .width
            .trim()
            .parse()
            .ok()
            .filter(|width| (160..=640).contains(width))
            .ok_or("Column width must be a whole number from 160 to 640.")?;
        settings.places = self
            .locations
            .iter()
            .map(|location| match location {
                Location::Builtin(kind) => Ok(Place::Builtin(*kind)),
                Location::Bookmark { name, path } => {
                    // Do not interpret arbitrary schemes as relative local paths.
                    if !path.starts_with('/')
                        && !path.starts_with("file://")
                        && path != "~"
                        && !path.starts_with("~/")
                    {
                        return Err(
                            "A bookmark needs an absolute local path, ~/ path or local file URI."
                                .into(),
                        );
                    }
                    let path =
                        local_path(Path::new(path), home, None).map_err(|e| e.to_string())?;
                    Ok(Place::Bookmark {
                        name: name.trim().to_owned(),
                        path,
                    })
                }
            })
            .collect::<Result<_, String>>()?;
        settings.shortcuts.clear();
        for (command, text) in &self.shortcuts {
            let bindings = text
                .split(';')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(|v| {
                    KeyChord::parse(v)
                        .map(|c| c.accelerator())
                        .map_err(|e| format!("{}: {e}", command.label()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let defaults = command
                .defaults()
                .iter()
                .map(|s| KeyChord::parse(s).unwrap().accelerator())
                .collect::<Vec<_>>();
            if bindings != defaults {
                settings.shortcuts.insert(*command, bindings);
            }
        }
        settings
            .validate(&self.base.actions)
            .map_err(|e| e.to_string())?;
        Ok(settings)
    }
    fn change(&mut self, change: Change) {
        match change {
            Change::Mode(v) => self.draft.view.mode = v,
            Change::Sort(v) => self.draft.view.sort = v,
            Change::Descending(v) => self.draft.view.descending = v,
            Change::Detail(detail, value) => {
                self.draft.view.details.retain(|d| *d != detail);
                if value {
                    self.draft.view.details.push(detail);
                }
            }
            Change::Sidebar(v) => self.draft.sidebar = v,
            Change::Field(field, value) => {
                let max = match field {
                    Field::Width => 5,
                    Field::Filter => 128,
                    Field::BookmarkName => 128,
                    Field::BookmarkPath => 49160,
                    Field::Shortcut => 1040,
                };
                if value.len() > max {
                    self.status = format!("This field is limited to {max} bytes.");
                    return;
                }
                match field {
                    Field::Width => self.width = value,
                    Field::Filter => {
                        self.filter = value;
                        return;
                    }
                    Field::Shortcut => {
                        self.shortcuts.insert(self.selected_command, value);
                    }
                    Field::BookmarkName | Field::BookmarkPath => {
                        if let Some(Location::Bookmark { name, path }) =
                            self.selected_place.and_then(|i| self.locations.get_mut(i))
                        {
                            *if matches!(field, Field::BookmarkName) {
                                name
                            } else {
                                path
                            } = value;
                        } else {
                            return;
                        }
                    }
                }
            }
            Change::SelectToolbar(index) if index < self.draft.toolbar.len() => {
                self.selected_toolbar = Some(index);
                self.revision += 1;
                return;
            }
            Change::MoveToolbar(delta) => {
                if let Some(index) = self.selected_toolbar {
                    let next = index
                        .saturating_add_signed(delta)
                        .min(self.draft.toolbar.len().saturating_sub(1));
                    if index < self.draft.toolbar.len() {
                        self.draft.toolbar.swap(index, next);
                        self.selected_toolbar = Some(next);
                        self.revision += 1;
                    }
                }
            }
            Change::RemoveToolbar => {
                if let Some(index) = self
                    .selected_toolbar
                    .filter(|i| *i < self.draft.toolbar.len())
                {
                    if self.draft.toolbar[index] == "location" {
                        self.status = "The location field must remain available.".into();
                        return;
                    }
                    self.draft.toolbar.remove(index);
                    self.selected_toolbar = (!self.draft.toolbar.is_empty())
                        .then_some(index.min(self.draft.toolbar.len().saturating_sub(1)));
                    self.revision += 1;
                }
            }
            Change::AddToolbar(id) => {
                if self.draft.toolbar.len() >= 64
                    || self.draft.toolbar.contains(&id)
                    || id == Command::QuickLook.id()
                    || !(Command::from_id(&id).is_some()
                        || id
                            .strip_prefix("action:")
                            .is_some_and(|id| self.base.actions.iter().any(|a| a.id == id)))
                {
                    return;
                }
                self.draft.toolbar.push(id);
                self.selected_toolbar = Some(self.draft.toolbar.len() - 1);
                self.revision += 1;
            }
            Change::SelectPlace(index) if index < self.locations.len() => {
                self.selected_place = Some(index);
                self.revision += 1;
                return;
            }
            Change::MovePlace(delta) => {
                if let Some(index) = self.selected_place.filter(|i| *i < self.locations.len()) {
                    let next = index
                        .saturating_add_signed(delta)
                        .min(self.locations.len() - 1);
                    self.locations.swap(index, next);
                    self.selected_place = Some(next);
                    self.revision += 1;
                }
            }
            Change::RemovePlace => {
                if let Some(index) = self.selected_place.filter(|i| *i < self.locations.len()) {
                    self.locations.remove(index);
                    self.selected_place = (!self.locations.is_empty())
                        .then_some(index.min(self.locations.len().saturating_sub(1)));
                    self.revision += 1;
                }
            }
            Change::AddPlace(kind) => {
                if self.locations.len() >= 256
                    || self
                        .locations
                        .iter()
                        .any(|p| matches!(p, Location::Builtin(k) if *k==kind))
                {
                    return;
                }
                self.locations.push(Location::Builtin(kind));
                self.selected_place = Some(self.locations.len() - 1);
                self.revision += 1;
            }
            Change::AddBookmark => {
                self.add_bookmark(self.folder.clone());
                return;
            }
            Change::SelectCommand(command) => {
                self.selected_command = command;
                self.revision += 1;
                return;
            }
            Change::ResetShortcut => {
                self.shortcuts.insert(
                    self.selected_command,
                    self.selected_command.defaults().join("; "),
                );
            }
            Change::Record(keyboard) => {
                self.recording_activation = keyboard;
                self.recording = true;
                self.status =
                    "Press a shortcut. Escape cancels; Backspace disables this command.".into();
                return;
            }
            _ => return,
        }
        self.dirty = true;
    }
}

impl Desktop {
    pub(super) fn show_preferences(&mut self, folder: PathBuf, bookmark: bool) -> Task<Message> {
        if let Some(editor) = &mut self.preferences_editor {
            if bookmark && editor.enabled() {
                editor.add_bookmark(folder);
            }
            return window::gain_focus(editor.window);
        }
        let (id, opened) = window::open(window::Settings {
            size: Size::new(720.0, 472.0),
            min_size: Some(Size::new(280.0, 240.0)),
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "org.canopa.Files".into(),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut editor = Editor::new(id, self.configuration.clone(), folder.clone());
        if bookmark {
            editor.add_bookmark(folder);
        }
        self.preferences_editor = Some(editor);
        opened.map(Message::Opened)
    }
    pub(super) fn close_preferences(&mut self, id: window::Id, forced: bool) -> Task<Message> {
        let Some(editor) = self.preferences_editor.as_mut().filter(|e| e.window == id) else {
            return Task::none();
        };
        if !forced && editor.busy {
            editor.status = "Wait for the configuration change to finish.".into();
            return Task::none();
        }
        if !forced && editor.dirty {
            editor.recording = false;
            editor.confirmation = Some(Confirmation::Close);
            return Task::none();
        }
        self.preferences_editor = None;
        Task::batch([window::close(id), self.exit_if_empty()])
    }
    fn write_preferences(&mut self, id: window::Id, operation: Operation) -> Task<Message> {
        let Some(editor) = self
            .preferences_editor
            .as_mut()
            .filter(|e| e.window == id && e.enabled())
        else {
            return Task::none();
        };
        if self.configuration_pending.is_some() {
            editor.status = "Another configuration change is still in progress.".into();
            return Task::none();
        }
        let Some(worker) = &self.configuration_worker else {
            editor.status = "The configuration worker is unavailable.".into();
            return Task::none();
        };
        let receiver = match operation {
            Operation::Save => match editor.settings(&self.home) {
                Ok(settings) => worker.save_settings(editor.base.clone(), settings),
                Err(error) => {
                    editor.status = error;
                    return Task::none();
                }
            },
            Operation::Reload => worker.reload(editor.base.clone()),
        };
        let receiver = match receiver {
            Ok(r) => r,
            Err(e) => {
                editor.status = e.to_string();
                return Task::none();
            }
        };
        self.configuration_request += 1;
        let request = self.configuration_request;
        self.configuration_pending = Some((request, id, operation));
        editor.busy = true;
        editor.status = if operation == Operation::Save {
            "Saving…"
        } else {
            "Reloading…"
        }
        .into();
        Task::perform(
            async move { receiver.await.ok().map(Arc::new) },
            move |result| Message::Preferences(Event::Finished(request, result)),
        )
    }
    pub(super) fn preferences_event(&mut self, event: Event) -> Task<Message> {
        if let Event::Finished(request, result) = event {
            let Some((expected, id, operation)) = self.configuration_pending else {
                return Task::none();
            };
            if request != expected {
                return Task::none();
            }
            self.configuration_pending = None;
            let result = result
                .as_deref()
                .cloned()
                .unwrap_or_else(|| Err("The configuration worker stopped before replying.".into()));
            let install = if let Ok(event) = &result {
                self.install_configuration(event.clone())
            } else {
                Task::none()
            };
            if let Some(editor) = self.preferences_editor.as_mut().filter(|e| e.window == id) {
                editor.busy = false;
                match result {
                    Ok(publication) => {
                        let snapshot = publication.current;
                        let page = editor.page;
                        let revision = editor.revision + 1;
                        let folder = editor.folder.clone();
                        *editor = Editor::new(id, (*snapshot).clone(), folder);
                        editor.page = page;
                        editor.revision = revision;
                        editor.status = if operation == Operation::Save {
                            "Saved. Preferences apply to all Files windows."
                        } else {
                            "Reloaded shared Files preferences."
                        }
                        .into();
                    }
                    Err(e) => editor.status = e,
                }
            }
            return Task::batch([install, self.exit_if_empty()]);
        }
        let id = match &event {
            Event::Page(t, _) | Event::Change(t, _) => t.window,
            Event::Save(id) | Event::Reload(id) | Event::Confirm(id, _) => *id,
            Event::Finished(..) => unreachable!(),
        };
        let Some(editor) = self
            .preferences_editor
            .as_mut()
            .filter(|e| e.window == id && !e.busy)
        else {
            return Task::none();
        };
        if let Event::Confirm(_, discard) = event {
            let confirmation = editor.confirmation.take();
            if !discard {
                return Task::none();
            }
            return match confirmation {
                Some(Confirmation::Close) => self.close_preferences(id, true),
                Some(Confirmation::Reload) => self.write_preferences(id, Operation::Reload),
                None => Task::none(),
            };
        }
        if !editor.enabled() {
            return Task::none();
        }
        match event {
            Event::Page(t, page) if editor.accepts(t) => editor.select_page(page),
            Event::Change(t, change) if editor.accepts(t) => editor.change(change),
            Event::Save(_) => return self.write_preferences(id, Operation::Save),
            Event::Reload(_) => {
                if editor.dirty {
                    editor.confirmation = Some(Confirmation::Reload);
                } else {
                    return self.write_preferences(id, Operation::Reload);
                }
            }
            _ => {}
        }
        Task::none()
    }
    pub(super) fn preferences_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        let Some(editor) = self
            .preferences_editor
            .as_mut()
            .filter(|e| e.window == id && !e.busy)
        else {
            return Task::none();
        };
        if editor.recording {
            if std::mem::take(&mut editor.recording_activation) && focus::activation_key(&key) {
                return Task::none();
            }
            match &key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                    editor.recording = false;
                    editor.status = "Shortcut recording cancelled.".into();
                }
                keyboard::Key::Named(keyboard::key::Named::Backspace) if modifiers.is_empty() => {
                    editor.recording = false;
                    editor.change(Change::Field(Field::Shortcut, String::new()));
                }
                _ => {
                    if let Some(chord) = key_chord(&key, modifiers) {
                        editor.recording = false;
                        editor.change(Change::Field(Field::Shortcut, chord.accelerator()));
                    }
                }
            }
            return Task::none();
        }
        if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
            if editor.confirmation.take().is_some() {
                return Task::none();
            }
            return self.close_preferences(id, false);
        }
        if editor.confirmation.is_some() {
            return Task::none();
        }
        // Page accelerators belong to the form widget: handling Alt+digits
        // here is too late to prevent native text insertion or preserve focus.
        if let keyboard::Key::Character(value) = key
            && modifiers.control()
            && !modifiers.alt()
            && !modifiers.logo()
        {
            return match value.to_ascii_lowercase().as_str() {
                "s" => self.preferences_event(Event::Save(id)),
                "r" => self.preferences_event(Event::Reload(id)),
                "w" => self.close_preferences(id, false),
                _ => Task::none(),
            };
        }
        Task::none()
    }
}

impl Desktop {
    pub(super) fn place_path(&self, place: &Place) -> Option<PathBuf> {
        Some(match place {
            Place::Bookmark { path, .. } => path.clone(),
            Place::Builtin(kind) => match kind {
                PlaceKind::Home => self.home.clone(),
                PlaceKind::Filesystem => PathBuf::from("/"),
                PlaceKind::Desktop
                | PlaceKind::Documents
                | PlaceKind::Downloads
                | PlaceKind::Pictures => self.personal_folders.path(*kind)?.to_owned(),
                PlaceKind::Trash | PlaceKind::Recovery => return None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_recording_ignores_its_activation_but_mouse_recording_accepts_the_first_key() {
        let root = tempfile::tempdir().unwrap();
        let (mut desktop, _) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let _ = desktop.show_preferences(root.path().to_owned(), false);
        let target = desktop.preferences_editor.as_ref().unwrap().target();
        let _ = desktop.preferences_event(Event::Change(target, Change::Record(true)));
        let _ = desktop.update(Message::CapturedKey(
            target.window,
            keyboard::Key::Named(keyboard::key::Named::Enter),
            keyboard::Modifiers::empty(),
        ));
        assert!(desktop.preferences_editor.as_ref().unwrap().recording);
        let _ = desktop.update(Message::CapturedKey(
            target.window,
            keyboard::Key::Named(keyboard::key::Named::F8),
            keyboard::Modifiers::empty(),
        ));
        assert_eq!(
            desktop.preferences_editor.as_ref().unwrap().shortcuts[&Command::Search],
            "F8"
        );
        let _ = desktop.preferences_event(Event::Change(target, Change::Record(false)));
        let _ = desktop.update(Message::CapturedKey(
            target.window,
            keyboard::Key::Named(keyboard::key::Named::Enter),
            keyboard::Modifiers::empty(),
        ));
        let editor = desktop.preferences_editor.as_ref().unwrap();
        assert!(!editor.recording);
        assert_eq!(editor.shortcuts[&Command::Search], "Return");
    }
    fn editor(root: &Path) -> Editor {
        let path = root.join("config.kdl");
        std::fs::write(&path, "schema-version 1\nnotifications { }\n").unwrap();
        Editor::new(
            window::Id::unique(),
            ConfigurationSnapshot::load(&canopa_config::Request::common(path)).unwrap(),
            root.to_owned(),
        )
    }
    #[test]
    fn draft_preserves_raw_bookmarks_and_uncommitted_text_across_pages_and_selection() {
        let root = tempfile::tempdir().unwrap();
        let mut e = editor(root.path());
        let raw = root
            .path()
            .join(std::ffi::OsString::from_vec(b"raw-\xff folder".to_vec()));
        e.add_bookmark(raw.clone());
        e.change(Change::Field(Field::BookmarkName, "Raw folder".into()));
        e.change(Change::Field(Field::Width, "421".into()));
        e.select_page(Page::Shortcuts);
        e.change(Change::Field(Field::Shortcut, "<Alt>f".into()));
        e.select_page(Page::Views);
        let s = e.settings(root.path()).unwrap();
        assert_eq!(s.view.column_width, 421);
        assert!(
            matches!(s.places.last(),Some(Place::Bookmark{name,path})if name=="Raw folder" && path==&raw)
        );
        assert_eq!(s.bindings(Command::Search), ["<Alt>f"]);
        e.select_page(Page::Locations);
        e.change(Change::AddBookmark);
        e.change(Change::Field(
            Field::BookmarkPath,
            "smb://remote/folder".into(),
        ));
        assert!(
            e.settings(root.path())
                .unwrap_err()
                .contains("absolute local")
        );
        e.change(Change::RemovePlace);
        assert_eq!(e.settings(root.path()).unwrap(), s);
    }
    #[test]
    fn width_and_shortcut_validation_happen_before_any_save_and_defaults_restore_bindings() {
        let root = tempfile::tempdir().unwrap();
        let mut e = editor(root.path());
        e.change(Change::Field(Field::Width, "12x".into()));
        assert!(e.settings(root.path()).unwrap_err().contains("160 to 640"));
        e.change(Change::Field(Field::Width, "640".into()));
        e.change(Change::Field(Field::Shortcut, "<Control>l".into()));
        assert!(
            e.settings(root.path())
                .unwrap_err()
                .contains("same shortcut")
        );
        e.change(Change::Field(Field::Shortcut, "Up".into()));
        assert!(
            e.settings(root.path())
                .unwrap_err()
                .contains("stay available")
        );
        e.change(Change::Field(Field::Shortcut, String::new()));
        assert!(
            e.settings(root.path())
                .unwrap()
                .bindings(Command::Search)
                .is_empty()
        );
        e.change(Change::ResetShortcut);
        let settings = e.settings(root.path()).unwrap();
        assert!(!settings.shortcuts.contains_key(&Command::Search));
    }
    #[test]
    fn stale_controls_cannot_edit_another_page_selection_or_reloaded_draft() {
        let root = tempfile::tempdir().unwrap();
        let (mut desktop, _) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let _ = desktop.show_preferences(root.path().to_owned(), false);
        let t = desktop.preferences_editor.as_ref().unwrap().target();
        let _ = desktop.preferences_event(Event::Page(t, Page::Locations));
        let _ =
            desktop.preferences_event(Event::Change(t, Change::Field(Field::Width, "500".into())));
        let editor = desktop.preferences_editor.as_ref().unwrap();
        assert_eq!(editor.width, "240");
        let original_locations = editor.locations.len();
        let fresh = editor.target();
        let _ = desktop.preferences_event(Event::Change(fresh, Change::AddBookmark));
        let _ = desktop.preferences_event(Event::Change(fresh, Change::RemovePlace));
        assert_eq!(
            desktop.preferences_editor.as_ref().unwrap().locations.len(),
            original_locations + 1
        );
        let _ = desktop.close_preferences(t.window, false);
        assert_eq!(
            desktop.preferences_editor.as_ref().unwrap().confirmation,
            Some(Confirmation::Close)
        );
        let _ = desktop.preferences_event(Event::Confirm(t.window, false));
        assert!(desktop.preferences_editor.as_ref().unwrap().dirty);
        let _ = desktop.preferences_event(Event::Reload(t.window));
        assert_eq!(
            desktop.preferences_editor.as_ref().unwrap().confirmation,
            Some(Confirmation::Reload)
        );
    }
    #[test]
    fn ordering_cannot_remove_location_or_duplicate_toolbar_items() {
        let root = tempfile::tempdir().unwrap();
        let mut e = editor(root.path());
        e.change(Change::SelectToolbar(3));
        e.change(Change::RemoveToolbar);
        assert_eq!(e.draft.toolbar[3], "location");
        e.change(Change::MoveToolbar(-1));
        assert_eq!(e.draft.toolbar[2], "location");
        e.change(Change::AddToolbar("refresh".into()));
        e.change(Change::AddToolbar("refresh".into()));
        assert_eq!(
            e.draft.toolbar.iter().filter(|s| *s == "refresh").count(),
            1
        );
        e.change(Change::AddToolbar("unknown".into()));
        assert!(e.settings(root.path()).is_ok());
        e.change(Change::RemoveToolbar);
        assert!(!e.draft.toolbar.contains(&"refresh".into()));
    }
}
