//! Window session restore: while `windows { restore #true; }` is set, the
//! Shell keeps a record of the open applications and where they sit in
//! Niri's layout, and at the start of the next session starts them again in
//! the same workspaces and columns at the same window widths, through Niri's
//! `SpawnPlaced` (the Canopa fork). Only applications with a desktop entry are
//! recorded; the windows of a stacked column after its first and dialogs are
//! left out, because a placement cannot stack windows or recreate dialogs.
//!
//! Window closes are saved late: the end of a session closes every window,
//! and the record must still hold the session as it was before.
//!
//! Applications are started with `gio launch` in their own scope, like any
//! other launch. Niri's activation token reaches Exec applications; a
//! D-Bus-activated application opens its window from another process and may
//! not present the token, so it can open in Niri's usual place.
use std::time::Duration;

use iced::Task;

use super::{App, Message};
use crate::desktop_apps::DesktopApplication;
use crate::niri::NiriState;

/// The state file kept under `$XDG_STATE_HOME/canopa/shell/`.
#[cfg(not(test))]
const FILE: &str = "windows.json";
/// Session records are short lists of desktop IDs; anything larger is not ours.
const MAX_BYTES: usize = 256 * 1024;
const MAX_WINDOWS: usize = 64;
/// How long a change waits before it is saved; closes wait longer.
const SAVE_DELAY: Duration = Duration::from_secs(1);
const CLOSE_SAVE_DELAY: Duration = Duration::from_secs(8);
/// Changes are not saved while restored applications are still opening.
const RESTORE_SETTLE: Duration = Duration::from_secs(20);
/// Restored applications start one at a time, so columns open in order.
const LAUNCH_INTERVAL: Duration = Duration::from_millis(400);

/// One application window and its place.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SavedWindow {
    /// The desktop ID it is started from.
    pub app: String,
    pub output: Option<String>,
    /// The workspace's 1-based index on its output, and its name if it has one.
    pub workspace: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_name: Option<String>,
    /// The 1-based column; `None` for a floating window.
    pub column: Option<usize>,
    /// The window's width in logical pixels. Niri sets a proportion of the
    /// working area less its gaps, which the Shell cannot see, so a saved
    /// proportion would shrink a little every session; a fixed width does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SessionRecord {
    pub version: u32,
    pub windows: Vec<SavedWindow>,
}

impl SessionRecord {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("the window session is too large".into());
        }
        let record: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if record.version != 1 {
            return Err(format!(
                "unsupported window session version {}",
                record.version
            ));
        }
        if record.windows.len() > MAX_WINDOWS {
            return Err("too many windows in the window session".into());
        }
        Ok(record)
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec_pretty(self).expect("window session serializes");
        bytes.push(b'\n');
        bytes
    }
}

/// The applications open in `niri`, in screen order.
pub(crate) fn record(niri: &NiriState, applications: &[DesktopApplication]) -> SessionRecord {
    let mut windows: Vec<(&niri_ipc::Window, &niri_ipc::Workspace)> = niri
        .windows
        .values()
        .filter(|window| window.parent_id.is_none())
        .filter(|window| {
            window
                .layout
                .pos_in_scrolling_layout
                .is_none_or(|(_, tile)| tile == 1)
        })
        .filter_map(|window| {
            let workspace = niri
                .workspaces
                .iter()
                .find(|workspace| Some(workspace.id) == window.workspace_id)?;
            Some((window, workspace))
        })
        .collect();
    windows.sort_by_key(|(window, workspace)| {
        (
            workspace.output.clone(),
            workspace.idx,
            window
                .layout
                .pos_in_scrolling_layout
                .map_or(usize::MAX, |(column, _)| column),
            window.id,
        )
    });
    let mut saved: Vec<SavedWindow> = Vec::new();
    for (window, workspace) in windows {
        let Some(app) = window.app_id.as_deref().and_then(|app_id| {
            crate::focused_window_icons::match_desktop_application_for_app_id(app_id, applications)
        }) else {
            continue;
        };
        // One window brings the others of a single-window application.
        if app.single_main_window && saved.iter().any(|other| other.app == app.id) {
            continue;
        }
        let column = window
            .layout
            .pos_in_scrolling_layout
            .map(|(column, _)| column);
        let width = column
            .map(|_| window.layout.window_size.0)
            .filter(|width| *width > 0);
        saved.push(SavedWindow {
            app: app.id.clone(),
            output: workspace.output.clone(),
            workspace: workspace.idx,
            workspace_name: workspace.name.clone(),
            column,
            width,
        });
        if saved.len() == MAX_WINDOWS {
            break;
        }
    }
    SessionRecord {
        version: 1,
        windows: saved,
    }
}

