//! Declarative controls for the shell-owned media selection.
use crate::media::{self, Operation, Request, Snapshot};
use canopa_panel_protocol::v5::{self, Content, HostMessage, Interaction, Session, Update};
use canopa_panel_protocol::{
    InteractionTarget, PanelAction, PanelContent, PanelControl, PanelControlOption, PanelItem,
    PanelSection,
};
use iced::futures::StreamExt;
use std::{collections::HashMap, time::Duration};
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};

#[derive(Default)]
struct MediaPanel {
    session: Session,
    requests: HashMap<(String, Option<String>), Request>,
}

impl MediaPanel {
    fn publish(
        &mut self,
        snapshot: &Snapshot,
        error: Option<String>,
    ) -> anyhow::Result<v5::Response> {
        self.requests.clear();
        let mut content = PanelContent::new("Media");
        content.error = error;
        let mut section = PanelSection {
            id: "media".into(),
            title: "Now playing".into(),
            icon: None,
            items: vec![],
            controls: vec![],
        };
        if snapshot.available {
            self.requests.insert(
                ("player".into(), Some("auto".into())),
                Request::Select(None),
            );
            let mut options = vec![PanelControlOption {
                value: "auto".into(),
                label: "Automatic".into(),
            }];
            let mut selected = "auto".to_string();
            for (index, player) in snapshot.players.iter().enumerate() {
                let value = index.to_string();
                self.requests.insert(
                    ("player".into(), Some(value.clone())),
                    Request::Select(Some(player.id.clone())),
                );
                if snapshot.pinned && snapshot.selected.as_ref() == Some(&player.id) {
                    selected = value.clone();
                }
                options.push(PanelControlOption {
                    value,
                    label: format!("{} · {}", player.name, player.status),
                });
            }
            section.controls.push(PanelControl {
                key: "player".into(),
                label: "Player".into(),
                value: selected,
                options,
            });
        }
        let (title, body, actions) = if let Some(player) = snapshot.current() {
            let toggle = if player.status == "Playing" {
                ("pause", "Pause", Operation::Pause)
            } else {
                ("play", "Play", Operation::Play)
            };
            let mut actions = vec![];
            for (key, label, operation) in [
                ("previous", "Previous", Operation::Previous),
                toggle,
                ("next", "Next", Operation::Next),
            ] {
                if player.allows(operation) {
                    actions.push(PanelAction::new(key, label));
                    self.requests.insert(
                        (key.into(), None),
                        Request::Control {
                            player: player.id.clone(),
                            track: player.track.clone(),
                            operation,
                        },
                    );
                }
            }
            let body = [
                player.artist.as_str(),
                player.name.as_str(),
                player.status.as_str(),
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
            (
                if player.title.is_empty() {
                    player.name.clone()
                } else {
                    player.title.clone()
                },
                Some(body),
                actions,
            )
        } else {
            (
                if snapshot.available {
                    "No media players are open."
                } else {
                    "Media is unavailable. Reconnecting…"
                }
                .into(),
                None,
                vec![],
            )
        };
        section.items.push(PanelItem {
            id: "track".into(),
            title,
            body,
            actions,
            attribution: None,
            styled_body: None,
            rich_body: None,
            metadata: vec![],
            slider: None,
        });
        content.sections.push(section);
        Ok(self
            .session
            .replace(Content::Rich(content), Update::Replace)?)
    }

    fn request(&self, message: &HostMessage) -> Option<Request> {
        if !self.session.accepts(message) {
            return None;
        }
        let HostMessage::Event {
            interaction:
                Interaction::Rich(canopa_panel_protocol::InteractionEvent::Invoke {
                    target,
                    action_key,
                    value,
                }),
            ..
        } = message
        else {
            return None;
        };
        let valid_target = if action_key == "player" {
            matches!(target, InteractionTarget::Section { id } if id == "media")
        } else {
            matches!(target, InteractionTarget::Item { id } if id == "track")
        };
        valid_target
            .then(|| {
                self.requests
                    .get(&(action_key.clone(), value.clone()))
                    .cloned()
            })
            .flatten()
    }
}

struct Client {
    proxy: zbus::Proxy<'static>,
    changes: zbus::proxy::SignalStream<'static>,
    owners: zbus::MessageStream,
}

impl Client {
    async fn connect() -> anyhow::Result<Self> {
        let conn = zbus::Connection::session().await?;
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender("org.freedesktop.DBus")?
            .interface("org.freedesktop.DBus")?
            .member("NameOwnerChanged")?
            .add_arg(media::SERVICE)?
            .build();
        let owners = zbus::MessageStream::for_match_rule(rule, &conn, Some(8)).await?;
        let owner = zbus::fdo::DBusProxy::new(&conn)
            .await?
            .get_name_owner(media::SERVICE.try_into()?)
            .await?;
        let proxy = zbus::Proxy::new_owned(
            conn,
            owner.to_string(),
            media::PATH.to_string(),
            media::INTERFACE.to_string(),
        )
        .await?;
        let changes = proxy.receive_signal("Changed").await?;
        Ok(Self {
            proxy,
            changes,
            owners,
        })
    }
    async fn snapshot(&self) -> anyhow::Result<Snapshot> {
        let json: String =
            tokio::time::timeout(Duration::from_millis(800), self.proxy.call("Snapshot", &()))
                .await??;
        anyhow::ensure!(json.len() <= 1024 * 1024, "Media snapshot too large");
        Ok(serde_json::from_str(&json)?)
    }
    async fn next(&mut self) -> anyhow::Result<Snapshot> {
        tokio::select! {
            message = self.changes.next() => {
                message.ok_or_else(|| anyhow::anyhow!("Media disconnected"))?;
                // A signal may predate the initial Snapshot reply. Read the
                // latest state so queued signals cannot rewind the selection.
                self.snapshot().await
            },
            _ = self.owners.next() => anyhow::bail!("Media connection changed"),
        }
    }
    async fn apply(&self, request: Request) -> anyhow::Result<()> {
        self.proxy
            .call::<_, _, ()>("Apply", &(serde_json::to_string(&request)?,))
            .await?;
        Ok(())
    }
}

async fn write(
    output: &mut (impl AsyncWrite + Unpin),
    response: v5::Response,
) -> anyhow::Result<()> {
    let bytes = response.encode()?;
    tokio::time::timeout(Duration::from_secs(2), async {
        output.write_all(&bytes).await?;
        output.flush().await
    })
    .await??;
    Ok(())
}

pub async fn run(
    mut input: impl AsyncBufRead + Unpin,
    mut output: impl AsyncWrite + Unpin,
) -> anyhow::Result<()> {
    let mut panel = MediaPanel::default();
    let mut snapshot = Snapshot::default();
    let mut client: Option<Client> = None;
    let mut retry = tokio::time::interval(Duration::from_secs(1));
    retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    write(&mut output, panel.publish(&snapshot, None)?).await?;
    loop {
        // Keep a partially consumed host frame alive across metadata updates.
        let frame = HostMessage::read_async(&mut input);
        tokio::pin!(frame);
        let message = loop {
            tokio::select! {
                message = &mut frame => break message?,
                _ = retry.tick(), if client.is_none() => {
                    if let Ok(Ok((connected, state))) = tokio::time::timeout(Duration::from_millis(800), async {
                        let connected = Client::connect().await?;
                        let state = connected.snapshot().await?;
                        Ok::<_, anyhow::Error>((connected, state))
                    }).await {
                        client = Some(connected);
                        snapshot = state;
                        write(&mut output, panel.publish(&snapshot, None)?).await?;
                    }
                },
                update = async { match client.as_mut() { Some(client) => client.next().await, None => std::future::pending().await } } => {
                    match update {
                        Ok(state) => { if state == snapshot { continue; } snapshot = state; },
                        Err(_) => { client = None; snapshot = Snapshot::default(); }
                    }
                    write(&mut output, panel.publish(&snapshot, None)?).await?;
                }
            }
        };
        let Some(message) = message else {
            return Ok(());
        };
        if matches!(message, HostMessage::Close { .. }) {
            return Ok(());
        }
        if let Some(request) = panel.request(&message)
            && let Some(client) = &client
        {
            let error =
                match tokio::time::timeout(Duration::from_secs(2), client.apply(request)).await {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(error.to_string()),
                    Err(_) => Some("Media control timed out. Please try again.".into()),
                };
            // Retire the interaction revision even if the player emits no update.
            write(&mut output, panel.publish(&snapshot, error)?).await?;
        }
    }
}

#[cfg(test)]
mod tests;
