use super::*;
use crate::test_bus::TestBus;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI64, Ordering},
};

fn path(value: &str) -> OwnedObjectPath {
    value.try_into().unwrap()
}
const DEVICE: &str = "/devices/wifi";
const AP: &str = "/accesspoints/home";
const PROFILE: &str = "/settings/home";
const ACTIVE: &str = "/connections/home";

struct Manager {
    runtime: tokio::runtime::Handle,
    delay_reply: bool,
    created: Arc<AtomicBool>,
    deactivated: Arc<AtomicBool>,
    selected_ap: Arc<Mutex<Option<OwnedObjectPath>>>,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl Manager {
    fn deactivate_connection(&self, _active: OwnedObjectPath) {
        self.deactivated.store(true, Ordering::SeqCst);
    }
    fn get_devices(&self) -> Vec<OwnedObjectPath> {
        vec![path(DEVICE)]
    }
    fn activate_connection(
        &self,
        connection: OwnedObjectPath,
        device: OwnedObjectPath,
        specific_object: OwnedObjectPath,
    ) -> OwnedObjectPath {
        assert_eq!(connection, path(PROFILE));
        assert_eq!(device, path(DEVICE));
        *self.selected_ap.lock().unwrap() = Some(specific_object);
        path(ACTIVE)
    }
    async fn add_and_activate_connection2(
        &self,
        connection: Settings,
        device: OwnedObjectPath,
        specific_object: OwnedObjectPath,
        options: HashMap<String, OwnedValue>,
    ) -> (
        OwnedObjectPath,
        OwnedObjectPath,
        HashMap<String, OwnedValue>,
    ) {
        assert_eq!(device, path(DEVICE));
        *self.selected_ap.lock().unwrap() = Some(specific_object);
        assert_eq!(<&str>::try_from(&options["persist"]).unwrap(), "memory");
        assert_eq!(
            <&str>::try_from(&connection["802-11-wireless-security"]["psk"]).unwrap(),
            "test-passphrase"
        );
        self.created.store(true, Ordering::SeqCst);
        if self.delay_reply {
            self.runtime
                .spawn(async {
                    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                })
                .await
                .unwrap();
        }
        (path(PROFILE), path(ACTIVE), HashMap::new())
    }
}
struct Device;
#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl Device {
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        2
    }
    #[zbus(property)]
    fn managed(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        100
    }
    #[zbus(property)]
    fn available_connections(&self) -> Vec<OwnedObjectPath> {
        vec![path(PROFILE)]
    }
}
struct Wireless {
    aps: Arc<Mutex<Vec<OwnedObjectPath>>>,
    last: Arc<AtomicI64>,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl Wireless {
    async fn request_scan(
        &self,
        _options: HashMap<String, OwnedValue>,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.last.fetch_add(1, Ordering::SeqCst);
        self.last_scan_changed(&emitter).await?;
        Ok(())
    }
    #[zbus(property)]
    fn last_scan(&self) -> i64 {
        self.last.load(Ordering::SeqCst)
    }
    #[zbus(property)]
    fn access_points(&self) -> Vec<OwnedObjectPath> {
        self.aps.lock().unwrap().clone()
    }
    #[zbus(property)]
    fn active_access_point(&self) -> OwnedObjectPath {
        path(AP)
    }
}
struct AccessPoint {
    ssid: Vec<u8>,
    rsn: u32,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl AccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        self.ssid.clone()
    }
    #[zbus(property)]
    fn strength(&self) -> u8 {
        75
    }
    #[zbus(property)]
    fn flags(&self) -> u32 {
        1
    }
    #[zbus(property)]
    fn wpa_flags(&self) -> u32 {
        0
    }
    #[zbus(property)]
    fn rsn_flags(&self) -> u32 {
        self.rsn
    }
}
struct Profile {
    deleted: Arc<AtomicBool>,
    saved: Arc<AtomicBool>,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings.Connection")]
impl Profile {
    fn get_settings(&self) -> Settings {
        nm_settings(
            &request(None),
            b"Home".to_vec(),
            WifiSecurity::Personal,
            "wpa-psk",
        )
    }
    fn save(&self) {
        self.saved.store(true, Ordering::SeqCst);
    }
    fn delete(&self) {
        self.deleted.store(true, Ordering::SeqCst);
    }
}
struct Active {
    state: u32,
    observed: Arc<AtomicBool>,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl Active {
    #[zbus(property)]
    fn state(&self) -> u32 {
        self.observed.store(true, Ordering::SeqCst);
        self.state
    }
}
fn request(secret: Option<&str>) -> WifiConnectRequest {
    WifiConnectRequest {
        device_id: "fixture-device".into(),
        network_id: Some(AP.into()),
        label: "Home".into(),
        credential: secret.map(String::from),
        hidden: false,
    }
}

pub(super) struct NmFixture {
    pub(super) _bus: Option<TestBus>,
    pub(super) _service: zbus::Connection,
    pub(super) backend: NetworkManagerBackend,
    pub(super) created: Arc<AtomicBool>,
    pub(super) saved: Arc<AtomicBool>,
    pub(super) deleted: Arc<AtomicBool>,
    pub(super) deactivated: Arc<AtomicBool>,
    pub(super) observed: Arc<AtomicBool>,
    pub(super) last: Arc<AtomicI64>,
    pub(super) aps: Arc<Mutex<Vec<OwnedObjectPath>>>,
    pub(super) selected_ap: Arc<Mutex<Option<OwnedObjectPath>>>,
}
impl NmFixture {
    async fn request(&self, secret: Option<&str>) -> WifiConnectRequest {
        selected_request(self.backend.snapshot().await.unwrap(), secret)
    }

