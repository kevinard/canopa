//! A fake iwd with an object manager, one station `wlan0` at [`DEVICE`] and
//! these networks: "Home" (saved, connected), "Cafe" (open), "Neighbour"
//! (personal) and "eduroam" (enterprise) in range, and "Hotel" saved but out of
//! range. A passphrase for a new network is asked from the client's agent and
//! recorded in [`FakeIwd::calls`], like every method the fake serves.
//! [`FakeNetworkd`] answers systemd-networkd's `Describe` for the same tests.
use super::TestBus;
use std::sync::{Arc, Mutex};
use zbus::{
    fdo,
    object_server::{ObjectServer, SignalEmitter},
    zvariant::OwnedObjectPath,
};

pub const DEVICE: &str = "/net/connman/iwd/0/4";

/// A network's object path under the station, as iwd names it.
pub fn network_path(name: &str, kind: &str) -> String {
    format!("{DEVICE}/{}_{kind}", hex(name))
}

/// A saved network's object path.
pub fn known_path(name: &str, kind: &str) -> String {
    format!("/net/connman/iwd/{}_{kind}", hex(name))
}

fn hex(name: &str) -> String {
    name.bytes().map(|byte| format!("{byte:02x}")).collect()
}

fn path(value: &str) -> OwnedObjectPath {
    value.try_into().unwrap()
}

#[derive(Default)]
pub struct State {
    pub powered: bool,
    /// The connected network's name.
    pub connected: Option<String>,
    /// Saved networks: name, type, auto-connect.
    pub known: Vec<(String, String, bool)>,
    /// The registered agent: its owner and path.
    pub agent: Option<(String, OwnedObjectPath)>,
    pub calls: Vec<String>,
}

type Shared = Arc<Mutex<State>>;

struct Agents(Shared);
#[zbus::interface(name = "net.connman.iwd.AgentManager")]
impl Agents {
    fn register_agent(
        &self,
        agent: OwnedObjectPath,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) {
        self.0.lock().unwrap().agent = Some((header.sender().unwrap().to_string(), agent));
    }
    fn unregister_agent(&self, agent: OwnedObjectPath) {
        let mut state = self.0.lock().unwrap();
        if state
            .agent
            .as_ref()
            .is_some_and(|(_, current)| *current == agent)
        {
            state.agent = None;
        }
    }
}

struct Device(Shared);
#[zbus::interface(name = "net.connman.iwd.Device")]
impl Device {
    #[zbus(property)]
    fn name(&self) -> &str {
        "wlan0"
    }
    #[zbus(property)]
    fn powered(&self) -> bool {
        self.0.lock().unwrap().powered
    }
    #[zbus(property)]
    fn set_powered(&self, powered: bool) {
        let mut state = self.0.lock().unwrap();
        state.powered = powered;
        state.calls.push(format!("Powered {powered}"));
    }
}

/// Ask the client's agent for `network`'s passphrase.
async fn ask(conn: &zbus::Connection, state: &Shared, network: &str) -> fdo::Result<String> {
    let (owner, agent) = state
        .lock()
        .unwrap()
        .agent
        .clone()
        .ok_or_else(|| fdo::Error::Failed("no agent".into()))?;
    let agent = zbus::Proxy::new(conn, owner, agent, "net.connman.iwd.Agent").await?;
    Ok(agent.call("RequestPassphrase", &(path(network),)).await?)
}

/// Save and connect `name`, as iwd does once a connection succeeds.
async fn joined(server: &ObjectServer, state: &Shared, name: &str, kind: &str) -> fdo::Result<()> {
    let new = {
        let mut state = state.lock().unwrap();
        state.connected = Some(name.into());
        let new = !state.known.iter().any(|(known, ..)| known == name);
        if new {
            state.known.push((name.into(), kind.into(), true));
        }
        new
    };
    if new {
        server
            .at(
                known_path(name, kind),
                Known {
                    name: name.into(),
                    kind: kind.into(),
                    state: state.clone(),
                },
            )
            .await?;
    }
    Ok(())
}

