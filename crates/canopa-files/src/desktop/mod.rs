//! Native GTK adapter. Filesystem work remains in toolkit-independent worker modules.
mod actions_ui;
mod appearance;
mod clipboard;
mod configuration_ui;
mod devices;
mod drag_drop;
mod jobs;
mod listing;
mod media_input;
mod operations_ui;
mod options;
mod preferences_ui;
mod quicklook;
mod recovery_ui;
use crate::storage;
mod tab;
mod trash_ui;
mod window;

use crate::preferences::{PreferenceStore, ViewSettings};
use gtk::{gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    io,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use window::BrowserWindow;

pub(super) struct Desktop {
    application: gtk::Application,
    windows: RefCell<Vec<Rc<BrowserWindow>>>,
    preferences: RefCell<PreferenceStore>,
    preference_path: PathBuf,
    preference_revision: Arc<AtomicU64>,
    preference_writer: Arc<Mutex<()>>,
    code_colors: Cell<[gtk::gdk::RGBA; 3]>,
    appearance_provider: RefCell<Option<gtk::CssProvider>>,
    configuration_revision: Cell<u64>,
    configuration_error: RefCell<Option<String>>,
    operations: RefCell<Option<Arc<crate::operations::OperationEngine>>>,
    undo_label: RefCell<String>,
    jobs: jobs::Queue,
    devices: devices::Devices,
    storage: Arc<storage::Storage>,
    clipboard: clipboard::Clipboard,
    operation_error: RefCell<Option<String>>,
    data_home: PathBuf,
    trash_window: RefCell<Option<Rc<trash_ui::TrashWindow>>>,
    recovery_window: RefCell<Option<Rc<recovery_ui::RecoveryWindow>>>,
    action_configuration: RefCell<crate::configuration::ConfigurationSnapshot>,
    action_editor: RefCell<Option<Rc<actions_ui::Editor>>>,
    preference_editor: RefCell<Option<Rc<preferences_ui::Editor>>>,
    active_drag: RefCell<Option<glib::WeakRef<gtk::DragSource>>>,
    drag_modifiers: Cell<gtk::gdk::ModifierType>,
    cancelled_drags: RefCell<Vec<glib::WeakRef<gtk::gdk::Drag>>>,
}

pub fn run(
    paths: Vec<PathBuf>,
    configuration: canopa_config::Request,
) -> io::Result<glib::ExitCode> {
    let action_configuration = crate::configuration::ConfigurationSnapshot::load(&configuration)
        .map_err(io::Error::other)?;
    let state_home = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(".local/state"))
        })
        .ok_or_else(|| io::Error::other("an absolute XDG_STATE_HOME or HOME is required"))?;
    let preference_path = state_home.join("canopa-files/views.json");
    let storage = storage::Storage::load(preference_path.with_file_name("volumes.json"))?;
    let preferences =
        PreferenceStore::load(&preference_path, action_configuration.settings.view.clone())?;
    let application = gtk::Application::builder()
        .application_id("org.canopa.Files")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let code_colors = code_colors(&action_configuration.appearance);
    let desktop = Rc::new(Desktop {
        application: application.clone(),
        windows: RefCell::new(Vec::new()),
        preferences: RefCell::new(preferences),
        preference_path,
        preference_revision: Arc::new(AtomicU64::new(0)),
        preference_writer: Arc::new(Mutex::new(())),
        code_colors: Cell::new(code_colors),
        appearance_provider: RefCell::new(None),
        configuration_revision: Cell::new(0),
        configuration_error: RefCell::new(None),
        operations: RefCell::new(None),
        undo_label: RefCell::new(String::from("Nothing to undo")),
        jobs: jobs::Queue::default(),
        devices: devices::Devices::default(),
        storage,
        clipboard: clipboard::Clipboard::default(),
        operation_error: RefCell::new(None),
        data_home: std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home().join(".local/share")),
        trash_window: RefCell::new(None),
        recovery_window: RefCell::new(None),
        action_configuration: RefCell::new(action_configuration),
        action_editor: RefCell::new(None),
        preference_editor: RefCell::new(None),
        active_drag: RefCell::new(None),
        drag_modifiers: Cell::new(gtk::gdk::ModifierType::empty()),
        cancelled_drags: RefCell::new(Vec::new()),
    });
    let weak = Rc::downgrade(&desktop);
    application.connect_startup(move |_| {
        if let Some(desktop) = weak.upgrade() {
            *desktop.appearance_provider.borrow_mut() = Some(appearance::install(
                &desktop.action_configuration.borrow().appearance,
            ));
            clipboard::initialize(&desktop);
            jobs::initialize(&desktop);
            trash_ui::initialize(&desktop);
            recovery_ui::initialize(&desktop);
            devices::initialize(&desktop);
            operations_ui::initialize(&desktop);
            actions_ui::initialize(&desktop);
            preferences_ui::initialize(&desktop);
            configuration_ui::initialize(&desktop);
        }
    });
    let weak = Rc::downgrade(&desktop);
    application.connect_activate(move |_| {
        let Some(desktop) = weak.upgrade() else {
            return;
        };
        let existing = desktop.windows.borrow().last().cloned();
        if let Some(existing) = existing {
            existing.native.present();
        } else {
            desktop.open_window(vec![home()]);
        }
    });
    let weak = Rc::downgrade(&desktop);
    application.connect_open(move |_, files, _| {
        let Some(desktop) = weak.upgrade() else {
            return;
        };
        let paths: Vec<_> = files.iter().filter_map(|file| file.path()).collect();
        if !paths.is_empty() {
            desktop.open_window(paths);
        }
    });
    let mut arguments = vec![String::from("canopa-files")];
    for path in paths {
        let file = gio::File::for_commandline_arg(path);
        if file.path().is_none() {
            return Err(io::Error::other(
                "only local files and file:// URIs are supported",
            ));
        }
        arguments.push(file.uri().to_string());
    }
    Ok(application.run_with_args(&arguments))
}

