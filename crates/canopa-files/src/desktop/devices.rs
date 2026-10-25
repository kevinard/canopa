//! Local storage exposed by GIO. Backend discovery stays on the main context;
//! directory reads use the normal asynchronous browser navigation boundary.
use super::{Desktop, clear_box, icon_button, tab::Tab};
use gtk::{gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::PathBuf,
    rc::{Rc, Weak},
};

#[derive(Default)]
pub(super) struct Devices {
    monitor: RefCell<Option<gio::VolumeMonitor>>,
    handlers: RefCell<Vec<glib::SignalHandlerId>>,
    views: RefCell<Vec<View>>,
    queued: Cell<bool>,
    busy: Cell<bool>,
    removing: Cell<bool>,
    mounts: RefCell<BTreeMap<usize, (PathBuf, String, u64)>>,
}
impl Devices {
    pub(super) fn removing(&self) -> bool {
        self.removing.get()
    }
}
impl Drop for Devices {
    fn drop(&mut self) {
        if let Some(monitor) = self.monitor.get_mut().take() {
            for handler in self.handlers.get_mut().drain(..) {
                monitor.disconnect(handler);
            }
        }
    }
}
#[derive(Clone)]
struct View {
    tab: Weak<Tab>,
    panel: glib::WeakRef<gtk::Box>,
    popup: Option<glib::WeakRef<gtk::Popover>>,
}

#[derive(Clone)]
enum Device {
    Volume(gio::Volume),
    Mount(gio::Mount),
}
impl Device {
    fn mount(&self) -> Option<gio::Mount> {
        match self {
            Self::Volume(volume) => volume.get_mount(),
            Self::Mount(mount) => Some(mount.clone()),
        }
    }
    fn name(&self) -> glib::GString {
        match self {
            Self::Volume(volume) => volume.name(),
            Self::Mount(mount) => mount.name(),
        }
    }
    fn identity(&self) -> usize {
        match self {
            Self::Volume(volume) => volume.as_ptr() as usize,
            Self::Mount(mount) => mount.as_ptr() as usize,
        }
    }
    fn can_open(&self) -> bool {
        self.mount().is_some() || matches!(self, Self::Volume(volume) if volume.can_mount())
    }
}

fn catalog(monitor: &gio::VolumeMonitor) -> Vec<Device> {
    let volumes: Vec<_> = monitor
        .volumes()
        .into_iter()
        .filter(|volume| {
            volume.identifier("class").as_deref() != Some("network")
                && volume
                    .activation_root()
                    .is_none_or(|root| root.path().is_some())
                && volume
                    .get_mount()
                    .is_none_or(|mount| !mount.is_shadowed() && mount.root().path().is_some())
        })
        .collect();
    let mut devices: Vec<_> = volumes.iter().cloned().map(Device::Volume).collect();
    devices.extend(
        monitor
            .mounts()
            .into_iter()
            .filter(|mount| {
                !mount.is_shadowed()
                    && mount.root().path().is_some()
                    && mount
                        .volume()
                        .is_none_or(|volume| !volumes.contains(&volume))
            })
            .map(Device::Mount),
    );
    devices.sort_by_key(|device| device.name().to_lowercase());
    devices
}

pub(super) fn initialize(app: &Rc<Desktop>) {
    let monitor = gio::VolumeMonitor::get();
    for signal in [
        "volume-added",
        "volume-removed",
        "volume-changed",
        "mount-added",
        "mount-removed",
        "mount-changed",
        "drive-connected",
        "drive-disconnected",
        "drive-changed",
    ] {
        let weak = Rc::downgrade(app);
        let handler = monitor.connect_local(signal, false, move |values| {
            if let Some(app) = weak.upgrade() {
                if signal == "mount-removed"
                    && let Ok(mount) = values[1].get::<gio::Mount>()
                    && let Some((path, _, epoch)) = app
                        .devices
                        .mounts
                        .borrow_mut()
                        .remove(&(mount.as_ptr() as usize))
                {
                    app.storage.removed(&path, epoch);
                }
                queue_refresh(&app);
            }
            None
        });
        app.devices.handlers.borrow_mut().push(handler);
    }
    *app.devices.monitor.borrow_mut() = Some(monitor);
    sync_storage(app);
}