struct Station(Shared);
#[zbus::interface(name = "net.connman.iwd.Station")]
impl Station {
    #[zbus(property)]
    fn state(&self) -> &str {
        if self.0.lock().unwrap().connected.is_some() {
            "connected"
        } else {
            "disconnected"
        }
    }
    #[zbus(property)]
    fn scanning(&self) -> bool {
        false
    }
    fn get_ordered_networks(&self) -> Vec<(OwnedObjectPath, i16)> {
        NETWORKS
            .iter()
            .zip([-5000_i16, -6000, -7000, -8000])
            .map(|((name, kind), signal)| (path(&network_path(name, kind)), signal))
            .collect()
    }
    fn scan(&self) {
        self.0.lock().unwrap().calls.push("Scan".into());
    }
    async fn disconnect(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        {
            let mut state = self.0.lock().unwrap();
            state.connected = None;
            state.calls.push("Disconnect".into());
        }
        self.state_changed(&emitter).await?;
        Ok(())
    }
    async fn connect_hidden_network(
        &self,
        name: String,
        #[zbus(connection)] conn: &zbus::Connection,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> fdo::Result<()> {
        // iwd shows the hidden network while it asks for its passphrase.
        let network = network_path(&name, "psk");
        server
            .at(
                network.as_str(),
                Network {
                    name: name.clone(),
                    kind: "psk".into(),
                    state: self.0.clone(),
                },
            )
            .await?;
        let secret = ask(conn, &self.0, &network).await?;
        self.0
            .lock()
            .unwrap()
            .calls
            .push(format!("ConnectHidden {name} {secret}"));
        joined(server, &self.0, &name, "psk").await
    }
}

/// The networks in range, strongest first.
const NETWORKS: [(&str, &str); 4] = [
    ("Home", "psk"),
    ("Cafe", "open"),
    ("Neighbour", "psk"),
    ("eduroam", "8021x"),
];

struct Network {
    name: String,
    kind: String,
    state: Shared,
}
#[zbus::interface(name = "net.connman.iwd.Network")]
impl Network {
    #[zbus(property)]
    fn name(&self) -> &str {
        &self.name
    }
    #[zbus(property, name = "Type")]
    fn network_type(&self) -> &str {
        &self.kind
    }
    #[zbus(property)]
    fn device(&self) -> OwnedObjectPath {
        path(DEVICE)
    }
    #[zbus(property)]
    fn connected(&self) -> bool {
        self.state.lock().unwrap().connected.as_deref() == Some(self.name.as_str())
    }
    #[zbus(property)]
    fn known_network(&self) -> OwnedObjectPath {
        let state = self.state.lock().unwrap();
        if state.known.iter().any(|(name, ..)| *name == self.name) {
            path(&known_path(&self.name, &self.kind))
        } else {
            path("/")
        }
    }
    async fn connect(
        &self,
        #[zbus(connection)] conn: &zbus::Connection,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> fdo::Result<()> {
        let known = self
            .state
            .lock()
            .unwrap()
            .known
            .iter()
            .any(|(name, ..)| *name == self.name);
        let call = if known || self.kind == "open" {
            format!("Connect {}", self.name)
        } else if self.kind == "psk" {
            let secret = ask(conn, &self.state, &network_path(&self.name, &self.kind)).await?;
            format!("Connect {} {secret}", self.name)
        } else {
            return Err(fdo::Error::Failed("not configured".into()));
        };
        self.state.lock().unwrap().calls.push(call);
        joined(server, &self.state, &self.name, &self.kind).await
    }
}

