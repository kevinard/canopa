//! Taskbar runtime: keeps one bar child per application in step with Niri's
//! windows and the pins, turns clicks into focus or launches, and runs the
//! application menu. Pins changed from the bar are saved as edits over each
//! taskbar's configured `pinned` list, in the Shell's own state file.
use std::collections::HashMap;
use std::time::{Duration, Instant};

use iced::Task;

use super::{App, Message};
use crate::config::ModuleParams;
use crate::modules::taskbar::{self, MenuCommand, MenuRow, PinStore, TaskbarEntry, TaskbarEvent};

/// How long a requested focus change stands in for Niri's report of it, so a
/// quick second click cycles on instead of repeating the first.
const FOCUS_REQUEST_GRACE: Duration = Duration::from_millis(1500);

/// The entries last rendered, keyed by the taskbar's configured name.
#[derive(Default)]
pub(super) struct TaskbarRuntime {
    entries: HashMap<String, Vec<TaskbarEntry>>,
    /// Loaded on first use; edits are kept even when saving fails.
    pins: Option<PinStore>,
    pin_writer: Option<std::sync::mpsc::Sender<Vec<u8>>>,
    focus_request: Option<FocusRequest>,
    menu: Option<OpenMenu>,
    /// Window thumbnails for taskbar tooltips, by window ID.
    previews: HashMap<u64, Preview>,
}

/// A window thumbnail from Niri's preview service.
struct Preview {
    handle: iced::widget::image::Handle,
    width: u32,
    height: u32,
}

/// Thumbnails fit this square, in logical pixels.
const PREVIEW_SIZE: u32 = 192;
/// A tooltip shows at most this many of an application's windows.
const MAX_PREVIEWS: usize = 4;
/// A focus change the taskbar asked Niri for and Niri may not have reported.
struct FocusRequest {
    window: u64,
    /// The window in front when the request was made.
    replaced: Option<u64>,
    at: Instant,
}

/// The rows of the taskbar menu on screen, for mapping a selection back.
struct OpenMenu {
    taskbar: String,
    key: String,
    rows: Vec<MenuRow>,
    /// The window the layout commands act on.
    window: Option<u64>,
}

impl TaskbarRuntime {
    /// Forget what was rendered, after configuration rebuilt the modules.
    pub(super) fn invalidate(&mut self) {
        self.entries.clear();
        self.menu = None;
    }

    fn entry(&self, key: &str) -> Option<&TaskbarEntry> {
        self.entries
            .values()
            .flatten()
            .find(|entry| entry.key == key)
    }

    fn entry_in(&self, taskbar: &str, key: &str) -> Option<&TaskbarEntry> {
        self.entries
            .get(taskbar)?
            .iter()
            .find(|entry| entry.key == key)
    }

    fn pins(&mut self) -> &PinStore {
        self.pins.get_or_insert_with(load_pins)
    }

    /// The window a click treats as current: one the taskbar just focused
    /// while Niri still reports the window it replaced.
    fn current_window(&self, front: Option<u64>) -> Option<u64> {
        match &self.focus_request {
            Some(request)
                if request.at.elapsed() < FOCUS_REQUEST_GRACE && front == request.replaced =>
            {
                Some(request.window)
            }
            _ => front,
        }
    }

    fn save_pins(&mut self) {
        let Some(bytes) = self.pins.as_ref().map(PinStore::to_bytes) else {
            return;
        };
        #[cfg(not(test))]
        {
            if self.pin_writer.is_none() {
                self.pin_writer = start_pin_writer();
            }
            if let Some(writer) = &self.pin_writer
                && writer.send(bytes).is_err()
            {
                self.pin_writer = None;
                tracing::warn!(target: "canopa_shell::modules", "taskbar pins could not be saved");
            }
        }
        #[cfg(test)]
        let _ = (bytes, &self.pin_writer);
    }
}

#[cfg(not(test))]
pub(super) fn applications() -> std::sync::Arc<Vec<crate::desktop_apps::DesktopApplication>> {
    crate::focused_window_icons::installed_applications()
}

// Unit tests never read the host's installed applications.
#[cfg(test)]
pub(super) fn applications() -> std::sync::Arc<Vec<crate::desktop_apps::DesktopApplication>> {
    Default::default()
}

#[cfg(not(test))]
fn pins_path() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".local/state"))
        })
        .map(|root| root.join("canopa/shell/taskbar-pins.json"))
}

