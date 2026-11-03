//! BlueZ discovery and application-local pairing. The agent is never made the
//! default agent and accepts requests only for the device the user selected.
use super::*;
use crate::provider_io::Input;
use anyhow::{Context, Result, ensure};
use canopa_panel_protocol::{
    PanelAction,
    v5::{FormContent, FormElement, FormEvent},
};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};
use zbus::{
    Connection, Proxy, fdo,
    names::OwnedUniqueName,
    zvariant::{OwnedObjectPath, OwnedValue},
};

const BLUEZ: &str = "org.bluez";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";
const AGENT_PATH: &str = "/org/canopa/BluetoothAgent";
const QUERY_TIMEOUT: Duration = Duration::from_secs(1);

type Path = OwnedObjectPath;
type Properties = HashMap<String, OwnedValue>;

fn string(properties: &Properties, name: &str) -> Option<String> {
    Some(<&str>::try_from(properties.get(name)?).ok()?.to_owned())
}

fn flag(properties: &Properties, name: &str) -> bool {
    properties.get(name).and_then(|v| bool::try_from(v).ok()) == Some(true)
}

fn display_name(properties: &Properties, fallback: &str) -> String {
    let label: String = string(properties, "Alias")
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    if label.trim().is_empty() {
        fallback.into()
    } else {
        label
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Adapter {
    path: Path,
    powered: bool,
    label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Device {
    path: Path,
    adapter: Path,
    label: String,
    paired: bool,
    connected: bool,
}

#[derive(Default, PartialEq, Eq)]
struct Snapshot {
    owner: Option<OwnedUniqueName>,
    adapters: Vec<Adapter>,
    devices: Vec<Device>,
}

impl Snapshot {
    async fn query(connection: &Connection) -> zbus::Result<Self> {
        let bus = fdo::DBusProxy::new(connection).await?;
        let owner = bus.get_name_owner(BLUEZ.try_into()?).await?;
        let manager = proxy(
            connection,
            &owner,
            "/",
            "org.freedesktop.DBus.ObjectManager",
        )
        .await?;
        let objects: fdo::ManagedObjects = manager.call("GetManagedObjects", &()).await?;
        let mut adapters = Vec::new();
        let mut devices = Vec::new();
        for (path, interfaces) in objects {
            for (interface, properties) in interfaces {
                match interface.as_str() {
                    ADAPTER => adapters.push(Adapter {
                        label: display_name(&properties, path.as_str()),
                        powered: flag(&properties, "Powered"),
                        path: path.clone(),
                    }),
                    DEVICE => {
                        let Some(adapter) = properties
                            .get("Adapter")
                            .and_then(|v| Path::try_from(v.try_clone().ok()?).ok())
                        else {
                            continue;
                        };
                        let fallback = string(&properties, "Address")
                            .unwrap_or_else(|| "Unnamed device".into());
                        devices.push(Device {
                            label: display_name(&properties, &fallback),
                            adapter,
                            path: path.clone(),
                            paired: flag(&properties, "Paired"),
                            connected: flag(&properties, "Connected"),
                        });
                    }
                    _ => {}
                }
            }
        }
        adapters.sort_by(|a, b| a.path.cmp(&b.path));
        adapters.truncate(8);
        devices.retain(|device| {
            adapters
                .iter()
                .any(|a| a.powered && a.path == device.adapter)
        });
        devices.sort_by(|a, b| {
            (!a.connected, !a.paired, &a.label, a.path.as_str()).cmp(&(
                !b.connected,
                !b.paired,
                &b.label,
                b.path.as_str(),
            ))
        });
        devices.truncate(32);
        Ok(Self {
            owner: Some(owner),
            adapters,
            devices,
        })
    }

    async fn read(connection: &Connection) -> Self {
        timeout(QUERY_TIMEOUT, Self::query(connection))
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default()
    }

    fn content(&self, scanning: &HashSet<Path>, error: Option<&str>) -> PanelContent {
        let mut content = PanelContent::new("Bluetooth");
        content.error = error.map(str::to_owned);
        if self.adapters.is_empty() {
            let message = if self.owner.is_some() {
                "No Bluetooth adapters found."
            } else {
                "Bluetooth service is unavailable."
            };
            content
                .sections
                .push(section("status", "", vec![item("unavailable", message)]));
        }
        for adapter in &self.adapters {
            let active_scan = scanning.contains(&adapter.path);
            let mut radio = item(adapter.path.as_str(), adapter.label.as_str());
            radio.body = Some(
                if active_scan {
                    "Searching for devices…"
                } else if adapter.powered {
                    "On"
                } else {
                    "Off"
                }
                .into(),
            );
            radio.actions.push(if adapter.powered {
                PanelAction::new("off", "Turn off")
            } else {
                PanelAction::new("on", "Turn on")
            });
            if adapter.powered {
                radio.actions.push(if active_scan {
                    PanelAction::new("stop", "Stop searching")
                } else {
                    PanelAction::new("scan", "Find devices")
                });
            }
            let mut items = vec![radio];
            for device in self
                .devices
                .iter()
                .filter(|d| d.adapter == adapter.path && (active_scan || d.paired || d.connected))
            {
                let mut row = item(device.path.as_str(), device.label.as_str());
                let (status, key, label) = if device.connected {
                    ("Connected", "disconnect", "Disconnect")
                } else if device.paired {
                    ("Paired", "connect", "Connect")
                } else {
                    ("Available", "pair", "Pair")
                };
                row.body = Some(status.into());
                row.actions.push(PanelAction::new(key, label));
                items.push(row);
            }
            content
                .sections
                .push(section(adapter.path.as_str(), "", items));
        }
        content
    }
}

async fn proxy<'a>(
    connection: &'a Connection,
    owner: &OwnedUniqueName,
    path: &str,
    interface: &str,
) -> zbus::Result<Proxy<'a>> {
    Proxy::new_owned(
        connection.clone(),
        owner.clone(),
        path.to_owned(),
        interface.to_owned(),
    )
    .await
}

#[derive(Clone, PartialEq, Eq)]
struct Attempt {
    serial: u64,
    owner: OwnedUniqueName,
    device: Path,
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum AgentError {
    Canceled,
    Rejected,
}

enum Question {
    Confirm(u32),
    Authorize,
    Service,
    Pin,
    Passkey,
    Display(String),
}

impl Question {
    fn label(&self) -> String {
        match self {
            Self::Confirm(code) => format!("Does the other device show {code:06}?"),
            Self::Authorize => "Allow this pairing request?".into(),
            Self::Service => "Allow this device to connect?".into(),
            Self::Pin => "Enter the PIN shown by the other device.".into(),
            Self::Passkey => "Enter the six-digit passkey shown by the other device.".into(),
            Self::Display(text) => text.clone(),
        }
    }
    fn reply(&self, value: &str) -> Option<String> {
        let accepted = match self {
            Self::Pin => {
                !value.is_empty()
                    && value.len() <= 16
                    && value.bytes().all(|b| b.is_ascii_alphanumeric())
            }
            Self::Passkey => value.len() == 6 && value.bytes().all(|b| b.is_ascii_digit()),
            Self::Display(_) => false,
            _ => true,
        };
        accepted.then(|| value.to_owned())
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
        if let Some(value) = self.question.reply(&self.entry) {
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

enum AgentEvent {
    Prompt(u64, Prompt),
    Dismiss(u64),
}

struct Agent {
    attempt: Attempt,
    events: mpsc::Sender<AgentEvent>,
}

impl Agent {
    fn identify(
        &self,
        header: &zbus::message::Header<'_>,
        device: Option<&Path>,
    ) -> Result<u64, AgentError> {
        if header.sender().map(|s| s.as_str()) != Some(self.attempt.owner.as_str())
            || device.is_some_and(|device| device != &self.attempt.device)
        {
            return Err(AgentError::Rejected);
        }
        Ok(self.attempt.serial)
    }

    async fn ask(&self, serial: u64, question: Question) -> Result<String, AgentError> {
        let (reply, result) = oneshot::channel();
        self.events
            .try_send(AgentEvent::Prompt(
                serial,
                Prompt {
                    question,
                    reply: Some(reply),
                    entry: String::new(),
                    invalid: false,
                },
            ))
            .map_err(|_| AgentError::Canceled)?;
        result.await.map_err(|_| AgentError::Canceled)
    }

    fn display(&self, serial: u64, label: String) -> Result<(), AgentError> {
        self.events
            .try_send(AgentEvent::Prompt(
                serial,
                Prompt {
                    question: Question::Display(label),
                    reply: None,
                    entry: String::new(),
                    invalid: false,
                },
            ))
            .map_err(|_| AgentError::Canceled)
    }
}

#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    async fn request_pin_code(
        &self,
        device: Path,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<String, AgentError> {
        self.ask(self.identify(&header, Some(&device))?, Question::Pin)
            .await
    }

    async fn request_passkey(
        &self,
        device: Path,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<u32, AgentError> {
        self.ask(self.identify(&header, Some(&device))?, Question::Passkey)
            .await?
            .parse()
            .map_err(|_| AgentError::Rejected)
    }

    async fn request_confirmation(
        &self,
        device: Path,
        passkey: u32,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), AgentError> {
        self.ask(
            self.identify(&header, Some(&device))?,
            Question::Confirm(passkey),
        )
        .await
        .map(drop)
    }

    async fn request_authorization(
        &self,
        device: Path,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), AgentError> {
        self.ask(self.identify(&header, Some(&device))?, Question::Authorize)
            .await
            .map(drop)
    }

    async fn authorize_service(
        &self,
        device: Path,
        _uuid: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), AgentError> {
        self.ask(self.identify(&header, Some(&device))?, Question::Service)
            .await
            .map(drop)
    }

    fn display_pin_code(
        &self,
        device: Path,
        pincode: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), AgentError> {
        let serial = self.identify(&header, Some(&device))?;
        self.display(
            serial,
            format!("Type {pincode} on the other device and press Enter."),
        )
    }

    fn display_passkey(
        &self,
        device: Path,
        passkey: u32,
        entered: u16,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> Result<(), AgentError> {
        let serial = self.identify(&header, Some(&device))?;
        self.display(
            serial,
            format!(
                "Type {passkey:06} on the other device and press Enter ({} of 6 digits entered).",
                entered.min(6)
            ),
        )
    }

    fn cancel(&self, #[zbus(header)] header: zbus::message::Header<'_>) {
        if let Ok(serial) = self.identify(&header, None) {
            let _ = self.events.try_send(AgentEvent::Dismiss(serial));
        }
    }

    fn release(&self, #[zbus(header)] header: zbus::message::Header<'_>) {
        self.cancel(header);
    }
}

struct Pairing {
    attempt: Attempt,
    label: String,
    task: JoinHandle<Result<()>>,
    stop: oneshot::Sender<()>,
    prompt: Option<Prompt>,
}

impl Pairing {
    fn content(&self) -> Content {
        let mut elements = vec![FormElement::Text {
            text: format!("Pair with {}", self.label),
        }];
        if let Some(prompt) = &self.prompt {
            elements.push(FormElement::Text {
                text: prompt.question.label(),
            });
            if matches!(prompt.question, Question::Pin | Question::Passkey) {
                elements.push(FormElement::TextInput {
                    id: "code".into(),
                    label: "Pairing code".into(),
                    value: String::new(),
                    secret: true,
                    error: prompt.invalid.then(|| "Enter a valid pairing code.".into()),
                });
            }
        } else {
            elements.push(FormElement::Text {
                text: "Waiting for the device…".into(),
            });
        }
        elements.push(FormElement::Button {
            id: "cancel".into(),
            label: "Cancel pairing".into(),
        });
        if self
            .prompt
            .as_ref()
            .is_some_and(|prompt| prompt.reply.is_some())
        {
            elements.push(FormElement::Button {
                id: "confirm".into(),
                label: "Confirm".into(),
            });
        }
        Content::Form(FormContent { elements })
    }
}

async fn pair(
    attempt: Attempt,
    events: mpsc::Sender<AgentEvent>,
    mut stop: oneshot::Receiver<()>,
) -> Result<()> {
    // BlueZ watches the unique sender of Pair and cancels that attempt when
    // it disconnects. CancelPairing can also cancel someone else's attempt.
    let connection = tokio::select! {
        biased;
        _ = &mut stop => return Ok(()),
        result = timeout(QUERY_TIMEOUT, Connection::system()) => result??,
    };
    let result = tokio::select! {
        biased;
        _ = &mut stop => Ok(()),
        result = pair_on_bus(&connection, attempt, events) => result,
    };
    let _ = timeout(QUERY_TIMEOUT, connection.close()).await;
    result
}

async fn pair_on_bus(
    connection: &Connection,
    attempt: Attempt,
    events: mpsc::Sender<AgentEvent>,
) -> Result<()> {
    connection
        .object_server()
        .at(
            AGENT_PATH,
            Agent {
                attempt: attempt.clone(),
                events,
            },
        )
        .await?;
    let manager = proxy(
        connection,
        &attempt.owner,
        "/org/bluez",
        "org.bluez.AgentManager1",
    )
    .await?;
    let device = proxy(connection, &attempt.owner, attempt.device.as_str(), DEVICE).await?;
    let path = Path::try_from(AGENT_PATH)?;
    timeout(
        QUERY_TIMEOUT,
        manager.call::<_, _, ()>("RegisterAgent", &(&path, "KeyboardDisplay")),
    )
    .await??;
    timeout(
        Duration::from_secs(60),
        device.call::<_, _, ()>("Pair", &()),
    )
    .await??;
    timeout(QUERY_TIMEOUT, device.set_property("Trusted", true)).await??;
    if !timeout(QUERY_TIMEOUT, device.get_property::<bool>("Connected")).await?? {
        timeout(
            Duration::from_secs(8),
            device.call::<_, _, ()>("Connect", &()),
        )
        .await??;
    }
    Ok(())
}

struct Bluetooth {
    panel: Session,
    snapshot: Snapshot,
    scanning: HashSet<Path>,
    pairing: Option<Pairing>,
    events: mpsc::Sender<AgentEvent>,
    serial: u64,
    error: Option<&'static str>,
}

impl Bluetooth {
    async fn publish(&mut self, output: &mut (impl AsyncWrite + Unpin)) -> Result<()> {
        if let Some(pairing) = &self.pairing {
            let frame = self.panel.replace(pairing.content(), Update::Replace)?;
            crate::provider_io::write_v5(output, frame).await?;
        } else {
            let content = self.snapshot.content(&self.scanning, self.error);
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
            let proxy = proxy(connection, owner, id, ADAPTER).await?;
            match key {
                "on" | "off" => {
                    proxy.set_property("Powered", key == "on").await?;
                    if key == "off" {
                        self.scanning.remove(&adapter.path);
                    }
                }
                "scan" if adapter.powered && !self.scanning.contains(&adapter.path) => {
                    proxy.call::<_, _, ()>("StartDiscovery", &()).await?;
                    self.scanning.insert(adapter.path.clone());
                }
                "stop" if self.scanning.contains(&adapter.path) => {
                    proxy.call::<_, _, ()>("StopDiscovery", &()).await?;
                    self.scanning.remove(&adapter.path);
                }
                _ => {}
            }
        } else if let Some(device) = snapshot.devices.iter().find(|d| d.path.as_str() == id) {
            ensure!(
                device.paired || device.connected || self.scanning.contains(&device.adapter),
                "Device is not visible."
            );
            let proxy = proxy(connection, owner, id, DEVICE).await?;
            match key {
                "pair" if !device.paired && !device.connected => {
                    self.serial += 1;
                    let attempt = Attempt {
                        serial: self.serial,
                        owner: owner.clone(),
                        device: device.path.clone(),
                    };
                    let (stop, canceled) = oneshot::channel();
                    let task = tokio::spawn(pair(attempt.clone(), self.events.clone(), canceled));
                    self.pairing = Some(Pairing {
                        attempt,
                        label: device.label.clone(),
                        task,
                        stop,
                        prompt: None,
                    });
                }
                "connect" if device.paired && !device.connected => {
                    proxy.call::<_, _, ()>("Connect", &()).await?
                }
                "disconnect" if device.connected => {
                    proxy.call::<_, _, ()>("Disconnect", &()).await?
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
                    self.error = Some("Pairing canceled.");
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
                (!matches!(result, Ok(Ok(())))).then_some("Bluetooth could not apply the change.");
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
                            Some(AgentEvent::Prompt(serial, prompt)) if serial == pairing.attempt.serial => pairing.prompt = Some(prompt),
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
                    bluetooth.error = (!matches!(result, Ok(Ok(())))).then_some("Bluetooth pairing or connection failed.");
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
