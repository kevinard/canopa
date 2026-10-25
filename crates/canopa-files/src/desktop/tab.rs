use super::{clear_box, home, icon_button, listing::Listing, window::BrowserWindow};
use crate::{
    Cancellation,
    browse::{Browser, Directory, Entry, HistoryDirection, SelectionMode},
    interface::{Command, Place, PlaceKind, SidebarMode},
    preferences::{ViewMode, ViewSettings},
    search::{SearchRequest, SearchScope},
};
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

pub(super) struct Tab {
    pub root: gtk::Box,
    pub label: gtk::Label,
    owner: Weak<BrowserWindow>,
    browser: RefCell<Option<Browser>>,
    location: gtk::Entry,
    toolbar_before: gtk::Box,
    toolbar_after: gtk::Box,
    toolbar_actions: RefCell<Vec<(gtk::Button, crate::actions::CustomAction)>>,
    search_bar: gtk::Box,
    search_entry: gtk::SearchEntry,
    recursive: gtk::CheckButton,
    more_results: gtk::Button,
    search_limit: Cell<usize>,
    sidebar: gtk::Box,
    columns: gtk::Box,
    action_toolbar: gtk::Box,
    action_scroll: gtk::ScrolledWindow,
    drop_hint: gtk::Label,
    drop_owner: RefCell<Option<(glib::WeakRef<gtk::Widget>, gdk::DragAction)>>,
    status: gtk::Label,
    error: gtk::Label,
    view_choice: gtk::DropDown,
    syncing: Cell<bool>,
    width: Cell<i32>,
    ancestors: RefCell<BTreeMap<PathBuf, Directory>>,
    visible_paths: RefCell<Vec<PathBuf>>,
    listing: RefCell<Option<Rc<Listing>>>,
    scrolls: RefCell<BTreeMap<PathBuf, f64>>,
    revision: Cell<u64>,
    search_revision: Cell<u64>,
    focus_search_when_ready: Cell<bool>,
    search_cancel: RefCell<Cancellation>,
    type_buffer: RefCell<(String, Instant)>,
    monitor: RefCell<Option<gio::FileMonitor>>,
    refresh_queued: Cell<bool>,
    refresh_revision: Cell<u64>,
}

