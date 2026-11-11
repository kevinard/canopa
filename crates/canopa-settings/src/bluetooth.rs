//! The Bluetooth page's service: BlueZ adapters and devices, the changes the
//! page asks for, and pairing through an application-local agent whose
//! questions the page answers.
//!
//! The page searches for nearby devices while it is shown: following updates
//! starts discovery on each powered adapter, and dropping them stops it.
//! Pairing uses [`canopa_services::bluetooth::pair`], which registers its own
//! agent for one attempt on a private connection and accepts questions only
//! from BlueZ about the selected device.
use crate::live::{LiveAction, LiveService, ServiceError};
use canopa_services::{
    ServiceState,
    bluetooth::{self, AgentEvent, Attempt, Question, Snapshot},
};
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
use zbus::{Connection, names::OwnedUniqueName};

fn no_service() -> String {
    fl!("settings-bluetooth-not-running")
}
fn no_bus() -> String {
    fl!("settings-error-bus")
}
const READ_TIMEOUT: Duration = Duration::from_secs(2);

/// A pairing attempt in progress, as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pairing {
    /// The device's object path.
    pub device: String,
    pub label: String,
    /// What BlueZ asks now; `None` while it works.
    pub question: Option<Question>,
}

/// What the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BluetoothState {
    pub snapshot: Snapshot,
    pub pairing: Option<Pairing>,
}

impl BluetoothState {
    pub fn device(&self, path: &str) -> Option<&bluetooth::Device> {
        self.snapshot
            .devices
            .iter()
            .find(|device| device.path.as_str() == path)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum BluetoothAction {
    /// An adapter path and whether it is on.
    Powered(String, bool),
    /// An adapter path and whether nearby devices can find it.
    Discoverable(String, bool),
    /// Pair, trust and connect a device.
    Pair(String),
    Connect(String),
    Disconnect(String),
    Trusted(String, bool),
    /// Remove the pairing.
    Forget(String),
    /// Answer the open pairing question: a PIN, a passkey, or empty to
    /// confirm or allow.
    Answer(String),
    /// Stop the pairing attempt; this also rejects its open question.
    CancelPairing,
}

impl std::fmt::Debug for BluetoothAction {
    // PINs the person typed never reach logs.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.control())
    }
}

impl LiveAction for BluetoothAction {
    fn control(&self) -> String {
        match self {
            Self::Powered(adapter, _) => format!("power-{adapter}"),
            Self::Discoverable(adapter, _) => format!("visible-{adapter}"),
            Self::Pair(device)
            | Self::Connect(device)
            | Self::Disconnect(device)
            | Self::Forget(device) => format!("device-{device}"),
            Self::Trusted(device, _) => format!("trust-{device}"),
            Self::Answer(_) | Self::CancelPairing => "pairing".into(),
        }
    }
}

fn failure(error: canopa_services::Error) -> ServiceError {
    use canopa_services::Error;
    match error {
        Error::Unavailable => ServiceError::Unavailable(no_service()),
        Error::NotAuthorized => ServiceError::Failed(fl!("settings-error-not-authorized")),
        Error::Changed => ServiceError::Failed(fl!("settings-error-device-changed")),
        Error::TimedOut => ServiceError::Failed(fl!("settings-bluetooth-timed-out")),
        Error::Invalid(message) => ServiceError::Failed(crate::service_text::invalid(message)),
        Error::Refused(message) => ServiceError::Failed(refusal(&message)),
    }
}

/// BlueZ's refusals in the page's words.
fn refusal(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if lower.contains("authentication") {
        fl!("settings-bluetooth-pairing-failed")
    } else if lower.contains("page timeout")
        || lower.contains("connection attempt failed")
        || lower.contains("connectionattemptfailed")
        || lower.contains("host is down")
        || lower.contains("abort-by-local")
    {
        fl!("settings-bluetooth-no-answer")
    } else if lower.contains("alreadyexists") || lower.contains("already exists") {
        fl!("settings-bluetooth-already-paired")
    } else if lower.contains("doesnotexist") || lower.contains("does not exist") {
        fl!("settings-bluetooth-unknown-device")
    } else if lower.contains("inprogress") || lower.contains("in progress") {
        fl!("settings-bluetooth-busy")
    } else {
        fl!("settings-bluetooth-refused", reason = message)
    }
}

/// The open pairing attempt: how to stop it and where its question's reply goes.
struct Attempting {
    serial: u64,
    stop: Option<oneshot::Sender<()>>,
    reply: Option<oneshot::Sender<String>>,
}

/// BlueZ for the page.
#[derive(Clone)]
pub struct BluetoothService {
    system: Arc<tokio::sync::OnceCell<Connection>>,
    /// The bus address for pairing's private connections; `None` is the system bus.
    address: Option<String>,
    attempt: Arc<Mutex<Option<Attempting>>>,
    pairing: watch::Sender<Option<Pairing>>,
    serial: Arc<AtomicU64>,
}

impl Default for BluetoothService {
    fn default() -> Self {
        Self {
            system: Arc::default(),
            address: None,
            attempt: Arc::default(),
            pairing: watch::channel(None).0,
            serial: Arc::default(),
        }
    }
}

impl BluetoothService {
    /// A service on an existing connection; pairing opens private connections
    /// to the bus at `address` (tests).
    pub fn with(system: Connection, address: String) -> Self {
        Self {
            system: Arc::new(tokio::sync::OnceCell::new_with(Some(system))),
            address: Some(address),
            ..Default::default()
        }
    }