impl Desktop {
    fn install_configuration(
        self: &Rc<Self>,
        snapshot: crate::configuration::ConfigurationSnapshot,
    ) {
        let files_changed = !self.action_configuration.borrow().same_files(&snapshot);
        let appearance_changed =
            self.action_configuration.borrow().appearance != snapshot.appearance;
        if appearance_changed {
            if let Some(provider) = self.appearance_provider.borrow().as_ref() {
                appearance::update(provider, &snapshot.appearance);
            }
            self.code_colors.set(code_colors(&snapshot.appearance));
        }
        // Already validated as one immutable candidate by ConfigurationSnapshot.
        self.preferences
            .borrow_mut()
            .set_defaults(snapshot.settings.view.clone())
            .expect("validated view defaults");
        *self.action_configuration.borrow_mut() = snapshot;
        self.configuration_revision
            .set(self.configuration_revision.get().wrapping_add(1));
        for window in self.windows.borrow().iter() {
            if files_changed {
                window.preferences_changed(true);
            }
            if appearance_changed {
                window.appearance_changed();
            }
        }
        if appearance_changed {
            if let Some(window) = self.trash_window.borrow().as_ref() {
                window.appearance_changed();
            }
            if let Some(window) = self.recovery_window.borrow().as_ref() {
                window.appearance_changed();
            }
        }
        if let Some(editor) = self.action_editor.borrow().as_ref() {
            editor.configuration_changed();
        }
        if let Some(editor) = self.preference_editor.borrow().as_ref() {
            editor.configuration_changed();
        }
    }
    fn configuration_status(&self, message: Option<String>) {
        *self.configuration_error.borrow_mut() = message;
        for window in self.windows.borrow().iter() {
            window.configuration_status(self.configuration_error.borrow().as_deref());
        }
    }

    fn open_window(self: &Rc<Self>, paths: Vec<PathBuf>) {
        let window = BrowserWindow::new(self);
        window.configuration_status(self.configuration_error.borrow().as_deref());
        self.windows.borrow_mut().push(window.clone());
        for path in paths {
            window.add_tab(path);
        }
        window.native.present();
    }

    fn store_view(self: &Rc<Self>, folder: &Path, view: Option<ViewSettings>) -> io::Result<()> {
        let snapshot = {
            let mut preferences = self.preferences.borrow_mut();
            if let Some(view) = view {
                preferences.remember_changes(folder, view)?;
            } else {
                preferences.use_defaults(folder);
            }
            preferences.clone()
        };
        for window in self.windows.borrow().iter() {
            window.preferences_changed(false);
        }
        let revision = self.preference_revision.fetch_add(1, Ordering::AcqRel) + 1;
        let latest = self.preference_revision.clone();
        let writer = self.preference_writer.clone();
        let path = self.preference_path.clone();
        let weak = Rc::downgrade(self);
        let hold = self.application.hold();
        glib::MainContext::default().spawn_local(async move {
            let _hold = hold;
            let result = gio::spawn_blocking(move || {
                let _guard = writer
                    .lock()
                    .map_err(|_| io::Error::other("preference writer failed"))?;
                if latest.load(Ordering::Acquire) != revision {
                    return Ok(());
                }
                snapshot.save(&path)
            })
            .await;
            if let Some(desktop) = weak.upgrade() {
                let error = match result {
                    Ok(Err(error)) => Some(error.to_string()),
                    Err(_) => Some("preference writer failed".into()),
                    _ => None,
                };
                if let Some(error) = error {
                    for window in desktop.windows.borrow().iter() {
                        if let Some(tab) = window.active_tab() {
                            tab.show_error(&format!("Could not save folder preferences: {error}"));
                        }
                    }
                }
            }
        });
        Ok(())
    }
}

fn code_colors(appearance: &canopa_theme::Appearance) -> [gtk::gdk::RGBA; 3] {
    let palette = appearance.palette();
    [palette.success, palette.warning, palette.muted].map(|canopa_theme::Rgb(r, g, b)| {
        gtk::gdk::RGBA::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            1.0,
        )
    })
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn icon_button(icon: &str, label: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button.add_css_class("flat");
    button
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn key_chord(
    key: gtk::gdk::Key,
    modifiers: gtk::gdk::ModifierType,
) -> Option<crate::interface::KeyChord> {
    let name = gtk::accelerator_name(
        key.to_lower(),
        modifiers & gtk::accelerator_get_default_mod_mask(),
    );
    crate::interface::KeyChord::parse(&name).ok()
}
