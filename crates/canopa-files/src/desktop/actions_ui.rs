//! Selection-aware command surfaces and the graphical custom-action editor.
use super::{Desktop, clear_box, icon_button, tab::Tab};
use crate::{actions::CustomAction, configuration::ConfigurationSnapshot};
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub(super) fn initialize(app: &Rc<Desktop>) {
    let action = gio::SimpleAction::new("custom-actions", None);
    let weak = Rc::downgrade(app);
    action.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            show(&app);
        }
    });
    app.application.add_action(&action);
}

pub(super) fn show(app: &Rc<Desktop>) {
    if let Some(editor) = app.action_editor.borrow().as_ref() {
        editor.native.present();
        return;
    }
    let editor = Editor::new(app);
    *app.action_editor.borrow_mut() = Some(editor.clone());
    editor.native.present();
    editor.reload();
}

fn app(tab: &Tab) -> Option<Rc<Desktop>> {
    tab.window()?.app.upgrade()
}

pub(super) fn menu(tab: &Rc<Tab>, panel: &gtk::Box, popover: &gtk::Popover) {
    let weak = Rc::downgrade(tab);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
    panel.append(&content);
    popover.connect_show(move |popover| {
        clear_box(&content);
        if let Some(tab) = weak.upgrade() {
            append_actions(&tab, &content, popover);
        }
    });
}

fn append_actions(tab: &Rc<Tab>, panel: &gtk::Box, popover: &gtk::Popover) {
    let Some(app) = app(tab) else { return };
    let entries = tab.selection_entries();
    for action in app
        .action_configuration
        .borrow()
        .actions
        .iter()
        .filter(|action| action.is_applicable(&entries))
    {
        let label = action
            .shortcut
            .as_ref()
            .and_then(gtk::accelerator_parse)
            .map(|(key, mods)| {
                format!(
                    "{} · {}",
                    action.name,
                    gtk::accelerator_get_label(key, mods)
                )
            })
            .unwrap_or_else(|| action.name.clone());
        let button = gtk::Button::with_label(&label);
        button.add_css_class("flat");
        let weak = Rc::downgrade(tab);
        let action = action.clone();
        let popover = popover.downgrade();
        button.connect_clicked(move |_| {
            if let Some(popover) = popover.upgrade() {
                popover.popdown();
            }
            if let Some(tab) = weak.upgrade() {
                run(&tab, action.clone());
            }
        });
        panel.append(&button);
    }
    let button = gtk::Button::with_label("Custom actions…");
    button.set_tooltip_text(Some(
        "Edit commands, conditions, shortcuts and toolbar actions",
    ));
    let weak = Rc::downgrade(&app);
    let popover = popover.downgrade();
    button.connect_clicked(move |_| {
        if let Some(popover) = popover.upgrade() {
            popover.popdown();
        }
        if let Some(app) = weak.upgrade() {
            show(&app);
        }
    });
    panel.append(&button);
}

fn run(tab: &Rc<Tab>, action: CustomAction) {
    if let Some(app) = app(tab) {
        super::jobs::custom_action(&app, tab, action);
    }
}

pub(super) fn toolbar(tab: &Rc<Tab>, container: &gtk::Box) {
    clear_box(container);
    let Some(app) = app(tab) else { return };
    for action in app
        .action_configuration
        .borrow()
        .actions
        .iter()
        .filter(|action| {
            action.toolbar
                && !app
                    .action_configuration
                    .borrow()
                    .settings
                    .toolbar
                    .contains(&format!("action:{}", action.id))
        })
    {
        let button = action_button(tab, action);
        container.append(&button);
    }
    container.set_visible(container.first_child().is_some());
}

pub(super) fn action_button(tab: &Rc<Tab>, action: &CustomAction) -> gtk::Button {
    let button = icon_button(
        action.icon.as_deref().unwrap_or("system-run-symbolic"),
        &action.name,
    );
    button.set_sensitive(action.is_applicable(&tab.selection_entries()));
    let weak = Rc::downgrade(tab);
    let action = action.clone();
    button.connect_clicked(move |_| {
        if let Some(tab) = weak.upgrade() {
            run(&tab, action.clone());
        }
    });
    button
}

