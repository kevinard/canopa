//! Device controls presented through the existing accessible rich panel contract.
use crate::audio::{self, AudioDeviceId, AudioSnapshot, DeviceKind, DeviceOperation};
use canopa_panel_protocol::v5::{self, Content, HostMessage, Interaction, Session, Update};
use canopa_panel_protocol::{
    InteractionTarget, PanelAction, PanelContent, PanelItem, PanelSection,
};
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    pin::Pin,
    time::Duration,
};
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};

type ActionMap = HashMap<(String, String), (AudioDeviceId, DeviceOperation)>;
type PendingOperation = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

#[derive(Default)]
struct AudioPanel {
    session: Session,
    actions: ActionMap,
    volumes: HashMap<String, AudioDeviceId>,
}

impl AudioPanel {
    fn publish(
        &mut self,
        snapshot: Option<&AudioSnapshot>,
        busy: bool,
        error: Option<&str>,
    ) -> Result<v5::Response, canopa_panel_protocol::ProtocolError> {
        self.actions.clear();
        self.volumes.clear();
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
                for (key, label, operation) in [
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
                    self.actions
                        .insert((id.clone(), key.into()), (device.id.clone(), operation));
                    actions.push(PanelAction::new(key, label));
                }
                let status = format!(
                    "{}{}{}",
                    if device.muted { "Muted" } else { "Unmuted" },
                    if device.is_default { " · Default" } else { "" },
                    if busy { " · Applying change…" } else { "" }
                );
                self.volumes.insert(id.clone(), device.id.clone());
                items.push(PanelItem {
                    slider: Some(canopa_panel_protocol::PanelSlider {
                        key: "volume".into(),
                        label: "Volume".into(),
                        min: 0,
                        max: 100,
                        step: 1,
                        value: device.max_volume_percent().round().clamp(0.0, 100.0) as u16,
                        unit: "%".into(),
                    }),
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
                    slider: None,
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
                    value,
                }),
            ..
        } = message
        else {
            return None;
        };
        if action_key == "volume" {
            return Some((
                self.volumes.get(id)?.clone(),
                DeviceOperation::SetVolume {
                    percent: value.as_deref()?.parse().ok()?,
                },
            ));
        }
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
    let mut pending_volume = false;
    let mut queued: VecDeque<(AudioDeviceId, DeviceOperation)> = VecDeque::new();
    let mut error = None;
    write(&mut output, panel.publish(current.as_ref(), false, None)?).await?;
    loop {
        // Preserve a partially read frame across unrelated device updates.
        let frame = HostMessage::read_async(&mut input);
        tokio::pin!(frame);
        let message = loop {
            if pending.is_none()
                && let Some((id, operation)) = queued.pop_front()
            {
                let had_error = error.take().is_some();
                pending_volume = matches!(operation, DeviceOperation::SetVolume { .. });
                // Keep the committed preview until the observer publishes the
                // actual volume; a progress echo would restore the old level.
                if !pending_volume || had_error {
                    write(&mut output, panel.publish(current.as_ref(), true, None)?).await?;
                }
                let audio = audio.clone();
                pending = Some(Box::pin(async move { audio.apply(id, operation).await }));
            }
            tokio::select! {
                message = &mut frame => break message?,
                change = state.changed() => {
                    change?;
                    current = state.borrow_and_update().clone();
                    write(&mut output, panel.publish(current.as_ref(), pending.is_some() && !pending_volume, error.as_deref())?).await?;
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
                    let latest = state.borrow_and_update().clone();
                    let changed = latest != current;
                    current = latest;
                    if !pending_volume || error.is_some() || changed {
                        write(&mut output, panel.publish(current.as_ref(), false, error.as_deref())?).await?;
                    }
                }
            }
        };
        let Some(message) = message else {
            return Ok(());
        };
        if matches!(message, HostMessage::Close { .. }) {
            return Ok(());
        }
        if let Some((id, operation)) = panel.action(&message) {
            // Validate the published revision on receipt. Accepted commands keep
            // their exact device identity, which the audio worker revalidates.
            // Coalesce only adjacent absolute volume edits: mute/default retain
            // their order relative to volume and to other devices.
            if matches!(operation, DeviceOperation::SetVolume { .. })
                && let Some((last_id, last_operation)) = queued.back_mut()
                && *last_id == id
                && matches!(last_operation, DeviceOperation::SetVolume { .. })
            {
                *last_operation = operation;
            } else if queued.len() < 8 {
                queued.push_back((id, operation));
            } else {
                error = Some("Too many audio changes are pending. Please try again.".into());
                write(
                    &mut output,
                    panel.publish(current.as_ref(), !pending_volume, error.as_deref())?,
                )
                .await?;
            }
        }
    }
}

#[cfg(test)]
mod tests;
