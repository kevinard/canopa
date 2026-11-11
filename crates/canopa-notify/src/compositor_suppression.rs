//! Fullscreen and screencast state from Niri's IPC event stream (ADR 0055).
//!
//! Niri reports every PipeWire screencast it serves, including Canopa's own
//! ScreenCast portal (which records through Niri's Mutter ScreenCast interface)
//! and xdg-desktop-portal-gnome, plus wlr-screencopy recorders, in one cast list.
//! Niri has no fullscreen flag, so a focused output's active tiled window whose
//! tile covers the whole logical output counts as fullscreen.
use canopa_notify::{Daemon, SuppressionState};
use niri_ipc::{Cast, Event, Output, Reply, Request, Response, Window, Workspace};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

const MAX_LINE_BYTES: u64 = 4 * 1024 * 1024;
const RETRY_MIN: Duration = Duration::from_secs(2);
const RETRY_MAX: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(crate) struct Tracker {
    windows: HashMap<u64, Window>,
    workspaces: HashMap<u64, Workspace>,
    outputs: HashMap<String, (u32, u32)>,
    casts: HashMap<u64, Cast>,
}

impl Tracker {
    pub(crate) fn set_outputs(&mut self, outputs: HashMap<String, Output>) {
        self.outputs = outputs
            .into_iter()
            .filter_map(|(name, output)| Some((name, output.logical.map(|l| (l.width, l.height))?)))
            .collect();
    }

    /// Apply one event; returns whether outputs should be queried again.
    pub(crate) fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::WorkspacesChanged { workspaces } => {
                self.workspaces = workspaces.into_iter().map(|w| (w.id, w)).collect();
                return true;
            }
            Event::WorkspaceActivated { id, focused } => {
                let output = self.workspaces.get(&id).and_then(|w| w.output.clone());
                for workspace in self.workspaces.values_mut() {
                    if workspace.output == output && output.is_some() {
                        workspace.is_active = workspace.id == id;
                    }
                    if focused {
                        workspace.is_focused = workspace.id == id;
                    }
                }
            }
            Event::WorkspaceActiveWindowChanged {
                workspace_id,
                active_window_id,
            } => {
                if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
                    workspace.active_window_id = active_window_id;
                }
            }
            Event::WindowsChanged { windows } => {
                self.windows = windows.into_iter().map(|w| (w.id, w)).collect();
            }
            Event::WindowOpenedOrChanged { window } => {
                if window.is_focused {
                    for other in self.windows.values_mut() {
                        other.is_focused = false;
                    }
                }
                self.windows.insert(window.id, window);
            }
            Event::WindowClosed { id } => {
                self.windows.remove(&id);
            }
            Event::WindowFocusChanged { id } => {
                for window in self.windows.values_mut() {
                    window.is_focused = Some(window.id) == id;
                }
            }
            Event::WindowLayoutsChanged { changes } => {
                for (id, layout) in changes {
                    if let Some(window) = self.windows.get_mut(&id) {
                        window.layout = layout;
                    }
                }
            }
            Event::CastsChanged { casts } => {
                self.casts = casts.into_iter().map(|c| (c.stream_id, c)).collect();
            }
            Event::CastStartedOrChanged { cast } => {
                self.casts.insert(cast.stream_id, cast);
            }
            Event::CastStopped { stream_id } => {
                self.casts.remove(&stream_id);
            }
            _ => {}
        }
        false
    }

    pub(crate) fn state(&self) -> SuppressionState {
        SuppressionState {
            fullscreen: self.focused_output_is_fullscreen(),
            screen_sharing: self.casts.values().any(|cast| cast.is_active),
        }
    }

    fn focused_output_is_fullscreen(&self) -> bool {
        let Some(workspace) = self.workspaces.values().find(|w| w.is_focused) else {
            return false;
        };
        let Some((width, height)) = workspace.output.as_ref().and_then(|o| self.outputs.get(o))
        else {
            return false;
        };
        let Some(window) = workspace
            .active_window_id
            .and_then(|id| self.windows.get(&id))
        else {
            return false;
        };
        let (tile_width, tile_height) = window.layout.tile_size;
        !window.is_floating
            && (tile_width - f64::from(*width)).abs() < 1.0
            && (tile_height - f64::from(*height)).abs() < 1.0
    }
}

