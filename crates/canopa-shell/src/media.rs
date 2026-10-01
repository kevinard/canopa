//! Shared now-playing state. The shell owns selection; the panel is a client.
use iced::futures::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tokio::sync::{mpsc, oneshot, watch};
use zbus::zvariant::OwnedValue;

pub const SERVICE: &str = "org.canopa.Shell.Media";
pub const PATH: &str = "/org/canopa/Shell/Media";
pub const INTERFACE: &str = "org.canopa.Shell.Media";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PREFIX: &str = "org.mpris.MediaPlayer2.";
const DEADLINE: Duration = Duration::from_millis(800);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerId {
    pub bus: String,
    pub owner: String,
    pub server: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub title: String,
    pub artist: String,
    pub track: String,
    pub status: String,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_next: bool,
    pub can_previous: bool,
}

impl Player {
    pub fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::Play => self.can_play,
            Operation::Pause => self.can_pause,
            Operation::Next => self.can_next,
            Operation::Previous => self.can_previous,
        }
    }
    pub fn label(&self) -> String {
        if self.title.is_empty() {
            self.name.clone()
        } else if self.artist.is_empty() {
            self.title.clone()
        } else {
            format!("{} — {}", self.artist, self.title)
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub available: bool,
    pub players: Vec<Player>,
    pub selected: Option<PlayerId>,
    pub pinned: bool,
}

impl Snapshot {
    pub fn current(&self) -> Option<&Player> {
        self.players
            .iter()
            .find(|p| Some(&p.id) == self.selected.as_ref())
    }

    fn reconcile(&mut self, mut players: Vec<Player>) {
        players.sort_by(|a, b| a.id.bus.cmp(&b.id.bus));
        self.players = players;
        if self.current().is_none() {
            self.pinned = false;
        }
        if !self.pinned {
            let rank = |p: &Player| match p.status.as_str() {
                "Playing" => 0,
                "Paused" => 1,
                _ => 2,
            };
            // Retain the current player among equally active candidates.
            self.selected = self
                .players
                .iter()
                .min_by_key(|p| (rank(p), Some(&p.id) != self.selected.as_ref()))
                .map(|p| p.id.clone());
        }
        self.available = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    Previous,
    Play,
    Pause,
    Next,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Select(Option<PlayerId>),
    Control {
        player: PlayerId,
        track: String,
        operation: Operation,
    },
}

struct Command {
    expires: tokio::time::Instant,
    request: Request,
    result: oneshot::Sender<Result<(), String>>,
}

struct MediaInterface {
    snapshots: watch::Receiver<Snapshot>,
    commands: mpsc::Sender<Command>,
}

#[zbus::interface(name = "org.canopa.Shell.Media")]
impl MediaInterface {
    fn snapshot(&self) -> zbus::fdo::Result<String> {
        serde_json::to_string(&*self.snapshots.borrow())
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))
    }

    async fn apply(&self, request: &str) -> zbus::fdo::Result<()> {
        if request.len() > 8192 {
            return Err(zbus::fdo::Error::InvalidArgs(
                "Media request is too large".into(),
            ));
        }
        let request = serde_json::from_str(request)
            .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid media request".into()))?;
        let (result, rx) = oneshot::channel();
        self.commands
            .try_send(Command {
                request,
                result,
                expires: tokio::time::Instant::now() + Duration::from_secs(1),
            })
            .map_err(|_| zbus::fdo::Error::Failed("Media controls are busy".into()))?;
        tokio::time::timeout(Duration::from_secs(3), rx)
            .await
            .map_err(|_| zbus::fdo::Error::Failed("Media control timed out".into()))?
            .map_err(|_| zbus::fdo::Error::Failed("Media controls disconnected".into()))?
            .map_err(zbus::fdo::Error::Failed)
    }
}

fn bounded(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(512)
        .collect()
}

async fn read_player(conn: &zbus::Connection, bus: String) -> anyhow::Result<Player> {
    let dbus = zbus::fdo::DBusProxy::new(conn).await?;
    let owner = dbus
        .get_name_owner(bus.as_str().try_into()?)
        .await?
        .to_string();
    let props = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(owner.clone())?
        .path(MPRIS_PATH)?
        .build()
        .await?;
    let values = props.get_all(PLAYER.try_into()?).await?;
    let root = props
        .get_all("org.mpris.MediaPlayer2".try_into()?)
        .await
        .unwrap_or_default();
    let metadata = values
        .get("Metadata")
        .and_then(|v| v.try_clone().ok())
        .and_then(|v| HashMap::<String, OwnedValue>::try_from(v).ok())
        .unwrap_or_default();
    let string = |map: &HashMap<String, OwnedValue>, key: &str| {
        map.get(key)
            .and_then(|v| <&str>::try_from(v).ok())
            .map(bounded)
            .unwrap_or_default()
    };
    let enabled = |key: &str| {
        values
            .get(key)
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false)
    };
    let can_control = enabled("CanControl");
    let artists = metadata
        .get("xesam:artist")
        .and_then(|v| v.try_clone().ok())
        .and_then(|v| Vec::<String>::try_from(v).ok())
        .unwrap_or_default();
    let name = string(&root, "Identity");
    let status = string(&values, "PlaybackStatus");
    anyhow::ensure!(
        matches!(status.as_str(), "Playing" | "Paused" | "Stopped"),
        "Invalid playback state"
    );
    let track = metadata
        .get("mpris:trackid")
        .and_then(|v| <&zbus::zvariant::ObjectPath<'_>>::try_from(v).ok())
        .map(ToString::to_string)
        .unwrap_or_default();
    anyhow::ensure!(track.len() <= 2048, "Media track identifier too long");
    Ok(Player {
        id: PlayerId {
            bus: bus.clone(),
            owner,
            server: conn.server_guid().to_string(),
        },
        name: if name.is_empty() {
            bounded(bus.trim_start_matches(PREFIX))
        } else {
            name
        },
        title: string(&metadata, "xesam:title"),
        artist: bounded(&artists.into_iter().take(16).collect::<Vec<_>>().join(", ")),
        track,
        status,
        can_play: can_control && enabled("CanPlay"),
        can_pause: can_control && enabled("CanPause"),
        can_next: can_control && enabled("CanGoNext"),
        can_previous: can_control && enabled("CanGoPrevious"),
    })
}

