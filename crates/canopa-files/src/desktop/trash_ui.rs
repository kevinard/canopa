use super::{
    Desktop, icon_button, jobs,
    quicklook::{PreviewOrigin, QuickLook},
};
use crate::{
    Cancellation,
    trash::{TrashItem, TrashLocation},
};
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub(super) struct TrashWindow {
    native: gtk::ApplicationWindow,
    app: Weak<Desktop>,
    items: RefCell<Vec<TrashItem>>,
    model: gtk::StringList,
    selection: gtk::SingleSelection,
    view: gtk::ListView,
    status: gtk::Label,
    restore: gtk::Button,
    restore_to: gtk::Button,
    location_choice: gtk::DropDown,
    roots: RefCell<Vec<std::path::PathBuf>>,
    cancellation: RefCell<Cancellation>,
    revision: Cell<u64>,
    monitors: RefCell<Vec<gio::FileMonitor>>,
    refresh_queued: Cell<bool>,
    preview: RefCell<Option<Rc<QuickLook>>>,
    syncing: Cell<bool>,
}

pub(super) fn initialize(app: &Rc<Desktop>) {
    let action = gio::SimpleAction::new("trash", None);
    let weak = Rc::downgrade(app);
    action.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            show(&app);
        }
    });
    app.application.add_action(&action);
}

pub(super) fn show(app: &Rc<Desktop>) {
    if let Some(window) = app.trash_window.borrow().as_ref() {
        window.native.present();
        return;
    }
    let window = TrashWindow::new(app);
    *app.trash_window.borrow_mut() = Some(window.clone());
    window.native.present();
    window.refresh();
}

pub(super) fn move_selection(tab: &Rc<super::tab::Tab>) {
    let paths = tab.selection_paths();
    if paths.is_empty() {
        return;
    }
    if let Some(app) = tab.window().and_then(|window| window.app.upgrade()) {
        jobs::trash(&app, Rc::downgrade(tab), paths);
    }
}

