use super::tab::Tab;
use crate::preferences::{Detail, SortField};
use gtk::prelude::*;
use std::{cell::Cell, rc::Rc};

pub(super) fn install(tab: &Rc<Tab>, button: &gtk::MenuButton) {
    let popover = gtk::Popover::new();
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 10);
    panel.set_margin_start(16);
    panel.set_margin_end(16);
    panel.set_margin_top(16);
    panel.set_margin_bottom(16);
    super::operations_ui::install_menu(tab, &panel, &popover);
    super::actions_ui::menu(tab, &panel, &popover);
    let heading = gtk::Label::new(Some("This folder"));
    heading.set_xalign(0.0);
    panel.append(&heading);
    let sort = gtk::DropDown::from_strings(&["Name", "Size", "Modified", "Kind"]);
    row(&panel, "Sort by", &sort);
    let reverse = gtk::CheckButton::with_label("Reverse order");
    panel.append(&reverse);
    let width = gtk::SpinButton::with_range(160.0, 640.0, 20.0);
    row(&panel, "Column width", &width);
    let mut details = Vec::new();
    for (name, detail) in [
        ("Show size", Detail::Size),
        ("Show modified time", Detail::Modified),
        ("Show kind", Detail::Kind),
    ] {
        let check = gtk::CheckButton::with_label(name);
        panel.append(&check);
        details.push((check, detail));
    }
    let reset = gtk::Button::with_mnemonic("Use _defaults");
    panel.append(&reset);
    let reveal = gtk::Button::with_label("Show containing folder");
    panel.append(&reveal);
    reveal.set_tooltip_text(Some("Reveal the selected search result in its folder"));
    let preferences = gtk::Button::with_label("Preferences…");
    panel.append(&preferences);
    let bookmark = gtk::Button::with_label("Bookmark this folder…");
    panel.append(&bookmark);
    // Frequently used destinations stay at the top, apart from view controls.
    panel.remove(&preferences);
    panel.remove(&bookmark);
    panel.prepend(&bookmark);
    panel.prepend(&preferences);
    let locations_button = gtk::Button::with_label("Locations ›");
    panel.prepend(&locations_button);
    let menu_pages = gtk::Stack::new();
    menu_pages.set_hhomogeneous(false);
    menu_pages.set_vhomogeneous(false);
    let locations_page = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let back = gtk::Button::with_label("‹ Folder menu");
    locations_page.append(&back);
    let locations = gtk::Box::new(gtk::Orientation::Vertical, 4);
    locations.set_margin_start(8);
    locations.set_margin_end(8);
    locations.set_margin_top(8);
    locations.set_margin_bottom(8);
    let places_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(true)
        .propagate_natural_height(true)
        .min_content_width(200)
        .max_content_width(280)
        .max_content_height(400)
        .child(&locations)
        .build();
    locations_page.append(&places_scroll);
    menu_pages.add_named(&locations_page, Some("locations"));
    let weak = Rc::downgrade(tab);
    let popup = popover.downgrade();
    let pages = menu_pages.downgrade();
    locations_button.connect_clicked(move |_| {
        super::clear_box(&locations);
        if let (Some(tab), Some(popup), Some(pages)) =
            (weak.upgrade(), popup.upgrade(), pages.upgrade())
        {
            tab.append_places(&locations, Some(&popup));
            if let Some(window) = tab.window() {
                places_scroll
                    .set_max_content_height((window.native.height() - 160).clamp(100, 400));
            }
            pages.set_visible_child_name("locations");
            if let Some(first) = locations.first_child() {
                first.grab_focus();
            }
        }
    });
    let pages = menu_pages.downgrade();
    let first = locations_button.downgrade();
    back.connect_clicked(move |_| {
        if let (Some(pages), Some(first)) = (pages.upgrade(), first.upgrade()) {
            pages.set_visible_child_name("folder");
            first.grab_focus();
        }
    });
    for (button, command) in [
        (preferences, crate::interface::Command::Preferences),
        (bookmark, crate::interface::Command::BookmarkFolder),
    ] {
        let weak = Rc::downgrade(tab);
        let popup = popover.downgrade();
        button.connect_clicked(move |_| {
            if let Some(popup) = popup.upgrade() {
                popup.popdown();
            }
            if let Some(tab) = weak.upgrade() {
                tab.command(command);
            }
        });
    }
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_height(true)
        .max_content_height(420)
        .child(&panel)
        .build();
    let weak = Rc::downgrade(tab);
    let weak_scroll = scroll.downgrade();
    popover.connect_show(move |_| {
        if let (Some(tab), Some(scroll)) = (weak.upgrade(), weak_scroll.upgrade())
            && let Some(window) = tab.window()
        {
            scroll.set_max_content_height((window.native.height() - 140).clamp(120, 500));
        }
    });
    menu_pages.add_named(&scroll, Some("folder"));
    let pages = menu_pages.downgrade();
    popover.connect_show(move |_| {
        if let Some(pages) = pages.upgrade() {
            pages.set_visible_child_name("folder");
        }
    });
    popover.set_child(Some(&menu_pages));
    button.set_popover(Some(&popover));
    let syncing = Rc::new(Cell::new(false));
    let weak = Rc::downgrade(tab);
    let sync = syncing.clone();
    let controls = (
        sort.clone(),
        reverse.clone(),
        width.clone(),
        details.clone(),
    );
    popover.connect_show(move |_| {
        if let Some(view) = weak.upgrade().and_then(|tab| tab.view()) {
            sync.set(true);
            controls.0.set_selected(match view.sort {
                SortField::Name => 0,
                SortField::Size => 1,
                SortField::Modified => 2,
                SortField::Kind => 3,
            });
            controls.1.set_active(view.descending);
            controls.2.set_value(f64::from(view.column_width));
            for (check, detail) in &controls.3 {
                check.set_active(view.details.contains(detail));
            }
            sync.set(false);
        }
    });
    let weak = Rc::downgrade(tab);
    let sync = syncing.clone();
    sort.connect_selected_notify(move |sort| {
        if !sync.get()
            && let Some(tab) = weak.upgrade()
        {
            tab.edit_view(|view| {
                view.sort = match sort.selected() {
                    1 => SortField::Size,
                    2 => SortField::Modified,
                    3 => SortField::Kind,
                    _ => SortField::Name,
                }
            });
        }
    });
    let weak = Rc::downgrade(tab);
    let sync = syncing.clone();
    reverse.connect_toggled(move |check| {
        if !sync.get()
            && let Some(tab) = weak.upgrade()
        {
            tab.edit_view(|view| view.descending = check.is_active());
        }
    });
    let weak = Rc::downgrade(tab);
    let sync = syncing.clone();
    width.connect_value_changed(move |width| {
        if !sync.get()
            && let Some(tab) = weak.upgrade()
        {
            tab.edit_view(|view| view.column_width = width.value_as_int() as u16);
        }
    });
    for (check, detail) in details {
        let weak = Rc::downgrade(tab);
        let sync = syncing.clone();
        check.connect_toggled(move |check| {
            if !sync.get()
                && let Some(tab) = weak.upgrade()
            {
                tab.edit_view(|view| {
                    view.details.retain(|existing| *existing != detail);
                    if check.is_active() {
                        view.details.push(detail);
                    }
                });
            }
        });
    }
    let weak = Rc::downgrade(tab);
    let popup = popover.downgrade();
    reset.connect_clicked(move |_| {
        if let Some(tab) = weak.upgrade() {
            tab.use_defaults();
        }
        if let Some(popup) = popup.upgrade() {
            popup.popdown();
        }
    });
    let weak = Rc::downgrade(tab);
    let popup = popover.downgrade();
    reveal.connect_clicked(move |_| {
        if let Some(tab) = weak.upgrade() {
            tab.reveal_selection();
        }
        if let Some(popup) = popup.upgrade() {
            popup.popdown();
        }
    });
}

fn row(panel: &gtk::Box, name: &str, control: &impl IsA<gtk::Widget>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let label = gtk::Label::new(Some(name));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    row.append(&label);
    row.append(control);
    panel.append(&row);
}