/// Where each saved window should open. Columns are renumbered per
/// workspace from 1, because the windows are started in order on a session
/// whose workspaces are empty. Workspaces are referred to by name when they
/// have one, otherwise by index, which Niri resolves on the focused output.
pub(crate) fn placements(record: &SessionRecord) -> Vec<(String, niri_ipc::Placement)> {
    let mut next_column: Vec<(Option<&str>, u8, usize)> = Vec::new();
    record
        .windows
        .iter()
        .map(|window| {
            let workspace = match &window.workspace_name {
                Some(name) => niri_ipc::WorkspaceReferenceArg::Name(name.clone()),
                None => niri_ipc::WorkspaceReferenceArg::Index(window.workspace),
            };
            let column_index = window.column.map(|_| {
                let key = (window.output.as_deref(), window.workspace);
                match next_column
                    .iter_mut()
                    .find(|(output, index, _)| (*output, *index) == key)
                {
                    Some((_, _, next)) => {
                        *next += 1;
                        *next
                    }
                    None => {
                        next_column.push((key.0, key.1, 1));
                        1
                    }
                }
            });
            (
                window.app.clone(),
                niri_ipc::Placement {
                    workspace: Some(workspace),
                    column_index,
                    width: window
                        .width
                        .filter(|width| (64..=16384).contains(width))
                        .map(niri_ipc::SizeChange::SetFixed),
                    display: None,
                    floating: Some(window.column.is_none()),
                },
            )
        })
        .collect()
}

/// The runtime half: when to restore and when to save.
#[derive(Default)]
pub(super) struct SessionRestore {
    /// The record last saved or read; saving waits for it.
    saved: Option<SessionRecord>,
    /// The record waiting to be saved, and its generation.
    pending: Option<(u64, SessionRecord)>,
    generation: u64,
    restore_checked: bool,
    /// Saving resumes at this time after a restore.
    settle_until: Option<std::time::Instant>,
}

impl App {
    /// After Niri state changes: restore once at session start, then keep
    /// the record current.
    pub(super) fn sync_window_session(&mut self) -> Task<Message> {
        // Nothing is decided before Niri has said which windows are open:
        // a Shell restart inside a session must not start them again.
        if !self.config.windows.restore
            || !self.niri.state().connected
            || !self.niri.state().windows_known
        {
            return Task::none();
        }
        if !self.session_restore.restore_checked {
            self.session_restore.restore_checked = true;
            return self.restore_window_session();
        }
        if self
            .session_restore
            .settle_until
            .is_some_and(|until| std::time::Instant::now() < until)
        {
            return Task::none();
        }
        let current = record(self.niri.state(), super::taskbar::applications());
        let known = self
            .session_restore
            .pending
            .as_ref()
            .map(|(_, record)| record)
            .or(self.session_restore.saved.as_ref());
        if known == Some(&current) {
            return Task::none();
        }
        let closed = known.is_some_and(|known| current.windows.len() < known.windows.len());
        self.session_restore.generation += 1;
        let generation = self.session_restore.generation;
        self.session_restore.pending = Some((generation, current));
        let delay = if closed { CLOSE_SAVE_DELAY } else { SAVE_DELAY };
        Task::perform(tokio::time::sleep(delay), move |()| {
            Message::WindowSessionSave(generation)
        })
    }

    pub(super) fn save_window_session(&mut self, generation: u64) {
        let Some((pending, _)) = &self.session_restore.pending else {
            return;
        };
        if *pending != generation {
            return;
        }
        let (_, record) = self.session_restore.pending.take().expect("checked above");
        tracing::debug!(
            target: "canopa_shell::session",
            windows = record.windows.len(),
            "saving the window session"
        );
        write_record(&record);
        self.session_restore.saved = Some(record);
    }

