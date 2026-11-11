//! BlueZ discovery and application-local pairing through
//! [`canopa_services::bluetooth`]. The agent is never made the default agent
//! and accepts requests only for the device the user selected.
use super::*;
use crate::provider_io::Input;
use anyhow::{Context, Result, ensure};
use canopa_panel_protocol::{
    PanelAction,
    v5::{FormContent, FormElement, FormEvent},
};
use canopa_services::bluetooth::{
    self as bluez, AgentEvent, Attempt, PairingRequest, Question, Snapshot,
};
use std::{collections::HashSet, time::Duration};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};
use zbus::{Connection, zvariant::OwnedObjectPath};

const QUERY_TIMEOUT: Duration = Duration::from_secs(1);

type Path = OwnedObjectPath;

fn content(snapshot: &Snapshot, scanning: &HashSet<Path>, error: Option<&str>) -> PanelContent {
    let mut content = PanelContent::new(fl!("shell-bluetooth-title"));
    content.error = error.map(str::to_owned);
    if snapshot.adapters.is_empty() {
        let message = if snapshot.owner.is_some() {
            fl!("shell-bluetooth-no-adapters")
        } else {
            fl!("shell-bluetooth-unavailable")
        };
        content
            .sections
            .push(section("status", "", vec![item("unavailable", &message)]));
    }
    for adapter in &snapshot.adapters {
        let active_scan = scanning.contains(&adapter.path);
        let mut radio = item(adapter.path.as_str(), adapter.label.as_str());
        radio.body = Some(if active_scan {
            fl!("shell-bluetooth-searching")
        } else if adapter.powered {
            fl!("shell-switch-on")
        } else {
            fl!("shell-switch-off")
        });
        radio.actions.push(if adapter.powered {
            PanelAction::new("off", fl!("shell-bluetooth-turn-off"))
        } else {
            PanelAction::new("on", fl!("shell-bluetooth-turn-on"))
        });
        if adapter.powered {
            radio.actions.push(if active_scan {
                PanelAction::new("stop", fl!("shell-bluetooth-stop-searching"))
            } else {
                PanelAction::new("scan", fl!("shell-bluetooth-find-devices"))
            });
        }
        let mut items = vec![radio];
        for device in snapshot
            .devices
            .iter()
            .filter(|d| d.adapter == adapter.path && (active_scan || d.paired || d.connected))
        {
            let mut row = item(device.path.as_str(), device.label.as_str());
            let (status, key, label) = if device.connected {
                (
                    fl!("shell-bluetooth-connected"),
                    "disconnect",
                    fl!("shell-bluetooth-disconnect"),
                )
            } else if device.paired {
                (
                    fl!("shell-bluetooth-paired"),
                    "connect",
                    fl!("shell-bluetooth-connect"),
                )
            } else {
                (
                    fl!("shell-bluetooth-available"),
                    "pair",
                    fl!("shell-bluetooth-pair"),
                )
            };
            row.body = Some(status);
            row.actions.push(PanelAction::new(key, label));
            items.push(row);
        }
        content
            .sections
            .push(section(adapter.path.as_str(), "", items));
    }
    content
}

fn question_label(question: &Question) -> String {
    match question {
        // Codes are passed as text: they keep their leading zeros and no grouping.
        Question::Confirm(code) => {
            fl!("shell-bluetooth-confirm-code", code = format!("{code:06}"))
        }
        Question::Authorize => fl!("shell-bluetooth-authorize"),
        Question::Service => fl!("shell-bluetooth-service"),
        Question::Pin => fl!("shell-bluetooth-enter-pin"),
        Question::Passkey => fl!("shell-bluetooth-enter-passkey"),
        Question::DisplayPinCode(pincode) => {
            fl!("shell-bluetooth-type-pin", pin = pincode.to_string())
        }
        Question::DisplayPasskey { passkey, entered } => fl!(
            "shell-bluetooth-type-passkey",
            passkey = format!("{passkey:06}"),
            entered = u64::from(*entered.min(&6))
        ),
    }
}

struct Prompt {
    question: Question,
    reply: Option<oneshot::Sender<String>>,
    entry: String,
    invalid: bool,
}

impl Prompt {
    fn submit(&mut self) -> bool {
        if let Some(value) = self.question.accepts(&self.entry) {
            if let Some(reply) = self.reply.take() {
                let _ = reply.send(value);
            }
            true
        } else {
            self.invalid = true;
            self.entry.clear();
            false
        }
    }
}

impl From<PairingRequest> for Prompt {
    fn from(request: PairingRequest) -> Self {
        Self {
            question: request.question,
            reply: request.reply,
            entry: String::new(),
            invalid: false,
        }
    }
}