impl Tab {
    pub fn new(owner: &Rc<BrowserWindow>) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_hexpand(true);
        root.set_vexpand(true);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        toolbar.add_css_class("toolbar");
        let location = gtk::Entry::new();
        location.set_hexpand(true);
        location.set_width_chars(1);
        location.update_property(&[gtk::accessible::Property::Label("Location")]);
        let toolbar_before = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let toolbar_after = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let before = toolbar_scroll(&toolbar_before);
        let after = toolbar_scroll(&toolbar_after);
        toolbar.append(&before);
        toolbar.append(&location);
        toolbar.append(&after);
        let options = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("Folder view preferences")
            .build();
        options.update_property(&[gtk::accessible::Property::Label("Folder view preferences")]);
        toolbar.append(&options);
        root.append(&toolbar);
        let action_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        action_toolbar.add_css_class("toolbar");
        let action_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .child(&action_toolbar)
            .min_content_height(40)
            .propagate_natural_height(true)
            .build();
        action_scroll.set_visible(false);
        root.append(&action_scroll);
        let drop_hint = gtk::Label::new(None);
        drop_hint.set_wrap(true);
        drop_hint.set_xalign(0.0);
        drop_hint.add_css_class("status");
        drop_hint.set_visible(false);
        drop_hint.set_valign(gtk::Align::End);
        drop_hint.set_hexpand(true);
        drop_hint.set_can_target(false);
        let search_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        search_bar.add_css_class("toolbar");
        let search_entry = gtk::SearchEntry::new();
        search_entry.set_hexpand(true);
        search_entry.set_placeholder_text(Some("Search filenames…"));
        let recursive = gtk::CheckButton::with_label("Subfolders");
        let more_results = gtk::Button::with_label("More");
        more_results.set_tooltip_text(Some("Include another 50,000 filename matches"));
        more_results.set_visible(false);
        let close_search = icon_button("window-close-symbolic", "Close search · Esc");
        search_bar.append(&search_entry);
        search_bar.append(&recursive);
        search_bar.append(&more_results);
        search_bar.append(&close_search);
        search_bar.set_visible(false);
        root.append(&search_bar);
        let error = gtk::Label::new(None);
        error.add_css_class("error");
        error.set_wrap(true);
        error.set_xalign(0.0);
        error.set_visible(false);
        root.append(&error);
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.set_vexpand(true);
        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 2);
        sidebar.add_css_class("sidebar");
        sidebar.set_width_request(160);
        let columns = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        columns.set_hexpand(true);
        columns.set_vexpand(true);
        let sidebar_scroll = gtk::ScrolledWindow::builder()
            .hexpand(false)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&sidebar)
            .build();
        body.append(&sidebar_scroll);
        body.append(&columns);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&body));
        overlay.set_vexpand(true);
        let sizing = gtk::DrawingArea::new();
        sizing.set_can_target(false);
        sizing.set_hexpand(true);
        sizing.set_vexpand(true);
        overlay.add_overlay(&sizing);
        overlay.add_overlay(&drop_hint);
        root.append(&overlay);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.add_css_class("status");
        let status = gtk::Label::new(Some("Loading…"));
        status.set_xalign(0.0);
        status.set_hexpand(true);
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let view_choice = gtk::DropDown::from_strings(&["Columns", "List", "Icons"]);
        view_choice.update_property(&[gtk::accessible::Property::Label("View")]);
        footer.append(&status);
        footer.append(&view_choice);
        root.append(&footer);
        let tab = Rc::new(Self {
            root,
            label: gtk::Label::new(Some("Files")),
            owner: Rc::downgrade(owner),
            browser: RefCell::new(None),
            location,
            toolbar_before,
            toolbar_after,
            toolbar_actions: RefCell::new(Vec::new()),
            search_bar,
            search_entry,
            recursive,
            more_results,
            search_limit: Cell::new(50_000),
            sidebar,
            columns,
            action_toolbar,
            action_scroll,
            drop_hint,
            drop_owner: RefCell::new(None),
            status,
            error,
            view_choice,
            syncing: Cell::new(false),
            width: Cell::new(960),
            ancestors: RefCell::new(BTreeMap::new()),
            visible_paths: RefCell::new(Vec::new()),
            listing: RefCell::new(None),
            scrolls: RefCell::new(BTreeMap::new()),
            revision: Cell::new(0),
            search_revision: Cell::new(0),
            focus_search_when_ready: Cell::new(false),
            search_cancel: RefCell::new(Cancellation::default()),
            type_buffer: RefCell::new((String::new(), Instant::now())),
            monitor: RefCell::new(None),
            refresh_queued: Cell::new(false),
            refresh_revision: Cell::new(0),
        });
        let weak = Rc::downgrade(&tab);
        tab.location.connect_activate(move |entry| {
            if let Some(tab) = weak.upgrade() {
                let text = entry.text();
                let path = tab
                    .path()
                    .filter(|path| path.to_string_lossy() == text.as_str())
                    .unwrap_or_else(|| {
                        let typed = PathBuf::from(text.as_str());
                        if text.as_str() == "~" {
                            home()
                        } else if let Some(tail) = text.strip_prefix("~/") {
                            home().join(tail)
                        } else if typed.is_absolute() {
                            typed
                        } else {
                            tab.path().unwrap_or_else(home).join(typed)
                        }
                    });
                tab.navigate(path, None);
            }
        });
        let weak = Rc::downgrade(&tab);
        close_search.connect_clicked(move |_| {
            if let Some(tab) = weak.upgrade() {
                tab.close_search();
            }
        });
        let weak = Rc::downgrade(&tab);
        tab.search_entry.connect_search_changed(move |_| {
            if let Some(tab) = weak.upgrade() {
                tab.search_limit.set(50_000);
                tab.search();
            }
        });
        let weak = Rc::downgrade(&tab);
        tab.recursive.connect_toggled(move |_| {
            if let Some(tab) = weak.upgrade() {
                tab.search_limit.set(50_000);
                tab.search();
            }
        });
        let weak = Rc::downgrade(&tab);
        tab.more_results.connect_clicked(move |_| {
            if let Some(tab) = weak.upgrade() {
                tab.search_limit
                    .set(tab.search_limit.get().saturating_add(50_000));
                tab.search();
            }
        });
        let weak = Rc::downgrade(&tab);
        tab.view_choice.connect_selected_notify(move |choice| {
            if let Some(tab) = weak.upgrade()
                && !tab.syncing.get()
            {
                tab.change_view(choice.selected());
            }
        });
        let weak = Rc::downgrade(&tab);
        sizing.connect_resize(move |_, width, _| {
            if let Some(tab) = weak.upgrade() {
                tab.width.set(width);
                tab.adapt_sidebar();
                tab.recursive.set_label(if width >= 560 {
                    Some("Subfolders")
                } else {
                    None
                });
                tab.recursive
                    .set_tooltip_text(Some("Include subfolders in filename search"));
                tab.render(false);
            }
        });
        tab.populate_sidebar();
        tab.rebuild_toolbar();
        super::options::install(&tab, &options);
        let weak = Rc::downgrade(&tab);
        tab.search_entry.connect_next_match(move |_| {
            if let Some(tab) = weak.upgrade() {
                tab.focus_search_results();
            }
        });
        let weak = Rc::downgrade(&tab);
        tab.search_entry.connect_activate(move |_| {
            if let Some(tab) = weak.upgrade() {
                tab.focus_search_results();
            }
        });
        tab
    }

    fn adapt_sidebar(&self) {
        let mode = self
            .window()
            .and_then(|window| window.app.upgrade())
            .map(|app| app.action_configuration.borrow().settings.sidebar)
            .unwrap_or_default();
        let visible = match mode {
            SidebarMode::Auto => self.width.get() >= 960,
            SidebarMode::Always => self.width.get() >= 560,
            SidebarMode::Never => false,
        };
        let visible = visible && self.sidebar.first_child().is_some();
        self.sidebar.set_visible(visible);
        if let Some(scroll) = self.sidebar.parent() {
            scroll.set_visible(visible);
        }
    }
    fn populate_sidebar(self: &Rc<Self>) {
        clear_box(&self.sidebar);
        self.append_places(&self.sidebar, None);
        self.adapt_sidebar();
    }
    pub fn append_places(self: &Rc<Self>, panel: &gtk::Box, popup: Option<&gtk::Popover>) {
        let Some(app) = self.window().and_then(|window| window.app.upgrade()) else {
            return;
        };
        let places = app.action_configuration.borrow().settings.places.clone();
        for place in places {
            let button = gtk::Button::with_label(place.label());
            button.add_css_class("flat");
            button.set_halign(gtk::Align::Fill);
            if let Some(label) = button.child().and_downcast::<gtk::Label>() {
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.set_max_width_chars(24);
                label.set_xalign(0.0);
            }
            let path = place_path(&place);
            button.set_tooltip_text(
                path.as_ref()
                    .map(|path| path.display().to_string())
                    .as_deref(),
            );
            if let Some(destination) = path.clone() {
                super::drag_drop::destination(self, button.upcast_ref(), move || {
                    Some(destination.clone())
                });
            }
            let weak = Rc::downgrade(self);
            let popup = popup.map(glib::object::ObjectExt::downgrade);
            button.connect_clicked(move |_| {
                if let Some(popup) = popup.as_ref().and_then(glib::WeakRef::upgrade) {
                    popup.popdown();
                }
                if let Some(tab) = weak.upgrade() {
                    if let Some(path) = &path {
                        tab.navigate(path.clone(), None);
                    } else if let Some(app) = tab.window().and_then(|window| window.app.upgrade()) {
                        super::trash_ui::show(&app);
                    }
                }
            });
            panel.append(&button);
        }
        super::devices::append(self, panel, popup);
    }
    fn rebuild_toolbar(self: &Rc<Self>) {
        clear_box(&self.toolbar_before);
        clear_box(&self.toolbar_after);
        self.toolbar_actions.borrow_mut().clear();
        let Some(app) = self.window().and_then(|window| window.app.upgrade()) else {
            return;
        };
        let configuration = app.action_configuration.borrow();
        let mut after = false;
        for id in &configuration.settings.toolbar {
            if id == "location" {
                after = true;
                continue;
            }
            let container = if after {
                &self.toolbar_after
            } else {
                &self.toolbar_before
            };
            if let Some(command) = Command::from_id(id) {
                let binding = configuration
                    .settings
                    .bindings(command)
                    .iter()
                    .filter_map(|binding| {
                        let chord = crate::interface::KeyChord::parse(binding).ok()?;
                        let (key, mods) = gtk::accelerator_parse(chord.accelerator())?;
                        Some(gtk::accelerator_get_label(key, mods).to_string())
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let button = icon_button(
                    command.icon(),
                    &format!(
                        "{}{}{}",
                        command.label(),
                        if binding.is_empty() { "" } else { " · " },
                        binding
                    ),
                );
                let weak = Rc::downgrade(self);
                button.connect_clicked(move |_| {
                    if let Some(tab) = weak.upgrade() {
                        tab.command(command);
                    }
                });
                container.append(&button);
            } else if let Some(action) = id
                .strip_prefix("action:")
                .and_then(|id| configuration.actions.iter().find(|a| a.id == id))
            {
                let button = super::actions_ui::action_button(self, action);
                container.append(&button);
                self.toolbar_actions
                    .borrow_mut()
                    .push((button, action.clone()));
            }
        }
        for group in [&self.toolbar_before, &self.toolbar_after] {
            if let Some(scroll) = group.parent() {
                scroll.set_visible(group.first_child().is_some());
            }
        }
    }
    pub fn preferences_changed(self: &Rc<Self>, interface: bool) {
        if interface {
            self.populate_sidebar();
            self.rebuild_toolbar();
            self.actions_changed();
        }
        let view = self.path().and_then(|path| {
            self.window()
                .and_then(|window| window.app.upgrade())
                .map(|app| app.preferences.borrow().for_folder(&path).clone())
        });
        if let Some(view) = view {
            let changed = {
                let mut state = self.browser.borrow_mut();
                state.as_mut().is_some_and(|browser| {
                    browser.view() != &view && browser.set_view(view).is_ok()
                })
            };
            if changed {
                self.render(true);
                self.notify_selection();
            }
        }
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.browser
            .borrow()
            .as_ref()
            .map(|browser| browser.path().to_owned())
    }

    pub fn window(&self) -> Option<Rc<BrowserWindow>> {
        self.owner.upgrade()
    }
    pub fn selection_paths(&self) -> Vec<PathBuf> {
        self.browser
            .borrow()
            .as_ref()
            .map(|browser| browser.selected().iter().cloned().collect())
            .unwrap_or_default()
    }
    pub fn select_for_drag(self: &Rc<Self>, paths: Vec<PathBuf>) {
        if let Some(browser) = self.browser.borrow_mut().as_mut() {
            browser.set_selection(paths);
        }
        if let (Some(list), Some(browser)) = (
            self.listing.borrow().as_ref(),
            self.browser.borrow().as_ref(),
        ) {
            list.sync(browser, false);
        }
        self.notify_selection();
    }
    pub fn selection_entries(&self) -> Vec<Entry> {
        self.browser
            .borrow()
            .as_ref()
            .map(|browser| {
                browser
                    .visible_entries()
                    .into_iter()
                    .filter(|entry| browser.selected().contains(&entry.path))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn drop_hint(
        &self,
        widget: &gtk::Widget,
        message: Option<&str>,
        action: gdk::DragAction,
    ) -> gdk::DragAction {
        let owner = self
            .drop_owner
            .borrow()
            .as_ref()
            .and_then(|(owner, action)| owner.upgrade().map(|owner| (owner, *action)));
        if let Some(message) = message {
            if let Some((_, action)) =
                owner.filter(|(owner, _)| owner != widget && owner.is_ancestor(widget))
            {
                return action;
            }
            *self.drop_owner.borrow_mut() = Some((widget.downgrade(), action));
            self.drop_hint.set_label(message);
            self.drop_hint.set_visible(true);
        } else if owner.as_ref().is_none_or(|(owner, _)| owner == widget) {
            self.drop_owner.borrow_mut().take();
            self.drop_hint.set_visible(false);
        }
        action
    }
    pub fn actions_changed(self: &Rc<Self>) {
        let entries = self.selection_entries();
        for (button, action) in self.toolbar_actions.borrow().iter() {
            button.set_sensitive(action.is_applicable(&entries));
        }
        super::actions_ui::toolbar(self, &self.action_toolbar);
        self.action_scroll
            .set_visible(self.action_toolbar.first_child().is_some());
    }
    pub fn preview_path(&self) -> Option<PathBuf> {
        let state = self.browser.borrow();
        let browser = state.as_ref()?;
        browser
            .focused()
            .filter(|path| browser.selected().contains(*path))
            .map(Path::to_owned)
            .or_else(|| browser.selected().first().cloned())
    }
    pub fn show_error(&self, message: &str) {
        self.error.set_label(message);
        self.error.set_visible(true);
    }
    pub fn cancel_work(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
        self.search_revision
            .set(self.search_revision.get().wrapping_add(1));
        self.search_cancel.borrow().cancel();
        if let Some(monitor) = self.monitor.borrow_mut().take() {
            monitor.cancel();
        }
    }

    pub(super) fn navigation_token(&self) -> (u64, u64) {
        (self.revision.get(), self.search_revision.get())
    }

    pub(super) fn storage_location(&self) -> Option<(PathBuf, PathBuf)> {
        let browser = self.browser.borrow();
        let directory = browser.as_ref()?.directory();
        Some((directory.path.clone(), directory.resolved_path.clone()))
    }

    pub fn navigate(self: &Rc<Self>, path: PathBuf, history: Option<HistoryDirection>) {
        self.navigate_target(path, history, false);
    }

    pub fn reveal_path(self: &Rc<Self>, path: PathBuf) {
        self.navigate_target(path, None, true);
    }

    fn navigate_target(
        self: &Rc<Self>,
        path: PathBuf,
        history: Option<HistoryDirection>,
        reveal_folder: bool,
    ) {
        let revision = self.revision.get().wrapping_add(1);
        self.revision.set(revision);
        self.search_cancel.borrow().cancel();
        self.search_revision
            .set(self.search_revision.get().wrapping_add(1));
        self.status.set_label("Loading…");
        self.error.set_visible(false);
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let result = gio::spawn_blocking(move || {
                let entry = Entry::read(&path)?;
                let reveal = (reveal_folder || !entry.is_directory()).then_some(entry.path.clone());
                let path = if reveal.is_some() {
                    entry.path.parent().unwrap_or(Path::new("/")).to_owned()
                } else {
                    entry.path
                };
                Directory::read(path).map(|directory| (directory, reveal))
            })
            .await;
            let Some(tab) = weak.upgrade().filter(|tab| tab.revision.get() == revision) else {
                return;
            };
            match result {
                Ok(Ok((directory, reveal))) => {
                    let path = directory.path.clone();
                    {
                        let mut state = tab.browser.borrow_mut();
                        match state.as_mut() {
                            Some(browser) => {
                                if let Some(direction) = history {
                                    browser.visit_history(directory, direction);
                                } else if browser.path() == path {
                                    browser.close_search();
                                    browser.refresh(directory);
                                } else {
                                    browser.visit(directory);
                                }
                            }
                            None => {
                                *state = Some(Browser::new(directory));
                            }
                        }
                        if let Some(browser) = state.as_mut() {
                            if let Some(app) =
                                tab.owner.upgrade().and_then(|owner| owner.app.upgrade())
                            {
                                let view = app.preferences.borrow().for_folder(&path).clone();
                                let _ = browser.set_view(view);
                            }
                            if let Some(reveal) = reveal {
                                browser.select(&reveal, SelectionMode::Replace);
                            }
                        }
                    }
                    tab.location.set_text(&path.to_string_lossy());
                    tab.label.set_label(
                        &path
                            .file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy(),
                    );
                    tab.search_bar.set_visible(false);
                    tab.ancestors.borrow_mut().clear();
                    tab.install_monitor(&path);
                    tab.render(true);
                    tab.notify_selection();
                    tab.focus_listing();
                }
                Ok(Err(error)) => {
                    tab.show_error(&error.to_string());
                    tab.update_status();
                }
                Err(_) => {
                    tab.show_error("Directory reader failed");
                    tab.update_status();
                }
            }
        });
    }

    fn install_monitor(self: &Rc<Self>, path: &Path) {
        if let Some(monitor) = self.monitor.borrow_mut().take() {
            monitor.cancel();
        }
        match gio::File::for_path(path)
            .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        {
            Ok(monitor) => {
                let weak = Rc::downgrade(self);
                monitor.connect_changed(move |_, _, _, _| {
                    let Some(tab) = weak.upgrade() else {
                        return;
                    };
                    if tab.refresh_queued.replace(true) {
                        return;
                    }
                    let weak = Rc::downgrade(&tab);
                    glib::timeout_add_local_once(Duration::from_millis(120), move || {
                        if let Some(tab) = weak.upgrade() {
                            tab.refresh_queued.set(false);
                            tab.refresh();
                        }
                    });
                });
                *self.monitor.borrow_mut() = Some(monitor);
            }
            Err(error) => self.show_error(&format!("Live folder updates unavailable: {error}")),
        }
    }

    fn refresh(self: &Rc<Self>) {
        let Some(path) = self.path() else {
            return;
        };
        if self.search_bar.is_visible() {
            self.search();
            return;
        }
        let revision = self.revision.get();
        let refresh_revision = self.refresh_revision.get().wrapping_add(1);
        self.refresh_revision.set(refresh_revision);
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let result = gio::spawn_blocking(move || Directory::read(path)).await;
            if let Some(tab) = weak.upgrade().filter(|tab| {
                tab.revision.get() == revision && tab.refresh_revision.get() == refresh_revision
            }) {
                match result {
                    Ok(Ok(directory)) => {
                        if let Some(browser) = tab.browser.borrow_mut().as_mut() {
                            browser.refresh(directory);
                        }
                        tab.render(true);
                        tab.notify_selection();
                    }
                    Ok(Err(error)) => tab.show_error(&error.to_string()),
                    Err(_) => tab.show_error("Directory reader failed"),
                }
            }
        });
    }

    pub fn render(self: &Rc<Self>, force: bool) {
        let (paths, mode, searching) = {
            let state = self.browser.borrow();
            let Some(browser) = state.as_ref() else {
                return;
            };
            let searching = browser.search().is_some();
            let paths = if searching || browser.view().mode != ViewMode::Columns {
                vec![browser.path().to_owned()]
            } else {
                browser.column_paths(
                    (self.width.get() - if self.sidebar.is_visible() { 160 } else { 0 }).max(1)
                        as f32,
                )
            };
            (paths, browser.view().mode, searching)
        };
        if !force && *self.visible_paths.borrow() == paths {
            return;
        }
        let had_focus = self
            .owner
            .upgrade()
            .and_then(|owner| GtkWindowExt::focus(&owner.native))
            .is_some_and(|focus| {
                self.listing
                    .borrow()
                    .as_ref()
                    .is_some_and(|list| focus.is_ancestor(&list.root))
            });
        *self.visible_paths.borrow_mut() = paths.clone();
        self.listing.borrow_mut().take();
        clear_box(&self.columns);
        self.syncing.set(true);
        self.view_choice.set_selected(match mode {
            ViewMode::Columns => 0,
            ViewMode::List => 1,
            ViewMode::Icons => 2,
        });
        self.syncing.set(false);
        for (index, path) in paths.iter().enumerate() {
            let current = index + 1 == paths.len();
            let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
            column.set_hexpand(true);
            column.set_vexpand(true);
            column.add_css_class("file-column");
            if !searching {
                let destination = path.clone();
                super::drag_drop::destination(self, column.upcast_ref(), move || {
                    Some(destination.clone())
                });
            }
            if paths.len() > 1 {
                let heading = gtk::Label::new(Some(
                    &path
                        .file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy(),
                ));
                heading.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                heading.add_css_class("column-heading");
                column.append(&heading);
            }
            let (entries, selected) = if current {
                let state = self.browser.borrow();
                let browser = state.as_ref().unwrap();
                (
                    Some(browser.visible_entries().into_iter().cloned().collect()),
                    browser.selected().clone(),
                )
            } else {
                let hidden = self
                    .browser
                    .borrow()
                    .as_ref()
                    .is_some_and(|b| b.show_hidden());
                let entries = self.ancestors.borrow().get(path).map(|directory| {
                    directory
                        .visible_entries(hidden, Some(&paths[index + 1]))
                        .into_iter()
                        .cloned()
                        .collect()
                });
                (entries, [paths[index + 1].clone()].into_iter().collect())
            };
            if let Some(entries) = entries {
                let settings = self.view().unwrap_or_default();
                let listing = Listing::new(self, entries, settings, current, selected, searching);
                let offset = if current {
                    self.browser
                        .borrow()
                        .as_ref()
                        .map_or(0.0, |b| f64::from(b.scroll()))
                } else {
                    self.scrolls.borrow().get(path).copied().unwrap_or(0.0)
                };
                let adjustment = listing.root.vadjustment();
                let restoring = Rc::new(Cell::new(true));
                let guard = restoring.clone();
                let weak = Rc::downgrade(self);
                let location = path.clone();
                adjustment.connect_value_changed(move |adjustment| {
                    if guard.get() {
                        return;
                    }
                    if let Some(tab) = weak.upgrade() {
                        tab.scrolls
                            .borrow_mut()
                            .insert(location.clone(), adjustment.value());
                        if current
                            && let Some(browser) = tab
                                .browser
                                .borrow_mut()
                                .as_mut()
                                .filter(|browser| browser.path() == location)
                        {
                            browser.set_scroll(adjustment.value() as f32);
                        }
                    }
                });
                listing.root.add_tick_callback(move |_, _| {
                    adjustment.set_value(offset);
                    restoring.set(false);
                    glib::ControlFlow::Break
                });
                column.append(&listing.root);
                if listing.entries.is_empty() {
                    let empty = gtk::Label::new(Some(if searching {
                        "No matching filenames"
                    } else {
                        "This folder is empty"
                    }));
                    empty.add_css_class("column-heading");
                    column.append(&empty);
                }
                if current {
                    *self.listing.borrow_mut() = Some(listing);
                }
            } else {
                column.append(&gtk::Label::new(Some("Loading…")));
                self.load_ancestor(path.clone());
            }
            self.columns.append(&column);
        }
        self.columns.set_homogeneous(true);
        self.update_status();
        if had_focus {
            self.focus_listing();
        }
    }

    fn load_ancestor(self: &Rc<Self>, path: PathBuf) {
        let revision = self.revision.get();
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let result = gio::spawn_blocking(move || Directory::read(path)).await;
            if let Some(tab) = weak.upgrade().filter(|tab| tab.revision.get() == revision)
                && let Ok(Ok(directory)) = result
            {
                tab.ancestors
                    .borrow_mut()
                    .insert(directory.path.clone(), directory);
                tab.render(true);
            }
        });
    }

    pub(super) fn focus_listing(&self) {
        if let (Some(list), Some(browser)) = (
            self.listing.borrow().as_ref(),
            self.browser.borrow().as_ref(),
        ) {
            list.sync(browser, true);
        }
    }
    fn notify_selection(self: &Rc<Self>) {
        self.update_status();
        self.actions_changed();
        if let Some(owner) = self.owner.upgrade() {
            owner.selection_changed();
        }
    }
    pub fn pointer_focus(&self, path: &Path, reset: bool) {
        if let Some(browser) = self.browser.borrow_mut().as_mut() {
            browser.focus_entry(path, reset);
        }
    }
    pub fn pointer_selection(self: &Rc<Self>, paths: Vec<PathBuf>) {
        if let Some(browser) = self.browser.borrow_mut().as_mut() {
            browser.set_selection(paths);
        }
        self.notify_selection();
    }
    fn sync_selection(self: &Rc<Self>) {
        self.focus_listing();
        self.notify_selection();
    }
    fn update_status(&self) {
        let state = self.browser.borrow();
        let Some(browser) = state.as_ref() else {
            return;
        };
        let count = browser.visible_entries().len();
        let selected = browser.selected().len();
        let suffix = if let Some(results) = browser.search() {
            format!(
                " · {} scanned{}",
                results.scanned,
                if results.truncated {
                    " · more available"
                } else {
                    ""
                }
            )
        } else if browser.show_hidden() {
            String::from(" · hidden files shown")
        } else {
            String::new()
        };
        self.status.set_label(&format!(
            "{count} items{}{}",
            if selected > 0 {
                format!(" · {selected} selected")
            } else {
                String::new()
            },
            suffix
        ));
    }

    pub fn open_entry(self: &Rc<Self>, entry: &Entry) {
        if entry.is_directory() {
            self.navigate(entry.path.clone(), None);
        } else {
            let uri = gio::File::for_path(&entry.path).uri();
            let weak = Rc::downgrade(self);
            gio::AppInfo::launch_default_for_uri_async(
                &uri,
                gio::AppLaunchContext::NONE,
                gio::Cancellable::NONE,
                move |result| {
                    if let (Err(error), Some(tab)) = (result, weak.upgrade()) {
                        tab.show_error(&error.to_string());
                    }
                },
            );
        }
    }
    fn open_focused(self: &Rc<Self>, folder_only: bool) {
        let entry = {
            let state = self.browser.borrow();
            state.as_ref().and_then(|browser| {
                browser.focused().and_then(|path| {
                    browser
                        .visible_entries()
                        .into_iter()
                        .find(|entry| entry.path == path)
                        .cloned()
                })
            })
        };
        if let Some(entry) = entry.filter(|entry| !folder_only || entry.is_directory()) {
            self.open_entry(&entry);
        }
    }
    fn parent(self: &Rc<Self>) {
        let parent = self
            .browser
            .borrow()
            .as_ref()
            .and_then(|browser| browser.parent().map(Path::to_owned));
        if let Some(parent) = parent {
            self.navigate(parent, None);
        }
    }
    fn history(self: &Rc<Self>, direction: HistoryDirection) {
        let target = self
            .browser
            .borrow()
            .as_ref()
            .and_then(|browser| browser.history_target(direction).map(Path::to_owned));
        if let Some(path) = target {
            self.navigate(path, Some(direction));
        }
    }
    fn change_view(self: &Rc<Self>, index: u32) {
        self.edit_view(|view| {
            view.mode = match index {
                1 => ViewMode::List,
                2 => ViewMode::Icons,
                _ => ViewMode::Columns,
            }
        });
    }

    pub fn view(&self) -> Option<ViewSettings> {
        self.browser
            .borrow()
            .as_ref()
            .map(|browser| browser.view().clone())
    }

    pub fn edit_view(self: &Rc<Self>, change: impl FnOnce(&mut ViewSettings)) {
        let change = {
            let mut state = self.browser.borrow_mut();
            let Some(browser) = state.as_mut() else {
                return;
            };
            let mut view = browser.view().clone();
            change(&mut view);
            if let Err(error) = browser.set_view(view.clone()) {
                self.show_error(&error.to_string());
                return;
            }
            (browser.path().to_owned(), view)
        };
        if let Some(app) = self.owner.upgrade().and_then(|owner| owner.app.upgrade())
            && let Err(error) = app.store_view(&change.0, Some(change.1))
        {
            self.show_error(&error.to_string());
        }
        self.render(true);
    }

    pub fn use_defaults(self: &Rc<Self>) {
        let Some(path) = self.path() else {
            return;
        };
        let Some(app) = self.owner.upgrade().and_then(|owner| owner.app.upgrade()) else {
            return;
        };
        if let Err(error) = app.store_view(&path, None) {
            self.show_error(&error.to_string());
            return;
        }
        let view = app.preferences.borrow().for_folder(&path).clone();
        if let Some(browser) = self.browser.borrow_mut().as_mut() {
            let _ = browser.set_view(view);
        }
        self.render(true);
    }

    pub fn reveal_selection(self: &Rc<Self>) {
        if let Some(path) = self.preview_path() {
            self.reveal_path(path);
        }
    }
    fn start_search(&self) {
        self.focus_search_when_ready.set(false);
        self.search_bar.set_visible(true);
        self.search_entry.grab_focus();
    }
    fn focus_search_results(self: &Rc<Self>) {
        let mut ready = false;
        if let Some(browser) = self.browser.borrow_mut().as_mut() {
            ready = browser
                .search()
                .is_some_and(|results| results.query == self.search_entry.text());
            if ready && browser.selected().is_empty() {
                browser.step(1, SelectionMode::Replace);
            }
        }
        self.focus_search_when_ready.set(!ready);
        self.sync_selection();
    }
    fn close_search(self: &Rc<Self>) {
        self.focus_search_when_ready.set(false);
        self.search_cancel.borrow().cancel();
        self.search_revision
            .set(self.search_revision.get().wrapping_add(1));
        self.search_bar.set_visible(false);
        self.more_results.set_visible(false);
        if let Some(browser) = self.browser.borrow_mut().as_mut() {
            browser.close_search();
        }
        self.render(true);
        self.sync_selection();
    }
    fn search(self: &Rc<Self>) {
        if !self.search_bar.is_visible() {
            return;
        }
        self.search_cancel.borrow().cancel();
        let cancel = Cancellation::default();
        *self.search_cancel.borrow_mut() = cancel.clone();
        let revision = self.search_revision.get().wrapping_add(1);
        self.search_revision.set(revision);
        let query = self.search_entry.text().to_string();
        if query.is_empty() {
            if let Some(browser) = self.browser.borrow_mut().as_mut() {
                browser.close_search();
            }
            self.render(true);
            self.notify_selection();
            return;
        }
        let Some(path) = self.path() else {
            return;
        };
        let mut request = SearchRequest::new(
            path,
            query,
            if self.recursive.is_active() {
                SearchScope::Recursive
            } else {
                SearchScope::CurrentFolder
            },
        );
        request.show_hidden = self
            .browser
            .borrow()
            .as_ref()
            .is_some_and(|browser| browser.show_hidden());
        request.maximum_results = self.search_limit.get();
        self.status.set_label("Searching filenames…");
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let result = gio::spawn_blocking(move || request.run(&cancel)).await;
            if let Some(tab) = weak
                .upgrade()
                .filter(|tab| tab.search_revision.get() == revision)
            {
                match result {
                    Ok(results) if !results.cancelled => {
                        tab.more_results.set_visible(results.truncated);
                        if !results.failures.is_empty() {
                            tab.show_error(&format!(
                                "{} locations could not be read",
                                results.failures.len()
                            ));
                        }
                        if let Some(browser) = tab.browser.borrow_mut().as_mut() {
                            browser.show_search(results);
                        }
                        tab.render(true);
                        if tab.focus_search_when_ready.replace(false) {
                            tab.focus_search_results();
                        }
                        tab.notify_selection();
                    }
                    Err(_) => tab.show_error("Search worker failed"),
                    _ => {}
                }
            }
        });
    }

    pub fn command(self: &Rc<Self>, command: Command) {
        if self.window().is_some_and(|window| window.command(command)) {
            return;
        }
        match command {
            Command::Back => self.history(HistoryDirection::Back),
            Command::Forward => self.history(HistoryDirection::Forward),
            Command::Parent => self.parent(),
            Command::EnterFolder => self.open_focused(true),
            Command::Open => self.open_focused(false),
            Command::QuickLook => {
                if let Some(window) = self.window() {
                    window.toggle_preview();
                }
            }
            Command::Search => self.start_search(),
            Command::Location => {
                self.location.grab_focus();
                self.location.select_region(0, -1);
            }
            Command::Refresh => self.refresh(),
            Command::SelectAll => {
                if let Some(browser) = self.browser.borrow_mut().as_mut() {
                    browser.select_all();
                }
                self.sync_selection();
            }
            Command::ShowHidden => {
                if let Some(browser) = self.browser.borrow_mut().as_mut() {
                    browser.set_show_hidden(!browser.show_hidden());
                }
                if self.search_bar.is_visible() {
                    self.search();
                } else {
                    self.render(true);
                    self.notify_selection();
                }
            }
            Command::Copy => super::clipboard::copy(self, false),
            Command::Cut => super::clipboard::copy(self, true),
            Command::Paste => super::clipboard::paste(self),
            Command::NewFolder => super::operations_ui::prompt(self, false),
            Command::Rename => super::operations_ui::prompt(self, true),
            Command::Trash => super::trash_ui::move_selection(self),
            Command::Undo => super::operations_ui::undo(self),
            Command::Columns => self.change_view(0),
            Command::List => self.change_view(1),
            Command::Icons => self.change_view(2),
            Command::CustomActions => {
                if let Some(app) = self.window().and_then(|window| window.app.upgrade()) {
                    super::actions_ui::show(&app);
                }
            }
            Command::BookmarkFolder => {
                if let Some(app) = self.window().and_then(|window| window.app.upgrade()) {
                    super::preferences_ui::bookmark(&app, self.path());
                }
            }
            _ => {}
        }
    }

    pub fn key(self: &Rc<Self>, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        self.key_from(key, modifiers, false)
    }
    pub fn preview_key(
        self: &Rc<Self>,
        key: gdk::Key,
        modifiers: gdk::ModifierType,
    ) -> glib::Propagation {
        self.key_from(key, modifiers, true)
    }
    fn key_from(
        self: &Rc<Self>,
        key: gdk::Key,
        modifiers: gdk::ModifierType,
        preview: bool,
    ) -> glib::Propagation {
        let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        let alt = modifiers.contains(gdk::ModifierType::ALT_MASK);
        let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        let stop = glib::Propagation::Stop;
        let command = self
            .window()
            .and_then(|window| window.app.upgrade())
            .and_then(|app| {
                let chord = super::key_chord(key, modifiers)?;
                app.action_configuration
                    .borrow()
                    .settings
                    .command_for(&chord)
            });
        if matches!(command, Some(Command::Location | Command::Search)) {
            self.command(command.unwrap());
            return stop;
        }
        let focus = if preview {
            None
        } else {
            self.owner
                .upgrade()
                .and_then(|owner| GtkWindowExt::focus(&owner.native))
        };
        let search_focused = focus.as_ref().is_some_and(|widget| {
            widget == &self.search_entry || widget.is_ancestor(&self.search_entry)
        });
        if search_focused && matches!(key, gdk::Key::Down | gdk::Key::Return | gdk::Key::KP_Enter) {
            self.focus_search_results();
            return stop;
        }
        let editing = focus.as_ref().is_some_and(|widget| {
            widget.is::<gtk::Text>() || widget.is::<gtk::Entry>() || widget.is::<gtk::SearchEntry>()
        });
        if key == gdk::Key::Escape {
            if self.search_bar.is_visible() {
                self.close_search();
            } else {
                self.focus_listing();
            }
            return stop;
        }
        if editing {
            return glib::Propagation::Proceed;
        }
        if super::actions_ui::shortcut(self, key, modifiers) {
            return stop;
        }
        if matches!(key, gdk::Key::Menu) || key == gdk::Key::F10 && shift {
            super::actions_ui::context(self, self.columns.upcast_ref(), 8.0, 8.0);
            return stop;
        }
        if focus
            .as_ref()
            .is_some_and(|widget| widget != &self.columns && !widget.is_ancestor(&self.columns))
        {
            return glib::Propagation::Proceed;
        }
        if let Some(command) = command {
            self.command(command);
            return stop;
        }
        if !control && !alt {
            match key {
                gdk::Key::Up | gdk::Key::Down | gdk::Key::Home | gdk::Key::End => {
                    if let Some(browser) = self.browser.borrow_mut().as_mut() {
                        let delta = if key == gdk::Key::Up {
                            -1
                        } else if key == gdk::Key::Down {
                            1
                        } else if key == gdk::Key::Home {
                            isize::MIN
                        } else {
                            isize::MAX
                        };
                        browser.step(
                            delta,
                            if shift {
                                SelectionMode::Extend
                            } else {
                                SelectionMode::Replace
                            },
                        );
                    }
                    self.sync_selection();
                    return stop;
                }
                _ => {}
            }
            if let Some(character) = key.to_unicode().filter(|character| !character.is_control()) {
                let prefix = {
                    let mut buffer = self.type_buffer.borrow_mut();
                    if buffer.1.elapsed() > Duration::from_secs(1) {
                        buffer.0.clear();
                    }
                    buffer.0.push(character);
                    buffer.1 = Instant::now();
                    buffer.0.clone()
                };
                if let Some(browser) = self.browser.borrow_mut().as_mut() {
                    browser.select_prefix(&prefix);
                }
                self.sync_selection();
                return stop;
            }
        }
        glib::Propagation::Proceed
    }
}

fn toolbar_scroll(content: &gtk::Box) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_width(true)
        .max_content_width(180)
        .child(content)
        .build()
}
fn place_path(place: &Place) -> Option<PathBuf> {
    let special = |directory, fallback| {
        glib::user_special_dir(directory).unwrap_or_else(|| home().join(fallback))
    };
    Some(match place {
        Place::Bookmark { path, .. } => path.clone(),
        Place::Builtin(kind) => match kind {
            PlaceKind::Home => home(),
            PlaceKind::Filesystem => PathBuf::from("/"),
            PlaceKind::Trash | PlaceKind::Recovery => return None,
            PlaceKind::Desktop => special(glib::UserDirectory::Desktop, "Desktop"),
            PlaceKind::Documents => special(glib::UserDirectory::Documents, "Documents"),
            PlaceKind::Downloads => special(glib::UserDirectory::Downloads, "Downloads"),
            PlaceKind::Pictures => special(glib::UserDirectory::Pictures, "Pictures"),
        },
    })
}
