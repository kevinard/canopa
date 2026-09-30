//! Device controls presented through the existing accessible rich panel contract.
use crate::audio::{self, AudioDeviceId, AudioSnapshot, DeviceKind, DeviceOperation};
use canopa_panel_protocol::v5::{self, Content, HostMessage, Interaction, Session, Update};
use canopa_panel_protocol::{
    InteractionTarget, PanelAction, PanelContent, PanelItem, PanelSection,
};
use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};

type ActionMap = HashMap<(String, String), (AudioDeviceId, DeviceOperation)>;
type PendingOperation = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

#[derive(Default)]
struct AudioPanel {
    session: Session,
    actions: ActionMap,
}

impl AudioPanel {
    fn publish(
        &mut self,
        snapshot: Option<&AudioSnapshot>,
        busy: bool,
        error: Option<&str>,
    ) -> Result<v5::Response, canopa_panel_protocol::ProtocolError> {
        self.actions.clear();
        let mut content = PanelContent::new("Sound");
        content.error = error.map(str::to_owned);
        let Some(snapshot) = snapshot else {
            content.error = Some("Audio is unavailable. Reconnecting…".into());
            return self
                .session
                .replace(Content::Rich(content), Update::Replace);
        };
        for (kind, title, empty) in [
            (
                DeviceKind::Output,
                "Speakers and headphones",
                "No output devices are available.",
            ),
            (
                DeviceKind::Input,
                "Microphones",
                "No input devices are available.",
            ),
        ] {
            let mut items = Vec::new();
            for device in snapshot
                .devices
                .iter()
                .filter(|device| device.id.kind == kind)
            {
                let id = format!("{kind:?}-{}-{}", device.id.session, device.id.index);
                let mut actions = Vec::new();
                {
                    for (key, label, operation) in [
                        (
                            "quieter",
                            "Volume −5%",
                            DeviceOperation::AdjustVolume { step: 5, up: false },
                        ),
                        (
                            "louder",
                            "Volume +5%",
                            DeviceOperation::AdjustVolume { step: 5, up: true },
                        ),
                        (
                            "mute",
                            if device.muted { "Unmute" } else { "Mute" },
                            DeviceOperation::SetMuted(!device.muted),
                        ),
                        ("default", "Use this device", DeviceOperation::SetDefault),
                    ] {
                        if key == "default" && device.is_default {
                            continue;
                        }
                        if key == "quieter" && device.volume <= 0.0 {
                            continue;
                        }
                        if key == "louder" && device.volume >= 100.0 {
                            continue;
                        }
                        self.actions
                            .insert((id.clone(), key.into()), (device.id.clone(), operation));
                        actions.push(PanelAction::new(key, label));
                    }
                }
                let status = format!(
                    "{:.0}% · {}{}{}",
                    device.volume,
                    if device.muted { "Muted" } else { "Unmuted" },
                    if device.is_default { " · Default" } else { "" },
                    if busy { " · Applying change…" } else { "" }
                );
                items.push(PanelItem {
                    id,
                    title: device.description.clone(),
                    body: Some(status),
                    attribution: None,
                    styled_body: None,
                    rich_body: None,
                    metadata: Vec::new(),
                    actions,
                });
            }
            if items.is_empty() {
                items.push(PanelItem {
                    id: format!("empty-{kind:?}"),
                    title: empty.into(),
                    body: None,
                    attribution: None,
                    styled_body: None,
                    rich_body: None,
                    metadata: Vec::new(),
                    actions: Vec::new(),
                });
            }
            content.sections.push(PanelSection {
                id: format!("{kind:?}"),
                title: title.into(),
                icon: None,
                items,
                controls: Vec::new(),
            });
        }
        self.session
            .replace(Content::Rich(content), Update::Replace)
    }

    fn action(&self, message: &HostMessage) -> Option<(AudioDeviceId, DeviceOperation)> {
        if !self.session.accepts(message) {
            return None;
        }
        let HostMessage::Event {
            interaction:
                Interaction::Rich(canopa_panel_protocol::InteractionEvent::Invoke {
                    target: InteractionTarget::Item { id },
                    action_key,
                    ..
                }),
            ..
        } = message
        else {
            return None;
        };
        self.actions.get(&(id.clone(), action_key.clone())).cloned()
    }
}

async fn write(
    output: &mut (impl AsyncWrite + Unpin),
    response: v5::Response,
) -> anyhow::Result<()> {
    let frame = response.encode()?;
    tokio::time::timeout(Duration::from_secs(2), async {
        output.write_all(&frame).await?;
        output.flush().await
    })
    .await??;
    Ok(())
}

pub async fn run(
    mut input: impl AsyncBufRead + Unpin,
    mut output: impl AsyncWrite + Unpin,
    server: Option<String>,
) -> anyhow::Result<()> {
    let (audio, mut state) = audio::monitor(server);
    let mut panel = AudioPanel::default();
    let mut current = state.borrow_and_update().clone();
    let mut pending: Option<PendingOperation> = None;
    let mut error = None;
    write(&mut output, panel.publish(current.as_ref(), false, None)?).await?;
    loop {
        // Preserve a partially read frame across unrelated device updates.
        let frame = HostMessage::read_async(&mut input);
        tokio::pin!(frame);
        let message = loop {
            tokio::select! {
                message = &mut frame => break message?,
                change = state.changed() => {
                    change?;
                    current = state.borrow_and_update().clone();
                    write(&mut output, panel.publish(current.as_ref(), pending.is_some(), error.as_deref())?).await?;
                }
                result = async {
                    match pending.as_mut() {
                        Some(operation) => operation.await,
                        None => std::future::pending().await,
                    }
                } => {
                    pending = None;
                    error = result.err();
                    // The observer supplies authoritative values; do not invent
                    // a successful volume/default state from command completion.
                    current = state.borrow_and_update().clone();
                    write(&mut output, panel.publish(current.as_ref(), false, error.as_deref())?).await?;
                }
            }
        };
        let Some(message) = message else {
            return Ok(());
        };
        if matches!(message, HostMessage::Close { .. }) {
            return Ok(());
        }
        if pending.is_none()
            && let Some((id, operation)) = panel.action(&message)
        {
            error = None;
            write(&mut output, panel.publish(current.as_ref(), true, None)?).await?;
            let audio = audio.clone();
            pending = Some(Box::pin(async move { audio.apply(id, operation).await }));
        }
    }
}

#[cfg(test)]
mod tests;