    pub(super) async fn new(state: u32, delay_reply: bool) -> Self {
        let bus = TestBus::new();
        let mut fixture = Self::on_bus(&bus, state, delay_reply).await;
        fixture._bus = Some(bus);
        fixture
    }
    pub(super) async fn on_bus(bus: &TestBus, state: u32, delay_reply: bool) -> Self {
        let service = bus.connect().await;
        let created = Arc::new(AtomicBool::new(false));
        let saved = Arc::new(AtomicBool::new(false));
        let deleted = Arc::new(AtomicBool::new(false));
        let deactivated = Arc::new(AtomicBool::new(false));
        let observed = Arc::new(AtomicBool::new(false));
        let last = Arc::new(AtomicI64::new(0));
        let aps = Arc::new(Mutex::new(vec![path(AP)]));
        let selected_ap = Arc::new(Mutex::new(None));
        service
            .object_server()
            .at(
                "/org/freedesktop/NetworkManager",
                Manager {
                    runtime: tokio::runtime::Handle::current(),
                    delay_reply,
                    created: created.clone(),
                    deactivated: deactivated.clone(),
                    selected_ap: selected_ap.clone(),
                },
            )
            .await
            .unwrap();
        service.object_server().at(DEVICE, Device).await.unwrap();
        service
            .object_server()
            .at(
                DEVICE,
                Wireless {
                    last: last.clone(),
                    aps: aps.clone(),
                },
            )
            .await
            .unwrap();
        service
            .object_server()
            .at(
                AP,
                AccessPoint {
                    ssid: b"Home".to_vec(),
                    rsn: 0x100,
                },
            )
            .await
            .unwrap();
        service
            .object_server()
            .at(
                PROFILE,
                Profile {
                    saved: saved.clone(),
                    deleted: deleted.clone(),
                },
            )
            .await
            .unwrap();
        service
            .object_server()
            .at(
                ACTIVE,
                Active {
                    state,
                    observed: observed.clone(),
                },
            )
            .await
            .unwrap();
        service
            .request_name("org.freedesktop.NetworkManager")
            .await
            .unwrap();
        let backend = NetworkManagerBackend::from_connection(bus.connect().await)
            .await
            .unwrap();
        Self {
            _bus: None,
            _service: service,
            backend,
            created,
            saved,
            deleted,
            deactivated,
            observed,
            last,
            aps,
            selected_ap,
        }
    }
}
#[tokio::test]
async fn networkmanager_adapter_uses_real_wire_contract_for_scan_and_connections() {
    let f = NmFixture::new(2, false).await;
    let snapshot = f.backend.scan().await.unwrap();
    assert_eq!(f.last.load(Ordering::SeqCst), 1);
    assert!(snapshot.networks[0].active && snapshot.networks[0].known);
    assert_eq!(snapshot.networks[0].security, WifiSecurity::Personal);
    f.backend.connect(f.request(None).await).await.unwrap();
    assert!(!f.created.load(Ordering::SeqCst));
    f.backend
        .connect(f.request(Some("test-passphrase")).await)
        .await
        .unwrap();
    f.backend.cleanup.finish().await;
    assert!(f.created.load(Ordering::SeqCst) && f.saved.load(Ordering::SeqCst));
    assert!(!f.deleted.load(Ordering::SeqCst));
}
#[tokio::test]
async fn canceled_activation_deletes_temporary_profile_and_stops_attempt() {
    let f = NmFixture::new(1, false).await;
    let mut work = Box::pin(f.backend.connect(f.request(Some("test-passphrase")).await));
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        tokio::select! {
            result = &mut work => panic!("activation must still be pending: {result:?}"),
            _ = async { while !f.observed.load(Ordering::SeqCst) { tokio::time::sleep(std::time::Duration::from_millis(10)).await; } } => {}
        }
    }).await.unwrap();
    drop(work);
    f.backend.cleanup.finish().await;
    assert!(f.deleted.load(Ordering::SeqCst) && f.deactivated.load(Ordering::SeqCst));
    assert!(!f.saved.load(Ordering::SeqCst));
}

