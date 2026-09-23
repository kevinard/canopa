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

type ReadingReceiver = mpsc::UnboundedReceiver<Result<(NetworkConnectivity, Option<String>), ()>>;
async fn expect(rx: &mut ReadingReceiver, connectivity: NetworkConnectivity, label: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let reading = rx.recv().await.unwrap().unwrap();
            if reading == (connectivity.clone(), Some(label.into())) {
                break;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn networkd_tracks_usable_links_while_manager_stays_online() {
    let bus = crate::test_bus::TestBus::new();
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
    let worker = tokio::spawn(run_networkd_task(bus.connect().await, tx));
    expect(&mut rx, NetworkConnectivity::Connected, "eth0").await;
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
        expect(&mut rx, NetworkConnectivity::Connected, expected).await;
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
    let bus = crate::test_bus::TestBus::new();
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
    let worker = tokio::spawn(run_nm_task(bus.connect().await, tx));
    expect(&mut rx, NetworkConnectivity::Unknown, "Wi-Fi").await;
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
    expect(&mut rx, NetworkConnectivity::Portal, "Wi-Fi").await;
    let active = service
        .object_server()
        .interface::<_, Active>("/Connection")
        .await
        .unwrap();
    let mut guard = active.get_mut().await;
    guard.id = "Renamed".into();
    guard.id_changed(active.signal_emitter()).await.unwrap();
    drop(guard);
    expect(&mut rx, NetworkConnectivity::Portal, "Renamed").await;
    worker.abort();
    let _ = worker.await;
}
