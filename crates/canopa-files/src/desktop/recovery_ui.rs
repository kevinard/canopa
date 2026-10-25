//! Read-only retained-version catalog; copies use the shared operation queue.
use super::{
    Desktop, icon_button, jobs,
    quicklook::{PreviewOrigin, QuickLook},
};
use crate::{
    Cancellation,
    operations::{RecoveryCursor, RecoveryItem, RecoveryStatus},
};
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

pub(super) struct RecoveryWindow {
    native: gtk::ApplicationWindow,
    app: Weak<Desktop>,
    items: RefCell<Vec<RecoveryItem>>,
    model: gtk::StringList,
    selection: gtk::SingleSelection,
    view: gtk::ListView,
    status: gtk::Label,
    problem: gtk::Label,
    retry: gtk::Button,
    copy: gtk::Button,
    more: gtk::Button,
    cursor: RefCell<Option<RecoveryCursor>>,
    loading: Cell<bool>,
    cancellation: RefCell<Cancellation>,
    revision: Cell<u64>,
    preview: RefCell<Option<Rc<QuickLook>>>,
    syncing: Cell<bool>,
}

pub(super) fn initialize(app: &Rc<Desktop>) {
    let action = gio::SimpleAction::new("recovery", None);
    let weak = Rc::downgrade(app);
    action.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            show(&app);
        }
    });
    app.application.add_action(&action);
}

pub(super) fn show(app: &Rc<Desktop>) {
    if let Some(window) = app.recovery_window.borrow().as_ref() {
        window.native.present();
        return;
    }
    let window = RecoveryWindow::new(app);
    *app.recovery_window.borrow_mut() = Some(window.clone());
    window.native.present();
    window.refresh();
}