fn sync_storage(app: &Rc<Desktop>) {
    let Some(monitor) = app.devices.monitor.borrow().clone() else {
        return;
    };
    let mounts: BTreeMap<_, _> = monitor
        .mounts()
        .into_iter()
        .filter(|mount| !mount.is_shadowed())
        .filter_map(|mount| {
            let path = mount.root().path()?;
            let device = mount
                .uuid()
                .filter(|uuid| !uuid.is_empty())
                .or_else(|| {
                    mount
                        .volume()
                        .and_then(|volume| volume.uuid())
                        .filter(|uuid| !uuid.is_empty())
                })
                .map(|uuid| format!("uuid:{uuid}"))
                .unwrap_or_else(|| format!("location:{}", mount.root().uri()));
            Some((mount.as_ptr() as usize, (path, device)))
        })
        .collect();
    let previous = app.devices.mounts.borrow().clone();
    for (key, (path, device, epoch)) in &previous {
        if mounts
            .get(key)
            .is_none_or(|(now, id)| now != path || id != device)
        {
            app.storage.removed(path, *epoch);
            app.devices.mounts.borrow_mut().remove(key);
        }
    }
    for (key, (path, device)) in mounts {
        if app
            .devices
            .mounts
            .borrow()
            .get(&key)
            .is_some_and(|(old, id, _)| old == &path && id == &device)
        {
            continue;
        }
        match app.storage.mounted(path.clone(), device.clone()) {
            Ok(observation) => {
                app.devices
                    .mounts
                    .borrow_mut()
                    .insert(key, (path, device, observation.epoch));
                let storage = app.storage.clone();
                let weak = Rc::downgrade(app);
                let hold = app.application.hold();
                glib::MainContext::default().spawn_local(async move {
                    let _hold = hold;
                    let result = gio::spawn_blocking(move || storage.prepare(observation)).await;
                    let message = match result {
                        Ok(Ok(())) => return,
                        Ok(Err(error)) => format!("Storage protection could not be prepared: {error}. Reconnect the device after correcting the problem."),
                        Err(_) => String::from("The storage protection worker failed. Restart Files before using the device."),
                    };
                    if let Some(app) = weak.upgrade() {
                        for window in app.windows.borrow().iter() {
                            if let Some(tab) = window.active_tab() { tab.show_error(&message); }
                        }
                    }
                });
            }
            Err(error) => {
                for window in app.windows.borrow().iter() {
                    if let Some(tab) = window.active_tab() {
                        tab.show_error(&error.to_string());
                    }
                }
            }
        }
    }
}

pub(super) fn append(tab: &Rc<Tab>, panel: &gtk::Box, popup: Option<&gtk::Popover>) {
    let Some(app) = tab.window().and_then(|window| window.app.upgrade()) else {
        return;
    };
    let group = gtk::Box::new(gtk::Orientation::Vertical, 2);
    panel.append(&group);
    let view = View {
        tab: Rc::downgrade(tab),
        panel: group.downgrade(),
        popup: popup.map(glib::object::ObjectExt::downgrade),
    };
    app.devices
        .views
        .borrow_mut()
        .retain(|view| view.panel.upgrade().is_some() && view.tab.upgrade().is_some());
    app.devices.views.borrow_mut().push(view.clone());
    draw(&app, &view);
}

fn queue_refresh(app: &Rc<Desktop>) {
    if app.devices.queued.replace(true) {
        return;
    }
    let weak = Rc::downgrade(app);
    glib::idle_add_local_once(move || {
        if let Some(app) = weak.upgrade() {
            app.devices.queued.set(false);
            sync_storage(&app);
            let views = {
                let mut views = app.devices.views.borrow_mut();
                views.retain(|view| view.panel.upgrade().is_some() && view.tab.upgrade().is_some());
                views.clone()
            };
            for view in views {
                draw(&app, &view);
            }
        }
    });
}

