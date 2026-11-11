//! MPRIS 2 media players on the session bus.
//!
//! Players are identified by bus name, unique owner and bus-server GUID, so a
//! control never reaches a replacement process. Discovery reads only the bus
//! daemon; [`Registry`] bounds the property reads that follow (64 players, 16
//! reads at once) and keeps the last valid reading through a failed read.
//! The Shell owns which player is current and shares it as
//! `org.canopa.Shell.Media`; `canopa_widgets::media::observe` follows that.
//!
//! ```no_run
//! use canopa_services::mpris::{self, Operation};
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let session = canopa_services::bus::session().await?;
//! for player in mpris::players(&session).await? {
//!     println!("{}: {} ({})", player.name, player.label(), player.status);
//!     if player.allows(Operation::Pause) {
//!         // Revalidates owner, track and capability before calling.
//!         mpris::control(&session, &player.id, &player.track, Operation::Pause).await.ok();
//!     }
//! }
//! # Ok(()) }
//! ```
use crate::Error;
use futures_util::{
    StreamExt, TryStreamExt,
    future::BoxFuture,
    stream::{self, FuturesUnordered},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use zbus::zvariant::OwnedValue;

pub const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
pub const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
pub const ROOT: &str = "org.mpris.MediaPlayer2";
/// Every player's bus name starts with this.
pub const PREFIX: &str = "org.mpris.MediaPlayer2.";
/// The deadline for discovery, one player's read and one control.
pub const DEADLINE: Duration = Duration::from_millis(800);
/// At most this many players are discovered.
pub const MAX_PLAYERS: usize = 64;
const CONCURRENT_READS: usize = 16;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    Previous,
    Play,
    Pause,
    Next,
}

/// Why a control was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MprisError {
    /// The owner or the track changed since the reading the request names.
    #[error("Player or track changed; try again")]
    Changed,
    /// The player does not currently advertise the operation.
    #[error("This player does not support that control")]
    Unsupported,
    /// The player sent invalid properties.
    #[error("{0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Bus(#[from] Error),
}

impl From<zbus::Error> for MprisError {
    fn from(error: zbus::Error) -> Self {
        Self::Bus(error.into())
    }
}

impl From<zbus::fdo::Error> for MprisError {
    fn from(error: zbus::fdo::Error) -> Self {
        Self::Bus(error.into())
    }
}

/// Plain text without control characters, at most 512 characters.
pub fn bounded(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(512)
        .collect()
}