#[tokio::test]
async fn cancellation_before_creation_reply_still_retires_daemon_resources() {
    let f = NmFixture::new(1, true).await;
    let mut work = Box::pin(f.backend.connect(f.request(Some("test-passphrase")).await));
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        tokio::select! {
            result = &mut work => panic!("must still be waiting for creation: {result:?}"),
            _ = async { while !f.created.load(Ordering::SeqCst) { tokio::time::sleep(std::time::Duration::from_millis(10)).await; } } => {}
        }
    }).await.unwrap();
    drop(work);
    f.backend.cleanup.finish().await;
    assert!(f.deleted.load(Ordering::SeqCst) && f.deactivated.load(Ordering::SeqCst));
}

#[test]
fn radio_security_signal_and_secrets_are_not_misrepresented() {
    assert_eq!(normalize_iwd_signal(-5500), 45);
    assert_eq!(normalize_iwd_signal(i16::MIN), 0);
    assert_eq!(nm_security(0x200, 1), WifiSecurity::Enterprise);
    assert_eq!(nm_security(0, 1), WifiSecurity::Enterprise);
    assert_eq!(nm_security(0, 0), WifiSecurity::Open);
    assert!(!format!("{:?}", request(Some("test-passphrase"))).contains("test-passphrase"));
}

type AgentAddress = Arc<std::sync::Mutex<Option<(String, OwnedObjectPath)>>>;
struct IwdManager;
#[zbus::interface(name = "org.freedesktop.DBus.ObjectManager")]
impl IwdManager {
    fn get_managed_objects(&self) -> ManagedObjects {
        HashMap::from([(
            path(DEVICE),
            HashMap::from([("net.connman.iwd.Station".into(), HashMap::new())]),
        )])
    }
}
struct IwdAgents(AgentAddress);
#[zbus::interface(name = "net.connman.iwd.AgentManager")]
impl IwdAgents {
    fn register_agent(
        &self,
        path: OwnedObjectPath,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) {
        *self.0.lock().unwrap() = Some((header.sender().unwrap().to_string(), path));
    }
    fn unregister_agent(&self, path: OwnedObjectPath) {
        let mut address = self.0.lock().unwrap();
        if address
            .as_ref()
            .is_some_and(|(_, current)| *current == path)
        {
            *address = None;
        }
    }
}
async fn ask_secret(conn: &zbus::Connection, address: &AgentAddress) -> zbus::fdo::Result<()> {
    let (owner, agent) = address
        .lock()
        .unwrap()
        .clone()
        .expect("agent registered before connect");
    let proxy = zbus::Proxy::new(conn, owner, agent, "net.connman.iwd.Agent").await?;
    let secret: String = proxy.call("RequestPassphrase", &(path(AP),)).await?;
    assert_eq!(secret, "test-passphrase");
    Ok(())
}
struct IwdStation {
    scanning: AtomicBool,
    address: AgentAddress,
}
#[zbus::interface(name = "net.connman.iwd.Station")]
impl IwdStation {
    #[zbus(property)]
    fn state(&self) -> &str {
        "connecting"
    }
    #[zbus(property)]
    fn connected_network(&self) -> OwnedObjectPath {
        path(AP)
    }
    #[zbus(property)]
    fn scanning(&self) -> bool {
        self.scanning.load(Ordering::SeqCst)
    }
    fn get_ordered_networks(&self) -> Vec<(OwnedObjectPath, i16)> {
        vec![(path(AP), -5500)]
    }
    async fn scan(
        &self,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.scanning.store(true, Ordering::SeqCst);
        self.scanning_changed(&emitter).await?;
        self.scanning.store(false, Ordering::SeqCst);
        self.scanning_changed(&emitter).await?;
        Ok(())
    }
    async fn connect_hidden_network(
        &self,
        ssid: &str,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        assert_eq!(ssid, "Home");
        ask_secret(conn, &self.address).await
    }
}
struct IwdNetwork {
    name: String,
    kind: String,
    device: OwnedObjectPath,
    address: AgentAddress,
}
#[zbus::interface(name = "net.connman.iwd.Network")]
impl IwdNetwork {
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
        self.device.clone()
    }
    #[zbus(property)]
    fn connected(&self) -> bool {
        true
    }
    async fn connect(&self, #[zbus(connection)] conn: &zbus::Connection) -> zbus::fdo::Result<()> {
        ask_secret(conn, &self.address).await
    }
}

