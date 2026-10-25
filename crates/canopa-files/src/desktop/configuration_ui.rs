//! Only one source preparation runs at a time. UI installation fences graphical
//! saves/reloads and keeps unsaved editors separate from the running candidate.
use super::Desktop;
use crate::configuration::{ConfigurationRuntime, ReloadEvent};
use gtk::{gio, glib};
use std::{rc::Rc, time::Duration};

pub(super) fn initialize(app: &Rc<Desktop>) {
    let mut runtime =
        ConfigurationRuntime::from_snapshot(app.action_configuration.borrow().clone());
    if !runtime.is_watching() {
        return;
    }
    let weak = Rc::downgrade(app);
    glib::MainContext::default().spawn_local(async move {
        loop {
            glib::timeout_future(Duration::from_millis(250)).await;
            let Some(revision) = weak.upgrade().map(|app| app.configuration_revision.get()) else {
                break;
            };
            let result = gio::spawn_blocking(move || {
                let event = runtime.poll();
                (runtime, event)
            }).await;
            let Some(app) = weak.upgrade() else {
                break;
            };
            let (next, event) = match result {
                Ok(result) => result,
                Err(_) => {
                    app.configuration_status(Some("The configuration reader failed. Restart Files to try again.".into()));
                    break;
                }
            };
            runtime = next;
            if app.configuration_revision.get() != revision {
                runtime.invalidate();
                continue;
            }
            match event {
                ReloadEvent::Unchanged => {},
                ReloadEvent::Updated(snapshot) => {
                    app.install_configuration(*snapshot);
                    app.configuration_status(None);
                },
                ReloadEvent::Rejected(error) => app.configuration_status(Some(format!("Configuration was not reloaded. The last valid settings remain active. Fix and save the configuration to retry. {error}"))),
            }
        }
    });
}