impl TrashWindow {
    pub(super) fn appearance_changed(&self) {
        if let Some(preview) = self.preview.borrow().as_ref() {
            preview.appearance_changed();
        }
    }
    fn new(app: &Rc<Desktop>) -> Rc<Self> {
        let native = gtk::ApplicationWindow::builder()
            .application(&app.application)
            .title("Trash — Canopa Files")
            .default_width(760)
            .default_height(540)
            .build();
        native.add_css_class("canopa-files");
        let header = gtk::HeaderBar::new();
        let restore = gtk::Button::with_label("Restore");
        restore.set_tooltip_text(Some("Restore to the original location · Ctrl+R"));
        let restore_to = icon_button(
            "folder-open-symbolic",
            "Restore to another folder · Ctrl+Shift+R",
        );
        let copy = icon_button("edit-copy-symbolic", "Copy the stored file · Ctrl+C");
        let refresh = icon_button("view-refresh-symbolic", "Refresh Trash · F5");
        native.set_titlebar(Some(&header));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        toolbar.add_css_class("toolbar");
        toolbar.append(&restore);
        toolbar.append(&restore_to);
        toolbar.append(&copy);
        toolbar.append(&refresh);
        root.append(&toolbar);
        let location_choice = gtk::DropDown::from_strings(&["All locations", "Main Trash"]);
        location_choice.set_tooltip_text(Some("All locations · Ctrl+0 / Main Trash · Ctrl+1"));
        location_choice.set_margin_start(14);
        location_choice.set_margin_end(14);
        location_choice.set_margin_top(8);
        root.append(&location_choice);
        let model = gtk::StringList::new(&[]);
        let selection = gtk::SingleSelection::new(Some(model.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let label = gtk::Label::new(None);
            label.set_xalign(0.0);
            label.set_wrap(true);
            label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            label.set_margin_start(14);
            label.set_margin_end(14);
            label.set_margin_top(10);
            label.set_margin_bottom(10);
            item.set_child(Some(&label));
        });
        factory.connect_bind(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            if let (Some(value), Some(label)) = (
                item.item().and_downcast::<gtk::StringObject>(),
                item.child().and_downcast::<gtk::Label>(),
            ) {
                label.set_label(&value.string());
            }
        });
        let view = gtk::ListView::new(Some(selection.clone()), Some(factory));
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&view)
            .build();
        root.append(&scroll);
        let status = gtk::Label::new(Some("Loading Trash…"));
        status.set_xalign(0.0);
        status.set_wrap(true);
        status.set_margin_start(14);
        status.set_margin_end(14);
        status.set_margin_bottom(12);
        root.append(&status);
        native.set_child(Some(&root));
        let window = Rc::new(Self {
            native,
            app: Rc::downgrade(app),
            items: RefCell::new(Vec::new()),
            model,
            selection,
            view,
            status,
            restore,
            restore_to,
            location_choice,
            roots: RefCell::new(Vec::new()),
            cancellation: RefCell::new(Cancellation::default()),
            revision: Cell::new(0),
            monitors: RefCell::new(Vec::new()),
            refresh_queued: Cell::new(false),
            preview: RefCell::new(None),
            syncing: Cell::new(false),
        });
        let weak = Rc::downgrade(&window);
        window.location_choice.connect_selected_notify(move |_| {
            if let Some(window) = weak.upgrade() {
                window.items.borrow_mut().clear();
                window.model.splice(0, window.model.n_items(), &[]);
                window.update_controls();
                window.status.set_label("Loading Trash…");
                window.refresh();
            }
        });
        let weak = Rc::downgrade(&window);
        window.selection.connect_selected_notify(move |_| {
            if let Some(window) = weak.upgrade().filter(|window| !window.syncing.get()) {
                window.update_controls();
                window.update_preview();
            }
        });
        let weak = Rc::downgrade(&window);
        window.restore.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.restore_item(false);
            }
        });
        let weak = Rc::downgrade(&window);
        window.restore_to.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.restore_item(true);
            }
        });
        let weak = Rc::downgrade(&window);
        copy.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.copy_item();
            }
        });
        let weak = Rc::downgrade(&window);
        refresh.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.refresh();
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&window);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            weak.upgrade().map_or(glib::Propagation::Proceed, |window| {
                window.key(key, modifiers)
            })
        });
        window.native.add_controller(keys);
        let weak = Rc::downgrade(&window);
        window.native.connect_close_request(move |_| {
            if let Some(window) = weak.upgrade() {
                let preview = window.preview.borrow_mut().take();
                if let Some(preview) = preview {
                    preview.native.close();
                }
                window.cancellation.borrow().cancel();
                for monitor in window.monitors.borrow_mut().drain(..) {
                    monitor.cancel();
                }
                if let Some(app) = window.app.upgrade() {
                    app.trash_window.borrow_mut().take();
                }
            }
            glib::Propagation::Proceed
        });
        window.update_controls();
        window
    }

    fn key(self: &Rc<Self>, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        match key {
            gdk::Key::Escape => self.native.close(),
            gdk::Key::space => self.toggle_preview(),
            gdk::Key::F5 => self.refresh(),
            gdk::Key::_0 if control => self.location_choice.set_selected(0),
            gdk::Key::_1 if control => self.location_choice.set_selected(1),
            gdk::Key::r | gdk::Key::R if control => self.restore_item(shift),
            gdk::Key::c | gdk::Key::C if control => self.copy_item(),
            gdk::Key::z | gdk::Key::Z if control => {
                if let Some(app) = self.app.upgrade()
                    && let Err(error) = jobs::undo_latest(&app)
                {
                    self.status.set_label(error);
                }
            }
            gdk::Key::j | gdk::Key::J if control => {
                if let Some(app) = self.app.upgrade() {
                    jobs::show(&app);
                }
            }
            gdk::Key::Down | gdk::Key::Up => {
                let count = self.items.borrow().len() as u32;
                if count > 0 {
                    let current = self.selection.selected();
                    let next = if current == gtk::INVALID_LIST_POSITION {
                        0
                    } else if key == gdk::Key::Down {
                        (current + 1).min(count - 1)
                    } else {
                        current.saturating_sub(1)
                    };
                    self.selection.set_selected(next);
                    self.view.grab_focus();
                    self.view.scroll_to(next, gtk::ListScrollFlags::FOCUS, None);
                }
            }
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    }
    fn toggle_preview(self: &Rc<Self>) {
        let previous = self.preview.borrow_mut().take();
        if let Some(preview) = previous {
            preview.native.close();
            return;
        }
        let preview = QuickLook::new(self);
        *self.preview.borrow_mut() = Some(preview.clone());
        self.update_preview();
        preview.native.present();
    }
    fn update_preview(&self) {
        let preview = self.preview.borrow().clone();
        if let Some(preview) = preview {
            let item = self.selected();
            let path = item.as_ref().map(|item| item.path.clone());
            let name = item.map(|item| item.name().to_string_lossy().into_owned());
            preview.follow_named(path, name);
        }
    }
    fn selected(&self) -> Option<TrashItem> {
        self.items
            .borrow()
            .get(self.selection.selected() as usize)
            .cloned()
    }
    fn update_controls(&self) {
        let item = self.selected();
        self.restore.set_sensitive(
            item.as_ref()
                .is_some_and(|item| item.original.is_some() && item.problem.is_none()),
        );
        self.restore_to
            .set_sensitive(item.as_ref().is_some_and(|item| item.problem.is_none()));
        if item.as_ref().is_some_and(|item| item.problem.is_some()) {
            self.status.set_label(
                "The original location is unavailable. Copy this item to recover its contents.",
            );
        }
    }
    fn copy_item(&self) {
        if let (Some(app), Some(item)) = (self.app.upgrade(), self.selected())
            && let Err(error) = super::clipboard::publish(&app, &[item.path], false)
        {
            self.status
                .set_label(&format!("Could not copy the stored file: {error}"));
        }
    }
    fn restore_item(self: &Rc<Self>, elsewhere: bool) {
        let (Some(app), Some(item)) = (self.app.upgrade(), self.selected()) else {
            return;
        };
        if !elsewhere {
            jobs::restore(&app, item, None);
            return;
        }
        let chooser = gtk::FileDialog::builder()
            .title("Restore to folder")
            .accept_label("_Restore here")
            .build();
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let Some(window) = weak.upgrade() else { return };
            if let Ok(folder) = chooser.select_folder_future(Some(&window.native)).await
                && let Some(path) = folder.path()
            {
                let destination = path.join(item.name());
                jobs::restore(&app, item, Some(destination));
            }
        });
    }
    pub(super) fn refresh(self: &Rc<Self>) {
        self.cancellation.borrow().cancel();
        let cancellation = Cancellation::default();
        *self.cancellation.borrow_mut() = cancellation.clone();
        let revision = self.revision.get().wrapping_add(1);
        self.revision.set(revision);
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let data_home = app.data_home.clone();
        let main_only = self.location_choice.selected() == 1;
        let weak = Rc::downgrade(self);
        let hold = app.application.hold();
        glib::MainContext::default().spawn_local(async move {
            let _hold = hold;
            let result = gio::spawn_blocking(move || -> std::io::Result<_> {
                let locations = if main_only {
                    vec![TrashLocation::home(&data_home)?]
                } else {
                    TrashLocation::available(&data_home)?
                };
                let mut items = Vec::new();
                let mut warnings = Vec::new();
                for location in &locations {
                    if cancellation.is_cancelled() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Interrupted,
                            "Trash read cancelled",
                        ));
                    }
                    match location.list(&cancellation) {
                        Ok(mut entries) => items.append(&mut entries),
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                            return Err(error);
                        }
                        Err(error) => {
                            warnings.push(format!("{}: {error}", location.directory().display()))
                        }
                    }
                }
                items.sort_by(|left, right| {
                    left.name()
                        .cmp(right.name())
                        .then_with(|| left.path.cmp(&right.path))
                });
                Ok((
                    items,
                    locations
                        .into_iter()
                        .map(|location| location.directory().to_owned())
                        .collect::<Vec<_>>(),
                    warnings,
                ))
            })
            .await;
            let Some(window) = weak
                .upgrade()
                .filter(|window| window.revision.get() == revision)
            else {
                return;
            };
            match result {
                Ok(Ok((items, roots, warnings))) => {
                    *window.roots.borrow_mut() = roots;
                    let selected = window.selected().map(|item| item.path);
                    let rows: Vec<_> = items
                        .iter()
                        .map(|item| {
                            format!(
                                "{}\n{}\n{}",
                                item.name().to_string_lossy(),
                                item.original
                                    .as_deref()
                                    .map(|path| path.display().to_string())
                                    .unwrap_or_else(|| String::from("Original location unknown")),
                                item.problem
                                    .as_deref()
                                    .or(item.deletion_date.as_deref())
                                    .unwrap_or("Deletion date unknown")
                            )
                        })
                        .collect();
                    window.syncing.set(true);
                    *window.items.borrow_mut() = items;
                    window.model.splice(
                        0,
                        window.model.n_items(),
                        &rows.iter().map(String::as_str).collect::<Vec<_>>(),
                    );
                    let selected = selected
                        .and_then(|path| {
                            window
                                .items
                                .borrow()
                                .iter()
                                .position(|item| item.path == path)
                        })
                        .map(|position| position as u32)
                        .unwrap_or(gtk::INVALID_LIST_POSITION);
                    window.selection.set_selected(selected);
                    window.syncing.set(false);
                    window.status.set_label(if !warnings.is_empty() {
                        "Some Trash locations could not be read. Hover here for details."
                    } else if window.items.borrow().is_empty() {
                        "Trash is empty."
                    } else {
                        "Select an item to restore it."
                    });
                    window.status.set_tooltip_text(
                        (!warnings.is_empty())
                            .then(|| warnings.join("\n"))
                            .as_deref(),
                    );
                    window.update_controls();
                    window.update_preview();
                    window.watch();
                }
                Ok(Err(error)) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Ok(Err(error)) => window
                    .status
                    .set_label(&format!("Could not read Trash: {error}")),
                Err(_) => window.status.set_label("The Trash reader failed"),
            }
        });
    }
    fn watch(self: &Rc<Self>) {
        for monitor in self.monitors.borrow_mut().drain(..) {
            monitor.cancel();
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let mut paths = vec![app.data_home.clone()];
        for root in self.roots.borrow().iter() {
            paths.push(root.join("files"));
            paths.push(root.join("info"));
        }
        for path in paths {
            if let Ok(monitor) = gio::File::for_path(path)
                .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
            {
                let weak = Rc::downgrade(self);
                monitor.connect_changed(move |_, _, _, _| {
                    if let Some(window) = weak.upgrade()
                        && !window.refresh_queued.replace(true)
                    {
                        let weak = Rc::downgrade(&window);
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(150),
                            move || {
                                if let Some(window) = weak.upgrade() {
                                    window.refresh_queued.set(false);
                                    window.refresh();
                                }
                            },
                        );
                    }
                });
                self.monitors.borrow_mut().push(monitor);
            }
        }
    }
}

impl PreviewOrigin for TrashWindow {
    fn native(&self) -> &gtk::ApplicationWindow {
        &self.native
    }
    fn app(&self) -> Option<Rc<Desktop>> {
        self.app.upgrade()
    }
    fn preview_key(
        self: Rc<Self>,
        key: gdk::Key,
        modifiers: gdk::ModifierType,
    ) -> glib::Propagation {
        self.key(key, modifiers)
    }
    fn preview_closed(&self, native: &gtk::Window) {
        let owns = self
            .preview
            .borrow()
            .as_ref()
            .is_some_and(|preview| preview.native == *native);
        if owns {
            self.preview.borrow_mut().take();
        }
    }
}