struct Pairing {
    attempt: Attempt,
    label: String,
    task: JoinHandle<Result<(), canopa_services::Error>>,
    stop: oneshot::Sender<()>,
    prompt: Option<Prompt>,
}

impl Pairing {
    fn content(&self) -> Content {
        let mut elements = vec![FormElement::Text {
            text: fl!("shell-bluetooth-pair-with", device = self.label.clone()),
        }];
        if let Some(prompt) = &self.prompt {
            elements.push(FormElement::Text {
                text: question_label(&prompt.question),
            });
            if prompt.question.needs_entry() {
                elements.push(FormElement::TextInput {
                    id: "code".into(),
                    label: fl!("shell-bluetooth-pairing-code"),
                    value: String::new(),
                    secret: true,
                    error: prompt.invalid.then(|| fl!("shell-bluetooth-invalid-code")),
                });
            }
        } else {
            elements.push(FormElement::Text {
                text: fl!("shell-bluetooth-waiting"),
            });
        }
        elements.push(FormElement::Button {
            id: "cancel".into(),
            label: fl!("shell-bluetooth-cancel-pairing"),
        });
        if self
            .prompt
            .as_ref()
            .is_some_and(|prompt| prompt.reply.is_some())
        {
            elements.push(FormElement::Button {
                id: "confirm".into(),
                label: fl!("shell-bluetooth-confirm"),
            });
        }
        Content::Form(FormContent { elements })
    }
}

struct Bluetooth {
    panel: Session,
    snapshot: Snapshot,
    scanning: HashSet<Path>,
    pairing: Option<Pairing>,
    events: mpsc::Sender<AgentEvent>,
    serial: u64,
    error: Option<String>,
}

impl Bluetooth {
    async fn publish(&mut self, output: &mut (impl AsyncWrite + Unpin)) -> Result<()> {
        if let Some(pairing) = &self.pairing {
            let frame = self.panel.replace(pairing.content(), Update::Replace)?;
            crate::provider_io::write_v5(output, frame).await?;
        } else {
            let content = content(&self.snapshot, &self.scanning, self.error.as_deref());
            publish(&mut self.panel, output, content).await?;
        }
        Ok(())
    }

    async fn refresh(&mut self, connection: &Connection) -> bool {
        let next = Snapshot::read(connection).await;
        if self.snapshot == next {
            return false;
        }
        if self.snapshot.owner != next.owner {
            self.scanning.clear();
        } else {
            self.scanning
                .retain(|path| next.adapters.iter().any(|a| a.powered && &a.path == path));
        }
        self.snapshot = next;
        true
    }

    async fn stop_pair(&mut self) {
        if let Some(Pairing {
            stop, prompt, task, ..
        }) = self.pairing.take()
        {
            let _ = stop.send(());
            drop(prompt);
            let _ = timeout(QUERY_TIMEOUT * 2, task).await;
        }
    }

