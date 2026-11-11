//! Exercise the shipped provider command on a private bus, with no host hardware.
#![cfg(feature = "desktop")]

#[path = "../src/test_bus.rs"]
mod test_bus;

use canopa_panel_protocol::{
    InteractionEvent, InteractionTarget,
    v5::{Content, FormElement, FormEvent, HostMessage, Interaction, Response},
};
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering::SeqCst},
    },
    time::Duration,
};
use test_bus::TestBus;
use tokio::{
    io::{AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};
use zbus::{
    Connection, Proxy, fdo,
    zvariant::{OwnedObjectPath, OwnedValue, Str},
};

const ADAPTER: &str = "/org/bluez/hci0";
const DEVICE: &str = "/org/bluez/hci0/dev_00_11_22_33_44_55";
const AGENT: &str = "/org/canopa/BluetoothAgent";
const LIMIT: Duration = Duration::from_secs(10);

#[derive(Default)]
struct State {
    scanning: AtomicBool,
    paired: AtomicBool,
    connected: AtomicBool,
    trusted: AtomicBool,
    agent_owner: std::sync::Mutex<Option<String>>,
    registered: AtomicBool,
}
struct Manager(Arc<State>);
struct Adapter(Arc<State>);
struct Device(Arc<State>);

#[zbus::interface(name = "org.bluez.AgentManager1")]
impl Manager {
    fn register_agent(
        &self,
        agent: OwnedObjectPath,
        capability: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) {
        assert_eq!(agent.as_str(), AGENT);
        assert_eq!(capability, "KeyboardDisplay");
        assert!(!self.0.registered.swap(true, SeqCst));
        *self.0.agent_owner.lock().unwrap() = Some(header.sender().unwrap().to_string());
    }
}

#[zbus::interface(name = "org.bluez.Adapter1")]
impl Adapter {
    fn start_discovery(&self) {
        self.0.scanning.store(true, SeqCst);
    }

    fn stop_discovery(&self) {
        self.0.scanning.store(false, SeqCst);
    }

    #[zbus(property)]
    fn powered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn alias(&self) -> &str {
        "Test adapter"
    }
}

#[zbus::interface(name = "org.bluez.Device1")]
impl Device {
    async fn pair(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> fdo::Result<()> {
        assert!(self.0.registered.load(SeqCst));
        let sender = header.sender().unwrap().to_owned();
        let agent = Proxy::new(connection, sender, AGENT, "org.bluez.Agent1").await?;
        agent
            .call::<_, _, ()>(
                "RequestConfirmation",
                &(OwnedObjectPath::try_from(DEVICE).unwrap(), 42_u32),
            )
            .await?;
        self.0.paired.store(true, SeqCst);
        Ok(())
    }

    fn connect(&self) {
        assert!(self.0.paired.load(SeqCst));
        self.0.connected.store(true, SeqCst);
    }

    fn disconnect(&self) {
        self.0.connected.store(false, SeqCst);
    }

    #[zbus(property)]
    fn adapter(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(ADAPTER).unwrap()
    }

    #[zbus(property)]
    fn alias(&self) -> &str {
        "Test headphones"
    }

    #[zbus(property)]
    fn paired(&self) -> bool {
        self.0.paired.load(SeqCst)
    }

    #[zbus(property)]
    fn connected(&self) -> bool {
        self.0.connected.load(SeqCst)
    }

    #[zbus(property)]
    fn trusted(&self) -> bool {
        self.0.trusted.load(SeqCst)
    }

    #[zbus(property)]
    fn set_trusted(&self, trusted: bool) {
        self.0.trusted.store(trusted, SeqCst);
    }
}

struct Profiles(std::sync::Mutex<String>);

#[zbus::interface(name = "org.freedesktop.UPower.PowerProfiles")]
impl Profiles {
    #[zbus(property)]
    fn active_profile(&self) -> String {
        self.0.lock().unwrap().clone()
    }

    #[zbus(property)]
    fn set_active_profile(&self, profile: String) {
        *self.0.lock().unwrap() = profile;
    }

    #[zbus(property)]
    fn profiles(&self) -> Vec<HashMap<String, OwnedValue>> {
        ["balanced", "power-saver"]
            .into_iter()
            .map(|name| HashMap::from([("Profile".into(), Str::from(name).into())]))
            .collect()
    }
}

struct Provider {
    process: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Provider {
    fn launch(bus: &TestBus, mode: &str) -> Self {
        let mut process = Command::new(env!("CARGO_BIN_EXE_canopa-shell"))
            .args(["--hardware-panel", mode])
            .env("CANOPA_LANGUAGE", "en")
            .env("DBUS_SYSTEM_BUS_ADDRESS", &bus.address)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        Self {
            input: process.stdin.take().unwrap(),
            output: BufReader::new(process.stdout.take().unwrap()),
            process,
        }
    }

    async fn read(&mut self, condition: impl Fn(&Content) -> bool) -> Response {
        timeout(LIMIT, async {
            loop {
                let response = Response::read_async(&mut self.output)
                    .await
                    .unwrap()
                    .expect("provider stopped");
                if condition(&response.content) {
                    return response;
                }
            }
        })
        .await
        .expect("provider did not publish the expected state")
    }

    async fn send(&mut self, message: HostMessage) {
        self.input
            .write_all(&message.encode().unwrap())
            .await
            .unwrap();
    }

    async fn rich(&mut self, revision: u64, id: &str, action: &str) {
        self.send(HostMessage::event(
            revision,
            Interaction::Rich(InteractionEvent::Invoke {
                target: InteractionTarget::Item { id: id.into() },
                action_key: action.into(),
                value: None,
            }),
        ))
        .await;
    }

    async fn close(mut self) {
        self.send(HostMessage::close()).await;
        self.input.shutdown().await.unwrap();
        drop(self.input);
        assert!(
            timeout(LIMIT, self.process.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
    }
}

fn has_action(content: &Content, id: &str, key: &str) -> bool {
    matches!(content, Content::Rich(content) if content.sections.iter().flat_map(|s| &s.items)
        .any(|item| item.id == id && item.actions.iter().any(|action| action.key == key)))
}

fn confirms(content: &Content) -> bool {
    matches!(content, Content::Form(form) if form.elements.iter().any(
        |element| matches!(element, FormElement::Text { text } if text.contains("000042"))))
}

#[tokio::test]
async fn bluetooth_pairs_only_after_confirm_and_reconnects() {
    let bus = TestBus::new();
    let state = Arc::new(State::default());
    let _service = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name("org.bluez")
        .unwrap()
        .serve_at("/", fdo::ObjectManager)
        .unwrap()
        .serve_at("/org/bluez", Manager(state.clone()))
        .unwrap()
        .serve_at(ADAPTER, Adapter(state.clone()))
        .unwrap()
        .serve_at(DEVICE, Device(state.clone()))
        .unwrap()
        .build()
        .await
        .unwrap();
    let mut provider = Provider::launch(&bus, "bluetooth");
    let radio = provider
        .read(|content| has_action(content, ADAPTER, "scan"))
        .await;
    assert!(!has_action(&radio.content, DEVICE, "pair"));
    provider.rich(radio.revision, ADAPTER, "scan").await;
    let discovered = provider
        .read(|content| has_action(content, DEVICE, "pair"))
        .await;
    provider.rich(discovered.revision, DEVICE, "pair").await;
    let question = provider.read(confirms).await;
    assert!(!state.paired.load(SeqCst));
    let confirm = Interaction::Form(FormEvent::ButtonActivated {
        element_id: "confirm".into(),
    });
    provider
        .send(HostMessage::event(discovered.revision, confirm.clone()))
        .await;
    assert!(!state.paired.load(SeqCst));
    provider
        .send(HostMessage::event(question.revision, confirm))
        .await;
    let connected = provider
        .read(|content| has_action(content, DEVICE, "disconnect"))
        .await;
    assert!(state.trusted.load(SeqCst));
    provider
        .rich(connected.revision, DEVICE, "disconnect")
        .await;
    let disconnected = provider
        .read(|content| has_action(content, DEVICE, "connect"))
        .await;
    provider
        .rich(disconnected.revision, DEVICE, "connect")
        .await;
    provider
        .read(|content| has_action(content, DEVICE, "disconnect"))
        .await;
    assert!(state.connected.load(SeqCst));
    provider.close().await;
}

#[tokio::test]
async fn closing_bluetooth_rejects_prompt_and_cancels_attempt() {
    let bus = TestBus::new();
    let state = Arc::new(State::default());
    let _service = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name("org.bluez")
        .unwrap()
        .serve_at("/", fdo::ObjectManager)
        .unwrap()
        .serve_at("/org/bluez", Manager(state.clone()))
        .unwrap()
        .serve_at(ADAPTER, Adapter(state.clone()))
        .unwrap()
        .serve_at(DEVICE, Device(state.clone()))
        .unwrap()
        .build()
        .await
        .unwrap();
    let mut provider = Provider::launch(&bus, "bluetooth");
    let frame = provider
        .read(|content| has_action(content, ADAPTER, "scan"))
        .await;
    provider.rich(frame.revision, ADAPTER, "scan").await;
    let frame = provider
        .read(|content| has_action(content, DEVICE, "pair"))
        .await;
    provider.rich(frame.revision, DEVICE, "pair").await;
    provider.read(confirms).await;
    provider.close().await;
    let owner = state.agent_owner.lock().unwrap().clone().unwrap();
    let observer = bus.connect().await;
    let dbus = fdo::DBusProxy::new(&observer).await.unwrap();
    assert!(
        !dbus
            .name_has_owner(owner.as_str().try_into().unwrap())
            .await
            .unwrap()
    );
    assert!(!state.paired.load(SeqCst));
}

#[tokio::test]
async fn power_lists_available_profiles_and_confirms_service_change() {
    let bus = TestBus::new();
    let service = bus.connect().await;
    service
        .object_server()
        .at(
            "/org/freedesktop/UPower/PowerProfiles",
            Profiles(std::sync::Mutex::new("balanced".into())),
        )
        .await
        .unwrap();
    service
        .request_name("org.freedesktop.UPower.PowerProfiles")
        .await
        .unwrap();
    let mut provider = Provider::launch(&bus, "power");
    let state = provider
        .read(|content| has_action(content, "profiles", "power-saver"))
        .await;
    assert!(!has_action(&state.content, "profiles", "performance"));
    provider
        .rich(state.revision, "profiles", "power-saver")
        .await;
    provider
        .read(|content| has_action(content, "profiles", "balanced"))
        .await;
    provider.close().await;
}
