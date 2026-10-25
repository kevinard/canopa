//! A transactional editor for the Files-owned shared configuration.
use super::{Desktop, home};
use crate::{
    configuration::ConfigurationSnapshot,
    interface::{Command, InterfaceSettings, KeyChord, Place, PlaceKind, SidebarMode},
    preferences::{Detail, SortField, ViewMode},
};
use gtk::{gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::{Rc, Weak},
};

pub(super) fn initialize(app: &Rc<Desktop>) {
    let action = gio::SimpleAction::new("preferences", None);
    let weak = Rc::downgrade(app);
    action.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            show(&app);
        }
    });
    app.application.add_action(&action);
}
pub(super) fn show(app: &Rc<Desktop>) {
    if let Some(editor) = app.preference_editor.borrow().as_ref() {
        editor.native.present();
        return;
    }
    let editor = Editor::new(app);
    *app.preference_editor.borrow_mut() = Some(editor.clone());
    editor.native.present();
}
pub(super) fn bookmark(app: &Rc<Desktop>, path: Option<PathBuf>) {
    show(app);
    if let Some(editor) = app.preference_editor.borrow().clone() {
        editor.pages.set_current_page(Some(2));
        let path = path.unwrap_or_else(home);
        if editor.native.is_active() {
            editor.edit_bookmark(None, path);
        } else {
            // Presenting a child before the new parent receives activation can
            // leave keyboard input in the parent on a tiling compositor.
            let pending = RefCell::new(Some(path));
            let weak = Rc::downgrade(&editor);
            editor.native.connect_is_active_notify(move |window| {
                if window.is_active()
                    && let Some(path) = pending.borrow_mut().take()
                    && let Some(editor) = weak.upgrade()
                {
                    editor.edit_bookmark(None, path);
                }
            });
        }
    }
}