    /// Start the recorded applications when the session has no application
    /// windows yet, so a Shell restart does not open them twice.
    fn restore_window_session(&mut self) -> Task<Message> {
        let saved = read_record();
        self.session_restore.saved = saved.clone();
        let already_open = !record(self.niri.state(), super::taskbar::applications())
            .windows
            .is_empty();
        let Some(saved) = saved.filter(|saved| !saved.windows.is_empty() && !already_open) else {
            return Task::none();
        };
        let launches: Vec<_> = placements(&saved)
            .into_iter()
            .filter_map(|(id, placement)| {
                super::taskbar::applications()
                    .iter()
                    .find(|app| app.id == id)
                    .cloned()
                    .map(|app| (app, placement))
            })
            .collect();
        tracing::info!(
            target: "canopa_shell::session",
            windows = launches.len(),
            "restoring the window session"
        );
        self.session_restore.settle_until = Some(
            std::time::Instant::now() + RESTORE_SETTLE + LAUNCH_INTERVAL * launches.len() as u32,
        );
        launch_in_order(launches);
        Task::none()
    }
}

/// The workspace a template opens on: the empty one Niri keeps after the
/// focused output's last workspace, as its 1-based index on that output.
pub(crate) fn template_workspace(niri: &NiriState) -> Option<u8> {
    let output = niri.focused_output()?;
    let last = niri
        .workspaces
        .iter()
        .filter(|workspace| workspace.output.as_deref() == Some(output))
        .max_by_key(|workspace| workspace.idx)?;
    let empty = !niri
        .windows
        .values()
        .any(|window| window.workspace_id == Some(last.id));
    // Niri keeps an empty workspace last; without one, the template opens
    // after the last workspace, which Niri then creates.
    Some(if empty {
        last.idx
    } else {
        last.idx.saturating_add(1)
    })
}

/// Where each application of `template` opens on workspace `index`.
pub(crate) fn template_placements(
    template: &crate::config::TemplateConfig,
    index: u8,
) -> Vec<(String, niri_ipc::Placement)> {
    template
        .apps
        .iter()
        .enumerate()
        .map(|(column, app)| {
            (
                app.desktop_id.clone(),
                niri_ipc::Placement {
                    workspace: Some(niri_ipc::WorkspaceReferenceArg::Index(index)),
                    column_index: Some(column + 1),
                    width: app.width_percent.map(niri_ipc::SizeChange::SetProportion),
                    display: None,
                    floating: Some(false),
                },
            )
        })
        .collect()
}

impl App {
    /// Open a configured workspace template on a new workspace and show it.
    pub(super) fn open_template(&mut self, name: &str) -> Result<(), String> {
        let template = self
            .config
            .templates
            .iter()
            .find(|template| template.name == name)
            .ok_or_else(|| format!("unknown template {name:?}"))?;
        if !self.niri.state().connected {
            return Err("Niri is unavailable".into());
        }
        let index =
            template_workspace(self.niri.state()).ok_or_else(|| "no focused output".to_string())?;
        let mut launches = Vec::new();
        for (id, placement) in template_placements(template, index) {
            let app = super::taskbar::applications()
                .iter()
                .find(|app| app.id == id)
                .cloned()
                .ok_or_else(|| format!("{id} is not installed"))?;
            launches.push((app, placement));
        }
        launch_in_order(launches);
        focus_workspace(index);
        Ok(())
    }
}

#[cfg(not(test))]
fn focus_workspace(index: u8) {
    tokio::spawn(async move {
        let action = niri_ipc::Action::FocusWorkspace {
            reference: niri_ipc::WorkspaceReferenceArg::Index(index),
        };
        if let Err(error) = crate::command::niri_action(action).await {
            tracing::warn!(target: "canopa_shell::session", %error, "template workspace not focused");
        }
    });
}

#[cfg(test)]
fn focus_workspace(_: u8) {}

#[cfg(not(test))]
fn launch_in_order(launches: Vec<(DesktopApplication, niri_ipc::Placement)>) {
    tokio::spawn(async move {
        for (app, placement) in launches {
            if let Err(error) =
                crate::command::spawn_desktop_application_placed(&app, placement).await
            {
                tracing::warn!(target: "canopa_shell::session", %error, app = %app.id, "window session restore failed");
            }
            tokio::time::sleep(LAUNCH_INTERVAL).await;
        }
    });
}

