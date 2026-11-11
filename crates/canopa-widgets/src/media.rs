use serde::{Deserialize, Serialize};

pub use canopa_services::mpris::{Operation, Player, PlayerId};

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

    pub fn reconcile(&mut self, mut players: Vec<Player>) {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Select(Option<PlayerId>),
    Control {
        player: PlayerId,
        track: String,
        operation: Operation,
    },
}

/// Observe the shell's selected player. There is deliberately no control method here.
/// Media selection remains with the shell, so the locker reflects pinned players.
pub async fn observe(sender: tokio::sync::watch::Sender<Snapshot>) {
    use futures_util::StreamExt;
    loop {
        let result = async {
            let connection = zbus::Connection::session().await?;
            let proxy = zbus::Proxy::new(
                &connection,
                "org.canopa.Shell.Media",
                "/org/canopa/Shell/Media",
                "org.canopa.Shell.Media",
            )
            .await?;
            let mut changed = proxy.receive_signal("Changed").await?;
            let mut owners = proxy.receive_owner_changed().await?;
            loop {
                let json: String = proxy.call("Snapshot", &()).await?;
                if json.len() > 1024 * 1024 {
                    return Err(anyhow::anyhow!("media snapshot exceeds budget"));
                }
                let snapshot: Snapshot = serde_json::from_str(&json)?;
                if sender.send(snapshot).is_err() {
                    return Ok(());
                }
                let ended = tokio::select! { next = changed.next() => next.is_none(), _ = owners.next() => true };
                if ended {
                    return Err(anyhow::anyhow!("media source disconnected"));
                }
            }
        }
        .await;
        if result.is_ok() || sender.is_closed() {
            return;
        }
        sender.send_replace(Snapshot::default());
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}