#[tokio::test]
async fn iwd_scan_credentials_and_hidden_connections_use_daemon_agent() {
    let fixture = IwdFixture::new().await;
    let backend = &fixture.backend;
    let snapshot = backend.scan().await.unwrap();
    assert_eq!(snapshot.networks[0].signal_strength, 45);
    assert!(
        !snapshot.networks[0].active,
        "connecting must not be reported as connected"
    );
    let selected = selected_request(snapshot, Some("test-passphrase"));
    backend.connect(selected.clone()).await.unwrap();
    let mut hidden = selected;
    hidden.hidden = true;
    hidden.network_id = None;
    backend.connect(hidden).await.unwrap();
}

pub(super) struct IwdFixture {
    pub(super) bus: TestBus,
    pub(super) service: zbus::Connection,
    pub(super) address: AgentAddress,
    pub(super) backend: IwdBackend,
}
impl IwdFixture {
    pub(super) async fn new() -> Self {
        let bus = TestBus::new();
        let service = bus.connect().await;
        let address: AgentAddress = Arc::new(std::sync::Mutex::new(None));
        service.object_server().at("/", IwdManager).await.unwrap();
        service
            .object_server()
            .at("/net/connman/iwd", IwdAgents(address.clone()))
            .await
            .unwrap();
        service
            .object_server()
            .at(
                DEVICE,
                IwdStation {
                    scanning: AtomicBool::new(false),
                    address: address.clone(),
                },
            )
            .await
            .unwrap();
        service
            .object_server()
            .at(
                AP,
                IwdNetwork {
                    address: address.clone(),
                    name: "Home".into(),
                    kind: "psk".into(),
                    device: path(DEVICE),
                },
            )
            .await
            .unwrap();
        service.request_name("net.connman.iwd").await.unwrap();
        let backend = IwdBackend::from_connection(bus.connect().await)
            .await
            .unwrap();
        Self {
            bus,
            service,
            address,
            backend,
        }
    }
}

#[tokio::test]
async fn selected_network_path_reuse_does_not_receive_credentials() {
    for changed_security in [false, true] {
        let fixture = NmFixture::new(2, false).await;
        let request = fixture.request(Some("test-passphrase")).await;
        let interface = fixture
            ._service
            .object_server()
            .interface::<_, AccessPoint>(AP)
            .await
            .unwrap();
        {
            let mut ap = interface.get_mut().await;
            if changed_security {
                ap.rsn = 0x400;
            } else {
                ap.ssid = b"Other network".to_vec();
            }
        }
        let result = fixture.backend.connect(request).await;
        assert!(
            result.is_err(),
            "an old selection followed a reused access-point path"
        );
        assert!(
            !fixture.created.load(Ordering::SeqCst),
            "credentials reached the changed target"
        );
    }
}

pub(super) fn selected_request(
    mut snapshot: WifiSnapshot,
    secret: Option<&str>,
) -> WifiConnectRequest {
    let selected = snapshot.networks.remove(0);
    WifiConnectRequest {
        device_id: snapshot.device_id.unwrap(),
        network_id: Some(selected.id),
        label: selected.label,
        credential: secret.map(String::from),
        hidden: false,
    }
}

#[tokio::test]
async fn selected_daemon_owner_cannot_be_replaced_even_at_identical_paths() {
    let fixture = NmFixture::new(2, false).await;
    let request = fixture.request(Some("test-passphrase")).await;
    fixture
        ._service
        .release_name("org.freedesktop.NetworkManager")
        .await
        .unwrap();
    let replacement = fixture._bus.as_ref().unwrap().connect().await;
    replacement
        .request_name("org.freedesktop.NetworkManager")
        .await
        .unwrap();
    let error = fixture.backend.connect(request).await.unwrap_err();
    assert_eq!(error.category(), WifiBackendErrorCategory::TargetChanged);
    assert!(!fixture.created.load(Ordering::SeqCst));
}