pub(super) fn shortcut(tab: &Rc<Tab>, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
    let Some(app) = app(tab) else { return false };
    let Some(chord) = super::key_chord(key, modifiers) else {
        return false;
    };
    let action = app
        .action_configuration
        .borrow()
        .actions
        .iter()
        .find(|action| {
            action
                .shortcut
                .as_ref()
                .and_then(|binding| crate::interface::KeyChord::parse(binding).ok())
                .is_some_and(|binding| binding == chord)
        })
        .cloned();
    if let Some(action) = action {
        if action.is_applicable(&tab.selection_entries()) {
            run(tab, action);
        } else {
            tab.show_error("This custom action does not apply to the current selection.");
        }
        true
    } else {
        false
    }
}

pub(super) fn context(tab: &Rc<Tab>, widget: &gtk::Widget, x: f64, y: f64) {
    let popover = gtk::Popover::new();
    popover.set_parent(widget);
    popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 4);
    panel.set_margin_start(8);
    panel.set_margin_end(8);
    panel.set_margin_top(8);
    panel.set_margin_bottom(8);
    for (label, command) in [
        ("Cut", 0),
        ("Copy", 1),
        ("Paste", 2),
        ("Rename…", 3),
        ("Move to Trash", 4),
    ] {
        let button = gtk::Button::with_label(label);
        button.add_css_class("flat");
        button.set_sensitive(command == 2 || !tab.selection_paths().is_empty());
        let weak = Rc::downgrade(tab);
        let weak_popover = popover.downgrade();
        button.connect_clicked(move |_| {
            if let Some(popover) = weak_popover.upgrade() {
                popover.popdown();
            }
            if let Some(tab) = weak.upgrade() {
                match command {
                    0 => super::clipboard::copy(&tab, true),
                    1 => super::clipboard::copy(&tab, false),
                    2 => super::clipboard::paste(&tab),
                    3 => super::operations_ui::prompt(&tab, true),
                    _ => super::trash_ui::move_selection(&tab),
                }
            }
        });
        panel.append(&button);
    }
    panel.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    append_actions(tab, &panel, &popover);
    popover.set_child(Some(&panel));
    popover.connect_closed(|popover| popover.unparent());
    popover.popup();
}

