use super::tab::Tab;
use crate::{
    browse::{Browser, Entry, EntryKind},
    preferences::{Detail, ViewMode, ViewSettings},
};
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::Cell,
    collections::BTreeSet,
    path::{Path, PathBuf},
    rc::Rc,
};

pub(super) struct Listing {
    pub root: gtk::ScrolledWindow,
    pub entries: Rc<Vec<Entry>>,
    model: gtk::SelectionModel,
    view: gtk::Widget,
    syncing: Cell<bool>,
}

impl Listing {
    pub fn new(
        tab: &Rc<Tab>,
        entries: Vec<Entry>,
        settings: ViewSettings,
        current: bool,
        selected: BTreeSet<PathBuf>,
        search: bool,
    ) -> Rc<Self> {
        let mode = settings.mode;
        let entries = Rc::new(entries);
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        for entry in entries.iter() {
            store.append(&glib::BoxedAnyObject::new(entry.clone()));
        }
        let model: gtk::SelectionModel = if current {
            gtk::MultiSelection::new(Some(store)).upcast()
        } else {
            let single = gtk::SingleSelection::new(Some(store));
            single.set_autoselect(false);
            single.set_can_unselect(true);
            single.upcast()
        };
        for (index, entry) in entries.iter().enumerate() {
            if selected.contains(&entry.path) {
                model.select_item(index as u32, false);
            }
        }
        let factory = gtk::SignalListItemFactory::new();
        let weak = Rc::downgrade(tab);
        factory.connect_setup(move |_, object| {
            let Some(item) = object.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let grid = mode == ViewMode::Icons && current;
            let row = gtk::Box::new(
                if grid {
                    gtk::Orientation::Vertical
                } else {
                    gtk::Orientation::Horizontal
                },
                8,
            );
            let icon = gtk::Image::new();
            icon.set_pixel_size(if grid { 48 } else { 20 });
            row.append(&icon);
            let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text.set_hexpand(true);
            let label = gtk::Label::new(None);
            label.set_xalign(if grid { 0.5 } else { 0.0 });
            label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            if grid {
                label.set_max_width_chars(18);
                row.set_width_request(110);
            }
            text.append(&label);
            let details = gtk::Label::new(None);
            details.set_xalign(if grid { 0.5 } else { 0.0 });
            details.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            details.add_css_class("file-details");
            details.set_visible(search || (current && mode == ViewMode::List));
            text.append(&details);
            row.append(&text);
            super::drag_drop::row(&weak, &row, item, current);
            if current {
                let click = gtk::GestureClick::new();
                click.set_propagation_phase(gtk::PropagationPhase::Capture);
                let weak_item = item.downgrade();
                let weak_left = weak.clone();
                click.connect_pressed(move |gesture, _, _, _| {
                    if let (Some(tab), Some(item)) = (weak_left.upgrade(), weak_item.upgrade())
                        && let Some(object) = item.item().and_downcast::<glib::BoxedAnyObject>()
                    {
                        let entry = object.borrow::<Entry>();
                        tab.pointer_focus(
                            &entry.path,
                            !gesture
                                .current_event_state()
                                .contains(gdk::ModifierType::SHIFT_MASK),
                        );
                    }
                });
                row.add_controller(click);
                let click = gtk::GestureClick::new();
                click.set_button(3);
                let weak = weak.clone();
                let weak_item = item.downgrade();
                click.connect_pressed(move |gesture, _, x, y| {
                    if let (Some(tab), Some(item)) = (weak.upgrade(), weak_item.upgrade())
                        && let Some(object) = item.item().and_downcast::<glib::BoxedAnyObject>()
                        && let Some(widget) = gesture.widget()
                    {
                        let path = object.borrow::<Entry>().path.clone();
                        if !tab.selection_paths().contains(&path) {
                            tab.pointer_focus(&path, true);
                            tab.pointer_selection(vec![path]);
                        }
                        super::actions_ui::context(&tab, &widget, x, y);
                        gesture.set_state(gtk::EventSequenceState::Claimed);
                    }
                });
                row.add_controller(click);
            }
            item.set_child(Some(&row));
        });
        factory.connect_bind(move |_, object| {
            let Some(item) = object.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let Some(object) = item.item().and_downcast::<glib::BoxedAnyObject>() else {
                return;
            };
            let entry = object.borrow::<Entry>();
            let Some(row) = item.child().and_downcast::<gtk::Box>() else {
                return;
            };
            let Some(icon) = row.first_child().and_downcast::<gtk::Image>() else {
                return;
            };
            let name = if entry.is_directory() {
                "folder-symbolic"
            } else {
                match entry.kind {
                    EntryKind::Symlink => "emblem-symbolic-link-symbolic",
                    _ => "text-x-generic-symbolic",
                }
            };
            icon.set_icon_name(Some(name));
            let Some(text) = icon.next_sibling().and_downcast::<gtk::Box>() else {
                return;
            };
            if let Some(label) = text.first_child().and_downcast::<gtk::Label>() {
                label.set_label(&entry.name());
                if let Some(details) = label.next_sibling().and_downcast::<gtk::Label>() {
                    let info = if search {
                        entry
                            .path
                            .parent()
                            .unwrap_or(Path::new("/"))
                            .to_string_lossy()
                            .into_owned()
                    } else {
                        settings
                            .details
                            .iter()
                            .filter_map(|detail| match detail {
                                Detail::Size => {
                                    (!entry.is_directory()).then(|| format_size(entry.size))
                                }
                                Detail::Modified => entry
                                    .modified
                                    .and_then(|time| {
                                        time.duration_since(std::time::UNIX_EPOCH).ok()
                                    })
                                    .and_then(|time| {
                                        glib::DateTime::from_unix_local(time.as_secs() as i64).ok()
                                    })
                                    .and_then(|time| time.format("%Y-%m-%d %H:%M").ok())
                                    .map(|text| text.to_string()),
                                Detail::Kind => Some(if entry.is_directory() {
                                    String::from("Folder")
                                } else if entry.kind == EntryKind::Symlink {
                                    String::from("Link")
                                } else {
                                    entry.path.extension().map_or_else(
                                        || String::from("File"),
                                        |extension| {
                                            format!(
                                                "{} file",
                                                extension.to_string_lossy().to_uppercase()
                                            )
                                        },
                                    )
                                }),
                            })
                            .collect::<Vec<_>>()
                            .join(" · ")
                    };
                    details.set_label(&info);
                    details.set_visible(
                        !info.is_empty() && (search || (current && mode == ViewMode::List)),
                    );
                }
            }
            row.set_tooltip_text(Some(&entry.path.to_string_lossy()));
        });
        let view: gtk::Widget = if current && mode == ViewMode::Icons {
            let grid = gtk::GridView::new(Some(model.clone()), Some(factory));
            grid.set_min_columns(1);
            grid.set_max_columns(20);
            grid.set_enable_rubberband(true);
            let weak = Rc::downgrade(tab);
            let paths = entries.clone();
            grid.connect_activate(move |_, position| {
                if let (Some(tab), Some(entry)) = (weak.upgrade(), paths.get(position as usize)) {
                    tab.open_entry(entry);
                }
            });
            grid.upcast()
        } else {
            let list = gtk::ListView::new(Some(model.clone()), Some(factory));
            list.set_enable_rubberband(current);
            list.set_single_click_activate(!current);
            let weak = Rc::downgrade(tab);
            let paths = entries.clone();
            list.connect_activate(move |_, position| {
                if let (Some(tab), Some(entry)) = (weak.upgrade(), paths.get(position as usize)) {
                    tab.open_entry(entry);
                }
            });
            list.upcast()
        };
        view.set_hexpand(true);
        view.set_vexpand(true);
        let root = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .hexpand(true)
            .child(&view)
            .build();
        root.set_min_content_width(80);
        let listing = Rc::new(Self {
            root,
            entries,
            model,
            view,
            syncing: Cell::new(false),
        });
        if current {
            let weak_tab = Rc::downgrade(tab);
            let weak_list = Rc::downgrade(&listing);
            listing.model.connect_selection_changed(move |_, _, _| {
                if let (Some(tab), Some(list)) = (weak_tab.upgrade(), weak_list.upgrade())
                    && !list.syncing.get()
                {
                    tab.pointer_selection(list.selected());
                }
            });
        }
        listing
    }