pub(super) struct Editor {
    pub native: gtk::ApplicationWindow,
    app: Weak<Desktop>,
    snapshot: RefCell<ConfigurationSnapshot>,
    draft: RefCell<InterfaceSettings>,
    pages: gtk::Notebook,
    mode: gtk::DropDown,
    sort: gtk::DropDown,
    reverse: gtk::CheckButton,
    width: gtk::SpinButton,
    details: Vec<(gtk::CheckButton, Detail)>,
    sidebar: gtk::DropDown,
    toolbar_names: gtk::StringList,
    toolbar_list: gtk::DropDown,
    toolbar_choices: RefCell<Vec<String>>,
    toolbar_add: gtk::DropDown,
    place_names: gtk::StringList,
    place_list: gtk::DropDown,
    place_add: gtk::DropDown,
    command: gtk::DropDown,
    shortcut_command: Cell<Command>,
    shortcut: gtk::Entry,
    syncing: Cell<bool>,
    busy: Cell<bool>,
    closing: Cell<bool>,
    status: gtk::Label,
}
impl Editor {
    pub(super) fn configuration_changed(&self) {
        if !self.busy.get()
            && let Some(app) = self.app.upgrade()
            && !self
                .snapshot
                .borrow()
                .same_source(&app.action_configuration.borrow())
        {
            self.status.set_label("Configuration changed outside this editor. Your draft is kept. Reload before saving.");
        }
    }
    fn new(app: &Rc<Desktop>) -> Rc<Self> {
        let snapshot = app.action_configuration.borrow().clone();
        let native = gtk::ApplicationWindow::builder()
            .application(&app.application)
            .title("Files Preferences")
            .default_width(520)
            .default_height(600)
            .build();
        native.add_css_class("canopa-files");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        for set in [
            gtk::Widget::set_margin_top,
            gtk::Widget::set_margin_bottom,
            gtk::Widget::set_margin_start,
            gtk::Widget::set_margin_end,
        ] {
            set(root.upcast_ref(), 16);
        }
        let pages = gtk::Notebook::new();
        pages.set_scrollable(true);
        pages.set_vexpand(true);
        root.append(&pages);
        let views = page(&pages, "Views");
        note(
            &views,
            "Defaults apply to folders unless you deliberately change that setting there. Use defaults in a folder clears its remembered choices.",
        );
        let mode = gtk::DropDown::from_strings(&["Columns", "List", "Icons"]);
        field(&views, "Default view", &mode);
        let sort = gtk::DropDown::from_strings(&["Name", "Size", "Modified", "Kind"]);
        field(&views, "Sort by", &sort);
        let reverse = gtk::CheckButton::with_label("Reverse order");
        views.append(&reverse);
        let width = gtk::SpinButton::with_range(160.0, 640.0, 20.0);
        field(&views, "Column width", &width);
        let mut details = Vec::new();
        for (label, detail) in [
            ("Show size", Detail::Size),
            ("Show modified time", Detail::Modified),
            ("Show kind", Detail::Kind),
        ] {
            let check = gtk::CheckButton::with_label(label);
            views.append(&check);
            details.push((check, detail));
        }
        let toolbar = page(&pages, "Toolbar");
        note(
            &toolbar,
            "Add, remove and reorder commands. The location field stays available; the menu always gives access to Preferences and Locations. Long button groups scroll in a narrow window.",
        );
        let toolbar_names = gtk::StringList::new(&[]);
        let toolbar_list = gtk::DropDown::new(Some(toolbar_names.clone()), None::<gtk::Expression>);
        field(&toolbar, "Toolbar item", &toolbar_list);
        let (toolbar_up, toolbar_down, toolbar_remove) = order_buttons(&toolbar);
        let mut toolbar_choices: Vec<_> = Command::ALL.iter().map(|c| c.id().to_owned()).collect();
        let mut labels: Vec<_> = Command::ALL.iter().map(|c| c.label().to_owned()).collect();
        for action in &snapshot.actions {
            toolbar_choices.push(format!("action:{}", action.id));
            labels.push(format!("Action: {}", action.name));
        }
        let toolbar_add =
            gtk::DropDown::from_strings(&labels.iter().map(String::as_str).collect::<Vec<_>>());
        toolbar_add.set_enable_search(true);
        toolbar_add.set_expression(Some(gtk::PropertyExpression::new(
            gtk::StringObject::static_type(),
            None::<gtk::Expression>,
            "string",
        )));
        field(&toolbar, "Available command", &toolbar_add);
        let add_command = gtk::Button::with_mnemonic("Add co_mmand");
        toolbar.append(&add_command);
        let reset_toolbar = gtk::Button::with_label("Restore default toolbar");
        toolbar.append(&reset_toolbar);
        let places = page(&pages, "Locations");
        note(
            &places,
            "Locations appear in this order in the sidebar and in the menu. Bookmarks refer to local folders; their names can differ from folder names.",
        );
        let sidebar =
            gtk::DropDown::from_strings(&["Automatic", "Show when there is room", "Hidden"]);
        field(&places, "Sidebar", &sidebar);
        let place_names = gtk::StringList::new(&[]);
        let place_list = gtk::DropDown::new(Some(place_names.clone()), None::<gtk::Expression>);
        field(&places, "Location", &place_list);
        let (place_up, place_down, place_remove) = order_buttons(&places);
        let edit_place = gtk::Button::with_mnemonic("_Edit bookmark…");
        places.append(&edit_place);
        let add_bookmark = gtk::Button::with_mnemonic("Add _bookmark…");
        places.append(&add_bookmark);
        let place_add = gtk::DropDown::from_strings(
            &PlaceKind::ALL.iter().map(|p| p.label()).collect::<Vec<_>>(),
        );
        field(&places, "Built-in location", &place_add);
        let add_builtin = gtk::Button::with_mnemonic("Add l_ocation");
        places.append(&add_builtin);
        let keys = page(&pages, "Shortcuts");
        note(
            &keys,
            "Choose a browser command and edit its shortcuts, separated by semicolons. For example: <Control>f; F6. An empty field disables the command. Text fields retain their standard editing keys. Arrow selection, Escape and the context-menu key remain available.",
        );
        let command = gtk::DropDown::from_strings(
            &Command::ALL.iter().map(|c| c.label()).collect::<Vec<_>>(),
        );
        field(&keys, "Command", &command);
        let shortcut = gtk::Entry::new();
        shortcut.set_width_chars(1);
        shortcut.set_hexpand(true);
        field(&keys, "Shortcuts", &shortcut);
        let record = gtk::Button::with_label("Record shortcut…");
        keys.append(&record);
        let reset_shortcut = gtk::Button::with_label("Use default shortcut");
        keys.append(&reset_shortcut);
        let status = gtk::Label::new(None);
        status.set_wrap(true);
        status.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        status.set_xalign(0.0);
        root.append(&status);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let reload = gtk::Button::with_label("Reload");
        let save = gtk::Button::with_label("Save");
        save.add_css_class("suggested-action");
        buttons.append(&reload);
        buttons.append(&save);
        root.append(&buttons);
        native.set_child(Some(&root));
        let editor = Rc::new(Self {
            native,
            app: Rc::downgrade(app),
            draft: RefCell::new(snapshot.settings.clone()),
            snapshot: RefCell::new(snapshot),
            pages,
            mode,
            sort,
            reverse,
            width,
            details,
            sidebar,
            toolbar_names,
            toolbar_list,
            toolbar_choices: RefCell::new(toolbar_choices),
            toolbar_add,
            place_names,
            place_list,
            place_add,
            command,
            shortcut_command: Cell::new(Command::Back),
            shortcut,
            syncing: Cell::new(false),
            busy: Cell::new(false),
            closing: Cell::new(false),
            status,
        });
        editor.populate();
        for (button, group, delta) in [
            (toolbar_up, true, -1),
            (toolbar_down, true, 1),
            (toolbar_remove, true, 0),
            (place_up, false, -1),
            (place_down, false, 1),
            (place_remove, false, 0),
        ] {
            let weak = Rc::downgrade(&editor);
            button.connect_clicked(move |_| {
                if let Some(editor) = weak.upgrade() {
                    editor.reorder(group, delta);
                }
            });
        }
        let weak = Rc::downgrade(&editor);
        add_command.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade()
                && let Some(id) = editor
                    .toolbar_choices
                    .borrow()
                    .get(editor.toolbar_add.selected() as usize)
            {
                if editor.draft.borrow().toolbar.contains(id) {
                    editor
                        .status
                        .set_label("That command is already in the toolbar.");
                    return;
                }
                editor.draft.borrow_mut().toolbar.push(id.clone());
                editor.rebuild_toolbar(None);
            }
        });
        let weak = Rc::downgrade(&editor);
        reset_toolbar.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.draft.borrow_mut().toolbar = InterfaceSettings::default().toolbar;
                editor.rebuild_toolbar(Some(0));
            }
        });
        let weak = Rc::downgrade(&editor);
        add_builtin.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade()
                && let Some(kind) = PlaceKind::ALL.get(editor.place_add.selected() as usize)
            {
                let place = Place::Builtin(*kind);
                if editor.draft.borrow().places.contains(&place) {
                    editor.status.set_label("That location is already listed.");
                    return;
                }
                editor.draft.borrow_mut().places.push(place);
                editor.rebuild_places(None);
            }
        });
        let weak = Rc::downgrade(&editor);
        add_bookmark.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.edit_bookmark(None, home());
            }
        });
        let weak = Rc::downgrade(&editor);
        edit_place.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                let index = editor.place_list.selected() as usize;
                let place = editor.draft.borrow().places.get(index).cloned();
                if let Some(Place::Bookmark { path, .. }) = place {
                    editor.edit_bookmark(Some(index), path);
                } else {
                    editor
                        .status
                        .set_label("Select a bookmark to edit its name or folder.");
                }
            }
        });
        let weak = Rc::downgrade(&editor);
        editor.command.connect_selected_notify(move |choice| {
            if let Some(editor) = weak.upgrade()
                && !editor.syncing.get()
            {
                editor.capture_shortcut();
                editor
                    .shortcut_command
                    .set(Command::ALL[choice.selected() as usize]);
                editor.show_shortcut();
            }
        });
        let weak = Rc::downgrade(&editor);
        reset_shortcut.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor
                    .draft
                    .borrow_mut()
                    .shortcuts
                    .remove(&editor.shortcut_command.get());
                editor.show_shortcut();
            }
        });
        let weak = Rc::downgrade(&editor);
        record.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.record_shortcut();
            }
        });
        let weak = Rc::downgrade(&editor);
        save.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.save();
            }
        });
        let weak = Rc::downgrade(&editor);
        reload.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.confirm_reload();
            }
        });
        let weak = Rc::downgrade(&editor);
        editor.native.connect_close_request(move |_| {
            let Some(editor) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if editor.busy.get() {
                return glib::Propagation::Stop;
            }
            if !editor.closing.get() && editor.dirty() {
                let dialog = gtk::AlertDialog::builder()
                    .message("Discard unsaved preference changes?")
                    .buttons(["Keep editing", "Discard"])
                    .cancel_button(0)
                    .default_button(0)
                    .build();
                glib::MainContext::default().spawn_local(async move {
                    if dialog.choose_future(Some(&editor.native)).await == Ok(1) {
                        editor.closing.set(true);
                        editor.native.close();
                    }
                });
                return glib::Propagation::Stop;
            }
            if let Some(app) = editor.app.upgrade() {
                app.preference_editor.borrow_mut().take();
            }
            glib::Propagation::Proceed
        });
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&editor);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(editor) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if modifiers.contains(gtk::gdk::ModifierType::ALT_MASK) {
                let page = match key {
                    gtk::gdk::Key::_1 => Some(0),
                    gtk::gdk::Key::_2 => Some(1),
                    gtk::gdk::Key::_3 => Some(2),
                    gtk::gdk::Key::_4 => Some(3),
                    _ => None,
                };
                if let Some(page) = page {
                    editor.pages.set_current_page(Some(page));
                    return glib::Propagation::Stop;
                }
            }
            if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                match key.to_lower() {
                    gtk::gdk::Key::s => {
                        editor.save();
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::r => {
                        editor.confirm_reload();
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }
            }
            if key == gtk::gdk::Key::Escape {
                editor.native.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        editor.native.add_controller(keys);
        editor
    }
    fn populate(&self) {
        self.syncing.set(true);
        let draft = self.draft.borrow();
        self.mode.set_selected(match draft.view.mode {
            ViewMode::Columns => 0,
            ViewMode::List => 1,
            ViewMode::Icons => 2,
        });
        self.sort.set_selected(match draft.view.sort {
            SortField::Name => 0,
            SortField::Size => 1,
            SortField::Modified => 2,
            SortField::Kind => 3,
        });
        self.reverse.set_active(draft.view.descending);
        self.width.set_value(f64::from(draft.view.column_width));
        for (check, detail) in &self.details {
            check.set_active(draft.view.details.contains(detail));
        }
        self.sidebar.set_selected(match draft.sidebar {
            SidebarMode::Auto => 0,
            SidebarMode::Always => 1,
            SidebarMode::Never => 2,
        });
        drop(draft);
        self.rebuild_toolbar_choices();
        self.rebuild_toolbar(Some(0));
        self.rebuild_places(Some(0));
        self.show_shortcut();
        self.syncing.set(false);
    }
    fn capture_shortcut(&self) {
        let command = self.shortcut_command.get();
        let bindings: Vec<_> = self
            .shortcut
            .text()
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let mut draft = self.draft.borrow_mut();
        if bindings
            .iter()
            .map(String::as_str)
            .eq(command.defaults().iter().copied())
        {
            draft.shortcuts.remove(&command);
        } else {
            draft.shortcuts.insert(command, bindings);
        }
    }
    fn show_shortcut(&self) {
        self.shortcut.set_text(
            &self
                .draft
                .borrow()
                .bindings(self.shortcut_command.get())
                .join("; "),
        );
    }
    fn capture(&self) -> InterfaceSettings {
        self.capture_shortcut();
        // Ctrl+S may arrive while the spin button still owns uncommitted text.
        self.width.update();
        let mut draft = self.draft.borrow_mut();
        draft.view.mode = match self.mode.selected() {
            1 => ViewMode::List,
            2 => ViewMode::Icons,
            _ => ViewMode::Columns,
        };
        draft.view.sort = match self.sort.selected() {
            1 => SortField::Size,
            2 => SortField::Modified,
            3 => SortField::Kind,
            _ => SortField::Name,
        };
        draft.view.descending = self.reverse.is_active();
        draft.view.column_width = self.width.value_as_int() as u16;
        let checked: Vec<_> = self
            .details
            .iter()
            .filter(|(check, _)| check.is_active())
            .map(|(_, detail)| *detail)
            .collect();
        // Preserve an existing detail order unless the user actually changes its membership.
        if checked.len() != draft.view.details.len()
            || checked.iter().any(|d| !draft.view.details.contains(d))
        {
            draft.view.details = checked;
        }
        draft.sidebar = match self.sidebar.selected() {
            1 => SidebarMode::Always,
            2 => SidebarMode::Never,
            _ => SidebarMode::Auto,
        };
        draft.clone()
    }
    fn dirty(&self) -> bool {
        self.capture() != self.snapshot.borrow().settings
    }
    fn rebuild_toolbar_choices(&self) {
        let mut ids: Vec<_> = Command::ALL
            .iter()
            .map(|command| command.id().to_owned())
            .collect();
        let mut names: Vec<_> = Command::ALL
            .iter()
            .map(|command| command.label().to_owned())
            .collect();
        for action in &self.snapshot.borrow().actions {
            ids.push(format!("action:{}", action.id));
            names.push(format!("Action: {}", action.name));
        }
        self.toolbar_add.set_model(Some(&gtk::StringList::new(
            &names.iter().map(String::as_str).collect::<Vec<_>>(),
        )));
        *self.toolbar_choices.borrow_mut() = ids;
    }
    fn rebuild_toolbar(&self, selected: Option<usize>) {
        let snapshot = self.snapshot.borrow();
        let names: Vec<String> = self
            .draft
            .borrow()
            .toolbar
            .iter()
            .map(|id| {
                if id == "location" {
                    "Location field".into()
                } else if let Some(command) = Command::from_id(id) {
                    command.label().into()
                } else {
                    id.strip_prefix("action:")
                        .and_then(|id| snapshot.actions.iter().find(|a| a.id == id))
                        .map(|a| format!("Action: {}", a.name))
                        .unwrap_or_else(|| id.clone())
                }
            })
            .collect();
        replace_names(&self.toolbar_names, &self.toolbar_list, names, selected);
    }
    fn rebuild_places(&self, selected: Option<usize>) {
        let names = self
            .draft
            .borrow()
            .places
            .iter()
            .map(|place| place.label().to_owned())
            .collect();
        replace_names(&self.place_names, &self.place_list, names, selected);
    }
    fn reorder(&self, toolbar: bool, delta: isize) {
        let index = if toolbar {
            self.toolbar_list.selected()
        } else {
            self.place_list.selected()
        } as usize;
        let mut draft = self.draft.borrow_mut();
        let count = if toolbar {
            draft.toolbar.len()
        } else {
            draft.places.len()
        };
        if index >= count {
            return;
        }
        let next = index.saturating_add_signed(delta).min(count - 1);
        if delta == 0 {
            if toolbar {
                if draft.toolbar[index] == "location" {
                    self.status
                        .set_label("The location field must remain available.");
                    return;
                }
                draft.toolbar.remove(index);
            } else {
                draft.places.remove(index);
            }
        } else if toolbar {
            draft.toolbar.swap(index, next);
        } else {
            draft.places.swap(index, next);
        }
        drop(draft);
        if toolbar {
            self.rebuild_toolbar(Some(next));
        } else {
            self.rebuild_places(Some(next));
        }
    }
    fn edit_bookmark(self: &Rc<Self>, index: Option<usize>, initial: PathBuf) {
        if self.busy.get() {
            return;
        }
        let window = gtk::Window::builder()
            .title("Bookmark")
            .transient_for(&self.native)
            .modal(true)
            .default_width(400)
            .build();
        let panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
        panel.set_margin_top(16);
        panel.set_margin_bottom(16);
        panel.set_margin_start(16);
        panel.set_margin_end(16);
        let name = gtk::Entry::new();
        name.set_width_chars(1);
        let initial_name = index
            .and_then(|i| {
                self.draft
                    .borrow()
                    .places
                    .get(i)
                    .map(|p| p.label().to_owned())
            })
            .unwrap_or_else(|| {
                initial
                    .file_name()
                    .unwrap_or(initial.as_os_str())
                    .to_string_lossy()
                    .into_owned()
            });
        name.set_text(&initial_name);
        field(&panel, "Name", &name);
        let path = Rc::new(RefCell::new(initial));
        let folder = gtk::Button::with_label("Choose folder…");
        panel.append(&folder);
        let selected = gtk::Label::new(Some(&path.borrow().display().to_string()));
        selected.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        selected.set_width_chars(1);
        panel.append(&selected);
        let error = gtk::Label::new(None);
        error.set_wrap(true);
        panel.append(&error);
        let save = gtk::Button::with_mnemonic("_Use bookmark");
        panel.append(&save);
        window.set_child(Some(&panel));
        let weak_window = window.downgrade();
        let current = path.clone();
        folder.connect_clicked(move |_| {
            let Some(window) = weak_window.upgrade() else {
                return;
            };
            let dialog = gtk::FileDialog::builder()
                .title("Bookmark folder")
                .initial_folder(&gio::File::for_path(&*current.borrow()))
                .build();
            let current = current.clone();
            let selected = selected.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Ok(file) = dialog.select_folder_future(Some(&window)).await
                    && let Some(path) = file.path()
                {
                    selected.set_label(&path.display().to_string());
                    *current.borrow_mut() = path;
                }
            });
        });
        let weak = Rc::downgrade(self);
        let dialog = window.downgrade();
        let saved_name = name.clone();
        save.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                let name = &saved_name;
                let place = Place::Bookmark {
                    name: name.text().trim().to_owned(),
                    path: path.borrow().clone(),
                };
                let mut candidate = editor.draft.borrow().clone();
                if let Some(index) = index {
                    if index < candidate.places.len() {
                        candidate.places[index] = place;
                    } else {
                        return;
                    }
                } else {
                    candidate.places.push(place);
                }
                if let Err(message) = candidate.validate(&editor.snapshot.borrow().actions) {
                    error.set_label(&message.to_string());
                    return;
                }
                *editor.draft.borrow_mut() = candidate;
                editor.rebuild_places(index);
                if let Some(window) = dialog.upgrade() {
                    window.close();
                }
            }
        });
        window.present();
        name.grab_focus();
        name.select_region(0, -1);
    }
    fn record_shortcut(self: &Rc<Self>) {
        let window = gtk::Window::builder()
            .title("Record Shortcut")
            .transient_for(&self.native)
            .modal(true)
            .default_width(360)
            .default_height(120)
            .build();
        let label = gtk::Label::new(Some(
            "Press a shortcut. Escape cancels; Backspace disables.",
        ));
        label.set_wrap(true);
        window.set_child(Some(&label));
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        let dialog = window.downgrade();
        keys.connect_key_pressed(move |_, key, _, mods| {
            if let (Some(editor), Some(dialog)) = (weak.upgrade(), dialog.upgrade()) {
                if key == gtk::gdk::Key::Escape {
                    dialog.close();
                } else if key == gtk::gdk::Key::BackSpace && mods.is_empty() {
                    editor.shortcut.set_text("");
                    dialog.close();
                } else if let Some(chord) = super::key_chord(key, mods) {
                    editor.shortcut.set_text(&chord.accelerator());
                    dialog.close();
                }
            }
            glib::Propagation::Stop
        });
        window.add_controller(keys);
        window.present();
    }
    fn set_busy(&self, busy: bool) {
        self.busy.set(busy);
        if busy {
            GtkWindowExt::set_focus(&self.native, None::<&gtk::Widget>);
        }
        if let Some(child) = self.native.child() {
            child.set_sensitive(!busy);
        }
    }
    fn save(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        let mut settings = self.capture();
        for bindings in settings.shortcuts.values_mut() {
            for binding in bindings {
                match KeyChord::parse(binding) {
                    Ok(chord) => *binding = chord.accelerator(),
                    Err(error) => {
                        self.status.set_label(&error.to_string());
                        return;
                    }
                }
            }
        }
        let snapshot = self.snapshot.borrow().clone();
        if let Err(error) = settings.validate(&snapshot.actions) {
            self.status.set_label(&error.to_string());
            return;
        }
        self.set_busy(true);
        self.status.set_label("Saving…");
        let editor = self.clone();
        let hold = self.app.upgrade().map(|app| app.application.hold());
        glib::MainContext::default().spawn_local(async move {
            let _hold = hold;
            let result = gio::spawn_blocking(move || snapshot.save_settings(settings)).await;
            editor.set_busy(false);
            match result {
                Ok(Ok(snapshot)) => {
                    *editor.draft.borrow_mut() = snapshot.settings.clone();
                    *editor.snapshot.borrow_mut() = snapshot.clone();
                    editor.populate();
                    editor
                        .status
                        .set_label("Saved. Preferences apply to all Files windows.");
                    if let Some(app) = editor.app.upgrade() {
                        app.install_configuration(snapshot);
                    }
                }
                Ok(Err(error)) => editor
                    .status
                    .set_label(&format!("Could not save preferences: {error}")),
                Err(_) => editor.status.set_label("The configuration writer failed."),
            }
        });
    }
    fn confirm_reload(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        if !self.dirty() {
            self.reload();
            return;
        }
        let dialog = gtk::AlertDialog::builder()
            .message("Reload and discard unsaved preference changes?")
            .buttons(["Keep editing", "Reload"])
            .cancel_button(0)
            .default_button(0)
            .build();
        let editor = self.clone();
        glib::MainContext::default().spawn_local(async move {
            if dialog.choose_future(Some(&editor.native)).await == Ok(1) {
                editor.reload();
            }
        });
    }
    fn reload(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        self.set_busy(true);
        self.status.set_label("Loading…");
        let snapshot = self.snapshot.borrow().clone();
        let editor = self.clone();
        let hold = self.app.upgrade().map(|app| app.application.hold());
        glib::MainContext::default().spawn_local(async move {
            let _hold = hold;
            let result = gio::spawn_blocking(move || snapshot.reload()).await;
            editor.set_busy(false);
            match result {
                Ok(Ok(snapshot)) => {
                    *editor.draft.borrow_mut() = snapshot.settings.clone();
                    *editor.snapshot.borrow_mut() = snapshot.clone();
                    editor.populate();
                    editor
                        .status
                        .set_label("Loaded shared Files configuration.");
                    if let Some(app) = editor.app.upgrade() {
                        app.install_configuration(snapshot);
                    }
                }
                Ok(Err(error)) => editor
                    .status
                    .set_label(&format!("Could not reload preferences: {error}")),
                Err(_) => editor.status.set_label("The configuration reader failed."),
            }
        });
    }
}
fn replace_names(
    model: &gtk::StringList,
    choice: &gtk::DropDown,
    names: Vec<String>,
    selected: Option<usize>,
) {
    model.splice(
        0,
        model.n_items(),
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    choice.set_selected(if names.is_empty() {
        gtk::INVALID_LIST_POSITION
    } else {
        selected.unwrap_or(names.len() - 1).min(names.len() - 1) as u32
    });
}
fn page(pages: &gtk::Notebook, name: &str) -> gtk::Box {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
    panel.set_margin_top(12);
    panel.set_margin_bottom(12);
    panel.set_margin_start(12);
    panel.set_margin_end(12);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&panel)
        .build();
    pages.append_page(&scroll, Some(&gtk::Label::new(Some(name))));
    panel
}
fn field(panel: &gtk::Box, text: &str, widget: &impl IsA<gtk::Widget>) {
    let mnemonic = match text {
        "Default view" => "Default _view",
        "Sort by" => "_Sort by",
        "Column width" => "_Column width",
        "Toolbar item" => "_Toolbar item",
        "Available command" => "_Available command",
        "Sidebar" => "_Sidebar",
        "Location" => "_Location",
        "Built-in location" => "_Built-in location",
        "Command" => "_Command",
        "Shortcuts" => "_Shortcuts",
        "Name" => "_Name",
        _ => text,
    };
    let label = gtk::Label::with_mnemonic(mnemonic);
    label.set_mnemonic_widget(Some(widget));
    label.set_xalign(0.0);
    panel.append(&label);
    widget
        .as_ref()
        .update_property(&[gtk::accessible::Property::Label(text)]);
    panel.append(widget);
}
fn note(panel: &gtk::Box, text: &str) {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    label.set_width_chars(1);
    panel.append(&label);
}
fn order_buttons(panel: &gtk::Box) -> (gtk::Button, gtk::Button, gtk::Button) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let up = gtk::Button::with_mnemonic("_Up");
    let down = gtk::Button::with_mnemonic("_Down");
    let remove = gtk::Button::with_mnemonic("_Remove");
    for button in [&up, &down, &remove] {
        row.append(button);
    }
    panel.append(&row);
    (up, down, remove)
}
