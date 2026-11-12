//! Taskbar runtime: keeps one bar child per application in step with Niri's
//! windows and the configured pins, and turns clicks into focus or launches.
use std::collections::HashMap;

use iced::Task;

use super::{App, Message};
use crate::config::ModuleParams;
use crate::modules::taskbar::{self, TaskbarEntry, TaskbarEvent};

/// The entries last rendered, keyed by the taskbar's configured name.
#[derive(Default)]
pub(super) struct TaskbarRuntime {
    entries: HashMap<String, Vec<TaskbarEntry>>,
}

impl TaskbarRuntime {
    /// Forget what was rendered, after configuration rebuilt the modules.
    pub(super) fn invalidate(&mut self) {
        self.entries.clear();
    }

    fn entry(&self, key: &str) -> Option<&TaskbarEntry> {
        self.entries
            .values()
            .flatten()
            .find(|entry| entry.key == key)
    }
}

#[cfg(not(test))]
fn applications() -> &'static [crate::desktop_apps::DesktopApplication] {
    crate::focused_window_icons::installed_applications()
}

// Unit tests never read the host's installed applications.
#[cfg(test)]
fn applications() -> &'static [crate::desktop_apps::DesktopApplication] {
    &[]
}

impl App {
    /// Rebuild taskbar children when the applications or their state changed.
    pub(super) fn refresh_taskbar(&mut self) {
        let parents: Vec<(String, Vec<String>)> = self
            .config
            .modules
            .iter()
            .filter_map(|module| match &module.params {
                ModuleParams::Taskbar(config) => Some((module.name.clone(), config.pinned.clone())),
                _ => None,
            })
            .collect();
        if parents.is_empty() {
            self.taskbar.entries.clear();
            return;
        }
        let state = self.niri.state();
        let windows: Vec<_> = state.windows.values().collect();
        // While a Shell surface holds the keyboard, Niri reports no focused
        // window; the focused workspace's active window is still in front.
        let focused = state.focused_window_id.or_else(|| {
            state
                .workspaces
                .iter()
                .find(|workspace| workspace.is_focused)
                .and_then(|workspace| workspace.active_window_id)
        });
        let desired: HashMap<_, _> = parents
            .into_iter()
            .map(|(name, pinned)| {
                let entries = taskbar::entries(&pinned, &windows, focused, applications());
                #[cfg(not(test))]
                let entries = entries
                    .into_iter()
                    .map(|mut entry| {
                        entry.icon =
                            crate::focused_window_icons::resolve_application_icon(&entry.key);
                        entry
                    })
                    .collect();
                (name, entries)
            })
            .collect();
        if desired == self.taskbar.entries {
            return;
        }
        self.reconcile_taskbar_dynamic_modules(&desired);
        self.taskbar.entries = desired;
    }

    pub(super) fn handle_taskbar_event(&mut self, event: TaskbarEvent) -> Task<Message> {
        let (key, launch) = match event {
            TaskbarEvent::Activate { key } => (key, false),
            TaskbarEvent::Launch { key } => (key, true),
        };
        let Some(entry) = self.taskbar.entry(&key) else {
            return Task::none();
        };
        let window = (!launch)
            .then(|| taskbar::window_to_focus(entry, self.niri.state().focused_window_id))
            .flatten();
        match (window, entry.desktop_id.clone()) {
            (Some(id), _) => {
                #[cfg(not(test))]
                tokio::spawn(async move {
                    if let Err(error) = crate::command::focus_window(id).await {
                        tracing::warn!(target: "canopa_shell::modules", %error, "taskbar focus failed");
                    }
                });
                #[cfg(test)]
                let _ = id;
            }
            (None, Some(desktop_id)) => {
                let Some(app) = applications()
                    .iter()
                    .find(|app| app.id == desktop_id)
                    .cloned()
                else {
                    return Task::none();
                };
                #[cfg(not(test))]
                tokio::spawn(async move {
                    if let Err(error) =
                        crate::command::spawn_desktop_application_fire_and_forget(&app, None).await
                    {
                        tracing::warn!(target: "canopa_shell::modules", %error, app = %app.id, "taskbar launch failed");
                    }
                });
                #[cfg(test)]
                let _ = app;
            }
            (None, None) => {}
        }
        Task::none()
    }
}