struct Known {
    name: String,
    kind: String,
    state: Shared,
}
#[zbus::interface(name = "net.connman.iwd.KnownNetwork")]
impl Known {
    #[zbus(property)]
    fn name(&self) -> &str {
        &self.name
    }
    #[zbus(property, name = "Type")]
    fn network_type(&self) -> &str {
        &self.kind
    }
    #[zbus(property)]
    fn hidden(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn auto_connect(&self) -> bool {
        let state = self.state.lock().unwrap();
        state
            .known
            .iter()
            .any(|(name, _, auto)| *name == self.name && *auto)
    }
    #[zbus(property)]
    fn set_auto_connect(&self, auto: bool) {
        let mut state = self.state.lock().unwrap();
        for known in state
            .known
            .iter_mut()
            .filter(|(name, ..)| *name == self.name)
        {
            known.2 = auto;
        }
        state
            .calls
            .push(format!("AutoConnect {} {auto}", self.name));
    }
    async fn forget(&self, #[zbus(object_server)] server: &ObjectServer) -> fdo::Result<()> {
        {
            let mut state = self.state.lock().unwrap();
            state.known.retain(|(name, ..)| *name != self.name);
            if state.connected.as_deref() == Some(self.name.as_str()) {
                state.connected = None;
            }
            state.calls.push(format!("Forget {}", self.name));
        }
        server
            .remove::<Self, _>(known_path(&self.name, &self.kind))
            .await?;
        Ok(())
    }
}

/// The fake. Close [`FakeIwd::connection`] to simulate iwd leaving.
pub struct FakeIwd {
    pub connection: zbus::Connection,
    pub state: Shared,
}

impl FakeIwd {
    pub async fn serve(bus: &TestBus) -> Self {
        let state: Shared = Arc::new(Mutex::new(State {
            powered: true,
            connected: Some("Home".into()),
            known: vec![
                ("Home".into(), "psk".into(), true),
                ("Hotel".into(), "psk".into(), false),
            ],
            ..Default::default()
        }));
        let mut builder = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at("/", fdo::ObjectManager)
            .unwrap()
            .serve_at("/net/connman/iwd", Agents(state.clone()))
            .unwrap()
            .serve_at(DEVICE, Device(state.clone()))
            .unwrap()
            .serve_at(DEVICE, Station(state.clone()))
            .unwrap();
        for (name, kind) in NETWORKS {
            builder = builder
                .serve_at(
                    network_path(name, kind),
                    Network {
                        name: name.into(),
                        kind: kind.into(),
                        state: state.clone(),
                    },
                )
                .unwrap();
        }
        for (name, kind) in [("Home", "psk"), ("Hotel", "psk")] {
            builder = builder
                .serve_at(
                    known_path(name, kind),
                    Known {
                        name: name.into(),
                        kind: kind.into(),
                        state: state.clone(),
                    },
                )
                .unwrap();
        }
        let connection = builder
            .name(crate::network::iwd::SERVICE)
            .unwrap()
            .build()
            .await
            .unwrap();
        Self { connection, state }
    }

    pub fn calls(&self) -> Vec<String> {
        self.state.lock().unwrap().calls.clone()
    }
}

/// systemd-networkd answering `Describe` with `description`.
pub struct FakeNetworkd {
    pub connection: zbus::Connection,
}

struct Networkd(String);
#[zbus::interface(name = "org.freedesktop.network1.Manager")]
impl Networkd {
    fn describe(&self) -> String {
        self.0.clone()
    }
}

/// One wired link, `enp5s0`, with an address, a gateway and a DNS server.
pub const WIRED: &str = r#"{"Interfaces":[
  {"Index":1,"Name":"lo","Type":"loopback","OperationalState":"carrier"},
  {"Index":2,"Name":"enp5s0","Type":"ether","OperationalState":"routable",
   "NetworkFile":"/etc/systemd/network/20-wired.network",
   "Addresses":[{"Family":2,"Address":[192,168,1,24],"PrefixLength":24}],
   "Routes":[{"Family":2,"Destination":[0,0,0,0],"DestinationPrefixLength":0,"Gateway":[192,168,1,1]}],
   "DNS":[{"Family":2,"Address":[192,168,1,1]}]},
  {"Index":3,"Name":"wlan0","Type":"wlan","OperationalState":"routable"}
]}"#;

impl FakeNetworkd {
    pub async fn serve(bus: &TestBus, description: &str) -> Self {
        let connection = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at("/org/freedesktop/network1", Networkd(description.into()))
            .unwrap()
            .name(crate::network::networkd::SERVICE)
            .unwrap()
            .build()
            .await
            .unwrap();
        Self { connection }
    }
}
