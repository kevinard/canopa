use super::{Desktop, tab::Tab};
use crate::operations::{OperationEngine, OperationRequest};
use gtk::{gio, glib, prelude::*};
use std::{rc::Rc, sync::Arc};

pub(super) fn initialize(app: &Rc<Desktop>) {
    let path = app.preference_path.with_file_name("operations.sqlite3");
    let weak = Rc::downgrade(app);
    let hold = app.application.hold();
    let storage = app.storage.clone();
    glib::MainContext::default().spawn_local(async move {
        let _hold = hold;
        let result = gio::spawn_blocking(move || {
            OperationEngine::open_with_recovery_check(path, |paths| {
                storage.snapshot().check(
                    &paths
                        .iter()
                        .cloned()
                        .map(super::storage::Access::entry)
                        .collect::<Vec<_>>(),
                )
            })
            .map(|engine| {
                let label = engine.undo_label();
                let id = engine.latest_undo_id();
                let interrupted = engine
                    .interrupted_operations(&crate::Cancellation::default())
                    .map(|issues| !issues.is_empty())
                    .unwrap_or(false);
                (Arc::new(engine), label, id, interrupted)
            })
        })
        .await;
        let Some(app) = weak.upgrade() else {
            return;
        };
        match result {
            Ok(Ok((engine, label, id, interrupted))) => {
                *app.operations.borrow_mut() = Some(engine);
                *app.undo_label.borrow_mut() =
                    label.unwrap_or_else(|| String::from("Nothing to undo"));
                super::jobs::ready(&app, id);
                if interrupted {
                    super::recovery_ui::show(&app);
                }
            }
            failure => {
                let message = match failure {
                    Ok(Err(error)) => error.to_string(),
                    _ => String::from("history reader failed"),
                };
                *app.operation_error.borrow_mut() = Some(message.clone());
                super::jobs::unavailable(&app, &message);
                for window in app.windows.borrow().iter() {
                    if let Some(tab) = window.active_tab() {
                        tab.show_error(&format!("File operations are unavailable: {message}"));
                    }
                }
            }
        }
    });
}

pub(super) fn run(app: &Rc<Desktop>, origin: &Rc<Tab>, request: Option<OperationRequest>) {
    super::jobs::mutation(app, origin, request);
}