    async fn apply(&mut self, connection: &Connection, id: &str, key: &str) -> Result<()> {
        let snapshot = &self.snapshot;
        let owner = snapshot
            .owner
            .as_ref()
            .context("Bluetooth is unavailable.")?;
        if let Some(adapter) = snapshot.adapters.iter().find(|a| a.path.as_str() == id) {
            match key {
                "on" | "off" => {
                    bluez::set_powered(connection, owner, id, key == "on").await?;
                    if key == "off" {
                        self.scanning.remove(&adapter.path);
                    }
                }
                "scan" if adapter.powered && !self.scanning.contains(&adapter.path) => {
                    bluez::start_discovery(connection, owner, id).await?;
                    self.scanning.insert(adapter.path.clone());
                }
                "stop" if self.scanning.contains(&adapter.path) => {
                    bluez::stop_discovery(connection, owner, id).await?;
                    self.scanning.remove(&adapter.path);
                }
                _ => {}
            }
        } else if let Some(device) = snapshot.devices.iter().find(|d| d.path.as_str() == id) {
            ensure!(
                device.paired || device.connected || self.scanning.contains(&device.adapter),
                "Device is not visible."
            );
            match key {
                "pair" if !device.paired && !device.connected => {
                    self.serial += 1;
                    let attempt = Attempt {
                        serial: self.serial,
                        owner: owner.clone(),
                        device: device.path.clone(),
                    };
                    let (stop, canceled) = oneshot::channel();
                    let task =
                        tokio::spawn(bluez::pair(attempt.clone(), self.events.clone(), canceled));
                    self.pairing = Some(Pairing {
                        attempt,
                        label: device.label.clone(),
                        task,
                        stop,
                        prompt: None,
                    });
                }
                "connect" if device.paired && !device.connected => {
                    bluez::connect(connection, owner, id).await?
                }
                "disconnect" if device.connected => {
                    bluez::disconnect(connection, owner, id).await?
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn event(&mut self, connection: &Connection, message: HostMessage) -> bool {
        if !self.panel.accepts(&message) {
            return false;
        }
        if let Some(pairing) = self.pairing.as_mut() {
            let HostMessage::Event {
                interaction: Interaction::Form(event),
                ..
            } = message
            else {
                return false;
            };
            match event {
                FormEvent::ButtonActivated { element_id } if element_id == "cancel" => {
                    self.stop_pair().await;
                    self.error = Some(fl!("shell-bluetooth-pairing-canceled"));
                }
                FormEvent::TextInputChanged { element_id, value } if element_id == "code" => {
                    if let Some(prompt) = pairing.prompt.as_mut() {
                        prompt.entry = value.chars().take(17).collect();
                    }
                    return false;
                }
                FormEvent::ButtonActivated { element_id } if element_id == "confirm" => {
                    if pairing.prompt.as_mut().is_some_and(Prompt::submit) {
                        pairing.prompt = None;
                    }
                }
                FormEvent::TextInputSubmitted { element_id, value } if element_id == "code" => {
                    if let Some(prompt) = pairing.prompt.as_mut() {
                        prompt.entry = value;
                        if prompt.submit() {
                            pairing.prompt = None;
                        }
                    }
                }
                _ => return false,
            }
        } else if let Some((id, key, None)) = action(&self.panel, &message) {
            // A path from an older daemon or a removed device is never retried.
            let previous = &self.snapshot;
            let owner = previous.owner.clone();
            let device = previous
                .devices
                .iter()
                .find(|d| d.path.as_str() == id)
                .cloned();
            let adapter = previous
                .adapters
                .iter()
                .find(|a| a.path.as_str() == id)
                .cloned();
            self.refresh(connection).await;
            if self.snapshot.owner != owner
                || device.is_some_and(|device| !self.snapshot.devices.contains(&device))
                || adapter.is_some_and(|adapter| !self.snapshot.adapters.contains(&adapter))
            {
                return true;
            }
            let result = timeout(Duration::from_secs(8), self.apply(connection, id, key)).await;
            self.error =
                (!matches!(result, Ok(Ok(())))).then(|| fl!("shell-bluetooth-change-failed"));
            if self.pairing.is_none() {
                self.refresh(connection).await;
            }
        } else {
            return false;
        }
        true
    }
}

pub(super) async fn run(
    mut input: Input<HostMessage>,
    output: &mut (impl AsyncWrite + Unpin),
) -> Result<()> {
    let (events, mut prompts) = mpsc::channel(8);
    let mut bluetooth = Bluetooth {
        panel: Session::default(),
        snapshot: Snapshot::default(),
        scanning: HashSet::new(),
        pairing: None,
        events,
        serial: 0,
        error: None,
    };
    bluetooth.publish(output).await?;
    let mut connection = None;
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let result = async {
        loop {
            tokio::select! {
                _ = tick.tick(), if bluetooth.pairing.is_none() => {
                    if connection.is_none()
                        && let Ok(Ok(bus)) = timeout(QUERY_TIMEOUT, Connection::system()).await
                    {
                        connection = Some(bus);
                    }
                    if let Some(bus) = &connection && bluetooth.refresh(bus).await {
                        bluetooth.publish(output).await?;
                    }
                }
                message = input.next() => {
                    let Some(message) = message? else { return Ok(()); };
                    if matches!(message, HostMessage::Close { .. }) { return Ok(()); }
                    if let Some(bus) = &connection && bluetooth.event(bus, message).await {
                        bluetooth.publish(output).await?;
                    }
                }
                event = prompts.recv() => {
                    if let Some(pairing) = bluetooth.pairing.as_mut() {
                        match event {
                            Some(AgentEvent::Request(serial, request)) if serial == pairing.attempt.serial => pairing.prompt = Some(request.into()),
                            Some(AgentEvent::Dismiss(serial)) if serial == pairing.attempt.serial => pairing.prompt = None,
                            _ => continue,
                        }
                        bluetooth.publish(output).await?;
                    }
                }
                result = async {
                    match bluetooth.pairing.as_mut() {
                        Some(pairing) => (&mut pairing.task).await,
                        None => std::future::pending().await,
                    }
                } => {
                    bluetooth.pairing = None;
                    bluetooth.error = (!matches!(result, Ok(Ok(())))).then(|| fl!("shell-bluetooth-pairing-failed"));
                    if let Some(bus) = &connection {
                        bluetooth.refresh(bus).await;
                    }
                    bluetooth.publish(output).await?;
                }
            }
        }
    }.await;
    bluetooth.stop_pair().await;
    if let Some(bus) = connection {
        // BlueZ releases this connection's discovery sessions when it leaves.
        let _ = timeout(QUERY_TIMEOUT, bus.close()).await;
    }
    result
}