impl RecoveryWindow {
    pub(super) fn appearance_changed(&self) {
        if let Some(preview) = self.preview.borrow().as_ref() {
            preview.appearance_changed();
        }
    }
    fn new(app: &Rc<Desktop>) -> Rc<Self> {
        let native = gtk::ApplicationWindow::builder()
            .application(&app.application)
            .title("Recovery — Canopa Files")
            .default_width(760)
            .default_height(560)
            .build();
        native.add_css_class("canopa-files");
        native.set_titlebar(Some(&gtk::HeaderBar::new()));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        toolbar.add_css_class("toolbar");
        let copy = gtk::Button::with_mnemonic("Recover _copy…");
        copy.set_tooltip_text(Some("Choose a new name and location · Ctrl+R"));
        let preview_button =
            icon_button("view-reveal-symbolic", "Preview retained version · Space");
        let refresh = icon_button("view-refresh-symbolic", "Refresh Recovery · F5");
        toolbar.append(&copy);
        toolbar.append(&preview_button);
        toolbar.append(&refresh);
        root.append(&toolbar);
        let hint = gtk::Label::new(Some(
            "Previous versions kept for Undo and protected files needing recovery. Recovering a copy leaves the retained version intact.",
        ));
        hint.set_wrap(true);
        hint.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        hint.set_xalign(0.0);
        hint.set_margin_start(14);
        hint.set_margin_end(14);
        root.append(&hint);
        let problem = gtk::Label::new(None);
        problem.set_wrap(true);
        problem.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        problem.set_xalign(0.0);
        problem.set_margin_start(14);
        problem.set_margin_end(14);
        problem.set_visible(false);
        root.append(&problem);
        let retry = gtk::Button::with_mnemonic("Retry _interrupted operation");
        retry.set_halign(gtk::Align::Start);
        retry.set_margin_start(14);
        retry.set_margin_end(14);
        retry.set_visible(false);
        root.append(&retry);
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
        let more = gtk::Button::with_label("Load more versions");
        more.set_halign(gtk::Align::Center);
        more.set_visible(false);
        root.append(&more);
        let status = gtk::Label::new(Some("Loading Recovery…"));
        status.set_xalign(0.0);
        status.set_wrap(true);
        status.set_wrap_mode(gtk::pango::WrapMode::WordChar);
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
            problem,
            retry,
            copy,
            more,
            cursor: RefCell::new(None),
            loading: Cell::new(false),
            cancellation: RefCell::new(Cancellation::default()),
            revision: Cell::new(0),
            preview: RefCell::new(None),
            syncing: Cell::new(false),
        });
        let weak = Rc::downgrade(&window);
        window.copy.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.recover_copy();
            }
        });
        let weak = Rc::downgrade(&window);
        window.retry.connect_clicked(move |button| {
            if let Some(window) = weak.upgrade()
                && let Some(app) = window.app.upgrade()
            {
                button.set_sensitive(false);
                window.status.set_label("Retrying interrupted operation…");
                jobs::retry_recovery(&app);
            }
        });
        let weak = Rc::downgrade(&window);
        preview_button.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.toggle_preview();
            }
        });
        let weak = Rc::downgrade(&window);
        refresh.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.refresh();
            }
        });
        let weak = Rc::downgrade(&window);
        window.more.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.load(true);
            }
        });
        let weak = Rc::downgrade(&window);
        window.selection.connect_selected_notify(move |_| {
            if let Some(window) = weak.upgrade().filter(|window| !window.syncing.get()) {
                window.update_controls();
                window.update_preview();
                if let Some(item) = window.selected() {
                    window.status.set_label(&item.message);
                }
            }
        });
        let weak = Rc::downgrade(&window);
        window.view.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.toggle_preview();
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
                window.cancellation.borrow().cancel();
                let preview = window.preview.borrow_mut().take();
                if let Some(preview) = preview {
                    preview.native.close();
                }
                if let Some(app) = window.app.upgrade() {
                    app.recovery_window.borrow_mut().take();
                }
            }
            glib::Propagation::Proceed
        });
        window.update_controls();
        window
    }

    fn selected(&self) -> Option<RecoveryItem> {
        self.items
            .borrow()
            .get(self.selection.selected() as usize)
            .cloned()
    }
    fn update_controls(&self) {
        let item = self.selected();
        self.copy.set_sensitive(
            !self.loading.get()
                && item.as_ref().is_some_and(|item| {
                    item.entry.as_ref().is_some_and(|entry| {
                        matches!(
                            entry.kind,
                            crate::browse::EntryKind::File
                                | crate::browse::EntryKind::Symlink
                                | crate::browse::EntryKind::Directory
                        )
                    })
                }),
        );
        self.more.set_visible(self.cursor.borrow().is_some());
        self.more.set_sensitive(!self.loading.get());
    }
    pub(super) fn refresh(self: &Rc<Self>) {
        self.load(false);
    }
    fn load(self: &Rc<Self>, append: bool) {
        if append && (self.loading.get() || self.cursor.borrow().is_none()) {
            return;
        }
        self.cancellation.borrow().cancel();
        let cancellation = Cancellation::default();
        *self.cancellation.borrow_mut() = cancellation.clone();
        let revision = self.revision.get().wrapping_add(1);
        self.revision.set(revision);
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let Some(engine) = app.operations.borrow().clone() else {
            self.status.set_label(
                "Recovery is unavailable while operation history is opening. Refresh to try again.",
            );
            return;
        };
        let cursor = if append {
            self.cursor.borrow().clone()
        } else {
            None
        };
        self.loading.set(true);
        self.update_controls();
        self.status.set_label("Loading Recovery…");
        let weak = Rc::downgrade(self);
        let hold = app.application.hold();
        glib::MainContext::default().spawn_local(async move {
            let _hold = hold;
            let result = gio::spawn_blocking(move || engine.recovery_items(cursor, &cancellation)).await;
            let Some(window) = weak.upgrade().filter(|window| window.revision.get() == revision) else { return };
            window.loading.set(false);
            match result {
                Ok(Ok(page)) => {
                    let interrupted = !page.interrupted.is_empty();
                    let message = page.interrupted.iter().map(|issue| format!("{}\n{}", issue.label, issue.message)).collect::<Vec<_>>().join("\n\n");
                    window.problem.set_label(&format!("{message}\nFile changes are paused. Recover copies to a new location, then resolve the occupied path or reconnect its drive and retry."));
                    window.problem.set_visible(interrupted);
                    window.retry.set_visible(interrupted);
                    window.retry.set_sensitive(interrupted);
                    let selected = window.selected().and_then(|item| item.entry.map(|entry| entry.path));
                    let rows: Vec<_> = page.items.iter().map(recovery_row).collect();
                    window.syncing.set(true);
                    let start = if append { window.model.n_items() } else { 0 };
                    if !append { window.items.borrow_mut().clear(); }
                    window.items.borrow_mut().extend(page.items);
                    window.model.splice(start, if append { 0 } else { window.model.n_items() }, &rows.iter().map(String::as_str).collect::<Vec<_>>());
                    *window.cursor.borrow_mut() = page.next;
                    if !append {
                        let selected = selected.and_then(|path| window.items.borrow().iter().position(|item| item.entry.as_ref().is_some_and(|entry| entry.path == path))).map(|index| index as u32).unwrap_or(gtk::INVALID_LIST_POSITION);
                        window.selection.set_selected(selected);
                    }
                    window.syncing.set(false);
                    window.status.set_label(if window.items.borrow().is_empty() { "No retained versions. Previous files appear here while needed by Undo, or when protected from cleanup." } else { "Select a version to preview it or recover a copy." });
                    if let Some(item) = window.selected() { window.status.set_label(&item.message); }
                    window.update_preview();
                }
                Ok(Err(error)) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Ok(Err(error)) => window.status.set_label(&format!("Could not read Recovery: {error}")),
                Err(_) => window.status.set_label("The Recovery reader failed."),
            }
            window.update_controls();
        });
    }
    fn recover_copy(self: &Rc<Self>) {
        if !self.copy.is_sensitive() {
            return;
        }
        let (Some(app), Some(item)) = (self.app.upgrade(), self.selected()) else {
            return;
        };
        let chooser = gtk::FileDialog::builder()
            .title("Recover a copy")
            .accept_label("_Recover copy")
            .initial_name(item.name().to_string_lossy())
            .build();
        if !item.original_is_recorded()
            && let Some(parent) = item.original.as_deref().and_then(std::path::Path::parent)
        {
            chooser.set_initial_folder(Some(&gio::File::for_path(parent)));
        }
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let Some(window) = weak.upgrade() else { return };
            if let Ok(file) = chooser.save_future(Some(&window.native)).await
                && let Some(destination) = file.path()
            {
                jobs::recover_copy(&app, item, destination);
            }
        });
    }
    fn toggle_preview(self: &Rc<Self>) {
        let previous = self.preview.borrow_mut().take();
        if let Some(preview) = previous {
            preview.native.close();
            return;
        }
        if self.selected().and_then(|item| item.entry).is_none() {
            return;
        }
        let preview = QuickLook::new(self);
        *self.preview.borrow_mut() = Some(preview.clone());
        self.update_preview();
        preview.native.present();
    }
    fn update_preview(&self) {
        if let Some(preview) = self.preview.borrow().clone() {
            let item = self.selected();
            let name = item
                .as_ref()
                .map(|item| item.name().to_string_lossy().into_owned());
            preview.follow_named(
                item.and_then(|item| item.entry.map(|entry| entry.path)),
                name,
            );
        }
    }
    fn key(self: &Rc<Self>, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        if modifiers.intersects(gdk::ModifierType::ALT_MASK | gdk::ModifierType::SUPER_MASK) {
            return glib::Propagation::Proceed;
        }
        match key {
            gdk::Key::Escape => self.native.close(),
            gdk::Key::space if !control => self.toggle_preview(),
            gdk::Key::F5 => self.refresh(),
            gdk::Key::r | gdk::Key::R if control => self.recover_copy(),
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
            gdk::Key::Up | gdk::Key::Down if !control => {
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
}

fn recovery_row(item: &RecoveryItem) -> String {
    let status = match item.status {
        RecoveryStatus::UndoBackup => "Needed by Undo",
        RecoveryStatus::NeedsRecovery => "Protected for recovery",
        RecoveryStatus::Unavailable => "Unavailable",
    };
    let details = item
        .entry
        .as_ref()
        .map(|entry| {
            let modified = entry
                .modified
                .map(|value| {
                    chrono::DateTime::<chrono::Local>::from(value)
                        .format("%Y-%m-%d %H:%M")
                        .to_string()
                })
                .unwrap_or_else(|| "Modified time unknown".into());
            let kind = match entry.kind {
                crate::browse::EntryKind::Directory => String::from("Folder"),
                crate::browse::EntryKind::File => super::listing::format_size(entry.size),
                crate::browse::EntryKind::Symlink => {
                    format!("Link · {}", super::listing::format_size(entry.size))
                }
                crate::browse::EntryKind::Other => String::from("Special file"),
            };
            format!("{kind} · {modified}")
        })
        .unwrap_or_else(|| item.message.clone());
    let original = item
        .original
        .as_ref()
        .map(|path| {
            if item.original_is_recorded() {
                format!("Recorded location: {}", path.display())
            } else {
                path.display().to_string()
            }
        })
        .unwrap_or_else(|| "Original location unknown".into());
    format!(
        "{} — {status}\n{original}\n{details}\n{}",
        item.name().to_string_lossy(),
        item.label
    )
}

impl PreviewOrigin for RecoveryWindow {
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
        if self
            .preview
            .borrow()
            .as_ref()
            .is_some_and(|preview| preview.native == *native)
        {
            self.preview.borrow_mut().take();
        }
    }
}