#[cfg(test)]
fn launch_in_order(_: Vec<(DesktopApplication, niri_ipc::Placement)>) {}

#[cfg(not(test))]
fn record_path() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".local/state"))
        })
        .map(|root| root.join("canopa/shell").join(FILE))
}

#[cfg(not(test))]
fn read_record() -> Option<SessionRecord> {
    let path = record_path()?;
    let bytes = canopa_file::private_state::read(&path, MAX_BYTES).ok()?;
    SessionRecord::parse(&bytes)
        .inspect_err(|error| {
            tracing::warn!(target: "canopa_shell::session", %error, "window session ignored");
        })
        .ok()
}

#[cfg(test)]
fn read_record() -> Option<SessionRecord> {
    None
}

#[cfg(not(test))]
fn write_record(record: &SessionRecord) {
    let Some(path) = record_path() else {
        return;
    };
    let bytes = record.to_bytes();
    std::thread::spawn(move || {
        if let Err(error) = canopa_file::private_state::write(&path, &bytes) {
            tracing::warn!(target: "canopa_shell::session", %error, "window session not saved");
        }
    });
}

#[cfg(test)]
fn write_record(_: &SessionRecord) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, single: bool) -> DesktopApplication {
        DesktopApplication {
            id: format!("{id}.desktop"),
            name: id.into(),
            single_main_window: single,
            ..DesktopApplication::default()
        }
    }

    fn workspace(id: u64, idx: u8, name: Option<&str>) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id,
            idx,
            name: name.map(str::to_owned),
            output: Some("eDP-1".into()),
            is_urgent: false,
            is_active: idx == 1,
            is_focused: idx == 1,
            active_window_id: None,
        }
    }

    fn window(id: u64, app: &str, workspace: u64, pos: Option<(usize, usize)>) -> niri_ipc::Window {
        niri_ipc::Window {
            id,
            title: None,
            app_id: Some(app.into()),
            pid: None,
            parent_id: None,
            workspace_id: Some(workspace),
            is_focused: false,
            is_floating: pos.is_none(),
            is_urgent: false,
            layout: niri_ipc::WindowLayout {
                pos_in_scrolling_layout: pos,
                tile_size: (960.0, 1000.0),
                window_size: (960, 1000),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
                pinned: None,
            },
            focus_timestamp: None,
        }
    }

    fn niri(windows: Vec<niri_ipc::Window>) -> NiriState {
        NiriState {
            workspaces: vec![workspace(1, 1, None), workspace(2, 2, Some("chat"))],
            windows: windows
                .into_iter()
                .map(|window| (window.id, window))
                .collect(),
            connected: true,
            output_sizes: [("eDP-1".to_owned(), (1920.0, 1080.0))].into(),
            ..NiriState::default()
        }
    }

    #[test]
    fn the_record_keeps_screen_order_and_leaves_out_what_cannot_be_placed() {
        let apps = [
            app("foot", false),
            app("firefox", false),
            app("signal", true),
        ];
        let mut dialog = window(7, "firefox", 1, None);
        dialog.parent_id = Some(2);
        let state = niri(vec![
            window(2, "firefox", 1, Some((2, 1))),
            window(1, "foot", 1, Some((1, 1))),
            // Stacked under Foot: not recorded.
            window(3, "foot", 1, Some((1, 2))),
            window(4, "signal", 2, Some((1, 1))),
            window(5, "signal", 2, Some((2, 1))),
            // No desktop entry.
            window(6, "org.example.Unknown", 1, Some((3, 1))),
            dialog,
            window(8, "foot", 2, None),
        ]);

        let record = record(&state, &apps);

        let summary: Vec<_> = record
            .windows
            .iter()
            .map(|window| (window.app.as_str(), window.workspace, window.column))
            .collect();
        assert_eq!(
            summary,
            [
                ("foot.desktop", 1, Some(1)),
                ("firefox.desktop", 1, Some(2)),
                ("signal.desktop", 2, Some(1)),
                ("foot.desktop", 2, None),
            ]
        );
        assert_eq!(record.windows[0].width, Some(960));
        assert_eq!(record.windows[2].workspace_name.as_deref(), Some("chat"));
        assert_eq!(record.windows[3].width, None);
    }

    #[test]
    fn placements_renumber_columns_and_name_workspaces() {
        let saved = |app: &str, workspace, name: Option<&str>, column| SavedWindow {
            app: app.into(),
            output: Some("eDP-1".into()),
            workspace,
            workspace_name: name.map(str::to_owned),
            column,
            width: Some(620),
        };
        let record = SessionRecord {
            version: 1,
            windows: vec![
                saved("a", 1, None, Some(2)),
                saved("b", 1, None, Some(5)),
                saved("c", 2, Some("chat"), Some(1)),
                saved("d", 2, Some("chat"), None),
            ],
        };

        let placements = placements(&record);

        assert_eq!(placements[0].1.column_index, Some(1));
        assert_eq!(placements[1].1.column_index, Some(2));
        assert_eq!(
            placements[0].1.workspace,
            Some(niri_ipc::WorkspaceReferenceArg::Index(1))
        );
        assert_eq!(
            placements[2].1.workspace,
            Some(niri_ipc::WorkspaceReferenceArg::Name("chat".into()))
        );
        assert_eq!(placements[2].1.column_index, Some(1));
        assert_eq!(placements[3].1.floating, Some(true));
        assert_eq!(placements[3].1.column_index, None);
        assert!(matches!(
            placements[0].1.width,
            Some(niri_ipc::SizeChange::SetFixed(620))
        ));
    }

    #[test]
    fn records_round_trip_and_foreign_files_are_refused() {
        let record = record(
            &niri(vec![window(1, "foot", 1, Some((1, 1)))]),
            &[app("foot", false)],
        );
        assert_eq!(SessionRecord::parse(&record.to_bytes()), Ok(record));
        assert!(SessionRecord::parse(br#"{"version":2,"windows":[]}"#).is_err());
        assert!(SessionRecord::parse(br#"{"version":1,"windows":[],"extra":1}"#).is_err());
    }

    #[test]
    fn restore_waits_for_the_window_list() {
        use crate::app::messages::AppNiriEvent;
        use crate::niri::NiriEvent;
        let config = crate::config::Config {
            windows: crate::config::WindowsConfig { restore: true },
            ..crate::config::Config::default()
        };
        let (mut app, _) = App::boot(config, vec![]);
        let send = |app: &mut App, event| {
            let _ = app.update(Message::Niri(AppNiriEvent::Ipc(event)));
        };
        send(&mut app, NiriEvent::Connected);
        send(
            &mut app,
            NiriEvent::Event(niri_ipc::Event::WorkspacesChanged {
                workspaces: vec![workspace(1, 1, None)],
            }),
        );
        assert!(!app.session_restore.restore_checked);

        send(
            &mut app,
            NiriEvent::Event(niri_ipc::Event::WindowsChanged {
                windows: vec![window(1, "foot", 1, Some((1, 1)))],
            }),
        );
        assert!(app.session_restore.restore_checked);
    }

    #[test]
    fn templates_open_on_the_empty_workspace_after_the_last() {
        let mut state = niri(vec![window(1, "foot", 1, Some((1, 1)))]);
        // Workspace 2 ("chat") is empty and last on eDP-1.
        assert_eq!(template_workspace(&state), Some(2));
        state.windows.insert(2, window(2, "foot", 2, Some((1, 1))));
        assert_eq!(template_workspace(&state), Some(3));

        let template = crate::config::TemplateConfig {
            name: "dev".into(),
            apps: vec![
                crate::config::TemplateApp {
                    desktop_id: "code.desktop".into(),
                    width_percent: Some(66.667),
                },
                crate::config::TemplateApp {
                    desktop_id: "foot.desktop".into(),
                    width_percent: None,
                },
            ],
        };
        let placements = template_placements(&template, 3);
        assert_eq!(placements[0].0, "code.desktop");
        assert_eq!(placements[1].1.column_index, Some(2));
        assert_eq!(
            placements[1].1.workspace,
            Some(niri_ipc::WorkspaceReferenceArg::Index(3))
        );
        assert!(matches!(
            placements[0].1.width,
            Some(niri_ipc::SizeChange::SetProportion(value)) if value == 66.667
        ));
        assert_eq!(placements[1].1.width, None);
    }

    #[test]
    fn unknown_templates_are_refused() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        assert!(app.open_template("dev").is_err());
    }
}