#[tokio::test]
async fn same_network_roaming_preserves_selection_and_uses_current_access_point() {
    let fixture = NmFixture::new(2, false).await;
    let request = fixture.request(None).await;
    let roam = "/accesspoints/roamed";
    fixture
        ._service
        .object_server()
        .at(
            roam,
            AccessPoint {
                ssid: b"Home".to_vec(),
                rsn: 0x100,
            },
        )
        .await
        .unwrap();
    *fixture.aps.lock().unwrap() = vec![path(roam)];
    let snapshot = fixture.backend.snapshot().await.unwrap();
    assert_eq!(
        snapshot.networks[0].id,
        *request.network_id.as_ref().unwrap()
    );
    fixture.backend.connect(request).await.unwrap();
    assert_eq!(*fixture.selected_ap.lock().unwrap(), Some(path(roam)));
}

#[tokio::test]
async fn iwd_agent_revalidates_network_properties_device_and_owner_on_every_request() {
    let fixture = IwdFixture::new().await;
    let request = selected_request(
        fixture.backend.snapshot().await.unwrap(),
        Some("test-passphrase"),
    );
    let registration = IwdAgentRegistration::register(
        &fixture.backend.conn,
        &fixture.backend.owner,
        &request,
        &path(DEVICE),
        "Home",
        "psk",
        fixture.backend.cleanup.clone(),
    )
    .await
    .unwrap();
    ask_secret(&fixture.service, &fixture.address)
        .await
        .unwrap();
    let network = fixture
        .service
        .object_server()
        .interface::<_, IwdNetwork>(AP)
        .await
        .unwrap();
    for change in 0..3 {
        {
            let mut target = network.get_mut().await;
            target.name = if change == 0 { "Other" } else { "Home" }.into();
            target.kind = if change == 1 { "open" } else { "psk" }.into();
            target.device = path(if change == 2 { "/other_device" } else { DEVICE });
        }
        assert!(
            ask_secret(&fixture.service, &fixture.address)
                .await
                .is_err()
        );
    }
    network.get_mut().await.device = path(DEVICE);
    ask_secret(&fixture.service, &fixture.address)
        .await
        .unwrap();
    fixture
        .service
        .release_name("net.connman.iwd")
        .await
        .unwrap();
    let replacement = fixture.bus.connect().await;
    replacement.request_name("net.connman.iwd").await.unwrap();
    assert!(
        ask_secret(&fixture.service, &fixture.address)
            .await
            .is_err()
    );
    assert!(ask_secret(&replacement, &fixture.address).await.is_err());
    drop(registration);
    fixture.backend.cleanup.finish().await;
    assert!(
        fixture.address.lock().unwrap().is_none(),
        "cleanup must unregister at the original owner"
    );
}

#[tokio::test]
async fn open_credential_prompt_is_retired_when_selected_network_changes() {
    let fixture = IwdFixture::new().await;
    let mut provider = WifiPanelProvider::new(Adapter::Iwd(fixture.backend));
    provider.initial_content().await.unwrap();
    let selected = provider.snapshot.networks[0].id.clone();
    provider
        .handle_event(ProviderEvent::SelectableListEntryActivated {
            element_id: "networks".into(),
            entry_id: selected,
        })
        .await
        .unwrap();
    assert!(provider.credential_prompt.is_some());
    provider
        .handle_event(ProviderEvent::TextInputChanged {
            element_id: "credential".into(),
            value: "test-passphrase".into(),
        })
        .await
        .unwrap();
    fixture
        .service
        .object_server()
        .interface::<_, IwdNetwork>(AP)
        .await
        .unwrap()
        .get_mut()
        .await
        .name = "Other".into();
    let document = provider.synchronize().await.unwrap();
    assert!(provider.credential_prompt.is_none());
    assert!(!format!("{document:?}").contains("test-passphrase"));
    assert!(
        provider
            .handle_event(ProviderEvent::ButtonActivated {
                element_id: "connect".into()
            })
            .await
            .unwrap()
            .is_empty()
    );
    assert!(fixture.address.lock().unwrap().is_none());
}
