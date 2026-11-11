//! Shared now-playing state. The shell owns selection; the panel is a client.
//! MPRIS discovery, reads and controls come from [`canopa_services::mpris`].
use canopa_services::mpris::{self, DEADLINE, PLAYER, PREFIX, Registry};
use futures_util::{StreamExt, future::BoxFuture, stream::FuturesUnordered};
use std::{collections::HashMap, time::Duration};
use tokio::sync::{mpsc, oneshot, watch};
use zbus::zvariant::OwnedValue;

pub const SERVICE: &str = "org.canopa.Shell.Media";
pub const PATH: &str = "/org/canopa/Shell/Media";
pub const INTERFACE: &str = "org.canopa.Shell.Media";

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
            .map_err(|_| zbus::fdo::Error::Failed(fl!("shell-media-busy")))?;
        tokio::time::timeout(Duration::from_secs(3), rx)
            .await
            .map_err(|_| zbus::fdo::Error::Failed(fl!("shell-media-timed-out")))?
            .map_err(|_| zbus::fdo::Error::Failed(fl!("shell-media-disconnected")))?
            .map_err(zbus::fdo::Error::Failed)
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
                "{}",
                fl!("shell-media-player-gone")
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
                "{}",
                fl!("shell-media-player-gone")
            );
            mpris::control(conn, &player, &track, operation).await?;
        }
    }
    Ok(())
}

async fn connected(
    conn: zbus::Connection,
    output: &watch::Sender<Snapshot>,
    mut commands: mpsc::Receiver<Command>,
) -> anyhow::Result<()> {
    let changes = mpris::properties_rule()?;
    let owners = mpris::owners_rule()?;
    // Install both matches before discovery to close the startup race.
    let mut changes = zbus::MessageStream::for_match_rule(changes, &conn, Some(128)).await?;
    let mut owners = zbus::MessageStream::for_match_rule(owners, &conn, Some(128)).await?;
    let mut state = Snapshot::default();
    let mut registry = Registry::default();
    let mut reads = FuturesUnordered::new();
    let mut inventory: Option<BoxFuture<'static, anyhow::Result<Vec<PlayerId>>>> = None;
    let mut inventory_dirty = true;
    let mut publication_needed = false;
    let mut initial_selection = true;
    loop {
        if inventory_dirty && inventory.is_none() {
            let conn = conn.clone();
            inventory = Some(Box::pin(
                async move { Ok(mpris::discover_owners(&conn).await?) },
            ));
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
                    let _ = command.result.send(Err(fl!("shell-media-expired")));
                    continue;
                }
                let target = match &command.request { Request::Control { player, .. } => Some(player.bus.clone()), _ => None };
                let result = tokio::time::timeout(DEADLINE.min(command.expires - now), apply(&conn, &mut state, command.request)).await
                    .map_err(|_| fl!("shell-media-timed-out")).and_then(|r| r.map_err(|e| e.to_string()));
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
