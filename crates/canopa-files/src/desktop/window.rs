use super::{Desktop, home, icon_button, quicklook::QuickLook, tab::Tab};
use crate::interface::Command;
use gtk::{gdk, glib, prelude::*};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::{Rc, Weak},
};

pub(super) struct BrowserWindow {
    pub native: gtk::ApplicationWindow,
    pub app: Weak<Desktop>,
    notebook: gtk::Notebook,
    configuration_error: gtk::Label,
    tabs: RefCell<Vec<Rc<Tab>>>,
    pub preview: RefCell<Option<Rc<QuickLook>>>,
}

impl BrowserWindow {
    pub fn new(app: &Rc<Desktop>) -> Rc<Self> {
        let native = gtk::ApplicationWindow::builder()
            .application(&app.application)
            .title("Canopa Files")
            .default_width(960)
            .default_height(660)
            .build();
        native.add_css_class("canopa-files");
        let notebook = gtk::Notebook::new();
        notebook.set_scrollable(true);
        notebook.set_show_tabs(false);
        notebook.set_vexpand(true);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let configuration_error = gtk::Label::new(None);
        configuration_error.add_css_class("error");
        configuration_error.set_wrap(true);
        configuration_error.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        configuration_error.set_xalign(0.0);
        configuration_error.set_visible(false);
        root.append(&configuration_error);
        root.append(&notebook);
        native.set_child(Some(&root));
        let header = gtk::HeaderBar::new();
        let add = icon_button("tab-new-symbolic", "New tab");
        let new = icon_button("window-new-symbolic", "New window");
        let operations = icon_button("view-list-symbolic", "Operations");
        header.pack_start(&operations);
        let weak = Rc::downgrade(app);
        operations.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                super::jobs::show(&app);
            }
        });
        header.pack_start(&add);
        header.pack_end(&new);
        native.set_titlebar(Some(&header));
        let session = Rc::new(Self {
            native,
            app: Rc::downgrade(app),
            notebook,
            configuration_error,
            tabs: RefCell::new(Vec::new()),
            preview: RefCell::new(None),
        });
        let weak = Rc::downgrade(&session);
        add.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                let path = window
                    .active_tab()
                    .and_then(|tab| tab.path())
                    .unwrap_or_else(home);
                window.add_tab(path);
            }
        });
        let weak = Rc::downgrade(&session);
        new.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade()
                && let Some(app) = window.app.upgrade()
            {
                app.open_window(vec![
                    window
                        .active_tab()
                        .and_then(|tab| tab.path())
                        .unwrap_or_else(home),
                ]);
            }
        });
        let weak = Rc::downgrade(&session);
        session.notebook.connect_switch_page(move |_, _, _| {
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(window) = weak.upgrade() {
                    window.selection_changed();
                }
            });
        });
        let controller = gtk::EventControllerKey::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&session);
        controller.connect_key_pressed(move |_, key, _, modifiers| {
            weak.upgrade().map_or(glib::Propagation::Proceed, |window| {
                window.key(key, modifiers)
            })
        });
        let weak = Rc::downgrade(app);
        controller.connect_modifiers(move |_, modifiers| {
            if let Some(app) = weak.upgrade()
                && app
                    .active_drag
                    .borrow()
                    .as_ref()
                    .and_then(glib::WeakRef::upgrade)
                    .is_some()
            {
                app.drag_modifiers.set(modifiers);
            }
            glib::Propagation::Proceed
        });
        session.native.add_controller(controller);
        let weak = Rc::downgrade(&session);
        session.native.connect_close_request(move |_| {
            if let Some(window) = weak.upgrade() {
                let preview = window.preview.borrow_mut().take();
                if let Some(preview) = preview {
                    preview.native.close();
                }
                for tab in window.tabs.borrow().iter() {
                    tab.cancel_work();
                }
                if let Some(app) = window.app.upgrade() {
                    app.windows
                        .borrow_mut()
                        .retain(|candidate| !Rc::ptr_eq(candidate, &window));
                }
            }
            glib::Propagation::Proceed
        });
        session
    }

    pub fn add_tab(self: &Rc<Self>, path: PathBuf) {
        let tab = Tab::new(self);
        let label = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        label.append(&tab.label);
        let close = icon_button("window-close-symbolic", "Close tab");
        label.append(&close);
        self.tabs.borrow_mut().push(tab.clone());
        let page = self.notebook.append_page(&tab.root, Some(&label));
        self.notebook.set_current_page(Some(page));
        self.notebook.set_show_tabs(self.tabs.borrow().len() > 1);
        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(&tab);
        close.connect_clicked(move |_| {
            if let (Some(window), Some(tab)) = (weak.upgrade(), weak_tab.upgrade()) {
                window.close_tab(&tab);
            }
        });
        tab.navigate(path, None);
    }

    pub fn active_tab(&self) -> Option<Rc<Tab>> {
        let page = self.notebook.current_page()?;
        let child = self.notebook.nth_page(Some(page))?;
        self.tabs
            .borrow()
            .iter()
            .find(|tab| tab.root == child)
            .cloned()
    }
    pub fn preferences_changed(&self, interface: bool) {
        for tab in self.tabs.borrow().iter() {
            tab.preferences_changed(interface);
        }
    }
    pub(super) fn configuration_status(&self, message: Option<&str>) {
        self.configuration_error.set_label(message.unwrap_or(""));
        self.configuration_error.set_visible(message.is_some());
    }
    pub(super) fn appearance_changed(&self) {
        if let Some(preview) = self.preview.borrow().as_ref() {
            preview.appearance_changed();
        }
    }

    pub fn command(self: &Rc<Self>, command: Command) -> bool {
        match command {
            Command::Operations => {
                if let Some(app) = self.app.upgrade() {
                    super::jobs::show(&app);
                }
            }
            Command::Recovery => {
                if let Some(app) = self.app.upgrade() {
                    super::recovery_ui::show(&app);
                }
            }
            Command::NewWindow => {
                if let Some(app) = self.app.upgrade() {
                    app.open_window(vec![
                        self.active_tab()
                            .and_then(|tab| tab.path())
                            .unwrap_or_else(home),
                    ]);
                }
            }
            Command::NewTab => self.add_tab(
                self.active_tab()
                    .and_then(|tab| tab.path())
                    .unwrap_or_else(home),
            ),
            Command::CloseTab => {
                if let Some(tab) = self.active_tab() {
                    self.close_tab(&tab);
                }
            }
            Command::NextTab | Command::PreviousTab => {
                let count = self.notebook.n_pages();
                if count > 0 {
                    let delta = if command == Command::PreviousTab {
                        count - 1
                    } else {
                        1
                    };
                    self.notebook.set_current_page(Some(
                        (self.notebook.current_page().unwrap_or(0) + delta) % count,
                    ));
                }
            }
            Command::Preferences => {
                if let Some(app) = self.app.upgrade() {
                    super::preferences_ui::show(&app);
                }
            }
            _ => return false,
        }
        true
    }

    fn close_tab(&self, tab: &Rc<Tab>) {
        tab.cancel_work();
        if self.tabs.borrow().len() == 1 {
            self.native.close();
            return;
        }
        if let Some(page) = self.notebook.page_num(&tab.root) {
            self.notebook.remove_page(Some(page));
        }
        self.tabs
            .borrow_mut()
            .retain(|candidate| !Rc::ptr_eq(candidate, tab));
        self.notebook.set_show_tabs(self.tabs.borrow().len() > 1);
        self.selection_changed();
    }

    pub fn selection_changed(&self) {
        if let Some(tab) = self.active_tab() {
            if let Some(path) = tab.path() {
                self.native.set_title(Some(&format!(
                    "{} — Canopa Files",
                    path.file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                )));
            }
            let preview = self.preview.borrow().clone();
            if let Some(preview) = preview {
                preview.follow(&tab);
            }
        }
    }

    pub fn toggle_preview(self: &Rc<Self>) {
        let old = self.preview.borrow_mut().take();
        if let Some(preview) = old {
            preview.native.close();
            return;
        }
        let preview = QuickLook::new(self);
        *self.preview.borrow_mut() = Some(preview.clone());
        if let Some(tab) = self.active_tab() {
            preview.follow(&tab);
        }
        preview.native.present();
    }

    pub fn key(self: &Rc<Self>, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        self.key_from(key, modifiers, false)
    }
    fn key_from(
        self: &Rc<Self>,
        key: gdk::Key,
        modifiers: gdk::ModifierType,
        preview: bool,
    ) -> glib::Propagation {
        if key == gdk::Key::Escape
            && let Some(app) = self.app.upgrade()
            && super::drag_drop::cancel(&app)
        {
            return glib::Propagation::Stop;
        }
        let mut focus = if preview {
            None
        } else {
            GtkWindowExt::focus(&self.native)
        };
        while let Some(widget) = focus {
            if widget.is::<gtk::Popover>() {
                return glib::Propagation::Proceed;
            }
            focus = widget.parent();
        }
        let command = self.app.upgrade().and_then(|app| {
            let chord = super::key_chord(key, modifiers)?;
            app.action_configuration
                .borrow()
                .settings
                .command_for(&chord)
        });
        if command.is_some_and(|command| self.command(command)) {
            return glib::Propagation::Stop;
        }
        self.active_tab().map_or(glib::Propagation::Proceed, |tab| {
            if preview {
                tab.preview_key(key, modifiers)
            } else {
                tab.key(key, modifiers)
            }
        })
    }
}

impl super::quicklook::PreviewOrigin for BrowserWindow {
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
        self.key_from(key, modifiers, true)
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