    async fn connection(&self) -> Result<Connection, ServiceError> {
        self.system
            .get_or_try_init(canopa_services::bus::system)
            .await
            .cloned()
            .map_err(|_| ServiceError::Unavailable(no_bus()))
    }

    async fn read(&self) -> Result<BluetoothState, ServiceError> {
        let system = self.connection().await?;
        let snapshot = tokio::time::timeout(READ_TIMEOUT, Snapshot::query(&system))
            .await
            .map_err(|_| ServiceError::Unavailable(no_service()))?
            .map_err(|error| match error {
                canopa_services::Error::Refused(_) | canopa_services::Error::Unavailable => {
                    ServiceError::Unavailable(no_service())
                }
                other => failure(other),
            })?;
        Ok(BluetoothState {
            snapshot,
            pairing: self.pairing.borrow().clone(),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Attempting>> {
        self.attempt
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    /// The current snapshot's owner and the device at `path`.
    async fn target(
        &self,
        path: &str,
    ) -> Result<(OwnedUniqueName, bluetooth::Device), ServiceError> {
        let snapshot = self.read().await?.snapshot;
        let owner = snapshot
            .owner
            .clone()
            .ok_or_else(|| ServiceError::Unavailable(no_service()))?;
        let device = snapshot
            .devices
            .into_iter()
            .find(|device| device.path.as_str() == path)
            .ok_or_else(|| ServiceError::Failed(fl!("settings-bluetooth-gone")))?;
        Ok((owner, device))
    }

    async fn owner(&self) -> Result<OwnedUniqueName, ServiceError> {
        self.read()
            .await?
            .snapshot
            .owner
            .ok_or_else(|| ServiceError::Unavailable(no_service()))
    }

    fn relay(&self, serial: u64, event: AgentEvent) {
        let mut attempt = self.lock();
        let Some(current) = attempt.as_mut().filter(|current| current.serial == serial) else {
            return;
        };
        let (reply, question) = match event {
            AgentEvent::Request(event_serial, request) if event_serial == serial => {
                (request.reply, Some(request.question))
            }
            AgentEvent::Dismiss(event_serial) if event_serial == serial => (None, None),
            _ => return,
        };
        // Replacing the sender cancels a question BlueZ no longer waits for.
        current.reply = reply;
        drop(attempt);
        self.pairing.send_modify(|pairing| {
            if let Some(pairing) = pairing {
                pairing.question = question;
            }
        });
    }

    async fn pair(&self, path: &str) -> Result<(), ServiceError> {
        let (owner, device) = self.target(path).await?;
        if self.lock().is_some() {
            return Err(ServiceError::Failed(fl!(
                "settings-bluetooth-other-pairing"
            )));
        }
        let serial = self.serial.fetch_add(1, Ordering::Relaxed) + 1;
        let (events, mut requests) = mpsc::channel(8);
        let (stop, stopped) = oneshot::channel();
        *self.lock() = Some(Attempting {
            serial,
            stop: Some(stop),
            reply: None,
        });
        self.pairing.send_replace(Some(Pairing {
            device: path.to_owned(),
            label: device.label.clone(),
            question: None,
        }));
        let attempt = Attempt {
            serial,
            owner,
            device: device.path.clone(),
        };
        let address = self.address.clone();
        let pairing = bluetooth::pair_with(attempt, events, stopped, move || async move {
            Ok(match address {
                Some(address) => {
                    zbus::connection::Builder::address(address.as_str())?
                        .build()
                        .await?
                }
                None => tokio::time::timeout(READ_TIMEOUT, Connection::system()).await??,
            })
        });
        tokio::pin!(pairing);
        let result = loop {
            tokio::select! {
                result = &mut pairing => break result,
                Some(event) = requests.recv() => self.relay(serial, event),
            }
        };
        {
            let mut attempt = self.lock();
            if attempt
                .as_ref()
                .is_some_and(|current| current.serial == serial)
            {
                *attempt = None;
            }
        }
        self.pairing.send_replace(None);
        result.map_err(failure)
    }

    fn answer(&self, value: &str) -> Result<(), ServiceError> {
        let question = self
            .pairing
            .borrow()
            .as_ref()
            .and_then(|pairing| pairing.question.clone())
            .ok_or_else(|| ServiceError::Failed(fl!("settings-bluetooth-question-gone")))?;
        let reply = question.accepts(value.trim()).ok_or_else(|| {
            ServiceError::Failed(match question {
                Question::Passkey => fl!("settings-bluetooth-passkey-hint"),
                Question::Pin => fl!("settings-bluetooth-pin-rules"),
                _ => fl!("settings-bluetooth-no-answer-needed"),
            })
        })?;
        let sender = self
            .lock()
            .as_mut()
            .and_then(|attempt| attempt.reply.take())
            .ok_or_else(|| ServiceError::Failed(fl!("settings-bluetooth-question-gone")))?;
        sender
            .send(reply)
            .map_err(|_| ServiceError::Failed(fl!("settings-bluetooth-question-gone")))?;
        self.pairing.send_modify(|pairing| {
            if let Some(pairing) = pairing {
                pairing.question = None;
            }
        });
        Ok(())
    }

    fn cancel(&self) {
        if let Some(attempt) = self.lock().as_mut() {
            attempt.reply = None;
            if let Some(stop) = attempt.stop.take() {
                let _ = stop.send(());
            }
        }
    }

    async fn run(&self, action: BluetoothAction) -> Result<(), ServiceError> {
        match action {
            BluetoothAction::Answer(value) => return self.answer(&value),
            BluetoothAction::CancelPairing => {
                self.cancel();
                return Ok(());
            }
            BluetoothAction::Pair(device) => return self.pair(&device).await,
            _ => {}
        }
        let system = self.connection().await?;
        let result = match &action {
            BluetoothAction::Powered(adapter, powered) => {
                let owner = self.owner().await?;
                bluetooth::set_powered(&system, &owner, adapter, *powered).await
            }
            BluetoothAction::Discoverable(adapter, visible) => {
                let owner = self.owner().await?;
                bluetooth::set_discoverable(&system, &owner, adapter, *visible).await
            }
            BluetoothAction::Connect(path) => {
                let (owner, device) = self.target(path).await?;
                tokio::time::timeout(
                    Duration::from_secs(15),
                    bluetooth::connect(&system, &owner, device.path.as_str()),
                )
                .await
                .unwrap_or(Err(canopa_services::Error::TimedOut))
            }
            BluetoothAction::Disconnect(path) => {
                let (owner, device) = self.target(path).await?;
                bluetooth::disconnect(&system, &owner, device.path.as_str()).await
            }
            BluetoothAction::Trusted(path, trusted) => {
                let (owner, device) = self.target(path).await?;
                bluetooth::set_trusted(&system, &owner, device.path.as_str(), *trusted).await
            }
            BluetoothAction::Forget(path) => {
                let (owner, device) = self.target(path).await?;
                bluetooth::remove_device(
                    &system,
                    &owner,
                    device.adapter.as_str(),
                    device.path.as_str(),
                )
                .await
            }
            BluetoothAction::Answer(_)
            | BluetoothAction::CancelPairing
            | BluetoothAction::Pair(_) => unreachable!("handled above"),
        };
        result.map_err(failure)
    }
}

/// Discovery this page started, stopped when the page stops following BlueZ.
struct Discovery {
    system: Connection,
    owner: Option<OwnedUniqueName>,
    adapters: HashSet<String>,
}

impl Discovery {
    /// Search on each powered adapter that this page is not searching on yet.
    async fn follow(&mut self, snapshot: &Snapshot) {
        if snapshot.owner != self.owner {
            // A new BlueZ forgot the old discovery sessions.
            self.adapters.clear();
            self.owner = snapshot.owner.clone();
        }
        let Some(owner) = self.owner.clone() else {
            return;
        };
        self.adapters.retain(|path| {
            snapshot
                .adapters
                .iter()
                .any(|adapter| adapter.powered && adapter.path.as_str() == path)
        });
        for adapter in snapshot.adapters.iter().filter(|adapter| adapter.powered) {
            if self.adapters.insert(adapter.path.to_string()) {
                // Another client may be searching already; that is fine.
                let _ = tokio::time::timeout(
                    READ_TIMEOUT,
                    bluetooth::start_discovery(&self.system, &owner, adapter.path.as_str()),
                )
                .await;
            }
        }
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        let (Some(owner), Ok(runtime)) = (self.owner.take(), tokio::runtime::Handle::try_current())
        else {
            return;
        };
        let system = self.system.clone();
        let adapters = std::mem::take(&mut self.adapters);
        runtime.spawn(async move {
            for adapter in adapters {
                let _ = tokio::time::timeout(
                    READ_TIMEOUT,
                    bluetooth::stop_discovery(&system, &owner, &adapter),
                )
                .await;
            }
        });
    }
}

enum Part {
    Snapshot(ServiceState<Snapshot>),
    Pairing(Option<Pairing>),
}

impl LiveService for BluetoothService {
    type State = BluetoothState;
    type Action = BluetoothAction;

    fn name(&self) -> String {
        "BlueZ".into()
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<BluetoothState, ServiceError>> {
        let service = self.clone();
        async move { service.read().await }.boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<BluetoothState, ServiceError>> {
        let service = self.clone();
        futures_util::stream::once(async move {
            let Ok(system) = service.connection().await else {
                return futures_util::stream::iter([Err(ServiceError::Unavailable(no_bus()))])
                    .boxed();
            };
            let mut pairing = service.pairing.subscribe();
            pairing.mark_unchanged();
            let pairing = futures_util::stream::unfold(pairing, |mut pairing| async move {
                pairing.changed().await.ok()?;
                let value = pairing.borrow_and_update().clone();
                Some((Part::Pairing(value), pairing))
            });
            let snapshots = bluetooth::watch(system.clone()).map(Part::Snapshot);
            let parts = futures_util::stream::select(snapshots.boxed(), pairing.boxed());
            let discovery = Discovery {
                system,
                owner: None,
                adapters: HashSet::new(),
            };
            let current = service.pairing.borrow().clone();
            futures_util::stream::unfold(
                (parts, discovery, None::<Snapshot>, current),
                |(mut parts, mut discovery, mut snapshot, mut pairing)| async move {
                    loop {
                        match parts.next().await? {
                            Part::Snapshot(ServiceState::Available(value)) => {
                                discovery.follow(&value).await;
                                snapshot = Some(value);
                            }
                            Part::Snapshot(ServiceState::Unavailable) => {
                                let item = Err(ServiceError::Unavailable(no_service()));
                                return Some((item, (parts, discovery, None, pairing)));
                            }
                            Part::Pairing(value) => pairing = value,
                        }
                        if let Some(current) = &snapshot {
                            let item = Ok(BluetoothState {
                                snapshot: current.clone(),
                                pairing: pairing.clone(),
                            });
                            return Some((item, (parts, discovery, snapshot, pairing)));
                        }
                    }
                },
            )
            .boxed()
        })
        .flatten()
        .boxed()
    }

    fn apply(
        &self,
        action: BluetoothAction,
    ) -> BoxFuture<'static, Result<Option<BluetoothState>, ServiceError>> {
        let service = self.clone();
        async move {
            service.run(action).await?;
            // Show the result now; BlueZ also reports it.
            Ok(service.read().await.ok())
        }
        .boxed()
    }
}

/// A short description of a device's kind.
pub fn kind_label(kind: bluetooth::DeviceKind) -> String {
    use bluetooth::DeviceKind as Kind;
    match kind {
        Kind::Headphones => fl!("settings-bluetooth-kind-headphones"),
        Kind::Headset => fl!("settings-bluetooth-kind-headset"),
        Kind::Speaker => fl!("settings-bluetooth-kind-audio"),
        Kind::Keyboard => fl!("settings-bluetooth-kind-keyboard"),
        Kind::Mouse => fl!("settings-bluetooth-kind-mouse"),
        Kind::Gamepad => fl!("settings-bluetooth-kind-gamepad"),
        Kind::Tablet => fl!("settings-bluetooth-kind-tablet"),
        Kind::Phone => fl!("settings-bluetooth-kind-phone"),
        Kind::Computer => fl!("settings-bluetooth-kind-computer"),
        Kind::Camera => fl!("settings-bluetooth-kind-camera"),
        Kind::Printer => fl!("settings-bluetooth-kind-printer"),
        Kind::Network => fl!("settings-bluetooth-kind-network"),
        Kind::Other => fl!("settings-bluetooth-kind-other"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_services::testing::{
        TestBus,
        bluez::{ADAPTER_PATH, DEVICE_PATH, FakeBluez},
    };
    use std::sync::atomic::Ordering::SeqCst;

    async fn next(
        updates: &mut BoxStream<'static, Result<BluetoothState, ServiceError>>,
    ) -> Result<BluetoothState, ServiceError> {
        tokio::time::timeout(Duration::from_secs(5), updates.next())
            .await
            .expect("an update")
            .expect("the stream continues")
    }

    #[tokio::test]
    async fn the_page_is_unavailable_without_bluez_and_searches_while_shown() {
        let bus = TestBus::new();
        let service = BluetoothService::with(bus.connect().await, bus.address.clone());
        assert!(matches!(
            service.snapshot().await,
            Err(ServiceError::Unavailable(message)) if message == no_service()
        ));
        let bluez = FakeBluez::serve(&bus, true).await;
        let state = service.snapshot().await.unwrap();
        assert_eq!(state.snapshot.adapters[0].label, "Test adapter");
        assert_eq!(state.device(DEVICE_PATH).unwrap().battery, Some(80));

        let mut updates = service.updates();
        assert!(next(&mut updates).await.is_ok());
        assert!(bluez.state.scanning.load(SeqCst), "searching while shown");
        drop(updates);
        tokio::time::timeout(Duration::from_secs(3), async {
            while bluez.state.scanning.load(SeqCst) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("leaving the page stops searching");

        for action in [
            BluetoothAction::Discoverable(ADAPTER_PATH.into(), true),
            BluetoothAction::Powered(ADAPTER_PATH.into(), false),
        ] {
            service.apply(action).await.unwrap();
        }
        assert!(bluez.state.discoverable.load(SeqCst));
        assert!(!bluez.state.powered.load(SeqCst));
        // Devices of an adapter that is off are hidden.
        assert!(matches!(
            service
                .apply(BluetoothAction::Connect(DEVICE_PATH.into()))
                .await,
            Err(ServiceError::Failed(_))
        ));
        bluez.connection.close().await.unwrap();
        assert!(matches!(
            service
                .apply(BluetoothAction::Powered(ADAPTER_PATH.into(), true))
                .await,
            Err(ServiceError::Unavailable(_))
        ));
    }

    #[tokio::test]
    async fn pairing_relays_the_confirmation_and_devices_can_be_trusted_and_forgotten() {
        let bus = TestBus::new();
        let bluez = FakeBluez::serve(&bus, true).await;
        let service = BluetoothService::with(bus.connect().await, bus.address.clone());
        let mut updates = service.updates();
        assert!(next(&mut updates).await.unwrap().pairing.is_none());

        // An answer with no question open is refused on the control.
        assert!(matches!(
            service.apply(BluetoothAction::Answer(String::new())).await,
            Err(ServiceError::Failed(_))
        ));
        let pairing = tokio::spawn(service.apply(BluetoothAction::Pair(DEVICE_PATH.into())));
        let question = loop {
            let state = next(&mut updates).await.unwrap();
            if let Some(Pairing {
                question: Some(question),
                label,
                ..
            }) = state.pairing
            {
                assert_eq!(label, "Test headphones");
                break question;
            }
        };
        assert_eq!(question, Question::Confirm(42));
        // A second attempt waits for the first.
        assert!(matches!(
            service
                .apply(BluetoothAction::Pair(DEVICE_PATH.into()))
                .await,
            Err(ServiceError::Failed(_))
        ));
        service
            .apply(BluetoothAction::Answer(String::new()))
            .await
            .unwrap();
        let state = pairing.await.unwrap().unwrap().unwrap();
        assert!(state.pairing.is_none());
        let device = state.device(DEVICE_PATH).unwrap();
        assert!(device.paired && device.connected && device.trusted);

        service
            .apply(BluetoothAction::Trusted(DEVICE_PATH.into(), false))
            .await
            .unwrap();
        assert!(!bluez.state.trusted.load(SeqCst));
        service
            .apply(BluetoothAction::Disconnect(DEVICE_PATH.into()))
            .await
            .unwrap();
        assert!(!bluez.state.connected.load(SeqCst));
        service
            .apply(BluetoothAction::Connect(DEVICE_PATH.into()))
            .await
            .unwrap();
        assert!(bluez.state.connected.load(SeqCst));
        let state = service
            .apply(BluetoothAction::Forget(DEVICE_PATH.into()))
            .await
            .unwrap()
            .unwrap();
        assert!(bluez.state.removed.load(SeqCst));
        assert!(state.device(DEVICE_PATH).is_none());
    }

    #[tokio::test]
    async fn cancelling_stops_the_attempt_and_clears_the_question() {
        let bus = TestBus::new();
        let bluez = FakeBluez::serve(&bus, true).await;
        let service = BluetoothService::with(bus.connect().await, bus.address.clone());
        let mut updates = service.updates();
        next(&mut updates).await.unwrap();
        let pairing = tokio::spawn(service.apply(BluetoothAction::Pair(DEVICE_PATH.into())));
        loop {
            if next(&mut updates)
                .await
                .unwrap()
                .pairing
                .is_some_and(|pairing| pairing.question.is_some())
            {
                break;
            }
        }
        service.apply(BluetoothAction::CancelPairing).await.unwrap();
        let state = pairing.await.unwrap().unwrap().unwrap();
        assert!(state.pairing.is_none());
        assert!(!bluez.state.paired.load(SeqCst));
    }

    #[test]
    fn controls_and_refusals_read_well() {
        assert_eq!(
            BluetoothAction::Pair("/d".into()).control(),
            BluetoothAction::Forget("/d".into()).control()
        );
        assert_ne!(
            BluetoothAction::Trusted("/d".into(), true).control(),
            BluetoothAction::Connect("/d".into()).control()
        );
        assert_eq!(
            format!("{:?}", BluetoothAction::Answer("1234".into())),
            "pairing"
        );
        assert!(refusal("org.bluez.Error.AuthenticationFailed").contains("Pairing did not"));
        assert!(refusal("Page Timeout").contains("did not answer"));
        assert_eq!(refusal("busy"), "BlueZ refused: busy");
    }
}