/// Read the player that currently owns `bus`.
pub async fn read_player(conn: &zbus::Connection, bus: String) -> Result<Player, MprisError> {
    let dbus = zbus::fdo::DBusProxy::new(conn).await?;
    let owner = dbus
        .get_name_owner(
            bus.as_str()
                .try_into()
                .map_err(|_| MprisError::Invalid("Invalid player name"))?,
        )
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

/// Read the player at `id`'s unique owner.
pub async fn read_owned_player(
    conn: &zbus::Connection,
    id: PlayerId,
) -> Result<Player, MprisError> {
    let props = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(id.owner.clone())?
        .path(MPRIS_PATH)?
        .build()
        .await?;
    let values = props
        .get_all(zbus::names::InterfaceName::from_static_str_unchecked(
            PLAYER,
        ))
        .await?;
    let root = props
        .get_all(zbus::names::InterfaceName::from_static_str_unchecked(ROOT))
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
    if !matches!(status.as_str(), "Playing" | "Paused" | "Stopped") {
        return Err(MprisError::Invalid("Invalid playback state"));
    }
    let track = metadata
        .get("mpris:trackid")
        .and_then(|v| <&zbus::zvariant::ObjectPath<'_>>::try_from(v).ok())
        .map(ToString::to_string)
        .unwrap_or_default();
    if track.len() > 2048 {
        return Err(MprisError::Invalid("Media track identifier too long"));
    }
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
pub async fn discover_owners(conn: &zbus::Connection) -> Result<Vec<PlayerId>, Error> {
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
        let owners: Vec<Option<PlayerId>> = stream::iter(names.into_iter().take(MAX_PLAYERS))
            .map(|bus| async move {
                let name = zbus::names::BusName::try_from(bus.as_str())
                    .map_err(|_| Error::Invalid("Invalid player name"))?;
                match dbus.get_name_owner(name).await {
                    Ok(owner) => Ok(Some(PlayerId {
                        bus,
                        owner: owner.to_string(),
                        server: conn.server_guid().to_string(),
                    })),
                    // The name can disappear between ListNames and GetNameOwner.
                    Err(zbus::fdo::Error::NameHasNoOwner(_)) => Ok(None),
                    Err(error) => Err(Error::from(error)),
                }
            })
            .buffer_unordered(CONCURRENT_READS)
            .try_collect()
            .await?;
        Ok::<_, Error>(owners.into_iter().flatten().collect())
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

/// A finished read for [`Registry::finish_read`].
pub type ReadResult = (PlayerId, u64, Option<Player>);

/// Discovered players and their bounded, signal-driven property reads.
///
/// Feed it inventories ([`discover_owners`]) and signals; it starts at most
/// 16 reads at once, one per player, and keeps a player's last valid reading
/// when a read fails. A read retired by a newer one never replaces data.
#[derive(Default)]
pub struct Registry {
    records: HashMap<String, PlayerRecord>,
    next_read: u64,
}

impl Registry {
    pub fn reconcile_owners(&mut self, owners: Vec<PlayerId>) {
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

    pub fn dirty(&mut self, bus: &str) {
        if let Some(record) = self.records.get_mut(bus) {
            record.dirty = true;
        }
    }

    pub fn properties_changed(&mut self, owner: &str) {
        for record in self
            .records
            .values_mut()
            .filter(|record| record.id.owner == owner)
        {
            record.dirty = true;
        }
    }

    pub fn start_reads(
        &mut self,
        conn: &zbus::Connection,
        reads: &mut FuturesUnordered<BoxFuture<'static, ReadResult>>,
    ) {
        for record in self.records.values_mut() {
            if reads.len() == CONCURRENT_READS {
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

    pub fn finish_read(&mut self, (id, read_id, player): ReadResult) -> bool {
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

    pub fn initial_reads_complete(&self) -> bool {
        !self.records.is_empty() && self.records.values().all(|record| record.initialized)
    }

    pub fn players(&self) -> Vec<Player> {
        self.records
            .values()
            .filter_map(|record| record.player.clone())
            .collect()
    }
}

/// Every discovered player, read once.
pub async fn players(conn: &zbus::Connection) -> Result<Vec<Player>, Error> {
    let owners = discover_owners(conn).await?;
    let mut players: Vec<Player> = stream::iter(owners)
        .map(|id| async move {
            tokio::time::timeout(DEADLINE, read_owned_player(conn, id))
                .await
                .ok()
                .and_then(Result::ok)
        })
        .buffer_unordered(CONCURRENT_READS)
        .filter_map(std::future::ready)
        .collect()
        .await;
    players.sort_by(|a, b| a.id.bus.cmp(&b.id.bus));
    Ok(players)
}

/// Ask `player` for `operation` after re-reading it: the owner must be the
/// same process, the track the one shown, and the operation still allowed.
pub async fn control(
    conn: &zbus::Connection,
    player: &PlayerId,
    track: &str,
    operation: Operation,
) -> Result<(), MprisError> {
    let current = read_player(conn, player.bus.clone()).await?;
    if current.id != *player || current.track != track {
        return Err(MprisError::Changed);
    }
    if !current.allows(operation) {
        return Err(MprisError::Unsupported);
    }
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
    Ok(())
}

/// Properties of any player. Install before discovery to close the startup race.
pub fn properties_rule() -> zbus::Result<zbus::MatchRule<'static>> {
    Ok(zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .path(MPRIS_PATH)?
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .build())
}

/// Bus name ownership; players come and go with their names.
pub fn owners_rule() -> zbus::Result<zbus::MatchRule<'static>> {
    Ok(zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .build())
}

#[cfg(test)]
mod tests;
