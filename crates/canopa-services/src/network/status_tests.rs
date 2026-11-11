use super::*;
use zbus::zvariant::OwnedObjectPath;

struct Manager;
#[zbus::interface(name = "org.freedesktop.network1.Manager")]
impl Manager {
    #[zbus(property)]
    fn online_state(&self) -> &str {
        "online"
    }
    fn list_links(&self) -> Vec<(i32, String, OwnedObjectPath)> {
        ["lo", "disconnected", "virtual", "eth0", "wlan0"]
            .into_iter()
            .enumerate()
            .map(|(i, name)| {
                (
                    i as i32,
                    name.into(),
                    format!("/org/freedesktop/network1/link/_{i}")
                        .try_into()
                        .unwrap(),
                )
            })
            .collect()
    }
}

struct Link {
    online: &'static str,
    operational: &'static str,
}
#[zbus::interface(name = "org.freedesktop.network1.Link")]
impl Link {
    #[zbus(property)]
    fn online_state(&self) -> &str {
        self.online
    }
    #[zbus(property)]
    fn operational_state(&self) -> &str {
        self.operational
    }
}

type ReadingReceiver = mpsc::UnboundedReceiver<Result<NetworkStatus, ()>>;
async fn expect(rx: &mut ReadingReceiver, connectivity: Connectivity, label: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let reading = rx.recv().await.unwrap().unwrap();
            if reading
                == (NetworkStatus {
                    connectivity: connectivity.clone(),
                    label: Some(label.into()),
                    ..Default::default()
                })
            {
                break;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn networkd_tracks_usable_links_while_manager_stays_online() {
    let bus = crate::testing::TestBus::new();
    let service = bus.connect().await;
    service
        .object_server()
        .at("/org/freedesktop/network1", Manager)
        .await
        .unwrap();
    for (index, online, operational) in [
        (0, "online", "routable"),
        (1, "offline", "no-carrier"),
        (2, "offline", "carrier"),
        (3, "online", "routable"),
        (4, "offline", "off"),
    ] {
        service
            .object_server()
            .at(
                format!("/org/freedesktop/network1/link/_{index}"),
                Link {
                    online,
                    operational,
                },
            )
            .await
            .unwrap();
    }
    service
        .request_name("org.freedesktop.network1")
        .await
        .unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let worker = tokio::spawn(run_networkd_task(
        bus.connect().await,
        service.unique_name().unwrap().to_string(),
        tx,
    ));
    expect(&mut rx, Connectivity::Connected, "eth0").await;
    for (index, online, operational, expected) in [
        (4, "online", "routable", "eth0, wlan0"),
        (3, "offline", "no-carrier", "wlan0"),
    ] {
        let link = service
            .object_server()
            .interface::<_, Link>(format!("/org/freedesktop/network1/link/_{index}"))
            .await
            .unwrap();
        let mut guard = link.get_mut().await;
        guard.online = online;
        guard.operational = operational;
        guard
            .online_state_changed(link.signal_emitter())
            .await
            .unwrap();
        guard
            .operational_state_changed(link.signal_emitter())
            .await
            .unwrap();
        drop(guard);
        expect(&mut rx, Connectivity::Connected, expected).await;
    }
    worker.abort();
    let _ = worker.await;
}

struct NM {
    connectivity: u32,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl NM {
    #[zbus(property)]
    fn connectivity(&self) -> u32 {
        self.connectivity
    }
    #[zbus(property)]
    fn primary_connection(&self) -> OwnedObjectPath {
        "/Connection".try_into().unwrap()
    }
}
struct Active {
    id: String,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl Active {
    #[zbus(property)]
    fn id(&self) -> &str {
        &self.id
    }
}

#[tokio::test]
async fn nm_unknown_portal_and_profile_rename_are_distinct() {
    let bus = crate::testing::TestBus::new();
    let service = bus.connect().await;
    let path = "/org/freedesktop/NetworkManager";
    service
        .object_server()
        .at(path, NM { connectivity: 0 })
        .await
        .unwrap();
    service
        .object_server()
        .at("/Connection", Active { id: "Wi-Fi".into() })
        .await
        .unwrap();
    service
        .request_name("org.freedesktop.NetworkManager")
        .await
        .unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let worker = tokio::spawn(run_nm_task(
        bus.connect().await,
        service.unique_name().unwrap().to_string(),
        tx,
    ));
    expect(&mut rx, Connectivity::Unknown, "Wi-Fi").await;
    let manager = service
        .object_server()
        .interface::<_, NM>(path)
        .await
        .unwrap();
    let mut guard = manager.get_mut().await;
    guard.connectivity = 2;
    guard
        .connectivity_changed(manager.signal_emitter())
        .await
        .unwrap();
    drop(guard);
    expect(&mut rx, Connectivity::Portal, "Wi-Fi").await;
    let active = service
        .object_server()
        .interface::<_, Active>("/Connection")
        .await
        .unwrap();
    let mut guard = active.get_mut().await;
    guard.id = "Renamed".into();
    guard.id_changed(active.signal_emitter()).await.unwrap();
    drop(guard);
    expect(&mut rx, Connectivity::Portal, "Renamed").await;
    worker.abort();
    let _ = worker.await;
}

#[tokio::test]
async fn status_observer_switches_live_bundles_and_recovers_after_absence() {
    use crate::network::bundle::{Bundle, Kind, NETWORKD, NM as NM_NAME};
    let bus = crate::testing::TestBus::new();
    let client = bus.connect().await;
    let service = bus.connect().await;
    service
        .object_server()
        .at("/org/freedesktop/NetworkManager", NM { connectivity: 4 })
        .await
        .unwrap();
    service
        .object_server()
        .at(
            "/Connection",
            Active {
                id: "Primary".into(),
            },
        )
        .await
        .unwrap();
    service
        .object_server()
        .at("/org/freedesktop/network1", Manager)
        .await
        .unwrap();
    for index in 0..5 {
        service
            .object_server()
            .at(
                format!("/org/freedesktop/network1/link/_{index}"),
                Link {
                    online: if index == 3 { "online" } else { "offline" },
                    operational: "routable",
                },
            )
            .await
            .unwrap();
    }
    let (tx, mut rx) = mpsc::unbounded_channel();
    let worker = tokio::spawn(observe(client.clone(), tx));
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    service.request_name(NETWORKD).await.unwrap();
    expect(&mut rx, Connectivity::Connected, "eth0").await;
    assert_eq!(
        Bundle::resolve(&client).await.unwrap().status.unwrap().kind,
        Kind::Networkd
    );
    service.request_name(NM_NAME).await.unwrap();
    expect(&mut rx, Connectivity::Full, "Primary").await;
    let bundle = Bundle::resolve(&client).await.unwrap();
    assert_eq!(bundle.status, bundle.control);
    service.release_name(NM_NAME).await.unwrap();
    expect(&mut rx, Connectivity::Connected, "eth0").await;
    service.release_name(NETWORKD).await.unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    service.request_name(NM_NAME).await.unwrap();
    expect(&mut rx, Connectivity::Full, "Primary").await;
    drop(rx);
    tokio::time::timeout(std::time::Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn read_and_watch_report_absence_as_a_state() {
    use futures_util::StreamExt;
    let bus = crate::testing::TestBus::new();
    let client = bus.connect().await;
    assert_eq!(read(&client).await, Ok(ServiceState::Unavailable));
    let mut states = watch(client.clone());
    let next = async |states: &mut crate::bus::StateStream<NetworkStatus>| {
        tokio::time::timeout(std::time::Duration::from_secs(3), states.next())
            .await
            .unwrap()
            .unwrap()
    };
    assert_eq!(next(&mut states).await, ServiceState::Unavailable);
    let service = bus.connect().await;
    service
        .object_server()
        .at("/org/freedesktop/NetworkManager", NM { connectivity: 4 })
        .await
        .unwrap();
    service
        .object_server()
        .at("/Connection", Active { id: "Home".into() })
        .await
        .unwrap();
    service
        .request_name("org.freedesktop.NetworkManager")
        .await
        .unwrap();
    let expected = NetworkStatus {
        connectivity: Connectivity::Full,
        label: Some("Home".into()),
        ..Default::default()
    };
    assert_eq!(
        next(&mut states).await,
        ServiceState::Available(expected.clone())
    );
    assert_eq!(read(&client).await, Ok(ServiceState::Available(expected)));
}

#[tokio::test]
async fn networkmanager_status_names_the_primary_kind_and_active_vpns() {
    use crate::testing::{TestBus, network::FakeNetworkManager};
    let bus = TestBus::new();
    let nm = FakeNetworkManager::serve(&bus).await;
    let client = bus.connect().await;
    let overview = crate::network::manager::read(&client).await.unwrap();
    let vpn = crate::network::manager::Target::from(
        overview.connection(crate::testing::network::VPN).unwrap(),
    );
    crate::network::manager::activate(&client, &vpn, "/", "/")
        .await
        .unwrap();
    let crate::ServiceState::Available(status) = read(&client).await.unwrap() else {
        panic!("status expected");
    };
    assert_eq!(status.kind, Some(crate::network::NetworkKind::Wired));
    assert_eq!(status.label.as_deref(), Some("Wired"));
    assert_eq!(status.vpn, ["Office VPN"]);
    drop(nm);
}