    fn selected(&self) -> Vec<PathBuf> {
        let selected = self.model.selection();
        let mut paths = Vec::new();
        if let Some((iter, first)) = gtk::BitsetIter::init_first(&selected) {
            for index in std::iter::once(first).chain(iter) {
                if let Some(entry) = self.entries.get(index as usize) {
                    paths.push(entry.path.clone());
                }
            }
        }
        paths
    }

    pub fn sync(&self, browser: &Browser, focus: bool) {
        self.syncing.set(true);
        if focus {
            self.view.grab_focus();
        }
        let selected = gtk::Bitset::new_empty();
        for (index, entry) in self.entries.iter().enumerate() {
            if browser.selected().contains(&entry.path) {
                selected.add(index as u32);
            }
        }
        self.model.set_selection(
            &selected,
            &gtk::Bitset::new_range(0, self.entries.len() as u32),
        );
        if focus
            && let Some(position) = browser
                .focused()
                .and_then(|path| self.entries.iter().position(|entry| entry.path == path))
        {
            if let Some(list) = self.view.downcast_ref::<gtk::ListView>() {
                list.scroll_to(position as u32, gtk::ListScrollFlags::FOCUS, None);
            }
            if let Some(grid) = self.view.downcast_ref::<gtk::GridView>() {
                grid.scroll_to(position as u32, gtk::ListScrollFlags::FOCUS, None);
            }
        }
        self.syncing.set(false);
    }
}

pub(super) fn format_size(size: u64) -> String {
    if size < 1024 {
        return format!("{size} B");
    }
    let units = ["KiB", "MiB", "GiB", "TiB"];
    let mut amount = size as f64 / 1024.0;
    let mut unit = 0;
    while amount >= 1024.0 && unit + 1 < units.len() {
        amount /= 1024.0;
        unit += 1;
    }
    format!("{amount:.1} {}", units[unit])
}