async fn discover(conn: &zbus::Connection) -> anyhow::Result<Vec<Player>> {
    let names = tokio::time::timeout(DEADLINE, async {
        zbus::fdo::DBusProxy::new(conn).await?.list_names().await
    })
    .await??;
    let mut names: Vec<String> = names
        .into_iter()
        .map(|n| n.to_string())
        .filter(|n| n.starts_with(PREFIX))
        .collect();
    names.sort();
    Ok(stream::iter(names.into_iter().take(64))
        .map(|name| async move {
            tokio::time::timeout(DEADLINE, read_player(conn, name))
                .await
                .ok()
                .and_then(Result::ok)
        })
        .buffer_unordered(16)
        .filter_map(|p| async { p })
        .collect()
        .await)
}

async fn apply(
    conn: &zbus::Connection,
    state: &mut Snapshot,
    request: Request,
) -> anyhow::Result<()> {
    match request {
        Request::Select(id) => {
            anyhow::ensure!(
                id.is_none() || state.players.iter().any(|p| Some(&p.id) == id.as_ref()),
                "Player is no longer available"
            );
            state.pinned = id.is_some();
            state.selected = id;
            state.reconcile(state.players.clone());
        }
        Request::Control {
            player,
            track,
            operation,
        } => {
            anyhow::ensure!(
                state.players.iter().any(|p| p.id == player),
                "Player is no longer available"
            );
            let current = read_player(conn, player.bus.clone()).await?;
            anyhow::ensure!(
                current.id == player && current.track == track,
                "Player or track changed; try again"
            );
            anyhow::ensure!(
                current.allows(operation),
                "This player does not support that control"
            );
            let method = match operation {
                Operation::Previous => "Previous",
                Operation::Play => "Play",
                Operation::Pause => "Pause",
                Operation::Next => "Next",
            };
            // Address the captured unique owner. Never redirect a late click to a replacement process.
            conn.call_method(
                Some(player.owner.as_str()),
                MPRIS_PATH,
                Some(PLAYER),
                method,
                &(),
            )
            .await?;
        }
    }
    Ok(())
}

async fn connected(
    conn: zbus::Connection,
    output: &watch::Sender<Snapshot>,
    mut commands: mpsc::Receiver<Command>,
) -> anyhow::Result<()> {
    let changes = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .path(MPRIS_PATH)?
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .build();
    let owners = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .build();
    // Install both matches before discovery to close the startup race.
    let mut changes = zbus::MessageStream::for_match_rule(changes, &conn, Some(128)).await?;
    let mut owners = zbus::MessageStream::for_match_rule(owners, &conn, Some(128)).await?;
    let mut state = Snapshot::default();
    loop {
        state.reconcile(discover(&conn).await?);
        if *output.borrow() != state {
            output.send_replace(state.clone());
            conn.emit_signal(
                None::<&str>,
                PATH,
                INTERFACE,
                "Changed",
                &serde_json::to_string(&state)?,
            )
            .await?;
        }
        loop {
            tokio::select! {
                _ = output.closed() => return Ok(()),
                signal = changes.next() => { signal.ok_or_else(|| anyhow::anyhow!("Media bus disconnected"))??; break; },
                signal = owners.next() => {
                    let signal = signal.ok_or_else(|| anyhow::anyhow!("Media bus disconnected"))??;
                    if let Ok((name, _, _)) = signal.body().deserialize::<(String, String, String)>()
                        && name.starts_with(PREFIX) { break; }
                },
                command = commands.recv() => {
                    let Some(command) = command else { return Ok(()); };
                    if command.result.is_closed() || tokio::time::Instant::now() >= command.expires {
                        let _ = command.result.send(Err("Media request expired; try again".into()));
                        continue;
                    }
                    let result = tokio::time::timeout(DEADLINE, apply(&conn, &mut state, command.request)).await
                        .map_err(|_| "Media control timed out".to_string()).and_then(|r| r.map_err(|e| e.to_string()));
                    let _ = command.result.send(result);
                    break;
                }
            }
        }
    }
}

/// Runs only while a media Module Data Source is needed. Cancellation drops the
/// connection, exported interface, matches and pending work together.
pub async fn run(output: watch::Sender<Snapshot>) {
    loop {
        let (commands, rx) = mpsc::channel(8);
        let connect = async {
            zbus::connection::Builder::session()?
                .name(SERVICE)?
                .serve_at(
                    PATH,
                    MediaInterface {
                        snapshots: output.subscribe(),
                        commands,
                    },
                )?
                .build()
                .await
        };
        if let Ok(Ok(conn)) = tokio::time::timeout(DEADLINE, connect).await
            && let Err(error) = connected(conn, &output, rx).await
        {
            tracing::debug!(%error, "Media connection ended");
        }
        output.send_replace(Snapshot::default());
        tokio::select! { _ = output.closed() => return, _ = tokio::time::sleep(Duration::from_secs(1)) => {} }
    }
}

#[cfg(test)]
mod tests;