pub(super) struct Editor {
    pub native: gtk::ApplicationWindow,
    app: Weak<Desktop>,
    snapshot: RefCell<ConfigurationSnapshot>,
    actions: RefCell<Vec<CustomAction>>,
    selected: Cell<Option<usize>>,
    syncing: Cell<bool>,
    dirty: Cell<bool>,
    closing: Cell<bool>,
    busy: Cell<bool>,
    chooser: gtk::DropDown,
    names: gtk::StringList,
    name: gtk::Entry,
    icon: gtk::Entry,
    command: gtk::Entry,
    patterns: gtk::Entry,
    files: gtk::CheckButton,
    directories: gtk::CheckButton,
    minimum: gtk::SpinButton,
    maximum: gtk::SpinButton,
    shortcut: gtk::Entry,
    toolbar: gtk::CheckButton,
    status: gtk::Label,
    save: gtk::Button,
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
        let native = gtk::ApplicationWindow::builder()
            .application(&app.application)
            .title("Custom Actions — Canopa Files")
            .default_width(720)
            .default_height(700)
            .build();
        native.add_css_class("canopa-files");
        native.set_titlebar(Some(&gtk::HeaderBar::new()));
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.set_margin_start(20);
        body.set_margin_end(20);
        body.set_margin_top(16);
        body.set_margin_bottom(16);
        let heading = gtk::Label::new(Some("Run your commands on selected files and folders."));
        heading.set_xalign(0.0);
        heading.set_wrap(true);
        body.append(&heading);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let names = gtk::StringList::new(&[]);
        let chooser = gtk::DropDown::new(Some(names.clone()), None::<gtk::Expression>);
        chooser.set_hexpand(true);
        chooser.update_property(&[gtk::accessible::Property::Label("Custom action")]);
        row.append(&chooser);
        let add = gtk::Button::with_mnemonic("_Add action");
        let remove = gtk::Button::with_mnemonic("_Delete");
        let up = icon_button("go-up-symbolic", "Move action earlier");
        let down = icon_button("go-down-symbolic", "Move action later");
        for button in [&add, &remove, &up, &down] {
            row.append(button);
        }
        body.append(&row);
        let name = field(&body, "_Name", "Resize images");
        let icon = field(&body, "_Icon (optional)", "image-x-generic-symbolic");
        let command = field(&body, "_Command", "my-script %F");
        let help = gtk::Label::new(Some(
            "%f first path · %F all paths · %n first name · %N all names\n%d current folder · %% percent\nQuote arguments with spaces. Commands run directly; call a shell explicitly when needed.",
        ));
        help.set_wrap(true);
        help.set_xalign(0.0);
        help.add_css_class("dim-label");
        body.append(&help);
        let patterns = field(
            &body,
            "Filename _patterns (optional, separated by ;)",
            "*.png; *.jpg; *.jpeg",
        );
        let kinds = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let files = gtk::CheckButton::with_label("Files");
        let directories = gtk::CheckButton::with_label("Folders");
        kinds.append(&files);
        kinds.append(&directories);
        body.append(&kinds);
        let counts = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let minimum = gtk::SpinButton::with_range(0.0, 1_000_000.0, 1.0);
        let maximum = gtk::SpinButton::with_range(0.0, 1_000_000.0, 1.0);
        for (label, spin) in [("Minimum items", &minimum), ("Maximum (0 = any)", &maximum)] {
            let group = gtk::Box::new(gtk::Orientation::Vertical, 4);
            let label = gtk::Label::new(Some(label));
            label.set_xalign(0.0);
            group.append(&label);
            group.append(spin);
            counts.append(&group);
        }
        body.append(&counts);
        let shortcut = field(&body, "_Shortcut (optional)", "<Control><Alt>i");
        let record = gtk::Button::with_label("Record shortcut…");
        body.append(&record);
        let toolbar = gtk::CheckButton::with_mnemonic("Toolbar _button");
        body.append(&toolbar);
        let note = gtk::Label::new(Some(
            "Actions run only when invoked. Their effects are controlled by the command and cannot be undone by Files.",
        ));
        note.set_wrap(true);
        note.set_xalign(0.0);
        note.add_css_class("dim-label");
        body.append(&note);
        let status = gtk::Label::new(None);
        status.set_wrap(true);
        status.set_xalign(0.0);
        status.set_selectable(true);
        body.append(&status);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let reload = gtk::Button::with_mnemonic("_Reload");
        let save = gtk::Button::with_mnemonic("_Save actions");
        save.add_css_class("suggested-action");
        buttons.append(&reload);
        buttons.append(&save);
        body.append(&buttons);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&body)
            .build();
        native.set_child(Some(&scroll));
        let snapshot = app.action_configuration.borrow().clone();
        let editor = Rc::new(Self {
            native,
            app: Rc::downgrade(app),
            actions: RefCell::new(snapshot.actions.clone()),
            snapshot: RefCell::new(snapshot),
            selected: Cell::new(None),
            syncing: Cell::new(false),
            dirty: Cell::new(false),
            closing: Cell::new(false),
            busy: Cell::new(false),
            chooser,
            names,
            name,
            icon,
            command,
            patterns,
            files,
            directories,
            minimum,
            maximum,
            shortcut,
            toolbar,
            status,
            save,
        });
        let weak = Rc::downgrade(&editor);
        editor.chooser.connect_selected_notify(move |chooser| {
            if let Some(editor) = weak.upgrade()
                && !editor.syncing.get()
            {
                editor.capture();
                editor.select(
                    (chooser.selected() != gtk::INVALID_LIST_POSITION)
                        .then_some(chooser.selected() as usize),
                );
            }
        });
        let weak = Rc::downgrade(&editor);
        add.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.capture();
                let id = format!("action-{}", glib::uuid_string_random());
                editor
                    .actions
                    .borrow_mut()
                    .push(CustomAction::new(id, "New action", ""));
                editor.dirty.set(true);
                let index = editor.actions.borrow().len() - 1;
                editor.rebuild(Some(index));
                editor.name.grab_focus();
                editor.name.select_region(0, -1);
            }
        });
        let weak = Rc::downgrade(&editor);
        remove.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade()
                && let Some(index) = editor.selected.get()
            {
                editor.actions.borrow_mut().remove(index);
                editor.dirty.set(true);
                editor.selected.set(None);
                let next = editor
                    .actions
                    .borrow()
                    .len()
                    .checked_sub(1)
                    .map(|last| index.min(last));
                editor.rebuild(next);
            }
        });
        for (button, direction) in [(up, -1isize), (down, 1)] {
            let weak = Rc::downgrade(&editor);
            button.connect_clicked(move |_| {
                if let Some(editor) = weak.upgrade()
                    && let Some(index) = editor.selected.get()
                {
                    editor.capture();
                    let next = index.saturating_add_signed(direction);
                    if next < editor.actions.borrow().len() && next != index {
                        editor.actions.borrow_mut().swap(index, next);
                        editor.dirty.set(true);
                        editor.rebuild(Some(next));
                    }
                }
            });
        }
        for entry in [
            &editor.name,
            &editor.icon,
            &editor.command,
            &editor.patterns,
            &editor.shortcut,
        ] {
            let weak = Rc::downgrade(&editor);
            entry.connect_changed(move |_| {
                if let Some(editor) = weak.upgrade()
                    && !editor.syncing.get()
                {
                    editor.dirty.set(true);
                }
            });
        }
        for check in [&editor.files, &editor.directories, &editor.toolbar] {
            let weak = Rc::downgrade(&editor);
            check.connect_toggled(move |_| {
                if let Some(editor) = weak.upgrade()
                    && !editor.syncing.get()
                {
                    editor.dirty.set(true);
                }
            });
        }
        for spin in [&editor.minimum, &editor.maximum] {
            let weak = Rc::downgrade(&editor);
            spin.connect_value_changed(move |_| {
                if let Some(editor) = weak.upgrade()
                    && !editor.syncing.get()
                {
                    editor.dirty.set(true);
                }
            });
        }
        let weak = Rc::downgrade(&editor);
        editor.save.connect_clicked(move |_| {
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
        record.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.record_shortcut();
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
            if editor.dirty.get() && !editor.closing.get() {
                editor.confirm_close();
                return glib::Propagation::Stop;
            }
            if let Some(app) = editor.app.upgrade() {
                app.action_editor.borrow_mut().take();
            }
            glib::Propagation::Proceed
        });
        let controller = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&editor);
        controller.connect_key_pressed(move |_, key, _, mods| {
            if let Some(editor) = weak.upgrade() {
                if mods.contains(gdk::ModifierType::CONTROL_MASK) && key.to_lower() == gdk::Key::s {
                    editor.save();
                    return glib::Propagation::Stop;
                }
                if mods.contains(gdk::ModifierType::CONTROL_MASK) && key.to_lower() == gdk::Key::r {
                    editor.confirm_reload();
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::Escape {
                    editor.native.close();
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        editor.native.add_controller(controller);
        editor.rebuild(if editor.actions.borrow().is_empty() {
            None
        } else {
            Some(0)
        });
        editor
    }

    fn capture(&self) {
        let Some(index) = self.selected.get() else {
            return;
        };
        if let Some(action) = self.actions.borrow_mut().get_mut(index) {
            action.name = self.name.text().to_string();
            action.command = self.command.text().to_string();
            action.icon = optional(&self.icon);
            action.shortcut = optional(&self.shortcut);
            action.toolbar = self.toolbar.is_active();
            action.conditions.minimum_selection = self.minimum.value_as_int() as usize;
            action.conditions.maximum_selection =
                (self.maximum.value_as_int() > 0).then_some(self.maximum.value_as_int() as usize);
            action.conditions.files = self.files.is_active();
            action.conditions.directories = self.directories.is_active();
            action.conditions.patterns = self
                .patterns
                .text()
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
        }
    }
    fn rebuild(&self, selected: Option<usize>) {
        self.syncing.set(true);
        let names: Vec<_> = self
            .actions
            .borrow()
            .iter()
            .map(|a| a.name.clone())
            .collect();
        self.names.splice(
            0,
            self.names.n_items(),
            &names.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        self.chooser
            .set_selected(selected.map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
        self.syncing.set(false);
        self.select(selected);
    }
    fn select(&self, selected: Option<usize>) {
        self.syncing.set(true);
        self.selected.set(selected);
        let action = selected
            .and_then(|i| self.actions.borrow().get(i).cloned())
            .unwrap_or_else(|| CustomAction::new("", "", ""));
        for (entry, text) in [
            (&self.name, action.name.as_str()),
            (&self.icon, action.icon.as_deref().unwrap_or("")),
            (&self.command, action.command.as_str()),
            (&self.shortcut, action.shortcut.as_deref().unwrap_or("")),
        ] {
            entry.set_text(text);
            entry.set_sensitive(selected.is_some());
        }
        self.patterns
            .set_text(&action.conditions.patterns.join("; "));
        self.patterns.set_sensitive(selected.is_some());
        self.files.set_active(action.conditions.files);
        self.directories.set_active(action.conditions.directories);
        self.toolbar.set_active(action.toolbar);
        self.minimum
            .set_value(action.conditions.minimum_selection as f64);
        self.maximum
            .set_value(action.conditions.maximum_selection.unwrap_or(0) as f64);
        for widget in [
            self.files.upcast_ref::<gtk::Widget>(),
            self.directories.upcast_ref(),
            self.toolbar.upcast_ref(),
            self.minimum.upcast_ref(),
            self.maximum.upcast_ref(),
        ] {
            widget.set_sensitive(selected.is_some());
        }
        self.syncing.set(false);
    }
    fn save(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        };
        self.capture();
        let mut actions = self.actions.borrow().clone();
        if let Err(error) = validate_shortcuts(&mut actions).and_then(|()| {
            self.snapshot
                .borrow()
                .settings
                .validate(&actions)
                .map_err(|error| error.to_string())
        }) {
            self.status.set_label(&error);
            return;
        }
        let snapshot = self.snapshot.borrow().clone();
        let weak = Rc::downgrade(self);
        self.busy.set(true);
        self.save.set_sensitive(false);
        GtkWindowExt::set_focus(&self.native, None::<&gtk::Widget>);
        if let Some(child) = self.native.child() {
            child.set_sensitive(false);
        }
        self.status.set_label("Saving…");
        let hold = self.app.upgrade().map(|app| app.application.hold());
        glib::MainContext::default().spawn_local(async move {
            let _hold = hold;
            let result = gio::spawn_blocking(move || snapshot.save_actions(actions)).await;
            if let Some(editor) = weak.upgrade() {
                editor.busy.set(false);
                editor.save.set_sensitive(true);
                if let Some(child) = editor.native.child() {
                    child.set_sensitive(true);
                }
                match result {
                    Ok(Ok(snapshot)) => {
                        *editor.actions.borrow_mut() = snapshot.actions.clone();
                        *editor.snapshot.borrow_mut() = snapshot.clone();
                        editor.dirty.set(false);
                        editor
                            .status
                            .set_label("Saved. Actions are ready in all Files windows.");
                        editor.rebuild(editor.selected.get());
                        if let Some(app) = editor.app.upgrade() {
                            app.install_configuration(snapshot);
                        }
                    }
                    Ok(Err(error)) => editor
                        .status
                        .set_label(&format!("Could not save actions: {error}")),
                    Err(_) => editor.status.set_label("The configuration writer failed."),
                }
            }
        });
    }
    fn reload(self: &Rc<Self>) {
        if self.busy.replace(true) {
            return;
        };
        if let Some(child) = self.native.child() {
            child.set_sensitive(false);
        }
        let snapshot = self.snapshot.borrow().clone();
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let result = gio::spawn_blocking(move || snapshot.reload()).await;
            if let Some(editor) = weak.upgrade() {
                editor.busy.set(false);
                if let Some(child) = editor.native.child() {
                    child.set_sensitive(true);
                }
                match result {
                    Ok(Ok(snapshot)) => {
                        *editor.actions.borrow_mut() = snapshot.actions.clone();
                        *editor.snapshot.borrow_mut() = snapshot.clone();
                        editor.selected.set(None);
                        editor.rebuild(if editor.actions.borrow().is_empty() {
                            None
                        } else {
                            Some(0)
                        });
                        editor.dirty.set(false);
                        editor
                            .status
                            .set_label("Configure an action, then save. Ctrl+S saves all actions.");
                        if let Some(app) = editor.app.upgrade() {
                            app.install_configuration(snapshot);
                        }
                    }
                    Ok(Err(error)) => editor
                        .status
                        .set_label(&format!("Could not reload actions: {error}")),
                    Err(_) => editor.status.set_label("The configuration reader failed."),
                }
            }
        });
    }
    fn confirm_close(self: &Rc<Self>) {
        let dialog = gtk::AlertDialog::builder()
            .message("Discard unsaved action changes?")
            .buttons(["Keep editing", "Discard"])
            .cancel_button(0)
            .default_button(0)
            .build();
        let editor = self.clone();
        glib::MainContext::default().spawn_local(async move {
            if dialog.choose_future(Some(&editor.native)).await == Ok(1) {
                editor.closing.set(true);
                editor.native.close();
            }
        });
    }
    fn confirm_reload(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        if !self.dirty.get() {
            self.reload();
            return;
        }
        let dialog = gtk::AlertDialog::builder()
            .message("Reload and discard unsaved action changes?")
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
    fn record_shortcut(self: &Rc<Self>) {
        let window = gtk::Window::builder()
            .title("Record Shortcut")
            .transient_for(&self.native)
            .modal(true)
            .default_width(360)
            .default_height(120)
            .build();
        let label = gtk::Label::new(Some("Press a shortcut. Escape cancels; Backspace clears."));
        label.set_wrap(true);
        label.set_margin_start(20);
        label.set_margin_end(20);
        window.set_child(Some(&label));
        let controller = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        let dialog = window.downgrade();
        controller.connect_key_pressed(move |_, key, _, mods| {
            if let (Some(editor), Some(dialog)) = (weak.upgrade(), dialog.upgrade()) {
                if key == gdk::Key::Escape {
                    dialog.close();
                } else if key == gdk::Key::BackSpace && mods.is_empty() {
                    editor.shortcut.set_text("");
                    dialog.close();
                } else if gtk::accelerator_valid(key, mods) {
                    editor.shortcut.set_text(&gtk::accelerator_name(
                        key.to_lower(),
                        mods & gtk::accelerator_get_default_mod_mask(),
                    ));
                    dialog.close();
                }
            }
            glib::Propagation::Stop
        });
        window.add_controller(controller);
        window.present();
    }
}

fn optional(entry: &gtk::Entry) -> Option<String> {
    let text = entry.text();
    (!text.trim().is_empty()).then(|| text.trim().to_owned())
}
fn field(parent: &gtk::Box, label: &str, placeholder: &str) -> gtk::Entry {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let entry = gtk::Entry::new();
    entry.set_hexpand(true);
    entry.set_width_chars(1);
    entry.set_placeholder_text(Some(placeholder));
    let label = gtk::Label::with_mnemonic(label);
    label.set_xalign(0.0);
    label.set_mnemonic_widget(Some(&entry));
    group.append(&label);
    group.append(&entry);
    parent.append(&group);
    entry
}
fn validate_shortcuts(actions: &mut [CustomAction]) -> Result<(), String> {
    for action in actions {
        action
            .validate()
            .map_err(|error| format!("{}: {error}", action.name))?;
        if let Some(shortcut) = &mut action.shortcut {
            *shortcut = crate::interface::KeyChord::parse(shortcut)
                .map_err(|error| error.to_string())?
                .accelerator();
        }
    }
    Ok(())
}
