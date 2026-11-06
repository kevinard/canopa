//! Shared now-playing state. The shell owns selection; the panel is a client.
use futures_util::{
    StreamExt, TryStreamExt,
    future::BoxFuture,
    stream::{self, FuturesUnordered},
};
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

pub use canopa_widgets::media::{Operation, Player, PlayerId, Request, Snapshot};

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
    read_owned_player(
        conn,
        PlayerId {
            bus,
            owner,
            server: conn.server_guid().to_string(),
        },
    )
    .await
}

async fn read_owned_player(conn: &zbus::Connection, id: PlayerId) -> anyhow::Result<Player> {
    let props = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(id.owner.clone())?
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
    let name = if name.is_empty() {
        bounded(id.bus.trim_start_matches(PREFIX))
    } else {
        name
    };
    Ok(Player {
        id,
        name,
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

/// Inventory reads only the bus daemon. Slow player properties are separate work.
async fn discover_owners(conn: &zbus::Connection) -> anyhow::Result<Vec<PlayerId>> {
    tokio::time::timeout(DEADLINE, async {
        let dbus = zbus::fdo::DBusProxy::new(conn).await?;
        let mut names: Vec<String> = dbus
            .list_names()
            .await?
            .into_iter()
            .map(|name| name.to_string())
            .filter(|name| name.starts_with(PREFIX))
            .collect();
        names.sort();
        let dbus = &dbus;
        let owners: Vec<Option<PlayerId>> = stream::iter(names.into_iter().take(64))
            .map(|bus| async move {
                match dbus.get_name_owner(bus.as_str().try_into()?).await {
                    Ok(owner) => Ok(Some(PlayerId {
                        bus,
                        owner: owner.to_string(),
                        server: conn.server_guid().to_string(),
                    })),
                    // The name can disappear between ListNames and GetNameOwner.
                    Err(zbus::fdo::Error::NameHasNoOwner(_)) => Ok(None),
                    Err(error) => Err(anyhow::Error::from(error)),
                }
            })
            .buffer_unordered(16)
            .try_collect()
            .await?;
        Ok::<_, anyhow::Error>(owners.into_iter().flatten().collect())
    })
    .await?
}

struct PlayerRecord {
    id: PlayerId,
    player: Option<Player>,
    dirty: bool,
    reading: Option<u64>,
    initialized: bool,
}

type ReadResult = (PlayerId, u64, Option<Player>);

#[derive(Default)]
struct PlayerRegistry {
    records: HashMap<String, PlayerRecord>,
    next_read: u64,
}

impl PlayerRegistry {
    fn reconcile_owners(&mut self, owners: Vec<PlayerId>) {
        let mut previous = std::mem::take(&mut self.records);
        self.records = owners
            .into_iter()
            .map(|id| {
                let record = previous
                    .remove(&id.bus)
                    .filter(|record| record.id == id)
                    .unwrap_or_else(|| PlayerRecord {
                        id: id.clone(),
                        player: None,
                        dirty: true,
                        reading: None,
                        initialized: false,
                    });
                (id.bus, record)
            })
            .collect();
    }

    fn dirty(&mut self, bus: &str) {
        if let Some(record) = self.records.get_mut(bus) {
            record.dirty = true;
        }
    }

    fn properties_changed(&mut self, owner: &str) {
        for record in self
            .records
            .values_mut()
            .filter(|record| record.id.owner == owner)
        {
            record.dirty = true;
        }
    }

    fn start_reads(
        &mut self,
        conn: &zbus::Connection,
        reads: &mut FuturesUnordered<BoxFuture<'static, ReadResult>>,
    ) {
        for record in self.records.values_mut() {
            if reads.len() == 16 {
                break;
            }
            if !record.dirty || record.reading.is_some() {
                continue;
            }
            record.dirty = false;
            self.next_read = self.next_read.wrapping_add(1);
            let read_id = self.next_read;
            record.reading = Some(read_id);
            let id = record.id.clone();
            let conn = conn.clone();
            reads.push(Box::pin(async move {
                let player = tokio::time::timeout(DEADLINE, read_owned_player(&conn, id.clone()))
                    .await
                    .ok()
                    .and_then(Result::ok);
                (id, read_id, player)
            }));
        }
    }

    fn finish_read(&mut self, (id, read_id, player): ReadResult) -> bool {
        let Some(record) = self
            .records
            .get_mut(&id.bus)
            .filter(|record| record.id == id && record.reading == Some(read_id))
        else {
            return false;
        };
        record.reading = None;
        let initial_read = !record.initialized;
        record.initialized = true;
        // One read per live record may advance the display even during a burst.
        // Signals received during that read retain the next refresh obligation.
        if let Some(player) = player
            && record.player.as_ref() != Some(&player)
        {
            record.player = Some(player);
            return true;
        }
        // A failed metadata read is not evidence of departure. Retain the last
        // valid presentation (including manual selection) until ownership changes.
        initial_read
    }

    fn initial_reads_complete(&self) -> bool {
        !self.records.is_empty() && self.records.values().all(|record| record.initialized)
    }

    fn players(&self) -> Vec<Player> {
        self.records
            .values()
            .filter_map(|record| record.player.clone())
            .collect()
    }
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
    let mut registry = PlayerRegistry::default();
    let mut reads = FuturesUnordered::new();
    let mut inventory: Option<BoxFuture<'static, anyhow::Result<Vec<PlayerId>>>> = None;
    let mut inventory_dirty = true;
    let mut publication_needed = false;
    let mut initial_selection = true;
    loop {
        if inventory_dirty && inventory.is_none() {
            let conn = conn.clone();
            inventory = Some(Box::pin(async move { discover_owners(&conn).await }));
            inventory_dirty = false;
        }
        registry.start_reads(&conn, &mut reads);
        if publication_needed {
            publication_needed = false;
            // Publish available players immediately, but resolve initial automatic
            // ties by bus name until the discovery cohort has completed its reads.
            // An explicit user selection always wins, even during discovery.
            initial_selection |= state.selected.is_none();
            if initial_selection && !state.pinned {
                state.selected = None;
            }
            state.reconcile(registry.players());
            if registry.initial_reads_complete() {
                initial_selection = false;
            }
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
        }
        // Metadata reads remain alive while commands and signals are serviced.
        // The select is fair; neither an update burst nor controls own the loop.
        tokio::select! {
            _ = output.closed() => return Ok(()),
            result = async { match inventory.as_mut() {
                Some(work) => work.await, None => std::future::pending().await,
            }} => {
                registry.reconcile_owners(result?);
                inventory = None;
                publication_needed = true;
            },
            Some(result) = reads.next(), if !reads.is_empty() => publication_needed |= registry.finish_read(result),
            signal = changes.next() => {
                let signal = signal.ok_or_else(|| anyhow::anyhow!("Media bus disconnected"))??;
                if let Ok((interface, _, _)) = signal.body().deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
                    && matches!(interface.as_str(), PLAYER | "org.mpris.MediaPlayer2")
                    && let Some(owner) = signal.header().sender() {
                    registry.properties_changed(owner.as_str());
                }
            },
            signal = owners.next() => {
                let signal = signal.ok_or_else(|| anyhow::anyhow!("Media bus disconnected"))??;
                if let Ok((name, _, _)) = signal.body().deserialize::<(String, String, String)>()
                    && name.starts_with(PREFIX) { inventory_dirty = true; }
            },
            command = commands.recv() => {
                let Some(command) = command else { return Ok(()); };
                let now = tokio::time::Instant::now();
                if command.result.is_closed() || now >= command.expires {
                    let _ = command.result.send(Err("Media request expired; try again".into()));
                    continue;
                }
                let target = match &command.request { Request::Control { player, .. } => Some(player.bus.clone()), _ => None };
                let result = tokio::time::timeout(DEADLINE.min(command.expires - now), apply(&conn, &mut state, command.request)).await
                    .map_err(|_| "Media control timed out".to_string()).and_then(|r| r.map_err(|e| e.to_string()));
                if let Some(bus) = target { registry.dirty(&bus); }
                publication_needed |= result.is_ok();
                let _ = command.result.send(result);
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
