use super::*;
use crate::testing::{
    TestBus,
    wifi::{self, AP, AccessPoint, AgentAddress, DEVICE, IwdNetwork, ask_secret, path, request},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI64, Ordering},
};

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
        let fake = wifi::NetworkManager::serve(bus, state, delay_reply).await;
        let backend = NetworkManagerBackend::from_connection(bus.connect().await)
            .await
            .unwrap();
        Self {
            _bus: None,
            _service: fake.service,
            backend,
            created: fake.created,
            saved: fake.saved,
            deleted: fake.deleted,
            deactivated: fake.deactivated,
            observed: fake.observed,
            last: fake.last,
            aps: fake.aps,
            selected_ap: fake.selected_ap,
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
    assert_eq!(crate::network::iwd::strength(-5500), 45);
    assert_eq!(crate::network::iwd::strength(i16::MIN), 0);
    assert_eq!(nm_security(0x200, 1), WifiSecurity::Enterprise);
    assert_eq!(nm_security(0, 1), WifiSecurity::Enterprise);
    assert_eq!(nm_security(0, 0), WifiSecurity::Open);
    assert!(!format!("{:?}", request(Some("test-passphrase"))).contains("test-passphrase"));
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
        let fake = wifi::Iwd::serve(&bus).await;
        let backend = IwdBackend::from_connection(bus.connect().await)
            .await
            .unwrap();
        Self {
            bus,
            service: fake.service,
            address: fake.address,
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
        request.credential.clone(),
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
async fn networkmanager_radio_disconnect_and_forget() {
    let bus = TestBus::new();
    let fake = wifi::NetworkManager::serve(&bus, 2, false).await;
    let backend = NetworkManagerBackend::from_connection(bus.connect().await)
        .await
        .unwrap();
    assert_eq!(backend.radio().await.unwrap(), Some(true));
    backend.set_radio(false).await.unwrap();
    assert!(fake.radio_off.load(Ordering::SeqCst));
    assert_eq!(backend.radio().await.unwrap(), Some(false));
    let snapshot = backend.set_radio(true).await.unwrap();
    let device = snapshot.device_id.clone().unwrap();
    backend.disconnect(&device).await.unwrap();
    assert!(fake.disconnected.load(Ordering::SeqCst));
    assert_eq!(
        backend.disconnect("stale").await.unwrap_err().category(),
        WifiBackendErrorCategory::TargetChanged
    );
    backend
        .forget(selected_request(snapshot, None))
        .await
        .unwrap();
    assert!(fake.deleted.load(Ordering::SeqCst));
}