fn draw(app: &Rc<Desktop>, view: &View) {
    let (Some(panel), Some(tab), Some(monitor)) = (
        view.panel.upgrade(),
        view.tab.upgrade(),
        app.devices.monitor.borrow().clone(),
    ) else {
        return;
    };
    let focused = panel
        .root()
        .and_downcast::<gtk::Window>()
        .and_then(|window| GtkWindowExt::focus(&window))
        .filter(|widget| widget.is_ancestor(&panel))
        .map(|widget| widget.widget_name());
    clear_box(&panel);
    let devices = catalog(&monitor);
    panel.set_visible(!devices.is_empty());
    if devices.is_empty() {
        if focused.is_some() {
            tab.command(crate::interface::Command::Location);
        }
        return;
    }
    let heading = gtk::Label::new(Some("Devices"));
    heading.set_xalign(0.0);
    heading.add_css_class("column-heading");
    panel.append(&heading);
    let mut restore = None;
    for device in devices {
        let mounted = device.mount();
        let path = mounted.as_ref().and_then(|mount| mount.root().path());
        let button = gtk::Button::with_label(&device.name());
        button.set_widget_name(&format!("device-open-{}", device.identity()));
        button.set_hexpand(true);
        button.set_sensitive(!app.devices.busy.get() && device.can_open());
        button.add_css_class("flat");
        button.set_tooltip_text(Some(&path.as_ref().map_or_else(
            || format!("Mount {}", device.name()),
            |path| path.display().to_string(),
        )));
        if let Some(label) = button.child().and_downcast::<gtk::Label>() {
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(24);
            label.set_xalign(0.0);
        }
        let weak = Rc::downgrade(&tab);
        let popup = view.popup.clone();
        let target = device.clone();
        button.connect_clicked(move |_| {
            if let Some(popup) = popup.as_ref().and_then(glib::WeakRef::upgrade) {
                popup.popdown();
            }
            if let Some(tab) = weak.upgrade() {
                if let Some(path) = path.as_ref() {
                    tab.navigate(path.clone(), None);
                } else if let Device::Volume(volume) = &target {
                    mount_volume(&tab, volume.clone());
                }
            }
        });
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        row.append(&button);
        if focused.as_ref() == Some(&button.widget_name()) {
            restore = Some(button.clone());
        }
        if let Some(mount) = mounted.filter(|mount| mount.can_unmount()) {
            let remove = icon_button("media-eject-symbolic", &format!("Unmount {}", mount.name()));
            remove.set_widget_name(&format!("device-remove-{}", device.identity()));
            remove.set_sensitive(!app.devices.busy.get());
            let weak = Rc::downgrade(&tab);
            let popup = view.popup.clone();
            remove.connect_clicked(move |_| {
                if let Some(popup) = popup.as_ref().and_then(glib::WeakRef::upgrade) {
                    popup.popdown();
                }
                if let Some(tab) = weak.upgrade() {
                    unmount(&tab, mount.clone());
                }
            });
            row.append(&remove);
            if focused.as_ref() == Some(&remove.widget_name()) {
                restore = Some(remove);
            }
        }
        panel.append(&row);
    }
    if !restore.is_some_and(|button| button.grab_focus()) && focused.is_some() {
        tab.command(crate::interface::Command::Location);
    }
}

fn mount_volume(tab: &Rc<Tab>, volume: gio::Volume) {
    tab.focus_listing();
    let Some(window) = tab.window() else { return };
    let Some(app) = window.app.upgrade() else {
        return;
    };
    if app.devices.busy.replace(true) {
        return;
    }
    queue_refresh(&app);
    let operation = gtk::MountOperation::new(Some(&window.native));
    let hold = app.application.hold();
    let origin = Rc::downgrade(tab);
    let intent = tab.navigation_token();
    glib::MainContext::default().spawn_local(async move {
        let _hold = hold;
        let result = volume
            .mount_future(gio::MountMountFlags::NONE, Some(&operation))
            .await;
        app.devices.busy.set(false);
        queue_refresh(&app);
        let Some(tab) = origin
            .upgrade()
            .filter(|tab| tab.navigation_token() == intent)
        else {
            return;
        };
        match result {
            Ok(()) => {
                let present = app
                    .devices
                    .monitor
                    .borrow()
                    .as_ref()
                    .is_some_and(|monitor| monitor.volumes().contains(&volume));
                if let Some(path) = volume
                    .get_mount()
                    .filter(|_| present)
                    .and_then(|mount| mount.root().path())
                {
                    tab.navigate(path, None);
                } else {
                    tab.show_error("The device is no longer available as a local folder.");
                }
            }
            Err(error) if !error.matches(gio::IOErrorEnum::Cancelled) => {
                tab.show_error(&format!("Could not mount {}: {error}", volume.name()))
            }
            Err(_) => {}
        }
    });
}

fn unmount(tab: &Rc<Tab>, mount: gio::Mount) {
    tab.focus_listing();
    let Some(window) = tab.window() else { return };
    let Some(app) = window.app.upgrade() else {
        return;
    };
    if app.devices.busy.get() {
        return;
    }
    if app.jobs.has_active() {
        tab.show_error("Wait for Files operations to finish, or cancel them in Operations, before removing a device.");
        return;
    }
    app.devices.busy.set(true);
    app.devices.removing.set(true);
    queue_refresh(&app);
    let operation = gtk::MountOperation::new(Some(&window.native));
    let hold = app.application.hold();
    let origin = Rc::downgrade(tab);
    glib::MainContext::default().spawn_local(async move {
        let _hold = hold;
        let result = mount
            .unmount_with_operation_future(gio::MountUnmountFlags::NONE, Some(&operation))
            .await;
        app.devices.busy.set(false);
        app.devices.removing.set(false);
        queue_refresh(&app);
        super::jobs::device_finished(&app);
        if let Err(error) = result
            && !error.matches(gio::IOErrorEnum::Cancelled)
            && let Some(tab) = origin.upgrade()
        {
            tab.show_error(&format!("Could not unmount {}: {error}", mount.name()));
        }
    });
}