async fn read_line(
    reader: &mut BufReader<tokio::net::UnixStream>,
) -> anyhow::Result<Option<String>> {
    let mut line = Vec::new();
    let read = (&mut *reader)
        .take(MAX_LINE_BYTES)
        .read_until(b'\n', &mut line)
        .await?;
    if read == 0 {
        return Ok(None);
    }
    anyhow::ensure!(
        line.last() == Some(&b'\n'),
        "oversized or truncated Niri IPC line"
    );
    Ok(Some(String::from_utf8(line)?))
}

async fn request(
    socket: &Path,
    request: &Request,
) -> anyhow::Result<(BufReader<tokio::net::UnixStream>, Response)> {
    let mut stream = tokio::net::UnixStream::connect(socket).await?;
    stream
        .write_all(format!("{}\n", serde_json::to_string(request)?).as_bytes())
        .await?;
    let mut reader = BufReader::new(stream);
    let line = read_line(&mut reader)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Niri closed the IPC connection"))?;
    let reply: Reply = serde_json::from_str(&line)?;
    Ok((
        reader,
        reply.map_err(|error| anyhow::anyhow!("Niri IPC error: {error}"))?,
    ))
}

async fn outputs(socket: &Path) -> anyhow::Result<HashMap<String, Output>> {
    match request(socket, &Request::Outputs).await?.1 {
        Response::Outputs(outputs) => Ok(outputs),
        _ => anyhow::bail!("unexpected Niri outputs reply"),
    }
}

async fn follow(daemon: &Daemon, socket: &Path) -> anyhow::Result<()> {
    let mut tracker = Tracker::default();
    tracker.set_outputs(outputs(socket).await?);
    let (mut events, response) = request(socket, &Request::EventStream).await?;
    anyhow::ensure!(
        matches!(response, Response::Handled),
        "unexpected Niri event-stream reply"
    );
    while let Some(line) = read_line(&mut events).await? {
        // Unknown future events are ignored rather than ending the stream.
        let Ok(event) = serde_json::from_str::<Event>(&line) else {
            continue;
        };
        if tracker.apply(event) {
            match outputs(socket).await {
                Ok(outputs) => tracker.set_outputs(outputs),
                Err(error) => eprintln!(
                    "level=warn component=suppression event=outputs_failed error={error:#}"
                ),
            }
        }
        daemon.set_suppression(tracker.state());
    }
    Ok(())
}