pub(super) fn prompt(tab: &Rc<Tab>, rename: bool) {
    let Some(owner) = tab.window() else {
        return;
    };
    let Some(parent) = tab.path() else {
        return;
    };
    let source = if rename {
        let Some(path) = tab.preview_path() else {
            return;
        };
        Some(path)
    } else {
        None
    };
    let dialog = gtk::Window::builder()
        .transient_for(&owner.native)
        .modal(true)
        .destroy_with_parent(true)
        .title(if rename { "Rename" } else { "New folder" })
        .default_width(400)
        .build();
    dialog.add_css_class("canopa-files");
    if let Some(app) = owner.app.upgrade() {
        dialog.set_application(Some(&app.application));
    }
    let header = gtk::HeaderBar::new();
    dialog.set_titlebar(Some(&header));
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
    dialog.set_child(Some(&panel));
    panel.set_margin_start(20);
    panel.set_margin_end(20);
    panel.set_margin_top(20);
    panel.set_margin_bottom(20);
    let entry = gtk::Entry::new();
    entry.set_activates_default(true);
    entry.set_hexpand(true);
    entry.update_property(&[gtk::accessible::Property::Label("Name")]);
    let old_name = source.as_ref().map(|path| {
        path.file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned()
    });
    entry.set_text(old_name.as_deref().unwrap_or("New Folder"));
    entry.select_region(0, -1);
    panel.append(&entry);
    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_wrap(true);
    error.set_visible(false);
    panel.append(&error);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let accept = gtk::Button::with_label(if rename { "Rename" } else { "Create" });
    buttons.append(&cancel);
    buttons.append(&accept);
    panel.append(&buttons);
    dialog.set_default_widget(Some(&accept));
    let window = dialog.downgrade();
    cancel.connect_clicked(move |_| {
        if let Some(window) = window.upgrade() {
            window.destroy();
        }
    });
    let weak = Rc::downgrade(tab);
    let window = dialog.downgrade();
    let input = entry.clone();
    accept.connect_clicked(move |_| {
        let Some(dialog) = window.upgrade() else {
            return;
        };
        let name = input.text();
        if name.is_empty() || matches!(name.as_str(), "." | "..") || name.contains('/') {
            error.set_label("Enter one name without a slash.");
            error.set_visible(true);
            return;
        }
        if old_name.as_deref() == Some(name.as_str()) {
            dialog.destroy();
            return;
        }
        if let Some(tab) = weak.upgrade().filter(|_| !rename || source.is_some())
            && let Some(app) = tab.window().and_then(|window| window.app.upgrade())
        {
            let request = if let Some(source) = &source {
                OperationRequest::Rename {
                    source: source.clone(),
                    destination: source.parent().unwrap_or(&parent).join(name.as_str()),
                }
            } else {
                OperationRequest::CreateFolder {
                    path: parent.join(name.as_str()),
                }
            };
            run(&app, &tab, Some(request));
        }
        dialog.destroy();
    });
    let controller = gtk::EventControllerKey::new();
    let window = dialog.downgrade();
    controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(window) = window.upgrade() {
                window.destroy();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    dialog.add_controller(controller);
    dialog.present();
    entry.grab_focus();
}

pub(super) fn undo(tab: &Rc<Tab>) {
    if let Some(app) = tab.window().and_then(|window| window.app.upgrade()) {
        run(&app, tab, None);
    }
}

pub(super) fn install_menu(tab: &Rc<Tab>, panel: &gtk::Box, popup: &gtk::Popover) {
    super::clipboard::install_menu(tab, panel, popup);
    let recovery = gtk::Button::with_label("Recovery…");
    recovery.add_css_class("flat");
    panel.append(&recovery);
    let weak = Rc::downgrade(tab);
    let popup_weak = popup.downgrade();
    recovery.connect_clicked(move |_| {
        if let Some(popup) = popup_weak.upgrade() {
            popup.popdown();
        }
        if let Some(tab) = weak.upgrade() {
            tab.command(crate::interface::Command::Recovery);
        }
    });
    for (label, show) in [("Move to Trash", false), ("Show Trash", true)] {
        let button = gtk::Button::with_label(label);
        button.add_css_class("flat");
        panel.append(&button);
        let weak = Rc::downgrade(tab);
        let popup = popup.downgrade();
        button.connect_clicked(move |_| {
            if let Some(popup) = popup.upgrade() {
                popup.popdown();
            }
            if let Some(tab) = weak.upgrade() {
                if show {
                    if let Some(app) = tab.window().and_then(|window| window.app.upgrade()) {
                        super::trash_ui::show(&app);
                    }
                } else {
                    super::trash_ui::move_selection(&tab);
                }
            }
        });
    }
    for (label, rename) in [("New folder…", false), ("Rename…", true)] {
        let button = gtk::Button::with_label(label);
        button.add_css_class("flat");
        panel.append(&button);
        let weak = Rc::downgrade(tab);
        let popup = popup.downgrade();
        button.connect_clicked(move |_| {
            if let Some(popup) = popup.upgrade() {
                popup.popdown();
            }
            if let Some(tab) = weak.upgrade() {
                prompt(&tab, rename);
            }
        });
    }
    let button = gtk::Button::with_label("Undo");
    button.add_css_class("flat");
    panel.append(&button);
    let weak = Rc::downgrade(tab);
    let popup_weak = popup.downgrade();
    button.connect_clicked(move |_| {
        if let Some(popup) = popup_weak.upgrade() {
            popup.popdown();
        }
        if let Some(tab) = weak.upgrade() {
            undo(&tab);
        }
    });
    let weak = Rc::downgrade(tab);
    popup.connect_show(move |_| {
        if let Some(app) = weak
            .upgrade()
            .and_then(|tab| tab.window())
            .and_then(|window| window.app.upgrade())
        {
            button.set_label(&app.undo_label.borrow());
            button.set_sensitive(
                app.operations.borrow().is_some() && *app.undo_label.borrow() != "Nothing to undo",
            );
        }
    });
    panel.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
}