#[cfg(not(test))]
fn load_pins() -> PinStore {
    let Some(path) = pins_path() else {
        return PinStore::default();
    };
    match canopa_file::private_state::read(&path, taskbar::PIN_STORE_MAX_BYTES) {
        Ok(bytes) => PinStore::parse(&bytes).unwrap_or_else(|error| {
            tracing::warn!(target: "canopa_shell::modules", %error, "taskbar pins ignored");
            PinStore::default()
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => PinStore::default(),
        Err(error) => {
            tracing::warn!(target: "canopa_shell::modules", %error, "taskbar pins unreadable");
            PinStore::default()
        }
    }
}

// Unit tests never read or write the host's pins.
#[cfg(test)]
fn load_pins() -> PinStore {
    PinStore::default()
}

/// One writer thread saves pin files in order, skipping to the newest.
#[cfg(not(test))]
fn start_pin_writer() -> Option<std::sync::mpsc::Sender<Vec<u8>>> {
    let path = pins_path()?;
    let (sender, receiver) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::Builder::new()
        .name("taskbar-pins".into())
        .spawn(move || {
            while let Ok(mut bytes) = receiver.recv() {
                while let Ok(newer) = receiver.try_recv() {
                    bytes = newer;
                }
                if let Err(error) = canopa_file::private_state::write(&path, &bytes) {
                    tracing::warn!(target: "canopa_shell::modules", %error, "taskbar pins not saved");
                }
            }
        })
        .ok()?;
    Some(sender)
}

/// The edge-adjacent corner of an icon where its menu opens.
fn menu_anchor(edge: &crate::config::Edge, bounds: iced::Rectangle) -> (i32, i32) {
    match edge {
        crate::config::Edge::Top | crate::config::Edge::Bottom => {
            (bounds.x as i32, (bounds.y + bounds.height) as i32)
        }
        crate::config::Edge::Left | crate::config::Edge::Right => {
            ((bounds.x + bounds.width) as i32, bounds.y as i32)
        }
    }
}

impl App {
    /// The window in front: Niri's focused window, or, while a Shell surface
    /// holds the keyboard (as after clicking the bar), the focused
    /// workspace's active window.
    fn taskbar_front_window(&self) -> Option<u64> {
        let state = self.niri.state();
        state.focused_window_id.or_else(|| {
            state
                .workspaces
                .iter()
                .find(|workspace| workspace.is_focused)
                .and_then(|workspace| workspace.active_window_id)
        })
    }

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
        let pins = self.taskbar.pins().clone();
        let focused = self.current_taskbar_window();
        let state = self.niri.state();
        let windows: Vec<_> = state.windows.values().collect();
        let desired: HashMap<_, _> = parents
            .into_iter()
            .map(|(name, configured)| {
                let pinned = pins
                    .edits(&name)
                    .map_or(configured.clone(), |edits| edits.apply(&configured));
                let entries = taskbar::entries(
                    &pinned,
                    &windows,
                    &state.workspaces,
                    focused,
                    &applications(),
                );
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

    fn current_taskbar_window(&self) -> Option<u64> {
        self.taskbar.current_window(self.taskbar_front_window())
    }

    pub(super) fn handle_taskbar_event(&mut self, event: TaskbarEvent) -> Task<Message> {
        match event {
            TaskbarEvent::Activate { key } => self.activate_taskbar_entry(&key),
            TaskbarEvent::Launch { key } => {
                if let Some(desktop_id) =
                    self.taskbar.entry(&key).and_then(|e| e.desktop_id.clone())
                {
                    launch_application(&desktop_id);
                }
                Task::none()
            }
            TaskbarEvent::Menu {
                key,
                module,
                window,
            } => {
                let target = super::widget_ops::module_widget_id(window, &module);
                let op = super::widget_ops::FindBounds {
                    target,
                    result: None,
                };
                iced::advanced::widget::operate(op).then(move |bounds| {
                    Task::done(Message::Taskbar(TaskbarEvent::MenuAt {
                        key: key.clone(),
                        window,
                        bounds,
                    }))
                })
            }
            TaskbarEvent::Previews(previews) => {
                let open = &self.niri.state().windows;
                self.taskbar.previews.retain(|id, _| open.contains_key(id));
                for (id, width, height, png) in previews {
                    if open.contains_key(&id) {
                        self.taskbar.previews.insert(
                            id,
                            Preview {
                                handle: iced::widget::image::Handle::from_bytes(png),
                                width,
                                height,
                            },
                        );
                    }
                }
                self.refresh_taskbar_tooltips()
            }
            TaskbarEvent::MenuAt {
                key,
                window,
                bounds,
            } => self.open_taskbar_menu(key, window, bounds),
        }
    }

    fn activate_taskbar_entry(&mut self, key: &str) -> Task<Message> {
        let front = self.taskbar_front_window();
        let current = self.taskbar.current_window(front);
        // A dialog in front stands for the window it belongs to.
        let current = current.map(|id| {
            self.niri
                .state()
                .windows
                .get(&id)
                .and_then(|window| window.parent_id)
                .unwrap_or(id)
        });
        let Some(entry) = self.taskbar.entry(key) else {
            return Task::none();
        };
        match taskbar::window_to_focus(entry, current) {
            Some(id) => {
                self.taskbar.focus_request = Some(FocusRequest {
                    window: id,
                    replaced: front,
                    at: Instant::now(),
                });
                focus_window(id);
            }
            None => {
                if let Some(desktop_id) = entry.desktop_id.clone() {
                    launch_application(&desktop_id);
                }
            }
        }
        Task::none()
    }

    fn open_taskbar_menu(
        &mut self,
        key: String,
        window: iced::window::Id,
        bounds: Option<iced::Rectangle>,
    ) -> Task<Message> {
        let Some((taskbar, entry)) = self.taskbar.entries.iter().find_map(|(name, entries)| {
            entries
                .iter()
                .find(|entry| entry.key == key)
                .map(|entry| (name.clone(), entry))
        }) else {
            return Task::none();
        };
        let applications = applications();
        let application = entry
            .desktop_id
            .as_deref()
            .and_then(|id| applications.iter().find(|app| app.id == id));
        // Layout commands act on the window in front, if it is this
        // application's, otherwise on its most recent one.
        let front = self.current_taskbar_window();
        let target_window = front
            .filter(|id| entry.windows.contains(id))
            .or_else(|| entry.windows.first().copied());
        let state = self.niri.state();
        let target = target_window
            .and_then(|id| state.windows.get(&id))
            .map(|window| taskbar::LayoutTarget::of(window, &state.workspaces));
        let rows = taskbar::menu(entry, application, target);
        if rows.is_empty() {
            return Task::none();
        }
        let Some(surface) = self.surfaces.surface(window) else {
            return Task::none();
        };
        let screen_size = surface.screen_size;
        let edge = Self::bar_for_surface_name(&self.config.bars, &surface.bar_name)
            .edge
            .clone();
        let anchor = match bounds {
            Some(bounds) => menu_anchor(&edge, bounds),
            None => {
                let point = self
                    .surfaces
                    .last_cursor()
                    .map_or(iced::Point::ORIGIN, |(_, point)| point);
                (point.x as i32, point.y as i32)
            }
        };
        let tree = taskbar::menu_tree(&rows, anchor, screen_size);
        self.taskbar.menu = Some(OpenMenu {
            taskbar,
            key,
            rows,
            window: target_window,
        });
        self.open_menu_popup(
            taskbar::MENU_SERVICE.to_owned(),
            zbus::zvariant::OwnedObjectPath::try_from(taskbar::MENU_PATH)
                .expect("valid taskbar menu path"),
            tree,
            window,
        )
    }

    /// Run the taskbar menu row the person chose.
    pub(super) fn handle_taskbar_menu_item(&mut self, item: i32) {
        let Some(menu) = self.taskbar.menu.take() else {
            return;
        };
        let Some(Some((_, command))) = usize::try_from(item)
            .ok()
            .and_then(|position| menu.rows.get(position))
        else {
            return;
        };
        if let MenuCommand::Layout(command) = command {
            if let Some(window) = menu.window {
                run_layout_actions(taskbar::layout_actions(*command, window));
            }
            return;
        }
        let Some(desktop_id) = self
            .taskbar
            .entry_in(&menu.taskbar, &menu.key)
            .and_then(|entry| entry.desktop_id.clone())
        else {
            return;
        };
        match command {
            // Run above: they need a window, not a desktop file.
            MenuCommand::Layout(_) => {}
            MenuCommand::Launch => launch_application(&desktop_id),
            MenuCommand::DesktopAction(action) => launch_desktop_action(&desktop_id, action),
            MenuCommand::Pin | MenuCommand::Unpin => {
                let configured = self
                    .config
                    .modules
                    .iter()
                    .find(|module| module.name == menu.taskbar)
                    .and_then(|module| match &module.params {
                        ModuleParams::Taskbar(config) => Some(config.pinned.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                self.taskbar.pins();
                if let Some(pins) = self.taskbar.pins.as_mut() {
                    pins.set(
                        &menu.taskbar,
                        &configured,
                        &desktop_id,
                        *command == MenuCommand::Pin,
                    );
                }
                self.taskbar.save_pins();
                self.refresh_taskbar();
            }
        }
    }
}

impl App {
    /// The taskbar entry a taskbar item module shows.
    fn taskbar_entry_for_module(&self, module: &crate::modules::ModuleId) -> Option<&TaskbarEntry> {
        let taskbar = module.parent_name()?;
        self.taskbar.entries.get(taskbar)?.iter().find(|entry| {
            crate::modules::taskbar::TaskbarItemModule::module_id(taskbar, &entry.key) == *module
        })
    }

    /// The thumbnails known for a taskbar item's windows, in screen order.
    pub(super) fn taskbar_cached_previews(
        &self,
        module: &crate::modules::ModuleId,
    ) -> Vec<crate::surface::tooltip_popup::TooltipPreview> {
        let Some(entry) = self.taskbar_entry_for_module(module) else {
            return Vec::new();
        };
        entry
            .layout_order
            .iter()
            .take(MAX_PREVIEWS)
            .filter_map(|id| self.taskbar.previews.get(id))
            .map(|preview| {
                // Fit the square, keeping the window's aspect.
                let scale = (PREVIEW_SIZE as f32 / preview.width.max(preview.height).max(1) as f32)
                    .min(1.0);
                crate::surface::tooltip_popup::TooltipPreview {
                    handle: preview.handle.clone(),
                    width: (preview.width as f32 * scale).round(),
                    height: (preview.height as f32 * scale).round(),
                }
            })
            .collect()
    }

    /// Thumbnails to show now for a taskbar item's tooltip, and a request for
    /// fresh ones. Other modules get none.
    pub(super) fn taskbar_tooltip_previews(
        &self,
        module: &crate::modules::ModuleId,
    ) -> (
        Vec<crate::surface::tooltip_popup::TooltipPreview>,
        Task<Message>,
    ) {
        let Some(entry) = self.taskbar_entry_for_module(module) else {
            return (Vec::new(), Task::none());
        };
        let windows: Vec<u64> = entry
            .layout_order
            .iter()
            .take(MAX_PREVIEWS)
            .copied()
            .collect();
        let request = match self.shared.dbus.session.clone() {
            Some(connection) if !windows.is_empty() && !cfg!(test) => {
                Task::perform(capture_previews(connection, windows), |previews| {
                    Message::Taskbar(TaskbarEvent::Previews(previews))
                })
            }
            _ => Task::none(),
        };
        (self.taskbar_cached_previews(module), request)
    }
}

/// Ask Niri's preview service (the Canopa fork's
/// `org.canopa.Niri.WindowPreview1`) for each window's thumbnail. Windows it
/// cannot render, or a compositor without the service, give none.
async fn capture_previews(
    connection: zbus::Connection,
    windows: Vec<u64>,
) -> Vec<(u64, u32, u32, Vec<u8>)> {
    let Some(client) = canopa_services::window_preview::Client::connect(connection, None).await
    else {
        return Vec::new();
    };
    let mut previews = Vec::new();
    // Keep the tooltip responsive even if several previews are slow to render.
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        for id in windows {
            if let Some(preview) = client.capture(id, PREVIEW_SIZE).await {
                previews.push((id, preview.width, preview.height, preview.png));
            }
        }
    })
    .await;
    previews
}

fn focus_window(id: u64) {
    #[cfg(not(test))]
    tokio::spawn(async move {
        if let Err(error) = crate::command::focus_window(id).await {
            tracing::warn!(target: "canopa_shell::modules", %error, "taskbar focus failed");
        }
    });
    #[cfg(test)]
    let _ = id;
}

fn run_layout_actions(actions: Vec<niri_ipc::Action>) {
    #[cfg(not(test))]
    tokio::spawn(async move {
        for action in actions {
            if let Err(error) = crate::command::niri_action(action).await {
                tracing::warn!(target: "canopa_shell::modules", %error, "taskbar layout command failed");
                return;
            }
        }
    });
    #[cfg(test)]
    let _ = actions;
}

fn launch_application(desktop_id: &str) {
    let Some(app) = applications()
        .iter()
        .find(|app| app.id == desktop_id)
        .cloned()
    else {
        return;
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

fn launch_desktop_action(desktop_id: &str, action: &str) {
    let Some(app) = applications()
        .iter()
        .find(|app| app.id == desktop_id)
        .cloned()
    else {
        return;
    };
    #[cfg(not(test))]
    {
        let action = action.to_owned();
        tokio::spawn(async move {
            if let Err(error) = crate::command::launch_desktop_action(&app, &action, None).await {
                tracing::warn!(target: "canopa_shell::modules", %error, app = %app.id, %action, "taskbar action failed");
            }
        });
    }
    #[cfg(test)]
    let _ = (app, action);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quick_second_click_cycles_from_the_window_just_requested() {
        let mut runtime = TaskbarRuntime::default();
        assert_eq!(runtime.current_window(Some(4)), Some(4));
        runtime.focus_request = Some(FocusRequest {
            window: 6,
            replaced: Some(4),
            at: Instant::now(),
        });
        assert_eq!(
            runtime.current_window(Some(4)),
            Some(6),
            "Niri has not reported the change yet"
        );
        assert_eq!(runtime.current_window(Some(6)), Some(6));
        assert_eq!(
            runtime.current_window(Some(9)),
            Some(9),
            "focus moved elsewhere since"
        );
        runtime.focus_request.as_mut().unwrap().at = Instant::now() - FOCUS_REQUEST_GRACE;
        assert_eq!(
            runtime.current_window(Some(4)),
            Some(4),
            "the request expired"
        );
    }

    #[test]
    fn menu_anchor_opens_beside_the_icon_away_from_the_bar_edge() {
        let bounds = iced::Rectangle::new(iced::Point::new(10.0, 4.0), iced::Size::new(40.0, 40.0));
        assert_eq!(menu_anchor(&crate::config::Edge::Bottom, bounds), (10, 44));
        assert_eq!(menu_anchor(&crate::config::Edge::Left, bounds), (50, 4));
    }

    #[test]
    fn tooltip_previews_follow_the_strip_and_forget_closed_windows() {
        use crate::app::messages::AppNiriEvent;
        use crate::niri::NiriEvent;
        let config = crate::config::Config {
            modules: vec![crate::config::ModuleConfig {
                name: "taskbar".into(),
                module_type: "taskbar".into(),
                params: ModuleParams::Taskbar(Default::default()),
                actions: Default::default(),
            }],
            ..crate::config::Config::default()
        };
        let (mut app, _) = App::boot(config, vec![]);
        let window = |id: u64, column: usize| niri_ipc::Window {
            id,
            title: None,
            app_id: Some("foot".into()),
            pid: None,
            parent_id: None,
            workspace_id: Some(1),
            is_focused: false,
            is_floating: false,
            is_urgent: false,
            layout: niri_ipc::WindowLayout {
                pos_in_scrolling_layout: Some((column, 1)),
                tile_size: (0.0, 0.0),
                window_size: (0, 0),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
                pinned: None,
            },
            focus_timestamp: None,
        };
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            niri_ipc::Event::WindowsChanged {
                windows: (1..=6).map(|id| window(id, 7 - id as usize)).collect(),
            },
        ))));
        let module = taskbar::TaskbarItemModule::module_id("taskbar", "foot");

        let _ = app.update(Message::Taskbar(TaskbarEvent::Previews(
            [6, 5, 4, 3, 2, 99]
                .into_iter()
                .map(|id| (id, 300, 150, vec![0]))
                .collect(),
        )));

        let previews = app.taskbar_cached_previews(&module);
        // Screen order (window 6 is in column 1), at most four, closed
        // windows dropped, and capped at the preview size.
        assert_eq!(previews.len(), 4);
        assert!(
            previews
                .iter()
                .all(|preview| (preview.width, preview.height) == (192.0, 96.0))
        );
        assert!(!app.taskbar.previews.contains_key(&99));
    }
}