/// Follow Niri until the task is aborted; without `NIRI_SOCKET` nothing runs.
/// A lost connection clears suppression and retries with bounded backoff.
pub(crate) fn spawn(daemon: Daemon) -> Option<tokio::task::JoinHandle<()>> {
    let socket = PathBuf::from(std::env::var_os(niri_ipc::socket::SOCKET_PATH_ENV)?);
    Some(tokio::spawn(async move {
        let mut retry = RETRY_MIN;
        loop {
            let started = std::time::Instant::now();
            if let Err(error) = follow(&daemon, &socket).await {
                eprintln!("level=warn component=suppression event=niri_ipc_failed error={error:#}");
            }
            daemon.set_suppression(SuppressionState::default());
            if started.elapsed() > RETRY_MAX {
                retry = RETRY_MIN;
            }
            tokio::time::sleep(retry).await;
            retry = (retry * 2).min(RETRY_MAX);
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(json: serde_json::Value) -> Event {
        serde_json::from_value(json).unwrap()
    }

    fn window(id: u64, workspace: u64, size: (f64, f64), focused: bool) -> serde_json::Value {
        serde_json::json!({
            "id": id, "title": "t", "app_id": "mpv", "pid": 1, "workspace_id": workspace,
            "is_focused": focused, "is_floating": false, "is_urgent": false,
            "layout": {
                "pos_in_scrolling_layout": [1, 1], "tile_size": [size.0, size.1],
                "window_size": [size.0 as i32, size.1 as i32], "tile_pos_in_workspace_view": [0.0, 0.0],
                "window_offset_in_tile": [0.0, 0.0]
            },
            "focus_timestamp": null
        })
    }

    fn workspaces(focused: u64, active_window: Option<u64>) -> Event {
        event(serde_json::json!({"WorkspacesChanged": {"workspaces": [
            {"id": 1, "idx": 1, "name": null, "output": "DP-1", "is_urgent": false,
             "is_active": true, "is_focused": focused == 1, "active_window_id": active_window},
            {"id": 2, "idx": 1, "name": null, "output": "HDMI-A-1", "is_urgent": false,
             "is_active": true, "is_focused": focused == 2, "active_window_id": null}
        ]}}))
    }

    fn tracker() -> Tracker {
        Tracker {
            outputs: HashMap::from([
                ("DP-1".into(), (1920, 1080)),
                ("HDMI-A-1".into(), (2560, 1440)),
            ]),
            ..Default::default()
        }
    }

    #[test]
    fn a_tile_covering_the_focused_output_is_fullscreen() {
        let mut tracker = tracker();
        assert!(
            tracker.apply(workspaces(1, Some(7))),
            "workspace changes refresh outputs"
        );
        tracker.apply(event(serde_json::json!({"WindowsChanged": {"windows": [window(7, 1, (1920.0, 1080.0), true)]}})));
        assert_eq!(
            tracker.state(),
            SuppressionState {
                fullscreen: true,
                screen_sharing: false
            }
        );
        // A maximized column beside the bar is smaller than the output.
        tracker.apply(event(serde_json::json!({"WindowLayoutsChanged": {"changes": [[7, {
            "pos_in_scrolling_layout": [1, 1], "tile_size": [1920.0, 1046.0], "window_size": [1920, 1046],
            "tile_pos_in_workspace_view": [0.0, 0.0], "window_offset_in_tile": [0.0, 0.0]}]]}})));
        assert!(!tracker.state().fullscreen);
    }

    #[test]
    fn fullscreen_on_another_output_does_not_suppress_the_focused_one() {
        let mut tracker = tracker();
        tracker.apply(workspaces(2, None));
        tracker.apply(event(serde_json::json!({"WindowsChanged": {"windows": [window(7, 1, (1920.0, 1080.0), false)]}})));
        tracker.apply(event(serde_json::json!({"WorkspaceActiveWindowChanged": {"workspace_id": 1, "active_window_id": 7}})));
        assert!(!tracker.state().fullscreen);
        tracker.apply(event(
            serde_json::json!({"WorkspaceActivated": {"id": 1, "focused": true}}),
        ));
        assert!(tracker.state().fullscreen);
        tracker.apply(event(serde_json::json!({"WindowClosed": {"id": 7}})));
        assert!(!tracker.state().fullscreen);
    }

    #[test]
    fn only_streaming_casts_count_as_sharing() {
        let mut tracker = tracker();
        let cast = |stream: u64, active: bool| {
            serde_json::json!({
                "stream_id": stream, "session_id": 1, "kind": "PipeWire",
                "target": {"Output": {"name": "DP-1"}}, "is_dynamic_target": false,
                "is_active": active, "pid": null, "pw_node_id": 42
            })
        };
        tracker.apply(event(
            serde_json::json!({"CastStartedOrChanged": {"cast": cast(3, false)}}),
        ));
        assert!(
            !tracker.state().screen_sharing,
            "a paused cast shows no content"
        );
        tracker.apply(event(
            serde_json::json!({"CastStartedOrChanged": {"cast": cast(3, true)}}),
        ));
        assert!(tracker.state().screen_sharing);
        tracker.apply(event(
            serde_json::json!({"CastsChanged": {"casts": [cast(3, true), cast(4, false)]}}),
        ));
        tracker.apply(event(serde_json::json!({"CastStopped": {"stream_id": 3}})));
        assert!(!tracker.state().screen_sharing);
    }
}
